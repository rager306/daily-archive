//! GraphStore error types.

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
