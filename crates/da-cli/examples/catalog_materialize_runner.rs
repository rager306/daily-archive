//! M440: catalog sweep materialization runner.
//!
//! Two modes:
//!
//! * `--audit-only` (S01): walk the M439 sweep pipeline cache, apply the
//!   ADR-065 `tei_declared_revision` fallback binding per paper, and write a
//!   machine-readable coverage report. Read-only, no graph store.
//! * `--run` (S02): replay cached parse/extraction/embedding artifacts into
//!   an embedded in-process graph store through the explicit MaterializeFsm
//!   (scan → bind → materialize → verify replay-noop), then export the store
//!   to a `.sgsnap` snapshot and a per-paper run report. Fully offline: the
//!   embedding vector comes from the sweep cache, no FD_API or GROBID
//!   service is contacted. D127 stays locked — the snapshot is candidate
//!   evidence, never a production import.
//!
//! Usage:
//!   cargo run -p da-cli --example catalog_materialize_runner -- --audit-only
//!   cargo run -p da-cli --example catalog_materialize_runner -- --run --limit 25
//!
//! Env knobs (see .env.example): DA_MATERIALIZE_LIMIT,
//! DA_MATERIALIZE_SNAPSHOT, DA_MATERIALIZE_RUN_REPORT.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilePipelineCache, SamyamaGraphStore};
#[cfg(feature = "pdf-classifier")]
use da_application::catalog_materialization::arxiv_stamp_revision;
use da_application::catalog_materialization::{
    PaperMaterializeEvent, PaperMaterializeFsm, PaperMaterializeState, RunMaterializeEvent,
    RunMaterializeFsm, RunMaterializeState, SKIP_AMBIGUOUS_PARSE_KEYS, SKIP_ASSET_MISSING,
    SKIP_NO_EMBEDDING_CACHE, SKIP_NO_EXTRACTION_CACHE, SKIP_NO_PARSE_CACHE,
};
use da_application::revision_materialization::{
    RevisionContentContext, RevisionManifestationInput,
};
use da_application::{ExtractionUseCase, IngestUseCase};
use da_cli::catalog_materialize::{
    CoverageReport, CoverageVerdict, LoadArtifactsError, audit, load_paper_artifacts,
    load_sweep_ok_papers, prior_completed_ids,
};
use da_domain::PdfTriageStatus;
use da_ports::graph_store::DirectGraphStore;
use da_ports::{Clock, SystemClock};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[derive(Debug, Parser)]
struct Args {
    /// Coverage audit only; no graph writes (S01 mode).
    #[arg(long)]
    audit_only: bool,
    /// Materialization run (S02 mode).
    #[arg(long)]
    run: bool,
    /// Version recovery over skipped papers (M441; feature pdf-classifier).
    /// Writes a typed recovery report; no graph writes.
    #[arg(long)]
    recover: bool,
    /// Content-addressed sweep pipeline cache root.
    #[arg(long, default_value = "artifacts/catalog_sweep/pipeline_cache")]
    cache_root: PathBuf,
    /// M439 sweep report (source of truth for `ok` papers + source hashes).
    #[arg(long, default_value = "artifacts/catalog_sweep/sweep_report.json")]
    sweep_report: PathBuf,
    /// S01 coverage report (input for --run, output for --audit-only).
    #[arg(
        long,
        default_value = "artifacts/catalog_sweep/materialization_coverage.json"
    )]
    report: PathBuf,
    /// Run report output (S02). Default: DA_MATERIALIZE_RUN_REPORT env or
    /// artifacts/catalog_sweep/materialization_run_report.json.
    #[arg(long)]
    run_report: Option<PathBuf>,
    /// Snapshot output (S02). Default: DA_MATERIALIZE_SNAPSHOT env or
    /// artifacts/catalog_sweep/catalog_materialized.sgsnap.
    #[arg(long)]
    snapshot: Option<PathBuf>,
    /// Canonical article catalog root (asset evidence for manifestations).
    #[arg(long, default_value = "data/article_catalog/article_catalog/arxiv")]
    catalog_root: PathBuf,
    /// Process only the first N materializable papers (canary runs).
    /// Default: DA_MATERIALIZE_LIMIT env or 0 (all).
    #[arg(long)]
    limit: Option<usize>,
    /// Resume an interrupted run (M442): preload the existing snapshot,
    /// skip papers already completed in the existing run report, merge
    /// records. Default: DA_MATERIALIZE_RESUME=1 enables.
    #[arg(long)]
    resume: bool,
    /// Recovery report output (--recover). Default:
    /// DA_MATERIALIZE_RECOVERY_REPORT env or
    /// artifacts/catalog_sweep/materialization_recovery.json.
    #[arg(long)]
    recovery_report: Option<PathBuf>,
}

impl Args {
    #[cfg(feature = "pdf-classifier")]
    fn recovery_report_path(&self) -> PathBuf {
        self.recovery_report.clone().unwrap_or_else(|| {
            PathBuf::from(env_or(
                "DA_MATERIALIZE_RECOVERY_REPORT",
                "artifacts/catalog_sweep/materialization_recovery.json",
            ))
        })
    }
}

impl Args {
    fn run_report_path(&self) -> PathBuf {
        self.run_report.clone().unwrap_or_else(|| {
            PathBuf::from(env_or(
                "DA_MATERIALIZE_RUN_REPORT",
                "artifacts/catalog_sweep/materialization_run_report.json",
            ))
        })
    }

    fn snapshot_path(&self) -> PathBuf {
        self.snapshot.clone().unwrap_or_else(|| {
            PathBuf::from(env_or(
                "DA_MATERIALIZE_SNAPSHOT",
                "artifacts/catalog_sweep/catalog_materialized.sgsnap",
            ))
        })
    }

    fn limit_value(&self) -> usize {
        self.limit
            .or_else(|| {
                std::env::var("DA_MATERIALIZE_LIMIT")
                    .ok()
                    .and_then(|v| v.trim().parse().ok())
            })
            .unwrap_or(0)
    }
}

fn main() -> Result<()> {
    // ADR-052 composition root: .env-backed rolling JSON logging (governor:
    // pipeline_composition_root_without_file_logging).
    let _log_guard = da_cli::logging::init_pipeline_logging("catalog_materialize_runner");
    let args = Args::parse();
    let modes = args.audit_only as u8 + args.run as u8 + args.recover as u8;
    if modes != 1 {
        anyhow::bail!("pass exactly one mode: --audit-only, --run or --recover");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;
    runtime.block_on(async_main(args))
}

async fn async_main(args: Args) -> Result<()> {
    anyhow::ensure!(
        args.cache_root.is_dir(),
        "cache root missing: {}",
        args.cache_root.display()
    );
    if args.audit_only {
        run_audit(&args)
    } else if args.recover {
        #[cfg(feature = "pdf-classifier")]
        {
            recovery::run_recovery(&args)
        }
        #[cfg(not(feature = "pdf-classifier"))]
        {
            anyhow::bail!(
                "--recover requires the pdf-classifier feature: cargo run -p da-cli --features pdf-classifier --example catalog_materialize_runner -- --recover"
            )
        }
    } else {
        run_materialization(&args).await
    }
}

/// M441 S02: offline version recovery over skipped papers. Feature-gated:
/// pdf-inspector links only under da-adapters/pdf-classifier.
#[cfg(feature = "pdf-classifier")]
mod recovery {

    use super::*;
    use da_application::catalog_materialization::{
        RECOVERY_REASON_NO_STAMP, RECOVERY_REASON_PDF_ERROR, RECOVERY_SOURCE_PDF_STAMP,
        arxiv_stamp_revision,
    };

    #[derive(Debug, serde::Serialize)]
    pub(super) struct RecoveryRecord {
        pub paper_id: String,
        pub category: Option<String>,
        pub outcome: &'static str,
        pub revision: Option<u32>,
        pub source: Option<&'static str>,
        pub reason: Option<&'static str>,
    }

    #[derive(Debug, Default, serde::Serialize)]
    pub(super) struct RecoveryTotals {
        pub recovered: usize,
        pub still_unbound: usize,
        pub pdf_error: usize,
    }

    #[derive(Debug, serde::Serialize)]
    pub(super) struct RecoveryReport {
        pub schema_version: u32,
        pub totals: RecoveryTotals,
        pub papers: Vec<RecoveryRecord>,
    }

    pub(super) fn run_recovery(args: &Args) -> Result<()> {
        let coverage: CoverageReport = serde_json::from_slice(&std::fs::read(&args.report)?)
            .with_context(|| format!("parse coverage report {}", args.report.display()))?;
        let targets: Vec<(String, Option<String>)> = coverage
            .papers
            .iter()
            .filter(|p| p.verdict == CoverageVerdict::Skipped)
            .map(|p| (p.paper_id.clone(), p.category.clone()))
            .collect();
        anyhow::ensure!(!targets.is_empty(), "no skipped papers in coverage report");

        let mut report = RecoveryReport {
            schema_version: 1,
            totals: RecoveryTotals::default(),
            papers: Vec::with_capacity(targets.len()),
        };
        for (paper_id, category) in targets {
            let pdf = args
                .catalog_root
                .join(category.as_deref().unwrap_or("unknown"))
                .join(&paper_id)
                .join("source")
                .join(format!("{paper_id}.pdf"));
            let record = match std::fs::read(&pdf) {
                Ok(bytes) => {
                    // The first two pages carry the arXiv stamp when present.
                    let options = da_adapters::pdf_inspector::PdfOptions::new().pages([0u32, 1]);
                    match da_adapters::pdf_inspector::process_pdf_mem_with_options(&bytes, options)
                    {
                        Ok(result) => {
                            let markdown = result.markdown.unwrap_or_default();
                            match arxiv_stamp_revision(&markdown, &paper_id) {
                                Some(revision) => {
                                    tracing::info!(paper = %paper_id, revision, source = "pdf_stamp", "revision recovered");
                                    report.totals.recovered += 1;
                                    RecoveryRecord {
                                        paper_id,
                                        category,
                                        outcome: "recovered",
                                        revision: Some(revision),
                                        source: Some(RECOVERY_SOURCE_PDF_STAMP),
                                        reason: None,
                                    }
                                }
                                None => {
                                    report.totals.still_unbound += 1;
                                    RecoveryRecord {
                                        paper_id,
                                        category,
                                        outcome: "still_unbound",
                                        revision: None,
                                        source: None,
                                        reason: Some(RECOVERY_REASON_NO_STAMP),
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            tracing::warn!(paper = %paper_id, %error, "pdf-inspector failed");
                            report.totals.pdf_error += 1;
                            RecoveryRecord {
                                paper_id,
                                category,
                                outcome: "pdf_error",
                                revision: None,
                                source: None,
                                reason: Some(RECOVERY_REASON_PDF_ERROR),
                            }
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(paper = %paper_id, %error, "asset missing for recovery");
                    report.totals.pdf_error += 1;
                    RecoveryRecord {
                        paper_id,
                        category,
                        outcome: "pdf_error",
                        revision: None,
                        source: None,
                        reason: Some(RECOVERY_REASON_PDF_ERROR),
                    }
                }
            };
            report.papers.push(record);
        }

        let path = args.recovery_report_path();
        let json = serde_json::to_vec_pretty(&report)?;
        da_cli::gold_graph::atomic_write(&path, &json)
            .with_context(|| format!("write recovery report {}", path.display()))?;
        println!(
            "recovery: {} recovered | {} still unbound | {} pdf errors",
            report.totals.recovered, report.totals.still_unbound, report.totals.pdf_error
        );
        println!("report: {}", path.display());
        Ok(())
    }
}

fn run_audit(args: &Args) -> Result<()> {
    let mut cache_ids: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&args.cache_root)
        .with_context(|| format!("read cache root {}", args.cache_root.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            cache_ids.push(name.to_string());
        }
    }
    cache_ids.sort();

    let ok_papers = load_sweep_ok_papers(&args.sweep_report)?;
    let cache = FilePipelineCache::new(&args.cache_root);
    let coverage = audit(&cache, &ok_papers, &cache_ids)?;

    let json = serde_json::to_vec_pretty(&coverage)?;
    da_cli::gold_graph::atomic_write(&args.report, &json)
        .with_context(|| format!("write coverage report {}", args.report.display()))?;

    println!(
        "coverage: {} ok papers | {} materializable | {} skipped",
        coverage.totals.papers, coverage.totals.materializable, coverage.totals.skipped
    );
    for (reason, count) in &coverage.totals.by_reason {
        println!("  {reason}: {count}");
    }
    if !coverage.reconcile.extra_cache_papers.is_empty() {
        println!(
            "  extra cache papers (not in sweep ok set): {}",
            coverage.reconcile.extra_cache_papers.len()
        );
    }
    println!("report: {}", args.report.display());
    Ok(())
}

#[derive(Debug, Serialize)]
struct PaperRunRecord {
    paper_id: String,
    revision: Option<u32>,
    #[serde(flatten)]
    state: PaperMaterializeState,
}

#[derive(Debug, Serialize)]
struct RunReport {
    schema_version: u32,
    snapshot: String,
    totals: RunTotals,
    run_state: RunMaterializeState,
    /// Prior-run records (raw JSON on resume) plus new typed records.
    papers: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Default)]
struct RunTotals {
    completed: usize,
    skipped: usize,
    failed: usize,
    replay_noop_violations: usize,
}

fn skip_reason_for(error: &LoadArtifactsError) -> &'static str {
    match error {
        LoadArtifactsError::NoParseCache
        | LoadArtifactsError::CacheLookupFailed {
            stage: "manifest", ..
        } => SKIP_NO_PARSE_CACHE,
        LoadArtifactsError::AmbiguousParseKeys { .. } => SKIP_AMBIGUOUS_PARSE_KEYS,
        LoadArtifactsError::NoExtractionCache => SKIP_NO_EXTRACTION_CACHE,
        LoadArtifactsError::NoEmbeddingCache | LoadArtifactsError::EmptyEmbedding => {
            SKIP_NO_EMBEDDING_CACHE
        }
        LoadArtifactsError::PayloadCorrupt { .. }
        | LoadArtifactsError::CacheLookupFailed { .. } => SKIP_NO_PARSE_CACHE,
    }
}

async fn run_materialization(args: &Args) -> Result<()> {
    // Input: coverage report (S01 artifact) filtered to materializable.
    let mut coverage: CoverageReport = serde_json::from_slice(&std::fs::read(&args.report)?)
        .with_context(|| format!("parse coverage report {}", args.report.display()))?;
    // Optional M441 merge: recovered revisions upgrade no_tei_revision skips
    // to materializable before target selection (env DA_MATERIALIZE_RECOVERY
    // points at the recovery report; unset = no merge).
    let mut recovered_count = 0usize;
    let recovery_path = std::env::var("DA_MATERIALIZE_RECOVERY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from);
    if let Some(path) = &recovery_path {
        let recovery: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)
            .with_context(|| format!("parse recovery report {}", path.display()))?;
        let mut revisions: BTreeMap<String, u32> = BTreeMap::new();
        if let Some(papers) = recovery.get("papers").and_then(|p| p.as_array()) {
            for row in papers {
                let (Some(outcome), Some(paper)) = (
                    row.get("outcome").and_then(|v| v.as_str()),
                    row.get("paper_id").and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                if outcome == "recovered"
                    && let Some(revision) = row.get("revision").and_then(|v| v.as_u64())
                {
                    revisions.insert(paper.to_string(), revision as u32);
                }
            }
        }
        for paper in &mut coverage.papers {
            if paper.verdict == CoverageVerdict::Skipped
                && let Some(revision) = revisions.remove(&paper.paper_id)
            {
                paper.verdict = CoverageVerdict::Materializable;
                paper.revision = Some(revision);
                paper.reason = None;
                recovered_count += 1;
            }
        }
        tracing::info!(recovered = recovered_count, "recovery merge applied");
    }
    if recovered_count > 0 {
        println!("recovery merge: +{recovered_count} materializable from pdf stamps");
    }
    let mut targets: Vec<(String, Option<String>, u32)> = coverage
        .papers
        .iter()
        .filter(|p| p.verdict == CoverageVerdict::Materializable)
        .filter_map(|p| {
            p.revision
                .map(|revision| (p.paper_id.clone(), p.category.clone(), revision))
        })
        .collect();
    let limit = args.limit_value();
    if limit > 0 {
        targets.truncate(limit);
    }
    anyhow::ensure!(
        !targets.is_empty(),
        "no materializable papers in coverage report"
    );

    // M442 crash-safe resume: an interrupted run continues instead of
    // restarting from zero. Prior completed papers are skipped (their graph
    // state is already in the preloaded snapshot); failed/skipped papers
    // are retried. Totals merge prior + new.
    let resume = args.resume
        || std::env::var("DA_MATERIALIZE_RESUME")
            .ok()
            .is_some_and(|v| v.trim() == "1");
    let run_report_path = args.run_report_path();
    let snapshot_path = args.snapshot_path();
    let mut prior_records: Vec<serde_json::Value> = Vec::new();
    let mut prior_totals = RunTotals::default();
    let mut prior_processed = 0usize;
    if resume && let Ok(prior_bytes) = std::fs::read(&run_report_path) {
        let prior_ids = prior_completed_ids(&prior_bytes)
            .with_context(|| format!("parse prior run report {}", run_report_path.display()))?;
        let prior: serde_json::Value = serde_json::from_slice(&prior_bytes)
            .with_context(|| format!("parse prior run report {}", run_report_path.display()))?;
        if let Some(papers) = prior.get("papers").and_then(|p| p.as_array()) {
            for paper in papers {
                let completed = paper
                    .get("state")
                    .and_then(|s| s.as_str())
                    .is_some_and(|s| s == "completed");
                if completed {
                    prior_processed += 1;
                }
                prior_records.push(paper.clone());
            }
        }
        if let Some(t) = prior.get("totals") {
            prior_totals.completed =
                t.get("completed").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            prior_totals.skipped = t.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            prior_totals.failed = t.get("failed").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            prior_totals.replay_noop_violations = t
                .get("replay_noop_violations")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
        }
        let before = targets.len();
        targets.retain(|(id, _, _)| !prior_ids.contains(id));
        println!(
            "resume: {} prior records ({} completed), targets {} -> {}",
            prior_records.len(),
            prior_processed,
            before,
            targets.len()
        );
    }

    // Source hashes from the sweep report (manifestation evidence).
    let sweep: serde_json::Value = serde_json::from_slice(&std::fs::read(&args.sweep_report)?)?;
    let mut source_sha: BTreeMap<String, String> = BTreeMap::new();
    if let Some(papers) = sweep.get("papers").and_then(|p| p.as_object()) {
        for (id, row) in papers {
            if let Some(sha) = row.get("source_sha256").and_then(|v| v.as_str()) {
                source_sha.insert(id.clone(), sha.to_string());
            }
        }
    }

    // Embedded in-process store; refuses remote/persistence modes honestly.
    let store: Arc<dyn DirectGraphStore> = Arc::new(SamyamaGraphStore::try_from_env()?);

    // Snapshot preload on resume: continue from the graph state the
    // interrupted run already wrote.
    if resume && snapshot_path.is_file() {
        let bytes = std::fs::read(&snapshot_path)
            .with_context(|| format!("read prior snapshot {}", snapshot_path.display()))?;
        store
            .import_snapshot(&bytes)
            .await
            .context("import prior snapshot")?;
        println!(
            "resume: preloaded snapshot {} ({} nodes / {} edges)",
            snapshot_path.display(),
            store.node_count().await,
            store.edge_count().await
        );
    }

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ingest =
        IngestUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));
    let extraction =
        ExtractionUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));
    let cache = FilePipelineCache::new(&args.cache_root);

    let mut run_fsm = RunMaterializeFsm::new();
    run_fsm.apply(RunMaterializeEvent::Started)?;
    for _ in 0..prior_processed {
        run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
    }
    let mut papers: Vec<PaperRunRecord> = Vec::with_capacity(targets.len());
    let mut totals = RunTotals::default();

    for (paper_id, category, revision) in targets {
        let mut fsm = PaperMaterializeFsm::new(&paper_id);
        let artifacts = match load_paper_artifacts(&cache, &paper_id) {
            Ok(artifacts) => {
                fsm.apply(PaperMaterializeEvent::ScanDone {
                    has_parse: true,
                    has_extraction: true,
                    has_embedding: true,
                })
                .expect("scan from pending is legal");
                artifacts
            }
            Err(error) => {
                tracing::warn!(paper = %paper_id, %error, "artifact load failed");
                fsm.apply(PaperMaterializeEvent::ScanDone {
                    has_parse: false,
                    has_extraction: false,
                    has_embedding: false,
                })
                .expect("scan from pending is legal");
                fsm.apply(PaperMaterializeEvent::BindingSkipped {
                    reason: skip_reason_for(&error),
                })
                .expect("skip from scanned is legal");
                totals.skipped += 1;
                papers.push(record(&paper_id, &mut fsm));
                run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
                continue;
            }
        };

        // Manifestation evidence: the canonical catalog asset behind the
        // cached parse. Missing asset → typed skip, never a phantom write.
        let asset = args
            .catalog_root
            .join(category.as_deref().unwrap_or("unknown"))
            .join(&paper_id)
            .join("source")
            .join(format!("{paper_id}.pdf"));
        let asset_meta = match std::fs::metadata(&asset) {
            Ok(meta) => meta,
            Err(_) => {
                fsm.apply(PaperMaterializeEvent::BindingSkipped {
                    reason: SKIP_ASSET_MISSING,
                })
                .expect("skip from scanned is legal");
                totals.skipped += 1;
                papers.push(record(&paper_id, &mut fsm));
                run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
                continue;
            }
        };
        // Manifestation digest must equal the parsed source digest (the
        // revision materialization validates 64-hex equality with
        // parsed.pdf_hash). Cross-check against the sweep-reported source
        // hash: drift is a typed failure, never a silent mismatch.
        let effective_sha = artifacts.parsed.pdf_hash.clone();
        if let Some(reported) = source_sha.get(&paper_id)
            && reported != &effective_sha
        {
            fsm.apply(PaperMaterializeEvent::Fail {
                stage: "materialize",
                error: format!(
                    "source digest drift for {paper_id}: sweep={reported} parsed={effective_sha}"
                ),
            })
            .expect("fail from live state is legal");
            totals.failed += 1;
            papers.push(record(&paper_id, &mut fsm));
            run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
            continue;
        }
        let asset_path = asset.display().to_string();
        let context = RevisionContentContext {
            source_code: "arxiv".to_string(),
            canonical_id: paper_id.clone(),
            revision,
            manifestations: vec![RevisionManifestationInput {
                format: "pdf".to_string(),
                sha256: effective_sha,
                bytes: asset_meta.len(),
                uri: format!("catalog:{asset_path}"),
                path: asset_path,
                capture_status: "captured_local".to_string(),
            }],
        };
        fsm.apply(PaperMaterializeEvent::BindingResolved { revision })
            .expect("binding from scanned is legal");

        // First materialization pass.
        let ingest_result = ingest
            .materialize_parsed_with_context(
                &artifacts.parsed,
                artifacts.embedding.clone(),
                category.clone(),
                Some("arxiv".to_string()),
                None,
                PdfTriageStatus::NotConfigured,
                Some(&context),
            )
            .await;
        let extract_result = match ingest_result {
            Ok(_) => {
                extraction
                    .materialize_extracted_with_context(
                        &artifacts.parsed,
                        artifacts.entities.clone(),
                        Some(&context),
                    )
                    .await
            }
            Err(e) => Err(e),
        };
        let first = match extract_result {
            Ok(result) => result,
            Err(error) => {
                fsm.apply(PaperMaterializeEvent::Fail {
                    stage: "materialize",
                    error: error.to_string(),
                })
                .expect("fail from live state is legal");
                totals.failed += 1;
                papers.push(record(&paper_id, &mut fsm));
                run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
                continue;
            }
        };
        fsm.apply(PaperMaterializeEvent::MaterializeDone {
            entities_created: first.entities_created,
            mentions_edges: first.mentions_edges,
        })
        .expect("materialize from bound is legal");

        // Replay: the same artifacts must be a no-op (idempotency gate).
        let before = (store.node_count().await, store.edge_count().await);
        let replay_ingest = ingest
            .materialize_parsed_with_context(
                &artifacts.parsed,
                artifacts.embedding.clone(),
                category.clone(),
                Some("arxiv".to_string()),
                None,
                PdfTriageStatus::NotConfigured,
                Some(&context),
            )
            .await;
        let replay_extract = match replay_ingest {
            Ok(_) => {
                extraction
                    .materialize_extracted_with_context(
                        &artifacts.parsed,
                        artifacts.entities,
                        Some(&context),
                    )
                    .await
            }
            Err(e) => Err(e),
        };
        let after = (store.node_count().await, store.edge_count().await);
        match replay_extract {
            Ok(_) => {
                let noop = before == after;
                if !noop {
                    totals.replay_noop_violations += 1;
                }
                fsm.apply(PaperMaterializeEvent::ReplayVerified { noop })
                    .expect("verify from materialized is legal");
                totals.completed += 1;
            }
            Err(error) => {
                fsm.apply(PaperMaterializeEvent::Fail {
                    stage: "verify",
                    error: error.to_string(),
                })
                .expect("fail from live state is legal");
                totals.failed += 1;
            }
        }
        papers.push(record(&paper_id, &mut fsm));
        run_fsm.apply(RunMaterializeEvent::PaperProcessed)?;
    }

    // Export.
    run_fsm.apply(RunMaterializeEvent::ExportStarted)?;
    let snapshot_bytes = store.export_snapshot().await.context("export snapshot")?;
    da_cli::gold_graph::atomic_write(&snapshot_path, &snapshot_bytes)
        .with_context(|| format!("write snapshot {}", snapshot_path.display()))?;
    run_fsm.apply(RunMaterializeEvent::ExportDone {
        snapshot: snapshot_path.display().to_string(),
    })?;

    totals.completed += prior_totals.completed;
    totals.skipped += prior_totals.skipped;
    totals.failed += prior_totals.failed;
    totals.replay_noop_violations += prior_totals.replay_noop_violations;
    let mut paper_values: Vec<serde_json::Value> = prior_records.into_iter().collect();
    paper_values.extend(
        papers
            .into_iter()
            .filter_map(|record| serde_json::to_value(record).ok()),
    );
    let report = RunReport {
        schema_version: 1,
        snapshot: snapshot_path.display().to_string(),
        totals,
        run_state: run_fsm.state().clone(),
        papers: paper_values,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    da_cli::gold_graph::atomic_write(&run_report_path, &json)
        .with_context(|| format!("write run report {}", run_report_path.display()))?;

    println!(
        "materialized: {} completed | {} skipped | {} failed | replay noop violations: {}",
        report.totals.completed,
        report.totals.skipped,
        report.totals.failed,
        report.totals.replay_noop_violations
    );
    println!("snapshot: {}", snapshot_path.display());
    println!("report: {}", run_report_path.display());

    anyhow::ensure!(
        report.totals.failed == 0 && report.totals.replay_noop_violations == 0,
        "materialization run did not pass its honesty gate"
    );
    Ok(())
}

fn record(paper_id: &str, fsm: &mut PaperMaterializeFsm) -> PaperRunRecord {
    let revision = match fsm.state() {
        PaperMaterializeState::Completed { revision, .. }
        | PaperMaterializeState::Materialized { revision, .. }
        | PaperMaterializeState::Bound { revision } => Some(*revision),
        _ => None,
    };
    PaperRunRecord {
        paper_id: paper_id.to_string(),
        revision,
        state: fsm.state().clone(),
    }
}
