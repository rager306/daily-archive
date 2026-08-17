//! Stage-1 figure type classification (ADR-060).
//!
//! Cheap heuristic path over FigureSemanticContext caption + citing prose.
//! Candidate-only: never authoritative, never import_eligible.

use da_domain::{
    FigureAnalysisError, FigureSemanticContext, FigureTypeClassification, FigureTypeLabelTable,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FigureClassifyError {
    #[error(transparent)]
    Domain(#[from] FigureAnalysisError),
    #[error("figure context invalid: {0}")]
    Context(String),
}

/// Stage-1 classifier. Holds a DATA-driven label table (bundled YAML by default).
pub struct FigureClassifyUseCase {
    labels: FigureTypeLabelTable,
}

impl FigureClassifyUseCase {
    pub fn bundled() -> Result<Self, FigureClassifyError> {
        Ok(Self {
            labels: FigureTypeLabelTable::bundled()?,
        })
    }

    pub fn new(labels: FigureTypeLabelTable) -> Self {
        Self { labels }
    }

    pub fn labels_schema_version(&self) -> &str {
        &self.labels.schema_version
    }

    /// Classify from grounded context text only (no image pixels in heuristic path).
    pub fn classify(
        &self,
        context: &FigureSemanticContext,
    ) -> Result<FigureTypeClassification, FigureClassifyError> {
        context
            .validate()
            .map_err(|e| FigureClassifyError::Context(e.to_string()))?;

        let mut text = String::new();
        text.push_str(&context.caption);
        text.push(' ');
        text.push_str(&context.containing_section.title);
        text.push(' ');
        text.push_str(&context.containing_section.text);
        for section in &context.neighboring_sections {
            text.push(' ');
            text.push_str(&section.title);
            text.push(' ');
            text.push_str(&section.text);
        }
        for reference in &context.reference_excerpts {
            text.push(' ');
            text.push_str(&reference.excerpt);
        }
        if let Some(summary) = &context.article_summary {
            text.push(' ');
            text.push_str(&summary.text);
        }

        Ok(self.labels.classify_from_text(
            &context.artifact_id,
            &text,
            &context.detector_metadata_sha256,
            &context.asset_sha256,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{
        BoundingBox72Dpi, FigureSectionContext, FigureSectionRelation, VisualAnalysisType,
        VisualArtifactKind,
    };

    fn sha64(seed: u8) -> String {
        format!("{:064x}", seed as u128)
    }

    fn context_with_caption(caption: &str) -> FigureSemanticContext {
        FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/fig.png".into(),
            asset_sha256: sha64(11),
            source_pdf_sha256: sha64(12),
            revision_vid: None,
            canonical_id: None,
            section_vid: None,
            detector_fingerprint: "pdffigures2@test".into(),
            detector_metadata_sha256: sha64(13),
            page_index: 3,
            region: BoundingBox72Dpi {
                x1: 0.0,
                y1: 0.0,
                x2: 100.0,
                y2: 100.0,
            },
            caption: caption.into(),
            containing_section: FigureSectionContext {
                relation: FigureSectionRelation::Containing,
                section_id: "sec-3".into(),
                path_id: "1.3".into(),
                title: "Method".into(),
                text: "The algorithm evaluates candidates on a minibatch.".into(),
                truncated: false,
                section_ordinal: None,
            },
            neighboring_sections: vec![],
            reference_excerpts: vec![],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: caption.len(),
            truncated: false,
        }
    }

    #[test]
    fn heuristic_stage1_classifies_gepa_overview_as_flowchart() {
        let uc = FigureClassifyUseCase::bundled().expect("bundled");
        let ctx = context_with_caption(
            "Figure 3: Overview of the GEPA algorithm. Candidate proposal uses reflective prompt mutation and system aware merge in an iterative loop.",
        );
        let c = uc.classify(&ctx).expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Flowchart);
        assert!(!c.authoritative && !c.import_eligible);
        assert_eq!(c.method, da_domain::FigureClassifyMethod::Heuristic);
        assert!(c.confidence >= 0.5);
        c.validate().unwrap();
    }

    #[test]
    fn heuristic_stage1_classifies_accuracy_plot() {
        let uc = FigureClassifyUseCase::bundled().expect("bundled");
        let ctx = context_with_caption(
            "Figure 4: Accuracy vs rollouts scatter plot for performance comparison.",
        );
        let c = uc.classify(&ctx).expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Plot);
    }

    #[test]
    fn heuristic_stage1_defaults_other_without_keywords() {
        let uc = FigureClassifyUseCase::bundled().expect("bundled");
        let ctx = context_with_caption("Figure 9: Illustration of the experimental setup.");
        let c = uc.classify(&ctx).expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Other);
        assert!(c.confidence <= 0.35);
    }
}
