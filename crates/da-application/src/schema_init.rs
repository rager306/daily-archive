//! Graph schema version contract.
//!
//! Index definitions are ontology data, not generated Cypher mirrors. The CLI
//! composition root loads `data/ontology/identity_indexes.yaml` through the
//! universal `kg-ontology::IdentityIndexPolicy` and applies it via
//! `DirectGraphStore`. Vector/performance indexes remain separate capabilities.

/// The domain owns the persisted schema version; application only re-exports it.
pub use da_domain::schema::CURRENT_SCHEMA_VERSION;

/// Schema version type alias for durable report fields.
pub type SchemaVersion = u32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_schema_version_is_stable() {
        assert_eq!(CURRENT_SCHEMA_VERSION, 1);
    }
}
