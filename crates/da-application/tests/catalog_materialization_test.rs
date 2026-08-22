//! M440 S02 T03: integration tests for the materialization composition the
//! catalog_materialize_runner drives: ingest + extraction with a revision
//! context, then a full replay that must be a graph no-op. State-based per
//! ADR-051: assertions read the resulting graph, not the call sequence.

mod common;

use common::mock_graph_store::MockGraphStore;
use da_application::catalog_materialization::{
    PaperMaterializeEvent, PaperMaterializeFsm, PaperMaterializeState,
};
use da_application::revision_materialization::{
    RevisionContentContext, RevisionManifestationInput,
};
use da_application::{ExtractionUseCase, IngestUseCase};
use da_domain::PdfTriageStatus;
use da_ports::extractor::ExtractedEntity;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, Section};
use da_ports::{Clock, SystemClock};
use std::sync::Arc;

fn revision_context(paper_id: &str, revision: u32) -> RevisionContentContext {
    RevisionContentContext {
        source_code: "arxiv".to_string(),
        canonical_id: paper_id.to_string(),
        revision,
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".to_string(),
            sha256: "a".repeat(64),
            bytes: 1024,
            uri: "catalog:test.pdf".to_string(),
            path: "test.pdf".to_string(),
            capture_status: "captured_local".to_string(),
        }],
    }
}

fn make_parsed(paper_id: &str) -> ParsedArticle {
    ParsedArticle {
        paper_id: paper_id.to_string(),
        title: format!("Title of {paper_id}"),
        abstract_text: "Abstract".to_string(),
        body_text: "body".to_string(),
        sections: vec![Section {
            title: "Introduction".to_string(),
            text: "intro text".to_string(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
        pdf_hash: "a".repeat(64),
    }
}

fn make_entity(label: &str) -> ExtractedEntity {
    ExtractedEntity {
        label: label.to_string(),
        entity_type: da_domain::entity::EntityType::Method,
        section_title: "Introduction".to_string(),
        section_ordinal: Some(0),
        char_start: 0,
        char_end: label.len(),
        surface: label.to_string(),
    }
}

#[tokio::test]
async fn runner_composition_replay_is_graph_noop() {
    let store: Arc<dyn DirectGraphStore> = Arc::new(MockGraphStore::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ingest =
        IngestUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));
    let extraction =
        ExtractionUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));

    let parsed = make_parsed("2401.02001");
    let entities = vec![make_entity("graph neural network"), make_entity("pagerank")];
    let vector = vec![0.25f32; 8];
    let context = revision_context("2401.02001", 2);

    let mut fsm = PaperMaterializeFsm::new("2401.02001");
    fsm.apply(PaperMaterializeEvent::ScanDone {
        has_parse: true,
        has_extraction: true,
        has_embedding: true,
    })
    .unwrap();
    fsm.apply(PaperMaterializeEvent::BindingResolved { revision: 2 })
        .unwrap();

    // First pass.
    ingest
        .materialize_parsed_with_context(
            &parsed,
            vector.clone(),
            Some("cs-cl".to_string()),
            Some("arxiv".to_string()),
            None,
            PdfTriageStatus::NotConfigured,
            Some(&context),
        )
        .await
        .unwrap();
    let first = extraction
        .materialize_extracted_with_context(&parsed, entities.clone(), Some(&context))
        .await
        .unwrap();
    assert!(
        first.entities_created > 0,
        "first pass must create entities"
    );
    fsm.apply(PaperMaterializeEvent::MaterializeDone {
        entities_created: first.entities_created,
        mentions_edges: first.mentions_edges,
    })
    .unwrap();

    // Replay: identical inputs must not change the graph.
    let before = (store.node_count().await, store.edge_count().await);
    ingest
        .materialize_parsed_with_context(
            &parsed,
            vector,
            Some("cs-cl".to_string()),
            Some("arxiv".to_string()),
            None,
            PdfTriageStatus::NotConfigured,
            Some(&context),
        )
        .await
        .unwrap();
    extraction
        .materialize_extracted_with_context(&parsed, entities, Some(&context))
        .await
        .unwrap();
    let after = (store.node_count().await, store.edge_count().await);
    assert_eq!(before, after, "replay must be a graph no-op");

    fsm.apply(PaperMaterializeEvent::ReplayVerified { noop: true })
        .unwrap();
    assert!(matches!(
        fsm.state(),
        PaperMaterializeState::Completed {
            replay_noop: true,
            ..
        }
    ));
}

#[tokio::test]
async fn distinct_revisions_do_not_collide() {
    // Two papers at different revisions must both land in the same store
    // without interference (identity is revision-scoped).
    let store: Arc<dyn DirectGraphStore> = Arc::new(MockGraphStore::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ingest =
        IngestUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));
    let extraction =
        ExtractionUseCase::materializer_only_with_clock(Arc::clone(&store), Arc::clone(&clock));

    for (paper_id, revision) in [("2401.03001", 1u32), ("2401.03002", 5u32)] {
        let parsed = make_parsed(paper_id);
        let context = revision_context(paper_id, revision);
        ingest
            .materialize_parsed_with_context(
                &parsed,
                vec![0.5f32; 8],
                None,
                Some("arxiv".to_string()),
                None,
                PdfTriageStatus::NotConfigured,
                Some(&context),
            )
            .await
            .unwrap();
        extraction
            .materialize_extracted_with_context(
                &parsed,
                vec![make_entity("transformer")],
                Some(&context),
            )
            .await
            .unwrap();
    }
    assert!(store.node_count().await > 0);
}

mod materialization_indexes_tests {
    use super::common::mock_graph_store::MockGraphStore;
    use da_application::materialization_indexes::ensure_materialization_indexes;
    use da_ports::graph_store::DirectGraphStore;
    use std::sync::Arc;

    #[tokio::test]
    async fn ensure_creates_all_hot_path_pairs_on_mock_store() {
        let store: Arc<dyn DirectGraphStore> = Arc::new(MockGraphStore::new());
        let created = ensure_materialization_indexes(&store).await.unwrap();
        assert_eq!(
            created.len(),
            da_application::materialization_indexes::MATERIALIZATION_PROPERTY_INDEXES.len()
        );
    }
}
