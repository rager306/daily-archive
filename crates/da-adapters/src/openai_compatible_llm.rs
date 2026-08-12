//! OpenAI-compatible LLM client for resolved model bindings (9router primary).

use async_trait::async_trait;
use da_domain::{assurance::ContentDigest, prompt_optimization::ResolvedModelBinding};
use da_ports::llm_client::{
    ChatMessage, ChatOptions, ChatResponse, LLMClient, LLMError, LLMResult,
};
use serde::Deserialize;
pub struct OpenAiCompatibleLlmClient {
    client: reqwest::Client,
    binding: ResolvedModelBinding,
    api_key: Option<String>,
}
#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Usage,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
    #[serde(default)]
    finish_reason: Option<String>,
}
#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}
#[derive(Default, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
}
impl OpenAiCompatibleLlmClient {
    pub fn new(
        binding: ResolvedModelBinding,
        api_key: Option<String>,
        timeout_seconds: u64,
    ) -> Result<Self, String> {
        binding.validate()?;
        if binding.protocol != "openai_chat_completions" || binding.provider != "openai" {
            return Err("OpenAI client requires openai_chat_completions binding".into());
        }
        if timeout_seconds == 0 {
            return Err("LLM timeout must be positive".into());
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            binding,
            api_key: api_key.filter(|x| !x.trim().is_empty()),
        })
    }
    pub fn binding(&self) -> &ResolvedModelBinding {
        &self.binding
    }
}
#[async_trait]
impl LLMClient for OpenAiCompatibleLlmClient {
    async fn chat(&self, messages: &[ChatMessage], opts: &ChatOptions) -> LLMResult<ChatResponse> {
        if !opts.model.is_empty() && opts.model != self.binding.model_name {
            return Err(LLMError::LLM(
                "requested model differs from resolved binding".into(),
            ));
        }
        let body = serde_json::json!({"model":self.binding.model_name,"messages":messages.iter().map(|x|serde_json::json!({"role":x.role,"content":x.content})).collect::<Vec<_>>(),"temperature":opts.temperature,"max_tokens":opts.max_tokens,"stream":false,"response_format":{"type":"json_object"},"enable_thinking":false});
        let mut request = self
            .client
            .post(&self.binding.endpoint)
            .header("Content-Type", "application/json")
            .json(&body);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| LLMError::LLM(format!("9router request failed: {e}")))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("missing")
            .to_owned();
        let declared_content_length = response.content_length();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| LLMError::LLM(format!("9router response read failed: {e}")))?;
        let digest = ContentDigest::of_bytes(&bytes);
        if !status.is_success() {
            tracing::error!(event="openai_compatible_http_error",status=%status,response_bytes=bytes.len(),response_sha256=digest.as_str(),"OpenAI-compatible provider returned an error");
            return Err(LLMError::LLM(format!(
                "9router HTTP {status}; response_bytes={} response_sha256={}",
                bytes.len(),
                digest.as_str()
            )));
        }
        if bytes.is_empty() {
            tracing::error!(event="openai_compatible_empty_success",status=%status,content_type=%content_type,declared_content_length,response_bytes=0,response_sha256=digest.as_str(),"OpenAI-compatible provider returned an empty success response");
            return Err(LLMError::LLM(format!(
                "9router empty success response; status={status} content_type={content_type} declared_content_length={declared_content_length:?} response_sha256={}",
                digest.as_str()
            )));
        }
        let parsed:Response=serde_json::from_slice(&bytes).map_err(|e|{tracing::error!(event="openai_compatible_invalid_json",status=%status,content_type=%content_type,declared_content_length,response_bytes=bytes.len(),response_sha256=digest.as_str(),error=%e,"OpenAI-compatible response parse failed");LLMError::LLM(format!("9router response JSON invalid; status={status} content_type={content_type} declared_content_length={declared_content_length:?} response_bytes={} response_sha256={}",bytes.len(),digest.as_str()))})?;
        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LLMError::LLM("9router response has no choices".into()))?;
        tracing::info!(
            event="openai_compatible_response_meta",
            finish_reason=?choice.finish_reason,
            prompt_tokens=parsed.usage.prompt_tokens,
            completion_tokens=parsed.usage.completion_tokens,
            content_present=choice.message.content.is_some(),
            reasoning_present=choice.message.reasoning_content.is_some(),
            "9router response metadata"
        );
        let message = choice.message;
        let text = message
            .content
            .clone()
            .filter(|x| (x.contains("</think>") || !x.contains("<think>")) && !x.trim().is_empty())
            .or_else(|| message.reasoning_content.filter(|x| !x.trim().is_empty()))
            .or_else(|| message.content.filter(|x| !x.trim().is_empty()))
            .ok_or_else(|| LLMError::LLM("9router response has no text content".into()))?;
        Ok(ChatResponse {
            text,
            ok: true,
            error: None,
            tokens_in: parsed.usage.prompt_tokens,
            tokens_out: parsed.usage.completion_tokens,
        })
    }
    async fn extract_structured(
        &self,
        text: &str,
        schema: &serde_json::Value,
    ) -> LLMResult<serde_json::Value> {
        let response = self
            .chat(
                &[
                    ChatMessage {
                        role: "system".into(),
                        content: format!(
                            "Return only JSON matching this schema: {}",
                            serde_json::to_string(schema)
                                .map_err(|e| LLMError::LLM(e.to_string()))?
                        ),
                    },
                    ChatMessage {
                        role: "user".into(),
                        content: text.into(),
                    },
                ],
                &ChatOptions {
                    model: self.binding.model_name.clone(),
                    max_tokens: 2000,
                    temperature: 0.0,
                },
            )
            .await?;
        serde_json::from_str(&response.text)
            .map_err(|e| LLMError::LLM(format!("structured output invalid: {e}")))
    }
    fn can_make_request(&self) -> bool {
        true
    }
    fn provider_id(&self) -> &str {
        "openai"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> ResolvedModelBinding {
        ResolvedModelBinding {
            binding_id: "paper-summary-generate-default".into(),
            model_id: "ninerouter-agnes".into(),
            provider: "openai".into(),
            protocol: "openai_chat_completions".into(),
            endpoint: "http://127.0.0.1:20128/v1/chat/completions".into(),
            model_name: "agnes-ai/agnes-2.0-flash".into(),
            tool_version: "1".into(),
            policy_version: "1".into(),
        }
    }
    #[tokio::test]
    async fn local_openai_compatible_roundtrip_reports_tokens() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let count = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.contains("agnes-ai/agnes-2.0-flash"));
            let body = r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}],"usage":{"prompt_tokens":12,"completion_tokens":5}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let mut resolved = binding();
        resolved.endpoint = format!("http://{address}/v1/chat/completions");
        let client = OpenAiCompatibleLlmClient::new(resolved, None, 5).unwrap();
        let response = client
            .chat(
                &[ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                &ChatOptions {
                    model: "agnes-ai/agnes-2.0-flash".into(),
                    max_tokens: 32,
                    temperature: 0.0,
                },
            )
            .await
            .unwrap();
        assert_eq!(response.text, "{\"ok\":true}");
        assert_eq!(response.tokens_in, Some(12));
        assert_eq!(response.tokens_out, Some(5));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn empty_success_response_fails_closed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0_u8; 16 * 1024];
            let _ = stream.read(&mut buf).await.unwrap();
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let mut resolved = binding();
        resolved.endpoint = format!("http://{address}/v1/chat/completions");
        let client = OpenAiCompatibleLlmClient::new(resolved, None, 5).unwrap();
        let result = client
            .chat(
                &[ChatMessage {
                    role: "user".into(),
                    content: "hi".into(),
                }],
                &ChatOptions {
                    model: "agnes-ai/agnes-2.0-flash".into(),
                    max_tokens: 8,
                    temperature: 0.0,
                },
            )
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty success"), "got: {err}");
        server.await.unwrap();
    }

    #[test]
    fn constructor_requires_openai_binding_and_positive_timeout() {
        assert!(OpenAiCompatibleLlmClient::new(binding(), None, 30).is_ok());
        let mut b = binding();
        b.protocol = "anthropic_messages".into();
        assert!(OpenAiCompatibleLlmClient::new(b, None, 30).is_err());
        assert!(OpenAiCompatibleLlmClient::new(binding(), None, 0).is_err());
    }
}
