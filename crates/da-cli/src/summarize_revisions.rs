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
    /// Write summarize-revisions.v1 JSON. Requires --graph so the input digest is file bytes.
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Write exported graph bytes after materialize (hashed into the output digest).
    #[arg(long)]
    pub output_graph: Option<PathBuf>,
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
    /// Exact-byte digest of the imported graph file. Empty when no --graph was given.
    pub input_snapshot_sha256: String,
    /// Exact-byte digest of exported graph bytes after materialize.
    pub output_snapshot_sha256: String,
    pub import_eligible: bool,
}

impl SummarizeRevisionsReport {
    pub fn ok(&self) -> bool {
        self.replay_failures.is_empty() && !self.import_eligible
    }

    pub fn derivation_ready(&self) -> bool {
        self.ok()
            && is_sha256_hex(&self.input_snapshot_sha256)
            && is_sha256_hex(&self.output_snapshot_sha256)
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn ensure_parent(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

pub async fn run(args: &SummarizeRevisionsArgs) -> Result<i32> {
    if args.report.is_some() && args.graph.is_none() {
        anyhow::bail!("--report requires --graph so input digest is exact file bytes");
    }
    let input_digest = match &args.graph {
        Some(path) => {
            let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
            da_domain::ContentDigest::of_bytes(&bytes)
                .as_str()
                .to_string()
        }
        None => String::new(),
    };
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

    use da_ports::graph_store::GraphStore;
    let output_bytes = store
        .export_snapshot()
        .await
        .map_err(|e| anyhow::anyhow!("export graph after summarize: {e}"))?;
    let output_digest = da_domain::ContentDigest::of_bytes(&output_bytes)
        .as_str()
        .to_string();
    if let Some(path) = &args.output_graph {
        ensure_parent(path)?;
        std::fs::write(path, &output_bytes).with_context(|| format!("write {}", path.display()))?;
    }

    let report = SummarizeRevisionsReport {
        schema_version: "summarize-revisions.v1".into(),
        method: da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD.into(),
        revisions_processed: walk.inputs.len(),
        revisions_skipped: walk.skipped.len(),
        summaries_written,
        skipped_empty_sections,
        replay_failures,
        input_snapshot_sha256: input_digest,
        output_snapshot_sha256: output_digest,
        import_eligible: false,
    };
    if let Some(path) = &args.report {
        if !report.derivation_ready() {
            anyhow::bail!(
                "refusing to write derivation report: replay_failures={} input_hex={} output_hex={}",
                report.replay_failures.len(),
                report.input_snapshot_sha256.len(),
                report.output_snapshot_sha256.len()
            );
        }
        ensure_parent(path)?;
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)
            .with_context(|| format!("write {}", path.display()))?;
    }
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

    fn digest(n: u8) -> String {
        format!("{n:064x}")
    }

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
            input_snapshot_sha256: digest(1),
            output_snapshot_sha256: digest(2),
            import_eligible: false,
        };
        crate::roundtrip_json(&ok);
        assert!(ok.ok());
        assert!(ok.derivation_ready());
        let bad = SummarizeRevisionsReport {
            replay_failures: vec!["vid:x: replay batch changed".into()],
            ..ok.clone()
        };
        assert!(!bad.ok());
        assert!(!bad.derivation_ready());
    }

    #[test]
    fn derivation_ready_requires_both_hex_digests() {
        let mut report = SummarizeRevisionsReport {
            schema_version: "summarize-revisions.v1".into(),
            method: "extractive_lead_v1".into(),
            revisions_processed: 0,
            revisions_skipped: 0,
            summaries_written: 0,
            skipped_empty_sections: 0,
            replay_failures: vec![],
            input_snapshot_sha256: String::new(),
            output_snapshot_sha256: digest(2),
            import_eligible: false,
        };
        assert!(report.ok());
        assert!(!report.derivation_ready());
        report.input_snapshot_sha256 = digest(1);
        assert!(report.derivation_ready());
    }
}
