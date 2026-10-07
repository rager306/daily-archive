//! S09/T04 (M514-1lss2s) — contract test for the process-local Java-disabled
//! smoke rehearsal.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it
//! rather than compiling an empty target. It binds the committed artifact,
//! re-derives every evidence sha256 from the one committed carrier table,
//! independently re-runs the T01 authorization gate over the committed S08 gate
//! ledger and human release record, asserts the live `not_authorized` state, and
//! rejects each named mutation. A synthetic in-memory authorization exercises the
//! authorized path and proves the zero Java-consult count is a measurement: the
//! positive control observes a synthetic consult through a real
//! [`JavaInvocationCounter`], and only then does `java_calls = 0` mean anything.
//!
//! The synthetic authorization is never written to a committed artifact.

use da_cli::native_family_receipt::{CarrierBinding, REQUIRED_LOCKS, sha256_prefixed};
use da_cli::native_java_disabled_smoke::{
    DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL, JAVA_DISABLED_SMOKE_AUTHORIZED,
    JAVA_DISABLED_SMOKE_NOT_AUTHORIZED, JAVA_DISABLED_SMOKE_REFUSAL_VOCABULARY,
    JavaDisabledSmokeExpectation, JavaDisabledSmokeInput, JavaDisabledSmokeReceipt,
    PositiveControlProvenance, SMOKE_EVIDENCE_CARRIERS, SMOKE_PRODUCT_COVERAGE_EFFECT,
    SMOKE_REFUSAL_NOT_AUTHORIZED, build_java_disabled_smoke_receipt, guard_from_counter,
    guard_not_installed, observe_positive_control, validate_java_disabled_smoke_receipt,
};
use da_parse::{
    HumanReleaseRecord, JavaInvocationCounter, MandatoryGate, NATIVE_ACCEPTANCE_CLAIM_SCOPE,
    NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION, NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT,
    NativeAcceptanceEvaluation, NativeRetirementAuthorization, PackageBinding,
    RETIREMENT_CLAIM_SCOPE_BLOCKED, RETIREMENT_PACKAGE_STATUS_AUTHORIZED,
    RetirementAuthorizationInput, authorize_native_retirement,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// The four required, non-clear gates the committed S08 ledger carries today.
///
/// M516-th0oby/S05 recorded the twelve unmeasurable `required_scope` families as
/// `required_family_downgrade`/`not_applicable` downgrades and shrank the
/// blocking set from sixteen to the four gates that still block.
const LIVE_BLOCKING_GATE_COUNT: usize = 4;
/// The fifteen `required_scope.families` the committed schema declares.
const LIVE_REQUIRED_FAMILY_COUNT: usize = 15;

const PACKAGE_REL: &str = "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
const SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";
const SYNTHETIC_PIN: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SYNTHETIC_FAMILY_PRESERVATION: &str = "parse.component.native_fulltext_composition_shadow";
const SYNTHETIC_FAMILY_HEADER: &str = "stage.header_composition";

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
    SMOKE_EVIDENCE_CARRIERS
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
    REQUIRED_LOCKS
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
    let schema: serde_yaml::Value = serde_yaml::from_slice(&read(root, SCHEMA_REL)?)?;
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
fn re_run_authorization(root: &Path) -> Result<NativeRetirementAuthorization, Box<dyn Error>> {
    let package_bytes = read(root, PACKAGE_REL)?;
    let package: serde_json::Value = serde_json::from_slice(&package_bytes)?;
    let gate_rows = package
        .get("mandatory_gates")
        .and_then(serde_json::Value::as_array)
        .ok_or("mandatory_gates")?;
    let gates: Vec<MandatoryGate> = gate_rows
        .iter()
        .map(map_gate)
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    let manifest: serde_yaml::Value = serde_yaml::from_slice(&read(
        root,
        da_cli::native_retirement_readiness::READINESS_MANIFEST_REL,
    )?)?;
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

    let package_pin = sha256_prefixed(&package_bytes);
    Ok(authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_owned(),
            declared_sha256: Some(package_pin.clone()),
            actual_sha256: Some(package_pin),
        },
        human_release,
        lifecycle_state_authorizes: false,
        locks: closed_locks(),
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

// ── synthetic fixtures (in-memory only; never written) ────────────────────

fn synthetic_gate(
    gate_id: &str,
    kind: &str,
    state: &str,
    family_id: Option<&str>,
) -> MandatoryGate {
    MandatoryGate {
        gate_id: gate_id.to_owned(),
        kind: kind.to_owned(),
        state: state.to_owned(),
        required: true,
        family_id: family_id.map(str::to_owned),
        metric_id: None,
        reason: None,
        evidence: "synthetic contract gate".to_owned(),
    }
}

fn synthetic_authorized() -> NativeRetirementAuthorization {
    authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_owned(),
            declared_sha256: Some(SYNTHETIC_PIN.to_owned()),
            actual_sha256: Some(SYNTHETIC_PIN.to_owned()),
        },
        human_release: HumanReleaseRecord {
            reviewer: Some("Jane Reviewer".to_owned()),
            decision: "approved".to_owned(),
            decision_record: Some("doc/review/HUMAN-REVIEW-TEMPLATE.md".to_owned()),
            prose_alone_authorizes_release: false,
            package_status: RETIREMENT_PACKAGE_STATUS_AUTHORIZED.to_owned(),
        },
        lifecycle_state_authorizes: false,
        locks: closed_locks(),
        required_families: vec![
            SYNTHETIC_FAMILY_PRESERVATION.to_owned(),
            SYNTHETIC_FAMILY_HEADER.to_owned(),
        ],
        evaluation: NativeAcceptanceEvaluation {
            schema_version: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_owned(),
            claim_scope: NATIVE_ACCEPTANCE_CLAIM_SCOPE.to_owned(),
            product_coverage_effect: NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT.to_owned(),
            candidate_source_path: "artifacts/parse_compat/native_corpus_acceptance.json"
                .to_owned(),
            candidate_series: None,
            threshold_policy_state: "frozen".to_owned(),
            frozen_reference_path: "artifacts/parse_compat/absorption_progress_reference_pins.json"
                .to_owned(),
            thresholds: Vec::new(),
            gates: vec![
                synthetic_gate(
                    "threshold.preservation_micro_occurrence",
                    "threshold",
                    "met",
                    Some(SYNTHETIC_FAMILY_PRESERVATION),
                ),
                synthetic_gate(
                    "required_scope.stage.header_composition",
                    "threshold_pending_family",
                    "met",
                    Some(SYNTHETIC_FAMILY_HEADER),
                ),
            ],
        },
    })
}

/// The authorized rehearsal with a real guard and a non-vacuous positive
/// control. Returns the receipt and the expectation it must satisfy.
fn authorized_fixture()
-> Result<(JavaDisabledSmokeReceipt, JavaDisabledSmokeExpectation), Box<dyn Error>> {
    let authorization = synthetic_authorized();
    if !authorization.s09_authorized {
        return Err("the synthetic authorization must authorize".into());
    }
    let selection = authorization
        .families
        .iter()
        .map(|family| family.family_id.clone())
        .collect::<Vec<_>>();
    let run_counter = JavaInvocationCounter::new();
    let guard = guard_from_counter(&run_counter, true);
    let control_counter = JavaInvocationCounter::new();
    let observed = observe_positive_control(&control_counter);
    if observed != 1 {
        return Err("the positive control must observe one consult".into());
    }
    let receipt = build_java_disabled_smoke_receipt(&JavaDisabledSmokeInput {
        evidence: Vec::new(),
        authorization: authorization.clone(),
        selection: selection.clone(),
        guard,
        positive_control: PositiveControlProvenance {
            attempted: 1,
            observed,
        },
    })?;
    let expectation = JavaDisabledSmokeExpectation {
        evidence: Vec::new(),
        required_family_ids: selection,
        authorization,
    };
    Ok((receipt, expectation))
}

fn expect_refusal(
    label: &str,
    receipt: &JavaDisabledSmokeReceipt,
    expectation: &JavaDisabledSmokeExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_java_disabled_smoke_receipt(receipt, expectation) {
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
fn smoke_receipt_is_canonical_and_records_the_live_refusal() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let evidence = expected_evidence(&root)?;
    assert_eq!(evidence.len(), 2, "the evidence carrier table drifted");
    assert_eq!(
        DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL,
        da_cli::native_retirement_readiness::READINESS_JAVA_DISABLED_SMOKE_REL,
        "the smoke artifact path drifted from the T02 readiness pointer"
    );

    let families = required_families(&root)?;
    assert_eq!(
        families.len(),
        LIVE_REQUIRED_FAMILY_COUNT,
        "the contract declares fifteen required families"
    );

    let rerun = re_run_authorization(&root)?;
    assert!(!rerun.s09_authorized);
    assert_eq!(rerun.blocking_gates.len(), LIVE_BLOCKING_GATE_COUNT);

    let expectation = JavaDisabledSmokeExpectation {
        evidence,
        required_family_ids: families.clone(),
        authorization: rerun,
    };

    let artifact_bytes = read(&root, DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL)?;
    let receipt = JavaDisabledSmokeReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_java_disabled_smoke_receipt(&receipt, &expectation)?;
    assert_eq!(
        receipt.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the live refusal is recorded verbatim ─────────────────────────────
    assert_eq!(receipt.verdict, JAVA_DISABLED_SMOKE_NOT_AUTHORIZED);
    assert_eq!(receipt.claim_scope, RETIREMENT_CLAIM_SCOPE_BLOCKED);
    assert_eq!(
        receipt.product_coverage_effect,
        SMOKE_PRODUCT_COVERAGE_EFFECT
    );
    assert!(!receipt.product_scope_confirmed);
    assert!(receipt.families_switched.is_empty());
    assert!(
        receipt
            .selection
            .iter()
            .all(|row| !row.switched && !row.java_disabled)
    );
    assert!(!receipt.guard.installed);
    assert_eq!(receipt.guard.observed_java_calls, 0);
    assert!(!receipt.guard.identity_isolated);
    assert!(!receipt.positive_control.non_vacuous);
    assert_eq!(receipt.positive_control.observed, 0);
    assert!(
        receipt
            .refusals
            .iter()
            .any(|refusal| refusal.code == SMOKE_REFUSAL_NOT_AUTHORIZED)
    );
    for refusal in &receipt.refusals {
        assert!(
            JAVA_DISABLED_SMOKE_REFUSAL_VOCABULARY.contains(&refusal.code.as_str()),
            "refusal `{}` is outside the closed vocabulary",
            refusal.code
        );
    }

    // ── the five locks are closed ─────────────────────────────────────────
    for (key, expected) in REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(key).map(String::as_str),
            Some(expected),
            "lock `{key}` drifted"
        );
    }

    // ── the per-family selection covers exactly the required families ─────
    assert_eq!(receipt.selection.len(), LIVE_REQUIRED_FAMILY_COUNT);
    let mut selection_ids: Vec<String> = receipt
        .selection
        .iter()
        .map(|row| row.family_id.clone())
        .collect();
    selection_ids.sort();
    let mut expected_ids = families.clone();
    expected_ids.sort();
    assert_eq!(selection_ids, expected_ids);
    // M515/S02: all three frozen thresholds resolve `met`, so the preservation
    // family and the two extraction families (reference segmentation, header
    // composition) are refused by the global human gate rather than by a family
    // gate. M516-th0oby/S05 then recorded a `required_family_downgrade`
    // (`not_applicable`) for the twelve remaining required families, so they are
    // non-blocking too. Every selected family is now refused by the global human
    // gate, and none may name a family blocking gate: the four blocking gates
    // carry no `family_id`.
    for row in &receipt.selection {
        assert!(
            row.blocking_gates.is_empty(),
            "family `{}` must be refused by the global human gate, not a family gate",
            row.family_id
        );
    }

    // ── named mutations are all refused ───────────────────────────────────
    let mut m = receipt.clone();
    m.product_scope_confirmed = true;
    expect_refusal(
        "product_scope_claimed",
        &m,
        &expectation,
        "ProductScopeClaimedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.families_switched.push(SYNTHETIC_FAMILY_HEADER.to_owned());
    expect_refusal(
        "family_switched_without_authorization",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.selection[0].switched = true;
    m.selection[0].java_disabled = true;
    expect_refusal(
        "selection_row_switched",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.guard.installed = true;
    expect_refusal(
        "guard_installed_while_refused",
        &m,
        &expectation,
        "ProductScopeClaimedWithoutAuthorization",
    )?;

    let mut m = receipt.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut m = receipt.clone();
    m.claim_scope = "authorized_retirement_release".to_owned();
    expect_refusal("claim_scope_drift", &m, &expectation, "ClaimScopeDrift")?;

    let mut m = receipt.clone();
    m.product_coverage_effect = "product".to_owned();
    expect_refusal(
        "product_coverage_claimed",
        &m,
        &expectation,
        "ProductCoverageClaimed",
    )?;

    let mut m = receipt.clone();
    m.refusals.clear();
    expect_refusal(
        "missing_not_authorized_refusal",
        &m,
        &expectation,
        "MissingTypedRefusal",
    )?;

    let mut m = receipt.clone();
    m.authorization.verdict = "authorized".to_owned();
    expect_refusal(
        "authorization_drift",
        &m,
        &expectation,
        "AuthorizationDrift",
    )?;

    let mut m = receipt.clone();
    m.selection[0]
        .blocking_gates
        .push("required_scope.rest.isalive.blocking".to_owned());
    expect_refusal(
        "selection_blocking_drift",
        &m,
        &expectation,
        "SelectionBlockingDrift",
    )?;

    let mut drifted = expectation.clone();
    drifted
        .required_family_ids
        .push("stage.absent_family".to_owned());
    expect_refusal(
        "dropped_family_against_expectation",
        &receipt,
        &drifted,
        "SelectionDropped",
    )?;

    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &receipt, &drifted, "EvidenceDrift")?;

    Ok(())
}

#[test]
fn authorized_fixture_proves_the_zero_is_a_measurement() -> Result<(), Box<dyn Error>> {
    let (receipt, expectation) = authorized_fixture()?;
    validate_java_disabled_smoke_receipt(&receipt, &expectation)?;

    assert_eq!(receipt.verdict, JAVA_DISABLED_SMOKE_AUTHORIZED);
    assert!(receipt.product_scope_confirmed);
    assert_eq!(
        receipt.product_coverage_effect,
        SMOKE_PRODUCT_COVERAGE_EFFECT
    );
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(
        receipt.guard.observed_java_calls, 0,
        "an authorized rehearsal must observe zero Java consults"
    );
    assert!(
        receipt.positive_control.non_vacuous,
        "the zero is only a measurement with a non-vacuous positive control"
    );
    assert_eq!(receipt.positive_control.observed, 1);
    assert_eq!(receipt.families_switched.len(), 2);
    assert!(receipt.refusals.is_empty());
    assert!(
        receipt
            .selection
            .iter()
            .all(|row| row.switched && row.java_disabled && row.authorized)
    );

    // ── mutations a builder cannot express are refused by the validator ───
    let mut m = receipt.clone();
    m.guard.observed_java_calls = 1;
    expect_refusal(
        "authorized_java_calls",
        &m,
        &expectation,
        "ObservedJavaOnDisabledFamilies",
    )?;

    let mut m = receipt.clone();
    m.guard.installed = false;
    expect_refusal(
        "authorized_guard_absent",
        &m,
        &expectation,
        "GuardNotInstalled",
    )?;

    let mut m = receipt.clone();
    m.guard.identity_isolated = false;
    expect_refusal(
        "authorized_guard_not_isolated",
        &m,
        &expectation,
        "GuardIdentityNotIsolated",
    )?;

    let mut m = receipt.clone();
    m.positive_control.observed = 0;
    expect_refusal(
        "authorized_vacuous_control",
        &m,
        &expectation,
        "PositiveControlNotObserved",
    )?;

    let mut m = receipt.clone();
    m.positive_control.non_vacuous = false;
    expect_refusal(
        "authorized_control_not_non_vacuous",
        &m,
        &expectation,
        "PositiveControlInconsistent",
    )?;

    let mut m = receipt.clone();
    m.product_scope_confirmed = false;
    expect_refusal(
        "authorized_scope_unconfirmed",
        &m,
        &expectation,
        "ProductScopeNotConfirmed",
    )?;

    let mut m = receipt.clone();
    m.families_switched.push("stage.absent_family".to_owned());
    expect_refusal(
        "switched_family_not_selected",
        &m,
        &expectation,
        "FamilyNotSelected",
    )?;

    let mut m = receipt.clone();
    let blocking = "required_scope.stage.header_composition.blocking".to_owned();
    m.authorization.families[0].blocking_gates = vec![blocking.clone()];
    m.selection[0].blocking_gates = vec![blocking];
    expect_refusal(
        "switched_family_with_blocking_gate",
        &m,
        &expectation,
        "FamilyGateBlocks",
    )?;

    let mut m = receipt.clone();
    m.selection[0].switched = false;
    m.selection[0].java_disabled = false;
    expect_refusal(
        "switched_family_row_not_switched",
        &m,
        &expectation,
        "FamilyNotSwitched",
    )?;

    Ok(())
}

#[test]
fn the_smoke_module_carries_no_runtime_side_channel() {
    let root = repo_root();
    let source = read_text(&root, "crates/da-cli/src/native_java_disabled_smoke.rs")
        .expect("the smoke module is readable");
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
            "the smoke module acquired a runtime side channel: `{forbidden}`"
        );
    }
    assert!(source.contains("pub fn build_java_disabled_smoke_receipt"));
    assert!(source.contains("pub fn validate_java_disabled_smoke_receipt"));
    assert!(source.contains("pub fn guard_from_counter"));
    assert!(source.contains("pub fn observe_positive_control"));
    assert!(source.contains("not_authorized"));
    assert!(source.contains("product_scope_confirmed"));
    // The default artifact path is the T02 readiness pointer, not a second one.
    assert_eq!(
        DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL,
        "artifacts/parse_compat/native_java_disabled_smoke.json"
    );
}

#[test]
fn the_smoke_refusal_vocabulary_is_closed_and_unique() -> Result<(), Box<dyn Error>> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for code in JAVA_DISABLED_SMOKE_REFUSAL_VOCABULARY {
        assert!(seen.insert(code), "duplicate refusal code `{code}`");
        assert!(!code.is_empty(), "refusal code must not be empty");
    }
    assert!(seen.contains(SMOKE_REFUSAL_NOT_AUTHORIZED));
    // The live receipt carries a typed refusal drawn from this closed set.
    let root = repo_root();
    let receipt = JavaDisabledSmokeReceipt::parse_json_bytes(&read(
        &root,
        DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL,
    )?)
    .expect("the committed artifact parses");
    assert!(
        receipt
            .refusals
            .iter()
            .all(|refusal| seen.contains(refusal.code.as_str()))
    );
    Ok(())
}

#[test]
fn the_live_smoke_is_not_a_runtime_route() {
    // The smoke is process-local: it must not appear in the parser port or any
    // `da` subcommand registration. This is a structural guard, not a comment.
    let root = repo_root();
    for (rel, needle) in [
        ("crates/da-cli/src/source_cli.rs", "java_disabled_smoke"),
        ("crates/da-cli/src/main.rs", "java_disabled_smoke"),
    ] {
        if let Ok(text) = read_text(&root, rel) {
            assert!(
                !text.contains(needle),
                "the smoke harness leaked into `{rel}` and became a runtime route"
            );
        }
    }
    // GuardProvenance is the route receipt vocabulary, reused unchanged.
    let provenance = guard_not_installed();
    assert!(!provenance.installed);
    let counter = JavaInvocationCounter::new();
    let installed = guard_from_counter(&counter, true);
    assert!(installed.installed);
    assert!(installed.identity_isolated);
    assert_eq!(installed.observed_java_calls, 0);
}
