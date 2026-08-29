//! M403 S03 — canonical article journey over the real in-memory Samyama store.
//!
//! One orchestrated run: parse → revision_materialization →
//! extraction_compute → extraction_materialization → embedding, validated
//! against the real DATA stage ledger. Compute stages are cache-honest
//! (identical replay is a CacheHit with no executor re-run); graph-materialize
//! stages always re-execute and replay with zero new writes (applied content
//! is reused). D127 stays false everywhere.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_adapters::SamyamaGraphStore;
use da_adapters::memory_pipeline_cache::MemoryPipelineCache;
use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::{
    ArticleJourneyComposition, ExtractionMaterializationStage, PipelineOrchestrator,
    RevisionMaterializationStage, StageDisposition,
};
use da_domain::entity::EntityType;
use da_ports::clock::{Clock, FixedClock};
use da_ports::extractor::ExtractedEntity;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{ParsedArticle, Section};
use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use std::path::Path;

const PAPER_ID: &str = "2507.19457";

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 8, 19, 0, 0, 0).unwrap(),
    ))
}

fn parsed_article() -> ParsedArticle {
    ParsedArticle {
        paper_id: PAPER_ID.into(),
        title: "Journey".into(),
        abstract_text: "abstract".into(),
        body_text: "body".into(),
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

fn manifestations() -> Vec<RevisionManifestationInput> {
    vec![RevisionManifestationInput {
        format: "pdf".into(),
        sha256: sha(7),
        bytes: 1000,
        uri: format!("https://arxiv.org/pdf/{PAPER_ID}v1"),
        path: format!("{PAPER_ID}v1.pdf"),
        capture_status: "captured".into(),
    }]
}

fn entities_payload() -> Vec<u8> {
    let entities = vec![
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
    ];
    serde_json::to_vec(&entities).expect("entities serialize")
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

/// Counting fake compute stage (parse / extraction_compute / embedding).
struct FakeStage {
    stage_id: &'static str,
    payload: Vec<u8>,
    calls: AtomicUsize,
}

impl FakeStage {
    fn new(stage_id: &'static str, payload: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            stage_id,
            payload,
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl PipelineStageExecutor for FakeStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: self.stage_id.into(),
            implementation_fingerprint: format!("FakeStage@{}", self.stage_id),
            configuration_fingerprint: "journey-fixture-v1".into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: format!("FakeOutput@{}", self.stage_id),
        }
    }

    async fn execute(
        &self,
        _request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(StageExecutionOutput {
            payload: self.payload.clone(),
            metadata: BTreeMap::new(),
        })
    }
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

#[tokio::test]
async fn canonical_article_journey_completes_and_replays_without_duplicates() {
    let store = Arc::new(SamyamaGraphStore::new());
    let cache = Arc::new(MemoryPipelineCache::default());

    // Real DATA ledger: cache modes and ACAP layers come from YAML.
    let ledger_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/ontology/pipeline_stage_contracts.yaml");
    let ledger = load_pipeline_stage_ledger(&ledger_path).expect("DATA ledger");
    let registry = Arc::new(ledger.registry().expect("registry"));
    assert_eq!(
        registry.cache_mode("revision_materialization").unwrap(),
        da_domain::StageCacheMode::GraphMaterialize
    );
    assert_eq!(
        registry.cache_mode("extraction_materialization").unwrap(),
        da_domain::StageCacheMode::GraphMaterialize
    );

    let journey =
        ArticleJourneyComposition::canonical(&registry, "article-journey").expect("journey");
    assert_eq!(
        journey.canonical_stages,
        vec![
            "parse",
            "revision_materialization",
            "extraction_compute",
            "extraction_materialization",
            "embedding",
        ]
    );

    let parse = FakeStage::new(
        "parse",
        serde_json::to_vec(&parsed_article()).expect("parsed serialize"),
    );
    let extract = FakeStage::new("extraction_compute", entities_payload());
    let embed = FakeStage::new("embedding", b"[0.1, 0.2]".to_vec());

    let revision_stage = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let extraction_stage =
        ExtractionMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);

    let orchestrator = PipelineOrchestrator::new(cache, clock(), registry)
        .with_executor("parse", parse.clone())
        .unwrap()
        .with_executor("revision_materialization", Arc::new(revision_stage))
        .unwrap()
        .with_executor("extraction_compute", extract.clone())
        .unwrap()
        .with_executor("extraction_materialization", Arc::new(extraction_stage))
        .unwrap()
        .with_executor("embedding", embed.clone())
        .unwrap();

    // ── First journey: everything executes, the graph gains content ────────
    let first = orchestrator
        .run(&journey.topology, source())
        .await
        .expect("journey completes");
    assert!(first.completed, "first journey must complete: {first:?}");
    assert_eq!(first.stages.len(), 5, "one record per journey step");
    for record in &first.stages {
        assert!(
            matches!(record.disposition, StageDisposition::Executed { .. }),
            "first run executes {}",
            record.stage_id
        );
        let admission = record.admission.as_ref().expect("journey is gated");
        assert!(admission.admitted, "{} admitted", record.stage_id);
    }
    assert_eq!(parse.calls(), 1);
    assert_eq!(extract.calls(), 1);
    assert_eq!(embed.calls(), 1);

    assert_eq!(
        query_count(&store, "MATCH (p:Paper) RETURN count(p) AS papers").await,
        1,
        "Paper materialized"
    );
    assert_eq!(
        query_count(
            &store,
            "MATCH (r:PaperRevision) RETURN count(r) AS revisions"
        )
        .await,
        1,
        "PaperRevision materialized"
    );
    assert_eq!(
        query_count(&store, "MATCH (s:Section) RETURN count(s) AS sections").await,
        2,
        "Sections materialized"
    );
    assert_eq!(
        query_count(&store, "MATCH (e:Entity) RETURN count(e) AS entities").await,
        2,
        "Entities materialized"
    );
    assert_eq!(
        query_count(
            &store,
            "MATCH (p:Paper)-[:MENTIONS]->(e:Entity) RETURN count(e) AS mentioned"
        )
        .await,
        2,
        "MENTIONS edges link entities to the Paper"
    );

    // ── Identical replay: compute stages are cache hits; graph stages
    // re-execute idempotently with zero new writes ──────────────────────────
    let second = orchestrator
        .run(&journey.topology, source())
        .await
        .expect("replay completes");
    assert!(second.completed);
    let dispositions: Vec<&str> = second
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
        "compute stages replay from cache; graph stages always re-execute"
    );
    assert_eq!(parse.calls(), 1, "parse never re-runs");
    assert_eq!(extract.calls(), 1, "extraction compute never re-runs");
    assert_eq!(embed.calls(), 1, "embedding never re-runs");

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

    // D127: nothing is import-eligible anywhere on the journey planes.
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
