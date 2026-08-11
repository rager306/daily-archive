//! Seal a replay-safe required-layer corpus checkpoint (M371/S09).
//!
//! Same-process: import pinned materialization `.sgsnap` → run class-qualified
//! graph tasks → bind ACAP aggregate identity → emit durable package.
//! Does not rebuild the corpus, does not unlock D127, does not invent PASS for
//! bootstrap summary/resolution/cluster classes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use clap::Parser;
use da_application::corpus_checkpoint::{
    CorpusCheckpointInput, CorpusCheckpointUseCase, tallies_from_class_results,
};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::corpus_checkpoint::CorpusCheckpointVerdict;
use da_domain::{ContentDigest, CorpusAssuranceReport};
use da_ports::graph_store::GraphStore;
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.sgsnap"
    )]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.json"
    )]
    materialization_report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/assurance_report.json"
    )]
    acap_report: PathBuf,
    #[arg(
        long,
        default_value = "data/graph_tasks/materialization_all_200.tasks.cypher"
    )]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/corpus_required_layer_checkpoint.json"
    )]
    package: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_tasks.json"
    )]
    tasks_report: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
    /// Ordered producer reports proving exact-byte derivation from materialization.
    #[arg(long = "derivation-report", value_delimiter = ',')]
    derivation_reports: Vec<PathBuf>,
    #[arg(long, default_value = "data/ontology/snapshot_derivation_policy.yaml")]
    derivation_policy: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("corpus_required_layer_checkpoint");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;

    let snapshot_path = resolve(&repo_root, &args.snapshot);
    let mat_path = resolve(&repo_root, &args.materialization_report);
    let acap_path = resolve(&repo_root, &args.acap_report);
    let tasks_path = resolve(&repo_root, &args.tasks);
    let package_path = resolve(&repo_root, &args.package);
    let tasks_report_path = resolve(&repo_root, &args.tasks_report);

    let snapshot_bytes = std::fs::read(&snapshot_path)
        .with_context(|| format!("read {}", snapshot_path.display()))?;
    let snapshot_sha = ContentDigest::of_bytes(&snapshot_bytes);
    let snapshot_identity = kg_storage::fingerprint_snapshot(&snapshot_bytes)
        .map_err(|error| anyhow!("fingerprint {}: {error}", snapshot_path.display()))?;

    let mat_bytes =
        std::fs::read(&mat_path).with_context(|| format!("read {}", mat_path.display()))?;
    let mat_report_sha = ContentDigest::of_bytes(&mat_bytes);
    let mat_json: serde_json::Value = serde_json::from_slice(&mat_bytes)?;
    let manifest_sha = mat_json
        .get("manifest_sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("materialization report missing manifest_sha256"))?;
    let declared_snap_sha = mat_json
        .get("output_snapshot_sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("materialization report missing output_snapshot_sha256"))?;
    let checkpoint_snapshot = da_domain::corpus_checkpoint::SnapshotArtifactIdentity {
        path: display_path(&repo_root, &snapshot_path),
        physical_sha256: snapshot_sha,
        semantic_sha256: ContentDigest::try_from(snapshot_identity.semantic_sha256)
            .map_err(|e| anyhow!("semantic snapshot digest: {e}"))?,
        semantic_algorithm: snapshot_identity.semantic_algorithm,
        nodes: snapshot_identity.node_count,
        edges: snapshot_identity.edge_count,
    };
    let derivation_paths = args
        .derivation_reports
        .iter()
        .map(|path| resolve(&repo_root, path))
        .collect::<Vec<_>>();
    let snapshot_lineage = da_cli::snapshot_lineage::load_lineage(
        ContentDigest::try_from(declared_snap_sha.to_string())
            .map_err(|e| anyhow!("materialization snapshot digest: {e}"))?,
        &resolve(&repo_root, &args.derivation_policy),
        &derivation_paths,
    )?;
    snapshot_lineage.validate_for(&checkpoint_snapshot)?;
    let import_eligible = mat_json
        .get("import_eligible")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    anyhow::ensure!(
        !import_eligible,
        "materialization import_eligible must be false"
    );

    let acap_bytes =
        std::fs::read(&acap_path).with_context(|| format!("read {}", acap_path.display()))?;
    let acap: CorpusAssuranceReport = serde_json::from_slice(&acap_bytes)
        .with_context(|| format!("parse ACAP {}", acap_path.display()))?;
    acap.validate()
        .map_err(|e| anyhow!("ACAP report invalid: {e}"))?;

    let suite_bytes =
        std::fs::read(&tasks_path).with_context(|| format!("read {}", tasks_path.display()))?;
    let suite_sha = hex::encode(Sha256::digest(&suite_bytes));
    let suite = parse_tasks_cypher(std::str::from_utf8(&suite_bytes)?)
        .map_err(|e| anyhow!("parse tasks: {e}"))?;

    // Same-process structural/fact validation against the pinned snapshot.
    kg_storage::refuse_persistence_path_from_env()?;
    let store = da_adapters::SamyamaGraphStore::from_env();
    store.import_snapshot(&snapshot_bytes).await?;
    let nodes = store.node_count().await as u64;
    let edges = store.edge_count().await as u64;
    anyhow::ensure!(nodes > 0, "snapshot contains zero nodes");
    anyhow::ensure!(
        nodes == snapshot_identity.node_count && edges == snapshot_identity.edge_count,
        "snapshot identity counts differ from imported graph: identity={}/{} graph={}/{}",
        snapshot_identity.node_count,
        snapshot_identity.edge_count,
        nodes,
        edges
    );
    let task_report = run_graph_task_suite(&store, &args.graph, &suite).await;
    let tasks_json = serde_json::to_vec_pretty(&serde_json::json!({
        "suite_file": display_path(&repo_root, &tasks_path),
        "snapshot": display_path(&repo_root, &snapshot_path),
        "import_eligible": false,
        "nodes": nodes,
        "edges": edges,
        "pass": task_report.pass,
        "empty": task_report.empty,
        "error": task_report.error,
        "failed": task_report.failed,
        "class_results": task_report.class_results,
        "tasks": task_report.tasks,
    }))?;
    atomic_write(&tasks_report_path, &tasks_json)?;

    let package = CorpusCheckpointUseCase::seal(CorpusCheckpointInput {
        package_id: "corpus-200-required-layers".into(),
        pipeline_revision: format!("manifest:{manifest_sha}"),
        materialization_manifest_sha256: ContentDigest::try_from(manifest_sha.to_string())
            .map_err(|e| anyhow!("manifest digest: {e}"))?,
        materialization_report_sha256: mat_report_sha,
        snapshot: checkpoint_snapshot,
        snapshot_lineage,
        acap_report_path: display_path(&repo_root, &acap_path),
        acap,
        suite_path: display_path(&repo_root, &tasks_path),
        suite_sha256: suite_sha,
        overall_tasks_passed: task_report.passed(),
        class_tallies: tallies_from_class_results(&task_report.class_results),
        required_task_classes: vec![],
        same_process_tasks: true,
        refreshed_at: Utc::now().to_rfc3339(),
        import_eligible: false,
    })
    .map_err(|e| anyhow!("seal checkpoint: {e}"))?;

    let package_json = serde_json::to_vec_pretty(&package)?;
    atomic_write(&package_path, &package_json)?;

    println!(
        "corpus_required_layer_checkpoint verdict={:?} articles={} nodes={} edges={} overall_tasks_passed={} findings={} import_eligible=false package={}",
        package.verdict,
        package.acap.article_count,
        package.snapshot.nodes,
        package.snapshot.edges,
        package.structural_tasks.overall_passed,
        package.findings.len(),
        display_path(&repo_root, &package_path),
    );
    for class in &package.required_task_classes {
        if let Some(row) = package.structural_tasks.by_class.get(class) {
            eprintln!(
                "  required_class {} status={:?} pass={} empty={} error={}",
                class.as_str(),
                row.status,
                row.pass,
                row.empty,
                row.error
            );
        }
    }
    for finding in &package.findings {
        eprintln!("  finding: {finding}");
    }
    if package.verdict != CorpusCheckpointVerdict::Pass {
        std::process::exit(1);
    }
    Ok(())
}

fn resolve(repo_root: &Path, relative: &Path) -> PathBuf {
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        repo_root.join(relative)
    }
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
        let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
    }
    Ok(())
}
