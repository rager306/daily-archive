//! Closed, derived-only evidence maturity vocabulary.
//!
//! This module is intentionally metadata-only. It does not participate in the
//! parse workflow, policy decisions, graph writes, promotion, or import.

use serde::{Deserialize, Serialize};

/// One of the five evidence families supported by the derived protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Closed diagnostic result emitted by a future pure derivation function.
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
}
