//! Generic cacheable DAG source for canonical artifact payloads.

use async_trait::async_trait;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub struct ArtifactPayloadStage {
    contract: StageContract,
    payload: Vec<u8>,
}

impl ArtifactPayloadStage {
    pub fn new(
        stage_id: impl Into<String>,
        payload: Vec<u8>,
        output_schema_fingerprint: impl Into<String>,
    ) -> Self {
        let payload_sha = hex::encode(Sha256::digest(&payload));
        Self {
            contract: StageContract {
                stage_id: stage_id.into(),
                implementation_fingerprint: "ArtifactPayloadStage@bytes-v1".into(),
                configuration_fingerprint: format!("payload@sha256:{payload_sha}"),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: output_schema_fingerprint.into(),
            },
            payload,
        }
    }
}

#[async_trait]
impl PipelineStageExecutor for ArtifactPayloadStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        _request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("source".into(), "canonical-artifact".into()),
                ("side_effects".into(), "none".into()),
            ]),
            payload: self.payload.clone(),
        })
    }
}
