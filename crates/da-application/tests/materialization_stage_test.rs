//! M403 S02 — real materialization stage executors over a mock store.
//!
//! Proves the two new `PipelineStageExecutor` impls delegate to the existing
//! idempotent use cases: first execute creates revision/entity content, the
//! identical second execute reuses with zero new writes, payloads are typed
//! serde reports, and every metadata surface carries D127 false.

mod common;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::mock_graph_store::MockGraphStore;
use da_application::candidate_materialization_stage::CandidateMaterializationReport;
use da_application::candidate_materialization_stage::CandidateMaterializationStage;
use da_application::equation_extraction_stage::EquationExtractionArtifact;
use da_application::revision_materialization::RevisionManifestationInput;
use da_application::{
    ExtractionMaterializationReport, ExtractionMaterializationStage, RevisionMaterializationReport,
    RevisionMaterializationStage,
};
use da_domain::entity::EntityType;
use da_ports::clock::{Clock, FixedClock};
use da_ports::extractor::ExtractedEntity;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, Section};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageExecutionError, StageExecutionRequest};

const PAPER_ID: &str = "2507.19457";

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
        pdf_hash: format!("{:064x}", 1),
    }
}

fn manifestations() -> Vec<RevisionManifestationInput> {
    vec![RevisionManifestationInput {
        format: "pdf".into(),
        sha256: format!("{:064x}", 1),
        bytes: 1000,
        uri: format!("https://arxiv.org/pdf/{PAPER_ID}v1"),
        path: format!("{PAPER_ID}v1.pdf"),
        capture_status: "captured".into(),
    }]
}

fn request(deps: &[(&str, Vec<u8>)]) -> StageExecutionRequest {
    StageExecutionRequest {
        article_id: PAPER_ID.into(),
        dependency_payloads: deps
            .iter()
            .map(|(name, payload)| ((*name).to_owned(), payload.clone()))
            .collect(),
    }
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

#[tokio::test]
async fn revision_stage_creates_then_reuses_with_zero_new_writes() {
    let store = Arc::new(MockGraphStore::new());
    let stage = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let contract = stage.contract();
    assert_eq!(contract.stage_id, "revision_materialization");
    assert!(!contract.configuration_fingerprint.is_empty());

    let parse_payload = serde_json::to_vec(&parsed_article()).expect("parsed serialize");
    let first = stage
        .execute(request(&[("parse", parse_payload.clone())]))
        .await
        .expect("first revision materialization");
    let first_report: RevisionMaterializationReport =
        serde_json::from_slice(&first.payload).expect("report decodes");
    assert!(first_report.result.nodes_created > 0, "first run creates");
    assert_eq!(first_report.result.import_eligible, false);
    assert_eq!(
        first.metadata.get("import_eligible").map(String::as_str),
        Some("false")
    );
    let nodes_after_first = store.node_count_total();

    let second = stage
        .execute(request(&[("parse", parse_payload)]))
        .await
        .expect("second revision materialization");
    let second_report: RevisionMaterializationReport =
        serde_json::from_slice(&second.payload).expect("report decodes");
    assert_eq!(
        second_report.result.nodes_created, 0,
        "replay creates nothing"
    );
    assert!(
        second_report.result.nodes_reused > 0,
        "replay reuses the established plane"
    );
    assert_eq!(
        store.node_count_total(),
        nodes_after_first,
        "replay leaves the graph unchanged"
    );

    // Cached payload validation accepts the canonical report shape.
    stage
        .validate_cached_payload(&second.payload)
        .expect("report is a valid cached payload");
}

#[tokio::test]
async fn revision_stage_stamps_d134_retrieval_flags_on_every_label() {
    // D134 (MEM480 parity with the legacy ingest path): every materialized
    // node carries retrieval_eligible. Paper/PaperRevision/Section are live
    // for retrieval; SourceManifestation is evidence-plane and stays false
    // (mirroring the ingest source surrogate). Caught live by the M405 soak:
    // `MATCH (n:Paper) WHERE n.retrieval_eligible = true` returned 0.
    let store = Arc::new(MockGraphStore::new());
    let stage = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let parse_payload = serde_json::to_vec(&parsed_article()).expect("parsed serialize");
    let first = stage
        .execute(request(&[("parse", parse_payload)]))
        .await
        .expect("revision materialization");

    async fn flag_of(store: &MockGraphStore, label: &str, key: &str, value: &str) -> Option<bool> {
        let node = store
            .find_node_by_string_property(label, key, value)
            .await
            .unwrap_or_else(|| panic!("no {label} node found by {key}={value}"));
        store
            .get_node_property_bool(node, "retrieval_eligible")
            .await
    }

    let report: RevisionMaterializationReport =
        serde_json::from_slice(&first.payload).expect("report decodes");
    let paper_vid = report.result.paper_vid.clone();
    let revision_vid = report.result.revision_vid.clone();
    let manifestation_sha = manifestations()[0].sha256.clone();

    assert_eq!(
        flag_of(&store, "Paper", "arxiv_id", PAPER_ID).await,
        Some(true),
        "Paper is retrieval-live (D134)"
    );
    assert_eq!(
        flag_of(&store, "PaperRevision", "vid", &revision_vid).await,
        Some(true),
        "PaperRevision content is retrieval-live (D134)"
    );
    assert_eq!(
        flag_of(&store, "Section", "section_id", "sec-0").await,
        Some(true),
        "Sections are retrieval-live (D134)"
    );
    assert_eq!(
        flag_of(&store, "SourceManifestation", "sha256", &manifestation_sha).await,
        Some(false),
        "SourceManifestation is evidence-plane, not retrieval-live"
    );
}

#[tokio::test]
async fn extraction_stage_materializes_entities_after_revision_plane_exists() {
    let store = Arc::new(MockGraphStore::new());
    // Seed the revision plane first: extraction links entities to the Paper.
    let revision = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let parse_payload = serde_json::to_vec(&parsed_article()).expect("parsed serialize");
    revision
        .execute(request(&[("parse", parse_payload.clone())]))
        .await
        .expect("revision plane");

    let stage = ExtractionMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);
    let contract = stage.contract();
    assert_eq!(contract.stage_id, "extraction_materialization");

    let first = stage
        .execute(request(&[
            ("parse", parse_payload.clone()),
            ("extract", entities_payload()),
        ]))
        .await
        .expect("first extraction materialization");
    let first_report: ExtractionMaterializationReport =
        serde_json::from_slice(&first.payload).expect("report decodes");
    assert_eq!(first_report.entities_created, 2);
    assert!(first_report.mentions_edges > 0);
    assert_eq!(first_report.import_eligible, false);
    let nodes_after_first = store.node_count_total();

    let second = stage
        .execute(request(&[
            ("parse", parse_payload),
            ("extract", entities_payload()),
        ]))
        .await
        .expect("second extraction materialization");
    let second_report: ExtractionMaterializationReport =
        serde_json::from_slice(&second.payload).expect("report decodes");
    assert_eq!(
        second_report.entities_created, 0,
        "replay creates no duplicate entities"
    );
    assert_eq!(
        store.node_count_total(),
        nodes_after_first,
        "replay leaves the graph unchanged"
    );
    assert_eq!(
        second.metadata.get("import_eligible").map(String::as_str),
        Some("false")
    );
    stage
        .validate_cached_payload(&second.payload)
        .expect("report is a valid cached payload");
}

#[tokio::test]
async fn stages_fail_closed_on_missing_dependencies_and_identity_mismatch() {
    let store = Arc::new(MockGraphStore::new());
    let revision = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let err = revision.execute(request(&[])).await.unwrap_err();
    assert!(matches!(
        err,
        StageExecutionError::Terminal {
            error_class,
            ..
        } if error_class == "missing_parsed_dependency"
    ));

    let mut other = parsed_article();
    other.paper_id = "9999.99999".into();
    let payload = serde_json::to_vec(&other).expect("serialize");
    let err = revision
        .execute(request(&[("parse", payload)]))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        StageExecutionError::Terminal {
            error_class,
            ..
        } if error_class == "article_identity_mismatch"
    ));
}

// ── M407 S02: candidate materialization over the equation family ────────────

fn equation_candidate(revision_vid: &str) -> da_domain::equation_candidate::EquationCandidate {
    let formula = da_ports::parser::ExtractedFormula {
        formula_id: "formula_0".into(),
        section_path_id: Some("1".into()),
        section_ordinal: Some(1),
        label: Some("(1)".into()),
        coordinates: Some("10,20,30,40;5".into()),
        representation: da_ports::parser::FormulaRepresentation::TeXMath("L=x".into()),
        span: Some(da_ports::parser::CharSpan { start: 1, end: 4 }),
        source_digest: format!("{:064x}", 7),
        missingness: None,
    };
    let sections: Vec<da_ports::parser::Section> = (0..3)
        .map(|ordinal| da_ports::parser::Section {
            title: format!("Section {ordinal}"),
            text: format!("text {ordinal}"),
            level: 1,
            path_id: Some(format!("{ordinal}")),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        })
        .collect();
    let input = da_application::equation_materialize::equation_candidate_input_from_extracted(
        &formula,
        revision_vid,
        &format!("{:064x}", 7),
        &sections,
    )
    .expect("conversion");
    da_domain::equation_candidate::EquationCandidate::new(input).expect("candidate")
}

fn equations_payload(candidates: Vec<da_domain::equation_candidate::EquationCandidate>) -> Vec<u8> {
    serde_json::to_vec(&EquationExtractionArtifact {
        candidates,
        missingness: None,
    })
    .expect("artifact serializes")
}

#[tokio::test]
async fn candidate_stage_writes_equations_then_replays_unchanged() {
    // The revision plane must exist first (candidates ground into its
    // sections), mirroring the journey topology.
    let store = Arc::new(MockGraphStore::new());
    let revision = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let parse_payload = serde_json::to_vec(&parsed_article()).expect("parsed serialize");
    let revision_out = revision
        .execute(request(&[("parse", parse_payload)]))
        .await
        .expect("revision plane");
    let revision_report: RevisionMaterializationReport =
        serde_json::from_slice(&revision_out.payload).expect("revision report");
    let revision_vid = revision_report.result.revision_vid.clone();

    let stage = CandidateMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);
    let payload = equations_payload(vec![
        equation_candidate(&revision_vid),
        equation_candidate(&revision_vid),
    ]);
    let first = stage
        .execute(request(&[("equations", payload.clone())]))
        .await
        .expect("first candidate materialization");
    let first_report: CandidateMaterializationReport =
        serde_json::from_slice(&first.payload).expect("report decodes");
    let equation_row = first_report
        .families
        .iter()
        .find(|row| row.family == "equation")
        .expect("equation row");
    assert_eq!(equation_row.status, "applied");
    assert_eq!(equation_row.candidates, 2);
    assert!(equation_row.nodes_created > 0);
    assert!(!first_report.import_eligible);
    let nodes_after_first = store.node_count_total();

    // Identical replay: unchanged, zero new writes.
    let second = stage
        .execute(request(&[("equations", payload)]))
        .await
        .expect("replay candidate materialization");
    let second_report: CandidateMaterializationReport =
        serde_json::from_slice(&second.payload).expect("report decodes");
    let replay_row = second_report
        .families
        .iter()
        .find(|row| row.family == "equation")
        .expect("equation row");
    assert_eq!(replay_row.status, "unchanged");
    assert_eq!(replay_row.nodes_created, 0, "replay creates nothing");
    assert_eq!(
        store.node_count_total(),
        nodes_after_first,
        "replay leaves the graph unchanged"
    );

    // Deferred families are typed, never fabricated.
    for family in ["figure", "table"] {
        let row = second_report
            .families
            .iter()
            .find(|row| row.family == family)
            .expect("family row");
        assert_eq!(row.status, "not_attempted");
        assert_eq!(row.candidates, 0);
        assert!(row.reason.as_deref().unwrap_or("").starts_with("deferred"));
    }
    assert_eq!(
        second.metadata.get("import_eligible").map(String::as_str),
        Some("false")
    );
    stage
        .validate_cached_payload(&second.payload)
        .expect("report is a valid cached payload");
}

#[tokio::test]
async fn candidate_stage_empty_artifact_is_typed_not_attempted() {
    let store = Arc::new(MockGraphStore::new());
    let stage = CandidateMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);
    let payload = serde_json::to_vec(&EquationExtractionArtifact {
        candidates: vec![],
        missingness: Some("parser_reported:NoFormulaElements".into()),
    })
    .expect("artifact serializes");
    let out = stage
        .execute(request(&[("equations", payload)]))
        .await
        .expect("empty candidate run");
    let report: CandidateMaterializationReport =
        serde_json::from_slice(&out.payload).expect("report decodes");
    let equation_row = report
        .families
        .iter()
        .find(|row| row.family == "equation")
        .expect("equation row");
    assert_eq!(equation_row.status, "not_attempted");
    assert!(
        equation_row
            .reason
            .as_deref()
            .unwrap_or("")
            .starts_with("parser_reported:")
    );
    assert_eq!(store.node_count_total(), 0, "nothing written");
}

// ── M408 S02: table family in candidate materialization ────────────────────

fn table_candidate(revision_vid: &str) -> da_domain::table_candidate::TableCandidate {
    let lineage = da_domain::table_candidate::TableCandidateLineage {
        revision_vid: revision_vid.into(),
        artifact_id: "Table1".into(),
        detector_name: "pdffigures2".into(),
        detector_fingerprint: "pdffigures2-0.0.12".into(),
        image_sha256: format!("{:064x}", 9),
        asset_path: "figures/Table1.png".into(),
        page_index: 3,
        section_vid: da_domain::vid::revision_section_vid(revision_vid, 1)
            .expect("section vid")
            .to_string(),
        section_title: "Section 1".into(),
        section_ordinal: 1,
        caption: "Table 1: results.".into(),
        source_pdf_sha256: format!("{:064x}", 7),
    };
    let extracted = da_ports::parser::ExtractedTable {
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
            digest: format!("{:064x}", 5),
        }],
        source_digest: format!("{:064x}", 3),
        missingness: None,
    };
    let structural = da_application::table_materialize::structural_table_from_extracted(&extracted);
    let report = da_domain::table_fidelity::compare_table_fidelity(None, Some(&structural))
        .expect("fidelity");
    da_domain::table_candidate::TableCandidate::assemble(
        &lineage,
        None,
        &structural,
        &report,
        Some("10,20,500,300;3".into()),
    )
    .expect("assemble")
}

fn tables_payload(candidates: Vec<da_domain::table_candidate::TableCandidate>) -> Vec<u8> {
    serde_json::to_vec(
        &da_application::table_extraction_stage::TableCandidateArtifact {
            candidates,
            join_rows: vec![],
            missingness: None,
        },
    )
    .expect("artifact serializes")
}

#[tokio::test]
async fn candidate_stage_writes_tables_then_replays_unchanged() {
    let store = Arc::new(MockGraphStore::new());
    let revision = RevisionMaterializationStage::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        clock(),
        "arxiv",
        PAPER_ID,
        1,
        manifestations(),
    );
    let parse_payload = serde_json::to_vec(&parsed_article()).expect("parsed serialize");
    let revision_out = revision
        .execute(request(&[("parse", parse_payload)]))
        .await
        .expect("revision plane");
    let revision_report: RevisionMaterializationReport =
        serde_json::from_slice(&revision_out.payload).expect("revision report");
    let revision_vid = revision_report.result.revision_vid.clone();

    let stage = CandidateMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);
    let equation = equations_payload(vec![equation_candidate(&revision_vid)]);
    let tables = tables_payload(vec![table_candidate(&revision_vid)]);
    let deps = [("equations", equation), ("tables", tables)];
    let first = stage
        .execute(request(&deps))
        .await
        .expect("combined candidate run");
    let first_report: CandidateMaterializationReport =
        serde_json::from_slice(&first.payload).expect("report decodes");
    let table_row = first_report
        .families
        .iter()
        .find(|row| row.family == "table")
        .expect("table row");
    assert_eq!(table_row.status, "applied");
    assert_eq!(table_row.candidates, 1);
    assert!(table_row.nodes_created > 0);
    let equation_row = first_report
        .families
        .iter()
        .find(|row| row.family == "equation")
        .expect("equation row");
    assert_eq!(equation_row.status, "applied");
    let nodes_after_first = store.node_count_total();

    let equation2 = equations_payload(vec![equation_candidate(&revision_vid)]);
    let tables2 = tables_payload(vec![table_candidate(&revision_vid)]);
    let deps2 = [("equations", equation2), ("tables", tables2)];
    let second = stage
        .execute(request(&deps2))
        .await
        .expect("combined replay");
    let second_report: CandidateMaterializationReport =
        serde_json::from_slice(&second.payload).expect("report decodes");
    for family in ["table", "equation"] {
        let row = second_report
            .families
            .iter()
            .find(|row| row.family == family)
            .expect("row");
        assert_eq!(row.status, "unchanged", "{family} replay unchanged");
        assert_eq!(row.nodes_created, 0);
    }
    assert_eq!(
        store.node_count_total(),
        nodes_after_first,
        "combined replay leaves the graph unchanged"
    );
    // Figure family stays honestly deferred.
    let figure_row = second_report
        .families
        .iter()
        .find(|row| row.family == "figure")
        .expect("figure row");
    assert_eq!(figure_row.status, "not_attempted");
}

#[tokio::test]
async fn candidate_stage_without_tables_dep_keeps_typed_deferred_row() {
    let store = Arc::new(MockGraphStore::new());
    let stage = CandidateMaterializationStage::new(store.clone() as Arc<dyn DirectGraphStore>);
    let out = stage
        .execute(request(&[("equations", equations_payload(vec![]))]))
        .await
        .expect("run without tables dep");
    let report: CandidateMaterializationReport =
        serde_json::from_slice(&out.payload).expect("report decodes");
    let table_row = report
        .families
        .iter()
        .find(|row| row.family == "table")
        .expect("table row");
    assert_eq!(table_row.status, "not_attempted");
    assert!(
        table_row
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("deferred")
    );
}
