//! kg-algorithms — Universal graph algorithms.
//!
//! Temporal edge resolution, causal chain walking, and traversal
//! helpers that operate on top of kg-ontology types. Zero
//! project-specific dependencies — works against generic temporal
//! edges from any domain.
//!
//! See ADR-050 for the crate family architecture.

pub mod temporal;

pub use temporal::{ResolutionOutcome, ResolutionRule, resolve_temporal_edges};
pub mod port;

#[cfg(feature = "solver")]
pub mod csr;

#[cfg(feature = "solver")]
pub use csr::{CsrExportError, csr_matrix_from_graph_view, materialize_unit_weights};

#[cfg(feature = "ppr")]
pub mod ppr;

#[cfg(feature = "ppr")]
pub use ppr::{PprError, personalized_pagerank_from_graph_view};

#[cfg(feature = "ppr")]
pub mod view_algorithms;

#[cfg(feature = "ppr")]
pub use view_algorithms::GraphViewAlgorithms;
