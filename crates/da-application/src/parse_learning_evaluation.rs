//! Offline, provider-independent same-n ablation evaluation (M464/S17/T05).
//! The evaluator consumes redacted critic observations at the application
//! boundary. It never writes graph state, reloads a route, or promotes a
//! candidate. A CLI adapter may obtain these observations from ruVLLM.

use da_ports::{CandidateManifest, Sha256Digest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

pub const ABLATION_SCHEMA_VERSION: &str = "parse-learning-ablation.v1";
pub const CRITIC_SCHEMA_VERSION: u16 = 1;

/// Machine-checkable identity of an evaluation producer.
///
/// The human-readable label on [`CriticObservation`] is diagnostic only. It
/// must never be used as evidence that two producers are independent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerProvenance {
    pub code_digest: Sha256Digest,
    pub input_digest: Sha256Digest,
    pub method: String,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContaminationAssessment {
    Detected,
    NotDetected,
    NotAssessed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenEvalCase {
    pub case_id: String,
    pub corpus: Sha256Digest,
    pub order: u32,
    pub expected_digest: Sha256Digest,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticStatus {
    Ok,
    Timeout,
    Oom,
    Malformed,
    Missing,
}

impl CriticObservation {
    fn validate(&self) -> Result<(), EvaluationError> {
        if self.schema_version != CRITIC_SCHEMA_VERSION || self.case_id.is_empty() {
            return Err(EvaluationError::MalformedRow);
        }
        if self.status == CriticStatus::Ok
            && (!self.metric_one.is_finite() || !self.metric_two.is_finite())
        {
            return Err(EvaluationError::NonFiniteMetric);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AblationReport {
    pub schema_version: String,
    pub candidate_id: Sha256Digest,
    pub corpus: Sha256Digest,
    pub order_digest: Sha256Digest,
    pub baseline_n: u64,
    pub candidate_n: u64,
    pub baseline_metric_one: f64,
    pub candidate_metric_one: f64,
    pub baseline_metric_two: f64,
    pub candidate_metric_two: f64,
    /// Recomputed from the candidate's explicit train/validation measurements.
    pub validation_gap: f64,
    #[serde(default)]
    pub quality_qualification: PromotionQualification,
    pub critic_failures: u64,
    pub producer_independent: bool,
    pub contamination_detected: bool,
    pub contamination_assessment: ContaminationAssessment,
    pub conservation_veto: bool,
    pub rollback_target: Option<Sha256Digest>,
    pub promotion_state: String,
    pub explicit_decision: Option<String>,
    pub deployed: bool,
    pub import_eligible: bool,
    pub report_digest: Sha256Digest,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvaluationError {
    #[error("evaluation set is empty")]
    EmptyEvaluation,
    #[error("evaluation cases are not unique or ordered")]
    InvalidOrder,
    #[error("baseline and candidate denominators differ")]
    DenominatorMismatch,
    #[error("critic row is malformed")]
    MalformedRow,
    #[error("critic metric is non-finite")]
    NonFiniteMetric,
    #[error("critic producer is not independent")]
    ProducerNotIndependent,
    #[error("producer provenance is incomplete")]
    UnknownProducerProvenance,
    #[error("evaluation corpus identity differs")]
    CorpusMismatch,
    #[error("candidate conservation veto")]
    ConservationVeto,
    #[error("candidate lock violation")]
    CandidateLockViolation,
    #[error("candidate train and validation measurements are not finite")]
    NonFiniteCandidateQuality,
    #[error("candidate validation gap exceeds the configured threshold")]
    ValidationGapExceeded,
    #[error("candidate validation quality is below the configured threshold")]
    ValidationQualityBelowThreshold,
}

/// Numeric policy is intentionally separate from review-package completeness.
/// A complete package can still fail this policy, and a passing policy never
/// grants human authorization or production/import rights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionQualityPolicy {
    pub minimum_validation_quality: f64,
    pub maximum_validation_gap: f64,
}

impl Default for PromotionQualityPolicy {
    fn default() -> Self {
        Self {
            minimum_validation_quality: 0.0,
            // f32 measurements are widened here; retain a small explicit
            // representation margin for values such as 0.8 - 0.7.
            maximum_validation_gap: 0.100001,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionQualification {
    Qualified,
    /// Rejected is the safe default: an unmeasured candidate never qualifies.
    #[default]
    Rejected,
}

/// Evaluate measured candidate quality only. The manifest's stored gap is a
/// legacy receipt field and is deliberately ignored as an authority input.
pub fn qualify_candidate_quality(
    candidate: &CandidateManifest,
    policy: PromotionQualityPolicy,
) -> Result<PromotionQualification, EvaluationError> {
    candidate
        .validate()
        .map_err(|_| EvaluationError::CandidateLockViolation)?;
    let train = candidate.train_quality as f64;
    let validation = candidate.validation_quality as f64;
    if !train.is_finite()
        || !validation.is_finite()
        || !policy.minimum_validation_quality.is_finite()
        || !policy.maximum_validation_gap.is_finite()
    {
        return Err(EvaluationError::NonFiniteCandidateQuality);
    }
    let gap = train - validation;
    Ok(
        if validation >= policy.minimum_validation_quality && gap <= policy.maximum_validation_gap {
            PromotionQualification::Qualified
        } else {
            PromotionQualification::Rejected
        },
    )
}

fn digest<T: Serialize + ?Sized>(value: &T) -> Sha256Digest {
    let bytes = serde_json::to_vec(value).expect("typed evaluation values serialize");
    Sha256Digest::new(hex::encode(Sha256::digest(bytes))).expect("sha256 is valid")
}

fn ordered_digest(cases: &[FrozenEvalCase]) -> Sha256Digest {
    digest(cases)
}

fn aggregate(rows: &[CriticObservation]) -> (f64, f64, u64) {
    let n = rows.len() as f64;
    let failures = rows.iter().filter(|r| r.status != CriticStatus::Ok).count() as u64;
    let one = rows
        .iter()
        .map(|r| {
            if r.status == CriticStatus::Ok {
                r.metric_one
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / n;
    let two = rows
        .iter()
        .map(|r| {
            if r.status == CriticStatus::Ok {
                r.metric_two
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / n;
    (one, two, failures)
}

/// Aggregate two independently produced evaluations over one exact ordered set.
/// Every supplied row remains in the denominator, including typed failures.
pub fn aggregate_same_n_ablation(
    candidate: &CandidateManifest,
    cases: &[FrozenEvalCase],
    baseline: &[CriticObservation],
    candidate_rows: &[CriticObservation],
    conservation_ok: bool,
    trainer_producer: &str,
) -> Result<AblationReport, EvaluationError> {
    // Compatibility bridge for the pre-lineage API. The label is converted to
    // a deterministic code identity only to keep existing redacted adapters
    // operational; new callers must supply explicit provenance below.
    let corpus = cases
        .first()
        .map(|case| case.corpus.clone())
        .ok_or(EvaluationError::EmptyEvaluation)?;
    let producer = |label: &str| ProducerProvenance {
        code_digest: digest(&label),
        input_digest: corpus.clone(),
        method: "legacy-label-bridge".into(),
        authority: "legacy-adapter".into(),
    };
    aggregate_same_n_ablation_with_provenance(
        candidate,
        cases,
        baseline,
        candidate_rows,
        conservation_ok,
        &producer(baseline.first().map_or("", |row| row.producer.as_str())),
        &producer(
            candidate_rows
                .first()
                .map_or("", |row| row.producer.as_str()),
        ),
        &producer(trainer_producer),
    )
}

/// Aggregate an ablation only when producer code and input lineage are
/// explicit and machine-checkable.
// NOTE: eight parameters today; the three ProducerProvenance refs are a candidate
// for a triad struct if this grows. Allow is scoped to this fn only.
#[allow(clippy::too_many_arguments)]
pub fn aggregate_same_n_ablation_with_provenance(
    candidate: &CandidateManifest,
    cases: &[FrozenEvalCase],
    baseline: &[CriticObservation],
    candidate_rows: &[CriticObservation],
    conservation_ok: bool,
    baseline_producer: &ProducerProvenance,
    candidate_producer: &ProducerProvenance,
    trainer: &ProducerProvenance,
) -> Result<AblationReport, EvaluationError> {
    candidate
        .validate()
        .map_err(|_| EvaluationError::CandidateLockViolation)?;
    if !conservation_ok {
        return Err(EvaluationError::ConservationVeto);
    }
    if cases.is_empty() || cases.iter().any(|c| c.case_id.is_empty()) {
        return Err(EvaluationError::EmptyEvaluation);
    }
    let mut ids = BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        if case.order != index as u32 || !ids.insert(case.case_id.clone()) {
            return Err(EvaluationError::InvalidOrder);
        }
    }
    if baseline.len() != cases.len() || candidate_rows.len() != cases.len() {
        return Err(EvaluationError::DenominatorMismatch);
    }
    if [baseline_producer, candidate_producer, trainer]
        .iter()
        .any(|producer| {
            producer.method.trim().is_empty()
                || producer.authority.trim().is_empty()
                || producer.input_digest != cases[0].corpus
        })
    {
        return Err(EvaluationError::CorpusMismatch);
    }
    if baseline_producer.code_digest == trainer.code_digest
        || candidate_producer.code_digest == trainer.code_digest
        || baseline_producer.code_digest == candidate_producer.code_digest
    {
        return Err(EvaluationError::ProducerNotIndependent);
    }
    for (case, (base, cand)) in cases.iter().zip(baseline.iter().zip(candidate_rows)) {
        base.validate()?;
        cand.validate()?;
        if base.case_id != case.case_id
            || cand.case_id != case.case_id
            || base.order != case.order
            || cand.order != case.order
            || base.corpus != case.corpus
            || cand.corpus != case.corpus
        {
            return Err(EvaluationError::CorpusMismatch);
        }
    }
    let (b1, b2, bf) = aggregate(baseline);
    let (c1, c2, cf) = aggregate(candidate_rows);
    let report_base = AblationReport {
        schema_version: ABLATION_SCHEMA_VERSION.into(),
        candidate_id: candidate.receipt.candidate_id.clone(),
        corpus: cases[0].corpus.clone(),
        order_digest: ordered_digest(cases),
        baseline_n: baseline.len() as u64,
        candidate_n: candidate_rows.len() as u64,
        baseline_metric_one: b1,
        candidate_metric_one: c1,
        baseline_metric_two: b2,
        candidate_metric_two: c2,
        validation_gap: candidate.train_quality as f64 - candidate.validation_quality as f64,
        quality_qualification: qualify_candidate_quality(
            candidate,
            PromotionQualityPolicy::default(),
        )?,
        critic_failures: bf + cf,
        producer_independent: true,
        contamination_detected: false,
        contamination_assessment: if bf + cf == 0 {
            ContaminationAssessment::NotDetected
        } else {
            // A failed critic row does not prove absence of contamination.
            ContaminationAssessment::NotAssessed
        },
        conservation_veto: false,
        rollback_target: candidate.receipt.prior_candidate.clone(),
        promotion_state: "candidate".into(),
        explicit_decision: None,
        deployed: false,
        import_eligible: false,
        report_digest: Sha256Digest::new("0".repeat(64)).expect("zero digest"),
    };
    let mut report = report_base;
    report.report_digest = digest(&report);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::{CandidateReceipt, LearningLineage, PromotionState};

    fn d(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).unwrap()
    }
    fn candidate() -> CandidateManifest {
        CandidateManifest {
            receipt: CandidateReceipt {
                schema_version: 1,
                candidate_id: d('a'),
                lineage: LearningLineage {
                    source: d('b'),
                    corpus: d('c'),
                    config: d('d'),
                    prior_policy: d('e'),
                    conservation: d('f'),
                    split: d('1'),
                },
                prior_candidate: Some(d('9')),
                conservation_ok: true,
                import_eligible: false,
                promotion_state: PromotionState::Candidate,
                deployed: false,
            },
            rank: 1,
            hidden_dim: 8,
            train_count: 2,
            validation_count: 1,
            train_quality: 0.8,
            validation_quality: 0.7,
            validation_gap: 0.1,
            weights_digest: d('2'),
        }
    }
    fn provenance(code: char, method: &str, authority: &str) -> ProducerProvenance {
        ProducerProvenance {
            code_digest: d(code),
            input_digest: d('c'),
            method: method.into(),
            authority: authority.into(),
        }
    }

    fn inputs() -> (
        Vec<FrozenEvalCase>,
        Vec<CriticObservation>,
        Vec<CriticObservation>,
    ) {
        let cases = (0..2)
            .map(|i| FrozenEvalCase {
                case_id: format!("case-{i}"),
                corpus: d('c'),
                order: i,
                expected_digest: d('e'),
            })
            .collect::<Vec<_>>();
        let rows = |producer: &str| {
            (0..2)
                .map(|i| CriticObservation {
                    schema_version: 1,
                    case_id: format!("case-{i}"),
                    corpus: d('c'),
                    order: i,
                    producer: producer.into(),
                    metric_one: 0.8,
                    metric_two: 0.6,
                    status: CriticStatus::Ok,
                })
                .collect()
        };
        (cases, rows("critic-a"), rows("critic-b"))
    }
    #[test]
    fn promotion_package_vs_quality() {
        let mut value = candidate();
        value.validation_gap = 99.0;
        let policy = PromotionQualityPolicy {
            minimum_validation_quality: 0.65,
            maximum_validation_gap: 0.100001,
        };
        assert_eq!(
            qualify_candidate_quality(&value, policy),
            Ok(PromotionQualification::Qualified)
        );
        let (cases, baseline, candidate_rows) = inputs();
        let report = aggregate_same_n_ablation(
            &value,
            &cases,
            &baseline,
            &candidate_rows,
            true,
            "sona-trainer",
        )
        .unwrap();
        assert_eq!(
            report.validation_gap,
            value.train_quality as f64 - value.validation_quality as f64
        );
        assert_eq!(
            report.quality_qualification,
            PromotionQualification::Qualified
        );

        value.validation_quality = 0.2;
        assert_eq!(
            qualify_candidate_quality(&value, policy),
            Ok(PromotionQualification::Rejected)
        );
    }

    #[test]
    fn explicit_train_validation_counts_are_required_for_numeric_qualification() {
        let mut value = candidate();
        value.validation_count = 0;
        assert_eq!(
            qualify_candidate_quality(&value, PromotionQualityPolicy::default()),
            Err(EvaluationError::CandidateLockViolation)
        );
    }

    #[test]
    fn same_n_and_failures_are_visible() {
        let (cases, mut b, c) = inputs();
        b[1].status = CriticStatus::Timeout;
        let r =
            aggregate_same_n_ablation(&candidate(), &cases, &b, &c, true, "sona-trainer").unwrap();
        assert_eq!((r.baseline_n, r.candidate_n), (2, 2));
        assert_eq!(r.critic_failures, 1);
        assert!(r.baseline_metric_one < r.candidate_metric_one);
        assert_eq!(
            r.contamination_assessment,
            ContaminationAssessment::NotAssessed
        );
    }
    #[test]
    fn producer_independence_rejects_same_generator_with_different_labels() {
        let (cases, baseline, candidate_rows) = inputs();
        let same_generator = provenance('a', "shared-generator", "fixture-authority");
        let trainer = provenance('e', "trainer", "fixture-authority");
        assert_eq!(
            aggregate_same_n_ablation_with_provenance(
                &candidate(),
                &cases,
                &baseline,
                &candidate_rows,
                true,
                &same_generator,
                &same_generator,
                &trainer,
            ),
            Err(EvaluationError::ProducerNotIndependent),
            "different observation labels must not establish provenance independence"
        );
    }
    #[test]
    fn rejects_order_mutation() {
        let (cases, b, mut c) = inputs();
        c[1].order = 0;
        assert_eq!(
            aggregate_same_n_ablation(&candidate(), &cases, &b, &c, true, "sona-trainer"),
            Err(EvaluationError::CorpusMismatch)
        );
    }
    #[test]
    fn rejects_conservation_and_non_independent_critic() {
        let (cases, b, c) = inputs();
        assert_eq!(
            aggregate_same_n_ablation(&candidate(), &cases, &b, &c, false, "sona-trainer"),
            Err(EvaluationError::ConservationVeto)
        );
        let mut same = c;
        same[0].producer = "sona-trainer".into();
        assert_eq!(
            aggregate_same_n_ablation(&candidate(), &cases, &b, &same, true, "sona-trainer"),
            Err(EvaluationError::ProducerNotIndependent)
        );
    }
    #[test]
    fn report_is_byte_stable() {
        let (cases, b, c) = inputs();
        let x = serde_json::to_vec(
            &aggregate_same_n_ablation(&candidate(), &cases, &b, &c, true, "sona-trainer").unwrap(),
        )
        .unwrap();
        let y = serde_json::to_vec(
            &aggregate_same_n_ablation(&candidate(), &cases, &b, &c, true, "sona-trainer").unwrap(),
        )
        .unwrap();
        assert_eq!(x, y);
    }
}
