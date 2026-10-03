//! S09/T02 (M514-1lss2s) — contract test for the canonical blocked
//! retirement-readiness receipt.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it
//! rather than compiling an empty target. It re-derives every evidence sha256
//! from the one committed carrier table, independently re-runs the T01
//! authorization gate over the committed S08 gate ledger and human release
//! record, binds the committed artifact, asserts its canonical byte
//! re-serialization, and rejects each named mutation.

use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_retirement_readiness::{
    DEFAULT_RETIREMENT_READINESS_ARTIFACT_REL, POINTER_SCHEMA_REQUIRED_SCOPE,
    READINESS_ACCEPTANCE_PACKAGE_REL, READINESS_BODY_RESIDUAL_TOKEN, READINESS_CLAIM_SCOPE_BLOCKED,
    READINESS_EVIDENCE_CARRIERS, READINESS_JAVA_DISABLED_SMOKE_REL,
    READINESS_M513_RELEASE_PACKAGE_REL, READINESS_MANIFEST_REL, READINESS_PRODUCT_COVERAGE_EFFECT,
    READINESS_SCHEMA_REL, RetirementReadinessError, RetirementReadinessExpectation,
    RetirementReadinessReceipt, validate_retirement_readiness_receipt,
};
use da_parse::{
    HumanReleaseRecord, MandatoryGate, NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID,
    NATIVE_ACCEPTANCE_CLAIM_SCOPE, NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION,
    NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT, NativeAcceptanceEvaluation, PackageBinding,
    RETIREMENT_CLAIM_SCOPE_BLOCKED, RETIREMENT_REQUIRED_LOCKS, RetirementAuthorizationInput,
    authorize_native_retirement,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// The sixteen required, non-clear gates the committed S08 ledger carries today.
const LIVE_BLOCKING_GATE_COUNT: usize = 16;
/// The fifteen `required_scope.families` the committed schema declares.
const LIVE_REQUIRED_FAMILY_COUNT: usize = 15;

// ───────────────────────────── helpers ─────────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

fn read_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(read(root, rel)?)?)
}

fn expected_evidence(root: &Path) -> Result<Vec<CarrierBinding>, Box<dyn Error>> {
    READINESS_EVIDENCE_CARRIERS
        .iter()
        .map(|(rel, pointer)| {
            Ok(CarrierBinding {
                manifest: (*rel).to_owned(),
                pointer: (*pointer).to_owned(),
                revision_sha256: sha256_prefixed(&read(root, rel)?),
            })
        })
        .collect()
}

fn closed_locks() -> BTreeMap<String, String> {
    RETIREMENT_REQUIRED_LOCKS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn map_gate(value: &serde_json::Value) -> Result<MandatoryGate, Box<dyn Error>> {
    let field = |name: &str| -> Result<String, Box<dyn Error>> {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("mandatory gate is missing `{name}`").into())
    };
    Ok(MandatoryGate {
        gate_id: field("gate_id")?,
        kind: field("kind")?,
        state: field("state")?,
        required: value
            .get("required")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        family_id: value
            .get("family_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        metric_id: value
            .get("metric_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        evidence: value
            .get("evidence_needed")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

fn required_families(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let schema: serde_yaml::Value = serde_yaml::from_slice(&read(root, READINESS_SCHEMA_REL)?)?;
    Ok(schema
        .get("required_scope")
        .and_then(|scope| scope.get("families"))
        .and_then(|families| families.as_sequence())
        .ok_or("required_scope.families")?
        .iter()
        .filter_map(|row| {
            row.get("family_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect())
}

/// Independently re-run the T01 authorization gate over the committed carriers.
/// This is what makes the carried verdict a re-derivation, not a restated claim.
fn re_run_authorization(
    root: &Path,
) -> Result<da_parse::NativeRetirementAuthorization, Box<dyn Error>> {
    let package_bytes = read(root, READINESS_ACCEPTANCE_PACKAGE_REL)?;
    let package: serde_json::Value = serde_json::from_slice(&package_bytes)?;
    let gate_rows = package
        .get("mandatory_gates")
        .and_then(serde_json::Value::as_array)
        .ok_or("mandatory_gates")?;
    let gates: Vec<MandatoryGate> = gate_rows
        .iter()
        .map(map_gate)
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    let manifest: serde_yaml::Value = serde_yaml::from_slice(&read(root, READINESS_MANIFEST_REL)?)?;
    let release = manifest
        .get("release_decision_package")
        .ok_or("release_decision_package")?;
    let review = release
        .get("human_release_review")
        .ok_or("human_release_review")?;
    let human_release = HumanReleaseRecord {
        reviewer: review
            .get("reviewer")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
        decision: review
            .get("decision")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned(),
        decision_record: review
            .get("decision_record")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
        prose_alone_authorizes_release: review
            .get("prose_alone_authorizes_release")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        package_status: release
            .get("package_status")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned(),
    };

    let locks: BTreeMap<String, String> = closed_locks();

    let package_pin = sha256_prefixed(&package_bytes);
    Ok(authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: READINESS_ACCEPTANCE_PACKAGE_REL.to_owned(),
            declared_sha256: Some(package_pin.clone()),
            actual_sha256: Some(package_pin),
        },
        human_release,
        lifecycle_state_authorizes: false,
        locks,
        required_families: required_families(root)?,
        evaluation: NativeAcceptanceEvaluation {
            schema_version: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_owned(),
            claim_scope: NATIVE_ACCEPTANCE_CLAIM_SCOPE.to_owned(),
            product_coverage_effect: NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT.to_owned(),
            candidate_source_path: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("candidate_source_path"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            candidate_series: None,
            threshold_policy_state: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("threshold_policy_state"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            frozen_reference_path: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("frozen_reference_path"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            thresholds: Vec::new(),
            gates,
        },
    }))
}

fn expect_refusal(
    label: &str,
    receipt: &RetirementReadinessReceipt,
    expectation: &RetirementReadinessExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_retirement_readiness_receipt(receipt, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

// ───────────────────────────── tests ─────────────────────────────

#[test]
fn readiness_receipt_is_canonical_and_records_the_blocked_state() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let evidence = expected_evidence(&root)?;
    assert_eq!(evidence.len(), 5, "the evidence carrier table drifted");

    let families = required_families(&root)?;
    assert_eq!(
        families.len(),
        LIVE_REQUIRED_FAMILY_COUNT,
        "the contract declares fifteen required families"
    );

    let expectation = RetirementReadinessExpectation {
        evidence: evidence.clone(),
        required_family_ids: families.clone(),
        body_residual_token: READINESS_BODY_RESIDUAL_TOKEN.to_owned(),
    };

    let artifact_bytes = read(&root, DEFAULT_RETIREMENT_READINESS_ARTIFACT_REL)?;
    let receipt = RetirementReadinessReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_retirement_readiness_receipt(&receipt, &expectation)?;
    assert_eq!(
        receipt.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the blocked state is recorded verbatim ────────────────────────────
    assert_eq!(receipt.claim_scope, "blocked_pending_human_authorization");
    assert_eq!(
        receipt.claim_scope, READINESS_CLAIM_SCOPE_BLOCKED,
        "the claim scope must equal the T01 blocked token"
    );
    assert_eq!(receipt.product_coverage_effect, "none");
    assert_eq!(
        receipt.product_coverage_effect,
        READINESS_PRODUCT_COVERAGE_EFFECT
    );
    assert!(receipt.goal_unmet, "the retirement goal stays unmet");
    assert!(
        receipt.families_switched.is_empty(),
        "no family may be switched while refused"
    );

    // ── the five locks are closed ─────────────────────────────────────────
    for (key, expected) in RETIREMENT_REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(key).map(String::as_str),
            Some(expected),
            "lock `{key}` drifted"
        );
    }

    // ── the human-gate invariance ─────────────────────────────────────────
    assert!(!receipt.human_gate.reviewer_named);
    assert_eq!(receipt.human_gate.decision, "pending");
    assert!(!receipt.human_gate.release_recorded);
    assert_eq!(receipt.human_gate.package_status, "pending_human_review");
    assert_eq!(
        receipt.human_gate.m513_package_status,
        "pending_human_review"
    );
    assert_eq!(
        receipt.human_gate.decision_record,
        "doc/review/HUMAN-REVIEW-TEMPLATE.md"
    );

    // ── the body residual survives verbatim ───────────────────────────────
    assert_eq!(receipt.body_residual.token, "p1_localization_uncertain");
    assert_eq!(
        receipt.body_residual.gate_id,
        NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID
    );
    assert_eq!(
        receipt.body_residual.evidence.manifest,
        READINESS_ACCEPTANCE_PACKAGE_REL
    );

    // ── the required action names the decision and the smoke ──────────────
    assert_eq!(
        receipt.next_required_action.human_release_decision,
        "named_human_release_decision_required"
    );
    assert_eq!(
        receipt.next_required_action.java_disabled_smoke_receipt,
        READINESS_JAVA_DISABLED_SMOKE_REL
    );
    assert!(!receipt.next_required_action.steps.is_empty());

    // ── the carried verdict is the gate's own re-derivation ───────────────
    let rerun = re_run_authorization(&root)?;
    assert_eq!(receipt.authorization.verdict, rerun.verdict);
    assert_eq!(receipt.authorization.s09_authorized, rerun.s09_authorized);
    assert_eq!(receipt.authorization.basis, rerun.basis);
    assert_eq!(receipt.authorization.claim_scope, rerun.claim_scope);
    assert_eq!(
        receipt.authorization.blocking_gates, rerun.blocking_gates,
        "the carried blocking set drifted from a fresh authorization"
    );
    assert!(!receipt.authorization.s09_authorized);
    assert_eq!(
        receipt.authorization.claim_scope,
        RETIREMENT_CLAIM_SCOPE_BLOCKED
    );
    assert_eq!(receipt.authorization.product_coverage_effect, "none");
    assert_eq!(
        receipt.authorization.blocking_gates.len(),
        LIVE_BLOCKING_GATE_COUNT
    );
    assert!(!receipt.authorization.human_reviewer_named);
    assert!(!receipt.authorization.human_decision_recorded);

    // ── the per-family plan covers exactly the required families ──────────
    assert_eq!(receipt.family_plan.len(), LIVE_REQUIRED_FAMILY_COUNT);
    let mut plan_ids: Vec<String> = receipt
        .family_plan
        .iter()
        .map(|row| row.family_id.clone())
        .collect();
    plan_ids.sort();
    let mut expected_ids = families.clone();
    expected_ids.sort();
    assert_eq!(plan_ids, expected_ids);
    assert!(
        receipt
            .family_plan
            .iter()
            .all(|row| !row.in_plan && !row.switched)
    );
    // M515/S02: all three frozen thresholds now resolve `met`, so the three
    // families they bind — preservation (native fulltext composition), extraction
    // micro (reference segmentation) and extraction weakest stratum (header
    // composition) — are refused by the global human gate rather than by a family
    // gate. Every other planned family still names its gate(s).
    let met_families = [
        "parse.component.native_fulltext_composition_shadow",
        "stage.reference_segmentation",
        "stage.header_composition",
    ];
    for row in &receipt.family_plan {
        if met_families.contains(&row.family_id.as_str()) {
            assert!(
                row.blocking_gates.is_empty(),
                "the met family `{}` must not name a blocking gate",
                row.family_id
            );
        } else {
            assert!(
                !row.blocking_gates.is_empty(),
                "every other planned family must name the gate(s) blocking it"
            );
        }
    }

    // ── the frozen M513 package still awaits the human decision ───────────
    let m513: serde_json::Value =
        serde_json::from_slice(&read(&root, READINESS_M513_RELEASE_PACKAGE_REL)?)?;
    assert_eq!(
        m513.get("package_status")
            .and_then(serde_json::Value::as_str),
        Some("pending_human_review")
    );

    // ── named mutations are all refused ───────────────────────────────────
    let mut m = receipt.clone();
    m.family_plan.clear();
    expect_refusal("dropped_family", &m, &expectation, "FamilyPlanDropped")?;

    let mut m = receipt.clone();
    m.families_switched
        .push("stage.header_composition".to_owned());
    expect_refusal(
        "family_switched_without_authorization",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.family_plan[0].switched = true;
    expect_refusal(
        "plan_row_switched",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.family_plan[0].in_plan = true;
    expect_refusal(
        "in_plan_while_refused",
        &m,
        &expectation,
        "InPlanWhileRefused",
    )?;

    let mut m = receipt.clone();
    m.family_plan[0].blocking_gates.clear();
    expect_refusal(
        "plan_blocking_drift",
        &m,
        &expectation,
        "FamilyPlanBlockingDrift",
    )?;

    let mut m = receipt.clone();
    m.goal_unmet = false;
    expect_refusal("goal_marked_met", &m, &expectation, "GoalClaimedMet")?;

    let mut m = receipt.clone();
    m.body_residual.token = "p1_localization_unknown".to_owned();
    expect_refusal(
        "renamed_body_residual",
        &m,
        &expectation,
        "BodyResidualRenamed",
    )?;

    let mut m = receipt.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut m = receipt.clone();
    m.product_coverage_effect = "product".to_owned();
    expect_refusal(
        "product_coverage_claim",
        &m,
        &expectation,
        "ProductCoverageClaimed",
    )?;

    let mut m = receipt.clone();
    m.claim_scope = "diagnostic_only".to_owned();
    expect_refusal("claim_scope_drift", &m, &expectation, "ClaimScopeDrift")?;

    let mut m = receipt.clone();
    m.human_gate.reviewer_named = true;
    expect_refusal(
        "human_record_changed",
        &m,
        &expectation,
        "HumanReleaseRecordDrift",
    )?;

    let mut m = receipt.clone();
    m.human_gate.decision = "recorded".to_owned();
    expect_refusal(
        "human_decision_claimed",
        &m,
        &expectation,
        "HumanReleaseRecordDrift",
    )?;

    let mut m = receipt.clone();
    m.human_gate.m513_package_status = "human_release_recorded".to_owned();
    expect_refusal(
        "m513_status_drift",
        &m,
        &expectation,
        "M513PackageStatusDrift",
    )?;

    let mut m = receipt.clone();
    m.authorization.s09_authorized = true;
    expect_refusal(
        "authorized_verdict_recorded_as_blocked",
        &m,
        &expectation,
        "AuthorizationNotRefused",
    )?;

    let mut m = receipt.clone();
    m.authorization.lifecycle_state_authorizes = true;
    expect_refusal(
        "lifecycle_authorization",
        &m,
        &expectation,
        "LifecycleAuthorization",
    )?;

    let mut m = receipt.clone();
    m.next_required_action.java_disabled_smoke_receipt = "elsewhere.json".to_owned();
    expect_refusal("next_action_drift", &m, &expectation, "NextActionDrift")?;

    let mut m = receipt.clone();
    m.authorization.package.actual_sha256 = Some(sha256_prefixed(b"drifted"));
    expect_refusal(
        "authorization_package_drift",
        &m,
        &expectation,
        "AuthorizationPackageDrift",
    )?;

    let mut drifted = expectation.clone();
    drifted
        .required_family_ids
        .push("stage.absent_family".to_owned());
    expect_refusal(
        "dropped_family_against_expectation",
        &receipt,
        &drifted,
        "FamilyPlanDropped",
    )?;

    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &receipt, &drifted, "EvidenceDrift")?;

    // A redaction leak must be refused by the serializer itself.
    let mut m = receipt.clone();
    m.next_required_action.steps = vec!["http://leak".to_owned()];
    assert!(
        matches!(
            m.artifact_json_bytes(),
            Err(RetirementReadinessError::RedactionLeak { .. })
        ),
        "a payload leak was not refused"
    );

    Ok(())
}

#[test]
fn the_readiness_module_carries_no_runtime_side_channel() {
    let root = repo_root();
    let source = read_text(&root, "crates/da-cli/src/native_retirement_readiness.rs")
        .expect("the readiness module is readable");
    for forbidden in [
        "std::fs",
        "std::env",
        "std::net",
        "std::process",
        "tracing::",
        "tokio::",
        "reqwest",
        "serde_yaml",
        "evaluate_native_acceptance(",
    ] {
        assert!(
            !source.contains(forbidden),
            "the readiness module acquired a runtime side channel: `{forbidden}`"
        );
    }
    assert!(source.contains("pub fn build_retirement_readiness_receipt"));
    assert!(source.contains("pub fn validate_retirement_readiness_receipt"));
    assert!(source.contains("p1_localization_uncertain"));
    assert!(
        source.contains("blocked_pending_human_authorization")
            || source.contains("RETIREMENT_CLAIM_SCOPE_BLOCKED")
    );
    // The pointer into the schema's required scope is the one the README names.
    assert_eq!(POINTER_SCHEMA_REQUIRED_SCOPE, "required_scope.families");
}
