//! Schema-as-code framework (ADR-040 §11.1).
//!
//! Universal trait + types re-exported from kg_ontology::schema
//! (M289-u4fzx5/S02). Project-specific all_node_schemas(),
//! schema_for_label(), and render_node_types_table() stay here.

// Re-export universal schema types from kg-ontology (ADR-050).
pub use kg_ontology::schema::{Field, FieldType, NodeSchemaDef, SchemaError};

/// Current schema version (ADR-040 §11.2).
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
        // ─── ADR-053 Layer B-C (D194) ───
        Box::new(crate::article::SummarySchema),
        Box::new(crate::article::QualitySignalSchema),
        Box::new(crate::article::MethodSchema),
        Box::new(crate::article::DatasetSchema),
        Box::new(crate::article::MetricSchema),
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

/// All registered node labels from the **compiled** ontology schema registry.
///
/// Prefer [`registered_node_labels_or_yaml`] for inventory — external meta-ontology first.
pub fn registered_node_labels() -> Vec<&'static str> {
    all_node_schemas().into_iter().map(|s| s.label()).collect()
}

/// All registered edge type constants from `relation.rs` modules.
/// Used by MCP server `get_schema` tool and graph task suite.
pub fn registered_edge_types() -> Vec<&'static str> {
    use crate::relation::bibliographic;
    // Base wire strings that are guaranteed present (used by HOT paths).
    // Full edge catalog comes from YAML via `registered_edge_types_or_yaml`.
    vec![bibliographic::CITES]
}

/// Edge type catalog from external meta-ontology (`data/ontology/edge_contracts.yaml`)
/// when present; otherwise falls back to the compiled [`registered_edge_types`].
///
/// ADR-050 / M324: edge contracts are DATA — MCP server and inventory must not
/// hardcode the full edge list.
pub fn registered_edge_types_or_yaml() -> Vec<String> {
    if let Ok(reg) = kg_ontology::registry::OntologyRegistry::load_project_meta_ontology()
        && !reg.edge_contracts().is_empty()
    {
        return reg.edge_types();
    }
    registered_edge_types()
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

/// Node labels from external meta-ontology (`data/ontology/node_schemas.yaml`) when
/// present; otherwise compiled [`registered_node_labels`].
///
/// ADR-050 / M324: system must support external meta-ontology — not hardcode discovery.
pub fn registered_node_labels_or_yaml() -> Vec<String> {
    if let Ok(reg) = kg_ontology::registry::OntologyRegistry::load_project_meta_ontology()
        && !reg.is_empty()
    {
        return reg.labels_owned();
    }
    registered_node_labels()
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Find a schema by its label, preferring YAML-loaded data when available.
///
/// ADR-050 Phase F: tries data/ontology/node_schemas.yaml first, falls
/// back to hardcoded all_node_schemas(). This is the opt-in YAML-driven
/// variant of schema_for_label() — the original function stays unchanged.
pub fn schema_for_label_or_yaml(label: &str) -> Option<Box<dyn NodeSchemaDef>> {
    crate::yaml_schema_adapter::all_node_schemas_or_yaml()
        .into_iter()
        .find(|s| s.label() == label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_node_schemas_has_thirty_six_types() {
        let schemas = all_node_schemas();
        assert_eq!(schemas.len(), 36); // + QualitySignal: real SummaryUseCase output
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
        assert!(labels.contains(&"QualitySignal"));
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
    fn test_registered_node_labels_matches_all_schemas() {
        let labels = registered_node_labels();
        let schemas = all_node_schemas();
        assert_eq!(labels.len(), schemas.len());
        assert!(labels.contains(&"Paper"));
        assert!(labels.contains(&"Section"));
        assert!(labels.contains(&"Reference"));
        for label in &labels {
            assert!(schema_for_label(label).is_some(), "orphan label {label}");
        }
    }

    #[test]
    fn test_registered_node_labels_or_yaml_includes_paper() {
        let labels = registered_node_labels_or_yaml();
        assert!(labels.len() >= 10);
        assert!(labels.iter().any(|l| l == "Paper"));
        assert!(labels.iter().any(|l| l == "Section"));
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
        "Author",
        "Category",
        "Citation",
        "Claim",
        "ConceptCluster",
        "Entity",
        "EvidenceBundle",
        "Institution",
        "MetricObservation",
        "Paper",
        "Reference",
        "ResearchProblem",
        "SchedulerTask",
        "Section",
        "Source",
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
        let mat = if materialized.contains(&label) {
            "✅"
        } else {
            "—"
        };
        s.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            label,
            req.iter()
                .map(|r| format!("`{r}`"))
                .collect::<Vec<_>>()
                .join(", "),
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
            "ArtifactVersion",
            "Author",
            "BaselineSnapshot",
            "Category",
            "Citation",
            "Claim",
            "ConceptCluster",
            "Concept",
            "Conflict",
            "Decision",
            "Entity",
            "EvidenceBundle",
            "ExperimentRun",
            "FailureEvent",
            "Hypothesis",
            "ImplementationAttempt",
            "Institution",
            "Intervention",
            "InterventionBundle",
            "MetricDefinition",
            "MetricObservation",
            "Paper",
            "Reference",
            "ResearchEnvironment",
            "ResearchIdea",
            "ResearchProblem",
            "ResultComparison",
            "SchedulerTask",
            "Section",
            "Source",
            "Topic",
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
                assert!(
                    line.contains("✅"),
                    "Paper must be marked materialized: {line}"
                );
            }
            if line.contains("`Entity`") {
                assert!(
                    line.contains("✅"),
                    "Entity must be marked materialized: {line}"
                );
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
