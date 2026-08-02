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
pub mod schema;
pub mod retry;
pub mod temporal;
pub mod vid;
pub mod validator;

pub use temporal::{
    OPEN, RetroactiveExtension, TemporalEdge, parse_datetime, validate_temporal_edge,
};
pub use validator::{PropertySnapshot, Severity, Violation, format_violations};

// Reserved module shells — populated in later migration phases (B-F)
// as code moves in from da-domain. Each module documents which phase
// will populate it, per ADR-050 §Migration path.

// schema_inner removed (M289-u4fzx5/S02): schema is now file-based.

pub mod aspects {
    //! `Aspect`, `AspectApplication` — cross-cutting trait mixins.
    //! Populated in Phase E. Provenance, bi-temporal, and similar
    //! aspect definitions will live in `aspects.yaml`.
}

pub mod versioning {
    //! `SchemaVersion`, `MigrationPolicy`, `VersionedSchema`.
    //! Populated in Phase D when schema_versions.yaml lands.
}

pub mod routing {
    //! Intrinsic vs Relational routing decisions.
    //! Populated in Phase D when properties.yaml declares routing.
}

pub mod mappings {
    //! Standard vocabulary mappings (schema.org, FaBiO, CiTO, PROV-O).
    //! Populated in Phase D when node_types.yaml mappings land.
}

pub mod registry;
