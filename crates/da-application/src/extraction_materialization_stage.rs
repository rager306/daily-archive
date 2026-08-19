//! Extraction materialization stage executor (M403 S02).
//!
//! Wraps the graph-only extraction boundary
//! ([`crate::extraction::ExtractionUseCase::materializer_only`]) behind
//! [`da_ports::pipeline_stage::PipelineStageExecutor`]. Inputs are the parse
//! payload (`ParsedArticle`) and the extraction compute payload
//! (`Vec<ExtractedEntity>` via the canonical cache codec). The stage payload
//! is the typed `ExtractionResult` report; graph writes are delegated to the
//! idempotent materializer. The orchestrator's always-run semantics for
//! `graph_materialize` stages guarantee replay re-applies with zero new
//! writes. D127 stays false.

use std::sync::Arc;

use async_trait::async_trait;
use da_ports::cache_payload::decode_extracted_entities;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};

use crate::extraction::ExtractionUseCase;

pub const EXTRACTION_MATERIALIZATION_IMPL_FINGERPRINT: &str = "ExtractionUseCase@materializer-v1";
pub const EXTRACTION_MATERIALIZATION_CONFIG_FINGERPRINT: &str = "materializer-default-v1";
pub const EXTRACTION_MATERIALIZATION_OUTPUT_SCHEMA: &str =
    "ExtractionMaterializationReport@serde-v1";

/// Cached-report wire shape for the extraction materialization stage.
///
/// Explicit projection of the use-case result: `ExtractionResult` itself is
/// not a serde type, and the stage wire contract must stay stable and
/// auditable independently of use-case evolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMaterializationReport {
    pub paper_id: String,
    pub entities_extracted: usize,
    pub entities_created: usize,
    pub mentions_edges: usize,
    pub found_in_edges: usize,
    pub evidence_bundles_created: usize,
    pub claims_created: usize,
    pub import_eligible: bool,
}

/// Canonical dependency names.
const DEFAULT_PARSE_DEPENDENCY: &str = "parse";
const DEFAULT_EXTRACTION_DEPENDENCY: &str = "extract";

pub struct ExtractionMaterializationStage {
    use_case: ExtractionUseCase,
    parse_dependency: String,
    extraction_dependency: String,
}

impl ExtractionMaterializationStage {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self {
            use_case: ExtractionUseCase::materializer_only(graph_store),
            parse_dependency: DEFAULT_PARSE_DEPENDENCY.into(),
            extraction_dependency: DEFAULT_EXTRACTION_DEPENDENCY.into(),
        }
    }

    pub fn with_dependencies(
        mut self,
        parse_dependency: impl Into<String>,
        extraction_dependency: impl Into<String>,
    ) -> Self {
        self.parse_dependency = parse_dependency.into();
        self.extraction_dependency = extraction_dependency.into();
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for ExtractionMaterializationStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "extraction_materialization".into(),
            implementation_fingerprint: EXTRACTION_MATERIALIZATION_IMPL_FINGERPRINT.into(),
            configuration_fingerprint: EXTRACTION_MATERIALIZATION_CONFIG_FINGERPRINT.into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: EXTRACTION_MATERIALIZATION_OUTPUT_SCHEMA.into(),
        }
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        serde_json::from_slice::<ExtractionMaterializationReport>(payload)
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
        let parse_payload = request
            .dependency_payloads
            .get(&self.parse_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for extraction materialization",
                    self.parse_dependency
                ),
            })?;
        let parsed: ParsedArticle = serde_json::from_slice(parse_payload).map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            }
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
        let extraction_payload = request
            .dependency_payloads
            .get(&self.extraction_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_extracted_dependency".into(),
                message: format!(
                    "dependency '{}' is required for extraction materialization",
                    self.extraction_dependency
                ),
            })?;
        let entities = decode_extracted_entities(extraction_payload).map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "extracted_entities_deserialization".into(),
                message: error.to_string(),
            }
        })?;
        let result = self
            .use_case
            .materialize_extracted(&parsed, entities)
            .await
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "extraction_materialization_failed".into(),
                message: error.to_string(),
            })?;
        let report = ExtractionMaterializationReport {
            paper_id: result.paper_id.clone(),
            entities_extracted: result.entities_extracted,
            entities_created: result.entities_created,
            mentions_edges: result.mentions_edges,
            found_in_edges: result.found_in_edges,
            evidence_bundles_created: result.evidence_bundles_created,
            claims_created: result.claims_created,
            import_eligible: false,
        };
        let output =
            serde_json::to_vec(&report).map_err(|error| StageExecutionError::Terminal {
                error_class: "extraction_report_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: std::collections::BTreeMap::from([
                (
                    "entities_created".into(),
                    report.entities_created.to_string(),
                ),
                ("mentions_edges".into(), report.mentions_edges.to_string()),
                ("found_in_edges".into(), report.found_in_edges.to_string()),
                ("graph_write".into(), "always-run-idempotent".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}
