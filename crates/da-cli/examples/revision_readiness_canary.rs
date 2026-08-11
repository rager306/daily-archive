//! Real Samyama revision readiness canary (M363/S03).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::revision_readiness::{
    RevisionReadinessPolicyBinding, RevisionReadinessUseCase,
};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_ports::{DirectGraphStore, GraphStore};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "data/graph_tasks/revision_readiness.tasks.cypher"
    )]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_readiness_canary.json"
    )]
    report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_readiness_canary.sgsnap"
    )]
    output_snapshot: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("revision_readiness_canary");
    let args = Args::parse();
    let root = std::env::current_dir()?.canonicalize()?;
    let input = resolve(&root, &args.snapshot);
    let tasks = resolve(&root, &args.tasks);
    let report_path = resolve(&root, &args.report);
    let output = resolve(&root, &args.output_snapshot);
    let bytes = std::fs::read(&input).with_context(|| format!("read {}", input.display()))?;
    let input_sha256 = hex::encode(Sha256::digest(&bytes));
    kg_storage::refuse_persistence_path_from_env()?;
    let store = Arc::new(da_adapters::SamyamaGraphStore::from_env());
    store.import_snapshot(&bytes).await?;

    let loaded =
        da_adapters::revision_readiness_policy::LoadedRevisionReadinessPolicy::load_project()?;
    let use_case = RevisionReadinessUseCase::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        RevisionReadinessPolicyBinding {
            policy: loaded.policy().clone(),
            configuration_sha256: loaded.identity().configuration_sha256.clone(),
        },
    )?;
    let mut results = Vec::new();
    let mut failures = Vec::new();
    for revision in store.get_nodes_by_label("PaperRevision").await {
        let Some(vid) = store.get_node_property_string(revision, "vid").await else {
            failures.push(format!("PaperRevision {revision} missing vid"));
            continue;
        };
        match use_case.materialize(revision, &vid).await {
            Ok(first) => match use_case.materialize(revision, &vid).await {
                Ok(replay)
                    if replay.signal_vid == first.signal_vid
                        && replay.evidence_sha256 == first.evidence_sha256
                        && !replay.edge_created
                        && replay.previous_signals_invalidated == 0 =>
                {
                    results.push(first)
                }
                Ok(_) => failures.push(format!("{vid}: replay changed identity or edge")),
                Err(error) => failures.push(format!("{vid}: replay: {error}")),
            },
            Err(error) => failures.push(format!("{vid}: {error}")),
        }
    }
    results.sort_by(|left, right| left.revision_vid.cmp(&right.revision_vid));
    let suite_source = std::fs::read_to_string(&tasks)?;
    let suite = parse_tasks_cypher(&suite_source).map_err(|error| anyhow!(error))?;
    let task_report = run_graph_task_suite(store.as_ref(), &args.graph, &suite).await;
    let output_bytes = store.export_snapshot().await?;
    let output_sha256 = hex::encode(Sha256::digest(&output_bytes));
    atomic_write(&output, &output_bytes)?;
    let mut current_signals = 0_usize;
    for node in store.get_nodes_by_label("QualitySignal").await {
        if store
            .get_node_property_string(node, "signal_type")
            .await
            .as_deref()
            == Some("agent_readiness")
            && store.get_node_property_bool(node, "is_current").await == Some(true)
        {
            current_signals += 1;
        }
    }
    let report = serde_json::json!({
        "schema_version": "revision-readiness-canary.v1",
        "input_snapshot": display(&root, &input),
        "input_snapshot_sha256": input_sha256,
        "output_snapshot": display(&root, &output),
        "output_snapshot_sha256": output_sha256,
        "output_snapshot_bytes": output_bytes.len(),
        "policy_identity": loaded.identity(),
        "results": results,
        "failures": failures,
        "current_agent_readiness_signals": current_signals,
        "tasks": task_report,
        "import_eligible": false,
    });
    atomic_write(&report_path, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "revision_readiness_canary revisions={} current_signals={} tasks_pass={} failed={} import_eligible=false report={}",
        results.len(),
        current_signals,
        task_report.pass,
        task_report.failed,
        display(&root, &report_path)
    );
    if results.len() < 2 || !failures.is_empty() || current_signals < 2 || task_report.failed {
        for failure in failures {
            eprintln!("failure: {failure}");
        }
        std::process::exit(1);
    }
    Ok(())
}

fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.into()
    } else {
        root.join(path)
    }
}
fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
