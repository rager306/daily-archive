//! Catalog-wide sweep runner: process every catalog article that is NOT in
//! the gold corpus through the verified parse → extract → embed pipeline
//! (graph-free), recording per-article metrics, timings, and typed
//! deviations into a JSON report. Resume-safe: articles already present in
//! an existing report are skipped unless `--force` is given.
//!
//! Usage:
//!   cargo run -p da-cli --example catalog_sweep_runner -- \
//!     --cache-root artifacts/catalog_sweep/pipeline_cache \
//!     --report artifacts/catalog_sweep/sweep_report.json \
//!     --workers 4 --include-embedding

use anyhow::Result;
use clap::Parser;
use da_adapters::{
    FdApiEmbedder, FilePipelineCache, GrobidParseStage, GrobidParser, GrobidProfile,
    ParsedArticleEmbeddingStage, RuleBasedExtractor, RuleExtractionStage,
};
use da_application::{PipelineOrchestrator, PipelineRunReport, StageDisposition};
use da_cli::catalog_sweep::{
    ArticleMetrics, CatalogEntry, DeviationKind, SweepThresholds, classify_deviations,
};
use da_ports::pipeline_cache::{PipelineCache, SourceIdentity};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageContract};
use kg_pipeline::PipelineBuilder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Parser)]
struct Args {
    /// Content-addressed cache root for this sweep.
    #[arg(long, default_value = "artifacts/catalog_sweep/pipeline_cache")]
    cache_root: PathBuf,
    /// Output JSON report (resume source as well).
    #[arg(long, default_value = "artifacts/catalog_sweep/sweep_report.json")]
    report: PathBuf,
    /// Thresholds DATA file.
    #[arg(long, default_value = "data/catalog_sweep_thresholds.yaml")]
    thresholds: PathBuf,
    /// Parallel article workers.
    #[arg(long, default_value_t = 4)]
    workers: usize,
    /// Include the embed stage (FD_API).
    #[arg(long, default_value_t = true)]
    include_embedding: bool,
    /// Process only the first N pending articles (smoke runs).
    #[arg(long)]
    limit: Option<usize>,
    /// Reprocess even if the article already has a record in the report.
    #[arg(long, default_value_t = false)]
    force: bool,
    /// GROBID request timeout seconds.
    #[arg(long, default_value_t = 300)]
    grobid_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PaperSweepRecord {
    paper_id: String,
    category: String,
    outcome: &'static str, // ok | failed | skipped_source
    source_sha256: Option<String>,
    metrics: Option<ArticleMetrics>,
    deviations: Vec<DeviationKind>,
    failure: Option<String>,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct SweepReport {
    schema_version: String,
    thresholds_file: PathBuf,
    total_catalog: usize,
    gold_count: usize,
    processed_target: usize,
    papers: BTreeMap<String, PaperSweepRecord>,
}

fn sha256_of(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Walk the canonical catalog and build inventory entries for articles that
/// are not part of the gold corpus.
fn build_inventory(
    root: &Path,
    gold_ids: &std::collections::BTreeSet<String>,
) -> Result<Vec<CatalogEntry>> {
    let mut out = Vec::new();
    let mut category_dirs: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    category_dirs.sort();
    for cat_dir in category_dirs {
        let category = cat_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let mut article_dirs: Vec<PathBuf> = std::fs::read_dir(&cat_dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        article_dirs.sort();
        for art in article_dirs {
            let Some(paper_id) = art.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let paper_id = paper_id.to_string();
            if gold_ids.contains(&paper_id) {
                continue;
            }
            let source_dir = art.join("source");
            let pdf_path = [source_dir.join(format!("{paper_id}.pdf"))]
                .into_iter()
                .find(|p| p.is_file());
            let html_path = [
                source_dir.join(format!("{paper_id}.html")),
                source_dir.join("abs.html"),
            ]
            .into_iter()
            .find(|p| p.is_file());
            let source_deviation = if !source_dir.exists() {
                Some(da_cli::catalog_sweep::SourceDeviation::NoSourceDir)
            } else if pdf_path.is_none() && html_path.is_none() {
                Some(da_cli::catalog_sweep::SourceDeviation::EmptySource)
            } else if pdf_path.is_none() {
                Some(da_cli::catalog_sweep::SourceDeviation::NoPdf)
            } else {
                None
            };
            let source_sha256 = pdf_path
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .map(|bytes| sha256_of(&bytes));
            out.push(CatalogEntry {
                paper_id,
                category: category.clone(),
                pdf_path,
                html_path,
                source_sha256,
                in_gold: false,
                source_deviation,
            });
        }
    }
    Ok(out)
}

fn fail_record(entry: &CatalogEntry, stage: &str, message: String) -> PaperSweepRecord {
    PaperSweepRecord {
        paper_id: entry.paper_id.clone(),
        category: entry.category.clone(),
        outcome: "failed",
        source_sha256: entry.source_sha256.clone(),
        metrics: None,
        deviations: vec![DeviationKind::Stage {
            stage: stage.into(),
            error_class: message.lines().next().unwrap_or("error").to_string(),
        }],
        failure: Some(message),
        elapsed_ms: 0,
    }
}

/// Extract metrics from a completed pipeline report + cached payloads.
fn collect_metrics(
    report: &PipelineRunReport,
    cache: &dyn PipelineCache,
    article_id: &str,
) -> Result<ArticleMetrics, String> {
    let parse_stage = report
        .stages
        .iter()
        .find(|s| s.step_name == "parse")
        .ok_or("missing parse stage")?;
    let parse_key = parse_stage
        .cache_key_sha256
        .clone()
        .ok_or("parse stage lacks cache key")?;
    let parse_entry = cache
        .manifest(article_id)
        .map_err(|e| format!("manifest: {e}"))?
        .into_iter()
        .find(|e| e.key.stage_id == "parse" && e.key_sha256 == parse_key)
        .ok_or("parse manifest entry not found")?;
    let CacheLookup::Hit(hit) = cache
        .lookup(&parse_entry.key, "2026-01-01T00:00:00Z")
        .map_err(|e| format!("lookup: {e}"))?
    else {
        return Err("parse cache miss on completed run".into());
    };
    let parsed: da_ports::parser::ParsedArticle =
        serde_json::from_slice(&hit.payload).map_err(|e| format!("deserialize parsed: {e}"))?;

    let entities =
        if let Some(extract_stage) = report.stages.iter().find(|s| s.step_name == "extract") {
            let extract_key = extract_stage
                .cache_key_sha256
                .clone()
                .ok_or("extract stage lacks cache key")?;
            let extract_entry = cache
                .manifest(article_id)
                .map_err(|e| format!("manifest: {e}"))?
                .into_iter()
                .find(|e| e.key.stage_id == "extraction_compute" && e.key_sha256 == extract_key)
                .ok_or("extract manifest entry not found")?;
            let CacheLookup::Hit(extract_hit) = cache
                .lookup(&extract_entry.key, "2026-01-01T00:00:00Z")
                .map_err(|e| format!("lookup: {e}"))?
            else {
                return Err("extract cache miss on completed run".into());
            };
            let entities: Vec<da_ports::extractor::ExtractedEntity> =
                serde_json::from_slice(&extract_hit.payload)
                    .map_err(|e| format!("deserialize entities: {e}"))?;
            let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
            for e in &entities {
                *by_type.entry(format!("{:?}", e.entity_type)).or_default() += 1;
            }
            (entities.len(), by_type)
        } else {
            (0, BTreeMap::new())
        };

    Ok(ArticleMetrics {
        sections: parsed.sections.len(),
        body_chars: parsed.body_text.chars().count(),
        citations: parsed.citations.len(),
        tables: parsed.tables.len(),
        formulas: parsed.formulas.len(),
        missingness_pages: parsed
            .page_text_missingness
            .as_ref()
            .map(|m| m.len() as u32)
            .unwrap_or(0),
        pdf_pages: 0,
        entities: entities.0,
        entities_by_type: entities.1,
        parse_ms: 0,
        extract_ms: 0,
        embed_ms: 0,
        pdf_type: None,
        has_encoding_issues: false,
    })
}

use da_ports::pipeline_cache::CacheLookup;

async fn process_one(
    entry: &CatalogEntry,
    cache: Arc<FilePipelineCache>,
    parser: Arc<GrobidParser>,
    extractor: Arc<RuleBasedExtractor>,
    embedder: Option<Arc<FdApiEmbedder>>,
    thresholds: &SweepThresholds,
) -> PaperSweepRecord {
    let t0 = Instant::now();
    let pdf_path = match entry.pdf_path.clone() {
        Some(p) => p,
        None => return fail_record(entry, "source", "pending entry lost its pdf".into()),
    };
    let source_sha256 = entry.source_sha256.clone().unwrap_or_default();
    let retry_after = "2026-01-01T00:00:00Z".to_string();

    let parse_stage: Arc<dyn PipelineStageExecutor> = Arc::new(
        GrobidParseStage::new(parser.clone(), pdf_path.clone(), retry_after.clone())
            .with_contract(StageContract {
                stage_id: "grobid_parse".into(),
                implementation_fingerprint: "GrobidParser@tei-navigation-v3".into(),
                configuration_fingerprint: "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula".into(),
                prompt_fingerprint: None,
                model_fingerprint: Some("grobid/grobid:0.9.0-crf".into()),
                output_schema_fingerprint: "ParsedArticle@serde-v3-unique-navigation".into(),
            })
            .with_retry_delay_seconds(30),
    );
    let extract_stage: Arc<dyn PipelineStageExecutor> = Arc::new(RuleExtractionStage::new(
        extractor.clone(),
        "parse",
        "RuleBasedExtractor@v1",
    ));

    let mut builder = PipelineBuilder::new("catalog-sweep-v1")
        .step("parse", "grobid_parse", &[])
        .step("extract", "rule_extract", &["parse"]);
    if embedder.is_some() {
        builder = builder.step("embed", "paper_embed", &["parse"]);
    }
    let pipeline = match builder.build() {
        Ok(p) => p,
        Err(e) => return fail_record(entry, "build", format!("pipeline build: {e}")),
    };

    let registry = match da_cli::production::embedded_stage_registry() {
        Ok(r) => r,
        Err(e) => return fail_record(entry, "registry", format!("stage registry: {e}")),
    };
    let orchestrator = PipelineOrchestrator::new(
        cache.clone() as Arc<dyn PipelineCache>,
        Arc::new(da_ports::SystemClock),
        registry,
    );
    let orchestrator = match orchestrator.with_executor("grobid_parse", parse_stage) {
        Ok(o) => o,
        Err(e) => return fail_record(entry, "wiring", format!("parse executor: {e}")),
    };
    let orchestrator = match orchestrator.with_executor("rule_extract", extract_stage) {
        Ok(o) => o,
        Err(e) => return fail_record(entry, "wiring", format!("extract executor: {e}")),
    };
    let orchestrator = if let Some(embedder) = &embedder {
        let embed_stage: Arc<dyn PipelineStageExecutor> = Arc::new(
            ParsedArticleEmbeddingStage::new(embedder.clone(), "parse", retry_after),
        );
        match orchestrator.with_executor("paper_embed", embed_stage) {
            Ok(o) => o,
            Err(e) => return fail_record(entry, "wiring", format!("embed executor: {e}")),
        }
    } else {
        orchestrator
    };

    let source = SourceIdentity {
        article_id: entry.paper_id.clone(),
        selected_format: da_ports::pipeline_cache::SourceFormat::Pdf,
        pdf_sha256: Some(source_sha256.clone()),
        html_sha256: None,
        acquisition_fingerprint: "canonical-article-catalog-v1".into(),
        normalization_fingerprint: "grobid-tei-navigation-v3".into(),
    };

    // First run populates cache; second run proves replay (all cache hits).
    let first = orchestrator.run(&pipeline, source.clone()).await;
    let first = match first {
        Ok(r) => r,
        Err(e) => return fail_record(entry, "pipeline", format!("{e:#}")),
    };
    if !first.completed {
        let failed = first.stages.iter().find_map(|s| match &s.disposition {
            StageDisposition::Failed {
                error_class,
                message,
                ..
            } => Some(format!("{}: {} ({})", s.step_name, error_class, message)),
            _ => None,
        });
        return fail_record(
            entry,
            "pipeline",
            failed.unwrap_or_else(|| "pipeline incomplete".into()),
        );
    }

    let metrics = match collect_metrics(&first, cache.as_ref(), &entry.paper_id) {
        Ok(m) => m,
        Err(e) => return fail_record(entry, "metrics", e),
    };
    let deviations = classify_deviations(&metrics, thresholds, None);
    PaperSweepRecord {
        paper_id: entry.paper_id.clone(),
        category: entry.category.clone(),
        outcome: "ok",
        source_sha256: entry.source_sha256.clone(),
        metrics: Some(metrics),
        deviations,
        failure: None,
        elapsed_ms: t0.elapsed().as_millis(),
    }
}

/// M446: DATA-first extractor construction — read the extraction
/// dictionaries from the repository YAML (env DA_EXTRACTION_PATTERNS,
/// default data/extraction_patterns.yaml) with an honest fallback to the
/// bundled snapshot when the file is absent. Previously these runners
/// compiled-in `RuleBasedExtractor::new()`, so editing the YAML never
/// changed runtime behavior.
fn extractor_from_data() -> da_adapters::RuleBasedExtractor {
    let path = std::env::var("DA_EXTRACTION_PATTERNS")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "data/extraction_patterns.yaml".to_string());
    match da_adapters::rule_extractor::ExtractionConfig::from_yaml_file(&path) {
        Ok(config) => da_adapters::RuleBasedExtractor::with_config(config),
        Err(_) => da_adapters::RuleBasedExtractor::new(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let _log_guard = da_cli::logging::init_pipeline_logging("catalog_sweep_runner");
    let args = Args::parse();
    let thresholds = Arc::new(
        SweepThresholds::load(&args.thresholds).map_err(|e| anyhow::anyhow!("thresholds: {e}"))?,
    );

    let manifest_path = PathBuf::from("artifacts/gold_corpus/gold_corpus_manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    let gold_ids: std::collections::BTreeSet<String> = manifest["records"]
        .as_array()
        .expect("manifest records")
        .iter()
        .filter_map(|r| r["paper_id"].as_str().map(String::from))
        .collect();

    let catalog_root = PathBuf::from("data/article_catalog/article_catalog/arxiv");
    let inventory = build_inventory(&catalog_root, &gold_ids)?;
    let with_pdf: Vec<CatalogEntry> = inventory
        .iter()
        .filter(|e| e.pdf_path.is_some() && e.source_deviation.is_none())
        .cloned()
        .collect();
    let source_anomalies: Vec<&CatalogEntry> = inventory
        .iter()
        .filter(|e| e.source_deviation.is_some())
        .collect();

    // Resume needs only the set of already-processed article ids; the fresh
    // run rewrites the whole report with typed records.
    let done_ids: std::collections::BTreeSet<String> = if args.report.exists() && !args.force {
        let bytes = std::fs::read(&args.report)?;
        let prev: serde_json::Value = serde_json::from_slice(&bytes)?;
        prev["papers"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let mut papers: BTreeMap<String, PaperSweepRecord> = BTreeMap::new();

    // Typed source anomalies — no pipeline run possible.
    for entry in &source_anomalies {
        papers
            .entry(entry.paper_id.clone())
            .or_insert_with(|| PaperSweepRecord {
                paper_id: entry.paper_id.clone(),
                category: entry.category.clone(),
                outcome: "skipped_source",
                source_sha256: entry.source_sha256.clone(),
                metrics: None,
                deviations: vec![DeviationKind::Source(
                    entry.source_deviation.clone().expect("anomaly"),
                )],
                failure: None,
                elapsed_ms: 0,
            });
    }

    let pending: Vec<CatalogEntry> = with_pdf
        .iter()
        .filter(|e| args.force || !done_ids.contains(&e.paper_id))
        .take(args.limit.unwrap_or(usize::MAX))
        .cloned()
        .collect();

    println!(
        "catalog sweep: catalog={} gold={} with_pdf={} anomalies={} pending={}",
        inventory.len(),
        gold_ids.len(),
        with_pdf.len(),
        source_anomalies.len(),
        pending.len()
    );

    let cache = Arc::new(FilePipelineCache::new(&args.cache_root));
    let parser = Arc::new(
        GrobidParser::from_env()
            .with_profile(GrobidProfile::structural())
            .with_timeout(std::time::Duration::from_secs(args.grobid_timeout_seconds)),
    );
    let extractor = Arc::new(extractor_from_data());
    let embedder = args
        .include_embedding
        .then(|| Arc::new(FdApiEmbedder::from_env()));

    // Bounded parallelism via a worker pool over an atomic index.
    let pending = Arc::new(pending);
    let queue = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));
    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total = pending.len();
    let started = Instant::now();
    let failures = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..args.workers.clamp(1, 16) {
        let queue = queue.clone();
        let results = results.clone();
        let done = done.clone();
        let failures = failures.clone();
        let cache = cache.clone();
        let parser = parser.clone();
        let extractor = extractor.clone();
        let embedder = embedder.clone();
        let thresholds = thresholds.clone();
        let pending = pending.clone();
        handles.push(tokio::spawn(async move {
            loop {
                let index = queue.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if index >= total {
                    break;
                }
                let entry = &pending[index];
                let t0 = Instant::now();
                let record = process_one(
                    entry,
                    cache.clone(),
                    parser.clone(),
                    extractor.clone(),
                    embedder.clone(),
                    &thresholds,
                )
                .await;
                if record.outcome == "failed" {
                    failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if n.is_multiple_of(10) || n == total {
                    let elapsed = started.elapsed().as_secs();
                    let rate = if elapsed > 0 { n as u64 / elapsed } else { 0 };
                    println!(
                        "progress {n}/{total} failed={} rate={rate}/s elapsed={elapsed}s",
                        failures.load(std::sync::atomic::Ordering::SeqCst)
                    );
                    let _ = std::io::stdout().flush();
                }
                let _ = t0;
                results.lock().unwrap().push(record);
            }
        }));
    }
    for h in handles {
        h.await.map_err(|e| anyhow::anyhow!("worker join: {e}"))?;
    }

    for record in Arc::try_unwrap(results)
        .map_err(|_| anyhow::anyhow!("results arc"))?
        .into_inner()
        .unwrap()
    {
        papers.insert(record.paper_id.clone(), record);
    }

    let report = SweepReport {
        schema_version: "catalog-sweep.v1".into(),
        thresholds_file: args.thresholds.clone(),
        total_catalog: inventory.len(),
        gold_count: gold_ids.len(),
        processed_target: with_pdf.len(),
        papers,
    };
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = args.report.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&report)?)?;
    std::fs::rename(&tmp, &args.report)?;
    println!(
        "sweep done: report={} elapsed={}s",
        args.report.display(),
        started.elapsed().as_secs()
    );
    Ok(())
}
