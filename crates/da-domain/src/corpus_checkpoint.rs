//! Corpus required-layer checkpoint package (M371/S09).
//!
//! Pure domain: bind verified ACAP aggregate identity, materialization snapshot
//! identity, and class-qualified structural task evidence into a durable
//! package. No filesystem, no graph I/O. Bootstrap semantic classes may be
//! no_claim/empty; they must not invent PASS.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::graph_tasks::{GraphTaskClass, TaskClassStatus};
use crate::{AssuranceVerdict, ContentDigest};

pub const CORPUS_CHECKPOINT_SCHEMA_VERSION: &str = "corpus-required-layer-checkpoint.v1";

/// Required ACAP layers that must be Observed for S09 corpus assurance.
pub const REQUIRED_ACAP_LAYERS: [&str; 7] = [
    "source",
    "parse",
    "structure",
    "extraction",
    "evidence",
    "embedding",
    "materialization",
];

/// Structural task classes that must PASS for a required-layer checkpoint.
/// Summary/resolution/cluster remain bootstrap until dedicated verticals (S10+).
pub const REQUIRED_STRUCTURAL_TASK_CLASSES: [GraphTaskClass; 2] =
    [GraphTaskClass::Structural, GraphTaskClass::Fact];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotIdentity {
    pub path: String,
    pub sha256: ContentDigest,
    pub nodes: u64,
    pub edges: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcapIdentity {
    pub report_path: String,
    pub report_sha256: String,
    pub policy_sha256: ContentDigest,
    pub schema_version: String,
    pub article_count: u64,
    pub verdict: AssuranceVerdict,
    pub observed_required_layers: Vec<String>,
    pub hard_failures: Vec<String>,
    pub d127_unlock_count: u64,
    pub replay_net_writes: u64,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskClassGateRow {
    pub class: GraphTaskClass,
    pub status: TaskClassStatus,
    pub task_count: usize,
    pub pass: usize,
    pub empty: usize,
    pub error: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralTaskEvidence {
    pub suite_path: String,
    pub suite_sha256: String,
    pub overall_passed: bool,
    pub by_class: BTreeMap<GraphTaskClass, TaskClassGateRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusCheckpointVerdict {
    /// Required ACAP layers + structural/fact task classes satisfied.
    Pass,
    /// Hard integrity or required structural gate failed.
    Fail,
    /// Package incomplete (missing evidence).
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusCheckpointPackage {
    pub schema_version: String,
    pub package_id: String,
    pub pipeline_revision: String,
    pub materialization_manifest_sha256: ContentDigest,
    pub materialization_report_sha256: ContentDigest,
    pub snapshot: SnapshotIdentity,
    pub acap: AcapIdentity,
    pub structural_tasks: StructuralTaskEvidence,
    pub required_acap_layers: Vec<String>,
    pub required_task_classes: Vec<GraphTaskClass>,
    pub findings: Vec<String>,
    pub verdict: CorpusCheckpointVerdict,
    pub import_eligible: bool,
    pub same_process_tasks: bool,
    pub refreshed_at: String,
    pub package_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CorpusCheckpointError {
    #[error("invalid corpus checkpoint input: {0}")]
    InvalidInput(String),
    #[error("invalid corpus checkpoint package: {0}")]
    InvalidPackage(String),
}

impl CorpusCheckpointPackage {
    pub fn validate(&self) -> Result<(), CorpusCheckpointError> {
        if self.schema_version != CORPUS_CHECKPOINT_SCHEMA_VERSION {
            return Err(CorpusCheckpointError::InvalidPackage(
                "schema version mismatch".into(),
            ));
        }
        if self.import_eligible {
            return Err(CorpusCheckpointError::InvalidPackage(
                "import_eligible must be false".into(),
            ));
        }
        if self.package_sha256 != package_digest(self) {
            return Err(CorpusCheckpointError::InvalidPackage(
                "package digest mismatch".into(),
            ));
        }
        let expected = verdict_for(&self.findings, self.acap.verdict);
        if self.verdict != expected {
            return Err(CorpusCheckpointError::InvalidPackage(
                "verdict does not match findings".into(),
            ));
        }
        Ok(())
    }
}

/// Inputs for sealing a corpus required-layer checkpoint package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealCorpusCheckpointInput {
    pub package_id: String,
    pub pipeline_revision: String,
    pub materialization_manifest_sha256: ContentDigest,
    pub materialization_report_sha256: ContentDigest,
    pub snapshot: SnapshotIdentity,
    pub acap: AcapIdentity,
    pub structural_tasks: StructuralTaskEvidence,
    pub same_process_tasks: bool,
    pub refreshed_at: String,
}

/// Build a sealed package from already-validated identities and task evidence.
pub fn seal_corpus_checkpoint(
    input: SealCorpusCheckpointInput,
) -> Result<CorpusCheckpointPackage, CorpusCheckpointError> {
    let SealCorpusCheckpointInput {
        package_id,
        pipeline_revision,
        materialization_manifest_sha256,
        materialization_report_sha256,
        snapshot,
        acap,
        structural_tasks,
        same_process_tasks,
        refreshed_at,
    } = input;

    if acap.import_eligible || acap.d127_unlock_count != 0 {
        return Err(CorpusCheckpointError::InvalidInput(
            "D127 must remain locked".into(),
        ));
    }
    if snapshot.sha256.as_str().len() != 64 {
        return Err(CorpusCheckpointError::InvalidInput(
            "snapshot sha256 must be canonical".into(),
        ));
    }
    if acap.article_count == 0 {
        return Err(CorpusCheckpointError::InvalidInput(
            "article_count must be > 0".into(),
        ));
    }

    let mut findings = Vec::new();

    if acap.verdict != AssuranceVerdict::Pass {
        findings.push(format!("acap_verdict:{:?}", acap.verdict).to_ascii_lowercase());
    }
    if !acap.hard_failures.is_empty() {
        findings.push(format!(
            "acap_hard_failures:{}",
            acap.hard_failures.join(",")
        ));
    }
    if acap.replay_net_writes != 0 {
        findings.push(format!("acap_replay_net_writes:{}", acap.replay_net_writes));
    }
    for layer in REQUIRED_ACAP_LAYERS {
        if !acap
            .observed_required_layers
            .iter()
            .any(|observed| observed == layer)
        {
            findings.push(format!("required_layer_not_observed:{layer}"));
        }
    }

    let required_task_classes = REQUIRED_STRUCTURAL_TASK_CLASSES.to_vec();
    for class in &required_task_classes {
        match structural_tasks.by_class.get(class) {
            Some(row) if row.status == TaskClassStatus::Passed => {}
            Some(row) => findings.push(format!(
                "required_task_class:{}:{:?}",
                class.as_str(),
                row.status
            )),
            None => findings.push(format!("required_task_class:{}:absent", class.as_str())),
        }
    }

    // Bootstrap classes may be empty/failed without inventing PASS, but if they
    // are present and Failed they stay visible as coverage findings (not silent).
    for (class, row) in &structural_tasks.by_class {
        if required_task_classes.contains(class) {
            continue;
        }
        if row.status == TaskClassStatus::Failed {
            findings.push(format!(
                "bootstrap_task_class_failed:{}:empty={}:error={}",
                class.as_str(),
                row.empty,
                row.error
            ));
        }
    }

    findings.sort();
    findings.dedup();
    let verdict = verdict_for(&findings, acap.verdict);

    let mut package = CorpusCheckpointPackage {
        schema_version: CORPUS_CHECKPOINT_SCHEMA_VERSION.into(),
        package_id,
        pipeline_revision,
        materialization_manifest_sha256,
        materialization_report_sha256,
        snapshot,
        acap,
        structural_tasks,
        required_acap_layers: REQUIRED_ACAP_LAYERS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        required_task_classes,
        findings,
        verdict,
        import_eligible: false,
        same_process_tasks,
        refreshed_at,
        package_sha256: String::new(),
    };
    package.package_sha256 = package_digest(&package);
    package.validate()?;
    Ok(package)
}

fn verdict_for(findings: &[String], acap_verdict: AssuranceVerdict) -> CorpusCheckpointVerdict {
    let hard = findings.iter().any(|f| {
        f.starts_with("acap_")
            || f.starts_with("required_layer_")
            || f.starts_with("required_task_class:")
    });
    if hard || acap_verdict != AssuranceVerdict::Pass {
        if findings.is_empty() {
            CorpusCheckpointVerdict::Incomplete
        } else {
            CorpusCheckpointVerdict::Fail
        }
    } else if findings
        .iter()
        .any(|f| f.starts_with("bootstrap_task_class_failed:"))
    {
        // Required gates passed; bootstrap empties are visible but do not block
        // required-layer checkpoint seal (S10+ owns those verticals).
        CorpusCheckpointVerdict::Pass
    } else if findings.is_empty() {
        CorpusCheckpointVerdict::Pass
    } else {
        CorpusCheckpointVerdict::Fail
    }
}

pub fn package_digest(package: &CorpusCheckpointPackage) -> String {
    let mut clone = package.clone();
    clone.package_sha256 = String::new();
    let json = serde_json::to_string(&clone).expect("package serializes");
    let hash = Sha256::digest(json.as_bytes());
    let mut binder = Vec::with_capacity(hash.len() + CORPUS_CHECKPOINT_SCHEMA_VERSION.len());
    binder.extend_from_slice(&hash);
    binder.extend_from_slice(CORPUS_CHECKPOINT_SCHEMA_VERSION.as_bytes());
    hex::encode(Sha256::digest(&binder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> ContentDigest {
        ContentDigest::of_text(seed)
    }

    fn acap_pass() -> AcapIdentity {
        AcapIdentity {
            report_path: "artifacts/article_context/assurance_report.json".into(),
            report_sha256: "a".repeat(64),
            policy_sha256: digest("policy"),
            schema_version: "article-context-assurance-aggregate.v1".into(),
            article_count: 200,
            verdict: AssuranceVerdict::Pass,
            observed_required_layers: REQUIRED_ACAP_LAYERS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            hard_failures: vec![],
            d127_unlock_count: 0,
            replay_net_writes: 0,
            import_eligible: false,
        }
    }

    fn structural_pass_with_bootstrap_empty() -> StructuralTaskEvidence {
        let mut by_class = BTreeMap::new();
        by_class.insert(
            GraphTaskClass::Structural,
            TaskClassGateRow {
                class: GraphTaskClass::Structural,
                status: TaskClassStatus::Passed,
                task_count: 5,
                pass: 5,
                empty: 0,
                error: 0,
            },
        );
        by_class.insert(
            GraphTaskClass::Fact,
            TaskClassGateRow {
                class: GraphTaskClass::Fact,
                status: TaskClassStatus::Passed,
                task_count: 1,
                pass: 1,
                empty: 0,
                error: 0,
            },
        );
        by_class.insert(
            GraphTaskClass::Summary,
            TaskClassGateRow {
                class: GraphTaskClass::Summary,
                status: TaskClassStatus::Failed,
                task_count: 1,
                pass: 0,
                empty: 1,
                error: 0,
            },
        );
        StructuralTaskEvidence {
            suite_path: "data/graph_tasks/gepa_hub.tasks.cypher".into(),
            suite_sha256: "b".repeat(64),
            overall_passed: false,
            by_class,
        }
    }

    fn seal(
        acap: AcapIdentity,
        structural_tasks: StructuralTaskEvidence,
        same_process: bool,
    ) -> Result<CorpusCheckpointPackage, CorpusCheckpointError> {
        seal_corpus_checkpoint(SealCorpusCheckpointInput {
            package_id: "pkg".into(),
            pipeline_revision: "manifest:test".into(),
            materialization_manifest_sha256: digest("manifest"),
            materialization_report_sha256: digest("mat-report"),
            snapshot: SnapshotIdentity {
                path: "snap.sgsnap".into(),
                sha256: digest("snap"),
                nodes: 23928,
                edges: 39387,
            },
            acap,
            structural_tasks,
            same_process_tasks: same_process,
            refreshed_at: "2026-08-10T00:00:00Z".into(),
        })
    }

    #[test]
    fn seal_passes_when_required_layers_and_structural_classes_hold() {
        let package = seal(acap_pass(), structural_pass_with_bootstrap_empty(), true).unwrap();
        assert_eq!(package.verdict, CorpusCheckpointVerdict::Pass);
        assert!(!package.import_eligible);
        assert!(
            package
                .findings
                .iter()
                .any(|f| f.starts_with("bootstrap_task_class_failed:summary"))
        );
        assert!(package.validate().is_ok());
        assert_eq!(package.package_sha256, package_digest(&package));
    }

    #[test]
    fn missing_required_layer_or_fact_class_fails() {
        let mut acap = acap_pass();
        acap.observed_required_layers
            .retain(|layer| layer != "evidence");
        let package = seal(acap, structural_pass_with_bootstrap_empty(), false).unwrap();
        assert_eq!(package.verdict, CorpusCheckpointVerdict::Fail);
        assert!(
            package
                .findings
                .iter()
                .any(|f| f == "required_layer_not_observed:evidence")
        );

        let mut tasks = structural_pass_with_bootstrap_empty();
        tasks.by_class.remove(&GraphTaskClass::Fact);
        let package = seal(acap_pass(), tasks, false).unwrap();
        assert_eq!(package.verdict, CorpusCheckpointVerdict::Fail);
        assert!(
            package
                .findings
                .iter()
                .any(|f| f == "required_task_class:fact:absent")
        );
    }

    #[test]
    fn d127_unlock_rejected() {
        let mut acap = acap_pass();
        acap.d127_unlock_count = 1;
        let err = seal(acap, structural_pass_with_bootstrap_empty(), false);
        assert!(err.is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let package = seal(acap_pass(), structural_pass_with_bootstrap_empty(), true).unwrap();
        let json = serde_json::to_vec(&package).unwrap();
        let back: CorpusCheckpointPackage = serde_json::from_slice(&json).unwrap();
        assert_eq!(package, back);
        assert!(back.validate().is_ok());
    }

    #[test]
    fn overall_suite_false_does_not_block_when_required_classes_pass() {
        let tasks = structural_pass_with_bootstrap_empty();
        assert!(!tasks.overall_passed);
        let package = seal(acap_pass(), tasks, true).unwrap();
        assert_eq!(package.verdict, CorpusCheckpointVerdict::Pass);
    }
}
