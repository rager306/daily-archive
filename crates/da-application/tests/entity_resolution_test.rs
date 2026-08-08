use std::sync::Arc;

use chrono::{TimeZone, Utc};
use da_application::{EntityResolutionError, EntityResolutionUseCase, ReviewedMergeRequest};
use da_domain::healing::HealingActor;
use da_ports::FixedClock;
use da_ports::graph_store::DirectGraphStore;

mod common;
use common::mock_graph_store::MockGraphStore;

async fn entity(store: &MockGraphStore, vid: &str, entity_type: &str, label: &str) -> u64 {
    let node = store.create_node("Entity").await.unwrap();
    for (key, value) in [("vid", vid), ("entity_type", entity_type), ("label", label)] {
        store
            .set_node_property_string(node, key, value.into())
            .await
            .unwrap();
    }
    store
        .set_node_property_bool(node, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(node, "schema_version", 1)
        .await
        .unwrap();
    node
}

fn use_case(store: &MockGraphStore) -> EntityResolutionUseCase {
    let shared: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    EntityResolutionUseCase::new(shared)
        .unwrap()
        .with_clock(Arc::new(FixedClock(
            Utc.with_ymd_and_hms(2026, 8, 8, 12, 0, 0).unwrap(),
        )))
}

#[tokio::test]
async fn blocking_is_data_driven_same_type_and_candidate_only() {
    let store = MockGraphStore::new();
    entity(&store, "vid:a", "Theorem", " Bayes'  Rule ").await;
    entity(&store, "vid:b", "Theorem", "bayes' rule").await;
    entity(&store, "vid:c", "Method", "bayes' rule").await;

    let result = use_case(&store).candidates().await;

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].left_vid, "vid:a");
    assert_eq!(result.candidates[0].right_vid, "vid:b");
    assert!(store.get_nodes_by_label("Decision").await.is_empty());
    for node in store.get_nodes_by_label("Entity").await {
        let snapshot = store.snapshot_node(node).unwrap();
        assert_eq!(
            snapshot
                .get("import_eligible")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            snapshot
                .get("retrieval_eligible")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }
}

#[tokio::test]
async fn reviewed_merge_records_decision_links_and_replays_idempotently() {
    let store = MockGraphStore::new();
    let keep = entity(&store, "vid:keep", "Model", "GPT-4").await;
    let merge = entity(&store, "vid:merge", "Model", "gpt4").await;
    let request = || ReviewedMergeRequest {
        keep_vid: "vid:keep".into(),
        merge_vid: "vid:merge".into(),
        reason: "Human reviewed punctuation-only alias".into(),
        actor: HealingActor::Human("alice".into()),
    };
    let resolver = use_case(&store);

    let first = resolver.resolve_reviewed_merge(request()).await.unwrap();
    let edge_count = store.edge_count().await;
    let second = resolver.resolve_reviewed_merge(request()).await.unwrap();
    let revised_reason = resolver
        .resolve_reviewed_merge(ReviewedMergeRequest {
            reason: "Operational replay with clarified wording".into(),
            ..request()
        })
        .await
        .unwrap();

    assert_eq!(first.decision_vid, second.decision_vid);
    assert_eq!(first.decision_vid, revised_reason.decision_vid);
    assert_eq!(first.decision_node_id, second.decision_node_id);
    assert_eq!(second.merge.edges_redirected, 0);
    assert_eq!(store.edge_count().await, edge_count);
    assert_eq!(store.get_nodes_by_label("Decision").await.len(), 1);
    assert_eq!(
        store
            .get_node_property_string(merge, "canonical_vid")
            .await
            .as_deref(),
        Some("vid:keep")
    );
    let merged_snapshot = store.snapshot_node(merge).unwrap();
    assert_eq!(
        merged_snapshot
            .get("retrieval_eligible")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        merged_snapshot
            .get("import_eligible")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        store
            .get_edges_between(
                first.decision_node_id,
                keep,
                da_domain::relation::decision::AFFECTS,
            )
            .await
            .len(),
        1
    );
    let affects = store
        .get_edges_between(
            first.decision_node_id,
            merge,
            da_domain::relation::decision::AFFECTS,
        )
        .await;
    assert_eq!(affects.len(), 1);
    for field in ["valid_at", "created_at"] {
        let value = store
            .get_edge_property_string(affects[0].0, field)
            .await
            .expect(field);
        assert!(chrono::DateTime::parse_from_rfc3339(&value).is_ok());
    }
    store.assert_graph_conforms("reviewed_merge_records_decision_links_and_replays_idempotently");
}

#[tokio::test]
async fn partial_silence_and_same_type_non_candidate_are_recoverable_or_rejected() {
    let store = MockGraphStore::new();
    entity(&store, "vid:keep", "Model", "GPT-4").await;
    let merge = entity(&store, "vid:merge", "Model", "gpt4").await;
    store
        .set_node_property_bool(merge, "retrieval_eligible", false)
        .await
        .unwrap();
    let resolver = use_case(&store);
    resolver
        .resolve_reviewed_merge(ReviewedMergeRequest {
            keep_vid: "vid:keep".into(),
            merge_vid: "vid:merge".into(),
            reason: "Recover interrupted silence-before-canonical state".into(),
            actor: HealingActor::Human("alice".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .get_node_property_string(merge, "canonical_vid")
            .await
            .as_deref(),
        Some("vid:keep")
    );

    entity(&store, "vid:other", "Model", "unrelated model").await;
    let error = resolver
        .resolve_reviewed_merge(ReviewedMergeRequest {
            keep_vid: "vid:keep".into(),
            merge_vid: "vid:other".into(),
            reason: "Not a configured candidate".into(),
            actor: HealingActor::Human("alice".into()),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, EntityResolutionError::NotCandidate));
}

#[tokio::test]
async fn non_candidate_and_cross_type_fail_before_decision() {
    let store = MockGraphStore::new();
    entity(&store, "vid:a", "Method", "LoRA").await;
    entity(&store, "vid:b", "Dataset", "LoRA").await;
    let resolver = use_case(&store);
    let error = resolver
        .resolve_reviewed_merge(ReviewedMergeRequest {
            keep_vid: "vid:a".into(),
            merge_vid: "vid:b".into(),
            reason: "invalid cross-type merge".into(),
            actor: HealingActor::Human("alice".into()),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, EntityResolutionError::TypeMismatch { .. }));
    assert!(store.get_nodes_by_label("Decision").await.is_empty());
    assert!(
        store
            .get_node_property_string(
                store
                    .find_node_by_string_property("Entity", "vid", "vid:b")
                    .await
                    .unwrap(),
                "canonical_vid",
            )
            .await
            .is_none()
    );
}
