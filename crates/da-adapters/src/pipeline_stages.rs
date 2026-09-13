//! Production pipeline-stage adapters for parsing and figure detection (M371 S02).

use async_trait::async_trait;
use da_parse::PROJECT_ARTICLE_SCHEMA_FINGERPRINT;
use da_ports::embedder::{Embedder, EmbedderError};
use da_ports::extractor::Extractor;
use da_ports::figure_detection::{FigureDetectionPort, FigureDetectionRequest};
use da_ports::parser::{ParsedArticle, ParserPort};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Historical baseline model identity for the Java GROBID transport
/// (`grobid/grobid:0.9.0-crf`, the D204/D321 baseline).
///
/// This is a legacy/rollback default only. Runtime callers that parsed
/// against a live service must stamp the ACTUAL probed release through
/// [`GrobidParseStage::with_backend_version`] so a Java GROBID upgrade
/// rotates the parse cache key instead of reusing another release's TEI
/// (ADR-076 W0 `backend_identity`).
pub const HISTORICAL_GROBID_MODEL_FINGERPRINT: &str = "grobid/grobid:0.9.0-crf";

pub struct GrobidParseStage {
    parser: Arc<dyn ParserPort>,
    pdf_path: PathBuf,
    retry_after: String,
    retry_delay_seconds: Option<i64>,
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
            retry_delay_seconds: None,
            contract: StageContract {
                stage_id: "grobid_parse".into(),
                implementation_fingerprint: "GrobidParser@tei-navigation-v3".into(),
                configuration_fingerprint:
                    "consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula"
                        .into(),
                prompt_fingerprint: None,
                model_fingerprint: Some(HISTORICAL_GROBID_MODEL_FINGERPRINT.into()),
                output_schema_fingerprint: PROJECT_ARTICLE_SCHEMA_FINGERPRINT.into(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }

    /// Stamp the ACTUAL probed `/api/version` string into this stage's cache
    /// identity at runtime (`model_fingerprint = grobid/grobid:{version}`),
    /// overriding the historical baseline default. Blank input is ignored and
    /// keeps the baseline. Callers wiring a live Java GROBID parse must use
    /// this with the probed release so two releases never share stage entries
    /// (ADR-076 W0 `backend_identity`).
    pub fn with_backend_version(mut self, version: &str) -> Self {
        if let Some(fingerprint) = da_parse::grobid_model_fingerprint(version) {
            self.contract.model_fingerprint = Some(fingerprint);
        }
        self
    }

    pub fn with_retry_delay_seconds(mut self, seconds: i64) -> Self {
        self.retry_delay_seconds = Some(seconds);
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
                retry_after: self.retry_delay_seconds.map_or_else(
                    || self.retry_after.clone(),
                    |seconds| {
                        (chrono::Utc::now() + chrono::Duration::seconds(seconds)).to_rfc3339()
                    },
                ),
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

pub struct ParsedArticleEmbeddingStage {
    embedder: Arc<dyn Embedder>,
    parsed_dependency: String,
    retry_after: String,
    contract: StageContract,
}

impl ParsedArticleEmbeddingStage {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        parsed_dependency: impl Into<String>,
        retry_after: impl Into<String>,
    ) -> Self {
        let dimensions = embedder.dimensions();
        let model_fingerprint = embedder.model_id().to_string();
        Self {
            embedder,
            parsed_dependency: parsed_dependency.into(),
            retry_after: retry_after.into(),
            contract: StageContract {
                stage_id: "paper_embed".into(),
                implementation_fingerprint: "ParsedArticleEmbeddingStage@title-abstract-v1".into(),
                configuration_fingerprint: format!(
                    "dimensions={dimensions};paper_fields=title,abstract;token_limit=backend_model_capability"
                ),
                prompt_fingerprint: None,
                model_fingerprint: Some(model_fingerprint),
                output_schema_fingerprint: "Vec<f32>@serde-v1".into(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for ParsedArticleEmbeddingStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload = request
            .dependency_payloads
            .get(&self.parsed_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for embedding",
                    self.parsed_dependency
                ),
            })?;
        let parsed: ParsedArticle =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            })?;
        if parsed.paper_id != request.article_id {
            return Err(StageExecutionError::Terminal {
                error_class: "article_identity_mismatch".into(),
                message: format!(
                    "parsed article '{}' does not match requested '{}'",
                    parsed.paper_id, request.article_id
                ),
            });
        }
        let text = format!("{}\n{}", parsed.title, parsed.abstract_text);
        let input_chars = text.chars().count();
        let vector = self
            .embedder
            .embed(&text)
            .await
            .map_err(|error| match error {
                EmbedderError::Unavailable(message) => StageExecutionError::Retryable {
                    error_class: "embedding_backend_unavailable".into(),
                    message,
                    retry_after: self.retry_after.clone(),
                },
                EmbedderError::EmbedFailed(message) => StageExecutionError::Terminal {
                    error_class: "embedding_request_rejected".into(),
                    message,
                },
            })?;
        if vector.len() != self.embedder.dimensions() {
            return Err(StageExecutionError::Terminal {
                error_class: "embedding_dimension_mismatch".into(),
                message: format!(
                    "embedder returned {} dimensions, expected {}",
                    vector.len(),
                    self.embedder.dimensions()
                ),
            });
        }
        let payload =
            serde_json::to_vec(&vector).map_err(|error| StageExecutionError::Terminal {
                error_class: "embedding_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("dimensions".into(), vector.len().to_string()),
                ("input_chars".into(), input_chars.to_string()),
                ("truncated".into(), false.to_string()),
            ]),
            payload,
        })
    }
}

pub struct HtmlParseStage {
    parser: Arc<dyn ParserPort>,
    html_path: PathBuf,
    contract: StageContract,
}

impl HtmlParseStage {
    pub fn new(parser: Arc<dyn ParserPort>, html_path: impl Into<PathBuf>) -> Self {
        Self {
            parser,
            html_path: html_path.into(),
            contract: StageContract {
                stage_id: "html_parse".into(),
                implementation_fingerprint: "HtmlParser@structured-sections-v1".into(),
                configuration_fingerprint: "main-content=article-or-md-content;headings=h1,h2,h3"
                    .into(),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: PROJECT_ARTICLE_SCHEMA_FINGERPRINT.into(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for HtmlParseStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let html_path = self
            .html_path
            .to_str()
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "invalid_html_path".into(),
                message: format!("HTML path is not UTF-8: {}", self.html_path.display()),
            })?;
        let parsed = self
            .parser
            .parse_html(html_path, &request.article_id)
            .await
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "html_parser_failed".into(),
                message: error.to_string(),
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
                ("html_hash".into(), parsed.pdf_hash.clone()),
            ]),
            payload,
        })
    }
}

/// Pure extraction adapter: cached ParsedArticle payload -> extracted entity payload.
pub struct RuleExtractionStage {
    extractor: Arc<dyn Extractor>,
    parsed_dependency: String,
    contract: StageContract,
}

impl RuleExtractionStage {
    pub fn new(
        extractor: Arc<dyn Extractor>,
        parsed_dependency: impl Into<String>,
        configuration_fingerprint: impl Into<String>,
    ) -> Self {
        let extractor_name = extractor.name().to_string();
        // M446: compose the extractor's effective configuration identity
        // into the cache key — static caller strings alone never
        // invalidated on dictionary changes.
        let static_part = configuration_fingerprint.into();
        let configuration_fingerprint = match extractor.cache_fingerprint() {
            Some(id) => format!("{static_part};rules_sha256={id}"),
            None => static_part,
        };
        Self {
            extractor,
            parsed_dependency: parsed_dependency.into(),
            contract: StageContract {
                stage_id: "rule_extract".into(),
                implementation_fingerprint: "RuleExtractionStage@parsed-sections-v1".into(),
                configuration_fingerprint,
                prompt_fingerprint: None,
                model_fingerprint: Some(extractor_name),
                output_schema_fingerprint:
                    da_ports::cache_payload::extracted_entity_output_schema_fingerprint(),
            },
        }
    }

    pub fn with_contract(mut self, contract: StageContract) -> Self {
        self.contract = contract;
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for RuleExtractionStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        da_ports::cache_payload::decode_extracted_entities(payload)
            .map(|_| ())
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "incompatible_cache_payload".into(),
                message: error.to_string(),
            })
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload = request
            .dependency_payloads
            .get(&self.parsed_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for extraction",
                    self.parsed_dependency
                ),
            })?;
        let parsed: ParsedArticle =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            })?;
        if parsed.paper_id != request.article_id {
            return Err(StageExecutionError::Terminal {
                error_class: "article_identity_mismatch".into(),
                message: format!(
                    "parsed article '{}' does not match requested '{}'",
                    parsed.paper_id, request.article_id
                ),
            });
        }
        let sections = parsed
            .sections
            .iter()
            .map(|section| (section.title.clone(), section.text.clone()))
            .collect::<Vec<_>>();
        let entities = self.extractor.extract(&sections).await.map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "entity_extraction_failed".into(),
                message: error.to_string(),
            }
        })?;
        let payload =
            serde_json::to_vec(&entities).map_err(|error| StageExecutionError::Terminal {
                error_class: "extracted_entity_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("entities".into(), entities.len().to_string()),
                ("sections".into(), sections.len().to_string()),
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
    #[test]
    fn rule_extract_contract_binds_rule_data_identity() {
        use crate::rule_extractor::ExtractionConfig;
        use std::sync::Arc;
        let cfg_a = ExtractionConfig::bundled();
        let mut cfg_b = ExtractionConfig::bundled();
        cfg_b.methods.phrases.push("zzz-test-phrase".into());
        let stage_a = RuleExtractionStage::new(
            Arc::new(crate::RuleBasedExtractor::with_config(cfg_a)),
            "parse",
            "static-v1",
        );
        let stage_b = RuleExtractionStage::new(
            Arc::new(crate::RuleBasedExtractor::with_config(cfg_b)),
            "parse",
            "static-v1",
        );
        let (a, b) = (stage_a.contract(), stage_b.contract());
        assert_ne!(
            a.configuration_fingerprint, b.configuration_fingerprint,
            "dictionary change must invalidate the extraction cache key"
        );
        assert!(a.configuration_fingerprint.contains("rules_sha256="));
    }

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
                tables: Vec::new(),
                table_missingness: None,
                formulas: Vec::new(),
                formula_missingness: None,
                page_text_missingness: None,
                pdf_hash: "a".repeat(64),
                ..Default::default()
            })
        }
    }

    struct FailingParser;

    #[async_trait]
    impl ParserPort for FailingParser {
        async fn parse_pdf(&self, _pdf_path: &str, _paper_id: &str) -> ParseResult<ParsedArticle> {
            Err(da_ports::parser::ParserError::Unavailable("down".into()))
        }
    }

    struct FixedEmbedder;

    #[async_trait]
    impl Embedder for FixedEmbedder {
        async fn embed(&self, text: &str) -> da_ports::embedder::EmbedResult<Vec<f32>> {
            assert!(text.contains("GEPA"));
            assert!(text.contains("Optimization"));
            assert!(!text.contains("BODY_MUST_NOT_ENTER_PAPER_EMBEDDING"));
            Ok(vec![0.25; 4])
        }

        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> da_ports::embedder::EmbedResult<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.25; 4]).collect())
        }

        fn dimensions(&self) -> usize {
            4
        }

        fn model_id(&self) -> &str {
            "fixture/fixed-4d"
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

    #[test]
    fn grobid_parse_stage_backend_version_rotates_model_identity() {
        let parser: Arc<dyn ParserPort> = Arc::new(FakeParser {
            calls: AtomicUsize::new(0),
        });
        let baseline = GrobidParseStage::new(parser.clone(), "/catalog/paper.pdf", "t");
        assert_eq!(
            baseline.contract().model_fingerprint.as_deref(),
            Some(HISTORICAL_GROBID_MODEL_FINGERPRINT)
        );
        let versioned = GrobidParseStage::new(parser.clone(), "/catalog/paper.pdf", "t")
            .with_backend_version("0.9.1");
        assert_eq!(
            versioned.contract().model_fingerprint.as_deref(),
            Some("grobid/grobid:0.9.1")
        );
        assert_ne!(
            baseline.contract().model_fingerprint,
            versioned.contract().model_fingerprint,
            "different releases never share a parse stage identity"
        );
        // Blank never invents a release: the baseline stays.
        let blank =
            GrobidParseStage::new(parser, "/catalog/paper.pdf", "t").with_backend_version("   ");
        assert_eq!(
            blank.contract().model_fingerprint.as_deref(),
            Some(HISTORICAL_GROBID_MODEL_FINGERPRINT)
        );
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
    async fn parsed_article_embedding_stage_is_typed_and_graph_pure() {
        let stage = ParsedArticleEmbeddingStage::new(
            Arc::new(FixedEmbedder),
            "parse",
            "2026-08-08T17:00:00Z",
        );
        let parsed = ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "Optimization".into(),
            body_text: "BODY_MUST_NOT_ENTER_PAPER_EMBEDDING".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: "a".repeat(64),
            ..Default::default()
        };
        let output = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::from([(
                    "parse".into(),
                    serde_json::to_vec(&parsed).unwrap(),
                )]),
            })
            .await
            .unwrap();
        let vector: Vec<f32> = serde_json::from_slice(&output.payload).unwrap();
        assert_eq!(vector, vec![0.25; 4]);
        assert_eq!(output.metadata["dimensions"], "4");
        assert!(
            stage
                .contract()
                .configuration_fingerprint
                .contains("token_limit=backend_model_capability")
        );
    }

    #[tokio::test]
    async fn html_parse_stage_emits_same_parsed_article_contract() {
        let root = std::env::temp_dir().join(format!(
            "da-html-stage-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("chapter.html");
        std::fs::write(
            &path,
            "<html><title>Graphs</title><article><h1>Methods</h1><p>GEPA</p></article></html>",
        )
        .unwrap();
        let stage = HtmlParseStage::new(Arc::new(crate::HtmlParser::new()), &path);
        let output = stage
            .execute(StageExecutionRequest {
                article_id: "gnn-ch-01".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap();
        let parsed: ParsedArticle = serde_json::from_slice(&output.payload).unwrap();
        assert_eq!(parsed.paper_id, "gnn-ch-01");
        assert_eq!(parsed.title, "Graphs");
        assert!(!parsed.sections.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rule_extraction_stage_consumes_parsed_dependency_and_is_graph_pure() {
        let stage = RuleExtractionStage::new(
            Arc::new(crate::RuleBasedExtractor::new()),
            "parse",
            "sha256:fixture-config",
        );
        let parsed = ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: vec![da_ports::parser::Section {
                title: "Methods".into(),
                text: "We compare GEPA and GRPO.".into(),
                level: 1,
                path_id: None,
                parent_section_id: None,
                order_in_parent: None,
                section_id: None,
            }],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: "a".repeat(64),
            ..Default::default()
        };
        let output = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::from([(
                    "parse".into(),
                    serde_json::to_vec(&parsed).unwrap(),
                )]),
            })
            .await
            .unwrap();
        let entities: Vec<da_ports::extractor::ExtractedEntity> =
            serde_json::from_slice(&output.payload).unwrap();
        assert!(entities.iter().any(|entity| entity.label == "GEPA"));
        assert_eq!(output.metadata["sections"], "1");
        // M446: the fingerprint now composes the extractor's rule-DATA
        // identity (rules_sha256=…) behind the caller-supplied part.
        let fp = stage.contract().configuration_fingerprint;
        assert!(
            fp.starts_with("sha256:fixture-config;rules_sha256="),
            "fp: {fp}"
        );
    }

    #[test]
    fn rule_extraction_stage_binds_canonical_schema_and_rejects_legacy_hits() {
        let stage = RuleExtractionStage::new(
            Arc::new(crate::RuleBasedExtractor::new()),
            "parse",
            "sha256:fixture-config",
        );
        assert_eq!(
            stage.contract().output_schema_fingerprint,
            da_ports::cache_payload::extracted_entity_output_schema_fingerprint()
        );
        let legacy = serde_json::to_vec(&serde_json::json!([{
            "label": "GEPA",
            "entity_type": "Method",
            "section_title": "Methods",
            "char_start": 12,
            "char_end": 16,
            "surface": "GEPA"
        }]))
        .unwrap();
        let error = stage.validate_cached_payload(&legacy).unwrap_err();
        assert!(matches!(
            error,
            StageExecutionError::Terminal { ref error_class, .. }
                if error_class == "incompatible_cache_payload"
        ));
        assert!(error.to_string().contains("section_ordinal"));
    }

    #[tokio::test]
    async fn rule_extraction_stage_rejects_missing_dependency() {
        let stage = RuleExtractionStage::new(
            Arc::new(crate::RuleBasedExtractor::new()),
            "parse",
            "sha256:fixture-config",
        );
        let error = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap_err();
        assert!(matches!(error, StageExecutionError::Terminal { .. }));
    }

    #[tokio::test]
    async fn grobid_retry_delay_is_computed_when_failure_is_observed() {
        let parser = Arc::new(FailingParser);
        let stage = GrobidParseStage::new(parser, "/catalog/paper.pdf", "2000-01-01T00:00:00Z")
            .with_retry_delay_seconds(30);
        let before = chrono::Utc::now();
        let error = stage
            .execute(StageExecutionRequest {
                article_id: "paper".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap_err();
        let StageExecutionError::Retryable { retry_after, .. } = error else {
            panic!("expected retryable failure")
        };
        let retry_after = chrono::DateTime::parse_from_rfc3339(&retry_after).unwrap();
        assert!(retry_after > before);
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
                revision_vid: None,
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
                revision_vid: None,
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
