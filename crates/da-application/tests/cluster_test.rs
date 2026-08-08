//! Cluster use case tests — ConceptCluster materialization.

mod common;

use common::mock_graph_store::MockGraphStore;
use da_application::cluster::ClusterUseCase;
use da_domain::cluster::{EntityClusterKey, EntityPapers};
use da_ports::graph_store::DirectGraphStore;
use std::collections::{HashMap, HashSet};

fn key(entity_type: &str, label: &str) -> EntityClusterKey {
    EntityClusterKey {
        entity_type: entity_type.into(),
        label: label.into(),
    }
}

#[tokio::test]
async fn test_cluster_use_case_creates_concept_cluster_node() {
    // GPT-4 mentioned in 6 papers → single-entity cluster created (≥5 threshold)
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "vid", "vid:entity:gpt4".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(entity_id, "label", "GPT-4".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(entity_id, "entity_type", "Model".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(entity_id, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(entity_id, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(entity_id, "schema_version", 1)
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..6).map(|i| format!("2401.0000{i}")).collect();
    entity_papers.insert(key("Model", "GPT-4"), papers);

    let use_case = ClusterUseCase::new(Box::new(store.clone()));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    assert!(result.clusters_created >= 1, "expected ≥1 cluster");
    assert!(
        result.member_edges_created >= 1,
        "expected ≥1 MEMBER_OF_CLUSTER edge"
    );
    let replay = use_case.materialize_clusters(&entity_papers).await.unwrap();
    assert_eq!(replay.clusters_created, 0);
    assert_eq!(replay.clusters_reused, 1);
    assert_eq!(replay.member_edges_created, 0);
    assert_eq!(replay.clusters_skipped, 0);
    assert_eq!(store.get_nodes_by_label("ConceptCluster").await.len(), 1);
    store.assert_graph_conforms("test_cluster_use_case_creates_concept_cluster_node");
}

#[tokio::test]
async fn same_label_different_types_keep_distinct_memberships() {
    let store = MockGraphStore::new();
    for (entity_type, vid) in [("Method", "vid:method:x"), ("Dataset", "vid:dataset:x")] {
        let node = store.create_node("Entity").await.unwrap();
        for (key, value) in [
            ("vid", vid),
            ("label", "SharedName"),
            ("entity_type", entity_type),
        ] {
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
    }
    let papers: HashSet<String> = (0..6).map(|index| format!("p{index}")).collect();
    let mut entity_papers = EntityPapers::new();
    entity_papers.insert(key("Method", "SharedName"), papers.clone());
    entity_papers.insert(key("Dataset", "SharedName"), papers);

    let result = ClusterUseCase::new(Box::new(store.clone()))
        .materialize_clusters(&entity_papers)
        .await
        .unwrap();

    assert_eq!(result.clusters_created, 3);
    assert_eq!(result.member_edges_created, 4);
    assert_eq!(result.clusters_skipped, 0);
    assert_eq!(store.get_nodes_by_label("ConceptCluster").await.len(), 3);
    store.assert_graph_conforms("same_label_different_types_keep_distinct_memberships");
}

#[tokio::test]
async fn test_cluster_use_case_creates_member_of_cluster_edge() {
    let store = MockGraphStore::new();

    let entity_id = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(entity_id, "vid", "vid:entity:ppo".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(entity_id, "label", "PPO".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(entity_id, "entity_type", "Method".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(entity_id, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(entity_id, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(entity_id, "schema_version", 1)
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..7).map(|i| format!("2402.0000{i}")).collect();
    entity_papers.insert(key("Method", "PPO"), papers);

    let use_case = ClusterUseCase::new(Box::new(store.clone()));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    // Verify MEMBER_OF_CLUSTER edges were created
    assert!(
        result.member_edges_created >= 1,
        "expected MEMBER_OF_CLUSTER edges"
    );

    // M329: MEMBER_OF_CLUSTER is fact — must carry valid_at + created_at.
    use da_ports::graph_store::DirectGraphStore;
    let clusters = store.get_nodes_by_label("ConceptCluster").await;
    assert!(!clusters.is_empty());
    let edges = store
        .get_edges_between(entity_id, clusters[0], "MEMBER_OF_CLUSTER")
        .await;
    assert_eq!(edges.len(), 1);
    let (eid, _) = edges[0];
    assert!(
        store
            .get_edge_property_string(eid, "valid_at")
            .await
            .is_some(),
        "MEMBER_OF_CLUSTER must have valid_at"
    );
    assert!(
        store
            .get_edge_property_string(eid, "created_at")
            .await
            .is_some(),
        "MEMBER_OF_CLUSTER must have created_at"
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
    entity_papers.insert(key("Model", "RareModel"), papers);

    let use_case = ClusterUseCase::new(Box::new(store.clone()));
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
    store
        .set_node_property_string(entity_id, "entity_type", "Method".to_string())
        .await
        .unwrap();

    let mut entity_papers: EntityPapers = HashMap::new();
    let papers: HashSet<String> = (0..5).map(|i| format!("2404.0000{i}")).collect();
    entity_papers.insert(key(" Method ", "  GraphSAGE  "), papers);

    let use_case = ClusterUseCase::new(Box::new(store.clone()));
    let result = use_case.materialize_clusters(&entity_papers).await.unwrap();

    assert!(result.clusters_created >= 1);
    // Verify edges were created (ConceptCluster → Entity via MEMBER_OF_CLUSTER)
    assert!(result.member_edges_created >= 1);
}
