//! Grounded figure-analysis use case over a provider-neutral multimodal port.

use da_domain::{
    FigureAnalysis, FigureAnalysisCitation, FigureAnalysisComponent, FigureAnalysisError,
    FigureAnalysisProvenance, FigureAnalysisRelationship, FigureSemanticContext,
    VisualAnalysisType,
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
}

impl FigureAnalysisUseCase {
    pub fn new(model: Arc<dyn MultimodalLlmPort>) -> Self {
        Self { model }
    }

    pub fn prompt_sha256() -> String {
        sha256(Self::system_prompt().as_bytes())
    }

    pub fn output_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "visual_type": {"type": "string", "enum": ["flowchart", "plot", "diagram", "table", "screenshot", "photograph", "other"]},
                "description": {"type": "string"},
                "components": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "name": {"type": "string"},
                            "role": {"type": "string"},
                            "visual_evidence": {"type": "string"}
                        },
                        "required": ["name", "role", "visual_evidence"]
                    },
                    "minItems": 1
                },
                "relationships": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "source_component": {"type": "string"},
                            "relation": {"type": "string"},
                            "target_component": {"type": "string"}
                        },
                        "required": ["source_component", "relation", "target_component"]
                    }
                },
                "findings": {"type": "array", "items": {"type": "string"}},
                "citations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "section_id": {"type": "string"},
                            "path_id": {"type": "string"},
                            "quote": {"type": "string", "maxLength": 160}
                        },
                        "required": ["section_id", "path_id", "quote"]
                    },
                    "minItems": 1
                },
                "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0}
            },
            "required": ["visual_type", "description", "components", "relationships", "findings", "citations", "confidence"],
            "x-output-guidance": {
                "visual_type": "Return exactly one enum string, never an array.",
                "citations": "Return at least one object with a verbatim quote copied from supplied context, max 160 characters.",
                "relationships": "source_component and target_component must exactly copy a declared components[].name string. Omit a relationship if either endpoint is uncertain."
            }
        })
    }

    pub fn output_schema_sha256() -> String {
        sha256(
            &serde_json::to_vec(&Self::output_schema())
                .expect("static figure analysis schema serializes"),
        )
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
        let context_bytes = serde_json::to_vec(context)?;
        let context_sha256 = sha256(&context_bytes);
        let user_context = format!(
            "Analyze the attached scientific visual using only the grounded context below. Every citation quote MUST be a verbatim substring copied from the supplied section or reference excerpt, at most 160 characters, with its existing section_id and path_id. Never reconstruct or extend a quote from prior knowledge.\n\n{}",
            serde_json::to_string_pretty(context)?
        );
        let response = self
            .model
            .complete_structured(MultimodalLlmRequest {
                system_prompt: Self::system_prompt().into(),
                user_context,
                image,
                output_schema: Self::output_schema(),
                tool_name: "record_figure_analysis".into(),
                max_tokens: 4_096,
                temperature: 0.0,
            })
            .await?;
        let structured_output = response.structured_output;
        let draft: FigureAnalysisDraft = serde_json::from_value(structured_output.clone())
            .map_err(|error| FigureAnalysisUseCaseError::StructuredOutput {
                message: error.to_string(),
                top_level_keys: structured_output
                    .as_object()
                    .map(|object| object.keys().cloned().collect())
                    .unwrap_or_default(),
            })?;
        validate_citations(&draft.citations, context)?;
        let route = response.route;
        let candidate_id = format!(
            "figure-analysis:{}:{}",
            context.artifact_id,
            &sha256(
                format!(
                    "{}|{}|{}|{}|{}|{}",
                    context.asset_sha256,
                    context_sha256,
                    route.route_id,
                    route.model,
                    response.resolved_model,
                    Self::output_schema_sha256()
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
                output_schema_sha256: Self::output_schema_sha256(),
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
        "You analyze scientific figures as non-authoritative candidate evidence. Describe only visible structure and claims supported by the supplied context. Do not invent components. Every analysis must cite short verbatim substrings from the supplied context using existing section_id and path_id. Relationship source_component and target_component values must exactly copy declared component names; omit uncertain relationships."
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
        VisualArtifactKind,
    };
    use da_ports::multimodal_llm::{
        ImageMediaType, ModelRouteDescriptor, MultimodalLlmResponse, MultimodalLlmResult,
    };

    struct FakeModel {
        output: serde_json::Value,
        route: ModelRouteDescriptor,
    }

    #[async_trait]
    impl MultimodalLlmPort for FakeModel {
        fn route(&self) -> ModelRouteDescriptor {
            self.route.clone()
        }

        async fn complete_structured(
            &self,
            _request: MultimodalLlmRequest,
        ) -> MultimodalLlmResult<MultimodalLlmResponse> {
            Ok(MultimodalLlmResponse {
                structured_output: self.output.clone(),
                route: self.route.clone(),
                resolved_model: "resolved-vision".into(),
                input_tokens: Some(100),
                output_tokens: Some(40),
                latency_ms: 12,
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
        let analysis = FigureAnalysisUseCase::new(Arc::new(FakeModel {
            output: output("mutates prompts"),
            route: route(),
        }))
        .analyze(image(), &context())
        .await
        .unwrap();
        assert_eq!(analysis.visual_type, VisualAnalysisType::Flowchart);
        assert_eq!(analysis.provenance.route_id, "test-route");
        assert_eq!(analysis.provenance.resolved_model, "resolved-vision");
        assert!(!analysis.authoritative && !analysis.import_eligible);
    }

    #[tokio::test]
    async fn requested_model_aliases_get_distinct_candidate_ids() {
        let first = FigureAnalysisUseCase::new(Arc::new(FakeModel {
            output: output("mutates prompts"),
            route: route(),
        }))
        .analyze(image(), &context())
        .await
        .unwrap();
        let mut aliased_route = route();
        aliased_route.model = "another-requested-alias".into();
        let second = FigureAnalysisUseCase::new(Arc::new(FakeModel {
            output: output("mutates prompts"),
            route: aliased_route,
        }))
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
        let use_case = FigureAnalysisUseCase::new(Arc::new(FakeModel {
            output: output("invented unsupported quote"),
            route: route(),
        }));
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
