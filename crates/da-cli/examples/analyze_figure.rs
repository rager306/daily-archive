//! Real provider-neutral cached visual analysis composition (M371 S03).

use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use clap::{Parser, ValueEnum};
use da_adapters::{AnthropicMultimodalClient, AnthropicMultimodalRoute, FilePipelineCache};
use da_application::{
    FigureAnalysisStage, FigureAnalysisUseCase, FigureContextArtifactStage, PipelineOrchestrator,
    StageDisposition,
};
use da_domain::{FigureAnalysis, FigureSemanticContext, VisualAnalysisType};
use da_ports::clock::SystemClock;
use da_ports::multimodal_llm::{ImageMediaType, MultimodalImage, MultimodalLlmPort};
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceFormat, SourceIdentity};
use kg_pipeline::PipelineBuilder;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RouteChoice {
    Auto,
    NineRouter,
    DirectMinimax,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    context: PathBuf,
    #[arg(long)]
    article_dir: PathBuf,
    #[arg(long)]
    article_id: String,
    #[arg(long, value_enum, default_value = "auto")]
    route: RouteChoice,
    #[arg(long, default_value = "http://127.0.0.1:20128/v1")]
    router_url: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value = "MiniMax-M3-512k")]
    direct_model: String,
    #[arg(long, default_value_t = 300)]
    timeout_seconds: u64,
    #[arg(long)]
    cache_schema_fingerprint: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, serde::Deserialize)]
struct ModelList {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct ModelEntry {
    id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let context_bytes = std::fs::read(&args.context)
        .with_context(|| format!("read context {}", args.context.display()))?;
    let context: FigureSemanticContext = serde_json::from_slice(&context_bytes)?;
    context.validate()?;
    let image_path = args.article_dir.join(&context.asset_path);
    let image_bytes = std::fs::read(&image_path)
        .with_context(|| format!("read canonical image {}", image_path.display()))?;
    let image_sha256 = sha256(&image_bytes);
    if image_sha256 != context.asset_sha256 {
        bail!("canonical image checksum differs from FigureSemanticContext");
    }
    let (route, selection_reason) = select_route(&args).await?;
    let route_descriptor = route.clone();
    let model: Arc<dyn MultimodalLlmPort> = Arc::new(AnthropicMultimodalClient::with_timeout(
        route,
        StdDuration::from_secs(args.timeout_seconds),
    )?);
    let context_stage = Arc::new(FigureContextArtifactStage::new(context.clone())?);
    let image = MultimodalImage {
        media_type: media_type(&image_path)?,
        bytes: image_bytes,
        sha256: image_sha256,
    };
    let retry_after = (Utc::now() + Duration::minutes(10)).to_rfc3339();
    let mut analysis_stage = FigureAnalysisStage::new(model, image, retry_after)
        .with_context_dependency("analysis_context");
    let cache_schema = args
        .cache_schema_fingerprint
        .clone()
        .unwrap_or_else(FigureAnalysisUseCase::output_schema_sha256);
    if args.cache_schema_fingerprint.is_some() {
        analysis_stage = analysis_stage
            .with_contract_versions(FigureAnalysisUseCase::prompt_sha256(), cache_schema.clone());
    }
    let analysis_stage = Arc::new(analysis_stage);
    let cache_root = args.article_dir.join("pipeline_cache");
    let cache = Arc::new(FilePipelineCache::new(&cache_root));
    let orchestrator = PipelineOrchestrator::new(cache.clone(), Arc::new(SystemClock))
        .with_executor("figure_context_artifact", context_stage)?
        .with_executor("figure_analysis", analysis_stage)?;
    let pipeline = PipelineBuilder::new("article-figure-analysis-v1")
        .step("analysis_context", "figure_context_artifact", &[])
        .step("analysis", "figure_analysis", &["analysis_context"])
        .build()
        .map_err(|error| anyhow::anyhow!("build figure analysis pipeline: {error}"))?;
    let source = SourceIdentity {
        article_id: args.article_id.clone(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(context.source_pdf_sha256.clone()),
        html_sha256: None,
        acquisition_fingerprint: "canonical-article-catalog-v1".into(),
        normalization_fingerprint: "figure-context-v2".into(),
    };

    let started = Instant::now();
    let report = orchestrator.run(&pipeline, source).await?;
    let wall_duration_ms = started.elapsed().as_millis() as u64;
    if !report.completed {
        bail!(
            "figure analysis pipeline did not complete: {:?}",
            report.stages
        );
    }
    let entry = cache
        .manifest(&args.article_id)?
        .into_iter()
        .find(|entry| {
            entry.key.stage_id == "analysis"
                && entry.key.output_schema_fingerprint == cache_schema
                && entry
                    .key
                    .model_fingerprint
                    .as_deref()
                    .is_some_and(|value| value.contains(&route_descriptor.route_id))
        })
        .context("analysis cache manifest entry missing")?;
    let analysis = match cache.lookup(&entry.key, &Utc::now().to_rfc3339())? {
        CacheLookup::Hit(hit) => serde_json::from_slice::<FigureAnalysis>(&hit.payload)?,
        _ => bail!("completed analysis was not readable as a cache hit"),
    };
    analysis.validate()?;
    semantic_acceptance(&analysis, &context)?;
    let output = args.output.unwrap_or_else(|| {
        args.article_dir
            .join("figures")
            .join(format!("{}.analysis.json", context.artifact_id))
    });
    let bytes = serde_json::to_vec_pretty(&analysis)?;
    write_atomic(&output, &bytes)?;
    let stages: Vec<_> = report
        .stages
        .iter()
        .map(|record| {
            let disposition = match record.disposition {
                StageDisposition::Executed { .. } => "executed",
                StageDisposition::CacheHit { .. } => "cache_hit",
                StageDisposition::RetrySuppressed { .. } => "retry_suppressed",
                StageDisposition::Failed { .. } => "failed",
            };
            serde_json::json!({
                "step": record.step_name,
                "stage": record.stage_id,
                "disposition": disposition
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "article_id": args.article_id,
            "artifact_id": context.artifact_id,
            "route_choice": format!("{:?}", args.route).to_lowercase(),
            "selection_reason": selection_reason,
            "route_id": analysis.provenance.route_id,
            "requested_model": analysis.provenance.requested_model,
            "resolved_model": analysis.provenance.resolved_model,
            "visual_type": analysis.visual_type,
            "component_count": analysis.components.len(),
            "citation_count": analysis.citations.len(),
            "candidate_id": analysis.candidate_id,
            "authoritative": analysis.authoritative,
            "import_eligible": analysis.import_eligible,
            "cache_schema_fingerprint": cache_schema,
            "wall_duration_ms": wall_duration_ms,
            "stages": stages,
            "output": output,
            "output_sha256": sha256(&bytes)
        }))?
    );
    Ok(())
}

async fn select_route(args: &Args) -> Result<(AnthropicMultimodalRoute, String)> {
    match args.route {
        RouteChoice::DirectMinimax => Ok((direct_route(args)?, "explicit_direct_minimax".into())),
        RouteChoice::NineRouter => {
            let model = args
                .model
                .clone()
                .context("--model is required for explicit nine-router route")?;
            Ok((router_route(args, model), "explicit_nine_router".into()))
        }
        RouteChoice::Auto => {
            let endpoint = format!(
                "{}/models/image-to-text",
                args.router_url.trim_end_matches('/')
            );
            let discovered = reqwest::Client::builder()
                .timeout(StdDuration::from_secs(5))
                .build()?
                .get(endpoint)
                .send()
                .await
                .ok()
                .and_then(|response| response.error_for_status().ok());
            if let Some(response) = discovered
                && let Ok(list) = response.json::<ModelList>().await
                && let Some(model) = args
                    .model
                    .clone()
                    .or_else(|| list.data.into_iter().next().map(|entry| entry.id))
            {
                let reason = if args.model.is_some() {
                    "auto_9router_explicit_model_probe"
                } else {
                    "auto_9router_discovered_vision_model"
                };
                return Ok((router_route(args, model), reason.into()));
            }
            Ok((
                direct_route(args)?,
                "auto_fallback_direct_minimax_no_router_vision_model".into(),
            ))
        }
    }
}

fn router_route(args: &Args, model: String) -> AnthropicMultimodalRoute {
    AnthropicMultimodalRoute::nine_router(
        &args.router_url,
        std::env::var("NINE_ROUTER_API_KEY").ok(),
        model,
    )
}

fn direct_route(args: &Args) -> Result<AnthropicMultimodalRoute> {
    let key = std::env::var("MINIMAX_API_KEY")
        .context("MINIMAX_API_KEY is required for direct MiniMax route")?;
    Ok(AnthropicMultimodalRoute::minimax_direct(
        key,
        &args.direct_model,
    ))
}

fn semantic_acceptance(analysis: &FigureAnalysis, context: &FigureSemanticContext) -> Result<()> {
    if analysis.artifact_id != context.artifact_id
        || analysis.visual_type != VisualAnalysisType::Flowchart
        || !contains_any(&analysis.description, &["gepa", "optimization", "flow"])
        || analysis.components.len() < 2
        || analysis.citations.is_empty()
        || analysis.authoritative
        || analysis.import_eligible
    {
        bail!("Figure3 analysis failed semantic acceptance");
    }
    let corpus = analysis
        .components
        .iter()
        .flat_map(|component| [&component.name, &component.role, &component.visual_evidence])
        .chain(analysis.findings.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if !contains_any(
        &corpus,
        &["reflect", "mutation", "candidate", "pareto", "evaluation"],
    ) {
        bail!("Figure3 analysis lacks GEPA process components");
    }
    Ok(())
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_lowercase();
    needles.iter().any(|needle| value.contains(needle))
}

fn media_type(path: &Path) -> Result<ImageMediaType> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(ImageMediaType::Png),
        Some("jpg" | "jpeg") => Ok(ImageMediaType::Jpeg),
        Some("webp") => Ok(ImageMediaType::Webp),
        Some("gif") => Ok(ImageMediaType::Gif),
        other => bail!("unsupported canonical image extension: {other:?}"),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("output has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
