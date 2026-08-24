//! TEI figures stage (M451 phase-2 port #4): untyped TEI figures ->
//! figures/index.json for arxiv_pdf_dot objects (parsed carries tei_xml).

use da_domain::tei_figures::{parse_tei_figures, TeiFigure, TeiFiguresStats};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum TeiFiguresStageError {
    #[error("tei figures stage: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("tei figures stage: parsed unreadable for {0}")]
    NoParsed(String),
    #[error("tei figures stage: parse failed: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct TeiFiguresStageOutput {
    pub paper_id: String,
    pub figures: Vec<TeiFigure>,
    pub index_json: String,
    pub stats: TeiFiguresStats,
}

pub fn build_tei_figures_stage(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
) -> Result<TeiFiguresStageOutput, TeiFiguresStageError> {
    let raw = store
        .read_raw(paper_id, "parsed/article.json")?
        .ok_or_else(|| TeiFiguresStageError::NoParsed(paper_id.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| TeiFiguresStageError::NoParsed(format!("{paper_id}: {e}")))?;
    let tei = parsed
        .get("tei_xml")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (figures, stats) = parse_tei_figures(&tei)
        .map_err(|e| TeiFiguresStageError::Parse(e.to_string()))?;
    let index_json = serde_json::to_string_pretty(&figures)
        .map_err(|e| TeiFiguresStageError::Parse(format!("serialize: {e}")))?;
    Ok(TeiFiguresStageOutput {
        paper_id: paper_id.to_string(),
        figures,
        index_json,
        stats,
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
    fn stage_reads_tei_from_parsed() {
        let tei = r##"<figure xml:id="fig_0"><figDesc>C.</figDesc></figure><ref type="figure" target="#fig_0">x</ref>"##;
        let store = FakeStore(BTreeMap::from([(
            ("a".into(), "parsed/article.json".into()),
            format!(r#"{{"tei_xml":{}}}"#, serde_json::to_string(tei).unwrap()),
        )]));
        let out = build_tei_figures_stage(&store, "a").unwrap();
        assert_eq!(out.stats.count, 1);
        assert_eq!(out.stats.total_in_text_refs, 1);
        assert!(out.index_json.contains("fig_0"));
    }
}
