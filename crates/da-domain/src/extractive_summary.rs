//! Pure extractive section/abstract summary computation (M371/S10).
//!
//! Deterministic grounded digests from section text — no LLM. Materializers
//! write the resulting payloads into Summary nodes with method=
//! `extractive_lead_v1`. Empty source text is typed missingness, not invented
//! prose.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXTRACTIVE_SUMMARY_METHOD: &str = "extractive_lead_v1";
pub const EXTRACTIVE_SUMMARY_SCHEMA_VERSION: &str = "extractive-summary.v1";

/// One grounded extractive summary ready for graph materialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractiveSummaryUnit {
    pub path_id: String,
    pub title: String,
    pub text: String,
    pub granularity: String,
    pub source_label: String,
    /// Exact immutable graph VID of the owning PaperRevision or Section.
    pub source_id: String,
    pub method: String,
    pub char_count: usize,
    pub source_text_sha256: String,
    pub source_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractiveSummaryBatch {
    pub schema_version: String,
    pub paper_id: String,
    pub abstract_summary: Option<ExtractiveSummaryUnit>,
    pub section_summaries: Vec<ExtractiveSummaryUnit>,
    pub skipped_empty_sections: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtractiveSummaryError {
    #[error("paper_id must not be empty")]
    EmptyPaperId,
    #[error("invalid section input: {0}")]
    InvalidSection(String),
}

/// Pure extractive policy: first N characters of normalized text, preferring
/// sentence boundaries when available.
pub fn extract_lead(text: &str, max_chars: usize) -> String {
    let normalized = normalize_ws(text);
    if normalized.is_empty() || max_chars == 0 {
        return String::new();
    }
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    // Prefer cut at sentence end within window.
    let window: String = normalized.chars().take(max_chars).collect();
    if let Some(idx) = window.rfind(['.', '!', '?']) {
        let candidate = window[..=idx].trim();
        // Accept a real sentence end even when shorter than the full window.
        if candidate.chars().count() >= 12 {
            return candidate.to_string();
        }
    }
    // Fallback: word boundary.
    if let Some(idx) = window.rfind(char::is_whitespace) {
        let candidate = window[..idx].trim();
        if !candidate.is_empty() {
            return candidate.to_string();
        }
    }
    window.trim().to_string()
}

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_sha256(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// Build a deterministic extractive batch from abstract + sections.
pub fn compute_extractive_summaries(
    paper_id: &str,
    revision_vid: &str,
    abstract_text: &str,
    sections: &[(String, String, String, String)], // (path_id, source_vid, title, text)
    max_chars: usize,
) -> Result<ExtractiveSummaryBatch, ExtractiveSummaryError> {
    if paper_id.trim().is_empty() {
        return Err(ExtractiveSummaryError::EmptyPaperId);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut section_summaries = Vec::new();
    let mut skipped_empty_sections = 0_usize;
    if revision_vid.trim().is_empty() {
        return Err(ExtractiveSummaryError::InvalidSection(
            "revision_vid must not be empty".into(),
        ));
    }
    for (path_id, source_vid, title, text) in sections {
        if path_id.trim().is_empty() {
            return Err(ExtractiveSummaryError::InvalidSection(
                "path_id must not be empty".into(),
            ));
        }
        if !seen.insert(path_id.clone()) {
            return Err(ExtractiveSummaryError::InvalidSection(format!(
                "duplicate path_id {path_id}"
            )));
        }
        let lead = extract_lead(text, max_chars);
        if lead.is_empty() {
            skipped_empty_sections += 1;
            continue;
        }
        section_summaries.push(ExtractiveSummaryUnit {
            path_id: path_id.clone(),
            title: title.clone(),
            text: lead.clone(),
            granularity: "section".into(),
            source_label: "Section".into(),
            source_id: source_vid.clone(),
            method: EXTRACTIVE_SUMMARY_METHOD.into(),
            char_count: lead.chars().count(),
            source_text_sha256: text_sha256(text),
            source_chars: text.chars().count(),
        });
    }

    let abstract_summary = {
        let lead = extract_lead(abstract_text, max_chars);
        if lead.is_empty() {
            None
        } else {
            Some(ExtractiveSummaryUnit {
                path_id: "abstract".into(),
                title: "Abstract".into(),
                text: lead.clone(),
                granularity: "abstract".into(),
                source_label: "PaperRevision".into(),
                source_id: revision_vid.to_string(),
                method: EXTRACTIVE_SUMMARY_METHOD.into(),
                char_count: lead.chars().count(),
                source_text_sha256: text_sha256(abstract_text),
                source_chars: abstract_text.chars().count(),
            })
        }
    };

    Ok(ExtractiveSummaryBatch {
        schema_version: EXTRACTIVE_SUMMARY_SCHEMA_VERSION.into(),
        paper_id: paper_id.to_string(),
        abstract_summary,
        section_summaries,
        skipped_empty_sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_lead_prefers_sentence_boundary() {
        let text = "First sentence. Second sentence continues with more detail about GEPA.";
        let lead = extract_lead(text, 40);
        assert_eq!(lead, "First sentence.");
    }

    #[test]
    fn empty_sections_are_skipped_not_invented() {
        let batch = compute_extractive_summaries(
            "2507.19457",
            "revision-1",
            "Problem approach results.",
            &[
                ("1".into(), "section-1".into(), "Intro".into(), "   ".into()),
                (
                    "2".into(),
                    "section-2".into(),
                    "Method".into(),
                    "We propose GEPA for prompt evolution.".into(),
                ),
            ],
            120,
        )
        .unwrap();
        assert!(batch.abstract_summary.is_some());
        assert_eq!(batch.section_summaries.len(), 1);
        assert_eq!(batch.skipped_empty_sections, 1);
        assert_eq!(batch.section_summaries[0].path_id, "2");
        assert_eq!(batch.section_summaries[0].method, EXTRACTIVE_SUMMARY_METHOD);
    }

    #[test]
    fn duplicate_path_id_fails_closed() {
        let err = compute_extractive_summaries(
            "p",
            "revision-1",
            "a",
            &[
                ("1".into(), "section-a".into(), "A".into(), "text a".into()),
                ("1".into(), "section-b".into(), "B".into(), "text b".into()),
            ],
            80,
        )
        .unwrap_err();
        assert!(matches!(err, ExtractiveSummaryError::InvalidSection(_)));
    }

    #[test]
    fn serde_roundtrip() {
        let batch = compute_extractive_summaries(
            "p",
            "revision-1",
            "Abstract body with results 12.3%.",
            &[(
                "1".into(),
                "section-1".into(),
                "Intro".into(),
                "Intro text about GEPA.".into(),
            )],
            80,
        )
        .unwrap();
        let json = serde_json::to_vec(&batch).unwrap();
        let back: ExtractiveSummaryBatch = serde_json::from_slice(&json).unwrap();
        assert_eq!(batch, back);
    }
}
