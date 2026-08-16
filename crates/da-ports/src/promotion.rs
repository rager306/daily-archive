//! PromotionCommitSink port — the fail-closed promotion commit boundary
//! (PIR-015).
//!
//! The application promotion service (`da-application::promotion`) validates a
//! domain transition and publishes the resulting candidate state plus its
//! evidence references through exactly one [`PromotionCommitSink::commit`]
//! call. The sink is the only place a promotion decision becomes durable and
//! externally observable. A failed commit must not leave any partial state
//! exposed as promoted: either the full [`PromotionDecision`] is stored
//! atomically or a typed error is returned.
//!
//! Contract (implementations must enforce):
//! - `commit` validates the decision via [`PromotionDecision::validate`]
//!   before storing — a decision that fails integrity is refused;
//! - committing the byte-identical decision again is an idempotent replay and
//!   returns `Ok(())` without duplicating the record;
//! - committing a different decision under the same `decision_id` fails closed
//!   with [`PromotionCommitError::Conflict`] — records are never overwritten;
//! - storage-backend failures surface as [`PromotionCommitError::Storage`].

use async_trait::async_trait;
use da_domain::promotion::{PromotionCandidate, PromotionState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Durable promotion decision record — the atomic unit of the commit.
///
/// Carries the full candidate (kind, state, proposer/verifier/reviewer/promoter
/// identities, evidence references, D127 `import_eligible=false`) plus commit
/// metadata stamped from the injected Clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDecision {
    /// Stable id of this decision record (idempotent replay key).
    pub decision_id: String,
    /// The transition result state committed by this decision.
    pub transition: PromotionState,
    /// RFC3339 commit timestamp, stamped by the injected Clock.
    pub committed_at: String,
    /// Full candidate state after the committed transition.
    pub candidate: PromotionCandidate,
}

impl PromotionDecision {
    /// Fail-closed integrity validation, run by every sink implementation
    /// before storing. A decision that fails this must never become durable.
    pub fn validate(&self) -> Result<(), PromotionCommitError> {
        let candidate_id = &self.candidate.candidate_id;
        if self.decision_id.trim().is_empty() {
            return Err(PromotionCommitError::Integrity {
                candidate_id: candidate_id.clone(),
            });
        }
        if self.transition != self.candidate.state {
            return Err(PromotionCommitError::Integrity {
                candidate_id: candidate_id.clone(),
            });
        }
        if chrono::DateTime::parse_from_rfc3339(&self.committed_at).is_err() {
            return Err(PromotionCommitError::Integrity {
                candidate_id: candidate_id.clone(),
            });
        }
        self.candidate
            .validate()
            .map_err(|_| PromotionCommitError::Integrity {
                candidate_id: candidate_id.clone(),
            })
    }
}

/// Typed failure of the promotion commit boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PromotionCommitError {
    /// A different decision already exists under the same decision id.
    #[error(
        "promotion decision id {decision_id} already exists with conflicting content for candidate {candidate_id}"
    )]
    Conflict {
        decision_id: String,
        candidate_id: String,
    },
    /// The decision failed integrity validation and must not be stored.
    #[error("promotion decision failed integrity validation for candidate {candidate_id}")]
    Integrity { candidate_id: String },
    /// Storage backend failure.
    #[error("promotion commit storage failure: {0}")]
    Storage(String),
}

/// Fail-closed durable commit boundary for promotion decisions.
///
/// Implementations must commit the decision atomically: either the full
/// decision (candidate state plus evidence references) becomes durable, or an
/// error is returned and nothing is exposed as promoted.
#[async_trait]
pub trait PromotionCommitSink: Send + Sync {
    /// Persist one promotion decision. Idempotent for byte-identical replays;
    /// conflicting content under the same `decision_id` fails closed.
    async fn commit(&self, decision: &PromotionDecision) -> Result<(), PromotionCommitError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::promotion::{EvidenceRef, PromotionCandidate, PromotionKind};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// In-memory contract fake for the sink. Records every accepted decision
    /// under its decision id and refuses conflicting overwrites.
    #[derive(Default)]
    struct FakeSink {
        decisions: Mutex<BTreeMap<String, PromotionDecision>>,
    }

    impl FakeSink {
        fn stored(&self, decision_id: &str) -> Option<PromotionDecision> {
            self.decisions.lock().unwrap().get(decision_id).cloned()
        }

        fn len(&self) -> usize {
            self.decisions.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl PromotionCommitSink for FakeSink {
        async fn commit(&self, decision: &PromotionDecision) -> Result<(), PromotionCommitError> {
            decision.validate()?;
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

    fn promoted_candidate() -> PromotionCandidate {
        PromotionCandidate::propose(
            PromotionKind::Gold,
            "cand-1",
            "alice",
            vec![evidence("ev:propose:1")],
        )
        .expect("propose")
        .verify("bob", vec![evidence("ev:verify:1")])
        .expect("verify")
        .review("carol", vec![evidence("ev:review:1")])
        .expect("review")
        .promote("dave", vec![evidence("ev:promote:1")])
        .expect("promote")
    }

    fn decision(
        decision_id: &str,
        transition: PromotionState,
        candidate: PromotionCandidate,
    ) -> PromotionDecision {
        PromotionDecision {
            decision_id: decision_id.into(),
            transition,
            committed_at: "2026-08-16T00:00:00Z".into(),
            candidate,
        }
    }

    #[tokio::test]
    async fn promotion_commit_sink_stores_full_decision() {
        let sink = FakeSink::default();
        let d = decision("dec-1", PromotionState::Promoted, promoted_candidate());
        sink.commit(&d).await.expect("commit");
        assert_eq!(sink.stored("dec-1"), Some(d));
        assert_eq!(sink.len(), 1);
    }

    #[tokio::test]
    async fn promotion_commit_sink_idempotent_replay_is_ok() {
        let sink = FakeSink::default();
        let d = decision("dec-1", PromotionState::Promoted, promoted_candidate());
        sink.commit(&d).await.expect("first commit");
        sink.commit(&d).await.expect("idempotent replay");
        assert_eq!(sink.len(), 1);
    }

    #[tokio::test]
    async fn promotion_commit_sink_rejects_conflicting_decision_id() {
        let sink = FakeSink::default();
        let first = decision("dec-1", PromotionState::Promoted, promoted_candidate());
        sink.commit(&first).await.expect("first commit");

        let mut candidate = promoted_candidate();
        candidate.evidence_refs.push(evidence("ev:extra:1"));
        let second = decision("dec-1", PromotionState::Promoted, candidate);
        let err = sink.commit(&second).await.expect_err("conflict");
        assert_eq!(
            err,
            PromotionCommitError::Conflict {
                decision_id: "dec-1".into(),
                candidate_id: "cand-1".into(),
            }
        );
        // The conflicting content must not have overwritten the first record.
        assert_eq!(sink.stored("dec-1"), Some(first));
    }

    #[tokio::test]
    async fn promotion_commit_sink_refuses_integrity_violation() {
        let sink = FakeSink::default();
        // transition result does not match the candidate state.
        let d = decision("dec-bad", PromotionState::Verified, promoted_candidate());
        let err = sink.commit(&d).await.expect_err("integrity");
        assert_eq!(
            err,
            PromotionCommitError::Integrity {
                candidate_id: "cand-1".into()
            }
        );
        assert_eq!(sink.len(), 0);

        // import_eligible leak is refused by the domain validator inside
        // PromotionDecision::validate.
        let mut candidate = promoted_candidate();
        candidate.import_eligible = !candidate.import_eligible;
        let d = decision("dec-leak", PromotionState::Promoted, candidate);
        let err = sink.commit(&d).await.expect_err("import leak");
        assert_eq!(
            err,
            PromotionCommitError::Integrity {
                candidate_id: "cand-1".into()
            }
        );
        assert_eq!(sink.len(), 0);
    }

    #[test]
    fn promotion_decision_serde_roundtrip() {
        crate::roundtrip_json(&decision(
            "dec-1",
            PromotionState::Promoted,
            promoted_candidate(),
        ));
    }
}
