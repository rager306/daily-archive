//! Strict text prompt execution over the provider-neutral legacy `LLMClient`.

use async_trait::async_trait;
use da_domain::{assurance::ContentDigest, prompt_optimization::PromptExecutionLineage};
use da_ports::{
    llm_client::{ChatMessage, ChatOptions, LLMClient},
    prompt_execution::{
        PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptExecutionResponse,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};
pub struct TextPromptExecutionAdapter {
    llm: Arc<dyn LLMClient>,
}
impl TextPromptExecutionAdapter {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }
}
#[async_trait]
impl PromptExecutionPort for TextPromptExecutionAdapter {
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
            .output_schema
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        if !request.media.is_empty() {
            return Err(PromptExecutionError::Contract(
                "text prompt adapter does not accept media".into(),
            ));
        }
        if request.run_id.trim().is_empty() {
            return Err(PromptExecutionError::Contract("run_id is empty".into()));
        }
        if request.task.route_policy_id != request.route.route_policy_id
            || request.task.output_schema_id != request.output_schema.output_schema_id
        {
            return Err(PromptExecutionError::Contract(
                "task route/schema reference mismatch".into(),
            ));
        }
        if request.task.temperature_milli > request.route.maximum_temperature_milli {
            return Err(PromptExecutionError::Contract(
                "task temperature exceeds route policy".into(),
            ));
        }
        if request.route.supports_images {
            return Err(PromptExecutionError::Contract(
                "text adapter cannot execute image route".into(),
            ));
        }
        request
            .resolved_model
            .validate()
            .map_err(PromptExecutionError::Contract)?;
        if request.route.model_binding_id != request.resolved_model.binding_id {
            return Err(PromptExecutionError::Contract(
                "route/model binding identity mismatch".into(),
            ));
        }
        if self.llm.provider_id() != request.resolved_model.provider {
            return Err(PromptExecutionError::Contract(format!(
                "provider identity mismatch expected={} actual={}",
                request.resolved_model.provider,
                self.llm.provider_id()
            )));
        }
        if !self.llm.can_make_request() {
            return Err(PromptExecutionError::Provider(
                "LLM provider is not configured".into(),
            ));
        }
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
        let system_template = render_once(&request.task.system_template, &request.variables)?;
        let schema_json = serde_json::to_string(&request.output_schema.schema)
            .map_err(|error| PromptExecutionError::Contract(error.to_string()))?;
        let system = format!("{system_template}\n\nOUTPUT JSON SCHEMA:\n{schema_json}");
        let user = render_once(&request.task.user_template, &request.variables)?;
        let model = request.resolved_model.model_name.clone();
        let endpoint = request.resolved_model.endpoint.clone();
        let rendered = format!("system\0{system}\0user\0{user}");
        let input_identity =
            serde_json::to_vec(&(request.variables.clone(), request.input_bytes.clone()))
                .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        let started = Instant::now();
        let response = self
            .llm
            .chat(
                &[
                    ChatMessage {
                        role: "system".into(),
                        content: system,
                    },
                    ChatMessage {
                        role: "user".into(),
                        content: user,
                    },
                ],
                &ChatOptions {
                    model: model.clone(),
                    max_tokens: request.task.max_output_tokens as usize,
                    temperature: f32::from(request.task.temperature_milli) / 1000.0,
                },
            )
            .await
            .map_err(|e| PromptExecutionError::Provider(e.to_string()))?;
        let latency_ms = started.elapsed().as_millis() as u64;
        if !response.ok {
            return Err(PromptExecutionError::Provider(
                response
                    .error
                    .unwrap_or_else(|| "provider returned unsuccessful response".into()),
            ));
        }
        let output = response.text.into_bytes();
        let output_sha256 = ContentDigest::of_bytes(&output);
        let response_chars = String::from_utf8_lossy(&output).chars().count();
        let instance: serde_json::Value = match serde_json::from_slice(&output) {
            Ok(value) => value,
            Err(direct_error) => {
                let text = String::from_utf8_lossy(&output);
                match extract_json_object(&text) {
                    Some(json_text) => match serde_json::from_str(&json_text) {
                        Ok(value) => {
                            tracing::warn!(
                                event = "prompt_output_json_extracted",
                                task_id = %request.task.task_id,
                                response_chars,
                                extracted_chars = json_text.len(),
                                "Provider output required JSON extraction from wrapper text"
                            );
                            value
                        }
                        Err(extract_error) => {
                            tracing::error!(
                                event = "prompt_output_invalid_json",
                                task_id = %request.task.task_id,
                                response_chars,
                                response_sha256 = output_sha256.as_str(),
                                direct_error = %direct_error,
                                extract_error = %extract_error,
                                "Prompt provider output failed JSON parsing and extraction"
                            );
                            return Err(PromptExecutionError::Schema(format!(
                                "output is not JSON: {direct_error}"
                            )));
                        }
                    },
                    None => {
                        let after_think = text
                            .find("</think>")
                            .map_or("NO_CLOSE_THINK", |_| "CLOSED_THINK");
                        tracing::error!(
                            event = "prompt_output_invalid_json",
                            task_id = %request.task.task_id,
                            response_chars,
                            response_sha256 = output_sha256.as_str(),
                            error = %direct_error,
                            response_prefix = %text.chars().take(200).collect::<String>(),
                            think_boundary = after_think,
                            "Prompt provider output failed JSON parsing"
                        );
                        return Err(PromptExecutionError::Schema(format!(
                            "output is not JSON: {direct_error}"
                        )));
                    }
                }
            }
        };
        let validator = jsonschema::validator_for(&request.output_schema.schema)
            .map_err(|e| PromptExecutionError::Contract(format!("output schema invalid: {e}")))?;
        if let Err(error) = validator.validate(&instance) {
            tracing::error!(
                event = "prompt_output_schema_violation",
                task_id = %request.task.task_id,
                response_chars,
                response_sha256 = output_sha256.as_str(),
                instance_path = %error.instance_path(),
                "Prompt provider output failed schema validation"
            );
            return Err(PromptExecutionError::Schema(format!(
                "output schema violation at {}",
                error.instance_path()
            )));
        }
        let output_bytes = serde_json::to_vec(&instance)
            .map_err(|e| PromptExecutionError::Contract(e.to_string()))?;
        let output_sha256 = ContentDigest::of_bytes(&output_bytes);
        let route_sha = ContentDigest::of_bytes(
            &serde_json::to_vec(&request.route)
                .map_err(|e| PromptExecutionError::Contract(e.to_string()))?,
        );
        let lineage = PromptExecutionLineage {
            run_id: request.run_id,
            task_id: request.task.task_id.clone(),
            task_spec_sha256: request
                .task
                .spec_sha256()
                .map_err(PromptExecutionError::Contract)?,
            rendered_prompt_sha256: ContentDigest::of_bytes(rendered.as_bytes()),
            output_schema_sha256: request
                .output_schema
                .sha256()
                .map_err(PromptExecutionError::Contract)?,
            route_policy_sha256: route_sha,
            endpoint_sha256: ContentDigest::of_text(&endpoint),
            model_binding_sha256: request
                .resolved_model
                .sha256()
                .map_err(PromptExecutionError::Contract)?,
            provider: self.llm.provider_id().into(),
            model,
            input_sha256: ContentDigest::of_bytes(&input_identity),
            output_sha256,
            input_tokens: u64::from(response.tokens_in.unwrap_or(0)),
            output_tokens: u64::from(response.tokens_out.unwrap_or(0)),
            latency_ms,
            temperature_milli: request.task.temperature_milli,
            max_output_tokens: request.task.max_output_tokens,
            estimated_cost_microunits: None,
            authoritative: false,
            import_eligible: false,
        };
        lineage.validate().map_err(PromptExecutionError::Contract)?;
        tracing::info!(event="prompt_execution_complete",run_id=%lineage.run_id,task_id=%lineage.task_id,provider=%lineage.provider,model=%lineage.model,input_tokens=lineage.input_tokens,output_tokens=lineage.output_tokens,latency_ms=lineage.latency_ms,authoritative=false,import_eligible=false,"Prompt execution completed");
        Ok(PromptExecutionResponse {
            output_bytes,
            lineage,
        })
    }
}
fn placeholders(template: &str) -> Result<BTreeSet<String>, PromptExecutionError> {
    let mut out = BTreeSet::new();
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let end = template[i + 1..]
                .find('}')
                .map(|x| i + 1 + x)
                .ok_or_else(|| {
                    PromptExecutionError::Render("unclosed template placeholder".into())
                })?;
            let name = &template[i + 1..end];
            let valid = !name.is_empty()
                && name.chars().enumerate().all(|(n, c)| {
                    if n == 0 {
                        c.is_ascii_alphabetic() || c == '_'
                    } else {
                        c.is_ascii_alphanumeric() || c == '_'
                    }
                });
            if !valid {
                return Err(PromptExecutionError::Render(format!(
                    "invalid template placeholder {name}"
                )));
            }
            out.insert(name.into());
            i = end + 1;
        } else if bytes[i] == b'}' {
            return Err(PromptExecutionError::Render(
                "unmatched template brace".into(),
            ));
        } else {
            i += 1;
        }
    }
    Ok(out)
}
fn render_once(
    template: &str,
    variables: &BTreeMap<String, String>,
) -> Result<String, PromptExecutionError> {
    let mut out = String::with_capacity(template.len());
    let mut cursor = 0;
    while let Some(relative) = template[cursor..].find('{') {
        let start = cursor + relative;
        out.push_str(&template[cursor..start]);
        let end = start
            + 1
            + template[start + 1..].find('}').ok_or_else(|| {
                PromptExecutionError::Render("unclosed template placeholder".into())
            })?;
        let name = &template[start + 1..end];
        out.push_str(variables.get(name).ok_or_else(|| {
            PromptExecutionError::Render(format!("missing template variable {name}"))
        })?);
        cursor = end + 1;
    }
    out.push_str(&template[cursor..]);
    Ok(out)
}
/// Extract a JSON object from text that may contain markdown fences or conversational preamble.
/// Returns the substring of the first balanced `{...}` block, or None if no valid JSON object is found.
/// Handles reasoning model tags (<think>...</think>) by trying post-reasoning text first,
/// then falling back to the full text.
fn extract_json_object(text: &str) -> Option<String> {
    let after_think = text.find("</think>").map_or(text, |pos| &text[pos + 8..]);
    find_balanced_json(after_think).or_else(|| find_balanced_json(text))
}
fn find_balanced_json(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return Some(trimmed.to_owned());
    }
    let start = trimmed.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in trimmed[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' if !escaped => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(trimmed[start..start + i + 1].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::prompt_optimization::{
        PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec, ResolvedModelBinding,
    };
    use da_ports::llm_client::{ChatResponse, LLMError, LLMResult};
    struct Fake {
        ok: bool,
        wrapper: bool,
    }
    #[async_trait]
    impl LLMClient for Fake {
        async fn chat(&self, _: &[ChatMessage], _: &ChatOptions) -> LLMResult<ChatResponse> {
            if self.ok {
                let text = if self.wrapper {
                    "Here is your result:\n{\"ok\":true}\nHope this helps!".into()
                } else {
                    "{\"ok\":true}".into()
                };
                Ok(ChatResponse {
                    text,
                    ok: true,
                    error: None,
                    tokens_in: Some(10),
                    tokens_out: Some(4),
                })
            } else {
                Err(LLMError::Timeout)
            }
        }
        async fn extract_structured(
            &self,
            _: &str,
            _: &serde_json::Value,
        ) -> LLMResult<serde_json::Value> {
            unreachable!()
        }
        fn can_make_request(&self) -> bool {
            true
        }
        fn provider_id(&self) -> &str {
            "fake"
        }
    }
    fn task() -> PromptTaskSpec {
        PromptTaskSpec {
            task_id: "summary".into(),
            version: "v1".into(),
            system_template: "System {title}".into(),
            user_template: "Body {text}".into(),
            output_schema_id: "schema".into(),
            route_policy_id: "route".into(),
            temperature_milli: 0,
            max_output_tokens: 100,
            eval_suite_id: "eval".into(),
            optimizer_enabled: false,
        }
    }
    fn route() -> PromptRoutePolicy {
        PromptRoutePolicy {
            route_policy_id: "route".into(),
            model_binding_id: "binding".into(),
            endpoint_override_env: Some("NINEROUTER_URL".into()),
            credential_env: "NINEROUTER_KEY".into(),
            supports_images: false,
            maximum_temperature_milli: 1000,
        }
    }
    fn schema() -> PromptOutputSchema {
        PromptOutputSchema {
            output_schema_id: "schema".into(),
            schema: serde_json::json!({"type":"object","additionalProperties":false,"required":["ok"],"properties":{"ok":{"type":"boolean"}}}),
        }
    }
    fn model() -> ResolvedModelBinding {
        ResolvedModelBinding {
            binding_id: "binding".into(),
            model_id: "model-id".into(),
            provider: "fake".into(),
            protocol: "test".into(),
            endpoint: "https://example.invalid/v1/chat/completions".into(),
            model_name: "model-v1".into(),
            tool_version: "1".into(),
            policy_version: "1".into(),
        }
    }
    fn adapter(ok: bool) -> TextPromptExecutionAdapter {
        TextPromptExecutionAdapter::new(Arc::new(Fake { ok, wrapper: false }))
    }
    fn request() -> PromptExecutionRequest {
        PromptExecutionRequest {
            run_id: "r1".into(),
            task: task(),
            route: route(),
            resolved_model: model(),
            output_schema: schema(),
            variables: BTreeMap::from([
                ("title".into(), "T".into()),
                ("text".into(), "X {not_recursive}".into()),
            ]),
            input_bytes: b"input".to_vec(),
            media: vec![],
        }
    }
    #[tokio::test]
    async fn complete_lineage_binds_real_contracts_without_authority() {
        let r = adapter(true).execute(request()).await.unwrap();
        assert_eq!(r.lineage.model, "model-v1");
        assert_eq!(r.lineage.input_tokens, 10);
        assert!(!r.lineage.authoritative && !r.lineage.import_eligible);
        assert_ne!(
            r.lineage.output_schema_sha256,
            ContentDigest::of_text("schema")
        );
    }
    #[tokio::test]
    async fn variable_drift_and_provider_failure_are_visible() {
        let mut x = request();
        x.variables.insert("extra".into(), "bad".into());
        assert!(matches!(
            adapter(true).execute(x).await.unwrap_err(),
            PromptExecutionError::Render(_)
        ));
        assert!(matches!(
            adapter(false).execute(request()).await.unwrap_err(),
            PromptExecutionError::Provider(_)
        ));
    }
    #[tokio::test]
    async fn schema_violation_is_rejected_before_response() {
        let mut x = request();
        x.output_schema.schema = serde_json::json!({"type":"object","required":["missing"]});
        assert!(matches!(
            adapter(true).execute(x).await.unwrap_err(),
            PromptExecutionError::Schema(_)
        ));
    }
    #[tokio::test]
    async fn wrapper_text_returns_canonical_json_bytes() {
        // Models that wrap JSON in conversational text must still yield canonical,
        // directly parseable output bytes (regression: raw wrapper leaked into output).
        let adapter = TextPromptExecutionAdapter::new(Arc::new(Fake {
            ok: true,
            wrapper: true,
        }));
        let response = adapter.execute(request()).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&response.output_bytes).unwrap();
        assert_eq!(parsed, serde_json::json!({ "ok": true }));
        assert_eq!(
            response.lineage.output_sha256,
            ContentDigest::of_bytes(&response.output_bytes)
        );
    }
    #[test]
    fn extract_json_handles_preamble_and_code_fences() {
        assert_eq!(
            extract_json_object("Here is the JSON:\n{\"a\":1}").unwrap(),
            "{\"a\":1}"
        );
        assert_eq!(
            extract_json_object("```json\n{\"b\":2}\n```").unwrap(),
            "{\"b\":2}"
        );
        assert_eq!(
            extract_json_object("{\"nested\":{\"x\":1}}").unwrap(),
            "{\"nested\":{\"x\":1}}"
        );
        assert_eq!(
            extract_json_object("Text {\"str\":\"contains } brace\"} trailing").unwrap(),
            "{\"str\":\"contains } brace\"}"
        );
        assert!(extract_json_object("no json here").is_none());
    }
    #[test]
    fn extract_json_strips_reasoning_think_blocks() {
        assert_eq!(
            extract_json_object("<think>Let me analyze...</think>{\"result\":42}").unwrap(),
            "{\"result\":42}"
        );
    }
}
