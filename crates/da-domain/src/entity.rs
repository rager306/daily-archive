//! Entity types (ADR-038 Module B + legacy ADR-028 typed schema).
//!
//! Two families (GRAPH-SCHEMA.md):
//! - Concrete: Method, Dataset, Metric, Task, Baseline, Model, Figure, Table,
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
    Baseline,
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
            EntityType::Baseline => "Baseline",
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

    /// All entity types (closed vocabulary).
    pub fn all() -> &'static [EntityType] {
        &[
            EntityType::Task,
            EntityType::Method,
            EntityType::Dataset,
            EntityType::Model,
            EntityType::Metric,
            EntityType::Baseline,
            EntityType::Figure,
            EntityType::Table,
            EntityType::Equation,
            EntityType::Concept,
            EntityType::Implementation,
            EntityType::Theorem,
            EntityType::Definition,
            EntityType::Problem,
            EntityType::Motivation,
            EntityType::Gap,
            EntityType::Contribution,
            EntityType::Hypothesis,
            EntityType::Finding,
            EntityType::Mechanism,
            EntityType::Limitation,
            EntityType::FutureWork,
        ]
    }

    /// Is this a concrete (Module B) entity type?
    pub fn is_concrete(&self) -> bool {
        matches!(
            self,
            EntityType::Task
                | EntityType::Method
                | EntityType::Dataset
                | EntityType::Model
                | EntityType::Metric
                | EntityType::Baseline
                | EntityType::Figure
                | EntityType::Table
                | EntityType::Equation
                | EntityType::Concept
                | EntityType::Implementation
                | EntityType::Theorem
                | EntityType::Definition
        )
    }

    /// Is this an abstract (Module C) entity type?
    pub fn is_abstract(&self) -> bool {
        !self.is_concrete()
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
