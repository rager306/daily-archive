//! kg-storage — Universal graph storage abstraction.
//!
//! Reusable crate providing the `GraphStore` and `DirectGraphStore`
//! traits and a `MockGraphStore` for tests. Zero project-specific
//! dependencies — works against generic node labels and string VID
//! values.
//!
//! See ADR-050 for the full architecture decision.

pub mod error;
pub mod mock;
pub mod traits;

#[cfg(feature = "samyama")]
pub mod samyama_adapter;

pub use error::{GraphResult, GraphStoreError};
pub use mock::MockGraphStore;
pub use traits::{DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult};

#[cfg(feature = "samyama")]
pub use samyama_adapter::{
    EdgeId, EdgeType, Label, NodeId, PropertyValue, RecordBatch, SamyamaGraphStore, SamyamaStore,
};
