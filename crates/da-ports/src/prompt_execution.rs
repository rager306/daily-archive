//! Provider-neutral execution boundary for optimizer-ready generative tasks.

use async_trait::async_trait;
use da_domain::prompt_optimization::{
    PromptExecutionLineage, PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptExecutionRequest {
    pub run_id: String,
    pub task: PromptTaskSpec,
    pub route: PromptRoutePolicy,
    pub output_schema: PromptOutputSchema,
    pub variables: std::collections::BTreeMap<String, String>,
    pub input_bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptExecutionResponse {
    pub output_bytes: Vec<u8>,
    pub lineage: PromptExecutionLineage,
}
#[derive(Debug, Error)]
pub enum PromptExecutionError {
    #[error("prompt task contract: {0}")]
    Contract(String),
    #[error("prompt rendering: {0}")]
    Render(String),
    #[error("prompt provider: {0}")]
    Provider(String),
    #[error("prompt output schema: {0}")]
    Schema(String),
}
#[async_trait]
pub trait PromptExecutionPort: Send + Sync {
    async fn execute(
        &self,
        request: PromptExecutionRequest,
    ) -> Result<PromptExecutionResponse, PromptExecutionError>;
}
