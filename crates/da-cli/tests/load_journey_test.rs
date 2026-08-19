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

#[tokio::test]
async fn journey_with_equations_materializes_candidates_and_replays() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache_dir = tempdir("cache-eq");
    let cache = Arc::new(FilePipelineCache::new(cache_dir.clone()));
    let ports = JourneyPorts {
        parser: Arc::new(FormulaParser),
        extractor: Arc::new(FakeExtractor),
        embedder: Arc::new(FakeEmbedder),
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
