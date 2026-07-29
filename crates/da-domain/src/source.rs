//! Source provenance node (ONTOLOGY-DESIGN Layer 0).
//!
//! Tracks where data came from — enables multi-source federation.
//! Every Work links to exactly one Source via FROM_SOURCE edge.

use crate::schema::{FieldType, NodeSchemaDef};

/// Source node schema (ONTOLOGY-DESIGN Layer 0).
pub struct SourceSchema;

impl NodeSchemaDef for SourceSchema {
    fn label(&self) -> &'static str {
        "Source"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("code", FieldType::String),
            ("source_type", FieldType::String),
            ("domain", FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("reliability_tier", FieldType::Integer),
            ("access_method", FieldType::String),
            ("base_url", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

/// Known source codes (closed vocabulary, extensible).
pub const SOURCE_ARXIV: &str = "arxiv";
pub const SOURCE_TEXTBOOK: &str = "textbook";
pub const SOURCE_STANFORD: &str = "stanford";
pub const SOURCE_OPENALEX: &str = "openalex";
pub const SOURCE_CROSSREF: &str = "crossref";
pub const SOURCE_SEMANTIC_SCHOLAR: &str = "semantic_scholar";

/// Known source types.
pub const TYPE_PDF: &str = "pdf";
pub const TYPE_HTML: &str = "html";
pub const TYPE_MARKDOWN: &str = "markdown";
pub const TYPE_API_JSON: &str = "api_json";
pub const TYPE_LATEX: &str = "latex";

/// Known domain profiles.
pub const DOMAIN_PAPER: &str = "scientific_paper";
pub const DOMAIN_TEXTBOOK: &str = "textbook";
pub const DOMAIN_LECTURE: &str = "lecture_notes";
pub const DOMAIN_CODE: &str = "code_repo";
pub const DOMAIN_BLOG: &str = "blog";

/// Reliability tiers (ONTOLOGY-DESIGN §3).
pub const TIER_CURATED: i64 = 1; // OpenAlex, Crossref
pub const TIER_EXTRACTED: i64 = 2; // GROBID, HTML parser
pub const TIER_USER: i64 = 3; // User-provided

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_source_schema_label() {
        assert_eq!(SourceSchema.label(), "Source");
    }

    #[test]
    fn test_source_schema_required_fields() {
        let fields = SourceSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"vid"));
        assert!(names.contains(&"code"));
        assert!(names.contains(&"source_type"));
        assert!(names.contains(&"domain"));
    }

    #[test]
    fn test_source_schema_validates() {
        let schema = SourceSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::json!("vid:source:arxiv"));
        props.insert("code".to_string(), serde_json::json!("arxiv"));
        props.insert("source_type".to_string(), serde_json::json!("pdf"));
        props.insert("domain".to_string(), serde_json::json!("scientific_paper"));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_source_schema_rejects_missing() {
        let schema = SourceSchema;
        let props = HashMap::new();
        assert!(schema.validate(&props).is_err());
    }

    #[test]
    fn test_known_source_constants() {
        assert_eq!(SOURCE_ARXIV, "arxiv");
        assert_eq!(SOURCE_TEXTBOOK, "textbook");
        assert_eq!(SOURCE_OPENALEX, "openalex");
        assert_eq!(TYPE_PDF, "pdf");
        assert_eq!(TYPE_HTML, "html");
        assert_eq!(DOMAIN_PAPER, "scientific_paper");
        assert_eq!(DOMAIN_TEXTBOOK, "textbook");
        assert_eq!(TIER_CURATED, 1);
        assert_eq!(TIER_EXTRACTED, 2);
    }
}
