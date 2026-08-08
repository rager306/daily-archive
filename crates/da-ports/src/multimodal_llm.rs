//! Provider-neutral multimodal model gateway (M371 S03, D199).
//!
//! The port describes capabilities and effective model-route identity without
//! binding application code to MiniMax, Anthropic, OpenAI, or 9router.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_MULTIMODAL_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelWireProtocol {
    AnthropicMessages,
    OpenAiChatCompletions,
    OpenAiResponses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageMediaType {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageMediaType {
    pub fn as_mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultimodalImage {
    pub media_type: ImageMediaType,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

impl MultimodalImage {
    pub fn validate(&self) -> Result<(), MultimodalLlmError> {
        if self.bytes.is_empty() {
            return Err(MultimodalLlmError::InvalidRequest(
                "image payload is empty".into(),
            ));
        }
        if self.bytes.len() > MAX_MULTIMODAL_IMAGE_BYTES {
            return Err(MultimodalLlmError::InvalidRequest(format!(
                "image payload exceeds {MAX_MULTIMODAL_IMAGE_BYTES} bytes"
            )));
        }
        let actual = hex::encode(Sha256::digest(&self.bytes));
        if actual != self.sha256 {
            return Err(MultimodalLlmError::ChecksumMismatch {
                expected: self.sha256.clone(),
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRouteDescriptor {
    pub route_id: String,
    pub protocol: ModelWireProtocol,
    pub model: String,
    pub endpoint_fingerprint: String,
    pub supports_images: bool,
    pub supports_structured_output: bool,
    pub may_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct MultimodalLlmRequest {
    pub system_prompt: String,
    pub user_context: String,
    pub image: MultimodalImage,
    pub output_schema: serde_json::Value,
    pub tool_name: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl MultimodalLlmRequest {
    pub fn validate(&self) -> Result<(), MultimodalLlmError> {
        self.image.validate()?;
        if self.system_prompt.trim().is_empty() || self.user_context.trim().is_empty() {
            return Err(MultimodalLlmError::InvalidRequest(
                "system_prompt and user_context are required".into(),
            ));
        }
        if self.tool_name.trim().is_empty() {
            return Err(MultimodalLlmError::InvalidRequest(
                "structured output tool_name is required".into(),
            ));
        }
        if !self.output_schema.is_object() {
            return Err(MultimodalLlmError::InvalidRequest(
                "output_schema must be a JSON object".into(),
            ));
        }
        if self.max_tokens == 0 || !self.temperature.is_finite() || self.temperature < 0.0 {
            return Err(MultimodalLlmError::InvalidRequest(
                "invalid max_tokens or temperature".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalLlmResponse {
    pub structured_output: serde_json::Value,
    pub route: ModelRouteDescriptor,
    pub resolved_model: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub latency_ms: u64,
}

#[derive(Debug, Error)]
pub enum MultimodalLlmError {
    #[error("invalid multimodal request: {0}")]
    InvalidRequest(String),
    #[error("image checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("model route is unavailable: {0}")]
    Unavailable(String),
    #[error("model request timed out after {timeout_ms} ms")]
    Timeout { timeout_ms: u64 },
    #[error("model route rate limited the request{retry}")]
    RateLimited { retry: String },
    #[error("model route returned HTTP {status}: {diagnostic}")]
    Http { status: u16, diagnostic: String },
    #[error("invalid structured model response: {0}")]
    InvalidResponse(String),
}

pub type MultimodalLlmResult<T> = Result<T, MultimodalLlmError>;

#[async_trait]
pub trait MultimodalLlmPort: Send + Sync {
    fn route(&self) -> ModelRouteDescriptor;

    async fn complete_structured(
        &self,
        request: MultimodalLlmRequest,
    ) -> MultimodalLlmResult<MultimodalLlmResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> MultimodalImage {
        let bytes = b"png-fixture".to_vec();
        MultimodalImage {
            media_type: ImageMediaType::Png,
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes,
        }
    }

    #[test]
    fn image_checksum_and_size_are_binding() {
        image().validate().unwrap();
        let mut bad = image();
        bad.sha256 = "a".repeat(64);
        assert!(matches!(
            bad.validate(),
            Err(MultimodalLlmError::ChecksumMismatch { .. })
        ));
        let oversized = MultimodalImage {
            media_type: ImageMediaType::Png,
            bytes: vec![0; MAX_MULTIMODAL_IMAGE_BYTES + 1],
            sha256: String::new(),
        };
        assert!(matches!(
            oversized.validate(),
            Err(MultimodalLlmError::InvalidRequest(_))
        ));
    }

    #[test]
    fn route_identity_preserves_protocol_model_and_fallback_semantics() {
        let direct = ModelRouteDescriptor {
            route_id: "minimax-direct".into(),
            protocol: ModelWireProtocol::AnthropicMessages,
            model: "MiniMax-M3-512k".into(),
            endpoint_fingerprint: "api.minimax.io/anthropic/v1/messages".into(),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: false,
        };
        let router = ModelRouteDescriptor {
            route_id: "9router-vision-combo".into(),
            protocol: ModelWireProtocol::AnthropicMessages,
            model: "vision-combo".into(),
            endpoint_fingerprint: "loopback-20128/v1/messages".into(),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: true,
        };
        assert_ne!(direct, router);
    }
}
