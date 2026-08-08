//! Pipeline stage execution port (M371 S01 T02).
//!
//! Application orchestration knows stage contracts and byte artifacts, but not
//! concrete HTTP/JAR/filesystem/LLM adapters. Composition roots inject stage
//! executors implementing this port.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageContract {
    pub stage_id: String,
    pub implementation_fingerprint: String,
    pub configuration_fingerprint: String,
    pub prompt_fingerprint: Option<String>,
    pub model_fingerprint: Option<String>,
    pub output_schema_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageExecutionRequest {
    pub article_id: String,
    pub dependency_payloads: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageExecutionOutput {
    pub payload: Vec<u8>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StageExecutionError {
    #[error("retryable stage failure ({error_class}): {message}")]
    Retryable {
        error_class: String,
        message: String,
        retry_after: String,
    },
    #[error("terminal stage failure ({error_class}): {message}")]
    Terminal {
        error_class: String,
        message: String,
    },
}

#[async_trait]
pub trait PipelineStageExecutor: Send + Sync {
    fn contract(&self) -> StageContract;

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_contract_roundtrips_without_losing_fingerprints() {
        let contract = StageContract {
            stage_id: "summary".into(),
            implementation_fingerprint: "summary-use-case@1ab34809".into(),
            configuration_fingerprint: "max-tokens-dynamic-v1".into(),
            prompt_fingerprint: Some("sha256:prompt-v4".into()),
            model_fingerprint: Some("MiniMax-M2.7-highspeed".into()),
            output_schema_fingerprint: "sha256:summary-schema-v2".into(),
        };
        let json = serde_json::to_string(&contract).unwrap();
        let decoded: StageContract = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, contract);
    }

    #[test]
    fn retryable_and_terminal_failures_are_distinct() {
        let retryable = StageExecutionError::Retryable {
            error_class: "timeout".into(),
            message: "MiniMax timed out".into(),
            retry_after: "2026-08-08T01:00:00Z".into(),
        };
        let terminal = StageExecutionError::Terminal {
            error_class: "invalid_schema".into(),
            message: "schema contract rejected output".into(),
        };
        assert_ne!(retryable, terminal);
    }
}
