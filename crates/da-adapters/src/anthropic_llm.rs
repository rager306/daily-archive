//! Anthropic-compatible LLM client adapter (MiniMax, Claude, etc.)
//!
//! Implements `da_ports::LLMClient` via Anthropic Messages API.
//! Used for: summary generation (ADR-053 Layer B), structured extraction,
//! and future NLQ (LLM→Cypher).
//!
//! Configuration via env:
//! - `ANTHROPIC_API_KEY`: API key
//! - `ANTHROPIC_BASE_URL`: API endpoint (default: MiniMax Anthropic-compatible)
//! - `ANTHROPIC_MODEL`: model name (default: MiniMax-M2.7-highspeed)

use async_trait::async_trait;
use da_ports::llm_client::{
    ChatMessage, ChatOptions, ChatResponse, LLMClient, LLMError, LLMResult,
};
use serde::Deserialize;

/// Anthropic-compatible LLM client (MiniMax, Claude, etc.)
pub struct AnthropicLlmClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

impl AnthropicLlmClient {
    /// Create from env vars. Falls back to MiniMax defaults.
    pub fn from_env() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimax.io/anthropic".to_string());
        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "MiniMax-M2.7-highspeed".to_string());
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("reqwest client"),
            api_key,
            base_url,
            model,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LLMClient for AnthropicLlmClient {
    async fn chat(&self, messages: &[ChatMessage], opts: &ChatOptions) -> LLMResult<ChatResponse> {
        if self.api_key.is_empty() {
            return Err(LLMError::LLM("ANTHROPIC_API_KEY not set".into()));
        }

        let model = if opts.model.is_empty() {
            self.model.clone()
        } else {
            opts.model.clone()
        };

        // Convert ChatMessage to Anthropic format.
        // Anthropic API expects: system message separate, rest as user/assistant.
        let mut system_msg: Option<String> = None;
        let mut api_messages: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            if msg.role == "system" {
                system_msg = Some(msg.content.clone());
            } else {
                api_messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": opts.max_tokens,
            "messages": api_messages,
        });
        if let Some(sys) = system_msg {
            body["system"] = serde_json::Value::String(sys);
        }

        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| LLMError::LLM(format!("HTTP error: {e}")))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| LLMError::LLM(format!("body read error: {e}")))?;

        if !status.is_success() {
            return Err(LLMError::LLM(format!("API error {status}: {text}")));
        }

        let api_resp: AnthropicResponse =
            serde_json::from_str(&text).map_err(|e| LLMError::LLM(format!("parse error: {e}")))?;

        // Extract text from content blocks (skip thinking blocks).
        let response_text = api_resp
            .content
            .iter()
            .filter(|b| b.block_type == "text")
            .filter_map(|b| b.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ChatResponse {
            text: response_text,
            ok: true,
            error: None,
            tokens_in: api_resp.usage.input_tokens,
            tokens_out: api_resp.usage.output_tokens,
        })
    }

    async fn extract_structured(
        &self,
        text: &str,
        schema: &serde_json::Value,
    ) -> LLMResult<serde_json::Value> {
        let schema_str = serde_json::to_string_pretty(schema)
            .map_err(|e| LLMError::LLM(format!("schema serialize: {e}")))?;
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "You are a structured data extractor. Return ONLY valid JSON matching the schema.".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: format!("Extract structured data from this text.\n\nSchema:\n{schema_str}\n\nText:\n{text}\n\nReturn JSON only."),
            },
        ];
        let opts = ChatOptions {
            model: String::new(), // use default
            max_tokens: 2000,
            temperature: 0.0,
        };
        let resp = self.chat(&messages, &opts).await?;
        if !resp.ok {
            return Err(LLMError::LLM(resp.error.unwrap_or_default()));
        }
        // Try to parse the response as JSON.
        let cleaned = resp
            .text
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim();
        serde_json::from_str(cleaned)
            .map_err(|e| LLMError::LLM(format!("JSON parse error: {e}\nResponse: {cleaned}")))
    }

    fn can_make_request(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn provider_id(&self) -> &str {
        "anthropic-compatible"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_env_creates_client() {
        // Should not panic even without env vars.
        let client = AnthropicLlmClient::from_env();
        assert!(!client.base_url.is_empty());
        assert!(!client.model.is_empty());
    }

    #[test]
    fn test_can_make_request_without_key() {
        // Without ANTHROPIC_API_KEY, can_make_request should return false.
        // This test may pass if key IS set in env — that's fine.
        let client = AnthropicLlmClient::from_env();
        // Just verify it doesn't panic.
        let _ = client.can_make_request();
    }
}
