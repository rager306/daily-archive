//! M400 S04 T03 — same-process canonical multimodal journey proof.
//!
//! One process, one canonical multimodal production path:
//!
//! 1. **Pipeline plane** — `MultimodalComposition::canonical` over the real
//!    DATA stage ledger (`data/ontology/pipeline_stage_contracts.yaml`)
//!    drives a `PipelineOrchestrator` with an exact-identity
//!    `MemoryPipelineCache`. Every gated multimodal stage is admitted through
//!    the ACAP FSM with a durable evidence reference; candidate
//!    materialization is a cross-cutting helper and is deliberately not
//!    gated. An identical replay hits the cache on every stage (executor call
//!    counts stay 1) and produces exactly one record per step — no duplicate
//!    stage records. The `MultimodalFingerprintSummary` binds the exact
//!    effective-input contract (source, GROBID profile, detector, schema
//!    registry, model route, output schemas) with `import_eligible=false`.
//! 2. **Materialization plane** — the same article's Figure/Table/Equation
//!    candidates are materialized by the real revision-grounded materializers
//!    over a real revision plane on the in-memory Samyama engine. First
//!    application writes candidate nodes + lineage edges with authority locks
//!    hard-locked false; an identical replay applies zero writes and reports
//!    `unchanged:replay` — no duplicate authoritative effects.
//! 3. **Semantic query plane** — the grounded graph answers semantic Cypher
//!    queries with exact revision/section lineage and preserves D127
//!    (`import_eligible=false`, `authoritative=false`) everywhere.
//!
//! Negative control: a cached `figure_analysis` output cannot admit a mid-FSM
//! entry — the gate blocks before cache acceptance, the executor never runs,
//! and the blocked record carries no cache key.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_adapters::SamyamaGraphStore;
use da_adapters::memory_pipeline_cache::MemoryPipelineCache;
use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_application::equation_materialize::{
    DIAGNOSTIC_APPLIED as EQUATION_APPLIED, DIAGNOSTIC_UNCHANGED_REPLAY,
    EquationCandidateMaterializer, equation_candidate_input_from_extracted,
};
use da_application::figure_materialize::{
    DIAGNOSTIC_APPLIED as FIGURE_APPLIED, FigureCandidateMaterializer,
};
use da_application::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationUseCase,
};
use da_application::table_materialize::{
    DIAGNOSTIC_APPLIED as TABLE_APPLIED, TableCandidateMaterializer,
    structural_table_from_extracted,
};
use da_application::{
    MultimodalComposition, MultimodalFingerprintSummary, PipelineOrchestrator, StageDisposition,
};
use da_domain::VisualArtifactKind;
use da_domain::equation_candidate::EquationCandidate;
use da_domain::figure_analysis::FigureTablePayload;
use da_domain::figure_candidate::{FigureCandidate, figure_candidate_vid};
use da_domain::relation::structure;
use da_domain::table_candidate::{TableCandidate, TableCandidateLineage};
use da_domain::table_fidelity::compare_table_fidelity;
use da_domain::vid::revision_section_vid;
use da_ports::clock::{Clock, FixedClock};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{
    CharSpan, ExtractedFormula, ExtractedTable, FormulaRepresentation, ParsedArticle, Section,
    TableCell,
};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageCacheKey,
};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use kg_pipeline::PipelineBuilder;

const PAPER_ID: &str = "2507.19457";

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

fn source(pdf_hash: &str) -> SourceIdentity {
    SourceIdentity {
        article_id: PAPER_ID.into(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(pdf_hash.into()),
        html_sha256: None,
        acquisition_fingerprint: "canonical-article-catalog-v1".into(),
        normalization_fingerprint: "grobid-navigation-v3".into(),
    }
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 8, 8, 0, 0, 0).unwrap(),
    ))
}

/// The real DATA pipeline stage ledger (19 canonical stages) — the journey
/// composes against the committed contracts, not a hand-built fixture.
fn data_registry() -> Arc<da_domain::StageIdRegistry> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/ontology/pipeline_stage_contracts.yaml");
    let ledger = load_pipeline_stage_ledger(&path).expect("load DATA stage ledger");
    assert!(
        !ledger.import_eligible,
        "D127: ledger import_eligible false"
    );
    Arc::new(ledger.registry().expect("DATA registry"))
}

/// Fake stage executor with a call counter — the replay-idempotence counter
/// for the pipeline plane. Contracts carry realistic multimodal fingerprints
/// (detector / schema registry / model route / output schema) so the cache
/// identity and the fingerprint summary are exact, not placeholders.
struct FakeStage {
    contract: StageContract,
    payload: Vec<u8>,
    calls: AtomicUsize,
}

impl FakeStage {
    fn new(
        stage_id: &str,
        payload: &[u8],
        configuration_fingerprint: &str,
        model_fingerprint: Option<&str>,
        output_schema_fingerprint: &str,
    ) -> Self {
        Self {
            contract: StageContract {
                stage_id: stage_id.into(),
                implementation_fingerprint: format!("{stage_id}-impl-v1"),
                configuration_fingerprint: configuration_fingerprint.into(),
                prompt_fingerprint: None,
                model_fingerprint: model_fingerprint.map(str::to_owned),
                output_schema_fingerprint: output_schema_fingerprint.into(),
            },
            payload: payload.to_vec(),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl PipelineStageExecutor for FakeStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
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

/// Wire the seven canonical multimodal executors into an orchestrator over
/// the real DATA registry and an exact-identity in-memory cache.
fn multimodal_orchestrator(
    cache: Arc<MemoryPipelineCache>,
    registry: Arc<da_domain::StageIdRegistry>,
) -> (PipelineOrchestrator, Vec<Arc<FakeStage>>) {
    let grobid_profile = "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula";
    let stages = vec![
        Arc::new(FakeStage::new(
            "grobid_parse",
            b"parsed-article",
            grobid_profile,
            Some("grobid/grobid:0.9.0-crf"),
            "ParsedArticle@serde-v3-unique-navigation",
        )),
        Arc::new(FakeStage::new(
            "figure_detect",
            b"detected-visual-artifacts",
            "render_dpi=150",
            Some("pdffigures2-0.0.12"),
            "DetectedVisualArtifact[]@serde-v1",
        )),
        Arc::new(FakeStage::new(
            "figure_context",
            b"grounded-figure-context",
            "budget=default-18k;neighbors=prev,next;refs=3",
            None,
            "FigureSemanticContext@serde-v2-summary-provenance",
        )),
        Arc::new(FakeStage::new(
            "figure_analysis",
            b"typed-figure-analysis",
            "schema_registry=figure-schemas.v1;schemas=diagram_analysis:sha1,chart_extraction:sha2",
            Some("route|anthropic_messages|vision|fallback=false"),
            "FigureAnalysis@serde-v1",
        )),
        Arc::new(FakeStage::new(
            "table_fidelity",
            b"typed-table-fidelity",
            "fidelity=tei-vs-visual-v2",
            None,
            "TableFidelityReport@serde-v1",
        )),
        Arc::new(FakeStage::new(
            "equation_extraction",
            b"typed-equation-inputs",
            "representation=mathml,texmath",
            None,
            "EquationCandidateInput[]@serde-v1",
        )),
        Arc::new(FakeStage::new(
            "candidate_materialization",
            b"candidate-evidence",
            "candidate_only=true;D127=false",
            None,
            "CandidateEvidenceGraph@serde-v1",
        )),
    ];
    let mut orchestrator = PipelineOrchestrator::new(cache, clock(), registry);
    for stage in &stages {
        orchestrator = orchestrator
            .with_executor(stage.contract.stage_id.clone(), stage.clone())
            .expect("register canonical executor");
    }
    (orchestrator, stages)
}

// ── Revision plane (shared by materialization + semantic query) ─────────────

fn revision_request(revision: u32, section_count: usize) -> RevisionMaterializationRequest {
    RevisionMaterializationRequest {
        source_code: "arxiv".into(),
        canonical_id: PAPER_ID.into(),
        revision,
        parsed: ParsedArticle {
            paper_id: PAPER_ID.into(),
            title: "GEPA".into(),
            abstract_text: "Reflective prompt mutation.".into(),
            body_text: "body".into(),
            sections: (0..section_count)
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
            pdf_hash: sha(u64::from(revision)),
            ..Default::default()
        },
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: sha(u64::from(revision)),
            bytes: 1000 + u64::from(revision),
            uri: format!("https://arxiv.org/pdf/{PAPER_ID}v{revision}"),
            path: format!("{PAPER_ID}v{revision}.pdf"),
            capture_status: "captured".into(),
        }],
    }
}

async fn seed_revision_plane_samyama(
    store: &Arc<SamyamaGraphStore>,
    revision: u32,
    section_count: usize,
) -> String {
    let uc = RevisionMaterializationUseCase::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );
    let out = uc
        .materialize(&revision_request(revision, section_count))
        .await
        .expect("revision materialization");
    out.revision_vid
}

// ── Candidate fixtures (exactly the S01/S02/S03 closure fixtures) ───────────

fn figure_candidate(revision_vid: &str) -> FigureCandidate {
    let section_vid = revision_section_vid(revision_vid, 2).unwrap();
    let vid = figure_candidate_vid(
        revision_vid,
        "Figure3",
        "pdffigures2-0.0.12",
        &sha(7),
        &section_vid,
    )
    .unwrap();
    FigureCandidate {
        vid,
        revision_vid: revision_vid.into(),
        artifact_id: "Figure3".into(),
        kind: VisualArtifactKind::Figure,
        detector_name: "pdffigures2".into(),
        detector_fingerprint: "pdffigures2-0.0.12".into(),
        detector_metadata_sha256: sha(3),
        source_pdf_sha256: sha(1),
        image_sha256: sha(7),
        asset_path: "figures/Figure3.png".into(),
        page_index: 3,
        section_vid,
        section_title: "Section 2".into(),
        section_ordinal: 2,
        caption: "Figure 3: Overview of the GEPA system.".into(),
        candidate_metadata_sha256: sha(4),
        authoritative: false,
        import_eligible: false,
        schema_version: da_domain::figure_candidate::FIGURE_CANDIDATE_SCHEMA_VERSION,
        materialization_diagnostics: None,
    }
}

fn ports_cell(row: u32, column: u32, text: &str) -> TableCell {
    TableCell {
        row,
        column,
        colspan: 1,
        rowspan: 1,
        text: text.into(),
        coordinates: Some(format!(
            "{row},{column},{},{};3",
            column * 100,
            row * 100 + 50
        )),
        span: Some(CharSpan {
            start: row * 100 + column * 10,
            end: row * 100 + column * 10 + text.len() as u32,
        }),
        digest: format!("{row:032x}{column:032x}"),
    }
}

fn perfect_cells() -> Vec<TableCell> {
    vec![
        ports_cell(0, 0, "Method"),
        ports_cell(0, 1, "F1"),
        ports_cell(1, 0, "GEPA"),
        ports_cell(1, 1, "0.87"),
        ports_cell(2, 0, "Baseline"),
        ports_cell(2, 1, "0.72"),
    ]
}

fn perfect_visual() -> FigureTablePayload {
    FigureTablePayload {
        headers: vec!["Method".into(), "F1".into()],
        rows: vec![
            vec!["GEPA".into(), "0.87".into()],
            vec!["Baseline".into(), "0.72".into()],
        ],
    }
}

fn table_candidate(revision_vid: &str) -> TableCandidate {
    let lineage = TableCandidateLineage {
        revision_vid: revision_vid.into(),
        artifact_id: "Table3".into(),
        detector_name: "pdffigures2".into(),
        detector_fingerprint: "pdffigures2-0.0.12".into(),
        image_sha256: sha(7),
        asset_path: "figures/Table3.png".into(),
        page_index: 3,
        section_vid: revision_section_vid(revision_vid, 2).unwrap(),
        section_title: "Section 2".into(),
        section_ordinal: 2,
        caption: "Table 3: GEPA results.".into(),
        source_pdf_sha256: sha(1),
    };
    let extracted = ExtractedTable {
        table_id: "tab_0".into(),
        section_path_id: Some("2".into()),
        section_ordinal: Some(2),
        caption: "Table 3 :".into(),
        coordinates: Some("10,20,500,300;3".into()),
        cells: perfect_cells(),
        source_digest: sha(7),
        missingness: None,
    };
    let structural = structural_table_from_extracted(&extracted);
    structural.validate().expect("structural projection valid");
    let visual = perfect_visual();
    let report =
        compare_table_fidelity(Some(&visual), Some(&structural)).expect("fidelity comparison");
    TableCandidate::assemble(
        &lineage,
        Some(&visual),
        &structural,
        &report,
        Some("10,20,500,300;3".into()),
    )
    .expect("assemble table candidate")
}

fn equation_candidate(revision_vid: &str) -> EquationCandidate {
    let formula = ExtractedFormula {
        formula_id: "formula_0".into(),
        section_path_id: Some("2".into()),
        section_ordinal: Some(2),
        label: Some("(1)".into()),
        coordinates: Some("10,20,30,40;5".into()),
        representation: FormulaRepresentation::TeXMath(
            "L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]".into(),
        ),
        span: Some(CharSpan {
            start: 120,
            end: 158,
        }),
        source_digest: sha(7),
        missingness: None,
    };
    let sections: Vec<Section> = (0..3)
        .map(|ordinal| Section {
            title: format!("Section {ordinal}"),
            text: format!("text {ordinal}"),
            level: 1,
            path_id: Some(format!("{ordinal}")),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        })
        .collect();
    let input = equation_candidate_input_from_extracted(&formula, revision_vid, &sha(1), &sections)
        .expect("ports to domain conversion");
    EquationCandidate::new(input).expect("assemble equation candidate")
}

/// Semantic query helper over the real Samyama engine.
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

// ────────────────────────────────────────────────────────────────────────────
// THE JOURNEY
// ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn canonical_multimodal_journey_proves_admission_cache_materialization_query_replay() {
    // ── Plane A: pipeline orchestration over the real DATA registry ─────────
    let registry = data_registry();
    let composition = MultimodalComposition::canonical(&registry, "production-multimodal-v1")
        .expect("canonical multimodal topology");
    assert_eq!(composition.topology.steps.len(), 7);
    assert_eq!(
        composition.canonical_stages,
        vec![
            "parse",
            "figure_context",
            "figure_analysis",
            "table_fidelity",
            "equation_extraction",
            "candidate_materialization",
        ]
    );

    let cache = Arc::new(MemoryPipelineCache::new());
    let (orchestrator, stages) = multimodal_orchestrator(cache.clone(), registry.clone());
    let src = source(&sha(1));

    // First run: every stage executes, gated stages are admitted with a
    // durable evidence chain, candidate materialization is a cross-cutting
    // helper and surfaces as `skipped` (admission None).
    let first = orchestrator
        .run(&composition.topology, src.clone())
        .await
        .expect("first multimodal run");
    assert!(first.completed);
    assert_eq!(first.stages.len(), 7, "one record per step, no duplicates");
    for record in &first.stages {
        assert!(
            matches!(record.disposition, StageDisposition::Executed { .. }),
            "first run executes {}",
            record.stage_id
        );
        assert!(
            record.cache_key_sha256.is_some(),
            "executed stage carries its exact cache key identity"
        );
    }
    // ACAP admission: the five gated multimodal stages are admitted
    // monotonically with durable evidence; materialize is not gated.
    let gated = [
        "parse",
        "figure_context",
        "figure_analysis",
        "table_fidelity",
        "equation_extraction",
    ];
    for record in first
        .stages
        .iter()
        .filter(|record| gated.contains(&record.stage_id.as_str()))
    {
        let admission = record
            .admission
            .as_ref()
            .expect("gated stage carries admission");
        assert!(admission.admitted, "{} must be admitted", record.stage_id);
        assert!(admission.error.is_none());
        assert!(
            !admission.evidence_ref.is_empty(),
            "admission carries durable evidence"
        );
    }
    let materialize = first
        .stages
        .iter()
        .find(|record| record.stage_id == "candidate_materialization")
        .expect("materialize record");
    assert!(
        materialize.admission.is_none(),
        "candidate materialization is a cross-cutting helper, not gated"
    );
    // Entry evidence starts from the checksum-bound source; downstream
    // evidence references the prior stage's durable cache key.
    let parse_record = &first.stages[0];
    assert_eq!(
        parse_record.admission.as_ref().unwrap().evidence_ref,
        format!("source:sha256:{}", sha(1))
    );
    // figure_detect resolves to the canonical parse identity.
    let figures = first
        .stages
        .iter()
        .find(|record| record.step_name == "figures")
        .expect("figures step");
    assert_eq!(figures.stage_id, "parse");
    assert_eq!(figures.resolved_from.as_deref(), Some("figure_detect"));

    // Identical replay: every stage is a cache hit with its exact key
    // identity, executors never re-run, one record per step.
    let second = orchestrator
        .run(&composition.topology, src.clone())
        .await
        .expect("multimodal replay");
    assert!(second.completed);
    assert_eq!(second.stages.len(), 7, "replay must not duplicate records");
    for record in &second.stages {
        // M403 S01: graph-materialize stages (candidate_materialization over
        // the real DATA ledger) always re-execute idempotently — the cached
        // report is lineage evidence, never a skip. Compute stages stay
        // plain cache hits.
        let expected_rerun = record.stage_id == "candidate_materialization";
        let executed = matches!(record.disposition, StageDisposition::Executed { .. });
        let cache_hit = matches!(record.disposition, StageDisposition::CacheHit { .. });
        if expected_rerun {
            assert!(
                executed,
                "graph-materialize stage always re-executes for {}",
                record.stage_id
            );
        } else {
            assert!(cache_hit, "replay hits cache for {}", record.stage_id);
        }
        assert!(
            record.cache_key_sha256.is_some(),
            "replay record carries its durable cache key"
        );
    }
    for stage in &stages {
        let expected_calls = if stage.contract.stage_id == "candidate_materialization" {
            2
        } else {
            1
        };
        assert_eq!(
            stage.calls(),
            expected_calls,
            "replay call count for {}",
            stage.contract.stage_id
        );
    }

    // The cache manifest is article-scoped and exposes the exact stage keys.
    let manifest = cache.manifest(PAPER_ID).expect("manifest");
    let stage_ids = manifest
        .iter()
        .map(|entry| entry.key.stage_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for canonical in [
        "parse",
        "figure_context",
        "figure_analysis",
        "table_fidelity",
        "equation_extraction",
        "candidate_materialization",
    ] {
        assert!(stage_ids.contains(canonical), "manifest has {canonical}");
    }
    assert!(
        manifest
            .iter()
            .all(|entry| entry.key.source.article_id == PAPER_ID)
    );

    // Exact fingerprint summary of the effective-input contract: source,
    // GROBID profile, detector, schema registry, model route, output schemas.
    let contracts = stages
        .iter()
        .map(|stage| (stage.contract.stage_id.clone(), stage.contract()))
        .collect::<BTreeMap<_, _>>();
    let summary = MultimodalFingerprintSummary::bind(
        &src,
        "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula",
        &contracts,
    );
    assert!(summary.source.contains(PAPER_ID));
    assert!(summary.source.contains("canonical-article-catalog-v1"));
    assert!(summary.grobid_profile.contains("profile=structural"));
    assert_eq!(summary.detector, "pdffigures2-0.0.12");
    assert!(summary.schema_registry.contains("figure-schemas.v1"));
    assert!(summary.model_route.contains("anthropic_messages"));
    assert_eq!(
        summary.output_schemas["figure_context"],
        "FigureSemanticContext@serde-v2-summary-provenance"
    );
    assert_eq!(
        summary.output_schemas["figure_analysis"],
        "FigureAnalysis@serde-v1"
    );
    assert!(!summary.import_eligible, "D127 stays false");

    // ── Plane B: candidate materialization over a real revision plane ────────
    // The same article/revision, materialized on the in-memory Samyama
    // engine by the real revision-grounded materializers.
    let store = Arc::new(SamyamaGraphStore::new());
    let revision = seed_revision_plane_samyama(&store, 1, 3).await;

    let figure_m = FigureCandidateMaterializer::new(store.clone() as Arc<dyn DirectGraphStore>);
    let table_m = TableCandidateMaterializer::new(store.clone() as Arc<dyn DirectGraphStore>);
    let equation_m = EquationCandidateMaterializer::new(store.clone() as Arc<dyn DirectGraphStore>);

    let figure = figure_candidate(&revision);
    let table = table_candidate(&revision);
    let equation = equation_candidate(&revision);

    let figure_first = figure_m.materialize(&figure).await.expect("figure write");
    assert!(figure_first.applied && !figure_first.unchanged);
    assert_eq!(figure_first.nodes_created, 1);
    assert_eq!(figure_first.edges_created, 2);
    assert_eq!(figure_first.materialization_diagnostics, FIGURE_APPLIED);
    assert!(!figure_first.import_eligible, "D127 figure");

    let table_first = table_m.materialize(&table).await.expect("table write");
    assert!(table_first.applied && !table_first.unchanged);
    assert_eq!(table_first.materialization_diagnostics, TABLE_APPLIED);
    assert!(!table_first.import_eligible, "D127 table");

    let equation_first = equation_m
        .materialize(&equation)
        .await
        .expect("equation write");
    assert!(equation_first.applied && !equation_first.unchanged);
    assert_eq!(equation_first.nodes_created, 1);
    assert_eq!(equation_first.edges_created, 2);
    assert_eq!(equation_first.materialization_diagnostics, EQUATION_APPLIED);
    assert!(!equation_first.import_eligible, "D127 equation");

    // Exact lineage edges on the real engine.
    let figure_node = store
        .find_node_by_string_property("FigureCandidate", "vid", &figure.vid)
        .await
        .expect("figure node");
    let revision_node = store
        .find_node_by_string_property("PaperRevision", "vid", &revision)
        .await
        .expect("revision node");
    assert_eq!(
        store
            .get_edges_between(revision_node, figure_node, structure::HAS_PART)
            .await
            .len(),
        1,
        "PaperRevision hasPart FigureCandidate"
    );
    let section_node = store
        .find_node_by_string_property("Section", "vid", &figure.section_vid)
        .await
        .expect("section node");
    assert_eq!(
        store
            .get_edges_between(figure_node, section_node, structure::GROUNDED_IN)
            .await
            .len(),
        1,
        "FigureCandidate groundedIn Section"
    );
    assert_eq!(
        store
            .get_node_property_bool(figure_node, "authoritative")
            .await,
        Some(false)
    );
    assert_eq!(
        store
            .get_node_property_bool(figure_node, "import_eligible")
            .await,
        Some(false)
    );

    // Identical replay applies zero writes (no duplicate authoritative
    // effects) for every materializer.
    let figure_replay = figure_m.materialize(&figure).await.expect("figure replay");
    assert!(!figure_replay.applied && figure_replay.unchanged);
    assert_eq!(figure_replay.nodes_created, 0);
    assert_eq!(figure_replay.edges_created, 0);
    assert_eq!(
        figure_replay.materialization_diagnostics,
        DIAGNOSTIC_UNCHANGED_REPLAY
    );

    let table_replay = table_m.materialize(&table).await.expect("table replay");
    assert!(!table_replay.applied && table_replay.unchanged);
    assert_eq!(table_replay.nodes_created, 0);
    assert_eq!(table_replay.edges_created, 0);
    assert_eq!(
        table_replay.materialization_diagnostics,
        DIAGNOSTIC_UNCHANGED_REPLAY
    );

    let equation_replay = equation_m
        .materialize(&equation)
        .await
        .expect("equation replay");
    assert!(!equation_replay.applied && equation_replay.unchanged);
    assert_eq!(equation_replay.nodes_created, 0);
    assert_eq!(equation_replay.edges_created, 0);
    assert_eq!(
        equation_replay.materialization_diagnostics,
        DIAGNOSTIC_UNCHANGED_REPLAY
    );

    // ── Plane C: semantic query over the grounded graph ──────────────────────
    // Every candidate is queryable with exact revision/section lineage and
    // hard-locked authority/import flags.
    assert_eq!(
        query_count(
            &store,
            "MATCH (r:PaperRevision)-[:hasPart]->(c:FigureCandidate)-[:groundedIn]->(s:Section) \
             WHERE c.revision_vid = r.vid AND c.section_vid = s.vid RETURN count(c) AS grounded"
        )
        .await,
        1,
        "grounded figure candidate lineage"
    );
    assert_eq!(
        query_count(
            &store,
            "MATCH (r:PaperRevision)-[:hasPart]->(t:TableCandidate) \
             WHERE t.revision_vid = r.vid RETURN count(t) AS tables"
        )
        .await,
        1,
        "revision-bound table candidate"
    );
    assert_eq!(
        query_count(
            &store,
            "MATCH (r:PaperRevision)-[:hasPart]->(e:EquationCandidate)-[:groundedIn]->(s:Section) \
             WHERE e.revision_vid = r.vid AND e.section_vid = s.vid RETURN count(e) AS equations"
        )
        .await,
        1,
        "grounded equation candidate lineage"
    );
    // D127: no candidate anywhere unlocks import or authority. The embedded
    // Cypher dialect does not support alternation between label expressions,
    // so each candidate plane is checked explicitly.
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
            "no {label} is import-eligible"
        );
        assert_eq!(
            query_count(
                &store,
                &format!("MATCH (c:{label}) WHERE c.authoritative = true RETURN count(c) AS authoritative")
            )
            .await,
            0,
            "no {label} is graph truth"
        );
    }
    // No duplicates after replay: exactly one candidate per type.
    assert_eq!(
        query_count(
            &store,
            "MATCH (c:FigureCandidate) RETURN count(c) AS figures"
        )
        .await,
        1,
        "figure replay did not duplicate nodes"
    );
    assert_eq!(
        query_count(&store, "MATCH (c:TableCandidate) RETURN count(c) AS tables").await,
        1,
        "table replay did not duplicate nodes"
    );
    assert_eq!(
        query_count(
            &store,
            "MATCH (c:EquationCandidate) RETURN count(c) AS equations"
        )
        .await,
        1,
        "equation replay did not duplicate nodes"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// NEGATIVE CONTROL: a cached output cannot bypass the admission gate
// ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cached_mid_fsm_entry_cannot_bypass_admission_gate() {
    // Seed a durable cache hit for figure_analysis (exact canonical key).
    let registry = data_registry();
    let cache = Arc::new(MemoryPipelineCache::new());
    let analysis = Arc::new(FakeStage::new(
        "figure_analysis",
        b"cached-analysis",
        "schema_registry=figure-schemas.v1",
        Some("route|anthropic_messages|vision|fallback=false"),
        "FigureAnalysis@serde-v1",
    ));
    let key = StageCacheKey {
        source: source(&sha(1)),
        stage_id: "figure_analysis".into(),
        dependency_output_sha256: BTreeMap::new(),
        implementation_fingerprint: analysis.contract().implementation_fingerprint.clone(),
        configuration_fingerprint: analysis.contract().configuration_fingerprint.clone(),
        prompt_fingerprint: analysis.contract().prompt_fingerprint.clone(),
        model_fingerprint: analysis.contract().model_fingerprint.clone(),
        output_schema_fingerprint: analysis.contract().output_schema_fingerprint.clone(),
    };
    cache
        .store_success(
            &key,
            b"cached-analysis",
            BTreeMap::new(),
            "2026-08-08T00:00:00Z",
        )
        .expect("seed cached hit");

    // The run starts mid-FSM (first step = figure_analysis). The gate must
    // block BEFORE cache acceptance: record is Failed, executor never runs.
    let mid_fsm = PipelineBuilder::new("mid-fsm-entry")
        .step("analysis", "figure_analysis", &[])
        .build()
        .unwrap();
    let orchestrator = PipelineOrchestrator::new(cache.clone(), clock(), registry)
        .with_executor("figure_analysis", analysis.clone())
        .expect("register executor");
    let report = orchestrator
        .run(&mid_fsm, source(&sha(1)))
        .await
        .expect("run");
    assert!(!report.completed);
    assert_eq!(report.stages.len(), 1);
    assert!(matches!(
        report.stages[0].disposition,
        StageDisposition::Failed {
            retryable: false,
            ..
        }
    ));
    assert_eq!(
        report.stages[0].cache_key_sha256, None,
        "blocked admission must not accept the cached hit"
    );
    let admission = report.stages[0]
        .admission
        .as_ref()
        .expect("blocked record carries admission");
    assert!(!admission.admitted);
    let error = admission.error.as_ref().expect("typed error");
    assert_eq!(error.error_class(), "admission:illegal_transition");
    assert_eq!(error.from, da_domain::ArticleContextState::SourceRegistered);
    assert_eq!(error.to, da_domain::ArticleContextState::Extracted);
    assert_eq!(
        analysis.calls(),
        0,
        "blocked transition must never reach the executor"
    );
    // The seeded cache entry was never consumed as a hit.
    assert!(matches!(
        cache.lookup(&key, "2026-08-08T00:00:00Z").unwrap(),
        CacheLookup::Hit(_)
    ));
}
