//! Hypergraph node types (ONTOLOGY-DESIGN Layer 6).
//!
//! Groups entities into higher-level concepts, method families, and
//! benchmark suites. Enables hypergraph queries and GNN community detection.

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

/// Known cluster types (closed vocabulary).
pub const CLUSTER_CONCEPT: &str = "concept_cluster";
pub const CLUSTER_METHOD_FAMILY: &str = "method_family";
pub const CLUSTER_BENCHMARK_SUITE: &str = "benchmark_suite";

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
    fn test_cluster_type_constants() {
        assert_eq!(CLUSTER_CONCEPT, "concept_cluster");
        assert_eq!(CLUSTER_METHOD_FAMILY, "method_family");
        assert_eq!(CLUSTER_BENCHMARK_SUITE, "benchmark_suite");
    }
}
