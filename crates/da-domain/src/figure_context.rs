//! Semantic context prepared for figure/table VLM analysis (M371 S02).
//!
//! Context preserves the detector artifact provenance and explicit document-tree
//! section resolution. It is candidate evidence only and never graph truth.

use crate::{BoundingBox72Dpi, VisualArtifactKind};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureSectionRelation {
    Containing,
    Previous,
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureSectionContext {
    pub relation: FigureSectionRelation,
    pub section_id: String,
    pub path_id: String,
    pub title: String,
    pub text: String,
    pub truncated: bool,
    /// One-based document ordinal (M400 S01). `None` only for contexts
    /// serialized before revision binding; newly assembled contexts always
    /// carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_ordinal: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleSummaryContext {
    pub source_id: String,
    pub source_text_sha256: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureReferenceExcerpt {
    pub section_id: String,
    pub path_id: String,
    pub title: String,
    pub matched_form: String,
    pub excerpt: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureSemanticContext {
    pub artifact_id: String,
    pub kind: VisualArtifactKind,
    pub asset_path: String,
    pub asset_sha256: String,
    pub source_pdf_sha256: String,
    /// PaperRevision VID this context is grounded in (M400 S01). `None` keeps
    /// legacy unbound contexts serialized before revision binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_vid: Option<String>,
    /// Versionless canonical article id (e.g. `2507.19457`), bound together
    /// with `revision_vid` and `section_vid` (all or none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_id: Option<String>,
    /// Revision-scoped Section VID
    /// (`revision_section_vid(revision_vid, containing section ordinal)`) —
    /// exact section lineage into the candidate plane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_vid: Option<String>,
    pub detector_fingerprint: String,
    pub detector_metadata_sha256: String,
    pub page_index: u32,
    pub region: BoundingBox72Dpi,
    pub caption: String,
    pub containing_section: FigureSectionContext,
    pub neighboring_sections: Vec<FigureSectionContext>,
    pub reference_excerpts: Vec<FigureReferenceExcerpt>,
    pub article_summary: Option<ArticleSummaryContext>,
    pub diagnostics: Vec<String>,
    pub total_context_chars: usize,
    pub truncated: bool,
}

impl FigureSemanticContext {
    pub fn validate(&self) -> Result<(), FigureContextError> {
        self.region
            .validate()
            .map_err(|error| FigureContextError::Invalid(error.to_string()))?;
        if self.artifact_id.trim().is_empty() {
            return Err(FigureContextError::Invalid("artifact_id is empty".into()));
        }
        if self.caption.trim().is_empty() {
            return Err(FigureContextError::Invalid("caption is empty".into()));
        }
        if self.containing_section.relation != FigureSectionRelation::Containing {
            return Err(FigureContextError::Invalid(
                "containing section relation must be containing".into(),
            ));
        }
        validate_section_identity(&self.containing_section)?;
        for section in &self.neighboring_sections {
            if section.relation == FigureSectionRelation::Containing {
                return Err(FigureContextError::Invalid(
                    "neighbor cannot have containing relation".into(),
                ));
            }
            validate_section_identity(section)?;
        }
        if self.asset_sha256.len() != 64
            || self.source_pdf_sha256.len() != 64
            || self.detector_metadata_sha256.len() != 64
            || self
                .article_summary
                .as_ref()
                .is_some_and(|summary| summary.source_text_sha256.len() != 64)
        {
            return Err(FigureContextError::Invalid(
                "figure context SHA-256 fields must contain 64 hex characters".into(),
            ));
        }
        self.validate_revision_identity()?;
        Ok(())
    }

    /// Revision identity is all-or-none: `revision_vid`, `canonical_id` and
    /// `section_vid` bind together, or the context stays legacy-unbound.
    /// When bound, `section_vid` must equal the revision-scoped Section VID
    /// derived from the containing section's one-based ordinal.
    fn validate_revision_identity(&self) -> Result<(), FigureContextError> {
        let bound = [
            self.revision_vid.is_some(),
            self.canonical_id.is_some(),
            self.section_vid.is_some(),
        ];
        let bound_count = bound.iter().filter(|flag| **flag).count();
        if bound_count != 0 && bound_count != 3 {
            return Err(FigureContextError::MixedRevisionIdentity);
        }
        if bound_count == 0 {
            return Ok(());
        }
        let revision_vid = self.revision_vid.as_deref().expect("bound");
        let canonical_id = self.canonical_id.as_deref().expect("bound");
        let section_vid = self.section_vid.as_deref().expect("bound");
        if revision_vid.len() != 64 || !revision_vid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(FigureContextError::Invalid(
                "revision_vid must be a 64-character SHA256 VID".into(),
            ));
        }
        if canonical_id.trim().is_empty() {
            return Err(FigureContextError::Invalid(
                "canonical_id must be non-empty".into(),
            ));
        }
        if section_vid.len() != 64 || !section_vid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(FigureContextError::Invalid(
                "section_vid must be a 64-character SHA256 VID".into(),
            ));
        }
        let ordinal = self
            .containing_section
            .section_ordinal
            .ok_or(FigureContextError::RevisionSectionVidMismatch)?;
        let expected = crate::vid::revision_section_vid(revision_vid, ordinal as usize)
            .map_err(FigureContextError::Invalid)?;
        if expected != section_vid {
            return Err(FigureContextError::RevisionSectionVidMismatch);
        }
        Ok(())
    }
}

fn validate_section_identity(section: &FigureSectionContext) -> Result<(), FigureContextError> {
    if section.section_id.trim().is_empty() || section.path_id.trim().is_empty() {
        return Err(FigureContextError::Invalid(
            "resolved sections require section_id and path_id".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FigureContextError {
    #[error("figure context requires an explicit containing section locator")]
    MissingSectionLocator,
    #[error("containing section was not found for locator '{0}'")]
    SectionNotFound(String),
    #[error("containing section locator '{locator}' matched {matches} sections")]
    AmbiguousSectionLocator { locator: String, matches: usize },
    #[error("resolved section '{0}' lacks section_id or path_id provenance")]
    MissingSectionProvenance(String),
    #[error("figure context revision identity is partially bound (all or none required)")]
    MixedRevisionIdentity,
    #[error("figure context section VID does not match revision and containing section ordinal")]
    RevisionSectionVidMismatch,
    #[error("invalid figure context: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(relation: FigureSectionRelation) -> FigureSectionContext {
        FigureSectionContext {
            relation,
            section_id: "section-3".into(),
            path_id: "3".into(),
            title: "GEPA System".into(),
            text: "GEPA proposes reflective prompt mutation.".into(),
            truncated: false,
            section_ordinal: None,
        }
    }

    #[test]
    fn context_roundtrips_with_source_provenance() {
        let context = FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "a".repeat(64),
            source_pdf_sha256: "b".repeat(64),
            revision_vid: None,
            canonical_id: None,
            section_vid: None,
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            detector_metadata_sha256: "c".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 20.0,
                x2: 100.0,
                y2: 120.0,
            },
            caption: "Figure 3: GEPA flowchart".into(),
            containing_section: section(FigureSectionRelation::Containing),
            neighboring_sections: vec![section(FigureSectionRelation::Previous)],
            reference_excerpts: vec![],
            article_summary: Some(ArticleSummaryContext {
                source_id: "summary-node-1".into(),
                source_text_sha256: "d".repeat(64),
                text: "GEPA improves performance with fewer rollouts.".into(),
                truncated: false,
            }),
            diagnostics: vec![],
            total_context_chars: 120,
            truncated: false,
        };
        context.validate().unwrap();
        let json = serde_json::to_string(&context).unwrap();
        let decoded: FigureSemanticContext = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, context);
    }

    #[test]
    fn rejects_neighbor_masquerading_as_containing_section() {
        let mut neighbor = section(FigureSectionRelation::Containing);
        neighbor.section_id = "neighbor".into();
        let mut context = FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "a".repeat(64),
            source_pdf_sha256: "b".repeat(64),
            revision_vid: None,
            canonical_id: None,
            section_vid: None,
            detector_fingerprint: "pdffigures2".into(),
            detector_metadata_sha256: "c".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 1.0,
                y1: 1.0,
                x2: 2.0,
                y2: 2.0,
            },
            caption: "Figure 3".into(),
            containing_section: section(FigureSectionRelation::Containing),
            neighboring_sections: vec![],
            reference_excerpts: vec![],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: 0,
            truncated: false,
        };
        context.neighboring_sections.push(neighbor);
        assert!(context.validate().is_err());
    }

    // ─── M400 S01 T02: revision-bound context identity ───

    fn revision_vid() -> String {
        "e".repeat(64)
    }

    /// Build the source-provenance context bound to a revision and a
    /// revision-scoped section VID for ordinal 2.
    fn bound_context() -> FigureSemanticContext {
        let context = FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "a".repeat(64),
            source_pdf_sha256: "b".repeat(64),
            revision_vid: Some(revision_vid()),
            canonical_id: Some("2507.19457".into()),
            section_vid: Some(crate::vid::revision_section_vid(&revision_vid(), 2).unwrap()),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            detector_metadata_sha256: "c".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 20.0,
                x2: 100.0,
                y2: 120.0,
            },
            caption: "Figure 3: GEPA flowchart".into(),
            containing_section: {
                let mut section = section(FigureSectionRelation::Containing);
                section.section_ordinal = Some(2);
                section
            },
            neighboring_sections: vec![section(FigureSectionRelation::Previous)],
            reference_excerpts: vec![],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: 0,
            truncated: false,
        };
        assert!(context.validate().is_ok());
        context
    }

    #[test]
    fn revision_bound_context_validates_and_roundtrips() {
        let context = bound_context();
        assert_eq!(
            context.revision_vid.as_deref(),
            Some(revision_vid().as_str())
        );
        assert_eq!(context.canonical_id.as_deref(), Some("2507.19457"));
        assert_eq!(
            context.section_vid.as_deref(),
            Some(
                crate::vid::revision_section_vid(&revision_vid(), 2)
                    .unwrap()
                    .as_str()
            )
        );
        assert_eq!(context.containing_section.section_ordinal, Some(2));
        crate::roundtrip_json(&context);
        let decoded: FigureSemanticContext =
            serde_json::from_str(&serde_json::to_string(&context).unwrap()).unwrap();
        assert_eq!(decoded, context);
        decoded.validate().unwrap();
    }

    #[test]
    fn legacy_json_without_revision_fields_deserializes_unbound() {
        // Serialize a legacy-shaped context (no revision fields, no ordinal)
        // exactly as it existed before M400 S01 revision binding.
        let json = serde_json::to_string(&FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "a".repeat(64),
            source_pdf_sha256: "b".repeat(64),
            revision_vid: None,
            canonical_id: None,
            section_vid: None,
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            detector_metadata_sha256: "c".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 20.0,
                x2: 100.0,
                y2: 120.0,
            },
            caption: "Figure 3: GEPA flowchart".into(),
            containing_section: section(FigureSectionRelation::Containing),
            neighboring_sections: vec![section(FigureSectionRelation::Previous)],
            reference_excerpts: vec![],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: 0,
            truncated: false,
        })
        .unwrap();
        // The serialized form must not carry the new fields when unbound.
        assert!(!json.contains("revision_vid"));
        assert!(!json.contains("section_vid"));
        assert!(!json.contains("section_ordinal"));
        // Old fixtures without the fields still deserialize (serde default).
        let decoded: FigureSemanticContext = serde_json::from_str(&json).unwrap();
        assert!(decoded.revision_vid.is_none());
        assert!(decoded.canonical_id.is_none());
        assert!(decoded.section_vid.is_none());
        assert_eq!(decoded.containing_section.section_ordinal, None);
        decoded.validate().unwrap();
    }

    #[test]
    fn rejects_partially_bound_revision_identity() {
        let mut context = bound_context();
        context.canonical_id = None;
        assert_eq!(
            context.validate(),
            Err(FigureContextError::MixedRevisionIdentity)
        );
        let mut context = bound_context();
        context.section_vid = None;
        assert_eq!(
            context.validate(),
            Err(FigureContextError::MixedRevisionIdentity)
        );
        let mut context = bound_context();
        context.revision_vid = None;
        assert_eq!(
            context.validate(),
            Err(FigureContextError::MixedRevisionIdentity)
        );
    }

    #[test]
    fn rejects_section_vid_mismatch_and_missing_ordinal() {
        let mut context = bound_context();
        context.section_vid = Some("f".repeat(64));
        assert_eq!(
            context.validate(),
            Err(FigureContextError::RevisionSectionVidMismatch)
        );
        let mut context = bound_context();
        context.containing_section.section_ordinal = None;
        assert_eq!(
            context.validate(),
            Err(FigureContextError::RevisionSectionVidMismatch)
        );
        // A section VID for a different ordinal must also mismatch.
        let mut context = bound_context();
        context.section_vid = Some(crate::vid::revision_section_vid(&revision_vid(), 3).unwrap());
        assert_eq!(
            context.validate(),
            Err(FigureContextError::RevisionSectionVidMismatch)
        );
    }

    #[test]
    fn rejects_malformed_bound_revision_identity() {
        let mut context = bound_context();
        context.revision_vid = Some("short".into());
        assert!(context.validate().is_err());
        let mut context = bound_context();
        context.canonical_id = Some("   ".into());
        assert!(context.validate().is_err());
        let mut context = bound_context();
        context.section_vid = Some("not-hex".into());
        assert!(context.validate().is_err());
    }
}
