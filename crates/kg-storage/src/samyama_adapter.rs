//! SamyamaGraphStore adapter — bridges kg-storage traits to Samyama
//! embedded graph store (ADR-040, ADR-041).
//!
//! Feature-gated behind `samyama` because it pulls the samyama vendor-source
//! dependency. Without the feature, kg-storage is universal (no project deps).
//!
//! Migrated from da-adapters/src/samyama_graph.rs in M285-z09px0/S09
//! (ADR-050 Phase B step 2 — single source of truth in universal crate).

#![cfg(feature = "samyama")]

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

// Use crate-local traits/types (not da_ports re-exports).
use crate::error::{GraphResult, GraphStoreError};
use crate::traits::{DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult};

// Re-export Samyama types for callers.
pub use samyama::graph::property::PropertyValue;
pub use samyama::graph::store::GraphStore as SamyamaStore;
pub use samyama::graph::types::{EdgeId, EdgeType, Label, NodeId};
pub use samyama::query::RecordBatch;

/// Samyama Graph embedded adapter.
/// Wraps GraphStore + QueryEngine for zero-network access.
pub struct SamyamaGraphStore {
    store: Arc<RwLock<SamyamaStore>>,
    engine: samyama::query::QueryEngine,
    tenant: String,
}

impl SamyamaGraphStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(SamyamaStore::new())),
            engine: samyama::query::QueryEngine::new(),
            tenant: "default".to_string(),
        }
    }

    pub fn with_store(store: Arc<RwLock<SamyamaStore>>) -> Self {
        Self {
            store,
            engine: samyama::query::QueryEngine::new(),
            tenant: "default".to_string(),
        }
    }

    /// Open an **embedded** process-local store (ignores `DA_GRAPH_MODE`).
    /// Prefer [`Self::try_from_env`] at composition roots so remote mode fails honestly.
    pub fn from_env() -> Self {
        let tenant =
            std::env::var("SAMYAMA_DEFAULT_TENANT").unwrap_or_else(|_| "default".to_string());
        let mut s = Self::new();
        s.tenant = tenant;
        s
    }

    /// Open store for an already-resolved [`crate::GraphMode`].
    ///
    /// `Embedded` → process-local Samyama; `Remote` → typed not-configured error.
    pub fn open_for_mode(mode: crate::GraphMode) -> Result<Self, crate::GraphModeError> {
        // M332: refuse mistaken RocksDB path env (persistence not wired for CLI).
        crate::graph_mode::refuse_persistence_path_from_env()?;
        match mode {
            crate::GraphMode::Embedded => Ok(Self::from_env()),
            crate::GraphMode::Remote => Err(crate::GraphModeError::RemoteNotConfigured),
        }
    }

    /// Mode-aware open: `DA_GRAPH_MODE=embedded` (default) → process-local store;
    /// `remote` → [`crate::GraphModeError::RemoteNotConfigured`] (no fake client).
    /// Non-empty `DA_GRAPH_DATA_PATH` → [`crate::GraphModeError::PersistenceNotConfigured`].
    pub fn try_from_env() -> Result<Self, crate::GraphModeError> {
        Self::open_for_mode(crate::GraphMode::from_env()?)
    }

    // ─── HOT PATH: Direct GraphStore API ──────────────────────

    pub async fn store_read(&self) -> tokio::sync::RwLockReadGuard<'_, SamyamaStore> {
        self.store.read().await
    }

    pub async fn store_write(&self) -> tokio::sync::RwLockWriteGuard<'_, SamyamaStore> {
        self.store.write().await
    }

    /// Resolve a node property from HashMap first, then ColumnStore.
    ///
    /// Samyama snapshot import (sgsnap v2) writes simple scalar properties into
    /// `node_columns` via `set_column_property`, not into the node HashMap.
    /// Getters must read both or export→import round-trips look empty.
    fn resolve_node_property(
        store: &SamyamaStore,
        node_id: u64,
        key: &str,
    ) -> Option<PropertyValue> {
        if let Some(prop) = store
            .get_node(NodeId(node_id))
            .and_then(|node| node.properties.get(key))
            .cloned()
        {
            return Some(prop);
        }
        let col = store.node_columns.get_property(node_id as usize, key);
        if col.is_null() { None } else { Some(col) }
    }

    /// Direct node creation (HOT path — no Cypher).
    pub async fn create_node_direct(&self, label: &str) -> NodeId {
        let mut store = self.store_write().await;
        store.create_node(Label::new(label))
    }

    /// Direct property set (HOT path — no Cypher).
    pub async fn set_property_direct(&self, node_id: NodeId, key: &str, value: PropertyValue) {
        let mut store = self.store_write().await;
        let _ = store.set_node_property(&self.tenant, node_id, key.to_string(), value);
    }

    /// Direct edge creation (HOT path — no Cypher). Returns Result.
    pub async fn create_edge_direct(
        &self,
        source: NodeId,
        target: NodeId,
        edge_type: &str,
    ) -> Result<EdgeId, samyama::GraphError> {
        let mut store = self.store_write().await;
        store.create_edge(source, target, EdgeType::new(edge_type))
    }

    /// Direct outgoing neighbors (HOT path).
    pub async fn outgoing_neighbors(&self, node_id: NodeId) -> Vec<(NodeId, EdgeId)> {
        let store = self.store_read().await;
        store.get_outgoing_neighbor_slice(node_id).to_vec()
    }

    /// Direct node lookup by label (HOT path).
    pub async fn nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        let store = self.store_read().await;
        store
            .get_nodes_by_label(&Label::new(label))
            .iter()
            .map(|n| n.id)
            .collect()
    }

    /// Direct vector add (HOT path).
    pub async fn add_vector_direct(
        &self,
        label: &str,
        property: &str,
        node_id: NodeId,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let store = self.store_read().await;
        store
            .vector_index
            .add_vector(label, property, node_id, &vector)
            .map_err(|e| e.to_string())
    }

    /// Direct vector search (HOT path).
    pub async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(NodeId, f32)>, String> {
        let store = self.store_read().await;
        store
            .vector_index
            .search(label, property, query, k)
            .map_err(|e| e.to_string())
    }

    // ─── Graph statistics ─────────────────────────────────────

    pub async fn node_count(&self) -> usize {
        self.store_read().await.all_nodes().len()
    }

    pub async fn edge_count(&self) -> usize {
        self.store_read().await.all_edges().len()
    }

    // ─── WARM PATH: Cypher via embedded engine ───────────────

    async fn execute_cypher(&self, cypher: &str) -> GraphResult<QueryResult> {
        let store = self.store.read().await;
        let batch: RecordBatch = self
            .engine
            .execute(cypher, &store)
            .map_err(|e| GraphStoreError::Query(e.to_string()))?;

        let columns = batch.columns.clone();
        let records: Vec<Vec<serde_json::Value>> = batch
            .records
            .into_iter()
            .map(|record| {
                columns
                    .iter()
                    .map(|col| {
                        record
                            .get(col)
                            .map(value_to_json)
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .collect()
            })
            .collect();

        Ok(QueryResult { columns, records })
    }
}

impl Default for SamyamaGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert Samyama Value → serde_json::Value
fn value_to_json(val: &samyama::query::Value) -> serde_json::Value {
    use samyama::query::Value;
    match val {
        Value::Property(pv) => property_to_json(pv),
        Value::Null => serde_json::Value::Null,
        Value::Node(_, _) => serde_json::Value::String("[node]".into()),
        Value::NodeRef(id) => serde_json::json!({"node_id": id.as_u64()}),
        Value::Edge(_, _) => serde_json::Value::String("[edge]".into()),
        Value::EdgeRef(id, ..) => serde_json::json!({"edge_id": id.as_u64()}),
        Value::Path { nodes, edges } => serde_json::json!({
            "nodes": nodes.iter().map(|n| n.as_u64()).collect::<Vec<_>>(),
            "edges": edges.iter().map(|e| e.as_u64()).collect::<Vec<_>>(),
        }),
    }
}

/// Convert PropertyValue → serde_json::Value
fn property_to_json(pv: &PropertyValue) -> serde_json::Value {
    match pv {
        PropertyValue::String(s) => serde_json::Value::String(s.clone()),
        PropertyValue::Integer(i) => serde_json::Value::Number((*i).into()),
        PropertyValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        PropertyValue::Boolean(b) => serde_json::Value::Bool(*b),
        PropertyValue::DateTime(ts) => serde_json::Value::Number((*ts).into()),
        PropertyValue::Null => serde_json::Value::Null,
        PropertyValue::Vector(v) => serde_json::json!(v),
        PropertyValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(property_to_json).collect())
        }
        PropertyValue::Map(map) => {
            let mut m = serde_json::Map::new();
            for (k, v) in map {
                m.insert(k.clone(), property_to_json(v));
            }
            serde_json::Value::Object(m)
        }
        PropertyValue::Duration {
            months,
            days,
            seconds,
            nanos,
        } => serde_json::json!({
            "months": months, "days": days, "seconds": seconds, "nanos": nanos,
        }),
    }
}

#[async_trait]
impl GraphStore for SamyamaGraphStore {
    async fn query(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.execute_cypher(cypher).await
    }

    async fn query_readonly(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.execute_cypher(cypher).await
    }

    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()> {
        let metric = match metric {
            VectorMetric::Cosine => samyama::vector::index::DistanceMetric::Cosine,
            VectorMetric::L2 => samyama::vector::index::DistanceMetric::L2,
            VectorMetric::Dot => samyama::vector::index::DistanceMetric::InnerProduct,
        };
        let store = self.store_read().await;
        store
            .vector_index
            .create_index(label, property, dimensions, metric)
            .map_err(|e| GraphStoreError::Vector(e.to_string()))?;
        Ok(())
    }

    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()> {
        let store = self.store_read().await;
        let label = samyama::graph::types::Label::new(label);
        store
            .property_index
            .create_index(label.clone(), property.to_string());
        // Samyama's low-level create_index starts empty. Backfill existing live
        // and snapshot-column values so indexed lookup is complete immediately
        // and safe for input-snapshot continuation.
        for node in store.get_nodes_by_label(&label) {
            if let Some(value) = Self::resolve_node_property(&store, node.id.0, property) {
                store
                    .property_index
                    .index_insert(&label, property, value, node.id);
            }
        }
        Ok(())
    }

    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        let store = self.store_read().await;
        let results = store
            .vector_index
            .search(label, property, query_vector, k)
            .map_err(|e| GraphStoreError::Vector(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|(node_id, score)| VectorSearchResult {
                vid: format!("{}", node_id.as_u64()),
                score,
                properties: serde_json::Value::Null,
            })
            .collect())
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        let mut buf = Vec::new();
        let store = self.store_read().await;
        samyama::snapshot::export_tenant(&store, &mut buf)
            .map_err(|e| GraphStoreError::Storage(e.to_string()))?;
        Ok(buf)
    }

    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()> {
        let mut store = self.store_write().await;
        samyama::snapshot::import_tenant(&mut store, &mut std::io::Cursor::new(data))
            .map_err(|e| GraphStoreError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

// ─── DirectGraphStore implementation (ADR-041 HOT path) ────

#[async_trait]
impl DirectGraphStore for SamyamaGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let mut store = self.store_write().await;
        let id = store.create_node(Label::new(label));
        Ok(id.as_u64())
    }

    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_node_property(
                &self.tenant,
                NodeId::new(node_id),
                key.to_string(),
                PropertyValue::String(value),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_node_property(
                &self.tenant,
                NodeId::new(node_id),
                key.to_string(),
                PropertyValue::Integer(value),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_node_property(
                &self.tenant,
                NodeId::new(node_id),
                key.to_string(),
                PropertyValue::Boolean(value),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_node_property(
                &self.tenant,
                NodeId::new(node_id),
                key.to_string(),
                PropertyValue::Float(value),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .create_edge(
                NodeId::new(source),
                NodeId::new(target),
                EdgeType::new(edge_type),
            )
            .map(|eid| eid.as_u64())
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn add_vector(
        &self,
        label: &str,
        property: &str,
        node_id: u64,
        vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        let store = self.store_read().await;
        store
            .vector_index
            .add_vector(label, property, NodeId::new(node_id), &vector)
            .map_err(|e| GraphStoreError::Vector(e.to_string()))
    }

    async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        let store = self.store_read().await;
        store
            .vector_index
            .search(label, property, query, k)
            .map(|results| {
                results
                    .into_iter()
                    .map(|(nid, score)| (nid.as_u64(), score))
                    .collect()
            })
            .map_err(|e| GraphStoreError::Vector(e.to_string()))
    }

    async fn node_count(&self) -> usize {
        self.store_read().await.all_nodes().len()
    }

    async fn edge_count(&self) -> usize {
        self.store_read().await.all_edges().len()
    }

    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        let store = self.store_read().await;
        let label = Label::new(label);
        if let Some(index) = store.property_index.get_index(&label, key) {
            let mut matches = index
                .read()
                .ok()?
                .get(&PropertyValue::String(value.to_string()));
            matches.sort_by_key(|node_id| node_id.0);
            return matches.first().map(|node_id| node_id.0);
        }
        for node in store.get_nodes_by_label(&label) {
            // M358: use resolve_node_property to also check node_columns
            // (snapshot import writes to column store, not HashMap)
            if let Some(prop) = Self::resolve_node_property(&store, node.id.0, key)
                && prop.as_string() == Some(value)
            {
                return Some(node.id.0);
            }
        }
        None
    }

    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        let store = self.store_read().await;
        let nid = NodeId(node_id);
        let mut result = Vec::new();
        // Write buffer edges.
        let neighbors = store.get_incoming_neighbor_slice(nid);
        for (source_id, edge_id) in neighbors {
            if let Some(edge) = store.get_edge(*edge_id) {
                result.push((source_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        // D194 M370 S01: also read FROZEN edges (from snapshot import).
        let idx = node_id as usize;
        let frozen = store.frozen_incoming_neighbors(idx);
        for (source_id, edge_id) in frozen {
            if let Some(edge) = store.get_edge(edge_id) {
                result.push((source_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        result
    }

    async fn get_outgoing_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        let store = self.store_read().await;
        let nid = NodeId(node_id);
        let mut result = Vec::new();
        // Write buffer edges (from create_edge during this session).
        let neighbors = store.get_outgoing_neighbor_slice(nid);
        for (target_id, edge_id) in neighbors {
            if let Some(edge) = store.get_edge(*edge_id) {
                result.push((target_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        // D194 M370 S01: also read FROZEN edges (from snapshot import via create_edge_stub).
        // Without this, snapshot-loaded graphs return 0 edges — the root cause
        // of WCC showing 2571 isolated nodes and PageRank returning empty.
        let idx = node_id as usize;
        let frozen = store.frozen_outgoing_neighbors(idx);
        for (target_id, edge_id) in frozen {
            if let Some(edge) = store.get_edge(edge_id) {
                result.push((target_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        result
    }

    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        let store = self.store_read().await;
        Self::resolve_node_property(&store, node_id, key)
            .and_then(|prop| prop.as_string().map(|s| s.to_string()))
    }

    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        let store = self.store_read().await;
        Self::resolve_node_property(&store, node_id, key).and_then(|prop| prop.as_integer())
    }

    async fn get_node_property_bool(&self, node_id: u64, key: &str) -> Option<bool> {
        let store = self.store_read().await;
        Self::resolve_node_property(&store, node_id, key).and_then(|prop| prop.as_boolean())
    }

    async fn get_node_property_float(&self, node_id: u64, key: &str) -> Option<f64> {
        let store = self.store_read().await;
        Self::resolve_node_property(&store, node_id, key).and_then(|prop| prop.as_float())
    }

    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        let store = self.store_read().await;
        store
            .get_nodes_by_label(&Label::new(label))
            .iter()
            .map(|n| n.id.0)
            .collect()
    }

    async fn get_all_node_ids(&self) -> Vec<u64> {
        let store = self.store_read().await;
        store.all_nodes().iter().map(|n| n.id.0).collect()
    }

    async fn get_node_labels(&self, node_id: u64) -> Vec<String> {
        let store = self.store_read().await;
        let mut labels = store
            .get_node(NodeId(node_id))
            .map(|node| {
                node.labels
                    .iter()
                    .map(|label| label.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        labels.sort();
        labels
    }

    async fn get_edges_between(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Vec<(u64, String)> {
        let store = self.store_read().await;
        let src = NodeId(source);
        let tgt = NodeId(target);
        let et = EdgeType::new(edge_type);
        store
            .all_edges()
            .into_iter()
            .filter(|e| e.source == src && e.target == tgt && e.edge_type == et)
            .map(|e| (e.id.0, e.edge_type.as_str().to_string()))
            .collect()
    }

    async fn get_edge_property_string(&self, edge_id: u64, key: &str) -> Option<String> {
        let store = self.store_read().await;
        store
            .get_edge_properties(EdgeId(edge_id))
            .and_then(|props| props.get(key))
            .and_then(|prop| prop.as_string().map(|s| s.to_string()))
    }

    async fn set_edge_property_string(
        &self,
        edge_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_edge_property(
                EdgeId(edge_id),
                key.to_string(),
                PropertyValue::String(value.to_string()),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn set_edge_property_float(
        &self,
        edge_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        let mut store = self.store_write().await;
        store
            .set_edge_property(
                EdgeId(edge_id),
                key.to_string(),
                PropertyValue::Float(value),
            )
            .map_err(|e| GraphStoreError::Storage(e.to_string()))
    }

    async fn get_edge_property_float(&self, edge_id: u64, key: &str) -> Option<f64> {
        // Override default impl: Samyama stores PropertyValue::Float directly,
        // so we read it natively instead of parsing from string.
        let store = self.store_read().await;
        store
            .get_edge_properties(EdgeId(edge_id))
            .and_then(|props| props.get(key).cloned())
            .and_then(|prop| {
                // Try Float first, then String parse fallback.
                if let Some(f) = prop.as_float() {
                    return Some(f);
                }
                prop.as_string().and_then(|s| s.parse::<f64>().ok())
            })
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    use crate::{GraphMode, GraphModeError};

    #[tokio::test]
    async fn property_index_backfills_and_tracks_future_identity_values() {
        let store = SamyamaGraphStore::new();
        let before = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(before, "vid", "vid:before".into())
            .await
            .unwrap();
        store.create_property_index("Paper", "vid").await.unwrap();
        assert_eq!(
            store
                .find_node_by_string_property("Paper", "vid", "vid:before")
                .await,
            Some(before)
        );

        let after = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(after, "vid", "vid:after".into())
            .await
            .unwrap();
        assert_eq!(
            store
                .find_node_by_string_property("Paper", "vid", "vid:after")
                .await,
            Some(after)
        );
        assert_eq!(
            store
                .find_node_by_string_property("Paper", "vid", "vid:missing")
                .await,
            None
        );
    }

    #[test]
    fn open_for_mode_embedded_ok() {
        assert_eq!(GraphMode::parse(None).unwrap(), GraphMode::Embedded);
        let store = SamyamaGraphStore::open_for_mode(GraphMode::Embedded).expect("embedded");
        let _ = store;
    }

    #[test]
    fn open_for_mode_remote_not_configured() {
        // Avoid unwrap_err: SamyamaGraphStore is not Debug.
        let result = SamyamaGraphStore::open_for_mode(GraphMode::Remote);
        let err = match result {
            Ok(_) => panic!("remote must not open a store"),
            Err(e) => e,
        };
        assert_eq!(err, GraphModeError::RemoteNotConfigured);
        let msg = err.to_string();
        assert!(msg.contains("not configured"), "{msg}");
        assert!(msg.contains("embedded"), "{msg}");
    }
}
