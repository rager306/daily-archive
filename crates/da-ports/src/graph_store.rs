//! GraphStore port — the primary graph + vector + query interface.
//!
//! ADR-040 §1 Tier 1: backed by Samyama Graph EmbeddedClient.
//! ADR-038 §5: 6 graph operators (O1-O6) as methods.

use async_trait::async_trait;
use da_domain::vid::Vid;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphStoreError {
    #[error("Node not found: {0}")]
    NotFound(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Vector error: {0}")]
    Vector(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

pub type GraphResult<T> = Result<T, GraphStoreError>;

/// A query result row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub records: Vec<Vec<serde_json::Value>>,
}

/// The graph store port. Samyama EmbeddedClient implements this.
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// Execute a Cypher write query.
    async fn query(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult>;

    /// Execute a Cypher read-only query.
    async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult>;

    /// Create a vector index for a label + property.
    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()>;

    /// Create a property index for a label + property (HOT path).
    /// Used by `da schema init` to create all indexes before loading.
    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()>;

    /// Vector similarity search.
    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>>;

    /// Export snapshot for backup/migration safety (ADR-040 §11.6).
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>>;

    /// Import snapshot for rollback (ADR-040 §11.6).
    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()>;

    /// Health check.
    async fn health(&self) -> GraphResult<bool>;
}

/// Distance metric for vector search.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VectorMetric {
    Cosine,
    L2,
    Dot,
}

/// A vector search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub vid: Vid,
    pub score: f32,
    pub properties: serde_json::Value,
}

/// Direct GraphStore API for HOT path operations (ADR-041).
/// EmbeddedClient implementations provide this; HTTP-only clients don't.
/// Use for: ingest (batch writes), vector insert, direct node/edge creation.
/// Latency: 0.01-0.1ms per operation (no Cypher parse/plan overhead).
#[async_trait]
pub trait DirectGraphStore: GraphStore {
    /// Create a node with a label. Returns internal NodeId.
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError>;

    /// Set a string property on a node.
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError>;

    /// Set an integer property on a node.
    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError>;

    /// Set a boolean property on a node.
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError>;

    /// Set a float property on a node (e.g., metric observations, confidence).
    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError>;

    /// Create a directed edge between two nodes.
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError>;

    /// Set a float property on an edge (e.g., weight, confidence, similarity).
    /// Phase 3 GNN readiness: edge weights enable PPR and message passing.
    async fn set_edge_property_float(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: f64,
    ) -> Result<(), GraphStoreError> {
        // Default: no-op (adapters override if supported)
        Ok(())
    }

    /// Set a string property on an edge (e.g., citation_type, evidence).
    async fn set_edge_property_string(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: &str,
    ) -> Result<(), GraphStoreError> {
        // Default: no-op (adapters override if supported)
        Ok(())
    }

    /// Add a vector to a node's vector index.
    async fn add_vector(
        &self,
        label: &str,
        property: &str,
        node_id: u64,
        vector: Vec<f32>,
    ) -> Result<(), GraphStoreError>;

    /// Direct vector search (no Cypher).
    async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError>;

    /// Count nodes in the graph.
    async fn node_count(&self) -> usize;

    /// Count edges in the graph.
    async fn edge_count(&self) -> usize;

    /// Find a node by label + string property value (HOT path).
    /// Returns the first matching NodeId, or None.
    /// O(n) scan — suitable for Phase 2 scale (≤10k nodes).
    /// For production scale, use a property index (Phase 5+).
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64>;

    /// Get all incoming edges to a node (for merge edge redirect, D135).
    /// Returns Vec of (source_node_id, edge_type).
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)>;

    /// Get all outgoing edges from a node (for PPR traversal, Phase 5).
    /// Returns Vec of (target_node_id, edge_type).
    async fn get_outgoing_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        // Default: empty (adapters override)
        Vec::new()
    }

    /// Read a string property from a node (for healing audit trail, D135).
    /// Returns None if the node or property doesn't exist.
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String>;

    /// Read an integer property from a node (for scheduler queue).
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64>;

    /// Get all node IDs with a given label (for scheduler queue scan).
    /// O(n) scan — suitable for Phase 2 scale.
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64>;
}
