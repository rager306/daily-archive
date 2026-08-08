//! Run one bounded gold-corpus PDF batch twice to prove cache replay semantics.
//!
//! This canary is deliberately graph-free. Parser and extractor stages cannot
//! mutate graph state; graph materialization is a later always-run audited step.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use da_adapters::{
    FilePipelineCache, GrobidParseStage, GrobidParser, RuleBasedExtractor, RuleExtractionStage,
};
use da_application::{PipelineOrchestrator, PipelineRunReport, StageDisposition};
use da_cli::gold_corpus::{CanonicalSourceKind, GoldCorpusManifest};
use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};
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
    first: PipelineRunReport,
    replay: PipelineRunReport,
}

#[derive(Debug, Serialize)]
struct CanaryReport {
    manifest_version: String,
    batch: usize,
    paper_count: usize,
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
    let records = manifest
        .records
        .iter()
        .filter(|record| record.batch == args.batch)
        .collect::<Vec<_>>();
    anyhow::ensure!(!records.is_empty(), "batch {} has no records", args.batch);
    anyhow::ensure!(
        records
            .iter()
            .all(|record| record.source.kind == CanonicalSourceKind::Pdf),
        "canary currently supports PDF records only; split HTML records into their adapter wave"
    );

    let extraction_configuration_fingerprint =
        file_fingerprint(Path::new("data/extraction_patterns.yaml"))?;
    let cache = Arc::new(FilePipelineCache::new(&args.cache_root));
    let pipeline = PipelineBuilder::new("gold-corpus-parse-extract-v1")
        .step("parse", "grobid_parse", &[])
        .step("extract", "rule_extract", &["parse"])
        .build()
        .map_err(|error| anyhow::anyhow!("build canary pipeline: {error}"))?;
    let mut papers = Vec::with_capacity(records.len());

    for record in records {
        let source_path = record.source.path.as_deref().context("PDF path missing")?;
        let source_sha256 = record
            .source
            .sha256
            .as_deref()
            .context("PDF source fingerprint missing")?;
        let retry_after = (Utc::now() + Duration::minutes(5)).to_rfc3339();
        let orchestrator =
            PipelineOrchestrator::new(cache.clone(), Arc::new(da_ports::SystemClock))
                .with_executor(
                    "grobid_parse",
                    Arc::new(GrobidParseStage::new(
                        Arc::new(GrobidParser::from_env()),
                        source_path,
                        retry_after,
                    )),
                )?
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
            selected_format: SourceFormat::Pdf,
            pdf_sha256: Some(source_sha256.into()),
            html_sha256: None,
            acquisition_fingerprint: "canonical-article-catalog-v1".into(),
            normalization_fingerprint: "grobid-tei-navigation-v3".into(),
        };
        let first = orchestrator.run(&pipeline, source.clone()).await?;
        let replay = orchestrator.run(&pipeline, source).await?;
        papers.push(PaperCanaryReport {
            paper_id: record.paper_id.clone(),
            source_sha256: source_sha256.into(),
            first,
            replay,
        });
    }

    let first = count_dispositions(papers.iter().map(|paper| &paper.first));
    let replay = count_dispositions(papers.iter().map(|paper| &paper.replay));
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
            first.executed == papers.len() * pipeline.steps.len()
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
        paper_count: papers.len(),
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
