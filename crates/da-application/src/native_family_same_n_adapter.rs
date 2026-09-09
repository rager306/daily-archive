//! Closed DTO boundary for one native scholarly-parse family same-n dry-run.
//!
//! This module freezes injected metadata and failure vocabulary only.  It does
//! not evaluate rows, perform I/O, promote candidates, or write graph state.

use crate::derived_evidence_maturity::FamilyId;
use crate::native_family_promotion_gate::{
    FamilyPromotionEvidence, NamedGap, PromotionGateBlockReason, PromotionGateDerivation,
    PromotionProofKind, ProofSlot, derive_promotion_gate,
};
use crate::parse_learning_evaluation::{
    AblationReport, CriticObservation, EvaluationError, FrozenEvalCase, aggregate_same_n_ablation,
};
use da_ports::CandidateManifest;
use da_ports::Sha256Digest;
use serde::{Deserialize, Serialize};

/// The operational Java rollback identity is deliberately separate from the
/// evaluator's prior-candidate digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InjectedProofRefs {
    pub java_rollback_target: String,
    pub java_rollback_proof_ref: String,
    pub architecture_quarantine_proof_ref: String,
}

/// Authority and integrity claims are copied, never healed, by later binding.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyIntegrityClaims {
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub pooling: bool,
    #[serde(default)]
    pub contamination: bool,
    #[serde(default)]
    pub known_gaps_elided: bool,
}

/// One bounded, source-family-bound reviewer-visible gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedFamilyGap {
    pub id: String,
    pub family: FamilyId,
    pub summary: String,
    pub must_remain_visible: bool,
}

pub const MAX_NAMED_GAP_ID_BYTES: usize = 128;
pub const MAX_NAMED_GAP_SUMMARY_BYTES: usize = 256;

/// Inputs for exactly one family. Rows are intentionally retained verbatim;
/// the canonical evaluator owns denominator, ordering, and metric semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilySameNInput {
    pub family: FamilyId,
    pub paper_id: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub grobid_profile: String,
    pub candidate: CandidateManifest,
    pub frozen_cases: Vec<FrozenEvalCase>,
    pub baseline_rows: Vec<CriticObservation>,
    pub candidate_rows: Vec<CriticObservation>,
    pub conservation_ok: bool,
    pub trainer_producer: String,
    pub proof_refs: InjectedProofRefs,
    #[serde(default)]
    pub integrity: FamilyIntegrityClaims,
    #[serde(default)]
    pub named_gaps: Vec<NamedFamilyGap>,
}

/// Closed validation result. There is no promoted/accepted terminal variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FamilySameNResult {
    Ready {
        family: FamilyId,
        paper_id: String,
    },
    Blocked {
        reason: PromotionGateBlockReason,
        field: String,
    },
}

/// The immutable output of one family binding.  The canonical report is kept
/// alongside the proof package so downstream reviewers can inspect the exact
/// evaluator result rather than a second metric projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilySameNBinding {
    pub report: AblationReport,
    pub evidence: FamilyPromotionEvidence,
    pub gate: PromotionGateDerivation,
    pub prior_candidate_lineage: Option<Sha256Digest>,
}

/// Binding can only stop at a typed block or the existing human gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FamilySameNBindingResult {
    Ready {
        binding: Box<FamilySameNBinding>,
    },
    Blocked {
        reason: PromotionGateBlockReason,
        field: String,
    },
}

const JAVA_ROLLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";

fn blocked(reason: PromotionGateBlockReason, field: &str) -> FamilySameNBindingResult {
    FamilySameNBindingResult::Blocked {
        reason,
        field: field.to_owned(),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_ref(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.chars().any(char::is_control)
        && !value.contains('\n')
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains("://")
}

fn source_bound_ref(report: &AblationReport, kind: PromotionProofKind) -> String {
    format!("same-n:{}:{}", report.report_digest.as_str(), kind.as_str())
}

fn evaluator_proof(
    report: &AblationReport,
    kind: PromotionProofKind,
    source: &str,
    artifact: &str,
) -> ProofSlot {
    ProofSlot {
        evidence_ref: source_bound_ref(report, kind),
        source_sha256: source.to_owned(),
        artifact_sha256: Some(artifact.to_owned()),
    }
}

fn report_identity_ok(input: &FamilySameNInput, report: &AblationReport) -> bool {
    report.schema_version == crate::parse_learning_evaluation::ABLATION_SCHEMA_VERSION
        && report.candidate_id == input.candidate.receipt.candidate_id
        && report.corpus == input.frozen_cases[0].corpus
        && report.baseline_n == input.frozen_cases.len() as u64
        && report.candidate_n == input.frozen_cases.len() as u64
        && report.producer_independent
        && !report.contamination_detected
        && !report.conservation_veto
        && report.promotion_state == "candidate"
        && report.explicit_decision.is_none()
        && !report.deployed
        && !report.import_eligible
        && valid_digest(report.report_digest.as_str())
}

fn prevalidate(input: &FamilySameNInput) -> Option<FamilySameNBindingResult> {
    if input.paper_id.trim().is_empty() {
        return Some(blocked(
            PromotionGateBlockReason::MalformedPackage,
            "paper_id",
        ));
    }
    for (field, value) in [
        ("source_sha256", input.source_sha256.as_str()),
        ("artifact_sha256", input.artifact_sha256.as_str()),
    ] {
        if !valid_digest(value) {
            return Some(blocked(PromotionGateBlockReason::MalformedPackage, field));
        }
    }
    if !matches!(input.grobid_profile.as_str(), "structural" | "rich") {
        return Some(blocked(
            PromotionGateBlockReason::MalformedPackage,
            "grobid_profile",
        ));
    }
    if input.frozen_cases.is_empty() {
        return Some(blocked(
            PromotionGateBlockReason::FailedEvidence,
            "frozen_cases",
        ));
    }
    let corpus = &input.frozen_cases[0].corpus;
    if input
        .frozen_cases
        .iter()
        .any(|case| case.corpus != *corpus || case.order as usize >= input.frozen_cases.len())
    {
        return Some(blocked(
            PromotionGateBlockReason::StaleIdentity,
            "frozen_cases.corpus",
        ));
    }
    if input.candidate.receipt.lineage.corpus != *corpus {
        return Some(blocked(
            PromotionGateBlockReason::StaleIdentity,
            "candidate.receipt.lineage.corpus",
        ));
    }
    let refs = [
        (
            "proof_refs.java_rollback_target",
            input.proof_refs.java_rollback_target.as_str(),
        ),
        (
            "proof_refs.java_rollback_proof_ref",
            input.proof_refs.java_rollback_proof_ref.as_str(),
        ),
        (
            "proof_refs.architecture_quarantine_proof_ref",
            input.proof_refs.architecture_quarantine_proof_ref.as_str(),
        ),
    ];
    if input.proof_refs.java_rollback_target != JAVA_ROLLBACK_TARGET
        || refs.iter().any(|(_, value)| !valid_ref(value))
        || input.proof_refs.java_rollback_proof_ref
            == input.proof_refs.architecture_quarantine_proof_ref
    {
        return Some(blocked(
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        ));
    }
    if let Err(field) = validate_named_gaps(input.family, &input.named_gaps) {
        return Some(blocked(PromotionGateBlockReason::MalformedPackage, &field));
    }
    if input.integrity.claims_promoted
        || input.integrity.deployed
        || input.integrity.import_eligible
        || input.integrity.auto_promote
        || input.integrity.pooling
        || input.integrity.contamination
        || input.integrity.known_gaps_elided
    {
        return Some(blocked(
            PromotionGateBlockReason::AttemptedAutomaticPromotion,
            "integrity",
        ));
    }
    None
}

/// Bind one family without changing evaluator inputs or owning any side effect.
/// The canonical evaluator is intentionally called exactly once in this path.
#[allow(clippy::too_many_lines)]
pub fn bind_family_same_n(input: FamilySameNInput) -> FamilySameNBindingResult {
    if let Some(result) = prevalidate(&input) {
        return result;
    }
    let report = match aggregate_same_n_ablation(
        &input.candidate,
        &input.frozen_cases,
        &input.baseline_rows,
        &input.candidate_rows,
        input.conservation_ok,
        &input.trainer_producer,
    ) {
        Ok(report) => report,
        Err(error) => return blocked(evaluation_error_reason(&error), "evaluator"),
    };
    if !report_identity_ok(&input, &report) {
        return blocked(PromotionGateBlockReason::StaleIdentity, "report");
    }

    let mut evidence = FamilyPromotionEvidence::new(input.family, input.paper_id.clone());
    evidence.source_sha256 = input.source_sha256.clone();
    evidence.artifact_sha256 = input.artifact_sha256.clone();
    evidence.grobid_profile = input.grobid_profile.clone();
    evidence.rollback_target = Some(input.proof_refs.java_rollback_target.clone());
    evidence.named_gaps = input
        .named_gaps
        .iter()
        .map(|gap| NamedGap {
            id: gap.id.clone(),
            family: gap.family,
            summary: gap.summary.clone(),
            must_remain_visible: gap.must_remain_visible,
        })
        .collect();
    for kind in [
        PromotionProofKind::ExactSameN,
        PromotionProofKind::OrderedIdentity,
        PromotionProofKind::Conservation,
        PromotionProofKind::DualMetrics,
        PromotionProofKind::SameEvaluationSet,
        PromotionProofKind::ValidationGap,
        PromotionProofKind::IndependentProducer,
    ] {
        evidence.set_proof(
            kind,
            evaluator_proof(&report, kind, &input.source_sha256, &input.artifact_sha256),
        );
    }
    evidence.set_proof(
        PromotionProofKind::Rollback,
        ProofSlot {
            evidence_ref: input.proof_refs.java_rollback_proof_ref.clone(),
            source_sha256: input.source_sha256.clone(),
            artifact_sha256: Some(input.artifact_sha256.clone()),
        },
    );
    evidence.set_proof(
        PromotionProofKind::ArchitectureQuarantine,
        ProofSlot {
            evidence_ref: input.proof_refs.architecture_quarantine_proof_ref.clone(),
            source_sha256: input.source_sha256.clone(),
            artifact_sha256: Some(input.artifact_sha256.clone()),
        },
    );
    let gate = derive_promotion_gate(&evidence);
    if !matches!(gate, PromotionGateDerivation::HumanGateRequired { .. }) {
        let (reason, field) = match gate {
            PromotionGateDerivation::Blocked { reason, .. } => (reason, "promotion_gate"),
            PromotionGateDerivation::Incomplete { .. } => {
                (PromotionGateBlockReason::MissingEvidence, "promotion_gate")
            }
            PromotionGateDerivation::HumanGateRequired { .. } => unreachable!(),
        };
        return blocked(reason, field);
    }
    FamilySameNBindingResult::Ready {
        binding: Box::new(FamilySameNBinding {
            prior_candidate_lineage: report.rollback_target.clone(),
            report,
            evidence,
            gate,
        }),
    }
}

/// T01's frozen canonical failure table. No evaluator error is silently lost.
pub const fn evaluation_error_reason(error: &EvaluationError) -> PromotionGateBlockReason {
    match error {
        EvaluationError::EmptyEvaluation => PromotionGateBlockReason::FailedEvidence,
        EvaluationError::InvalidOrder => PromotionGateBlockReason::InvalidOrder,
        EvaluationError::DenominatorMismatch => PromotionGateBlockReason::DenominatorMismatch,
        EvaluationError::MalformedRow => PromotionGateBlockReason::MalformedPackage,
        EvaluationError::NonFiniteMetric => PromotionGateBlockReason::NonfiniteMetric,
        EvaluationError::ProducerNotIndependent => PromotionGateBlockReason::ProducerNotIndependent,
        EvaluationError::CorpusMismatch => PromotionGateBlockReason::StaleIdentity,
        EvaluationError::ConservationVeto => PromotionGateBlockReason::ConservationVeto,
        EvaluationError::CandidateLockViolation => PromotionGateBlockReason::CandidateLockViolation,
    }
}

/// Validate one injected gap without inspecting or retaining raw source data.
pub fn validate_named_gap(family: FamilyId, gap: &NamedFamilyGap) -> Result<(), String> {
    if gap.id.trim().is_empty() {
        return Err("named_gaps.id".into());
    }
    if gap.id.len() > MAX_NAMED_GAP_ID_BYTES || has_forbidden_text(&gap.id) {
        return Err("named_gaps.id".into());
    }
    if gap.family != family {
        return Err("named_gaps.family".into());
    }
    if !gap.must_remain_visible {
        return Err("named_gaps.must_remain_visible".into());
    }
    if gap.summary.trim().is_empty() || gap.summary.len() > MAX_NAMED_GAP_SUMMARY_BYTES {
        return Err("named_gaps.summary".into());
    }
    if has_forbidden_text(&gap.summary) {
        return Err("named_gaps.summary".into());
    }
    Ok(())
}

/// Validate all gaps; an empty list is valid and no supplied gap is dropped.
pub fn validate_named_gaps(family: FamilyId, gaps: &[NamedFamilyGap]) -> Result<(), String> {
    for gap in gaps {
        validate_named_gap(family, gap)?;
    }
    Ok(())
}

fn has_forbidden_text(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    [
        "<tei",
        "</tei",
        "<xml",
        "</xml",
        "body_text",
        "pdf_bytes",
        "secret",
        "api_key",
        "apikey",
        "token",
        "password",
        "authorization",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || value.starts_with('/')
        || value.starts_with('\\')
        || lower.contains("://")
        || lower.contains("@localhost")
        || lower.contains("@127.0.0.1")
        || lower.contains("www.")
        || lower.split_whitespace().any(|word| {
            let candidate =
                word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-');
            candidate.contains('.')
                && candidate.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_learning_evaluation::CriticStatus;
    use da_ports::{CandidateReceipt, LearningLineage, PromotionState, Sha256Digest};

    fn digest(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).unwrap()
    }

    fn candidate() -> CandidateManifest {
        CandidateManifest {
            receipt: CandidateReceipt {
                schema_version: 1,
                candidate_id: digest('a'),
                lineage: LearningLineage {
                    source: digest('b'),
                    corpus: digest('c'),
                    config: digest('d'),
                    prior_policy: digest('e'),
                    conservation: digest('f'),
                    split: digest('1'),
                },
                prior_candidate: Some(digest('9')),
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
            weights_digest: digest('2'),
        }
    }

    fn input(family: FamilyId) -> FamilySameNInput {
        let cases = (0..2)
            .map(|order| FrozenEvalCase {
                case_id: format!("case-{order}"),
                corpus: digest('c'),
                order,
                expected_digest: digest('e'),
            })
            .collect::<Vec<_>>();
        let rows = |producer: &str| {
            (0..2)
                .map(|order| CriticObservation {
                    schema_version: 1,
                    case_id: format!("case-{order}"),
                    corpus: digest('c'),
                    order,
                    producer: producer.into(),
                    metric_one: 0.8,
                    metric_two: 0.6,
                    status: CriticStatus::Ok,
                })
                .collect()
        };
        FamilySameNInput {
            family,
            paper_id: "paper-1".into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
            candidate: candidate(),
            frozen_cases: cases,
            baseline_rows: rows("critic-a"),
            candidate_rows: rows("critic-b"),
            conservation_ok: true,
            trainer_producer: "sona-trainer".into(),
            proof_refs: InjectedProofRefs {
                java_rollback_target: JAVA_ROLLBACK_TARGET.into(),
                java_rollback_proof_ref: "java-rollback-receipt".into(),
                architecture_quarantine_proof_ref: "architecture-quarantine-receipt".into(),
            },
            integrity: FamilyIntegrityClaims::default(),
            named_gaps: vec![gap(family, "visible bounded gap")],
        }
    }

    fn gap(family: FamilyId, summary: &str) -> NamedFamilyGap {
        NamedFamilyGap {
            id: "gap-1".into(),
            family,
            summary: summary.into(),
            must_remain_visible: true,
        }
    }

    #[test]
    fn all_five_families_bind_to_nine_ordered_unique_refs_and_human_gate() {
        for family in FamilyId::ALL {
            let result = bind_family_same_n(input(family));
            let FamilySameNBindingResult::Ready { binding } = result else {
                panic!("{family:?} did not bind: {result:?}");
            };
            assert_eq!(binding.report.baseline_n, 2);
            assert_eq!(binding.report.candidate_n, 2);
            assert_eq!(binding.evidence.family, family);
            assert_eq!(binding.evidence.proofs.len(), PromotionProofKind::ALL.len());
            let refs = match &binding.gate {
                PromotionGateDerivation::HumanGateRequired {
                    proofs,
                    evidence_refs,
                    deployed,
                    import_eligible,
                    this_wave_promotes,
                    ..
                } => {
                    assert_eq!(proofs, &PromotionProofKind::ALL.to_vec());
                    assert!(!deployed && !import_eligible && this_wave_promotes.is_none());
                    evidence_refs
                }
                gate => panic!("unexpected gate: {gate:?}"),
            };
            assert_eq!(refs.len(), 9);
            assert!(refs.windows(2).all(|pair| pair[0] != pair[1]));
            assert!(
                refs[..7]
                    .iter()
                    .all(|reference| reference.starts_with("same-n:"))
            );
            assert_eq!(refs[7], "java-rollback-receipt");
            assert_eq!(refs[8], "architecture-quarantine-receipt");
            assert_eq!(
                binding.evidence.rollback_target.as_deref(),
                Some(JAVA_ROLLBACK_TARGET)
            );
            assert_eq!(binding.evidence.named_gaps.len(), 1);
            assert_eq!(binding.prior_candidate_lineage, Some(digest('9')));
        }
    }

    #[test]
    fn mixed_statuses_remain_in_canonical_denominator() {
        let mut value = input(FamilyId::Body);
        value.baseline_rows[1].status = CriticStatus::Timeout;
        value.baseline_rows[1].metric_one = 999.0;
        value.baseline_rows[1].metric_two = 999.0;
        let FamilySameNBindingResult::Ready { binding } = bind_family_same_n(value) else {
            panic!("mixed status must be represented by canonical report");
        };
        assert_eq!(binding.report.baseline_n, 2);
        assert_eq!(binding.report.critic_failures, 1);
        assert!(binding.report.baseline_metric_one < 1.0);
    }

    #[test]
    fn identity_and_authority_negatives_fail_closed_before_or_after_evaluator() {
        let mut stale = input(FamilyId::Header);
        stale.candidate.receipt.lineage.corpus = digest('0');
        assert!(matches!(
            bind_family_same_n(stale),
            FamilySameNBindingResult::Blocked {
                reason: PromotionGateBlockReason::StaleIdentity,
                field
            } if field == "candidate.receipt.lineage.corpus"
        ));

        let mut confused_refs = input(FamilyId::Body);
        confused_refs.proof_refs.architecture_quarantine_proof_ref =
            confused_refs.proof_refs.java_rollback_proof_ref.clone();
        assert!(matches!(
            bind_family_same_n(confused_refs),
            FamilySameNBindingResult::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                field
            } if field == "proof_refs"
        ));

        let mut authority = input(FamilyId::Visual);
        authority.integrity.claims_promoted = true;
        assert!(matches!(
            bind_family_same_n(authority),
            FamilySameNBindingResult::Blocked {
                reason: PromotionGateBlockReason::AttemptedAutomaticPromotion,
                field
            } if field == "integrity"
        ));
    }

    #[test]
    fn dto_result_is_tagged_and_roundtrips() {
        let gap_value = gap(FamilyId::Header, "visible gap");
        let gap_json = serde_json::to_vec(&gap_value).unwrap();
        assert_eq!(
            serde_json::from_slice::<NamedFamilyGap>(&gap_json).unwrap(),
            gap_value
        );

        let values = [
            FamilySameNResult::Ready {
                family: FamilyId::Header,
                paper_id: "paper-1".into(),
            },
            FamilySameNResult::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                field: "named_gaps.summary".into(),
            },
        ];
        for value in values {
            let json = serde_json::to_value(&value).unwrap();
            assert!(json.get("status").is_some());
            assert_eq!(
                serde_json::from_value::<FamilySameNResult>(json).unwrap(),
                value
            );
        }
    }

    #[test]
    fn every_evaluation_error_has_a_stable_typed_mapping() {
        let errors = [
            (
                EvaluationError::EmptyEvaluation,
                PromotionGateBlockReason::FailedEvidence,
            ),
            (
                EvaluationError::InvalidOrder,
                PromotionGateBlockReason::InvalidOrder,
            ),
            (
                EvaluationError::DenominatorMismatch,
                PromotionGateBlockReason::DenominatorMismatch,
            ),
            (
                EvaluationError::MalformedRow,
                PromotionGateBlockReason::MalformedPackage,
            ),
            (
                EvaluationError::NonFiniteMetric,
                PromotionGateBlockReason::NonfiniteMetric,
            ),
            (
                EvaluationError::ProducerNotIndependent,
                PromotionGateBlockReason::ProducerNotIndependent,
            ),
            (
                EvaluationError::CorpusMismatch,
                PromotionGateBlockReason::StaleIdentity,
            ),
            (
                EvaluationError::ConservationVeto,
                PromotionGateBlockReason::ConservationVeto,
            ),
            (
                EvaluationError::CandidateLockViolation,
                PromotionGateBlockReason::CandidateLockViolation,
            ),
        ];
        for (error, expected) in errors {
            assert_eq!(evaluation_error_reason(&error), expected);
        }
    }

    #[test]
    fn gaps_allow_empty_and_valid_utf8_but_reject_untrusted_boundaries() {
        assert!(validate_named_gaps(FamilyId::Body, &[]).is_ok());
        assert!(validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, "visible gap")).is_ok());
        for bad in [
            "",
            "line\nbreak",
            "<TEI payload>",
            "/absolute/path",
            "https://example.invalid/secret",
            "api_key=hidden",
        ] {
            assert!(
                validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, bad)).is_err(),
                "{bad:?}"
            );
        }
        let mut too_long = "x".repeat(MAX_NAMED_GAP_SUMMARY_BYTES);
        too_long.push('x');
        assert!(validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, &too_long)).is_err());
    }

    #[test]
    fn gaps_require_family_and_visibility() {
        let mut wrong_family = gap(FamilyId::Header, "gap");
        assert!(validate_named_gap(FamilyId::Body, &wrong_family).is_err());
        wrong_family.family = FamilyId::Body;
        wrong_family.must_remain_visible = false;
        assert!(validate_named_gap(FamilyId::Body, &wrong_family).is_err());
    }
}
