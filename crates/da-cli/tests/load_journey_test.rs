//! M404 S01 — `da load --journey` content pipeline over the real store.
//!
//! Fake ports (parser/extractor/embedder) over a REAL in-memory Samyama store
//! and a REAL file cache in a tempdir, driven by the DATA ledger registry.
//! Proves: journey completes with graph content, the embedding vector lands
//! behind the legacy sha guard, an identical replay keeps compute stages as
//! cache hits while materialization stages re-execute with zero duplicate
//! nodes, and D127 stays false everywhere.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_adapters::{FilePipelineCache, GrobidProfile, SamyamaGraphStore};
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::{PipelineRunReport, StageDisposition};
use da_cli::load_journey::{JourneyPorts, load_parse_contract, run_journey_content};
use da_domain::entity::EntityType;
use da_domain::{LoadCacheRecord, StageIdRegistry};
use da_ports::clock::{Clock, FixedClock};
use da_ports::embedder::EmbedResult;
use da_ports::embedder::{Embedder, EmbedderError};
use da_ports::extractor::ExtractResult;
use da_ports::extractor::{ExtractedEntity, Extractor, ExtractorError};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::ParseResult;
use da_ports::parser::{ParsedArticle, ParserError, ParserPort, Section};
use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};

const PAPER_ID: &str = "2507.19457";

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 8, 19, 12, 0, 0).unwrap(),
    ))
}

fn parsed_article() -> ParsedArticle {
    ParsedArticle {
        paper_id: PAPER_ID.into(),
        title: "Journey CLI".into(),
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
        pdf_hash: sha(7),
    }
}

struct FakeParser;

#[async_trait]
impl ParserPort for FakeParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        let mut parsed = parsed_article();
        parsed.paper_id = paper_id.to_string();
        Ok(parsed)
    }
}

struct FakeExtractor;

#[async_trait]
impl Extractor for FakeExtractor {
    async fn extract(&self, _sections: &[(String, String)]) -> ExtractResult<Vec<ExtractedEntity>> {
        Ok(vec![
            ExtractedEntity {
                label: "GEPA".into(),
                entity_type: EntityType::Method,
                section_title: "Section 0".into(),
                section_ordinal: Some(0),
                char_start: 0,
                char_end: 4,
                surface: "GEPA".into(),
            },
            ExtractedEntity {
                label: "F1".into(),
                entity_type: EntityType::Metric,
                section_title: "Section 1".into(),
                section_ordinal: Some(1),
                char_start: 0,
                char_end: 2,
                surface: "F1".into(),
            },
        ])
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
        let mut out = Vec::with_capacity(texts.len());
        for _ in texts {
            out.push(vec![0.1, 0.2, 0.3]);
        }
        Ok(out)
    }

    fn dimensions(&self) -> usize {
        3
    }

    fn model_id(&self) -> &str {
        "fake-embedder"
    }
}

// Silence unused-import noise for result aliases used in signatures.
#[allow(dead_code)]
fn _aliases(_: Option<EmbedderError>, _: Option<ExtractorError>, _: Option<ParserError>) {}

fn parsed_article_with_formulas() -> ParsedArticle {
    let mut parsed = parsed_article();
    parsed.formulas = (0..2)
        .map(|i| da_ports::parser::ExtractedFormula {
            formula_id: format!("formula_{i}"),
            section_path_id: Some("1".into()),
            section_ordinal: Some(1),
            label: Some(format!("({i})")),
            coordinates: Some("10,20,30,40;5".into()),
            representation: da_ports::parser::FormulaRepresentation::TeXMath("L=x".into()),
            span: Some(da_ports::parser::CharSpan { start: 1, end: 4 }),
            source_digest: sha(7),
            missingness: None,
        })
        .collect();
    parsed
}

struct FormulaParser;

#[async_trait]
impl ParserPort for FormulaParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        let mut parsed = parsed_article_with_formulas();
        parsed.paper_id = paper_id.to_string();
        Ok(parsed)
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

async fn query_count(store: &SamyamaGraphStore, cypher: &str) -> usize {
    let result = store
        .query_readonly("daily_archive", cypher)
        .await
        .expect("readonly cypher");
    let cell = result
        .records
        .first()
        .and_then(|record| record.first())
        .expect("count row");
    match cell {
        serde_json::Value::Number(number) => number.as_u64().unwrap_or(0) as usize,
        serde_json::Value::String(text) => text.parse().unwrap_or(0),
        _ => panic!("unexpected count cell {cell:?}"),
    }
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m404-journey-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    dir
}

async fn run_journey(
    ports: &JourneyPorts,
    store: &Arc<SamyamaGraphStore>,
    cache: Arc<FilePipelineCache>,
) -> Result<(da_application::LoadContentOutcome, PipelineRunReport), String> {
    let pdf_dir = tempdir("pdf");
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
        false,
        false,
        false,
        None,
    )
    .await
}

#[tokio::test]
async fn journey_content_completes_and_replays_over_file_cache() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache_dir = tempdir("cache");
    let cache = Arc::new(FilePipelineCache::new(cache_dir.clone()));
    let ports = JourneyPorts {
        parser: Arc::new(FakeParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: None,
    };

    // Parse contract keeps the legacy load-v1 fingerprints so the operator's
    // existing parse cache entries stay reusable.
    let parse_contract = load_parse_contract(&GrobidProfile::rich());
    assert_eq!(
        parse_contract.implementation_fingerprint,
        "da_adapters::GrobidParser@load-v1"
    );
    assert_eq!(parse_contract.output_schema_fingerprint, "ParsedArticle.v1");

    // ── First journey ──────────────────────────────────────────────────────
    let (outcome, report) = run_journey(&ports, &store, cache.clone())
        .await
        .expect("first journey");
    assert!(report.completed);
    assert_eq!(report.stages.len(), 5);
    for record in &report.stages {
        assert!(
            matches!(record.disposition, StageDisposition::Executed { .. }),
            "first run executes {}",
            record.stage_id
        );
    }
    assert_eq!(outcome.ingest.title, "Journey CLI");
    assert_eq!(outcome.ingest.section_count, 2);
    assert_eq!(outcome.ingest.citation_count, 0);
    assert!(outcome.ingest.revision_vid.is_some());
    assert!(!outcome.ingest.import_eligible);
    let extract = outcome.extract.expect("extract outcome");
    assert_eq!(extract.entities_extracted, 2);
    assert_eq!(extract.entities_created, 2);
    assert!(extract.mentions_edges > 0);

    assert_eq!(
        query_count(&store, "MATCH (p:Paper) RETURN count(p) AS papers").await,
        1
    );
    assert_eq!(
        query_count(&store, "MATCH (s:Section) RETURN count(s) AS sections").await,
        2
    );
    assert_eq!(
        query_count(&store, "MATCH (e:Entity) RETURN count(e) AS entities").await,
        2
    );

    // Embedding vector landed behind the legacy sha guard.
    let paper_node = store
        .find_node_by_string_property("Paper", "arxiv_id", PAPER_ID)
        .await
        .expect("paper node");
    let embedding_sha = store
        .get_node_property_string(paper_node, "embedding_sha256")
        .await
        .expect("embedding sha stamped");

    // ── Identical replay over the same store and file cache ────────────────
    let (outcome2, report2) = run_journey(&ports, &store, cache.clone())
        .await
        .expect("replay journey");
    assert!(report2.completed);
    let dispositions: Vec<&str> = report2
        .stages
        .iter()
        .map(|record| match record.disposition {
            StageDisposition::Executed { .. } => "executed",
            StageDisposition::CacheHit { .. } => "cache_hit",
            StageDisposition::RetrySuppressed { .. } => "retry_suppressed",
            StageDisposition::Failed { .. } => "failed",
        })
        .collect();
    assert_eq!(
        dispositions,
        vec![
            "cache_hit",
            "executed",
            "cache_hit",
            "executed",
            "cache_hit"
        ],
        "compute stages replay from the file cache; graph stages always re-run"
    );
    assert_eq!(
        query_count(&store, "MATCH (p:Paper) RETURN count(p) AS papers").await,
        1,
        "no duplicate Paper"
    );
    assert_eq!(
        query_count(&store, "MATCH (s:Section) RETURN count(s) AS sections").await,
        2,
        "no duplicate Sections"
    );
    assert_eq!(
        query_count(&store, "MATCH (e:Entity) RETURN count(e) AS entities").await,
        2,
        "no duplicate Entities"
    );
    assert_eq!(
        store
            .get_node_property_string(paper_node, "embedding_sha256")
            .await
            .as_deref(),
        Some(embedding_sha.as_str()),
        "vector guard is idempotent across replays"
    );

    // Load cache records carry honest statuses.
    let statuses: BTreeMap<String, String> = outcome2
        .cache_records
        .iter()
        .map(|record: &LoadCacheRecord| (record.stage_id.clone(), record.status.clone()))
        .collect();
    assert_eq!(statuses.get("parse").map(String::as_str), Some("hit"));
    assert_eq!(
        statuses.get("revision_materialization").map(String::as_str),
        Some("bypass")
    );
    assert_eq!(
        statuses.get("extraction_compute").map(String::as_str),
        Some("hit")
    );
    assert_eq!(
        statuses
            .get("extraction_materialization")
            .map(String::as_str),
        Some("bypass")
    );
    assert_eq!(statuses.get("embedding").map(String::as_str), Some("hit"));

    // D127: nothing unlocks import anywhere on the journey planes.
    for label in ["Paper", "PaperRevision", "Section", "Entity"] {
        assert_eq!(
            query_count(
                &store,
                &format!(
                    "MATCH (c:{label}) WHERE c.import_eligible = true RETURN count(c) AS unlocked"
                )
            )
            .await,
            0,
            "no {label} is import-eligible"
        );
    }
}

struct FakeDetector;

#[async_trait]
impl da_ports::figure_detection::FigureDetectionPort for FakeDetector {
    fn detector_fingerprint(&self) -> &str {
        "fake-detector-v1"
    }

    async fn detect(
        &self,
        request: da_ports::figure_detection::FigureDetectionRequest,
    ) -> da_ports::figure_detection::FigureDetectionPortResult<
        da_ports::figure_detection::FigureDetectionResult,
    > {
        Ok(da_ports::figure_detection::FigureDetectionResult {
            detector_fingerprint: "fake-detector-v1".into(),
            metadata_path: std::path::PathBuf::from("figures/manifest.json"),
            detector_metadata_sha256: sha(4),
            manifest_sha256: sha(4),
            artifacts: vec![table_artifact_fixture()],
            duration_ms: 1,
            revision_vid: request.revision_vid,
        })
    }
}

fn table_artifact_fixture() -> da_domain::figure::DetectedVisualArtifact {
    da_domain::figure::DetectedVisualArtifact {
        artifact_id: "Table1".into(),
        kind: da_domain::figure::VisualArtifactKind::Table,
        detector_name: "fake".into(),
        detector_fingerprint: "fake-detector-v1".into(),
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
        caption: "Table 1: results.".into(),
        image_text: vec![],
        render_dpi: 72,
        asset_path: "figures/Table1.png".into(),
        asset_sha256: sha(9),
        metadata_sha256: sha(4),
    }
}

fn parsed_article_with_tables_and_formulas() -> ParsedArticle {
    let mut parsed = parsed_article_with_formulas();
    parsed.tables = vec![da_ports::parser::ExtractedTable {
        table_id: "tab_0".into(),
        section_path_id: Some("1".into()),
        section_ordinal: Some(1),
        caption: "Table 1 :".into(),
        coordinates: Some("10,20,500,300;3".into()),
        cells: vec![da_ports::parser::TableCell {
            row: 0,
            column: 0,
            colspan: 1,
            rowspan: 1,
            text: "v".into(),
            coordinates: None,
            span: None,
            digest: sha(5),
        }],
        source_digest: sha(3),
        missingness: None,
    }];
    parsed
}

struct TableParser;

#[async_trait]
impl ParserPort for TableParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        let mut parsed = parsed_article_with_tables_and_formulas();
        parsed.paper_id = paper_id.to_string();
        Ok(parsed)
    }
}

#[tokio::test]
async fn journey_with_equations_materializes_candidates_and_replays() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache_dir = tempdir("cache-eq");
    let cache = Arc::new(FilePipelineCache::new(cache_dir.clone()));
    let ports = JourneyPorts {
        parser: Arc::new(FormulaParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: None,
    };
    let pdf_dir = tempdir("pdf-eq");
    let pdf_path = pdf_dir.join(format!("{PAPER_ID}v1.pdf"));
    std::fs::write(&pdf_path, b"%PDF-fixture").expect("pdf fixture");

    async fn run(
        store: &Arc<SamyamaGraphStore>,
        cache: Arc<FilePipelineCache>,
        ports: &JourneyPorts,
        pdf_path: &std::path::Path,
    ) -> Result<(da_application::LoadContentOutcome, PipelineRunReport), String> {
        da_cli::load_journey::run_journey_content(
            ports,
            store,
            cache,
            clock(),
            ledger_registry(),
            PAPER_ID,
            pdf_path,
            source(),
            "arxiv",
            1,
            manifestations(),
            true,
            false,
            false,
            None,
        )
        .await
    };

    let (outcome, report) = run(&store, cache.clone(), &ports, &pdf_path)
        .await
        .expect("equations journey");
    assert!(report.completed);
    assert_eq!(
        report.stages.len(),
        7,
        "extended topology: five base + two equation steps"
    );
    assert!(outcome.ingest.revision_vid.is_some());
    assert_eq!(
        query_count(&store, "MATCH (e:EquationCandidate) RETURN count(e) AS eqs").await,
        2,
        "equation candidates materialized"
    );
    // Candidate authority locks (D127 + candidate-only).
    assert_eq!(
        query_count(
            &store,
            "MATCH (e:EquationCandidate) WHERE e.import_eligible = true RETURN count(e) AS unlocked"
        )
        .await,
        0
    );

    // Identical replay: compute stages cache-hit; graph stages re-run
    // idempotently with zero duplicate candidates.
    let (_, report2) = run(&store, cache, &ports, &pdf_path)
        .await
        .expect("equations replay");
    assert!(report2.completed);
    let dispositions: Vec<&str> = report2
        .stages
        .iter()
        .map(|record| match record.disposition {
            StageDisposition::Executed { .. } => "executed",
            StageDisposition::CacheHit { .. } => "cache_hit",
            StageDisposition::RetrySuppressed { .. } => "retry_suppressed",
            StageDisposition::Failed { .. } => "failed",
        })
        .collect();
    assert_eq!(
        dispositions,
        vec![
            "cache_hit",
            "executed",
            "cache_hit",
            "executed",
            "cache_hit",
            "cache_hit",
            "executed"
        ],
        "compute (parse/extract/embed/equations) replays from cache; graph stages always re-run"
    );
    assert_eq!(
        query_count(&store, "MATCH (e:EquationCandidate) RETURN count(e) AS eqs").await,
        2,
        "no duplicate equation candidates"
    );
}

#[tokio::test]
async fn journey_with_tables_runs_detection_anchored_branch() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache_dir = tempdir("cache-tb");
    let cache = Arc::new(FilePipelineCache::new(cache_dir.clone()));
    let ports = JourneyPorts {
        parser: Arc::new(TableParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: Some(Arc::new(FakeDetector)),
    };
    let pdf_dir = tempdir("pdf-tb");
    let pdf_path = pdf_dir.join(format!("{PAPER_ID}v1.pdf"));
    std::fs::write(&pdf_path, b"%PDF-fixture").expect("pdf fixture");

    let run = async |store: &Arc<SamyamaGraphStore>, cache: Arc<FilePipelineCache>| {
        da_cli::load_journey::run_journey_content(
            &ports,
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
            false,
            None,
        )
        .await
    };

    let (outcome, report) = run(&store, cache.clone()).await.expect("tables journey");
    assert!(report.completed);
    assert_eq!(report.stages.len(), 9, "with_tables topology: nine steps");
    assert!(outcome.ingest.revision_vid.is_some());
    assert_eq!(
        query_count(&store, "MATCH (e:EquationCandidate) RETURN count(e) AS eqs").await,
        2,
        "equation candidates still materialize"
    );
    assert_eq!(
        query_count(&store, "MATCH (t:TableCandidate) RETURN count(t) AS tabs").await,
        1,
        "detection-anchored table candidate materialized"
    );
    for label in ["TableCandidate", "EquationCandidate"] {
        assert_eq!(
            query_count(
                &store,
                &format!(
                    "MATCH (c:{label}) WHERE c.import_eligible = true RETURN count(c) AS unlocked"
                )
            )
            .await,
            0,
            "no {label} unlocks import"
        );
    }

    // Identical replay: compute (parse/figures/extract/embed/equations/tables)
    // cache-hits; graph stages re-run idempotently.
    let (_, report2) = run(&store, cache).await.expect("tables replay");
    assert!(report2.completed);
    let dispositions: Vec<&str> = report2
        .stages
        .iter()
        .map(|record| match record.disposition {
            StageDisposition::Executed { .. } => "executed",
            StageDisposition::CacheHit { .. } => "cache_hit",
            StageDisposition::RetrySuppressed { .. } => "retry_suppressed",
            StageDisposition::Failed { .. } => "failed",
        })
        .collect();
    assert_eq!(
        dispositions,
        vec![
            "cache_hit", // parse
            "cache_hit", // figures (detection)
            "executed",  // revision
            "cache_hit", // extract
            "executed",  // materialize
            "cache_hit", // tables
            "cache_hit", // embed
            "cache_hit", // equations
            "executed",  // candidates
        ],
        "only graph-materialize stages re-execute on replay"
    );
    assert_eq!(
        query_count(&store, "MATCH (t:TableCandidate) RETURN count(t) AS tabs").await,
        1,
        "no duplicate table candidates"
    );
}

#[tokio::test]
async fn journey_tables_without_detector_fails_closed_at_composition() {
    // The CLI layer turns an unconfigured detector into a typed skip and
    // runs the equations journey; the composition itself refuses a tables
    // run without a detector (defense in depth).
    let ports = JourneyPorts {
        parser: Arc::new(TableParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: None,
    };
    let store = Arc::new(SamyamaGraphStore::new());
    let cache = Arc::new(FilePipelineCache::new(tempdir("cache-nodet")));
    let pdf_dir = tempdir("pdf-nodet");
    let pdf_path = pdf_dir.join(format!("{PAPER_ID}v1.pdf"));
    std::fs::write(&pdf_path, b"%PDF-fixture").expect("pdf fixture");
    let err = da_cli::load_journey::run_journey_content(
        &ports,
        &store,
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
        false,
        None,
    )
    .await
    .expect_err("tables without detector must fail closed");
    assert!(err.contains("detector"), "{err}");
}

// ── M409 S03: per-image figure branch ───────────────────────────────────────

struct FakeLlm;

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
        request: da_ports::multimodal_llm::MultimodalLlmRequest,
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
                task_id: "figure_analysis".into(),
                task_spec_sha256: da_domain::ContentDigest::of_text(&sha(5)),
                rendered_prompt_sha256: da_domain::ContentDigest::of_text(&sha(6)),
                output_schema_sha256: da_domain::ContentDigest::of_text(&sha(7)),
                route_policy_sha256: da_domain::ContentDigest::of_text(&sha(8)),
                endpoint_sha256: da_domain::ContentDigest::of_text(&sha(9)),
                model_binding_sha256: da_domain::ContentDigest::of_text(&sha(10)),
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
        // Write a real (tiny) PNG next to the PDF so the analysis stage can
        // read the image asset back.
        let dir = request.canonical_article_dir.join("figures");
        std::fs::create_dir_all(&dir).expect("figures dir");
        // 1x1 PNG (deterministic bytes).
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
            artifacts: vec![
                da_domain::figure::DetectedVisualArtifact {
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
                },
                da_domain::figure::DetectedVisualArtifact {
                    artifact_id: "Table1".into(),
                    kind: da_domain::figure::VisualArtifactKind::Table,
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
                    caption: "Table 1: results.".into(),
                    image_text: vec![],
                    render_dpi: 72,
                    asset_path: "figures/Table1.png".into(),
                    asset_sha256: sha(9),
                    metadata_sha256: sha(4),
                },
            ],
            duration_ms: 1,
            revision_vid: request.revision_vid,
        })
    }
}

fn parsed_with_figure_caption() -> ParsedArticle {
    let mut parsed = parsed_article_with_tables_and_formulas();
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

#[tokio::test]
async fn journey_with_figures_runs_per_image_branch() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache_dir = tempdir("cache-fig");
    let cache = Arc::new(FilePipelineCache::new(cache_dir.clone()));
    let ports = JourneyPorts {
        parser: Arc::new(FigureParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
        detector: Some(Arc::new(FigureDetector)),
    };
    let branch = da_cli::load_journey::FigureBranchSettings {
        model: Arc::new(FakeLlm),
        prompt_fingerprint: sha(5),
        schema_fingerprint: sha(6),
        context_schema: "FigureSemanticContext@serde-v2-summary-provenance".into(),
    };
    let pdf_dir = tempdir("pdf-fig");
    let pdf_path = pdf_dir.join(format!("{PAPER_ID}v1.pdf"));
    std::fs::write(&pdf_path, b"%PDF-fixture").expect("pdf fixture");

    let run = async |store: &Arc<SamyamaGraphStore>, cache: Arc<FilePipelineCache>| {
        da_cli::load_journey::run_journey_content(
            &ports,
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
            Some(branch.clone()),
        )
        .await
    };

    let (outcome, report) = run(&store, cache.clone()).await.expect("figures journey");
    assert!(report.completed);
    assert_eq!(report.stages.len(), 8, "with_figures topology: eight steps");
    assert!(outcome.ingest.revision_vid.is_some());
    assert_eq!(
        query_count(&store, "MATCH (f:FigureCandidate) RETURN count(f) AS figs").await,
        1,
        "figure candidate materialized through the per-image branch"
    );
    assert_eq!(
        query_count(&store, "MATCH (t:TableCandidate) RETURN count(t) AS tabs").await,
        1,
        "table candidates still materialize"
    );
    for label in ["FigureCandidate", "TableCandidate", "EquationCandidate"] {
        assert_eq!(
            query_count(
                &store,
                &format!(
                    "MATCH (c:{label}) WHERE c.import_eligible = true RETURN count(c) AS unlocked"
                )
            )
            .await,
            0,
            "no {label} unlocks import"
        );
    }

    // Identical replay: parse figures tables embed equations and both
    // per-image mini steps cache-hit; graph stages re-execute idempotently.
    let (_, report2) = run(&store, cache).await.expect("figures replay");
    assert!(report2.completed);
    assert_eq!(
        query_count(&store, "MATCH (f:FigureCandidate) RETURN count(f) AS figs").await,
        1,
        "no duplicate figure candidates"
    );
    assert_eq!(
        query_count(&store, "MATCH (t:TableCandidate) RETURN count(t) AS tabs").await,
        1,
        "no duplicate table candidates"
    );
}
