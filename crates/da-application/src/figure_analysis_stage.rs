//! Cache-aware pipeline stage for non-authoritative figure analysis.
//! Stage contract binds DATA figure schema registry identity (M389 S01).

use crate::figure_schema::FigureSchemaService;
use crate::{FigureAnalysisUseCase, FigureAnalysisUseCaseError};
use async_trait::async_trait;
use da_domain::FigureSemanticContext;
use da_ports::multimodal_llm::{
    ModelWireProtocol, MultimodalImage, MultimodalLlmError, MultimodalLlmPort,
};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Source stage that publishes a validated canonical FigureSemanticContext into
/// the DAG. Its schema and payload hash fence all downstream analysis stages.
pub struct FigureContextArtifactStage {
    context: FigureSemanticContext,
    contract: StageContract,
}

impl FigureContextArtifactStage {
    pub fn new(context: FigureSemanticContext) -> Result<Self, StageExecutionError> {
        context
            .validate()
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "invalid_canonical_figure_context".into(),
                message: error.to_string(),
            })?;
        let bytes =
            serde_json::to_vec(&context).map_err(|error| StageExecutionError::Terminal {
                error_class: "figure_context_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(Self {
            context,
            contract: StageContract {
                stage_id: "figure_context_artifact".into(),
                implementation_fingerprint: "FigureContextArtifactStage@v1".into(),
                configuration_fingerprint: format!(
                    "context_sha256={}",
                    hex::encode(Sha256::digest(&bytes))
                ),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: "FigureSemanticContext@serde-v2-summary-provenance"
                    .into(),
            },
        })
    }
}

#[async_trait]
impl PipelineStageExecutor for FigureContextArtifactStage {
    fn contract(&self) -> StageContract {
        self.contract.clone()
    }

    async fn execute(
        &self,
        _request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload =
            serde_json::to_vec(&self.context).map_err(|error| StageExecutionError::Terminal {
                error_class: "figure_context_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("artifact_id".into(), self.context.artifact_id.clone()),
                (
                    "section_path".into(),
                    self.context.containing_section.path_id.clone(),
                ),
                ("authoritative".into(), "false".into()),
            ]),
            payload,
        })
    }
}

pub struct FigureAnalysisStage {
    model: Arc<dyn MultimodalLlmPort>,
    image: MultimodalImage,
    context_dependency: String,
    retry_after: String,
    contract: StageContract,
    /// Stage1 planned schema when context is known at construction (M393).
    selected_schema: Option<(String, String)>,
}

impl FigureAnalysisStage {
    /// DATA schema identity for stage cache keys (registry version + every schema sha).
    /// YAML edits to any per-type schema must invalidate the cache (M393).
    pub fn schema_registry_fingerprint() -> String {
        let svc = FigureSchemaService::bundled().expect("bundled figure schemas");
        let rows = svc.registry_identities().expect("registry identities");
        let joined = rows
            .into_iter()
            .map(|(id, sha)| format!("{id}:{sha}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "schema_registry={};schemas={}",
            svc.registry_schema_version(),
            joined
        )
    }

    pub fn new(
        model: Arc<dyn MultimodalLlmPort>,
        image: MultimodalImage,
        retry_after: impl Into<String>,
    ) -> Self {
        let route = model.route();
        Self {
            model,
            image,
            context_dependency: "context".into(),
            retry_after: retry_after.into(),
            selected_schema: None,
            contract: StageContract {
                stage_id: "figure_analysis".into(),
                implementation_fingerprint:
                    "FigureAnalysisUseCase@grounded-v4-all-schema-identities".into(),
                configuration_fingerprint: String::new(),
                prompt_fingerprint: Some(FigureAnalysisUseCase::prompt_sha256()),
                model_fingerprint: Some(format!(
                    "{}|{}|{}|fallback={}",
                    route.route_id,
                    protocol_name(route.protocol),
                    route.model,
                    route.may_fallback
                )),
                output_schema_fingerprint: FigureAnalysisUseCase::output_schema_sha256(),
            },
        }
    }

    pub fn with_context_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.context_dependency = dependency.into();
        self
    }

    pub fn with_contract_versions(
        mut self,
        prompt_fingerprint: impl Into<String>,
        schema_fingerprint: impl Into<String>,
    ) -> Self {
        self.contract.prompt_fingerprint = Some(prompt_fingerprint.into());
        self.contract.output_schema_fingerprint = schema_fingerprint.into();
        self
    }

    /// Bind Stage1 planned schema_id into the cache key when context is available.
    pub fn with_planned_schema(
        mut self,
        context: &FigureSemanticContext,
    ) -> Result<Self, FigureAnalysisUseCaseError> {
        let use_case = FigureAnalysisUseCase::new(self.model.clone());
        let (classification, selection) = use_case.plan_schema(context)?;
        self.selected_schema = Some((
            selection.schema_id,
            classification.visual_type.as_wire().to_string(),
        ));
        Ok(self)
    }
}

#[async_trait]
impl PipelineStageExecutor for FigureAnalysisStage {
    fn contract(&self) -> StageContract {
        let mut contract = self.contract.clone();
        let mut fingerprint = format!(
            "image_sha256={};prompt_sha256={};schema_sha256={};{}",
            self.image.sha256,
            contract.prompt_fingerprint.as_deref().unwrap_or("missing"),
            contract.output_schema_fingerprint,
            Self::schema_registry_fingerprint()
        );
        if let Some((schema_id, stage1_type)) = &self.selected_schema {
            fingerprint.push_str(&format!(
                ";selected_schema_id={schema_id};stage1_type={stage1_type}"
            ));
        }
        contract.configuration_fingerprint = fingerprint;
        contract
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload = request
            .dependency_payloads
            .get(&self.context_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_figure_context_dependency".into(),
                message: format!("missing dependency {}", self.context_dependency),
            })?;
        let context: FigureSemanticContext =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "invalid_figure_context_payload".into(),
                message: error.to_string(),
            })?;
        let analysis = FigureAnalysisUseCase::new(self.model.clone())
            .analyze(self.image.clone(), &context)
            .await
            .map_err(|error| map_error(error, &self.retry_after))?;
        let output =
            serde_json::to_vec(&analysis).map_err(|error| StageExecutionError::Terminal {
                error_class: "figure_analysis_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("candidate_id".into(), analysis.candidate_id),
                ("artifact_id".into(), analysis.artifact_id),
                (
                    "visual_type".into(),
                    format!("{:?}", analysis.visual_type).to_lowercase(),
                ),
                (
                    "component_count".into(),
                    analysis.components.len().to_string(),
                ),
                ("route_id".into(), analysis.provenance.route_id),
                ("resolved_model".into(), analysis.provenance.resolved_model),
                ("authoritative".into(), "false".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}

fn map_error(error: FigureAnalysisUseCaseError, retry_after: &str) -> StageExecutionError {
    match error {
        FigureAnalysisUseCaseError::Model(
            MultimodalLlmError::Unavailable(_)
            | MultimodalLlmError::Timeout { .. }
            | MultimodalLlmError::RateLimited { .. },
        ) => StageExecutionError::Retryable {
            error_class: "multimodal_route_retryable".into(),
            message: error.to_string(),
            retry_after: retry_after.into(),
        },
        other => StageExecutionError::Terminal {
            error_class: "figure_analysis_contract".into(),
            message: other.to_string(),
        },
    }
}

fn protocol_name(protocol: ModelWireProtocol) -> &'static str {
    match protocol {
        ModelWireProtocol::AnthropicMessages => "anthropic_messages",
        ModelWireProtocol::OpenAiChatCompletions => "openai_chat_completions",
        ModelWireProtocol::OpenAiResponses => "openai_responses",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use da_domain::{
        BoundingBox72Dpi, FigureReferenceExcerpt, FigureSectionContext, FigureSectionRelation,
        VisualArtifactKind, assurance::ContentDigest, prompt_optimization::PromptExecutionLineage,
    };
    use da_ports::multimodal_llm::{
        ImageMediaType, ModelRouteDescriptor, MultimodalLlmRequest, MultimodalLlmResponse,
        MultimodalLlmResult,
    };
    use sha2::{Digest, Sha256};

    struct FakeModel {
        fail: bool,
    }

    #[async_trait]
    impl MultimodalLlmPort for FakeModel {
        fn route(&self) -> ModelRouteDescriptor {
            ModelRouteDescriptor {
                route_id: "test-route".into(),
                protocol: ModelWireProtocol::AnthropicMessages,
                model: "vision-model".into(),
                endpoint_fingerprint: "fixture".into(),
                supports_images: true,
                supports_structured_output: true,
                may_fallback: false,
            }
        }

        async fn complete_structured(
            &self,
            _request: MultimodalLlmRequest,
        ) -> MultimodalLlmResult<MultimodalLlmResponse> {
            if self.fail {
                return Err(MultimodalLlmError::Timeout { timeout_ms: 10 });
            }
            Ok(MultimodalLlmResponse {
                structured_output: serde_json::json!({
                    "visual_type":"flowchart",
                    "description":"GEPA flowchart",
                    "components":[{"name":"Mutation","role":"propose","visual_evidence":"box"}],
                    "relationships":[],
                    "findings":["candidates evaluated"],
                    "citations":[{"section_id":"sec-3","path_id":"3","quote":"mutates prompts"}],
                    "confidence":0.9
                }),
                route: self.route(),
                resolved_model: "vision-model".into(),
                input_tokens: Some(10),
                output_tokens: Some(5),
                latency_ms: 1,
                prompt_lineage: Some(PromptExecutionLineage {
                    run_id: "figure-stage-test".into(),
                    task_id: "figure_analysis".into(),
                    task_spec_sha256: ContentDigest::of_text("task"),
                    rendered_prompt_sha256: ContentDigest::of_text("prompt"),
                    output_schema_sha256: ContentDigest::of_text("schema"),
                    route_policy_sha256: ContentDigest::of_text("route"),
                    endpoint_sha256: ContentDigest::of_text("endpoint"),
                    model_binding_sha256: ContentDigest::of_text("binding"),
                    provider: "fixed".into(),
                    model: "vision-model".into(),
                    input_sha256: ContentDigest::of_text("input"),
                    output_sha256: ContentDigest::of_text("output"),
                    input_tokens: 10,
                    output_tokens: 5,
                    latency_ms: 1,
                    temperature_milli: 0,
                    max_output_tokens: 4096,
                    estimated_cost_microunits: None,
                    authoritative: false,
                    import_eligible: false,
                }),
            })
        }
    }

    fn image() -> MultimodalImage {
        let bytes = b"png".to_vec();
        MultimodalImage {
            media_type: ImageMediaType::Png,
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes,
        }
    }

    fn context() -> FigureSemanticContext {
        FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: image().sha256,
            source_pdf_sha256: "a".repeat(64),
            revision_vid: None,
            canonical_id: None,
            section_vid: None,
            detector_fingerprint: "detector".into(),
            detector_metadata_sha256: "b".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 1.0,
                y1: 1.0,
                x2: 2.0,
                y2: 2.0,
            },
            caption: "Figure 3: GEPA flowchart".into(),
            containing_section: FigureSectionContext {
                relation: FigureSectionRelation::Containing,
                section_id: "sec-3".into(),
                path_id: "3".into(),
                title: "GEPA".into(),
                text: "GEPA mutates prompts.".into(),
                truncated: false,
                section_ordinal: None,
            },
            neighboring_sections: vec![],
            reference_excerpts: vec![FigureReferenceExcerpt {
                section_id: "sec-3".into(),
                path_id: "3".into(),
                title: "GEPA".into(),
                matched_form: "figure 3".into(),
                excerpt: "GEPA mutates prompts.".into(),
                truncated: false,
            }],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: 50,
            truncated: false,
        }
    }

    fn request() -> StageExecutionRequest {
        StageExecutionRequest {
            article_id: "2507.19457".into(),
            dependency_payloads: BTreeMap::from([(
                "context".into(),
                serde_json::to_vec(&context()).unwrap(),
            )]),
        }
    }

    #[tokio::test]
    async fn canonical_context_source_stage_is_content_addressed() {
        let stage = FigureContextArtifactStage::new(context()).unwrap();
        let output = stage
            .execute(StageExecutionRequest {
                article_id: "2507.19457".into(),
                dependency_payloads: BTreeMap::new(),
            })
            .await
            .unwrap();
        assert_eq!(output.metadata["artifact_id"], "Figure3");
        assert!(
            stage
                .contract()
                .configuration_fingerprint
                .contains("context_sha256=")
        );
        let decoded: FigureSemanticContext = serde_json::from_slice(&output.payload).unwrap();
        assert_eq!(decoded, context());
    }

    #[tokio::test]
    async fn stage_emits_non_authoritative_candidate() {
        let stage = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        );
        let output = stage.execute(request()).await.unwrap();
        assert_eq!(output.metadata["visual_type"], "flowchart");
        assert_eq!(output.metadata["authoritative"], "false");
        assert_eq!(output.metadata["import_eligible"], "false");
    }

    #[tokio::test]
    async fn timeout_becomes_retryable_stage_failure() {
        let stage = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: true }),
            image(),
            "2026-08-08T01:00:00Z",
        );
        assert!(matches!(
            stage.execute(request()).await,
            Err(StageExecutionError::Retryable { .. })
        ));
    }

    #[test]
    fn route_image_prompt_and_schema_are_in_cache_contract() {
        let before = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        );
        let after = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        )
        .with_contract_versions("prompt-v2", "schema-v2");
        assert_ne!(before.contract(), after.contract());
        assert!(
            before
                .contract()
                .configuration_fingerprint
                .contains(&image().sha256)
        );
        assert!(
            before
                .contract()
                .model_fingerprint
                .unwrap()
                .contains("test-route")
        );
    }

    #[test]
    fn configuration_fingerprint_binds_schema_registry_identity() {
        let stage = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        );
        let fp = stage.contract().configuration_fingerprint;
        assert!(
            fp.contains("schema_registry=figure-schemas.v1"),
            "missing registry version in {fp}"
        );
        for schema_id in [
            "diagram_analysis",
            "chart_extraction",
            "table_structure",
            "photo_caption",
            "other_description",
        ] {
            assert!(
                fp.contains(&format!("{schema_id}:")),
                "missing {schema_id} in {fp}"
            );
        }
        let registry_fp = FigureAnalysisStage::schema_registry_fingerprint();
        assert!(fp.contains(&registry_fp));
        // implementation fence bumped so old caches do not silently reuse
        assert!(
            stage
                .contract()
                .implementation_fingerprint
                .contains("all-schema-identities")
        );
        assert!(!fp.contains("selected_schema_id="));
    }

    #[test]
    fn planned_schema_fingerprint_differs_for_plot_and_flowchart() {
        let flowchart_ctx = context();
        let mut plot_ctx = context();
        plot_ctx.caption =
            "Figure 2: Performance comparison scatter plot of accuracy vs rollouts".into();
        let flowchart = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        )
        .with_planned_schema(&flowchart_ctx)
        .expect("plan flowchart");
        let plot = FigureAnalysisStage::new(
            Arc::new(FakeModel { fail: false }),
            image(),
            "2026-08-08T01:00:00Z",
        )
        .with_planned_schema(&plot_ctx)
        .expect("plan plot");
        let fp_flow = flowchart.contract().configuration_fingerprint;
        let fp_plot = plot.contract().configuration_fingerprint;
        assert!(fp_flow.contains("selected_schema_id=diagram_analysis"));
        assert!(fp_flow.contains("stage1_type=flowchart"));
        assert!(fp_plot.contains("selected_schema_id=chart_extraction"));
        assert!(fp_plot.contains("stage1_type=plot"));
        assert_ne!(fp_flow, fp_plot);
        // Full registry binding remains even when a selected schema is present.
        assert!(fp_plot.contains("schema_registry=figure-schemas.v1"));
        assert!(fp_plot.contains("table_structure:"));
    }
}
