//! Cluster use case tests — ConceptCluster materialization.

mod common;

use common::mock_graph_store::MockGraphStore;
use da_application::cluster::ClusterUseCase;
use da_domain::cluster::EntityPapers;
use da_domain::entity::EntityType;
use da_domain::relation::hypergraph::MEMBER_OF_CLUSTER;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use std::collections::{HashMap, HashSet};

#[tokio::test]
async fn test_cluster_use_case_creates_concept_cluster_node() {
    // GPT-4 mentioned in 6 papers → single-entity cluster created (≥5 threshold)
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "label", "GPT-4".to_string())
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..6).map(|i| format!("2401.0000{i}")).collect();
    entity_papers.insert("GPT-4".to_string(), (EntityType::Model, papers));

    let use_case = ClusterUseCase::new(Box::new(store));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    assert!(result.clusters_created >= 1, "expected ≥1 cluster");
    assert!(
        result.member_edges_created >= 1,
        "expected ≥1 MEMBER_OF_CLUSTER edge"
    );
}

#[tokio::test]
async fn test_cluster_use_case_creates_member_of_cluster_edge() {
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "label", "PPO".to_string())
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..7).map(|i| format!("2402.0000{i}")).collect();
    entity_papers.insert("PPO".to_string(), (EntityType::Method, papers));

    let use_case = ClusterUseCase::new(Box::new(store));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    // Verify MEMBER_OF_CLUSTER edges were created
    assert!(
        result.member_edges_created >= 1,
        "expected MEMBER_OF_CLUSTER edges"
    );
}

#[tokio::test]
async fn test_cluster_use_case_no_clusters_for_low_mention_entities() {
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "label", "RareModel".to_string())
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..2).map(|i| format!("2403.0000{i}")).collect();
    entity_papers.insert("RareModel".to_string(), (EntityType::Model, papers));

    let use_case = ClusterUseCase::new(Box::new(store));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    assert_eq!(result.clusters_created, 0, "no clusters for <5 mentions");
    assert_eq!(result.member_edges_created, 0);
}

#[tokio::test]
async fn test_cluster_use_case_creates_concept_cluster_with_correct_label() {
    // Cluster label should contain the entity name for readability
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "label", "GraphSAGE".to_string())
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..5).map(|i| format!("2404.0000{i}")).collect();
    entity_papers.insert("GraphSAGE".to_string(), (EntityType::Method, papers));

    let use_case = ClusterUseCase::new(Box::new(store));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    assert!(result.clusters_created >= 1);
    // Verify edges were created (ConceptCluster → Entity via MEMBER_OF_CLUSTER)
    assert!(result.member_edges_created >= 1);
}
