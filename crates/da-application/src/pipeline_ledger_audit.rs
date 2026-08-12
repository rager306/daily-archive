//! Runtime audit of observed production stages against the semantic DATA ledger.

use std::collections::{BTreeMap, BTreeSet};

use da_domain::pipeline_ledger::PipelineStageLedger;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedPipelineStage {
    pub stage_id: String,
    pub ordinal: usize,
    pub status: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub context_items_in: u64,
    pub context_items_out: u64,
    pub failure_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineLedgerAuditReport {
    pub schema_version: String,
    pub ledger_sha256: String,
    pub observed: Vec<ObservedPipelineStage>,
    pub findings: Vec<String>,
    pub pass: bool,
    pub import_eligible: bool,
    pub report_sha256: String,
}

pub fn audit_pipeline_ledger(
    ledger: &PipelineStageLedger,
    ledger_bytes: &[u8],
    observed: Vec<ObservedPipelineStage>,
) -> PipelineLedgerAuditReport {
    let mut findings = Vec::new();
    if let Err(error) = ledger.validate() {
        findings.push(error);
    }
    let contracts = ledger
        .stages
        .iter()
        .map(|stage| (stage.id.as_str(), stage))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut ordinals = BTreeMap::new();
    for record in &observed {
        if !seen.insert(record.stage_id.as_str()) {
            findings.push(format!("duplicate observed stage {}", record.stage_id));
        }
        match contracts.get(record.stage_id.as_str()) {
            None => findings.push(format!("unknown observed stage {}", record.stage_id)),
            Some(_) => {
                ordinals.insert(record.stage_id.as_str(), record.ordinal);
                if record.status != "complete" {
                    findings.push(format!("stage {} status is not complete", record.stage_id));
                }
                if !canonical_digest(&record.input_sha256)
                    || !canonical_digest(&record.output_sha256)
                {
                    findings.push(format!(
                        "stage {} has invalid context digest",
                        record.stage_id
                    ));
                }
                if !record.failure_visible {
                    findings.push(format!(
                        "stage {} lacks failure visibility",
                        record.stage_id
                    ));
                }
            }
        }
    }
    for contract in &ledger.stages {
        if !contract.conditional && !seen.contains(contract.id.as_str()) {
            findings.push(format!("missing required stage {}", contract.id));
        }
        if let Some(ordinal) = ordinals.get(contract.id.as_str()) {
            for dependency in &contract.depends_on {
                if let Some(dependency_ordinal) = ordinals.get(dependency.as_str()) {
                    if dependency_ordinal >= ordinal {
                        findings.push(format!(
                            "stage {} runs before dependency {}",
                            contract.id, dependency
                        ));
                    }
                } else if !contracts[dependency.as_str()].conditional {
                    findings.push(format!(
                        "stage {} missing required dependency {}",
                        contract.id, dependency
                    ));
                }
            }
        }
    }
    let mut report = PipelineLedgerAuditReport {
        schema_version: "pipeline-ledger-audit.v1".into(),
        ledger_sha256: hex::encode(Sha256::digest(ledger_bytes)),
        observed,
        pass: findings.is_empty(),
        findings,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = report_digest(&report);
    report
}

impl PipelineLedgerAuditReport {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "pipeline-ledger-audit.v1"
            || self.import_eligible
            || self.pass != self.findings.is_empty()
        {
            return Err("pipeline audit schema/verdict/D127 invalid".into());
        }
        if self.report_sha256 != report_digest(self) {
            return Err("pipeline audit digest mismatch".into());
        }
        Ok(())
    }
}

fn canonical_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn report_digest(report: &PipelineLedgerAuditReport) -> String {
    let mut clone = report.clone();
    clone.report_sha256.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("audit serializes"),
    ))
}
