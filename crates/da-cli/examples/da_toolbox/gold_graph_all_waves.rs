//! Materialize a complete ordered set of verified gold waves in one process.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilePipelineCache, SamyamaGraphStore};
use da_cli::gold_corpus::GoldCorpusManifest;
use da_cli::gold_graph::{
    MaterializationRuntime, atomic_write, catalog_metadata_index, ensure_identity_indexes,
    load_identity_indexes, load_wave_report, materialize_waves, sha256,
};
use da_cli::materialization_progress::{
    MaterializationPhase, MaterializationProgress, ProgressReporter,
};
use da_domain::ContentDigest;
use da_ports::clock::FixedClock;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    /// Ordered report paths; repeat once per expected batch.
    #[arg(long, required = true)]
    wave_report: Vec<PathBuf>,
    #[arg(long)]
    expected_batches: usize,
    #[arg(long)]
    cache_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    manifest: PathBuf,
    #[arg(long)]
    output_snapshot: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(
        long,
        default_value = "scripts/governor/checks/article_context_assurance.yaml"
    )]
    assurance_policy: PathBuf,
    /// Explicit transaction time shared by all waves (RFC3339).
    #[arg(long)]
    valid_at: String,
    /// Parallel immutable cache preparation workers per wave.
    #[arg(long)]
    prepare_workers: Option<usize>,
    #[arg(long, default_value = "data/ontology/identity_indexes.yaml")]
    identity_indexes: PathBuf,
    #[arg(long, default_value = "data/ontology/node_schemas.yaml")]
    node_schemas: PathBuf,
    /// Human/structured progress heartbeat interval in seconds.
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=300))]
    progress_interval_seconds: u64,
    /// Emit STALL when no paper/stage boundary advances for this many seconds.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(2..=3600))]
    progress_stall_seconds: u64,
    /// Disable heartbeat rendering (structured stage events remain available).
    #[arg(long, default_value_t = false)]
    no_progress: bool,
}

#[derive(Debug, Serialize)]
struct AllWavesReport {
    schema_version: &'static str,
    manifest_sha256: String,
    policy_sha256: String,
    identity_indexes_sha256: String,
    node_schemas_sha256: String,
    valid_at: String,
    wave_report_sha256: Vec<String>,
    output_snapshot: String,
    output_snapshot_sha256: String,
    same_process: bool,
    prepare_workers: usize,
    progress: da_cli::materialization_progress::MaterializationProgressSnapshot,
    import_eligible: bool,
    materialization: da_cli::gold_graph::MultiWaveMaterialization,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("gold_graph_all_waves");
    let args = Args::parse();
    kg_storage::refuse_persistence_path_from_env()?;

    let manifest_bytes = std::fs::read(&args.manifest)
        .with_context(|| format!("read {}", args.manifest.display()))?;
    let manifest_sha256 = sha256(&manifest_bytes);
    let manifest: GoldCorpusManifest =
        serde_json::from_slice(&manifest_bytes).context("parse corpus manifest")?;
    let metadata = catalog_metadata_index(&manifest)?;

    let mut reports = Vec::with_capacity(args.wave_report.len());
    let mut wave_report_sha256 = Vec::with_capacity(args.wave_report.len());
    for path in &args.wave_report {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        wave_report_sha256.push(sha256(&bytes));
        let report = load_wave_report(path)?;
        anyhow::ensure!(
            report.manifest_sha256 == manifest_sha256,
            "wave {} does not bind the supplied manifest",
            report.batch
        );
        reports.push(report);
    }

    let policy_bytes = std::fs::read(&args.assurance_policy)
        .with_context(|| format!("read {}", args.assurance_policy.display()))?;
    let policy_sha256 = sha256(&policy_bytes);
    let valid_at = chrono::DateTime::parse_from_rfc3339(&args.valid_at)
        .with_context(|| format!("invalid --valid-at RFC3339: {}", args.valid_at))?
        .with_timezone(&chrono::Utc);
    let clock: Arc<dyn da_ports::Clock> = Arc::new(FixedClock::new(valid_at));

    let prepare_workers = args.prepare_workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
    });
    anyhow::ensure!(prepare_workers > 0, "--prepare-workers must be positive");

    let concrete = Arc::new(SamyamaGraphStore::from_env());
    anyhow::ensure!(
        concrete.node_count().await == 0 && concrete.edge_count().await == 0,
        "all-wave rebuild requires an empty graph"
    );
    let indexes = load_identity_indexes(&args.identity_indexes, &args.node_schemas)?;
    ensure_identity_indexes(concrete.as_ref(), &indexes).await?;
    let identity_indexes_sha256 = sha256(&std::fs::read(&args.identity_indexes)?);
    let node_schemas_sha256 = sha256(&std::fs::read(&args.node_schemas)?);
    let metadata_lookup = |paper_id: &str| {
        metadata
            .get(paper_id)
            .cloned()
            .with_context(|| format!("canonical metadata missing for {paper_id}"))
    };
    let total_papers = reports.iter().map(|report| report.papers.len()).sum();
    let progress = MaterializationProgress::new(reports.len(), total_papers);
    let reporter = (!args.no_progress).then(|| {
        ProgressReporter::spawn(
            progress.clone(),
            std::time::Duration::from_secs(args.progress_interval_seconds),
            std::time::Duration::from_secs(args.progress_stall_seconds),
        )
    });
    let runtime = MaterializationRuntime {
        policy_sha256: ContentDigest::of_bytes(&policy_bytes),
        clock,
        prepare_workers,
        metadata: &metadata_lookup,
        progress: Some(progress.clone()),
    };
    let store: Arc<dyn DirectGraphStore> = concrete.clone();
    let materialization = materialize_waves(
        store,
        &FilePipelineCache::new(&args.cache_root),
        &reports,
        args.expected_batches,
        &runtime,
    )
    .await?;

    progress.phase(MaterializationPhase::Export, reports.len());
    let snapshot = concrete.export_snapshot().await?;
    let output_snapshot_sha256 = sha256(&snapshot);
    atomic_write(&args.output_snapshot, &snapshot)?;
    progress.complete();
    let progress_snapshot =
        progress.snapshot(std::time::Duration::from_secs(args.progress_stall_seconds));
    if let Some(reporter) = reporter {
        reporter.stop();
    }
    let report = AllWavesReport {
        schema_version: "gold-graph-all-waves.v2",
        manifest_sha256,
        policy_sha256,
        identity_indexes_sha256,
        node_schemas_sha256,
        valid_at: materialization.valid_at.clone(),
        wave_report_sha256,
        output_snapshot: args.output_snapshot.display().to_string(),
        output_snapshot_sha256,
        same_process: true,
        prepare_workers,
        progress: progress_snapshot,
        import_eligible: materialization.import_eligible,
        materialization,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
