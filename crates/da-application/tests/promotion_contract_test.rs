//! PIR-015 promotion contract test — every candidate surface (prompt, gold,
//! entity-resolution) produces the same `PromotionCandidate`/evidence shape
//! and advances through the shared fail-closed `PromotionService` without
//! enabling optimizer auto-promotion or import (D127 lock).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use da_application::entity_resolution::ReviewedMergeResult;
use da_application::promotion::{
    PromotionService, PromotionServiceError, PromotionTransitionRequest,
};
use da_domain::assurance::ContentDigest;
use da_domain::eval::{GoldEntity, MultiPaperGoldIndex, PredictedEntity, promote_candidates};
use da_domain::healing::{HealingActor, HealingOperation, MergeResult, ProvenanceEvent};
use da_domain::promotion::{
    EvidenceRef, PromotionCandidate, PromotionError, PromotionKind, PromotionState,
};
use da_domain::prompt_optimization::{PromptCandidate, PromptCandidateState, PromptEvaluation};
use da_ports::FixedClock;
use da_ports::promotion::{PromotionCommitError, PromotionCommitSink, PromotionDecision};

/// In-memory fail-closed sink mirroring the port contract: validate before
/// store, idempotent replay of byte-identical decisions, Conflict on the same
/// decision id with different content, and an optional forced storage failure.
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

fn service_with(sink: FakeSink) -> (PromotionService, Arc<FakeSink>) {
    let sink = Arc::new(sink);
    let service = PromotionService::from_shared_with_clock(
        sink.clone(),
        Arc::new(FixedClock::from_timestamp(1_780_000_000)),
    );
    (service, sink)
}

fn evidence(reference: &str) -> EvidenceRef {
    EvidenceRef::new(reference)
}

/// Advance a candidate from its current state to Promoted through the
/// fail-closed service, using independent actors and non-empty evidence per
/// transition. Returns the final (Promoted) durable decision.
async fn advance_through_service(
    service: &PromotionService,
    candidate: PromotionCandidate,
    prefix: &str,
) -> PromotionDecision {
    let mut current = candidate;
    let mut step = 0u32;
    loop {
        step += 1;
        let decision_id = format!("{prefix}-{step}");
        let request = match current.state {
            PromotionState::Proposed => PromotionTransitionRequest::Verify {
                decision_id,
                candidate: current,
                verifier: format!("contract:verifier:{prefix}"),
                evidence_refs: vec![evidence(&format!("ev:{prefix}:verify:{step}"))],
            },
            PromotionState::Verified => PromotionTransitionRequest::Review {
                decision_id,
                candidate: current,
                reviewer: format!("contract:reviewer:{prefix}"),
                evidence_refs: vec![evidence(&format!("ev:{prefix}:review:{step}"))],
            },
            PromotionState::Reviewed => PromotionTransitionRequest::Promote {
                decision_id,
                candidate: current,
                promoter: format!("contract:promoter:{prefix}"),
                evidence_refs: vec![evidence(&format!("ev:{prefix}:promote:{step}"))],
            },
            state => {
                panic!("advance_through_service expects a non-terminal candidate, got {state:?}")
            }
        };
        let decision = service.commit(request).await.expect("commit must succeed");
        current = decision.candidate.clone();
        if decision.transition == PromotionState::Promoted {
            return decision;
        }
    }
}

// ── prompt surface fixtures ──

fn prompt_base() -> PromptCandidate {
    PromptCandidate {
        candidate_id: "prompt-cand-1".into(),
        task_id: "summary".into(),
        state: PromptCandidateState::BaselineRegistered,
        baseline_spec_sha256: ContentDigest::of_text("baseline"),
        candidate_spec_sha256: None,
        evaluation: None,
        human_decision_ref: None,
        deployed: false,
        import_eligible: false,
    }
}

fn prompt_evaluation() -> PromptEvaluation {
    PromptEvaluation {
        baseline_n: 10,
        candidate_n: 10,
        train_fixture_sha256: ContentDigest::of_text("train"),
        validation_fixture_sha256: ContentDigest::of_text("val"),
        metric_basis_points: BTreeMap::from([("faithfulness".into(), 9000)]),
        validation_gap_basis_points: 200,
        hard_invariants_pass: true,
        contamination_detected: false,
        latency_budget_pass: true,
        cost_budget_pass: true,
    }
}

// ── prompt surface ──

#[tokio::test]
async fn prompt_candidate_maps_through_shared_protocol_to_promoted() {
    let (service, sink) = service_with(FakeSink::default());

    // CandidateProposed → Proposed.
    let proposed_c = prompt_base()
        .propose(ContentDigest::of_text("candidate"))
        .expect("propose");
    let mapped = proposed_c.to_promotion_candidate().expect("adapter");
    assert_eq!(mapped.state, PromotionState::Proposed);
    assert_eq!(mapped.kind, PromotionKind::Prompt);
    assert_eq!(mapped.proposer, "prompt:registry:summary");
    assert_eq!(mapped.verifier, None);
    assert_eq!(mapped.evidence_refs.len(), 1);
    assert!(!mapped.import_eligible, "D127 lock");

    // Evaluated → Verified by the independent evaluation suite.
    let evaluated_c = proposed_c
        .clone()
        .evaluate(prompt_evaluation(), 500)
        .expect("evaluate");
    let mapped = evaluated_c.to_promotion_candidate().expect("adapter");
    assert_eq!(mapped.state, PromotionState::Verified);
    assert_eq!(
        mapped.verifier.as_deref(),
        Some("prompt:evaluation:summary")
    );
    assert!(
        mapped.evidence_refs.len() >= 4,
        "fixtures + same-n + metric"
    );

    // Eligible → Reviewed (never Promoted — optimizer auto-promotion stays off).
    let eligible_c = evaluated_c.clone().mark_eligible().expect("eligible");
    let mapped = eligible_c.to_promotion_candidate().expect("adapter");
    assert_eq!(mapped.state, PromotionState::Reviewed);
    assert_eq!(
        mapped.reviewer.as_deref(),
        Some("prompt:eligibility:summary")
    );
    assert_ne!(mapped.state, PromotionState::Promoted);
    assert!(!mapped.import_eligible);

    // Continue the Reviewed candidate to Promoted through the fail-closed
    // service: kind/identities/evidence are the shared-protocol shape.
    let decision = advance_through_service(&service, mapped, "prompt").await;
    assert_eq!(decision.transition, PromotionState::Promoted);
    assert_eq!(decision.candidate.kind, PromotionKind::Prompt);
    assert_eq!(decision.candidate.proposer, "prompt:registry:summary");
    assert_eq!(
        decision.candidate.verifier.as_deref(),
        Some("prompt:evaluation:summary")
    );
    assert_eq!(
        decision.candidate.reviewer.as_deref(),
        Some("prompt:eligibility:summary")
    );
    assert_eq!(
        decision.candidate.promoter.as_deref(),
        Some("contract:promoter:prompt")
    );
    assert!(!decision.candidate.import_eligible);
    assert!(
        decision
            .candidate
            .evidence_refs
            .iter()
            .any(|e| e.kind.as_deref() == Some("artifact_hash"))
    );
    assert!(
        sink.stored("prompt-1").is_some(),
        "promote decision durable"
    );
}

// ── gold surface ──

#[tokio::test]
async fn gold_multi_paper_promote_maps_through_shared_protocol() {
    let (service, sink) = service_with(FakeSink::default());

    // RLHF is gold in papers B and C but not in paper A (the paper being
    // processed), so the constrained pipeline promotes it for A.
    let paper_a = vec![
        GoldEntity {
            label: "GRPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "LoRA".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let paper_b = vec![
        GoldEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "DPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let paper_c = vec![
        GoldEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "PPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let index = MultiPaperGoldIndex::from_papers(vec![
        ("2507.19457", paper_a.as_slice()),
        ("2510.26094", paper_b.as_slice()),
        ("2601.00001", paper_c.as_slice()),
    ]);
    let predicted = vec![
        PredictedEntity {
            label: "GRPO".into(),
            entity_type: "Method".into(),
        },
        PredictedEntity {
            label: "LoRA".into(),
            entity_type: "Method".into(),
        },
        PredictedEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
        },
    ];
    let promoted = promote_candidates(&paper_a, &predicted, &index, 2);
    assert_eq!(promoted.len(), 1, "only RLHF is multi-paper and not local");
    assert_eq!(promoted[0].label, "RLHF");

    // Constrained gold result → shared-protocol proposal with durable
    // multi-paper evidence.
    let proposal = promoted[0]
        .to_promotion_proposal(&["2510.26094".into(), "2601.00001".into()], 2)
        .expect("adapter");
    assert_eq!(proposal.state, PromotionState::Proposed);
    assert_eq!(proposal.kind, PromotionKind::Gold);
    assert_eq!(proposal.proposer, "gold:multi-paper-promote");
    assert!(
        proposal
            .evidence_refs
            .iter()
            .any(|e| e.reference == "gold:2510.26094:RLHF:Method")
    );
    assert!(
        proposal
            .evidence_refs
            .iter()
            .any(|e| e.reference == "gold:constraint:min_papers=2")
    );
    assert!(!proposal.import_eligible, "D127 lock");

    // Full journey through the fail-closed service.
    let decision = advance_through_service(&service, proposal, "gold").await;
    assert_eq!(decision.transition, PromotionState::Promoted);
    assert_eq!(decision.candidate.kind, PromotionKind::Gold);
    assert_eq!(decision.candidate.proposer, "gold:multi-paper-promote");
    assert!(!decision.candidate.import_eligible);
    assert!(sink.stored("gold-3").is_some(), "promote decision durable");

    // Negative: no durable paper evidence → no promotion proposal; below the
    // multi-paper constraint → refused.
    let err = promoted[0]
        .to_promotion_proposal(&[], 2)
        .expect_err("empty papers");
    assert_eq!(err, PromotionError::MissingEvidence);
    let err = promoted[0]
        .to_promotion_proposal(&["2510.26094".into()], 2)
        .expect_err("below constraint");
    assert_eq!(err, PromotionError::MissingEvidence);
}

// ── entity-resolution surface ──

#[tokio::test]
async fn reviewed_entity_resolution_merge_maps_through_shared_protocol() {
    let (service, sink) = service_with(FakeSink::default());

    let merge = MergeResult {
        kept_vid: "vid:entity:e1".into(),
        merged_vid: "vid:entity:e2".into(),
        edges_redirected: 3,
        provenance: ProvenanceEvent::new(
            HealingOperation::Merge,
            HealingActor::Human("alice".into()),
            vec!["vid:entity:e1".into(), "vid:entity:e2".into()],
            "exact alias merge".into(),
        ),
    };
    let result = ReviewedMergeResult {
        decision_vid: "vid:decision:dr-42".into(),
        decision_node_id: 42,
        merge,
    };

    let mapped = result.to_promotion_candidate().expect("adapter");
    assert_eq!(mapped.state, PromotionState::Reviewed);
    assert_eq!(mapped.kind, PromotionKind::EntityResolution);
    assert_eq!(mapped.proposer, "human:alice");
    assert_eq!(
        mapped.verifier.as_deref(),
        Some("entity-resolution:blocking-check")
    );
    assert_eq!(
        mapped.reviewer.as_deref(),
        Some("entity-resolution:decision-review")
    );
    assert!(
        mapped
            .evidence_refs
            .iter()
            .any(|e| e.reference == "decision:vid:decision:dr-42")
    );
    assert!(
        mapped
            .evidence_refs
            .iter()
            .any(|e| e.kind.as_deref() == Some("blocking_profile"))
    );
    assert!(!mapped.import_eligible, "D127 lock");

    // Continue to Promoted through the fail-closed service.
    let decision = advance_through_service(&service, mapped, "er").await;
    assert_eq!(decision.transition, PromotionState::Promoted);
    assert_eq!(decision.candidate.kind, PromotionKind::EntityResolution);
    assert_eq!(
        decision.candidate.promoter.as_deref(),
        Some("contract:promoter:er")
    );
    assert!(!decision.candidate.import_eligible);
    let stored = sink.stored("er-1").expect("promote decision durable");
    assert_eq!(stored.transition, PromotionState::Promoted);
    assert!(!stored.candidate.import_eligible);
}

// ── no optimizer auto-promotion / no import ──

#[tokio::test]
async fn no_surface_enables_optimizer_auto_promotion_or_import() {
    // Prompt Eligible maps to Reviewed — never Promoted — and stays undeployed.
    let eligible = prompt_base()
        .propose(ContentDigest::of_text("candidate"))
        .unwrap()
        .evaluate(prompt_evaluation(), 500)
        .unwrap()
        .mark_eligible()
        .unwrap();
    let mapped = eligible.to_promotion_candidate().unwrap();
    assert_ne!(mapped.state, PromotionState::Promoted);
    assert_eq!(mapped.state, PromotionState::Reviewed);
    assert!(!eligible.deployed, "prompt candidate stays undeployed");
    assert!(!eligible.import_eligible);
    assert!(!mapped.import_eligible);

    // D127 import leak in the prompt candidate is refused by the adapter
    // before any promotion candidate exists.
    let mut leaked = eligible.clone();
    leaked.import_eligible = true;
    let err = leaked
        .to_promotion_candidate()
        .expect_err("import leak refused");
    assert!(matches!(err, PromotionError::InvalidState(_)));
}

#[test]
fn prompt_baseline_and_rejected_have_no_promotion_path() {
    // BaselineRegistered: nothing was proposed → no promotion candidate.
    let err = prompt_base()
        .to_promotion_candidate()
        .expect_err("baseline refused");
    assert!(matches!(err, PromotionError::InvalidState(_)));

    // Rejected prompt candidate: terminal in its own FSM → no promotion path.
    let rejected = prompt_base()
        .propose(ContentDigest::of_text("candidate"))
        .unwrap()
        .evaluate(prompt_evaluation(), 500)
        .unwrap()
        .mark_eligible()
        .unwrap();
    let mut rejected = rejected;
    rejected.state = PromptCandidateState::Rejected;
    rejected.validate().expect("rejected payload valid");
    let err = rejected
        .to_promotion_candidate()
        .expect_err("rejected refused");
    assert!(matches!(err, PromotionError::InvalidState(_)));
}

// ── fail-closed negative surface ──

#[tokio::test]
async fn service_refuses_self_confirmation_double_promotion_and_missing_evidence() {
    let (service, sink) = service_with(FakeSink::default());

    // Self-confirmation: the proposer must not verify its own candidate.
    let proposal = PromotionCandidate::propose(
        PromotionKind::Gold,
        "gold:RLHF:Method",
        "gold:multi-paper-promote",
        vec![evidence("ev:gold:propose")],
    )
    .expect("proposal");
    let err = service
        .commit(PromotionTransitionRequest::Verify {
            decision_id: "neg-1".into(),
            candidate: proposal.clone(),
            verifier: "gold:multi-paper-promote".into(),
            evidence_refs: vec![evidence("ev:verify")],
        })
        .await
        .expect_err("self-confirmation refused");
    assert!(matches!(
        err,
        PromotionServiceError::Domain(PromotionError::SelfConfirmation { .. })
    ));
    assert_eq!(sink.len(), 0, "no decision committed on refusal");

    // Full legal path, then double promotion is refused.
    let verified = service
        .commit(PromotionTransitionRequest::Verify {
            decision_id: "neg-2".into(),
            candidate: proposal,
            verifier: "contract:verifier:neg".into(),
            evidence_refs: vec![evidence("ev:verify")],
        })
        .await
        .expect("verify");
    let reviewed = service
        .commit(PromotionTransitionRequest::Review {
            decision_id: "neg-3".into(),
            candidate: verified.candidate,
            reviewer: "contract:reviewer:neg".into(),
            evidence_refs: vec![evidence("ev:review")],
        })
        .await
        .expect("review");
    let promoted = service
        .commit(PromotionTransitionRequest::Promote {
            decision_id: "neg-4".into(),
            candidate: reviewed.candidate,
            promoter: "contract:promoter:neg".into(),
            evidence_refs: vec![evidence("ev:promote")],
        })
        .await
        .expect("promote");
    let err = service
        .commit(PromotionTransitionRequest::Promote {
            decision_id: "neg-5".into(),
            candidate: promoted.candidate,
            promoter: "contract:promoter2:neg".into(),
            evidence_refs: vec![evidence("ev:promote:2")],
        })
        .await
        .expect_err("double promotion refused");
    assert!(matches!(
        err,
        PromotionServiceError::Domain(PromotionError::AlreadyPromoted)
    ));
    assert_eq!(sink.len(), 3, "the refused double-promote stored nothing");

    // Missing evidence on promote fails closed, nothing is committed.
    let (service2, sink2) = service_with(FakeSink::default());
    let reviewed_candidate = PromotionCandidate::propose(
        PromotionKind::EntityResolution,
        "er-merge:e1:e2",
        "human:alice",
        vec![evidence("ev:merge")],
    )
    .unwrap()
    .verify(
        "entity-resolution:blocking-check",
        vec![evidence("ev:blocking")],
    )
    .unwrap()
    .review(
        "entity-resolution:decision-review",
        vec![evidence("ev:decision")],
    )
    .unwrap();
    let err = service2
        .commit(PromotionTransitionRequest::Promote {
            decision_id: "neg-6".into(),
            candidate: reviewed_candidate,
            promoter: "contract:promoter:neg".into(),
            evidence_refs: vec![],
        })
        .await
        .expect_err("missing evidence refused");
    assert!(matches!(
        err,
        PromotionServiceError::Domain(PromotionError::MissingEvidence)
    ));
    assert_eq!(sink2.len(), 0);
}

#[tokio::test]
async fn commit_failure_never_exposes_promoted_partial_success() {
    let (service, sink) = service_with(FakeSink::default());
    let reviewed_candidate = PromotionCandidate::propose(
        PromotionKind::EntityResolution,
        "er-merge:e1:e2",
        "human:alice",
        vec![evidence("ev:merge")],
    )
    .unwrap()
    .verify(
        "entity-resolution:blocking-check",
        vec![evidence("ev:blocking")],
    )
    .unwrap()
    .review(
        "entity-resolution:decision-review",
        vec![evidence("ev:decision")],
    )
    .unwrap();

    sink.fail_next_commit();
    let err = service
        .commit(PromotionTransitionRequest::Promote {
            decision_id: "fail-1".into(),
            candidate: reviewed_candidate,
            promoter: "contract:promoter:fail".into(),
            evidence_refs: vec![evidence("ev:promote")],
        })
        .await
        .expect_err("sink failure surfaces as a typed commit error");
    assert!(matches!(
        err,
        PromotionServiceError::Commit(PromotionCommitError::Storage(_))
    ));
    assert_eq!(sink.len(), 0, "no promoted decision may be durable");
    assert!(sink.stored("fail-1").is_none());
}

// ── slice verification: every attempt exposes the full audit shape ──

#[tokio::test]
async fn every_attempt_exposes_kind_identities_evidence_transition_and_typed_refusal() {
    let (service, sink) = service_with(FakeSink::default());

    // Successful promotion: kind, proposer, verifier, reviewer, promoter,
    // evidence references, transition result.
    let eligible = prompt_base()
        .propose(ContentDigest::of_text("candidate"))
        .unwrap()
        .evaluate(prompt_evaluation(), 500)
        .unwrap()
        .mark_eligible()
        .unwrap();
    let mapped = eligible.to_promotion_candidate().unwrap();
    let decision = advance_through_service(&service, mapped, "expose").await;
    let candidate = &decision.candidate;
    assert_eq!(candidate.kind, PromotionKind::Prompt);
    assert_eq!(candidate.proposer, "prompt:registry:summary");
    assert_eq!(
        candidate.verifier.as_deref(),
        Some("prompt:evaluation:summary")
    );
    assert_eq!(
        candidate.reviewer.as_deref(),
        Some("prompt:eligibility:summary")
    );
    assert_eq!(
        candidate.promoter.as_deref(),
        Some("contract:promoter:expose")
    );
    assert!(!candidate.evidence_refs.is_empty());
    assert_eq!(decision.transition, PromotionState::Promoted);

    // Refused attempt: typed refusal reason, nothing committed.
    let proposed = PromotionCandidate::propose(
        PromotionKind::Prompt,
        "prompt-cand-1",
        "prompt:registry:summary",
        vec![evidence("ev:propose")],
    )
    .unwrap();
    let err = service
        .commit(PromotionTransitionRequest::Verify {
            decision_id: "expose-neg".into(),
            candidate: proposed,
            verifier: "prompt:registry:summary".into(), // the proposer
            evidence_refs: vec![evidence("ev:x")],
        })
        .await
        .expect_err("self-confirm refused");
    assert!(matches!(
        err,
        PromotionServiceError::Domain(PromotionError::SelfConfirmation { .. })
    ));
    assert!(
        !sink.decisions.lock().unwrap().contains_key("expose-neg"),
        "refused attempt must not be durable"
    );
}
