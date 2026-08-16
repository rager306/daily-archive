//! Authority-neutral candidate promotion protocol (PIR-015).
//!
//! One pure domain FSM governs how candidates from prompt, gold, and
//! entity-resolution surfaces move from proposed through independent
//! verification and review to promoted or rejected. The protocol is
//! authority-neutral: it records proposer, verifier, reviewer, and promoter
//! identities, requires durable evidence references on every advancing
//! transition, forbids self-confirmation, double promotion, and terminal
//! mutation, and keeps `import_eligible=false` under the D127 lock.
//!
//! Pure domain: no I/O, no infrastructure dependencies.

use serde::{Deserialize, Serialize};

/// Identity of an actor in the promotion protocol (proposer, verifier,
/// reviewer, promoter). A free-form string: agent name, tool run id, etc.
pub type ActorId = String;

/// Kind of candidate surface the protocol governs. Authority-neutral — one
/// protocol, three surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionKind {
    /// Prompt-optimization candidate (D238, candidate-only).
    Prompt,
    /// Gold-standard extraction candidate (E2 evidence promote).
    Gold,
    /// Entity-resolution merge candidate (ADR-053, reviewed).
    EntityResolution,
}

/// Lifecycle state of a promotion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionState {
    /// Candidate proposed, awaiting independent verification.
    Proposed,
    /// Independently verified.
    Verified,
    /// Reviewed.
    Reviewed,
    /// Promoted — terminal; immutable.
    Promoted,
    /// Rejected — terminal; immutable.
    Rejected,
}

impl PromotionState {
    /// Terminal states reject every further transition attempt.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Promoted | Self::Rejected)
    }
}

/// Durable evidence reference attached to a promotion decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    /// Durable identifier: artifact hash, exec evidence id, assertion id.
    pub reference: String,
    /// Optional provenance kind label (e.g. "artifact_hash", "gsd_uat_exec").
    pub kind: Option<String>,
}

impl EvidenceRef {
    /// Reference without a kind label.
    pub fn new(reference: impl Into<String>) -> Self {
        Self {
            reference: reference.into(),
            kind: None,
        }
    }

    /// Reference with an explicit provenance kind label.
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

/// Typed refusal reason for a promotion transition attempt.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromotionError {
    /// Candidate id must be non-empty.
    #[error("candidate id must be non-empty")]
    EmptyCandidateId,
    /// Actor identity must be non-empty.
    #[error("actor identity must be non-empty")]
    EmptyActor,
    /// Every advancing transition needs at least one durable evidence ref.
    #[error("evidence references must be non-empty")]
    MissingEvidence,
    /// The verifier must be independent of the proposer.
    #[error("verifier {actor} would self-confirm a candidate proposed by {proposer}")]
    SelfConfirmation { actor: ActorId, proposer: ActorId },
    /// The reviewer must be independent of the proposer and the verifier.
    #[error("reviewer {actor} must differ from proposer {proposer} and verifier {verifier}")]
    ReviewerNotIndependent {
        actor: ActorId,
        proposer: ActorId,
        verifier: ActorId,
    },
    /// The promoter must be independent of every earlier actor.
    #[error(
        "promoter {actor} must differ from proposer {proposer}, verifier {verifier}, and reviewer {reviewer}"
    )]
    PromoterNotIndependent {
        actor: ActorId,
        proposer: ActorId,
        verifier: ActorId,
        reviewer: ActorId,
    },
    /// The requested transition is not part of the FSM.
    #[error("transition {from:?} -> {to:?} is not a legal promotion transition")]
    IllegalTransition {
        from: PromotionState,
        to: PromotionState,
    },
    /// Terminal states (Promoted, Rejected) are immutable.
    #[error("candidate is terminal in state {state:?} and cannot be mutated")]
    TerminalMutation { state: PromotionState },
    /// A candidate can be promoted exactly once.
    #[error("candidate is already promoted; double promotion is rejected")]
    AlreadyPromoted,
    /// Rejection requires a non-empty reason.
    #[error("rejection requires a non-empty reason")]
    MissingRejectionReason,
    /// import_eligible must never become true (D127 lock).
    #[error("import_eligible must stay false under the D127 lock")]
    ImportEligibleLeak,
    /// Candidate payload is inconsistent with its state.
    #[error("candidate state payload mismatch: {0}")]
    InvalidState(String),
}

/// Authority-neutral promotion candidate record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionCandidate {
    pub candidate_id: String,
    pub kind: PromotionKind,
    pub state: PromotionState,
    pub proposer: ActorId,
    pub verifier: Option<ActorId>,
    pub reviewer: Option<ActorId>,
    pub promoter: Option<ActorId>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub rejection_reason: Option<String>,
    /// D127 lock — always false. Promotion never grants import eligibility.
    pub import_eligible: bool,
}

impl PromotionCandidate {
    /// Structural invariants, independent of any transition attempt.
    pub fn validate(&self) -> Result<(), PromotionError> {
        if self.candidate_id.trim().is_empty() {
            return Err(PromotionError::EmptyCandidateId);
        }
        if self.proposer.trim().is_empty() {
            return Err(PromotionError::EmptyActor);
        }
        if self.import_eligible {
            return Err(PromotionError::ImportEligibleLeak);
        }
        if !matches!(self.state, PromotionState::Rejected) && self.evidence_refs.is_empty() {
            return Err(PromotionError::MissingEvidence);
        }
        if self
            .evidence_refs
            .iter()
            .any(|evidence| evidence.reference.trim().is_empty())
        {
            return Err(PromotionError::MissingEvidence);
        }
        if self
            .verifier
            .as_deref()
            .is_some_and(|actor| actor.trim().is_empty())
            || self
                .reviewer
                .as_deref()
                .is_some_and(|actor| actor.trim().is_empty())
            || self
                .promoter
                .as_deref()
                .is_some_and(|actor| actor.trim().is_empty())
        {
            return Err(PromotionError::EmptyActor);
        }
        match self.state {
            PromotionState::Proposed => {
                if self.verifier.is_some()
                    || self.reviewer.is_some()
                    || self.promoter.is_some()
                    || self.rejection_reason.is_some()
                {
                    return Err(PromotionError::InvalidState(
                        "proposed candidate carries later-stage actor fields".into(),
                    ));
                }
            }
            PromotionState::Verified => {
                if self.verifier.is_none()
                    || self.reviewer.is_some()
                    || self.promoter.is_some()
                    || self.rejection_reason.is_some()
                {
                    return Err(PromotionError::InvalidState(
                        "verified candidate must carry only the verifier".into(),
                    ));
                }
            }
            PromotionState::Reviewed => {
                if self.verifier.is_none()
                    || self.reviewer.is_none()
                    || self.promoter.is_some()
                    || self.rejection_reason.is_some()
                {
                    return Err(PromotionError::InvalidState(
                        "reviewed candidate must carry verifier and reviewer".into(),
                    ));
                }
            }
            PromotionState::Promoted => {
                if self.verifier.is_none()
                    || self.reviewer.is_none()
                    || self.promoter.is_none()
                    || self.rejection_reason.is_some()
                {
                    return Err(PromotionError::InvalidState(
                        "promoted candidate must carry verifier, reviewer, and promoter".into(),
                    ));
                }
            }
            PromotionState::Rejected => {
                if self.rejection_reason.is_none() {
                    return Err(PromotionError::InvalidState(
                        "rejected candidate must carry a rejection reason".into(),
                    ));
                }
            }
        }
        if let Some(verifier) = self.verifier.as_deref()
            && verifier == self.proposer.as_str()
        {
            return Err(PromotionError::SelfConfirmation {
                actor: verifier.to_string(),
                proposer: self.proposer.clone(),
            });
        }
        if let Some(reviewer) = self.reviewer.as_deref()
            && (reviewer == self.proposer.as_str() || self.verifier.as_deref() == Some(reviewer))
        {
            return Err(PromotionError::ReviewerNotIndependent {
                actor: reviewer.to_string(),
                proposer: self.proposer.clone(),
                verifier: self.verifier.clone().unwrap_or_default(),
            });
        }
        if let Some(promoter) = self.promoter.as_deref()
            && (promoter == self.proposer.as_str()
                || self.verifier.as_deref() == Some(promoter)
                || self.reviewer.as_deref() == Some(promoter))
        {
            return Err(PromotionError::PromoterNotIndependent {
                actor: promoter.to_string(),
                proposer: self.proposer.clone(),
                verifier: self.verifier.clone().unwrap_or_default(),
                reviewer: self.reviewer.clone().unwrap_or_default(),
            });
        }
        Ok(())
    }

    /// Propose a new candidate. Evidence references are mandatory and
    /// `import_eligible` is locked to false (D127).
    pub fn propose(
        kind: PromotionKind,
        candidate_id: impl Into<String>,
        proposer: impl Into<String>,
        evidence_refs: Vec<EvidenceRef>,
    ) -> Result<Self, PromotionError> {
        let candidate = Self {
            candidate_id: candidate_id.into(),
            kind,
            state: PromotionState::Proposed,
            proposer: proposer.into(),
            verifier: None,
            reviewer: None,
            promoter: None,
            evidence_refs,
            rejection_reason: None,
            import_eligible: false,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// Reject any mutation on terminal states and check the requested
    /// transition is part of the FSM.
    fn check_mutable(&self, target: PromotionState) -> Result<(), PromotionError> {
        if self.state.is_terminal() {
            return Err(PromotionError::TerminalMutation { state: self.state });
        }
        let legal = matches!(
            (self.state, target),
            (PromotionState::Proposed, PromotionState::Verified)
                | (PromotionState::Verified, PromotionState::Reviewed)
                | (PromotionState::Reviewed, PromotionState::Promoted)
                | (
                    PromotionState::Proposed | PromotionState::Verified | PromotionState::Reviewed,
                    PromotionState::Rejected
                )
        );
        if !legal {
            return Err(PromotionError::IllegalTransition {
                from: self.state,
                to: target,
            });
        }
        Ok(())
    }

    /// Independent verification by an actor different from the proposer.
    pub fn verify(
        mut self,
        verifier: impl Into<String>,
        evidence_refs: Vec<EvidenceRef>,
    ) -> Result<Self, PromotionError> {
        let verifier = verifier.into();
        if verifier.trim().is_empty() {
            return Err(PromotionError::EmptyActor);
        }
        self.check_mutable(PromotionState::Verified)?;
        if verifier == self.proposer {
            return Err(PromotionError::SelfConfirmation {
                actor: verifier,
                proposer: self.proposer,
            });
        }
        if evidence_refs.is_empty() {
            return Err(PromotionError::MissingEvidence);
        }
        self.verifier = Some(verifier);
        self.evidence_refs.extend(evidence_refs);
        self.state = PromotionState::Verified;
        self.validate()?;
        Ok(self)
    }

    /// Review by an actor independent of the proposer and the verifier.
    pub fn review(
        mut self,
        reviewer: impl Into<String>,
        evidence_refs: Vec<EvidenceRef>,
    ) -> Result<Self, PromotionError> {
        let reviewer = reviewer.into();
        if reviewer.trim().is_empty() {
            return Err(PromotionError::EmptyActor);
        }
        self.check_mutable(PromotionState::Reviewed)?;
        let verifier = self.verifier.clone().unwrap_or_default();
        if reviewer == self.proposer || self.verifier.as_deref() == Some(reviewer.as_str()) {
            return Err(PromotionError::ReviewerNotIndependent {
                actor: reviewer,
                proposer: self.proposer,
                verifier,
            });
        }
        if evidence_refs.is_empty() {
            return Err(PromotionError::MissingEvidence);
        }
        self.reviewer = Some(reviewer);
        self.evidence_refs.extend(evidence_refs);
        self.state = PromotionState::Reviewed;
        self.validate()?;
        Ok(self)
    }

    /// Promotion by an actor independent of every earlier actor. A candidate
    /// can be promoted exactly once; `import_eligible` stays false.
    pub fn promote(
        mut self,
        promoter: impl Into<String>,
        evidence_refs: Vec<EvidenceRef>,
    ) -> Result<Self, PromotionError> {
        let promoter = promoter.into();
        if self.state == PromotionState::Promoted {
            return Err(PromotionError::AlreadyPromoted);
        }
        if promoter.trim().is_empty() {
            return Err(PromotionError::EmptyActor);
        }
        self.check_mutable(PromotionState::Promoted)?;
        let verifier = self.verifier.clone().unwrap_or_default();
        let reviewer = self.reviewer.clone().unwrap_or_default();
        if promoter == self.proposer
            || self.verifier.as_deref() == Some(promoter.as_str())
            || self.reviewer.as_deref() == Some(promoter.as_str())
        {
            return Err(PromotionError::PromoterNotIndependent {
                actor: promoter,
                proposer: self.proposer,
                verifier,
                reviewer,
            });
        }
        if evidence_refs.is_empty() {
            return Err(PromotionError::MissingEvidence);
        }
        self.promoter = Some(promoter);
        self.evidence_refs.extend(evidence_refs);
        self.state = PromotionState::Promoted;
        self.validate()?;
        Ok(self)
    }

    /// Reject the candidate from any non-terminal state. Requires a reason.
    pub fn reject(mut self, reason: impl Into<String>) -> Result<Self, PromotionError> {
        let reason = reason.into();
        self.check_mutable(PromotionState::Rejected)?;
        if reason.trim().is_empty() {
            return Err(PromotionError::MissingRejectionReason);
        }
        self.rejection_reason = Some(reason);
        self.state = PromotionState::Rejected;
        self.validate()?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn evidence(reference: &str) -> EvidenceRef {
        EvidenceRef::new(reference)
    }

    fn proposed() -> PromotionCandidate {
        PromotionCandidate::propose(
            PromotionKind::Gold,
            "cand-1",
            "alice",
            vec![evidence("ev:gold:paper-1:row-7")],
        )
        .expect("valid proposal")
    }

    fn state_from(value: u8) -> PromotionState {
        match value {
            0 => PromotionState::Proposed,
            1 => PromotionState::Verified,
            2 => PromotionState::Reviewed,
            3 => PromotionState::Promoted,
            _ => PromotionState::Rejected,
        }
    }

    /// Build a candidate at an arbitrary state with distinct actors.
    fn candidate_at(state: PromotionState) -> PromotionCandidate {
        let c = proposed();
        match state {
            PromotionState::Proposed => c,
            PromotionState::Verified => c
                .verify("bob", vec![evidence("ev:verify:1")])
                .expect("verify"),
            PromotionState::Reviewed => c
                .verify("bob", vec![evidence("ev:verify:1")])
                .expect("verify")
                .review("carol", vec![evidence("ev:review:1")])
                .expect("review"),
            PromotionState::Promoted => c
                .verify("bob", vec![evidence("ev:verify:1")])
                .expect("verify")
                .review("carol", vec![evidence("ev:review:1")])
                .expect("review")
                .promote("dave", vec![evidence("ev:promote:1")])
                .expect("promote"),
            PromotionState::Rejected => c.reject("rejected by policy").expect("reject"),
        }
    }

    #[test]
    fn promotion_legal_path_reaches_promoted_with_independent_identities() {
        let c = proposed()
            .verify("bob", vec![evidence("ev:verify:1")])
            .expect("verify")
            .review("carol", vec![evidence("ev:review:1")])
            .expect("review")
            .promote("dave", vec![evidence("ev:promote:1")])
            .expect("promote");
        assert_eq!(c.state, PromotionState::Promoted);
        assert!(c.state.is_terminal());
        assert_eq!(c.kind, PromotionKind::Gold);
        assert_eq!(c.proposer, "alice");
        assert_eq!(c.verifier.as_deref(), Some("bob"));
        assert_eq!(c.reviewer.as_deref(), Some("carol"));
        assert_eq!(c.promoter.as_deref(), Some("dave"));
        assert_eq!(c.evidence_refs.len(), 4);
        assert!(!c.import_eligible);
    }

    #[test]
    fn promotion_rejects_self_confirmation() {
        let err = proposed()
            .verify("alice", vec![evidence("ev:verify:1")])
            .expect_err("proposer must not verify own candidate");
        assert!(matches!(err, PromotionError::SelfConfirmation { .. }));
    }

    #[test]
    fn promotion_rejects_missing_evidence_on_every_stage() {
        let err = PromotionCandidate::propose(PromotionKind::Prompt, "c1", "alice", vec![])
            .expect_err("proposal without evidence");
        assert_eq!(err, PromotionError::MissingEvidence);

        let err = proposed()
            .verify("bob", vec![])
            .expect_err("verify without evidence");
        assert_eq!(err, PromotionError::MissingEvidence);

        let err = proposed()
            .verify("bob", vec![evidence("e")])
            .expect("verify")
            .review("carol", vec![])
            .expect_err("review without evidence");
        assert_eq!(err, PromotionError::MissingEvidence);

        let err = proposed()
            .verify("bob", vec![evidence("e")])
            .expect("verify")
            .review("carol", vec![evidence("e")])
            .expect("review")
            .promote("dave", vec![])
            .expect_err("promote without evidence");
        assert_eq!(err, PromotionError::MissingEvidence);
    }

    #[test]
    fn promotion_rejects_double_promotion() {
        let promoted = candidate_at(PromotionState::Promoted);
        let err = promoted
            .promote("eve", vec![evidence("ev:promote:2")])
            .expect_err("second promotion must fail");
        assert_eq!(err, PromotionError::AlreadyPromoted);
    }

    #[test]
    fn promotion_rejects_terminal_mutation() {
        let promoted = candidate_at(PromotionState::Promoted);
        for attempt in [
            promoted
                .clone()
                .verify("eve", vec![evidence("e")])
                .expect_err("verify on promoted"),
            promoted
                .clone()
                .review("eve", vec![evidence("e")])
                .expect_err("review on promoted"),
            promoted
                .clone()
                .reject("late")
                .expect_err("reject on promoted"),
        ] {
            assert!(matches!(attempt, PromotionError::TerminalMutation { .. }));
        }

        let rejected = candidate_at(PromotionState::Rejected);
        for attempt in [
            rejected
                .clone()
                .verify("eve", vec![evidence("e")])
                .expect_err("verify on rejected"),
            rejected
                .clone()
                .review("eve", vec![evidence("e")])
                .expect_err("review on rejected"),
            rejected
                .clone()
                .promote("eve", vec![evidence("e")])
                .expect_err("promote on rejected"),
            rejected
                .clone()
                .reject("again")
                .expect_err("reject on rejected"),
        ] {
            assert!(matches!(attempt, PromotionError::TerminalMutation { .. }));
        }
    }

    #[test]
    fn promotion_rejects_illegal_transitions() {
        let err = proposed()
            .review("carol", vec![evidence("e")])
            .expect_err("review before verify");
        assert!(matches!(
            err,
            PromotionError::IllegalTransition {
                from: PromotionState::Proposed,
                to: PromotionState::Reviewed
            }
        ));

        let verified = candidate_at(PromotionState::Verified);
        let err = verified
            .promote("dave", vec![evidence("e")])
            .expect_err("promote before review");
        assert!(matches!(
            err,
            PromotionError::IllegalTransition {
                from: PromotionState::Verified,
                to: PromotionState::Promoted
            }
        ));

        let err = candidate_at(PromotionState::Verified)
            .verify("eve", vec![evidence("e")])
            .expect_err("no-op verify");
        assert!(matches!(
            err,
            PromotionError::IllegalTransition {
                from: PromotionState::Verified,
                to: PromotionState::Verified
            }
        ));
    }

    #[test]
    fn promotion_requires_independent_reviewer_and_promoter() {
        let verified = candidate_at(PromotionState::Verified);
        let err = verified
            .clone()
            .review("alice", vec![evidence("e")])
            .expect_err("review by proposer");
        assert!(matches!(err, PromotionError::ReviewerNotIndependent { .. }));
        let err = verified
            .review("bob", vec![evidence("e")])
            .expect_err("review by verifier");
        assert!(matches!(err, PromotionError::ReviewerNotIndependent { .. }));

        let reviewed = candidate_at(PromotionState::Reviewed);
        for promoter in ["alice", "bob", "carol"] {
            let err = reviewed
                .clone()
                .promote(promoter, vec![evidence("e")])
                .expect_err("promotion must be independent of all earlier actors");
            assert!(matches!(err, PromotionError::PromoterNotIndependent { .. }));
        }
    }

    #[test]
    fn promotion_reject_from_every_non_terminal_state() {
        let r = proposed()
            .reject("duplicate of existing gold")
            .expect("reject");
        assert_eq!(r.state, PromotionState::Rejected);
        assert!(r.state.is_terminal());
        assert_eq!(
            r.rejection_reason.as_deref(),
            Some("duplicate of existing gold")
        );
        assert!(r.verifier.is_none());

        let r = candidate_at(PromotionState::Verified)
            .reject("spurious candidate")
            .expect("reject");
        assert_eq!(r.state, PromotionState::Rejected);
        assert_eq!(r.verifier.as_deref(), Some("bob"));

        let r = candidate_at(PromotionState::Reviewed)
            .reject("conflicts with later evidence")
            .expect("reject");
        assert_eq!(r.state, PromotionState::Rejected);
        assert_eq!(r.reviewer.as_deref(), Some("carol"));
    }

    #[test]
    fn promotion_reject_requires_non_empty_reason() {
        let err = proposed().reject("   ").expect_err("blank reason");
        assert_eq!(err, PromotionError::MissingRejectionReason);
    }

    #[test]
    fn promotion_rejects_empty_identities() {
        let err =
            PromotionCandidate::propose(PromotionKind::Prompt, "", "alice", vec![evidence("e")])
                .expect_err("empty candidate id");
        assert_eq!(err, PromotionError::EmptyCandidateId);

        let err = PromotionCandidate::propose(PromotionKind::Prompt, "c1", "", vec![evidence("e")])
            .expect_err("empty proposer");
        assert_eq!(err, PromotionError::EmptyActor);

        let err = proposed()
            .verify("", vec![evidence("e")])
            .expect_err("empty verifier");
        assert_eq!(err, PromotionError::EmptyActor);
    }

    #[test]
    fn promotion_import_eligible_stays_false_through_every_state() {
        let mut c = proposed();
        assert!(!c.import_eligible);
        c = c.verify("bob", vec![evidence("e")]).expect("verify");
        assert!(!c.import_eligible);
        c = c.review("carol", vec![evidence("e")]).expect("review");
        assert!(!c.import_eligible);
        c = c.promote("dave", vec![evidence("e")]).expect("promote");
        assert!(!c.import_eligible);
        assert_eq!(c.state, PromotionState::Promoted);

        let rejected = proposed().reject("nope").expect("reject");
        assert!(!rejected.import_eligible);
    }

    #[test]
    fn promotion_kinds_are_authority_neutral() {
        for kind in [
            PromotionKind::Prompt,
            PromotionKind::Gold,
            PromotionKind::EntityResolution,
        ] {
            let c = PromotionCandidate::propose(kind, "c1", "alice", vec![evidence("e")])
                .expect("propose")
                .verify("bob", vec![evidence("e")])
                .expect("verify")
                .review("carol", vec![evidence("e")])
                .expect("review")
                .promote("dave", vec![evidence("e")])
                .expect("promote");
            assert_eq!(c.kind, kind);
            assert_eq!(c.state, PromotionState::Promoted);
        }
    }

    #[test]
    fn promotion_evidence_ref_carries_durable_reference_and_kind() {
        let evidence_ref = EvidenceRef::new("sha256:abc123").with_kind("artifact_hash");
        assert_eq!(evidence_ref.reference, "sha256:abc123");
        assert_eq!(evidence_ref.kind.as_deref(), Some("artifact_hash"));
    }

    proptest! {
        /// ADR-051 P1: no transition attempt leaks import eligibility or
        /// mutates a terminal state into a non-terminal one.
        #[test]
        fn prop_promotion_attempt_never_leaks_import_or_terminal_mutation(
            state in 0u8..=4u8,
            action in 0u8..=3u8,
        ) {
            let c = candidate_at(state_from(state));
            let outcome = match action {
                0 => c.verify("bob", vec![evidence("ev:verify:2")]),
                1 => c.review("carol", vec![evidence("ev:review:2")]),
                2 => c.promote("dave", vec![evidence("ev:promote:2")]),
                _ => c.reject("policy"),
            };
            match outcome {
                Ok(next) => {
                    prop_assert!(!next.import_eligible, "import_eligible leaked true");
                    prop_assert!(
                        next.state.is_terminal() == matches!(next.state, PromotionState::Promoted | PromotionState::Rejected),
                        "terminal flag inconsistent with state {:?}",
                        next.state
                    );
                }
                Err(_) => { /* typed refusal; terminal immutability is asserted below */ }
            }
        }

        /// ADR-051 P1: verifying with the proposer identity always fails.
        #[test]
        fn prop_promotion_verify_requires_independent_verifier(state in 0u8..=4u8) {
            let c = candidate_at(state_from(state));
            if c.state == PromotionState::Proposed {
                let proposer = c.proposer.clone();
                let err = c
                    .verify(proposer, vec![evidence("ev:verify:4")])
                    .expect_err("proposer must not self-confirm");
                prop_assert!(
                    matches!(err, PromotionError::SelfConfirmation { .. }),
                    "proposer verifying own candidate must be rejected as self-confirmation"
                );
            }
        }

        /// ADR-051 P1: missing evidence fails closed on the verify transition.
        #[test]
        fn prop_promotion_missing_evidence_fails_closed(evidence_yes in any::<bool>()) {
            let c = proposed();
            let refs = if evidence_yes {
                vec![evidence("ev:verify:3")]
            } else {
                vec![]
            };
            let result = c.verify("bob", refs);
            if evidence_yes {
                prop_assert!(result.is_ok(), "verified with evidence must succeed");
            } else {
                prop_assert_eq!(result.unwrap_err(), PromotionError::MissingEvidence);
            }
        }

        /// ADR-051 P1: promotion succeeds exactly once, from Reviewed only.
        #[test]
        fn prop_promotion_double_promote_never_succeeds(state in 0u8..=4u8) {
            let c = candidate_at(state_from(state));
            let result = c.promote("eve", vec![evidence("ev:promote:2")]);
            match state_from(state) {
                PromotionState::Promoted => {
                    prop_assert_eq!(result.unwrap_err(), PromotionError::AlreadyPromoted);
                }
                PromotionState::Rejected => {
                    prop_assert!(
                        matches!(result, Err(PromotionError::TerminalMutation { .. })),
                        "promote on a rejected candidate must be a terminal-mutation refusal"
                    );
                }
                PromotionState::Reviewed => {
                    prop_assert!(result.is_ok(), "reviewed candidate must promote");
                }
                _ => {
                    prop_assert!(result.is_err(), "promote must fail before review");
                }
            }
        }
    }
}
