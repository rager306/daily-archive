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
    AblationReport, CriticObservation, EvaluationError, FrozenEvalCase, ProducerProvenance,
    ProvenanceBasis, aggregate_same_n_ablation_with_provenance,
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

/// Explicit machine identities for the three producers of one family same-n
/// run. The display label on each row is diagnostic only; independence is
/// decided from these code/input/method identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyProducerProvenance {
    pub baseline: ProducerProvenance,
    pub candidate: ProducerProvenance,
    pub trainer: ProducerProvenance,
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
    #[serde(default)]
    pub this_wave_promotes: Option<String>,
    #[serde(default)]
    pub named_gap_hidden: bool,
    #[serde(default)]
    pub stale_identity: bool,
    #[serde(default)]
    pub unsafe_production_call: bool,
    #[serde(default)]
    pub architecture_quarantine_breach: bool,
    #[serde(default)]
    pub candidate_lock_violation: bool,
    #[serde(default)]
    pub failed_evidence: bool,
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
    /// Explicit identities; labels never establish independence here.
    pub producers: FamilyProducerProvenance,
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
        && report.provenance_basis == ProvenanceBasis::Explicit
        && !report.contamination_detected
        && !report.conservation_veto
        && report.promotion_state == "candidate"
        && report.explicit_decision.is_none()
        && !report.deployed
        && !report.import_eligible
        && valid_digest(report.report_digest.as_str())
}

fn prevalidate(input: &FamilySameNInput) -> Option<FamilySameNBindingResult> {
    let integrity = &input.integrity;
    if integrity.claims_promoted
        || integrity.deployed
        || integrity.import_eligible
        || integrity.auto_promote
        || integrity
            .this_wave_promotes
            .as_deref()
            .is_some_and(|value| value != "none")
    {
        return Some(blocked(
            PromotionGateBlockReason::AttemptedAutomaticPromotion,
            "integrity",
        ));
    }
    let integrity_flags = [
        (integrity.pooling, PromotionGateBlockReason::Pooling),
        (
            integrity.named_gap_hidden || integrity.known_gaps_elided,
            PromotionGateBlockReason::NamedGapHidden,
        ),
        (
            integrity.contamination,
            PromotionGateBlockReason::Contamination,
        ),
        (
            integrity.stale_identity,
            PromotionGateBlockReason::StaleIdentity,
        ),
        (
            integrity.unsafe_production_call,
            PromotionGateBlockReason::UnsafeProductionCall,
        ),
        (
            integrity.architecture_quarantine_breach,
            PromotionGateBlockReason::ArchitectureQuarantineBreach,
        ),
        (
            integrity.candidate_lock_violation,
            PromotionGateBlockReason::CandidateLockViolation,
        ),
        (
            integrity.failed_evidence,
            PromotionGateBlockReason::FailedEvidence,
        ),
    ];
    if let Some((true, reason)) = integrity_flags.into_iter().find(|(set, _)| *set) {
        return Some(blocked(reason, "integrity"));
    }

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
    None
}

/// Bind one family without changing evaluator inputs or owning any side effect.
/// The canonical evaluator is intentionally called exactly once in this path.
#[allow(clippy::too_many_lines)]
pub fn bind_family_same_n(input: FamilySameNInput) -> FamilySameNBindingResult {
    if let Some(result) = prevalidate(&input) {
        return result;
    }
    let report = match aggregate_same_n_ablation_with_provenance(
        &input.candidate,
        &input.frozen_cases,
        &input.baseline_rows,
        &input.candidate_rows,
        input.conservation_ok,
        &input.producers.baseline,
        &input.producers.candidate,
        &input.producers.trainer,
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
        EvaluationError::UnknownProducerProvenance => PromotionGateBlockReason::StaleIdentity,
        EvaluationError::CorpusMismatch => PromotionGateBlockReason::StaleIdentity,
        EvaluationError::ConservationVeto => PromotionGateBlockReason::ConservationVeto,
        EvaluationError::CandidateLockViolation => PromotionGateBlockReason::CandidateLockViolation,
        EvaluationError::NonFiniteCandidateQuality
        | EvaluationError::ValidationGapExceeded
        | EvaluationError::ValidationQualityBelowThreshold => {
            PromotionGateBlockReason::FailedEvidence
        }
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

    fn producer(code: char, method: &str) -> ProducerProvenance {
        ProducerProvenance {
            code_digest: digest(code),
            input_digest: digest('c'),
            method: method.into(),
            authority: "fixture-authority".into(),
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
            producers: FamilyProducerProvenance {
                baseline: producer('1', "critic-a-model"),
                candidate: producer('2', "critic-b-model"),
                trainer: producer('3', "sona-trainer"),
            },
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
            assert_eq!(binding.report.provenance_basis, ProvenanceBasis::Explicit);
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
    fn different_labels_of_the_same_producer_are_not_independent() {
        let mut value = input(FamilyId::Header);
        for row in &mut value.baseline_rows {
            row.producer = "critic-label-one".into();
        }
        for row in &mut value.candidate_rows {
            row.producer = "critic-label-two".into();
        }
        // Distinct display labels, identical explicit machine identity: the
        // label bridge would report these as independent, the explicit path
        // must not.
        value.producers.candidate = value.producers.baseline.clone();
        assert_blocked(
            value,
            PromotionGateBlockReason::ProducerNotIndependent,
            "evaluator",
        );
    }

    #[test]
    fn legacy_label_bridge_report_cannot_authorize_promotion() {
        let value = input(FamilyId::Header);
        let FamilySameNBindingResult::Ready { binding } = bind_family_same_n(value.clone()) else {
            panic!("fixture must bind")
        };
        let mut report = binding.report;
        assert_eq!(report.provenance_basis, ProvenanceBasis::Explicit);
        // A label-bridged report is structurally identical otherwise; only its
        // identity basis differs, and that alone must fail the family gate.
        report.provenance_basis = ProvenanceBasis::LegacyLabelBridge;
        assert!(
            !report_identity_ok(&value, &report),
            "label-bridged independence must never authorize the family gate"
        );
    }

    #[test]
    fn producer_provenance_must_bind_to_the_frozen_corpus() {
        let mut unbound_input = input(FamilyId::Header);
        unbound_input.producers.baseline.input_digest = digest('0');
        assert_blocked(
            unbound_input,
            PromotionGateBlockReason::StaleIdentity,
            "evaluator",
        );

        let mut unbound_method = input(FamilyId::Header);
        unbound_method.producers.candidate.method = "  ".into();
        assert_blocked(
            unbound_method,
            PromotionGateBlockReason::StaleIdentity,
            "evaluator",
        );
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

    fn assert_blocked(value: FamilySameNInput, expected: PromotionGateBlockReason, field: &str) {
        assert!(
            matches!(
                bind_family_same_n(value),
                FamilySameNBindingResult::Blocked { reason, field: actual }
                    if reason == expected && actual == field
            ),
            "expected {expected:?} at {field}"
        );
    }

    #[test]
    fn adversarial_integrity_mutation_catalog_is_fail_closed() {
        let authority = [
            "claims_promoted",
            "deployed",
            "import_eligible",
            "auto_promote",
            "this_wave_promotes",
        ];
        for name in authority {
            let mut value = input(FamilyId::Header);
            match name {
                "claims_promoted" => value.integrity.claims_promoted = true,
                "deployed" => value.integrity.deployed = true,
                "import_eligible" => value.integrity.import_eligible = !false,
                "auto_promote" => value.integrity.auto_promote = true,
                "this_wave_promotes" => value.integrity.this_wave_promotes = Some("header".into()),
                _ => unreachable!(),
            }
            assert_blocked(
                value,
                PromotionGateBlockReason::AttemptedAutomaticPromotion,
                "integrity",
            );
            assert!(!name.is_empty());
        }

        let integrity = [
            ("pooling", PromotionGateBlockReason::Pooling),
            ("named_gap_hidden", PromotionGateBlockReason::NamedGapHidden),
            ("contamination", PromotionGateBlockReason::Contamination),
            ("stale_identity", PromotionGateBlockReason::StaleIdentity),
            (
                "unsafe_production_call",
                PromotionGateBlockReason::UnsafeProductionCall,
            ),
            (
                "architecture_quarantine_breach",
                PromotionGateBlockReason::ArchitectureQuarantineBreach,
            ),
            (
                "candidate_lock_violation",
                PromotionGateBlockReason::CandidateLockViolation,
            ),
            ("failed_evidence", PromotionGateBlockReason::FailedEvidence),
        ];
        for (name, reason) in integrity {
            let mut value = input(FamilyId::Header);
            match name {
                "pooling" => value.integrity.pooling = true,
                "named_gap_hidden" => value.integrity.named_gap_hidden = true,
                "contamination" => value.integrity.contamination = true,
                "stale_identity" => value.integrity.stale_identity = true,
                "unsafe_production_call" => value.integrity.unsafe_production_call = true,
                "architecture_quarantine_breach" => {
                    value.integrity.architecture_quarantine_breach = true
                }
                "candidate_lock_violation" => value.integrity.candidate_lock_violation = true,
                "failed_evidence" => value.integrity.failed_evidence = true,
                _ => unreachable!(),
            }
            assert_blocked(value, reason, "integrity");
            assert!(!name.is_empty());
        }

        let mut none_wave = input(FamilyId::Header);
        none_wave.integrity.this_wave_promotes = Some("none".into());
        assert!(matches!(
            bind_family_same_n(none_wave),
            FamilySameNBindingResult::Ready { .. }
        ));

        let mut stale_candidate = input(FamilyId::Header);
        stale_candidate.candidate.receipt.lineage.corpus = digest('0');
        assert_blocked(
            stale_candidate,
            PromotionGateBlockReason::StaleIdentity,
            "candidate.receipt.lineage.corpus",
        );
        let mut stale_case = input(FamilyId::Header);
        stale_case.frozen_cases[1].corpus = digest('0');
        assert_blocked(
            stale_case,
            PromotionGateBlockReason::StaleIdentity,
            "frozen_cases.corpus",
        );

        let mut missing_candidate_row = input(FamilyId::Header);
        missing_candidate_row.candidate_rows.pop();
        assert_blocked(
            missing_candidate_row,
            PromotionGateBlockReason::DenominatorMismatch,
            "evaluator",
        );
        let mut critic_corpus = input(FamilyId::Header);
        critic_corpus.baseline_rows[0].corpus = digest('0');
        assert_blocked(
            critic_corpus,
            PromotionGateBlockReason::StaleIdentity,
            "evaluator",
        );
        let mut frozen_case_id = input(FamilyId::Header);
        frozen_case_id.frozen_cases[0].case_id.clear();
        assert_blocked(
            frozen_case_id,
            PromotionGateBlockReason::FailedEvidence,
            "evaluator",
        );
        let mut nonfinite = input(FamilyId::Header);
        nonfinite.baseline_rows[0].metric_one = f64::NAN;
        assert_blocked(
            nonfinite,
            PromotionGateBlockReason::NonfiniteMetric,
            "evaluator",
        );
        let mut invalid_order = input(FamilyId::Header);
        invalid_order.frozen_cases[1].order = 0;
        assert_blocked(
            invalid_order,
            PromotionGateBlockReason::InvalidOrder,
            "evaluator",
        );
        let mut duplicate_order = input(FamilyId::Header);
        duplicate_order.frozen_cases[1].case_id = duplicate_order.frozen_cases[0].case_id.clone();
        assert_blocked(
            duplicate_order,
            PromotionGateBlockReason::InvalidOrder,
            "evaluator",
        );
        let mut producer_collision = input(FamilyId::Header);
        producer_collision.producers.candidate = producer_collision.producers.baseline.clone();
        assert_blocked(
            producer_collision,
            PromotionGateBlockReason::ProducerNotIndependent,
            "evaluator",
        );
        let mut trainer_collision = input(FamilyId::Header);
        trainer_collision.producers.trainer = trainer_collision.producers.baseline.clone();
        assert_blocked(
            trainer_collision,
            PromotionGateBlockReason::ProducerNotIndependent,
            "evaluator",
        );
        let mut conservation = input(FamilyId::Header);
        conservation.conservation_ok = false;
        assert_blocked(
            conservation,
            PromotionGateBlockReason::ConservationVeto,
            "evaluator",
        );
        let mut candidate_lock = input(FamilyId::Header);
        candidate_lock.candidate.receipt.deployed = true;
        assert_blocked(
            candidate_lock,
            PromotionGateBlockReason::CandidateLockViolation,
            "evaluator",
        );
        let mut nonfinite_validation_gap = input(FamilyId::Header);
        nonfinite_validation_gap.candidate.validation_gap = f32::NAN;
        assert_blocked(
            nonfinite_validation_gap,
            PromotionGateBlockReason::CandidateLockViolation,
            "evaluator",
        );

        let mut empty_cases = input(FamilyId::Header);
        empty_cases.frozen_cases.clear();
        assert_blocked(
            empty_cases,
            PromotionGateBlockReason::FailedEvidence,
            "frozen_cases",
        );
        let mut malformed_source = input(FamilyId::Header);
        malformed_source.source_sha256 = "not-a-digest".into();
        assert_blocked(
            malformed_source,
            PromotionGateBlockReason::MalformedPackage,
            "source_sha256",
        );
        let mut malformed_profile = input(FamilyId::Header);
        malformed_profile.grobid_profile = "unknown".into();
        assert_blocked(
            malformed_profile,
            PromotionGateBlockReason::MalformedPackage,
            "grobid_profile",
        );
        let mut missing_java = input(FamilyId::Header);
        missing_java.proof_refs.java_rollback_target.clear();
        assert_blocked(
            missing_java,
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        );
        let mut non_java = input(FamilyId::Header);
        non_java.proof_refs.java_rollback_target = "python-grobid@1.0".into();
        assert_blocked(
            non_java,
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        );
        let mut confused_with_prior = input(FamilyId::Header);
        confused_with_prior.proof_refs.java_rollback_target = digest('9').as_str().into();
        assert_blocked(
            confused_with_prior,
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        );
        let mut equal_refs = input(FamilyId::Header);
        equal_refs.proof_refs.architecture_quarantine_proof_ref =
            equal_refs.proof_refs.java_rollback_proof_ref.clone();
        assert_blocked(
            equal_refs,
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        );
        let mut empty_rollback = input(FamilyId::Header);
        empty_rollback.proof_refs.java_rollback_proof_ref.clear();
        assert_blocked(
            empty_rollback,
            PromotionGateBlockReason::MalformedPackage,
            "proof_refs",
        );
        let mut cross_family_gap = input(FamilyId::Header);
        cross_family_gap.named_gaps[0].family = FamilyId::Body;
        assert_blocked(
            cross_family_gap,
            PromotionGateBlockReason::MalformedPackage,
            "named_gaps.family",
        );

        let mut authority_precedes_malformed = input(FamilyId::Header);
        authority_precedes_malformed.integrity.auto_promote = true;
        authority_precedes_malformed.source_sha256 = "malicious gap payload".into();
        assert_blocked(
            authority_precedes_malformed,
            PromotionGateBlockReason::AttemptedAutomaticPromotion,
            "integrity",
        );
    }

    #[test]
    fn integrity_claims_are_backward_compatible_serde_defaults() {
        let json = serde_json::json!({"claims_promoted": false});
        let claims: FamilyIntegrityClaims = serde_json::from_value(json).unwrap();
        assert_eq!(claims, FamilyIntegrityClaims::default());
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
