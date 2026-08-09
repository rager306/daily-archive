//! Same-process semantic task, finalize policy, and snapshot seal.

use anyhow::{Context, Result};
use clap::Parser;
use da_application::{GraphFinalizeUseCase, PipelineRunReport};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::finalize::FinalizeVerdict;
use da_ports::graph_store::GraphStore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    input_snapshot: PathBuf,
    #[arg(long)]
    pipeline_report: PathBuf,
    #[arg(long)]
    tasks: PathBuf,
    #[arg(long)]
    output_snapshot: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
}

#[derive(Debug, Serialize)]
struct SealReport {
    finalize: da_domain::finalize::GraphFinalizeReport,
    semantic_tasks: da_cli::graph_tasks::GraphTaskSuiteReport,
    input_snapshot_sha256: String,
    output_snapshot_sha256: Option<String>,
    output_snapshot: Option<String>,
    import_eligible: bool,
    same_process: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("pipeline_finalize");
    let args = Args::parse();
    kg_storage::refuse_persistence_path_from_env()?;
    let snapshot = std::fs::read(&args.input_snapshot)
        .with_context(|| format!("read {}", args.input_snapshot.display()))?;
    let input_snapshot_sha256 = hex::encode(Sha256::digest(&snapshot));
    let pipeline_report: PipelineRunReport = serde_json::from_slice(
        &std::fs::read(&args.pipeline_report)
            .with_context(|| format!("read {}", args.pipeline_report.display()))?,
    )
    .context("deserialize PipelineRunReport")?;
    let suite = parse_tasks_cypher(
        &std::fs::read_to_string(&args.tasks)
            .with_context(|| format!("read {}", args.tasks.display()))?,
    )
    .map_err(anyhow::Error::msg)?;

    let store = da_adapters::SamyamaGraphStore::from_env();
    store.import_snapshot(&snapshot).await?;
    anyhow::ensure!(
        store.node_count().await > 0,
        "input snapshot contains zero nodes"
    );
    let semantic_tasks = run_graph_task_suite(&store, &args.graph, &suite).await;
    let finalizer = GraphFinalizeUseCase::load_project().map_err(anyhow::Error::msg)?;
    let finalize = finalizer
        .finalize(&store, &pipeline_report, semantic_tasks.passed())
        .await;

    let (output_snapshot_sha256, output_snapshot) = if finalize.verdict == FinalizeVerdict::Pass
        && finalize.snapshot_eligible
        && semantic_tasks.passed()
    {
        let sealed = store.export_snapshot().await?;
        let checksum = hex::encode(Sha256::digest(&sealed));
        atomic_write(&args.output_snapshot, &sealed)?;
        (
            Some(checksum),
            Some(args.output_snapshot.display().to_string()),
        )
    } else {
        (None, None)
    };
    let failed = finalize.verdict != FinalizeVerdict::Pass || !semantic_tasks.passed();
    let report = SealReport {
        finalize,
        semantic_tasks,
        input_snapshot_sha256,
        output_snapshot_sha256,
        output_snapshot,
        import_eligible: false,
        same_process: true,
    };
    atomic_write(&args.report, &(serde_json::to_vec_pretty(&report)?))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    anyhow::ensure!(
        !failed,
        "finalize or semantic task suite failed; snapshot not sealed"
    );
    Ok(())
}

fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}
