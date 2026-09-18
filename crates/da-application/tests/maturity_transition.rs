//! M509-yzblqx S03 T01 — evidence completeness and human authorization are two
//! independently derived dimensions of the review transition.
//!
//! The tests drive the public derivation surfaces only: no fixture files, no
//! store, no graph, no parse route. They pin (1) `ReviewReady` as the evidence
//! terminal of a complete receipt set, (2) `Pending` as an explicit,
//! independently readable authorization dimension, and (3) the composite
//! `HumanGateRequired` label as *required*, never *granted*.

use da_application::derived_evidence_maturity::{
    BlockedReason, BoundReceipt, FamilyEvidenceSet, FamilyId, HumanAuthorization,
    MaturityDerivation, MaturityState, ReceiptKind, derive_gate_maturity, derive_maturity,
    derive_review_readiness, derive_review_readiness_with_authorization,
};
use da_application::derived_transition_evaluator::{
    ClaimedTransition, TransitionEvaluation, TransitionObservations, evaluate_transition,
};
use serde_json::json;

const SOURCE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OTHER_SHA256: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn receipt(kind: ReceiptKind, reference: &str) -> BoundReceipt {
    BoundReceipt {
        kind,
        evidence_ref: reference.into(),
        source_sha256: SOURCE_SHA256.into(),
        artifact_sha256: ARTIFACT_SHA256.into(),
        grobid_profile: "structural".into(),
    }
}

fn evidence_set(prefix: usize) -> FamilyEvidenceSet {
    let mut set = FamilyEvidenceSet {
        family: FamilyId::Body,
        paper_id: "paper-1".into(),
        inventory: None,
        oracle: None,
        atom: None,
        sequence: None,
        shadow: None,
        review: None,
        human_decision_ref: None,
        freshness: None,
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
    };
    for (index, kind) in ReceiptKind::ALL.into_iter().enumerate().take(prefix) {
        let value = Some(receipt(kind, &format!("ref-{index}")));
        match kind {
            ReceiptKind::Inventory => set.inventory = value,
            ReceiptKind::Oracle => set.oracle = value,
            ReceiptKind::Atom => set.atom = value,
            ReceiptKind::Sequence => set.sequence = value,
            ReceiptKind::Shadow => set.shadow = value,
            ReceiptKind::Review => set.review = value,
        }
    }
    set
}

fn complete_set() -> FamilyEvidenceSet {
    evidence_set(ReceiptKind::ALL.len())
}

fn approved(decision_ref: &str) -> HumanAuthorization {
    HumanAuthorization::Authorized {
        decision_ref: decision_ref.into(),
    }
}

/// Build the transition facts through the closed wire (`serde(default)` covers
/// the detector fields; the identity fields must match the receipts).
fn observations() -> TransitionObservations {
    serde_json::from_value(json!({
        "expected_paper_id": "paper-1",
        "expected_family": "body",
        "expected_source_sha256": SOURCE_SHA256,
        "expected_artifact_sha256": ARTIFACT_SHA256,
        "expected_profile": "structural",
        "observed_source_sha256": SOURCE_SHA256,
        "observed_artifact_sha256": ARTIFACT_SHA256,
    }))
    .expect("closed transition observations")
}

fn reached_state(derivation: MaturityDerivation) -> MaturityState {
    match derivation {
        MaturityDerivation::Reached { state, .. } => state,
        other => panic!("expected a reached state, got {other:?}"),
    }
}

fn blocked_reason(derivation: &MaturityDerivation) -> BlockedReason {
    match derivation {
        MaturityDerivation::Blocked { reason, .. } => *reason,
        other => panic!("expected a typed block, got {other:?}"),
    }
}

#[test]
fn complete_evidence_derives_review_ready_and_never_an_authorization() {
    let complete = complete_set();

    match derive_maturity(&complete) {
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } => {
            assert_eq!(
                state,
                MaturityState::ReviewReady,
                "the evidence terminal of a complete receipt set is ReviewReady"
            );
            assert_eq!(evidence_refs.len(), ReceiptKind::ALL.len());
        }
        other => panic!("complete evidence must derive a reached state, got {other:?}"),
    }

    let readiness = derive_review_readiness(&complete).expect("complete evidence is review-ready");
    assert_eq!(readiness.maturity, MaturityState::ReviewReady);
    assert_eq!(readiness.authorization, HumanAuthorization::Pending);
    assert_eq!(readiness.authorization.as_str(), "pending");
    assert!(!readiness.authorizes_production_or_import());

    // `human_decision_ref` is diagnostic provenance, never an approval: the
    // authorization dimension stays unresolved even when it is populated.
    let mut with_diagnostic_ref = complete.clone();
    with_diagnostic_ref.human_decision_ref = Some("diagnostic-only-ref".into());
    let readiness =
        derive_review_readiness(&with_diagnostic_ref).expect("evidence is still complete");
    assert_eq!(readiness.authorization, HumanAuthorization::Pending);
    assert!(!readiness.authorizes_production_or_import());
}

#[test]
fn explicit_authorization_is_a_separate_input_not_a_consequence_of_completeness() {
    let complete = complete_set();
    let approval = approved("human-approval-1");

    let authorized = derive_review_readiness_with_authorization(&complete, approval.clone())
        .expect("complete evidence is review-ready");
    assert_eq!(authorized.maturity, MaturityState::ReviewReady);
    assert_eq!(authorized.authorization, approval);
    assert!(authorized.authorizes_production_or_import());

    // The evidence dimension is identical with or without an approval: the two
    // dimensions are derived independently.
    let unresolved = derive_review_readiness(&complete).expect("complete evidence is review-ready");
    assert_eq!(authorized.maturity, unresolved.maturity);
    assert_eq!(authorized.evidence_refs, unresolved.evidence_refs);
    assert_ne!(authorized.authorization, unresolved.authorization);

    // An approval cannot fabricate evidence.
    let early = evidence_set(1);
    let early_authorized = derive_review_readiness_with_authorization(&early, approval)
        .expect("inventory is a valid prefix");
    assert_eq!(early_authorized.maturity, MaturityState::InventoryBound);
    assert!(!early_authorized.authorizes_production_or_import());

    // A blank reference is not an approval at all.
    let blank =
        derive_review_readiness_with_authorization(&complete, approved("   ")).expect("complete");
    assert_eq!(blank.authorization.as_str(), "authorized");
    assert!(!blank.authorizes_production_or_import());
}

#[test]
fn gate_label_is_required_not_granted_and_keeps_the_evidence_dimension() {
    let complete = complete_set();

    assert_eq!(
        reached_state(derive_gate_maturity(
            &complete,
            &HumanAuthorization::Pending
        )),
        MaturityState::HumanGateRequired,
        "complete evidence with an unresolved authorization requires the human gate"
    );
    assert_eq!(
        reached_state(derive_gate_maturity(
            &complete,
            &approved("human-approval-1")
        )),
        MaturityState::ReviewReady,
        "an explicit approval satisfies the gate without changing the evidence state"
    );
    assert_eq!(
        reached_state(derive_gate_maturity(&complete, &approved("   "))),
        MaturityState::HumanGateRequired,
        "a blank reference leaves the gate required"
    );

    // An incomplete set is never labelled as an awaiting-human gate.
    assert_eq!(
        reached_state(derive_gate_maturity(
            &evidence_set(3),
            &HumanAuthorization::Pending
        )),
        MaturityState::AtomProven
    );

    // Integrity flags still win over both dimensions.
    let mut pooling = complete_set();
    pooling.pooling = true;
    assert_eq!(
        blocked_reason(&derive_gate_maturity(
            &pooling,
            &HumanAuthorization::Pending
        )),
        BlockedReason::Pooling
    );
    assert!(matches!(
        derive_review_readiness_with_authorization(&pooling, approved("human-approval-1")),
        Err(MaturityDerivation::Blocked {
            reason: BlockedReason::Pooling,
            ..
        })
    ));
}

#[test]
fn missing_evidence_and_awaiting_human_are_distinguishable() {
    let facts = observations();
    let review = receipt(ReceiptKind::Review, "review-ref");

    // Evidence missing: a review receipt over a set without shadow is a ladder
    // gap, and the evaluation exposes no review-readiness dimension at all.
    let gapped = evaluate_transition(&evidence_set(4), Some(&review), &facts, None);
    assert!(matches!(
        gapped,
        TransitionEvaluation::Blocked {
            reason: BlockedReason::LadderGap,
            ..
        }
    ));
    assert!(gapped.review_readiness().is_none());

    // Awaiting a human: the same receipt over complete-prefix evidence lands on
    // the human gate, whose readiness is ReviewReady with `Pending`
    // authorization.
    let shadow_only = evidence_set(ReceiptKind::ALL.len() - 1);
    let gate = evaluate_transition(&shadow_only, Some(&review), &facts, None);
    assert!(matches!(gate, TransitionEvaluation::HumanGate { .. }));
    let readiness = gate
        .review_readiness()
        .expect("a human-gate landing exposes both review dimensions");
    assert_eq!(readiness.maturity, MaturityState::ReviewReady);
    assert_eq!(readiness.evidence_refs.len(), ReceiptKind::ALL.len());
    assert_eq!(readiness.authorization, HumanAuthorization::Pending);
    assert!(!readiness.authorizes_production_or_import());

    // A review claim may advertise either the evidence landing or the composite
    // gate label; both are visible to the operator as the same two dimensions.
    let claim = ClaimedTransition {
        current_state: Some(MaturityState::ShadowObserved),
        next_allowed_state: MaturityState::ReviewReady,
        goal_kind: ReceiptKind::Review,
    };
    let claimed = evaluate_transition(&shadow_only, Some(&review), &facts, Some(&claim));
    assert!(matches!(claimed, TransitionEvaluation::HumanGate { .. }));
    assert_eq!(
        claimed.review_readiness().map(|value| value.maturity),
        Some(MaturityState::ReviewReady)
    );

    // A non-review outcome carries no review-readiness dimension.
    let oracle = receipt(ReceiptKind::Oracle, "oracle-ref");
    let advanced = evaluate_transition(&evidence_set(1), Some(&oracle), &facts, None);
    assert!(matches!(advanced, TransitionEvaluation::Advanced { .. }));
    assert!(advanced.review_readiness().is_none());
}

#[test]
fn stale_or_malformed_evidence_closes_both_dimensions() {
    let complete = complete_set();

    // A stale observation blocks the transition with a typed reason; the human
    // authorization dimension cannot reopen it.
    let mut stale = observations();
    stale.observed_source_sha256 = Some(OTHER_SHA256.into());
    let blocked = evaluate_transition(&complete, None, &stale, None);
    assert!(matches!(
        blocked,
        TransitionEvaluation::Blocked {
            reason: BlockedReason::StaleIdentity,
            ..
        }
    ));
    assert!(blocked.review_readiness().is_none());

    // A malformed claim is rejected before any readiness is reported.
    let bad_claim = ClaimedTransition {
        current_state: Some(MaturityState::InventoryBound),
        next_allowed_state: MaturityState::ReviewReady,
        goal_kind: ReceiptKind::Review,
    };
    let malformed = evaluate_transition(&complete, None, &observations(), Some(&bad_claim));
    assert!(matches!(
        malformed,
        TransitionEvaluation::Blocked {
            reason: BlockedReason::MalformedPrompt,
            ..
        }
    ));
    assert!(malformed.review_readiness().is_none());
}
