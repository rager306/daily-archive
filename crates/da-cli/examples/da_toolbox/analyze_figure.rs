//! Real provider-neutral cached visual analysis composition (M371 S03).

use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use clap::Parser;
use da_adapters::{
    FilePipelineCache,
    model_registry::{load_model_registry, validate_prompt_model_bindings},
    openai_multimodal_prompt::OpenAiMultimodalPromptAdapter,
    prompt_multimodal_bridge::PromptMultimodalBridge,
    prompt_task_registry::load_prompt_task_registry,
};
use da_application::{
    FigureAnalysisStage, FigureContextArtifactStage, PipelineOrchestrator, StageDisposition,
};
use da_domain::{FigureAnalysis, FigureSemanticContext, VisualAnalysisType};
use da_ports::clock::SystemClock;
use da_ports::multimodal_llm::{ImageMediaType, MultimodalImage, MultimodalLlmPort};
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceFormat, SourceIdentity};
use kg_pipeline::PipelineBuilder;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    context: PathBuf,
    #[arg(long)]
    article_dir: PathBuf,
    #[arg(long)]
    article_id: String,
    #[arg(long)]
    cache_schema_fingerprint: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("analyze_figure");
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
    let prompt_registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let model_registry_path =
        std::env::var("DA_MODEL_REGISTRY").unwrap_or_else(|_| "models.yaml".into());
    let prompt_registry =
        load_prompt_task_registry(Path::new(&prompt_registry_path)).map_err(anyhow::Error::msg)?;
    let model_registry =
        load_model_registry(Path::new(&model_registry_path)).map_err(anyhow::Error::msg)?;
    validate_prompt_model_bindings(&model_registry, &prompt_registry)
        .map_err(anyhow::Error::msg)?;
    let task = prompt_registry
        .task("figure_analysis")
        .map_err(anyhow::Error::msg)?
        .clone();
    let route = prompt_registry
        .route(&task.route_policy_id)
        .map_err(anyhow::Error::msg)?
        .clone();
    let schema = prompt_registry
        .output_schema(&task.output_schema_id)
        .map_err(anyhow::Error::msg)?
        .clone();
    let endpoint_override = route
        .endpoint_override_env
        .as_deref()
        .and_then(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty());
    let resolved_model = model_registry
        .resolve(&route.model_binding_id, endpoint_override.as_deref())
        .map_err(anyhow::Error::msg)?;
    let credential = std::env::var(&route.credential_env).ok();
    let timeout_seconds = std::env::var("DA_LLM_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let prompt_executor = Arc::new(
        OpenAiMultimodalPromptAdapter::new(credential, timeout_seconds)
            .map_err(anyhow::Error::msg)?,
    );
    let bridge = PromptMultimodalBridge::new(
        prompt_executor,
        task.clone(),
        route.clone(),
        resolved_model.clone(),
        schema.clone(),
    )
    .map_err(anyhow::Error::msg)?;
    let route_descriptor = bridge.route();
    let model: Arc<dyn MultimodalLlmPort> = Arc::new(bridge);
    let selection_reason = "models_yaml:figure-qa-primary".to_string();
    let context_stage = Arc::new(FigureContextArtifactStage::new(context.clone())?);
    let image = MultimodalImage {
        media_type: media_type(&image_path)?,
        bytes: image_bytes,
        sha256: image_sha256,
    };
    let retry_after = (Utc::now() + Duration::minutes(10)).to_rfc3339();
    let prompt_registry =
        da_cli::production::load_unified_prompt_registry().map_err(anyhow::Error::msg)?;
    let mut analysis_stage = FigureAnalysisStage::new(model, image, retry_after, prompt_registry)
        .with_context_dependency("analysis_context")
        .with_planned_schema(&context)
        .map_err(anyhow::Error::msg)?;
    let registry_schema = schema
        .sha256()
        .map_err(anyhow::Error::msg)?
        .as_str()
        .to_string();
    let cache_schema = args
        .cache_schema_fingerprint
        .clone()
        .unwrap_or(registry_schema);
    analysis_stage = analysis_stage.with_contract_versions(
        task.spec_sha256()
            .map_err(anyhow::Error::msg)?
            .as_str()
            .to_string(),
        cache_schema.clone(),
    );
    let analysis_stage = Arc::new(analysis_stage);
    let cache_root = args.article_dir.join("pipeline_cache");
    let cache = Arc::new(FilePipelineCache::new(&cache_root));
    let stage_registry =
        da_cli::production::embedded_stage_registry().map_err(anyhow::Error::msg)?;
    let orchestrator =
        PipelineOrchestrator::new(cache.clone(), Arc::new(SystemClock), stage_registry)
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
            "model_binding_id": resolved_model.binding_id,
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

fn semantic_acceptance(analysis: &FigureAnalysis, context: &FigureSemanticContext) -> Result<()> {
    let flowchart_semantics = analysis.visual_type == VisualAnalysisType::Flowchart
        || (analysis.visual_type == VisualAnalysisType::Diagram
            && analysis.description.to_lowercase().contains("flowchart"));
    if analysis.artifact_id != context.artifact_id
        || !flowchart_semantics
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
