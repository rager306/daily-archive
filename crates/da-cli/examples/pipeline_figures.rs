//! Real three-stage cached figure pipeline composition (M371 S02).

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use da_adapters::{FigureDetectionStage, FilePipelineCache, GrobidParseStage, PdfFigures2Adapter};
use da_application::{
    FigureContextStage, FigureSectionLocator, PipelineOrchestrator, StageDisposition,
};
use da_domain::FigureSemanticContext;
use da_parse::GrobidParser;
use da_ports::clock::SystemClock;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceFormat, SourceIdentity};
use da_ports::{FigureDetectionPort, FigureDetectionRequest, ParserPort};
use kg_pipeline::PipelineBuilder;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    pdf: PathBuf,
    #[arg(long)]
    article_dir: PathBuf,
    #[arg(long)]
    article_id: String,
    #[arg(long, env = "PDFFIGURES2_JAR")]
    jar: PathBuf,
    #[arg(long)]
    artifact_id: String,
    #[arg(long)]
    section_path: String,
    #[arg(
        long,
        default_value = "FigureSemanticContext@serde-v2-summary-provenance"
    )]
    context_schema: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("pipeline_figures");
    let args = Args::parse();
    let pdf_bytes = std::fs::read(&args.pdf)
        .with_context(|| format!("read canonical PDF {}", args.pdf.display()))?;
    let pdf_sha256 = hex::encode(Sha256::digest(&pdf_bytes));
    let retry_after = (Utc::now() + Duration::minutes(10)).to_rfc3339();
    let parser: Arc<dyn ParserPort> = Arc::new(GrobidParser::from_env());
    let detector: Arc<dyn FigureDetectionPort> = Arc::new(PdfFigures2Adapter::new(&args.jar));
    let parse_stage = Arc::new(GrobidParseStage::new(parser, &args.pdf, &retry_after));
    let figure_stage = Arc::new(FigureDetectionStage::new(
        detector,
        FigureDetectionRequest {
            article_id: args.article_id.clone(),
            source_pdf_path: args.pdf.clone(),
            source_pdf_sha256: pdf_sha256.clone(),
            canonical_article_dir: args.article_dir.clone(),
            revision_vid: None,
        },
        &retry_after,
    ));
    let context_stage = Arc::new(
        FigureContextStage::new(
            &args.artifact_id,
            FigureSectionLocator::PathId(args.section_path.clone()),
            None,
        )
        .with_output_schema_fingerprint(&args.context_schema),
    );
    let cache_root = args.article_dir.join("pipeline_cache");
    let cache = Arc::new(FilePipelineCache::new(&cache_root));
    let stage_registry =
        da_cli::production::embedded_stage_registry().map_err(anyhow::Error::msg)?;
    let orchestrator =
        PipelineOrchestrator::new(cache.clone(), Arc::new(SystemClock), stage_registry)
            .with_executor("grobid_parse", parse_stage)?
            .with_executor("figure_detect", figure_stage)?
            .with_executor("figure_context", context_stage)?;
    let pipeline = PipelineBuilder::new("article-figure-context-v1")
        .step("parse", "grobid_parse", &[])
        .step("figures", "figure_detect", &[])
        .step("context", "figure_context", &["parse", "figures"])
        .build()
        .map_err(|error| anyhow::anyhow!("build figure pipeline topology: {error}"))?;
    let source = SourceIdentity {
        article_id: args.article_id.clone(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(pdf_sha256),
        html_sha256: None,
        acquisition_fingerprint: "canonical-article-catalog-v1".into(),
        normalization_fingerprint: "grobid-navigation-v3".into(),
    };

    let started = Instant::now();
    let report = orchestrator.run(&pipeline, source).await?;
    let duration_ms = started.elapsed().as_millis() as u64;
    let context_output_sha256 = report.stages.iter().find_map(|record| {
        (record.step_name == "context")
            .then(|| match &record.disposition {
                StageDisposition::Executed { output_sha256 }
                | StageDisposition::CacheHit { output_sha256 } => Some(output_sha256.clone()),
                _ => None,
            })
            .flatten()
    });
    let context_entry = cache
        .manifest(&args.article_id)?
        .into_iter()
        .find(|entry| {
            entry.key.stage_id == "context"
                && entry.key.output_schema_fingerprint == args.context_schema
        })
        .context("context cache manifest entry missing after completed run")?;
    let context = match cache.lookup(&context_entry.key, &Utc::now().to_rfc3339())? {
        CacheLookup::Hit(hit) => serde_json::from_slice::<FigureSemanticContext>(&hit.payload)
            .context("deserialize cached FigureSemanticContext")?,
        _ => anyhow::bail!("completed context stage was not readable as a cache hit"),
    };
    context.validate()?;
    if context.artifact_id != args.artifact_id
        || context.containing_section.path_id != args.section_path
        || !context.caption.contains("GEPA")
        || context.reference_excerpts.is_empty()
    {
        anyhow::bail!("cached FigureSemanticContext failed semantic acceptance");
    }
    let stage_json: Vec<_> = report
        .stages
        .iter()
        .map(|record| {
            let (disposition, output_sha256) = match &record.disposition {
                StageDisposition::Executed { output_sha256 } => {
                    ("executed", Some(output_sha256.as_str()))
                }
                StageDisposition::CacheHit { output_sha256 } => {
                    ("cache_hit", Some(output_sha256.as_str()))
                }
                StageDisposition::RetrySuppressed { .. } => ("retry_suppressed", None),
                StageDisposition::Failed { .. } => ("failed", None),
            };
            serde_json::json!({
                "step": record.step_name,
                "stage": record.stage_id,
                "disposition": disposition,
                "output_sha256": output_sha256,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "pipeline_id": report.pipeline_id,
            "article_id": report.article_id,
            "completed": report.completed,
            "duration_ms": duration_ms,
            "context_output_sha256": context_output_sha256,
            "context_schema": args.context_schema,
            "context_semantic": {
                "artifact_id": context.artifact_id,
                "section_path": context.containing_section.path_id,
                "reference_count": context.reference_excerpts.len(),
                "context_chars": context.total_context_chars,
                "caption_contains_gepa": context.caption.contains("GEPA")
            },
            "cache_root": cache_root,
            "stages": stage_json,
            "import_eligible": false
        }))?
    );
    if !report.completed {
        anyhow::bail!("figure pipeline did not complete");
    }
    Ok(())
}
