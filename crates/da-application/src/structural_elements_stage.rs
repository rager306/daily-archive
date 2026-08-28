//! Structural elements stage (M451 phase-2): detect LaTeX-environment
//! kinds in parsed sections; DATA detectors; output for the CLI to
//! persist (context/structural_elements.json + manifest artifact entry).

use da_domain::structural_elements::{
    SectionInput, StructuralDetectors, StructuralElement, detect_all,
};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum StructuralStageError {
    #[error("structural stage: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("structural stage: parsed unreadable for {0}")]
    NoParsed(String),
    #[error("structural stage: detectors DATA invalid: {0}")]
    Detectors(String),
}

#[derive(Debug, Clone)]
pub struct StructuralStageOutput {
    pub paper_id: String,
    pub elements: Vec<StructuralElement>,
    pub elements_json: String,
    pub by_kind: std::collections::BTreeMap<String, usize>,
    pub definition_sources: usize,
}

pub fn build_structural_elements(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
    detectors_yaml: &str,
) -> Result<StructuralStageOutput, StructuralStageError> {
    let detectors = StructuralDetectors::from_yaml_str(detectors_yaml)
        .map_err(|e| StructuralStageError::Detectors(e.to_string()))?;

    let raw = store
        .read_raw(paper_id, "parsed/article.json")?
        .ok_or_else(|| StructuralStageError::NoParsed(paper_id.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| StructuralStageError::NoParsed(format!("{paper_id}: {e}")))?;
    let sections: Vec<SectionInput> =
        serde_json::from_value(parsed.get("sections").cloned().unwrap_or_default())
            .map_err(|e| StructuralStageError::NoParsed(format!("{paper_id}: sections: {e}")))?;

    let elements = detect_all(&detectors, &sections);
    let mut by_kind = std::collections::BTreeMap::new();
    let mut defs = 0;
    for e in &elements {
        *by_kind.entry(e.kind.clone()).or_insert(0) += 1;
        if e.definition_source {
            defs += 1;
        }
    }
    let elements_json = serde_json::to_string_pretty(&elements)
        .map_err(|e| StructuralStageError::NoParsed(format!("{paper_id}: serialize: {e}")))?;
    Ok(StructuralStageOutput {
        paper_id: paper_id.to_string(),
        elements,
        elements_json,
        by_kind,
        definition_sources: defs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct FakeStore {
        files: BTreeMap<(String, String), String>,
    }
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
            Ok(self.files.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    const DETECTORS: &str = "kinds:\n  definition: {pattern: '^Definition \\d+\\.\\d+', definition_source: true}\nextraction:\n  numbering_capture: '\\d+\\.\\d+'\n";

    #[test]
    fn stage_detects_from_parsed_sections() {
        let store = FakeStore {
            files: BTreeMap::from([(
                ("m1".into(), "parsed/article.json".into()),
                r#"{"sections":[{"section_id":"s1","page_start":12,"text":"Definition 1.2. A set is...\nplain line"}]}"#.to_string(),
            )]),
        };
        let out = build_structural_elements(&store, "m1", DETECTORS).unwrap();
        assert_eq!(out.elements.len(), 1);
        assert_eq!(out.by_kind.get("definition"), Some(&1));
        assert_eq!(out.definition_sources, 1);
        assert!(out.elements_json.contains("\"kind\": \"definition\""));
    }
}
