//! Paper types (ADR-038 Module A — Factual/Metadata).

use crate::vid::Vid;
use serde::{Deserialize, Serialize};

/// Publication status of a paper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperStatus {
    Preprint,
    Published,
    Accepted,
    Submitted,
}

/// A scientific paper (Module A: Factual/Metadata).
/// Schema-as-code: this struct IS the schema (ADR-040 §11.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub vid: Vid,
    pub arxiv_id: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub pdf_hash: Option<String>,
    pub published_at: i64,
    pub ingested_at: i64,
    pub domain_profile: String,
    // Temporality (ADR-037 §6)
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub version: u32,
    pub superseded_by: Option<Vid>,
    // Quality + evidence
    pub evidence_ready: bool,
    pub import_eligible: bool,
    // Schema version (ADR-040 §11.2)
    pub schema_version: u32,
}

impl Paper {
    pub fn new(arxiv_id: &str, title: &str) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            vid: crate::vid::paper_vid(arxiv_id),
            arxiv_id: arxiv_id.to_string(),
            title: title.to_string(),
            abstract_text: None,
            doi: None,
            pdf_hash: None,
            published_at: now,
            ingested_at: now,
            domain_profile: "paper".to_string(),
            valid_from: now,
            valid_to: None,
            version: 1,
            superseded_by: None,
            evidence_ready: false,
            import_eligible: false,
            schema_version: 1,
        }
    }
}

/// Schema definition for Paper nodes.
pub struct PaperSchema;

impl crate::schema::NodeSchemaDef for PaperSchema {
    fn label(&self) -> &'static str {
        "Paper"
    }

    fn required_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("vid", crate::schema::FieldType::String),
            ("arxiv_id", crate::schema::FieldType::String),
            ("title", crate::schema::FieldType::String),
            ("valid_from", crate::schema::FieldType::DateTime),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("abstract_text", crate::schema::FieldType::String),
            ("doi", crate::schema::FieldType::String),
            ("pdf_hash", crate::schema::FieldType::String),
            ("section_count", crate::schema::FieldType::Integer),
            ("citation_count", crate::schema::FieldType::Integer),
            ("valid_to", crate::schema::FieldType::DateTime),
            ("superseded_by", crate::schema::FieldType::String),
            ("evidence_ready", crate::schema::FieldType::Boolean),
            ("import_eligible", crate::schema::FieldType::Boolean),
            ("retrieval_eligible", crate::schema::FieldType::Boolean),
            ("schema_version", crate::schema::FieldType::Integer),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_new() {
        let p = Paper::new("1206.6423", "Seq2Seq Models for KG Link Prediction");
        assert_eq!(p.arxiv_id, "1206.6423");
        assert_eq!(p.vid.len(), 64); // SHA256 hex
        assert!(!p.import_eligible); // D127: always false
        assert!(!p.evidence_ready);
        assert_eq!(p.version, 1);
    }

    #[test]
    fn test_paper_vid_matches() {
        let p = Paper::new("1206.6423", "Test");
        assert_eq!(p.vid, crate::vid::paper_vid("1206.6423"));
    }
}
