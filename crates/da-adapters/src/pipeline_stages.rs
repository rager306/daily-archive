//! Production pipeline-stage adapters for parsing and figure detection (M371 S02).

use async_trait::async_trait;
use da_ports::figure_detection::{FigureDetectionPort, FigureDetectionRequest};
use da_ports::parser::ParserPort;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct GrobidParseStage {
    parser: Arc<dyn ParserPort>,
    pdf_path: PathBuf,
    retry_after: String,
    contract: StageContract,
}

impl GrobidParseStage {
    pub fn new(
        parser: Arc<dyn ParserPort>,
        pdf_path: impl Into<PathBuf>,
        retry_after: impl Into<String>,
    ) -> Self {
        Self {
            parser,
            pdf_path: pdf_path.into(),
            retry_after: retry_after.into(),
            contract: StageContract {
                stage_id: "grobid_parse".into(),
                implementation_fingerprint: "GrobidParser@tei-navigation-v3".into(),
                configuration_fingerprint:
                    "consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note"
                        .into(),
                prompt_fingerprint: None,
                model_fingerprint: Some("grobid/grobid:0.9.0-crf".into()),
                output_schema_fingerprint: "ParsedArticle@serde-v3-unique-navigation".into(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for GrobidParseStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let pdf_path = self
            .pdf_path
            .to_str()
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "invalid_pdf_path".into(),
                message: format!("PDF path is not UTF-8: {}", self.pdf_path.display()),
            })?;
        let parsed = self
            .parser
            .parse_pdf(pdf_path, &request.article_id)
            .await
            .map_err(|error| StageExecutionError::Retryable {
                error_class: "parser_unavailable_or_failed".into(),
                message: error.to_string(),
                retry_after: self.retry_after.clone(),
            })?;
        let payload =
            serde_json::to_vec(&parsed).map_err(|error| StageExecutionError::Terminal {
                error_class: "parsed_article_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("sections".into(), parsed.sections.len().to_string()),
                ("citations".into(), parsed.citations.len().to_string()),
                ("pdf_hash".into(), parsed.pdf_hash.clone()),
            ]),
            payload,
        })
    }
}

pub struct FigureDetectionStage {
    detector: Arc<dyn FigureDetectionPort>,
    request: FigureDetectionRequest,
    retry_after: String,
    contract: StageContract,
}

impl FigureDetectionStage {
    pub fn new(
        detector: Arc<dyn FigureDetectionPort>,
        request: FigureDetectionRequest,
        retry_after: impl Into<String>,
    ) -> Self {
        let detector_fingerprint = detector.detector_fingerprint().to_owned();
        Self {
            detector,
            request,
            retry_after: retry_after.into(),
            contract: StageContract {
                stage_id: "figure_detect".into(),
                implementation_fingerprint:
                    "PdfFigures2Adapter@canonical-manifest-v3-stable-metadata".into(),
                configuration_fingerprint: "render_dpi=150;format=png;manifest_last=true".into(),
                prompt_fingerprint: None,
                model_fingerprint: Some(detector_fingerprint),
                output_schema_fingerprint: "DetectedVisualArtifact[]@serde-v1".into(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for FigureDetectionStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        if request.article_id != self.request.article_id {
            return Err(StageExecutionError::Terminal {
                error_class: "article_identity_mismatch".into(),
                message: format!(
                    "stage configured for {}, requested {}",
                    self.request.article_id, request.article_id
                ),
            });
        }
        let result = self
            .detector
            .detect(self.request.clone())
            .await
            .map_err(|error| StageExecutionError::Retryable {
                error_class: "figure_detector_unavailable_or_failed".into(),
                message: error.to_string(),
                retry_after: self.retry_after.clone(),
            })?;
        let payload = serde_json::to_vec(&result.artifacts).map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "figure_artifact_serialization".into(),
                message: error.to_string(),
            }
        })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("artifacts".into(), result.artifacts.len().to_string()),
                (
                    "detector_metadata_sha256".into(),
                    result.detector_metadata_sha256,
                ),
                ("manifest_sha256".into(), result.manifest_sha256),
                ("duration_ms".into(), result.duration_ms.to_string()),
            ]),
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::figure_detection::{
        FigureDetectionError, FigureDetectionPortResult, FigureDetectionResult,
    };
    use da_ports::parser::{ParseResult, ParsedArticle};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeParser {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ParserPort for FakeParser {
        async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ParsedArticle {
                paper_id: paper_id.into(),
                title: "GEPA".into(),
                abstract_text: "Reflective prompt evolution".into(),
                body_text: "body".into(),
                sections: vec![],
                citations: vec![],
                layout_json: None,
                tei_xml: None,
                pdf_hash: "a".repeat(64),
            })
        }
    }

    struct FailingDetector;

    #[async_trait]
    impl FigureDetectionPort for FailingDetector {
        fn detector_fingerprint(&self) -> &str {
            "fake-detector-v1"
        }

        async fn detect(
            &self,
            _request: FigureDetectionRequest,
        ) -> FigureDetectionPortResult<FigureDetectionResult> {
            Err(FigureDetectionError::Timeout { timeout_ms: 20 })
        }
    }

    #[tokio::test]
    async fn parse_stage_emits_serialized_article_and_metadata() {
        let parser = Arc::new(FakeParser {
            calls: AtomicUsize::new(0),
        });
        let stage =
            GrobidParseStage::new(parser.clone(), "/catalog/paper.pdf", "2026-08-08T01:00:00Z");
        let output = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap();
        let parsed: ParsedArticle = serde_json::from_slice(&output.payload).unwrap();
        assert_eq!(parsed.paper_id, "2507.19457");
        assert_eq!(output.metadata["pdf_hash"], "a".repeat(64));
        assert_eq!(parser.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn detector_failure_is_retryable_and_preserves_retry_after() {
        let stage = FigureDetectionStage::new(
            Arc::new(FailingDetector),
            FigureDetectionRequest {
                article_id: "2507.19457".into(),
                source_pdf_path: "/catalog/paper.pdf".into(),
                source_pdf_sha256: "a".repeat(64),
                canonical_article_dir: "/catalog/paper".into(),
            },
            "2026-08-08T01:00:00Z",
        );
        let error = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            StageExecutionError::Retryable { ref retry_after, .. }
                if retry_after == "2026-08-08T01:00:00Z"
        ));
    }

    #[tokio::test]
    async fn detector_stage_rejects_article_identity_mismatch() {
        let stage = FigureDetectionStage::new(
            Arc::new(FailingDetector),
            FigureDetectionRequest {
                article_id: "configured".into(),
                source_pdf_path: "/catalog/paper.pdf".into(),
                source_pdf_sha256: "a".repeat(64),
                canonical_article_dir: "/catalog/paper".into(),
            },
            "2026-08-08T01:00:00Z",
        );
        let error = stage
            .execute(StageExecutionRequest {
                article_id: "other".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap_err();
        assert!(matches!(error, StageExecutionError::Terminal { .. }));
    }
}
