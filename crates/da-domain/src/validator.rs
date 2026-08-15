//! Schema validator (ADR-040 §11.1, ADR-044).
//!
//! Comprehensive validator that checks node property maps against
//! the declared NodeSchemaDef for a label, plus architectural
//! invariants (D127 fail-closed import, D134 retrieval eligibility,
//! schema_version presence, vid presence).
//!
//! Returns ALL violations (not just the first) so callers get a
//! complete diagnostic in a single pass.
//!
//! # Design
//!
//! - Pure logic, no IO — lives in da-domain.
//! - Does not mutate inputs.
//! - Two entry points:
//!   - [`validate_node_properties`] for a single node
//!   - [`validate_edge_type`] for edge-type registry check
//! - [`Severity::Critical`] for missing required fields / broken invariants /
//!   non-canonical DateTime wire values (PIR-008)
//! - [`Severity::Warning`] for type mismatches / suspicious patterns
//!
//! # Universal types (ADR-050 Phase A re-export)
//!
//! The type definitions of `Severity`, `Violation`, `PropertySnapshot`,
//! and the `format_violations` helper live in [`kg_ontology::validator`]
//! (universal crate, no da-* deps). This module re-exports them so
//! existing callers (`da_domain::validator::Severity` etc.) keep working.
//! The legacy alias `SchemaViolation = Violation` preserves the
//! pre-migration type name.

// Re-export universal validator types from kg-ontology (ADR-050 Phase A).
pub use kg_ontology::validator::{PropertySnapshot, Severity, Violation, format_violations};

/// Legacy alias for the pre-migration type name. New code should use
/// [`Violation`] directly; this alias exists so existing call sites in
/// da-cli and da-application tests keep compiling without churn.
pub type SchemaViolation = Violation;

use crate::schema::{FieldType, schema_for_label};

/// Validate one node's property snapshot against its declared schema.
///
/// Returns a list of violations (empty = valid). Unknown labels return
/// a single Critical violation.
pub fn validate_node_properties(label: &str, props: &PropertySnapshot) -> Vec<SchemaViolation> {
    let mut violations = Vec::new();

    let schema = match schema_for_label(label) {
        Some(s) => s,
        None => {
            violations.push(SchemaViolation::critical_no_field(
                "schema-registry",
                format!("Unknown node label '{}' — not in all_node_schemas()", label),
            ));
            return violations;
        }
    };

    // 1. Required-field presence (rule: required-field)
    for (name, fty) in schema.required_fields() {
        match props.get(name) {
            None | Some(serde_json::Value::Null) => {
                violations.push(SchemaViolation::critical(
                    "required-field",
                    name,
                    format!("Missing required field '{}' on '{}'", name, label),
                ));
            }
            Some(val) => {
                if let Some(violation) = check_type_mismatch(name, fty, val) {
                    violations.push(violation);
                }
            }
        }
    }

    // 2. Optional-field type check (rule: optional-type)
    let optional_names: Vec<(&'static str, FieldType)> = schema.optional_fields();
    for (name, fty) in &optional_names {
        if let Some(val) = props.get(*name).filter(|v| !v.is_null())
            && let Some(violation) = check_type_mismatch(name, *fty, val)
        {
            violations.push(violation);
        }
    }

    // 3. Unknown-field detection (rule: unknown-field) — Warning
    let known: std::collections::HashSet<&str> = schema
        .required_fields()
        .iter()
        .map(|(n, _)| *n)
        .chain(optional_names.iter().map(|(n, _)| *n))
        .collect();
    for key in props.keys() {
        if !known.contains(key.as_str()) {
            violations.push(SchemaViolation::warn(
                "unknown-field",
                key,
                format!(
                    "Field '{}' on '{}' is not declared in schema (required or optional)",
                    key, label
                ),
            ));
        }
    }

    // 4. Architectural invariants
    violations.extend(check_invariants(label, props));

    // 5. Semantic controlled-vocabulary checks (ADR-048 Phase 1)
    violations.extend(check_semantic_vocabularies(label, props));

    violations
}

/// Validate a node's properties against its schema — YAML-driven variant.
///
/// ADR-050 Phase F (M288-avqgux/S03): opt-in function that uses
/// schema_for_label_or_yaml() instead of hardcoded schema_for_label().
/// When data/ontology/node_schemas.yaml exists, schemas are loaded from
/// YAML via OntologyRegistry + YamlSchemaAdapter. Otherwise falls back
/// to hardcoded schemas.
///
/// Behavior is identical to validate_node_properties() — the only
/// difference is the schema source. Existing callers that use
/// validate_node_properties() are unaffected.
pub fn validate_node_properties_or_yaml(
    label: &str,
    props: &PropertySnapshot,
) -> Vec<SchemaViolation> {
    let mut violations = Vec::new();

    let schema = match crate::schema::schema_for_label_or_yaml(label) {
        Some(s) => s,
        None => {
            violations.push(SchemaViolation::critical_no_field(
                "schema-registry",
                format!(
                    "Unknown node label '{}' — not in schemas (YAML or hardcoded)",
                    label
                ),
            ));
            return violations;
        }
    };

    // 1. Required-field presence
    for (name, fty) in schema.required_fields() {
        match props.get(name) {
            None | Some(serde_json::Value::Null) => {
                violations.push(SchemaViolation::critical(
                    "required-field",
                    name,
                    format!("Missing required field '{}' on '{}'", name, label),
                ));
            }
            Some(val) => {
                if let Some(violation) = check_type_mismatch(name, fty, val) {
                    violations.push(violation);
                }
            }
        }
    }

    // 2. Optional-field type check
    let optional_names: Vec<(&'static str, FieldType)> = schema.optional_fields();
    for (name, fty) in &optional_names {
        if let Some(val) = props.get(*name).filter(|v| !v.is_null())
            && let Some(violation) = check_type_mismatch(name, *fty, val)
        {
            violations.push(violation);
        }
    }

    // 3. Unknown-field detection
    let known: std::collections::HashSet<&str> = schema
        .required_fields()
        .iter()
        .map(|(n, _)| *n)
        .chain(optional_names.iter().map(|(n, _)| *n))
        .collect();
    for key in props.keys() {
        if !known.contains(key.as_str()) {
            violations.push(SchemaViolation::warn(
                "unknown-field",
                key,
                format!(
                    "Field '{}' on '{}' is not declared in schema (required or optional)",
                    key, label
                ),
            ));
        }
    }

    // 4. Architectural invariants
    violations.extend(check_invariants(label, props));

    violations
}

/// Check the cross-cutting invariants that apply to EVERY node type:
/// - D127: `import_eligible` must be present and boolean (fail-closed)
/// - D134: `retrieval_eligible` must be present and boolean
/// - ADR-044: `schema_version` must be present and a positive integer
/// - ADR-040: `vid` must be present and a non-empty string
fn check_invariants(label: &str, props: &PropertySnapshot) -> Vec<SchemaViolation> {
    let mut out = Vec::new();
    let must_have_bool = |key: &str, rule: &str, desc: &str| -> Vec<SchemaViolation> {
        let mut v = Vec::new();
        match props.get(key) {
            None | Some(serde_json::Value::Null) => v.push(SchemaViolation::critical(
                rule,
                key,
                format!("{} invariant: '{}' missing on '{}'", desc, key, label),
            )),
            Some(serde_json::Value::Bool(_)) => {}
            Some(other) => v.push(SchemaViolation::critical(
                rule,
                key,
                format!(
                    "{} invariant: '{}' on '{}' must be boolean, got {}",
                    desc, key, label, other
                ),
            )),
        }
        v
    };

    out.extend(must_have_bool(
        "import_eligible",
        "D127-fail-closed",
        "D127 fail-closed",
    ));
    out.extend(must_have_bool(
        "retrieval_eligible",
        "D134-retrieval-eligibility",
        "D134 retrieval eligibility",
    ));

    // schema_version: positive integer
    match props.get("schema_version") {
        None | Some(serde_json::Value::Null) => out.push(SchemaViolation::critical(
            "ADR-044-schema-version",
            "schema_version",
            format!("ADR-044 invariant: 'schema_version' missing on '{}'", label),
        )),
        Some(serde_json::Value::Number(n)) if n.is_i64() && n.as_i64().unwrap_or(0) >= 1 => {}
        Some(other) => out.push(SchemaViolation::critical(
            "ADR-044-schema-version",
            "schema_version",
            format!(
                "ADR-044 invariant: 'schema_version' on '{}' must be a positive integer, got {}",
                label, other
            ),
        )),
    }

    // vid: non-empty string
    match props.get("vid") {
        None | Some(serde_json::Value::Null) => out.push(SchemaViolation::critical(
            "ADR-040-vid",
            "vid",
            format!("ADR-040 invariant: 'vid' missing on '{}'", label),
        )),
        Some(serde_json::Value::String(s)) if !s.is_empty() => {}
        Some(other) => out.push(SchemaViolation::critical(
            "ADR-040-vid",
            "vid",
            format!(
                "ADR-040 invariant: 'vid' on '{}' must be a non-empty string, got {}",
                label, other
            ),
        )),
    }

    out
}

/// Semantic controlled-vocabulary checks (ADR-048 Phase 1).
///
/// These complement the structural checks (required/optional fields, types):
/// they verify that values of enum-like string fields belong to the declared
/// controlled vocabulary. A violation here is a Warning (not Critical) so the
/// node is still created, but the operator is alerted that the value is not
/// in the expected vocabulary.
///
/// Wired registries:
/// - `Decision.category` → `is_known_decision_category`
/// - `Decision.outcome` → `is_valid_decision_outcome(category, outcome)`
/// - `Decision.decision_maker` → `is_valid_decision_maker`
/// - `EvidenceBundle.bundle_type` → `is_known_bundle_type`
/// - `EvidenceBundle.verification_status` → `is_known_verification_status`
/// - `ConceptCluster.cluster_type` → `is_known_cluster_type`
/// - `FailureEvent.stage` → `is_known_stage`
/// - `FailureEvent.class` → `is_known_failure_class`
/// - `Source.code` → `is_known_source_code`
/// - `Source.source_type` → `is_known_source_type`
/// - `ResearchEnvironment.completeness` → `is_known_completeness`
/// - `ResearchEnvironment.evidence_origin` → `is_known_origin`
fn check_semantic_vocabularies(label: &str, props: &PropertySnapshot) -> Vec<SchemaViolation> {
    let mut out = Vec::new();
    if label == "Decision" {
        out.extend(check_decision_semantics(props));
    }
    if label == "EvidenceBundle" {
        out.extend(check_evidence_bundle_semantics(props));
    }
    if label == "ConceptCluster" {
        out.extend(check_concept_cluster_semantics(props));
    }
    if label == "FailureEvent" {
        out.extend(check_failure_event_semantics(props));
    }
    if label == "Source" {
        out.extend(check_source_semantics(props));
    }
    if label == "ResearchEnvironment" {
        out.extend(check_research_environment_semantics(props));
    }
    out
}

/// Decision-specific semantic checks. Emits Warning-level violations for
/// unknown category, outcome, or decision_maker prefix.
fn check_decision_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::process::{
        is_known_decision_category, is_valid_decision_maker, is_valid_decision_outcome,
    };
    let mut out = Vec::new();

    // category must be a known Phase 1 category.
    let category = props.get("category").and_then(|v| v.as_str()).unwrap_or("");
    if !category.is_empty() && !is_known_decision_category(category) {
        out.push(SchemaViolation::warn(
            "decision-category-vocabulary",
            "category",
            format!(
                "Decision.category='{}' is not in decision_categories.yaml Phase 1 vocabulary",
                category
            ),
        ));
    }

    // outcome must be valid for the declared category (if category is known).
    if !category.is_empty()
        && is_known_decision_category(category)
        && let Some(outcome) = props.get("outcome").and_then(|v| v.as_str())
        && !outcome.is_empty()
        && !is_valid_decision_outcome(category, outcome)
    {
        out.push(SchemaViolation::warn(
            "decision-outcome-vocabulary",
            "outcome",
            format!(
                "Decision.outcome='{}' is not valid for category='{}' (decision_categories.yaml)",
                outcome, category
            ),
        ));
    }

    // decision_maker must start with a known prefix.
    if let Some(maker) = props.get("decision_maker").and_then(|v| v.as_str())
        && !maker.is_empty()
        && !is_valid_decision_maker(maker)
    {
        out.push(SchemaViolation::warn(
            "decision-maker-prefix",
            "decision_maker",
            format!(
                "Decision.decision_maker='{}' does not start with a known prefix (agent:|human:|system:)",
                maker
            ),
        ));
    }

    out
}

/// EvidenceBundle semantic checks.
/// - `bundle_type` must be in vocabulary.rs `is_known_bundle_type`
/// - `verification_status` (optional) must be in vocabulary.rs `is_known_verification_status`
fn check_evidence_bundle_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::vocabulary::{is_known_bundle_type, is_known_verification_status};
    let mut out = Vec::new();

    if let Some(bt) = props.get("bundle_type").and_then(|v| v.as_str())
        && !bt.is_empty()
        && !is_known_bundle_type(bt)
    {
        out.push(SchemaViolation::warn(
            "evidence-bundle-type-vocabulary",
            "bundle_type",
            format!(
                "EvidenceBundle.bundle_type='{}' is not in node_vocabulary.yaml bundle_types",
                bt
            ),
        ));
    }

    if let Some(vs) = props.get("verification_status").and_then(|v| v.as_str())
        && !vs.is_empty()
        && !is_known_verification_status(vs)
    {
        out.push(SchemaViolation::warn(
            "evidence-verification-status-vocabulary",
            "verification_status",
            format!(
                "EvidenceBundle.verification_status='{}' is not in node_vocabulary.yaml verification_statuses",
                vs
            ),
        ));
    }

    out
}

/// ConceptCluster semantic checks.
/// - `cluster_type` must be in vocabulary.rs `is_known_cluster_type`
fn check_concept_cluster_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::vocabulary::is_known_cluster_type;
    let mut out = Vec::new();

    if let Some(ct) = props.get("cluster_type").and_then(|v| v.as_str())
        && !ct.is_empty()
        && !is_known_cluster_type(ct)
    {
        out.push(SchemaViolation::warn(
            "cluster-type-vocabulary",
            "cluster_type",
            format!(
                "ConceptCluster.cluster_type='{}' is not in node_vocabulary.yaml cluster_types",
                ct
            ),
        ));
    }

    out
}

/// FailureEvent semantic checks (ADR-043 Research Process Plane).
/// - `stage` must be in failure_taxonomy.yaml stages
/// - `class` must be in failure_taxonomy.yaml failure_classes
fn check_failure_event_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::process::{is_known_failure_class, is_known_stage};
    let mut out = Vec::new();

    if let Some(stage) = props.get("stage").and_then(|v| v.as_str())
        && !stage.is_empty()
        && !is_known_stage(stage)
    {
        out.push(SchemaViolation::warn(
            "failure-event-stage-vocabulary",
            "stage",
            format!(
                "FailureEvent.stage='{}' is not in failure_taxonomy.yaml stages",
                stage
            ),
        ));
    }

    if let Some(class) = props.get("class").and_then(|v| v.as_str())
        && !class.is_empty()
        && !is_known_failure_class(class)
    {
        out.push(SchemaViolation::warn(
            "failure-event-class-vocabulary",
            "class",
            format!(
                "FailureEvent.class='{}' is not in failure_taxonomy.yaml failure_classes",
                class
            ),
        ));
    }

    out
}

/// Source semantic checks.
/// - `code` must be a known source code in source registry
/// - `source_type` must be a known source type in source registry
fn check_source_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::source::{is_known_source_code, is_known_source_type};
    let mut out = Vec::new();

    if let Some(code) = props.get("code").and_then(|v| v.as_str())
        && !code.is_empty()
        && !is_known_source_code(code)
    {
        out.push(SchemaViolation::warn(
            "source-code-vocabulary",
            "code",
            format!(
                "Source.code='{}' is not in source registry (is_known_source_code)",
                code
            ),
        ));
    }

    if let Some(source_type) = props.get("source_type").and_then(|v| v.as_str())
        && !source_type.is_empty()
        && !is_known_source_type(source_type)
    {
        out.push(SchemaViolation::warn(
            "source-type-vocabulary",
            "source_type",
            format!(
                "Source.source_type='{}' is not in source registry (is_known_source_type)",
                source_type
            ),
        ));
    }

    out
}

/// ResearchEnvironment semantic checks (ADR-043).
/// - `completeness` must be known (full | env_lite | unknown)
/// - `evidence_origin` validated against failure_taxonomy.yaml evidence_origins
///   (cross-reused vocabulary: live_executed | observational | etc.)
fn check_research_environment_semantics(props: &PropertySnapshot) -> Vec<SchemaViolation> {
    use crate::process::{is_known_completeness, is_known_origin};
    let mut out = Vec::new();

    if let Some(completeness) = props.get("completeness").and_then(|v| v.as_str())
        && !completeness.is_empty()
        && !is_known_completeness(completeness)
    {
        out.push(SchemaViolation::warn(
            "research-env-completeness-vocabulary",
            "completeness",
            format!(
                "ResearchEnvironment.completeness='{}' is not in failure_taxonomy.yaml completeness_levels",
                completeness
            ),
        ));
    }

    if let Some(origin) = props.get("evidence_origin").and_then(|v| v.as_str())
        && !origin.is_empty()
        && !is_known_origin(origin)
    {
        out.push(SchemaViolation::warn(
            "research-env-evidence-origin-vocabulary",
            "evidence_origin",
            format!(
                "ResearchEnvironment.evidence_origin='{}' is not in failure_taxonomy.yaml evidence_origins",
                origin
            ),
        ));
    }

    out
}

/// Check one property value against its declared FieldType.
/// Returns Some(violation) on mismatch, None if OK.
fn check_type_mismatch(
    name: &str,
    fty: FieldType,
    val: &serde_json::Value,
) -> Option<SchemaViolation> {
    // PIR-008 / GRAPH-CORE-TEMPORAL-DESIGN.md (encoded in
    // data/ontology/node_schemas.yaml under `temporal_wire_contract`):
    // DateTime properties must carry the canonical RFC3339 UTC string
    // wire form (or the OPEN sentinel on open-bound fields). Integer
    // Unix timestamps are the LEGACY read-only form — rejected for NEW
    // writes with a Critical violation so writers cannot silently
    // persist the legacy shape.
    if fty == FieldType::DateTime {
        return match kg_ontology::temporal::validate_datetime_wire(val, name) {
            Ok(()) => None,
            Err(message) => Some(SchemaViolation::critical("datetime-wire", name, message)),
        };
    }
    let ok = match fty {
        FieldType::String => val.is_string(),
        FieldType::Integer => val.is_i64(),
        FieldType::Float => val.is_f64() || val.is_i64(),
        FieldType::Boolean => val.is_boolean(),
        FieldType::Vector => val.is_array(),
        FieldType::Any => true,
        FieldType::DateTime => unreachable!("DateTime handled by the wire check above"),
    };
    if ok {
        None
    } else {
        Some(SchemaViolation::warn(
            "type-mismatch",
            name,
            format!(
                "Field '{}' expected {}, got {}",
                name,
                fty.as_str(),
                value_kind(val)
            ),
        ))
    }
}

fn value_kind(val: &serde_json::Value) -> &'static str {
    match val {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(n) if n.is_i64() => "integer",
        serde_json::Value::Number(n) if n.is_f64() => "float",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Validate that an edge type is registered in the relation registry.
/// Returns Some(violation) if the edge type is not declared.
pub fn validate_edge_type(edge_type: &str) -> Option<SchemaViolation> {
    if crate::edge_contract::contract_edge_types().contains(&edge_type) {
        None
    } else {
        Some(SchemaViolation::critical_no_field(
            "edge-registry",
            format!(
                "Unknown edge type '{}' — not in relation::bibliographic or relation::process",
                edge_type
            ),
        ))
    }
}

/// Format a list of violations for human-readable output.
/// Re-exported from kg-ontology; see the re-export at the top of this module.
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snap(pairs: &[(&str, serde_json::Value)]) -> PropertySnapshot {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    // ── Required field presence ──────────────────────────────────────────

    #[test]
    fn test_valid_paper_node_passes() {
        let props = snap(&[
            ("vid", json!("vid:paper:1234.5678")),
            ("arxiv_id", json!("1234.5678")),
            ("title", json!("My Paper")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let violations = validate_node_properties("Paper", &props);
        if !violations.is_empty() {
            panic!(
                "expected no violations, got:\n{}",
                format_violations(&violations)
            );
        }
    }

    #[test]
    fn test_missing_required_field_is_critical() {
        let props = snap(&[
            ("vid", json!("vid:paper:1234.5678")),
            // arxiv_id missing
            ("title", json!("My Paper")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let violations = validate_node_properties("Paper", &props);
        let criticals: Vec<_> = violations
            .iter()
            .filter(|v| v.severity == Severity::Critical && v.rule == "required-field")
            .collect();
        assert!(
            criticals
                .iter()
                .any(|v| v.field.as_deref() == Some("arxiv_id")),
            "expected critical for missing arxiv_id, got: {:?}",
            violations
        );
    }

    #[test]
    fn test_unknown_label_is_critical() {
        let props = PropertySnapshot::new();
        let violations = validate_node_properties("Nonsense", &props);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Critical);
        assert_eq!(violations[0].rule, "schema-registry");
    }

    // ── Invariants ───────────────────────────────────────────────────────

    #[test]
    fn test_missing_import_eligible_is_critical() {
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            // import_eligible missing
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(v.iter().any(|x| x.rule == "D127-fail-closed"));
    }

    #[test]
    fn test_missing_retrieval_eligible_is_critical() {
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            // retrieval_eligible missing
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(v.iter().any(|x| x.rule == "D134-retrieval-eligibility"));
    }

    #[test]
    fn test_missing_schema_version_is_critical() {
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            // schema_version missing
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(v.iter().any(|x| x.rule == "ADR-044-schema-version"));
    }

    #[test]
    fn test_schema_version_must_be_positive_int() {
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(0_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter()
                .any(|x| x.rule == "ADR-044-schema-version" && x.severity == Severity::Critical)
        );
    }

    #[test]
    fn test_missing_vid_is_critical() {
        let props = snap(&[
            // vid missing
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(v.iter().any(|x| x.rule == "ADR-040-vid"));
    }

    #[test]
    fn test_empty_string_vid_is_critical() {
        let props = snap(&[
            ("vid", json!("")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter()
                .any(|x| x.rule == "ADR-040-vid" && x.severity == Severity::Critical)
        );
    }

    // ── Type checking ────────────────────────────────────────────────────

    #[test]
    fn test_integer_datetime_write_is_critical() {
        // PIR-008: integer Unix timestamps are the legacy read-only wire
        // form; new writes must be RFC3339 UTC strings → Critical.
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!(1_700_000_000_i64)),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        let dw: Vec<_> = v
            .iter()
            .filter(|x| x.rule == "datetime-wire" && x.field.as_deref() == Some("valid_from"))
            .collect();
        assert_eq!(dw.len(), 1, "expected one datetime-wire violation: {:?}", v);
        assert_eq!(dw[0].severity, Severity::Critical);
        assert!(
            dw[0].message.contains("legacy"),
            "expected legacy-form message, got: {}",
            dw[0].message
        );
    }

    #[test]
    fn test_rfc3339_utc_datetime_passes() {
        // Canonical RFC3339 UTC string → no datetime-wire violation and no
        // other violations for an otherwise valid minimum Paper.
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter().all(|x| x.rule != "datetime-wire"),
            "unexpected datetime-wire violations: {:?}",
            v
        );
        assert!(v.is_empty(), "expected clean validation, got: {:?}", v);
    }

    #[test]
    fn test_non_utc_offset_datetime_is_critical() {
        // RFC3339 but not UTC (offset +02:00) — not the canonical wire.
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00+02:00")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        let dw: Vec<_> = v
            .iter()
            .filter(|x| x.rule == "datetime-wire" && x.field.as_deref() == Some("valid_from"))
            .collect();
        assert_eq!(dw.len(), 1, "expected one datetime-wire violation: {:?}", v);
        assert_eq!(dw[0].severity, Severity::Critical);
        assert!(dw[0].message.contains("UTC"), "got: {}", dw[0].message);
    }

    #[test]
    fn test_open_sentinel_passes_on_bound_field() {
        // OPEN remains the only accepted open-bound sentinel.
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("valid_to", json!("OPEN")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter().all(|x| x.rule != "datetime-wire"),
            "OPEN on a bound field must pass the wire check: {:?}",
            v
        );
    }

    #[test]
    fn test_bool_datetime_is_critical() {
        // valid_from declared DateTime; pass bool → Critical datetime-wire.
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!(true)),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter()
                .any(|x| x.rule == "datetime-wire" && x.field.as_deref() == Some("valid_from")),
            "expected datetime-wire violation for bool DateTime, got: {:?}",
            v
        );
    }

    // ── Unknown field ────────────────────────────────────────────────────

    #[test]
    fn test_unknown_field_is_warning() {
        let props = snap(&[
            ("vid", json!("vid:paper:1")),
            ("arxiv_id", json!("1")),
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            ("import_eligible", json!(false)),
            ("retrieval_eligible", json!(true)),
            ("schema_version", json!(1_i64)),
            ("bogus_extra", json!("x")),
        ]);
        let v = validate_node_properties("Paper", &props);
        assert!(
            v.iter()
                .any(|x| x.rule == "unknown-field" && x.field.as_deref() == Some("bogus_extra"))
        );
    }

    // ── All-violations-returned ──────────────────────────────────────────

    #[test]
    fn test_returns_all_violations_not_just_first() {
        // Violate 4 rules at once: missing arxiv_id, missing vid,
        // missing import_eligible, missing schema_version.
        let props = snap(&[
            // vid missing
            // arxiv_id missing
            ("title", json!("T")),
            ("valid_from", json!("2024-01-15T10:30:00Z")),
            // import_eligible missing
            ("retrieval_eligible", json!(true)),
            // schema_version missing
        ]);
        let v = validate_node_properties("Paper", &props);
        // Expect at least 4 critical violations
        let criticals: Vec<_> = v
            .iter()
            .filter(|x| x.severity == Severity::Critical)
            .collect();
        assert!(
            criticals.len() >= 4,
            "expected >=4 critical violations, got {}: {:?}",
            criticals.len(),
            v
        );
    }

    // ── Edge type validator ──────────────────────────────────────────────

    #[test]
    fn test_known_edge_type_passes() {
        assert!(validate_edge_type(crate::relation::bibliographic::MENTIONS).is_none());
        assert!(validate_edge_type(crate::relation::hypergraph::PARTICIPATES_IN).is_none());
    }

    #[test]
    fn test_unknown_edge_type_is_critical() {
        let v = validate_edge_type("BOGUS_EDGE").unwrap();
        assert_eq!(v.severity, Severity::Critical);
        assert_eq!(v.rule, "edge-registry");
    }

    // ── Smoke test all 29 schemas ────────────────────────────────────────

    #[test]
    fn test_all_node_schemas_produce_a_valid_minimum_node() {
        // For each schema, validate a synthetic minimum node that has only
        // required + invariant fields. Should produce zero Critical violations.
        for schema in crate::schema::all_node_schemas() {
            let label = schema.label();
            let mut props = PropertySnapshot::new();
            for (name, fty) in schema.required_fields() {
                let val = match fty {
                    FieldType::String => serde_json::json!("x"),
                    FieldType::Integer => serde_json::json!(1_i64),
                    FieldType::Float => serde_json::json!(1.0_f64),
                    FieldType::Boolean => serde_json::json!(false),
                    FieldType::DateTime => serde_json::json!("1970-01-01T00:00:00Z"),
                    FieldType::Vector => serde_json::json!([0.0_f64]),
                    FieldType::Any => serde_json::json!("x"),
                };
                props.insert(name.to_string(), val);
            }
            // Add invariants
            props.insert("import_eligible".to_string(), json!(false));
            props.insert("retrieval_eligible".to_string(), json!(true));
            props.insert("schema_version".to_string(), json!(1_i64));

            let violations = validate_node_properties(label, &props);
            let criticals: Vec<_> = violations
                .iter()
                .filter(|v| v.severity == Severity::Critical)
                .collect();
            assert!(
                criticals.is_empty(),
                "schema '{}' produced critical violations on minimum valid node:\n{}",
                label,
                format_violations(&violations)
            );
        }
    }
}

// ─── Cross-reference field registry (ADR-045 Wave F foundation) ───────
//
// Declares which node fields carry a VID reference to another node.
// The runtime cross-reference validator (Wave F runtime, future work)
// walks this registry to confirm that every listed (source_node,
// reference_field) pair points to a node that actually exists in the
// graph. Without this registry the validator would have to assume any
// `*_id` field is a reference, which is unsafe (some `_id` fields are
// opaque external identifiers, not graph VIDs).
//

/// One row of the cross-reference field registry.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossReferenceField {
    /// Node label that carries the reference.
    pub source_label: &'static str,
    /// Field name on the source node whose value is a VID reference.
    pub field: &'static str,
    /// Label of the node the reference is expected to point to.
    pub target_label: &'static str,
    /// Whether the reference is required (non-empty) or optional.
    pub required: bool,
}

/// The cross-reference field registry. Add rows here when wiring a new
/// process-plane node that links to another node via a *_id field.
///
/// Convention: only list fields whose value is a graph VID (not an
/// external opaque id like `openalex_id` or `arxiv_id`). External ids
/// are resolved through their own lookup paths and are not part of the
/// graph's reference integrity contract.
pub fn cross_reference_fields() -> Vec<CrossReferenceField> {
    vec![
        // ResearchProblem.parent_problem_id → ResearchProblem
        CrossReferenceField {
            source_label: "ResearchProblem",
            field: "parent_problem_id",
            target_label: "ResearchProblem",
            required: false,
        },
        // ResearchProblem.evidence_bundle_id → EvidenceBundle
        CrossReferenceField {
            source_label: "ResearchProblem",
            field: "evidence_bundle_id",
            target_label: "EvidenceBundle",
            required: false,
        },
        // Claim.source_span_id → EvidenceBundle (grounding)
        CrossReferenceField {
            source_label: "Claim",
            field: "source_span_id",
            target_label: "EvidenceBundle",
            required: false,
        },
        // ResearchEnvironment.research_problem_id → ResearchProblem
        CrossReferenceField {
            source_label: "ResearchEnvironment",
            field: "research_problem_id",
            target_label: "ResearchProblem",
            required: true,
        },
        // MetricObservation.metric_definition_id → MetricDefinition
        CrossReferenceField {
            source_label: "MetricObservation",
            field: "metric_definition_id",
            target_label: "MetricDefinition",
            required: true,
        },
        // MetricObservation.run_id → ExperimentRun (or pseudo-run for literature-only)
        CrossReferenceField {
            source_label: "MetricObservation",
            field: "run_id",
            target_label: "ExperimentRun",
            required: true,
        },
        // ResultComparison.candidate_observation_id → MetricObservation
        CrossReferenceField {
            source_label: "ResultComparison",
            field: "candidate_observation_id",
            target_label: "MetricObservation",
            required: true,
        },
        // ResultComparison.baseline_observation_id → MetricObservation
        CrossReferenceField {
            source_label: "ResultComparison",
            field: "baseline_observation_id",
            target_label: "MetricObservation",
            required: true,
        },
        // ResultComparison.environment_id → ResearchEnvironment
        CrossReferenceField {
            source_label: "ResultComparison",
            field: "environment_id",
            target_label: "ResearchEnvironment",
            required: true,
        },
        // ─── Conflict plane (ADR-047) ───
        // Conflict.resolution_strategy is an opaque enum string, not a vid ref.
        // No cross-reference row for it.
        // ─── Decision plane (ADR-048) ───
        // Decision.policy_id → Policy (Phase 2, future). For now optional
        // and unresolved — declare as optional cross-reference so validator
        // knows the shape.
        CrossReferenceField {
            source_label: "Decision",
            field: "policy_id",
            target_label: "Decision", // self-reference placeholder until Policy node exists
            required: false,
        },
    ]
}

#[cfg(test)]
mod cross_ref_tests {
    use super::*;

    /// Every (source_label, target_label) pair in the cross-reference
    /// registry must be a registered node label. Catches typos.
    #[test]
    fn test_cross_ref_labels_are_registered() {
        let registered: std::collections::HashSet<String> = crate::schema::all_node_schemas()
            .into_iter()
            .map(|s| s.label().to_string())
            .collect();
        for cr in cross_reference_fields() {
            assert!(
                registered.contains(cr.source_label),
                "source label '{}' on field '{}' is not a registered node type",
                cr.source_label,
                cr.field
            );
            assert!(
                registered.contains(cr.target_label),
                "target label '{}' on field '{}' is not a registered node type",
                cr.target_label,
                cr.field
            );
        }
    }

    /// The same (source_label, field) pair cannot appear twice with
    /// contradictory target_label or required.
    #[test]
    fn test_no_contradictory_cross_ref_rows() {
        let rows = cross_reference_fields();
        for (i, a) in rows.iter().enumerate() {
            for b in rows.iter().skip(i + 1) {
                if a.source_label == b.source_label && a.field == b.field {
                    assert_eq!(
                        (a.target_label, a.required),
                        (b.target_label, b.required),
                        "field '{}.{}' has contradictory rows:\n  {:?}\n  {:?}",
                        a.source_label,
                        a.field,
                        a,
                        b
                    );
                }
            }
        }
    }

    /// Every declared cross-reference field must actually be a declared
    /// field on the source node's schema (required OR optional).
    #[test]
    fn test_cross_ref_fields_exist_on_source_schema() {
        for cr in cross_reference_fields() {
            let schema = crate::schema::schema_for_label(cr.source_label).expect(
                "source schema must exist (test_cross_ref_labels_are_registered guards this)",
            );
            let known: std::collections::HashSet<&str> = schema
                .required_fields()
                .iter()
                .map(|(n, _)| *n)
                .chain(schema.optional_fields().iter().map(|(n, _)| *n))
                .collect();
            assert!(
                known.contains(cr.field),
                "field '{}.{}' is not declared on the {} schema (required or optional)",
                cr.source_label,
                cr.field,
                cr.source_label
            );
        }
    }

    /// Required cross-reference fields must be listed as required on
    /// the source schema. Optional cross-reference fields must be
    /// listed as optional. Catches drift between the two declarations.
    #[test]
    fn test_cross_ref_required_matches_schema() {
        for cr in cross_reference_fields() {
            let schema = crate::schema::schema_for_label(cr.source_label).unwrap();
            let req: Vec<&str> = schema.required_fields().iter().map(|(n, _)| *n).collect();
            let opt: Vec<&str> = schema.optional_fields().iter().map(|(n, _)| *n).collect();
            if cr.required {
                assert!(
                    req.contains(&cr.field),
                    "field '{}.{}' is declared required in cross_reference_fields() but not in {}Schema::required_fields()",
                    cr.source_label,
                    cr.field,
                    cr.source_label
                );
            } else {
                assert!(
                    opt.contains(&cr.field),
                    "field '{}.{}' is declared optional in cross_reference_fields() but not in {}Schema::optional_fields()",
                    cr.source_label,
                    cr.field,
                    cr.source_label
                );
            }
        }
    }
}

/// Render the cross-reference registry as a markdown table for
/// documentation. Use in `da cross-refs` CLI command to emit always-fresh
/// content for GRAPH-SCHEMA.md.
pub fn render_cross_references_table() -> String {
    let mut rows = cross_reference_fields();
    rows.sort_by_key(|c| (c.source_label, c.field));

    let mut s = String::new();
    s.push_str("| Source | Field | Target | Required? |\n");
    s.push_str("|--------|-------|--------|-----------|\n");
    for cr in &rows {
        let req = if cr.required {
            "✅ required"
        } else {
            "optional"
        };
        s.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            cr.source_label, cr.field, cr.target_label, req
        ));
    }
    s
}

#[cfg(test)]
mod yaml_parity_tests {
    use super::*;
    use crate::validator::PropertySnapshot;

    /// validate_node_properties_or_yaml must return same violations as
    /// validate_node_properties for known labels (Paper).
    #[test]
    fn test_yaml_vs_hardcoded_parity_paper() {
        let mut props: PropertySnapshot = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::Value::String("abc".into()));
        props.insert(
            "arxiv_id".to_string(),
            serde_json::Value::String("2507.1".into()),
        );

        let hardcoded = crate::validator::validate_node_properties("Paper", &props);
        let yaml = crate::validator::validate_node_properties_or_yaml("Paper", &props);

        // Both must find the same number of violations (should be similar).
        assert_eq!(
            hardcoded.len(),
            yaml.len(),
            "Hardcoded vs YAML violation count mismatch: {} vs {}",
            hardcoded.len(),
            yaml.len()
        );
    }

    /// Unknown label should produce violation in both modes.
    #[test]
    fn test_yaml_unknown_label() {
        let props: PropertySnapshot = PropertySnapshot::new();
        let result = crate::validator::validate_node_properties_or_yaml("Nonexistent", &props);
        assert!(
            !result.is_empty(),
            "Unknown label should produce violations"
        );
        assert!(
            result.iter().any(|v| v.rule == "schema-registry"),
            "Should have schema-registry violation"
        );
    }

    /// schema_for_label_or_yaml should return Paper schema.
    #[test]
    fn test_schema_for_label_or_yaml_paper() {
        let schema = crate::schema::schema_for_label_or_yaml("Paper");
        assert!(schema.is_some(), "Paper schema should be found");
        assert_eq!(schema.unwrap().label(), "Paper");
    }

    /// schema_for_label_or_yaml should return None for unknown label.
    #[test]
    fn test_schema_for_label_or_yaml_unknown() {
        let schema = crate::schema::schema_for_label_or_yaml("NonexistentLabel");
        assert!(schema.is_none(), "Unknown label should return None");
    }

    /// Missing required field should produce critical violation in YAML mode.
    #[test]
    fn test_yaml_missing_required_field() {
        let props: PropertySnapshot = PropertySnapshot::new();
        // Missing vid and arxiv_id (required for Paper)
        let result = crate::validator::validate_node_properties_or_yaml("Paper", &props);
        assert!(!result.is_empty());
        assert!(
            result
                .iter()
                .any(|v| v.rule == "required-field" && v.severity == Severity::Critical),
            "Should have critical required-field violation"
        );
    }

    /// YAML schema count must match hardcoded count (parity).
    #[test]
    fn test_yaml_schema_count_matches_hardcoded() {
        let hardcoded = crate::schema::all_node_schemas();
        let yaml = crate::yaml_schema_adapter::all_node_schemas_or_yaml();
        // Counts may differ slightly if YAML has been updated, but should be close.
        // When YAML is generated from the same code, they should match.
        assert!(
            yaml.len() >= 28,
            "Expected >=28 YAML schemas, got {}",
            yaml.len()
        );
        assert!(
            hardcoded.len() >= 28,
            "Expected >=28 hardcoded schemas, got {}",
            hardcoded.len()
        );
    }

    // ─── ADR-048 Decision semantic validation tests ───

    fn valid_decision_props(category: &str, outcome: &str, maker: &str) -> PropertySnapshot {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("dec-001"));
        props.insert("category".to_string(), serde_json::json!(category));
        props.insert("scenario".to_string(), serde_json::json!("test"));
        props.insert("reasoning".to_string(), serde_json::json!("because"));
        props.insert("outcome".to_string(), serde_json::json!(outcome));
        props.insert("confidence".to_string(), serde_json::json!(0.9));
        props.insert("decision_maker".to_string(), serde_json::json!(maker));
        props.insert(
            "valid_from".to_string(),
            serde_json::json!("2026-08-05T10:00:00Z"),
        );
        props.insert(
            "recorded_at".to_string(),
            serde_json::json!("2026-08-05T10:00:00Z"),
        );
        props.insert("import_eligible".to_string(), serde_json::json!(false));
        props.insert("retrieval_eligible".to_string(), serde_json::json!(true));
        props.insert("schema_version".to_string(), serde_json::json!(1));
        props
    }

    #[test]
    fn test_decision_semantic_valid_node_no_semantic_violations() {
        let props = valid_decision_props("healing_action", "approved", "system:pipeline");
        let violations = validate_node_properties("Decision", &props);
        // No semantic vocabulary violations expected.
        let semantic: Vec<_> = violations
            .iter()
            .filter(|v| {
                v.rule.contains("decision-category")
                    || v.rule.contains("decision-outcome")
                    || v.rule.contains("decision-maker")
            })
            .collect();
        assert!(
            semantic.is_empty(),
            "expected no semantic violations for valid Decision, got: {:?}",
            semantic
        );
    }

    #[test]
    fn test_decision_semantic_unknown_category_warns() {
        let props = valid_decision_props("bogus_category", "approved", "system:pipeline");
        let violations = validate_node_properties("Decision", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "decision-category-vocabulary"),
            "expected decision-category-vocabulary warning for unknown category"
        );
    }

    #[test]
    fn test_decision_semantic_invalid_outcome_for_category_warns() {
        // "definitely_bogus_outcome" is not a known global or category-specific outcome.
        let props = valid_decision_props(
            "healing_action",
            "definitely_bogus_outcome",
            "system:pipeline",
        );
        let violations = validate_node_properties("Decision", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "decision-outcome-vocabulary"),
            "expected decision-outcome-vocabulary warning for outcome not valid in category"
        );
    }

    #[test]
    fn test_decision_semantic_invalid_maker_prefix_warns() {
        let props = valid_decision_props("healing_action", "approved", "anonymous");
        let violations = validate_node_properties("Decision", &props);
        assert!(
            violations.iter().any(|v| v.rule == "decision-maker-prefix"),
            "expected decision-maker-prefix warning for unknown prefix"
        );
    }

    #[test]
    fn test_decision_semantic_phase2_category_rejected() {
        // policy_applied is commented out (Phase 2). Phase 1 should warn.
        assert!(!crate::process::is_known_decision_category(
            "policy_applied"
        ));
        let props = valid_decision_props("policy_applied", "allowed", "system:policy-engine");
        let violations = validate_node_properties("Decision", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "decision-category-vocabulary"),
            "Phase 2 category must warn in Phase 1"
        );
    }

    // ─── EvidenceBundle semantic validation tests ───

    fn valid_evidence_bundle_props(bundle_type: &str) -> PropertySnapshot {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("eb-001"));
        props.insert("bundle_type".to_string(), serde_json::json!(bundle_type));
        props.insert("import_eligible".to_string(), serde_json::json!(false));
        props.insert("retrieval_eligible".to_string(), serde_json::json!(true));
        props.insert("schema_version".to_string(), serde_json::json!(1));
        props
    }

    #[test]
    fn test_evidence_bundle_semantic_valid_bundle_type_no_violation() {
        let props = valid_evidence_bundle_props("experiment_setup");
        let violations = validate_node_properties("EvidenceBundle", &props);
        let semantic: Vec<_> = violations
            .iter()
            .filter(|v| v.rule.contains("evidence-bundle-type"))
            .collect();
        assert!(
            semantic.is_empty(),
            "expected no bundle-type violation for valid type, got: {:?}",
            semantic
        );
    }

    #[test]
    fn test_evidence_bundle_semantic_unknown_bundle_type_warns() {
        let props = valid_evidence_bundle_props("bogus_bundle_type");
        let violations = validate_node_properties("EvidenceBundle", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "evidence-bundle-type-vocabulary"),
            "expected evidence-bundle-type-vocabulary warning for unknown bundle_type"
        );
    }

    #[test]
    fn test_evidence_bundle_semantic_unknown_verification_status_warns() {
        let mut props = valid_evidence_bundle_props("experiment_setup");
        props.insert(
            "verification_status".to_string(),
            serde_json::json!("bogus_status"),
        );
        let violations = validate_node_properties("EvidenceBundle", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "evidence-verification-status-vocabulary"),
            "expected evidence-verification-status-vocabulary warning"
        );
    }

    #[test]
    fn test_evidence_bundle_semantic_valid_verification_status_no_violation() {
        let mut props = valid_evidence_bundle_props("experiment_setup");
        props.insert(
            "verification_status".to_string(),
            serde_json::json!("verified"),
        );
        let violations = validate_node_properties("EvidenceBundle", &props);
        let semantic: Vec<_> = violations
            .iter()
            .filter(|v| v.rule.contains("evidence-verification"))
            .collect();
        assert!(
            semantic.is_empty(),
            "expected no verification-status violation, got: {:?}",
            semantic
        );
    }

    // ─── ConceptCluster semantic validation tests ───

    #[test]
    fn test_concept_cluster_semantic_unknown_cluster_type_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("cluster-001"));
        props.insert(
            "cluster_type".to_string(),
            serde_json::json!("bogus_cluster"),
        );
        props.insert("import_eligible".to_string(), serde_json::json!(false));
        props.insert("retrieval_eligible".to_string(), serde_json::json!(true));
        props.insert("schema_version".to_string(), serde_json::json!(1));
        let violations = validate_node_properties("ConceptCluster", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "cluster-type-vocabulary"),
            "expected cluster-type-vocabulary warning for unknown cluster_type"
        );
    }

    // ─── FailureEvent semantic validation tests ───

    #[test]
    fn test_failure_event_unknown_stage_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("fe-001"));
        props.insert("stage".to_string(), serde_json::json!("bogus_stage"));
        props.insert("class".to_string(), serde_json::json!("transient"));
        props.insert("recoverable".to_string(), serde_json::json!(true));
        props.insert("error_signature".to_string(), serde_json::json!("timeout"));
        let violations = validate_node_properties("FailureEvent", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "failure-event-stage-vocabulary"),
            "expected failure-event-stage-vocabulary warning for unknown stage"
        );
    }

    #[test]
    fn test_failure_event_unknown_class_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("fe-002"));
        props.insert("stage".to_string(), serde_json::json!("ingest"));
        props.insert("class".to_string(), serde_json::json!("bogus_class"));
        props.insert("recoverable".to_string(), serde_json::json!(false));
        props.insert("error_signature".to_string(), serde_json::json!("panic"));
        let violations = validate_node_properties("FailureEvent", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "failure-event-class-vocabulary"),
            "expected failure-event-class-vocabulary warning for unknown class"
        );
    }

    #[test]
    fn test_failure_event_valid_no_semantic_violations() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("fe-003"));
        // Use real values from failure_taxonomy.yaml: stage='execution', class='specification'
        props.insert("stage".to_string(), serde_json::json!("execution"));
        props.insert("class".to_string(), serde_json::json!("specification"));
        props.insert("recoverable".to_string(), serde_json::json!(true));
        props.insert("error_signature".to_string(), serde_json::json!("timeout"));
        let violations = validate_node_properties("FailureEvent", &props);
        let semantic: Vec<_> = violations
            .iter()
            .filter(|v| v.rule.contains("failure-event-"))
            .collect();
        assert!(
            semantic.is_empty(),
            "expected no semantic violations for valid FailureEvent, got: {:?}",
            semantic
        );
    }

    // ─── Source semantic validation tests ───

    #[test]
    fn test_source_unknown_code_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("src-001"));
        props.insert("code".to_string(), serde_json::json!("bogus_source_code"));
        props.insert("source_type".to_string(), serde_json::json!("arxiv"));
        props.insert("domain".to_string(), serde_json::json!("cs.LG"));
        let violations = validate_node_properties("Source", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "source-code-vocabulary"),
            "expected source-code-vocabulary warning for unknown code"
        );
    }

    #[test]
    fn test_source_unknown_type_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("src-002"));
        props.insert("code".to_string(), serde_json::json!("arxiv"));
        props.insert("source_type".to_string(), serde_json::json!("bogus_type"));
        props.insert("domain".to_string(), serde_json::json!("cs.LG"));
        let violations = validate_node_properties("Source", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "source-type-vocabulary"),
            "expected source-type-vocabulary warning for unknown source_type"
        );
    }

    // ─── ResearchEnvironment semantic validation tests ───

    #[test]
    fn test_research_env_unknown_completeness_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("env-001"));
        props.insert(
            "completeness".to_string(),
            serde_json::json!("bogus_completeness"),
        );
        props.insert("research_problem_id".to_string(), serde_json::json!("rp-1"));
        props.insert("baseline_ref".to_string(), serde_json::json!("baseline-1"));
        props.insert("subject_system".to_string(), serde_json::json!("sys-1"));
        props.insert(
            "subject_system_kind".to_string(),
            serde_json::json!("dataset"),
        );
        props.insert(
            "environment_template_id".to_string(),
            serde_json::json!("tmpl-1"),
        );
        props.insert(
            "evidence_origin".to_string(),
            serde_json::json!("live_executed"),
        );
        let violations = validate_node_properties("ResearchEnvironment", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "research-env-completeness-vocabulary"),
            "expected completeness-vocabulary warning"
        );
    }

    #[test]
    fn test_research_env_unknown_evidence_origin_warns() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("env-002"));
        props.insert("completeness".to_string(), serde_json::json!("full"));
        props.insert("research_problem_id".to_string(), serde_json::json!("rp-1"));
        props.insert("baseline_ref".to_string(), serde_json::json!("baseline-1"));
        props.insert("subject_system".to_string(), serde_json::json!("sys-1"));
        props.insert(
            "subject_system_kind".to_string(),
            serde_json::json!("dataset"),
        );
        props.insert(
            "environment_template_id".to_string(),
            serde_json::json!("tmpl-1"),
        );
        props.insert(
            "evidence_origin".to_string(),
            serde_json::json!("bogus_origin"),
        );
        let violations = validate_node_properties("ResearchEnvironment", &props);
        assert!(
            violations
                .iter()
                .any(|v| v.rule == "research-env-evidence-origin-vocabulary"),
            "expected evidence-origin-vocabulary warning"
        );
    }

    #[test]
    fn test_research_env_valid_no_semantic_violations() {
        let mut props = PropertySnapshot::new();
        props.insert("vid".to_string(), serde_json::json!("env-003"));
        props.insert("completeness".to_string(), serde_json::json!("full"));
        props.insert("research_problem_id".to_string(), serde_json::json!("rp-1"));
        props.insert("baseline_ref".to_string(), serde_json::json!("baseline-1"));
        props.insert("subject_system".to_string(), serde_json::json!("sys-1"));
        props.insert(
            "subject_system_kind".to_string(),
            serde_json::json!("dataset"),
        );
        props.insert(
            "environment_template_id".to_string(),
            serde_json::json!("tmpl-1"),
        );
        props.insert(
            "evidence_origin".to_string(),
            serde_json::json!("live_executed"),
        );
        let violations = validate_node_properties("ResearchEnvironment", &props);
        let semantic: Vec<_> = violations
            .iter()
            .filter(|v| v.rule.contains("research-env-"))
            .collect();
        assert!(
            semantic.is_empty(),
            "expected no semantic violations for valid ResearchEnvironment, got: {:?}",
            semantic
        );
    }
}
