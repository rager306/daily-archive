//! First-class ontology acceptance runner (M371/S14).
//!
//! Same-process composition:
//!   import enriched snapshot
//!   → class-qualified semantic task suite (all 5 product classes)
//!   → seal corpus checkpoint with FULL_SEMANTIC required classes
//!   → optional export of accepted checkpoint snapshot
//!
//! Does not rebuild the corpus, does not unlock D127, does not invent PASS.
//! Bootstrap scholarly evidence remains candidate-only outside this gate.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use clap::Parser;
use da_application::corpus_checkpoint::{
    CorpusCheckpointInput, CorpusCheckpointUseCase, tallies_from_class_results,
};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::corpus_checkpoint::{CorpusCheckpointVerdict, FULL_SEMANTIC_REQUIRED_TASK_CLASSES};
use da_domain::graph_tasks::TaskClassStatus;
use da_domain::{ContentDigest, CorpusAssuranceReport};
use da_ports::graph_store::GraphStore;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_with_resolution.sgsnap"
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
        default_value = "artifacts/article_context/ontology_acceptance_report.json"
    )]
    report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/corpus_semantic_checkpoint.json"
    )]
    checkpoint: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_tasks_semantic.json"
    )]
    tasks_report: PathBuf,
    /// When checkpoint PASSes, also export the in-memory store as accepted snap.
    #[arg(
        long,
        default_value = "artifacts/article_context/accepted_corpus_checkpoint.sgsnap"
    )]
    accepted_snapshot: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
    #[arg(long, default_value_t = true)]
    export_on_pass: bool,
}

#[derive(Debug, Serialize)]
struct AcceptanceReport {
    schema_version: &'static str,
    verdict: String,
    same_process: bool,
    import_eligible: bool,
    snapshot_path: String,
    snapshot_physical_sha256: String,
    snapshot_semantic_sha256: String,
    snapshot_semantic_algorithm: String,
    nodes: u64,
    edges: u64,
    acap_verdict: String,
    acap_article_count: u64,
    task_pass: usize,
    task_empty: usize,
    task_error: usize,
    task_failed: bool,
    class_results: serde_json::Value,
    required_task_classes: Vec<String>,
    required_classes_passed: bool,
    checkpoint_verdict: String,
    checkpoint_findings: Vec<String>,
    checkpoint_path: String,
    accepted_snapshot_path: Option<String>,
    accepted_snapshot_sha256: Option<String>,
    package_sha256: String,
    refreshed_at: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("ontology_acceptance");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;

    let snapshot_path = resolve(&repo_root, &args.snapshot);
    let mat_path = resolve(&repo_root, &args.materialization_report);
    let acap_path = resolve(&repo_root, &args.acap_report);
    let tasks_path = resolve(&repo_root, &args.tasks);
    let report_path = resolve(&repo_root, &args.report);
    let checkpoint_path = resolve(&repo_root, &args.checkpoint);
    let tasks_report_path = resolve(&repo_root, &args.tasks_report);
    let accepted_path = resolve(&repo_root, &args.accepted_snapshot);

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
    anyhow::ensure!(
        !mat_json
            .get("import_eligible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
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

    kg_storage::refuse_persistence_path_from_env()?;
    let store = da_adapters::SamyamaGraphStore::from_env();
    store.import_snapshot(&snapshot_bytes).await?;
    let nodes = store.node_count().await as u64;
    let edges = store.edge_count().await as u64;
    anyhow::ensure!(nodes > 0, "input snapshot contains zero nodes");
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
        "required_task_classes": FULL_SEMANTIC_REQUIRED_TASK_CLASSES
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>(),
    }))?;
    atomic_write(&tasks_report_path, &tasks_json)?;

    let required = FULL_SEMANTIC_REQUIRED_TASK_CLASSES.to_vec();
    let required_classes_passed = required.iter().all(|class| {
        task_report
            .class_results
            .get(class)
            .map(|row| row.status == TaskClassStatus::Passed)
            .unwrap_or(false)
    });

    let package = CorpusCheckpointUseCase::seal(CorpusCheckpointInput {
        package_id: "corpus-200-full-semantic".into(),
        pipeline_revision: format!("manifest:{manifest_sha}+semantic"),
        materialization_manifest_sha256: ContentDigest::try_from(manifest_sha.to_string())
            .map_err(|e| anyhow!("manifest digest: {e}"))?,
        materialization_report_sha256: mat_report_sha,
        snapshot: da_domain::corpus_checkpoint::SnapshotArtifactIdentity {
            path: display_path(&repo_root, &snapshot_path),
            physical_sha256: snapshot_sha.clone(),
            semantic_sha256: ContentDigest::try_from(snapshot_identity.semantic_sha256.clone())
                .map_err(|e| anyhow!("semantic snapshot digest: {e}"))?,
            semantic_algorithm: snapshot_identity.semantic_algorithm.clone(),
            nodes,
            edges,
        },
        acap_report_path: display_path(&repo_root, &acap_path),
        acap: acap.clone(),
        suite_path: display_path(&repo_root, &tasks_path),
        suite_sha256: suite_sha,
        overall_tasks_passed: task_report.passed(),
        class_tallies: tallies_from_class_results(&task_report.class_results),
        required_task_classes: required.clone(),
        same_process_tasks: true,
        refreshed_at: Utc::now().to_rfc3339(),
        import_eligible: false,
    })
    .map_err(|e| anyhow!("seal semantic checkpoint: {e}"))?;

    let package_json = serde_json::to_vec_pretty(&package)?;
    atomic_write(&checkpoint_path, &package_json)?;

    let mut accepted_snapshot_path = None;
    let mut accepted_snapshot_sha256 = None;
    if args.export_on_pass && package.verdict == CorpusCheckpointVerdict::Pass {
        let sealed = store.export_snapshot().await?;
        let checksum = hex::encode(Sha256::digest(&sealed));
        atomic_write(&accepted_path, &sealed)?;
        accepted_snapshot_path = Some(display_path(&repo_root, &accepted_path));
        accepted_snapshot_sha256 = Some(checksum);
    }

    let verdict = if package.verdict == CorpusCheckpointVerdict::Pass && required_classes_passed {
        "PASS"
    } else {
        "FAIL"
    };

    let acceptance = AcceptanceReport {
        schema_version: "ontology-acceptance.v2",
        verdict: verdict.into(),
        same_process: true,
        import_eligible: false,
        snapshot_path: display_path(&repo_root, &snapshot_path),
        snapshot_physical_sha256: snapshot_sha.as_str().to_string(),
        snapshot_semantic_sha256: snapshot_identity.semantic_sha256,
        snapshot_semantic_algorithm: snapshot_identity.semantic_algorithm,
        nodes,
        edges,
        acap_verdict: format!("{:?}", acap.verdict).to_ascii_lowercase(),
        acap_article_count: acap.article_count,
        task_pass: task_report.pass,
        task_empty: task_report.empty,
        task_error: task_report.error,
        task_failed: task_report.failed,
        class_results: serde_json::to_value(&task_report.class_results)?,
        required_task_classes: required.iter().map(|c| c.as_str().to_string()).collect(),
        required_classes_passed,
        checkpoint_verdict: format!("{:?}", package.verdict).to_ascii_lowercase(),
        checkpoint_findings: package.findings.clone(),
        checkpoint_path: display_path(&repo_root, &checkpoint_path),
        accepted_snapshot_path,
        accepted_snapshot_sha256,
        package_sha256: package.package_sha256.clone(),
        refreshed_at: package.refreshed_at.clone(),
    };
    atomic_write(&report_path, &serde_json::to_vec_pretty(&acceptance)?)?;

    println!(
        "ontology_acceptance verdict={} articles={} nodes={} edges={} required_classes_passed={} checkpoint={:?} findings={} import_eligible=false report={}",
        acceptance.verdict,
        acceptance.acap_article_count,
        acceptance.nodes,
        acceptance.edges,
        acceptance.required_classes_passed,
        package.verdict,
        package.findings.len(),
        display_path(&repo_root, &report_path),
    );
    for class in &required {
        if let Some(row) = task_report.class_results.get(class) {
            eprintln!(
                "  class {} status={:?} pass={} empty={} error={}",
                class.as_str(),
                row.status,
                row.pass,
                row.empty,
                row.error
            );
        } else {
            eprintln!("  class {} status=absent", class.as_str());
        }
    }
    for finding in &package.findings {
        eprintln!("  finding: {finding}");
    }
    if acceptance.verdict != "PASS" {
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
