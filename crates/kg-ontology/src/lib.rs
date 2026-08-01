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

pub mod validator;
pub mod temporal;

pub use validator::{
    PropertySnapshot, Severity, Violation, format_violations,
};
pub use temporal::{
    TemporalEdge, RetroactiveExtension, OPEN,
    validate_temporal_edge, parse_datetime,
};

// Reserved module shells — populated in later migration phases (B-F)
// as code moves in from da-domain. Each module documents which phase
// will populate it, per ADR-050 §Migration path.

pub mod schema {
    //! `LoadedSchema`, `NodeType`, `PropertyDef`, `FieldType`, `Cardinality`.
    //! Populated in Phase D when the YAML loader lands. Today da-domain
    //! holds the NodeSchemaDef trait + 31 XSchema structs; Phase D
    //! exports them to YAML and this module becomes the single source.
}

pub mod edge_contract {
    //! `EdgeContract`, `EdgeEndpoint`.
    //! Populated in Phase D alongside schema. Today da-domain holds
    //! the edge_contracts() registry; Phase D moves the data to YAML.
}

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

pub mod registry {
    //! `OntologyRegistry` — top-level API. Loads YAML ontology from
    //! disk or bundled fallback. Populated in Phase D.
}
