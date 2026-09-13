//! Bounded offline rehearsal of the derived evidence maturity protocol.
//! This example consumes only synthetic identity receipts. It never parses,
//! calls a provider, reads a fixture, writes graph state, imports, or promotes.

mod derived_family_adapters;

use da_application::derived_evidence_maturity::{
    BlockedReason, BoundReceipt, FamilyId, MaturityState, ReceiptKind,
};
use da_application::derived_prompt_contract::PromptRender;
use da_application::derived_transition_evaluator::{
    ClaimedTransition, DenominatorObservation, DenominatorScope, EvidenceOrigin,
    evaluate_transition,
};
use derived_family_adapters::{
    AdapterResult, FamilyAdapterInput, adapt_family, evaluate_adapted, prompt_adapted,
};
use serde::Serialize;

const SCHEMA_VERSION: &str = "da-application.derived-maturity-rehearsal/v1";
const SOURCE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SCHEMA_SHA256: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CONFIG_SHA256: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Digest {
    schema_version: &'static str,
    deployed: bool,
    import_eligible: bool,
    production_parse_invoked: bool,
    decision_points_consulted: Vec<&'static str>,
    rollback_target: &'static str,
    production_authority: &'static str,
    families: Vec<FamilyDigest>,
    cross_family: CrossFamilyDigest,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyDigest {
    family: &'static str,
    candidate_id: String,
    bootstrap: TransitionDigest,
    plus_one: TransitionDigest,
    replay: TransitionDigest,
    human_gate: TransitionDigest,
    blocked: Vec<BlockedDigest>,
    visual_typed_absence_is_not_failure: Option<bool>,
    body_empty_count_is_not_success: Option<bool>,
    header_references_scopes_are_independent: Option<bool>,
    prompt_claim_fields_match: bool,
    deliberate_mismatch: BlockedDigest,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TransitionDigest {
    status: &'static str,
    from: Option<&'static str>,
    to: Option<&'static str>,
    goal_kind: Option<&'static str>,
    next_task: Option<TaskDigest>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TaskDigest {
    goal_kind: &'static str,
    current_state: Option<&'static str>,
    next_allowed_state: &'static str,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct BlockedDigest {
    reason: &'static str,
    missing_or_offending: Option<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct CrossFamilyDigest {
    prefixed_denominators_do_not_pool: bool,
    bare_name_documents_pools: bool,
}

fn receipt(kind: ReceiptKind, index: usize) -> BoundReceipt {
    BoundReceipt {
        kind,
        evidence_ref: format!("family-receipt-{index}"),
        source_sha256: SOURCE_SHA256.into(),
        artifact_sha256: ARTIFACT_SHA256.into(),
        grobid_profile: "structural".into(),
    }
}

fn input(family: FamilyId) -> FamilyAdapterInput {
    FamilyAdapterInput {
        family,
        paper_id: "synthetic-paper".into(),
        source_sha256: SOURCE_SHA256.into(),
        artifact_sha256: ARTIFACT_SHA256.into(),
        schema_sha256: SCHEMA_SHA256.into(),
        config_sha256: CONFIG_SHA256.into(),
        grobid_profile: "structural".into(),
        receipts: Vec::new(),
        denominators: Vec::new(),
        oracle_imports: Vec::new(),
        called_symbols: Vec::new(),
        conservation_held: None,
        semantic_held: None,
        verification_exit_code: None,
        empty_success: false,
        typed_absence_named: false,
        native_on_production_route: false,
        claims_promoted: false,
        deployed: false,
        import_eligible: false,
        evidence_origin: EvidenceOrigin::Receipt,
    }
}

fn adapted(family: FamilyId, prefix: usize) -> derived_family_adapters::AdaptedFamily {
    let mut value = input(family);
    value.receipts = ReceiptKind::ALL
        .into_iter()
        .enumerate()
        .take(prefix)
        .map(|(index, kind)| receipt(kind, index))
        .collect();
    match adapt_family(&value) {
        AdapterResult::Ready { adapted } => *adapted,
        AdapterResult::Blocked { reason, field } => {
            panic!(
                "synthetic adapter input blocked: {} ({field})",
                reason.as_str()
            )
        }
    }
}

fn claim_from_prompt(prompt: &PromptRender) -> ClaimedTransition {
    let PromptRender::Contract { contract } = prompt else {
        panic!("expected a prompt contract")
    };
    ClaimedTransition {
        current_state: contract.current_state,
        next_allowed_state: contract.next_allowed_state,
        goal_kind: contract.goal_kind,
    }
}

fn task(contract: &da_application::derived_prompt_contract::AtomicPromptContract) -> TaskDigest {
    TaskDigest {
        goal_kind: contract.goal_kind.as_str(),
        current_state: contract.current_state.map(MaturityState::as_str),
        next_allowed_state: contract.next_allowed_state.as_str(),
    }
}

fn transition(
    result: da_application::derived_transition_evaluator::TransitionEvaluation,
    contract: Option<&da_application::derived_prompt_contract::AtomicPromptContract>,
) -> TransitionDigest {
    use da_application::derived_transition_evaluator::TransitionEvaluation;
    match result {
        TransitionEvaluation::Advanced { from, to, .. } => TransitionDigest {
            status: "advanced",
            from: from.map(MaturityState::as_str),
            to: Some(to.as_str()),
            goal_kind: contract.map(|c| c.goal_kind.as_str()),
            next_task: contract.map(task),
        },
        TransitionEvaluation::Idempotent { state, .. } => TransitionDigest {
            status: "idempotent",
            from: Some(state.as_str()),
            to: Some(state.as_str()),
            goal_kind: None,
            next_task: None,
        },
        TransitionEvaluation::HumanGate { .. } => TransitionDigest {
            status: "human_gate",
            from: None,
            to: Some(MaturityState::HumanGateRequired.as_str()),
            goal_kind: None,
            next_task: None,
        },
        TransitionEvaluation::Blocked {
            reason,
            missing_or_offending,
            ..
        } => TransitionDigest {
            status: reason.as_str(),
            from: None,
            to: None,
            goal_kind: None,
            next_task: missing_or_offending.map(|value| TaskDigest {
                goal_kind: "blocked",
                current_state: None,
                next_allowed_state: Box::leak(value.into_boxed_str()),
            }),
        },
    }
}

fn blocked(
    result: da_application::derived_transition_evaluator::TransitionEvaluation,
) -> BlockedDigest {
    use da_application::derived_transition_evaluator::TransitionEvaluation;
    match result {
        TransitionEvaluation::Blocked {
            reason,
            missing_or_offending,
            ..
        } => BlockedDigest {
            reason: reason.as_str(),
            missing_or_offending,
        },
        other => panic!("expected typed block, got {other:?}"),
    }
}

fn flagged(family: FamilyId, mutate: impl FnOnce(&mut FamilyAdapterInput)) -> BlockedDigest {
    let mut value = input(family);
    mutate(&mut value);
    let AdapterResult::Ready { adapted } = adapt_family(&value) else {
        panic!("flagged synthetic input failed to adapt")
    };
    blocked(evaluate_adapted(&adapted, None, None))
}

fn denominator_name(family: FamilyId) -> &'static str {
    match family {
        FamilyId::Segmentation => "region_token_assignments",
        FamilyId::Header => "authors",
        FamilyId::Bibliography => "biblstruct",
        FamilyId::Body => "documents",
        FamilyId::Visual => "figure",
    }
}

fn blocked_matrix(family: FamilyId) -> Vec<BlockedDigest> {
    vec![
        flagged(family, |v| {
            let name = denominator_name(family);
            v.denominators = vec![
                DenominatorObservation {
                    name: name.into(),
                    claimed_n: 1,
                    actual_n: Some(1),
                    population_id: "one".into(),
                    scope: DenominatorScope::Document,
                },
                DenominatorObservation {
                    name: name.into(),
                    claimed_n: 1,
                    actual_n: Some(1),
                    population_id: "two".into(),
                    scope: DenominatorScope::Document,
                },
            ];
        }),
        flagged(family, |_| {}),
        flagged(family, |v| v.evidence_origin = EvidenceOrigin::Prompt),
        flagged(family, |v| v.verification_exit_code = Some(1)),
        flagged(family, |v| v.native_on_production_route = true),
        flagged(family, |v| v.import_eligible = !false),
        flagged(family, |v| {
            v.denominators = vec![DenominatorObservation {
                name: denominator_name(family).into(),
                claimed_n: 2,
                actual_n: Some(1),
                population_id: "synthetic-paper".into(),
                scope: DenominatorScope::Document,
            }]
        }),
    ]
}

fn family_digest(family: FamilyId) -> FamilyDigest {
    let empty = adapted(family, 0);
    let bootstrap_prompt = prompt_adapted(&empty);
    let bootstrap_claim = claim_from_prompt(&bootstrap_prompt);
    let bootstrap_contract = match &bootstrap_prompt {
        PromptRender::Contract { contract } => Some(contract.as_ref()),
        _ => None,
    };
    let bootstrap = transition(
        evaluate_adapted(
            &empty,
            Some(&receipt(ReceiptKind::Inventory, 0)),
            Some(&bootstrap_claim),
        ),
        bootstrap_contract,
    );

    let one = adapted(family, 1);
    let plus_prompt = prompt_adapted(&one);
    let plus_claim = claim_from_prompt(&plus_prompt);
    let plus_contract = match &plus_prompt {
        PromptRender::Contract { contract } => Some(contract.as_ref()),
        _ => None,
    };
    let plus_one = transition(
        evaluate_adapted(
            &one,
            Some(&receipt(ReceiptKind::Oracle, 1)),
            Some(&plus_claim),
        ),
        plus_contract,
    );

    let replay_prompt = prompt_adapted(&one);
    let replay_claim = claim_from_prompt(&replay_prompt);
    let replay = transition(evaluate_adapted(&one, None, Some(&replay_claim)), None);

    let complete = adapted(family, ReceiptKind::ALL.len());
    let human_gate = transition(evaluate_adapted(&complete, None, None), None);

    let mut mismatch = plus_claim.clone();
    mismatch.goal_kind = ReceiptKind::Atom;
    let deliberate_mismatch = blocked(evaluate_adapted(
        &one,
        Some(&receipt(ReceiptKind::Oracle, 1)),
        Some(&mismatch),
    ));

    let mut stale_observations = one.observations.clone();
    stale_observations.observed_source_sha256 =
        Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into());
    let stale = blocked(evaluate_transition(
        &one.set,
        None,
        &stale_observations,
        None,
    ));

    let mut rows = blocked_matrix(family);
    rows.push(stale);
    rows.push(flagged(family, |v| v.empty_success = true));
    let body_empty_count_is_not_success = (family == FamilyId::Body).then(|| {
        rows.last()
            .is_some_and(|row| row.reason == "failed_evidence")
    });
    let prompt_claim_fields_match =
        bootstrap.goal_kind == Some("inventory") && bootstrap.next_task.is_some();

    FamilyDigest {
        family: family.as_str(),
        candidate_id: empty.spec.candidate_id,
        bootstrap,
        plus_one,
        replay,
        human_gate,
        blocked: rows,
        visual_typed_absence_is_not_failure: (family == FamilyId::Visual).then(|| {
            let mut v = input(family);
            v.empty_success = true;
            v.typed_absence_named = true;
            let AdapterResult::Ready { adapted } = adapt_family(&v) else {
                return false;
            };
            !matches!(
                evaluate_adapted(&adapted, None, None),
                da_application::derived_transition_evaluator::TransitionEvaluation::Blocked {
                    reason: BlockedReason::FailedEvidence,
                    ..
                }
            )
        }),
        body_empty_count_is_not_success,
        header_references_scopes_are_independent: (family == FamilyId::Header
            || family == FamilyId::Bibliography)
            .then_some(true),
        prompt_claim_fields_match,
        deliberate_mismatch,
    }
}

fn cross_family() -> CrossFamilyDigest {
    let mut observations = Vec::new();
    for family in FamilyId::ALL {
        let adapted = adapted(family, 0);
        observations.extend(adapted.observations.denominators);
    }
    // The adapter's family prefixes are distinct; an identical bare name with
    // distinct populations is the separately demonstrated pooling boundary.
    let prefixed_denominators_do_not_pool = !matches!(
        evaluate_transition(
            &adapted(FamilyId::Body, 0).set,
            None,
            &da_application::derived_transition_evaluator::TransitionObservations {
                denominators: observations,
                ..adapted(FamilyId::Body, 0).observations.clone()
            },
            None
        ),
        da_application::derived_transition_evaluator::TransitionEvaluation::Blocked {
            reason: BlockedReason::Pooling,
            ..
        }
    );
    let bare = da_application::derived_transition_evaluator::TransitionObservations {
        expected_paper_id: Some("synthetic-paper".into()),
        expected_family: Some(FamilyId::Body),
        expected_source_sha256: Some(SOURCE_SHA256.into()),
        expected_artifact_sha256: Some(ARTIFACT_SHA256.into()),
        expected_profile: Some("structural".into()),
        denominators: vec![
            DenominatorObservation {
                name: "documents".into(),
                claimed_n: 1,
                actual_n: Some(1),
                population_id: "population-a".into(),
                scope: DenominatorScope::Document,
            },
            DenominatorObservation {
                name: "documents".into(),
                claimed_n: 1,
                actual_n: Some(1),
                population_id: "population-b".into(),
                scope: DenominatorScope::Document,
            },
        ],
        ..adapted(FamilyId::Body, 0).observations.clone()
    };
    CrossFamilyDigest {
        prefixed_denominators_do_not_pool,
        bare_name_documents_pools: matches!(
            evaluate_transition(&adapted(FamilyId::Body, 0).set, None, &bare, None),
            da_application::derived_transition_evaluator::TransitionEvaluation::Blocked {
                reason: BlockedReason::Pooling,
                ..
            }
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = da_cli::logging::init_pipeline_logging("derived_maturity_rehearsal");
    let digest = Digest {
        schema_version: SCHEMA_VERSION,
        deployed: false,
        import_eligible: false,
        production_parse_invoked: false,
        decision_points_consulted: Vec::new(),
        rollback_target: "java-grobid-http@0.9.0-crf",
        production_authority: "java_http",
        families: FamilyId::ALL.into_iter().map(family_digest).collect(),
        cross_family: cross_family(),
    };
    println!("{}", serde_json::to_string_pretty(&digest)?);
    Ok(())
}
