//! Cacheable pure cluster detection stage with explicit canonical input.

use async_trait::async_trait;
use da_domain::cluster::{
    EntityPapers, cluster_configuration_fingerprint, detect_clusters, normalize_entity_key,
};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPapersEntry {
    pub entity_type: String,
    pub label: String,
    pub paper_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPapersArtifact {
    pub entries: Vec<EntityPapersEntry>,
}

pub struct ClusterDetectionStage {
    dependency: String,
}

impl ClusterDetectionStage {
    pub fn new() -> Self {
        Self {
            dependency: "entity_papers".into(),
        }
    }

    pub fn with_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.dependency = dependency.into();
        self
    }
}

impl Default for ClusterDetectionStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PipelineStageExecutor for ClusterDetectionStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "cluster_detection".into(),
            implementation_fingerprint: "detect_clusters@typed-deterministic-v2".into(),
            configuration_fingerprint: cluster_configuration_fingerprint(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: "Vec<DetectedCluster>@serde-v1".into(),
        }
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload = request
            .dependency_payloads
            .get(&self.dependency)
            .ok_or_else(|| {
                terminal(
                    "missing_entity_papers_dependency",
                    format!("missing dependency {}", self.dependency),
                )
            })?;
        let artifact: EntityPapersArtifact = serde_json::from_slice(payload)
            .map_err(|error| terminal("invalid_entity_papers_payload", error.to_string()))?;
        let mut entity_papers = EntityPapers::new();
        for entry in artifact.entries {
            if entry.entity_type.trim().is_empty() || entry.label.trim().is_empty() {
                return Err(terminal(
                    "invalid_entity_papers_entry",
                    "entity_type and label must be non-empty",
                ));
            }
            let key = normalize_entity_key(&entry.entity_type, &entry.label);
            let papers: HashSet<_> = entry
                .paper_ids
                .into_iter()
                .filter(|id| !id.trim().is_empty())
                .collect();
            if papers.is_empty() {
                return Err(terminal(
                    "invalid_entity_papers_entry",
                    "paper_ids must contain a non-empty ID",
                ));
            }
            entity_papers.entry(key).or_default().extend(papers);
        }
        let detected = detect_clusters(&entity_papers);
        let member_count: usize = detected.iter().map(|cluster| cluster.members.len()).sum();
        let output = serde_json::to_vec(&detected)
            .map_err(|error| terminal("cluster_detection_serialization", error.to_string()))?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("input_entity_count".into(), entity_papers.len().to_string()),
                ("cluster_count".into(), detected.len().to_string()),
                ("member_count".into(), member_count.to_string()),
                ("side_effects".into(), "none".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}

fn terminal(error_class: &str, message: impl Into<String>) -> StageExecutionError {
    StageExecutionError::Terminal {
        error_class: error_class.into(),
        message: message.into(),
    }
}
