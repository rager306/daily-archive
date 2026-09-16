//! Policy-variant ablation aggregation for the closed legal parse policy
//! (M507-4fk6m6/S07/T02).
//!
//! A closed parse policy is not a MicroLoRA candidate: it has no rank and no
//! hidden dimension, so wrapping it in [`da_ports::CandidateManifest`] would
//! fabricate LoRA semantics the policy never had. This module therefore carries
//! its own [`PolicyVariantManifest`] while reusing the *types and invariants* of
//! [`crate::parse_learning_evaluation`] — ordered frozen cases, exact same-n
//! denominators, independent producer provenance, dual metrics, split
//! qualification and the conservation veto. There is exactly one error dialect
//! ([`EvaluationError`]); no second error enum exists.
//!
//! The aggregator is pure: no I/O, no tracing, no graph or route write and no
//! promotion. Every report is candidate-only (`promotion_state = "candidate"`,
//! `deployed = false`, `import_eligible = false`, `auto_promote = "forbidden"`).
//!
//! Independence is enforced without a self-oracle: the variant's own
//! `policy_hash` is carried into the independence check as a producer identity,
//! so an evaluator that *is* the policy under evaluation (including a
//! Java-derived one) is rejected by the shared helper rather than corroborating
//! its own artifact.

use crate::parse_learning_evaluation::{
    CriticObservation, EvaluationError, FrozenEvalCase, ProducerProvenance, PromotionQualification,
    PromotionQualityPolicy, aggregate_rows, digest_value, qualify_quality, validate_denominators,
    validate_ordered_cases, validate_producer_independence, validate_producer_lineage,
    validate_rows,
};
use da_ports::{PromotionState, Sha256Digest, SplitIdentity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Schema identity of a policy-variant ablation report.
pub const POLICY_ABLATION_SCHEMA_VERSION: &str = "parse-policy-ablation.v1";
/// Primary dual metric: agreement of the policy's legal dispatch with the
/// frozen closed action set.
pub const POLICY_DISPATCH_METRIC_ONE: &str = "legal_dispatch_agreement";
/// Secondary dual metric: share of cases where the policy yields a non-fallback
/// action.
pub const POLICY_DISPATCH_METRIC_TWO: &str = "non_fallback_action_rate";
/// Promotion is never derived from an ablation; it requires a separate,
/// explicitly authorized decision.
pub const POLICY_ABLATION_AUTO_PROMOTE: &str = "forbidden";
/// Every policy ablation report stays a candidate.
pub const POLICY_ABLATION_PROMOTION_STATE: &str = "candidate";

/// The evaluated unit: one closed legal policy variant.
///
/// `policy_hash` is the digest of the executed policy artifact and
/// `baseline_policy_hash` is the digest of the policy the variant replaces.
/// [`PromotionState`] has a single `Candidate` variant, so a promoted or
/// deployed state is unrepresentable; `deployed`/`import_eligible` are still
/// validated because they arrive from untrusted injected metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyVariantManifest {
    pub variant_id: String,
    pub policy_hash: Sha256Digest,
    pub baseline_policy_hash: Sha256Digest,
    pub registry_digest: Sha256Digest,
    pub label: String,
    pub promotion_state: PromotionState,
    pub deployed: bool,
    pub import_eligible: bool,
}

impl PolicyVariantManifest {
    /// Fail-closed candidate-only lock. A variant that claims deployment,
    /// import eligibility, an empty label or an unnamed identity is rejected
    /// before any aggregation happens.
    pub fn validate(&self) -> Result<(), EvaluationError> {
        if self.variant_id.trim().is_empty()
            || self.label.trim().is_empty()
            || self.deployed
            || self.import_eligible
        {
            return Err(EvaluationError::CandidateLockViolation);
        }
        Ok(())
    }
}

/// Ablation of one policy variant against its baseline over one exact ordered
/// same-n case set.
///
/// `train_quality`/`validation_quality` are the candidate's primary metric
/// measured over the [`SplitIdentity`] sides; `validation_gap` is their
/// difference. All lock fields are constant: this report can never claim
/// deployment, import eligibility or automatic promotion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyAblationReport {
    pub schema_version: String,
    pub variant_id: String,
    pub baseline_policy_hash: Sha256Digest,
    pub candidate_policy_hash: Sha256Digest,
    pub corpus: Sha256Digest,
    pub order_digest: Sha256Digest,
    pub split: SplitIdentity,
    /// Exact same-n size shared by baseline and candidate denominators.
    pub n: u64,
    pub baseline_metric_one: f64,
    pub baseline_metric_two: f64,
    pub candidate_metric_one: f64,
    pub candidate_metric_two: f64,
    pub metric_one_delta: f64,
    pub train_quality: f64,
    pub validation_quality: f64,
    pub validation_gap: f64,
    pub quality_qualification: PromotionQualification,
    pub critic_failures: u64,
    pub producer_independent: bool,
    pub conservation_veto: bool,
    pub promotion_state: String,
    pub deployed: bool,
    pub import_eligible: bool,
    pub auto_promote: String,
    pub report_digest: Sha256Digest,
}

/// The variant's own artifact identity. An evaluator carrying this code digest
/// would be the policy evaluating itself, so the shared independence check
/// rejects it.
fn policy_self_producer(variant: &PolicyVariantManifest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: variant.policy_hash.clone(),
        input_digest: variant.registry_digest.clone(),
        method: "policy-artifact".into(),
        authority: "parse-policy-registry".into(),
    }
}

/// The split must partition the frozen cases exactly: every case identity
/// appears on one side only, and every side member is a known case identity.
fn validate_split(cases: &[FrozenEvalCase], split: &SplitIdentity) -> Result<(), EvaluationError> {
    split
        .validate()
        .map_err(|_| EvaluationError::InvalidOrder)?;
    let known: BTreeSet<&Sha256Digest> = cases.iter().map(|case| &case.expected_digest).collect();
    let members: Vec<&Sha256Digest> = split.train.iter().chain(split.validation.iter()).collect();
    let unique: BTreeSet<&Sha256Digest> = members.iter().copied().collect();
    if unique.len() != members.len() {
        return Err(EvaluationError::InvalidOrder);
    }
    if unique.len() != known.len() || unique.iter().any(|digest| !known.contains(digest)) {
        return Err(EvaluationError::CorpusMismatch);
    }
    Ok(())
}

/// Aggregate one closed-policy variant against its baseline.
///
/// Fail-closed in this order: variant lock, conservation veto, case
/// order/uniqueness, same-n denominators, split partition, producer lineage,
/// producer independence (including the policy self identity), row correctness
/// and numeric qualification. Any deviation returns a typed [`EvaluationError`]
/// and no partial report is ever returned.
///
/// `split` is supplied explicitly because a [`SplitIdentity`] is an evaluation
/// input, not a property of the policy artifact.
#[allow(clippy::too_many_arguments)]
pub fn aggregate_policy_variant_ablation(
    variant: &PolicyVariantManifest,
    cases: &[FrozenEvalCase],
    baseline: &[CriticObservation],
    candidate_rows: &[CriticObservation],
    conservation_ok: bool,
    split: &SplitIdentity,
    baseline_producer: &ProducerProvenance,
    candidate_producer: &ProducerProvenance,
    quality_policy: PromotionQualityPolicy,
) -> Result<PolicyAblationReport, EvaluationError> {
    variant.validate()?;
    if !conservation_ok {
        return Err(EvaluationError::ConservationVeto);
    }
    validate_ordered_cases(cases)?;
    validate_denominators(cases.len(), baseline.len(), candidate_rows.len())?;
    validate_split(cases, split)?;
    validate_producer_lineage(&[baseline_producer, candidate_producer], &cases[0].corpus)?;
    let policy_self = policy_self_producer(variant);
    validate_producer_independence(&[baseline_producer, candidate_producer, &policy_self])?;
    validate_rows(cases, baseline, candidate_rows)?;

    let (b1, b2, bf) = aggregate_rows(baseline);
    let (c1, c2, cf) = aggregate_rows(candidate_rows);
    if !b1.is_finite() || !b2.is_finite() || !c1.is_finite() || !c2.is_finite() {
        // Per-row validation cannot see sum overflow: a run of finite rows can
        // still aggregate to a non-finite metric, and a non-finite report metric
        // serializes as `null` and would silently weaken the evidence.
        return Err(EvaluationError::NonFiniteMetric);
    }

    // Split membership is a case identity (`expected_digest`); `validate_split`
    // guarantees exact coverage, so both sides are non-empty.
    let mut train_rows = Vec::new();
    let mut validation_rows = Vec::new();
    for (case, row) in cases.iter().zip(candidate_rows.iter()) {
        if split.train.contains(&case.expected_digest) {
            train_rows.push(row.clone());
        } else {
            validation_rows.push(row.clone());
        }
    }
    let (train_quality, _, _) = aggregate_rows(&train_rows);
    let (validation_quality, _, _) = aggregate_rows(&validation_rows);
    let validation_gap = train_quality - validation_quality;
    let quality_qualification = qualify_quality(train_quality, validation_quality, quality_policy)?;

    let report_base = PolicyAblationReport {
        schema_version: POLICY_ABLATION_SCHEMA_VERSION.into(),
        variant_id: variant.variant_id.clone(),
        baseline_policy_hash: variant.baseline_policy_hash.clone(),
        candidate_policy_hash: variant.policy_hash.clone(),
        corpus: cases[0].corpus.clone(),
        order_digest: digest_value(cases),
        split: split.clone(),
        n: cases.len() as u64,
        baseline_metric_one: b1,
        baseline_metric_two: b2,
        candidate_metric_one: c1,
        candidate_metric_two: c2,
        metric_one_delta: c1 - b1,
        train_quality,
        validation_quality,
        validation_gap,
        quality_qualification,
        critic_failures: bf + cf,
        producer_independent: true,
        conservation_veto: false,
        promotion_state: POLICY_ABLATION_PROMOTION_STATE.into(),
        deployed: false,
        import_eligible: false,
        auto_promote: POLICY_ABLATION_AUTO_PROMOTE.into(),
        report_digest: Sha256Digest::new("0".repeat(64)).expect("zero digest"),
    };
    let mut report = report_base;
    report.report_digest = digest_value(&report);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_learning_evaluation::CriticStatus;

    fn d(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).expect("fixed digest")
    }

    fn variant() -> PolicyVariantManifest {
        PolicyVariantManifest {
            variant_id: "grammar-route-always-page-outline".into(),
            policy_hash: d('7'),
            baseline_policy_hash: d('6'),
            registry_digest: d('5'),
            label: "always page_outline".into(),
            promotion_state: PromotionState::Candidate,
            deployed: false,
            import_eligible: false,
        }
    }

    fn cases() -> Vec<FrozenEvalCase> {
        (0..3)
            .map(|order| FrozenEvalCase {
                case_id: format!("case-{order}"),
                corpus: d('c'),
                order,
                expected_digest: d(['0', '1', '2'][order as usize]),
            })
            .collect()
    }

    fn split() -> SplitIdentity {
        SplitIdentity {
            train: vec![d('0'), d('1')],
            validation: vec![d('2')],
        }
    }

    fn producer(code: char) -> ProducerProvenance {
        ProducerProvenance {
            code_digest: d(code),
            input_digest: d('c'),
            method: "policy-dispatch-critic".into(),
            authority: "fixture-authority".into(),
        }
    }

    fn rows(producer_label: &str, metrics: [f64; 3]) -> Vec<CriticObservation> {
        metrics
            .iter()
            .enumerate()
            .map(|(order, metric_one)| CriticObservation {
                schema_version: 1,
                case_id: format!("case-{order}"),
                corpus: d('c'),
                order: order as u32,
                producer: producer_label.into(),
                metric_one: *metric_one,
                metric_two: *metric_one / 2.0,
                status: CriticStatus::Ok,
            })
            .collect()
    }

    fn baseline_rows() -> Vec<CriticObservation> {
        rows("dispatch-critic-a", [0.5, 0.5, 0.5])
    }

    fn candidate_rows() -> Vec<CriticObservation> {
        // train = (0.9 + 0.7) / 2 = 0.8; validation = 0.6; gap = 0.2
        rows("dispatch-critic-b", [0.9, 0.7, 0.6])
    }

    fn quality_policy() -> PromotionQualityPolicy {
        PromotionQualityPolicy {
            minimum_validation_quality: 0.5,
            maximum_validation_gap: 0.25,
        }
    }

    fn evaluate(
        variant: &PolicyVariantManifest,
        cases: &[FrozenEvalCase],
        baseline: &[CriticObservation],
        candidate: &[CriticObservation],
        conservation_ok: bool,
        split: &SplitIdentity,
        policy: PromotionQualityPolicy,
    ) -> Result<PolicyAblationReport, EvaluationError> {
        aggregate_policy_variant_ablation(
            variant,
            cases,
            baseline,
            candidate,
            conservation_ok,
            split,
            &producer('a'),
            &producer('b'),
            policy,
        )
    }

    fn happy() -> PolicyAblationReport {
        evaluate(
            &variant(),
            &cases(),
            &baseline_rows(),
            &candidate_rows(),
            true,
            &split(),
            quality_policy(),
        )
        .expect("fixture ablation aggregates")
    }

    #[test]
    fn policy_ablation_dialect_is_pinned() {
        assert_eq!(POLICY_ABLATION_SCHEMA_VERSION, "parse-policy-ablation.v1");
        assert_eq!(POLICY_DISPATCH_METRIC_ONE, "legal_dispatch_agreement");
        assert_eq!(POLICY_DISPATCH_METRIC_TWO, "non_fallback_action_rate");
        assert_eq!(POLICY_ABLATION_PROMOTION_STATE, "candidate");
        assert_eq!(POLICY_ABLATION_AUTO_PROMOTE, "forbidden");
    }

    #[test]
    fn report_is_candidate_only_split_qualified_and_byte_stable() {
        let report = happy();
        assert_eq!(report.schema_version, POLICY_ABLATION_SCHEMA_VERSION);
        assert_eq!(report.variant_id, variant().variant_id);
        assert_eq!(report.baseline_policy_hash, d('6'));
        assert_eq!(report.candidate_policy_hash, d('7'));
        assert_eq!(report.corpus, d('c'));
        assert_eq!(report.split, split());
        assert_eq!(report.order_digest, digest_value(&cases()));
        assert_eq!(report.n, 3);
        assert!((report.baseline_metric_one - 0.5).abs() < 1e-12);
        assert!((report.candidate_metric_one - 2.2 / 3.0).abs() < 1e-12);
        assert!((report.baseline_metric_two - 0.25).abs() < 1e-12);
        assert!((report.candidate_metric_two - 1.1 / 3.0).abs() < 1e-12);
        assert!(
            (report.metric_one_delta - (report.candidate_metric_one - report.baseline_metric_one))
                .abs()
                < 1e-12
        );
        assert!((report.train_quality - 0.8).abs() < 1e-12);
        assert!((report.validation_quality - 0.6).abs() < 1e-12);
        assert!((report.validation_gap - 0.2).abs() < 1e-12);
        assert_eq!(
            report.quality_qualification,
            PromotionQualification::Qualified
        );
        assert_eq!(report.critic_failures, 0);
        assert!(report.producer_independent);
        assert!(!report.conservation_veto);
        assert_eq!(report.promotion_state, "candidate");
        assert!(!report.deployed && !report.import_eligible);
        assert_eq!(report.auto_promote, "forbidden");

        let rerun = happy();
        assert_eq!(report.report_digest, rerun.report_digest);
        assert_eq!(
            serde_json::to_vec(&report).expect("report serializes"),
            serde_json::to_vec(&rerun).expect("report serializes")
        );

        let mut changed_rows = candidate_rows();
        changed_rows[2].metric_one = 0.55;
        let changed = evaluate(
            &variant(),
            &cases(),
            &baseline_rows(),
            &changed_rows,
            true,
            &split(),
            quality_policy(),
        )
        .expect("fixture ablation aggregates");
        assert_ne!(
            report.report_digest, changed.report_digest,
            "a changed observation must change the report digest"
        );
    }

    #[test]
    fn variant_lock_and_conservation_veto_fail_closed() {
        let fixtures = (cases(), baseline_rows(), candidate_rows(), split());
        let (cases, baseline, candidate, split) = fixtures;

        let mut deployed = variant();
        deployed.deployed = true;
        assert_eq!(
            evaluate(
                &deployed,
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::CandidateLockViolation)
        );

        let mut import_eligible = variant();
        import_eligible.import_eligible = true;
        assert_eq!(
            evaluate(
                &import_eligible,
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::CandidateLockViolation)
        );

        let mut unlabeled = variant();
        unlabeled.label = "   ".into();
        assert_eq!(
            evaluate(
                &unlabeled,
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::CandidateLockViolation)
        );

        let mut unnamed = variant();
        unnamed.variant_id = String::new();
        assert_eq!(
            evaluate(
                &unnamed,
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::CandidateLockViolation)
        );

        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &candidate,
                false,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::ConservationVeto)
        );
    }

    #[test]
    fn ordered_cases_denominators_and_empty_set_fail_closed() {
        let (cases, baseline, candidate, split) =
            (cases(), baseline_rows(), candidate_rows(), split());

        let mut unordered = cases.clone();
        unordered[1].order = 0;
        assert_eq!(
            evaluate(
                &variant(),
                &unordered,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::InvalidOrder)
        );

        let mut duplicates = cases.clone();
        duplicates[2].case_id = duplicates[0].case_id.clone();
        assert_eq!(
            evaluate(
                &variant(),
                &duplicates,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::InvalidOrder)
        );

        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline[..2],
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::DenominatorMismatch)
        );
        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &candidate[..2],
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::DenominatorMismatch)
        );

        let empty: Vec<FrozenEvalCase> = Vec::new();
        assert_eq!(
            evaluate(
                &variant(),
                &empty,
                &baseline,
                &candidate,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::EmptyEvaluation)
        );
    }

    #[test]
    fn split_overlap_unknown_duplicate_and_incomplete_are_rejected() {
        let (cases, baseline, candidate) = (cases(), baseline_rows(), candidate_rows());

        for bad_split in [
            // overlap
            SplitIdentity {
                train: vec![d('0')],
                validation: vec![d('0')],
            },
            // empty side
            SplitIdentity {
                train: Vec::new(),
                validation: vec![d('2')],
            },
            // duplicated membership
            SplitIdentity {
                train: vec![d('0'), d('0'), d('1')],
                validation: vec![d('2')],
            },
        ] {
            assert_eq!(
                evaluate(
                    &variant(),
                    &cases,
                    &baseline,
                    &candidate,
                    true,
                    &bad_split,
                    quality_policy()
                ),
                Err(EvaluationError::InvalidOrder),
                "structurally invalid split must not aggregate"
            );
        }

        for bad_split in [
            // unknown member
            SplitIdentity {
                train: vec![d('0'), d('1')],
                validation: vec![d('f')],
            },
            // uncovered case identity
            SplitIdentity {
                train: vec![d('0')],
                validation: vec![d('1')],
            },
        ] {
            assert_eq!(
                evaluate(
                    &variant(),
                    &cases,
                    &baseline,
                    &candidate,
                    true,
                    &bad_split,
                    quality_policy()
                ),
                Err(EvaluationError::CorpusMismatch),
                "split must partition exactly the frozen case identities"
            );
        }
    }

    #[test]
    fn identical_producer_or_policy_self_identity_is_rejected() {
        let (cases, baseline, candidate, split) =
            (cases(), baseline_rows(), candidate_rows(), split());
        let shared = producer('a');

        assert_eq!(
            aggregate_policy_variant_ablation(
                &variant(),
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                &shared,
                &shared,
                quality_policy(),
            ),
            Err(EvaluationError::ProducerNotIndependent),
            "two producers with one code identity are not independent"
        );

        // The policy under evaluation cannot evaluate itself, even when it
        // arrives under a different method/authority label.
        let self_oracle = ProducerProvenance {
            code_digest: variant().policy_hash,
            ..producer('a')
        };
        assert_eq!(
            aggregate_policy_variant_ablation(
                &variant(),
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                &producer('a'),
                &self_oracle,
                quality_policy(),
            ),
            Err(EvaluationError::ProducerNotIndependent)
        );
    }

    #[test]
    fn non_finite_metric_and_non_finite_policy_fail_closed() {
        let (cases, baseline, split) = (cases(), baseline_rows(), split());

        let mut nan_rows = candidate_rows();
        nan_rows[0].metric_one = f64::NAN;
        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &nan_rows,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::NonFiniteMetric)
        );

        let nan_policy = PromotionQualityPolicy {
            minimum_validation_quality: 0.5,
            maximum_validation_gap: f64::NAN,
        };
        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &candidate_rows(),
                true,
                &split,
                nan_policy
            ),
            Err(EvaluationError::NonFiniteCandidateQuality)
        );
    }

    #[test]
    fn row_corpus_and_producer_lineage_mismatch_fail_closed() {
        let (cases, baseline, split) = (cases(), baseline_rows(), split());

        let mut malformed = candidate_rows();
        malformed[0].schema_version = 2;
        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &malformed,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::MalformedRow)
        );

        let mut foreign_corpus = candidate_rows();
        foreign_corpus[1].corpus = d('d');
        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &foreign_corpus,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::CorpusMismatch)
        );

        let foreign_producer = ProducerProvenance {
            input_digest: d('d'),
            ..producer('b')
        };
        assert_eq!(
            aggregate_policy_variant_ablation(
                &variant(),
                &cases,
                &baseline,
                &candidate_rows(),
                true,
                &split,
                &producer('a'),
                &foreign_producer,
                quality_policy(),
            ),
            Err(EvaluationError::CorpusMismatch)
        );

        let anonymous_producer = ProducerProvenance {
            method: String::new(),
            ..producer('b')
        };
        assert_eq!(
            aggregate_policy_variant_ablation(
                &variant(),
                &cases,
                &baseline,
                &candidate_rows(),
                true,
                &split,
                &producer('a'),
                &anonymous_producer,
                quality_policy(),
            ),
            Err(EvaluationError::CorpusMismatch)
        );
    }

    #[test]
    fn aggregate_overflow_is_rejected_even_when_every_row_is_finite() {
        let (cases, baseline, split) = (cases(), baseline_rows(), split());
        let huge = rows("dispatch-critic-b", [1.0e308, 1.0e308, 1.0e308]);
        assert!(huge.iter().all(|row| row.metric_one.is_finite()));

        assert_eq!(
            evaluate(
                &variant(),
                &cases,
                &baseline,
                &huge,
                true,
                &split,
                quality_policy()
            ),
            Err(EvaluationError::NonFiniteMetric),
            "finite rows that aggregate to a non-finite metric must fail closed"
        );
    }

    #[test]
    fn failures_stay_in_the_denominator_and_lower_split_quality() {
        let (cases, baseline, split) = (cases(), baseline_rows(), split());
        let mut failed = candidate_rows();
        failed[2].status = CriticStatus::Timeout;

        let report = evaluate(
            &variant(),
            &cases,
            &baseline,
            &failed,
            true,
            &split,
            quality_policy(),
        )
        .expect("failures are typed, not fatal");

        assert_eq!(report.n, 3);
        assert_eq!(report.critic_failures, 1);
        assert!((report.candidate_metric_one - (0.9 + 0.7) / 3.0).abs() < 1e-12);
        assert!((report.validation_quality - 0.0).abs() < 1e-12);
        assert!(report.validation_gap > 0.25);
        assert_eq!(
            report.quality_qualification,
            PromotionQualification::Rejected
        );
    }

    #[test]
    fn qualification_requires_both_minimum_quality_and_gap() {
        let (cases, baseline, candidate, split) =
            (cases(), baseline_rows(), candidate_rows(), split());

        // Measured train/validation quality is 0.8 / 0.6 (gap ~0.2).
        for rejected_policy in [
            PromotionQualityPolicy {
                minimum_validation_quality: 0.65,
                maximum_validation_gap: 0.25,
            },
            PromotionQualityPolicy {
                minimum_validation_quality: 0.5,
                maximum_validation_gap: 0.1,
            },
        ] {
            let report = evaluate(
                &variant(),
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                rejected_policy,
            )
            .expect("policy thresholds are not fatal");
            assert_eq!(
                report.quality_qualification,
                PromotionQualification::Rejected
            );
        }

        let boundary = PromotionQualityPolicy {
            minimum_validation_quality: 0.6,
            maximum_validation_gap: 0.200001,
        };
        let report = evaluate(
            &variant(),
            &cases,
            &baseline,
            &candidate,
            true,
            &split,
            boundary,
        )
        .expect("policy thresholds are not fatal");
        assert_eq!(
            report.quality_qualification,
            PromotionQualification::Qualified
        );
    }
}
