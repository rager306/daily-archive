//! Learning-critic observation port (ADR-075 §2.7 learning surface, D471
//! `learning.ruvllm_ablation` horizon).
//!
//! A learning critic observes one frozen parse-learning case and returns a
//! redacted [`CriticObservation`]. The DTO mirrors the application evaluation
//! contract (`da_application::parse_learning_evaluation`) field for field, so
//! a CLI adapter can feed the offline same-n ablation without a bespoke
//! mapping layer.
//!
//! The port carries identities and bounded numeric receipts only — never raw
//! PDF/TEI bytes, prompts or completions. Subjects crossing this boundary must
//! pass [`crate::parse_learning::reject_raw_content`]; [`CriticCase::validate`]
//! enforces it.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::parse_learning::{Sha256Digest, reject_raw_content};

/// Schema version of a critic observation row. Kept in lockstep with the
/// application contract (`parse-learning-ablation.v1` critic rows).
pub const CRITIC_SCHEMA_VERSION: u16 = 1;

/// Machine-checkable identity of a critic producer.
///
/// The human-readable label on [`CriticObservation`] is diagnostic only. It
/// must never be used as evidence that two producers are independent; the
/// `code_digest` is what the application validator compares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerProvenance {
    pub code_digest: Sha256Digest,
    pub input_digest: Sha256Digest,
    pub method: String,
    pub authority: String,
}

/// One redacted critic row over a frozen case.
///
/// Field-for-field mirror of the application's `CriticObservation`: the same
/// serde shape (`deny_unknown_fields`, snake_case status) so serialized rows
/// are accepted by the `parse-learning-ablation.v1` aggregator unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticObservation {
    pub schema_version: u16,
    pub case_id: String,
    pub corpus: Sha256Digest,
    pub order: u32,
    /// Display label only; not used for independence decisions.
    pub producer: String,
    pub metric_one: f64,
    pub metric_two: f64,
    pub status: CriticStatus,
}

/// Typed critic outcome. A failed row stays in the evaluation denominator and
/// scores `0.0`; it is never silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticStatus {
    Ok,
    Timeout,
    Oom,
    Malformed,
    Missing,
}

/// A frozen case offered to a critic.
///
/// The subject is a bounded, redacted representation of the artifact under
/// critique (for example a structured summary of parse output). It is not raw
/// source content; [`CriticCase::validate`] applies the redaction barrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticCase {
    pub case_id: String,
    pub corpus: Sha256Digest,
    pub order: u32,
    pub subject: String,
}

impl CriticCase {
    /// Non-empty identity plus the redaction barrier over the subject.
    pub fn validate(&self) -> Result<(), LearningCriticError> {
        if self.case_id.is_empty() {
            return Err(LearningCriticError::EmptyCase);
        }
        reject_raw_content(&self.subject).map_err(|_| LearningCriticError::RawContent)
    }
}

impl CriticObservation {
    /// Row-level validation mirrored from the application contract: schema
    /// version, non-empty identity and finite metrics on a passing row.
    pub fn validate(&self) -> Result<(), LearningCriticError> {
        if self.schema_version != CRITIC_SCHEMA_VERSION || self.case_id.is_empty() {
            return Err(LearningCriticError::MalformedRow);
        }
        if self.status == CriticStatus::Ok
            && (!self.metric_one.is_finite() || !self.metric_two.is_finite())
        {
            return Err(LearningCriticError::NonFiniteMetric);
        }
        Ok(())
    }
}

/// Failures a critic producer can report before a row exists.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LearningCriticError {
    #[error("critic case identity is empty")]
    EmptyCase,
    #[error("critic subject violates the redaction barrier")]
    RawContent,
    #[error("critic row is malformed")]
    MalformedRow,
    #[error("critic metric is non-finite")]
    NonFiniteMetric,
}

/// Port: a provider of critic observations for the offline learning
/// evaluation. ruVLLM is one adapter; the application never knows which.
pub trait LearningCriticPort: Send + Sync {
    /// Machine identity of this producer bound to the corpus it observes.
    /// `input_digest` must equal the corpus digest of the evaluated cases.
    fn provenance(&self, corpus: &Sha256Digest) -> ProducerProvenance;

    /// Observe one frozen case. Returns a redacted row or a typed failure;
    /// no raw content crosses in either direction.
    fn observe(&self, case: &CriticCase) -> Result<CriticObservation, LearningCriticError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> Sha256Digest {
        Sha256Digest::new(format!("{seed:064x}")).expect("64 hex chars form a valid digest")
    }

    fn case(subject: &str) -> CriticCase {
        CriticCase {
            case_id: "case-000".into(),
            corpus: digest(1),
            order: 0,
            subject: subject.into(),
        }
    }

    #[test]
    fn case_rejects_empty_identity() {
        let mut c = case("sections: 7; references: 41");
        c.case_id = String::new();
        assert_eq!(c.validate(), Err(LearningCriticError::EmptyCase));
    }

    #[test]
    fn case_rejects_raw_content_subjects() {
        for raw in ["<teiCorpus>", "pdf_bytes: [...]", "raw_tei here"] {
            assert_eq!(
                case(raw).validate(),
                Err(LearningCriticError::RawContent),
                "subject must be refused: {raw}"
            );
        }
    }

    #[test]
    fn case_accepts_bounded_redacted_subject() {
        assert_eq!(
            case("sections: 7; references: 41; figures: 3").validate(),
            Ok(())
        );
    }

    #[test]
    fn observation_validation_mirrors_application_rules() {
        let ok = CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: "case-000".into(),
            corpus: digest(1),
            order: 0,
            producer: "critic-under-test".into(),
            metric_one: 0.75,
            metric_two: 1.0,
            status: CriticStatus::Ok,
        };
        assert_eq!(ok.validate(), Ok(()));

        let mut wrong_version = ok.clone();
        wrong_version.schema_version = CRITIC_SCHEMA_VERSION + 1;
        assert_eq!(
            wrong_version.validate(),
            Err(LearningCriticError::MalformedRow)
        );

        let mut non_finite = ok;
        non_finite.metric_one = f64::NAN;
        assert_eq!(
            non_finite.validate(),
            Err(LearningCriticError::NonFiniteMetric)
        );
    }

    #[test]
    fn observation_json_matches_application_field_shape() {
        let row = CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: "case-000".into(),
            corpus: digest(1),
            order: 0,
            producer: "critic-under-test".into(),
            metric_one: 0.75,
            metric_two: 1.0,
            status: CriticStatus::Ok,
        };
        let json: serde_json::Value = serde_json::to_value(&row).expect("serializable");
        // serde_json::Value sorts object keys alphabetically by default; the
        // contract is the FIELD SET matching the application row, not order.
        let mut keys: Vec<&str> = json
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "case_id",
                "corpus",
                "metric_one",
                "metric_two",
                "order",
                "producer",
                "schema_version",
                "status"
            ]
        );
        assert_eq!(json["status"], serde_json::json!("ok"));
    }
}
