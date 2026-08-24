//! Enriched layer stage (M451 phase-2 port #7): OpenAlex work anchor +
//! positional author binding, over INJECTED work observations (the
//! reference fetcher cache: artifacts/pipeline_cache/openalex_works/).
//! ADR-055 lineage: provenance carries endpoint/fetched_at/status/cache_key.

use da_domain::enriched_authors::{bind_authors, parse_tei_authors, EnrichedAuthor, WorkAuthorship};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum EnrichedLayerError {
    #[error("enriched stage: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("enriched stage: parsed unreadable for {0}")]
    NoParsed(String),
    #[error("enriched stage: work observation invalid: {0}")]
    Work(String),
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct WorkObservation {
    pub http_status: u16,
    #[serde(default)]
    pub fetched_at: String,
    #[serde(default)]
    pub work: Option<serde_json::Value>,
    #[serde(default)]
    pub terminal: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct EnrichedLayerOutput {
    pub paper_id: String,
    pub authors_json: String,
    pub venue_json: String,
    pub topics_json: String,
    pub provenance_json: String,
    pub stats: serde_json::Value,
    pub not_found: bool,
}

pub fn build_enriched_layer(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
    work_obs_json: &str,
) -> Result<EnrichedLayerOutput, EnrichedLayerError> {
    let obs: WorkObservation = serde_json::from_str(work_obs_json)
        .map_err(|e| EnrichedLayerError::Work(e.to_string()))?;

    let raw = store
        .read_raw(paper_id, "parsed/article.json")?
        .ok_or_else(|| EnrichedLayerError::NoParsed(paper_id.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| EnrichedLayerError::NoParsed(format!("{paper_id}: {e}")))?;
    let tei = parsed.get("tei_xml").and_then(|v| v.as_str()).unwrap_or("");
    let tei_authors = parse_tei_authors(tei);

    let endpoint = "api.openalex.org/works/doi:10.48550/arXiv.{pid}".to_string();
    let prov = serde_json::json!({
        "endpoint": endpoint.replace("{pid}", paper_id),
        "fetched_at": obs.fetched_at,
        "http_status": obs.http_status,
        "cache_key": format!("openalex_work:{paper_id}"),
    });

    let Some(work) = obs.work else {
        return Ok(EnrichedLayerOutput {
            paper_id: paper_id.to_string(),
            not_found: true,
            authors_json: String::new(),
            venue_json: String::new(),
            topics_json: String::new(),
            provenance_json: serde_json::to_string_pretty(&prov).unwrap(),
            stats: serde_json::json!({"missingness": "openalex_not_found"}),
        });
    };

    // authorships (slim cache shape: a_id/display_name/orcid/institutions)
    let authorships: Vec<WorkAuthorship> = serde_json::from_value(
        work.get("authorships").cloned().unwrap_or_default(),
    )
    .map_err(|e| EnrichedLayerError::Work(format!("authorships: {e}")))?;
    let bound: Vec<EnrichedAuthor> = bind_authors(&tei_authors, &authorships);

    let venue = work.get("source").cloned().unwrap_or(serde_json::Value::Null);
    let topics: Vec<serde_json::Value> = serde_json::from_value(
        work.get("topics").cloned().unwrap_or_default(),
    )
    .unwrap_or_default();

    let with_a = bound.iter().filter(|a| a.openalex_a_id.is_some()).count();
    let with_orcid = bound.iter().filter(|a| a.orcid.is_some()).count();
    let confirmed = bound.iter().filter(|a| a.binding == "position_surname_confirmed").count();
    let mismatch = bound.iter().filter(|a| a.binding == "position_surname_mismatch").count();
    let mut prov_full = prov;
    prov_full["work_id"] = work.get("work_id").cloned().unwrap_or_default();

    Ok(EnrichedLayerOutput {
        paper_id: paper_id.to_string(),
        not_found: false,
        authors_json: serde_json::to_string_pretty(&bound).unwrap(),
        venue_json: serde_json::to_string_pretty(&venue).unwrap(),
        topics_json: serde_json::to_string_pretty(&topics).unwrap(),
        provenance_json: serde_json::to_string_pretty(&prov_full).unwrap(),
        stats: serde_json::json!({
            "authors": bound.len(),
            "with_a_id": with_a,
            "with_orcid": with_orcid,
            "surname_confirmed": confirmed,
            "binding_mismatch": mismatch,
            "topics": topics.len(),
            "cited_by_count": work.get("cited_by_count").cloned().unwrap_or_default(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct FakeStore(BTreeMap<(String, String), String>);
    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str { "article" }
        fn norm(&self) -> Result<da_domain::schema_invariants::ObjectNorm, SchemaObjectStoreError> { unimplemented!() }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> { Ok(vec![]) }
        fn read_object(&self, _id: &str) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> { Ok(None) }
        fn read_collection(&self, _id: &str, _rel: &str) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> { Ok(None) }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> { Ok(true) }
        fn file_backed_checks(&self) -> Box<dyn da_domain::schema_invariants::FileBackedChecks + '_> { unimplemented!() }
        fn read_raw(&self, id: &str, rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(self.0.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    #[test]
    fn stage_binds_and_reports_not_found() {
        let tei = r#"<fileDesc><persName><forename>Jin</forename><surname>Lee</surname></persName></fileDesc>"#;
        let store = FakeStore(BTreeMap::from([(
            ("p".into(), "parsed/article.json".into()),
            format!(r#"{{"tei_xml":{}}}"#, serde_json::to_string(tei).unwrap()),
        )]));
        let ok = serde_json::json!({
            "http_status": 200, "fetched_at": "t",
            "work": {"work_id": "W1", "cited_by_count": 5,
                     "authorships": [{"a_id": "A1", "display_name": "Jin Lee", "orcid": "0", "institutions": []}],
                     "source": {"id": "S1", "display_name": "NeurIPS"}, "topics": [{"id": "T1"}]}
        });
        let out = build_enriched_layer(&store, "p", ok.to_string().as_str()).unwrap();
        assert!(!out.not_found);
        assert!(out.stats["authors"] == 1);
        assert!(out.stats["surname_confirmed"] == 1);
        assert!(out.authors_json.contains("position_surname_confirmed"));

        let nf = r#"{"http_status": 404, "terminal": true, "work": null}"#;
        let out2 = build_enriched_layer(&store, "p", nf).unwrap();
        assert!(out2.not_found);
        assert_eq!(out2.stats["missingness"], "openalex_not_found");
    }
}
