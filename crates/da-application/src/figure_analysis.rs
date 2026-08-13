//! Grounded figure-analysis use case over a provider-neutral multimodal port.
//! Stage1 classify + Stage2 DATA schema selection (ADR-060, M388 S05).

use crate::figure_classify::{FigureClassifyError, FigureClassifyUseCase};
use crate::figure_schema::{FigureSchemaError, FigureSchemaService};
use da_domain::{
    FigureAnalysis, FigureAnalysisCitation, FigureAnalysisComponent, FigureAnalysisError,
    FigureAnalysisProvenance, FigureAnalysisRelationship, FigureSchemaSelection,
    FigureSemanticContext, FigureTypeClassification, VisualAnalysisType,
};
use da_ports::multimodal_llm::{
    ModelWireProtocol, MultimodalImage, MultimodalLlmError, MultimodalLlmPort, MultimodalLlmRequest,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;

pub const FIGURE_ANALYSIS_PROMPT_VERSION: &str = "figure-analysis-grounded-v1";
pub const FIGURE_ANALYSIS_SCHEMA_VERSION: &str = "FigureAnalysisDraft@v1";

pub struct FigureAnalysisUseCase {
    model: Arc<dyn MultimodalLlmPort>,
    classifier: FigureClassifyUseCase,
    schemas: FigureSchemaService,
}

impl FigureAnalysisUseCase {
    pub fn new(model: Arc<dyn MultimodalLlmPort>) -> Self {
        Self {
            model,
            classifier: FigureClassifyUseCase::bundled().expect("bundled figure type labels"),
            schemas: FigureSchemaService::bundled().expect("bundled figure schemas"),
        }
    }

    pub fn with_routing(
        model: Arc<dyn MultimodalLlmPort>,
        classifier: FigureClassifyUseCase,
        schemas: FigureSchemaService,
    ) -> Self {
        Self {
            model,
            classifier,
            schemas,
        }
    }

    pub fn prompt_sha256() -> String {
        sha256(Self::system_prompt().as_bytes())
    }

    /// Default Stage-2 schema JSON (diagram/flowchart) for stage cache fingerprints.
    /// Live `analyze` selects per-type DATA schema via Stage1.
    pub fn output_schema() -> serde_json::Value {
        FigureSchemaService::bundled()
            .expect("bundled figure schemas")
            .select_for_type(VisualAnalysisType::Flowchart)
            .expect("diagram schema")
            .json_schema
    }

    pub fn output_schema_sha256() -> String {
        sha256(
            &serde_json::to_vec(&Self::output_schema())
                .expect("static figure analysis schema serializes"),
        )
    }

    /// Stage1 classify + Stage2 DATA schema selection (no VLM).
    pub fn plan_schema(
        &self,
        context: &FigureSemanticContext,
    ) -> Result<(FigureTypeClassification, FigureSchemaSelection), FigureAnalysisUseCaseError> {
        let classification = self.classifier.classify(context)?;
        let selection = self.schemas.select_for_classification(&classification)?;
        Ok((classification, selection))
    }

    pub async fn analyze(
        &self,
        image: MultimodalImage,
        context: &FigureSemanticContext,
    ) -> Result<FigureAnalysis, FigureAnalysisUseCaseError> {
        context.validate()?;
        image.validate()?;
        if image.sha256 != context.asset_sha256 {
            return Err(FigureAnalysisUseCaseError::ImageContextMismatch {
                image: image.sha256,
                context: context.asset_sha256.clone(),
            });
        }
        let route = self.model.route();
        if !route.supports_images || !route.supports_structured_output {
            return Err(FigureAnalysisUseCaseError::RouteCapability(format!(
                "route {} lacks image or structured-output capability",
                route.route_id
            )));
        }
        let (classification, selection) = self.plan_schema(context)?;
        let selected_schema_sha = selection.schema_sha256_hex()?;
        let context_bytes = serde_json::to_vec(context)?;
        let context_sha256 = sha256(&context_bytes);
        let user_context = format!(
            "Analyze the attached scientific visual using only the grounded context below. Stage-1 type hint: {} (schema_id={}). Every citation quote MUST be a verbatim substring copied from the supplied section or reference excerpt, at most 160 characters, with its existing section_id and path_id. Never reconstruct or extend a quote from prior knowledge.\n\n{}",
            classification.visual_type.as_wire(),
            selection.schema_id,
            serde_json::to_string_pretty(context)?
        );
        let response = self
            .model
            .complete_structured(MultimodalLlmRequest {
                system_prompt: Self::system_prompt().into(),
                user_context,
                image,
                output_schema: selection.json_schema.clone(),
                tool_name: "record_figure_analysis".into(),
                max_tokens: 4_096,
                temperature: 0.0,
            })
            .await?;
        let prompt_lineage = response.prompt_lineage.as_ref().ok_or_else(|| {
            FigureAnalysisUseCaseError::RouteCapability(
                "figure analysis requires PromptExecution lineage".into(),
            )
        })?;
        prompt_lineage
            .validate()
            .map_err(FigureAnalysisUseCaseError::RouteCapability)?;
        if prompt_lineage.task_id != "figure_analysis" {
            return Err(FigureAnalysisUseCaseError::RouteCapability(
                "figure prompt lineage task mismatch".into(),
            ));
        }
        let structured_output = response.structured_output;
        let draft = map_structured_to_draft(&structured_output, &classification)?;
        validate_citations(&draft.citations, context)?;
        let route = response.route;
        let candidate_id = format!(
            "figure-analysis:{}:{}",
            context.artifact_id,
            &sha256(
                format!(
                    "{}|{}|{}|{}|{}|{}|{}",
                    context.asset_sha256,
                    context_sha256,
                    route.route_id,
                    route.model,
                    response.resolved_model,
                    selection.schema_id,
                    selected_schema_sha
                )
                .as_bytes()
            )[..16]
        );
        let analysis = FigureAnalysis {
            candidate_id,
            artifact_id: context.artifact_id.clone(),
            visual_type: draft.visual_type,
            description: draft.description,
            components: draft.components,
            relationships: draft.relationships,
            findings: draft.findings,
            citations: draft.citations,
            confidence: draft.confidence,
            provenance: FigureAnalysisProvenance {
                image_sha256: context.asset_sha256.clone(),
                context_sha256,
                route_id: route.route_id,
                protocol: protocol_name(route.protocol).into(),
                requested_model: route.model,
                resolved_model: response.resolved_model,
                prompt_sha256: Self::prompt_sha256(),
                // Prefer DATA schema digest; fall back to lineage if empty.
                output_schema_sha256: if selected_schema_sha.len() == 64 {
                    selected_schema_sha
                } else {
                    prompt_lineage.output_schema_sha256.as_str().into()
                },
                schema_id: selection.schema_id.clone(),
                stage1_visual_type: classification.visual_type.as_wire().into(),
                prompt_task_sha256: prompt_lineage.task_spec_sha256.as_str().into(),
                route_policy_sha256: prompt_lineage.route_policy_sha256.as_str().into(),
                model_binding_sha256: prompt_lineage.model_binding_sha256.as_str().into(),
                input_sha256: prompt_lineage.input_sha256.as_str().into(),
                output_sha256: prompt_lineage.output_sha256.as_str().into(),
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                latency_ms: response.latency_ms,
            },
            authoritative: false,
            import_eligible: false,
        };
        analysis.validate()?;
        Ok(analysis)
    }

    fn system_prompt() -> &'static str {
        "You analyze scientific figures as non-authoritative candidate evidence. Describe only visible structure and claims supported by the supplied context. Do not invent components. Every analysis must cite short verbatim substrings from the supplied context using existing section_id and path_id. Relationship source_component and target_component values must exactly copy declared component names; omit uncertain relationships. Follow the provided JSON schema for this visual type."
    }
}

#[derive(Debug, Deserialize)]
struct FigureAnalysisDraft {
    visual_type: VisualAnalysisType,
    description: String,
    components: Vec<FigureAnalysisComponent>,
    relationships: Vec<FigureAnalysisRelationship>,
    findings: Vec<String>,
    citations: Vec<FigureAnalysisCitation>,
    confidence: f32,
}

/// Map type-specific VLM JSON into the shared FigureAnalysis candidate shape.
fn map_structured_to_draft(
    value: &serde_json::Value,
    classification: &FigureTypeClassification,
) -> Result<FigureAnalysisDraft, FigureAnalysisUseCaseError> {
    let object = value
        .as_object()
        .ok_or_else(|| FigureAnalysisUseCaseError::StructuredOutput {
            message: "expected object".into(),
            top_level_keys: vec![],
        })?;

    let visual_type = object
        .get("visual_type")
        .and_then(|v| v.as_str())
        .and_then(VisualAnalysisType::from_wire)
        .unwrap_or_else(|| classification.visual_type.clone());

    let description = object
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut findings = object
        .get("findings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Chart honesty: surface typed draft fields into findings without inventing graph truth.
    enrich_findings_from_typed_draft(object, &visual_type, &mut findings);

    let citations: Vec<FigureAnalysisCitation> = object
        .get("citations")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| FigureAnalysisUseCaseError::StructuredOutput {
            message: format!("citations: {error}"),
            top_level_keys: object.keys().cloned().collect(),
        })?
        .unwrap_or_default();

    let confidence = object
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|f| f as f32)
        .unwrap_or(classification.confidence);

    let mut components: Vec<FigureAnalysisComponent> = object
        .get("components")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| FigureAnalysisUseCaseError::StructuredOutput {
            message: format!("components: {error}"),
            top_level_keys: object.keys().cloned().collect(),
        })?
        .unwrap_or_default();

    let relationships: Vec<FigureAnalysisRelationship> = object
        .get("relationships")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| FigureAnalysisUseCaseError::StructuredOutput {
            message: format!("relationships: {error}"),
            top_level_keys: object.keys().cloned().collect(),
        })?
        .unwrap_or_default();

    // Chart / table / photo drafts may omit diagram components — synthesize one
    // so domain FigureAnalysis (still diagram-shaped) validates.
    if components.is_empty() {
        components.push(synthesize_component(value, &visual_type, &description));
    }

    if description.trim().is_empty() {
        return Err(FigureAnalysisUseCaseError::StructuredOutput {
            message: "description missing".into(),
            top_level_keys: object.keys().cloned().collect(),
        });
    }

    Ok(FigureAnalysisDraft {
        visual_type,
        description,
        components,
        relationships,
        findings,
        citations,
        confidence,
    })
}

fn enrich_findings_from_typed_draft(
    object: &serde_json::Map<String, serde_json::Value>,
    visual_type: &VisualAnalysisType,
    findings: &mut Vec<String>,
) {
    match visual_type {
        VisualAnalysisType::Plot => {
            if let Some(chart_type) = object.get("chart_type").and_then(|v| v.as_str()) {
                let tag = format!("chart_type={chart_type}");
                if !findings.iter().any(|f| f == &tag) {
                    findings.insert(0, tag);
                }
            }
            if let Some(points) = object.get("key_data_points").and_then(|v| v.as_array()) {
                let labels: Vec<&str> = points
                    .iter()
                    .filter_map(|p| p.get("label").and_then(|x| x.as_str()))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !labels.is_empty() {
                    let tag = format!("key_data_points={}", labels.join(","));
                    if !findings.iter().any(|f| f.starts_with("key_data_points=")) {
                        findings.insert(1.min(findings.len()), tag);
                    }
                }
            }
        }
        VisualAnalysisType::Table => {
            if let Some(headers) = object.get("headers").and_then(|v| v.as_array()) {
                let names: Vec<&str> = headers
                    .iter()
                    .filter_map(|h| h.as_str())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !names.is_empty() {
                    let tag = format!("table_headers={}", names.join(","));
                    if !findings.iter().any(|f| f.starts_with("table_headers=")) {
                        findings.insert(0, tag);
                    }
                }
            }
        }
        _ => {}
    }
}

fn synthesize_component(
    value: &serde_json::Value,
    visual_type: &VisualAnalysisType,
    description: &str,
) -> FigureAnalysisComponent {
    let name = match visual_type {
        VisualAnalysisType::Plot => value
            .get("chart_type")
            .and_then(|v| v.as_str())
            .map(|c| format!("chart:{c}"))
            .unwrap_or_else(|| "chart".into()),
        VisualAnalysisType::Table => "table".into(),
        VisualAnalysisType::Photograph | VisualAnalysisType::Screenshot => value
            .get("visible_objects")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("visual")
            .to_string(),
        _ => visual_type.as_wire().to_string(),
    };
    let evidence = description.chars().take(120).collect::<String>();
    FigureAnalysisComponent {
        name,
        role: format!("stage2:{}", visual_type.as_wire()),
        visual_evidence: if evidence.is_empty() {
            visual_type.as_wire().into()
        } else {
            evidence
        },
    }
}

fn validate_citations(
    citations: &[FigureAnalysisCitation],
    context: &FigureSemanticContext,
) -> Result<(), FigureAnalysisError> {
    let mut sources = Vec::new();
    sources.push((
        context.containing_section.section_id.as_str(),
        context.containing_section.path_id.as_str(),
        context.containing_section.text.as_str(),
    ));
    sources.extend(context.neighboring_sections.iter().map(|section| {
        (
            section.section_id.as_str(),
            section.path_id.as_str(),
            section.text.as_str(),
        )
    }));
    sources.extend(context.reference_excerpts.iter().map(|reference| {
        (
            reference.section_id.as_str(),
            reference.path_id.as_str(),
            reference.excerpt.as_str(),
        )
    }));
    for citation in citations {
        let grounded = sources.iter().any(|(section_id, path_id, text)| {
            *section_id == citation.section_id
                && *path_id == citation.path_id
                && contains_case_insensitive(text, &citation.quote)
        });
        if !grounded {
            return Err(FigureAnalysisError::UngroundedCitation(format!(
                "{}:{} {:?}",
                citation.section_id, citation.path_id, citation.quote
            )));
        }
    }
    Ok(())
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    value.to_lowercase().contains(&needle.to_lowercase())
}

fn protocol_name(protocol: ModelWireProtocol) -> &'static str {
    match protocol {
        ModelWireProtocol::AnthropicMessages => "anthropic_messages",
        ModelWireProtocol::OpenAiChatCompletions => "openai_chat_completions",
        ModelWireProtocol::OpenAiResponses => "openai_responses",
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Debug, Error)]
pub enum FigureAnalysisUseCaseError {
    #[error(transparent)]
    Context(#[from] da_domain::FigureContextError),
    #[error(transparent)]
    Domain(#[from] FigureAnalysisError),
    #[error(transparent)]
    Classify(#[from] FigureClassifyError),
    #[error(transparent)]
    Schema(#[from] FigureSchemaError),
    #[error(transparent)]
    Model(#[from] MultimodalLlmError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid structured figure analysis: {message}; top-level keys={top_level_keys:?}")]
    StructuredOutput {
        message: String,
        top_level_keys: Vec<String>,
    },
    #[error("image/context checksum mismatch: image {image}, context {context}")]
    ImageContextMismatch { image: String, context: String },
    #[error("model route capability mismatch: {0}")]
    RouteCapability(String),
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
        ImageMediaType, ModelRouteDescriptor, MultimodalLlmResponse, MultimodalLlmResult,
    };

    struct FakeModel {
        output: serde_json::Value,
        route: ModelRouteDescriptor,
        last_schema_id_hint: std::sync::Mutex<Option<String>>,
    }

    impl FakeModel {
        fn new(output: serde_json::Value, route: ModelRouteDescriptor) -> Self {
            Self {
                output,
                route,
                last_schema_id_hint: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl MultimodalLlmPort for FakeModel {
        fn route(&self) -> ModelRouteDescriptor {
            self.route.clone()
        }

        async fn complete_structured(
            &self,
            request: MultimodalLlmRequest,
        ) -> MultimodalLlmResult<MultimodalLlmResponse> {
            // Capture whether chart-oriented fields were requested (DATA schema).
            let hint = if request
                .output_schema
                .pointer("/properties/key_data_points")
                .is_some()
            {
                Some("chart_extraction".into())
            } else if request
                .output_schema
                .pointer("/properties/components")
                .is_some()
            {
                Some("diagram_analysis".into())
            } else {
                Some("other".into())
            };
            *self.last_schema_id_hint.lock().unwrap() = hint;
            Ok(MultimodalLlmResponse {
                structured_output: self.output.clone(),
                route: self.route.clone(),
                resolved_model: "resolved-vision".into(),
                input_tokens: Some(100),
                output_tokens: Some(40),
                latency_ms: 12,
                prompt_lineage: Some(PromptExecutionLineage {
                    run_id: "figure-test".into(),
                    task_id: "figure_analysis".into(),
                    task_spec_sha256: ContentDigest::of_text("task"),
                    rendered_prompt_sha256: ContentDigest::of_text("prompt"),
                    output_schema_sha256: ContentDigest::of_text("schema"),
                    route_policy_sha256: ContentDigest::of_text("route"),
                    endpoint_sha256: ContentDigest::of_text("endpoint"),
                    model_binding_sha256: ContentDigest::of_text("binding"),
                    provider: "fixed".into(),
                    model: "resolved-vision".into(),
                    input_sha256: ContentDigest::of_text("input"),
                    output_sha256: ContentDigest::of_text("output"),
                    input_tokens: 100,
                    output_tokens: 40,
                    latency_ms: 12,
                    temperature_milli: 0,
                    max_output_tokens: 4096,
                    estimated_cost_microunits: None,
                    authoritative: false,
                    import_eligible: false,
                }),
            })
        }
    }

    fn route() -> ModelRouteDescriptor {
        ModelRouteDescriptor {
            route_id: "test-route".into(),
            protocol: ModelWireProtocol::AnthropicMessages,
            model: "requested-vision".into(),
            endpoint_fingerprint: "fixture/v1/messages".into(),
            supports_images: true,
            supports_structured_output: true,
            may_fallback: true,
        }
    }

    fn context() -> FigureSemanticContext {
        FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: sha256(b"png"),
            source_pdf_sha256: "a".repeat(64),
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
                title: "GEPA System".into(),
                text: "GEPA mutates prompts and evaluates candidates.".into(),
                truncated: false,
            },
            neighboring_sections: vec![],
            reference_excerpts: vec![FigureReferenceExcerpt {
                section_id: "sec-3".into(),
                path_id: "3".into(),
                title: "GEPA System".into(),
                matched_form: "figure 3".into(),
                excerpt: "GEPA mutates prompts and evaluates candidates.".into(),
                truncated: false,
            }],
            article_summary: None,
            diagnostics: vec![],
            total_context_chars: 100,
            truncated: false,
        }
    }

    fn output(quote: &str) -> serde_json::Value {
        serde_json::json!({
            "visual_type": "flowchart",
            "description": "GEPA optimization flowchart",
            "components": [{"name":"Reflective mutation","role":"Propose candidates","visual_evidence":"mutation box"}],
            "relationships": [],
            "findings": ["Candidates are evaluated"],
            "citations": [{"section_id":"sec-3","path_id":"3","quote":quote}],
            "confidence": 0.9
        })
    }

    fn image() -> MultimodalImage {
        MultimodalImage {
            media_type: ImageMediaType::Png,
            bytes: b"png".to_vec(),
            sha256: sha256(b"png"),
        }
    }

    #[tokio::test]
    async fn grounded_candidate_keeps_route_and_authority_provenance() {
        let fake = Arc::new(FakeModel::new(output("mutates prompts"), route()));
        let analysis = FigureAnalysisUseCase::new(fake.clone())
            .analyze(image(), &context())
            .await
            .unwrap();
        assert_eq!(analysis.visual_type, VisualAnalysisType::Flowchart);
        assert_eq!(analysis.provenance.route_id, "test-route");
        assert_eq!(analysis.provenance.resolved_model, "resolved-vision");
        assert!(!analysis.authoritative && !analysis.import_eligible);
        assert_eq!(
            fake.last_schema_id_hint.lock().unwrap().as_deref(),
            Some("diagram_analysis")
        );
        // Provenance carries DATA schema digest (64 hex), not the fake lineage "schema" text.
        assert_eq!(analysis.provenance.output_schema_sha256.len(), 64);
        assert_eq!(analysis.provenance.schema_id, "diagram_analysis");
        assert_eq!(analysis.provenance.stage1_visual_type, "flowchart");
    }

    #[tokio::test]
    async fn stage1_plot_caption_selects_chart_extraction_schema() {
        let mut ctx = context();
        ctx.caption =
            "Figure 2: Performance comparison scatter plot of accuracy vs rollouts".into();
        let chart_output = serde_json::json!({
            "visual_type": "plot",
            "chart_type": "scatter",
            "description": "Accuracy vs rollouts scatter",
            "x_axis": {"label": "rollouts"},
            "y_axis": {"label": "accuracy"},
            "key_data_points": [{"label": "GEPA", "x": "100", "y": "0.9"}],
            "findings": ["GEPA dominates"],
            "citations": [{"section_id":"sec-3","path_id":"3","quote":"mutates prompts"}],
            "confidence": 0.8
        });
        let fake = Arc::new(FakeModel::new(chart_output, route()));
        let analysis = FigureAnalysisUseCase::new(fake.clone())
            .analyze(image(), &ctx)
            .await
            .unwrap();
        assert_eq!(analysis.visual_type, VisualAnalysisType::Plot);
        assert!(!analysis.components.is_empty());
        assert!(
            analysis.findings.iter().any(|f| f == "chart_type=scatter"),
            "chart_type must surface in findings: {:?}",
            analysis.findings
        );
        assert!(
            analysis
                .findings
                .iter()
                .any(|f| f.starts_with("key_data_points=") && f.contains("GEPA")),
            "key_data_points labels must surface: {:?}",
            analysis.findings
        );
        assert_eq!(analysis.provenance.schema_id, "chart_extraction");
        assert_eq!(analysis.provenance.stage1_visual_type, "plot");
        assert_eq!(
            fake.last_schema_id_hint.lock().unwrap().as_deref(),
            Some("chart_extraction")
        );
        let (_, selection) = FigureAnalysisUseCase::new(fake).plan_schema(&ctx).unwrap();
        assert_eq!(selection.schema_id, "chart_extraction");
    }

    #[tokio::test]
    async fn plan_schema_flowchart_caption_selects_diagram_analysis() {
        let uc = FigureAnalysisUseCase::new(Arc::new(FakeModel::new(
            output("mutates prompts"),
            route(),
        )));
        let (class, selection) = uc.plan_schema(&context()).unwrap();
        assert_eq!(class.visual_type, VisualAnalysisType::Flowchart);
        assert_eq!(selection.schema_id, "diagram_analysis");
    }

    #[tokio::test]
    async fn requested_model_aliases_get_distinct_candidate_ids() {
        let first = FigureAnalysisUseCase::new(Arc::new(FakeModel::new(
            output("mutates prompts"),
            route(),
        )))
        .analyze(image(), &context())
        .await
        .unwrap();
        let mut aliased_route = route();
        aliased_route.model = "another-requested-alias".into();
        let second = FigureAnalysisUseCase::new(Arc::new(FakeModel::new(
            output("mutates prompts"),
            aliased_route,
        )))
        .analyze(image(), &context())
        .await
        .unwrap();
        assert_eq!(
            first.provenance.resolved_model,
            second.provenance.resolved_model
        );
        assert_ne!(first.candidate_id, second.candidate_id);
    }

    #[tokio::test]
    async fn ungrounded_quote_and_image_mismatch_are_rejected() {
        let use_case = FigureAnalysisUseCase::new(Arc::new(FakeModel::new(
            output("invented unsupported quote"),
            route(),
        )));
        assert!(matches!(
            use_case.analyze(image(), &context()).await,
            Err(FigureAnalysisUseCaseError::Domain(
                FigureAnalysisError::UngroundedCitation(_)
            ))
        ));
        let mut wrong = image();
        wrong.bytes = b"other".to_vec();
        wrong.sha256 = sha256(b"other");
        assert!(matches!(
            use_case.analyze(wrong, &context()).await,
            Err(FigureAnalysisUseCaseError::ImageContextMismatch { .. })
        ));
    }
}
