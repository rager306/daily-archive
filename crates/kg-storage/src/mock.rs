//! MockGraphStore — test double for GraphStore + DirectGraphStore.
//!
//! Thread-safe in-memory mock that records all node/edge operations.
//! Clone-able (Arc-backed inner state) so tests can hand one clone to a
//! use case (as `Box<dyn DirectGraphStore>`) and keep another for
//! post-run inspection.
//!
//! Honors the same contract as a real adapter:
//! - `create_node(label)` records the label for later filtering
//! - `set_node_property_*` records all properties
//! - `find_node_by_string_property(label, key, value)` filters by BOTH
//!   label AND the property value (matches adapter semantics)
//! - `get_nodes_by_label(label)` filters by label

#![allow(dead_code)]

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{GraphResult, GraphStoreError};
use crate::traits::{
    DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult,
};

/// Property value stored per (node_id, key).
#[derive(Debug, Clone, Default)]
pub struct MockProps {
    pub string_props: HashMap<(u64, String), String>,
    pub int_props: HashMap<(u64, String), i64>,
    pub float_props: HashMap<(u64, String), f64>,
    pub bool_props: HashMap<(u64, String), bool>,
}

/// Inner state — behind Arc so MockGraphStore is Clone-able while
/// keeping shared state across clones.
#[derive(Debug, Default)]
struct MockGraphStoreInner {
    nodes: Mutex<Vec<(u64, String)>>,
    /// (edge_id, source, target, edge_type)
    edges: Mutex<Vec<(u64, u64, u64, String)>>,
    counter: AtomicUsize,
    props: Mutex<MockProps>,
    /// Edge properties: (edge_id, key) → value
    edge_props: Mutex<HashMap<(u64, String), String>>,
    snapshot_calls: AtomicUsize,
    import_calls: AtomicUsize,
    snapshot_data: Mutex<Option<Vec<u8>>>,
}

/// Mock GraphStore that records all operations.
#[derive(Debug, Clone)]
pub struct MockGraphStore {
    inner: Arc<MockGraphStoreInner>,
}

impl MockGraphStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MockGraphStoreInner::default()),
        }
    }

    pub fn node_count_total(&self) -> usize {
        self.inner.nodes.lock().unwrap().len()
    }

    pub fn edge_count_total(&self) -> usize {
        self.inner.edges.lock().unwrap().len()
    }

    pub fn snapshot_call_count(&self) -> usize {
        self.inner.snapshot_calls.load(Ordering::SeqCst)
    }

    pub fn import_call_count(&self) -> usize {
        self.inner.import_calls.load(Ordering::SeqCst)
    }

    /// Set a canned payload for export_snapshot.
    pub fn set_snapshot_data(&self, data: Vec<u8>) {
        *self.inner.snapshot_data.lock().unwrap() = Some(data);
    }
}

impl Default for MockGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
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
        self.inner.snapshot_calls.fetch_add(1, Ordering::SeqCst);
        let data = self
            .inner
            .snapshot_data
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| b"mock-snapshot-data".to_vec());
        Ok(data)
    }
    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        self.inner.import_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let id = self.inner.counter.fetch_add(1, Ordering::SeqCst) as u64;
        self.inner.nodes.lock().unwrap().push((id, label.to_string()));
        Ok(id)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .props
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
        self.inner
            .props
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
        self.inner
            .props
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
        self.inner
            .props
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
        let mut edges = self.inner.edges.lock().unwrap();
        let edge_id = edges.len() as u64;
        edges.push((edge_id, source, target, edge_type.to_string()));
        Ok(edge_id)
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
        self.inner.nodes.lock().unwrap().len()
    }
    async fn edge_count(&self) -> usize {
        self.inner.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        let nodes = self.inner.nodes.lock().unwrap();
        let props = self.inner.props.lock().unwrap();
        for (node_id, node_label) in nodes.iter() {
            if node_label == label
                && let Some(stored) = props.string_props.get(&(*node_id, key.to_string()))
                && stored == value
            {
                return Some(*node_id);
            }
        }
        None
    }
    async fn get_outgoing_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner
            .edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s, _, _)| *s == node_id)
            .map(|(_, _, t, et)| (*t, et.clone()))
            .collect()
    }
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner
            .edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, _, t, _)| *t == node_id)
            .map(|(_, s, _, et)| (*s, et.clone()))
            .collect()
    }
    async fn get_edges_between(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Vec<(u64, String)> {
        self.inner
            .edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s, t, et)| *s == source && *t == target && et == edge_type)
            .map(|(eid, _, _, et)| (*eid, et.clone()))
            .collect()
    }
    async fn get_edge_property_string(&self, edge_id: u64, key: &str) -> Option<String> {
        self.inner
            .edge_props
            .lock()
            .unwrap()
            .get(&(edge_id, key.to_string()))
            .cloned()
    }
    async fn set_edge_property_string_v2(
        &self,
        edge_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .edge_props
            .lock()
            .unwrap()
            .insert((edge_id, key.to_string()), value.to_string());
        Ok(())
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.inner
            .props
            .lock()
            .unwrap()
            .string_props
            .get(&(node_id, key.to_string()))
            .cloned()
    }
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        self.inner
            .props
            .lock()
            .unwrap()
            .int_props
            .get(&(node_id, key.to_string()))
            .copied()
    }
    async fn get_node_property_bool(&self, node_id: u64, key: &str) -> Option<bool> {
        self.inner
            .props
            .lock()
            .unwrap()
            .bool_props
            .get(&(node_id, key.to_string()))
            .copied()
    }
    async fn get_node_property_float(&self, node_id: u64, key: &str) -> Option<f64> {
        self.inner
            .props
            .lock()
            .unwrap()
            .float_props
            .get(&(node_id, key.to_string()))
            .copied()
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.inner
            .nodes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l == label)
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_node_and_set_property() {
        let store = MockGraphStore::new();
        let id = store.create_node("TestNode").await.unwrap();
        store
            .set_node_property_string(id, "name", "Alice".to_string())
            .await
            .unwrap();
        assert_eq!(store.node_count_total(), 1);
        assert_eq!(
            store.get_node_property_string(id, "name").await,
            Some("Alice".to_string())
        );
    }

    #[tokio::test]
    async fn test_find_node_by_string_property_filters_by_label() {
        let store = MockGraphStore::new();
        let a = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(a, "arxiv_id", "1234.5678".to_string())
            .await
            .unwrap();
        let b = store.create_node("Citation").await.unwrap();
        store
            .set_node_property_string(b, "arxiv_id", "1234.5678".to_string())
            .await
            .unwrap();
        // find_node_by_string_property must filter by label
        let paper = store
            .find_node_by_string_property("Paper", "arxiv_id", "1234.5678")
            .await;
        assert_eq!(paper, Some(a));
        let citation = store
            .find_node_by_string_property("Citation", "arxiv_id", "1234.5678")
            .await;
        assert_eq!(citation, Some(b));
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let store = MockGraphStore::new();
        let clone = store.clone();
        let id = clone.create_node("Shared").await.unwrap();
        // Original sees the node created via the clone
        assert_eq!(store.node_count_total(), 1);
        assert_eq!(
            store.get_nodes_by_label("Shared").await,
            vec![id]
        );
    }

    #[tokio::test]
    async fn test_export_snapshot_returns_canned_data() {
        let store = MockGraphStore::new();
        store.set_snapshot_data(b"test-payload".to_vec());
        let data = store.export_snapshot().await.unwrap();
        assert_eq!(data, b"test-payload");
        assert_eq!(store.snapshot_call_count(), 1);
    }
}

    #[tokio::test]
    async fn test_get_edges_between_filters_correctly() {
        let store = MockGraphStore::new();
        let a = store.create_node("Paper").await.unwrap();
        let b = store.create_node("Entity").await.unwrap();
        let c = store.create_node("Entity").await.unwrap();

        let e1 = store.create_edge(a, b, "MENTIONS").await.unwrap();
        let _e2 = store.create_edge(a, c, "MENTIONS").await.unwrap();
        let _e3 = store.create_edge(a, b, "CITES").await.unwrap();

        let edges = store.get_edges_between(a, b, "MENTIONS").await;
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, e1); // edge_id
        assert_eq!(edges[0].1, "MENTIONS");
    }

    #[tokio::test]
    async fn test_edge_property_set_and_get() {
        let store = MockGraphStore::new();
        let a = store.create_node("Paper").await.unwrap();
        let b = store.create_node("Entity").await.unwrap();
        let edge_id = store.create_edge(a, b, "MENTIONS").await.unwrap();

        store
            .set_edge_property_string_v2(edge_id, "valid_at", "2024-01-01T00:00:00Z")
            .await
            .unwrap();

        let val = store.get_edge_property_string(edge_id, "valid_at").await;
        assert_eq!(
            val,
            Some("2024-01-01T00:00:00Z".to_string())
        );
    }
