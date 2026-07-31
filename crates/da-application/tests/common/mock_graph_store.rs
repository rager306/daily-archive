//! Shared test support — MockGraphStore for integration tests.

#![allow(dead_code)]

use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Mock GraphStore that records all node/edge operations.
/// Thread-safe via Mutex. All methods return Ok or empty results.
pub struct MockGraphStore {
    pub nodes: Mutex<Vec<(u64, String)>>,
    pub edges: Mutex<Vec<(u64, u64, String)>>,
    pub counter: AtomicUsize,
    pub labels: Mutex<std::collections::HashMap<u64, String>>,
}

impl MockGraphStore {
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(Vec::new()),
            edges: Mutex::new(Vec::new()),
            counter: AtomicUsize::new(0),
            labels: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Ok(vec![])
    }
    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait::async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst) as u64;
        self.nodes.lock().unwrap().push((id, label.to_string()));
        Ok(id)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        if key == "label" {
            self.labels.lock().unwrap().insert(node_id, value);
        }
        Ok(())
    }
    async fn set_node_property_int(
        &self,
        _node_id: u64,
        _key: &str,
        _value: i64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        _node_id: u64,
        _key: &str,
        _value: bool,
    ) -> Result<(), GraphStoreError> {
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
    async fn set_edge_property_float(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: f64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_edge_property_string(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: &str,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn add_vector(
        &self,
        _label: &str,
        _property: &str,
        _node_id: u64,
        _vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _label: &str,
        _property: &str,
        _query: &[f32],
        _k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.nodes.lock().unwrap().len()
    }
    async fn edge_count(&self) -> usize {
        self.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(
        &self,
        _label: &str,
        _property: &str,
        _value: &str,
    ) -> Option<u64> {
        None
    }
    async fn get_outgoing_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_incoming_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        if key == "label" {
            self.labels.lock().unwrap().get(&node_id).cloned()
        } else {
            None
        }
    }
    async fn get_node_property_int(&self, _node_id: u64, _key: &str) -> Option<i64> {
        None
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.nodes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l == label)
            .map(|(id, _)| *id)
            .collect()
    }
}
