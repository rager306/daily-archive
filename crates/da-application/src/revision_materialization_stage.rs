//! Revision materialization stage executor (M403 S02).
//!
//! Wraps [`RevisionMaterializationUseCase`] behind
//! [`da_ports::pipeline_stage::PipelineStageExecutor`]. The stage payload is
//! the typed [`RevisionMaterializationResult`] report; graph writes are
//! delegated to the idempotent use case. Combined with the orchestrator's
//! always-run semantics for `graph_materialize` stages (M403 S01), a replay
//! re-applies with zero new writes and refreshes the stored report. D127
//! stays false (`import_eligible=false` on every node).

use std::sync::Arc;

use async_trait::async_trait;
use da_ports::clock::Clock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationResult,
    RevisionMaterializationUseCase,
};

pub const REVISION_MATERIALIZATION_IMPL_FINGERPRINT: &str =
    "RevisionMaterializationUseCase@revision-owned-v1";
pub const REVISION_MATERIALIZATION_OUTPUT_SCHEMA: &str = "RevisionMaterializationResult@serde-v1";

/// Cached-report wire shape for the revision materialization stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMaterializationReport {
    pub result: RevisionMaterializationResult,
}

/// Canonical dependency name for the parse payload.
const DEFAULT_PARSE_DEPENDENCY: &str = "parse";

pub struct RevisionMaterializationStage {
    use_case: RevisionMaterializationUseCase,
    source_code: String,
    canonical_id: String,
    revision: u32,
    manifestations: Vec<RevisionManifestationInput>,
    parse_dependency: String,
}

impl RevisionMaterializationStage {
    pub fn new(
        graph_store: Arc<dyn DirectGraphStore>,
        clock: Arc<dyn Clock>,
        source_code: impl Into<String>,
        canonical_id: impl Into<String>,
        revision: u32,
        manifestations: Vec<RevisionManifestationInput>,
    ) -> Self {
        Self {
            use_case: RevisionMaterializationUseCase::new(graph_store, clock),
            source_code: source_code.into(),
            canonical_id: canonical_id.into(),
            revision,
            manifestations,
            parse_dependency: DEFAULT_PARSE_DEPENDENCY.into(),
        }
    }

    pub fn with_parse_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.parse_dependency = dependency.into();
        self
    }

    /// Configuration fingerprint binds the exact identity inputs: source
    /// code, canonical id, revision and the manifestation list checksum.
    pub fn configuration_fingerprint(&self) -> String {
        let identity = serde_json::json!({
            "source_code": self.source_code,
            "canonical_id": self.canonical_id,
            "revision": self.revision,
            "manifestations": self.manifestations,
        });
        let bytes = serde_json::to_vec(&identity).expect("identity serializes");
        hex::encode(Sha256::digest(&bytes))
    }
}

#[async_trait]
impl PipelineStageExecutor for RevisionMaterializationStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "revision_materialization".into(),
            implementation_fingerprint: REVISION_MATERIALIZATION_IMPL_FINGERPRINT.into(),
            configuration_fingerprint: self.configuration_fingerprint(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: REVISION_MATERIALIZATION_OUTPUT_SCHEMA.into(),
        }
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        serde_json::from_slice::<RevisionMaterializationReport>(payload)
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
            .get(&self.parse_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for revision materialization",
                    self.parse_dependency
                ),
            })?;
        let parsed: ParsedArticle =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            })?;
        if parsed.paper_id != self.canonical_id {
            return Err(StageExecutionError::Terminal {
                error_class: "article_identity_mismatch".into(),
                message: format!(
                    "parsed article '{}' does not match canonical id '{}'",
                    parsed.paper_id, self.canonical_id
                ),
            });
        }
        let result = self
            .use_case
            .materialize(&RevisionMaterializationRequest {
                source_code: self.source_code.clone(),
                canonical_id: self.canonical_id.clone(),
                revision: self.revision,
                parsed,
                manifestations: self.manifestations.clone(),
            })
            .await
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "revision_materialization_failed".into(),
                message: error.to_string(),
            })?;
        let report = RevisionMaterializationReport {
            result: result.clone(),
        };
        let output =
            serde_json::to_vec(&report).map_err(|error| StageExecutionError::Terminal {
                error_class: "revision_report_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: std::collections::BTreeMap::from([
                ("paper_vid".into(), result.paper_vid.clone()),
                ("revision_vid".into(), result.revision_vid.clone()),
                ("nodes_created".into(), result.nodes_created.to_string()),
                ("nodes_reused".into(), result.nodes_reused.to_string()),
                ("edges_created".into(), result.edges_created.to_string()),
                ("edges_reused".into(), result.edges_reused.to_string()),
                (
                    "sections_created".into(),
                    result.sections_created.to_string(),
                ),
                (
                    "manifestations_created".into(),
                    result.manifestations_created.to_string(),
                ),
                ("graph_write".into(), "always-run-idempotent".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}
