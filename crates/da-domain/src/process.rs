//! Research Process Plane kernel types (ADR-043, PROCESS-SCHEMA-P0).
//!
//! Execution-grounded scientific memory nodes. These are cross-cutting
//! (distributed across L1–L6), not a new Layer 8.
//!
//! Canonical chain:
//!   ResearchProblem → ResearchEnvironment → ResearchIdea → Hypothesis →
//!   Intervention → ImplementationAttempt → ArtifactVersion →
//!   ExperimentRun → MetricObservation → ResultComparison → Claim
//!   (+ FailureEvent, ReplicationRun)
//!
//! Invariants (see ADR-043 §Invariants, PROCESS-SCHEMA-P0 §16):
//!   1. ResearchIdea ≠ Hypothesis ≠ Claim
//!   2. ImplementationAttempt ≠ ExperimentRun
//!   3. FailureEvent ≠ NegativeResult (no REFUTES edge)
//!   4. MetricObservation ≠ ResultComparison ≠ Claim ≠ RewardSignal
//!   5. Experimental Claim requires VALID_UNDER → ResearchEnvironment
//!   6. Environment completeness explicit (full | env_lite | unknown)

use crate::schema::{FieldType, NodeSchemaDef};
use std::collections::HashSet;
use std::sync::OnceLock;

// ─── Failure taxonomy YAML loading ───

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct FailureTaxonomyYaml {
    version: String,
    stages: Vec<CodeNameEntry>,
    classes: Vec<CodeNameEntry>,
    completeness_tiers: Vec<CodeNameEntry>,
    evidence_origins: Vec<CodeNameEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct CodeNameEntry {
    code: String,
    name: String,
}

const BUNDLED_TAXONOMY_YAML: &str = include_str!("../../../data/failure_taxonomy.yaml");

/// Failure taxonomy registry — stages, classes, completeness, origins.
pub struct FailureTaxonomyRegistry {
    pub stages: HashSet<String>,
    pub classes: HashSet<String>,
    pub completeness_tiers: HashSet<String>,
    pub evidence_origins: HashSet<String>,
}

impl FailureTaxonomyRegistry {
    fn load() -> Self {
        let yaml_str = std::fs::read_to_string("data/failure_taxonomy.yaml")
            .unwrap_or_else(|_| BUNDLED_TAXONOMY_YAML.to_string());
        let data: FailureTaxonomyYaml =
            serde_yaml::from_str(&yaml_str).expect("failure_taxonomy.yaml must be valid YAML");
        Self {
            stages: data.stages.iter().map(|e| e.code.clone()).collect(),
            classes: data.classes.iter().map(|e| e.code.clone()).collect(),
            completeness_tiers: data
                .completeness_tiers
                .iter()
                .map(|e| e.code.clone())
                .collect(),
            evidence_origins: data
                .evidence_origins
                .iter()
                .map(|e| e.code.clone())
                .collect(),
        }
    }

    fn instance() -> &'static FailureTaxonomyRegistry {
        static REGISTRY: OnceLock<FailureTaxonomyRegistry> = OnceLock::new();
        REGISTRY.get_or_init(FailureTaxonomyRegistry::load)
    }
}

/// Check if a failure stage code is known.
pub fn is_known_stage(code: &str) -> bool {
    FailureTaxonomyRegistry::instance().stages.contains(code)
}

/// Check if a failure class code is known.
pub fn is_known_failure_class(code: &str) -> bool {
    FailureTaxonomyRegistry::instance().classes.contains(code)
}

/// Check if a completeness tier is known.
pub fn is_known_completeness(code: &str) -> bool {
    FailureTaxonomyRegistry::instance()
        .completeness_tiers
        .contains(code)
}

/// Check if an evidence origin is known.
pub fn is_known_origin(code: &str) -> bool {
    FailureTaxonomyRegistry::instance()
        .evidence_origins
        .contains(code)
}

// ═══════════════════════════════════════════════════════════════
// 1. ResearchProblem
// ═══════════════════════════════════════════════════════════════

/// What must be improved or explained. Domain-agnostic.
pub struct ResearchProblemSchema;

impl NodeSchemaDef for ResearchProblemSchema {
    fn label(&self) -> &'static str {
        "ResearchProblem"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("text", FieldType::String),
            ("problem_type", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("parent_problem_id", FieldType::String),
            ("domain_pack_id", FieldType::String),
            ("source_span_id", FieldType::String),
            ("evidence_bundle_id", FieldType::String),
            ("origin", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 2. ResearchEnvironment (two-tier: full / env_lite)
// ═══════════════════════════════════════════════════════════════

/// Fully or partially specified verification context.
/// The only legitimate anchor for experimental-effectiveness claims.
/// Two-tier: `full` (hashable, live) or `env_lite` (literature fingerprint).
pub struct ResearchEnvironmentSchema;

impl NodeSchemaDef for ResearchEnvironmentSchema {
    fn label(&self) -> &'static str {
        "ResearchEnvironment"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("completeness", FieldType::String), // full | env_lite | unknown
            ("research_problem_id", FieldType::String),
            ("baseline_ref", FieldType::String),
            ("subject_system", FieldType::String),
            ("subject_system_kind", FieldType::String),
            ("environment_template_id", FieldType::String),
            ("evidence_origin", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("environment_hash", FieldType::String),
            ("input_data_refs", FieldType::String),
            ("eval_data_refs", FieldType::String),
            ("protocol_ref", FieldType::String),
            ("metric_definition_ids", FieldType::String),
            ("objective_function", FieldType::String),
            ("compute_budget", FieldType::Float),
            ("wall_clock_budget", FieldType::Integer),
            ("sample_size_budget", FieldType::Integer),
            ("hardware_or_lab_profile", FieldType::String),
            ("allowed_change_scope", FieldType::String),
            ("protected_eval_artifacts", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 3. BaselineSnapshot
// ═══════════════════════════════════════════════════════════════

/// Concrete baseline artifact+config, not an abstract name.
pub struct BaselineSnapshotSchema;

impl NodeSchemaDef for BaselineSnapshotSchema {
    fn label(&self) -> &'static str {
        "BaselineSnapshot"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("artifact_ref_id", FieldType::String),
            ("description", FieldType::String),
            ("baseline_type", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("performance_ref", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 4. ResearchIdea
// ═══════════════════════════════════════════════════════════════

/// Natural-language proposal. Not necessarily testable yet.
/// Lineage edges: VARIANT_OF, REFINES, COMBINES, GENERALIZES, etc.
pub struct ResearchIdeaSchema;

impl NodeSchemaDef for ResearchIdeaSchema {
    fn label(&self) -> &'static str {
        "ResearchIdea"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("text", FieldType::String),
            ("idea_type", FieldType::String),
            ("research_problem_id", FieldType::String),
            ("proposed_at", FieldType::DateTime),
            ("proposed_by", FieldType::String),
            ("status", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("search_campaign_id", FieldType::String),
            ("source_span_id", FieldType::String),
            ("origin", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 5. Hypothesis (first-class, ≠ Claim)
// ═══════════════════════════════════════════════════════════════

/// Formal pre-test expectation under a specific environment.
/// DISTINCT from Claim (post-evidence proposition).
/// Only completed valid runs → ResultComparison → SUPPORTS/REFUTES this.
pub struct HypothesisSchema;

impl NodeSchemaDef for HypothesisSchema {
    fn label(&self) -> &'static str {
        "Hypothesis"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("text", FieldType::String),
            ("environment_id", FieldType::String),
            ("metric_definition_id", FieldType::String),
            ("direction", FieldType::String),
            ("research_idea_id", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("expected_effect_size", FieldType::Float),
            ("confidence_prior", FieldType::Float),
            ("source_span_id", FieldType::String),
            ("origin", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 6. Intervention
// ═══════════════════════════════════════════════════════════════

/// Normalized change to method/architecture/protocol/exposure/params.
pub struct InterventionSchema;

impl NodeSchemaDef for InterventionSchema {
    fn label(&self) -> &'static str {
        "Intervention"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("target_component", FieldType::String),
            ("change_type", FieldType::String),
            ("change_scope", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("parameter_before", FieldType::String),
            ("parameter_after", FieldType::String),
            ("implementation_artifact_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 7. InterventionBundle (compound recipe)
// ═══════════════════════════════════════════════════════════════

/// Compound recipe of interventions. Avoids storing multi-change recipe
/// as single opaque Method. Enables ablation evidence (P1).
pub struct InterventionBundleSchema;

impl NodeSchemaDef for InterventionBundleSchema {
    fn label(&self) -> &'static str {
        "InterventionBundle"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("recipe_kind", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 8. ImplementationAttempt
// ═══════════════════════════════════════════════════════════════

/// Attempt to turn an idea into an executable artifact.
/// ≠ ExperimentRun (successful patch is not an experiment).
pub struct ImplementationAttemptSchema;

impl NodeSchemaDef for ImplementationAttemptSchema {
    fn label(&self) -> &'static str {
        "ImplementationAttempt"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("research_idea_id", FieldType::String),
            ("attempt_number", FieldType::Integer),
            ("status", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("artifact_version_id", FieldType::String),
            ("failure_event_id", FieldType::String),
            ("patch_diff_ref", FieldType::String),
            ("repair_attempts", FieldType::Integer),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 9. ArtifactVersion
// ═══════════════════════════════════════════════════════════════

/// Immutable code/config/model/container/notebook snapshot.
pub struct ArtifactVersionSchema;

impl NodeSchemaDef for ArtifactVersionSchema {
    fn label(&self) -> &'static str {
        "ArtifactVersion"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("content_hash", FieldType::String),
            ("artifact_kind", FieldType::String),
            ("uri", FieldType::String),
            ("immutable", FieldType::Boolean),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("path", FieldType::String),
            ("line_start", FieldType::Integer),
            ("line_end", FieldType::Integer),
            ("parent_artifact_id", FieldType::String),
            ("created_at", FieldType::DateTime),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 10. ExperimentRun
// ═══════════════════════════════════════════════════════════════

/// Execution of an artifact in an environment.
/// Only status=completed (not invalid) runs may produce observations.
pub struct ExperimentRunSchema;

impl NodeSchemaDef for ExperimentRunSchema {
    fn label(&self) -> &'static str {
        "ExperimentRun"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("environment_id", FieldType::String),
            ("artifact_version_id", FieldType::String),
            ("run_type", FieldType::String),
            ("status", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("intervention_bundle_id", FieldType::String),
            ("started_at", FieldType::DateTime),
            ("finished_at", FieldType::DateTime),
            ("wall_clock_sec", FieldType::Integer),
            ("seed", FieldType::String),
            ("preregistered", FieldType::Boolean),
            ("hypothesis_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 11. MetricDefinition
// ═══════════════════════════════════════════════════════════════

/// Metric name + computation protocol. Reusable across runs.
/// ≠ MetricObservation (per-run measured value).
pub struct MetricDefinitionSchema;

impl NodeSchemaDef for MetricDefinitionSchema {
    fn label(&self) -> &'static str {
        "MetricDefinition"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("name", FieldType::String),
            ("direction", FieldType::String),
            ("split", FieldType::String),
            ("computation_protocol", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("unit", FieldType::String),
            ("metric_code_artifact_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 12. MetricObservation
// ═══════════════════════════════════════════════════════════════

/// Raw measured value. NOT a comparison. NOT a claim.
pub struct MetricObservationSchema;

impl NodeSchemaDef for MetricObservationSchema {
    fn label(&self) -> &'static str {
        "MetricObservation"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("run_id", FieldType::String),
            ("metric_definition_id", FieldType::String),
            ("value", FieldType::Float),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("std_dev", FieldType::Float),
            ("n_seeds", FieldType::Integer),
            ("logged_artifact_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
                    ("document_id", FieldType::String),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("recorded_at", FieldType::DateTime),
            ("superseded_at", FieldType::DateTime),
]
    }
}

// ═══════════════════════════════════════════════════════════════
// 13. ResultComparison
// ═══════════════════════════════════════════════════════════════

/// Derived comparison: candidate vs baseline/peer observation.
/// ≠ MetricObservation (raw) ≠ Claim (interpreted proposition).
pub struct ResultComparisonSchema;

impl NodeSchemaDef for ResultComparisonSchema {
    fn label(&self) -> &'static str {
        "ResultComparison"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("candidate_observation_id", FieldType::String),
            ("baseline_observation_id", FieldType::String),
            ("environment_id", FieldType::String),
            ("valid", FieldType::Boolean),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("absolute_delta", FieldType::Float),
            ("relative_delta", FieldType::Float),
            ("significance_test", FieldType::String),
            ("p_value", FieldType::Float),
            ("confidence_interval_low", FieldType::Float),
            ("confidence_interval_high", FieldType::Float),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════
// 14. FailureEvent
// ═══════════════════════════════════════════════════════════════

/// Structured non-execution or invalidity cause.
/// NEVER has REFUTES → Hypothesis edge.
/// Failure ≠ negative result. Only completed valid runs can refute.
pub struct FailureEventSchema;

impl NodeSchemaDef for FailureEventSchema {
    fn label(&self) -> &'static str {
        "FailureEvent"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("stage", FieldType::String),
            ("class", FieldType::String),
            ("recoverable", FieldType::Boolean),
            ("error_signature", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("log_ref_id", FieldType::String),
            ("artifact_version_id", FieldType::String),
            ("environment_id", FieldType::String),
            ("repair_attempt_count", FieldType::Integer),
            ("resolved_by", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

/// Conflict node (ADR-047).
///
/// First-class object representing factual disagreement between
/// fact-bearing nodes (Claim, Entity, Reference, MetricObservation).
/// Lives in Layer 7 — system-derived, not source-derived.
pub struct ConflictSchema;

impl NodeSchemaDef for ConflictSchema {
    fn label(&self) -> &'static str {
        "Conflict"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("kind", FieldType::String),
            ("field", FieldType::String),
            ("status", FieldType::String),
            ("severity", FieldType::String),
            ("detected_at", FieldType::DateTime),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("resolution_strategy", FieldType::String),
            ("resolution_value", FieldType::String),
            ("resolved_at", FieldType::DateTime),
            // Bi-temporal fields (ADR-046)
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("recorded_at", FieldType::DateTime),
            ("superseded_at", FieldType::DateTime),
            // Invariants
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

/// Decision node (ADR-048).
///
/// First-class object capturing every system action with a lifecycle:
/// healing, conflict resolution, promotion, schema migration,
/// extraction override, hypothesis test, retraction recording, etc.
/// Lives in Layer 8 — system-derived, bi-temporal, causally linked.
pub struct DecisionSchema;

impl NodeSchemaDef for DecisionSchema {
    fn label(&self) -> &'static str {
        "Decision"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("category", FieldType::String),
            ("scenario", FieldType::String),
            ("reasoning", FieldType::String),
            ("outcome", FieldType::String),
            ("confidence", FieldType::Float),
            ("decision_maker", FieldType::String),
            // Bi-temporal valid axis (ADR-046)
            ("valid_from", FieldType::DateTime),
            // Bi-temporal transaction axis (ADR-046)
            ("recorded_at", FieldType::DateTime),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("policy_id", FieldType::String),
            // Bi-temporal open bounds
            ("valid_to", FieldType::DateTime),
            ("superseded_at", FieldType::DateTime),
            // Semantic precedent search embedding (Phase 2)
            ("reasoning_embedding", FieldType::Vector),
            // Invariants
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
}

// Failure taxonomy constants moved to data/failure_taxonomy.yaml.
// Use is_known_stage(), is_known_failure_class() for validation.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_vid(id: &str) -> HashMap<String, serde_json::Value> {
        let mut m = HashMap::new();
        m.insert("vid".to_string(), serde_json::json!(id));
        m
    }

    // ─── Label tests ───

    #[test]
    fn test_research_problem_label() {
        assert_eq!(ResearchProblemSchema.label(), "ResearchProblem");
    }

    #[test]
    fn test_research_environment_label() {
        assert_eq!(ResearchEnvironmentSchema.label(), "ResearchEnvironment");
    }

    #[test]
    fn test_baseline_snapshot_label() {
        assert_eq!(BaselineSnapshotSchema.label(), "BaselineSnapshot");
    }

    #[test]
    fn test_research_idea_label() {
        assert_eq!(ResearchIdeaSchema.label(), "ResearchIdea");
    }

    #[test]
    fn test_hypothesis_label() {
        assert_eq!(HypothesisSchema.label(), "Hypothesis");
    }

    #[test]
    fn test_intervention_label() {
        assert_eq!(InterventionSchema.label(), "Intervention");
    }

    #[test]
    fn test_intervention_bundle_label() {
        assert_eq!(InterventionBundleSchema.label(), "InterventionBundle");
    }

    #[test]
    fn test_implementation_attempt_label() {
        assert_eq!(ImplementationAttemptSchema.label(), "ImplementationAttempt");
    }

    #[test]
    fn test_artifact_version_label() {
        assert_eq!(ArtifactVersionSchema.label(), "ArtifactVersion");
    }

    #[test]
    fn test_experiment_run_label() {
        assert_eq!(ExperimentRunSchema.label(), "ExperimentRun");
    }

    #[test]
    fn test_metric_definition_label() {
        assert_eq!(MetricDefinitionSchema.label(), "MetricDefinition");
    }

    #[test]
    fn test_metric_observation_label() {
        assert_eq!(MetricObservationSchema.label(), "MetricObservation");
    }

    #[test]
    fn test_result_comparison_label() {
        assert_eq!(ResultComparisonSchema.label(), "ResultComparison");
    }

    #[test]
    fn test_failure_event_label() {
        assert_eq!(FailureEventSchema.label(), "FailureEvent");
    }

    // ─── Required fields tests ───

    #[test]
    fn test_research_environment_required_fields() {
        let fields = ResearchEnvironmentSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"completeness"));
        assert!(names.contains(&"environment_template_id"));
        assert!(names.contains(&"evidence_origin"));
    }

    #[test]
    fn test_hypothesis_links_environment_and_metric() {
        let fields = HypothesisSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"environment_id"));
        assert!(names.contains(&"metric_definition_id"));
        assert!(names.contains(&"direction"));
        assert!(names.contains(&"research_idea_id"));
    }

    #[test]
    fn test_metric_observation_has_value() {
        let fields = MetricObservationSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"value"));
        assert!(names.contains(&"run_id"));
    }

    #[test]
    fn test_result_comparison_has_valid_flag() {
        let fields = ResultComparisonSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"valid"));
        assert!(names.contains(&"candidate_observation_id"));
        assert!(names.contains(&"baseline_observation_id"));
        assert!(names.contains(&"environment_id"));
    }

    #[test]
    fn test_failure_event_has_stage_and_class() {
        let fields = FailureEventSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"stage"));
        assert!(names.contains(&"class"));
        assert!(names.contains(&"recoverable"));
    }

    #[test]
    fn test_artifact_version_has_hash() {
        let fields = ArtifactVersionSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"content_hash"));
        assert!(names.contains(&"artifact_kind"));
        assert!(names.contains(&"immutable"));
    }

    // ─── Validation tests ───

    #[test]
    fn test_research_problem_validates() {
        let schema = ResearchProblemSchema;
        let mut props = make_vid("vid:problem:test");
        props.insert(
            "text".to_string(),
            serde_json::json!("improve GNN on heterophilous graphs"),
        );
        props.insert("problem_type".to_string(), serde_json::json!("improvement"));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_research_problem_missing_text_fails() {
        let schema = ResearchProblemSchema;
        let props = make_vid("vid:problem:test");
        assert!(schema.validate(&props).is_err());
    }

    #[test]
    fn test_research_environment_full_completeness() {
        let schema = ResearchEnvironmentSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::json!("vid:env:abc123"));
        props.insert("completeness".to_string(), serde_json::json!("full"));
        props.insert(
            "research_problem_id".to_string(),
            serde_json::json!("vid:problem:p1"),
        );
        props.insert(
            "baseline_ref".to_string(),
            serde_json::json!("vid:baseline:b1"),
        );
        props.insert(
            "subject_system".to_string(),
            serde_json::json!("Qwen2.5-Math-1.5B"),
        );
        props.insert(
            "subject_system_kind".to_string(),
            serde_json::json!("model_checkpoint"),
        );
        props.insert(
            "environment_template_id".to_string(),
            serde_json::json!("cs.LG"),
        );
        props.insert(
            "evidence_origin".to_string(),
            serde_json::json!("live_executed"),
        );
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_hypothesis_validates() {
        let schema = HypothesisSchema;
        let mut props = make_vid("vid:hyp:h1");
        props.insert(
            "text".to_string(),
            serde_json::json!("GAT residual improves heterophilous accuracy"),
        );
        props.insert(
            "environment_id".to_string(),
            serde_json::json!("vid:env:e1"),
        );
        props.insert(
            "metric_definition_id".to_string(),
            serde_json::json!("vid:metric:acc"),
        );
        props.insert("direction".to_string(), serde_json::json!("increase"));
        props.insert(
            "research_idea_id".to_string(),
            serde_json::json!("vid:idea:i1"),
        );
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_failure_event_validates() {
        let schema = FailureEventSchema;
        let mut props = make_vid("vid:fail:f1");
        props.insert("stage".to_string(), serde_json::json!("execution"));
        props.insert("class".to_string(), serde_json::json!("oom"));
        props.insert("recoverable".to_string(), serde_json::json!(true));
        props.insert(
            "error_signature".to_string(),
            serde_json::json!("CUDA OOM at batch=256"),
        );
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_metric_observation_validates() {
        let schema = MetricObservationSchema;
        let mut props = make_vid("vid:obs:o1");
        props.insert("run_id".to_string(), serde_json::json!("vid:run:r1"));
        props.insert(
            "metric_definition_id".to_string(),
            serde_json::json!("vid:metric:acc"),
        );
        props.insert("value".to_string(), serde_json::json!(0.694));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_result_comparison_validates() {
        let schema = ResultComparisonSchema;
        let mut props = make_vid("vid:cmp:c1");
        props.insert(
            "candidate_observation_id".to_string(),
            serde_json::json!("vid:obs:o1"),
        );
        props.insert(
            "baseline_observation_id".to_string(),
            serde_json::json!("vid:obs:o0"),
        );
        props.insert(
            "environment_id".to_string(),
            serde_json::json!("vid:env:e1"),
        );
        props.insert("valid".to_string(), serde_json::json!(true));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_result_comparison_invalid_flag() {
        let schema = ResultComparisonSchema;
        let mut props = make_vid("vid:cmp:c2");
        props.insert(
            "candidate_observation_id".to_string(),
            serde_json::json!("vid:obs:o2"),
        );
        props.insert(
            "baseline_observation_id".to_string(),
            serde_json::json!("vid:obs:o0"),
        );
        props.insert(
            "environment_id".to_string(),
            serde_json::json!("vid:env:e1"),
        );
        props.insert("valid".to_string(), serde_json::json!(false)); // invalid comparison
        assert!(schema.validate(&props).is_ok()); // schema validates, but valid=false means cannot SUPPORTS
    }

    // ─── Invariant: retrieval_eligible present on all schemas ───

    #[test]
    fn test_all_process_schemas_have_retrieval_eligible() {
        let schemas: Vec<Box<dyn NodeSchemaDef>> = vec![
            Box::new(ResearchProblemSchema),
            Box::new(ResearchEnvironmentSchema),
            Box::new(BaselineSnapshotSchema),
            Box::new(ResearchIdeaSchema),
            Box::new(HypothesisSchema),
            Box::new(InterventionSchema),
            Box::new(InterventionBundleSchema),
            Box::new(ImplementationAttemptSchema),
            Box::new(ArtifactVersionSchema),
            Box::new(ExperimentRunSchema),
            Box::new(MetricDefinitionSchema),
            Box::new(MetricObservationSchema),
            Box::new(ResultComparisonSchema),
            Box::new(FailureEventSchema),
        ];
        for schema in &schemas {
            let all_fields: Vec<&str> = schema
                .required_fields()
                .iter()
                .chain(schema.optional_fields().iter())
                .map(|(n, _)| *n)
                .collect();
            assert!(
                all_fields.contains(&"retrieval_eligible"),
                "{} missing retrieval_eligible",
                schema.label()
            );
            assert!(
                all_fields.contains(&"import_eligible"),
                "{} missing import_eligible (D127)",
                schema.label()
            );
        }
    }

    // ─── Taxonomy registry tests ───

    #[test]
    fn test_completeness_tiers_from_yaml() {
        assert!(is_known_completeness("full"));
        assert!(is_known_completeness("env_lite"));
        assert!(is_known_completeness("unknown"));
        assert!(!is_known_completeness("invalid"));
    }

    #[test]
    fn test_failure_classes_from_yaml() {
        assert!(is_known_failure_class("oom"));
        assert!(is_known_failure_class("timeout"));
        assert!(is_known_failure_class("guard_violation"));
        assert!(is_known_failure_class("completed_without_improvement"));
        assert!(!is_known_failure_class("not_a_real_failure"));
    }

    #[test]
    fn test_stages_from_yaml() {
        assert!(is_known_stage("execution"));
        assert!(is_known_stage("implementation"));
        assert!(is_known_stage("replication"));
        assert!(!is_known_stage("not_a_stage"));
    }

    #[test]
    fn test_origins_from_yaml() {
        assert!(is_known_origin("literature_reported"));
        assert!(is_known_origin("live_executed"));
        assert!(is_known_origin("observational"));
        assert!(!is_known_origin("unknown_origin"));
    }
}
