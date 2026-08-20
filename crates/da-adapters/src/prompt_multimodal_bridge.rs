//! Compatibility bridge: PromptExecutionPort drives existing figure pipeline port.

use async_trait::async_trait;
use da_domain::prompt_optimization::{
    PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec, ResolvedModelBinding,
};
use da_ports::{
    multimodal_llm::{
        ModelRouteDescriptor, ModelWireProtocol, MultimodalImage, MultimodalLlmError,
        MultimodalLlmPort, MultimodalLlmRequest, MultimodalLlmResponse, MultimodalLlmResult,
    },
    prompt_execution::{
        PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptMedia,
    },
};
use std::{collections::BTreeMap, sync::Arc};
pub struct PromptMultimodalBridge {
    executor: Arc<dyn PromptExecutionPort>,
    task: PromptTaskSpec,
    route: PromptRoutePolicy,
    model: ResolvedModelBinding,
    schema: PromptOutputSchema,
}
impl PromptMultimodalBridge {
    pub fn new(
        executor: Arc<dyn PromptExecutionPort>,
        task: PromptTaskSpec,
        route: PromptRoutePolicy,
        model: ResolvedModelBinding,
        schema: PromptOutputSchema,
    ) -> Result<Self, String> {
        task.validate()?;
        route.validate()?;
        model.validate()?;
        schema.validate()?;
        if task.task_id != "figure_analysis" {
            return Err(format!(
                "figure prompt bridge contract mismatch: task_id={}",
                task.task_id
            ));
        }
        if task.route_policy_id != route.route_policy_id {
            return Err(format!(
                "figure prompt bridge contract mismatch: task.route={} route.id={}",
                task.route_policy_id, route.route_policy_id
            ));
        }
        if task.output_schema_id != schema.output_schema_id {
            return Err(format!(
                "figure prompt bridge contract mismatch: task.schema={} schema.id={}",
                task.output_schema_id, schema.output_schema_id
            ));
        }
        if route.model_binding_id != model.binding_id {
            return Err(format!(
                "figure prompt bridge contract mismatch: route.binding={} model.binding={}",
                route.model_binding_id, model.binding_id
            ));
        }
        if !route.supports_images {
            return Err(
                "figure prompt bridge contract mismatch: route.supports_images=false".into(),
            );
        }
        if model.protocol != "openai_chat_completions" {
            return Err(format!(
                "figure prompt bridge contract mismatch: protocol={}",
                model.protocol
            ));
        }
        Ok(Self {
            executor,
            task,
            route,
            model,
            schema,
        })
    }
}
#[async_trait]
impl MultimodalLlmPort for PromptMultimodalBridge {
    fn route(&self) -> ModelRouteDescriptor {
        ModelRouteDescriptor {
            route_id: self.model.binding_id.clone(),
            protocol: ModelWireProtocol::OpenAiChatCompletions,
            model: self.model.model_name.clone(),
            endpoint_fingerprint: self
                .model
                .sha256()
                .expect("binding validated")
                .as_str()
                .into(),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: false,
        }
    }
    async fn complete_structured(
        &self,
        request: MultimodalLlmRequest,
    ) -> MultimodalLlmResult<MultimodalLlmResponse> {
        request.validate()?;
        let image = media(request.image)?;
        let run_id = format!("figure:{}:{}", self.task.version, image.sha256);
        let response = self
            .executor
            .execute(PromptExecutionRequest {
                run_id,
                task: self.task.clone(),
                route: self.route.clone(),
                resolved_model: self.model.clone(),
                output_schema: self.schema.clone(),
                variables: BTreeMap::from([(
                    "grounded_context".into(),
                    request.user_context.clone(),
                )]),
                input_bytes: request.user_context.into_bytes(),
                media: vec![image],
            })
            .await
            .map_err(map_error)?;
        let output: serde_json::Value = serde_json::from_slice(&response.output_bytes)
            .map_err(|e| MultimodalLlmError::InvalidResponse(e.to_string()))?;
        Ok(MultimodalLlmResponse {
            structured_output: output,
            route: self.route(),
            resolved_model: response.lineage.model.clone(),
            input_tokens: u32::try_from(response.lineage.input_tokens).ok(),
            output_tokens: u32::try_from(response.lineage.output_tokens).ok(),
            latency_ms: response.lineage.latency_ms,
            prompt_lineage: Some(response.lineage),
        })
    }
}
fn media(image: MultimodalImage) -> MultimodalLlmResult<PromptMedia> {
    image.validate()?;
    Ok(PromptMedia {
        media_type: image.media_type.as_mime().into(),
        bytes: image.bytes,
        sha256: image.sha256,
    })
}
fn map_error(error: PromptExecutionError) -> MultimodalLlmError {
    match error {
        PromptExecutionError::Timeout { timeout_ms } => MultimodalLlmError::Timeout { timeout_ms },
        PromptExecutionError::RateLimited { retry } => MultimodalLlmError::RateLimited { retry },
        PromptExecutionError::Unavailable(x) => MultimodalLlmError::Unavailable(x),
        PromptExecutionError::Contract(x) | PromptExecutionError::Render(x) => {
            MultimodalLlmError::InvalidRequest(x)
        }
        PromptExecutionError::Schema(x) => MultimodalLlmError::InvalidResponse(x),
        PromptExecutionError::Provider(x) => MultimodalLlmError::Unavailable(x),
    }
}
