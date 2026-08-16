//! PromotionService use case — fail-closed promotion commit boundary
//! (PIR-015).
//!
//! The service is the application-side half of the unified promotion protocol
//! (slice S02). It takes a typed transition request, validates it against the
//! pure domain FSM (`da_domain::promotion`), builds a durable
//! [`PromotionDecision`] carrying proposer/verifier/reviewer/promoter identity
//! and evidence references, and publishes candidate state plus evidence only
//! through a single [`PromotionCommitSink::commit`] call.
//!
//! Fail-closed order (nothing is ever exposed as promoted on a failure):
//! 1. input validation (non-empty `decision_id`);
//! 2. domain FSM transition — typed refusal
//!    ([`PromotionError`]) returns before any commit;
//! 3. durable decision built with the injected [`Clock`] stamp;
//! 4. exactly one sink commit; a sink failure returns
//!    [`PromotionServiceError::Commit`] — the caller receives an error, never
//!    a partial success carrying a promoted candidate.
//!
//! The service is authority-neutral: the same boundary serves Prompt, Gold,
//! and EntityResolution candidates (`PromotionKind`). It does not unlock
//! import (`import_eligible` stays false under D127), does not enable
//! optimizer auto-promotion, and adds no external I/O beyond the injected sink.

use std::sync::Arc;

use da_domain::promotion::{
    EvidenceRef, PromotionCandidate, PromotionError, PromotionKind, PromotionState,
};
use da_ports::promotion::{PromotionCommitError, PromotionCommitSink, PromotionDecision};
use tracing::instrument;

/// The promotion transition to commit through the fail-closed boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionAction {
    /// Propose a new candidate (kind + candidate_id + proposer + evidence).
    Propose,
    /// Independent verification of a proposed candidate.
    Verify,
    /// Independent review of a verified candidate.
    Review,
    /// Promotion of a reviewed candidate (exactly once).
    Promote,
    /// Rejection from any non-terminal state.
    Reject,
}

/// Request to commit one promotion transition. Each variant carries the
/// durable `decision_id` (idempotent replay key) plus the transition-specific
/// inputs; advancing transitions require non-empty `evidence_refs`, which the
/// domain FSM enforces before anything is committed.
#[derive(Debug, Clone)]
pub enum PromotionTransitionRequest {
    /// Commit a new proposal.
    Propose {
        decision_id: String,
        kind: PromotionKind,
        candidate_id: String,
        proposer: String,
        evidence_refs: Vec<EvidenceRef>,
    },
    /// Commit an independent verification.
    Verify {
        decision_id: String,
        candidate: PromotionCandidate,
        verifier: String,
        evidence_refs: Vec<EvidenceRef>,
    },
    /// Commit a review.
    Review {
        decision_id: String,
        candidate: PromotionCandidate,
        reviewer: String,
        evidence_refs: Vec<EvidenceRef>,
    },
    /// Commit a promotion (exactly once per candidate).
    Promote {
        decision_id: String,
        candidate: PromotionCandidate,
        promoter: String,
        evidence_refs: Vec<EvidenceRef>,
    },
    /// Commit a rejection with a reason.
    Reject {
        decision_id: String,
        candidate: PromotionCandidate,
        reason: String,
    },
}

impl PromotionTransitionRequest {
    /// The durable decision id for the resulting record.
    pub fn decision_id(&self) -> &str {
        match self {
            Self::Propose { decision_id, .. }
            | Self::Verify { decision_id, .. }
            | Self::Review { decision_id, .. }
            | Self::Promote { decision_id, .. }
            | Self::Reject { decision_id, .. } => decision_id,
        }
    }

    /// The transition this request commits.
    pub fn action(&self) -> PromotionAction {
        match self {
            Self::Propose { .. } => PromotionAction::Propose,
            Self::Verify { .. } => PromotionAction::Verify,
            Self::Review { .. } => PromotionAction::Review,
            Self::Promote { .. } => PromotionAction::Promote,
            Self::Reject { .. } => PromotionAction::Reject,
        }
    }

    /// Candidate id targeted by the request (`"<new>"` for a proposal).
    pub fn candidate_id(&self) -> &str {
        match self {
            Self::Propose { candidate_id, .. } => candidate_id,
            Self::Verify { candidate, .. }
            | Self::Review { candidate, .. }
            | Self::Promote { candidate, .. }
            | Self::Reject { candidate, .. } => &candidate.candidate_id,
        }
    }

    /// The actor performing the transition.
    pub fn actor(&self) -> &str {
        match self {
            Self::Propose { proposer, .. } => proposer,
            Self::Verify { verifier, .. } => verifier,
            Self::Review { reviewer, .. } => reviewer,
            Self::Promote { promoter, .. } => promoter,
            Self::Reject { reason, .. } => reason,
        }
    }

    /// Number of new evidence references carried by the request.
    pub fn evidence_count(&self) -> usize {
        match self {
            Self::Propose { evidence_refs, .. }
            | Self::Verify { evidence_refs, .. }
            | Self::Review { evidence_refs, .. }
            | Self::Promote { evidence_refs, .. } => evidence_refs.len(),
            Self::Reject { .. } => 0,
        }
    }
}

/// Typed refusal / failure of a promotion commit attempt. Every attempt
/// exposes a typed reason: either the domain refused the transition
/// ([`PromotionServiceError::Domain`]) or the durable commit failed
/// ([`PromotionServiceError::Commit`]). On any error, no candidate state or
/// evidence was published as promoted.
#[derive(Debug, thiserror::Error)]
pub enum PromotionServiceError {
    /// The durable decision id must be non-empty.
    #[error("promotion decision id must be non-empty")]
    EmptyDecisionId,
    /// The domain FSM refused the transition (self-confirmation, missing
    /// evidence, double promotion, illegal transition, terminal mutation,
    /// import leak, ...).
    #[error("promotion transition refused: {0}")]
    Domain(#[from] PromotionError),
    /// The durable commit failed; nothing was published.
    #[error("promotion decision commit failed: {0}")]
    Commit(#[from] PromotionCommitError),
}

/// Fail-closed promotion commit service.
///
/// Holds the injected [`Clock`] (ADR-051 Contour 1) and the
/// [`PromotionCommitSink`] port. All state transitions and evidence are
/// published only through [`PromotionService::commit`], which performs at most
/// one sink call per attempt.
pub struct PromotionService {
    clock: Arc<dyn da_ports::Clock>,
    sink: Arc<dyn PromotionCommitSink>,
}

impl PromotionService {
    /// Service with a system clock and an owned sink.
    pub fn new(sink: Box<dyn PromotionCommitSink>) -> Self {
        Self::from_shared_with_clock(Arc::from(sink), Arc::new(da_ports::SystemClock))
    }

    /// Service with a system clock and a shared sink.
    pub fn from_shared(sink: Arc<dyn PromotionCommitSink>) -> Self {
        Self::from_shared_with_clock(sink, Arc::new(da_ports::SystemClock))
    }

    /// Service with an injected clock and an owned sink.
    pub fn with_clock(sink: Box<dyn PromotionCommitSink>, clock: Arc<dyn da_ports::Clock>) -> Self {
        Self::from_shared_with_clock(Arc::from(sink), clock)
    }

    /// Service with an injected clock and a shared sink.
    pub fn from_shared_with_clock(
        sink: Arc<dyn PromotionCommitSink>,
        clock: Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self { clock, sink }
    }

    /// Commit one promotion transition through the fail-closed boundary.
    ///
    /// On success returns the durable [`PromotionDecision`] — candidate state,
    /// all identities, evidence references, transition result, and the Clock
    /// stamp. On failure returns a typed refusal; the sink was not called or
    /// its failure was surfaced, so no partial success is exposed as promoted.
    #[instrument(skip_all, fields(
        decision_id = %request.decision_id(),
        action = ?request.action(),
        candidate_id = %request.candidate_id(),
        actor = %request.actor(),
        evidence_count = request.evidence_count(),
    ))]
    pub async fn commit(
        &self,
        request: PromotionTransitionRequest,
    ) -> Result<PromotionDecision, PromotionServiceError> {
        if request.decision_id().trim().is_empty() {
            return Err(PromotionServiceError::EmptyDecisionId);
        }

        // 1) Validate the domain transition — typed refusal, nothing committed.
        let decision_id = request.decision_id().to_string();
        let (transition, candidate) = self.apply_transition(request)?;

        // 2) Build the durable decision and publish through ONE sink call.
        let decision = PromotionDecision {
            decision_id,
            transition,
            committed_at: self.clock.now().to_rfc3339(),
            candidate,
        };
        self.sink.commit(&decision).await?;
        tracing::info!(
            candidate_id = %decision.candidate.candidate_id,
            kind = ?decision.candidate.kind,
            transition = ?decision.transition,
            evidence_refs = decision.candidate.evidence_refs.len(),
            decision_id = %decision.decision_id,
            "promotion decision committed"
        );
        Ok(decision)
    }

    /// Apply the domain FSM transition. Pure — no clock, no sink. The returned
    /// candidate is validated by the domain before this method returns, so any
    /// refusal happens before the durable commit step.
    fn apply_transition(
        &self,
        request: PromotionTransitionRequest,
    ) -> Result<(PromotionState, PromotionCandidate), PromotionServiceError> {
        match request {
            PromotionTransitionRequest::Propose {
                kind,
                candidate_id,
                proposer,
                evidence_refs,
                ..
            } => {
                let candidate =
                    PromotionCandidate::propose(kind, candidate_id, proposer, evidence_refs)?;
                Ok((PromotionState::Proposed, candidate))
            }
            PromotionTransitionRequest::Verify {
                candidate,
                verifier,
                evidence_refs,
                ..
            } => {
                candidate.validate()?;
                let candidate = candidate.verify(verifier, evidence_refs)?;
                Ok((PromotionState::Verified, candidate))
            }
            PromotionTransitionRequest::Review {
                candidate,
                reviewer,
                evidence_refs,
                ..
            } => {
                candidate.validate()?;
                let candidate = candidate.review(reviewer, evidence_refs)?;
                Ok((PromotionState::Reviewed, candidate))
            }
            PromotionTransitionRequest::Promote {
                candidate,
                promoter,
                evidence_refs,
                ..
            } => {
                candidate.validate()?;
                let candidate = candidate.promote(promoter, evidence_refs)?;
                Ok((PromotionState::Promoted, candidate))
            }
            PromotionTransitionRequest::Reject {
                candidate, reason, ..
            } => {
                candidate.validate()?;
                let candidate = candidate.reject(reason)?;
                Ok((PromotionState::Rejected, candidate))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::{FixedClock, PromotionCommitSink};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// In-memory sink fake mirroring the port contract (validate, idempotent
    /// replay, conflict on same decision id, optional forced failure).
    #[derive(Default)]
    struct FakeSink {
        decisions: Mutex<BTreeMap<String, PromotionDecision>>,
        fail_next: Mutex<bool>,
    }

    impl FakeSink {
        fn stored(&self, decision_id: &str) -> Option<PromotionDecision> {
            self.decisions.lock().unwrap().get(decision_id).cloned()
        }

        fn len(&self) -> usize {
            self.decisions.lock().unwrap().len()
        }

        /// Fail the next commit with a storage error (partial-commit probe).
        fn fail_next_commit(&self) {
            *self.fail_next.lock().unwrap() = true;
        }
    }

    #[async_trait::async_trait]
    impl PromotionCommitSink for FakeSink {
        async fn commit(&self, decision: &PromotionDecision) -> Result<(), PromotionCommitError> {
            decision.validate()?;
            if *self.fail_next.lock().unwrap() {
                *self.fail_next.lock().unwrap() = false;
                return Err(PromotionCommitError::Storage(
                    "simulated backend outage".into(),
                ));
            }
            let mut decisions = self.decisions.lock().unwrap();
            match decisions.get(&decision.decision_id) {
                Some(existing) if existing == decision => Ok(()),
                Some(_) => Err(PromotionCommitError::Conflict {
                    decision_id: decision.decision_id.clone(),
                    candidate_id: decision.candidate.candidate_id.clone(),
                }),
                None => {
                    decisions.insert(decision.decision_id.clone(), decision.clone());
                    Ok(())
                }
            }
        }
    }

    fn evidence(reference: &str) -> EvidenceRef {
        EvidenceRef::new(reference)
    }

    fn fixed_clock() -> Arc<dyn da_ports::Clock> {
        Arc::new(FixedClock::from_timestamp(1_780_000_000))
    }

    fn service_with(sink: FakeSink) -> (PromotionService, Arc<FakeSink>) {
        let sink = Arc::new(sink);
        let service = PromotionService::from_shared_with_clock(sink.clone(), fixed_clock());
        (service, sink)
    }

    fn proposed_candidate() -> PromotionCandidate {
        PromotionCandidate::propose(
            PromotionKind::Gold,
            "cand-1",
            "alice",
            vec![evidence("ev:propose:1")],
        )
        .expect("propose")
    }

    fn verified_candidate() -> PromotionCandidate {
        proposed_candidate()
            .verify("bob", vec![evidence("ev:verify:1")])
            .expect("verify")
    }

    fn reviewed_candidate() -> PromotionCandidate {
        verified_candidate()
            .review("carol", vec![evidence("ev:review:1")])
            .expect("review")
    }

    #[tokio::test]
    async fn promotion_service_commits_full_legal_path() {
        let (service, sink) = service_with(FakeSink::default());

        let proposed = service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "dec-1".into(),
                kind: PromotionKind::EntityResolution,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:1")],
            })
            .await
            .expect("propose commits");
        assert_eq!(proposed.transition, PromotionState::Proposed);
        assert!(!proposed.candidate.import_eligible);

        let verified = service
            .commit(PromotionTransitionRequest::Verify {
                decision_id: "dec-2".into(),
                candidate: proposed.candidate.clone(),
                verifier: "bob".into(),
                evidence_refs: vec![evidence("ev:verify:1")],
            })
            .await
            .expect("verify commits");
        assert_eq!(verified.transition, PromotionState::Verified);
        assert_eq!(verified.candidate.verifier.as_deref(), Some("bob"));

        let reviewed = service
            .commit(PromotionTransitionRequest::Review {
                decision_id: "dec-3".into(),
                candidate: verified.candidate.clone(),
                reviewer: "carol".into(),
                evidence_refs: vec![evidence("ev:review:1")],
            })
            .await
            .expect("review commits");
        assert_eq!(reviewed.transition, PromotionState::Reviewed);
        assert_eq!(reviewed.candidate.reviewer.as_deref(), Some("carol"));

        let promoted = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-4".into(),
                candidate: reviewed.candidate.clone(),
                promoter: "dave".into(),
                evidence_refs: vec![evidence("ev:promote:1")],
            })
            .await
            .expect("promote commits");

        assert_eq!(promoted.transition, PromotionState::Promoted);
        assert!(promoted.candidate.state.is_terminal());
        assert_eq!(promoted.candidate.kind, PromotionKind::EntityResolution);
        assert_eq!(promoted.candidate.proposer, "alice");
        assert_eq!(promoted.candidate.verifier.as_deref(), Some("bob"));
        assert_eq!(promoted.candidate.reviewer.as_deref(), Some("carol"));
        assert_eq!(promoted.candidate.promoter.as_deref(), Some("dave"));
        assert_eq!(promoted.candidate.evidence_refs.len(), 4);
        assert!(!promoted.candidate.import_eligible);
        // Every decision was durably stored by the single sink boundary.
        assert_eq!(sink.len(), 4);
        assert_eq!(sink.stored("dec-4"), Some(promoted));
    }

    #[tokio::test]
    async fn promotion_service_refuses_self_confirmation_before_commit() {
        let (service, sink) = service_with(FakeSink::default());
        let err = service
            .commit(PromotionTransitionRequest::Verify {
                decision_id: "dec-x".into(),
                candidate: proposed_candidate(),
                verifier: "alice".into(), // the proposer
                evidence_refs: vec![evidence("ev:verify:1")],
            })
            .await
            .expect_err("self-confirmation must be refused");
        assert!(matches!(
            err,
            PromotionServiceError::Domain(PromotionError::SelfConfirmation { .. })
        ));
        assert_eq!(sink.len(), 0, "no decision may be committed on refusal");
    }

    #[tokio::test]
    async fn promotion_service_refuses_missing_evidence_before_commit() {
        let (service, sink) = service_with(FakeSink::default());
        let err = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-x".into(),
                candidate: reviewed_candidate(),
                promoter: "dave".into(),
                evidence_refs: vec![],
            })
            .await
            .expect_err("missing evidence must be refused");
        assert!(matches!(
            err,
            PromotionServiceError::Domain(PromotionError::MissingEvidence)
        ));
        assert_eq!(sink.len(), 0);
    }

    #[tokio::test]
    async fn promotion_service_refuses_double_promotion_before_commit() {
        let (service, sink) = service_with(FakeSink::default());
        let already_promoted = reviewed_candidate()
            .promote("dave", vec![evidence("ev:promote:1")])
            .expect("promote");
        let err = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-x".into(),
                candidate: already_promoted,
                promoter: "eve".into(),
                evidence_refs: vec![evidence("ev:promote:2")],
            })
            .await
            .expect_err("double promotion must be refused");
        assert!(matches!(
            err,
            PromotionServiceError::Domain(PromotionError::AlreadyPromoted)
        ));
        assert_eq!(sink.len(), 0);
    }

    #[tokio::test]
    async fn promotion_service_refuses_illegal_transition_before_commit() {
        let (service, sink) = service_with(FakeSink::default());
        let err = service
            .commit(PromotionTransitionRequest::Review {
                decision_id: "dec-x".into(),
                candidate: proposed_candidate(), // review before verify
                reviewer: "carol".into(),
                evidence_refs: vec![evidence("ev:review:1")],
            })
            .await
            .expect_err("review from proposed must be refused");
        assert!(matches!(
            err,
            PromotionServiceError::Domain(PromotionError::IllegalTransition { .. })
        ));
        assert_eq!(sink.len(), 0);
    }

    #[tokio::test]
    async fn promotion_service_commit_failure_never_exposes_partial_success() {
        let (service, sink) = service_with(FakeSink::default());
        sink.fail_next_commit();
        let err = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-x".into(),
                candidate: reviewed_candidate(),
                promoter: "dave".into(),
                evidence_refs: vec![evidence("ev:promote:1")],
            })
            .await
            .expect_err("sink failure must surface as a typed commit error");
        assert!(matches!(
            err,
            PromotionServiceError::Commit(PromotionCommitError::Storage(_))
        ));
        // Fail-closed: the promoted decision must not be durably stored and
        // the caller never received a success carrying a promoted candidate.
        assert_eq!(sink.len(), 0);
        assert!(sink.stored("dec-x").is_none());
    }

    #[tokio::test]
    async fn promotion_service_reject_commits_reason_and_state() {
        let (service, sink) = service_with(FakeSink::default());
        let rejected = service
            .commit(PromotionTransitionRequest::Reject {
                decision_id: "dec-r".into(),
                candidate: verified_candidate(),
                reason: "duplicate of existing gold row".into(),
            })
            .await
            .expect("reject commits");
        assert_eq!(rejected.transition, PromotionState::Rejected);
        assert_eq!(
            rejected.candidate.rejection_reason.as_deref(),
            Some("duplicate of existing gold row")
        );
        assert!(rejected.candidate.state.is_terminal());
        assert_eq!(sink.len(), 1);
    }

    #[tokio::test]
    async fn promotion_service_requires_non_empty_decision_id() {
        let (service, sink) = service_with(FakeSink::default());
        let err = service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "   ".into(),
                kind: PromotionKind::Prompt,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:1")],
            })
            .await
            .expect_err("blank decision id must be refused");
        assert!(matches!(err, PromotionServiceError::EmptyDecisionId));
        assert_eq!(sink.len(), 0);
    }

    #[tokio::test]
    async fn promotion_service_commit_is_idempotent_replay() {
        let (service, sink) = service_with(FakeSink::default());
        let request = PromotionTransitionRequest::Propose {
            decision_id: "dec-1".into(),
            kind: PromotionKind::Prompt,
            candidate_id: "cand-1".into(),
            proposer: "alice".into(),
            evidence_refs: vec![evidence("ev:propose:1")],
        };
        let first = service.commit(request.clone()).await.expect("first");
        let replay = service.commit(request).await.expect("idempotent replay");
        assert_eq!(first, replay);
        assert_eq!(sink.len(), 1, "replay must not duplicate the record");
    }

    #[tokio::test]
    async fn promotion_service_sink_conflict_fails_closed() {
        let (service, sink) = service_with(FakeSink::default());
        service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "dec-1".into(),
                kind: PromotionKind::Prompt,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:1")],
            })
            .await
            .expect("first commit");

        // Same decision id, different content (different evidence) → conflict.
        let err = service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "dec-1".into(),
                kind: PromotionKind::Prompt,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:2")],
            })
            .await
            .expect_err("conflicting decision id must fail closed");
        assert!(matches!(
            err,
            PromotionServiceError::Commit(PromotionCommitError::Conflict { .. })
        ));
        assert_eq!(sink.len(), 1, "conflicting content must not overwrite");
    }

    #[tokio::test]
    async fn promotion_service_stamps_injected_clock_on_committed_at() {
        let (service, _) = service_with(FakeSink::default());
        let proposed = service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "dec-t".into(),
                kind: PromotionKind::Gold,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:1")],
            })
            .await
            .expect("propose");
        // FixedClock 1780000000 → 2026-06-01T12:26:40Z.
        assert_eq!(
            proposed.committed_at,
            chrono::DateTime::from_timestamp(1_780_000_000, 0)
                .unwrap()
                .to_rfc3339()
        );
    }

    #[tokio::test]
    async fn promotion_service_rejects_invalid_incoming_candidate_state() {
        let (service, sink) = service_with(FakeSink::default());
        // A Verified-shaped candidate carrying a reviewer is structurally
        // invalid; the service must refuse it before any commit.
        let mut corrupt = verified_candidate();
        corrupt.reviewer = Some("carol".into());
        let err = service
            .commit(PromotionTransitionRequest::Verify {
                decision_id: "dec-x".into(),
                candidate: corrupt,
                verifier: "bob".into(),
                evidence_refs: vec![evidence("ev:verify:1")],
            })
            .await
            .expect_err("invalid incoming candidate must be refused");
        assert!(matches!(
            err,
            PromotionServiceError::Domain(PromotionError::InvalidState(_))
        ));
        assert_eq!(sink.len(), 0);
    }

    #[tokio::test]
    async fn promotion_service_import_eligible_stays_false_through_commits() {
        // D127: every committed decision carries import_eligible=false, from
        // proposal through promotion, even when the caller passes a candidate
        // that claims import eligibility.
        let (service, sink) = service_with(FakeSink::default());

        let proposed = service
            .commit(PromotionTransitionRequest::Propose {
                decision_id: "dec-i1".into(),
                kind: PromotionKind::Gold,
                candidate_id: "cand-1".into(),
                proposer: "alice".into(),
                evidence_refs: vec![evidence("ev:propose:1")],
            })
            .await
            .expect("propose");
        assert!(!proposed.candidate.import_eligible);

        let mut leak_attempt = verified_candidate();
        leak_attempt.import_eligible = !leak_attempt.import_eligible;
        let verified = service
            .commit(PromotionTransitionRequest::Verify {
                decision_id: "dec-i2".into(),
                candidate: leak_attempt,
                verifier: "bob".into(),
                evidence_refs: vec![evidence("ev:verify:1")],
            })
            .await
            .expect_err("import leak in the incoming candidate must be refused");
        assert!(matches!(
            verified,
            PromotionServiceError::Domain(PromotionError::ImportEligibleLeak)
        ));
        assert_eq!(sink.len(), 1, "the leaked verify must not be committed");

        let verified = service
            .commit(PromotionTransitionRequest::Verify {
                decision_id: "dec-i2".into(),
                candidate: proposed.candidate,
                verifier: "bob".into(),
                evidence_refs: vec![evidence("ev:verify:1")],
            })
            .await
            .expect("verify");
        assert!(!verified.candidate.import_eligible);

        let reviewed = service
            .commit(PromotionTransitionRequest::Review {
                decision_id: "dec-i3".into(),
                candidate: verified.candidate,
                reviewer: "carol".into(),
                evidence_refs: vec![evidence("ev:review:1")],
            })
            .await
            .expect("review");
        assert!(!reviewed.candidate.import_eligible);

        let promoted = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-i4".into(),
                candidate: reviewed.candidate,
                promoter: "dave".into(),
                evidence_refs: vec![evidence("ev:promote:1")],
            })
            .await
            .expect("promote");
        assert!(!promoted.candidate.import_eligible);
        assert_eq!(promoted.candidate.state, PromotionState::Promoted);
        // The stored promoted decision is the authority: import stays false.
        assert!(!sink.stored("dec-i4").unwrap().candidate.import_eligible);
    }

    #[tokio::test]
    async fn promotion_service_exposes_kind_and_identities_on_every_attempt() {
        // Slice verification: every promotion attempt exposes candidate kind,
        // proposer, verifier, reviewer, promoter, evidence references,
        // transition result, and typed refusal reason.
        let (service, _) = service_with(FakeSink::default());
        let promoted = service
            .commit(PromotionTransitionRequest::Promote {
                decision_id: "dec-v".into(),
                candidate: reviewed_candidate(),
                promoter: "dave".into(),
                evidence_refs: vec![evidence("ev:promote:1")],
            })
            .await
            .expect("promote");
        assert_eq!(promoted.candidate.kind, PromotionKind::Gold);
        assert_eq!(promoted.candidate.proposer, "alice");
        assert_eq!(promoted.candidate.verifier.as_deref(), Some("bob"));
        assert_eq!(promoted.candidate.reviewer.as_deref(), Some("carol"));
        assert_eq!(promoted.candidate.promoter.as_deref(), Some("dave"));
        assert_eq!(promoted.candidate.evidence_refs.len(), 4);
        assert_eq!(promoted.transition, PromotionState::Promoted);
    }
}
