//! GraphStore port — the primary graph + vector + query interface.
//!
//! ADR-040 §1 Tier 1: backed by Samyama Graph EmbeddedClient.
//! ADR-038 §5: 6 graph operators (O1-O6) as methods.
//!
//! # Universal types (ADR-050 Phase B re-export)
//!
//! The trait definitions, error types, and result structs live in
//! [`kg_storage`] (universal crate, no da-* deps). This module
//! re-exports them so existing callers (`da_ports::graph_store::*`)
//! keep working without churn.

// Re-export universal storage traits and types from kg-storage (ADR-050 Phase B).
pub use kg_storage::error::{GraphResult, GraphStoreError};
pub use kg_storage::traits::{
    DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult,
};

/// Backward-compat alias: callers that used `Vid` for `VectorSearchResult.vid`
/// can still reach it through da_domain::vid::Vid. At the storage boundary
/// the type is `String` (universal); `Vid = String` (da-domain type alias).
pub use da_domain::vid::Vid;
