//! Revision-plane schema-as-code registrations (M400 S02 T03, ADR-045 Wave E).
//!
//! `PaperRevision` and `SourceManifestation` are revision-plane node types
//! materialized by `da-application` (revision_materialization.rs) with no
//! dedicated domain struct — they are label-only graph records. Their field
//! contracts live in `data/ontology/node_schemas.yaml` (ADR-050 DATA source);
//! these structs mirror that DATA so the compiled `all_node_schemas()`
//! registry (the schema-check / audit-fields audit surface, ADR-045 Wave E)
//! stays consistent with the external meta-ontology.

use crate::schema::{FieldType, NodeSchemaDef};

/// Immutable article revision node DATA schema (ADR-058 revision plane).
#[derive(Debug, Default)]
pub struct PaperRevisionSchema;

impl NodeSchemaDef for PaperRevisionSchema {
    fn label(&self) -> &'static str {
        "PaperRevision"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("paper_vid", FieldType::String),
            ("canonical_id", FieldType::String),
            ("source_code", FieldType::String),
            ("source_key", FieldType::String),
            ("revision", FieldType::Integer),
            ("content_identity_sha256", FieldType::String),
            ("is_current", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("published_at", FieldType::DateTime),
            ("abstract_text", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            // M434: the shared embedding_write helper stamps the vector
            // digest on the revision content node (content_label path).
            ("embedding_sha256", FieldType::String),
        ]
    }
}

/// Immutable article source manifestation node DATA schema (ADR-058
/// revision plane: pdf/html bytes captured for a revision).
#[derive(Debug, Default)]
pub struct SourceManifestationSchema;

impl NodeSchemaDef for SourceManifestationSchema {
    fn label(&self) -> &'static str {
        "SourceManifestation"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("format", FieldType::String),
            ("sha256", FieldType::String),
            ("bytes", FieldType::Integer),
            ("uri", FieldType::String),
            ("path", FieldType::String),
            ("capture_status", FieldType::String),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("media_type", FieldType::String),
            ("captured_at", FieldType::DateTime),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M434: the shared embedding_write helper stamps the digest on the
    /// revision content node.
    #[test]
    fn paper_revision_schema_has_embedding_sha256() {
        let schema = crate::revision_schema::PaperRevisionSchema;
        let names: Vec<&str> = schema.optional_fields().iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"embedding_sha256"), "{names:?}");
    }
}
