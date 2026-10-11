//! S10 T04 — prove the single governed process on a hermetic request.
//!
//! One driven figures journey over the fake parser/detector/multimodal harness
//! and a REAL `FilePipelineCache` produces exactly one
//! [`GovernedProcessRecord`], and every field of that record is checked against
//! the live DATA it claims to bind:
//!
//! 1. the `profile_route` decision is `native` with a non-empty `policy_hash`
//!    and equals the decision computed from `data/policies/registry.yaml`;
//! 2. the consulted decision-point set is a subset of
//!    `consult_status.parse_pdf_consults` read from that same YAML, and hangs no
//!    later/unregistered point (`batch_admission`, `concurrency_tune`,
//!    `retry_policy`, `summary_depth`, `head_fallback`, `visual_bundle`,
//!    `critic_sample`) off the FSM;
//! 3. every admission code is in the closed set;
//! 4. the figure `PromptLineageRecord` resolves and its fingerprints match the
//!    real `data/ontology/prompt_registry.yaml` loaded through the S09 loader;
//! 5. `import_eligible` stays false.
//!
//! A negative case mutates the registry so it disagrees with the emitted
//! lineage: the journey must refuse with a typed error and materialize nothing.
//!
//! The harness mirrors `load_journey_test.rs`; the record is the artifact under
//! test, so the registry is loaded through `da_adapters::prompt_registry` (the
//! S09 loader) rather than the CLI convenience wrapper.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_adapters::{FilePipelineCache, SamyamaGraphStore};
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::{LoadContentOutcome, PipelineRunReport};
use da_cli::load_journey::{FigureBranchSettings, JourneyPorts, run_journey_content};
use da_domain::StageIdRegistry;
use da_domain::entity::EntityType;
use da_domain::governed_process_record::GOVERNED_ADMISSION_CODES;
use da_domain::prompt_registry::{PromptRegistry, PromptRegistryEntry};
use da_ports::clock::{Clock, FixedClock};
use da_ports::embedder::EmbedResult;
use da_ports::embedder::Embedder;
use da_ports::extractor::{ExtractResult, ExtractedEntity, Extractor};
use da_ports::parser::ParseResult;
use da_ports::parser::{ParsedArticle, ParserPort, Section};
use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};

const PAPER_ID: &str = "2507.19457";

/// The live FSM policy registry (DATA). Read for the allowed-consult set; the
/// journey reads the same file through `DA_POLICY_REGISTRY`.
const POLICY_REGISTRY_YAML: &str = include_str!("../../../data/policies/registry.yaml");

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Pin the DATA paths the journey resolves from the environment. Cargo runs
/// tests with CWD = crate root, so the repo-relative defaults would miss the
/// workspace files; pinning makes the journey consult the real DATA registry.
fn pin_workspace_env() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = workspace_root();
        // SAFETY: pinned once before any concurrent reader in this binary.
        unsafe {
            std::env::set_var(
                "DA_POLICY_REGISTRY",
                root.join("data/policies/registry.yaml"),
            );
            std::env::set_var(
                "DA_FIGURE_QUOTE_POLICY",
                root.join("data/ontology/figure_quote_policy.yaml"),
            );
        }
    });
}

/// Load the REAL unified prompt registry through the S09 loader with the real
/// task registry and the workspace root, and resolve the figure entry.
fn data_prompt_registry() -> (PromptRegistry, PromptRegistryEntry) {
    let root = workspace_root();
    let task_registry = da_adapters::prompt_task_registry::load_prompt_task_registry(
        &root.join("data/ontology/prompt_tasks.yaml"),
    )
    .expect("data/ontology/prompt_tasks.yaml loads");
    let registry = da_adapters::prompt_registry::load_prompt_registry(
        &root.join("data/ontology/prompt_registry.yaml"),
        &task_registry,
        &root,
    )
    .expect("data/ontology/prompt_registry.yaml loads");
    let entry = registry
        .resolve("figure_analysis", "figure-analysis.v3")
        .expect("figure_analysis@figure-analysis.v3 is declared in DATA")
        .clone();
    (registry, entry)
}

/// `consult_status.parse_pdf_consults` read from the live policy registry YAML.
fn allowed_parse_pdf_consults() -> BTreeSet<String> {
    #[derive(serde::Deserialize)]
    struct ConsultStatus {
        parse_pdf_consults: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    struct Doc {
        consult_status: ConsultStatus,
    }
    let doc: Doc = serde_yaml::from_str(POLICY_REGISTRY_YAML).expect("policy registry YAML parses");
    doc.consult_status.parse_pdf_consults.into_iter().collect()
}

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 8, 19, 12, 0, 0).unwrap(),
    ))
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m516-s10-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    dir
}

fn parsed_article() -> ParsedArticle {
    ParsedArticle {
        paper_id: PAPER_ID.into(),
        title: "Governed Process".into(),
        abstract_text: "abstract".into(),
        body_text: "body text".into(),
        sections: (0..2)
            .map(|ordinal| Section {
                title: format!("Section {ordinal}"),
                text: format!("text {ordinal}"),
                level: 1,
                path_id: Some(format!("{ordinal}")),
                section_id: Some(format!("sec-{ordinal}")),
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
        pdf_hash: sha(7),
        ..Default::default()
    }
}

fn parsed_with_figure_caption() -> ParsedArticle {
    let mut parsed = parsed_article();
    parsed.sections[1].text = format!(
        "{} Figure 3: Overview of the GEPA system.",
        parsed.sections[1].text
    );
    parsed
}

struct FigureParser;

#[async_trait]
impl ParserPort for FigureParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        let mut parsed = parsed_with_figure_caption();
        parsed.paper_id = paper_id.to_string();
        Ok(parsed)
    }
}

struct FakeExtractor;

#[async_trait]
impl Extractor for FakeExtractor {
    async fn extract(&self, _sections: &[(String, String)]) -> ExtractResult<Vec<ExtractedEntity>> {
        Ok(vec![ExtractedEntity {
            label: "GEPA".into(),
            entity_type: EntityType::Method,
            section_title: "Section 0".into(),
            section_ordinal: Some(0),
            char_start: 0,
            char_end: 4,
            surface: "GEPA".into(),
        }])
    }

    fn name(&self) -> &str {
        "fake-extractor"
    }
}

struct FakeEmbedder;

#[async_trait]
impl Embedder for FakeEmbedder {
    async fn embed(&self, _text: &str) -> EmbedResult<Vec<f32>> {
        Ok(vec![0.1, 0.2, 0.3])
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
    }

    fn dimensions(&self) -> usize {
        3
    }

    fn model_id(&self) -> &str {
        "fake-embedder"
    }
}

/// The model emits the REAL registry-bound lineage for
/// `figure_analysis@figure-analysis.v3`; the fingerprint fields are the digests
/// the loader computed, never hand-picked constants.
struct FakeLlm {
    prompt_sha256: da_domain::ContentDigest,
    output_schema_sha256: da_domain::ContentDigest,
}

#[async_trait]
impl da_ports::multimodal_llm::MultimodalLlmPort for FakeLlm {
    fn route(&self) -> da_ports::multimodal_llm::ModelRouteDescriptor {
        da_ports::multimodal_llm::ModelRouteDescriptor {
            route_id: "fake-figure-analysis".into(),
            protocol: da_ports::multimodal_llm::ModelWireProtocol::OpenAiChatCompletions,
            model: "fake".into(),
            endpoint_fingerprint: "fake".into(),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: false,
        }
    }

    async fn complete_structured(
        &self,
        _request: da_ports::multimodal_llm::MultimodalLlmRequest,
    ) -> da_ports::multimodal_llm::MultimodalLlmResult<
        da_ports::multimodal_llm::MultimodalLlmResponse,
    > {
        let structured = serde_json::to_value(da_domain::figure_analysis::FigureAnalysis {
            candidate_id: "cand".into(),
            artifact_id: "Figure3".into(),
            visual_type: da_domain::figure_analysis::VisualAnalysisType::Plot,
            description: "fake plot".into(),
            components: vec![],
            relationships: vec![],
            findings: vec![],
            citations: vec![da_domain::figure_analysis::FigureAnalysisCitation {
                section_id: "sec-1".into(),
                path_id: "1".into(),
                quote: "Overview of the GEPA system".into(),
            }],
            confidence: 0.9,
            provenance: da_domain::figure_analysis::FigureAnalysisProvenance {
                image_sha256: sha(9),
                context_sha256: sha(3),
                route_id: "fake".into(),
                protocol: "openai_chat_completions".into(),
                requested_model: "fake".into(),
                resolved_model: "fake".into(),
                prompt_id: String::new(),
                prompt_version: String::new(),
                prompt_sha256: sha(5),
                output_schema_sha256: sha(6),
                schema_id: "plot".into(),
                stage1_visual_type: "plot".into(),
                prompt_task_sha256: sha(7),
                route_policy_sha256: sha(8),
                model_binding_sha256: sha(9),
                input_sha256: sha(10),
                output_sha256: sha(11),
                input_tokens: None,
                output_tokens: None,
                latency_ms: 1,
            },
            typed_payload: None,
            authoritative: false,
            import_eligible: false,
        })
        .expect("serialize");
        Ok(da_ports::multimodal_llm::MultimodalLlmResponse {
            structured_output: structured,
            route: self.route(),
            resolved_model: "fake".into(),
            input_tokens: None,
            output_tokens: None,
            latency_ms: 1,
            prompt_lineage: Some(da_domain::prompt_optimization::PromptExecutionLineage {
                run_id: sha(1),
                prompt_id: "figure_analysis".into(),
                prompt_version: "figure-analysis.v3".into(),
                prompt_sha256: self.prompt_sha256.clone(),
                task_spec_sha256: da_domain::ContentDigest::of_text(&sha(5)),
                rendered_prompt_sha256: da_domain::ContentDigest::of_text(&sha(6)),
                output_schema_sha256: self.output_schema_sha256.clone(),
                route_policy_sha256: da_domain::ContentDigest::of_text(&sha(8)),
                endpoint_sha256: da_domain::ContentDigest::of_text(&sha(9)),
                model_binding_sha256: da_domain::ContentDigest::of_text(&sha(10)),
                inference_config_sha256: da_domain::ContentDigest::of_text(&sha(13)),
                context_bundle_sha256: da_domain::ContentDigest::of_text(&sha(14)),
                provider: "fake".into(),
                model: "fake".into(),
                input_sha256: da_domain::ContentDigest::of_text(&sha(11)),
                output_sha256: da_domain::ContentDigest::of_text(&sha(12)),
                input_tokens: 1,
                output_tokens: 1,
                latency_ms: 1,
                temperature_milli: 0,
                max_output_tokens: 1024,
                estimated_cost_microunits: None,
                authoritative: false,
                import_eligible: false,
            }),
        })
    }
}

struct FigureDetector;

#[async_trait]
impl da_ports::figure_detection::FigureDetectionPort for FigureDetector {
    fn detector_fingerprint(&self) -> &str {
        "fake-figure-detector-v1"
    }

    async fn detect(
        &self,
        request: da_ports::figure_detection::FigureDetectionRequest,
    ) -> da_ports::figure_detection::FigureDetectionPortResult<
        da_ports::figure_detection::FigureDetectionResult,
    > {
        let dir = request.canonical_article_dir.join("figures");
        std::fs::create_dir_all(&dir).expect("figures dir");
        // 1x1 PNG (deterministic bytes) so the analysis stage can read it back.
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        std::fs::write(dir.join("Figure3.png"), png).expect("png write");
        let png_sha = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(png))
        };
        Ok(da_ports::figure_detection::FigureDetectionResult {
            detector_fingerprint: "fake-figure-detector-v1".into(),
            metadata_path: dir.join("manifest.json"),
            detector_metadata_sha256: sha(4),
            manifest_sha256: sha(4),
            artifacts: vec![da_domain::figure::DetectedVisualArtifact {
                artifact_id: "Figure3".into(),
                kind: da_domain::figure::VisualArtifactKind::Figure,
                detector_name: "fake-figure-detector".into(),
                detector_fingerprint: "fake-figure-detector-v1".into(),
                source_pdf_sha256: sha(7),
                page_index: 3,
                region: da_domain::figure::BoundingBox72Dpi {
                    x1: 10.0,
                    y1: 20.0,
                    x2: 500.0,
                    y2: 300.0,
                },
                caption_region: da_domain::figure::BoundingBox72Dpi {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 1.0,
                },
                caption: "Figure 3: Overview of the GEPA system.".into(),
                image_text: vec![],
                render_dpi: 72,
                asset_path: "figures/Figure3.png".into(),
                asset_sha256: png_sha,
                metadata_sha256: sha(4),
            }],
            duration_ms: 1,
            revision_vid: request.revision_vid,
        })
    }
}

fn source() -> SourceIdentity {
    SourceIdentity {
        article_id: PAPER_ID.into(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(sha(7)),
        html_sha256: None,
        acquisition_fingerprint: "canonical-article-catalog-v1".into(),
        normalization_fingerprint: "grobid-navigation-v3".into(),
    }
}

fn manifestations() -> Vec<RevisionManifestationInput> {
    vec![RevisionManifestationInput {
        format: "pdf".into(),
        sha256: sha(7),
        bytes: 1000,
        uri: format!("catalog:{PAPER_ID}v1.pdf"),
        path: format!("{PAPER_ID}v1.pdf"),
        capture_status: "captured_local".into(),
    }]
}

fn ledger_registry() -> Arc<StageIdRegistry> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/ontology/pipeline_stage_contracts.yaml");
    let ledger =
        da_adapters::pipeline_ledger::load_pipeline_stage_ledger(&path).expect("DATA ledger");
    Arc::new(ledger.registry().expect("registry"))
}

/// Drive the hermetic figures journey over a real `FilePipelineCache`.
async fn run_figures(
    ports: &JourneyPorts,
    store: &Arc<SamyamaGraphStore>,
    cache: Arc<FilePipelineCache>,
    branch: FigureBranchSettings,
) -> Result<(LoadContentOutcome, PipelineRunReport), String> {
    let pdf_dir = tempdir("pdf-gov");
    let pdf_path = pdf_dir.join(format!("{PAPER_ID}v1.pdf"));
    std::fs::write(&pdf_path, b"%PDF-fixture").expect("pdf fixture");
    run_journey_content(
        ports,
        store,
        cache,
        clock(),
        ledger_registry(),
        PAPER_ID,
        &pdf_path,
        source(),
        "arxiv",
        1,
        manifestations(),
        true,
        true,
        true,
        Some(branch),
        None,
    )
    .await
}

fn figure_ports() -> JourneyPorts {
    JourneyPorts {
        parser: Arc::new(FigureParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: Some(Arc::new(FigureDetector)),
    }
}

#[tokio::test]
async fn hermetic_figures_journey_binds_real_data_policy_and_registry() {
    pin_workspace_env();
    let store = Arc::new(SamyamaGraphStore::new());
    let cache = Arc::new(FilePipelineCache::new(tempdir("cache-gov")));
    let (registry, entry) = data_prompt_registry();
    let branch = FigureBranchSettings {
        model: Arc::new(FakeLlm {
            prompt_sha256: entry.prompt_sha256.clone(),
            output_schema_sha256: entry.schema_sha256.clone(),
        }),
        prompt_fingerprint: entry.prompt_sha256.as_str().to_string(),
        schema_fingerprint: entry.schema_sha256.as_str().to_string(),
        context_schema: "FigureSemanticContext@serde-v2-summary-provenance".into(),
        prompt_registry: registry.clone(),
    };

    let (outcome, report) = run_figures(&figure_ports(), &store, cache, branch)
        .await
        .expect("hermetic figures journey completes");
    assert!(report.completed);

    let record = outcome
        .governed_process_record
        .expect("the journey attaches exactly one governed-process record");
    record.validate().expect("record is fail-closed valid");

    // (1) profile_route decision binds the real DATA native structural default.
    let profile = record
        .policy_decisions
        .iter()
        .find(|decision| decision.decision_point == "profile_route")
        .expect("profile_route decision is recorded");
    assert_eq!(profile.policy_source, "native", "profile_route is native");
    assert!(
        !profile.policy_hash.is_empty(),
        "policy_hash is a non-empty stamp"
    );
    assert_eq!(profile.resolved_value, "structural");
    let real_decision =
        da_cli::parse_profile::corpus_grobid_profile_decision_from_yaml(POLICY_REGISTRY_YAML);
    assert_eq!(profile.resolved_value, real_decision.profile.as_str());
    assert_eq!(
        profile.policy_hash, real_decision.policy_hash,
        "the record's decision equals the decision computed from live DATA"
    );

    // (2) consulted set ⊆ DATA parse_pdf_consults, and no later/unregistered point.
    let consulted: BTreeSet<String> = record
        .policy_decisions
        .iter()
        .map(|decision| decision.decision_point.clone())
        .collect();
    let allowed = allowed_parse_pdf_consults();
    assert!(
        consulted.is_subset(&allowed),
        "consulted {consulted:?} must be a subset of {allowed:?}"
    );
    for later in [
        "batch_admission",
        "concurrency_tune",
        "retry_policy",
        "summary_depth",
        "head_fallback",
        "visual_bundle",
        "critic_sample",
    ] {
        assert!(
            !consulted.contains(later),
            "later/unregistered point {later} must not be consulted"
        );
    }

    // (3) every admission code is in the closed set.
    for admission in &record.admissions {
        assert!(
            GOVERNED_ADMISSION_CODES.contains(&admission.code.as_str()),
            "admission code {} for {} is not closed",
            admission.code,
            admission.stage_id
        );
    }
    assert!(
        !record.admissions.is_empty(),
        "the figures topology reports per-stage admissions"
    );

    // (4) the figure lineage resolves and matches the real registry.
    let figure = record
        .prompt_lineages
        .iter()
        .find(|lineage| lineage.prompt_id == "figure_analysis")
        .expect("figure_analysis lineage is recorded");
    assert_eq!(figure.prompt_version, "figure-analysis.v3");
    let registered = registry
        .resolve(&figure.prompt_id, &figure.prompt_version)
        .expect("the recorded lineage resolves in the real registry");
    assert_eq!(
        figure.prompt_sha256,
        registered.prompt_sha256.as_str(),
        "recorded prompt fingerprint equals the registered one"
    );
    assert_eq!(
        figure.schema_sha256,
        registered.schema_sha256.as_str(),
        "recorded schema fingerprint equals the registered one"
    );

    // (5) candidate-only.
    assert!(!record.import_eligible);
}

#[tokio::test]
async fn journey_refuses_a_registry_that_disagrees_with_emitted_lineage() {
    pin_workspace_env();
    let store = Arc::new(SamyamaGraphStore::new());
    let cache = Arc::new(FilePipelineCache::new(tempdir("cache-gov-neg")));
    let (registry, entry) = data_prompt_registry();

    // The model still emits the REAL registry fingerprints; the registry handed
    // to the stage is mutated so it disagrees with that emitted lineage.
    let mut disagreeing = registry.clone();
    let target = disagreeing
        .entries
        .iter_mut()
        .find(|candidate| {
            candidate.prompt_id == "figure_analysis"
                && candidate.prompt_version == "figure-analysis.v3"
        })
        .expect("figure_analysis entry");
    target.prompt_sha256 = da_domain::ContentDigest::of_text("tampered-registry-template");
    assert_ne!(
        target.prompt_sha256, entry.prompt_sha256,
        "the disagreeing registry must actually differ"
    );

    let branch = FigureBranchSettings {
        model: Arc::new(FakeLlm {
            prompt_sha256: entry.prompt_sha256.clone(),
            output_schema_sha256: entry.schema_sha256.clone(),
        }),
        prompt_fingerprint: entry.prompt_sha256.as_str().to_string(),
        schema_fingerprint: entry.schema_sha256.as_str().to_string(),
        context_schema: "FigureSemanticContext@serde-v2-summary-provenance".into(),
        prompt_registry: disagreeing,
    };

    let error = run_figures(&figure_ports(), &store, cache, branch)
        .await
        .expect_err("a registry disagreeing with the emitted lineage must refuse");
    assert!(
        error.contains("figure mini run"),
        "the refusal surfaces through the figure branch: {error}"
    );
    assert!(
        error.contains("fingerprint mismatch"),
        "the typed lineage refusal names the mismatch: {error}"
    );
}
