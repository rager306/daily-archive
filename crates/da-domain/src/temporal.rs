//! BiTemporal Fact helpers (ADR-046).
//!
//! Re-exports universal node-level bi-temporal helpers from
//! kg_ontology::bitemporal (M289-u4fzx5/S01). The universal functions
//! operate on `PropertyMap = HashMap<String, serde_json::Value>`.
//! da-domain provides the `PropertySnapshot` type alias for backward
//! compatibility.
//!
//! Also re-exports edge-level temporal types from kg_ontology::temporal.
//!
//! # Universal types (ADR-050)
//!
//! - Edge-level: `TemporalEdge`, `RetroactiveExtension` (kg_ontology::temporal)
//! - Node-level: `read_datetime`, `read_bound`, `is_active_at`,
//!   `was_known_at`, `is_current`, `validate_bitemporal` (kg_ontology::bitemporal)

// Re-export edge-level temporal types from kg-ontology (ADR-050 Phase A).
pub use kg_ontology::temporal::{
    OPEN, RetroactiveExtension, TemporalEdge, parse_datetime, validate_temporal_edge,
};

// Re-export node-level bi-temporal helpers from kg-ontology (M289-u4fzx5/S01).
pub use kg_ontology::bitemporal::{
    PropertyMap, is_active_at, is_current, read_bound, read_datetime, validate_bitemporal,
};

/// Project-specific type alias for backward compatibility.
/// Callers that used `da_domain::temporal::PropertySnapshot` continue to work.
pub type PropertySnapshot = PropertyMap;
