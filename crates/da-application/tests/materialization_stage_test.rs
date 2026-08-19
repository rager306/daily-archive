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
