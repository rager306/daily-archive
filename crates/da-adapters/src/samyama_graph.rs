//! Samyama Graph adapter — implements GraphStore port.
//!
//! ADR-041: Uses EmbeddedClient (direct in-process GraphStore access).
//! Three-tier access:
//!   HOT:   direct GraphStore API via store_write()/store_read() — 0.01ms
//!   WARM:  Cypher via EmbeddedClient.query() — 1-5ms (plan cache for repeats)
//!   COLD:  Cypher via HTTP RemoteClient — 5-15ms (CLI/external only)
//!
//! Samyama embedded = zero network overhead, same process.

use async_trait::async_trait;
use da_ports::graph_store::{GraphStore, GraphStoreError, GraphResult, QueryResult, VectorMetric, VectorSearchResult};
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export for callers that need direct access
pub use samyama::graph::store::GraphStore as SamyamaStore;
pub use samyama::graph::property::PropertyValue;
pub use samyama::graph::types::{NodeId, EdgeId, Label, EdgeType};

/// Samyama Graph embedded adapter.
/// Wraps EmbeddedClient for zero-network access.
pub struct SamyamaGraphStore {
    store: Arc<RwLock<SamyamaStore>>,
    engine: samyama::query::QueryEngine,
    graph_name: String,
}

impl SamyamaGraphStore {
    /// Create a new embedded store (fresh empty graph).
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(SamyamaStore::new())),
            engine: samyama::query::QueryEngine::new(),
            graph_name: "default".to_string(),
        }
    }

    /// Create with an existing shared store (for multi-crate sharing).
    pub fn with_store(store: Arc<RwLock<SamyamaStore>>) -> Self {
        Self {
            store,
            engine: samyama::query::QueryEngine::new(),
            graph_name: "default".to_string(),
        }
    }

    /// From environment (graph name from SAMYAMA_DEFAULT_TENANT).
    pub fn from_env() -> Self {
        let graph = std::env::var("SAMYAMA_DEFAULT_TENANT")
            .unwrap_or_else(|_| "default".to_string());
        let mut s = Self::new();
        s.graph_name = graph;
        s
    }

    // ─── HOT PATH: Direct GraphStore API ──────────────────────

    /// Get a read lock on the underlying GraphStore.
    /// Use for direct node/edge/vector operations (0.01ms).
    pub async fn store_read(&self) -> tokio::sync::RwLockReadGuard<'_, SamyamaStore> {
        self.store.read().await
    }

    /// Get a write lock on the underlying GraphStore.
    /// Use for direct create/update/delete operations (0.01ms).
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
        store.set_node_property(node_id, key, value);
    }

    /// Direct edge creation (HOT path — no Cypher).
    pub async fn create_edge_direct(&self, source: NodeId, target: NodeId, edge_type: &str) -> EdgeId {
        let mut store = self.store_write().await;
        store.create_edge(source, target, EdgeType::new(edge_type))
    }

    /// Direct outgoing neighbors (HOT path — no Cypher).
    pub async fn outgoing_neighbors(&self, node_id: NodeId) -> Vec<(NodeId, EdgeId)> {
        let store = self.store_read().await;
        store.get_outgoing_neighbor_slice(node_id).to_vec()
    }

    /// Direct node lookup by label (HOT path — no Cypher).
    pub async fn nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        let store = self.store_read().await;
        store.get_nodes_by_label(&Label::new(label))
            .iter().map(|n| n.id).collect()
    }

    // ─── WARM PATH: Cypher via embedded engine ───────────────

    /// Execute Cypher query using embedded QueryEngine (1-5ms with plan cache).
    async fn execute_cypher(&self, cypher: &str) -> GraphResult<QueryResult> {
        let store = self.store.read().await;
        match self.engine.execute(cypher, &*store) {
            Ok(batch) => Ok(QueryResult {
                columns: batch.columns,
                records: batch.records
                    .into_iter()
                    .map(|row| row.into_iter()
                        .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                        .collect())
                    .collect(),
            }),
            Err(e) => Err(GraphStoreError::Query(e.to_string())),
        }
    }

    // ─── Graph statistics ─────────────────────────────────────

    /// Get total node count.
    pub async fn node_count(&self) -> usize {
        self.store_read().await.all_nodes().len()
    }

    /// Get total edge count.
    pub async fn edge_count(&self) -> usize {
        self.store_read().await.all_edges().len()
    }
}

impl Default for SamyamaGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GraphStore for SamyamaGraphStore {
    async fn query(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        tracing::debug!(cypher = %cypher, "Samyama embedded write query");
        self.execute_cypher(cypher).await
    }

    async fn query_readonly(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        tracing::debug!(cypher = %cypher, "Samyama embedded read query");
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
        let mut store = self.store_write().await;
        store.vector_index.create_index(label, property, dimensions, metric)
            .map_err(|e| GraphStoreError::Vector(e.to_string()))?;
        tracing::info!(label, property, dimensions, "Vector index created");
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
        let index = store.vector_index.get_index(label, property)
            .ok_or_else(|| GraphStoreError::Vector(format!(
                "No vector index for {label}.{property}"
            )))?;
        let results = index.search(query_vector, k)
            .map_err(|e| GraphStoreError::Vector(e.to_string()))?;

        Ok(results.into_iter().map(|(node_id, score)| {
            VectorSearchResult {
                vid: format!("{}", node_id.as_u64()),
                score,
                properties: serde_json::Value::Null,
            }
        }).collect())
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        // ADR-040 §11.6: Samyama .sgsnap export
        let mut buf = Vec::new();
        let store = self.store_read().await;
        samyama::snapshot::export_tenant(&*store, &mut buf)
            .map_err(|e| GraphStoreError::Storage(e.to_string()))?;
        Ok(buf)
    }

    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()> {
        let mut store = self.store_write().await;
        samyama::snapshot::import_tenant(&mut *store, &mut std::io::Cursor::new(data))
            .map_err(|e| GraphStoreError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn health(&self) -> GraphResult<bool> {
        // Embedded client is always healthy if the struct exists
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_node_direct() {
        let store = SamyamaGraphStore::new();
        let id = store.create_node_direct("Paper").await;
        store.set_property_direct(id, "title", PropertyValue::String("Test".into())).await;
        
        let count = store.node_count().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_create_edge_direct() {
        let store = SamyamaGraphStore::new();
        let a = store.create_node_direct("Author").await;
        let p = store.create_node_direct("Paper").await;
        let _edge = store.create_edge_direct(a, p, "AUTHORED").await;

        let neighbors = store.outgoing_neighbors(a).await;
        assert!(!neighbors.is_empty());
    }

    #[tokio::test]
    async fn test_health_always_true() {
        let store = SamyamaGraphStore::new();
        assert!(store.health().await.unwrap());
    }
}
