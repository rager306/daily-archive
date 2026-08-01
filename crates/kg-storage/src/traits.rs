//! GraphStore and DirectGraphStore traits.
//!
//! Universal graph storage abstraction. The `GraphStore` trait covers
//! query + vector + index operations; `DirectGraphStore` extends it with
//! HOT-path CRUD operations for embedded clients.
//!
//! Vid is `String` (universal). Project-specific crates may use a typed
//! Vid alias from their domain crate; at the storage boundary it is
//! always a string.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{GraphResult, GraphStoreError};

/// A query result row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub records: Vec<Vec<serde_json::Value>>,
}

/// Distance metric for vector search.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VectorMetric {
    Cosine,
    L2,
    Dot,
}

/// A vector search result. `vid` is a string (universal — project-specific
/// crates may use a typed alias, but at the storage layer it is a string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub vid: String,
    pub score: f32,
    pub properties: serde_json::Value,
}

/// The graph store port. Samyama EmbeddedClient implements this.
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn query(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult>;
    async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult>;
    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()>;
    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()>;
    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>>;
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>>;
    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()>;
    async fn health(&self) -> GraphResult<bool>;
}

/// Direct GraphStore API for HOT path operations.
/// EmbeddedClient implementations provide this; HTTP-only clients don't.
#[async_trait]
pub trait DirectGraphStore: GraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError>;
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError>;
    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError>;
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError>;
    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError>;
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError>;
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
        label: &str,
        property: &str,
        node_id: u64,
        vector: Vec<f32>,
    ) -> Result<(), GraphStoreError>;
    async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError>;
    async fn node_count(&self) -> usize;
    async fn edge_count(&self) -> usize;
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64>;
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)>;
    async fn get_outgoing_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String>;
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64>;
    async fn get_node_property_bool(&self, node_id: u64, key: &str) -> Option<bool> {
        self.get_node_property_string(node_id, key)
            .await
            .and_then(|s| match s.to_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            })
    }
    async fn get_node_property_float(&self, node_id: u64, key: &str) -> Option<f64> {
        self.get_node_property_string(node_id, key)
            .await
            .and_then(|s| s.parse::<f64>().ok())
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64>;

    /// Get all edges of a given type between two specific nodes.
    /// Used by temporal resolution (ADR-047) to find potentially-
    /// contradicting edges when a new temporal edge is written.
    ///
    /// Returns Vec of (edge_id, edge_type) pairs. Callers then read
    /// individual edge properties via `get_edge_property_*` methods.
    async fn get_edges_between(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Vec<(u64, String)> {
        // Default: filter outgoing edges from source by type + target.
        let outgoing = self.get_outgoing_edges(source).await;
        outgoing
            .into_iter()
            .filter(|(t, et)| *t == target && et == edge_type)
            .map(|(_, et)| (0u64, et)) // edge_id unknown by default impl
            .collect()
    }

    /// Read a string property from an edge.
    /// Used for temporal field reads (valid_at, invalid_at, etc.).
    async fn get_edge_property_string(
        &self,
        _edge_id: u64,
        _key: &str,
    ) -> Option<String> {
        None // default: edges are opaque; adapters override
    }

    /// Set a string property on an edge. Used for temporal field writes
    /// (invalid_at, expired_at during edge invalidation).
    async fn set_edge_property_string_v2(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: &str,
    ) -> Result<(), GraphStoreError> {
        Ok(()) // default no-op; adapters override
    }
}
