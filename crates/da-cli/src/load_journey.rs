//! Journey content pipeline for `da load --journey` (M404 S01).
//!
//! Composes the canonical article journey (M403 `ArticleJourneyComposition`)
//! with the REAL production executors behind injectable ports:
//!
//! - `parse` — `GrobidParseStage` under the **load-v1 fingerprint contract**
//!   (`da_adapters::GrobidParser@load-v1` + profile configuration +
//!   `ParsedArticle.v1` output schema), so the operator's existing
//!   content-addressed parse cache keeps hitting. This mirrors
//!   `load::parse_cache_key` exactly.
//! - `revision_materialization` / `extraction_materialization` — the M403
//!   executors (always-run, idempotent; the orchestrator never accepts a
//!   cache hit as a skip for `graph_materialize` stages).
//! - `extraction_compute` — `RuleExtractionStage` over the bundled rule
//!   extractor (new cache namespace; the legacy inline path did not cache).
//! - `embedding` — `ParsedArticleEmbeddingStage` (new cache namespace: the
//!   stage emits serde JSON, not the legacy le-bytes vector).
//!
//! Stage contracts are snapshotted before the executors are handed to the
//! orchestrator, so the exact cache keys can be reconstructed afterwards to
//! recover payloads from the cache — the orchestrator stays the single
//! writer, this module only observes.
//!
//! After the orchestrated run, the embedding vector is attached to the Paper
//! node behind the SAME `embedding_sha256` idempotence guard as the legacy
//! ingest materializer (`da_application::embedding_fingerprint`): identical
//! sha is a no-op, a different sha fails closed.
//!
//! D127 stays false: nothing here writes authoritative graph truth.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use da_adapters::{
    FdApiEmbedder, FigureDetectionStage, GrobidParseStage, GrobidParser, GrobidProfile,
    ParsedArticleEmbeddingStage, PdfFigures2Adapter, RuleExtractionStage, SamyamaGraphStore,
};
use da_application::candidate_materialization_stage::CandidateMaterializationStage;
use da_application::equation_extraction_stage::EquationExtractionStage;
use da_application::extraction_materialization_stage::ExtractionMaterializationReport;
use da_application::extraction_materialization_stage::ExtractionMaterializationStage;
use da_application::figure_analysis_stage::FigureAnalysisStage;
use da_application::figure_candidate_assembly::assemble_from_context;
use da_application::figure_candidate_assembly::locate_figure_section;
use da_application::figure_context::FigureRevisionIdentity;
use da_application::figure_context::FigureSectionLocator;
use da_application::figure_context_stage::FigureContextStage;
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::revision_materialization_stage::RevisionMaterializationReport;
use da_application::revision_materialization_stage::RevisionMaterializationStage;
use da_application::table_extraction_stage::TableExtractionStage;
use da_application::{
    ArticleJourneyComposition, LoadContentOutcome, LoadExtractOutcome, LoadIngestOutcome,
    PipelineOrchestrator, PipelineRunReport, StageDisposition, embedding_fingerprint,
};
use da_domain::figure_analysis::FigureQuotePolicy;
use da_domain::{LoadCacheRecord, StageCacheMode, StageIdRegistry};
use da_ports::clock::Clock;
use da_ports::embedder::Embedder;
use da_ports::extractor::Extractor;
use da_ports::figure_detection::{FigureDetectionPort, FigureDetectionRequest};
use da_ports::graph_store::DirectGraphStore;
use da_ports::multimodal_llm::{ImageMediaType, MultimodalImage, MultimodalLlmPort};
use da_ports::parser::{ParsedArticle, ParserPort};
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceIdentity, StageCacheKey};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageContract, StageExecutionRequest};
use kg_pipeline::PipelineBuilder;

/// Figure branch settings resolved from the DATA prompt/model registries.
#[derive(Clone)]
pub struct FigureBranchSettings {
    pub model: Arc<dyn MultimodalLlmPort>,
    pub prompt_fingerprint: String,
    pub schema_fingerprint: String,
    pub context_schema: String,
}

/// Injectable production ports for the journey composition.
pub struct JourneyPorts {
    pub parser: Arc<dyn ParserPort>,
    pub extractor: Arc<dyn Extractor>,
    pub embedder: Arc<dyn Embedder>,
    /// Optional visual detector (PDFFIGURES2_JAR surface). `None` keeps the
    /// table branch skipped with a typed reason.
    pub detector: Option<Arc<dyn FigureDetectionPort>>,
}

impl JourneyPorts {
    /// Resolve every port from the existing env/config surfaces. No new env
    /// keys: GROBID_*, FD_API_* and PDFFIGURES2_JAR are reused exactly as
    /// the legacy load path resolves them.
    pub fn from_env() -> Self {
        Self {
            parser: Arc::new(GrobidParser::from_env()),
            extractor: Arc::new(extractor_from_data()),
            embedder: Arc::new(FdApiEmbedder::from_env()),
            detector: PdfFigures2Adapter::from_env()
                .ok()
                .map(|adapter| Arc::new(adapter) as Arc<dyn FigureDetectionPort>),
        }
    }
}

/// Load-compatible parse contract (mirrors `load::parse_cache_key` for PDF).
pub fn load_parse_contract(profile: &GrobidProfile) -> StageContract {
    StageContract {
        stage_id: "parse".into(),
        implementation_fingerprint: "da_adapters::GrobidParser@load-v1".into(),
        configuration_fingerprint: profile.configuration_fingerprint(),
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: "ParsedArticle.v1".into(),
    }
}

/// Reconstruct the orchestrator's exact cache key for one journey step from
/// dependency output digests and the snapshotted executor contract.
fn journey_cache_key(
    source: &SourceIdentity,
    stage_id: &str,
    dependency_output_sha256: BTreeMap<String, String>,
    contract: &StageContract,
) -> StageCacheKey {
    StageCacheKey {
        source: source.clone(),
        stage_id: stage_id.into(),
        dependency_output_sha256,
        implementation_fingerprint: contract.implementation_fingerprint.clone(),
        configuration_fingerprint: contract.configuration_fingerprint.clone(),
        prompt_fingerprint: contract.prompt_fingerprint.clone(),
        model_fingerprint: contract.model_fingerprint.clone(),
        output_schema_fingerprint: contract.output_schema_fingerprint.clone(),
    }
}

/// Read a stage payload back from the cache by reconstructing its key.
fn cached_payload(
    cache: &Arc<dyn PipelineCache>,
    now: &str,
    key: &StageCacheKey,
) -> Result<Vec<u8>, String> {
    match cache.lookup(key, now).map_err(|error| error.to_string())? {
        CacheLookup::Hit(hit) => Ok(hit.payload),
        other => Err(format!(
            "journey payload for stage {} is not cached ({other:?})",
            key.stage_id
        )),
    }
}

fn output_sha_of(report: &PipelineRunReport, step_name: &str) -> Result<String, String> {
    report
        .stages
        .iter()
        .find(|record| record.step_name == step_name)
        .and_then(|record| match &record.disposition {
            StageDisposition::Executed { output_sha256 } => Some(output_sha256.clone()),
            StageDisposition::CacheHit { output_sha256 } => Some(output_sha256.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("journey step {step_name} has no successful output digest"))
}

/// Run the canonical article journey for one revision-bound PDF source.
///
/// `revision` / `manifestations` must come from the load plane's revision
/// classification (the caller fails closed on versionless sources). Returns
/// the load outcome plus the orchestrator run report for the operator
/// surface.
/// Resolve the figure branch settings from the DATA prompt and model
/// registries plus the route env surfaces (same resolver the production
/// multimodal composition uses). `None` means the route is unconfigured.
pub fn resolve_figure_branch_from_env() -> Option<FigureBranchSettings> {
    let settings = match crate::production::MultimodalProductionSettings::from_env() {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(
                error = %error,
                event = "da_load_journey_figures_route_unresolved"
            );
            return None;
        }
    };
    Some(FigureBranchSettings {
        model: settings.model.clone(),
        prompt_fingerprint: settings.analysis_prompt_fingerprint.clone(),
        schema_fingerprint: settings.analysis_schema_fingerprint.clone(),
        context_schema: settings.context_output_schema.clone(),
    })
}

/// Load the figure quote policy window (ADR-062). `DA_FIGURE_QUOTE_POLICY`
/// overrides the path; default `data/ontology/figure_quote_policy.yaml`.
pub fn load_figure_quote_policy_from_env() -> Result<FigureQuotePolicy, String> {
    let path = std::env::var("DA_FIGURE_QUOTE_POLICY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "data/ontology/figure_quote_policy.yaml".to_string());
    da_adapters::load_figure_quote_policy(std::path::Path::new(&path))
}

#[allow(clippy::too_many_arguments)]
pub async fn run_journey_content(
    ports: &JourneyPorts,
    store: &Arc<SamyamaGraphStore>,
    cache: Arc<dyn PipelineCache>,
    clock: Arc<dyn Clock>,
    stage_registry: Arc<StageIdRegistry>,
    paper_id: &str,
    pdf_path: &Path,
    source: SourceIdentity,
    source_code: &str,
    revision: u32,
    manifestations: Vec<RevisionManifestationInput>,
    with_equations: bool,
    with_tables: bool,
    with_figures: bool,
    figure_branch: Option<FigureBranchSettings>,
) -> Result<(LoadContentOutcome, PipelineRunReport), String> {
    let journey = if with_figures {
        ArticleJourneyComposition::with_figures(&stage_registry, "da-load-journey")
    } else if with_tables {
        ArticleJourneyComposition::with_tables(&stage_registry, "da-load-journey")
    } else if with_equations {
        ArticleJourneyComposition::with_equations(&stage_registry, "da-load-journey")
    } else {
        ArticleJourneyComposition::canonical(&stage_registry, "da-load-journey")
    }
    .map_err(|error| error.to_string())?;

    let retry_after = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    let grobid_profile = ports_profile();
    let parse_stage = Arc::new(
        GrobidParseStage::new(
            ports.parser.clone(),
            pdf_path.display().to_string(),
            &retry_after,
        )
        .with_contract(load_parse_contract(&grobid_profile)),
    );
    let extract_stage = Arc::new(RuleExtractionStage::new(
        ports.extractor.clone(),
        "parse",
        "rules.phrases@bundled-v1",
    ));
    let embed_stage = Arc::new(ParsedArticleEmbeddingStage::new(
        ports.embedder.clone(),
        "parse",
        &retry_after,
    ));
    let revision_stage = Arc::new(RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock.clone(),
        source_code,
        paper_id,
        revision,
        manifestations,
    ));
    let extraction_stage = Arc::new(ExtractionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>
    ));

    let revision_vid = da_domain::paper_revision_vid(source_code, paper_id, revision)
        .map_err(|error| format!("journey revision vid: {error}"))?
        .to_string();
    let source_pdf_sha = source.pdf_sha256.clone().unwrap_or_default();
    let equation_stage = Arc::new(EquationExtractionStage::new(
        revision_vid.clone(),
        source_pdf_sha.clone(),
    ));
    let table_stage = Arc::new(TableExtractionStage::new(
        revision_vid.clone(),
        source_pdf_sha.clone(),
    ));
    let candidate_stage = Arc::new(CandidateMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>
    ));

    // Snapshot contracts BEFORE moving executors into the orchestrator.
    let parse_contract = parse_stage.contract();
    let embed_contract = embed_stage.contract();
    let revision_contract = revision_stage.contract();
    let extraction_contract = extraction_stage.contract();
    let equation_contract = equation_stage.contract();
    let table_contract = table_stage.contract();
    let candidate_contract = candidate_stage.contract();

    let mut orchestrator =
        PipelineOrchestrator::new(cache.clone(), clock.clone(), stage_registry.clone())
            .with_executor("parse", parse_stage.clone())
            .map_err(|error| error.to_string())?
            .with_executor("revision_materialization", revision_stage.clone())
            .map_err(|error| error.to_string())?
            .with_executor("extraction_compute", extract_stage.clone())
            .map_err(|error| error.to_string())?
            .with_executor("extraction_materialization", extraction_stage.clone())
            .map_err(|error| error.to_string())?
            .with_executor("embedding", embed_stage.clone())
            .map_err(|error| error.to_string())?;
    let mut detection_stage_arc: Option<Arc<da_adapters::FigureDetectionStage>> = None;
    if with_tables || with_figures {
        let Some(detector) = ports.detector.clone() else {
            return Err("branch requires a configured detector".into());
        };
        let figures_stage = Arc::new(FigureDetectionStage::new(
            detector,
            FigureDetectionRequest {
                article_id: paper_id.to_string(),
                source_pdf_path: pdf_path.to_path_buf(),
                source_pdf_sha256: source_pdf_sha.clone(),
                canonical_article_dir: pdf_path.parent().map(Path::to_path_buf).unwrap_or_default(),
                revision_vid: Some(revision_vid.clone()),
            },
            &retry_after,
        ));
        orchestrator = orchestrator
            .with_executor("figure_detect", figures_stage.clone())
            .map_err(|error| error.to_string())?;
        detection_stage_arc = Some(figures_stage);
    }
    if with_tables || with_figures {
        orchestrator = orchestrator
            .with_executor("table_fidelity", table_stage.clone())
            .map_err(|error| error.to_string())?;
    }
    if with_equations || with_tables || with_figures {
        orchestrator = orchestrator
            .with_executor("equation_extraction", equation_stage.clone())
            .map_err(|error| error.to_string())?;
    }
    if with_equations || with_tables {
        // The figures branch materializes candidates in the driver with all
        // three family payloads together.
        orchestrator = orchestrator
            .with_executor("candidate_materialization", candidate_stage.clone())
            .map_err(|error| error.to_string())?;
    }

    let report = orchestrator
        .run(&journey.topology, source.clone())
        .await
        .map_err(|error| error.to_string())?;
    if !report.completed {
        let failed = report
            .stages
            .iter()
            .rev()
            .find(|record| matches!(record.disposition, StageDisposition::Failed { .. }))
            .map(|record| {
                format!(
                    "journey stage {} failed: {:?}",
                    record.stage_id, record.disposition
                )
            })
            .unwrap_or_else(|| "journey did not complete".into());
        return Err(failed);
    }

    // ── Recover payloads by reconstructing the exact orchestrator keys ──────
    let now = clock.now().to_rfc3339();
    let parse_sha = output_sha_of(&report, "parse")?;
    let revision_sha = output_sha_of(&report, "revision")?;
    let extract_sha = output_sha_of(&report, "extract")?;
    let parse_payload = cached_payload(
        &cache,
        &now,
        &journey_cache_key(&source, "parse", BTreeMap::new(), &parse_contract),
    )?;
    let parsed: ParsedArticle = serde_json::from_slice(&parse_payload)
        .map_err(|error| format!("journey parse payload decode: {error}"))?;

    let revision_payload = cached_payload(
        &cache,
        &now,
        &journey_cache_key(
            &source,
            "revision_materialization",
            BTreeMap::from([("parse".to_string(), parse_sha.clone())]),
            &revision_contract,
        ),
    )?;
    let revision_report: RevisionMaterializationReport = serde_json::from_slice(&revision_payload)
        .map_err(|error| format!("journey revision report decode: {error}"))?;

    let extraction_payload = cached_payload(
        &cache,
        &now,
        &journey_cache_key(
            &source,
            "extraction_materialization",
            BTreeMap::from([
                ("revision".to_string(), revision_sha),
                ("extract".to_string(), extract_sha),
                ("parse".to_string(), parse_sha.clone()),
            ]),
            &extraction_contract,
        ),
    )?;
    let extraction_report: ExtractionMaterializationReport =
        serde_json::from_slice(&extraction_payload)
            .map_err(|error| format!("journey extraction report decode: {error}"))?;

    let embed_payload = cached_payload(
        &cache,
        &now,
        &journey_cache_key(
            &source,
            "embedding",
            BTreeMap::from([("parse".to_string(), parse_sha.clone())]),
            &embed_contract,
        ),
    )?;
    let vector: Vec<f32> = serde_json::from_slice(&embed_payload)
        .map_err(|error| format!("journey embedding payload decode: {error}"))?;

    // ── Vector attach parity: the legacy embedding_sha256 guard ────────────
    let embedding_sha = embedding_fingerprint(&vector);
    let paper_node = store
        .find_node_by_string_property("Paper", "arxiv_id", paper_id)
        .await
        .ok_or_else(|| format!("journey: Paper node missing for {paper_id}"))?;
    match store
        .get_node_property_string(paper_node, "embedding_sha256")
        .await
    {
        Some(existing) if existing == embedding_sha => {}
        Some(_existing) => {
            // Sanctioned replacement seam (ADR-037 / D277): a changed embedder
            // yields a different embedding_sha. Enqueue an idempotent EmbedStale
            // task so the scheduler re-embeds and applies replace_vector on the
            // same node; never hard-fail the journey.
            let now = chrono::Utc::now().timestamp();
            if let Err(error) = da_application::GraphScheduler::add_pending_embed_stale_to(
                store.as_ref(),
                &da_domain::scheduler::RetryPolicy::default(),
                paper_id,
                now,
            )
            .await
            {
                tracing::warn!(
                    article_id = paper_id,
                    error = %error,
                    event = "scheduler_embed_stale_enqueue_failed"
                );
            } else {
                tracing::info!(article_id = paper_id, event = "embed_stale_enqueued");
            }
        }
        None => {
            store
                .add_vector("Paper", "embedding", paper_node, vector)
                .await
                .map_err(|error| format!("journey vector attach: {error}"))?;
            store
                .set_node_property_string(paper_node, "embedding_sha256", embedding_sha)
                .await
                .map_err(|error| format!("journey vector sha stamp: {error}"))?;
        }
    }

    // ── Family payloads (equations and tables) for the candidate stage ─────
    let mut cache_records = Vec::new();
    let mut equation_payload = serde_json::to_vec(
        &da_application::equation_extraction_stage::EquationExtractionArtifact {
            candidates: Vec::new(),
            missingness: None,
        },
    )
    .expect("empty artifact serializes");
    let mut tables_payload = serde_json::to_vec(
        &da_application::table_extraction_stage::TableCandidateArtifact {
            candidates: Vec::new(),
            join_rows: Vec::new(),
            missingness: None,
        },
    )
    .expect("empty artifact serializes");
    if with_equations || with_tables || with_figures {
        let equations_sha = output_sha_of(&report, "equations")?;
        equation_payload = cached_payload(
            &cache,
            &now,
            &journey_cache_key(
                &source,
                "equation_extraction",
                BTreeMap::from([("parse".to_string(), parse_sha.clone())]),
                &equation_contract,
            ),
        )?;
        let _ = equations_sha;
    }
    if with_tables || with_figures {
        tables_payload = cached_payload(
            &cache,
            &now,
            &journey_cache_key(
                &source,
                "table_fidelity",
                BTreeMap::from([
                    ("parse".to_string(), parse_sha.clone()),
                    ("figures".to_string(), output_sha_of(&report, "figures")?),
                ]),
                &table_contract,
            ),
        )?;
    }

    // ── Figures branch (M409 S03): per-image mini-runs + driver assembly ───
    let mut candidate_report: Option<
        da_application::candidate_materialization_stage::CandidateMaterializationReport,
    > = None;
    if with_figures {
        let Some(branch) = &figure_branch else {
            return Err("figures branch requires model settings".into());
        };
        let Some(detection_stage) = &detection_stage_arc else {
            return Err("figures branch requires the detection stage".into());
        };
        let detection_contract = detection_stage.contract();
        let figures_payload = cached_payload(
            &cache,
            &now,
            &journey_cache_key(
                &source,
                "parse", // figure_detect canonicalizes to parse; the step name
                BTreeMap::from([("parse".to_string(), parse_sha.clone())]),
                &detection_contract,
            ),
        );
        // figure_detect is a parse alias: its step key is the canonical parse
        // key only when the detection contract matches the parse cache. The
        // detection payload is actually stored under the figure_detect step
        // key; retry with that identity.
        let figures_payload = match figures_payload {
            Ok(payload) => payload,
            Err(_) => cached_payload(
                &cache,
                &now,
                &journey_cache_key(
                    &source,
                    "figure_detect",
                    BTreeMap::from([("parse".to_string(), parse_sha.clone())]),
                    &detection_contract,
                ),
            )?,
        };
        let artifacts: Vec<da_domain::figure::DetectedVisualArtifact> =
            serde_json::from_slice(&figures_payload)
                .map_err(|error| format!("detection payload decode: {error}"))?;

        let mut figure_candidates = Vec::new();
        let mut figure_records: Vec<LoadCacheRecord> = Vec::new();
        let article_dir = pdf_path.parent().map(Path::to_path_buf).unwrap_or_default();
        let quote_policy = match load_figure_quote_policy_from_env() {
            Ok(policy) => policy,
            Err(error) => {
                tracing::warn!(
                    article_id = paper_id,
                    error = %error,
                    event = "da_load_journey_figures_quote_policy_unresolved"
                );
                return Err(format!("figure quote policy unresolved: {error}"));
            }
        };
        for artifact in artifacts
            .iter()
            .filter(|a| a.kind == da_domain::VisualArtifactKind::Figure)
        {
            let (section_id, _path_id, _ordinal) =
                match locate_figure_section(&artifact.caption, &parsed) {
                    Ok(grounded) => grounded,
                    Err(error) => {
                        // M419 S02: caption grounding failed — try the figure
                        // region's own PDF text and ground to the unique
                        // section containing it; otherwise keep the typed skip.
                        let fallback = try_region_text_grounding(pdf_path, artifact, &parsed);
                        match fallback {
                            Some((section_id, path_id)) => {
                                tracing::info!(
                                    article_id = paper_id,
                                    artifact_id = %artifact.artifact_id,
                                    event = "figure_grounded_via_region_text"
                                );
                                (section_id, path_id, 0)
                            }
                            None => {
                                tracing::warn!(
                                    article_id = paper_id,
                                    artifact_id = %artifact.artifact_id,
                                    reason = %error,
                                    event = "da_load_journey_figure_grounding_skipped"
                                );
                                continue;
                            }
                        }
                    }
                };
            let image_path = article_dir.join(&artifact.asset_path);
            let image_bytes = match std::fs::read(&image_path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(
                        article_id = paper_id,
                        artifact_id = %artifact.artifact_id,
                        path = %image_path.display(),
                        reason = %error,
                        event = "da_load_journey_figure_image_missing"
                    );
                    continue;
                }
            };
            let media_type = match artifact
                .asset_path
                .rsplit('.')
                .next()
                .map(|extension| extension.to_ascii_lowercase())
                .as_deref()
            {
                Some("png") => ImageMediaType::Png,
                Some("jpg") | Some("jpeg") => ImageMediaType::Jpeg,
                Some("webp") => ImageMediaType::Webp,
                _ => ImageMediaType::Png,
            };
            let context_stage = FigureContextStage::new(
                artifact.artifact_id.clone(),
                FigureSectionLocator::SectionId(section_id),
                None,
            )
            .with_dependencies("parse", "figures")
            .with_revision_identity(FigureRevisionIdentity {
                revision_vid: revision_vid.clone(),
                canonical_id: paper_id.to_string(),
            })
            .with_output_schema_fingerprint(&branch.context_schema);
            let analysis_stage = FigureAnalysisStage::new(
                branch.model.clone(),
                MultimodalImage {
                    media_type,
                    bytes: image_bytes.clone(),
                    sha256: artifact.asset_sha256.clone(),
                },
                &retry_after,
            )
            .with_context_dependency("context")
            .with_contract_versions(&branch.prompt_fingerprint, &branch.schema_fingerprint)
            .with_quote_policy(quote_policy.clone());
            let context_contract = context_stage.contract();
            let analysis_contract = analysis_stage.contract();

            let mini = PipelineBuilder::new(&format!("figure-{}", artifact.artifact_id))
                .step("parse", "parse", &[])
                .step("figures", "figure_detect", &["parse"])
                .step("context", "figure_context", &["parse", "figures"])
                .step("analysis", "figure_analysis", &["context"])
                .build()
                .map_err(|error| format!("figure mini topology: {error}"))?;
            let mini_orchestrator =
                PipelineOrchestrator::new(cache.clone(), clock.clone(), stage_registry.clone())
                    .with_executor("parse", parse_stage.clone())
                    .map_err(|error| error.to_string())?
                    .with_executor("figure_detect", detection_stage.clone())
                    .map_err(|error| error.to_string())?
                    .with_executor("figure_context", Arc::new(context_stage))
                    .map_err(|error| error.to_string())?
                    .with_executor("figure_analysis", Arc::new(analysis_stage))
                    .map_err(|error| error.to_string())?;
            let mini_report = mini_orchestrator
                .run(&mini, source.clone())
                .await
                .map_err(|error| format!("figure mini run: {error}"))?;
            if !mini_report.completed {
                let failed = mini_report
                    .stages
                    .iter()
                    .rev()
                    .find(|record| matches!(record.disposition, StageDisposition::Failed { .. }))
                    .map(|record| format!("{}: {:?}", record.stage_id, record.disposition))
                    .unwrap_or_else(|| "no failed record".into());
                return Err(format!(
                    "figure mini run incomplete for {} ({})",
                    artifact.artifact_id, failed
                ));
            }
            for record in &mini_report.stages {
                let status = match record.disposition {
                    StageDisposition::CacheHit { .. } => "hit",
                    StageDisposition::Executed { .. } => "miss",
                    _ => "retryable_failure",
                };
                figure_records.push(LoadCacheRecord {
                    stage_id: record.stage_id.clone(),
                    resolved_from: record.resolved_from.clone(),
                    status: status.into(),
                    key_sha256: record.cache_key_sha256.clone(),
                    payload_sha256: None,
                });
            }
            let context_sha = output_sha_of(&mini_report, "context")?;
            let analysis_sha = output_sha_of(&mini_report, "analysis")?;
            let context_payload = cached_payload(
                &cache,
                &now,
                &journey_cache_key(
                    &source,
                    "figure_context",
                    BTreeMap::from([
                        ("parse".to_string(), parse_sha.clone()),
                        ("figures".to_string(), output_sha_of(&report, "figures")?),
                    ]),
                    &context_contract,
                ),
            )?;
            let analysis_payload = cached_payload(
                &cache,
                &now,
                &journey_cache_key(
                    &source,
                    "figure_analysis",
                    BTreeMap::from([("context".to_string(), context_sha)]),
                    &analysis_contract,
                ),
            )?;
            let _ = analysis_sha;
            let context: da_domain::figure_context::FigureSemanticContext =
                serde_json::from_slice(&context_payload)
                    .map_err(|error| format!("figure context decode: {error}"))?;
            let analysis: da_domain::figure_analysis::FigureAnalysis =
                serde_json::from_slice(&analysis_payload)
                    .map_err(|error| format!("figure analysis decode: {error}"))?;
            let candidate = assemble_from_context(
                &context,
                &analysis,
                &artifact.detector_name,
                &parsed,
                &revision_vid,
                &source_pdf_sha,
            )
            .map_err(|error| format!("figure candidate assembly: {error}"))?;
            figure_candidates.push(candidate);
        }

        // Final combined candidates run through the stage directly (all
        // three family payloads; the stage is idempotent).
        let mut deps = BTreeMap::new();
        deps.insert("equations".to_string(), equation_payload.clone());
        deps.insert("tables".to_string(), tables_payload.clone());
        deps.insert(
            "figures".to_string(),
            serde_json::to_vec(&figure_candidates).expect("candidates serialize"),
        );
        let final_out = candidate_stage
            .execute(StageExecutionRequest {
                article_id: paper_id.to_string(),
                dependency_payloads: deps,
            })
            .await
            .map_err(|error| format!("figures candidate run: {error}"))?;
        let report_value: da_application::candidate_materialization_stage::CandidateMaterializationReport =
            serde_json::from_slice(&final_out.payload).expect("candidate report decodes");
        candidate_report = Some(report_value);
        cache_records.extend(figure_records);
        cache_records.push(LoadCacheRecord {
            stage_id: "candidate_materialization".into(),
            resolved_from: None,
            status: "bypass".into(),
            key_sha256: None,
            payload_sha256: None,
        });
    }

    // ── Map journey records to load cache records ──────────────────────────
    let mut cache_records_from_report = report
        .stages
        .iter()
        .map(|record| {
            let (status, payload_sha) = match &record.disposition {
                StageDisposition::Executed { output_sha256 } => {
                    let graph_stage = stage_registry
                        .cache_mode(&record.stage_id)
                        .map(|mode| mode == StageCacheMode::GraphMaterialize)
                        .unwrap_or(false);
                    (
                        if graph_stage { "bypass" } else { "miss" }.to_string(),
                        Some(output_sha256.clone()),
                    )
                }
                StageDisposition::CacheHit { output_sha256 } => {
                    ("hit".to_string(), Some(output_sha256.clone()))
                }
                StageDisposition::RetrySuppressed { .. } | StageDisposition::Failed { .. } => {
                    ("retryable_failure".to_string(), None)
                }
            };
            LoadCacheRecord {
                stage_id: record.stage_id.clone(),
                resolved_from: record.resolved_from.clone(),
                status,
                key_sha256: record.cache_key_sha256.clone(),
                payload_sha256: payload_sha,
            }
        })
        .collect::<Vec<LoadCacheRecord>>();
    cache_records_from_report.append(&mut cache_records);

    let outcome = LoadContentOutcome {
        ingest: LoadIngestOutcome {
            paper_id: paper_id.to_string(),
            title: parsed.title.clone(),
            body_chars: parsed.body_text.chars().count(),
            section_count: parsed.sections.len(),
            citation_count: parsed.citations.len(),
            snapshot_path: None,
            revision_vid: Some(revision_report.result.revision_vid.clone()),
            import_eligible: false,
        },
        extract: Some(LoadExtractOutcome {
            entities_extracted: extraction_report.entities_extracted,
            entities_created: extraction_report.entities_created,
            mentions_edges: extraction_report.mentions_edges,
            found_in_edges: extraction_report.found_in_edges,
        }),
        cache_records: cache_records_from_report,
        conservation: None,
    };
    let _ = candidate_contract;
    let _ = candidate_report;
    Ok((outcome, report))
}

/// Journey mode pins the PDF plane: GROBID profile from the shared env
/// surface, exactly like the legacy path default.
fn ports_profile() -> GrobidProfile {
    GrobidParser::from_env().profile()
}

/// Minimum region-text length considered sufficient grounding evidence.
/// Shorter fragments match too many sections to be trustworthy.
/// Exercised by tests and pdf-classifier builds; unused in the default
/// feature-off binary build.
#[allow(dead_code)]
const MIN_REGION_TEXT_CHARS: usize = 16;

/// Ground a figure to the UNIQUE section whose text contains `region_text`
/// (case-insensitive). Returns `(section_id, path_id)` of that section.
/// None when the text is too short, matches zero sections, or is ambiguous
/// (2+ matches) — grounding must never guess.
#[allow(dead_code)]
pub fn ground_section_by_region_text(
    parsed: &ParsedArticle,
    region_text: &str,
) -> Option<(String, String)> {
    let needle = region_text.trim().to_lowercase();
    if needle.chars().count() < MIN_REGION_TEXT_CHARS {
        return None;
    }
    let mut matched = parsed.sections.iter().filter_map(|section| {
        section
            .text
            .to_lowercase()
            .contains(&needle)
            .then(|| {
                Some((
                    section.section_id.clone()?,
                    section.path_id.clone().unwrap_or_default(),
                ))
            })
            .flatten()
    });
    let first = matched.next()?;
    matched.next().is_none().then_some(first)
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

/// IO side of the fallback: extract the figure region's PDF text via the
/// pdf-inspector adapter and apply [`ground_section_by_region_text`].
/// Only built with the `pdf-classifier` feature; otherwise always None.
#[allow(unused_variables)]
fn try_region_text_grounding(
    pdf_path: &Path,
    artifact: &da_domain::figure::DetectedVisualArtifact,
    parsed: &ParsedArticle,
) -> Option<(String, String)> {
    #[cfg(feature = "pdf-classifier")]
    {
        // The method lives in the RegionTextPort trait — the import must be
        // inside the cfg block so non-feature builds see no unused import.
        use da_ports::region_text::RegionTextPort;
        let bytes = std::fs::read(pdf_path).ok()?;
        let region = artifact.region;
        let outcome = da_adapters::PdfInspectorRegionText::new().extract_text_in_region(
            &bytes,
            artifact.page_index,
            [
                region.x1 as f32,
                region.y1 as f32,
                region.x2 as f32,
                region.y2 as f32,
            ],
        )?;
        if outcome.needs_ocr {
            return None;
        }
        ground_section_by_region_text(parsed, &outcome.text)
    }
    #[cfg(not(feature = "pdf-classifier"))]
    {
        None
    }
}

#[cfg(test)]
mod region_grounding_tests {
    use super::ground_section_by_region_text;
    use da_ports::parser::ParsedArticle;

    fn parsed(sections: &[(&str, &str, &str, &str)]) -> ParsedArticle {
        ParsedArticle {
            paper_id: "p".into(),
            title: "T".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: sections
                .iter()
                .map(|(id, path, _title, text)| da_ports::parser::Section {
                    title: id.to_string(),
                    text: text.to_string(),
                    level: 1,
                    path_id: Some(path.to_string()),
                    section_id: Some(id.to_string()),
                    ..Default::default()
                })
                .collect(),
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: String::new(),
        }
    }

    #[test]
    fn unique_containing_section_grounds() {
        let p = parsed(&[
            ("sec-1", "1", "A", "intro text about baselines"),
            (
                "sec-4",
                "4",
                "D",
                "results discuss the partitioning ratio here",
            ),
        ]);
        let got = ground_section_by_region_text(&p, "the partitioning ratio");
        assert_eq!(got, Some(("sec-4".into(), "4".into())));
    }

    #[test]
    fn ambiguous_or_missing_matches_refuse_to_guess() {
        let p = parsed(&[
            ("sec-1", "1", "A", "shared phrase about partitioning"),
            (
                "sec-2",
                "2",
                "B",
                "also mentions shared phrase about partitioning",
            ),
        ]);
        assert_eq!(
            ground_section_by_region_text(&p, "shared phrase about partitioning"),
            None
        );
        assert_eq!(
            ground_section_by_region_text(&p, "text that appears nowhere"),
            None
        );
        assert_eq!(ground_section_by_region_text(&p, "  too short  "), None);
    }
}
