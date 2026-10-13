//! Deterministic ruVLLM reflection critic: three perspectives folded by
//! `UnifiedCritique::combine`, emitting parse-learning-ablation.v1 rows.

use crate::subject::StructuralProfile;
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
/// Stable code identity of this producer. v2: the score is now the vendor
/// unified critique calibrated by structural evidence (audit Н-1).
pub const CRITIC_CODE: &str = "critic-producer.ruvllm-reflection-unified.v2";
pub const CRITIC_METHOD: &str = "ruvllm_reflection_unified_structural_calibration";
pub const CRITIC_AUTHORITY: &str = "ruvllm-reflection-deterministic";
pub const CRITIC_LABEL: &str = "ruvllm-reflection-unified";
const CRITIC_TASK: &str = "parse-learning critic observation";
const PERSPECTIVE_WEIGHTS: [f32; 3] = [1.2, 1.0, 0.8];
/// Weight of the vendor unified critique half of the calibrated score.
const VENDOR_WEIGHT: f64 = 0.5;
/// Weight of the structural-evidence half of the calibrated score.
const STRUCTURAL_WEIGHT: f64 = 0.5;
/// Calibrated score at or above this threshold passes (metric_two = 1.0).
/// Aligned with the degraded-profile boundary: below 0.8 is degraded.
const PASS_THRESHOLD: f64 = 0.8;

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
    #[error("io: {0}")]
    Io(String),
    #[error("unknown model id (not in registry): {0}")]
    UnknownModel(String),
    #[error("model file missing: {0}")]
    ModelMissing(String),
    #[error("backend: {0}")]
    Backend(String),
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

    /// Calibrated observation for one redacted case.
    ///
    /// Root cause of the const-1.0 defect (audit Н-1): every vendored
    /// perspective is an absence-of-badness checker written for Rust code -
    /// error markers, TODO markers, unbalanced brackets, indentation. A
    /// redacted structural profile (`sections: 0; references: 0; ...`) has
    /// none of those markers, so each perspective returned penalty 0 and
    /// [`UnifiedCritique::combine`] collapsed every subject - healthy or
    /// degraded - onto 1.0. The calibration here adds the missing
    /// presence-of-goodness half: the subject is parsed back into its
    /// structural profile and scored for substantive parse markers
    /// ([`StructuralProfile::evidence`]), then folded with the vendor score.
    ///
    /// A subject that does not parse as a structural profile is reported as
    /// [`CriticStatus::Malformed`] with zeroed metrics (fail safe: the
    /// aggregate in `da_application` then counts the row as a failure, it
    /// never inherits the vendor's 1.0).
    pub fn observe(&self, case: &CriticCase) -> Result<CriticObservation, CriticError> {
        if case.case_id.is_empty() {
            return Err(CriticError::EmptyCaseId);
        }
        let Some(profile) = StructuralProfile::parse(&case.subject) else {
            return Ok(CriticObservation {
                schema_version: CRITIC_SCHEMA_VERSION,
                case_id: case.case_id.clone(),
                corpus: case.corpus.clone(),
                order: case.order,
                producer: CRITIC_LABEL.into(),
                metric_one: 0.0,
                metric_two: 0.0,
                status: CriticStatus::Malformed,
            });
        };
        let unified = self.unified_critique(&case.subject);
        let vendor = f64::from(unified.combined_score);
        if !vendor.is_finite() {
            return Err(CriticError::NonFiniteMetric);
        }
        let evidence = profile.evidence();
        if !evidence.is_finite() {
            return Err(CriticError::NonFiniteMetric);
        }
        let metric_one = VENDOR_WEIGHT * vendor + STRUCTURAL_WEIGHT * evidence;
        let observation = CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: case.case_id.clone(),
            corpus: case.corpus.clone(),
            order: case.order,
            producer: CRITIC_LABEL.into(),
            metric_one,
            metric_two: f64::from(metric_one >= PASS_THRESHOLD),
            status: CriticStatus::Ok,
        };
        Ok(observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim `derive_subject` output of the frozen-corpus fixtures
    /// (crates/da-parse/tests/fixtures/preserved/*.tei.xml).
    const WELL_FORMED_HUB: &str = "sections: 74; references: 81; figures: 25; tables: 3; \
        formulas: 24; abstract_present: true; funding_present: false; \
        notes: 116; keywords: false; language_set: true";
    const WELL_FORMED_RECA: &str = "sections: 65; references: 79; figures: 58; tables: 3; \
        formulas: 12; abstract_present: true; funding_present: false; \
        notes: 103; keywords: false; language_set: true";
    const WELL_FORMED_RLM: &str = "sections: 61; references: 50; figures: 31; tables: 2; \
        formulas: 2; abstract_present: true; funding_present: false; \
        notes: 58; keywords: false; language_set: true";
    const WELL_FORMED: [&str; 4] = [
        WELL_FORMED_HUB,
        WELL_FORMED_RECA,
        WELL_FORMED_RLM,
        // no formulas at all: still a healthy fulltext parse
        "sections: 41; references: 45; figures: 20; tables: 2; formulas: 0; \
         abstract_present: true; funding_present: false; notes: 31; \
         keywords: false; language_set: true",
    ];

    /// Degraded subjects: empty TEI, header-only stub, truncated body,
    /// bibliography lost, header metadata lost.
    const DEGRADED_EMPTY: &str = "sections: 0; references: 0; figures: 0; tables: 0; \
        formulas: 0; abstract_present: false; funding_present: false; \
        notes: 0; keywords: false; language_set: false";
    const DEGRADED_HEADER_ONLY: &str = "sections: 0; references: 0; figures: 0; tables: 0; \
        formulas: 0; abstract_present: false; funding_present: false; \
        notes: 0; keywords: false; language_set: true";
    const DEGRADED_TRUNCATED: &str = "sections: 1; references: 0; figures: 0; tables: 0; \
        formulas: 0; abstract_present: false; funding_present: false; \
        notes: 0; keywords: false; language_set: false";
    const DEGRADED_NO_BIBLIOGRAPHY: &str = "sections: 74; references: 0; figures: 25; \
        tables: 3; formulas: 24; abstract_present: true; \
        funding_present: false; notes: 116; keywords: false; language_set: true";
    const DEGRADED_NO_HEADER: &str = "sections: 31; references: 76; figures: 24; tables: 11; \
        formulas: 14; abstract_present: false; funding_present: false; \
        notes: 8; keywords: false; language_set: false";
    const DEGRADED: [&str; 5] = [
        DEGRADED_EMPTY,
        DEGRADED_HEADER_ONLY,
        DEGRADED_TRUNCATED,
        DEGRADED_NO_BIBLIOGRAPHY,
        DEGRADED_NO_HEADER,
    ];

    fn observe(subject: &str) -> CriticObservation {
        RuvllmReflectionCritic::new()
            .observe(&CriticCase {
                case_id: "t-case".into(),
                corpus: "test-corpus".into(),
                order: 0,
                subject: subject.into(),
            })
            .expect("observation")
    }

    #[test]
    fn empty_and_degraded_profiles_score_below_0_8() {
        for subject in DEGRADED {
            let o = observe(subject);
            assert_eq!(o.status, CriticStatus::Ok);
            assert!(
                o.metric_one < 0.8,
                "degraded subject scored {metric}, expected < 0.8: {subject}",
                metric = o.metric_one
            );
            assert_eq!(o.metric_two, 0.0);
        }
        // the empty-string subject must also stay below the boundary
        // (it fast-fails all vendor perspectives, then has no evidence)
        let o = observe("");
        assert!(o.metric_one < 0.8, "empty subject scored {}", o.metric_one);
        assert_eq!(o.metric_two, 0.0);
    }

    #[test]
    fn well_formed_profiles_score_at_least_0_9() {
        for subject in WELL_FORMED {
            let o = observe(subject);
            assert_eq!(o.status, CriticStatus::Ok);
            assert!(
                o.metric_one >= 0.9,
                "well-formed subject scored {metric}, expected >= 0.9: {subject}",
                metric = o.metric_one
            );
            assert_eq!(o.metric_two, 1.0);
        }
    }

    #[test]
    fn discrimination_gap_between_well_formed_and_degraded_is_at_least_0_15() {
        let best_degraded = DEGRADED
            .iter()
            .map(|s| observe(s).metric_one)
            .fold(f64::NEG_INFINITY, f64::max);
        let worst_well = WELL_FORMED
            .iter()
            .map(|s| observe(s).metric_one)
            .fold(f64::INFINITY, f64::min);
        let gap = worst_well - best_degraded;
        assert!(
            gap >= 0.15,
            "discrimination gap {gap} < 0.15 (best degraded {best_degraded}, worst well-formed {worst_well})"
        );
    }

    #[test]
    fn vendor_score_alone_no_longer_saturates_degraded_profiles() {
        // The Н-1 regression guard: with vendor perspectives alone every
        // degraded profile scored exactly 1.0.
        for subject in DEGRADED {
            let critic = RuvllmReflectionCritic::new();
            let unified = critic.unified_critique(subject);
            let o = observe(subject);
            assert!(
                o.metric_one < f64::from(unified.combined_score),
                "calibrated score must sit strictly below the vendor score on a degraded subject"
            );
        }
    }

    #[test]
    fn repeated_observation_is_deterministic() {
        let case = CriticCase {
            case_id: "determinism".into(),
            corpus: "test-corpus".into(),
            order: 7,
            subject: WELL_FORMED_HUB.into(),
        };
        let first = RuvllmReflectionCritic::new()
            .observe(&case)
            .expect("observation");
        for _ in 0..3 {
            // fresh instance AND repeated call: identical bytes
            let again = RuvllmReflectionCritic::new()
                .observe(&case)
                .expect("observation");
            assert_eq!(
                serde_json::to_string(&first).unwrap(),
                serde_json::to_string(&again).unwrap()
            );
        }
        let same_bytes = serde_json::to_string(&first).unwrap();
        assert_eq!(same_bytes, serde_json::to_string(&first).unwrap());
    }

    #[test]
    fn unparseable_subject_is_malformed_with_zero_metrics() {
        let o = observe("not a structural profile");
        assert_eq!(o.status, CriticStatus::Malformed);
        assert_eq!(o.metric_one, 0.0);
        assert_eq!(o.metric_two, 0.0);
    }

    #[test]
    fn empty_case_id_is_rejected() {
        let err = RuvllmReflectionCritic::new()
            .observe(&CriticCase {
                case_id: String::new(),
                corpus: "c".into(),
                order: 0,
                subject: WELL_FORMED_HUB.into(),
            })
            .unwrap_err();
        assert!(matches!(err, CriticError::EmptyCaseId));
    }

    #[test]
    fn subject_grammar_roundtrip_survives_the_critic_boundary() {
        // the exact derive_subject grammar parses back into a profile
        let tei = "<TEI><teiHeader xml:lang=\"en\"><abstract/></teiHeader><text><body>\
                   <div/><div/><div/><figure/><note/></body><back><listBibl>\
                   <biblStruct/><biblStruct/><biblStruct/><biblStruct/><biblStruct/>\
                   </listBibl></back></text></TEI>";
        let subject = crate::subject::derive_subject(tei);
        let o = observe(&subject);
        assert_eq!(o.status, CriticStatus::Ok);
        assert!(
            o.metric_one >= 0.9,
            "roundtrip subject scored {}",
            o.metric_one
        );
    }
}
