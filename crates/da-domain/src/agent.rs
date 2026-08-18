//! Read-only scientific agent FSM value types (R095, domain half).
//!
//! Pure `da-domain` state machine for a plan → read → synthesize → verify
//! loop against an identity-checked graph read plane. These types perform no
//! I/O, never write graph truth, and carry no write-authority field (D127 /
//! D238 discipline): admission guards are fail-closed and typed, so illegal
//! skips, missing entry state, empty evidence, budget overflow, and identity
//! mismatch enter a typed visible failure ([`AgentState::FailedVisible`] +
//! [`AgentFailureCode`]) instead of a silent default or empty result (R087).
//!
//! # State set (roadmap-binding)
//!
//! ```text
//! Created → Planning → Searching → Reading → Synthesis → Verification → Verified
//!                                                              ↘ (any state) → FailedVisible
//! ```
//!
//! [`AgentState::can_transition_to`] admits only adjacent forward steps or the
//! failure sink; [`AgentState::FailedVisible`] is a terminal sink (nothing
//! leaves it). The surface mirrors the ACAP admission pattern in
//! `assurance.rs` ([`ArticleContextState`]) but with agent-specific failure
//! codes so `BudgetOverflow` / `IdentityMismatch` stay visible.
//!
//! # ADR-039 forward extension
//!
//! ADR-039's binding SymFSM refines the verify step as `SYNTHESIS → REVIEW
//! (fail-closed, includes VERIFY checks) → OUTPUT`. This slice implements the
//! **roadmap state names exactly** (`Verification → Verified`) and deliberately
//! does **not** add `REVIEW` or `OUTPUT` states: `Verification` models the
//! VERIFY checks (structural/citation answer-to-evidence grounding), and
//! `REVIEW` is a documented forward-extension point for graph-readiness
//! post-checks and multi-agent swarm waves (S05/future). Do not add REVIEW here.
//!
//! # Identity and evidence
//!
//! `GraphIdentity` (T02) is a domain-local value (semantic digest + tenant +
//! node/edge counts) so the S03 port layer can map `kg-storage
//! SnapshotIdentity` onto it without inverting the onion; `da-domain` has no
//! `kg-storage` dependency. `AgentQueryEvidence` (T03) records truncation
//! explicitly (`truncated: bool`) — never implied by empty rows (R087).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Agent FSM state set (roadmap-binding, see module doc).
///
/// 7 forward states + the terminal failure sink `FailedVisible`. The failure
/// sink is reachable from any state but nothing transitions out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Created,
    Planning,
    Searching,
    Reading,
    Synthesis,
    Verification,
    Verified,
    FailedVisible,
}

impl AgentState {
    /// Declared forward transitions only, plus the failure sink.
    ///
    /// Admitted pairs are the adjacent roadmap steps `Created → Planning →
    /// Searching → Reading → Synthesis → Verification → Verified`, or any
    /// transition whose target is `FailedVisible`. Everything else (skips,
    /// backward steps, self-transitions, anything out of `FailedVisible`)
    /// is `false`.
    pub fn can_transition_to(self, target: Self) -> bool {
        use AgentState::*;
        target == FailedVisible
            || matches!(
                (self, target),
                (Created, Planning)
                    | (Planning, Searching)
                    | (Searching, Reading)
                    | (Reading, Synthesis)
                    | (Synthesis, Verification)
                    | (Verification, Verified)
            )
    }

    /// Fail-closed agent admission decision for a transition from `self` to
    /// `target` (mirrors `ArticleContextState::admit_transition`).
    ///
    /// Blocked (in priority order) when:
    /// - the admission carries no durable evidence reference →
    ///   [`AgentFailureCode::MissingEvidence`];
    /// - `self` is `FailedVisible`, the terminal sink →
    ///   [`AgentFailureCode::TerminalSink`];
    /// - the pair is not a declared forward transition nor the failure sink →
    ///   [`AgentFailureCode::IllegalTransition`].
    ///
    /// Pure domain value — no I/O, no graph truth. The orchestrator passes a
    /// durable record (result hash / manifest key / checksum) as
    /// `evidence_ref`, never a caller-supplied verbatim string.
    pub fn admit_transition(self, target: Self, evidence_ref: &str) -> AgentAdmissionDecision {
        if evidence_ref.trim().is_empty() {
            return AgentAdmissionDecision::blocked(AgentAdmissionError::new(
                AgentFailureCode::MissingEvidence,
                "admission requires a durable evidence reference (result hash / manifest key / checksum)",
                evidence_ref.to_string(),
                self,
                target,
            ));
        }
        if self == Self::FailedVisible {
            return AgentAdmissionDecision::blocked(AgentAdmissionError::new(
                AgentFailureCode::TerminalSink,
                "prior state is FailedVisible, a terminal agent sink: no state may follow a failure disposition",
                evidence_ref.to_string(),
                self,
                target,
            ));
        }
        if target == Self::FailedVisible || self.can_transition_to(target) {
            AgentAdmissionDecision::admitted(evidence_ref.to_string())
        } else {
            AgentAdmissionDecision::blocked(AgentAdmissionError::new(
                AgentFailureCode::IllegalTransition,
                format!(
                    "{self:?} -> {target:?} is not a declared agent forward transition (target is not the failure sink)"
                ),
                evidence_ref.to_string(),
                self,
                target,
            ))
        }
    }
}

/// Stable machine-readable agent failure codes (R087).
///
/// Part of the report/log surface: [`AgentAdmissionError::error_class`]
/// renders the stable token `agent_failure:<code>` that run reports and logs
/// carry. Codes are agent-specific (not a reuse of
/// `TransitionAdmissionCode`) so `BudgetOverflow` and `IdentityMismatch`
/// stay visible on the agent line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentFailureCode {
    /// `prior -> target` is not a declared agent forward transition and
    /// target is not the failure sink `FailedVisible`.
    IllegalTransition,
    /// The run has no prior agent state (first step without entry evidence).
    /// Runs must start from a defined entry state with durable evidence.
    MissingEntryState,
    /// The prior state is `FailedVisible`, a terminal agent sink: no state
    /// may follow a failure disposition.
    TerminalSink,
    /// Admission was requested without a durable evidence reference.
    MissingEvidence,
    /// The agent budget (queries / evidence entries / synthesis chars) is
    /// exhausted; the refusal happens before counters advance (fail-closed).
    BudgetOverflow,
    /// Expected vs actual read-plane identity differ (semantic digest,
    /// tenant, or node/edge counts); both sides are named in the reason.
    IdentityMismatch,
}

impl AgentFailureCode {
    /// Stable wire/log token (matches the serde snake_case spelling).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IllegalTransition => "illegal_transition",
            Self::MissingEntryState => "missing_entry_state",
            Self::TerminalSink => "terminal_sink",
            Self::MissingEvidence => "missing_evidence",
            Self::BudgetOverflow => "budget_overflow",
            Self::IdentityMismatch => "identity_mismatch",
        }
    }
}

impl std::fmt::Display for AgentFailureCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed agent runtime admission refusal (R087).
///
/// Pure domain value: stable code + human/typed reason + durable evidence
/// reference + both transition endpoints. No I/O. Emitted into run reports
/// and logs. The `evidence_ref` must name a durable record produced by the
/// orchestrator itself, never a caller-supplied string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("agent transition {from:?} -> {to:?} not admitted ({code}): {reason}")]
pub struct AgentAdmissionError {
    pub code: AgentFailureCode,
    pub reason: String,
    pub evidence_ref: String,
    pub from: AgentState,
    pub to: AgentState,
}

impl AgentAdmissionError {
    pub fn new(
        code: AgentFailureCode,
        reason: impl Into<String>,
        evidence_ref: impl Into<String>,
        from: AgentState,
        to: AgentState,
    ) -> Self {
        Self {
            code,
            reason: reason.into(),
            evidence_ref: evidence_ref.into(),
            from,
            to,
        }
    }

    /// Stable error-class token for reports and logs: `agent_failure:<code>`.
    pub fn error_class(&self) -> String {
        format!("agent_failure:{}", self.code.as_str())
    }
}

/// Agent runtime admission decision for one FSM transition (R095).
///
/// Computed from a prior agent state and the target state. Fail-closed: a
/// blocked decision carries the typed [`AgentAdmissionError`] and the
/// executor must not proceed. Pure value — serializable into run reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAdmissionDecision {
    pub admitted: bool,
    /// Durable evidence reference backing this decision. For admitted
    /// transitions it names the record establishing the prior state; for
    /// blocked transitions the record the refusal is based on.
    pub evidence_ref: String,
    /// Typed refusal when `admitted == false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentAdmissionError>,
}

impl AgentAdmissionDecision {
    pub fn admitted(evidence_ref: impl Into<String>) -> Self {
        Self {
            admitted: true,
            evidence_ref: evidence_ref.into(),
            error: None,
        }
    }

    pub fn blocked(error: AgentAdmissionError) -> Self {
        let evidence_ref = error.evidence_ref.clone();
        Self {
            admitted: false,
            evidence_ref,
            error: Some(error),
        }
    }
}

/// Agent runtime admission for one FSM transition (R095), including the
/// run-entry case.
///
/// `prior` is the established agent state before the step runs. `None` means
/// the run provides no prior state (first step without entry evidence):
/// fail-closed — [`AgentFailureCode::MissingEntryState`]. The orchestrator
/// must supply an explicit entry state derived from durable source evidence.
pub fn admit_transition(
    prior: Option<AgentState>,
    target: AgentState,
    evidence_ref: &str,
) -> AgentAdmissionDecision {
    match prior {
        Some(prior) => prior.admit_transition(target, evidence_ref),
        None => AgentAdmissionDecision::blocked(AgentAdmissionError::new(
            AgentFailureCode::MissingEntryState,
            "no prior agent state: the run must start from a defined entry state with durable evidence",
            evidence_ref.to_string(),
            AgentState::Created,
            target,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const ALL_STATES: [AgentState; 8] = [
        AgentState::Created,
        AgentState::Planning,
        AgentState::Searching,
        AgentState::Reading,
        AgentState::Synthesis,
        AgentState::Verification,
        AgentState::Verified,
        AgentState::FailedVisible,
    ];

    // ── FSM topology (T01) ──

    #[test]
    fn legal_created_to_verified_sequence_admits_each_step_with_evidence() {
        // Demo: the full legal walk carries a non-empty evidence ref per step.
        let steps = [
            (AgentState::Created, AgentState::Planning),
            (AgentState::Planning, AgentState::Searching),
            (AgentState::Searching, AgentState::Reading),
            (AgentState::Reading, AgentState::Synthesis),
            (AgentState::Synthesis, AgentState::Verification),
            (AgentState::Verification, AgentState::Verified),
        ];
        for (i, (from, to)) in steps.iter().enumerate() {
            let decision = from.admit_transition(*to, &format!("ev-step-{i}"));
            assert!(decision.admitted, "{from:?} -> {to:?} should admit");
            assert_eq!(decision.evidence_ref, format!("ev-step-{i}"));
            assert!(decision.error.is_none());
        }
    }

    #[test]
    fn exhaustive_eight_by_eight_transition_matrix() {
        // Every pair is decided exactly as the declared topology says.
        for from in ALL_STATES {
            for to in ALL_STATES {
                let decision = from.admit_transition(to, "ev-1");
                // Terminal sink: the sink pair itself is refused out of
                // FailedVisible (admit_transition guards before topology).
                let expected = (to == AgentState::FailedVisible
                    && from != AgentState::FailedVisible)
                    || matches!(
                        (from, to),
                        (AgentState::Created, AgentState::Planning)
                            | (AgentState::Planning, AgentState::Searching)
                            | (AgentState::Searching, AgentState::Reading)
                            | (AgentState::Reading, AgentState::Synthesis)
                            | (AgentState::Synthesis, AgentState::Verification)
                            | (AgentState::Verification, AgentState::Verified)
                    );
                assert_eq!(
                    decision.admitted, expected,
                    "admission mismatch for {from:?} -> {to:?}"
                );
                if expected {
                    assert!(decision.error.is_none(), "{from:?} -> {to:?}");
                    assert_eq!(decision.evidence_ref, "ev-1");
                } else {
                    let error = decision.error.expect("blocked decision carries error");
                    assert_eq!(error.from, from);
                    assert_eq!(error.to, to);
                    assert_eq!(error.evidence_ref, "ev-1");
                }
            }
        }
    }

    #[test]
    fn illegal_skip_searching_to_verified_fails_closed() {
        let decision = AgentState::Searching.admit_transition(AgentState::Verified, "ev-1");
        assert!(!decision.admitted);
        let error = decision.error.expect("blocked decision carries error");
        assert_eq!(error.code, AgentFailureCode::IllegalTransition);
        assert_eq!(error.error_class(), "agent_failure:illegal_transition");
        assert_eq!(error.from, AgentState::Searching);
        assert_eq!(error.to, AgentState::Verified);
        assert_eq!(error.evidence_ref, "ev-1");
        assert!(!error.reason.is_empty());
    }

    #[test]
    fn illegal_skip_created_to_synthesis_fails_closed() {
        let decision = AgentState::Created.admit_transition(AgentState::Synthesis, "ev-1");
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            AgentFailureCode::IllegalTransition
        );
    }

    #[test]
    fn backward_jump_fails_closed() {
        let decision = AgentState::Verified.admit_transition(AgentState::Planning, "ev-1");
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            AgentFailureCode::IllegalTransition
        );
    }

    #[test]
    fn failure_sink_is_reachable_from_any_state() {
        for from in ALL_STATES {
            if from == AgentState::FailedVisible {
                continue; // sink-to-sink is refused, covered below
            }
            let decision = from.admit_transition(AgentState::FailedVisible, "ev-fail");
            assert!(decision.admitted, "prior {from:?} must reach the sink");
            assert_eq!(decision.evidence_ref, "ev-fail");
            assert!(decision.error.is_none());
        }
    }

    #[test]
    fn failed_visible_is_a_terminal_sink() {
        for target in ALL_STATES {
            let decision = AgentState::FailedVisible.admit_transition(target, "ev-1");
            assert!(!decision.admitted, "target {target:?}");
            assert_eq!(decision.error.unwrap().code, AgentFailureCode::TerminalSink);
        }
    }

    #[test]
    fn admission_requires_durable_evidence() {
        for (from, to) in [
            (AgentState::Created, AgentState::Planning),
            (AgentState::Reading, AgentState::Synthesis),
        ] {
            for blank in ["", "   ", "\t\n"] {
                let decision = from.admit_transition(to, blank);
                assert!(!decision.admitted, "{from:?} -> {to:?} with {blank:?}");
                assert_eq!(
                    decision.error.unwrap().code,
                    AgentFailureCode::MissingEvidence
                );
            }
        }
        // No silent empty-result stand-in: a blank evidence ref never admits.
        let decision = AgentState::Verification.admit_transition(AgentState::Verified, "");
        assert_eq!(
            decision.error.unwrap().code,
            AgentFailureCode::MissingEvidence
        );
    }

    #[test]
    fn missing_prior_state_fails_closed_on_entry() {
        let decision = admit_transition(None, AgentState::Searching, "ev-1");
        assert!(!decision.admitted);
        let error = decision.error.unwrap();
        assert_eq!(error.code, AgentFailureCode::MissingEntryState);
        assert_eq!(error.error_class(), "agent_failure:missing_entry_state");
        assert_eq!(error.from, AgentState::Created);
        assert_eq!(error.to, AgentState::Searching);
        // An explicit entry state with durable evidence admits the first step.
        let decision = admit_transition(
            Some(AgentState::Created),
            AgentState::Planning,
            "sha256:entry-v1",
        );
        assert!(decision.admitted);
        assert_eq!(decision.evidence_ref, "sha256:entry-v1");
    }

    #[test]
    fn all_failure_codes_have_stable_tokens() {
        assert_eq!(
            AgentFailureCode::IllegalTransition.as_str(),
            "illegal_transition"
        );
        assert_eq!(
            AgentFailureCode::MissingEntryState.as_str(),
            "missing_entry_state"
        );
        assert_eq!(AgentFailureCode::TerminalSink.as_str(), "terminal_sink");
        assert_eq!(
            AgentFailureCode::MissingEvidence.as_str(),
            "missing_evidence"
        );
        assert_eq!(AgentFailureCode::BudgetOverflow.as_str(), "budget_overflow");
        assert_eq!(
            AgentFailureCode::IdentityMismatch.as_str(),
            "identity_mismatch"
        );
    }

    #[test]
    fn blocked_decision_exposes_full_error_shape() {
        let decision = AgentState::Planning.admit_transition(AgentState::Verified, "ev-1");
        let error = decision.error.expect("blocked");
        assert_eq!(error.code, AgentFailureCode::IllegalTransition);
        assert!(!error.reason.is_empty());
        assert_eq!(error.error_class(), "agent_failure:illegal_transition");
    }

    // ── Serde stability (ADR-051 §4.4, T01 subset; full sweep in T04) ──

    #[test]
    fn agent_state_serde_roundtrip_all_variants() {
        for state in ALL_STATES {
            crate::roundtrip_json(&state);
        }
        // snake_case wire spelling is binding for reports (S04 persists JSON).
        assert_eq!(
            serde_json::to_string(&AgentState::Verification).unwrap(),
            "\"verification\""
        );
        assert_eq!(
            serde_json::to_string(&AgentState::FailedVisible).unwrap(),
            "\"failed_visible\""
        );
    }

    #[test]
    fn agent_failure_code_serde_roundtrip_all_variants() {
        for code in [
            AgentFailureCode::IllegalTransition,
            AgentFailureCode::MissingEntryState,
            AgentFailureCode::TerminalSink,
            AgentFailureCode::MissingEvidence,
            AgentFailureCode::BudgetOverflow,
            AgentFailureCode::IdentityMismatch,
        ] {
            crate::roundtrip_json(&code);
        }
        assert_eq!(
            serde_json::to_string(&AgentFailureCode::BudgetOverflow).unwrap(),
            "\"budget_overflow\""
        );
    }

    #[test]
    fn admission_decision_serde_roundtrip() {
        crate::roundtrip_json(&AgentAdmissionDecision::admitted("sha256:plan-v1"));
        let blocked = AgentAdmissionDecision::blocked(AgentAdmissionError::new(
            AgentFailureCode::IllegalTransition,
            "not declared",
            "ev-1",
            AgentState::Created,
            AgentState::Synthesis,
        ));
        crate::roundtrip_json(&blocked);
        let json = serde_json::to_string(&blocked).unwrap();
        // Wire format carries the snake_case code, not the derived class token.
        assert!(json.contains("\"code\":\"illegal_transition\""));
        assert_eq!(
            blocked.error.unwrap().error_class(),
            "agent_failure:illegal_transition"
        );
    }

    // ── Property tests (ADR-051 P1) ──

    proptest! {
        #[test]
        fn only_declared_forward_or_failure_agent_transitions_are_legal(a in 0usize..8, b in 0usize..8) {
            let expected = b == 7 || (a < 6 && b == a + 1);
            prop_assert_eq!(ALL_STATES[a].can_transition_to(ALL_STATES[b]), expected);
        }
    }
}
