//! Pure, fail-closed evaluation of one derived evidence transition.
//!
//! This module consumes only injected facts and immutable receipt identities. It
//! has no authority to write, import, promote, or consult a production route.

use serde::{Deserialize, Serialize};

use crate::derived_evidence_maturity::{
    BlockedReason, BoundReceipt, FamilyEvidenceSet, FamilyId, HumanAuthorization,
    MaturityDerivation, MaturityState, ReceiptKind, ReviewReadiness, can_advance,
    derive_gate_maturity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenominatorScope {
    Document,
    Corpus,
    Family,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenominatorObservation {
    pub name: String,
    pub claimed_n: u64,
    pub actual_n: Option<u64>,
    pub population_id: String,
    pub scope: DenominatorScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    #[default]
    Receipt,
    Prompt,
    ModelOutput,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionObservations {
    #[serde(default)]
    pub expected_paper_id: Option<String>,
    #[serde(default)]
    pub expected_family: Option<FamilyId>,
    #[serde(default)]
    pub expected_source_sha256: Option<String>,
    #[serde(default)]
    pub expected_artifact_sha256: Option<String>,
    #[serde(default)]
    pub expected_profile: Option<String>,
    #[serde(default)]
    pub observed_source_sha256: Option<String>,
    #[serde(default)]
    pub observed_artifact_sha256: Option<String>,
    #[serde(default)]
    pub denominators: Vec<DenominatorObservation>,
    #[serde(default)]
    pub oracle_imports: Vec<String>,
    #[serde(default)]
    pub forbidden_oracle_imports: Vec<String>,
    #[serde(default)]
    pub evidence_origin: EvidenceOrigin,
    #[serde(default)]
    pub conservation_held: Option<bool>,
    #[serde(default)]
    pub semantic_held: Option<bool>,
    #[serde(default)]
    pub called_symbols: Vec<String>,
    #[serde(default)]
    pub verification_exit_code: Option<i32>,
    #[serde(default)]
    pub empty_success: bool,
    #[serde(default)]
    pub typed_absence_named: bool,
    #[serde(default)]
    pub native_on_production_route: bool,
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimedTransition {
    pub current_state: Option<MaturityState>,
    pub next_allowed_state: MaturityState,
    pub goal_kind: ReceiptKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransitionEvaluation {
    Advanced {
        from: Option<MaturityState>,
        to: MaturityState,
        evidence_refs: Vec<String>,
        raised_flags: Vec<BlockedReason>,
    },
    Blocked {
        reason: BlockedReason,
        missing_or_offending: Option<String>,
        evidence_refs: Vec<String>,
        raised_flags: Vec<BlockedReason>,
    },
    HumanGate {
        evidence_refs: Vec<String>,
        raised_flags: Vec<BlockedReason>,
    },
    Idempotent {
        state: MaturityState,
        evidence_refs: Vec<String>,
        raised_flags: Vec<BlockedReason>,
    },
}

impl TransitionEvaluation {
    fn blocked(
        reason: BlockedReason,
        offending: Option<String>,
        refs: Vec<String>,
        raised_flags: Vec<BlockedReason>,
    ) -> Self {
        Self::Blocked {
            reason,
            missing_or_offending: offending,
            evidence_refs: refs,
            raised_flags,
        }
    }

    /// The two independently readable review dimensions of a human-gate landing.
    ///
    /// `Some` exactly when the evidence dimension is complete: that is
    /// `ReviewReady`, never `HumanGateRequired` (which means *required*, not a
    /// completed review). The authorization dimension observed here is the
    /// unresolved default `Pending`, because the evaluator consumes evidence
    /// only and has no approval input; an explicit approval is supplied through
    /// `derive_review_readiness_with_authorization`. `None` for every other
    /// outcome, so a caller can tell a review that lacks evidence apart from
    /// one that waits on a human decision.
    pub fn review_readiness(&self) -> Option<ReviewReadiness> {
        match self {
            Self::HumanGate { evidence_refs, .. } => Some(ReviewReadiness {
                maturity: MaturityState::ReviewReady,
                evidence_refs: evidence_refs.clone(),
                authorization: HumanAuthorization::Pending,
            }),
            _ => None,
        }
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn set_slot(set: &mut FamilyEvidenceSet, receipt: BoundReceipt) {
    match receipt.kind {
        ReceiptKind::Inventory => set.inventory = Some(receipt),
        ReceiptKind::Oracle => set.oracle = Some(receipt),
        ReceiptKind::Atom => set.atom = Some(receipt),
        ReceiptKind::Sequence => set.sequence = Some(receipt),
        ReceiptKind::Shadow => set.shadow = Some(receipt),
        ReceiptKind::Review => set.review = Some(receipt),
    }
}

fn slot(set: &FamilyEvidenceSet, kind: ReceiptKind) -> Option<&BoundReceipt> {
    match kind {
        ReceiptKind::Inventory => set.inventory.as_ref(),
        ReceiptKind::Oracle => set.oracle.as_ref(),
        ReceiptKind::Atom => set.atom.as_ref(),
        ReceiptKind::Sequence => set.sequence.as_ref(),
        ReceiptKind::Shadow => set.shadow.as_ref(),
        ReceiptKind::Review => set.review.as_ref(),
    }
}

fn same_identity(left: &BoundReceipt, right: &BoundReceipt) -> bool {
    left.kind == right.kind
        && left.evidence_ref == right.evidence_ref
        && left.source_sha256 == right.source_sha256
        && left.artifact_sha256 == right.artifact_sha256
        && left.grobid_profile == right.grobid_profile
}

fn validate_observations(
    observations: &TransitionObservations,
) -> Result<(), (BlockedReason, String)> {
    for (name, digest) in [
        (
            "expected_source_sha256",
            observations.expected_source_sha256.as_deref(),
        ),
        (
            "expected_artifact_sha256",
            observations.expected_artifact_sha256.as_deref(),
        ),
        (
            "observed_source_sha256",
            observations.observed_source_sha256.as_deref(),
        ),
        (
            "observed_artifact_sha256",
            observations.observed_artifact_sha256.as_deref(),
        ),
    ] {
        if let Some(value) = digest
            && !valid_digest(value)
        {
            return Err((BlockedReason::MalformedReceipt, name.to_owned()));
        }
    }
    if let Some(profile) = observations.expected_profile.as_deref()
        && profile != "structural"
        && profile != "rich"
    {
        return Err((
            BlockedReason::MalformedReceipt,
            "expected_profile".to_owned(),
        ));
    }
    for denominator in &observations.denominators {
        if denominator.name.trim().is_empty() {
            return Err((
                BlockedReason::MalformedReceipt,
                "denominator.name".to_owned(),
            ));
        }
        if denominator.population_id.trim().is_empty() {
            return Err((
                BlockedReason::MalformedReceipt,
                "denominator.population_id".to_owned(),
            ));
        }
    }
    Ok(())
}

fn raised_flags(set: &FamilyEvidenceSet, before: &FamilyEvidenceSet) -> Vec<BlockedReason> {
    let mut result = Vec::new();
    let pairs = [
        (
            set.claims_promoted && !before.claims_promoted,
            BlockedReason::AttemptedAutomaticPromotion,
        ),
        (
            set.deployed && !before.deployed,
            BlockedReason::AttemptedAutomaticPromotion,
        ),
        (
            set.import_eligible && !before.import_eligible,
            BlockedReason::AttemptedAutomaticPromotion,
        ),
        (set.pooling && !before.pooling, BlockedReason::Pooling),
        (
            set.contamination && !before.contamination,
            BlockedReason::Contamination,
        ),
        (
            set.stale_identity && !before.stale_identity,
            BlockedReason::StaleIdentity,
        ),
        (
            set.denominator_drift && !before.denominator_drift,
            BlockedReason::DenominatorDrift,
        ),
        (
            set.conservation_loss && !before.conservation_loss,
            BlockedReason::ConservationLoss,
        ),
        (
            set.semantic_drift && !before.semantic_drift,
            BlockedReason::SemanticDrift,
        ),
        (
            set.unsafe_production_call && !before.unsafe_production_call,
            BlockedReason::UnsafeProductionCall,
        ),
        (
            set.failed_evidence && !before.failed_evidence,
            BlockedReason::FailedEvidence,
        ),
    ];
    for (present, reason) in pairs {
        if present && !result.contains(&reason) {
            result.push(reason);
        }
    }
    result
}

fn evidence_refs(derivation: &MaturityDerivation) -> Vec<String> {
    match derivation {
        MaturityDerivation::Reached { evidence_refs, .. }
        | MaturityDerivation::Blocked { evidence_refs, .. } => evidence_refs.clone(),
    }
}

/// Evaluate exactly one proposed receipt against an immutable prior set.
///
/// All observations are treated as untrusted facts. The prior is cloned before
/// flags or a receipt are applied, and the returned enum is diagnostic only.
pub fn evaluate_transition(
    prior: &FamilyEvidenceSet,
    proposed: Option<&BoundReceipt>,
    observations: &TransitionObservations,
    claimed: Option<&ClaimedTransition>,
) -> TransitionEvaluation {
    if let Err((reason, offending)) = validate_observations(observations) {
        return TransitionEvaluation::blocked(reason, Some(offending), Vec::new(), Vec::new());
    }

    let mut candidate = prior.clone();
    if observations
        .expected_paper_id
        .as_deref()
        .is_some_and(|id| id.trim() != prior.paper_id.trim())
        || observations
            .expected_family
            .is_some_and(|family| family != prior.family)
    {
        candidate.stale_identity = true;
    }
    if let Some(expected) = observations.expected_source_sha256.as_deref()
        && (candidate_receipts(prior, proposed)
            .iter()
            .any(|receipt| receipt.source_sha256 != expected)
            || observations
                .observed_source_sha256
                .as_deref()
                .is_some_and(|actual| actual != expected))
    {
        candidate.stale_identity = true;
    }
    if let Some(expected) = observations.expected_artifact_sha256.as_deref()
        && (proposed.is_some_and(|receipt| receipt.artifact_sha256 != expected)
            || observations
                .observed_artifact_sha256
                .as_deref()
                .is_some_and(|actual| actual != expected))
    {
        candidate.stale_identity = true;
    }
    if let Some(expected) = observations.expected_profile.as_deref()
        && candidate_receipts(prior, proposed)
            .iter()
            .any(|receipt| receipt.grobid_profile != expected)
    {
        candidate.stale_identity = true;
    }

    let mut same_receipt = false;
    if let Some(receipt) = proposed {
        if let Some(existing) = slot(&candidate, receipt.kind) {
            if same_identity(existing, receipt) {
                same_receipt = true;
            } else {
                candidate.stale_identity = true;
            }
        } else {
            set_slot(&mut candidate, receipt.clone());
        }
    }

    if matches!(
        observations.evidence_origin,
        EvidenceOrigin::Prompt | EvidenceOrigin::ModelOutput
    ) || !matches!(observations.evidence_origin, EvidenceOrigin::Receipt)
    {
        candidate.contamination = true;
    }
    if observations.oracle_imports.iter().any(|item| {
        observations
            .forbidden_oracle_imports
            .iter()
            .any(|forbidden| forbidden == item)
    }) {
        candidate.contamination = true;
    }
    if observations
        .denominators
        .iter()
        .any(|item| item.actual_n != Some(item.claimed_n))
        && observations
            .denominators
            .iter()
            .any(|item| item.actual_n.is_some())
    {
        candidate.denominator_drift = true;
    }
    for left in &observations.denominators {
        for right in &observations.denominators {
            if left.name == right.name
                && (left.population_id != right.population_id
                    || (left.scope == DenominatorScope::Corpus
                        && right.scope == DenominatorScope::Document)
                    || (left.scope == DenominatorScope::Document
                        && right.scope == DenominatorScope::Corpus))
            {
                candidate.pooling = true;
            }
        }
    }
    if observations.conservation_held == Some(false) {
        candidate.conservation_loss = true;
    }
    if observations.semantic_held == Some(false) {
        candidate.semantic_drift = true;
    }
    const FORBIDDEN: [&str; 9] = [
        "parse-pdf",
        "ParserPort",
        "source_cli",
        "GraphStore",
        "import",
        "promoted",
        "run_parse_pdf",
        "import_write",
        "PromotionService",
    ];
    if observations.native_on_production_route
        || observations
            .called_symbols
            .iter()
            .any(|symbol| FORBIDDEN.contains(&symbol.as_str()))
    {
        candidate.unsafe_production_call = true;
    }
    if observations
        .verification_exit_code
        .is_some_and(|code| code != 0)
        || (observations.empty_success && !observations.typed_absence_named)
    {
        candidate.failed_evidence = true;
    }
    candidate.claims_promoted |= observations.claims_promoted;
    candidate.deployed |= observations.deployed;
    candidate.import_eligible |= observations.import_eligible;

    // Earlier proof rungs may carry an explicitly unknown denominator. Review
    // is different: it is the landing proof for the terminal human gate, so a
    // missing exact n must be surfaced instead of being mistaken for complete.
    // Compute the landing before adding this diagnostic flag, otherwise the
    // flag itself would hide the state that triggered the requirement.
    let candidate_reaches_human_gate = matches!(
        // The evaluator has no approval input, so the composite gate label is
        // derived against the unresolved default authorization.
        derive_gate_maturity(&candidate, &HumanAuthorization::Pending),
        MaturityDerivation::Reached {
            state: MaturityState::HumanGateRequired,
            ..
        }
    );
    let claimed_review_ready =
        claimed.is_some_and(|claim| claim.next_allowed_state == MaturityState::ReviewReady);
    if observations
        .denominators
        .iter()
        .any(|item| item.actual_n.is_none())
        && (claimed_review_ready || candidate_reaches_human_gate)
    {
        candidate.denominator_drift = true;
    }

    let raised = raised_flags(&candidate, prior);
    let derivation = derive_gate_maturity(&candidate, &HumanAuthorization::Pending);
    let prior_derivation = derive_gate_maturity(prior, &HumanAuthorization::Pending);
    let from = match prior_derivation {
        MaturityDerivation::Reached { state, .. } => Some(state),
        MaturityDerivation::Blocked {
            reason: BlockedReason::MissingEvidence,
            ..
        } if prior.inventory.is_none() => None,
        MaturityDerivation::Blocked { .. } => None,
    };

    if let Some(offending) = reconcile_claim(claimed, proposed, &prior_derivation, &derivation) {
        return TransitionEvaluation::blocked(
            BlockedReason::MalformedPrompt,
            Some(offending),
            evidence_refs(&derivation),
            raised,
        );
    }
    match derivation {
        MaturityDerivation::Blocked {
            reason,
            missing_or_offending,
            evidence_refs,
        } => TransitionEvaluation::blocked(reason, missing_or_offending, evidence_refs, raised),
        MaturityDerivation::Reached {
            state: MaturityState::HumanGateRequired,
            evidence_refs,
        } if proposed.is_none()
            || from == Some(MaturityState::HumanGateRequired)
            || (proposed.is_some_and(|receipt| receipt.kind == ReceiptKind::Review)
                && matches!(
                    from,
                    Some(MaturityState::ShadowObserved | MaturityState::ReviewReady)
                )) =>
        {
            TransitionEvaluation::HumanGate {
                evidence_refs,
                raised_flags: raised,
            }
        }
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } if from.is_none()
            && state == MaturityState::InventoryBound
            && proposed.is_some_and(|r| r.kind == ReceiptKind::Inventory) =>
        {
            TransitionEvaluation::Advanced {
                from,
                to: state,
                evidence_refs,
                raised_flags: raised,
            }
        }
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } if from.is_some_and(|source| can_advance(source, state)) => {
            TransitionEvaluation::Advanced {
                from,
                to: state,
                evidence_refs,
                raised_flags: raised,
            }
        }
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } if from == Some(state) && (proposed.is_none() || same_receipt) => {
            TransitionEvaluation::Idempotent {
                state,
                evidence_refs,
                raised_flags: raised,
            }
        }
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } => TransitionEvaluation::blocked(
            BlockedReason::MalformedPrompt,
            Some(state.as_str().to_owned()),
            evidence_refs,
            raised,
        ),
    }
}

fn expected_goal(state: Option<MaturityState>) -> ReceiptKind {
    match state {
        None => ReceiptKind::Inventory,
        Some(MaturityState::InventoryBound) => ReceiptKind::Oracle,
        Some(MaturityState::OracleBound) => ReceiptKind::Atom,
        Some(MaturityState::AtomProven) => ReceiptKind::Sequence,
        Some(MaturityState::SequenceProven) => ReceiptKind::Shadow,
        Some(MaturityState::ShadowObserved | MaturityState::ReviewReady)
        | Some(MaturityState::HumanGateRequired) => ReceiptKind::Review,
    }
}

fn reached_state(derivation: &MaturityDerivation) -> Option<MaturityState> {
    match derivation {
        MaturityDerivation::Reached { state, .. } => Some(*state),
        MaturityDerivation::Blocked { .. } => None,
    }
}

/// Reconcile an optional untrusted claim with the actual immutable derivation.
/// Review deliberately accepts both the advertised `review_ready` landing and
/// the evaluator's terminal `human_gate_required` landing.
fn reconcile_claim(
    claimed: Option<&ClaimedTransition>,
    proposed: Option<&BoundReceipt>,
    prior: &MaturityDerivation,
    candidate: &MaturityDerivation,
) -> Option<String> {
    let claim = claimed?;
    let prior_state = reached_state(prior).or_else(|| {
        matches!(
            prior,
            MaturityDerivation::Blocked {
                reason: BlockedReason::MissingEvidence,
                ..
            }
        )
        .then_some(MaturityState::InventoryBound)
        .filter(|_| false)
    });

    // A blocked prior/candidate keeps its typed evidence reason as the primary
    // diagnostic. Claims are reconciled whenever the actual landing is known,
    // including exact replay with no proposed receipt.
    let landing = reached_state(candidate)?;
    if claim.current_state != prior_state {
        return Some("current_state".to_owned());
    }

    let goal = proposed
        .map(|receipt| receipt.kind)
        .unwrap_or_else(|| expected_goal(prior_state));
    if claim.goal_kind != goal {
        return Some("goal_kind".to_owned());
    }

    let is_review = goal == ReceiptKind::Review;
    let expected_next = if proposed.is_none() && prior_state == Some(landing) {
        landing.successor().unwrap_or(landing)
    } else {
        landing
    };
    let next_ok = if is_review {
        matches!(
            claim.next_allowed_state,
            MaturityState::ReviewReady | MaturityState::HumanGateRequired
        )
    } else {
        claim.next_allowed_state == expected_next
    };
    if !next_ok {
        return Some("next_allowed_state".to_owned());
    }

    None
}

fn candidate_receipts<'a>(
    prior: &'a FamilyEvidenceSet,
    proposed: Option<&'a BoundReceipt>,
) -> Vec<&'a BoundReceipt> {
    let mut receipts = ReceiptKind::ALL
        .into_iter()
        .filter_map(|kind| slot(prior, kind))
        .collect::<Vec<_>>();
    if let Some(receipt) = proposed {
        receipts.push(receipt);
    }
    receipts
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn observations() -> TransitionObservations {
        TransitionObservations {
            expected_paper_id: Some("paper-1".into()),
            expected_family: Some(FamilyId::Body),
            expected_source_sha256: Some(DIGEST.into()),
            expected_artifact_sha256: Some(DIGEST.into()),
            expected_profile: Some("structural".into()),
            observed_source_sha256: Some(DIGEST.into()),
            observed_artifact_sha256: Some(DIGEST.into()),
            ..Default::default()
        }
    }

    impl Default for TransitionObservations {
        fn default() -> Self {
            Self {
                expected_paper_id: None,
                expected_family: None,
                expected_source_sha256: None,
                expected_artifact_sha256: None,
                expected_profile: None,
                observed_source_sha256: None,
                observed_artifact_sha256: None,
                denominators: Vec::new(),
                oracle_imports: Vec::new(),
                forbidden_oracle_imports: Vec::new(),
                evidence_origin: EvidenceOrigin::Receipt,
                conservation_held: None,
                semantic_held: None,
                called_symbols: Vec::new(),
                verification_exit_code: None,
                empty_success: false,
                typed_absence_named: false,
                native_on_production_route: false,
                claims_promoted: false,
                deployed: false,
                import_eligible: false,
            }
        }
    }

    fn prior() -> FamilyEvidenceSet {
        FamilyEvidenceSet {
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
        }
    }

    fn receipt(kind: ReceiptKind, reference: &str) -> BoundReceipt {
        BoundReceipt {
            kind,
            evidence_ref: reference.into(),
            source_sha256: DIGEST.into(),
            artifact_sha256: DIGEST.into(),
            grobid_profile: "structural".into(),
        }
    }

    #[test]
    fn wire_is_closed_and_unknown_fields_fail() {
        let value = serde_json::to_string(&observations()).unwrap();
        assert!(serde_json::from_str::<TransitionObservations>(&value).is_ok());
        assert!(serde_json::from_str::<TransitionObservations>(&format!("{value}}}")).is_err());
        assert!(serde_json::from_str::<DenominatorScope>("\"not_a_scope\"").is_err());
    }

    #[test]
    fn malformed_identity_wins_before_stale() {
        let mut facts = observations();
        facts.expected_source_sha256 = Some("bad".into());
        facts.expected_paper_id = Some("other".into());
        assert!(matches!(
            evaluate_transition(&prior(), None, &facts, None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::MalformedReceipt,
                ..
            }
        ));
    }

    #[test]
    fn inventory_bootstrap_and_one_step_advance() {
        let inventory = receipt(ReceiptKind::Inventory, "inventory");
        assert!(matches!(
            evaluate_transition(&prior(), Some(&inventory), &observations(), None),
            TransitionEvaluation::Advanced {
                from: None,
                to: MaturityState::InventoryBound,
                ..
            }
        ));
        let mut seeded = prior();
        seeded.inventory = Some(inventory);
        let oracle = receipt(ReceiptKind::Oracle, "oracle");
        assert!(matches!(
            evaluate_transition(&seeded, Some(&oracle), &observations(), None),
            TransitionEvaluation::Advanced {
                from: Some(MaturityState::InventoryBound),
                to: MaturityState::OracleBound,
                ..
            }
        ));
    }

    #[test]
    fn detectors_union_without_mutating_prior() {
        let original = prior();
        let mut facts = observations();
        facts.evidence_origin = EvidenceOrigin::Prompt;
        facts.conservation_held = Some(false);
        facts.semantic_held = Some(false);
        facts.native_on_production_route = true;
        facts.verification_exit_code = Some(2);
        facts.empty_success = true;
        facts.claims_promoted = true;
        let result = evaluate_transition(&original, None, &facts, None);
        assert!(matches!(
            result,
            TransitionEvaluation::Blocked {
                reason: BlockedReason::AttemptedAutomaticPromotion,
                ..
            }
        ));
        assert!(!original.contamination && !original.claims_promoted);
    }

    #[test]
    fn pooling_requires_population_identity_not_equal_counts() {
        let mut facts = observations();
        facts.denominators = vec![
            DenominatorObservation {
                name: "items".into(),
                claimed_n: 2,
                actual_n: Some(2),
                population_id: "a".into(),
                scope: DenominatorScope::Family,
            },
            DenominatorObservation {
                name: "other".into(),
                claimed_n: 2,
                actual_n: Some(2),
                population_id: "b".into(),
                scope: DenominatorScope::Family,
            },
        ];
        assert!(!matches!(
            evaluate_transition(&prior(), None, &facts, None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::Pooling,
                ..
            }
        ));
        facts.denominators[1].name = "items".into();
        assert!(matches!(
            evaluate_transition(&prior(), None, &facts, None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::Pooling,
                ..
            }
        ));
    }

    #[test]
    fn review_landing_requires_exact_n_and_accepts_both_review_claim_states() {
        let mut seeded = prior();
        for (index, kind) in ReceiptKind::ALL
            .into_iter()
            .take(ReceiptKind::ALL.len() - 1)
            .enumerate()
        {
            set_slot(&mut seeded, receipt(kind, &format!("ref-{index}")));
        }
        let review = receipt(ReceiptKind::Review, "review");
        let mut facts = observations();
        facts.denominators = vec![DenominatorObservation {
            name: "sections".into(),
            claimed_n: 6,
            actual_n: Some(6),
            population_id: "paper-1".into(),
            scope: DenominatorScope::Document,
        }];
        let claim = ClaimedTransition {
            current_state: Some(MaturityState::ShadowObserved),
            next_allowed_state: MaturityState::HumanGateRequired,
            goal_kind: ReceiptKind::Review,
        };
        assert!(matches!(
            evaluate_transition(&seeded, Some(&review), &facts, Some(&claim)),
            TransitionEvaluation::HumanGate { .. }
        ));

        let mut incomplete = facts.clone();
        incomplete.denominators[0].actual_n = None;
        assert!(matches!(
            evaluate_transition(&seeded, Some(&review), &incomplete, Some(&claim)),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::DenominatorDrift,
                ..
            }
        ));

        let mut ready_claim = claim.clone();
        ready_claim.next_allowed_state = MaturityState::ReviewReady;
        assert!(matches!(
            evaluate_transition(&seeded, Some(&review), &facts, Some(&ready_claim)),
            TransitionEvaluation::HumanGate { .. }
        ));
    }

    #[test]
    fn claims_reconcile_current_goal_and_next_without_skipping() {
        let inventory = receipt(ReceiptKind::Inventory, "inventory");
        let mut seeded = prior();
        seeded.inventory = Some(inventory.clone());
        let oracle = receipt(ReceiptKind::Oracle, "oracle");

        let valid = ClaimedTransition {
            current_state: Some(MaturityState::InventoryBound),
            next_allowed_state: MaturityState::OracleBound,
            goal_kind: ReceiptKind::Oracle,
        };
        assert!(matches!(
            evaluate_transition(&seeded, Some(&oracle), &observations(), Some(&valid)),
            TransitionEvaluation::Advanced { .. }
        ));

        for (field, claim) in [
            (
                "current_state",
                ClaimedTransition {
                    current_state: Some(MaturityState::OracleBound),
                    ..valid.clone()
                },
            ),
            (
                "goal_kind",
                ClaimedTransition {
                    goal_kind: ReceiptKind::Atom,
                    ..valid.clone()
                },
            ),
            (
                "next_allowed_state",
                ClaimedTransition {
                    next_allowed_state: MaturityState::SequenceProven,
                    ..valid.clone()
                },
            ),
        ] {
            match evaluate_transition(&seeded, Some(&oracle), &observations(), Some(&claim)) {
                TransitionEvaluation::Blocked {
                    reason: BlockedReason::MalformedPrompt,
                    missing_or_offending: Some(offending),
                    ..
                } => assert_eq!(offending, field),
                other => panic!("{field} claim unexpectedly evaluated as {other:?}"),
            }
        }

        let replay_claim = ClaimedTransition {
            current_state: Some(MaturityState::InventoryBound),
            next_allowed_state: MaturityState::OracleBound,
            goal_kind: ReceiptKind::Oracle,
        };
        assert!(matches!(
            evaluate_transition(&seeded, None, &observations(), Some(&replay_claim)),
            TransitionEvaluation::Idempotent { .. }
        ));
        let mut conflicting = replay_claim;
        conflicting.next_allowed_state = MaturityState::SequenceProven;
        assert!(matches!(
            evaluate_transition(&seeded, None, &observations(), Some(&conflicting)),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::MalformedPrompt,
                missing_or_offending: Some(ref field),
                ..
            } if field == "next_allowed_state"
        ));
    }

    #[test]
    fn seven_state_matrix_advances_only_one_rung() {
        // Five ordinary receipts advance one rung. Review is intentionally
        // asymmetric: S02 advertises ReviewReady, while six receipts land in
        // the terminal HumanGateRequired state. The seventh row checks the
        // stable terminal replay.
        let expected = [
            (
                0usize,
                ReceiptKind::Inventory,
                MaturityState::InventoryBound,
            ),
            (1, ReceiptKind::Oracle, MaturityState::OracleBound),
            (2, ReceiptKind::Atom, MaturityState::AtomProven),
            (3, ReceiptKind::Sequence, MaturityState::SequenceProven),
            (4, ReceiptKind::Shadow, MaturityState::ShadowObserved),
        ];
        for (prefix_len, kind, state) in expected {
            let mut seeded = prior();
            for (receipt_index, prior_kind) in
                ReceiptKind::ALL.into_iter().enumerate().take(prefix_len)
            {
                set_slot(
                    &mut seeded,
                    receipt(prior_kind, &format!("ref-{receipt_index}")),
                );
            }
            let proposed = receipt(kind, &format!("next-{prefix_len}"));
            assert!(matches!(
                evaluate_transition(&seeded, Some(&proposed), &observations(), None),
                TransitionEvaluation::Advanced { to, .. } if to == state
            ));
        }

        let mut shadow = prior();
        for (index, kind) in ReceiptKind::ALL.into_iter().enumerate().take(5) {
            set_slot(&mut shadow, receipt(kind, &format!("ref-{index}")));
        }
        let review = receipt(ReceiptKind::Review, "next-review");
        assert!(matches!(
            evaluate_transition(&shadow, Some(&review), &observations(), None),
            TransitionEvaluation::HumanGate { .. }
        ));

        let mut complete = shadow;
        set_slot(&mut complete, review);
        assert!(matches!(
            evaluate_transition(&complete, None, &observations(), None),
            TransitionEvaluation::HumanGate { .. }
        ));
    }

    #[test]
    fn replay_is_deterministic_and_evaluation_wire_roundtrips() {
        let inventory = receipt(ReceiptKind::Inventory, "inventory");
        let first = evaluate_transition(&prior(), Some(&inventory), &observations(), None);
        let second = evaluate_transition(&prior(), Some(&inventory), &observations(), None);
        assert_eq!(first, second);
        let encoded = serde_json::to_string(&first).unwrap();
        assert_eq!(first, serde_json::from_str(&encoded).unwrap());
    }

    #[test]
    fn replacement_and_replay_are_fail_closed_and_deterministic() {
        let inventory = receipt(ReceiptKind::Inventory, "inventory");
        let mut seeded = prior();
        seeded.inventory = Some(inventory.clone());
        let replay = evaluate_transition(&seeded, Some(&inventory), &observations(), None);
        assert!(matches!(
            replay,
            TransitionEvaluation::Idempotent {
                state: MaturityState::InventoryBound,
                ..
            }
        ));
        let replacement = receipt(ReceiptKind::Inventory, "other");
        assert!(matches!(
            evaluate_transition(&seeded, Some(&replacement), &observations(), None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::StaleIdentity,
                ..
            }
        ));
    }
}
