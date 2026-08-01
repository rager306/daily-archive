//! Samyama Graph adapter — implements GraphStore port.
//!
//! ADR-041: Uses EmbeddedClient (direct in-process GraphStore access).
//! Three-tier access:
//!   HOT:   direct GraphStore API via store_write()/store_read() — 0.01ms
//!   WARM:  Cypher via QueryEngine.execute() — 1-5ms (plan cache for repeats)
//!   COLD:  Cypher via HTTP RemoteClient — 5-15ms (CLI/external only)

use async_trait::async_trait;
use da_ports::graph_store::{
    GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric, VectorSearchResult,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export Samyama types for callers
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

    pub fn from_env() -> Self {
        let tenant =
            std::env::var("SAMYAMA_DEFAULT_TENANT").unwrap_or_else(|_| "default".to_string());
        let mut s = Self::new();
        s.tenant = tenant;
        s
    }

    // ─── HOT PATH: Direct GraphStore API ──────────────────────

    pub async fn store_read(&self) -> tokio::sync::RwLockReadGuard<'_, SamyamaStore> {
        self.store.read().await
    }

    pub async fn store_write(&self) -> tokio::sync::RwLockWriteGuard<'_, SamyamaStore> {
        self.store.write().await
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

        // Convert RecordBatch → QueryResult
        // Record contains HashMap<String, Value> bindings; columns tells us which to extract
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
        tracing::debug!(cypher = %cypher, "Samyama embedded write");
        self.execute_cypher(cypher).await
    }

    async fn query_readonly(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        tracing::debug!(cypher = %cypher, "Samyama embedded read");
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
        tracing::info!(label, property, dimensions, "Vector index created");
        Ok(())
    }

    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()> {
        let store = self.store_read().await;
        store.property_index.create_index(
            samyama::graph::types::Label::new(label),
            property.to_string(),
        );
        tracing::info!(label, property, "Property index created");
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
impl da_ports::graph_store::DirectGraphStore for SamyamaGraphStore {
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
        for node in store.get_nodes_by_label(&label) {
            if let Some(prop) = node.properties.get(key)
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
        let neighbors = store.get_incoming_neighbor_slice(nid);
        let mut result = Vec::new();
        for (source_id, edge_id) in neighbors {
            if let Some(edge) = store.get_edge(*edge_id) {
                result.push((source_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        result
    }

    async fn get_outgoing_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        let store = self.store_read().await;
        let nid = NodeId(node_id);
        let neighbors = store.get_outgoing_neighbor_slice(nid);
        let mut result = Vec::new();
        for (target_id, edge_id) in neighbors {
            if let Some(edge) = store.get_edge(*edge_id) {
                result.push((target_id.0, edge.edge_type.as_str().to_string()));
            }
        }
        result
    }

    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        let store = self.store_read().await;
        store
            .get_node(NodeId(node_id))
            .and_then(|node| node.properties.get(key))
            .and_then(|prop| prop.as_string().map(|s| s.to_string()))
    }

    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        let store = self.store_read().await;
        store
            .get_node(NodeId(node_id))
            .and_then(|node| node.properties.get(key))
            .and_then(|prop| prop.as_integer())
    }

    async fn get_node_property_bool(&self, node_id: u64, key: &str) -> Option<bool> {
        let store = self.store_read().await;
        store
            .get_node(NodeId(node_id))
            .and_then(|node| node.properties.get(key))
            .and_then(|prop| prop.as_boolean())
    }

    async fn get_node_property_float(&self, node_id: u64, key: &str) -> Option<f64> {
        let store = self.store_read().await;
        store
            .get_node(NodeId(node_id))
            .and_then(|node| node.properties.get(key))
            .and_then(|prop| prop.as_float())
    }

    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        let store = self.store_read().await;
        store
            .get_nodes_by_label(&Label::new(label))
            .iter()
            .map(|n| n.id.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_node_direct() {
        let store = SamyamaGraphStore::new();
        let id = store.create_node_direct("Paper").await;
        store
            .set_property_direct(id, "title", PropertyValue::String("Test".into()))
            .await;
        assert_eq!(store.node_count().await, 1);
    }

    #[tokio::test]
    async fn test_create_edge_direct() {
        let store = SamyamaGraphStore::new();
        let a = store.create_node_direct("Author").await;
        let p = store.create_node_direct("Paper").await;
        let _edge = store.create_edge_direct(a, p, "AUTHORED").await.unwrap();
        let neighbors = store.outgoing_neighbors(a).await;
        assert!(!neighbors.is_empty());
    }

    #[tokio::test]
    async fn test_get_outgoing_edges_returns_targets() {
        // Phase 5: get_outgoing_edges must return target nodes for PPR traversal.
        use da_ports::graph_store::DirectGraphStore;
        let store = SamyamaGraphStore::new();
        let paper = store.create_node_direct("Paper").await;
        let entity = store.create_node_direct("Entity").await;
        store
            .create_edge_direct(paper, entity, da_domain::relation::bibliographic::MENTIONS)
            .await
            .unwrap();

        // Paper has outgoing MENTIONS edge to Entity
        let outgoing = store.get_outgoing_edges(paper.0).await;
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].0, entity.0); // target node ID
        assert_eq!(outgoing[0].1, da_domain::relation::bibliographic::MENTIONS);

        // Entity has no outgoing edges
        let entity_outgoing = store.get_outgoing_edges(entity.0).await;
        assert!(entity_outgoing.is_empty());
    }

    #[tokio::test]
    async fn test_health_always_true() {
        let store = SamyamaGraphStore::new();
        assert!(store.health().await.unwrap());
    }
}
