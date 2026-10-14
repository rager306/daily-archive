//! Content vector declarations at the concrete CLI composition boundary.

use std::sync::Arc;

use da_adapters::SamyamaGraphStore;
use da_application::{IngestUseCase, RevisionContentContext, RevisionManifestationInput};
use da_cli::production::ensure_embedding_index;
use da_ports::FixedClock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, Section};
use samyama::vector::index::{DistanceMetric, Quantization};

fn article() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2507.19457".into(),
        title: "Embedding composition".into(),
        abstract_text: "Abstract".into(),
        body_text: "Body".into(),
        pdf_hash: "7".repeat(64),
        sections: vec![Section {
            title: "Introduction".into(),
            text: "Body".into(),
            level: 1,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn revision() -> RevisionContentContext {
    RevisionContentContext {
        source_code: "arxiv".into(),
        canonical_id: "2507.19457".into(),
        revision: 1,
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: "7".repeat(64),
            bytes: 1000,
            uri: "catalog:2507.19457v1.pdf".into(),
            path: "2507.19457v1.pdf".into(),
            capture_status: "captured_local".into(),
        }],
    }
}

#[tokio::test]
async fn missing_declaration_remains_a_materialization_error() {
    let store = Arc::new(SamyamaGraphStore::new());
    let materializer = IngestUseCase::materializer_only_with_clock(
        store.clone(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );
    let parsed = article();
    assert!(
        materializer
            .materialize_parsed_with_context(
                &parsed,
                vec![0.1, 0.2, 0.3],
                None,
                Some("arxiv".into()),
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                None,
            )
            .await
            .is_err()
    );
    let paper = store
        .find_node_by_string_property("Paper", "arxiv_id", &parsed.paper_id)
        .await
        .unwrap();
    assert!(
        store
            .get_node_property_string(paper, "embedding_sha256")
            .await
            .is_none()
    );
    assert!(
        store
            .store_read()
            .await
            .vector_index
            .get_index("Paper", "embedding")
            .is_none()
    );
}

#[tokio::test]
async fn declared_paper_and_revision_vectors_materialize_and_replay() {
    let parsed = article();
    let revision = revision();
    let vector = vec![0.1, 0.2, 0.3];
    for context in [None, Some(&revision)] {
        let label = if context.is_some() {
            "PaperRevision"
        } else {
            "Paper"
        };
        let store = Arc::new(SamyamaGraphStore::new());
        ensure_embedding_index(store.as_ref(), label, vector.len())
            .await
            .unwrap();
        let materializer = IngestUseCase::materializer_only_with_clock(
            store.clone(),
            Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        );
        let mut first_sha = None;
        let mut counts = None;
        for _ in 0..2 {
            let result = materializer
                .materialize_parsed_with_context(
                    &parsed,
                    vector.clone(),
                    None,
                    Some("arxiv".into()),
                    None,
                    da_domain::PdfTriageStatus::NotConfigured,
                    context,
                )
                .await
                .unwrap();
            assert!(!result.import_eligible);
            let node = if let Some(context) = context {
                store
                    .find_node_by_string_property(label, "vid", &context.revision_vid().unwrap())
                    .await
                    .unwrap()
            } else {
                store
                    .find_node_by_string_property(label, "arxiv_id", &parsed.paper_id)
                    .await
                    .unwrap()
            };
            assert_eq!(
                store.get_node_property_bool(node, "import_eligible").await,
                Some(false)
            );
            let sha = store
                .get_node_property_string(node, "embedding_sha256")
                .await
                .unwrap();
            if let Some(first) = &first_sha {
                assert_eq!(&sha, first);
            } else {
                first_sha = Some(sha);
            }
            let current_counts = (store.node_count().await, store.edge_count().await);
            if let Some(first) = counts {
                assert_eq!(current_counts, first);
            } else {
                counts = Some(current_counts);
            }
            let hits = DirectGraphStore::vector_search_direct(
                store.as_ref(),
                label,
                "embedding",
                &vector,
                10,
            )
            .await
            .unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].0, node);
            let graph = store.store_read().await;
            let index = graph.vector_index.get_index(label, "embedding").unwrap();
            let declaration = index.read().unwrap();
            assert_eq!(declaration.dimensions(), vector.len());
            assert_eq!(declaration.metric(), DistanceMetric::Cosine);
            assert_eq!(declaration.len(), 1);
        }
    }
}

#[tokio::test]
async fn existing_declaration_is_preserved_and_dimension_conflicts_refuse_before_writes() {
    let store = SamyamaGraphStore::new();
    let original_index = {
        let graph = store.store_write().await;
        graph
            .vector_index
            .create_index_named(
                Some("content-vectors"),
                "PaperRevision",
                "embedding",
                2,
                DistanceMetric::L2,
                Quantization::Fp16,
            )
            .unwrap();
        assert!(
            graph
                .vector_index
                .set_model_id("PaperRevision", "embedding", "fixture-model")
        );
        graph
            .vector_index
            .get_index("PaperRevision", "embedding")
            .unwrap()
    };
    let node = store.create_node("PaperRevision").await.unwrap();
    let vector = vec![1.0, 0.0];
    store
        .add_vector("PaperRevision", "embedding", node, vector.clone())
        .await
        .unwrap();
    let before_counts = (store.node_count().await, store.edge_count().await);

    assert!(
        ensure_embedding_index(&store, "PaperRevision", 3)
            .await
            .is_err()
    );
    ensure_embedding_index(&store, "PaperRevision", 2)
        .await
        .unwrap();
    assert_eq!(
        (store.node_count().await, store.edge_count().await),
        before_counts
    );
    {
        let graph = store.store_read().await;
        let index = graph
            .vector_index
            .get_index("PaperRevision", "embedding")
            .unwrap();
        assert!(Arc::ptr_eq(&index, &original_index));
        let declaration = index.read().unwrap();
        assert_eq!(declaration.dimensions(), 2);
        assert_eq!(declaration.metric(), DistanceMetric::L2);
        assert_eq!(declaration.quantization(), Quantization::Fp16);
        assert_eq!(declaration.len(), 1);
        assert_eq!(
            graph
                .vector_index
                .model_id("PaperRevision", "embedding")
                .as_deref(),
            Some("fixture-model")
        );
        assert_eq!(
            graph.vector_index.resolve_name("content-vectors"),
            Some(("PaperRevision".into(), "embedding".into()))
        );
    }
    let hits =
        DirectGraphStore::vector_search_direct(&store, "PaperRevision", "embedding", &vector, 10)
            .await
            .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, node);
}
