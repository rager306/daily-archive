//! OpenAI-compatible typed-media PromptExecutionPort for 9router figure tasks.

use async_trait::async_trait;
use base64::Engine;
use da_domain::{assurance::ContentDigest, prompt_optimization::PromptExecutionLineage};
use da_ports::prompt_execution::{
    PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptExecutionResponse,
};
use serde::Deserialize;
use std::{collections::BTreeSet, time::Instant};
pub struct OpenAiMultimodalPromptAdapter {
    client: reqwest::Client,
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
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}
impl OpenAiMultimodalPromptAdapter {
    pub fn new(api_key: Option<String>, timeout_seconds: u64) -> Result<Self, String> {
        if timeout_seconds == 0 {
            return Err("multimodal timeout must be positive".into());
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            api_key: api_key.filter(|x| !x.trim().is_empty()),
        })
    }
}
#[async_trait]
impl PromptExecutionPort for OpenAiMultimodalPromptAdapter {
    async fn execute(
        &self,
        request: PromptExecutionRequest,
    ) -> Result<PromptExecutionResponse, PromptExecutionError> {
        request
            .task
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        request
            .route
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        request
            .resolved_model
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        request
            .output_schema
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        if request.run_id.trim().is_empty()
            || request.task.route_policy_id != request.route.route_policy_id
            || request.task.output_schema_id != request.output_schema.output_schema_id
            || request.route.model_binding_id != request.resolved_model.binding_id
        {
            return Err(PromptExecutionError::Contract(
                "multimodal task/route/model/schema identity mismatch".into(),
            ));
        }
        if !request.route.supports_images
            || request.resolved_model.protocol != "openai_chat_completions"
            || request.resolved_model.provider != "openai"
        {
            return Err(PromptExecutionError::Contract(
                "multimodal route lacks OpenAI image capability".into(),
            ));
        }
        if request.media.len() != 1 {
            return Err(PromptExecutionError::Contract(
                "figure prompt requires exactly one media item".into(),
            ));
        }
        request.media[0].validate()?;
        let expected = placeholders(&request.task.system_template)?
            .into_iter()
            .chain(placeholders(&request.task.user_template)?)
            .collect::<BTreeSet<_>>();
        let actual = request.variables.keys().cloned().collect::<BTreeSet<_>>();
        if expected != actual {
            return Err(PromptExecutionError::Render(format!(
                "template variable set mismatch expected={expected:?} actual={actual:?}"
            )));
        }
        let schema_json = serde_json::to_string(&request.output_schema.schema)
            .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        let system = format!(
            "{}\n\nOUTPUT JSON SCHEMA:\n{schema_json}",
            render(&request.task.system_template, &request.variables)?
        );
        let user = render(&request.task.user_template, &request.variables)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&request.media[0].bytes);
        let data_url = format!("data:{};base64,{encoded}", request.media[0].media_type);
        let body = serde_json::json!({"model":request.resolved_model.model_name,"messages":[{"role":"system","content":system},{"role":"user","content":[{"type":"text","text":user},{"type":"image_url","image_url":{"url":data_url}}]}],"temperature":f32::from(request.task.temperature_milli)/1000.0,"max_tokens":request.task.max_output_tokens,"response_format":{"type":"json_object"},"stream":false});
        let mut builder = self
            .client
            .post(&request.resolved_model.endpoint)
            .header("content-type", "application/json")
            .json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let started = Instant::now();
        let response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                PromptExecutionError::Timeout {
                    timeout_ms: started.elapsed().as_millis() as u64,
                }
            } else {
                PromptExecutionError::Unavailable(e.to_string())
            }
        })?;
        let status = response.status();
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|x| x.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|e| PromptExecutionError::Unavailable(e.to_string()))?;
        let response_sha = ContentDigest::of_bytes(&bytes);
        if status.as_u16() == 429 {
            return Err(PromptExecutionError::RateLimited {
                retry: retry
                    .map(|x| format!("; retry-after={x}"))
                    .unwrap_or_default(),
            });
        }
        if !status.is_success() {
            tracing::error!(event="multimodal_prompt_http_error",status=%status,response_bytes=bytes.len(),response_sha256=response_sha.as_str(),"9router multimodal request failed");
            return Err(PromptExecutionError::Provider(format!(
                "9router HTTP {status}; response_bytes={} response_sha256={}",
                bytes.len(),
                response_sha.as_str()
            )));
        }
        let parsed: Response = serde_json::from_slice(&bytes).map_err(|e| {
            PromptExecutionError::Schema(format!("9router response JSON invalid: {e}"))
        })?;
        let message = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| PromptExecutionError::Schema("9router response has no choices".into()))?
            .message;
        let text = message
            .content
            .filter(|x| !x.trim().is_empty())
            .or(message.reasoning_content.filter(|x| !x.trim().is_empty()))
            .ok_or_else(|| {
                PromptExecutionError::Schema("9router response has no text content".into())
            })?;
        let response_chars = text.chars().count();
        let output_sha256 = ContentDigest::of_text(&text);
        let instance: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(direct_error) => match crate::prompt_execution::extract_json_object(&text) {
                Some(json_text) => match serde_json::from_str(&json_text) {
                    Ok(value) => {
                        tracing::warn!(
                            event = "multimodal_prompt_output_json_extracted",
                            response_chars,
                            extracted_chars = json_text.len(),
                            "Multimodal provider output required JSON extraction from wrapper text"
                        );
                        value
                    }
                    Err(extract_error) => {
                        tracing::error!(
                            event = "multimodal_prompt_output_invalid_json",
                            response_chars,
                            response_sha256 = output_sha256.as_str(),
                            direct_error = %direct_error,
                            extract_error = %extract_error,
                            response_prefix = %text.chars().take(200).collect::<String>(),
                            "Multimodal provider output failed JSON parsing and extraction"
                        );
                        return Err(PromptExecutionError::Schema(format!(
                            "figure output is not JSON: {direct_error}"
                        )));
                    }
                },
                None => {
                    tracing::error!(
                        event = "multimodal_prompt_output_invalid_json",
                        response_chars,
                        response_sha256 = output_sha256.as_str(),
                        error = %direct_error,
                        response_prefix = %text.chars().take(200).collect::<String>(),
                        "Multimodal provider output failed JSON parsing"
                    );
                    return Err(PromptExecutionError::Schema(format!(
                        "figure output is not JSON: {direct_error}"
                    )));
                }
            },
        };
        let validator = jsonschema::validator_for(&request.output_schema.schema)
            .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        if let Err(error) = validator.validate(&instance) {
            return Err(PromptExecutionError::Schema(format!(
                "figure output schema violation at {}",
                error.instance_path()
            )));
        }
        let output_bytes = serde_json::to_vec(&instance)
            .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        let input_identity = serde_json::to_vec(&(
            request.variables.clone(),
            request.input_bytes.clone(),
            request
                .media
                .iter()
                .map(|x| (&x.media_type, &x.sha256))
                .collect::<Vec<_>>(),
        ))
        .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        let lineage = PromptExecutionLineage {
            run_id: request.run_id,
            task_id: request.task.task_id.clone(),
            task_spec_sha256: request
                .task
                .spec_sha256()
                .map_err(PromptExecutionError::Contract)?,
            rendered_prompt_sha256: ContentDigest::of_bytes(format!("{system}\0{user}").as_bytes()),
            output_schema_sha256: request
                .output_schema
                .sha256()
                .map_err(PromptExecutionError::Contract)?,
            route_policy_sha256: ContentDigest::of_bytes(
                &serde_json::to_vec(&request.route)
                    .map_err(|e| PromptExecutionError::Contract(e.to_string()))?,
            ),
            endpoint_sha256: ContentDigest::of_text(&request.resolved_model.endpoint),
            model_binding_sha256: request
                .resolved_model
                .sha256()
                .map_err(PromptExecutionError::Contract)?,
            provider: request.resolved_model.provider,
            model: request.resolved_model.model_name,
            input_sha256: ContentDigest::of_bytes(&input_identity),
            output_sha256: ContentDigest::of_bytes(&output_bytes),
            input_tokens: parsed.usage.prompt_tokens.unwrap_or(0),
            output_tokens: parsed.usage.completion_tokens.unwrap_or(0),
            latency_ms: started.elapsed().as_millis() as u64,
            temperature_milli: request.task.temperature_milli,
            max_output_tokens: request.task.max_output_tokens,
            estimated_cost_microunits: None,
            authoritative: false,
            import_eligible: false,
        };
        lineage.validate().map_err(PromptExecutionError::Contract)?;
        tracing::info!(event="multimodal_prompt_execution_complete",run_id=%lineage.run_id,task_id=%lineage.task_id,model=%lineage.model,image_sha256=%request.media[0].sha256,input_tokens=lineage.input_tokens,output_tokens=lineage.output_tokens,latency_ms=lineage.latency_ms,authoritative=false,import_eligible=false,"Multimodal prompt execution completed");
        Ok(PromptExecutionResponse {
            output_bytes,
            lineage,
        })
    }
}
fn placeholders(template: &str) -> Result<BTreeSet<String>, PromptExecutionError> {
    let mut out = BTreeSet::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .ok_or_else(|| PromptExecutionError::Render("unclosed placeholder".into()))?;
        let name = &after[..end];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(PromptExecutionError::Render("invalid placeholder".into()));
        }
        out.insert(name.into());
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return Err(PromptExecutionError::Render(
            "unmatched placeholder brace".into(),
        ));
    }
    Ok(out)
}
fn render(
    template: &str,
    variables: &std::collections::BTreeMap<String, String>,
) -> Result<String, PromptExecutionError> {
    let mut out = template.to_string();
    for key in placeholders(template)? {
        out = out.replace(
            &format!("{{{key}}}"),
            variables
                .get(&key)
                .ok_or_else(|| PromptExecutionError::Render(format!("missing variable {key}")))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::prompt_optimization::{
        PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec, ResolvedModelBinding,
    };
    use da_ports::prompt_execution::{PromptExecutionRequest, PromptMedia};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn request(endpoint: String) -> PromptExecutionRequest {
        let bytes = b"png-fixture".to_vec();
        PromptExecutionRequest {
            run_id: "figure-run".into(),
            task: PromptTaskSpec {
                task_id: "figure_analysis".into(),
                version: "v1".into(),
                system_template: "system".into(),
                user_template: "context {grounded_context}".into(),
                output_schema_id: "figure".into(),
                route_policy_id: "route".into(),
                temperature_milli: 0,
                max_output_tokens: 1000,
                eval_suite_id: "eval".into(),
                optimizer_enabled: false,
            },
            route: PromptRoutePolicy {
                route_policy_id: "route".into(),
                model_binding_id: "figure-binding".into(),
                endpoint_override_env: None,
                credential_env: "KEY".into(),
                supports_images: true,
                maximum_temperature_milli: 1000,
            },
            resolved_model: ResolvedModelBinding {
                binding_id: "figure-binding".into(),
                model_id: "vision".into(),
                provider: "openai".into(),
                protocol: "openai_chat_completions".into(),
                endpoint,
                model_name: "novita/qwen/qwen3-vl-235b-a22b-instruct".into(),
                tool_version: "1".into(),
                policy_version: "1".into(),
            },
            output_schema: PromptOutputSchema {
                output_schema_id: "figure".into(),
                schema: serde_json::json!({"type":"object","additionalProperties":false,"required":["ok"],"properties":{"ok":{"type":"boolean"}}}),
            },
            variables: BTreeMap::from([("grounded_context".into(), "evidence".into())]),
            input_bytes: b"context".to_vec(),
            media: vec![PromptMedia {
                media_type: "image/png".into(),
                sha256: hex::encode(Sha256::digest(&bytes)),
                bytes,
            }],
        }
    }
    #[tokio::test]
    async fn local_openai_multimodal_roundtrip_binds_media_and_lineage() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = vec![0u8; 65536];
            let n = stream.read(&mut data).await.unwrap();
            let text = String::from_utf8_lossy(&data[..n]);
            assert!(text.contains("data:image/png;base64"));
            assert!(text.contains("novita/qwen/qwen3-vl-235b-a22b-instruct"));
            let content = r#"{"ok":true}"#;
            let body=serde_json::json!({"choices":[{"message":{"content":content}}],"usage":{"prompt_tokens":20,"completion_tokens":6}}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let adapter = OpenAiMultimodalPromptAdapter::new(None, 5).unwrap();
        let response = adapter
            .execute(request(format!("http://{address}/v1/chat/completions")))
            .await
            .unwrap();
        assert_eq!(response.lineage.input_tokens, 20);
        assert_eq!(response.lineage.output_tokens, 6);
        assert!(!response.lineage.authoritative && !response.lineage.import_eligible);
        server.await.unwrap();
    }
    #[tokio::test]
    async fn missing_or_corrupt_media_fails_before_network() {
        let adapter = OpenAiMultimodalPromptAdapter::new(None, 1).unwrap();
        let mut value = request("http://127.0.0.1:9/v1/chat/completions".into());
        value.media.clear();
        assert!(matches!(
            adapter.execute(value).await.unwrap_err(),
            PromptExecutionError::Contract(_)
        ));
        let mut value = request("http://127.0.0.1:9/v1/chat/completions".into());
        value.media[0].sha256 = "a".repeat(64);
        assert!(matches!(
            adapter.execute(value).await.unwrap_err(),
            PromptExecutionError::Contract(_)
        ));
    }
}
