//! Application-owned pipeline stage for semantic figure context assembly.

use crate::{FigureContextAssembler, FigureRevisionIdentity, FigureSectionLocator};
use async_trait::async_trait;
use da_domain::{ArticleSummaryContext, DetectedVisualArtifact};
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub struct FigureContextStage {
    artifact_id: String,
    section_locator: FigureSectionLocator,
    article_summary: Option<ArticleSummaryContext>,
    revision_identity: Option<FigureRevisionIdentity>,
    parse_dependency: String,
    figures_dependency: String,
    contract: StageContract,
}

impl FigureContextStage {
    pub fn new(
        artifact_id: impl Into<String>,
        section_locator: FigureSectionLocator,
        article_summary: Option<ArticleSummaryContext>,
    ) -> Self {
        let artifact_id = artifact_id.into();
        Self {
            artifact_id: artifact_id.clone(),
            section_locator,
            article_summary,
            revision_identity: None,
            parse_dependency: "parse".into(),
            figures_dependency: "figures".into(),
            contract: StageContract {
                stage_id: "figure_context".into(),
                implementation_fingerprint: "FigureContextAssembler@explicit-section-v2".into(),
                configuration_fingerprint: format!(
                    "artifact_id={artifact_id};budget=default-18k;neighbors=prev,next;refs=3"
                ),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: "FigureSemanticContext@serde-v2-summary-provenance"
                    .into(),
            },
        }
    }

    pub fn with_dependencies(
        mut self,
        parse_dependency: impl Into<String>,
        figures_dependency: impl Into<String>,
    ) -> Self {
        self.parse_dependency = parse_dependency.into();
        self.figures_dependency = figures_dependency.into();
        self
    }

    pub fn with_output_schema_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.contract.output_schema_fingerprint = fingerprint.into();
        self
    }

    /// Bind revision identity into the assembled context (M400 S01).
    ///
    /// When set, the assembled `FigureSemanticContext` carries the revision
    /// VID, canonical id and revision-scoped section VID. Unset keeps the
    /// stage legacy-unbound; revision binding changes the output schema, so
    /// callers that set it should also override the output schema fingerprint.
    pub fn with_revision_identity(mut self, revision_identity: FigureRevisionIdentity) -> Self {
        self.revision_identity = Some(revision_identity);
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for FigureContextStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let parsed_payload = request
            .dependency_payloads
            .get(&self.parse_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parse_dependency".into(),
                message: format!("missing dependency {}", self.parse_dependency),
            })?;
        let figures_payload = request
            .dependency_payloads
            .get(&self.figures_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_figures_dependency".into(),
                message: format!("missing dependency {}", self.figures_dependency),
            })?;
        let article: ParsedArticle = serde_json::from_slice(parsed_payload).map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "invalid_parsed_article_payload".into(),
                message: error.to_string(),
            }
        })?;
        let artifacts: Vec<DetectedVisualArtifact> = serde_json::from_slice(figures_payload)
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "invalid_figure_artifact_payload".into(),
                message: error.to_string(),
            })?;
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == self.artifact_id)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "figure_artifact_not_found".into(),
                message: format!("artifact {} not found", self.artifact_id),
            })?;
        let parsed_summary = self.article_summary.clone().or_else(|| {
            (!article.abstract_text.trim().is_empty()).then(|| ArticleSummaryContext {
                source_id: format!("grobid:{}#abstract", request.article_id),
                source_text_sha256: hex::encode(Sha256::digest(article.abstract_text.as_bytes())),
                text: article.abstract_text.clone(),
                truncated: false,
            })
        });
        let context = FigureContextAssembler::with_default_budget()
            .assemble(
                artifact,
                &article,
                Some(&self.section_locator),
                parsed_summary.as_ref(),
                self.revision_identity.as_ref(),
            )
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "figure_context_contract".into(),
                message: error.to_string(),
            })?;
        let payload =
            serde_json::to_vec(&context).map_err(|error| StageExecutionError::Terminal {
                error_class: "figure_context_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("artifact_id".into(), context.artifact_id),
                ("section_path".into(), context.containing_section.path_id),
                (
                    "reference_count".into(),
                    context.reference_excerpts.len().to_string(),
                ),
                (
                    "context_chars".into(),
                    context.total_context_chars.to_string(),
                ),
                ("truncated".into(), context.truncated.to_string()),
            ]),
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{BoundingBox72Dpi, VisualArtifactKind};
    use da_ports::parser::{CitationEntry, Section};

    fn request() -> StageExecutionRequest {
        let article = ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "GEPA abstract".into(),
            body_text: String::new(),
            sections: vec![Section {
                title: "GEPA System".into(),
                text: "As shown in Figure 3, GEPA mutates prompts.".into(),
                level: 1,
                path_id: Some("3".into()),
                parent_section_id: None,
                order_in_parent: Some(0),
                section_id: Some("sec-3".into()),
            }],
            citations: Vec::<CitationEntry>::new(),
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            pdf_hash: "a".repeat(64),
        };
        let artifact = DetectedVisualArtifact {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "detector-v1".into(),
            source_pdf_sha256: "a".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 1.0,
                y1: 1.0,
                x2: 2.0,
                y2: 2.0,
            },
            caption_region: BoundingBox72Dpi {
                x1: 1.0,
                y1: 3.0,
                x2: 2.0,
                y2: 4.0,
            },
            caption: "Figure 3: GEPA flowchart".into(),
            image_text: vec!["GEPA".into()],
            render_dpi: 150,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "b".repeat(64),
            metadata_sha256: "c".repeat(64),
        };
        StageExecutionRequest {
            article_id: "2507.19457".into(),
            dependency_payloads: BTreeMap::from([
                ("parse".into(), serde_json::to_vec(&article).unwrap()),
                (
                    "figures".into(),
                    serde_json::to_vec(&vec![artifact]).unwrap(),
                ),
            ]),
        }
    }

    #[tokio::test]
    async fn context_stage_consumes_named_dependencies() {
        let stage =
            FigureContextStage::new("Figure3", FigureSectionLocator::PathId("3".into()), None);
        let output = stage.execute(request()).await.unwrap();
        assert_eq!(output.metadata["artifact_id"], "Figure3");
        assert_eq!(output.metadata["section_path"], "3");
        assert_eq!(output.metadata["reference_count"], "1");
    }

    #[tokio::test]
    async fn missing_dependency_and_artifact_are_explicit() {
        let stage =
            FigureContextStage::new("missing", FigureSectionLocator::PathId("3".into()), None);
        let error = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap_err();
        assert!(matches!(error, StageExecutionError::Terminal { .. }));
        let error = stage.execute(request()).await.unwrap_err();
        assert!(matches!(
            error,
            StageExecutionError::Terminal { ref error_class, .. }
                if error_class == "figure_artifact_not_found"
        ));
    }

    #[test]
    fn schema_override_changes_only_context_contract() {
        let before =
            FigureContextStage::new("Figure3", FigureSectionLocator::PathId("3".into()), None);
        let after =
            FigureContextStage::new("Figure3", FigureSectionLocator::PathId("3".into()), None)
                .with_output_schema_fingerprint("FigureSemanticContext@serde-v3");
        assert_ne!(
            before.contract().output_schema_fingerprint,
            after.contract().output_schema_fingerprint
        );
    }

    #[test]
    fn artifact_id_is_part_of_configuration_fingerprint() {
        let one =
            FigureContextStage::new("Figure1", FigureSectionLocator::PathId("1".into()), None);
        let two =
            FigureContextStage::new("Figure2", FigureSectionLocator::PathId("2".into()), None);
        assert_ne!(
            one.contract().configuration_fingerprint,
            two.contract().configuration_fingerprint
        );
        assert!(
            one.contract()
                .configuration_fingerprint
                .contains("artifact_id=Figure1"),
            "{}",
            one.contract().configuration_fingerprint
        );
    }

    #[tokio::test]
    async fn revision_identity_binds_into_stage_payload() {
        let revision_vid = "e".repeat(64);
        let stage =
            FigureContextStage::new("Figure3", FigureSectionLocator::PathId("3".into()), None)
                .with_revision_identity(crate::FigureRevisionIdentity {
                    revision_vid: revision_vid.clone(),
                    canonical_id: "2507.19457".into(),
                })
                .with_output_schema_fingerprint("FigureSemanticContext@serde-v4-revision-bound");
        let output = stage.execute(request()).await.unwrap();
        let context: da_domain::FigureSemanticContext =
            serde_json::from_slice(&output.payload).unwrap();
        assert_eq!(context.revision_vid.as_deref(), Some(revision_vid.as_str()));
        assert_eq!(context.canonical_id.as_deref(), Some("2507.19457"));
        // Single-section fixture: containing ordinal is 1.
        assert_eq!(context.containing_section.section_ordinal, Some(1));
        let expected = da_domain::vid::revision_section_vid(&revision_vid, 1).unwrap();
        assert_eq!(context.section_vid.as_deref(), Some(expected.as_str()));
        context.validate().unwrap();
    }
}
