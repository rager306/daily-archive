//! kg-ontology — Universal knowledge graph ontology.
//!
//! Reusable crate for schema-driven property-graph knowledge bases.
//! Loads ontology from YAML (`data/ontology/*.yaml`), validates nodes
//! and edges against declared schemas, manages schema versioning and
//! migrations, and exposes cross-cutting aspects (provenance, bi-temporal).
//!
//! # Design
//!
//! - **Zero project-specific dependencies.** This crate does not know
//!   about Paper, Claim, daily-archive, or any specific domain. It
//!   works against generic `PropertySnapshot` inputs.
//! - **Data, not code.** All schema data (node types, edge contracts,
//!   aspects, validation rules, versions, dictionaries) lives in YAML.
//!   Rust code holds loader logic + validator logic only.
//! - **Bundled fallback.** A reference ontology is compiled in via
//!   `include_str!` so the crate works zero-config. Callers override
//!   by placing files in `data/ontology/` or by calling
//!   `OntologyRegistry::load_from_dir()`.
//!
//! See ADR-050 for the full architecture decision and migration plan.

pub mod bitemporal;
pub mod edge_contract;
pub mod healing;
pub mod retry;
pub mod schema;
pub mod temporal;
pub mod validator;
pub mod vid;

pub use temporal::{
    OPEN, RetroactiveExtension, TemporalEdge, parse_datetime, validate_temporal_edge,
};
pub use validator::{PropertySnapshot, Severity, Violation, format_violations};

// Reserved module shells — populated in later migration phases (B-F)
// as code moves in from da-domain. Each module documents which phase
// will populate it, per ADR-050 §Migration path.

// schema_inner removed (M289-u4fzx5/S02): schema is now file-based.

pub mod registry;
pub use registry::{OntologyRegistry, TemporalClass, YamlEdgeContract};
