//! Evidence types (ADR-037 §9, ADR-040 §13 EvidenceStore).
//!
//! Every entity/relation must have at least one EvidenceAssertion
//! with a resolvable SourceSpan before import_eligible can be considered.

use serde::{Deserialize, Serialize};

/// Unique ID for an evidence assertion.
pub type EvidenceId = String;

/// How a span is grounded in the source document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    /// Grounded to page + bounding box (best — from ODL layout JSON).
    PageBbox,
    /// Grounded to char offsets only (fallback — from hybrid body markdown).
    CharOnly,
    /// Grounded to TEI XML coordinates.
    Tei,
}

/// Epistemic status of an evidence assertion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicStatus {
    /// Fully verified: artifact exists, hash matches, span in bounds.
    Verified,
    /// Staged: not yet verified but structurally valid.
    Staged,
    /// Pending review.
    Pending,
}

/// A span within a source document pointing to evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub span_type: SpanType,
    pub page: Option<u32>,
    pub bbox: Option<[f64; 4]>,
    pub char_start: Option<usize>,
    pub char_end: Option<usize>,
    pub surface: Option<String>,
    pub artifact_role: String,
    pub artifact_hash: String,
    pub justified_char_only: bool,
}

impl SourceSpan {
    /// Check if this span has page or bbox grounding.
    pub fn has_page_or_bbox(&self) -> bool {
        self.page.is_some() || self.bbox.is_some()
    }
}

/// An evidence assertion linking a claim to an immutable source artifact.
/// ADR-040 §13.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceAssertion {
    pub claim: String,
    pub span_type: SpanType,
    pub page: Option<u32>,
    pub bbox: Option<[f64; 4]>,
    pub char_start: Option<usize>,
    pub char_end: Option<usize>,
    pub artifact_hash: String,
    pub artifact_path: String,
    pub epistemic_status: EpistemicStatus,
    pub created_at: i64,
}

/// Result of verifying an evidence chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceVerification {
    pub node_vid: String,
    pub artifact_exists: bool,
    pub hash_matches: bool,
    pub span_in_bounds: bool,
    pub verdict: EvidenceVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerdict {
    Valid,
    ArtifactMissing,
    HashMismatch,
    SpanOutOfBounds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_has_page_or_bbox() {
        let span = SourceSpan {
            span_type: SpanType::PageBbox,
            page: Some(1),
            bbox: Some([1.0, 2.0, 3.0, 4.0]),
            char_start: None,
            char_end: None,
            surface: None,
            artifact_role: "odl_layout".to_string(),
            artifact_hash: "abc123".to_string(),
            justified_char_only: false,
        };
        assert!(span.has_page_or_bbox());

        let char_only = SourceSpan {
            span_type: SpanType::CharOnly,
            page: None,
            bbox: None,
            char_start: Some(0),
            char_end: Some(10),
            surface: Some("test".to_string()),
            artifact_role: "hybrid_body".to_string(),
            artifact_hash: "abc123".to_string(),
            justified_char_only: true,
        };
        assert!(!char_only.has_page_or_bbox());
    }
}
