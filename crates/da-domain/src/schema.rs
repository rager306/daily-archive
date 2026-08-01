//! Schema-as-code framework (ADR-040 §11.1).
//!
//! Schema enforcement lives in Rust types, not DDL.
//! Every node type implements NodeSchemaDef, generating:
//! - validate(): pre-write check
//! - to_properties(): Rust → PropertyValue map
//! - from_properties(): PropertyValue → Rust (with type checking)

use thiserror::Error;

/// Schema validation error.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Missing required field: {0}")]
    MissingRequired(&'static str),

    #[error("Type mismatch on field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },

    #[error("Invalid value for field '{field}': {reason}")]
    InvalidValue { field: &'static str, reason: String },
}

/// Declared field type for schema validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Vector,
    Any,
}

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Integer => "integer",
            FieldType::Float => "float",
            FieldType::Boolean => "boolean",
            FieldType::DateTime => "datetime",
            FieldType::Vector => "vector",
            FieldType::Any => "any",
        }
    }
}

/// A field in a schema definition.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: &'static str,
    pub field_type: FieldType,
    pub required: bool,
    pub default: Option<&'static str>,
}

impl Field {
    pub fn string(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::String,
            required: true,
            default: None,
        }
    }
    pub fn integer(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Integer,
            required: true,
            default: None,
        }
    }
    pub fn float(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Float,
            required: true,
            default: None,
        }
    }
    pub fn boolean(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Boolean,
            required: true,
            default: None,
        }
    }
    pub fn datetime(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::DateTime,
            required: true,
            default: None,
        }
    }
}

/// Trait for schema definitions (ADR-040 §11.1).
/// Each node type implements this to define its schema.
pub trait NodeSchemaDef {
    /// The Cypher label for this node type.
    fn label(&self) -> &'static str;

    /// Required fields (must be present and non-null).
    fn required_fields(&self) -> Vec<(&'static str, FieldType)>;

    /// Optional fields (may be absent or null).
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![]
    }

    /// Validate a property map against this schema.
    fn validate(
        &self,
        props: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), SchemaError> {
        for (name, _fty) in self.required_fields() {
            match props.get(&name.to_string()) {
                None => return Err(SchemaError::MissingRequired(name)),
                Some(serde_json::Value::Null) => return Err(SchemaError::MissingRequired(name)),
                _ => {}
            }
        }
        Ok(())
    }
}

/// Current schema version (ADR-040 §11.2).
/// Incremented when schema changes require migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Registry of all node schema definitions (GRAPH-SCHEMA.md).
/// Loading code validates against these before writing.
pub fn all_node_schemas() -> Vec<Box<dyn NodeSchemaDef>> {
    vec![
        // ─── Publication plane ───
        Box::new(crate::paper::PaperSchema),
        Box::new(crate::article::SectionSchema),
        Box::new(crate::article::AuthorSchema),
        Box::new(crate::article::InstitutionSchema),
        Box::new(crate::article::ConceptSchema),
        Box::new(crate::article::TopicSchema),
        Box::new(crate::article::CategorySchema),
        Box::new(crate::article::ReferenceSchema),
        Box::new(crate::relation::CitationSchema),
        Box::new(crate::entity::EntitySchema),
        Box::new(crate::source::SourceSchema),
        // ─── Operational State (Layer 1) ───
        Box::new(crate::scheduler::SchedulerTaskSchema),
        // ─── Evidence & Community (Layer 6) ───
        Box::new(crate::hypergraph::ConceptClusterSchema),
        Box::new(crate::evidence_bundle::EvidenceBundleSchema),
        Box::new(crate::evidence_bundle::ClaimSchema),
        // ─── Research Process Plane (ADR-043) ───
        Box::new(crate::process::ResearchProblemSchema),
        Box::new(crate::process::ResearchEnvironmentSchema),
        Box::new(crate::process::BaselineSnapshotSchema),
        Box::new(crate::process::ResearchIdeaSchema),
        Box::new(crate::process::HypothesisSchema),
        Box::new(crate::process::InterventionSchema),
        Box::new(crate::process::InterventionBundleSchema),
        Box::new(crate::process::ImplementationAttemptSchema),
        Box::new(crate::process::ArtifactVersionSchema),
        Box::new(crate::process::ExperimentRunSchema),
        Box::new(crate::process::MetricDefinitionSchema),
        Box::new(crate::process::MetricObservationSchema),
        Box::new(crate::process::ResultComparisonSchema),
        Box::new(crate::process::FailureEventSchema),
        // ─── Conflict plane (ADR-047) ───
        Box::new(crate::process::ConflictSchema),
        // ─── Decision plane (ADR-048) ───
        Box::new(crate::process::DecisionSchema),
    ]
}

/// Find a schema by its label.
pub fn schema_for_label(label: &str) -> Option<Box<dyn NodeSchemaDef>> {
    all_node_schemas().into_iter().find(|s| s.label() == label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_node_schemas_has_twenty_nine_types() {
        let schemas = all_node_schemas();
        assert_eq!(schemas.len(), 31);
        let labels: Vec<&str> = schemas.iter().map(|s| s.label()).collect();
        // Publication plane
        assert!(labels.contains(&"Paper"));
        assert!(labels.contains(&"Section"));
        assert!(labels.contains(&"Author"));
        assert!(labels.contains(&"Institution"));
        assert!(labels.contains(&"Concept"));
        assert!(labels.contains(&"Topic"));
        assert!(labels.contains(&"Category"));
        assert!(labels.contains(&"Reference"));
        assert!(labels.contains(&"Citation"));
        assert!(labels.contains(&"Entity"));
        assert!(labels.contains(&"Source"));
        // Evidence & Community
        assert!(labels.contains(&"ConceptCluster"));
        assert!(labels.contains(&"EvidenceBundle"));
        assert!(labels.contains(&"Claim"));
        // Research Process Plane (ADR-043)
        assert!(labels.contains(&"ResearchProblem"));
        assert!(labels.contains(&"ResearchEnvironment"));
        assert!(labels.contains(&"BaselineSnapshot"));
        assert!(labels.contains(&"ResearchIdea"));
        assert!(labels.contains(&"Hypothesis"));
        assert!(labels.contains(&"Intervention"));
        assert!(labels.contains(&"InterventionBundle"));
        assert!(labels.contains(&"ImplementationAttempt"));
        assert!(labels.contains(&"ArtifactVersion"));
        assert!(labels.contains(&"ExperimentRun"));
        assert!(labels.contains(&"MetricDefinition"));
        assert!(labels.contains(&"MetricObservation"));
        assert!(labels.contains(&"ResultComparison"));
        assert!(labels.contains(&"FailureEvent"));
        assert!(labels.contains(&"SchedulerTask"));
    }

    #[test]
    fn test_schema_for_label() {
        let paper = schema_for_label("Paper");
        assert!(paper.is_some());
        assert_eq!(paper.unwrap().label(), "Paper");

        let unknown = schema_for_label("Unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_paper_schema_validates_with_required_fields() {
        use std::collections::HashMap;
        let schema = crate::paper::PaperSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::json!("vid:paper:123"));
        props.insert("arxiv_id".to_string(), serde_json::json!("1234.5678"));
        props.insert("title".to_string(), serde_json::json!("Test"));
        props.insert("valid_from".to_string(), serde_json::json!(1234567890));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_paper_schema_rejects_missing_required() {
        use std::collections::HashMap;
        let schema = crate::paper::PaperSchema;
        let props = HashMap::new(); // missing all required
        assert!(schema.validate(&props).is_err());
    }
}

/// Render all registered node schemas as a markdown table for documentation.
/// Use in `da schema-list` CLI command to emit always-fresh content for
/// GRAPH-SCHEMA.md. Columns: label, required fields, optional field count,
/// and whether the schema is materialized in the pipeline (per the known
/// materialization list maintained here).
pub fn render_node_types_table() -> String {
    // Materialized labels — keep in sync with crates/da-application/src
    // create_node call sites. Verified by da schema-check CLI.
    let materialized: &[&str] = &[
        "Author", "Category", "Citation", "Claim", "ConceptCluster", "Entity",
        "EvidenceBundle", "Institution", "MetricObservation", "Paper",
        "Reference", "ResearchProblem", "SchedulerTask", "Section", "Source",
        "Topic",
    ];

    let mut schemas = all_node_schemas();
    schemas.sort_by_key(|s| s.label());

    let mut s = String::new();
    s.push_str("| Label | Required fields | Optional fields | Materialized? |\n");
    s.push_str("|-------|-----------------|-----------------|---------------|\n");
    for schema in &schemas {
        let label = schema.label();
        let req: Vec<&str> = schema.required_fields().iter().map(|(n, _)| *n).collect();
        let opt_count = schema.optional_fields().len();
        let mat = if materialized.contains(&label) { "✅" } else { "—" };
        s.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            label,
            req.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(", "),
            opt_count,
            mat
        ));
    }
    s
}

#[cfg(test)]
mod render_tests {
    use super::render_node_types_table;

    #[test]
    fn test_render_node_types_table_has_header_and_all_labels() {
        let table = render_node_types_table();
        assert!(table.contains("| Label | Required fields | Optional fields | Materialized? |"));
        // Every registered schema must appear
        for label in [
            "ArtifactVersion", "Author", "BaselineSnapshot", "Category",
            "Citation", "Claim", "ConceptCluster", "Concept", "Conflict",
            "Decision", "Entity", "EvidenceBundle", "ExperimentRun", "FailureEvent",
            "Hypothesis", "ImplementationAttempt", "Institution",
            "Intervention", "InterventionBundle", "MetricDefinition",
            "MetricObservation", "Paper", "Reference", "ResearchEnvironment",
            "ResearchIdea", "ResearchProblem", "ResultComparison",
            "SchedulerTask", "Section", "Source", "Topic",
        ] {
            assert!(
                table.contains(&format!("`{label}`")),
                "label `{label}` missing from rendered table:\n{table}"
            );
        }
    }

    #[test]
    fn test_render_node_types_table_marks_known_materialized() {
        let table = render_node_types_table();
        // Paper, Entity, Author are materialized — must show ✅.
        for line in table.lines() {
            if line.contains("`Paper`") {
                assert!(line.contains("✅"), "Paper must be marked materialized: {line}");
            }
            if line.contains("`Entity`") {
                assert!(line.contains("✅"), "Entity must be marked materialized: {line}");
            }
            if line.contains("`Hypothesis`") {
                assert!(
                    line.contains("—"),
                    "Hypothesis must NOT be marked materialized: {line}"
                );
            }
        }
    }
}
