//! Ontology-driven graph finalization policy and report types.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizePolicy {
    pub version: String,
    pub policy_id: String,
    pub allow_import_eligible_true: bool,
    pub fail_on_unknown_edge_types: bool,
    pub fail_on_schema_violations: bool,
    pub require_nonempty_graph: bool,
    pub required_nonempty_labels: Vec<String>,
    pub required_stage_outcomes: Vec<String>,
    pub snapshot_on_pass_only: bool,
    pub semantic_tasks_required: bool,
}

impl FinalizePolicy {
    pub fn load_project() -> Result<(Self, String), String> {
        let candidates = [
            std::path::PathBuf::from("data/ontology/finalize_policy.yaml"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../data/ontology/finalize_policy.yaml"),
        ];
        let source = candidates
            .iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
            .ok_or_else(|| {
                "finalize_policy.yaml not found; refusing bundled fallback".to_string()
            })?;
        let policy: Self = serde_yaml::from_str(&source).map_err(|error| error.to_string())?;
        policy.validate()?;
        let fingerprint = format!("sha256:{}", hex::encode(Sha256::digest(source.as_bytes())));
        Ok((policy, fingerprint))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != "1" || self.policy_id.trim().is_empty() {
            return Err("unsupported or unnamed finalize policy".into());
        }
        if self.allow_import_eligible_true {
            return Err("D127 requires allow_import_eligible_true=false".into());
        }
        if !self.fail_on_unknown_edge_types
            || !self.fail_on_schema_violations
            || !self.snapshot_on_pass_only
            || !self.semantic_tasks_required
        {
            return Err(
                "finalize policy must fail closed for schema, edges, snapshot and semantic tasks"
                    .into(),
            );
        }
        if self.required_nonempty_labels.is_empty() || self.required_stage_outcomes.is_empty() {
            return Err("finalize policy requires labels and stage outcomes".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FinalizeVerdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FinalizeFinding {
    pub code: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFinalizeReport {
    pub policy_id: String,
    pub policy_fingerprint: String,
    pub verdict: FinalizeVerdict,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub validated_nodes: usize,
    pub schema_violations: usize,
    pub import_eligible_true: usize,
    pub unknown_edge_types: Vec<String>,
    pub stage_outcomes: std::collections::BTreeMap<String, bool>,
    pub pipeline_report_fingerprint: String,
    pub semantic_tasks_passed: bool,
    pub unregistered_node_labels: Vec<String>,
    pub findings: Vec<FinalizeFinding>,
    pub snapshot_eligible: bool,
    pub import_eligible: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_policy_is_fail_closed_and_d127_locked() {
        let (policy, fingerprint) = FinalizePolicy::load_project().unwrap();
        assert!(!policy.allow_import_eligible_true);
        assert!(policy.fail_on_unknown_edge_types);
        assert!(policy.fail_on_schema_violations);
        assert!(policy.snapshot_on_pass_only);
        assert!(policy.semantic_tasks_required);
        assert!(fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn unsafe_policy_is_rejected() {
        let mut policy: FinalizePolicy =
            serde_yaml::from_str(include_str!("../../../data/ontology/finalize_policy.yaml"))
                .unwrap();
        policy.allow_import_eligible_true = true;
        assert!(policy.validate().unwrap_err().contains("D127"));
    }
}
