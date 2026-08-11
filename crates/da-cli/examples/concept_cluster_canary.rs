//! Derived ConceptCluster canary (M371/S12 product wave).
//!
//! Same-process: import pinned snapshot (prefer summary-enriched) → inventory
//! Paper-[:MENTIONS]->Entity co-occurrence → pure detect_clusters → materialize
//! ConceptCluster + MEMBER_OF_CLUSTER → re-run cluster-class graph tasks.
//! No LLM. D127 stays false.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::cluster::ClusterUseCase;
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_ports::{DirectGraphStore, GraphStore};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_with_summaries.sgsnap"
    )]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "data/graph_tasks/materialization_all_200.tasks.cypher"
    )]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/concept_cluster_canary.json"
    )]
    report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_with_clusters.sgsnap"
    )]
    output_snapshot: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("concept_cluster_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let snapshot_path = resolve(&repo_root, &args.snapshot);
    // Fallback to base materialization snap if summary-enriched missing.
    let snapshot_path = if snapshot_path.is_file() {
        snapshot_path
    } else {
        resolve(
            &repo_root,
            Path::new("artifacts/article_context/materialization_all_200.sgsnap"),
        )
    };
    let tasks_path = resolve(&repo_root, &args.tasks);
    let report_path = resolve(&repo_root, &args.report);
    let out_snap_path = resolve(&repo_root, &args.output_snapshot);

    let snapshot_bytes = std::fs::read(&snapshot_path)
        .with_context(|| format!("read {}", snapshot_path.display()))?;
    let input_snap_sha = hex::encode(Sha256::digest(&snapshot_bytes));

    kg_storage::refuse_persistence_path_from_env()?;
    let store = Arc::new(da_adapters::SamyamaGraphStore::from_env());
    store.import_snapshot(&snapshot_bytes).await?;

    let entity_count = store.get_nodes_by_label("Entity").await.len();
    let paper_count = store.get_nodes_by_label("Paper").await.len();

    let uc = ClusterUseCase::from_shared(store.clone() as Arc<dyn da_ports::DirectGraphStore>);
    let (entity_papers, first) = uc.materialize_from_graph().await?;
    let unique_entities = entity_papers.len();
    let multi_paper = entity_papers
        .values()
        .filter(|papers| papers.len() >= 2)
        .count();
    let high_mention = entity_papers
        .values()
        .filter(|papers| papers.len() >= da_domain::cluster::min_single_mentions())
        .count();

    // Replay must be pure reuse (no new clusters/edges).
    let (_, replay) = uc.materialize_from_graph().await?;

    let cluster_nodes = store.get_nodes_by_label("ConceptCluster").await.len() as u64;
    let out_bytes = store.export_snapshot().await?;
    let out_snap_sha = hex::encode(Sha256::digest(&out_bytes));
    atomic_write(&out_snap_path, &out_bytes)?;

    let suite_src = std::fs::read_to_string(&tasks_path)
        .with_context(|| format!("read {}", tasks_path.display()))?;
    let suite = parse_tasks_cypher(&suite_src).map_err(|e| anyhow!("parse tasks: {e}"))?;
    let task_report = run_graph_task_suite(store.as_ref(), &args.graph, &suite).await;
    let cluster_class = task_report
        .class_results
        .get(&da_domain::graph_tasks::GraphTaskClass::Cluster)
        .cloned();

    let report = serde_json::json!({
        "schema_version": "concept-cluster-canary.v1",
        "input_snapshot": display_path(&repo_root, &snapshot_path),
        "input_snapshot_sha256": input_snap_sha,
        "output_snapshot": display_path(&repo_root, &out_snap_path),
        "output_snapshot_sha256": out_snap_sha,
        "algorithm": {
            "min_single_mentions": da_domain::cluster::min_single_mentions(),
            "min_cooccurrence": da_domain::cluster::min_cooccurrence(),
            "config_fingerprint": da_domain::cluster::cluster_configuration_fingerprint(),
        },
        "inventory": {
            "paper_nodes": paper_count,
            "entity_nodes": entity_count,
            "unique_entity_keys": unique_entities,
            "entities_in_ge_2_papers": multi_paper,
            "entities_meeting_single_threshold": high_mention,
        },
        "first_pass": {
            "clusters_created": first.clusters_created,
            "clusters_reused": first.clusters_reused,
            "member_edges_created": first.member_edges_created,
            "clusters_skipped": first.clusters_skipped,
        },
        "replay": {
            "clusters_created": replay.clusters_created,
            "clusters_reused": replay.clusters_reused,
            "member_edges_created": replay.member_edges_created,
            "clusters_skipped": replay.clusters_skipped,
        },
        "concept_cluster_nodes": cluster_nodes,
        "import_eligible": false,
        "tasks": {
            "suite": display_path(&repo_root, &tasks_path),
            "pass": task_report.pass,
            "empty": task_report.empty,
            "error": task_report.error,
            "failed": task_report.failed,
            "class_results": task_report.class_results,
            "cluster_class": cluster_class,
        },
    });
    atomic_write(&report_path, &serde_json::to_vec_pretty(&report)?)?;

    println!(
        "concept_cluster_canary entities={} unique_keys={} clusters_created={} clusters_reused={} member_edges={} concept_cluster_nodes={} replay_created={} cluster_class={:?} import_eligible=false report={}",
        entity_count,
        unique_entities,
        first.clusters_created,
        first.clusters_reused,
        first.member_edges_created,
        cluster_nodes,
        replay.clusters_created,
        cluster_class
            .as_ref()
            .map(|c| format!("{:?}", c.status))
            .unwrap_or_else(|| "absent".into()),
        display_path(&repo_root, &report_path),
    );

    // Product gates
    if first.clusters_created + first.clusters_reused == 0 || cluster_nodes == 0 {
        eprintln!("no ConceptCluster materialized — inventory may be below thresholds");
        std::process::exit(1);
    }
    if replay.clusters_created != 0 || replay.member_edges_created != 0 {
        eprintln!(
            "replay not idempotent: created={} edges={}",
            replay.clusters_created, replay.member_edges_created
        );
        std::process::exit(1);
    }
    if let Some(class) = cluster_class
        && class.status != da_domain::graph_tasks::TaskClassStatus::Passed
    {
        eprintln!("cluster class did not PASS: {:?}", class);
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
