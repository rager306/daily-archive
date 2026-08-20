//! Shared infrastructure context for production article commands.
//!
//! This module owns environment, filesystem and concrete adapter defaults. It
//! intentionally does not execute stages or move infrastructure defaults into
//! da-application.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use da_adapters::{
    FigureDetectionStage, FilePipelineCache, GrobidParseStage, GrobidParser, GrobidProfile,
    PdfFigures2Adapter, SamyamaGraphStore,
};
use da_application::{
    FigureAnalysisStage, FigureContextStage, FigureRevisionIdentity, FigureSectionLocator,
    MultimodalComposition, MultimodalFingerprintSummary, PipelineOrchestrator,
};
use da_domain::{PipelineStageLedger, StageIdRegistry};
use da_ports::figure_detection::{FigureDetectionPort, FigureDetectionRequest};
use da_ports::multimodal_llm::{ModelRouteDescriptor, ModelWireProtocol, MultimodalLlmPort};
use da_ports::parser::ParserPort;
use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageContract};
use kg_pipeline::Pipeline;
use sha2::{Digest, Sha256};

const PIPELINE_LEDGER_YAML: &str =
    include_str!("../../../data/ontology/pipeline_stage_contracts.yaml");

pub struct ProductionContext {
    pub store: Arc<SamyamaGraphStore>,
    pub cache: Option<Arc<FilePipelineCache>>,
    pub clock: Arc<dyn da_ports::Clock>,
    pub stage_registry: Arc<StageIdRegistry>,
}

impl ProductionContext {
    pub fn new(
        store: Arc<SamyamaGraphStore>,
        cache_root: Option<&Path>,
        materialization_time: Option<&str>,
    ) -> Result<Self, String> {
        let cache = match cache_root {
            Some(root) => {
                std::fs::create_dir_all(root).map_err(|error| {
                    format!("create pipeline cache {}: {error}", root.display())
                })?;
                Some(Arc::new(FilePipelineCache::new(root)))
            }
            None => None,
        };
        let clock: Arc<dyn da_ports::Clock> =
            match parse_materialization_time(materialization_time)? {
                None => Arc::new(da_ports::SystemClock),
                Some(parsed) => Arc::new(da_ports::FixedClock::from_timestamp(parsed.timestamp())),
            };
        let stage_registry = embedded_stage_registry()?;
        Ok(Self {
            store,
            cache,
            clock,
            stage_registry,
        })
    }

    pub fn from_env(
        cache_root: Option<&Path>,
        materialization_time: Option<&str>,
    ) -> Result<Self, String> {
        let store = Arc::new(
            SamyamaGraphStore::try_from_env()
                .map_err(|error| format!("graph store open failed: {error}"))?,
        );
        Self::new(store, cache_root, materialization_time)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRevision {
    context: da_application::RevisionContentContext,
    method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCatalogPdf {
    pub path: PathBuf,
    pub revision_context: Option<da_application::RevisionContentContext>,
    pub binding_method: &'static str,
}

/// Resolve a catalog PDF for a legacy command. If a bare PDF is byte-identical
/// to one authoritative versioned manifestation, return that exact revision;
/// otherwise return the bare path with an explicit versionless fallback.
pub fn resolve_catalog_pdf(
    catalog_root: &Path,
    paper_id: &str,
) -> Result<Option<ResolvedCatalogPdf>, String> {
    let mut bare = None;
    let mut versioned = Vec::new();
    for entry in walkdir::WalkDir::new(catalog_root)
        .max_depth(12)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".pdf") else {
            continue;
        };
        if stem == paper_id {
            bare = Some(path.to_path_buf());
        } else if let Some(revision) = stem
            .strip_prefix(format!("{paper_id}v").as_str())
            .and_then(|value| value.parse::<u32>().ok())
            && revision > 0
        {
            versioned.push(path.to_path_buf());
        }
    }
    versioned.sort();

    let bare_sha = match &bare {
        Some(path) => {
            let bytes =
                std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
            use sha2::Digest as _;
            Some(hex::encode(sha2::Sha256::digest(bytes)))
        }
        None => None,
    };

    let mut exact = Vec::new();
    for path in &versioned {
        match revision_context_for_versioned_pdf(path, paper_id) {
            Ok(Some(context)) => exact.push((
                path.clone(),
                ResolvedRevision {
                    context,
                    method: "versioned_filename_checksum",
                },
            )),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if let Some(bare_sha) = bare_sha
        && exact.len() > 1
    {
        let matching = exact
            .iter()
            .filter(|(_, binding)| {
                binding
                    .context
                    .manifestations
                    .iter()
                    .any(|manifestation| manifestation.sha256 == bare_sha)
            })
            .collect::<Vec<_>>();
        if matching.len() == 1 {
            let binding = &matching[0].1;
            return Ok(Some(ResolvedCatalogPdf {
                path: bare.expect("bare digest source"),
                revision_context: Some(binding.context.clone()),
                binding_method: binding.method,
            }));
        }
    }
    if exact.len() == 1 {
        let (path, binding) = exact.into_iter().next().expect("one exact binding");
        return Ok(Some(ResolvedCatalogPdf {
            path,
            revision_context: Some(binding.context),
            binding_method: binding.method,
        }));
    }
    if exact.len() > 1 {
        return Err(format!("ambiguous exact catalog revisions for {paper_id}"));
    }
    if let Some(path) = bare {
        return Ok(Some(ResolvedCatalogPdf {
            path,
            revision_context: None,
            binding_method: "versionless_fallback",
        }));
    }
    Ok(None)
}

pub fn parse_materialization_time(
    value: Option<&str>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    value
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map(|time| time.with_timezone(&chrono::Utc))
                .map_err(|error| format!("invalid --materialization-time {value:?}: {error}"))
        })
        .transpose()
}

pub fn embedded_stage_registry() -> Result<Arc<StageIdRegistry>, String> {
    let ledger: PipelineStageLedger = serde_yaml::from_str(PIPELINE_LEDGER_YAML)
        .map_err(|error| format!("parse embedded pipeline ledger: {error}"))?;
    Ok(Arc::new(ledger.registry()?))
}

/// Derive an exact revision context from a versioned local PDF filename.
/// Bare versionless input returns None; this helper never invents revision 1.
pub fn revision_context_for_versioned_pdf(
    pdf_path: &Path,
    paper_id: &str,
) -> Result<Option<da_application::RevisionContentContext>, String> {
    let Some(name) = pdf_path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    let Some(stem) = name.strip_suffix(".pdf") else {
        return Ok(None);
    };
    let Some(revision) = stem
        .strip_prefix(format!("{paper_id}v").as_str())
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return Ok(None);
    };
    if revision == 0 {
        return Err(format!("invalid revision 0 in {}", pdf_path.display()));
    }
    let bytes =
        std::fs::read(pdf_path).map_err(|error| format!("read {}: {error}", pdf_path.display()))?;
    if bytes.is_empty() {
        return Err(format!("empty versioned source {}", pdf_path.display()));
    }
    use sha2::Digest as _;
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    let manifestation = da_application::RevisionManifestationInput {
        format: "pdf".into(),
        sha256,
        bytes: bytes.len() as u64,
        uri: format!("catalog:{}", pdf_path.display()),
        path: pdf_path.display().to_string(),
        capture_status: "captured_local".into(),
    };
    let locator = da_domain::ArticleLocator::Arxiv {
        id: paper_id.to_string(),
        revision: Some(revision),
    };
    let classifier =
        da_application::RevisionClassifyUseCase::bundled().map_err(|error| error.to_string())?;
    classifier
        .revision_context_for_classified_load(&locator, "arxiv", paper_id, vec![manifestation])
        .map_err(|error| error.to_string())
}

// ─── Multimodal production composition (M400 S04 T02) ───────────────────────
//
// The production composition wires the canonical multimodal stages
// (figure_context, figure_analysis, table_fidelity, equation_extraction,
// candidate_materialization) into one orchestrator with exact cache identity:
// source, GROBID profile, detector, schema registry, model route, and output
// schema fingerprints all rotate the stage cache keys. Every setting comes
// from an existing env/config surface (GROBID_*, PDFFIGURES2_JAR,
// DA_PROMPT_TASK_REGISTRY, DA_MODEL_REGISTRY, DA_LLM_TIMEOUT_SECONDS, route
// credential env) — never a hardcoded runtime value. Candidate materialization
// stays candidate-only: D127 remains false everywhere.

/// External settings for the multimodal production composition.
///
/// The parser, detector and model are injected as ports so tests can use
/// deterministic fakes; [`MultimodalProductionSettings::from_env`] resolves
/// them from the configured env surfaces for real production runs.
pub struct MultimodalProductionSettings {
    /// GROBID parser (production: `GrobidParser::from_env()`).
    pub parser: Arc<dyn ParserPort>,
    /// GROBID consolidation profile (production: from the env parser).
    pub grobid_profile: GrobidProfile,
    /// Figure/table detector (production: `PdfFigures2Adapter::from_env()`).
    pub detector: Arc<dyn FigureDetectionPort>,
    /// Multimodal LLM port (production: prompt-task registry + model registry
    /// + OpenAI-compatible adapter bridge).
    pub model: Arc<dyn MultimodalLlmPort>,
    /// Retry-after window for retryable stage failures.
    pub retry_after: String,
    /// Output schema fingerprint for the figure context stage.
    pub context_output_schema: String,
    /// Prompt fingerprint for the figure analysis stage (DATA registry sha).
    pub analysis_prompt_fingerprint: String,
    /// Output schema fingerprint for the figure analysis stage.
    pub analysis_schema_fingerprint: String,
}

impl MultimodalProductionSettings {
    /// Resolve every external setting from the configured env surfaces.
    ///
    /// Fail-closed: a missing detector JAR or an unresolvable model binding
    /// returns an explicit error instead of silently composing a degraded
    /// pipeline (admission stays fail-closed).
    pub fn from_env() -> Result<Self, String> {
        let parser = GrobidParser::from_env();
        let grobid_profile = parser.profile();
        let parser: Arc<dyn ParserPort> = Arc::new(parser);
        let detector: Arc<dyn FigureDetectionPort> =
            Arc::new(PdfFigures2Adapter::from_env().map_err(|error| error.to_string())?);
        let model = resolve_multimodal_model_from_env()?;
        let retry_after = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
        let context_output_schema = std::env::var("DA_MULTIMODAL_CONTEXT_SCHEMA")
            .unwrap_or_else(|_| "FigureSemanticContext@serde-v2-summary-provenance".into());
        Ok(Self {
            parser,
            grobid_profile,
            detector,
            model,
            retry_after,
            context_output_schema,
            analysis_prompt_fingerprint: resolve_analysis_prompt_fingerprint()?,
            analysis_schema_fingerprint: resolve_analysis_schema_fingerprint()?,
        })
    }

    /// Effective-input contract fingerprint of the figure analysis stage
    /// (model route + schema registry + prompt + output schema). The stage
    /// itself is per-image and built at runtime; the contract identity is
    /// stable at composition time.
    pub fn analysis_contract(&self) -> StageContract {
        let route = self.model.route();
        StageContract {
            stage_id: "figure_analysis".into(),
            implementation_fingerprint: "FigureAnalysisUseCase@grounded-v4-all-schema-identities"
                .into(),
            configuration_fingerprint: FigureAnalysisStage::schema_registry_fingerprint(),
            prompt_fingerprint: Some(self.analysis_prompt_fingerprint.clone()),
            model_fingerprint: Some(model_route_fingerprint(&route)),
            output_schema_fingerprint: self.analysis_schema_fingerprint.clone(),
        }
    }
}

/// Article inputs the multimodal production composition binds against.
pub struct MultimodalArticleBinding {
    pub article_id: String,
    pub pdf_path: PathBuf,
    pub canonical_article_dir: PathBuf,
    pub revision_vid: Option<String>,
    pub artifact_id: String,
    pub section_path: String,
}

/// A composed multimodal production run: orchestrator wired with the real
/// parse/detection/context executors, the canonical topology, and the exact
/// fingerprint summary of the effective-input contract.
pub struct ProductionMultimodalComposition {
    pub orchestrator: PipelineOrchestrator,
    pub topology: Pipeline,
    pub composition: MultimodalComposition,
    pub fingerprints: MultimodalFingerprintSummary,
    pub source: SourceIdentity,
}

/// Wire the canonical multimodal production composition for one article.
///
/// Requires a pipeline cache root (cache identity is a hard invariant of the
/// composition: source + profile + detector + schema + model + output schema
/// must be observable). The returned orchestrator carries parse, figure
/// detection and figure context executors with exact contract fingerprints;
/// the figure analysis stage is per-image and built at runtime with
/// [`figure_analysis_stage_for`]. The fingerprint summary binds the analysis
/// contract identity without an image.
///
/// D127 stays false: no executor in the composition writes authoritative graph
/// truth; candidate materialization is candidate-only by DATA contract.
pub fn compose_multimodal_production(
    context: &ProductionContext,
    settings: MultimodalProductionSettings,
    binding: MultimodalArticleBinding,
) -> Result<ProductionMultimodalComposition, String> {
    let cache = context.cache.clone().ok_or_else(|| {
        "multimodal production composition requires a pipeline cache root (cache identity is a hard invariant)"
            .to_string()
    })?;
    let pdf_bytes = std::fs::read(&binding.pdf_path)
        .map_err(|error| format!("read canonical PDF {}: {error}", binding.pdf_path.display()))?;
    let pdf_sha256 = sha256_hex(&pdf_bytes);
    let source = SourceIdentity {
        article_id: binding.article_id.clone(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(pdf_sha256.clone()),
        html_sha256: None,
        acquisition_fingerprint: format!("canonical-article-catalog-v1;pdf={pdf_sha256}"),
        normalization_fingerprint: "grobid-navigation-v3".into(),
    };

    // parse stage — GROBID profile bound into the configuration fingerprint.
    let profile_fp = settings.grobid_profile.configuration_fingerprint();
    let parse_stage = Arc::new(
        GrobidParseStage::new(
            settings.parser.clone(),
            &binding.pdf_path,
            &settings.retry_after,
        )
        .with_contract(StageContract {
            stage_id: "grobid_parse".into(),
            implementation_fingerprint: "GrobidParser@tei-navigation-v3".into(),
            configuration_fingerprint: profile_fp.clone(),
            prompt_fingerprint: None,
            model_fingerprint: Some("grobid/grobid:0.9.0-crf".into()),
            output_schema_fingerprint: "ParsedArticle@serde-v3-unique-navigation".into(),
        }),
    );

    // figure detection stage — detector fingerprint bound as model_fingerprint.
    let figures_stage = Arc::new(FigureDetectionStage::new(
        settings.detector.clone(),
        FigureDetectionRequest {
            article_id: binding.article_id.clone(),
            source_pdf_path: binding.pdf_path.clone(),
            source_pdf_sha256: pdf_sha256.clone(),
            canonical_article_dir: binding.canonical_article_dir.clone(),
            revision_vid: binding.revision_vid.clone(),
        },
        &settings.retry_after,
    ));

    // figure context stage — revision identity + output schema bound.
    let mut context_stage = FigureContextStage::new(
        &binding.artifact_id,
        FigureSectionLocator::PathId(binding.section_path.clone()),
        None,
    )
    .with_output_schema_fingerprint(&settings.context_output_schema);
    if let Some(revision_vid) = &binding.revision_vid {
        context_stage = context_stage
            .with_revision_identity(FigureRevisionIdentity {
                revision_vid: revision_vid.clone(),
                canonical_id: binding.article_id.clone(),
            })
            .with_output_schema_fingerprint(&settings.context_output_schema);
    }
    let context_stage = Arc::new(context_stage);

    // Fingerprint summary: bind the live stage contracts + analysis identity.
    let mut contracts = BTreeMap::new();
    contracts.insert("grobid_parse".to_string(), parse_stage.contract());
    contracts.insert("figure_detect".to_string(), figures_stage.contract());
    contracts.insert("figure_context".to_string(), context_stage.contract());
    contracts.insert("figure_analysis".to_string(), settings.analysis_contract());

    let composition =
        MultimodalComposition::canonical(&context.stage_registry, "production-multimodal-v1")
            .map_err(|error| format!("canonical multimodal topology: {error}"))?;
    let orchestrator =
        PipelineOrchestrator::new(cache, context.clock.clone(), context.stage_registry.clone())
            .with_executor("grobid_parse", parse_stage)
            .map_err(|error| format!("register parse executor: {error}"))?
            .with_executor("figure_detect", figures_stage)
            .map_err(|error| format!("register figure_detect executor: {error}"))?
            .with_executor("figure_context", context_stage)
            .map_err(|error| format!("register figure_context executor: {error}"))?;
    let fingerprints = MultimodalFingerprintSummary::bind(&source, &profile_fp, &contracts);

    Ok(ProductionMultimodalComposition {
        orchestrator,
        topology: composition.topology.clone(),
        composition,
        fingerprints,
        source,
    })
}

/// Build the per-image figure analysis executor for a detected artifact.
///
/// The stage binds the model route, prompt, schema registry and output schema
/// fingerprints from the composition settings; the caller supplies the
/// rendered image from the detection result. The returned stage is registered
/// against the `analysis` topology step.
pub fn figure_analysis_stage_for(
    settings: &MultimodalProductionSettings,
    image: da_ports::multimodal_llm::MultimodalImage,
) -> FigureAnalysisStage {
    FigureAnalysisStage::new(settings.model.clone(), image, &settings.retry_after)
        .with_context_dependency("context")
        .with_contract_versions(
            &settings.analysis_prompt_fingerprint,
            &settings.analysis_schema_fingerprint,
        )
}

fn model_route_fingerprint(route: &ModelRouteDescriptor) -> String {
    format!(
        "{}|{}|{}|fallback={}",
        route.route_id,
        protocol_name(route.protocol),
        route.model,
        route.may_fallback
    )
}

fn protocol_name(protocol: ModelWireProtocol) -> &'static str {
    match protocol {
        ModelWireProtocol::AnthropicMessages => "anthropic_messages",
        ModelWireProtocol::OpenAiChatCompletions => "openai_chat_completions",
        ModelWireProtocol::OpenAiResponses => "openai_responses",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Optional `DA_FIGURE_MODEL_BINDING` override of the DATA route binding.
/// Empty/unset keeps the ledger binding (`figure-qa-primary` today).
/// Unknown ids fail later at `ModelRegistry::resolve` — never silently
/// fall back to a different model.
fn figure_model_binding_id_from(data_binding: &str, override_binding: Option<&str>) -> String {
    match override_binding
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => value.to_string(),
        None => data_binding.to_string(),
    }
}

fn figure_model_binding_id(data_binding: &str) -> String {
    figure_model_binding_id_from(
        data_binding,
        std::env::var("DA_FIGURE_MODEL_BINDING").ok().as_deref(),
    )
}

/// Resolve the multimodal LLM port from the DATA prompt-task registry, the
/// model registry and the route credential/endpoint env surfaces
/// (mirrors `analyze_figure.rs`).
fn resolve_multimodal_model_from_env() -> Result<Arc<dyn MultimodalLlmPort>, String> {
    use da_adapters::{
        model_registry::load_model_registry,
        openai_multimodal_prompt::OpenAiMultimodalPromptAdapter,
        prompt_multimodal_bridge::PromptMultimodalBridge,
        prompt_task_registry::load_prompt_task_registry,
    };
    let prompt_registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let model_registry_path =
        std::env::var("DA_MODEL_REGISTRY").unwrap_or_else(|_| "models.yaml".into());
    let prompt_registry = load_prompt_task_registry(Path::new(&prompt_registry_path))
        .map_err(|error| error.to_string())?;
    let model_registry =
        load_model_registry(Path::new(&model_registry_path)).map_err(|error| error.to_string())?;
    da_adapters::model_registry::validate_prompt_model_bindings(&model_registry, &prompt_registry)
        .map_err(|error| error.to_string())?;
    let task = prompt_registry
        .task("figure_analysis")
        .map_err(|error| error.to_string())?
        .clone();
    let route = prompt_registry
        .route(&task.route_policy_id)
        .map_err(|error| error.to_string())?
        .clone();
    let schema = prompt_registry
        .output_schema(&task.output_schema_id)
        .map_err(|error| error.to_string())?
        .clone();
    let endpoint_override = route
        .endpoint_override_env
        .as_deref()
        .and_then(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty());
    let binding_id = figure_model_binding_id(&route.model_binding_id);
    let resolved_model = model_registry
        .resolve(&binding_id, endpoint_override.as_deref())
        .map_err(|error| error.to_string())?;
    // Bridge requires route.model_binding_id == resolved.binding_id.
    // An env override selects another DATA binding; keep the route policy
    // id/credentials and retarget only the binding field.
    let mut route = route;
    route.model_binding_id = binding_id;
    let credential = std::env::var(&route.credential_env).ok();
    let timeout_seconds = std::env::var("DA_LLM_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let prompt_executor = Arc::new(
        OpenAiMultimodalPromptAdapter::new(credential, timeout_seconds)
            .map_err(|error| error.to_string())?,
    );
    let bridge = PromptMultimodalBridge::new(
        prompt_executor,
        task.clone(),
        route.clone(),
        resolved_model.clone(),
        schema.clone(),
    )
    .map_err(|error| error.to_string())?;
    Ok(Arc::new(bridge))
}

/// Resolve the figure analysis prompt fingerprint from the DATA registry
/// (env surface `DA_PROMPT_TASK_REGISTRY`).
fn resolve_analysis_prompt_fingerprint() -> Result<String, String> {
    use da_adapters::prompt_task_registry::load_prompt_task_registry;
    let prompt_registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let prompt_registry = load_prompt_task_registry(Path::new(&prompt_registry_path))
        .map_err(|error| error.to_string())?;
    let task = prompt_registry
        .task("figure_analysis")
        .map_err(|error| error.to_string())?;
    Ok(task
        .spec_sha256()
        .map_err(|error| error.to_string())?
        .as_str()
        .to_string())
}

/// Resolve the figure analysis output schema fingerprint from the DATA
/// registry (env surface `DA_PROMPT_TASK_REGISTRY`).
fn resolve_analysis_schema_fingerprint() -> Result<String, String> {
    use da_adapters::prompt_task_registry::load_prompt_task_registry;
    let prompt_registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let prompt_registry = load_prompt_task_registry(Path::new(&prompt_registry_path))
        .map_err(|error| error.to_string())?;
    let task = prompt_registry
        .task("figure_analysis")
        .map_err(|error| error.to_string())?;
    let schema = prompt_registry
        .output_schema(&task.output_schema_id)
        .map_err(|error| error.to_string())?;
    Ok(schema
        .sha256()
        .map_err(|error| error.to_string())?
        .as_str()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_binding_override_empty_keeps_data_route() {
        assert_eq!(
            super::figure_model_binding_id_from("figure-qa-primary", None),
            "figure-qa-primary"
        );
        assert_eq!(
            super::figure_model_binding_id_from("figure-qa-primary", Some("")),
            "figure-qa-primary"
        );
        assert_eq!(
            super::figure_model_binding_id_from("figure-qa-primary", Some("  ")),
            "figure-qa-primary"
        );
    }

    #[test]
    fn figure_binding_override_selects_luna_without_changing_data() {
        assert_eq!(
            super::figure_model_binding_id_from("figure-qa-primary", Some("figure-qa-luna")),
            "figure-qa-luna"
        );
    }

    #[test]
    fn context_loads_registry_cache_and_fixed_clock() {
        let root =
            std::env::temp_dir().join(format!("da-production-context-{}", std::process::id()));
        let context = ProductionContext::new(
            Arc::new(SamyamaGraphStore::new()),
            Some(&root),
            Some("2023-11-14T22:13:20Z"),
        )
        .unwrap();
        assert_eq!(
            context.stage_registry.resolve("embed").unwrap().as_str(),
            "embedding"
        );
        assert_eq!(
            context.clock.now(),
            chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap()
        );
        assert_eq!(context.cache.as_ref().unwrap().root(), root.as_path());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_resolver_prefers_exact_version_for_identical_bare_bytes() {
        let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/article_catalog");
        let resolved = resolve_catalog_pdf(&catalog, "2506.07853")
            .unwrap()
            .expect("catalog paper");
        let context = resolved.revision_context.expect("exact v5 context");
        assert_eq!(context.revision, 5);
        assert_eq!(resolved.binding_method, "versioned_filename_checksum");
        assert!(
            resolved
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "2506.07853.pdf")
                || resolved
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "2506.07853v5.pdf")
        );
    }

    #[test]
    fn production_command_stage_inventory_is_canonical() {
        let registry = embedded_stage_registry().unwrap();
        let canonical = ["parse", "embedding", "extraction_compute", "summary"];
        for stage in canonical {
            registry.parse_canonical(stage).unwrap();
        }
        assert_eq!(registry.resolve("embed").unwrap().as_str(), "embedding");
        assert_eq!(
            registry.resolve("extract").unwrap().as_str(),
            "extraction_compute"
        );
        assert!(registry.parse_canonical("semantic_enrichment").is_err());
    }

    #[test]
    fn versioned_pdf_adapter_binds_exact_and_never_invents_v1() {
        let root =
            std::env::temp_dir().join(format!("da-production-revision-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let versioned = root.join("2506.07853v5.pdf");
        std::fs::write(&versioned, b"version-five").unwrap();
        let context = revision_context_for_versioned_pdf(&versioned, "2506.07853")
            .unwrap()
            .expect("exact context");
        assert_eq!(context.revision, 5);
        assert_eq!(context.manifestations.len(), 1);
        assert_eq!(context.manifestations[0].bytes, 12);

        let bare = root.join("2506.07853.pdf");
        std::fs::write(&bare, b"version-five").unwrap();
        assert!(
            revision_context_for_versioned_pdf(&bare, "2506.07853")
                .unwrap()
                .is_none()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_materialization_time_fails_before_context_creation() {
        let error =
            ProductionContext::new(Arc::new(SamyamaGraphStore::new()), None, Some("not-a-time"))
                .err()
                .unwrap();
        assert!(error.contains("invalid --materialization-time"), "{error}");
    }

    // ── Multimodal production composition (M400 S04 T02) ─────────────────────

    struct FakeParser;

    #[async_trait::async_trait]
    impl ParserPort for FakeParser {
        async fn parse_pdf(
            &self,
            _pdf_path: &str,
            _paper_id: &str,
        ) -> da_ports::parser::ParseResult<da_ports::parser::ParsedArticle> {
            Err(da_ports::parser::ParserError::Unavailable(
                "fake parser".into(),
            ))
        }
    }

    struct FakeDetector {
        fingerprint: String,
    }

    #[async_trait::async_trait]
    impl FigureDetectionPort for FakeDetector {
        fn detector_fingerprint(&self) -> &str {
            &self.fingerprint
        }

        async fn detect(
            &self,
            _request: FigureDetectionRequest,
        ) -> da_ports::figure_detection::FigureDetectionPortResult<
            da_ports::figure_detection::FigureDetectionResult,
        > {
            Err(
                da_ports::figure_detection::FigureDetectionError::Unavailable(
                    "fake detector".into(),
                ),
            )
        }
    }

    struct FakeModel {
        route: ModelRouteDescriptor,
    }

    #[async_trait::async_trait]
    impl MultimodalLlmPort for FakeModel {
        fn route(&self) -> ModelRouteDescriptor {
            self.route.clone()
        }

        async fn complete_structured(
            &self,
            _request: da_ports::multimodal_llm::MultimodalLlmRequest,
        ) -> da_ports::multimodal_llm::MultimodalLlmResult<
            da_ports::multimodal_llm::MultimodalLlmResponse,
        > {
            Err(da_ports::multimodal_llm::MultimodalLlmError::Unavailable(
                "fake model".into(),
            ))
        }
    }

    fn fake_settings() -> MultimodalProductionSettings {
        MultimodalProductionSettings {
            parser: Arc::new(FakeParser),
            grobid_profile: GrobidProfile::structural(),
            detector: Arc::new(FakeDetector {
                fingerprint: "pdffigures2-fake-0.0.12".into(),
            }),
            model: Arc::new(FakeModel {
                route: ModelRouteDescriptor {
                    route_id: "fixture-route".into(),
                    protocol: ModelWireProtocol::AnthropicMessages,
                    model: "vision-fixture".into(),
                    endpoint_fingerprint: "fixture-endpoint".into(),
                    supports_images: true,
                    supports_structured_output: true,
                    may_fallback: false,
                },
            }),
            retry_after: "2026-08-17T01:00:00Z".into(),
            context_output_schema: "FigureSemanticContext@serde-v2-summary-provenance".into(),
            analysis_prompt_fingerprint: "prompt-fixture-sha".into(),
            analysis_schema_fingerprint: "schema-fixture-sha".into(),
        }
    }

    fn fake_binding() -> MultimodalArticleBinding {
        MultimodalArticleBinding {
            article_id: "2507.19457".into(),
            pdf_path: PathBuf::from("/tmp/2507.19457.pdf"),
            canonical_article_dir: PathBuf::from("/tmp/article"),
            revision_vid: Some("a".repeat(64)),
            artifact_id: "Figure3".into(),
            section_path: "3".into(),
        }
    }

    #[test]
    fn multimodal_composition_binds_exact_fingerprints() {
        let root =
            std::env::temp_dir().join(format!("da-multimodal-composition-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let pdf = root.join("2507.19457.pdf");
        std::fs::write(&pdf, b"canonical-pdf-bytes").unwrap();
        let context =
            ProductionContext::new(Arc::new(SamyamaGraphStore::new()), Some(&root), None).unwrap();
        let mut binding = fake_binding();
        binding.pdf_path = pdf.clone();
        binding.canonical_article_dir = root.clone();
        let composed = compose_multimodal_production(&context, fake_settings(), binding).unwrap();

        // Canonical stage inventory validated against the DATA registry.
        assert_eq!(
            composed.composition.canonical_stages,
            vec![
                "parse",
                "figure_context",
                "figure_analysis",
                "table_fidelity",
                "equation_extraction",
                "candidate_materialization",
            ]
        );
        // Exact source fingerprint from the canonical PDF bytes.
        let pdf_sha = sha256_hex(b"canonical-pdf-bytes");
        assert!(composed.fingerprints.source.contains(&pdf_sha));
        assert!(composed.source.pdf_sha256.as_deref() == Some(pdf_sha.as_str()));
        // GROBID profile fingerprint bound into the parse contract.
        assert!(
            composed
                .fingerprints
                .grobid_profile
                .contains("profile=structural")
        );
        // Detector fingerprint bound as the figure_detect model fingerprint.
        assert_eq!(composed.fingerprints.detector, "pdffigures2-fake-0.0.12");
        // Schema registry + model route bound from the analysis contract.
        assert!(
            composed
                .fingerprints
                .schema_registry
                .contains("schema_registry=")
        );
        assert!(composed.fingerprints.model_route.contains("fixture-route"));
        assert!(
            composed
                .fingerprints
                .model_route
                .contains("anthropic_messages")
        );
        // Output schema fingerprints per canonical stage.
        assert_eq!(
            composed.fingerprints.output_schemas["figure_context"],
            "FigureSemanticContext@serde-v2-summary-provenance"
        );
        assert_eq!(
            composed.fingerprints.output_schemas["figure_analysis"],
            "schema-fixture-sha"
        );
        // D127 stays false.
        assert!(!composed.fingerprints.import_eligible);
        // The orchestrator carries the three wired executors.
        assert_eq!(composed.topology.steps.len(), 7);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn multimodal_composition_fails_closed_without_cache_root() {
        let context =
            ProductionContext::new(Arc::new(SamyamaGraphStore::new()), None, None).unwrap();
        let error = match compose_multimodal_production(&context, fake_settings(), fake_binding()) {
            Err(error) => error,
            Ok(_) => panic!("multimodal composition must fail closed without a cache root"),
        };
        assert!(error.contains("cache root"), "{error}");
    }

    #[test]
    fn analysis_contract_binds_route_schema_and_prompt() {
        let contract = fake_settings().analysis_contract();
        assert_eq!(contract.stage_id, "figure_analysis");
        assert_eq!(
            contract.prompt_fingerprint.as_deref(),
            Some("prompt-fixture-sha")
        );
        assert_eq!(contract.output_schema_fingerprint, "schema-fixture-sha");
        assert!(
            contract
                .model_fingerprint
                .as_deref()
                .unwrap()
                .contains("fixture-route")
        );
        assert!(
            contract
                .configuration_fingerprint
                .contains("schema_registry=")
        );
    }

    #[test]
    fn figure_analysis_stage_binds_composition_fingerprints() {
        let image = da_ports::multimodal_llm::MultimodalImage {
            media_type: da_ports::multimodal_llm::ImageMediaType::Png,
            sha256: "b".repeat(64),
            bytes: b"png".to_vec(),
        };
        let stage = figure_analysis_stage_for(&fake_settings(), image);
        let contract = stage.contract();
        assert!(contract.prompt_fingerprint.as_deref() == Some("prompt-fixture-sha"));
        assert_eq!(contract.output_schema_fingerprint, "schema-fixture-sha");
        assert!(
            contract
                .model_fingerprint
                .as_deref()
                .unwrap()
                .contains("fixture-route")
        );
    }
}
