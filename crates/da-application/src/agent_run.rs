//! Verified read-only agent run use case (R095, S03 T03).
//!
//! Orchestrates the S02 domain FSM
//! (`Created → Planning → Searching → Reading → Synthesis → Verification →
//! Verified`, sink `FailedVisible`) against an identity-checked read plane
//! ([`da_ports::AgentReadPlane`]). This is the executable half of R095: a
//! caller submits an [`AgentRunRequest`] and receives a serde-stable
//! [`AgentRunReport`] whose blocked path is always
//! `error_class = agent_failure:<code>` (R087) — never a happy empty result.
//!
//! # FSM topology constraints (why the flow is shaped this way)
//!
//! `AgentState::can_transition_to` admits only adjacent forward steps or the
//! failure sink — there is no self-transition, so:
//!
//! - **every plan query executes inside `Reading`**, with
//!   [`AgentBudget::record_query`] as the per-query gate (fail-closed: a
//!   refusal happens before counters advance);
//! - **every synthesis claim is produced inside `Synthesis`**, with the
//!   synthesis-chars budget recorded atomically before claims enter the
//!   bundle.
//!
//! Identity is checked **twice** (R091 lineage): a pre-flight check after
//! Planning — *before any query* — and the final
//! [`ContextBundle::verify_verification`] at `Verification → Verified`. Each
//! FSM step is admitted with a durable evidence reference produced by the
//! orchestrator itself (entry ref → plan digest → `da.identity:<digest>` →
//! evidence digest → claims digest → verification digest).
//!
//! The public [`verify_run_evidence_chain`] helper independently recomputes
//! the durable evidence chain (plan → identity → evidence → claims →
//! verification) from persisted plan + report data, so S04 CLI capture and
//! `--verify` — and the S05 Governor shell-out (D273) — can never drift
//! from the digest math.
//!
//! # Failure semantics (R087)
//!
//! Every failure path ends at `FailedVisible` with a typed
//! [`AgentAdmissionError`] in the report:
//!
//! - `invalid_plan` — empty, over-budget, zero-`max_rows`, or containing a
//!   query that fails read-only cypher validation (fails before any query);
//! - `identity_mismatch` — pre-flight or final, naming both sides;
//! - `budget_overflow` — query / evidence / synthesis budget, counters
//!   unchanged by the refusal;
//! - `missing_evidence` — dangling synthesis claims at verification;
//! - `read_plane_unavailable` — transport / identity-unavailable / query
//!   failure surfaced by the plane.
//!
//! The use case never holds a write handle: the port exposes identity plus
//! bounded read-only query only, and the plan cyphers are pre-validated
//! read-only (D127 stays locked). Instrumented per ADR-052 (use-case
//! convention); `da-domain` stays tracing-free.

use std::sync::Arc;

use da_domain::{
    AgentAdmissionDecision, AgentAdmissionError, AgentBudget, AgentClaim, AgentFailureCode,
    AgentState, ContentDigest, ContextBundle, GraphIdentity, agent_verify_identity,
};
use da_ports::{AgentReadPlane, AgentSynthesizer};
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// One plan query (R095, S03 T03).
///
/// The query must pass read-only cypher validation ([`validate_plan`]) — the
/// server allowlist (S01) is the second line of defense. Serde-stable: S04
/// persists plan + report for recomputation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlanQuery {
    /// Read-only cypher text executed against the read plane.
    pub query: String,
    /// Human purpose of the query (what the agent is looking for).
    pub purpose: String,
}

/// Verified agent run request (R095, S03 T03).
///
/// Serde-stable (S04 persists it alongside the report). The caller supplies
/// the durable entry evidence, the identity the run plans against, the graph
/// name, the plan, and the budget bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunRequest {
    /// Durable run-entry evidence reference (result hash / manifest key /
    /// checksum) backing `Created → Planning`.
    pub entry_evidence_ref: String,
    /// Identity of the graph the run plans against (R091 lineage). Checked
    /// pre-flight and again at verification.
    pub expected_identity: GraphIdentity,
    /// Read-plane graph name the plan queries execute against.
    pub graph: String,
    /// Ordered plan: every query executes inside `Reading`.
    pub plan: Vec<AgentPlanQuery>,
    /// Fail-closed spend bounds (queries / evidence entries / synthesis
    /// chars). The only load bound on the run, with
    /// `max_rows_per_query` per-query.
    pub budget: AgentBudget,
    /// Adapter-side row cut per query; zero fails closed (invalid_plan).
    pub max_rows_per_query: u64,
}

/// Durable agent run report (R095/R087, S03 T03).
///
/// Serde-stable — S04 persists it atomically. Always returned by
/// [`AgentRunUseCase::run`] (never a happy empty stand-in): on success
/// `terminal_state == Verified` and `error == None`; on any failure
/// `terminal_state == FailedVisible` and `error` carries the terminal typed
/// refusal whose `error_class()` is always `agent_failure:<code>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunReport {
    /// Terminal FSM state: `Verified` or `FailedVisible`.
    pub terminal_state: AgentState,
    /// Full FSM decision history: every admission attempt (admitted or
    /// blocked) plus the failure-sink admission.
    pub decisions: Vec<AgentAdmissionDecision>,
    /// Run context bundle: expected identity, query evidence, claims.
    pub bundle: ContextBundle,
    /// Budget at run end — spent counters vs the max bounds.
    pub budget: AgentBudget,
    /// Identity the run planned against (R091 lineage).
    pub expected_identity: GraphIdentity,
    /// Actual read-plane identity observed (last successful handshake);
    /// `None` when the plane was never reachable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_identity: Option<GraphIdentity>,
    /// Terminal blocked decision's error (R087). `None` on the happy path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentAdmissionError>,
}

/// Verified read-only agent runner (R095, S03 T03).
///
/// Walks the S02 FSM against the injected read plane. `run` always returns
/// an [`AgentRunReport`]; failures are typed visible refusals
/// (`agent_failure:<code>`), never a happy empty result.
pub struct AgentRunUseCase {
    plane: Arc<dyn AgentReadPlane>,
    synthesizer: Arc<dyn AgentSynthesizer>,
}

impl AgentRunUseCase {
    pub fn new(plane: Arc<dyn AgentReadPlane>, synthesizer: Arc<dyn AgentSynthesizer>) -> Self {
        Self { plane, synthesizer }
    }

    /// Execute one verified read-only agent run (ADR-052 instrumented).
    ///
    /// Flow: `Created → Planning` (entry evidence) → plan validation
    /// (invalid plans never execute a query) → `Planning → Searching` (plan
    /// digest) → **pre-flight identity** (before any query) →
    /// `Searching → Reading` (`da.identity:<digest>`) → execute every plan
    /// query inside Reading (per-query budget gate) → `Reading → Synthesis`
    /// (evidence digest) → produce every claim inside Synthesis (chars
    /// budget) → `Synthesis → Verification` (claims digest) → fresh identity
    /// + `verify_verification` → `Verified`. Any refusal sinks to
    /// `FailedVisible` with the typed error.
    #[instrument(skip_all, fields(
        graph = %request.graph,
        plan_len = request.plan.len(),
        max_queries = request.budget.max_queries,
        max_evidence_entries = request.budget.max_evidence_entries,
        max_rows_per_query = request.max_rows_per_query,
    ))]
    pub async fn run(&self, request: &AgentRunRequest) -> AgentRunReport {
        let report = self.run_inner(request).await;
        tracing::info!(
            terminal_state = ?report.terminal_state,
            error_class = report.error.as_ref().map(|error| error.error_class()),
            spent_queries = report.budget.spent_queries,
            evidence_entries = report.bundle.evidence.len(),
            claims = report.bundle.claims.len(),
            "agent run finished"
        );
        report
    }

    async fn run_inner(&self, request: &AgentRunRequest) -> AgentRunReport {
        let mut decisions: Vec<AgentAdmissionDecision> = Vec::new();
        let mut state = AgentState::Created;
        let mut budget = request.budget.clone();
        let mut bundle = ContextBundle::new(request.expected_identity.clone());
        let mut actual_identity: Option<GraphIdentity> = None;

        // 1) Created -> Planning (run entry, durable entry evidence).
        let decision =
            AgentState::Created.admit_transition(AgentState::Planning, &request.entry_evidence_ref);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        state = AgentState::Planning;

        // 2) Plan validation (within Planning): an unexecutable plan must
        //    never reach the read plane (invalid_plan, R087).
        let plan_digest = plan_digest(&request.plan);
        if let Err(reason) = validate_plan(request) {
            let error = AgentAdmissionError::new(
                AgentFailureCode::InvalidPlan,
                reason,
                plan_digest.clone(),
                AgentState::Planning,
                AgentState::Searching,
            );
            decisions.push(AgentAdmissionDecision::blocked(error.clone()));
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }

        // 3) Planning -> Searching (evidence: plan digest).
        let decision = AgentState::Planning.admit_transition(AgentState::Searching, &plan_digest);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        state = AgentState::Searching;

        // 4) Pre-flight identity (within Searching, BEFORE any query — R091).
        //    The handshake result doubles as the evidence for Searching ->
        //    Reading, so a mismatch or unavailable plane sinks here with zero
        //    queries executed.
        let identity_ref = match self.plane.identity().await {
            Ok(identity) => {
                actual_identity = Some(identity.clone());
                let identity_ref = format!("da.identity:{}", identity.semantic_digest.as_str());
                if let Err(mismatch) = agent_verify_identity(&request.expected_identity, &identity)
                {
                    let error = mismatch.into_admission(
                        identity_ref.clone(),
                        AgentState::Searching,
                        AgentState::Reading,
                    );
                    decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                    return failed_report(
                        decisions,
                        bundle,
                        budget,
                        &request.expected_identity,
                        actual_identity,
                        state,
                        error,
                    );
                }
                identity_ref
            }
            Err(plane_error) => {
                let error = AgentAdmissionError::new(
                    AgentFailureCode::ReadPlaneUnavailable,
                    format!("read-plane identity unavailable: {plane_error}"),
                    "da.identity:unavailable",
                    AgentState::Searching,
                    AgentState::Reading,
                );
                decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                return failed_report(
                    decisions,
                    bundle,
                    budget,
                    &request.expected_identity,
                    actual_identity,
                    state,
                    error,
                );
            }
        };

        // 5) Searching -> Reading (evidence: da.identity:<semantic digest>).
        let decision = AgentState::Searching.admit_transition(AgentState::Reading, &identity_ref);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        state = AgentState::Reading;

        // 6) Reading: execute every plan query. record_query is the
        //    per-query gate; evidence entries are charged per result; a
        //    transport/query failure sinks as read_plane_unavailable.
        for query in &request.plan {
            if let Err(budget_error) = budget.record_query() {
                let error = budget_error.into_admission(
                    format!("query:{}", query.purpose),
                    AgentState::Reading,
                    AgentState::Synthesis,
                );
                decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                return failed_report(
                    decisions,
                    bundle,
                    budget,
                    &request.expected_identity,
                    actual_identity,
                    state,
                    error,
                );
            }
            let evidence = match self
                .plane
                .query(&request.graph, &query.query, request.max_rows_per_query)
                .await
            {
                Ok(evidence) => evidence,
                Err(plane_error) => {
                    let error = AgentAdmissionError::new(
                        AgentFailureCode::ReadPlaneUnavailable,
                        format!("read-plane query failed: {plane_error}"),
                        format!("query:{}", query.purpose),
                        AgentState::Reading,
                        AgentState::Synthesis,
                    );
                    decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                    return failed_report(
                        decisions,
                        bundle,
                        budget,
                        &request.expected_identity,
                        actual_identity,
                        state,
                        error,
                    );
                }
            };
            if let Err(budget_error) = budget.record_evidence(1) {
                let error = budget_error.into_admission(
                    evidence.result_ref.clone(),
                    AgentState::Reading,
                    AgentState::Synthesis,
                );
                decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                return failed_report(
                    decisions,
                    bundle,
                    budget,
                    &request.expected_identity,
                    actual_identity,
                    state,
                    error,
                );
            }
            bundle.add_evidence(evidence);
        }

        // 7) Reading -> Synthesis (evidence: evidence digest).
        let evidence_digest = evidence_digest(&bundle);
        let decision =
            AgentState::Reading.admit_transition(AgentState::Synthesis, &evidence_digest);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        state = AgentState::Synthesis;

        // 8) Synthesis: produce every claim. The chars budget is recorded
        //    atomically before any claim enters the bundle.
        let claims = self.synthesizer.synthesize(&bundle);
        let synthesis_chars: u64 = claims
            .iter()
            .map(|claim| claim.text.chars().count() as u64)
            .sum();
        if let Err(budget_error) = budget.record_synthesis_chars(synthesis_chars) {
            let error = budget_error.into_admission(
                "synthesis:claims",
                AgentState::Synthesis,
                AgentState::Verification,
            );
            decisions.push(AgentAdmissionDecision::blocked(error.clone()));
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        for claim in claims {
            bundle.add_claim(claim);
        }

        // 9) Synthesis -> Verification (evidence: claims digest).
        let claims_digest = claims_digest(&bundle);
        let decision =
            AgentState::Synthesis.admit_transition(AgentState::Verification, &claims_digest);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }
        state = AgentState::Verification;

        // 10) Verification -> Verified: fresh identity, then the domain
        //     guard (durable ref -> identity -> grounding). A mismatch or a
        //     dangling claim sinks as identity_mismatch / missing_evidence.
        let actual = match self.plane.identity().await {
            Ok(identity) => {
                actual_identity = Some(identity.clone());
                identity
            }
            Err(plane_error) => {
                let error = AgentAdmissionError::new(
                    AgentFailureCode::ReadPlaneUnavailable,
                    format!("read-plane identity unavailable at verification: {plane_error}"),
                    "da.identity:unavailable",
                    AgentState::Verification,
                    AgentState::Verified,
                );
                decisions.push(AgentAdmissionDecision::blocked(error.clone()));
                return failed_report(
                    decisions,
                    bundle,
                    budget,
                    &request.expected_identity,
                    actual_identity,
                    state,
                    error,
                );
            }
        };
        let verification_ref = verification_digest(&bundle, &actual);
        let decision = bundle.verify_verification(&actual, &verification_ref);
        decisions.push(decision.clone());
        if let Some(error) = decision.error {
            return failed_report(
                decisions,
                bundle,
                budget,
                &request.expected_identity,
                actual_identity,
                state,
                error,
            );
        }

        AgentRunReport {
            terminal_state: AgentState::Verified,
            decisions,
            bundle,
            budget,
            expected_identity: request.expected_identity.clone(),
            actual_identity,
            error: None,
        }
    }
}

/// Default grounded synthesis policy (R095, S03 T03).
///
/// Produces exactly one grounded claim per evidence entry: the claim text
/// states the query and row count (naming truncation when rows were cut
/// off), and the claim grounds onto the entry's `result_ref` — always
/// present in the bundle, so the default policy never dangles. Deterministic
/// and LLM-free; a task-specific policy plugs in behind
/// [`AgentSynthesizer`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicSynthesizer;

impl AgentSynthesizer for DeterministicSynthesizer {
    fn synthesize(&self, bundle: &ContextBundle) -> Vec<AgentClaim> {
        bundle
            .evidence
            .iter()
            .map(|entry| {
                let text = if entry.truncated {
                    format!("{} returned {} rows (truncated)", entry.query, entry.rows)
                } else {
                    format!("{} returned {} rows", entry.query, entry.rows)
                };
                AgentClaim::new(text, entry.result_ref.clone())
            })
            .collect()
    }
}

/// Fail-closed plan validation (R087/R095, S03 T03).
///
/// Refuses — as [`AgentFailureCode::InvalidPlan`], before any query — a plan
/// that cannot execute: empty, over-budget (`plan.len() > max_queries`),
/// with a zero `max_rows_per_query` (fail-closed row cut), or containing a
/// query that fails read-only cypher validation. The server allowlist (S01)
/// is the second line of defense.
fn validate_plan(request: &AgentRunRequest) -> Result<(), String> {
    if request.plan.is_empty() {
        return Err("agent plan is empty: a run requires at least one read-only query".into());
    }
    if request.max_rows_per_query == 0 {
        return Err(
            "max_rows_per_query must be > 0 (fail-closed: zero rows are never served)".into(),
        );
    }
    if (request.plan.len() as u64) > request.budget.max_queries {
        return Err(format!(
            "agent plan has {} queries but the budget allows at most {} (over-budget plan cannot execute)",
            request.plan.len(),
            request.budget.max_queries
        ));
    }
    for (index, entry) in request.plan.iter().enumerate() {
        if let Err(error) = kg_storage::validate_readonly_cypher(&entry.query) {
            return Err(format!(
                "agent plan query #{index} fails read-only cypher validation: {error}"
            ));
        }
    }
    Ok(())
}

/// Durable evidence ref for `Planning → Searching`: a digest over the
/// canonical plan JSON (stable field order). The orchestrator produces it —
/// never a caller-supplied string.
///
/// Public (S04 T01) so CLI capture and `--verify` recompute the same
/// canonical serde shape and never drift from the orchestrator's math (D273:
/// S05 Governor shells out instead of re-hashing in Python).
pub fn plan_digest(plan: &[AgentPlanQuery]) -> String {
    let canonical = serde_json::to_string(plan)
        .expect("agent plan serialization is infallible (plain String fields)");
    format!(
        "sha256:plan:{}",
        ContentDigest::of_text(&canonical).as_str()
    )
}

/// Durable evidence ref for `Reading → Synthesis`: a digest over the
/// canonical evidence list (query, result_ref, rows, truncated, duration).
///
/// Public (S04 T01): see [`plan_digest`].
pub fn evidence_digest(bundle: &ContextBundle) -> String {
    let canonical = serde_json::to_string(&bundle.evidence)
        .expect("evidence serialization is infallible (plain fields)");
    format!(
        "sha256:evidence:{}",
        ContentDigest::of_text(&canonical).as_str()
    )
}

/// Durable evidence ref for `Synthesis → Verification`: a digest over the
/// canonical claims list.
///
/// Public (S04 T01): see [`plan_digest`].
pub fn claims_digest(bundle: &ContextBundle) -> String {
    let canonical = serde_json::to_string(&bundle.claims)
        .expect("claims serialization is infallible (plain fields)");
    format!(
        "sha256:claims:{}",
        ContentDigest::of_text(&canonical).as_str()
    )
}

/// Durable evidence ref for `Verification → Verified`: a digest over the
/// bundle plus the actual identity observed at verification (R091 lineage
/// bound into the final admission).
///
/// Public (S04 T01): see [`plan_digest`].
pub fn verification_digest(bundle: &ContextBundle, actual: &GraphIdentity) -> String {
    let canonical = serde_json::json!({
        "bundle": bundle,
        "actual_identity": actual,
    });
    format!(
        "sha256:verify:{}",
        ContentDigest::of_text(&canonical.to_string()).as_str()
    )
}

/// Independent recomputation of a finished run's durable evidence chain
/// (S04 T01 — the public evidence-chain verifier).
///
/// `plan` and `report` are exactly the serde-stable pair S04 persists in
/// the CLI envelope (plan JSON + [`AgentRunReport`]). Every evidence ref
/// that is a deterministic function of persisted data is recomputed here —
/// plan digest, `da.identity:<digest>`, evidence digest, claims digest,
/// verification digest — and compared with the recorded FSM decision
/// history. Blocked refs that are adapter-internal failure records
/// (`query:<purpose>`, `synthesis:claims`, a raw `sha256:result:<hex>`)
/// are not recomputable from the report; those steps are checked for
/// decision/error consistency instead. Also verified: claim grounding on
/// the happy path, expected/actual identity equality, the bundle identity
/// lineage, and the `terminal_state` ↔ `error` contract (R087).
///
/// Returns the first failing check as `Err("<check name>")` so
/// `da agent-run --verify` (S04) can print PASS/FAIL with the first
/// failure (D273: Governor shells out to this verifier, never re-hashes
/// canonical serde JSON in Python).
pub fn verify_run_evidence_chain(
    plan: &[AgentPlanQuery],
    report: &AgentRunReport,
) -> Result<(), String> {
    let decisions = &report.decisions;
    let failed = report.terminal_state == AgentState::FailedVisible;

    // R087 contract: Verified carries no error, FailedVisible always
    // carries the typed refusal.
    if failed != report.error.is_some() {
        return Err(format!(
            "terminal_state/error: terminal_state {} is inconsistent with error {}",
            if failed { "FailedVisible" } else { "Verified" },
            if report.error.is_some() {
                "present"
            } else {
                "absent"
            }
        ));
    }

    let check = |index: usize, name: &str, expected: &str| -> Result<(), String> {
        match decisions.get(index) {
            Some(decision) if decision.evidence_ref == expected => Ok(()),
            Some(decision) => Err(format!(
                "{name}: recorded evidence_ref {} does not match the independently recomputed {expected}",
                decision.evidence_ref
            )),
            None => Err(format!("{name}: decision missing from the report history")),
        }
    };
    // A blocked decision's ref is a failure record — verify the decision and
    // its error agree (tamper consistency) and the error names this step.
    let check_blocked = |index: usize,
                         name: &str,
                         from: AgentState,
                         to: AgentState|
     -> Result<(), String> {
        let decision = decisions
            .get(index)
            .ok_or_else(|| format!("{name}: decision missing from the report history"))?;
        let error = decision.error.as_ref().ok_or_else(|| {
            format!("{name}: expected a blocked decision but the step was admitted")
        })?;
        if decision.evidence_ref != error.evidence_ref {
            return Err(format!(
                "{name}: decision evidence_ref {} disagrees with its error evidence_ref {}",
                decision.evidence_ref, error.evidence_ref
            ));
        }
        if error.from != from || error.to != to {
            return Err(format!(
                "{name}: recorded error transition {:?} -> {:?} is not the expected {from:?} -> {to:?}",
                error.from, error.to
            ));
        }
        Ok(())
    };
    // The failure sink is always the last decision on a failed report; every
    // forward step is at a fixed index, so a run that sinks early ends the
    // walk there.
    let sink_is_next = |step: usize| failed && step + 1 == decisions.len();
    let sink_check = || -> Result<(), String> {
        let sink = decisions.last().expect("failed report has a sink decision");
        let error = report
            .error
            .as_ref()
            .expect("failed report carries the typed error");
        if !sink.admitted || sink.error.is_some() {
            return Err(
                "failure sink: the terminal admission must be admitted and error-free".into(),
            );
        }
        if sink.evidence_ref != error.evidence_ref {
            return Err(format!(
                "failure sink: recorded evidence_ref {} does not match the terminal error evidence_ref {}",
                sink.evidence_ref, error.evidence_ref
            ));
        }
        Ok(())
    };

    // Step 0: Created -> Planning. The entry ref is request-owned (not
    // recomputable from plan + report alone); the record itself must be an
    // admitted, error-free admission with a non-empty durable ref.
    match decisions.first() {
        Some(decision)
            if decision.admitted
                && decision.error.is_none()
                && !decision.evidence_ref.is_empty() => {}
        Some(decision) => {
            return Err(format!(
                "Created -> Planning: unexpected entry decision (admitted={}, error={:?})",
                decision.admitted,
                decision.error.as_ref().map(|error| error.error_class())
            ));
        }
        None => return Err("Created -> Planning: decision missing from the report history".into()),
    }
    let mut step = 1usize;

    // Step 1: Planning -> Searching — the plan digest (also the ref of an
    // invalid-plan refusal: the error records the same digest).
    let plan_ref = plan_digest(plan);
    check(step, "plan digest", &plan_ref)?;
    step += 1;
    if sink_is_next(step) {
        return sink_check();
    }

    // Step 2: Searching -> Reading — the observed identity ref
    // (`da.identity:<semantic digest>`, or `da.identity:unavailable` when
    // the plane was never reachable); both admitted and blocked steps carry
    // exactly this ref.
    let identity_ref = match &report.actual_identity {
        Some(identity) => format!("da.identity:{}", identity.semantic_digest.as_str()),
        None => "da.identity:unavailable".to_string(),
    };
    check(step, "identity ref", &identity_ref)?;
    step += 1;
    if sink_is_next(step) {
        return sink_check();
    }

    // Step 3: Reading -> Synthesis — the evidence digest when admitted; a
    // budget/transport refusal ref (`query:<purpose>` / result hash) is a
    // failure record checked for decision/error consistency.
    if matches!(decisions.get(step), Some(decision) if decision.error.is_some()) {
        check_blocked(
            step,
            "evidence refusal record",
            AgentState::Reading,
            AgentState::Synthesis,
        )?;
    } else {
        let evidence_ref = evidence_digest(&report.bundle);
        check(step, "evidence digest", &evidence_ref)?;
    }
    step += 1;
    if sink_is_next(step) {
        return sink_check();
    }

    // Step 4: Synthesis -> Verification — the claims digest when admitted;
    // the synthesis-budget refusal record (`synthesis:claims`) otherwise.
    if matches!(decisions.get(step), Some(decision) if decision.error.is_some()) {
        check_blocked(
            step,
            "claims refusal record",
            AgentState::Synthesis,
            AgentState::Verification,
        )?;
    } else {
        let claims_ref = claims_digest(&report.bundle);
        check(step, "claims digest", &claims_ref)?;
    }
    step += 1;
    if sink_is_next(step) {
        return sink_check();
    }

    // Step 5: Verification -> Verified — the verification digest over the
    // bundle plus the identity observed at verification; a
    // read_plane_unavailable refusal records `da.identity:unavailable`.
    let verification_ref = match &report.error {
        Some(error)
            if error.code == AgentFailureCode::ReadPlaneUnavailable
                && error.from == AgentState::Verification
                && error.to == AgentState::Verified =>
        {
            "da.identity:unavailable".to_string()
        }
        _ => match &report.actual_identity {
            Some(identity) => verification_digest(&report.bundle, identity),
            None => "da.identity:unavailable".to_string(),
        },
    };
    check(step, "verification digest", &verification_ref)?;
    step += 1;
    if sink_is_next(step) {
        return sink_check();
    }

    // Cross-checks (S04 verify contract).
    if report.terminal_state == AgentState::Verified {
        match &report.actual_identity {
            Some(actual) if actual == &report.expected_identity => {}
            Some(actual) => {
                return Err(format!(
                    "identity equality: a Verified run observed identity {} but planned against {}",
                    actual.semantic_digest.as_str(),
                    report.expected_identity.semantic_digest.as_str()
                ));
            }
            None => return Err("identity equality: a Verified run has no actual identity".into()),
        }
        if !report.bundle.all_claims_grounded() {
            return Err("claim grounding: a Verified run has a dangling claim".into());
        }
    }
    if let Some(error) = &report.error
        && error.code == AgentFailureCode::MissingEvidence
        && report.bundle.all_claims_grounded()
    {
        return Err(
            "claim grounding: missing_evidence recorded but every claim is grounded".into(),
        );
    }
    if report.bundle.expected_identity != report.expected_identity {
        return Err(
            "bundle identity: report bundle expected_identity disagrees with the run expected_identity"
                .into(),
        );
    }
    Ok(())
}

/// Build the terminal `FailedVisible` report: records the failure-sink
/// admission (`from → FailedVisible`, always admitted by topology) and
/// carries the terminal typed error (R087).
fn failed_report(
    decisions: Vec<AgentAdmissionDecision>,
    bundle: ContextBundle,
    budget: AgentBudget,
    expected_identity: &GraphIdentity,
    actual_identity: Option<GraphIdentity>,
    from: AgentState,
    error: AgentAdmissionError,
) -> AgentRunReport {
    let mut decisions = decisions;
    let sink = from.admit_transition(AgentState::FailedVisible, &error.evidence_ref);
    decisions.push(sink);
    AgentRunReport {
        terminal_state: AgentState::FailedVisible,
        decisions,
        bundle,
        budget,
        expected_identity: expected_identity.clone(),
        actual_identity,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use da_domain::AgentQueryEvidence;
    use da_ports::{AgentReadPlane, AgentReadPlaneError, AgentReadPlaneResult};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn test_identity() -> GraphIdentity {
        GraphIdentity::new(ContentDigest::of_text("g1"), "tenant-a", 10, 4)
    }

    fn other_identity() -> GraphIdentity {
        GraphIdentity::new(ContentDigest::of_text("g2"), "tenant-b", 12, 5)
    }

    fn happy_results() -> Vec<AgentQueryEvidence> {
        vec![
            AgentQueryEvidence::new(
                "MATCH (p:Paper) RETURN p.title LIMIT 5",
                "sha256:q1",
                3,
                false,
                12,
            ),
            AgentQueryEvidence::new(
                "MATCH (e:Entity) WHERE e.name = 'GRPO' RETURN e.name",
                "sha256:q2",
                1,
                false,
                8,
            ),
        ]
    }

    /// Test fake read plane: canned identity + canned per-query results,
    /// with optional identity/query transport failures and a query-call
    /// counter so tests can assert zero-queries-executed guarantees.
    struct FakeAgentReadPlane {
        identity: GraphIdentity,
        results: Vec<AgentQueryEvidence>,
        query_calls: AtomicU64,
        identity_error: Option<String>,
        query_error: Option<String>,
        identity_calls: AtomicU64,
        fail_identity_on_call: Option<u64>,
    }

    impl FakeAgentReadPlane {
        fn happy(identity: GraphIdentity, results: Vec<AgentQueryEvidence>) -> Self {
            Self {
                identity,
                results,
                query_calls: AtomicU64::new(0),
                identity_error: None,
                query_error: None,
                identity_calls: AtomicU64::new(0),
                fail_identity_on_call: None,
            }
        }

        fn with_identity_error(identity: GraphIdentity, message: &str) -> Self {
            Self {
                identity,
                results: Vec::new(),
                query_calls: AtomicU64::new(0),
                identity_error: Some(message.to_string()),
                query_error: None,
                identity_calls: AtomicU64::new(0),
                fail_identity_on_call: None,
            }
        }

        /// Pre-flight identity succeeds, the verification-time identity
        /// (second call) is refused — the `read_plane_unavailable` path at
        /// `Verification → Verified` (R091 double check).
        fn with_late_identity_error(
            identity: GraphIdentity,
            results: Vec<AgentQueryEvidence>,
            message: &str,
        ) -> Self {
            Self {
                identity,
                results,
                query_calls: AtomicU64::new(0),
                identity_error: Some(message.to_string()),
                query_error: None,
                identity_calls: AtomicU64::new(0),
                fail_identity_on_call: Some(2),
            }
        }

        fn with_query_error(
            identity: GraphIdentity,
            results: Vec<AgentQueryEvidence>,
            message: &str,
        ) -> Self {
            Self {
                identity,
                results,
                query_calls: AtomicU64::new(0),
                identity_error: None,
                query_error: Some(message.to_string()),
                identity_calls: AtomicU64::new(0),
                fail_identity_on_call: None,
            }
        }

        fn query_calls(&self) -> u64 {
            self.query_calls.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl AgentReadPlane for FakeAgentReadPlane {
        async fn identity(&self) -> AgentReadPlaneResult<GraphIdentity> {
            let call = self.identity_calls.fetch_add(1, Ordering::Relaxed) + 1;
            let fail_now = match self.fail_identity_on_call {
                Some(fail_on) => call == fail_on,
                None => self.identity_error.is_some(),
            };
            if fail_now {
                return Err(AgentReadPlaneError::Transport(
                    self.identity_error
                        .clone()
                        .unwrap_or_else(|| "identity unavailable".into()),
                ));
            }
            Ok(self.identity.clone())
        }

        async fn query(
            &self,
            _graph: &str,
            _cypher: &str,
            _max_rows: u64,
        ) -> AgentReadPlaneResult<AgentQueryEvidence> {
            if let Some(message) = &self.query_error {
                return Err(AgentReadPlaneError::Transport(message.clone()));
            }
            let index = self.query_calls.fetch_add(1, Ordering::Relaxed) as usize;
            Ok(self
                .results
                .get(index)
                .cloned()
                .expect("fake plane has a canned result for every plan query"))
        }
    }

    fn sample_request() -> AgentRunRequest {
        AgentRunRequest {
            entry_evidence_ref: "sha256:entry-v1".into(),
            expected_identity: test_identity(),
            graph: "hub".into(),
            plan: vec![
                AgentPlanQuery {
                    query: "MATCH (p:Paper) RETURN p.title LIMIT 5".into(),
                    purpose: "paper titles".into(),
                },
                AgentPlanQuery {
                    query: "MATCH (e:Entity) WHERE e.name = 'GRPO' RETURN e.name".into(),
                    purpose: "GRPO entity".into(),
                },
            ],
            budget: AgentBudget::new(2, 4, 1000),
            max_rows_per_query: 50,
        }
    }

    fn use_case(plane: FakeAgentReadPlane) -> AgentRunUseCase {
        AgentRunUseCase::new(Arc::new(plane), Arc::new(DeterministicSynthesizer))
    }

    #[tokio::test]
    async fn legal_walk_reaches_verified_with_full_fsm_history() {
        let plane = Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results()));
        let use_case = AgentRunUseCase::new(plane.clone(), Arc::new(DeterministicSynthesizer));
        let report = use_case.run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::Verified);
        assert!(report.error.is_none(), "happy path carries no error");
        assert_eq!(report.decisions.len(), 6, "six forward admissions");
        assert!(
            report.decisions.iter().all(|decision| decision.admitted),
            "every step admitted on the legal walk"
        );
        // Evidence-ref scheme pins the durable lineage per step.
        assert_eq!(report.decisions[0].evidence_ref, "sha256:entry-v1");
        assert!(report.decisions[1].evidence_ref.starts_with("sha256:plan:"));
        assert!(report.decisions[2].evidence_ref.starts_with("da.identity:"));
        assert!(
            report.decisions[3]
                .evidence_ref
                .starts_with("sha256:evidence:")
        );
        assert!(
            report.decisions[4]
                .evidence_ref
                .starts_with("sha256:claims:")
        );
        assert!(
            report.decisions[5]
                .evidence_ref
                .starts_with("sha256:verify:")
        );
        // All queries executed, all evidence and claims grounded.
        assert_eq!(plane.query_calls(), 2, "both plan queries executed");
        assert_eq!(report.bundle.evidence.len(), 2);
        assert_eq!(report.bundle.claims.len(), 2);
        assert!(report.bundle.all_claims_grounded());
        assert_eq!(report.budget.spent_queries, 2);
        assert_eq!(report.budget.spent_evidence_entries, 2);
        assert!(report.budget.spent_synthesis_chars > 0);
        assert_eq!(report.actual_identity, Some(test_identity()));
        assert_eq!(report.expected_identity, test_identity());
    }

    #[tokio::test]
    async fn wrong_identity_fails_preflight_with_zero_queries() {
        let plane = Arc::new(FakeAgentReadPlane::happy(other_identity(), happy_results()));
        let use_case = AgentRunUseCase::new(plane.clone(), Arc::new(DeterministicSynthesizer));
        let report = use_case.run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::IdentityMismatch);
        assert_eq!(error.error_class(), "agent_failure:identity_mismatch");
        assert_eq!(error.from, AgentState::Searching);
        assert_eq!(error.to, AgentState::Reading);
        // R091: zero queries executed, zero budget spent, no evidence.
        assert_eq!(plane.query_calls(), 0, "zero queries executed");
        assert_eq!(report.bundle.evidence.len(), 0);
        assert_eq!(report.budget.spent_queries, 0);
        // The report names the actual side that mismatched.
        assert_eq!(report.actual_identity, Some(other_identity()));
        // Both sides are named in the reason (R087).
        assert!(error.reason.contains("tenant-a"));
        assert!(error.reason.contains("tenant-b"));
        // History: entry + plan admissions, blocked Searching->Reading, sink.
        assert_eq!(report.decisions.len(), 4);
        assert!(report.decisions[2].error.is_some(), "blocked step recorded");
        assert!(report.decisions[3].admitted, "sink admission recorded");
    }

    struct DanglingSynthesizer;

    impl AgentSynthesizer for DanglingSynthesizer {
        fn synthesize(&self, _bundle: &ContextBundle) -> Vec<AgentClaim> {
            vec![AgentClaim::new("ungrounded claim", "sha256:does-not-exist")]
        }
    }

    #[tokio::test]
    async fn dangling_claim_fails_verification_as_missing_evidence() {
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let use_case = AgentRunUseCase::new(Arc::new(plane), Arc::new(DanglingSynthesizer));
        let report = use_case.run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::MissingEvidence);
        assert_eq!(error.error_class(), "agent_failure:missing_evidence");
        assert_eq!(error.from, AgentState::Verification);
        assert_eq!(error.to, AgentState::Verified);
        assert!(
            error.reason.contains("sha256:does-not-exist"),
            "the dangling ref is named"
        );
        // Queries ran before verification; the failure is at the final guard.
        assert_eq!(report.bundle.evidence.len(), 2);
        assert_eq!(report.budget.spent_queries, 2);
        // History: 5 admitted steps + blocked verification + sink.
        assert_eq!(report.decisions.len(), 7);
    }

    #[tokio::test]
    async fn budget_overflow_fails_closed_with_counters_unchanged() {
        let mut request = sample_request();
        // Plan has 2 queries; the evidence budget admits only 1 entry, so the
        // second query's evidence charge overflows after the query executed.
        request.budget = AgentBudget::new(2, 1, 1000);
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::BudgetOverflow);
        assert_eq!(error.error_class(), "agent_failure:budget_overflow");
        assert_eq!(error.from, AgentState::Reading);
        assert_eq!(error.to, AgentState::Synthesis);
        assert!(error.reason.contains("1/1"));
        // Fail-closed: the refusal happened BEFORE the counter advanced.
        assert_eq!(report.budget.spent_queries, 2);
        assert_eq!(
            report.budget.spent_evidence_entries, 1,
            "refusal must not advance the evidence counter"
        );
        assert_eq!(report.bundle.evidence.len(), 1);
    }

    #[tokio::test]
    async fn synthesis_budget_overflow_fails_closed_before_claims_enter_bundle() {
        let mut request = sample_request();
        // Zero synthesis chars allowed: the claims are refused atomically.
        request.budget = AgentBudget::new(2, 4, 0);
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::BudgetOverflow);
        assert_eq!(error.error_class(), "agent_failure:budget_overflow");
        assert_eq!(error.from, AgentState::Synthesis);
        assert_eq!(error.to, AgentState::Verification);
        assert_eq!(
            report.budget.spent_synthesis_chars, 0,
            "refusal must not advance the synthesis counter"
        );
        assert_eq!(
            report.bundle.claims.len(),
            0,
            "no claims entered the bundle"
        );
    }

    #[tokio::test]
    async fn truncated_passthrough_preserves_explicit_flag() {
        let plane = FakeAgentReadPlane::happy(
            test_identity(),
            vec![
                AgentQueryEvidence::new(
                    "MATCH (p:Paper) RETURN p.title LIMIT 5",
                    "sha256:q1",
                    0,
                    true, // explicit truncation: rows == 0 but cut off (R087)
                    4,
                ),
                AgentQueryEvidence::new(
                    "MATCH (e:Entity) WHERE e.name = 'GRPO' RETURN e.name",
                    "sha256:q2",
                    1,
                    false,
                    6,
                ),
            ],
        );
        let report = use_case(plane).run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::Verified);
        assert!(report.bundle.evidence[0].truncated);
        assert_eq!(report.bundle.evidence[0].rows, 0);
        // The default synthesizer names truncation in the grounded claim.
        assert!(report.bundle.claims[0].text.contains("truncated"));
        assert!(report.bundle.claims[0].text.contains("returned 0 rows"));
        assert_eq!(report.bundle.claims[0].evidence_ref, "sha256:q1");
    }

    #[tokio::test]
    async fn mutating_plan_is_invalid_plan_before_any_query() {
        let mut request = sample_request();
        request.plan[0].query = "CREATE (n:Paper {title: 'x'})".into();
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::InvalidPlan);
        assert_eq!(error.error_class(), "agent_failure:invalid_plan");
        assert_eq!(error.from, AgentState::Planning);
        assert_eq!(error.to, AgentState::Searching);
        assert!(error.reason.contains("read-only"));
        assert_eq!(report.budget.spent_queries, 0, "no query executed");
        assert_eq!(report.bundle.evidence.len(), 0);
        // History: entry admission + blocked plan step + sink.
        assert_eq!(report.decisions.len(), 3);
    }

    #[tokio::test]
    async fn empty_plan_is_invalid_plan() {
        let mut request = sample_request();
        request.plan.clear();
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::InvalidPlan);
        assert_eq!(error.error_class(), "agent_failure:invalid_plan");
        assert_eq!(error.from, AgentState::Planning);
        assert_eq!(error.to, AgentState::Searching);
        assert!(error.reason.contains("empty"));
        assert_eq!(report.budget.spent_queries, 0);
    }

    #[tokio::test]
    async fn over_budget_plan_is_invalid_plan() {
        let mut request = sample_request();
        request.plan.push(AgentPlanQuery {
            query: "MATCH (r:Reference) RETURN r.title".into(),
            purpose: "references".into(),
        });
        // max_queries = 2 < plan.len() = 3.
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::InvalidPlan);
        assert_eq!(error.error_class(), "agent_failure:invalid_plan");
        assert!(error.reason.contains("over-budget"));
        assert_eq!(report.budget.spent_queries, 0);
    }

    #[tokio::test]
    async fn zero_max_rows_is_invalid_plan() {
        let mut request = sample_request();
        request.max_rows_per_query = 0;
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::InvalidPlan);
        assert_eq!(error.error_class(), "agent_failure:invalid_plan");
        assert!(error.reason.contains("max_rows_per_query"));
    }

    #[tokio::test]
    async fn blank_entry_evidence_is_missing_evidence_at_entry() {
        let mut request = sample_request();
        request.entry_evidence_ref = "   ".into();
        let plane = FakeAgentReadPlane::happy(test_identity(), happy_results());
        let report = use_case(plane).run(&request).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::MissingEvidence);
        assert_eq!(error.error_class(), "agent_failure:missing_evidence");
        assert_eq!(error.from, AgentState::Created);
        assert_eq!(error.to, AgentState::Planning);
        assert_eq!(report.budget.spent_queries, 0);
        // History: blocked Created->Planning + sink.
        assert_eq!(report.decisions.len(), 2);
    }

    #[tokio::test]
    async fn identity_unavailable_is_read_plane_unavailable() {
        let plane = FakeAgentReadPlane::with_identity_error(
            test_identity(),
            "connect refused: DA.IDENTITY unreachable",
        );
        let report = use_case(plane).run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::ReadPlaneUnavailable);
        assert_eq!(error.error_class(), "agent_failure:read_plane_unavailable");
        assert_eq!(error.from, AgentState::Searching);
        assert_eq!(error.to, AgentState::Reading);
        assert!(error.reason.contains("identity unavailable"));
        assert!(report.actual_identity.is_none(), "plane never reachable");
        assert_eq!(report.budget.spent_queries, 0);
    }

    #[tokio::test]
    async fn query_transport_failure_is_read_plane_unavailable() {
        let plane = FakeAgentReadPlane::with_query_error(
            test_identity(),
            happy_results(),
            "connection lost mid-query",
        );
        let report = use_case(plane).run(&sample_request()).await;

        assert_eq!(report.terminal_state, AgentState::FailedVisible);
        let error = report.error.expect("typed visible failure");
        assert_eq!(error.code, AgentFailureCode::ReadPlaneUnavailable);
        assert_eq!(error.error_class(), "agent_failure:read_plane_unavailable");
        assert_eq!(error.from, AgentState::Reading);
        assert_eq!(error.to, AgentState::Synthesis);
        assert!(error.reason.contains("query failed"));
        // The attempted query consumed its budget slot (fail-closed).
        assert_eq!(report.budget.spent_queries, 1);
        assert_eq!(report.bundle.evidence.len(), 0);
    }

    #[tokio::test]
    async fn request_and_report_serde_roundtrip_is_stable() {
        crate::roundtrip_json(&sample_request());
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let report = use_case.run(&sample_request()).await;
        crate::roundtrip_json(&report);
        // Wire shape is binding for S04's atomic JSON report.
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"terminal_state\":\"verified\""));
        assert!(json.contains("\"spent_queries\":2"));
        assert!(json.contains("\"expected_identity\""));
        assert!(json.contains("\"actual_identity\""));
    }

    #[tokio::test]
    async fn failed_report_wire_carries_snake_case_code() {
        let mut request = sample_request();
        request.plan[0].query = "MERGE (n:Paper)".into();
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let report = use_case.run(&request).await;
        crate::roundtrip_json(&report);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"terminal_state\":\"failed_visible\""));
        assert!(json.contains("\"code\":\"invalid_plan\""));
    }

    // ---- S04 T01: public evidence-chain verifier ----

    /// Assert the chain recomputes cleanly, with the failing check named.
    /// S04 T01 public-surface lock: the re-exported digest helpers compute
    /// exactly the evidence refs the orchestrator records, so CLI capture
    /// and `--verify` (which recompute from persisted data) cannot drift.
    /// Referenced through the crate-root re-export path on purpose.
    #[tokio::test]
    async fn public_digest_reexports_match_recorded_evidence_refs() {
        let report = use_case(FakeAgentReadPlane::happy(test_identity(), happy_results()))
            .run(&sample_request())
            .await;
        assert_eq!(report.terminal_state, AgentState::Verified);

        let plan = &sample_request().plan;
        let bundle = &report.bundle;
        let identity = test_identity();

        // Re-exported surface (crate root), not the local `super::*` imports.
        assert_eq!(crate::plan_digest(plan), report.decisions[1].evidence_ref);
        assert_eq!(
            crate::evidence_digest(bundle),
            report.decisions[3].evidence_ref
        );
        assert_eq!(
            crate::claims_digest(bundle),
            report.decisions[4].evidence_ref
        );
        assert_eq!(
            crate::verification_digest(bundle, &identity),
            report.decisions[5].evidence_ref
        );
        // Scheme prefixes lock the canonical shape.
        assert!(crate::plan_digest(plan).starts_with("sha256:plan:"));
        assert!(crate::evidence_digest(bundle).starts_with("sha256:evidence:"));
        assert!(crate::claims_digest(bundle).starts_with("sha256:claims:"));
        assert!(crate::verification_digest(bundle, &identity).starts_with("sha256:verify:"));
    }

    fn expect_chain_ok(plan: &[AgentPlanQuery], report: &AgentRunReport) {
        if let Err(check) = verify_run_evidence_chain(plan, report) {
            panic!("evidence chain must verify on this report: {check}");
        }
    }

    #[tokio::test]
    async fn verify_chain_recomputes_happy_path_from_persisted_data() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let request = sample_request();
        let report = use_case.run(&request).await;
        assert_eq!(report.terminal_state, AgentState::Verified);

        // Persist exactly what S04 puts in the envelope (plan JSON + report
        // JSON, serde-stable), then recompute from the deserialized copies —
        // the S05 shell-out contract (D273).
        let plan: Vec<AgentPlanQuery> =
            serde_json::from_str(&serde_json::to_string(&request.plan).unwrap()).unwrap();
        let report: AgentRunReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        expect_chain_ok(&plan, &report);
    }

    #[tokio::test]
    async fn verify_chain_detects_plan_tampering() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let mut request = sample_request();
        let report = use_case.run(&request).await;
        // The envelope's persisted plan no longer matches the run's plan.
        request.plan[0].query = "MATCH (p:Paper) RETURN p.vid".into();
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(err.starts_with("plan digest"), "first failing check: {err}");
    }

    #[tokio::test]
    async fn verify_chain_detects_evidence_tampering() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let request = sample_request();
        let mut report = use_case.run(&request).await;
        report.bundle.evidence[0].result_ref = "sha256:tampered".into();
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(
            err.starts_with("evidence digest"),
            "first failing check: {err}"
        );
    }

    #[tokio::test]
    async fn verify_chain_detects_claim_tampering_on_verified_run() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let request = sample_request();
        let mut report = use_case.run(&request).await;
        report.bundle.claims[0].text = "forged claim text".into();
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(
            err.starts_with("claims digest"),
            "first failing check: {err}"
        );
    }

    #[tokio::test]
    async fn verify_chain_detects_identity_and_history_tampering() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let request = sample_request();
        // Observed identity swapped -> identity ref (and later verify digest)
        // no longer recompute.
        let mut report = use_case.run(&request).await;
        report.actual_identity = Some(other_identity());
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(
            err.starts_with("identity ref"),
            "first failing check: {err}"
        );

        // Recorded decision history tampered -> plan digest mismatch. A
        // fresh plane is needed: the fake consumes one canned result per
        // query call.
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let mut report = use_case.run(&request).await;
        report.decisions[1].evidence_ref = "sha256:plan:deadbeef".into();
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(err.starts_with("plan digest"), "first failing check: {err}");
    }

    #[tokio::test]
    async fn verify_chain_detects_forged_failure_sink() {
        let use_case = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DeterministicSynthesizer),
        );
        let mut request = sample_request();
        request.budget = AgentBudget::new(2, 1, 1000); // evidence overflow at Reading
        let mut report = use_case.run(&request).await;
        report.decisions.last_mut().unwrap().evidence_ref = "sha256:forged".into();
        let err = verify_run_evidence_chain(&request.plan, &report).unwrap_err();
        assert!(
            err.starts_with("failure sink"),
            "first failing check: {err}"
        );
    }

    #[tokio::test]
    async fn verify_chain_recomputes_blocked_reports() {
        // Pre-flight identity mismatch: zero queries, 4 decisions.
        let report = use_case(FakeAgentReadPlane::happy(other_identity(), happy_results()))
            .run(&sample_request())
            .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::IdentityMismatch)
        );
        expect_chain_ok(&sample_request().plan, &report);

        // Read plane never reachable: identity unavailable, zero queries.
        let report = use_case(FakeAgentReadPlane::with_identity_error(
            test_identity(),
            "connection refused",
        ))
        .run(&sample_request())
        .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::ReadPlaneUnavailable)
        );
        assert!(report.actual_identity.is_none());
        expect_chain_ok(&sample_request().plan, &report);

        // Evidence budget overflow at Reading: refusal before counters advance.
        let mut request = sample_request();
        request.budget = AgentBudget::new(2, 1, 1000);
        let report = use_case(FakeAgentReadPlane::happy(test_identity(), happy_results()))
            .run(&request)
            .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::BudgetOverflow)
        );
        expect_chain_ok(&request.plan, &report);

        // Query transport failure at Reading: partial evidence, refusal record.
        let report = use_case(FakeAgentReadPlane::with_query_error(
            test_identity(),
            happy_results(),
            "connection lost mid-query",
        ))
        .run(&sample_request())
        .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::ReadPlaneUnavailable)
        );
        expect_chain_ok(&sample_request().plan, &report);

        // Synthesis chars budget overflow: claims refused atomically.
        let mut request = sample_request();
        request.budget = AgentBudget::new(2, 4, 0);
        let report = use_case(FakeAgentReadPlane::happy(test_identity(), happy_results()))
            .run(&request)
            .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::BudgetOverflow)
        );
        expect_chain_ok(&request.plan, &report);

        // Dangling claims at verification: missing_evidence.
        let report = AgentRunUseCase::new(
            Arc::new(FakeAgentReadPlane::happy(test_identity(), happy_results())),
            Arc::new(DanglingSynthesizer),
        )
        .run(&sample_request())
        .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::MissingEvidence)
        );
        expect_chain_ok(&sample_request().plan, &report);

        // Verification-time identity refusal: `da.identity:unavailable` at
        // Verification -> Verified (R091 double check).
        let report = use_case(FakeAgentReadPlane::with_late_identity_error(
            test_identity(),
            happy_results(),
            "connection lost at verification",
        ))
        .run(&sample_request())
        .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::ReadPlaneUnavailable)
        );
        assert_eq!(
            report.error.as_ref().unwrap().from,
            AgentState::Verification
        );
        assert_eq!(report.actual_identity, Some(test_identity()));
        expect_chain_ok(&sample_request().plan, &report);

        // Invalid plan (over budget): refuses before any query.
        let mut request = sample_request();
        request.budget = AgentBudget::new(1, 4, 1000);
        let report = use_case(FakeAgentReadPlane::happy(test_identity(), happy_results()))
            .run(&request)
            .await;
        assert_eq!(
            report.error.as_ref().map(|e| e.code),
            Some(AgentFailureCode::InvalidPlan)
        );
        expect_chain_ok(&request.plan, &report);
    }
}
