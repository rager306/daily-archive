//! Production corpus revision summaries (M383 S03).
//! Reuses ExtractiveSummaryUseCase. No LLM. No implicit revision=1.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use da_adapters::section_summary_policy::LoadedSectionRolePolicy;
use da_application::extractive_summary::{
    ExtractiveSummaryUseCase, SectionRolePolicyBinding, collect_revision_summary_inputs,
};
use da_ports::DirectGraphStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Args)]
pub struct SummarizeRevisionsArgs {
    /// Optional graph file to import into this process before walking PaperRevision nodes.
    #[arg(long)]
    pub graph: Option<PathBuf>,
    #[arg(long, default_value_t = 160)]
    pub max_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummarizeRevisionsReport {
    pub schema_version: String,
    pub method: String,
    pub revisions_processed: usize,
    pub revisions_skipped: usize,
    pub summaries_written: usize,
    pub skipped_empty_sections: usize,
    pub replay_failures: Vec<String>,
    pub import_eligible: bool,
}

impl SummarizeRevisionsReport {
    pub fn ok(&self) -> bool {
        self.replay_failures.is_empty() && !self.import_eligible
    }
}

pub async fn run(args: &SummarizeRevisionsArgs) -> Result<i32> {
    let store = open_process_store(args.graph.as_deref()).await?;
    let loaded_policy = LoadedSectionRolePolicy::load_project()?;
    let policy_identity = loaded_policy.identity().clone();
    let use_case = ExtractiveSummaryUseCase::with_role_policy(
        store.clone() as Arc<dyn DirectGraphStore>,
        SectionRolePolicyBinding {
            policy: loaded_policy.policy().clone(),
            configuration_sha256: policy_identity.configuration_sha256.clone(),
        },
    )
    .map_err(anyhow::Error::msg)?;

    let walk = collect_revision_summary_inputs(store.as_ref(), args.max_chars).await;
    let mut summaries_written = 0_usize;
    let mut skipped_empty_sections = 0_usize;
    let mut replay_failures = Vec::new();

    for input in &walk.inputs {
        match use_case.materialize(input.clone()).await {
            Ok(first) => match use_case.materialize(input.clone()).await {
                Ok(replay) if replay.batch == first.batch => {
                    summaries_written +=
                        usize::from(first.abstract_written) + first.section_summaries_written;
                    skipped_empty_sections += first.skipped_empty_sections;
                }
                Ok(_) => {
                    replay_failures.push(format!("{}: replay batch changed", input.revision_vid))
                }
                Err(error) => {
                    replay_failures.push(format!("{}: replay: {error}", input.revision_vid))
                }
            },
            Err(error) => replay_failures.push(format!("{}: {error}", input.revision_vid)),
        }
    }

    let report = SummarizeRevisionsReport {
        schema_version: "summarize-revisions.v1".into(),
        method: da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD.into(),
        revisions_processed: walk.inputs.len(),
        revisions_skipped: walk.skipped.len(),
        summaries_written,
        skipped_empty_sections,
        replay_failures,
        import_eligible: false,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.ok() { Ok(0) } else { Ok(1) }
}

async fn open_process_store(
    graph: Option<&std::path::Path>,
) -> Result<Arc<da_adapters::SamyamaGraphStore>> {
    use da_ports::graph_store::GraphStore;
    let store = Arc::new(
        da_adapters::SamyamaGraphStore::try_from_env()
            .map_err(|e| anyhow::anyhow!("graph store open failed: {e}"))?,
    );
    if let Some(path) = graph {
        let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        store
            .import_snapshot(&data)
            .await
            .map_err(|e| anyhow::anyhow!("graph import failed: {e}"))?;
    }
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrip_and_fail_closed_on_replay() {
        let ok = SummarizeRevisionsReport {
            schema_version: "summarize-revisions.v1".into(),
            method: "extractive_lead_v1".into(),
            revisions_processed: 2,
            revisions_skipped: 1,
            summaries_written: 5,
            skipped_empty_sections: 1,
            replay_failures: vec![],
            import_eligible: false,
        };
        crate::roundtrip_json(&ok);
        assert!(ok.ok());
        let bad = SummarizeRevisionsReport {
            replay_failures: vec!["vid:x: replay batch changed".into()],
            ..ok
        };
        assert!(!bad.ok());
    }
}
