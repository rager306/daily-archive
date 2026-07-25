//! LLM client port — chat + structured extraction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LLMError {
    #[error("LLM error: {0}")]
    LLM(String),
    #[error("Rate limited: {0}")]
    RateLimited(String),
    #[error("Timeout")]
    Timeout,
}

pub type LLMResult<T> = Result<T, LLMError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    pub model: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            model: "glm-5.2".to_string(),
            max_tokens: 1400,
            temperature: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub text: String,
    pub ok: bool,
    pub error: Option<String>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
}

/// LLM client port.
/// 9router (OpenAI-compatible) and MiniMax (Anthropic-compatible) implement this.
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, messages: &[ChatMessage], opts: &ChatOptions) -> LLMResult<ChatResponse>;

    async fn extract_structured(
        &self,
        text: &str,
        schema: &serde_json::Value,
    ) -> LLMResult<serde_json::Value>;

    fn can_make_request(&self) -> bool;

    fn provider_id(&self) -> &str;
}
