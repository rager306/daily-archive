//! Entity types (ADR-038 Module B — Textually Mentioned).

use crate::evidence::SourceSpan;
use crate::vid::Vid;
use serde::{Deserialize, Serialize};

/// Entity types we extract from papers.
/// ADR-038 §2 Module B: Task, Method, Dataset, Model, Metric, Baseline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EntityType {
    Task,
    Method,
    Dataset,
    Model,
    Metric,
    Baseline,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Task => "Task",
            EntityType::Method => "Method",
            EntityType::Dataset => "Dataset",
            EntityType::Model => "Model",
            EntityType::Metric => "Metric",
            EntityType::Baseline => "Baseline",
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
            ("description", crate::schema::FieldType::String),
            ("confidence", crate::schema::FieldType::Float),
            ("schema_version", crate::schema::FieldType::Integer),
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
}
