//! Bounded ordered additive mutation canary (M371/S08).
//!
//! Process-local MockGraphStore + InstrumentedGraphStore observation.
//! Does not touch critical materializer APIs, does not unlock D127, and does
//! not write to the production Samyama graph.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Parser;
use da_application::ordered_mutation::OrderedMutationUseCase;
use da_domain::ordered_mutation::{OrderedMutationPlan, OrderedMutationStep};
use da_ports::DirectGraphStore;
use kg_storage::MockGraphStore;

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/ordered_mutation_canary.json"
    )]
    report: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("ordered_mutation_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let report_path = if args.report.is_absolute() {
        args.report.clone()
    } else {
        repo_root.join(&args.report)
    };

    let plan = OrderedMutationPlan::new(
        "ordered-mutation-canary-v1",
        vec![
            OrderedMutationStep::CreateNode {
                label: "Paper".into(),
                bind: "paper".into(),
            },
            OrderedMutationStep::SetString {
                bind: "paper".into(),
                key: "arxiv_id".into(),
                value: "2507.19457".into(),
            },
            OrderedMutationStep::SetString {
                bind: "paper".into(),
                key: "title".into(),
                value: "GEPA canary".into(),
            },
            OrderedMutationStep::SetInt {
                bind: "paper".into(),
                key: "year".into(),
                value: 2025,
            },
            OrderedMutationStep::SetBool {
                bind: "paper".into(),
                key: "import_eligible".into(),
                value: false,
            },
            OrderedMutationStep::CreateNode {
                label: "Entity".into(),
                bind: "entity".into(),
            },
            OrderedMutationStep::SetString {
                bind: "entity".into(),
                key: "name".into(),
                value: "GEPA".into(),
            },
            OrderedMutationStep::CreateEdge {
                source_bind: "paper".into(),
                target_bind: "entity".into(),
                edge_type: "MENTIONS".into(),
            },
        ],
    );

    let store: Arc<dyn DirectGraphStore> = Arc::new(MockGraphStore::new());
    let report = OrderedMutationUseCase::execute(&plan, store)
        .await
        .map_err(|e| anyhow!("ordered mutation canary failed: {e}"))?;

    let json = serde_json::to_vec_pretty(&report)?;
    atomic_write(&report_path, &json)?;

    println!(
        "ordered_mutation_canary plan={} verdict={:?} first_applied={} replay_property_unchanged={} findings={} import_eligible=false report={}",
        report.plan_id,
        report.verdict,
        report.first.counters.applied(),
        report.replay.counters.property.unchanged,
        report.findings.len(),
        display_path(&repo_root, &report_path),
    );
    for finding in &report.findings {
        eprintln!("  finding: {finding}");
    }
    if report.verdict != da_application::ordered_mutation::OrderedMutationVerdict::Pass {
        std::process::exit(1);
    }
    Ok(())
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::File::open(parent).and_then(|dir| dir.sync_all());
    }
    Ok(())
}
