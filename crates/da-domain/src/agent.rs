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
//! # Identity, budget and evidence
//!
//! `GraphIdentity` (T02) is a domain-local value (semantic digest + tenant +
//! node/edge counts) so the S03 port layer can map `kg-storage
//! SnapshotIdentity` onto it without inverting the onion; `da-domain` has no
//! `kg-storage` dependency. `AgentBudget` (T02) is a fail-closed spend bound
//! (`max_queries`, `max_evidence_entries`, `max_synthesis_chars`): every
//! `record_*` refusal happens **before** counters advance, so a runaway loop
//! becomes a typed visible failure. `AgentQueryEvidence` (T03) records
//! truncation explicitly (`truncated: bool`) — never implied by empty rows
//! (R087).

use crate::assurance::ContentDigest;
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

/// Domain-local read-plane identity for an agent run (R091 lineage, T02).
///
/// S03 maps `kg-storage SnapshotIdentity` onto this value type and checks
/// expected vs actual before any query. The value stays domain-local:
/// `da-domain` has no `kg-storage` dependency, so no `physical_sha256` or
/// gzip-decode surface leaks into the domain — only the semantic digest,
/// tenant, and node/edge counts the agent line actually reasons about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphIdentity {
    /// Semantic digest of the read-plane graph the agent plans against.
    pub semantic_digest: ContentDigest,
    /// Tenant scope the graph belongs to. Must be non-empty (validated by
    /// [`GraphIdentity::validate`]) — an empty tenant would silently merge
    /// read planes across tenants.
    pub tenant: String,
    /// Node count reported by the read plane.
    pub nodes: u64,
    /// Edge count reported by the read plane.
    pub edges: u64,
}

impl GraphIdentity {
    pub fn new(
        semantic_digest: ContentDigest,
        tenant: impl Into<String>,
        nodes: u64,
        edges: u64,
    ) -> Self {
        Self {
            semantic_digest,
            tenant: tenant.into(),
            nodes,
            edges,
        }
    }

    /// Fail-closed identity check: an empty tenant is invalid (R091) —
    /// rejecting it here prevents a silent cross-tenant merge later.
    pub fn validate(&self) -> Result<(), GraphIdentityError> {
        if self.tenant.trim().is_empty() {
            Err(GraphIdentityError::EmptyTenant)
        } else {
            Ok(())
        }
    }

    /// Cheap equality probe for pre-checks. Not a substitute for
    /// [`verify_identity`], which names both sides on mismatch.
    pub fn matches(&self, other: &GraphIdentity) -> bool {
        self == other
    }
}

impl std::fmt::Display for GraphIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GraphIdentity{{semantic_digest: {}, tenant: {:?}, nodes: {}, edges: {}}}",
            self.semantic_digest.as_str(),
            self.tenant,
            self.nodes,
            self.edges
        )
    }
}

/// Typed read-plane identity errors (R087/R091, T02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum GraphIdentityError {
    /// The identity carries an empty tenant. Rejected by
    /// [`GraphIdentity::validate`] before any query.
    #[error("agent graph identity requires a non-empty tenant")]
    EmptyTenant,
    /// Expected vs actual read-plane identity differ. The reason names both
    /// sides fully: semantic digest, tenant, node and edge counts. The two
    /// identities are boxed so the error stays small (clippy
    /// `result_large_err`); Deref keeps them fully usable.
    #[error("agent graph identity mismatch: expected {expected}, actual {actual}")]
    IdentityMismatch {
        expected: Box<GraphIdentity>,
        actual: Box<GraphIdentity>,
    },
}

impl GraphIdentityError {
    /// Bridge onto the typed admission surface: the refusal becomes a
    /// blocked decision with `error_class = agent_failure:identity_mismatch`
    /// (the S03/T03 verification guard calls this before admitting
    /// `Verification -> Verified`).
    pub fn into_admission(
        self,
        evidence_ref: impl Into<String>,
        from: AgentState,
        to: AgentState,
    ) -> AgentAdmissionError {
        AgentAdmissionError::new(
            AgentFailureCode::IdentityMismatch,
            self.to_string(),
            evidence_ref,
            from,
            to,
        )
    }
}

/// Compare the identity an agent planned against with the identity of the
/// graph it actually read (R091 lineage). Equal identities admit; any
/// divergence (digest, tenant, or counts) is a typed
/// [`GraphIdentityError::IdentityMismatch`] naming both sides — never a
/// silent empty result (R087). Both sides are validated first: two invalid
/// (empty-tenant) identities must not bless each other.
pub fn verify_identity(
    expected: &GraphIdentity,
    actual: &GraphIdentity,
) -> Result<(), GraphIdentityError> {
    expected.validate()?;
    actual.validate()?;
    if expected == actual {
        Ok(())
    } else {
        Err(GraphIdentityError::IdentityMismatch {
            expected: Box::new(expected.clone()),
            actual: Box::new(actual.clone()),
        })
    }
}

/// Fail-closed agent run budget (R095, T02).
///
/// Bounds query, evidence-entry and synthesis-character spend so a runaway
/// plan-read-synthesize loop becomes a typed visible refusal
/// ([`AgentBudgetError`]) before counters advance — the `BudgetOverflow`
/// guard on the agent line (R087). Pure domain value; the orchestrator
/// (S03/S04) holds one per run and calls the `record_*` methods at the exact
/// moment spend happens, before admitting the transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBudget {
    pub max_queries: u64,
    pub max_evidence_entries: u64,
    pub max_synthesis_chars: u64,
    pub spent_queries: u64,
    pub spent_evidence_entries: u64,
    pub spent_synthesis_chars: u64,
}

impl AgentBudget {
    pub fn new(max_queries: u64, max_evidence_entries: u64, max_synthesis_chars: u64) -> Self {
        Self {
            max_queries,
            max_evidence_entries,
            max_synthesis_chars,
            spent_queries: 0,
            spent_evidence_entries: 0,
            spent_synthesis_chars: 0,
        }
    }

    pub fn remaining_queries(&self) -> u64 {
        self.max_queries.saturating_sub(self.spent_queries)
    }

    pub fn remaining_evidence_entries(&self) -> u64 {
        self.max_evidence_entries
            .saturating_sub(self.spent_evidence_entries)
    }

    pub fn remaining_synthesis_chars(&self) -> u64 {
        self.max_synthesis_chars
            .saturating_sub(self.spent_synthesis_chars)
    }

    /// Record one read-plane query. Fail-closed: refuses
    /// ([`AgentBudgetError::QueryOverflow`]) when the limit is reached and
    /// leaves `spent_queries` unchanged.
    pub fn record_query(&mut self) -> Result<(), AgentBudgetError> {
        let next = self
            .spent_queries
            .checked_add(1)
            .ok_or(AgentBudgetError::QueryOverflow {
                max: self.max_queries,
                spent: self.spent_queries,
            })?;
        if next > self.max_queries {
            return Err(AgentBudgetError::QueryOverflow {
                max: self.max_queries,
                spent: self.spent_queries,
            });
        }
        self.spent_queries = next;
        Ok(())
    }

    /// Record `entries` evidence entries (one query may yield many).
    /// Fail-closed: a batch that would overshoot the limit is refused
    /// atomically — no partial credit, `spent_evidence_entries` unchanged.
    pub fn record_evidence(&mut self, entries: u64) -> Result<(), AgentBudgetError> {
        let next = self.spent_evidence_entries.checked_add(entries).ok_or(
            AgentBudgetError::EvidenceOverflow {
                max: self.max_evidence_entries,
                spent: self.spent_evidence_entries,
            },
        )?;
        if next > self.max_evidence_entries {
            return Err(AgentBudgetError::EvidenceOverflow {
                max: self.max_evidence_entries,
                spent: self.spent_evidence_entries,
            });
        }
        self.spent_evidence_entries = next;
        Ok(())
    }

    /// Record `chars` synthesis characters. Fail-closed: refuses before
    /// `spent_synthesis_chars` advances.
    pub fn record_synthesis_chars(&mut self, chars: u64) -> Result<(), AgentBudgetError> {
        let next = self.spent_synthesis_chars.checked_add(chars).ok_or(
            AgentBudgetError::SynthesisOverflow {
                max: self.max_synthesis_chars,
                spent: self.spent_synthesis_chars,
            },
        )?;
        if next > self.max_synthesis_chars {
            return Err(AgentBudgetError::SynthesisOverflow {
                max: self.max_synthesis_chars,
                spent: self.spent_synthesis_chars,
            });
        }
        self.spent_synthesis_chars = next;
        Ok(())
    }
}

/// Typed agent budget refusal (R087, T02).
///
/// Carries the limit and the spend at refusal time. Every refusal happens
/// **before** the corresponding counter advances (fail-closed) — a guard
/// failure is visible, never a silent empty result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum AgentBudgetError {
    #[error(
        "agent query budget overflow: {spent}/{max} queries already spent; refusing before counters advance"
    )]
    QueryOverflow { max: u64, spent: u64 },
    #[error(
        "agent evidence budget overflow: {spent}/{max} evidence entries already spent; refusing before counters advance"
    )]
    EvidenceOverflow { max: u64, spent: u64 },
    #[error(
        "agent synthesis budget overflow: {spent}/{max} synthesis chars already spent; refusing before counters advance"
    )]
    SynthesisOverflow { max: u64, spent: u64 },
}

impl AgentBudgetError {
    /// Bridge onto the typed admission surface: the refusal becomes a
    /// blocked decision with `error_class = agent_failure:budget_overflow`.
    pub fn into_admission(
        self,
        evidence_ref: impl Into<String>,
        from: AgentState,
        to: AgentState,
    ) -> AgentAdmissionError {
        AgentAdmissionError::new(
            AgentFailureCode::BudgetOverflow,
            self.to_string(),
            evidence_ref,
            from,
            to,
        )
    }
}

/// One read-plane query result recorded as agent evidence (R095, T03).
///
/// Grounds synthesis claims: a claim's `evidence_ref` must match an entry's
/// `result_ref`. Truncation is an **explicit** `truncated: bool` — it is
/// never implied by `rows == 0` (R087): an empty result with `truncated:
/// false` is a genuine empty answer, while `truncated: true` records that
/// rows were cut off at the read plane. Pure domain value, no I/O.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentQueryEvidence {
    /// The read-plane query text that produced this result.
    pub query: String,
    /// Durable reference to the query result record (result hash / manifest
    /// key / checksum). Claims ground onto this value.
    pub result_ref: String,
    /// Number of rows returned.
    pub rows: u64,
    /// Explicit truncation flag (R087): rows were cut off. Never derived
    /// from `rows == 0`.
    pub truncated: bool,
    /// Query duration in milliseconds, as reported by the read plane.
    pub duration_ms: u64,
}

impl AgentQueryEvidence {
    pub fn new(
        query: impl Into<String>,
        result_ref: impl Into<String>,
        rows: u64,
        truncated: bool,
        duration_ms: u64,
    ) -> Self {
        Self {
            query: query.into(),
            result_ref: result_ref.into(),
            rows,
            truncated,
            duration_ms,
        }
    }
}

/// One synthesis claim produced by the agent (R095, T03).
///
/// Every claim must be grounded: `evidence_ref` names the durable result
/// record ([`AgentQueryEvidence::result_ref`]) already present in the
/// [`ContextBundle`]. A claim whose ref is blank or not present in the
/// bundle is a dangling claim — the verification guard refuses it as
/// [`AgentFailureCode::MissingEvidence`] (R087), never a silent empty
/// result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentClaim {
    /// The claim text (answer fragment).
    pub text: String,
    /// Durable evidence reference grounding this claim. Must match an
    /// [`AgentQueryEvidence::result_ref`] already in the bundle.
    pub evidence_ref: String,
}

impl AgentClaim {
    pub fn new(text: impl Into<String>, evidence_ref: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            evidence_ref: evidence_ref.into(),
        }
    }
}

/// Typed grounding refusal for agent claims (R087, T03).
///
/// Produced by [`ContextBundle::grounding_error`]; both variants bridge onto
/// [`AgentFailureCode::MissingEvidence`] via [`into_admission`] so a dangling
/// claim becomes a typed visible failure with
/// `error_class = agent_failure:missing_evidence`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum AgentGroundingError {
    /// A claim carries a blank evidence reference.
    #[error(
        "agent claim #{claim_index} carries an empty evidence reference; every synthesis claim must name query evidence"
    )]
    EmptyEvidenceRef { claim_index: usize },
    /// A claim references a result record not present in the bundle.
    #[error(
        "agent claim #{claim_index} references evidence {evidence_ref:?} that is not in the bundle; every synthesis claim must ground onto query evidence already recorded"
    )]
    DanglingEvidenceRef {
        claim_index: usize,
        evidence_ref: String,
    },
}

impl AgentGroundingError {
    /// Bridge onto the typed admission surface: the refusal becomes a
    /// blocked decision with `error_class = agent_failure:missing_evidence`.
    pub fn into_admission(
        self,
        evidence_ref: impl Into<String>,
        from: AgentState,
        to: AgentState,
    ) -> AgentAdmissionError {
        AgentAdmissionError::new(
            AgentFailureCode::MissingEvidence,
            self.to_string(),
            evidence_ref,
            from,
            to,
        )
    }
}

/// Agent context bundle grounding synthesis claims to query evidence and
/// binding the run to the read-plane identity it planned against
/// (R095/R091, T03).
///
/// Built up during the run: `expected_identity` is set at planning time (the
/// graph the agent plans against), evidence entries are appended as queries
/// return, claims are appended during synthesis. At `Verification → Verified`
/// the caller (S03) passes the actual read-plane identity (from a
/// `DA.IDENTITY` handshake) to [`ContextBundle::verify_verification`], which
/// admits only when the identity matches **and** every claim is grounded.
/// Pure domain value, fail-closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBundle {
    /// Identity of the graph the agent planned against (R091 lineage).
    pub expected_identity: GraphIdentity,
    /// Query evidence already in the bundle, in arrival order.
    pub evidence: Vec<AgentQueryEvidence>,
    /// Synthesis claims produced so far.
    pub claims: Vec<AgentClaim>,
}

impl ContextBundle {
    pub fn new(expected_identity: GraphIdentity) -> Self {
        Self {
            expected_identity,
            evidence: Vec::new(),
            claims: Vec::new(),
        }
    }

    pub fn add_evidence(&mut self, evidence: AgentQueryEvidence) {
        self.evidence.push(evidence);
    }

    pub fn add_claim(&mut self, claim: AgentClaim) {
        self.claims.push(claim);
    }

    /// Typed grounding check (R087): every claim's `evidence_ref` must be
    /// non-empty and must match an evidence entry's `result_ref` already in
    /// the bundle. A dangling claim is a typed [`AgentGroundingError`],
    /// never a silent empty result.
    pub fn grounding_error(&self) -> Result<(), AgentGroundingError> {
        for (index, claim) in self.claims.iter().enumerate() {
            if claim.evidence_ref.trim().is_empty() {
                return Err(AgentGroundingError::EmptyEvidenceRef { claim_index: index });
            }
            if !self
                .evidence
                .iter()
                .any(|entry| entry.result_ref == claim.evidence_ref)
            {
                return Err(AgentGroundingError::DanglingEvidenceRef {
                    claim_index: index,
                    evidence_ref: claim.evidence_ref.clone(),
                });
            }
        }
        Ok(())
    }

    /// Boolean probe matching the slice contract: `false` for a dangling
    /// claim evidence ref. Prefer [`grounding_error`] for the typed reason.
    pub fn all_claims_grounded(&self) -> bool {
        self.grounding_error().is_ok()
    }

    /// T03 identity verification guard for `Verification → Verified`
    /// (R087/R091). Admits only when:
    ///
    /// 1. the step carries a durable evidence reference (else
    ///    [`AgentFailureCode::MissingEvidence`]);
    /// 2. the graph the agent planned against is the graph it actually read
    ///    — [`verify_identity`] names both sides on mismatch
    ///    ([`AgentFailureCode::IdentityMismatch`]);
    /// 3. every synthesis claim is grounded in query evidence already in the
    ///    bundle ([`AgentFailureCode::MissingEvidence`]).
    ///
    /// Pure and fail-closed: a blocked guard never yields a silent empty
    /// result, it yields this typed blocked decision.
    pub fn verify_verification(
        &self,
        actual_identity: &GraphIdentity,
        evidence_ref: &str,
    ) -> AgentAdmissionDecision {
        let from = AgentState::Verification;
        let to = AgentState::Verified;
        if evidence_ref.trim().is_empty() {
            return AgentAdmissionDecision::blocked(AgentAdmissionError::new(
                AgentFailureCode::MissingEvidence,
                "verification admission requires a durable evidence reference",
                evidence_ref.to_string(),
                from,
                to,
            ));
        }
        if let Err(err) = verify_identity(&self.expected_identity, actual_identity) {
            return AgentAdmissionDecision::blocked(err.into_admission(evidence_ref, from, to));
        }
        if let Err(err) = self.grounding_error() {
            return AgentAdmissionDecision::blocked(err.into_admission(evidence_ref, from, to));
        }
        AgentAdmissionDecision::admitted(evidence_ref.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn agent_admission_error_serde_roundtrip() {
        // Direct roundtrip of the error shape (not only transitively inside a
        // decision): S04 persists run reports, so the wire spelling of every
        // field is binding — rename_all drift here becomes a silent report
        // break.
        let err = AgentAdmissionError::new(
            AgentFailureCode::BudgetOverflow,
            "budget exhausted",
            "ev-1",
            AgentState::Searching,
            AgentState::Reading,
        );
        crate::roundtrip_json(&err);
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"code\":\"budget_overflow\""));
        assert!(json.contains("\"from\":\"searching\""));
        assert!(json.contains("\"to\":\"reading\""));
        assert!(json.contains("\"evidence_ref\":\"ev-1\""));
        assert!(json.contains("\"reason\":\"budget exhausted\""));
    }

    // ── GraphIdentity (T02) ──

    #[test]
    fn graph_identity_validate_rejects_empty_tenant() {
        let digest = ContentDigest::of_text("plan-graph-v1");
        let valid = GraphIdentity::new(digest.clone(), "tenant-a", 10, 4);
        assert!(valid.validate().is_ok());

        for blank in ["", "   "] {
            let invalid = GraphIdentity::new(digest.clone(), blank, 10, 4);
            let err = invalid.validate().unwrap_err();
            assert_eq!(err, GraphIdentityError::EmptyTenant);
            assert!(err.to_string().contains("non-empty tenant"));
        }
    }

    #[test]
    fn verify_identity_accepts_identical_read_planes() {
        let expected = GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 4);
        let actual = GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 4);
        assert!(verify_identity(&expected, &actual).is_ok());
        assert!(expected.matches(&actual));
    }

    #[test]
    fn verify_identity_mismatch_names_both_sides() {
        let expected = GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 4);
        let actual = GraphIdentity::new(ContentDigest::of_text("g2"), "tenant-b", 12, 5);
        let err = verify_identity(&expected, &actual).unwrap_err();
        assert_eq!(
            err,
            GraphIdentityError::IdentityMismatch {
                expected: Box::new(expected.clone()),
                actual: Box::new(actual.clone()),
            }
        );
        // Typed access to both sides stays available through Deref.
        let mismatch = verify_identity(&expected, &actual).unwrap_err();
        let GraphIdentityError::IdentityMismatch {
            expected: e,
            actual: a,
        } = &mismatch
        else {
            panic!("expected IdentityMismatch");
        };
        assert_eq!(**e, expected);
        assert_eq!(**a, actual);
        let reason = err.to_string();
        // Both sides fully named: semantic digest, tenant, node/edge counts.
        assert!(
            reason.contains(expected.semantic_digest.as_str()),
            "reason must name expected digest"
        );
        assert!(
            reason.contains(actual.semantic_digest.as_str()),
            "reason must name actual digest"
        );
        assert!(
            reason.contains("tenant-a"),
            "reason must name expected tenant"
        );
        assert!(
            reason.contains("tenant-b"),
            "reason must name actual tenant"
        );
        assert!(
            reason.contains("nodes: 10"),
            "reason must name expected node count"
        );
        assert!(
            reason.contains("nodes: 12"),
            "reason must name actual node count"
        );
        assert!(
            reason.contains("edges: 4"),
            "reason must name expected edge count"
        );
        assert!(
            reason.contains("edges: 5"),
            "reason must name actual edge count"
        );

        // Bridge: the mismatch is a typed blocked admission decision with the
        // stable agent_failure:identity_mismatch class (slice verification).
        let admission =
            err.into_admission("ev-verify", AgentState::Verification, AgentState::Verified);
        assert_eq!(admission.code, AgentFailureCode::IdentityMismatch);
        assert_eq!(admission.error_class(), "agent_failure:identity_mismatch");
        assert_eq!(admission.from, AgentState::Verification);
        assert_eq!(admission.to, AgentState::Verified);
        assert_eq!(admission.evidence_ref, "ev-verify");
        assert!(admission.reason.contains("tenant-a"));
        assert!(admission.reason.contains("tenant-b"));
    }

    #[test]
    fn identity_mismatch_is_visible_for_each_dimension() {
        // A divergence in any single dimension is a visible typed mismatch.
        let base = GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 4);
        let cases = [
            GraphIdentity::new(ContentDigest::of_text("g2"), "tenant-a", 10, 4),
            GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-b", 10, 4),
            GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 11, 4),
            GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 5),
        ];
        for case in cases {
            assert_eq!(
                verify_identity(&base, &case).unwrap_err(),
                GraphIdentityError::IdentityMismatch {
                    expected: Box::new(base.clone()),
                    actual: Box::new(case.clone()),
                },
                "divergence must be a typed mismatch"
            );
        }
    }

    #[test]
    fn verify_identity_refuses_invalid_identities_even_when_equal() {
        // Two empty-tenant identities are equal but must not bless each other.
        let bogus = GraphIdentity::new(ContentDigest::of_text("g1"), "", 10, 4);
        assert_eq!(
            verify_identity(&bogus, &bogus).unwrap_err(),
            GraphIdentityError::EmptyTenant
        );
    }

    // ── AgentBudget (T02) ──

    #[test]
    fn budget_admits_spend_up_to_limit_and_exhausts() {
        let mut budget = AgentBudget::new(3, 10, 1000);
        for _ in 0..3 {
            assert!(budget.record_query().is_ok());
        }
        assert_eq!(budget.spent_queries, 3);
        assert_eq!(budget.remaining_queries(), 0);
        assert!(budget.record_query().is_err());
        assert_eq!(budget.spent_queries, 3, "refusal must not advance counters");
    }

    #[test]
    fn budget_refuses_query_overflow_with_typed_error() {
        let mut budget = AgentBudget::new(2, 10, 1000);
        budget.record_query().unwrap();
        budget.record_query().unwrap();
        let before = budget.spent_queries;
        let err = budget.record_query().unwrap_err();
        assert_eq!(err, AgentBudgetError::QueryOverflow { max: 2, spent: 2 });
        assert_eq!(
            budget.spent_queries, before,
            "fail-closed: counters unchanged"
        );

        // Bridge: typed blocked admission decision with agent_failure:budget_overflow.
        let admission = err.into_admission("ev-q", AgentState::Searching, AgentState::Reading);
        assert_eq!(admission.code, AgentFailureCode::BudgetOverflow);
        assert_eq!(admission.error_class(), "agent_failure:budget_overflow");
        assert_eq!(admission.from, AgentState::Searching);
        assert_eq!(admission.to, AgentState::Reading);
        assert_eq!(admission.evidence_ref, "ev-q");
        assert!(admission.reason.contains("2/2"));
    }

    #[test]
    fn budget_refuses_evidence_overflow_atomically() {
        let mut budget = AgentBudget::new(10, 5, 1000);
        budget.record_evidence(3).unwrap();
        budget.record_evidence(2).unwrap();
        assert_eq!(budget.spent_evidence_entries, 5);

        // Overshooting single batch refused atomically: no partial credit.
        let before = budget.spent_evidence_entries;
        let err = budget.record_evidence(1).unwrap_err();
        assert_eq!(err, AgentBudgetError::EvidenceOverflow { max: 5, spent: 5 });
        assert_eq!(budget.spent_evidence_entries, before);

        let mut budget = AgentBudget::new(10, 5, 1000);
        budget.record_evidence(4).unwrap();
        let before = budget.spent_evidence_entries;
        assert!(budget.record_evidence(2).is_err());
        assert_eq!(
            budget.spent_evidence_entries, before,
            "no partial credit on an overshooting batch"
        );
        assert_eq!(budget.remaining_evidence_entries(), 1);
    }

    #[test]
    fn budget_refuses_synthesis_overflow_before_counter_advances() {
        let mut budget = AgentBudget::new(10, 10, 100);
        budget.record_synthesis_chars(60).unwrap();
        let before = budget.spent_synthesis_chars;
        let err = budget.record_synthesis_chars(41).unwrap_err();
        assert_eq!(
            err,
            AgentBudgetError::SynthesisOverflow {
                max: 100,
                spent: 60
            }
        );
        assert_eq!(
            budget.spent_synthesis_chars, before,
            "fail-closed: counters unchanged"
        );
    }

    #[test]
    fn zero_max_budget_refuses_everything_fail_closed() {
        let mut budget = AgentBudget::new(0, 0, 0);
        assert_eq!(
            budget.record_query().unwrap_err(),
            AgentBudgetError::QueryOverflow { max: 0, spent: 0 }
        );
        assert_eq!(budget.spent_queries, 0);
        assert_eq!(
            budget.record_evidence(1).unwrap_err(),
            AgentBudgetError::EvidenceOverflow { max: 0, spent: 0 }
        );
        assert_eq!(budget.spent_evidence_entries, 0);
        assert_eq!(
            budget.record_synthesis_chars(1).unwrap_err(),
            AgentBudgetError::SynthesisOverflow { max: 0, spent: 0 }
        );
        assert_eq!(budget.spent_synthesis_chars, 0);
    }

    #[test]
    fn budget_counters_cannot_be_driven_past_limits_by_tampering() {
        // Even with public spent fields tampered past the limit, record_*
        // still refuses — the guard reads the counters, never trusts a
        // monotonic caller (fail-closed under adversarial state).
        let mut budget = AgentBudget::new(5, 5, 500);
        budget.spent_queries = 5; // at limit
        assert_eq!(
            budget.record_query().unwrap_err(),
            AgentBudgetError::QueryOverflow { max: 5, spent: 5 }
        );
        budget.spent_queries = 99; // past limit (tampered)
        assert_eq!(
            budget.record_query().unwrap_err(),
            AgentBudgetError::QueryOverflow { max: 5, spent: 99 }
        );
        assert_eq!(budget.spent_queries, 99, "refusal leaves state untouched");
        assert_eq!(budget.remaining_queries(), 0);
    }

    // ── Serde stability (ADR-051 §4.4, T02 subset; full sweep in T04) ──

    #[test]
    fn graph_identity_and_budget_serde_roundtrip() {
        crate::roundtrip_json(&GraphIdentity::new(
            ContentDigest::of_text("g1"),
            "tenant-a",
            10,
            4,
        ));
        crate::roundtrip_json(&AgentBudget::new(3, 10, 1000));
        crate::roundtrip_json(&AgentBudgetError::QueryOverflow { max: 3, spent: 3 });
        crate::roundtrip_json(&AgentBudgetError::EvidenceOverflow { max: 5, spent: 5 });
        crate::roundtrip_json(&AgentBudgetError::SynthesisOverflow {
            max: 100,
            spent: 60,
        });
        crate::roundtrip_json(&GraphIdentityError::EmptyTenant);
        crate::roundtrip_json(&GraphIdentityError::IdentityMismatch {
            expected: Box::new(GraphIdentity::new(
                ContentDigest::of_text("g1"),
                "tenant-a",
                10,
                4,
            )),
            actual: Box::new(GraphIdentity::new(
                ContentDigest::of_text("g2"),
                "tenant-b",
                12,
                5,
            )),
        });
        // Wire spellings are binding for S04's JSON report.
        let json = serde_json::to_string(&GraphIdentity::new(
            ContentDigest::of_text("g1"),
            "tenant-a",
            10,
            4,
        ))
        .unwrap();
        assert!(json.contains("\"tenant\":\"tenant-a\""));
        assert!(json.contains("\"nodes\":10"));
        assert!(json.contains("\"edges\":4"));
    }

    // ── ContextBundle / AgentQueryEvidence / AgentClaim (T03) ──

    fn test_identity(label: &str, tenant: &str, nodes: u64, edges: u64) -> GraphIdentity {
        GraphIdentity::new(ContentDigest::of_text(label), tenant, nodes, edges)
    }

    fn bundle_with_evidence() -> ContextBundle {
        let mut bundle = ContextBundle::new(test_identity("g1", "tenant-a", 10, 4));
        bundle.add_evidence(AgentQueryEvidence::new(
            "match method:GRPO",
            "sha256:q1",
            3,
            false,
            12,
        ));
        bundle.add_evidence(AgentQueryEvidence::new(
            "match method:GEPA",
            "sha256:q2",
            0,
            true, // explicit truncation: rows==0 but truncated=true (R087)
            8,
        ));
        bundle
    }

    #[test]
    fn query_evidence_records_truncation_explicitly_never_implied_by_rows() {
        // rows==0 with truncated==false is a genuine empty answer; the flag
        // is explicit and preserved through the wire format (R087).
        let empty_not_truncated = AgentQueryEvidence::new("match:none", "sha256:e0", 0, false, 4);
        let json = serde_json::to_string(&empty_not_truncated).unwrap();
        assert!(json.contains("\"truncated\":false"));
        assert!(json.contains("\"rows\":0"));
        crate::roundtrip_json(&empty_not_truncated);

        let truncated_empty = AgentQueryEvidence::new("match:many", "sha256:t0", 0, true, 9);
        let json = serde_json::to_string(&truncated_empty).unwrap();
        assert!(json.contains("\"truncated\":true"));
        crate::roundtrip_json(&truncated_empty);

        let full = AgentQueryEvidence::new("match:GRPO", "sha256:q1", 3, false, 12);
        let json = serde_json::to_string(&full).unwrap();
        // Query evidence records query, result_ref, rows, truncated, duration_ms.
        assert!(json.contains("\"query\":\"match:GRPO\""));
        assert!(json.contains("\"result_ref\":\"sha256:q1\""));
        assert!(json.contains("\"rows\":3"));
        assert!(json.contains("\"truncated\":false"));
        assert!(json.contains("\"duration_ms\":12"));
        crate::roundtrip_json(&full);
    }

    #[test]
    fn context_bundle_grounds_claims_against_recorded_evidence() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new(
            "GRPO is used for RL fine-tuning",
            "sha256:q1",
        ));
        assert!(bundle.all_claims_grounded());
        assert!(bundle.grounding_error().is_ok());
    }

    #[test]
    fn all_claims_grounded_is_false_for_a_dangling_claim_ref() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("dangling claim", "sha256:does-not-exist"));
        assert!(!bundle.all_claims_grounded());
        let err = bundle.grounding_error().unwrap_err();
        assert_eq!(
            err,
            AgentGroundingError::DanglingEvidenceRef {
                claim_index: 0,
                evidence_ref: "sha256:does-not-exist".into(),
            }
        );

        // Bridge: a dangling claim is a typed blocked admission decision with
        // error_class = agent_failure:missing_evidence (slice verification).
        let admission =
            err.into_admission("ev-verify", AgentState::Verification, AgentState::Verified);
        assert_eq!(admission.code, AgentFailureCode::MissingEvidence);
        assert_eq!(admission.error_class(), "agent_failure:missing_evidence");
        assert_eq!(admission.from, AgentState::Verification);
        assert_eq!(admission.to, AgentState::Verified);
        assert!(admission.reason.contains("sha256:does-not-exist"));
    }

    #[test]
    fn all_claims_grounded_is_false_for_an_empty_claim_ref() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("blank-ref claim", ""));
        bundle.add_claim(AgentClaim::new("grounded claim", "sha256:q1"));
        assert!(!bundle.all_claims_grounded());
        assert_eq!(
            bundle.grounding_error().unwrap_err(),
            AgentGroundingError::EmptyEvidenceRef { claim_index: 0 }
        );
    }

    #[test]
    fn verify_verification_admits_grounded_and_identity_matched() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("claim A", "sha256:q1"));
        bundle.add_claim(AgentClaim::new("claim B", "sha256:q2"));
        // The graph the agent planned against is the graph it read.
        let actual = test_identity("g1", "tenant-a", 10, 4);
        let decision = bundle.verify_verification(&actual, "sha256:verify-run-1");
        assert!(
            decision.admitted,
            "Verification -> Verified must admit when grounded and identity matches"
        );
        assert_eq!(decision.evidence_ref, "sha256:verify-run-1");
        assert!(decision.error.is_none());
    }

    #[test]
    fn verify_verification_blocks_identity_mismatch_naming_both_sides() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("claim A", "sha256:q1"));
        // The agent read a different graph than it planned against
        // (R091 lineage): different digest, tenant, and counts.
        let actual = test_identity("g2", "tenant-b", 12, 5);
        let decision = bundle.verify_verification(&actual, "sha256:verify-run-1");
        assert!(!decision.admitted);
        let error = decision.error.expect("blocked decision carries error");
        assert_eq!(error.code, AgentFailureCode::IdentityMismatch);
        assert_eq!(error.error_class(), "agent_failure:identity_mismatch");
        assert_eq!(error.from, AgentState::Verification);
        assert_eq!(error.to, AgentState::Verified);
        assert_eq!(error.evidence_ref, "sha256:verify-run-1");
        // Both sides named: expected and actual digest, tenant, counts.
        assert!(error.reason.contains("tenant-a"), "names expected tenant");
        assert!(error.reason.contains("tenant-b"), "names actual tenant");
        assert!(error.reason.contains("nodes: 10"), "names expected nodes");
        assert!(error.reason.contains("nodes: 12"), "names actual nodes");
        assert!(error.reason.contains("edges: 4"), "names expected edges");
        assert!(error.reason.contains("edges: 5"), "names actual edges");
    }

    #[test]
    fn verify_verification_blocks_dangling_claim_as_missing_evidence() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("ungrounded claim", "sha256:nope"));
        let actual = test_identity("g1", "tenant-a", 10, 4);
        let decision = bundle.verify_verification(&actual, "sha256:verify-run-1");
        assert!(!decision.admitted);
        let error = decision.error.expect("blocked decision carries error");
        assert_eq!(error.code, AgentFailureCode::MissingEvidence);
        assert_eq!(error.error_class(), "agent_failure:missing_evidence");
        assert!(error.reason.contains("sha256:nope"));
    }

    #[test]
    fn verify_verification_requires_durable_evidence_ref() {
        let bundle = bundle_with_evidence();
        let actual = test_identity("g1", "tenant-a", 10, 4);
        for blank in ["", "   "] {
            let decision = bundle.verify_verification(&actual, blank);
            assert!(!decision.admitted, "blank {blank:?}");
            assert_eq!(
                decision.error.unwrap().code,
                AgentFailureCode::MissingEvidence
            );
        }
    }

    #[test]
    fn verify_verification_vacuous_without_claims_but_identity_bound() {
        // Zero claims is vacuously grounded; the guard still enforces the
        // R091 lineage (identity) and the durable evidence ref.
        let bundle = bundle_with_evidence();
        let actual = test_identity("g1", "tenant-a", 10, 4);
        let decision = bundle.verify_verification(&actual, "sha256:verify-run-1");
        assert!(decision.admitted);
        // Identity still enforced with no claims present.
        let wrong = test_identity("g2", "tenant-a", 10, 4);
        let decision = bundle.verify_verification(&wrong, "sha256:verify-run-1");
        assert_eq!(
            decision.error.unwrap().code,
            AgentFailureCode::IdentityMismatch
        );
    }

    // ── Serde stability (ADR-051 §4.4, T03 subset; full sweep in T04) ──

    #[test]
    fn context_bundle_serde_roundtrip() {
        let mut bundle = bundle_with_evidence();
        bundle.add_claim(AgentClaim::new("claim A", "sha256:q1"));
        crate::roundtrip_json(&bundle);
        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("\"expected_identity\""));
        assert!(json.contains("\"result_ref\":\"sha256:q2\""));
        assert!(json.contains("\"truncated\":true"));
        assert!(json.contains("\"duration_ms\":12"));

        crate::roundtrip_json(&AgentClaim::new("claim", "sha256:q1"));
        crate::roundtrip_json(&AgentGroundingError::EmptyEvidenceRef { claim_index: 0 });
        crate::roundtrip_json(&AgentGroundingError::DanglingEvidenceRef {
            claim_index: 1,
            evidence_ref: "sha256:nope".into(),
        });
    }
}

// ─── ADR-051 P1: Property tests (T04) ───
// Extracted into the workspace `proptest_tests` module convention so
// `cargo test -p da-domain --lib proptest` actually discovers the agent FSM
// matrix and budget atomicity invariants (same pattern as entity.rs /
// vid.rs / confidence.rs / relation.rs — the filter matches the module
// path, not the test names).

#[cfg(test)]
mod proptest_tests {
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

    const ALL_FAILURE_CODES: [AgentFailureCode; 6] = [
        AgentFailureCode::IllegalTransition,
        AgentFailureCode::MissingEntryState,
        AgentFailureCode::TerminalSink,
        AgentFailureCode::MissingEvidence,
        AgentFailureCode::BudgetOverflow,
        AgentFailureCode::IdentityMismatch,
    ];

    fn arb_failure_code() -> impl Strategy<Value = AgentFailureCode> {
        (0usize..ALL_FAILURE_CODES.len()).prop_map(|i| ALL_FAILURE_CODES[i])
    }

    proptest! {
        #[test]
        fn budget_refusals_are_atomic_and_counters_stay_within_limits(
            max in 0u64..64u64,
            batch in 1u64..16u64,
        ) {
            let mut budget = AgentBudget::new(max, max, max);
            for _ in 0..200usize {
                let before = budget.spent_queries;
                match budget.record_query() {
                    Ok(()) => prop_assert_eq!(budget.spent_queries, before + 1),
                    Err(_) => {
                        prop_assert_eq!(
                            budget.spent_queries, before,
                            "refusal must not advance counters"
                        );
                        prop_assert!(before >= max);
                    }
                }
                prop_assert!(budget.spent_queries <= max);
            }

            let mut budget = AgentBudget::new(max, max, max);
            for _ in 0..200usize {
                let before = budget.spent_evidence_entries;
                match budget.record_evidence(batch) {
                    Ok(()) => {
                        prop_assert_eq!(budget.spent_evidence_entries, before + batch)
                    }
                    Err(_) => {
                        prop_assert_eq!(
                            budget.spent_evidence_entries, before,
                            "refusal must not advance counters"
                        );
                        prop_assert!(
                            before + batch > max,
                            "refusal implies the batch would overshoot"
                        );
                    }
                }
                prop_assert!(budget.spent_evidence_entries <= max);
            }
        }
    }

    proptest! {
        #[test]
        fn only_declared_forward_or_failure_agent_transitions_are_legal(a in 0usize..8, b in 0usize..8) {
            let expected = b == 7 || (a < 6 && b == a + 1);
            prop_assert_eq!(ALL_STATES[a].can_transition_to(ALL_STATES[b]), expected);
        }
    }

    // T04 bonus property: the stable `as_str` token matches the serde
    // snake_case wire spelling for every failure code — drift here would
    // silently break S04 report consumers reading `agent_failure:<code>`.
    proptest! {
        #[test]
        fn failure_code_as_str_matches_serde_wire(code in arb_failure_code()) {
            let json = serde_json::to_string(&code).unwrap();
            let wire = json.trim_matches('"');
            prop_assert_eq!(code.as_str(), wire);
        }
    }
}
