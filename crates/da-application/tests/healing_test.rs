//! Integration tests for GraphHealingUseCase.

#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use da_application::GraphHealingUseCase;
use da_domain::healing::HealingActor;
use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};

// Reuse MockGraphStore pattern from extraction tests
struct MockGraphStore {
    nodes: AtomicUsize,
    props: Mutex<std::collections::HashMap<(u64, String), String>>,
    bools: Mutex<std::collections::HashMap<(u64, String), bool>>,
    edges: Mutex<Vec<(u64, u64, String)>>,
}

#[async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _: &str, _: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _: &str, _: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _: &str,
        _: &str,
        _: usize,
        _: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn create_property_index(&self, _: &str, _: &str) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &str,
        _: &[f32],
        _: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Ok(vec![])
    }
    async fn import_snapshot(&self, _: &[u8]) -> GraphResult<()> {
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, _: &str) -> Result<u64, GraphStoreError> {
        Ok(self.nodes.fetch_add(1, Ordering::SeqCst) as u64)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_int(&self, _: u64, _: &str, _: i64) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        self.bools
            .lock()
            .unwrap()
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        self.edges
            .lock()
            .unwrap()
            .push((source, target, edge_type.to_string()));
        Ok(0)
    }
    async fn add_vector(
        &self,
        _: &str,
        _: &str,
        _: u64,
        _: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _: &str,
        _: &str,
        _: &[f32],
        _: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.nodes.load(Ordering::SeqCst)
    }
    async fn edge_count(&self) -> usize {
        self.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(&self, _: &str, key: &str, value: &str) -> Option<u64> {
        let props = self.props.lock().unwrap();
        for ((node_id, k), v) in props.iter() {
            if k == key && v == value {
                return Some(*node_id);
            }
        }
        None
    }
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        let edges = self.edges.lock().unwrap();
        edges
            .iter()
            .filter(|(_, target, _)| *target == node_id)
            .map(|(source, _, edge_type)| (*source, edge_type.clone()))
            .collect()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.props
            .lock()
            .unwrap()
            .get(&(node_id, key.to_string()))
            .cloned()
    }
    async fn get_node_property_int(&self, _node_id: u64, _key: &str) -> Option<i64> {
        None
    }
    async fn get_nodes_by_label(&self, _label: &str) -> Vec<u64> {
        Vec::new()
    }
}

fn make_store() -> MockGraphStore {
    MockGraphStore {
        nodes: AtomicUsize::new(0),
        props: Mutex::new(std::collections::HashMap::new()),
        bools: Mutex::new(std::collections::HashMap::new()),
        edges: Mutex::new(Vec::new()),
    }
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
        .create_edge(paper_id, merge_id, "MENTIONS")
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
