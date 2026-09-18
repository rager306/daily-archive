//! Closed, pure evidence gate for native scholarly-parse family dry-runs.
//!
//! This module accepts only injected evidence. It has no parse, persistence,
//! deployment, import, or promotion capability; its highest result is a
//! request for human review.

use crate::derived_evidence_maturity::FamilyId;
use crate::parse_learning_evaluation::{
    ContaminationAssessment, ProducerProvenance, PromotionQualityPolicy,
};
use serde::{Deserialize, Serialize};

/// The nine mandatory, ordered proof slots in a family review package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionProofKind {
    ExactSameN,
    OrderedIdentity,
    Conservation,
    DualMetrics,
    SameEvaluationSet,
    ValidationGap,
    IndependentProducer,
    Rollback,
    ArchitectureQuarantine,
}

impl PromotionProofKind {
    pub const ALL: [Self; 9] = [
        Self::ExactSameN,
        Self::OrderedIdentity,
        Self::Conservation,
        Self::DualMetrics,
        Self::SameEvaluationSet,
        Self::ValidationGap,
        Self::IndependentProducer,
        Self::Rollback,
        Self::ArchitectureQuarantine,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactSameN => "exact_same_n",
            Self::OrderedIdentity => "ordered_identity",
            Self::Conservation => "conservation",
            Self::DualMetrics => "dual_metrics",
            Self::SameEvaluationSet => "same_evaluation_set",
            Self::ValidationGap => "validation_gap",
            Self::IndependentProducer => "independent_producer",
            Self::Rollback => "rollback",
            Self::ArchitectureQuarantine => "architecture_quarantine",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Typed fail-closed reasons reserved for current and later detectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionGateBlockReason {
    MissingEvidence,
    UnknownFamily,
    MalformedPackage,
    AttemptedAutomaticPromotion,
    Pooling,
    NamedGapHidden,
    StaleIdentity,
    DenominatorMismatch,
    Contamination,
    NonfiniteMetric,
    MissingRollback,
    UnsafeProductionCall,
    ProducerNotIndependent,
    ConservationVeto,
    InvalidOrder,
    ArchitectureQuarantineBreach,
    CandidateLockViolation,
    FailedEvidence,
}

/// One digest-bound, durable proof reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofSlot {
    pub evidence_ref: String,
    pub source_sha256: String,
    #[serde(default)]
    pub artifact_sha256: Option<String>,
}

/// A gap that must remain visible to a later human reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedGap {
    pub id: String,
    pub family: FamilyId,
    pub summary: String,
    pub must_remain_visible: bool,
}

/// Injected evidence package. Proof occupancy and measured values are separate
/// concerns: completeness lives in the proof slots, qualification inputs live
/// in the measured fields, and an absent measurement is never read as a clean
/// claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyPromotionEvidence {
    pub family: FamilyId,
    pub paper_id: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub grobid_profile: String,
    #[serde(default)]
    pub proofs: std::collections::BTreeMap<PromotionProofKind, ProofSlot>,
    #[serde(default)]
    pub named_gaps: Vec<NamedGap>,
    #[serde(default)]
    pub rollback_target: Option<String>,
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub this_wave_promotes: Option<String>,
    #[serde(default)]
    pub known_gaps_elided: bool,
    #[serde(default)]
    pub pooling: bool,
    #[serde(default)]
    pub named_gap_hidden: bool,
    #[serde(default)]
    pub stale_identity: bool,
    #[serde(default)]
    pub denominator_mismatch: bool,
    /// Tri-state measured contamination. An absent measurement deserializes to
    /// `NotAssessed`, never to a clean `NotDetected`.
    #[serde(default = "contamination_not_assessed")]
    pub contamination: ContaminationAssessment,
    /// Measured producer identities. Independence is a property of these
    /// values; display labels never establish it.
    #[serde(default)]
    pub producer_provenance: Vec<ProducerProvenance>,
    /// Measured candidate train quality; `None` means it was never measured.
    #[serde(default)]
    pub measured_train_quality: Option<f64>,
    /// Measured candidate validation quality; `None` means never measured.
    #[serde(default)]
    pub measured_validation_quality: Option<f64>,
    #[serde(default)]
    pub nonfinite_metric: bool,
    #[serde(default)]
    pub producer_not_independent: bool,
    #[serde(default)]
    pub conservation_veto: bool,
    #[serde(default)]
    pub invalid_order: bool,
    #[serde(default)]
    pub unsafe_production_call: bool,
    #[serde(default)]
    pub architecture_quarantine_breach: bool,
    #[serde(default)]
    pub candidate_lock_violation: bool,
    #[serde(default)]
    pub failed_evidence: bool,
}

impl FamilyPromotionEvidence {
    pub fn new(family: FamilyId, paper_id: impl Into<String>) -> Self {
        Self {
            family,
            paper_id: paper_id.into(),
            source_sha256: String::new(),
            artifact_sha256: String::new(),
            grobid_profile: String::new(),
            proofs: Default::default(),
            named_gaps: Vec::new(),
            rollback_target: None,
            claims_promoted: false,
            deployed: false,
            import_eligible: false,
            auto_promote: false,
            this_wave_promotes: None,
            known_gaps_elided: false,
            pooling: false,
            named_gap_hidden: false,
            stale_identity: false,
            denominator_mismatch: false,
            contamination: ContaminationAssessment::NotAssessed,
            producer_provenance: Vec::new(),
            measured_train_quality: None,
            measured_validation_quality: None,
            nonfinite_metric: false,
            producer_not_independent: false,
            conservation_veto: false,
            invalid_order: false,
            unsafe_production_call: false,
            architecture_quarantine_breach: false,
            candidate_lock_violation: false,
            failed_evidence: false,
        }
    }

    pub fn set_proof(&mut self, kind: PromotionProofKind, slot: ProofSlot) {
        self.proofs.insert(kind, slot);
    }

    pub fn clear_proof(&mut self, kind: PromotionProofKind) {
        self.proofs.remove(&kind);
    }

    #[cfg(test)]
    fn set_integrity_flag(&mut self, name: &str) {
        match name {
            "pooling" => self.pooling = true,
            "named_gap_hidden" => self.named_gap_hidden = true,
            "stale_identity" => self.stale_identity = true,
            "denominator_mismatch" => self.denominator_mismatch = true,
            "contamination" => self.contamination = ContaminationAssessment::Detected,
            "nonfinite_metric" => self.nonfinite_metric = true,
            "producer_not_independent" => self.producer_not_independent = true,
            "conservation_veto" => self.conservation_veto = true,
            "invalid_order" => self.invalid_order = true,
            "unsafe_production_call" => self.unsafe_production_call = true,
            "architecture_quarantine_breach" => self.architecture_quarantine_breach = true,
            "candidate_lock_violation" => self.candidate_lock_violation = true,
            "failed_evidence" => self.failed_evidence = true,
            _ => panic!("unknown test flag {name}"),
        }
    }
}

/// Closed result: no automatic or terminal admission state exists here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromotionGateDerivation {
    Incomplete {
        missing: Vec<PromotionProofKind>,
        evidence_refs: Vec<String>,
    },
    Blocked {
        reason: PromotionGateBlockReason,
        missing_or_offending: Option<String>,
        evidence_refs: Vec<String>,
    },
    HumanGateRequired {
        proofs: Vec<PromotionProofKind>,
        named_gaps: Vec<NamedGap>,
        evidence_refs: Vec<String>,
        deployed: bool,
        this_wave_promotes: Option<String>,
        import_eligible: bool,
    },
}

/// Deliberately compile-time constant: this gate can never auto-promote.
pub const fn can_auto_promote() -> bool {
    false
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn contamination_not_assessed() -> ContaminationAssessment {
    ContaminationAssessment::NotAssessed
}

fn blocked(reason: PromotionGateBlockReason, detail: &str) -> PromotionGateDerivation {
    PromotionGateDerivation::Blocked {
        reason,
        missing_or_offending: Some(detail.to_owned()),
        evidence_refs: Vec::new(),
    }
}

/// Derive a review disposition using deterministic, fail-closed precedence.
pub fn derive_promotion_gate(package: &FamilyPromotionEvidence) -> PromotionGateDerivation {
    if package.claims_promoted
        || package.deployed
        || package.import_eligible
        || package.auto_promote
        || package
            .this_wave_promotes
            .as_deref()
            .is_some_and(|value| value != "none")
    {
        return blocked(
            PromotionGateBlockReason::AttemptedAutomaticPromotion,
            "authority_claim",
        );
    }

    let flags = [
        (package.pooling, PromotionGateBlockReason::Pooling),
        (
            package.named_gap_hidden || package.known_gaps_elided,
            PromotionGateBlockReason::NamedGapHidden,
        ),
        (
            package.contamination == ContaminationAssessment::Detected,
            PromotionGateBlockReason::Contamination,
        ),
        (
            package.stale_identity,
            PromotionGateBlockReason::StaleIdentity,
        ),
        (
            package.denominator_mismatch,
            PromotionGateBlockReason::DenominatorMismatch,
        ),
        (
            package.conservation_veto,
            PromotionGateBlockReason::ConservationVeto,
        ),
        (
            package.nonfinite_metric,
            PromotionGateBlockReason::NonfiniteMetric,
        ),
        (
            package.producer_not_independent,
            PromotionGateBlockReason::ProducerNotIndependent,
        ),
        (
            package.invalid_order,
            PromotionGateBlockReason::InvalidOrder,
        ),
        (
            package.unsafe_production_call,
            PromotionGateBlockReason::UnsafeProductionCall,
        ),
        (
            package.architecture_quarantine_breach,
            PromotionGateBlockReason::ArchitectureQuarantineBreach,
        ),
        (
            package.candidate_lock_violation,
            PromotionGateBlockReason::CandidateLockViolation,
        ),
        (
            package.failed_evidence,
            PromotionGateBlockReason::FailedEvidence,
        ),
    ];
    if let Some((_, reason)) = flags.into_iter().find(|(set, _)| *set) {
        return blocked(reason, "integrity_flag");
    }

    if package.paper_id.trim().is_empty()
        || !valid_digest(&package.source_sha256)
        || !valid_digest(&package.artifact_sha256)
        || !matches!(package.grobid_profile.as_str(), "structural" | "rich")
    {
        return blocked(
            PromotionGateBlockReason::MalformedPackage,
            "package_identity",
        );
    }

    let mut refs = Vec::new();
    let mut missing = Vec::new();
    for kind in PromotionProofKind::ALL {
        match package.proofs.get(&kind) {
            Some(slot) if slot.evidence_ref.trim().is_empty() => {
                return blocked(PromotionGateBlockReason::MissingEvidence, kind.as_str());
            }
            Some(slot)
                if !valid_digest(&slot.source_sha256)
                    || !slot
                        .source_sha256
                        .eq_ignore_ascii_case(&package.source_sha256)
                    || slot
                        .artifact_sha256
                        .as_deref()
                        .is_some_and(|digest| !valid_digest(digest)) =>
            {
                return blocked(PromotionGateBlockReason::MalformedPackage, kind.as_str());
            }
            Some(slot) => {
                if refs.iter().any(|reference| reference == &slot.evidence_ref) {
                    return blocked(
                        PromotionGateBlockReason::MalformedPackage,
                        "duplicate_evidence_ref",
                    );
                }
                refs.push(slot.evidence_ref.clone());
            }
            None => missing.push(kind),
        }
    }

    if package.named_gaps.iter().any(|gap| {
        gap.id.trim().is_empty() || gap.summary.trim().is_empty() || !gap.must_remain_visible
    }) {
        return blocked(PromotionGateBlockReason::NamedGapHidden, "named_gap");
    }
    if package.proofs.contains_key(&PromotionProofKind::Rollback)
        && package
            .rollback_target
            .as_deref()
            .is_none_or(|target| target.trim().is_empty())
    {
        return blocked(PromotionGateBlockReason::MissingRollback, "rollback_target");
    }
    if !missing.is_empty() {
        return PromotionGateDerivation::Incomplete {
            missing,
            evidence_refs: refs,
        };
    }

    PromotionGateDerivation::HumanGateRequired {
        proofs: PromotionProofKind::ALL.to_vec(),
        named_gaps: package.named_gaps.clone(),
        evidence_refs: refs,
        deployed: false,
        this_wave_promotes: None,
        import_eligible: false,
    }
}

/// Explicit numeric and cardinality policy that measured values are judged
/// against. The policy never substitutes for a measurement: every threshold is
/// applied to a measured value, and an absent measurement is unqualified.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionQualificationPolicy {
    /// Measured candidate quality thresholds, shared with the evaluation
    /// dialect (`parse_learning_evaluation::PromotionQualityPolicy`).
    pub quality: PromotionQualityPolicy,
    /// Minimum number of pairwise-distinct, complete measured producer
    /// identities required to establish independence.
    pub minimum_independent_producers: usize,
}

impl Default for PromotionQualificationPolicy {
    fn default() -> Self {
        Self {
            quality: PromotionQualityPolicy::default(),
            // The same-n protocol compares exactly three measured producers:
            // the baseline evaluator, the candidate evaluator and the trainer.
            minimum_independent_producers: 3,
        }
    }
}

/// One typed reason a package is not technically qualified. The two
/// contamination variants keep *detected* separate from *never assessed*:
/// unknown contamination is not clean. These names are the vocabulary an
/// external contract consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDisqualifier {
    /// Two measured producers share one code digest (shared provenance).
    SharedProducerDigest,
    /// Too few distinct, complete measured producer identities.
    WeakProducerIndependence,
    /// Contamination was measured and detected.
    ContaminationDetected,
    /// Contamination was never assessed; unknown is not a clean measurement.
    ContaminationNotAssessed,
    /// Candidate train/validation quality was never measured.
    UnmeasuredQuality,
    /// Measured validation gap exceeds the explicit threshold.
    ValidationGapExceeded,
    /// Measured validation quality is below the explicit threshold.
    ValidationQualityBelowThreshold,
}

/// The independent qualification dimension, computed only from measured
/// values. It says nothing about review-package completeness, and it never
/// grants a promotion: the highest review disposition stays
/// [`PromotionGateDerivation::HumanGateRequired`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "qualified", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromotionQualificationDecision {
    Qualified {
        independent_producers: usize,
        contamination: ContaminationAssessment,
        validation_gap: f64,
    },
    Unqualified {
        reason: PromotionDisqualifier,
        /// The measured value that failed the policy, preserved verbatim so an
        /// operator can see *which* measurement blocked the promotion.
        measured: String,
    },
}

fn unqualified(
    reason: PromotionDisqualifier,
    measured: impl Into<String>,
) -> PromotionQualificationDecision {
    PromotionQualificationDecision::Unqualified {
        reason,
        measured: measured.into(),
    }
}

fn shared_code_digest(producers: &[ProducerProvenance]) -> Option<String> {
    for (index, producer) in producers.iter().enumerate() {
        if producers[index + 1..]
            .iter()
            .any(|other| other.code_digest == producer.code_digest)
        {
            return Some(producer.code_digest.as_str().to_owned());
        }
    }
    None
}

/// Compute technical promotion qualification from measured values only.
///
/// Completeness is deliberately *not* consulted: [`derive_promotion_gate`] owns
/// review-package completeness, this function owns measured qualification. A
/// complete package can be unqualified, and an incomplete package can carry
/// measured values that satisfy the policy. Unknown contamination, a shared
/// producer digest and too few measured producer identities are disqualifying,
/// never clean.
pub fn derive_promotion_qualification(
    package: &FamilyPromotionEvidence,
    policy: PromotionQualificationPolicy,
) -> PromotionQualificationDecision {
    match package.contamination {
        ContaminationAssessment::Detected => {
            return unqualified(PromotionDisqualifier::ContaminationDetected, "detected");
        }
        ContaminationAssessment::NotAssessed => {
            return unqualified(
                PromotionDisqualifier::ContaminationNotAssessed,
                "not_assessed",
            );
        }
        ContaminationAssessment::NotDetected => {}
    }

    if let Some(digest) = shared_code_digest(&package.producer_provenance) {
        return unqualified(PromotionDisqualifier::SharedProducerDigest, digest);
    }
    let distinct = package
        .producer_provenance
        .iter()
        .filter(|producer| {
            !producer.method.trim().is_empty() && !producer.authority.trim().is_empty()
        })
        .count();
    if distinct < policy.minimum_independent_producers {
        return unqualified(
            PromotionDisqualifier::WeakProducerIndependence,
            format!("{distinct} < {}", policy.minimum_independent_producers),
        );
    }

    let (Some(train_quality), Some(validation_quality)) = (
        package.measured_train_quality,
        package.measured_validation_quality,
    ) else {
        return unqualified(PromotionDisqualifier::UnmeasuredQuality, "not_measured");
    };
    if !train_quality.is_finite() || !validation_quality.is_finite() {
        return unqualified(PromotionDisqualifier::UnmeasuredQuality, "nonfinite");
    }
    let validation_gap = train_quality - validation_quality;
    if validation_quality < policy.quality.minimum_validation_quality {
        return unqualified(
            PromotionDisqualifier::ValidationQualityBelowThreshold,
            format!(
                "{validation_quality} < {}",
                policy.quality.minimum_validation_quality
            ),
        );
    }
    if validation_gap > policy.quality.maximum_validation_gap {
        return unqualified(
            PromotionDisqualifier::ValidationGapExceeded,
            format!(
                "{validation_gap} > {}",
                policy.quality.maximum_validation_gap
            ),
        );
    }

    PromotionQualificationDecision::Qualified {
        independent_producers: distinct,
        contamination: ContaminationAssessment::NotDetected,
        validation_gap,
    }
}

/// One package's two independent decisions: review-package completeness and
/// measured technical qualification. Neither implies the other, and neither
/// grants a promotion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionGateReport {
    pub completeness: PromotionGateDerivation,
    pub qualification: PromotionQualificationDecision,
}

/// Derive both dimensions from one package. The default policy is explicit and
/// overridable; the completeness decision is untouched by qualification.
pub fn derive_promotion_gate_report(
    package: &FamilyPromotionEvidence,
    policy: PromotionQualificationPolicy,
) -> PromotionGateReport {
    PromotionGateReport {
        completeness: derive_promotion_gate(package),
        qualification: derive_promotion_qualification(package, policy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(index: usize) -> ProofSlot {
        ProofSlot {
            evidence_ref: format!("evidence-{index}"),
            source_sha256: "a".repeat(64),
            artifact_sha256: Some("b".repeat(64)),
        }
    }

    fn package() -> FamilyPromotionEvidence {
        let mut package = FamilyPromotionEvidence::new(FamilyId::Body, "paper-1");
        package.source_sha256 = "a".repeat(64);
        package.artifact_sha256 = "b".repeat(64);
        package.grobid_profile = "structural".into();
        for (index, kind) in PromotionProofKind::ALL.into_iter().enumerate() {
            package.set_proof(kind, slot(index));
        }
        package.named_gaps.push(NamedGap {
            id: "body-shadow".into(),
            family: FamilyId::Body,
            summary: "body remains shadow-only".into(),
            must_remain_visible: true,
        });
        package.rollback_target = Some("java-grobid-http@0.9.0-crf".into());
        package
    }

    #[test]
    fn closed_vocab_has_exact_order_and_wire_roundtrip() {
        assert_eq!(PromotionProofKind::ALL.len(), 9);
        assert_eq!(
            PromotionProofKind::ALL.map(PromotionProofKind::as_str),
            [
                "exact_same_n",
                "ordered_identity",
                "conservation",
                "dual_metrics",
                "same_evaluation_set",
                "validation_gap",
                "independent_producer",
                "rollback",
                "architecture_quarantine"
            ]
        );
        for kind in PromotionProofKind::ALL {
            assert_eq!(PromotionProofKind::parse(kind.as_str()), Some(kind));
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            assert_eq!(
                serde_json::from_str::<PromotionProofKind>(&json).unwrap(),
                kind
            );
        }
        assert!(PromotionProofKind::parse(concat!("promo", "ted")).is_none());
        assert!(serde_json::from_str::<PromotionProofKind>("\"pooled_winner\"").is_err());
        assert!(!can_auto_promote());
    }

    #[test]
    fn complete_package_stops_at_human_gate_and_is_byte_stable() {
        for family in FamilyId::ALL {
            let mut value = package();
            value.family = family;
            for gap in &mut value.named_gaps {
                gap.family = family;
            }
            assert!(matches!(
                derive_promotion_gate(&value),
                PromotionGateDerivation::HumanGateRequired { .. }
            ));
        }

        let result = derive_promotion_gate(&package());
        let PromotionGateDerivation::HumanGateRequired {
            proofs,
            evidence_refs,
            deployed,
            import_eligible,
            this_wave_promotes,
            named_gaps,
        } = &result
        else {
            panic!("expected human gate: {result:?}")
        };
        assert_eq!(proofs, &PromotionProofKind::ALL.to_vec());
        assert_eq!(evidence_refs.len(), 9);
        assert!(!deployed && !import_eligible && this_wave_promotes.is_none());
        assert_eq!(named_gaps.len(), 1);
        let first = serde_json::to_vec(&result).unwrap();
        let second =
            serde_json::to_vec(&serde_json::from_slice::<PromotionGateDerivation>(&first).unwrap())
                .unwrap();
        assert_eq!(first, second);
        assert!(
            !first
                .windows(concat!("promo", "ted").len())
                .any(|w| w == concat!("promo", "ted").as_bytes())
        );
    }

    #[test]
    fn omitted_proofs_are_ordered_and_never_review_ready() {
        for omitted in PromotionProofKind::ALL {
            let mut value = package();
            value.clear_proof(omitted);
            assert!(matches!(
                derive_promotion_gate(&value),
                PromotionGateDerivation::Incomplete { ref missing, .. }
                    if missing == &vec![omitted]
            ));
        }
    }

    #[test]
    fn authority_claims_dominate_completeness_and_integrity_flags_have_fixed_priority() {
        let mut value = package();
        value.claims_promoted = true;
        value.pooling = true;
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::AttemptedAutomaticPromotion,
                ..
            }
        ));
        value.claims_promoted = false;
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::Pooling,
                ..
            }
        ));
    }

    #[test]
    fn malformed_identity_gap_and_rollback_fail_closed() {
        let mut value = package();
        value.source_sha256 = "bad".into();
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                ..
            }
        ));
        value = package();
        value
            .proofs
            .get_mut(&PromotionProofKind::DualMetrics)
            .unwrap()
            .source_sha256 = "c".repeat(64);
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                ..
            }
        ));
        value = package();
        value
            .proofs
            .get_mut(&PromotionProofKind::DualMetrics)
            .unwrap()
            .evidence_ref = "evidence-0".into();
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                ..
            }
        ));
        value = package();
        value.known_gaps_elided = true;
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::NamedGapHidden,
                ..
            }
        ));
        value = package();
        value.rollback_target = None;
        assert!(matches!(
            derive_promotion_gate(&value),
            PromotionGateDerivation::Blocked {
                reason: PromotionGateBlockReason::MissingRollback,
                ..
            }
        ));
    }

    #[test]
    fn every_reserved_integrity_reason_is_typed() {
        let flags = [
            ("pooling", PromotionGateBlockReason::Pooling),
            ("named_gap_hidden", PromotionGateBlockReason::NamedGapHidden),
            ("stale_identity", PromotionGateBlockReason::StaleIdentity),
            (
                "denominator_mismatch",
                PromotionGateBlockReason::DenominatorMismatch,
            ),
            ("contamination", PromotionGateBlockReason::Contamination),
            (
                "nonfinite_metric",
                PromotionGateBlockReason::NonfiniteMetric,
            ),
            (
                "producer_not_independent",
                PromotionGateBlockReason::ProducerNotIndependent,
            ),
            (
                "conservation_veto",
                PromotionGateBlockReason::ConservationVeto,
            ),
            ("invalid_order", PromotionGateBlockReason::InvalidOrder),
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
        for (name, expected) in flags {
            let mut value = package();
            value.set_integrity_flag(name);
            assert!(
                matches!(derive_promotion_gate(&value), PromotionGateDerivation::Blocked { reason, .. } if reason == expected)
            );
        }
    }

    #[test]
    fn absent_measurements_deserialize_fail_closed_and_never_qualify() {
        let mut minimal = serde_json::to_value(package()).unwrap();
        let object = minimal.as_object_mut().unwrap();
        object.remove("contamination");
        object.remove("producer_provenance");
        object.remove("measured_train_quality");
        object.remove("measured_validation_quality");
        let back: FamilyPromotionEvidence = serde_json::from_value(minimal).unwrap();
        assert_eq!(back.contamination, ContaminationAssessment::NotAssessed);
        assert!(back.producer_provenance.is_empty());
        assert!(back.measured_train_quality.is_none());
        assert!(back.measured_validation_quality.is_none());

        // Completeness ignores absent measurements (it has its own concern).
        assert!(matches!(
            derive_promotion_gate(&back),
            PromotionGateDerivation::HumanGateRequired { .. }
        ));
        // Qualification never reads absence as clean.
        assert!(matches!(
            derive_promotion_qualification(&back, PromotionQualificationPolicy::default()),
            PromotionQualificationDecision::Unqualified {
                reason: PromotionDisqualifier::ContaminationNotAssessed,
                ref measured,
            } if measured == "not_assessed"
        ));
    }
}
