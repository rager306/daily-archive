//! Candidate materialization stage executor (M407 S02).
//!
//! Graph-materialize executor over the M400 candidate materializers. This
//! slice wires the **equation family** end to end; the figure and table
//! families are deferred (figures need the LLM route, tables need
//! detection-driven visual reconstruction that does not exist as production
//! code) and are reported as typed `not_attempted` rows with zero counts —
//! never fabricated.
//!
//! Ledger contract: `candidate_materialization` is `graph_materialize` with
//! `candidate_only: true` — the orchestrator's always-run semantics apply
//! (a cached report is lineage evidence; the idempotent materializers
//! re-execute), every written node is `authoritative=false` and
//! `import_eligible=false` (D127).

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use da_ports::graph_store::DirectGraphStore;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};

use crate::equation_extraction_stage::EquationExtractionArtifact;
use crate::equation_materialize::EquationCandidateMaterializer;
use crate::figure_materialize::FigureCandidateMaterializer;
use crate::table_extraction_stage::TableCandidateArtifact;
use crate::table_materialize::TableCandidateMaterializer;

pub const CANDIDATE_MATERIALIZATION_IMPL_FINGERPRINT: &str =
    "CandidateMaterializationStage@all-families-v3";
pub const CANDIDATE_MATERIALIZATION_CONFIG_FINGERPRINT: &str = "all-families-v3";
pub const CANDIDATE_MATERIALIZATION_OUTPUT_SCHEMA: &str = "CandidateMaterializationReport@serde-v1";

/// Per-family outcome row inside the combined report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateFamilyRow {
    pub family: String,
    /// applied | unchanged | not_attempted
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub candidates: usize,
    pub nodes_created: usize,
    pub edges_created: usize,
}

/// Combined candidate materialization report (stage payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateMaterializationReport {
    pub families: Vec<CandidateFamilyRow>,
    pub import_eligible: bool,
}

/// Canonical dependency names for the family artifact payloads.
const DEFAULT_EQUATION_DEPENDENCY: &str = "equations";
const DEFAULT_TABLES_DEPENDENCY: &str = "tables";
const DEFAULT_FIGURES_DEPENDENCY: &str = "figures";

pub struct CandidateMaterializationStage {
    materializer: EquationCandidateMaterializer,
    table_materializer: TableCandidateMaterializer,
    figure_materializer: FigureCandidateMaterializer,
    equation_dependency: String,
    tables_dependency: String,
    figures_dependency: String,
}

impl CandidateMaterializationStage {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self {
            materializer: EquationCandidateMaterializer::new(graph_store.clone()),
            table_materializer: TableCandidateMaterializer::new(graph_store.clone()),
            figure_materializer: FigureCandidateMaterializer::new(graph_store),
            equation_dependency: DEFAULT_EQUATION_DEPENDENCY.into(),
            tables_dependency: DEFAULT_TABLES_DEPENDENCY.into(),
            figures_dependency: DEFAULT_FIGURES_DEPENDENCY.into(),
        }
    }

    pub fn with_equation_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.equation_dependency = dependency.into();
        self
    }

    pub fn with_tables_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.tables_dependency = dependency.into();
        self
    }

    pub fn with_figures_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.figures_dependency = dependency.into();
        self
    }
}

#[async_trait]
impl PipelineStageExecutor for CandidateMaterializationStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "candidate_materialization".into(),
            implementation_fingerprint: CANDIDATE_MATERIALIZATION_IMPL_FINGERPRINT.into(),
            configuration_fingerprint: CANDIDATE_MATERIALIZATION_CONFIG_FINGERPRINT.into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: CANDIDATE_MATERIALIZATION_OUTPUT_SCHEMA.into(),
        }
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        serde_json::from_slice::<CandidateMaterializationReport>(payload)
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
            .get(&self.equation_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_equation_dependency".into(),
                message: format!(
                    "dependency '{}' is required for candidate materialization",
                    self.equation_dependency
                ),
            })?;
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "equation_artifact_deserialization".into(),
                message: error.to_string(),
            })?;

        let mut nodes_created = 0usize;
        let mut edges_created = 0usize;
        let mut applied_any = false;
        for candidate in &artifact.candidates {
            let outcome = self
                .materializer
                .materialize(candidate)
                .await
                .map_err(|error| StageExecutionError::Terminal {
                    error_class: "equation_candidate_materialization_failed".into(),
                    message: error.to_string(),
                })?;
            nodes_created += outcome.nodes_created;
            edges_created += outcome.edges_created;
            applied_any |= outcome.applied;
        }
        let equation_status = if artifact.candidates.is_empty() {
            "not_attempted"
        } else if applied_any {
            "applied"
        } else {
            "unchanged"
        };
        let equation_reason = if artifact.candidates.is_empty() {
            artifact.missingness.clone()
        } else {
            None
        };
        let equation_row = CandidateFamilyRow {
            family: "equation".into(),
            status: equation_status.into(),
            reason: equation_reason,
            candidates: artifact.candidates.len(),
            nodes_created,
            edges_created,
        };

        // ── Table family (M408 S02) ────────────────────────────────────────
        // Without the tables dependency the row keeps its typed deferred
        // reason; with it, TableCandidates materialize through the M400
        // materializer with the same create/unchanged idempotence contract.
        let table_row =
            match request.dependency_payloads.get(&self.tables_dependency) {
                None => CandidateFamilyRow {
                    family: "table".into(),
                    status: "not_attempted".into(),
                    reason: Some("deferred: tables dependency not wired in this topology".into()),
                    candidates: 0,
                    nodes_created: 0,
                    edges_created: 0,
                },
                Some(payload) => {
                    let artifact: TableCandidateArtifact = serde_json::from_slice(payload)
                        .map_err(|error| StageExecutionError::Terminal {
                            error_class: "table_artifact_deserialization".into(),
                            message: error.to_string(),
                        })?;
                    let mut table_nodes = 0usize;
                    let mut table_edges = 0usize;
                    let mut table_applied = false;
                    for candidate in &artifact.candidates {
                        let outcome = self
                            .table_materializer
                            .materialize(candidate)
                            .await
                            .map_err(|error| StageExecutionError::Terminal {
                                error_class: "table_candidate_materialization_failed".into(),
                                message: error.to_string(),
                            })?;
                        table_nodes += outcome.nodes_created;
                        table_edges += outcome.edges_created;
                        table_applied |= outcome.applied;
                    }
                    let status = if artifact.candidates.is_empty() {
                        "not_attempted"
                    } else if table_applied {
                        "applied"
                    } else {
                        "unchanged"
                    };
                    let reason = if artifact.candidates.is_empty() {
                        artifact.missingness.clone()
                    } else {
                        None
                    };
                    CandidateFamilyRow {
                        family: "table".into(),
                        status: status.into(),
                        reason,
                        candidates: artifact.candidates.len(),
                        nodes_created: table_nodes,
                        edges_created: table_edges,
                    }
                }
            };

        let figure_row = match request.dependency_payloads.get(&self.figures_dependency) {
            None => CandidateFamilyRow {
                family: "figure".into(),
                status: "not_attempted".into(),
                reason: Some("deferred: figures dependency not wired in this topology".into()),
                candidates: 0,
                nodes_created: 0,
                edges_created: 0,
            },
            Some(payload) => {
                let candidates: Vec<da_domain::figure_candidate::FigureCandidate> =
                    serde_json::from_slice(payload).map_err(|error| {
                        StageExecutionError::Terminal {
                            error_class: "figure_artifact_deserialization".into(),
                            message: error.to_string(),
                        }
                    })?;
                let mut figure_nodes = 0usize;
                let mut figure_edges = 0usize;
                let mut figure_applied = false;
                for candidate in &candidates {
                    let outcome = self
                        .figure_materializer
                        .materialize(candidate)
                        .await
                        .map_err(|error| StageExecutionError::Terminal {
                            error_class: "figure_candidate_materialization_failed".into(),
                            message: error.to_string(),
                        })?;
                    figure_nodes += outcome.nodes_created;
                    figure_edges += outcome.edges_created;
                    figure_applied |= outcome.applied;
                }
                let status = if candidates.is_empty() {
                    "not_attempted"
                } else if figure_applied {
                    "applied"
                } else {
                    "unchanged"
                };
                CandidateFamilyRow {
                    family: "figure".into(),
                    status: status.into(),
                    reason: None,
                    candidates: candidates.len(),
                    nodes_created: figure_nodes,
                    edges_created: figure_edges,
                }
            }
        };

        let report = CandidateMaterializationReport {
            families: vec![equation_row, figure_row, table_row],
            import_eligible: false,
        };
        let output =
            serde_json::to_vec(&report).map_err(|error| StageExecutionError::Terminal {
                error_class: "candidate_report_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("equations".into(), artifact.candidates.len().to_string()),
                ("nodes_created".into(), nodes_created.to_string()),
                ("edges_created".into(), edges_created.to_string()),
                ("graph_write".into(), "always-run-idempotent".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
}
