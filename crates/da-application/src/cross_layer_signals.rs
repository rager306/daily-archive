//! Cross-layer consistency signals (AutoResearchEval Insight 1):
//! «улики, опровергающие отказ, уже в run directory — сравнение не
//! выполняется». Здесь сравнение выполняется: summary↔parsed,
//! extracted↔equations. Diagnostic (ACAP), не hard invariants.

use da_domain::summaries_llm::grounding_score;
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum CrossLayerError {
    #[error("cross-layer: {0}")]
    Store(#[from] SchemaObjectStoreError),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CrossLayerSignals {
    pub summary_parsed_grounding: Option<f64>,
    pub extracted_equations_consistency: Option<f64>,
}

pub fn compute_cross_layer(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
) -> Result<CrossLayerSignals, CrossLayerError> {
    let summary_g = match (
        read_json(store, paper_id, "summaries/abstract.json"),
        read_json(store, paper_id, "parsed/article.json"),
    ) {
        (Ok(Some(sum)), Ok(Some(parsed))) => {
            let tldr = sum.get("tldr").and_then(|v| v.as_str()).unwrap_or("");
            let body = parsed
                .get("body_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if tldr.is_empty() || body.is_empty() {
                None
            } else {
                Some((grounding_score(tldr, body) * 1000.0).round() / 1000.0)
            }
        }
        _ => None,
    };

    let eq_consistency = match (
        read_json(store, paper_id, "extracted/entities.json"),
        read_json(store, paper_id, "equations/index.json"),
    ) {
        (Ok(Some(ents)), Ok(Some(eqs))) => {
            let labels: Vec<String> = ents
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.get("label").and_then(|l| l.as_str()))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            if labels.is_empty() {
                None
            } else {
                let eq_text = eqs
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default()
                    .to_lowercase();
                let hit = labels
                    .iter()
                    .filter(|l| l.len() > 3 && eq_text.contains(&l.to_lowercase()))
                    .count();
                Some((hit as f64 / labels.len() as f64 * 1000.0).round() / 1000.0)
            }
        }
        _ => None,
    };

    Ok(CrossLayerSignals {
        summary_parsed_grounding: summary_g,
        extracted_equations_consistency: eq_consistency,
    })
}

fn read_json(
    store: &dyn SchemaObjectStore,
    id: &str,
    rel: &str,
) -> Result<Option<serde_json::Value>, SchemaObjectStoreError> {
    match store.read_raw(id, rel)? {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| SchemaObjectStoreError::Io(format!("{id}/{rel}: {e}"))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct FakeStore(BTreeMap<(String, String), String>);
    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str {
            "article"
        }
        fn norm(&self) -> Result<da_domain::schema_invariants::ObjectNorm, SchemaObjectStoreError> {
            unimplemented!()
        }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> {
            Ok(vec![])
        }
        fn read_object(
            &self,
            _id: &str,
        ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn read_collection(
            &self,
            _id: &str,
            _rel: &str,
        ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> {
            Ok(true)
        }
        fn file_backed_checks(
            &self,
        ) -> Box<dyn da_domain::schema_invariants::FileBackedChecks + '_> {
            unimplemented!()
        }
        fn read_raw(&self, id: &str, rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(self.0.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    #[test]
    fn grounded_summary_scores_across_layers() {
        let body = "The teacher-student model derives exact ODEs for learning dynamics and shows compositional generalization is fast after primitive pre-training in the high-dimensional limit.";
        let mut files = BTreeMap::new();
        files.insert(("p".to_string(), "summaries/abstract.json".to_string()),
            r#"{"tldr":"Exact ODEs govern teacher-student learning; compositional generalization is fast after pre-training."}"#.to_string());
        files.insert(
            ("p".to_string(), "parsed/article.json".to_string()),
            format!(
                r#"{{"body_text":{}}}"#,
                serde_json::to_string(body).unwrap()
            ),
        );
        files.insert(("p".to_string(), "extracted/entities.json".to_string()),
            r#"[{"label":"teacher-student"},{"label":"compositional generalization"},{"label":"nonexistent"}]"#.to_string());
        files.insert(
            ("p".to_string(), "equations/index.json".to_string()),
            r#"[{"representation":{"plain_text":"teacher-student framework ODE"}}]"#.to_string(),
        );
        let store = FakeStore(files);
        let out = compute_cross_layer(&store, "p").unwrap();
        let g = out.summary_parsed_grounding.unwrap();
        assert!(g > 0.5, "grounding {g}");
        let eq = out.extracted_equations_consistency.unwrap();
        assert!((eq - 0.333).abs() < 0.01, "eq {eq}");
    }
}
