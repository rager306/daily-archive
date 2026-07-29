//! Entity types (ADR-038 Module B + legacy ADR-028 typed schema).
//!
//! Two families (GRAPH-SCHEMA.md):
//! - Concrete: Method, Dataset, Metric, Task, Model, Figure, Table,
//!   Equation, Concept, Implementation, Theorem, Definition
//! - Abstract: Problem, Motivation, Gap, Contribution, Hypothesis, Finding,
//!   Mechanism, Limitation, FutureWork

use crate::evidence::SourceSpan;
use crate::vid::Vid;
use serde::{Deserialize, Serialize};

/// Entity types we extract from papers.
/// Closed vocabulary (ADR-028 §2.1 + ADR-038 Module B).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EntityType {
    // Concrete (Module B)
    Task,
    Method,
    Dataset,
    Model,
    Metric,
    Figure,
    Table,
    Equation,
    Concept,
    Implementation,
    Theorem,
    Definition,
    // Abstract (Module C — ADR-028)
    Problem,
    Motivation,
    Gap,
    Contribution,
    Hypothesis,
    Finding,
    Mechanism,
    Limitation,
    FutureWork,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Task => "Task",
            EntityType::Method => "Method",
            EntityType::Dataset => "Dataset",
            EntityType::Model => "Model",
            EntityType::Metric => "Metric",
            EntityType::Figure => "Figure",
            EntityType::Table => "Table",
            EntityType::Equation => "Equation",
            EntityType::Concept => "Concept",
            EntityType::Implementation => "Implementation",
            EntityType::Theorem => "Theorem",
            EntityType::Definition => "Definition",
            EntityType::Problem => "Problem",
            EntityType::Motivation => "Motivation",
            EntityType::Gap => "Gap",
            EntityType::Contribution => "Contribution",
            EntityType::Hypothesis => "Hypothesis",
            EntityType::Finding => "Finding",
            EntityType::Mechanism => "Mechanism",
            EntityType::Limitation => "Limitation",
            EntityType::FutureWork => "FutureWork",
        }
    }
}

/// An entity extracted from a paper (Module B).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub vid: Vid,
    pub label: String,
    pub entity_type: EntityType,
    pub description: Option<String>,
    pub source_spans: Vec<SourceSpan>,
    pub confidence: f32,
    pub schema_version: u32,
}

/// Schema definition for Entity nodes (ADR-040 §11.1).
pub struct EntitySchema;

impl crate::schema::NodeSchemaDef for EntitySchema {
    fn label(&self) -> &'static str {
        "Entity"
    }

    fn required_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("vid", crate::schema::FieldType::String),
            ("label", crate::schema::FieldType::String),
            ("entity_type", crate::schema::FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("section", crate::schema::FieldType::String),
            ("char_start", crate::schema::FieldType::Integer),
            ("char_end", crate::schema::FieldType::Integer),
            ("surface", crate::schema::FieldType::String),
            ("description", crate::schema::FieldType::String),
            ("confidence", crate::schema::FieldType::Float),
            ("valid_from", crate::schema::FieldType::DateTime),
            ("schema_version", crate::schema::FieldType::Integer),
            ("evidence_ready", crate::schema::FieldType::Boolean),
            ("import_eligible", crate::schema::FieldType::Boolean),
            ("retrieval_eligible", crate::schema::FieldType::Boolean),
            // Phase 3: GNN readiness — entity label embedding (bge-m3 1024d)
            ("embedding", crate::schema::FieldType::Vector),
            ("domain_tags", crate::schema::FieldType::String),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_roundtrip() {
        let t = EntityType::Method;
        let json = serde_json::to_string(&t).unwrap();
        let back: EntityType = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_entity_schema_has_embedding_field() {
        // Phase 3 GNN readiness: Entity nodes must support vector embeddings
        use crate::schema::NodeSchemaDef;
        let schema = EntitySchema;
        let optional = schema.optional_fields();
        let names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"embedding"),
            "EntitySchema must have 'embedding' optional field for GNN readiness, got: {names:?}"
        );
        // Verify it's Vector type
        let embedding_field = optional.iter().find(|(n, _)| *n == "embedding");
        assert!(embedding_field.is_some(), "embedding field must exist");
        let (_, ftype) = embedding_field.unwrap();
        assert_eq!(
            *ftype,
            crate::schema::FieldType::Vector,
            "embedding field must be Vector type"
        );
    }

    #[test]
    fn test_entity_schema_has_domain_tags() {
        // Cross-domain support: Entity nodes carry domain_tags for filtering
        use crate::schema::NodeSchemaDef;
        let schema = EntitySchema;
        let optional = schema.optional_fields();
        let names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"domain_tags"),
            "EntitySchema must have 'domain_tags' for cross-domain support"
        );
    }
}
