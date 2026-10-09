//! Pure ACAP admission policy value (ADR-055 / S07).
//!
//! The runtime ACAP admission semantics are declared once in DATA: the ordered
//! `fsm.states` list plus the top-level `admission` block of
//! `scripts/governor/checks/article_context_assurance.yaml`. This module holds
//! the pure, I/O-free value the runtime executes, plus the fail-closed
//! constructor that binds such a declaration to it. The governor reads the
//! same file, and the shared parity vector fixture
//! (`governor/fixtures/acap_runtime_control_plane/admission_parity.vectors.json`)
//! is the agreement oracle between the two readers.
//!
//! No I/O and no YAML parsing live here: `da-adapters::acap_policy` loads the
//! DATA into this value (R138 onion — infrastructure loads, domain decides).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::assurance::ArticleContextState;

/// The canonical ACAP FSM layer order, verbatim from
/// `article_context_assurance.yaml#fsm.states`. Used by
/// [`AcapAdmissionPolicy::canonical`] and to reject a declaration that omits a
/// state. A declaration may permute the order (rank comes from the declaration)
/// but may not drop a state.
pub const ACAP_FSM_ORDER: [ArticleContextState; 15] = [
    ArticleContextState::SourceRegistered,
    ArticleContextState::SourceVerified,
    ArticleContextState::Parsed,
    ArticleContextState::Structured,
    ArticleContextState::Extracted,
    ArticleContextState::EvidenceLinked,
    ArticleContextState::Embedded,
    ArticleContextState::Materialized,
    ArticleContextState::Enriched,
    ArticleContextState::Resolved,
    ArticleContextState::Summarized,
    ArticleContextState::Finalized,
    ArticleContextState::TaskAccepted,
    ArticleContextState::AcceptedCheckpoint,
    ArticleContextState::FailedVisible,
];

/// Forward-progress rule declared by the `admission.forward_rule` DATA field.
///
/// Only [`AcapForwardRule::MonotoneSubset`] is declared today. Additional rules
/// are deliberately not defined here: the governor's admission rule
/// (`_acap_admission_violations`) must implement the same rule, so inventing a
/// rule the other reader does not execute would create a second truth. A new
/// rule arrives together with its governor implementation and a parity vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcapForwardRule {
    /// A gated stage may establish any ACAP rank at or after the prior rank, so
    /// forward layer skips are legal (with a typed reason when the declaration
    /// requires one).
    MonotoneSubset,
}

impl std::fmt::Display for AcapForwardRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MonotoneSubset => f.write_str("monotone_subset"),
        }
    }
}

/// Pure ACAP admission policy: the declared state order plus the declared
/// legality rules the runtime executes.
///
/// Fields stay private so the `order`/`rank` pair can never disagree; build one
/// with [`AcapAdmissionPolicy::new`] (fail closed) or
/// [`AcapAdmissionPolicy::canonical`] (the pinned default). No I/O, no graph
/// truth — a serializable decision value is derived from it elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcapAdmissionPolicy {
    order: Vec<ArticleContextState>,
    rank: BTreeMap<ArticleContextState, u8>,
    entry_states: BTreeSet<ArticleContextState>,
    failure_sink: ArticleContextState,
    forward_rule: AcapForwardRule,
    same_layer_repeat: bool,
    evidence_required: bool,
    skip_requires_typed_reason: bool,
}

impl AcapAdmissionPolicy {
    /// Bind a declaration to a policy, failing closed on anything inconsistent.
    ///
    /// Rejected: an empty `order`; a duplicate state in `order`; an `order` that
    /// omits any [`ArticleContextState`]; an empty `entry_states`; an entry
    /// state or the `failure_sink` that is not in `order`.
    pub fn new(
        order: Vec<ArticleContextState>,
        entry_states: BTreeSet<ArticleContextState>,
        failure_sink: ArticleContextState,
        forward_rule: AcapForwardRule,
        same_layer_repeat: bool,
        evidence_required: bool,
        skip_requires_typed_reason: bool,
    ) -> Result<Self, String> {
        if order.is_empty() {
            return Err("ACAP admission policy order must not be empty".to_string());
        }
        let mut rank: BTreeMap<ArticleContextState, u8> = BTreeMap::new();
        for (index, state) in order.iter().enumerate() {
            if rank.insert(*state, index as u8).is_some() {
                return Err(format!(
                    "ACAP admission policy order contains duplicate state {state:?}"
                ));
            }
        }
        // The declaration must list every ACAP state: a partial order would make
        // rank() answer None for a state a stage could legitimately establish.
        for state in ACAP_FSM_ORDER {
            if !rank.contains_key(&state) {
                return Err(format!(
                    "ACAP admission policy order omits declared state {state:?}"
                ));
            }
        }
        if entry_states.is_empty() {
            return Err("ACAP admission policy must declare at least one entry state".to_string());
        }
        for state in &entry_states {
            if !rank.contains_key(state) {
                return Err(format!(
                    "ACAP entry state {state:?} is not in the declared state order"
                ));
            }
        }
        if !rank.contains_key(&failure_sink) {
            return Err(format!(
                "ACAP failure sink {failure_sink:?} is not in the declared state order"
            ));
        }
        Ok(Self {
            order,
            rank,
            entry_states,
            failure_sink,
            forward_rule,
            same_layer_repeat,
            evidence_required,
            skip_requires_typed_reason,
        })
    }

    /// The pinned default: exactly the DATA semantics (`fsm.states` order +
    /// the `admission` block). The shared parity vectors are asserted against
    /// this value and against the DATA-loaded policy so the two cannot drift.
    pub fn canonical() -> Self {
        Self::new(
            ACAP_FSM_ORDER.to_vec(),
            [
                ArticleContextState::SourceRegistered,
                ArticleContextState::SourceVerified,
                ArticleContextState::Parsed,
            ]
            .into_iter()
            .collect(),
            ArticleContextState::FailedVisible,
            AcapForwardRule::MonotoneSubset,
            true,
            true,
            true,
        )
        .expect("the canonical ACAP admission policy is a valid declaration")
    }

    /// Declared state order (rank 0 first).
    pub fn order(&self) -> &[ArticleContextState] {
        &self.order
    }

    /// Declared entry states: the only states a run may start from.
    pub fn entry_states(&self) -> &BTreeSet<ArticleContextState> {
        &self.entry_states
    }

    /// Declared failure sink: reachable from any state, terminal once reached.
    pub fn failure_sink(&self) -> ArticleContextState {
        self.failure_sink
    }

    /// Declared forward-progress rule.
    pub fn forward_rule(&self) -> AcapForwardRule {
        self.forward_rule
    }

    /// Whether a same-layer repeat (target rank == prior rank) is admitted.
    pub fn same_layer_repeat(&self) -> bool {
        self.same_layer_repeat
    }

    /// Whether admission requires a non-empty durable evidence reference.
    pub fn evidence_required(&self) -> bool {
        self.evidence_required
    }

    /// Whether an admitted forward layer skip requires a typed skip reason.
    pub fn skip_requires_typed_reason(&self) -> bool {
        self.skip_requires_typed_reason
    }

    /// Declared rank of a state, or `None` when the state is not in the order
    /// (only reachable for a policy that was constructed outside `new`).
    pub fn rank(&self, state: ArticleContextState) -> Option<u8> {
        self.rank.get(&state).copied()
    }

    /// A forward layer skip: the target rank is strictly more than one layer
    /// ahead of the prior rank. Pure rank predicate — it says nothing about
    /// whether the skip is *admitted* (that is
    /// `admit_stage_transition_with_policy`) and deliberately treats the
    /// failure sink like any other ranked state; callers that care exclude the
    /// sink themselves.
    pub fn is_forward_skip(&self, prior: ArticleContextState, target: ArticleContextState) -> bool {
        match (self.rank(prior), self.rank(target)) {
            (Some(prior_rank), Some(target_rank)) => target_rank > prior_rank.saturating_add(1),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance::ArticleContextState;

    fn entries(states: [ArticleContextState; 3]) -> BTreeSet<ArticleContextState> {
        states.into_iter().collect()
    }

    fn canonical_entries() -> BTreeSet<ArticleContextState> {
        entries([
            ArticleContextState::SourceRegistered,
            ArticleContextState::SourceVerified,
            ArticleContextState::Parsed,
        ])
    }

    fn build(order: Vec<ArticleContextState>) -> Result<AcapAdmissionPolicy, String> {
        AcapAdmissionPolicy::new(
            order,
            canonical_entries(),
            ArticleContextState::FailedVisible,
            AcapForwardRule::MonotoneSubset,
            true,
            true,
            true,
        )
    }

    #[test]
    fn acap_canonical_policy_binds_the_data_declaration() {
        let policy = AcapAdmissionPolicy::canonical();
        assert_eq!(policy.order().len(), 15);
        assert_eq!(policy.rank(ArticleContextState::SourceRegistered), Some(0));
        assert_eq!(policy.rank(ArticleContextState::Parsed), Some(2));
        assert_eq!(policy.rank(ArticleContextState::Summarized), Some(10));
        assert_eq!(policy.rank(ArticleContextState::FailedVisible), Some(14));
        assert_eq!(
            policy.entry_states(),
            &entries([
                ArticleContextState::SourceRegistered,
                ArticleContextState::SourceVerified,
                ArticleContextState::Parsed,
            ])
        );
        assert_eq!(policy.failure_sink(), ArticleContextState::FailedVisible);
        assert_eq!(policy.forward_rule(), AcapForwardRule::MonotoneSubset);
        assert!(policy.same_layer_repeat());
        assert!(policy.evidence_required());
        assert!(policy.skip_requires_typed_reason());
    }

    #[test]
    fn acap_forward_skip_is_rank_delta_greater_than_one() {
        let policy = AcapAdmissionPolicy::canonical();
        use ArticleContextState::*;
        // parsed(2) -> summarized(10) skips layers.
        assert!(policy.is_forward_skip(Parsed, Summarized));
        // adjacent forward move is not a skip.
        assert!(!policy.is_forward_skip(Parsed, Structured));
        // same layer is not a skip.
        assert!(!policy.is_forward_skip(Parsed, Parsed));
        // regression is not a forward skip.
        assert!(!policy.is_forward_skip(Summarized, Parsed));
        // the failure sink is ranked; is_forward_skip is a pure rank predicate.
        assert!(policy.is_forward_skip(Parsed, FailedVisible));
    }

    #[test]
    fn acap_new_rejects_empty_order() {
        let error = build(vec![]).expect_err("empty order must fail closed");
        assert!(error.contains("must not be empty"), "{error}");
    }

    #[test]
    fn acap_new_rejects_duplicate_states() {
        let mut order = ACAP_FSM_ORDER.to_vec();
        order.push(ArticleContextState::Parsed);
        let error = build(order).expect_err("duplicate state must fail closed");
        assert!(error.contains("duplicate state"), "{error}");
    }

    #[test]
    fn acap_new_rejects_order_that_omits_a_state() {
        let order = ACAP_FSM_ORDER
            .iter()
            .copied()
            .filter(|state| *state != ArticleContextState::Summarized)
            .collect();
        let error = build(order).expect_err("an omitted state must fail closed");
        assert!(error.contains("omits declared state"), "{error}");
    }

    #[test]
    fn acap_new_rejects_failure_sink_absent_from_order() {
        // Drop the sink, then ask for it as the sink: the totality check fires
        // first, which is still the fail-closed outcome.
        let order = ACAP_FSM_ORDER
            .iter()
            .copied()
            .filter(|state| *state != ArticleContextState::FailedVisible)
            .collect();
        assert!(build(order).is_err());
    }

    #[test]
    fn acap_new_rejects_entry_state_absent_from_order() {
        let order = ACAP_FSM_ORDER
            .iter()
            .copied()
            .filter(|state| *state != ArticleContextState::Parsed)
            .collect();
        assert!(build(order).is_err());
    }

    #[test]
    fn acap_new_rejects_empty_entry_states() {
        let error = AcapAdmissionPolicy::new(
            ACAP_FSM_ORDER.to_vec(),
            BTreeSet::new(),
            ArticleContextState::FailedVisible,
            AcapForwardRule::MonotoneSubset,
            true,
            true,
            true,
        )
        .expect_err("no entry state must fail closed");
        assert!(error.contains("at least one entry state"), "{error}");
    }

    #[test]
    fn acap_new_accepts_a_permuted_order() {
        // Rank comes from the declaration; a permutation is legal as long as no
        // state is dropped. This mirrors the loader binding arbitrary DATA.
        let mut order = ACAP_FSM_ORDER.to_vec();
        order.swap(2, 10);
        let policy = build(order).expect("a complete permutation is valid");
        assert_eq!(policy.rank(ArticleContextState::Summarized), Some(2));
        assert_eq!(policy.rank(ArticleContextState::Parsed), Some(10));
        // The entry set and sink are still members of the reordered declaration.
        assert!(policy.entry_states().contains(&ArticleContextState::Parsed));
    }

    #[test]
    fn acap_forward_rule_serializes_as_the_data_token() {
        assert_eq!(
            serde_json::to_string(&AcapForwardRule::MonotoneSubset).unwrap(),
            "\"monotone_subset\""
        );
        assert_eq!(
            AcapForwardRule::MonotoneSubset.to_string(),
            "monotone_subset"
        );
        let parsed: AcapForwardRule = serde_yaml::from_str("monotone_subset").unwrap();
        assert_eq!(parsed, AcapForwardRule::MonotoneSubset);
        assert!(serde_yaml::from_str::<AcapForwardRule>("strict_adjacent").is_err());
    }
}
