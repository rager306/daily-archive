//! Run one bounded gold-corpus source batch twice to prove cache replay semantics.
//!
//! This canary is deliberately graph-free. Parser and extractor stages cannot
//! mutate graph state; graph materialization is a later always-run audited step.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use da_adapters::{
    FilePipelineCache, GrobidParseStage, GrobidParser, HtmlParseStage, HtmlParser,
    RuleBasedExtractor, RuleExtractionStage,
};
use da_application::{PipelineOrchestrator, PipelineRunReport, StageDisposition};
use da_cli::gold_corpus::{CanonicalSourceKind, GoldCorpusManifest};
use da_domain::eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
use da_ports::extractor::ExtractedEntity;
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageOutcome,
};
use kg_pipeline::PipelineBuilder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    manifest: PathBuf,
    #[arg(long, default_value_t = 1)]
    batch: usize,
    #[arg(long, default_value = "artifacts/gold_corpus/pipeline_cache")]
    cache_root: PathBuf,
    #[arg(long, default_value = "artifacts/gold_corpus/canary_batch_01.json")]
    report: PathBuf,
    /// Require the first pass to execute every stage (use with a fresh cache root).
    #[arg(long)]
    require_cold_first: bool,
}

#[derive(Debug, Default, Serialize)]
struct DispositionCounts {
    executed: usize,
    cache_hit: usize,
    retry_suppressed: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct PaperCanaryReport {
    paper_id: String,
    source_sha256: String,
    gold_entities: usize,
    predicted_entities: usize,
    true_positives: usize,
    false_positives: usize,
    false_negatives: usize,
    precision: f64,
    recall: f64,
    f1: f64,
    extraction_payload_sha256: String,
    first: PipelineRunReport,
    replay: PipelineRunReport,
}

#[derive(Debug, Serialize)]
struct BatchMetrics {
    gold_entities: usize,
    predicted_entities: usize,
    true_positives: usize,
    false_positives: usize,
    false_negatives: usize,
    micro_precision: f64,
    micro_recall: f64,
    micro_f1: f64,
    macro_f1: f64,
}

#[derive(Debug, serde::Deserialize)]
struct GoldFixture {
    paper_id: String,
    entities: Vec<GoldEntity>,
}

#[derive(Debug, Serialize)]
struct CanaryReport {
    manifest_version: String,
    batch: usize,
    batch_manifest_records: usize,
    paper_count: usize,
    skipped_unavailable: usize,
    metrics: BatchMetrics,
    first: DispositionCounts,
    replay: DispositionCounts,
    replay_all_cache_hit: bool,
    graph_net_writes: usize,
    graph_boundary: &'static str,
    import_eligible: bool,
    papers: Vec<PaperCanaryReport>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let manifest: GoldCorpusManifest = serde_json::from_slice(
        &std::fs::read(&args.manifest)
            .with_context(|| format!("read {}", args.manifest.display()))?,
    )
    .context("deserialize gold corpus manifest")?;
    let batch_records = manifest
        .records
        .iter()
        .filter(|record| record.batch == args.batch)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !batch_records.is_empty(),
        "batch {} has no records",
        args.batch
    );
    let records = batch_records
        .iter()
        .copied()
        .filter(|record| record.source.kind != CanonicalSourceKind::Unavailable)
        .collect::<Vec<_>>();
    let skipped_unavailable = batch_records.len() - records.len();
    anyhow::ensure!(
        !records.is_empty(),
        "batch {} has no source-ready records",
        args.batch
    );

    let extraction_configuration_fingerprint =
        file_fingerprint(Path::new("data/extraction_patterns.yaml"))?;
    let cache = Arc::new(FilePipelineCache::new(&args.cache_root));
    let mut papers = Vec::with_capacity(records.len());

    for record in records {
        let source_path = record
            .source
            .path
            .as_deref()
            .context("canonical source path missing")?;
        let source_sha256 = record
            .source
            .sha256
            .as_deref()
            .context("canonical source fingerprint missing")?;
        let retry_after = (Utc::now() + Duration::minutes(5)).to_rfc3339();
        let (parse_stage_id, parse_stage, source_format): (
            &str,
            Arc<dyn da_ports::pipeline_stage::PipelineStageExecutor>,
            SourceFormat,
        ) = match record.source.kind {
            CanonicalSourceKind::Pdf => (
                "grobid_parse",
                Arc::new(GrobidParseStage::new(
                    Arc::new(GrobidParser::from_env()),
                    source_path,
                    retry_after,
                )),
                SourceFormat::Pdf,
            ),
            CanonicalSourceKind::Html => (
                "html_parse",
                Arc::new(HtmlParseStage::new(
                    Arc::new(HtmlParser::new()),
                    source_path,
                )),
                SourceFormat::Html,
            ),
            CanonicalSourceKind::Unavailable => unreachable!("unavailable records were filtered"),
        };
        let pipeline = PipelineBuilder::new("gold-corpus-parse-extract-v1")
            .step("parse", parse_stage_id, &[])
            .step("extract", "rule_extract", &["parse"])
            .build()
            .map_err(|error| anyhow::anyhow!("build canary pipeline: {error}"))?;
        let orchestrator =
            PipelineOrchestrator::new(cache.clone(), Arc::new(da_ports::SystemClock))
                .with_executor(parse_stage_id, parse_stage)?
                .with_executor(
                    "rule_extract",
                    Arc::new(RuleExtractionStage::new(
                        Arc::new(RuleBasedExtractor::new()),
                        "parse",
                        extraction_configuration_fingerprint.clone(),
                    )),
                )?;
        let source = SourceIdentity {
            article_id: record.paper_id.clone(),
            selected_format: source_format,
            pdf_sha256: (record.source.kind == CanonicalSourceKind::Pdf)
                .then(|| source_sha256.into()),
            html_sha256: (record.source.kind == CanonicalSourceKind::Html)
                .then(|| source_sha256.into()),
            acquisition_fingerprint: "canonical-article-catalog-v1".into(),
            normalization_fingerprint: match record.source.kind {
                CanonicalSourceKind::Pdf => "grobid-tei-navigation-v3".into(),
                CanonicalSourceKind::Html => "html-structured-sections-v1".into(),
                CanonicalSourceKind::Unavailable => unreachable!(),
            },
        };
        let first = orchestrator.run(&pipeline, source.clone()).await?;
        let replay = orchestrator.run(&pipeline, source).await?;
        let extraction_payload_sha256 = stage_output_sha256(&replay, "extract")?;
        let extracted =
            load_verified_extraction(cache.as_ref(), &record.paper_id, &extraction_payload_sha256)?;
        let gold: GoldFixture = serde_json::from_slice(
            &std::fs::read(&record.fixture_path)
                .with_context(|| format!("read {}", record.fixture_path))?,
        )
        .with_context(|| format!("parse {}", record.fixture_path))?;
        anyhow::ensure!(
            gold.paper_id == record.paper_id,
            "gold identity changed after manifest"
        );
        let predicted = extracted
            .iter()
            .map(|entity| PredictedEntity {
                label: entity.label.clone(),
                entity_type: entity.entity_type.as_str().into(),
            })
            .collect::<Vec<_>>();
        let metrics = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);
        papers.push(PaperCanaryReport {
            paper_id: record.paper_id.clone(),
            source_sha256: source_sha256.into(),
            gold_entities: metrics.gold_count,
            predicted_entities: metrics.predicted_count,
            true_positives: metrics.true_positives,
            false_positives: metrics.false_positives,
            false_negatives: metrics.false_negatives,
            precision: metrics.precision,
            recall: metrics.recall,
            f1: metrics.f1,
            extraction_payload_sha256,
            first,
            replay,
        });
    }

    let first = count_dispositions(papers.iter().map(|paper| &paper.first));
    let replay = count_dispositions(papers.iter().map(|paper| &paper.replay));
    let metrics = aggregate_metrics(&papers);
    let replay_all_cache_hit = papers.iter().all(|paper| {
        paper.replay.completed
            && paper
                .replay
                .stages
                .iter()
                .all(|stage| matches!(stage.disposition, StageDisposition::CacheHit { .. }))
    });
    if args.require_cold_first {
        anyhow::ensure!(
            first.executed == papers.len() * 2
                && first.cache_hit == 0
                && first.failed == 0
                && first.retry_suppressed == 0,
            "cold first pass was not fully executed: {}",
            serde_json::to_string(&first)?
        );
    }
    anyhow::ensure!(
        replay_all_cache_hit,
        "unchanged replay was not fully cached"
    );
    let report = CanaryReport {
        manifest_version: manifest.version,
        batch: args.batch,
        batch_manifest_records: batch_records.len(),
        paper_count: papers.len(),
        skipped_unavailable,
        metrics,
        first,
        replay,
        replay_all_cache_hit,
        graph_net_writes: 0,
        graph_boundary: "no GraphStore is injected into cacheable parse/extract stages",
        import_eligible: false,
        papers,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn stage_output_sha256(report: &PipelineRunReport, step_name: &str) -> Result<String> {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.step_name == step_name)
        .with_context(|| format!("missing stage '{step_name}' in pipeline report"))?;
    match &stage.disposition {
        StageDisposition::Executed { output_sha256 }
        | StageDisposition::CacheHit { output_sha256 } => Ok(output_sha256.clone()),
        other => anyhow::bail!("stage '{step_name}' has no successful output: {other:?}"),
    }
}

fn load_verified_extraction(
    cache: &dyn PipelineCache,
    article_id: &str,
    output_sha256: &str,
) -> Result<Vec<ExtractedEntity>> {
    let matches = cache
        .manifest(article_id)?
        .into_iter()
        .filter(|entry| {
            entry.key.stage_id == "extract"
                && matches!(
                    &entry.outcome,
                    StageOutcome::Success { payload_sha256, .. }
                        if payload_sha256 == output_sha256
                )
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        matches.len() == 1,
        "expected one extraction cache entry for {article_id}/{output_sha256}, found {}",
        matches.len()
    );
    let key = &matches[0].key;
    let CacheLookup::Hit(hit) = cache.lookup(key, &Utc::now().to_rfc3339())? else {
        anyhow::bail!("verified extraction entry was not a cache hit for {article_id}");
    };
    serde_json::from_slice(&hit.payload)
        .with_context(|| format!("deserialize cached extraction for {article_id}"))
}

fn aggregate_metrics(papers: &[PaperCanaryReport]) -> BatchMetrics {
    let gold_entities = papers.iter().map(|paper| paper.gold_entities).sum();
    let predicted_entities = papers.iter().map(|paper| paper.predicted_entities).sum();
    let true_positives = papers.iter().map(|paper| paper.true_positives).sum();
    let false_positives = papers.iter().map(|paper| paper.false_positives).sum();
    let false_negatives = papers.iter().map(|paper| paper.false_negatives).sum();
    let micro_precision = ratio(true_positives, true_positives + false_positives);
    let micro_recall = ratio(true_positives, true_positives + false_negatives);
    let micro_f1 = harmonic_mean(micro_precision, micro_recall);
    let macro_f1 = if papers.is_empty() {
        0.0
    } else {
        papers.iter().map(|paper| paper.f1).sum::<f64>() / papers.len() as f64
    };
    BatchMetrics {
        gold_entities,
        predicted_entities,
        true_positives,
        false_positives,
        false_negatives,
        micro_precision,
        micro_recall,
        micro_f1,
        macro_f1,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn harmonic_mean(left: f64, right: f64) -> f64 {
    if left + right == 0.0 {
        0.0
    } else {
        2.0 * left * right / (left + right)
    }
}

fn count_dispositions<'a>(
    reports: impl Iterator<Item = &'a PipelineRunReport>,
) -> DispositionCounts {
    let mut counts = DispositionCounts::default();
    for stage in reports.flat_map(|report| &report.stages) {
        match stage.disposition {
            StageDisposition::Executed { .. } => counts.executed += 1,
            StageDisposition::CacheHit { .. } => counts.cache_hit += 1,
            StageDisposition::RetrySuppressed { .. } => counts.retry_suppressed += 1,
            StageDisposition::Failed { .. } => counts.failed += 1,
        }
    }
    counts
}

fn file_fingerprint(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}
