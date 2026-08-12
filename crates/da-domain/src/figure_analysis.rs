//! Non-authoritative visual analysis candidate (M371 S03, ADR-053).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAnalysisType {
    Flowchart,
    Plot,
    Diagram,
    Table,
    Screenshot,
    Photograph,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisComponent {
    pub name: String,
    pub role: String,
    pub visual_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisRelationship {
    pub source_component: String,
    pub relation: String,
    pub target_component: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisCitation {
    pub section_id: String,
    pub path_id: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisProvenance {
    pub image_sha256: String,
    pub context_sha256: String,
    pub route_id: String,
    pub protocol: String,
    pub requested_model: String,
    pub resolved_model: String,
    pub prompt_sha256: String,
    pub output_schema_sha256: String,
    pub prompt_task_sha256: String,
    pub route_policy_sha256: String,
    pub model_binding_sha256: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureAnalysis {
    pub candidate_id: String,
    pub artifact_id: String,
    pub visual_type: VisualAnalysisType,
    pub description: String,
    pub components: Vec<FigureAnalysisComponent>,
    pub relationships: Vec<FigureAnalysisRelationship>,
    pub findings: Vec<String>,
    pub citations: Vec<FigureAnalysisCitation>,
    pub confidence: f32,
    pub provenance: FigureAnalysisProvenance,
    pub authoritative: bool,
    pub import_eligible: bool,
}

impl FigureAnalysis {
    pub fn validate(&self) -> Result<(), FigureAnalysisError> {
        for (field, value) in [
            ("candidate_id", self.candidate_id.as_str()),
            ("artifact_id", self.artifact_id.as_str()),
            ("description", self.description.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(FigureAnalysisError::Missing(field));
            }
        }
        if self.components.is_empty() {
            return Err(FigureAnalysisError::Missing("components"));
        }
        if self.components.iter().any(|component| {
            component.name.trim().is_empty()
                || component.role.trim().is_empty()
                || component.visual_evidence.trim().is_empty()
        }) {
            return Err(FigureAnalysisError::Invalid(
                "components require name, role and visual_evidence".into(),
            ));
        }
        let component_names: HashSet<_> = self
            .components
            .iter()
            .map(|component| component.name.as_str())
            .collect();
        if component_names.len() != self.components.len() {
            return Err(FigureAnalysisError::Invalid(
                "component names must be unique".into(),
            ));
        }
        if self.relationships.iter().any(|relationship| {
            relationship.relation.trim().is_empty()
                || !component_names.contains(relationship.source_component.as_str())
                || !component_names.contains(relationship.target_component.as_str())
        }) {
            return Err(FigureAnalysisError::Invalid(
                "relationship endpoints must reference declared components".into(),
            ));
        }
        if self
            .findings
            .iter()
            .any(|finding| finding.trim().is_empty())
        {
            return Err(FigureAnalysisError::Invalid(
                "findings cannot contain empty values".into(),
            ));
        }
        if self.citations.is_empty()
            || self.citations.iter().any(|citation| {
                citation.section_id.trim().is_empty()
                    || citation.path_id.trim().is_empty()
                    || citation.quote.trim().is_empty()
            })
        {
            return Err(FigureAnalysisError::Invalid(
                "at least one complete context citation is required".into(),
            ));
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(FigureAnalysisError::Invalid(
                "confidence must be finite and between 0 and 1".into(),
            ));
        }
        for (field, value) in [
            ("image_sha256", &self.provenance.image_sha256),
            ("context_sha256", &self.provenance.context_sha256),
            ("prompt_sha256", &self.provenance.prompt_sha256),
            (
                "output_schema_sha256",
                &self.provenance.output_schema_sha256,
            ),
            ("prompt_task_sha256", &self.provenance.prompt_task_sha256),
            ("route_policy_sha256", &self.provenance.route_policy_sha256),
            (
                "model_binding_sha256",
                &self.provenance.model_binding_sha256,
            ),
            ("input_sha256", &self.provenance.input_sha256),
            ("output_sha256", &self.provenance.output_sha256),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(FigureAnalysisError::Invalid(format!(
                    "{field} must be a SHA-256 digest"
                )));
            }
        }
        if self.authoritative || self.import_eligible {
            return Err(FigureAnalysisError::AuthorityViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FigureAnalysisError {
    #[error("missing figure analysis field: {0}")]
    Missing(&'static str),
    #[error("invalid figure analysis: {0}")]
    Invalid(String),
    #[error("figure analysis candidate cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("ungrounded context citation: {0}")]
    UngroundedCitation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> FigureAnalysis {
        FigureAnalysis {
            candidate_id: "figure-analysis:2507.19457:Figure3:v1".into(),
            artifact_id: "Figure3".into(),
            visual_type: VisualAnalysisType::Flowchart,
            description: "GEPA optimization flowchart".into(),
            components: vec![FigureAnalysisComponent {
                name: "Reflective mutation".into(),
                role: "Proposes prompt variants".into(),
                visual_evidence: "mutation arrow".into(),
            }],
            relationships: vec![],
            findings: vec!["Candidate evaluation feeds Pareto selection".into()],
            citations: vec![FigureAnalysisCitation {
                section_id: "sec-3".into(),
                path_id: "3".into(),
                quote: "GEPA mutates prompts".into(),
            }],
            confidence: 0.9,
            provenance: FigureAnalysisProvenance {
                image_sha256: "a".repeat(64),
                context_sha256: "b".repeat(64),
                route_id: "minimax-direct".into(),
                protocol: "anthropic_messages".into(),
                requested_model: "MiniMax-M3-512k".into(),
                resolved_model: "MiniMax-M3".into(),
                prompt_sha256: "c".repeat(64),
                output_schema_sha256: "d".repeat(64),
                prompt_task_sha256: "e".repeat(64),
                route_policy_sha256: "f".repeat(64),
                model_binding_sha256: "1".repeat(64),
                input_sha256: "2".repeat(64),
                output_sha256: "3".repeat(64),
                input_tokens: Some(100),
                output_tokens: Some(30),
                latency_ms: 500,
            },
            authoritative: false,
            import_eligible: false,
        }
    }

    #[test]
    fn candidate_roundtrips_and_stays_non_authoritative() {
        let value = candidate();
        value.validate().unwrap();
        let decoded: FigureAnalysis =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn rejects_unknown_relationship_endpoints() {
        let mut value = candidate();
        value.relationships.push(FigureAnalysisRelationship {
            source_component: "Reflective mutation".into(),
            relation: "feeds".into(),
            target_component: "Invented component".into(),
        });
        assert!(matches!(
            value.validate(),
            Err(FigureAnalysisError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_authority_promotion_and_invalid_confidence() {
        let mut value = candidate();
        value.authoritative = true;
        assert_eq!(
            value.validate(),
            Err(FigureAnalysisError::AuthorityViolation)
        );
        value.authoritative = false;
        value.confidence = f32::NAN;
        assert!(matches!(
            value.validate(),
            Err(FigureAnalysisError::Invalid(_))
        ));
    }
}
