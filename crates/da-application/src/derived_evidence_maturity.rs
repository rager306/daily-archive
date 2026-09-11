//! Closed, derived-only evidence maturity vocabulary.
//!
//! This module is intentionally metadata-only. It does not participate in the
//! parse workflow, policy decisions, graph writes, promotion, or import.

use serde::{Deserialize, Serialize};

/// One of the five evidence families supported by the derived protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyId {
    Segmentation,
    Header,
    Bibliography,
    Body,
    Visual,
}

impl FamilyId {
    pub const ALL: [Self; 5] = [
        Self::Segmentation,
        Self::Header,
        Self::Bibliography,
        Self::Body,
        Self::Visual,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Segmentation => "segmentation",
            Self::Header => "header",
            Self::Bibliography => "bibliography",
            Self::Body => "body",
            Self::Visual => "visual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "segmentation" => Some(Self::Segmentation),
            "header" => Some(Self::Header),
            "bibliography" => Some(Self::Bibliography),
            "body" => Some(Self::Body),
            "visual" => Some(Self::Visual),
            _ => None,
        }
    }
}

/// Ordered derived maturity ladder. There is deliberately no promoted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaturityState {
    InventoryBound,
    OracleBound,
    AtomProven,
    SequenceProven,
    ShadowObserved,
    ReviewReady,
    HumanGateRequired,
}

impl MaturityState {
    pub const ALL: [Self; 7] = [
        Self::InventoryBound,
        Self::OracleBound,
        Self::AtomProven,
        Self::SequenceProven,
        Self::ShadowObserved,
        Self::ReviewReady,
        Self::HumanGateRequired,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InventoryBound => "inventory_bound",
            Self::OracleBound => "oracle_bound",
            Self::AtomProven => "atom_proven",
            Self::SequenceProven => "sequence_proven",
            Self::ShadowObserved => "shadow_observed",
            Self::ReviewReady => "review_ready",
            Self::HumanGateRequired => "human_gate_required",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "inventory_bound" => Some(Self::InventoryBound),
            "oracle_bound" => Some(Self::OracleBound),
            "atom_proven" => Some(Self::AtomProven),
            "sequence_proven" => Some(Self::SequenceProven),
            "shadow_observed" => Some(Self::ShadowObserved),
            "review_ready" => Some(Self::ReviewReady),
            "human_gate_required" => Some(Self::HumanGateRequired),
            _ => None,
        }
    }

    pub const fn ladder_index(self) -> usize {
        match self {
            Self::InventoryBound => 0,
            Self::OracleBound => 1,
            Self::AtomProven => 2,
            Self::SequenceProven => 3,
            Self::ShadowObserved => 4,
            Self::ReviewReady => 5,
            Self::HumanGateRequired => 6,
        }
    }

    pub const fn successor(self) -> Option<Self> {
        match self {
            Self::InventoryBound => Some(Self::OracleBound),
            Self::OracleBound => Some(Self::AtomProven),
            Self::AtomProven => Some(Self::SequenceProven),
            Self::SequenceProven => Some(Self::ShadowObserved),
            Self::ShadowObserved => Some(Self::ReviewReady),
            Self::ReviewReady => Some(Self::HumanGateRequired),
            Self::HumanGateRequired => None,
        }
    }
}

/// Receipt kind corresponding to one rung of the maturity ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    Inventory,
    Oracle,
    Atom,
    Sequence,
    Shadow,
    Review,
}

impl ReceiptKind {
    pub const ALL: [Self; 6] = [
        Self::Inventory,
        Self::Oracle,
        Self::Atom,
        Self::Sequence,
        Self::Shadow,
        Self::Review,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Oracle => "oracle",
            Self::Atom => "atom",
            Self::Sequence => "sequence",
            Self::Shadow => "shadow",
            Self::Review => "review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "inventory" => Some(Self::Inventory),
            "oracle" => Some(Self::Oracle),
            "atom" => Some(Self::Atom),
            "sequence" => Some(Self::Sequence),
            "shadow" => Some(Self::Shadow),
            "review" => Some(Self::Review),
            _ => None,
        }
    }
}

/// Typed reasons why a derived state cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    MissingEvidence,
    LadderGap,
    AttemptedAutomaticPromotion,
    UnknownFamily,
    MalformedReceipt,
    StaleIdentity,
    DenominatorDrift,
    Pooling,
    Contamination,
    SemanticDrift,
    ConservationLoss,
    UnsafeProductionCall,
    MalformedPrompt,
    FailedEvidence,
}

impl BlockedReason {
    pub const ALL: [Self; 14] = [
        Self::MissingEvidence,
        Self::LadderGap,
        Self::AttemptedAutomaticPromotion,
        Self::UnknownFamily,
        Self::MalformedReceipt,
        Self::StaleIdentity,
        Self::DenominatorDrift,
        Self::Pooling,
        Self::Contamination,
        Self::SemanticDrift,
        Self::ConservationLoss,
        Self::UnsafeProductionCall,
        Self::MalformedPrompt,
        Self::FailedEvidence,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingEvidence => "missing_evidence",
            Self::LadderGap => "ladder_gap",
            Self::AttemptedAutomaticPromotion => "attempted_automatic_promotion",
            Self::UnknownFamily => "unknown_family",
            Self::MalformedReceipt => "malformed_receipt",
            Self::StaleIdentity => "stale_identity",
            Self::DenominatorDrift => "denominator_drift",
            Self::Pooling => "pooling",
            Self::Contamination => "contamination",
            Self::SemanticDrift => "semantic_drift",
            Self::ConservationLoss => "conservation_loss",
            Self::UnsafeProductionCall => "unsafe_production_call",
            Self::MalformedPrompt => "malformed_prompt",
            Self::FailedEvidence => "failed_evidence",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

/// An identity-only receipt bound to one immutable source and artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundReceipt {
    pub kind: ReceiptKind,
    pub evidence_ref: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub grobid_profile: String,
}

/// The complete, injected receipt set for one paper and one evidence family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyEvidenceSet {
    pub family: FamilyId,
    pub paper_id: String,
    #[serde(default)]
    pub inventory: Option<BoundReceipt>,
    #[serde(default)]
    pub oracle: Option<BoundReceipt>,
    #[serde(default)]
    pub atom: Option<BoundReceipt>,
    #[serde(default)]
    pub sequence: Option<BoundReceipt>,
    #[serde(default)]
    pub shadow: Option<BoundReceipt>,
    #[serde(default)]
    pub review: Option<BoundReceipt>,
    #[serde(default)]
    pub human_decision_ref: Option<String>,
    #[serde(default)]
    pub stale_identity: bool,
    #[serde(default)]
    pub denominator_drift: bool,
    #[serde(default)]
    pub pooling: bool,
    #[serde(default)]
    pub contamination: bool,
    #[serde(default)]
    pub semantic_drift: bool,
    #[serde(default)]
    pub conservation_loss: bool,
    #[serde(default)]
    pub unsafe_production_call: bool,
    #[serde(default)]
    pub failed_evidence: bool,
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
}

/// Closed diagnostic result emitted by the pure derivation function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MaturityDerivation {
    Reached {
        state: MaturityState,
        evidence_refs: Vec<String>,
    },
    Blocked {
        reason: BlockedReason,
        missing_or_offending: Option<String>,
        evidence_refs: Vec<String>,
    },
}

/// Human authorization is orthogonal to evidence maturity.
///
/// `Pending` is the safe default: a complete review package is not a human
/// decision. In particular, `human_decision_ref` is diagnostic provenance and
/// must never be interpreted as an authorization grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HumanAuthorization {
    Pending,
    Authorized { decision_ref: String },
}

/// The machine-checkable result of separating review-package readiness from
/// human authorization. A `ReviewReady` result may still be `Pending`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadiness {
    pub maturity: MaturityState,
    pub evidence_refs: Vec<String>,
    pub authorization: HumanAuthorization,
}

impl ReviewReadiness {
    /// Production/import behavior is allowed only after an explicit,
    /// non-empty human authorization. Readiness alone never grants rights.
    pub fn authorizes_production_or_import(&self) -> bool {
        self.maturity == MaturityState::ReviewReady
            && matches!(
                &self.authorization,
                HumanAuthorization::Authorized { decision_ref }
                    if !decision_ref.trim().is_empty()
            )
    }
}

/// Derive review-package completeness without conflating it with a human gate.
///
/// This is the canonical readiness surface for callers that need to decide
/// whether review work can begin. The older `derive_maturity` function keeps
/// its closed ladder and `HumanGateRequired` terminal state for wire/backward
/// compatibility; this function deliberately reports the evidence terminal as
/// `ReviewReady` and leaves authorization `Pending`.
pub fn derive_review_readiness(
    set: &FamilyEvidenceSet,
) -> Result<ReviewReadiness, MaturityDerivation> {
    match derive_maturity(set) {
        MaturityDerivation::Reached {
            state: MaturityState::HumanGateRequired,
            evidence_refs,
        } => Ok(ReviewReadiness {
            maturity: MaturityState::ReviewReady,
            evidence_refs,
            authorization: HumanAuthorization::Pending,
        }),
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } => Ok(ReviewReadiness {
            maturity: state,
            evidence_refs,
            authorization: HumanAuthorization::Pending,
        }),
        blocked @ MaturityDerivation::Blocked { .. } => Err(blocked),
    }
}

fn blocked(
    reason: BlockedReason,
    missing_or_offending: impl Into<Option<String>>,
    evidence_refs: Vec<String>,
) -> MaturityDerivation {
    MaturityDerivation::Blocked {
        reason,
        missing_or_offending: missing_or_offending.into(),
        evidence_refs,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Derive the highest contiguous maturity prefix from immutable receipts.
///
/// This function has no authority to promote, import, write, or consult a
/// runtime parse state. Explicit authority or integrity claims always win
/// over receipt presence and are returned as typed blocks.
pub fn derive_maturity(set: &FamilyEvidenceSet) -> MaturityDerivation {
    if set.claims_promoted || set.deployed || set.import_eligible {
        return blocked(
            BlockedReason::AttemptedAutomaticPromotion,
            Some("authority_claim".to_owned()),
            Vec::new(),
        );
    }

    let integrity_flags = [
        (set.pooling, BlockedReason::Pooling),
        (set.contamination, BlockedReason::Contamination),
        (set.stale_identity, BlockedReason::StaleIdentity),
        (set.denominator_drift, BlockedReason::DenominatorDrift),
        (set.conservation_loss, BlockedReason::ConservationLoss),
        (set.semantic_drift, BlockedReason::SemanticDrift),
        (
            set.unsafe_production_call,
            BlockedReason::UnsafeProductionCall,
        ),
        (set.failed_evidence, BlockedReason::FailedEvidence),
    ];
    if let Some((_, reason)) = integrity_flags.into_iter().find(|(set, _)| *set) {
        return blocked(reason, Some("integrity_flag".to_owned()), Vec::new());
    }

    if set.paper_id.trim().is_empty() {
        return blocked(
            BlockedReason::MalformedReceipt,
            Some("paper_id".to_owned()),
            Vec::new(),
        );
    }

    let slots = [
        (ReceiptKind::Inventory, "inventory", set.inventory.as_ref()),
        (ReceiptKind::Oracle, "oracle", set.oracle.as_ref()),
        (ReceiptKind::Atom, "atom", set.atom.as_ref()),
        (ReceiptKind::Sequence, "sequence", set.sequence.as_ref()),
        (ReceiptKind::Shadow, "shadow", set.shadow.as_ref()),
        (ReceiptKind::Review, "review", set.review.as_ref()),
    ];
    let mut source_sha256: Option<&str> = None;
    for (expected_kind, name, receipt) in slots
        .iter()
        .filter_map(|(kind, name, receipt)| receipt.map(|receipt| (*kind, *name, receipt)))
    {
        if receipt.kind != expected_kind {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.kind")),
                Vec::new(),
            );
        }
        if receipt.evidence_ref.trim().is_empty() {
            return blocked(
                BlockedReason::MissingEvidence,
                Some(format!("{name}.evidence_ref")),
                Vec::new(),
            );
        }
        if !valid_digest(&receipt.source_sha256) {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.source_sha256")),
                Vec::new(),
            );
        }
        if !valid_digest(&receipt.artifact_sha256) {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.artifact_sha256")),
                Vec::new(),
            );
        }
        if receipt.grobid_profile != "structural" && receipt.grobid_profile != "rich" {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.grobid_profile")),
                Vec::new(),
            );
        }
        if let Some(source) = source_sha256 {
            if source != receipt.source_sha256 {
                return blocked(
                    BlockedReason::MalformedReceipt,
                    Some(format!("{name}.source_sha256")),
                    Vec::new(),
                );
            }
        } else {
            source_sha256 = Some(&receipt.source_sha256);
        }
    }

    let first_gap = slots.iter().position(|(_, _, receipt)| receipt.is_none());
    let contiguous = slots.iter().take_while(|(_, _, receipt)| receipt.is_some());
    let evidence_refs: Vec<String> = contiguous
        .map(|(_, _, receipt)| {
            receipt
                .expect("take_while guarantees receipt")
                .evidence_ref
                .clone()
        })
        .collect();
    if let Some(index) = first_gap
        && slots[index + 1..]
            .iter()
            .any(|(_, _, later)| later.is_some())
    {
        return blocked(
            BlockedReason::LadderGap,
            Some(slots[index].1.to_owned()),
            evidence_refs,
        );
    }
    if evidence_refs.is_empty() {
        blocked(
            BlockedReason::MissingEvidence,
            Some(
                first_gap
                    .map(|index| slots[index].1.to_owned())
                    .unwrap_or_else(|| "inventory".to_owned()),
            ),
            evidence_refs,
        )
    } else {
        MaturityDerivation::Reached {
            state: if evidence_refs.len() == ReceiptKind::ALL.len() {
                MaturityState::HumanGateRequired
            } else {
                MaturityState::ALL[evidence_refs.len() - 1]
            },
            evidence_refs,
        }
    }
}

/// True only for one immediate forward step on the closed ladder.
pub fn can_advance(from: MaturityState, to: MaturityState) -> bool {
    match from.successor() {
        Some(next) => next.ladder_index() == to.ladder_index(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_wire_vocabularies_have_exact_sizes_and_names() {
        assert_eq!(FamilyId::ALL.len(), 5);
        assert_eq!(MaturityState::ALL.len(), 7);
        assert_eq!(ReceiptKind::ALL.len(), 6);
        assert_eq!(BlockedReason::ALL.len(), 14);
        assert_eq!(
            MaturityState::ALL.map(MaturityState::as_str),
            [
                "inventory_bound",
                "oracle_bound",
                "atom_proven",
                "sequence_proven",
                "shadow_observed",
                "review_ready",
                "human_gate_required",
            ]
        );
        assert!(MaturityState::parse("promoted").is_none());
    }

    #[test]
    fn all_wire_values_roundtrip_and_unknown_values_are_rejected() {
        for family in FamilyId::ALL {
            assert_eq!(FamilyId::parse(family.as_str()), Some(family));
        }
        for state in MaturityState::ALL {
            assert_eq!(MaturityState::parse(state.as_str()), Some(state));
        }
        for kind in ReceiptKind::ALL {
            assert_eq!(ReceiptKind::parse(kind.as_str()), Some(kind));
        }
        for reason in BlockedReason::ALL {
            assert_eq!(BlockedReason::parse(reason.as_str()), Some(reason));
        }
        assert!(FamilyId::parse("lattice").is_none());
        assert!(MaturityState::parse("future_state").is_none());
        assert!(ReceiptKind::parse("decision").is_none());
        assert!(BlockedReason::parse("invented_reason").is_none());
    }

    #[test]
    fn derivation_is_tagged_and_roundtrips() {
        let values = [
            MaturityDerivation::Reached {
                state: MaturityState::ReviewReady,
                evidence_refs: vec!["receipt-a".into()],
            },
            MaturityDerivation::Blocked {
                reason: BlockedReason::LadderGap,
                missing_or_offending: Some("atom".into()),
                evidence_refs: vec!["receipt-a".into()],
            },
        ];
        for value in values {
            let json = serde_json::to_value(&value).unwrap();
            let back: MaturityDerivation = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(value, back);
            assert!(
                json.get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
            );
        }
    }

    #[test]
    fn every_ladder_step_has_only_its_immediate_successor() {
        for (index, state) in MaturityState::ALL.iter().copied().enumerate() {
            let successor = state.successor();
            if let Some(next) = successor {
                assert_eq!(next.ladder_index(), index + 1);
                assert!(can_advance(state, next));
            } else {
                assert_eq!(index, MaturityState::ALL.len() - 1);
            }
            for target in MaturityState::ALL {
                assert_eq!(can_advance(state, target), successor == Some(target));
            }
        }
    }

    #[test]
    fn self_backward_and_skipped_transitions_are_rejected() {
        for (index, state) in MaturityState::ALL.iter().copied().enumerate() {
            assert!(!can_advance(state, state));
            if index > 0 {
                assert!(!can_advance(state, MaturityState::ALL[index - 1]));
            }
            if index + 2 < MaturityState::ALL.len() {
                assert!(!can_advance(state, MaturityState::ALL[index + 2]));
            }
        }
        assert!(!can_advance(
            MaturityState::HumanGateRequired,
            MaturityState::InventoryBound
        ));
    }

    fn receipt(kind: ReceiptKind, reference: &str) -> BoundReceipt {
        BoundReceipt {
            kind,
            evidence_ref: reference.into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
        }
    }

    fn set() -> FamilyEvidenceSet {
        FamilyEvidenceSet {
            family: FamilyId::Body,
            paper_id: "paper-1".into(),
            inventory: None,
            oracle: None,
            atom: None,
            sequence: None,
            shadow: None,
            review: None,
            human_decision_ref: None,
            stale_identity: false,
            denominator_drift: false,
            pooling: false,
            contamination: false,
            semantic_drift: false,
            conservation_loss: false,
            unsafe_production_call: false,
            failed_evidence: false,
            claims_promoted: false,
            deployed: false,
            import_eligible: false,
        }
    }

    fn reached(result: MaturityDerivation) -> (MaturityState, Vec<String>) {
        match result {
            MaturityDerivation::Reached {
                state,
                evidence_refs,
            } => (state, evidence_refs),
            other => panic!("expected reached result, got {other:?}"),
        }
    }

    fn blocked_reason(result: MaturityDerivation) -> BlockedReason {
        match result {
            MaturityDerivation::Blocked { reason, .. } => reason,
            other => panic!("expected blocked result, got {other:?}"),
        }
    }

    #[test]
    fn every_contiguous_prefix_derives_exact_state_and_ordered_refs() {
        let kinds = ReceiptKind::ALL;
        for prefix in 1..=kinds.len() {
            let mut family = set();
            for (index, kind) in kinds.into_iter().enumerate().take(prefix) {
                let value = Some(receipt(kind, &format!("ref-{index}")));
                match kind {
                    ReceiptKind::Inventory => family.inventory = value,
                    ReceiptKind::Oracle => family.oracle = value,
                    ReceiptKind::Atom => family.atom = value,
                    ReceiptKind::Sequence => family.sequence = value,
                    ReceiptKind::Shadow => family.shadow = value,
                    ReceiptKind::Review => family.review = value,
                }
            }
            let (state, refs) = reached(derive_maturity(&family));
            let expected_state = if prefix == kinds.len() {
                MaturityState::HumanGateRequired
            } else {
                MaturityState::ALL[prefix - 1]
            };
            assert_eq!(state, expected_state);
            assert_eq!(
                refs,
                (0..prefix).map(|i| format!("ref-{i}")).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn empty_and_sparse_sets_fail_closed() {
        let mut family = set();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MissingEvidence
        );
        family.shadow = Some(receipt(ReceiptKind::Shadow, "shadow-ref"));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::LadderGap
        );
        match derive_maturity(&family) {
            MaturityDerivation::Blocked { evidence_refs, .. } => assert!(evidence_refs.is_empty()),
            other => panic!("expected block, got {other:?}"),
        }
    }

    #[test]
    fn authority_and_integrity_flags_have_fixed_priority() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));
        family.failed_evidence = true;
        family.pooling = true;
        family.claims_promoted = true;
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::AttemptedAutomaticPromotion
        );
        family.claims_promoted = false;
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::Pooling
        );
    }

    #[test]
    fn malformed_receipts_never_downgrade_to_a_reached_state() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Oracle, "wrong-kind"));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, ""));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MissingEvidence
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, "ref"));
        family.inventory.as_mut().unwrap().source_sha256 = "bad".into();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, "ref"));
        family.oracle = Some(receipt(ReceiptKind::Oracle, "oracle-ref"));
        family.oracle.as_mut().unwrap().source_sha256 = "c".repeat(64);
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.oracle.as_mut().unwrap().source_sha256 = "a".repeat(64);
        family.oracle.as_mut().unwrap().grobid_profile = "unknown".into();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
    }

    #[test]
    fn fsm_readiness_authorization_keeps_review_ready_pending() {
        let mut family = set();
        for (slot, kind) in [
            (&mut family.inventory, ReceiptKind::Inventory),
            (&mut family.oracle, ReceiptKind::Oracle),
            (&mut family.atom, ReceiptKind::Atom),
            (&mut family.sequence, ReceiptKind::Sequence),
            (&mut family.shadow, ReceiptKind::Shadow),
            (&mut family.review, ReceiptKind::Review),
        ] {
            *slot = Some(receipt(kind, kind.as_str()));
        }
        family.human_decision_ref = Some("diagnostic-only-ref".into());

        let readiness =
            derive_review_readiness(&family).expect("complete evidence is review-ready");
        assert_eq!(readiness.maturity, MaturityState::ReviewReady);
        assert_eq!(readiness.evidence_refs.len(), ReceiptKind::ALL.len());
        assert_eq!(readiness.authorization, HumanAuthorization::Pending);
        assert!(!readiness.authorizes_production_or_import());

        let authorized = ReviewReadiness {
            authorization: HumanAuthorization::Authorized {
                decision_ref: "human-approval-1".into(),
            },
            ..readiness.clone()
        };
        assert!(authorized.authorizes_production_or_import());
        assert!(
            !ReviewReadiness {
                authorization: HumanAuthorization::Authorized {
                    decision_ref: "  ".into(),
                },
                ..readiness
            }
            .authorizes_production_or_import()
        );
    }

    #[test]
    fn incomplete_evidence_is_not_review_ready_and_human_gate_is_not_granted() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));
        let readiness = derive_review_readiness(&family).expect("inventory is a valid prefix");
        assert_eq!(readiness.maturity, MaturityState::InventoryBound);
        assert_eq!(readiness.authorization, HumanAuthorization::Pending);
        assert!(!readiness.authorizes_production_or_import());

        family.review = Some(receipt(ReceiptKind::Review, "review-ref"));
        assert!(matches!(
            derive_review_readiness(&family),
            Err(MaturityDerivation::Blocked {
                reason: BlockedReason::LadderGap,
                ..
            })
        ));
    }

    #[test]
    fn human_decision_is_diagnostic_only_and_containers_are_strict() {
        let mut family = set();
        family.human_decision_ref = Some("human-1".into());
        for (slot, kind) in [
            (&mut family.inventory, ReceiptKind::Inventory),
            (&mut family.oracle, ReceiptKind::Oracle),
            (&mut family.atom, ReceiptKind::Atom),
            (&mut family.sequence, ReceiptKind::Sequence),
            (&mut family.shadow, ReceiptKind::Shadow),
            (&mut family.review, ReceiptKind::Review),
        ] {
            *slot = Some(receipt(kind, kind.as_str()));
        }
        assert_eq!(
            reached(derive_maturity(&family)).0,
            MaturityState::HumanGateRequired
        );
        let mut json = serde_json::to_value(&family).unwrap();
        json["unexpected"] = true.into();
        assert!(serde_json::from_value::<FamilyEvidenceSet>(json).is_err());
    }
}
