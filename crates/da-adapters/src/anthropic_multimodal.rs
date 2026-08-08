//! Provider-neutral Anthropic Messages multimodal route adapter (M371 S03, D199).
//!
//! Supports direct provider routes (e.g. MiniMax X-Api-Key) and gateways such
//! as 9router (Bearer auth) without leaking route details into application code.

use async_trait::async_trait;
use base64::Engine;
use da_ports::multimodal_llm::{
    ModelRouteDescriptor, ModelWireProtocol, MultimodalLlmError, MultimodalLlmPort,
    MultimodalLlmRequest, MultimodalLlmResponse, MultimodalLlmResult,
};
use serde::Deserialize;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_DIAGNOSTIC_CHARS: usize = 2_000;

#[derive(Clone)]
pub enum AnthropicRouteAuth {
    XApiKey(String),
    Bearer(String),
    None,
}

impl std::fmt::Debug for AnthropicRouteAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::XApiKey(_) => formatter.write_str("XApiKey([REDACTED])"),
            Self::Bearer(_) => formatter.write_str("Bearer([REDACTED])"),
            Self::None => formatter.write_str("None"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicMultimodalRoute {
    pub route_id: String,
    /// Accepts either endpoint root (`.../anthropic`) or SDK-style v1 root
    /// (`http://127.0.0.1:20128/v1`).
    pub endpoint_root: String,
    pub model: String,
    pub auth: AnthropicRouteAuth,
    pub may_fallback: bool,
}

impl AnthropicMultimodalRoute {
    pub fn minimax_direct(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            route_id: "minimax-direct".into(),
            endpoint_root: "https://api.minimax.io/anthropic".into(),
            model: model.into(),
            auth: AnthropicRouteAuth::XApiKey(api_key.into()),
            may_fallback: false,
        }
    }

    pub fn nine_router(
        endpoint_root: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            route_id: "9router-anthropic".into(),
            endpoint_root: endpoint_root.into(),
            model: model.into(),
            auth: api_key
                .map(AnthropicRouteAuth::Bearer)
                .unwrap_or(AnthropicRouteAuth::None),
            may_fallback: true,
        }
    }

    pub fn messages_url(&self) -> Result<String, MultimodalLlmError> {
        let root = self.endpoint_root.trim().trim_end_matches('/');
        if !(root.starts_with("http://") || root.starts_with("https://")) {
            return Err(MultimodalLlmError::InvalidRequest(
                "Anthropic route endpoint must use http or https".into(),
            ));
        }
        if root.ends_with("/v1") {
            Ok(format!("{root}/messages"))
        } else {
            Ok(format!("{root}/v1/messages"))
        }
    }

    fn descriptor(&self) -> ModelRouteDescriptor {
        let endpoint = self
            .messages_url()
            .unwrap_or_else(|_| "invalid-endpoint".into());
        ModelRouteDescriptor {
            route_id: self.route_id.clone(),
            protocol: ModelWireProtocol::AnthropicMessages,
            model: self.model.clone(),
            endpoint_fingerprint: endpoint_fingerprint(&endpoint),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: self.may_fallback,
        }
    }
}

pub struct AnthropicMultimodalClient {
    client: reqwest::Client,
    route: AnthropicMultimodalRoute,
    timeout: Duration,
}

impl AnthropicMultimodalClient {
    pub fn new(route: AnthropicMultimodalRoute) -> Result<Self, MultimodalLlmError> {
        Self::with_timeout(route, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        route: AnthropicMultimodalRoute,
        timeout: Duration,
    ) -> Result<Self, MultimodalLlmError> {
        route.messages_url()?;
        if route.route_id.trim().is_empty() || route.model.trim().is_empty() {
            return Err(MultimodalLlmError::InvalidRequest(
                "route_id and model are required".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| MultimodalLlmError::Unavailable(error.to_string()))?;
        Ok(Self {
            client,
            route,
            timeout,
        })
    }
}

#[async_trait]
impl MultimodalLlmPort for AnthropicMultimodalClient {
    fn route(&self) -> ModelRouteDescriptor {
        self.route.descriptor()
    }

    async fn complete_structured(
        &self,
        request: MultimodalLlmRequest,
    ) -> MultimodalLlmResult<MultimodalLlmResponse> {
        request.validate()?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&request.image.bytes);
        let body = serde_json::json!({
            "model": self.route.model,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "system": request.system_prompt,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": request.image.media_type.as_mime(),
                            "data": encoded
                        }
                    },
                    {"type": "text", "text": request.user_context}
                ]
            }],
            "tools": [{
                "name": request.tool_name,
                "description": "Return validated structured multimodal analysis.",
                "input_schema": request.output_schema
            }],
            "tool_choice": {"type": "tool", "name": request.tool_name}
        });
        let url = self.route.messages_url()?;
        let mut builder = self
            .client
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);
        builder = match &self.route.auth {
            AnthropicRouteAuth::XApiKey(key) => builder.header("x-api-key", key),
            AnthropicRouteAuth::Bearer(key) => builder.bearer_auth(key),
            AnthropicRouteAuth::None => builder,
        };
        let started = Instant::now();
        let response = builder.send().await.map_err(|error| {
            if error.is_timeout() {
                MultimodalLlmError::Timeout {
                    timeout_ms: self.timeout.as_millis() as u64,
                }
            } else {
                MultimodalLlmError::Unavailable(error.to_string())
            }
        })?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|error| MultimodalLlmError::Unavailable(error.to_string()))?;
        if status.as_u16() == 429 {
            return Err(MultimodalLlmError::RateLimited {
                retry: retry_after
                    .map(|value| format!("; retry-after={value}"))
                    .unwrap_or_default(),
            });
        }
        if !status.is_success() {
            return Err(MultimodalLlmError::Http {
                status: status.as_u16(),
                diagnostic: bounded_diagnostic(&bytes),
            });
        }
        let decoded: AnthropicResponse = serde_json::from_slice(&bytes).map_err(|error| {
            MultimodalLlmError::InvalidResponse(format!("response JSON: {error}"))
        })?;
        let structured_output = decoded
            .content
            .iter()
            .find(|block| block.block_type == "tool_use")
            .and_then(|block| block.input.clone())
            .ok_or_else(|| {
                MultimodalLlmError::InvalidResponse(
                    "forced structured response contained no tool_use input".into(),
                )
            })?;
        let latency_ms = started.elapsed().as_millis() as u64;
        tracing::info!(
            route_id = %self.route.route_id,
            protocol = "anthropic_messages",
            requested_model = %self.route.model,
            resolved_model = %decoded.model,
            image_bytes = request.image.bytes.len(),
            context_chars = request.user_context.chars().count(),
            latency_ms,
            input_tokens = ?decoded.usage.input_tokens,
            output_tokens = ?decoded.usage.output_tokens,
            event = "multimodal_llm_complete"
        );
        Ok(MultimodalLlmResponse {
            structured_output,
            route: self.route.descriptor(),
            resolved_model: decoded.model,
            input_tokens: decoded.usage.input_tokens,
            output_tokens: decoded.usage.output_tokens,
            latency_ms,
        })
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    model: String,
    content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    input: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

fn endpoint_fingerprint(url: &str) -> String {
    url.replace("127.0.0.1", "loopback")
        .replace("localhost", "loopback")
}

fn bounded_diagnostic(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.chars().take(MAX_DIAGNOSTIC_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::multimodal_llm::{ImageMediaType, MultimodalImage};
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    fn request() -> MultimodalLlmRequest {
        let bytes = b"png-fixture".to_vec();
        MultimodalLlmRequest {
            system_prompt: "Analyze scientific figures.".into(),
            user_context: "Figure 3: GEPA flowchart".into(),
            image: MultimodalImage {
                media_type: ImageMediaType::Png,
                sha256: hex::encode(Sha256::digest(&bytes)),
                bytes,
            },
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {"visual_type": {"type": "string"}},
                "required": ["visual_type"]
            }),
            tool_name: "record_figure_analysis".into(),
            max_tokens: 1024,
            temperature: 0.0,
        }
    }

    async fn fixture_server(
        status: u16,
        response_body: &'static str,
        extra_headers: &'static str,
    ) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut temporary = [0u8; 4096];
            loop {
                let count = stream.read(&mut temporary).await.unwrap();
                if count == 0 {
                    break;
                }
                buffer.extend_from_slice(&temporary[..count]);
                let Some(header_end) = find_bytes(&buffer, b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&buffer[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or_default();
                if buffer.len() >= header_end + 4 + length {
                    break;
                }
            }
            let captured = String::from_utf8_lossy(&buffer).into_owned();
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{extra_headers}\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            let _ = sender.send(captured);
        });
        (format!("http://{address}/v1"), receiver)
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    const TOOL_RESPONSE: &str = r#"{
      "model":"MiniMax-M3",
      "content":[
        {"type":"thinking","thinking":"hidden"},
        {"type":"tool_use","id":"tool-1","name":"record_figure_analysis","input":{"visual_type":"flowchart"}}
      ],
      "usage":{"input_tokens":120,"output_tokens":30}
    }"#;

    #[test]
    fn endpoint_normalization_prevents_double_v1() {
        let router = AnthropicMultimodalRoute::nine_router(
            "http://127.0.0.1:20128/v1",
            None,
            "vision-combo",
        );
        assert_eq!(
            router.messages_url().unwrap(),
            "http://127.0.0.1:20128/v1/messages"
        );
        let direct = AnthropicMultimodalRoute::minimax_direct("secret", "MiniMax-M3-512k");
        assert_eq!(
            direct.messages_url().unwrap(),
            "https://api.minimax.io/anthropic/v1/messages"
        );
        assert!(!format!("{direct:?}").contains("secret"));
    }

    #[tokio::test]
    async fn direct_route_uses_x_api_key_image_blocks_and_forced_tool() {
        let (root, captured) = fixture_server(200, TOOL_RESPONSE, "").await;
        let route = AnthropicMultimodalRoute {
            route_id: "direct-fixture".into(),
            endpoint_root: root,
            model: "MiniMax-M3-512k".into(),
            auth: AnthropicRouteAuth::XApiKey("direct-secret".into()),
            may_fallback: false,
        };
        let response = AnthropicMultimodalClient::new(route)
            .unwrap()
            .complete_structured(request())
            .await
            .unwrap();
        assert_eq!(response.structured_output["visual_type"], "flowchart");
        assert_eq!(response.resolved_model, "MiniMax-M3");
        let http = captured.await.unwrap();
        assert!(http.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(
            http.to_ascii_lowercase()
                .contains("x-api-key: direct-secret")
        );
        assert!(http.contains("\"type\":\"image\""));
        assert!(http.contains("\"type\":\"tool\""));
        assert!(http.contains("record_figure_analysis"));
    }

    #[tokio::test]
    async fn router_route_uses_bearer_auth_and_preserves_fallback_identity() {
        let (root, captured) = fixture_server(200, TOOL_RESPONSE, "").await;
        let route = AnthropicMultimodalRoute::nine_router(
            root,
            Some("router-secret".into()),
            "vision-combo",
        );
        let response = AnthropicMultimodalClient::new(route)
            .unwrap()
            .complete_structured(request())
            .await
            .unwrap();
        assert!(response.route.may_fallback);
        let http = captured.await.unwrap();
        assert!(
            http.to_ascii_lowercase()
                .contains("authorization: bearer router-secret")
        );
        assert!(!http.to_ascii_lowercase().contains("x-api-key"));
    }

    #[tokio::test]
    async fn rate_limit_preserves_retry_after_without_secret_diagnostic() {
        let (root, _) = fixture_server(429, r#"{"error":"busy"}"#, "retry-after: 7\r\n").await;
        let route = AnthropicMultimodalRoute::nine_router(
            root,
            Some("never-log-this".into()),
            "vision-combo",
        );
        let error = AnthropicMultimodalClient::new(route)
            .unwrap()
            .complete_structured(request())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            MultimodalLlmError::RateLimited { ref retry } if retry.contains('7')
        ));
        assert!(!error.to_string().contains("never-log-this"));
    }

    #[tokio::test]
    async fn text_only_response_is_rejected_when_tool_was_forced() {
        let body = r#"{"model":"m","content":[{"type":"text","text":"{}"}],"usage":{}}"#;
        let (root, _) = fixture_server(200, body, "").await;
        let client = AnthropicMultimodalClient::new(AnthropicMultimodalRoute::nine_router(
            root, None, "vision",
        ))
        .unwrap();
        assert!(matches!(
            client.complete_structured(request()).await,
            Err(MultimodalLlmError::InvalidResponse(_))
        ));
    }
}
