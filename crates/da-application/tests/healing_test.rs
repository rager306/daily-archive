//! Integration tests for GraphHealingUseCase.

#![cfg(test)]

use da_application::GraphHealingUseCase;
use da_domain::healing::HealingActor;
use da_ports::graph_store::DirectGraphStore;

mod common;

use common::mock_graph_store::MockGraphStore;

fn make_store() -> MockGraphStore {
    MockGraphStore::new()
}

async fn make_entity(store: &MockGraphStore, vid: &str, label: &str) -> u64 {
    let node = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(node, "vid", vid.to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "label", label.to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "retrieval_eligible", true)
        .await
        .unwrap();
    node
}

#[tokio::test]
async fn test_silence_sets_retrieval_eligible_false() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store));

    let result = use_case
        .silence(
            "vid:entity:Method:GPT",
            "Entity",
            "wrong extraction",
            HealingActor::System("extractor".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.vid, "vid:entity:Method:GPT");
    assert!(result.previous_eligible);
    assert_eq!(
        result.provenance.operation,
        da_domain::healing::HealingOperation::Silence
    );
}

#[tokio::test]
async fn test_merge_creates_supersedes_edge() {
    let store = make_store();
    let _keep_id = make_entity(&store, "vid:entity:Method:Transformer", "Transformer").await;
    let merge_id = make_entity(&store, "vid:entity:Method:Transformers", "Transformers").await;
    // Simulate a Paper mentioning the merge target
    let paper_id = store.create_node("Paper").await.unwrap();
    store
        .create_edge(
            paper_id,
            merge_id,
            da_domain::relation::bibliographic::MENTIONS,
        )
        .await
        .unwrap();
    let use_case = GraphHealingUseCase::new(Box::new(store));

    let result = use_case
        .merge(
            "vid:entity:Method:Transformer",
            "vid:entity:Method:Transformers",
            "same entity, different surface forms",
            HealingActor::Human("alice".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.kept_vid, "vid:entity:Method:Transformer");
    assert_eq!(result.merged_vid, "vid:entity:Method:Transformers");
    // SUPERSEDES edge + redirected MENTIONS edge = 2 new edges + original MENTIONS
    // total edges: 1 (original MENTIONS) + 1 (SUPERSEDES) + 1 (redirected MENTIONS) = 3
    assert_eq!(result.edges_redirected, 1);
    let edges = use_case.graph_store.edge_count().await;
    assert!(edges >= 3);
}

#[tokio::test]
async fn test_correct_updates_property() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store));

    let result = use_case
        .correct(
            "vid:entity:Method:GPT",
            "Entity",
            "label",
            "GPT-4",
            "typo: should be GPT-4",
            HealingActor::Human("bob".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.key, "label");
    assert_eq!(result.old_value, "GPT"); // captured from graph, not "unknown"
    assert_eq!(result.new_value, "GPT-4");
    assert_eq!(
        result.provenance.operation,
        da_domain::healing::HealingOperation::Correct
    );
}

#[tokio::test]
async fn test_silence_not_found_errors() {
    let store = make_store();
    let use_case = GraphHealingUseCase::new(Box::new(store));

    let result = use_case
        .silence(
            "vid:entity:Method:Nonexistent",
            "Entity",
            "test",
            HealingActor::System("test".to_string()),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_unsilence_restores_retrieval() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store));

    // First silence
    use_case
        .silence(
            "vid:entity:Method:GPT",
            "Entity",
            "deprecated",
            HealingActor::System("test".to_string()),
        )
        .await
        .unwrap();

    // Then unsilence
    let result = use_case
        .unsilence(
            "vid:entity:Method:GPT",
            "Entity",
            HealingActor::Human("admin".to_string()),
        )
        .await
        .unwrap();

    assert!(!result.previous_eligible); // was false before unsilence
}
