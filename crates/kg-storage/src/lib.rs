//! kg-storage — Universal graph storage abstraction.
//!
//! Reusable crate providing the `GraphStore` and `DirectGraphStore`
//! traits, the `SamyamaGraphStore` adapter (feature-gated), and a
//! `MockGraphStore` for tests. Zero project-specific dependencies —
//! works against generic node labels and string VID values.
//!
//! See ADR-050 for the full architecture decision.

pub mod error;
pub mod mock;
pub mod traits;

pub use error::{GraphResult, GraphStoreError};
pub use mock::MockGraphStore;
pub use traits::{DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult};

// Reserved for Phase B follow-up: when the full Samyama adapter moves
// here from da-adapters, it will live in samyama_adapter.rs behind
// a `samyama` feature flag. For now the stub lives here unconditionally.
pub mod samyama_adapter;
pub use samyama_adapter::SamyamaGraphStore;
