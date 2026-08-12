//! Provider-neutral execution boundary for optimizer-ready generative tasks.

use async_trait::async_trait;
use da_domain::prompt_optimization::{
    PromptExecutionLineage, PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec,
    ResolvedModelBinding,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptMedia {
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

impl PromptMedia {
    pub fn validate(&self) -> Result<(), PromptExecutionError> {
        if !matches!(
            self.media_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp" | "image/gif"
        ) {
            return Err(PromptExecutionError::Contract(format!(
                "unsupported prompt media type {}",
                self.media_type
            )));
        }
        if self.bytes.is_empty()
            || self.bytes.len() > crate::multimodal_llm::MAX_MULTIMODAL_IMAGE_BYTES
        {
            return Err(PromptExecutionError::Contract(
                "prompt media size invalid".into(),
            ));
        }
        let actual = hex::encode(sha2::Sha256::digest(&self.bytes));
        if actual != self.sha256 {
            return Err(PromptExecutionError::Contract(
                "prompt media checksum mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptExecutionRequest {
    pub run_id: String,
    pub task: PromptTaskSpec,
    pub route: PromptRoutePolicy,
    pub resolved_model: ResolvedModelBinding,
    pub output_schema: PromptOutputSchema,
    pub variables: std::collections::BTreeMap<String, String>,
    pub input_bytes: Vec<u8>,
    pub media: Vec<PromptMedia>,
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
    #[error("prompt provider unavailable: {0}")]
    Unavailable(String),
    #[error("prompt provider timeout after {timeout_ms} ms")]
    Timeout { timeout_ms: u64 },
    #[error("prompt provider rate limited{retry}")]
    RateLimited { retry: String },
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

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn media_requires_supported_type_bounded_bytes_and_exact_digest() {
        let bytes = b"png".to_vec();
        let media = PromptMedia {
            media_type: "image/png".into(),
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes,
        };
        media.validate().unwrap();
        let mut bad = media.clone();
        bad.sha256 = "a".repeat(64);
        assert!(bad.validate().is_err());
        let mut bad = media;
        bad.media_type = "text/plain".into();
        assert!(bad.validate().is_err());
    }
}
