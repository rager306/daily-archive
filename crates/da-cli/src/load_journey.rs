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
    FdApiEmbedder, GrobidParseStage, GrobidParser, GrobidProfile, ParsedArticleEmbeddingStage,
    RuleBasedExtractor, RuleExtractionStage, SamyamaGraphStore,
};
use da_application::candidate_materialization_stage::CandidateMaterializationStage;
use da_application::equation_extraction_stage::EquationExtractionStage;
use da_application::extraction_materialization_stage::ExtractionMaterializationReport;
use da_application::extraction_materialization_stage::ExtractionMaterializationStage;
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::revision_materialization_stage::RevisionMaterializationReport;
use da_application::revision_materialization_stage::RevisionMaterializationStage;
use da_application::{
    ArticleJourneyComposition, LoadContentOutcome, LoadExtractOutcome, LoadIngestOutcome,
    PipelineOrchestrator, PipelineRunReport, StageDisposition, embedding_fingerprint,
};
use da_domain::{LoadCacheRecord, StageCacheMode, StageIdRegistry};
use da_ports::clock::Clock;
use da_ports::embedder::Embedder;
use da_ports::extractor::Extractor;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, ParserPort};
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceIdentity, StageCacheKey};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageContract};

/// Injectable production ports for the journey composition.
pub struct JourneyPorts {
    pub parser: Arc<dyn ParserPort>,
    pub extractor: Arc<dyn Extractor>,
    pub embedder: Arc<dyn Embedder>,
}

impl JourneyPorts {
    /// Resolve every port from the existing env/config surfaces. No new env
    /// keys: GROBID_* and FD_API_* surfaces are reused exactly as the legacy
    /// load path resolves them.
    pub fn from_env() -> Self {
        Self {
            parser: Arc::new(GrobidParser::from_env()),
            extractor: Arc::new(RuleBasedExtractor::new()),
            embedder: Arc::new(FdApiEmbedder::from_env()),
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
) -> Result<(LoadContentOutcome, PipelineRunReport), String> {
    let journey = if with_equations {
        ArticleJourneyComposition::with_equations(&stage_registry, "da-load-journey")
    } else {
        ArticleJourneyComposition::canonical(&stage_registry, "da-load-journey")
    }
    .map_err(|error| error.to_string())?;

    let retry_after = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    let grobid_profile = ports_profile();
    let parse_stage = GrobidParseStage::new(
        ports.parser.clone(),
        pdf_path.display().to_string(),
        &retry_after,
    )
    .with_contract(load_parse_contract(&grobid_profile));
    let extract_stage =
        RuleExtractionStage::new(ports.extractor.clone(), "parse", "rules.phrases@bundled-v1");
    let embed_stage =
        ParsedArticleEmbeddingStage::new(ports.embedder.clone(), "parse", &retry_after);
    let revision_stage = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock.clone(),
        source_code,
        paper_id,
        revision,
        manifestations,
    );
    let extraction_stage =
        ExtractionMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);

    // M407: the equation branch is pure TEI compute plus candidate-only
    // graph evidence; revision identity is computable upfront (pure VID), so
    // the stage contracts stay stable before the run.
    let revision_vid = da_domain::paper_revision_vid(source_code, paper_id, revision)
        .map_err(|error| format!("journey revision vid: {error}"))?
        .to_string();
    let equation_stage = EquationExtractionStage::new(
        revision_vid.clone(),
        source.pdf_sha256.clone().unwrap_or_default(),
    );
    let candidate_stage =
        CandidateMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);

    // Snapshot the exact contracts BEFORE the executors move into the
    // orchestrator: these fingerprints reconstruct the cache keys below.
    let parse_contract = parse_stage.contract();
    let embed_contract = embed_stage.contract();
    let revision_contract = revision_stage.contract();
    let extraction_contract = extraction_stage.contract();

    let mut orchestrator =
        PipelineOrchestrator::new(cache.clone(), clock.clone(), stage_registry.clone())
            .with_executor("parse", Arc::new(parse_stage))
            .map_err(|error| error.to_string())?
            .with_executor("revision_materialization", Arc::new(revision_stage))
            .map_err(|error| error.to_string())?
            .with_executor("extraction_compute", Arc::new(extract_stage))
            .map_err(|error| error.to_string())?
            .with_executor("extraction_materialization", Arc::new(extraction_stage))
            .map_err(|error| error.to_string())?
            .with_executor("embedding", Arc::new(embed_stage))
            .map_err(|error| error.to_string())?;
    if with_equations {
        orchestrator = orchestrator
            .with_executor("equation_extraction", Arc::new(equation_stage))
            .map_err(|error| error.to_string())?
            .with_executor("candidate_materialization", Arc::new(candidate_stage))
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
            BTreeMap::from([("parse".to_string(), parse_sha)]),
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
        Some(existing) => {
            return Err(format!(
                "embedding replacement is not supported safely for paper {paper_id}: existing={existing}, requested={embedding_sha}"
            ));
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

    // ── Map journey records to load cache records ──────────────────────────
    let cache_records = report
        .stages
        .iter()
        .map(|record| {
            let (status, payload_sha) = match &record.disposition {
                StageDisposition::Executed { output_sha256 } => {
                    // Graph-materialize stages always re-execute over their
                    // cached report: report that honestly as `bypass`.
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
        .collect();

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
        cache_records,
        conservation: None,
    };
    Ok((outcome, report))
}

/// Journey mode pins the PDF plane: GROBID profile from the shared env
/// surface, exactly like the legacy path default.
fn ports_profile() -> GrobidProfile {
    GrobidParser::from_env().profile()
}
