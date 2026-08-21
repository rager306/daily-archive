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

    /// All closed-vocabulary entity types (ADR-028 + ADR-038).
    pub fn all() -> &'static [EntityType] {
        &ENTITY_TYPES
    }
}

/// All 21 adopted entity types (concrete + abstract).
pub const ENTITY_TYPES: [EntityType; 21] = [
    EntityType::Task,
    EntityType::Method,
    EntityType::Dataset,
    EntityType::Model,
    EntityType::Metric,
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
];

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
            // Healing fields (set by GraphHealingUseCase on silence/correct/merge)
            ("deprecated_reason", crate::schema::FieldType::String),
            ("last_healed_at", crate::schema::FieldType::DateTime),
            // Canonical-resolution audit: points to the retained Entity VID.
            ("canonical_vid", crate::schema::FieldType::String),
            // Phase 3: GNN readiness — entity label embedding (bge-m3 1024d)
            ("embedding", crate::schema::FieldType::Vector),
            // M418/M429: embedding_sha256 stamped beside every vector write
            // so the Unchanged fast-path can verify replay integrity.
            ("embedding_sha256", crate::schema::FieldType::String),
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

    #[test]
    fn test_entity_types_count_21() {
        assert_eq!(ENTITY_TYPES.len(), 21);
        assert_eq!(EntityType::all().len(), 21);
    }
}

// ─── ADR-051 P1: Property-based tests (M354) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    fn arb_entity_type() -> impl Strategy<Value = EntityType> {
        (0usize..ENTITY_TYPES.len()).prop_map(|i| ENTITY_TYPES[i].clone())
    }

    proptest! {
        /// Property 1: as_str is never empty for any entity type.
        #[test]
        fn prop_as_str_nonempty(t in arb_entity_type()) {
            prop_assert!(!t.as_str().is_empty());
        }

        /// Property 2: serde JSON roundtrip preserves EntityType.
        #[test]
        fn prop_serde_roundtrip(t in arb_entity_type()) {
            let json = serde_json::to_string(&t).expect("serialize");
            let back: EntityType = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(t, back);
            // PascalCase wire: no spaces, starts with uppercase letter
            prop_assert!(json.starts_with('"') && json.ends_with('"'));
            let inner = &json[1..json.len() - 1];
            prop_assert!(!inner.is_empty());
            prop_assert!(inner.chars().next().unwrap().is_ascii_uppercase());
        }

        /// Property 3: as_str equals serde wire string (PascalCase).
        #[test]
        fn prop_as_str_matches_serde_wire(t in arb_entity_type()) {
            let json = serde_json::to_string(&t).unwrap();
            let wire = json.trim_matches('"');
            prop_assert_eq!(t.as_str(), wire);
        }
    }

    #[test]
    fn prop_all_as_str_unique() {
        let mut seen = HashSet::new();
        for t in EntityType::all() {
            assert!(seen.insert(t.as_str()), "duplicate as_str: {}", t.as_str());
        }
        assert_eq!(seen.len(), 21);
    }
}
