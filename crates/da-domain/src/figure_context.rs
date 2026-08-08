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
}
