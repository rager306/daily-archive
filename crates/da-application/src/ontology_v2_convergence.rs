//! Pure composition of independently validated ontology v2 evidence.
//!
//! Checkpoint audit/identity evidence and process-local endpoint evidence are
//! intentionally separate bindings. This use case never implies they observed
//! one graph, never opens files, and never grants import authority.

use da_domain::ContentDigest;
use serde::{Deserialize, Serialize};

use crate::identity_compatibility::{IdentityCompatibilityReport, IdentityCompatibilityVerdict};
use crate::ontology_contract_audit::{
    AuditSeverity, ContractAuditVerdict, OntologyContractAuditReport,
};
use crate::ontology_endpoint_evidence::OntologyEndpointEvidence;

pub const CONVERGENCE_REPORT_SCHEMA_VERSION: &str = "ontology-v2-convergence-canary.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBinding {
    pub role: String,
    pub source_ref: String,
    pub source_sha256: String,
    pub report_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyV2ConvergenceInput {
    pub checkpoint_audit: OntologyContractAuditReport,
    pub checkpoint_identity: IdentityCompatibilityReport,
    pub endpoint_runtime: OntologyEndpointEvidence,
    pub bindings: Vec<EvidenceBinding>,
    pub task_wires: Vec<String>,
    pub task_wire_source_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyV2ConvergenceVerdict {
    Converged,
    Incomplete,
    Drift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ConvergenceRemainder {
    pub code: String,
    pub subject: String,
    pub source_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyV2ConvergenceReport {
    pub schema_version: String,
    pub checkpoint_graph_sha256: String,
    pub endpoint_graph_sha256: String,
    pub contract_audit_sha256: String,
    pub identity_compatibility_sha256: String,
    pub endpoint_evidence_sha256: String,
    pub task_wire_source_sha256: String,
    pub task_wires: Vec<String>,
    pub bindings: Vec<EvidenceBinding>,
    pub verdict: OntologyV2ConvergenceVerdict,
    pub remainders: Vec<ConvergenceRemainder>,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl OntologyV2ConvergenceReport {
    pub fn validate(&self) -> Result<(), OntologyV2ConvergenceError> {
        if self.schema_version != CONVERGENCE_REPORT_SCHEMA_VERSION
            || self.import_eligible
            || !valid_digest(&self.checkpoint_graph_sha256)
            || !valid_digest(&self.endpoint_graph_sha256)
            || !valid_digest(&self.contract_audit_sha256)
            || !valid_digest(&self.identity_compatibility_sha256)
            || !valid_digest(&self.endpoint_evidence_sha256)
            || !valid_digest(&self.task_wire_source_sha256)
        {
            return Err(OntologyV2ConvergenceError::InvalidReport(
                "schema, digest, or D127 authority drift".into(),
            ));
        }
        let expected = if self
            .remainders
            .iter()
            .any(|item| item.code.starts_with("drift:"))
        {
            OntologyV2ConvergenceVerdict::Drift
        } else if self.remainders.is_empty() {
            OntologyV2ConvergenceVerdict::Converged
        } else {
            OntologyV2ConvergenceVerdict::Incomplete
        };
        if self.verdict != expected || self.report_sha256 != report_digest(self) {
            return Err(OntologyV2ConvergenceError::InvalidReport(
                "verdict or report digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

pub struct OntologyV2ConvergenceUseCase;

impl OntologyV2ConvergenceUseCase {
    pub fn compose(
        mut input: OntologyV2ConvergenceInput,
    ) -> Result<OntologyV2ConvergenceReport, OntologyV2ConvergenceError> {
        input
            .checkpoint_audit
            .validate()
            .map_err(|error| OntologyV2ConvergenceError::InvalidInput(error.to_string()))?;
        input
            .checkpoint_identity
            .validate()
            .map_err(|error| OntologyV2ConvergenceError::InvalidInput(error.to_string()))?;
        input
            .endpoint_runtime
            .validate()
            .map_err(|error| OntologyV2ConvergenceError::InvalidInput(error.to_string()))?;
        validate_bindings(&input)?;
        for (surface, endpoint_digest) in [
            ("node_schemas", &input.endpoint_runtime.node_schemas_sha256),
            (
                "edge_contracts",
                &input.endpoint_runtime.edge_contracts_sha256,
            ),
            (
                "finalize_policy",
                &input.endpoint_runtime.finalize_policy_sha256,
            ),
        ] {
            if input
                .checkpoint_audit
                .input
                .source_fingerprints
                .get(surface)
                != Some(endpoint_digest)
            {
                return Err(OntologyV2ConvergenceError::InvalidInput(format!(
                    "checkpoint and endpoint evidence use different {surface} revisions"
                )));
            }
        }

        input.task_wires.sort();
        input.task_wires.dedup();
        if input.task_wires.iter().any(|wire| wire.trim().is_empty())
            || !valid_digest(&input.task_wire_source_sha256)
        {
            return Err(OntologyV2ConvergenceError::InvalidInput(
                "task wire projection is malformed".into(),
            ));
        }
        let audit_task_wires = input
            .checkpoint_audit
            .input
            .task_wires
            .iter()
            .map(|wire| wire.edge_type.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if input
            .checkpoint_audit
            .input
            .source_fingerprints
            .get("graph_tasks")
            != Some(&input.task_wire_source_sha256)
        {
            return Err(OntologyV2ConvergenceError::InvalidInput(
                "task wire source digest is not bound to the contract audit".into(),
            ));
        }
        if input
            .task_wires
            .iter()
            .any(|wire| !audit_task_wires.contains(wire.as_str()))
        {
            return Err(OntologyV2ConvergenceError::InvalidInput(
                "task wire projection is not bound to the contract audit".into(),
            ));
        }

        let checkpoint_graph_sha256 = input
            .checkpoint_audit
            .input
            .source_fingerprints
            .get("runtime_snapshot")
            .cloned()
            .ok_or_else(|| {
                OntologyV2ConvergenceError::InvalidInput(
                    "contract audit lacks runtime snapshot binding".into(),
                )
            })?;
        if checkpoint_graph_sha256 != input.checkpoint_identity.graph_source_sha256 {
            return Err(OntologyV2ConvergenceError::InvalidInput(
                "checkpoint audit and identity evidence observe different graph sources".into(),
            ));
        }

        let mut remainders = Vec::new();
        for finding in &input.checkpoint_audit.findings {
            let code = if finding.severity == AuditSeverity::Error {
                format!("drift:audit:{}", finding.code)
            } else {
                format!("audit:{}", finding.code)
            };
            remainders.push(ConvergenceRemainder {
                code,
                subject: finding.subject.clone(),
                source_role: "checkpoint_audit".into(),
            });
        }
        match input.checkpoint_audit.verdict {
            ContractAuditVerdict::Drift
                if !remainders.iter().any(|r| r.code.starts_with("drift:")) =>
            {
                remainders.push(ConvergenceRemainder {
                    code: "drift:audit:verdict".into(),
                    subject: "contract audit verdict".into(),
                    source_role: "checkpoint_audit".into(),
                });
            }
            _ => {}
        }
        for index in &input.checkpoint_identity.indexes {
            use crate::identity_compatibility::IdentityIndexStatus;
            match index.status {
                IdentityIndexStatus::Compatible => {}
                IdentityIndexStatus::Unobserved => remainders.push(ConvergenceRemainder {
                    code: "identity:unobserved".into(),
                    subject: format!("{}.{}", index.label, index.property),
                    source_role: "checkpoint_identity".into(),
                }),
                IdentityIndexStatus::Drift => remainders.push(ConvergenceRemainder {
                    code: "drift:identity:index".into(),
                    subject: format!("{}.{}", index.label, index.property),
                    source_role: "checkpoint_identity".into(),
                }),
            }
        }
        if input.checkpoint_identity.verdict == IdentityCompatibilityVerdict::Drift
            && !remainders
                .iter()
                .any(|r| r.code.starts_with("drift:identity"))
        {
            remainders.push(ConvergenceRemainder {
                code: "drift:identity:verdict".into(),
                subject: "identity compatibility verdict".into(),
                source_role: "checkpoint_identity".into(),
            });
        }
        if checkpoint_graph_sha256 != input.endpoint_runtime.graph_semantics_sha256 {
            remainders.push(ConvergenceRemainder {
                code: "composition:split_graph_sources".into(),
                subject: "checkpoint and endpoint evidence observe different runtime graphs".into(),
                source_role: "composition".into(),
            });
        }
        if !input.endpoint_runtime.finalize_passed {
            for blocker in &input.endpoint_runtime.finalize_blockers {
                remainders.push(ConvergenceRemainder {
                    code: "endpoint:finalize_incomplete".into(),
                    subject: blocker.clone(),
                    source_role: "endpoint_runtime".into(),
                });
            }
        }
        remainders.sort();
        remainders.dedup();
        input.bindings.sort_by(|a, b| a.role.cmp(&b.role));
        let verdict = if remainders
            .iter()
            .any(|item| item.code.starts_with("drift:"))
        {
            OntologyV2ConvergenceVerdict::Drift
        } else if remainders.is_empty() {
            OntologyV2ConvergenceVerdict::Converged
        } else {
            OntologyV2ConvergenceVerdict::Incomplete
        };
        let mut report = OntologyV2ConvergenceReport {
            schema_version: CONVERGENCE_REPORT_SCHEMA_VERSION.into(),
            checkpoint_graph_sha256,
            endpoint_graph_sha256: input.endpoint_runtime.graph_semantics_sha256.clone(),
            contract_audit_sha256: input.checkpoint_audit.report_sha256.clone(),
            identity_compatibility_sha256: input.checkpoint_identity.report_sha256.clone(),
            endpoint_evidence_sha256: input.endpoint_runtime.report_sha256.clone(),
            task_wire_source_sha256: input.task_wire_source_sha256,
            task_wires: input.task_wires,
            bindings: input.bindings,
            verdict,
            remainders,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_digest(&report);
        report.validate()?;
        Ok(report)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OntologyV2ConvergenceError {
    #[error("invalid ontology v2 convergence input: {0}")]
    InvalidInput(String),
    #[error("invalid ontology v2 convergence report: {0}")]
    InvalidReport(String),
}

fn validate_bindings(input: &OntologyV2ConvergenceInput) -> Result<(), OntologyV2ConvergenceError> {
    let by_role = input
        .bindings
        .iter()
        .map(|binding| (binding.role.as_str(), binding))
        .collect::<std::collections::BTreeMap<_, _>>();
    if by_role.len() != input.bindings.len() {
        return Err(OntologyV2ConvergenceError::InvalidInput(
            "duplicate evidence binding role".into(),
        ));
    }
    let expected = [
        (
            "checkpoint_audit",
            input.checkpoint_audit.report_sha256.as_str(),
        ),
        (
            "checkpoint_identity",
            input.checkpoint_identity.report_sha256.as_str(),
        ),
        (
            "endpoint_runtime",
            input.endpoint_runtime.report_sha256.as_str(),
        ),
    ];
    for (role, report_sha256) in expected {
        let binding = by_role.get(role).ok_or_else(|| {
            OntologyV2ConvergenceError::InvalidInput(format!("missing {role} binding"))
        })?;
        if binding.report_sha256 != report_sha256
            || binding.source_ref.trim().is_empty()
            || !valid_digest(&binding.source_sha256)
        {
            return Err(OntologyV2ConvergenceError::InvalidInput(format!(
                "invalid {role} binding"
            )));
        }
    }
    if by_role.len() != expected.len() {
        return Err(OntologyV2ConvergenceError::InvalidInput(
            "unexpected evidence binding role".into(),
        ));
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn report_digest(report: &OntologyV2ConvergenceReport) -> String {
    let mut copy = report.clone();
    copy.report_sha256.clear();
    let bytes = serde_json::to_vec(&copy).expect("convergence report is serializable");
    format!("sha256:{}", ContentDigest::of_bytes(&bytes).as_str())
}
