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

pub use error::{GraphResult, GraphStoreError};
pub use mock::MockGraphStore;
pub use traits::{DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult};

// Phase B follow-up (M285-z09px0/S05): SamyamaGraphStore full impl
// (GraphStore + DirectGraphStore) will move here from
// da-adapters/src/samyama_graph.rs behind a `samyama` feature flag.
// Until then the stub is removed (dead-code); da-adapters remains
// the single source of SamyamaGraphStore truth.
