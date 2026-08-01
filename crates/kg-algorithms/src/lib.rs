//! kg-algorithms — Universal graph algorithms.
//!
//! Temporal edge resolution, causal chain walking, and traversal
//! helpers that operate on top of kg-ontology types. Zero
//! project-specific dependencies — works against generic temporal
//! edges from any domain.
//!
//! See ADR-050 for the crate family architecture.

pub mod temporal;

pub use temporal::{resolve_temporal_edges, ResolutionOutcome, ResolutionRule};
