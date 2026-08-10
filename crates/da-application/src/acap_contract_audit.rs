//! Detect-only ACAP contract convergence audit (M371/S07/T01).
//!
//! Compares ACAP surface observations (policy YAML, Rust DTO constants,
//! embedded reports, governor expectations) to expose schema collisions,
//! type drift, layer drift and missing canonical aggregate evidence.
//!
//! This use case never loads files, mutates the graph, or grants import
//! authority. All observations are injected as typed DTOs.

use std::collections::BTreeMap;

use da_domain::ACAP_AGGREGATE_REPORT_SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ACAP_AUDIT_INPUT_SCHEMA_VERSION: &str = "acap-contract-audit-input.v1";
pub const ACAP_AUDIT_REPORT_SCHEMA_VERSION: &str = "acap-contract-audit-report.v1";

// ---------------------------------------------------------------------------
// Observations — typed values injected by the CLI composition root.
// ---------------------------------------------------------------------------

/// One policy layer observed from the YAML surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct PolicyLayerObservation {
    pub id: String,
    pub enforcement: String,
    pub state: String,
    pub hard_metrics: Vec<String>,
    pub quality_metrics: Vec<String>,
    pub source_ref: String,
}

/// A per-article report surface observation (Rust DTO shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustReportSurface {
    pub per_article_schema: String,
    pub policy_id: String,
    pub aggregate_schema: String,
    pub hard_failures_kind: String,
    pub source_ref: String,
}

/// Embedded reports observed in a durable corpus artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedReportsSurface {
    pub count: usize,
    pub layers_observed: Vec<String>,
    pub source_ref: String,
}

/// Canonical aggregate report surface observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateReportSurface {
    pub present: bool,
    pub declared_path: String,
    pub source_ref: String,
}

/// Governor contract expectations observed from the policy's
/// `assurance_report_contract` section and the governor source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernorContractSurface {
    pub expected_schema_version: String,
    pub expected_hard_failures_kind: String,
    pub required_top_level: Vec<String>,
    pub source_ref: String,
}

// ---------------------------------------------------------------------------
// Audit input — all surface observations collected by the CLI.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcapContractAuditInput {
    pub schema_version: String,
    pub source_fingerprints: BTreeMap<String, String>,

    // Policy surface (parsed from YAML)
    pub policy_id: String,
    pub policy_schema_version: String,
    pub policy_rollout: String,
    pub policy_layers: Vec<PolicyLayerObservation>,
    pub policy_metric_names: Vec<String>,
    pub policy_fsm_states: Vec<String>,

    // Aggregate contract (from policy assurance_report_contract)
    pub aggregate_schema_declared: String,
    pub aggregate_path_declared: String,
    pub aggregate_required_keys: Vec<String>,

    // Rust DTO surface
    pub rust_surface: RustReportSurface,

    // Embedded reports surface
    pub embedded_reports: EmbeddedReportsSurface,

    // Aggregate report surface
    pub aggregate_report: AggregateReportSurface,

    // Governor contract surface
    pub governor_contract: GovernorContractSurface,

    pub import_eligible: bool,
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AcapAuditSeverity {
    Error,
    Warning,
    CoverageGap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct AcapContractFinding {
    pub severity: AcapAuditSeverity,
    pub code: String,
    pub subject: String,
    pub detail: String,
    pub source_refs: Vec<String>,
}

// ---------------------------------------------------------------------------
// Verdict and report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcapContractAuditVerdict {
    Converged,
    Incomplete,
    Drift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcapContractAuditReport {
    pub schema_version: String,
    pub input: AcapContractAuditInput,
    pub findings: Vec<AcapContractFinding>,
    pub verdict: AcapContractAuditVerdict,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl AcapContractAuditReport {
    pub fn validate(&self) -> Result<(), AcapContractAuditError> {
        if self.schema_version != ACAP_AUDIT_REPORT_SCHEMA_VERSION || self.import_eligible {
            return Err(AcapContractAuditError::InvalidReport(
                "schema or D127 authority drift".into(),
            ));
        }
        if self.report_sha256 != report_digest(self) {
            return Err(AcapContractAuditError::InvalidReport(
                "report digest mismatch".into(),
            ));
        }
        let expected = verdict_for(&self.findings);
        if self.verdict != expected {
            return Err(AcapContractAuditError::InvalidReport(
                "verdict does not match findings".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AcapContractAuditError {
    #[error("invalid ACAP audit input: {0}")]
    InvalidInput(String),
    #[error("invalid ACAP audit report: {0}")]
    InvalidReport(String),
}

// ---------------------------------------------------------------------------
// Use case
// ---------------------------------------------------------------------------

pub struct AcapContractAuditUseCase;

impl AcapContractAuditUseCase {
    pub fn audit(
        mut input: AcapContractAuditInput,
    ) -> Result<AcapContractAuditReport, AcapContractAuditError> {
        validate_input(&input)?;

        // Normalize ordering for deterministic output.
        input.policy_layers.sort();
        input.policy_metric_names.sort();
        input.policy_metric_names.dedup();
        input.policy_fsm_states.sort();
        input.policy_fsm_states.dedup();
        input.aggregate_required_keys.sort();
        input.aggregate_required_keys.dedup();
        input.rust_surface.hard_failures_kind = input.rust_surface.hard_failures_kind.to_string();
        input.embedded_reports.layers_observed.sort();
        input.embedded_reports.layers_observed.dedup();

        let mut findings = Vec::new();

        // 1. Schema version collision: per-article and aggregate share a version.
        if input.rust_surface.per_article_schema == input.aggregate_schema_declared {
            findings.push(AcapContractFinding {
                severity: AcapAuditSeverity::Error,
                code: "schema_version_collision".into(),
                subject: "per_article_and_aggregate_share_schema".into(),
                detail: format!(
                    "Rust per-article schema '{}' and policy aggregate schema '{}' are \
                     identical — a corpus aggregate must use a distinct schema version \
                     ('{}') to prevent contract confusion.",
                    input.rust_surface.per_article_schema,
                    input.aggregate_schema_declared,
                    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION
                ),
                source_refs: vec![
                    input.rust_surface.source_ref.clone(),
                    "policy.assurance_report_contract.schema_version".into(),
                ],
            });
        }

        // 2. Policy ID drift between Rust and YAML.
        if input.rust_surface.policy_id != input.policy_id {
            findings.push(AcapContractFinding {
                severity: AcapAuditSeverity::Error,
                code: "policy_id_drift".into(),
                subject: "rust_yaml_policy_id_mismatch".into(),
                detail: format!(
                    "Rust ACAP_POLICY_ID='{}' but YAML policy_id='{}'",
                    input.rust_surface.policy_id, input.policy_id
                ),
                source_refs: vec![
                    input.rust_surface.source_ref.clone(),
                    "policy.policy_id".into(),
                ],
            });
        }

        // 3. hard_failures type drift between Rust and governor.
        if input.rust_surface.hard_failures_kind
            != input.governor_contract.expected_hard_failures_kind
        {
            findings.push(AcapContractFinding {
                severity: AcapAuditSeverity::Error,
                code: "hard_failures_type_drift".into(),
                subject: "rust_governor_hard_failures_type_mismatch".into(),
                detail: format!(
                    "Rust produces hard_failures as '{}' but governor expects '{}'. \
                     Non-empty Vec<String> will never equal numeric 0.",
                    input.rust_surface.hard_failures_kind,
                    input.governor_contract.expected_hard_failures_kind
                ),
                source_refs: vec![
                    input.rust_surface.source_ref.clone(),
                    input.governor_contract.source_ref.clone(),
                ],
            });
        }

        // 4. Required policy layers absent from observed embedded reports.
        let metric_set: std::collections::BTreeSet<String> =
            input.policy_metric_names.iter().cloned().collect();
        let observed_layers: std::collections::BTreeSet<String> = input
            .embedded_reports
            .layers_observed
            .iter()
            .cloned()
            .collect();
        let policy_layer_ids: std::collections::BTreeSet<String> =
            input.policy_layers.iter().map(|l| l.id.clone()).collect();

        for layer in &input.policy_layers {
            // 4a. Unknown metric in policy layer.
            for metric in layer
                .hard_metrics
                .iter()
                .chain(layer.quality_metrics.iter())
            {
                if !metric_set.contains(metric) {
                    findings.push(AcapContractFinding {
                        severity: AcapAuditSeverity::Warning,
                        code: "metric_undefined".into(),
                        subject: format!("{}.{}", layer.id, metric),
                        detail: format!(
                            "layer '{}' references metric '{}' not in metric_definitions",
                            layer.id, metric
                        ),
                        source_refs: vec![layer.source_ref.clone()],
                    });
                }
            }
            // 4b. Layer state not in FSM.
            if !input.policy_fsm_states.contains(&layer.state) {
                findings.push(AcapContractFinding {
                    severity: AcapAuditSeverity::Warning,
                    code: "fsm_state_unknown".into(),
                    subject: format!("{}.{}", layer.id, layer.state),
                    detail: format!(
                        "layer '{}' declares state '{}' not in fsm.states",
                        layer.id, layer.state
                    ),
                    source_refs: vec![layer.source_ref.clone()],
                });
            }
            // 4c. Required layer absent from observed reports.
            if layer.enforcement == "required" && !observed_layers.contains(&layer.id) {
                findings.push(AcapContractFinding {
                    severity: AcapAuditSeverity::Warning,
                    code: "required_layer_absent".into(),
                    subject: layer.id.clone(),
                    detail: format!(
                        "required layer '{}' not observed in embedded reports (count={})",
                        layer.id, input.embedded_reports.count
                    ),
                    source_refs: vec![
                        layer.source_ref.clone(),
                        input.embedded_reports.source_ref.clone(),
                    ],
                });
            }
        }

        // 5. Unknown report layer — observed in reports but not in policy.
        for layer_id in observed_layers {
            if !policy_layer_ids.contains(&layer_id) {
                findings.push(AcapContractFinding {
                    severity: AcapAuditSeverity::Warning,
                    code: "unknown_report_layer".into(),
                    subject: layer_id.clone(),
                    detail: format!("report layer '{}' not declared in policy layers", layer_id),
                    source_refs: vec![input.embedded_reports.source_ref.clone()],
                });
            }
        }

        // 6. Canonical aggregate report missing.
        if !input.aggregate_report.present {
            let severity = if input.policy_rollout == "bootstrap" {
                AcapAuditSeverity::CoverageGap
            } else {
                AcapAuditSeverity::Error
            };
            findings.push(AcapContractFinding {
                severity,
                code: "aggregate_report_missing".into(),
                subject: input.aggregate_report.declared_path.clone(),
                detail: format!(
                    "canonical aggregate report not found at '{}' (rollout={})",
                    input.aggregate_report.declared_path, input.policy_rollout
                ),
                source_refs: vec![input.aggregate_report.source_ref.clone()],
            });
        }

        // 7. Aggregate schema version mismatch vs Rust declared aggregate.
        if input.aggregate_schema_declared != input.rust_surface.aggregate_schema {
            findings.push(AcapContractFinding {
                severity: AcapAuditSeverity::Error,
                code: "aggregate_schema_drift".into(),
                subject: "aggregate_schema_version".into(),
                detail: format!(
                    "policy declares aggregate schema '{}' but Rust expects '{}'",
                    input.aggregate_schema_declared, input.rust_surface.aggregate_schema
                ),
                source_refs: vec![
                    "policy.assurance_report_contract.schema_version".into(),
                    input.rust_surface.source_ref.clone(),
                ],
            });
        }

        // 8. Governor expected schema vs policy declared schema.
        if input.governor_contract.expected_schema_version != input.aggregate_schema_declared {
            findings.push(AcapContractFinding {
                severity: AcapAuditSeverity::Warning,
                code: "governor_schema_mismatch".into(),
                subject: "governor_vs_policy_schema".into(),
                detail: format!(
                    "governor expects schema '{}' but policy declares '{}'",
                    input.governor_contract.expected_schema_version,
                    input.aggregate_schema_declared
                ),
                source_refs: vec![
                    input.governor_contract.source_ref.clone(),
                    "policy.assurance_report_contract.schema_version".into(),
                ],
            });
        }

        findings.sort();
        findings.dedup();
        let verdict = verdict_for(&findings);
        let report = AcapContractAuditReport {
            schema_version: ACAP_AUDIT_REPORT_SCHEMA_VERSION.into(),
            input,
            findings,
            verdict,
            import_eligible: false,
            report_sha256: String::new(),
        };
        let digest = report_digest(&report);
        let mut report = report;
        report.report_sha256 = digest;
        report.validate()?;
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// Helpers — following the S06 precedent exactly.
// ---------------------------------------------------------------------------

fn validate_input(input: &AcapContractAuditInput) -> Result<(), AcapContractAuditError> {
    if input.schema_version != ACAP_AUDIT_INPUT_SCHEMA_VERSION {
        return Err(AcapContractAuditError::InvalidInput(
            "schema version mismatch".into(),
        ));
    }
    if input.import_eligible {
        return Err(AcapContractAuditError::InvalidInput(
            "D127 import_eligible must be false".into(),
        ));
    }
    if input.policy_id.trim().is_empty() {
        return Err(AcapContractAuditError::InvalidInput(
            "policy_id must not be empty".into(),
        ));
    }
    if input.policy_layers.is_empty() {
        return Err(AcapContractAuditError::InvalidInput(
            "policy must declare at least one layer".into(),
        ));
    }
    for layer in &input.policy_layers {
        if layer.id.trim().is_empty() {
            return Err(AcapContractAuditError::InvalidInput(
                "layer id must not be empty".into(),
            ));
        }
        if layer.enforcement != "required" && layer.enforcement != "bootstrap" {
            return Err(AcapContractAuditError::InvalidInput(format!(
                "layer '{}' enforcement must be 'required' or 'bootstrap', got '{}'",
                layer.id, layer.enforcement
            )));
        }
    }
    // Validate fingerprints are sha256-like or empty.
    for (key, value) in &input.source_fingerprints {
        if !value.is_empty() && !valid_digest(value) {
            return Err(AcapContractAuditError::InvalidInput(format!(
                "source_fingerprint '{}' is not a valid sha256 hex digest",
                key
            )));
        }
    }
    Ok(())
}

fn verdict_for(findings: &[AcapContractFinding]) -> AcapContractAuditVerdict {
    if findings
        .iter()
        .any(|f| f.severity == AcapAuditSeverity::Error)
    {
        AcapContractAuditVerdict::Drift
    } else if findings.is_empty() {
        AcapContractAuditVerdict::Converged
    } else {
        AcapContractAuditVerdict::Incomplete
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn report_digest(report: &AcapContractAuditReport) -> String {
    // Hash everything except the report_sha256 field itself.
    let mut clone = report.clone();
    clone.report_sha256 = String::new();
    let json = serde_json::to_string(&clone).expect("report must serialize");
    let hash = Sha256::digest(json.as_bytes());
    // Bind the hash to the canonical schema version so a digest from one
    // report schema cannot be replayed against another.
    let mut binder = Vec::with_capacity(hash.len() + ACAP_AUDIT_REPORT_SCHEMA_VERSION.len());
    binder.extend_from_slice(&hash);
    binder.extend_from_slice(ACAP_AUDIT_REPORT_SCHEMA_VERSION.as_bytes());
    hex::encode(Sha256::digest(&binder))
}
