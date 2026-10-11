//! Deterministic ruVLLM reflection critic: three perspectives folded by
//! `UnifiedCritique::combine`, emitting parse-learning-ablation.v1 rows.

use ruvllm::claude_flow::AgentType;
use ruvllm::reflection::{
    CompletenessChecker, ConsistencyChecker, CorrectnessChecker, ExecutionContext, Perspective,
    UnifiedCritique,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Schema version of a critic observation row (parse-learning-ablation.v1).
pub const CRITIC_SCHEMA_VERSION: u16 = 1;
/// Stable code identity of this producer.
pub const CRITIC_CODE: &str = "critic-producer.ruvllm-reflection-unified.v1";
pub const CRITIC_METHOD: &str = "ruvllm_reflection_unified_critique";
pub const CRITIC_AUTHORITY: &str = "ruvllm-reflection-deterministic";
pub const CRITIC_LABEL: &str = "ruvllm-reflection-unified";
const CRITIC_TASK: &str = "parse-learning critic observation";
const PERSPECTIVE_WEIGHTS: [f32; 3] = [1.2, 1.0, 0.8];

/// A redacted frozen case submitted for critique.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticCase {
    pub case_id: String,
    /// Hex sha256 of the corpus identity.
    pub corpus: String,
    pub order: u32,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CriticStatus {
    Ok,
    Timeout,
    Oom,
    Malformed,
    Missing,
}

/// One parse-learning-ablation.v1 critic row (field-compatible with
/// `da_application::parse_learning_evaluation::CriticObservation`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticObservation {
    pub schema_version: u16,
    pub case_id: String,
    pub corpus: String,
    pub order: u32,
    pub producer: String,
    pub metric_one: f64,
    pub metric_two: f64,
    pub status: CriticStatus,
}

/// Producer provenance block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerProvenance {
    pub code_digest: String,
    pub input_digest: String,
    pub method: String,
    pub authority: String,
}

#[derive(Debug, Error)]
pub enum CriticError {
    #[error("non-finite metric from ruvllm reflection")]
    NonFiniteMetric,
    #[error("empty case_id")]
    EmptyCaseId,
}

pub fn identity_digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// Deterministic CPU critic built from the ruvllm reflection perspectives.
pub struct RuvllmReflectionCritic {
    correctness: CorrectnessChecker,
    completeness: CompletenessChecker,
    consistency: ConsistencyChecker,
}

impl Default for RuvllmReflectionCritic {
    fn default() -> Self {
        Self::new()
    }
}

impl RuvllmReflectionCritic {
    pub fn new() -> Self {
        Self {
            correctness: CorrectnessChecker::new(),
            completeness: CompletenessChecker::new(),
            consistency: ConsistencyChecker::new(),
        }
    }

    fn unified_critique(&self, subject: &str) -> UnifiedCritique {
        let context = ExecutionContext::new(CRITIC_TASK, AgentType::Reviewer, subject);
        let critiques = vec![
            self.correctness.critique(subject, &context),
            self.completeness.critique(subject, &context),
            self.consistency.critique(subject, &context),
        ];
        UnifiedCritique::combine(critiques, &PERSPECTIVE_WEIGHTS)
    }

    pub fn provenance(&self, corpus: &str) -> ProducerProvenance {
        ProducerProvenance {
            code_digest: identity_digest(CRITIC_CODE),
            input_digest: corpus.to_string(),
            method: CRITIC_METHOD.into(),
            authority: CRITIC_AUTHORITY.into(),
        }
    }

    pub fn observe(&self, case: &CriticCase) -> Result<CriticObservation, CriticError> {
        if case.case_id.is_empty() {
            return Err(CriticError::EmptyCaseId);
        }
        let unified = self.unified_critique(&case.subject);
        let combined = f64::from(unified.combined_score);
        if !combined.is_finite() {
            return Err(CriticError::NonFiniteMetric);
        }
        let observation = CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: case.case_id.clone(),
            corpus: case.corpus.clone(),
            order: case.order,
            producer: CRITIC_LABEL.into(),
            metric_one: combined,
            metric_two: if unified.passed { 1.0 } else { 0.0 },
            status: CriticStatus::Ok,
        };
        Ok(observation)
    }
}
