//! Shared test support — MockGraphStore for integration tests.
//!
//! Single source-of-truth mock for DirectGraphStore + GraphStore traits.
//! Honors the same contract as the real SamyamaGraphStore:
//!   - `create_node(label)` records the label for later filtering
//!   - `set_node_property_string/int/float/bool` records all properties
//!   - `find_node_by_string_property(label, key, value)` filters by BOTH
//!     label AND the property value (matches SamyamaGraphStore semantics)
//!   - `get_nodes_by_label(label)` filters by label
//!
//! Replaces per-test MockGraphStore impls in batch_ingest_test, enrich_test,
//! extraction_test, healing_test (MEM499, MEM500).

#![allow(dead_code)]

use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Property value stored per (node_id, key).
#[derive(Debug, Clone, Default)]
pub struct MockProps {
    pub string_props: HashMap<(u64, String), String>,
    pub int_props: HashMap<(u64, String), i64>,
    pub float_props: HashMap<(u64, String), f64>,
    pub bool_props: HashMap<(u64, String), bool>,
}

/// Mock GraphStore that records all node/edge operations.
/// Thread-safe via Mutex. All methods return Ok or empty results.
pub struct MockGraphStore {
    /// (node_id, label) — one entry per create_node call.
    pub nodes: Mutex<Vec<(u64, String)>>,
    /// (source, target, edge_type) — one entry per create_edge call.
    pub edges: Mutex<Vec<(u64, u64, String)>>,
    /// Monotonic node id counter.
    pub counter: AtomicUsize,
    /// All properties ever set, indexed by (node_id, key).
    pub props: Mutex<MockProps>,
}

impl MockGraphStore {
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(Vec::new()),
            edges: Mutex::new(Vec::new()),
            counter: AtomicUsize::new(0),
            props: Mutex::new(MockProps::default()),
        }
    }

    /// Convenience: total number of create_node calls.
    pub fn node_count_total(&self) -> usize {
        self.nodes.lock().unwrap().len()
    }

    /// Convenience: total number of create_edge calls.
    pub fn edge_count_total(&self) -> usize {
        self.edges.lock().unwrap().len()
    }
}

impl Default for MockGraphStore {
    fn default() -> Self {
        Self::new()
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
        self.props
            .lock()
            .unwrap()
            .string_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .int_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .float_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .bool_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        let mut edges = self.edges.lock().unwrap();
        let edge_id = edges.len() as u64;
        edges.push((source, target, edge_type.to_string()));
        Ok(edge_id)
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
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        // Real SamyamaGraphStore filters by label; the mock must honor
        // the same contract (MEM495). Walk nodes with matching label and
        // check their string property.
        let nodes = self.nodes.lock().unwrap();
        let props = self.props.lock().unwrap();
        for (node_id, node_label) in nodes.iter() {
            if node_label == label {
                if let Some(stored) = props.string_props.get(&(*node_id, key.to_string())) {
                    if stored == value {
                        return Some(*node_id);
                    }
                }
            }
        }
        None
    }
    async fn get_outgoing_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        self.edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(s, _, _)| *s == _node_id)
            .map(|(_, t, et)| (*t, et.clone()))
            .collect()
    }
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, t, _)| *t == node_id)
            .map(|(s, _, et)| (*s, et.clone()))
            .collect()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.props
            .lock()
            .unwrap()
            .string_props
            .get(&(node_id, key.to_string()))
            .cloned()
    }
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        self.props
            .lock()
            .unwrap()
            .int_props
            .get(&(node_id, key.to_string()))
            .copied()
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
