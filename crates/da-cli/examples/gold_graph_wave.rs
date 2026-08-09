//! Materialize one verified gold cache wave into one shared Samyama graph.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilePipelineCache, SamyamaGraphStore};
use da_cli::gold_corpus::GoldCorpusManifest;
use da_cli::gold_graph::{
    MaterializationRuntime, atomic_write, catalog_metadata_index, ensure_identity_indexes,
    load_identity_indexes, load_wave_report, materialize_wave, sha256,
};
use da_domain::ContentDigest;
use da_ports::clock::FixedClock;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    wave_report: PathBuf,
    #[arg(long)]
    cache_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    manifest: PathBuf,
    #[arg(long)]
    input_snapshot: Option<PathBuf>,
    #[arg(long)]
    output_snapshot: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(
        long,
        default_value = "scripts/governor/checks/article_context_assurance.yaml"
    )]
    assurance_policy: PathBuf,
    /// Explicit transaction time for reproducible graph truth (RFC3339).
    #[arg(long)]
    valid_at: String,
    /// Parallel immutable cache preparation workers (ordered graph commit stays single-writer).
    #[arg(long)]
    prepare_workers: Option<usize>,
    #[arg(long, default_value = "data/ontology/identity_indexes.yaml")]
    identity_indexes: PathBuf,
    #[arg(long, default_value = "data/ontology/node_schemas.yaml")]
    node_schemas: PathBuf,
}

#[derive(Debug, Serialize)]
struct GraphWaveReport {
    materialization: da_cli::gold_graph::BatchGraphMaterialization,
    input_snapshot_sha256: Option<String>,
    output_snapshot_sha256: String,
    output_snapshot: String,
    identity_indexes_sha256: String,
    node_schemas_sha256: String,
    same_process: bool,
    valid_at: String,
    import_eligible: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("gold_graph_wave");
    let args = Args::parse();
    kg_storage::refuse_persistence_path_from_env()?;
    let wave = load_wave_report(&args.wave_report)?;
    let manifest_bytes = std::fs::read(&args.manifest)
        .with_context(|| format!("read {}", args.manifest.display()))?;
    anyhow::ensure!(
        sha256(&manifest_bytes) == wave.manifest_sha256,
        "wave report does not bind the supplied corpus manifest"
    );
    let manifest: GoldCorpusManifest =
        serde_json::from_slice(&manifest_bytes).context("parse corpus manifest")?;
    let metadata = catalog_metadata_index(&manifest)?;

    let concrete = Arc::new(SamyamaGraphStore::from_env());
    let input_snapshot_sha256 = if let Some(path) = &args.input_snapshot {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        concrete.import_snapshot(&bytes).await?;
        Some(sha256(&bytes))
    } else {
        None
    };
    let policy_bytes = std::fs::read(&args.assurance_policy)
        .with_context(|| format!("read {}", args.assurance_policy.display()))?;
    let policy_sha256 = ContentDigest::of_bytes(&policy_bytes);
    let valid_at = chrono::DateTime::parse_from_rfc3339(&args.valid_at)
        .with_context(|| format!("invalid --valid-at RFC3339: {}", args.valid_at))?
        .with_timezone(&chrono::Utc);
    let clock: Arc<dyn da_ports::Clock> = Arc::new(FixedClock::new(valid_at));
    let indexes = load_identity_indexes(&args.identity_indexes, &args.node_schemas)?;
    ensure_identity_indexes(concrete.as_ref(), &indexes).await?;
    let identity_indexes_sha256 = sha256(&std::fs::read(&args.identity_indexes)?);
    let node_schemas_sha256 = sha256(&std::fs::read(&args.node_schemas)?);

    let prepare_workers = args.prepare_workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
    });
    anyhow::ensure!(prepare_workers > 0, "--prepare-workers must be positive");
    let metadata_lookup = |paper_id: &str| {
        metadata
            .get(paper_id)
            .cloned()
            .with_context(|| format!("canonical metadata missing for {paper_id}"))
    };
    let runtime = MaterializationRuntime {
        policy_sha256,
        clock,
        prepare_workers,
        metadata: &metadata_lookup,
    };
    let store: Arc<dyn DirectGraphStore> = concrete.clone();
    let materialization = materialize_wave(
        store,
        &FilePipelineCache::new(&args.cache_root),
        &wave,
        &runtime,
    )
    .await?;
    if !materialization.passed() {
        eprintln!(
            "materialization_failure={}",
            serde_json::to_string(&materialization)?
        );
    }
    anyhow::ensure!(
        materialization.passed(),
        "cache-to-graph replay gate failed"
    );
    anyhow::ensure!(
        concrete.node_count().await > 0,
        "materialized graph is empty"
    );

    let snapshot = concrete.export_snapshot().await?;
    let output_snapshot_sha256 = sha256(&snapshot);
    atomic_write(&args.output_snapshot, &snapshot)?;
    let report = GraphWaveReport {
        import_eligible: materialization.import_eligible,
        valid_at: materialization.valid_at.clone(),
        materialization,
        input_snapshot_sha256,
        output_snapshot_sha256,
        output_snapshot: args.output_snapshot.display().to_string(),
        identity_indexes_sha256,
        node_schemas_sha256,
        same_process: true,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
