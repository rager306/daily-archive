//! ConceptCluster node — derived semantic community (ONTOLOGY-DESIGN Layer 6).
//!
//! IMPORTANT: ConceptCluster is NOT an evidence unit, NOT a claim, NOT a
//! reasoning step. It is a derived community object for concept communities,
//! method families, and benchmark suites.
//! Source-grounded evidence requires EvidenceBundle (future P1 work).

use crate::schema::{FieldType, NodeSchemaDef};

/// ConceptCluster node — GNN-derived or manual grouping of related entities.
/// Example: {PPO, DPO, GRPO, GEPA} → "RL optimization methods"
pub struct ConceptClusterSchema;

impl NodeSchemaDef for ConceptClusterSchema {
    fn label(&self) -> &'static str {
        "ConceptCluster"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("label", FieldType::String),
            ("cluster_type", FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("description", FieldType::String),
            ("member_count", FieldType::Integer),
            ("embedding", FieldType::Vector),
            ("retrieval_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

// Cluster type vocabulary moved to data/node_vocabulary.yaml.
// Use crate::vocabulary::is_known_cluster_type() for validation.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_concept_cluster_schema_label() {
        assert_eq!(ConceptClusterSchema.label(), "ConceptCluster");
    }

    #[test]
    fn test_concept_cluster_required_fields() {
        let fields = ConceptClusterSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"vid"));
        assert!(names.contains(&"label"));
        assert!(names.contains(&"cluster_type"));
    }

    #[test]
    fn test_concept_cluster_has_embedding() {
        let fields = ConceptClusterSchema.optional_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"embedding"),
            "ConceptClusterSchema must have embedding for GNN, got: {names:?}"
        );
    }

    #[test]
    fn test_concept_cluster_validates() {
        let schema = ConceptClusterSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::json!("vid:hyper:rl-methods"));
        props.insert(
            "label".to_string(),
            serde_json::json!("RL optimization methods"),
        );
        props.insert(
            "cluster_type".to_string(),
            serde_json::json!("method_family"),
        );
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_cluster_types_from_registry() {
        assert!(crate::vocabulary::is_known_cluster_type("concept_cluster"));
        assert!(crate::vocabulary::is_known_cluster_type("method_family"));
        assert!(crate::vocabulary::is_known_cluster_type("benchmark_suite"));
    }
}
