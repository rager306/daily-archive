//! Corpus required-layer checkpoint composition (M371/S09).
//!
//! Pure application: maps ACAP aggregate + structural task class tallies into a
//! sealed domain package. No filesystem, no graph I/O.

use std::collections::BTreeMap;

use da_domain::corpus_checkpoint::{
    AcapIdentity, CorpusCheckpointError, CorpusCheckpointPackage, REQUIRED_ACAP_LAYERS,
    SealCorpusCheckpointInput, SnapshotIdentity, StructuralTaskEvidence, TaskClassGateRow,
    seal_corpus_checkpoint,
};
use da_domain::graph_tasks::{GraphTaskClass, TaskClassStatus};
use da_domain::{ContentDigest, CorpusAssuranceReport, CorpusLayerStatus};
use serde::{Deserialize, Serialize};

/// Compact class tally injected from the CLI graph task suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InjectedTaskClassTally {
    pub class: GraphTaskClass,
    pub status: TaskClassStatus,
    pub task_count: usize,
    pub pass: usize,
    pub empty: usize,
    pub error: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusCheckpointInput {
    pub package_id: String,
    pub pipeline_revision: String,
    pub materialization_manifest_sha256: ContentDigest,
    pub materialization_report_sha256: ContentDigest,
    pub snapshot_path: String,
    pub snapshot_sha256: ContentDigest,
    pub snapshot_nodes: u64,
    pub snapshot_edges: u64,
    pub acap_report_path: String,
    pub acap: CorpusAssuranceReport,
    pub suite_path: String,
    pub suite_sha256: String,
    pub overall_tasks_passed: bool,
    pub class_tallies: Vec<InjectedTaskClassTally>,
    /// Empty => structural+fact only; set FULL_SEMANTIC for S14 acceptance.
    pub required_task_classes: Vec<GraphTaskClass>,
    pub same_process_tasks: bool,
    pub refreshed_at: String,
    pub import_eligible: bool,
}

pub struct CorpusCheckpointUseCase;

impl CorpusCheckpointUseCase {
    pub fn seal(
        input: CorpusCheckpointInput,
    ) -> Result<CorpusCheckpointPackage, CorpusCheckpointError> {
        if input.import_eligible {
            return Err(CorpusCheckpointError::InvalidInput(
                "import_eligible must be false".into(),
            ));
        }
        // Fail closed if aggregate itself is invalid.
        input
            .acap
            .validate()
            .map_err(|e| CorpusCheckpointError::InvalidInput(e.to_string()))?;

        let observed_required_layers = input
            .acap
            .layer_results
            .iter()
            .filter(|layer| {
                REQUIRED_ACAP_LAYERS.contains(&layer.layer_id.as_str())
                    && layer.status == CorpusLayerStatus::Observed
                    && layer.fail_articles == 0
            })
            .map(|layer| layer.layer_id.clone())
            .collect::<Vec<_>>();

        let acap = AcapIdentity {
            report_path: input.acap_report_path,
            report_sha256: input.acap.report_sha256.clone(),
            policy_sha256: input.acap.policy_sha256.clone(),
            schema_version: input.acap.schema_version.clone(),
            article_count: input.acap.article_count,
            verdict: input.acap.verdict,
            observed_required_layers,
            hard_failures: input.acap.aggregate.hard_failures.clone(),
            d127_unlock_count: input.acap.aggregate.d127_unlock_count,
            replay_net_writes: input.acap.aggregate.replay_net_writes,
            import_eligible: input.acap.import_eligible,
        };

        let mut by_class = BTreeMap::new();
        for tally in input.class_tallies {
            by_class.insert(
                tally.class,
                TaskClassGateRow {
                    class: tally.class,
                    status: tally.status,
                    task_count: tally.task_count,
                    pass: tally.pass,
                    empty: tally.empty,
                    error: tally.error,
                },
            );
        }

        seal_corpus_checkpoint(SealCorpusCheckpointInput {
            package_id: input.package_id,
            pipeline_revision: input.pipeline_revision,
            materialization_manifest_sha256: input.materialization_manifest_sha256,
            materialization_report_sha256: input.materialization_report_sha256,
            snapshot: SnapshotIdentity {
                path: input.snapshot_path,
                sha256: input.snapshot_sha256,
                nodes: input.snapshot_nodes,
                edges: input.snapshot_edges,
            },
            acap,
            structural_tasks: StructuralTaskEvidence {
                suite_path: input.suite_path,
                suite_sha256: input.suite_sha256,
                overall_passed: input.overall_tasks_passed,
                by_class,
            },
            required_task_classes: input.required_task_classes,
            same_process_tasks: input.same_process_tasks,
            refreshed_at: input.refreshed_at,
        })
    }
}

/// Map suite class_results into injected tallies (CLI helper purity boundary).
pub fn tallies_from_class_results(
    class_results: &BTreeMap<GraphTaskClass, da_domain::graph_tasks::TaskClassResult>,
) -> Vec<InjectedTaskClassTally> {
    class_results
        .values()
        .map(|row| InjectedTaskClassTally {
            class: row.class,
            status: row.status,
            task_count: row.task_count,
            pass: row.pass,
            empty: row.empty,
            error: row.error,
        })
        .collect()
}
