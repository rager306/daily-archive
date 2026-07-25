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
