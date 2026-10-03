//! S09/T02 (M514-1lss2s) — canonical blocked retirement-readiness receipt runner.
//!
//! This is the executable half of the T02 readiness contract. It reads the
//! committed carriers, re-runs the T01 authorization gate over the committed S08
//! gate ledger and the committed human release record, and writes
//! `artifacts/parse_compat/grobid_retirement_readiness.json` atomically (tmp +
//! rename). It binds, by repository-relative path + pointer + sha256:
//!
//! * the S08 acceptance package (`mandatory_gates`);
//! * the human release record inside `data/parse/acceptance_manifest.yaml`;
//! * the S02 `required_scope.families` and `authorization` blocks;
//! * the frozen M513 release-decision package (`remainders`).
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No carrier is read and nothing is written.
//!   Rejects each named mutation (dropped family, switched family, goal marked
//!   met, renamed body residual, opened lock, product-coverage claim, changed
//!   human record, drifted claim scope, an authorized verdict recorded as
//!   blocked, a lifecycle authorization, a drifted next action, a non-pending
//!   M513 package).
//! * default — canonical run over the committed carriers.
//!
//! Fail closed: `claim_scope = blocked_pending_human_authorization`,
//! `product_coverage_effect = none`, `goal_unmet = true`,
//! `families_switched = []`, the five locks closed, and no family in the plan.
//! Nothing here switches a family, writes a graph or claims product coverage.

use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_retirement_readiness::{
    BodyResidualBinding, DEFAULT_RETIREMENT_READINESS_ARTIFACT_REL, FamilyPlanRow,
    HumanGateInvariance, NextRequiredAction, READINESS_ACCEPTANCE_PACKAGE_REL,
    READINESS_BODY_RESIDUAL_TOKEN, READINESS_EVIDENCE_CARRIERS, READINESS_HUMAN_DECISION_ACTION,
    READINESS_HUMAN_DECISION_PENDING, READINESS_JAVA_DISABLED_SMOKE_ACTION,
    READINESS_JAVA_DISABLED_SMOKE_REL, READINESS_M513_PACKAGE_STATUS_PENDING,
    READINESS_M513_RELEASE_PACKAGE_REL, READINESS_MANIFEST_REL, READINESS_PACKAGE_STATUS_PENDING,
    READINESS_SCHEMA_REL, RetirementReadinessError, RetirementReadinessExpectation,
    RetirementReadinessInput, RetirementReadinessReceipt, build_retirement_readiness_receipt,
    validate_retirement_readiness_receipt,
};
use da_parse::{
    HumanReleaseRecord, MandatoryGate, NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID,
    NATIVE_ACCEPTANCE_CLAIM_SCOPE, NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION,
    NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT, NativeAcceptanceEvaluation, PackageBinding,
    RETIREMENT_REQUIRED_LOCKS, RetirementAuthorizationInput, authorize_native_retirement,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// A well-formed `sha256:` pin used only by the hermetic self-check.
const SYNTHETIC_PIN: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// The synthetic contract family used only by the hermetic self-check.
const SYNTHETIC_FAMILY: &str = "stage.header_composition";

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_RETIREMENT_READINESS_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!("native_retirement_readiness [--root DIR] [--report FILE] [--self-check]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args {
        root,
        report,
        self_check,
    }
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_retirement_readiness");
    let args = parse_args();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_retirement_readiness: {error}");
            ExitCode::FAILURE
        }
    }
}

// ─────────────────────────────── helpers ───────────────────────────────

fn read_bytes(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

/// Build the evidence list from the one shared carrier table.
fn evidence_from_root(root: &Path) -> Result<Vec<CarrierBinding>, Box<dyn Error>> {
    READINESS_EVIDENCE_CARRIERS
        .iter()
        .map(|(rel, pointer)| {
            let bytes = read_bytes(root, rel)?;
            Ok(CarrierBinding {
                manifest: (*rel).to_owned(),
                pointer: (*pointer).to_owned(),
                revision_sha256: sha256_prefixed(&bytes),
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

fn rendered_locks(package: &serde_json::Value) -> BTreeMap<String, String> {
    package
        .get("locks")
        .and_then(serde_json::Value::as_object)
        .map(|locks| {
            locks
                .iter()
                .map(|(key, value)| {
                    let rendered = match value {
                        serde_json::Value::String(value) => value.clone(),
                        other => other.to_string(),
                    };
                    (key.clone(), rendered)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ───────────────────────────── full run ─────────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }
    let root = args.root.as_path();
    let evidence = evidence_from_root(root)?;

    // ── the S08 gate ledger ────────────────────────────────────────────────
    let package_bytes = read_bytes(root, READINESS_ACCEPTANCE_PACKAGE_REL)?;
    let package: serde_json::Value = serde_json::from_slice(&package_bytes)?;
    let gate_rows = package
        .get("mandatory_gates")
        .and_then(serde_json::Value::as_array)
        .ok_or("the S08 package carries no mandatory_gates")?;
    let gates: Vec<MandatoryGate> = gate_rows
        .iter()
        .map(map_gate)
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    // ── the S02 required scope, read by pointer ────────────────────────────
    let schema_bytes = read_bytes(root, READINESS_SCHEMA_REL)?;
    let schema: serde_yaml::Value = serde_yaml::from_slice(&schema_bytes)?;
    let required_families: Vec<String> = schema
        .get("required_scope")
        .and_then(|scope| scope.get("families"))
        .and_then(|families| families.as_sequence())
        .ok_or("the schema carries no required_scope.families")?
        .iter()
        .filter_map(|row| {
            row.get("family_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect();

    // ── the human release record, read by pointer ──────────────────────────
    let manifest_bytes = read_bytes(root, READINESS_MANIFEST_REL)?;
    let manifest: serde_yaml::Value = serde_yaml::from_slice(&manifest_bytes)?;
    let release = manifest
        .get("release_decision_package")
        .ok_or("the manifest carries no release_decision_package")?;
    let review = release
        .get("human_release_review")
        .ok_or("the manifest carries no human_release_review")?;
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

    // ── the frozen M513 release package ────────────────────────────────────
    let m513_bytes = read_bytes(root, READINESS_M513_RELEASE_PACKAGE_REL)?;
    let m513: serde_json::Value = serde_json::from_slice(&m513_bytes)?;
    let m513_package_status = m513
        .get("package_status")
        .and_then(serde_json::Value::as_str)
        .ok_or("the M513 package carries no package_status")?
        .to_owned();

    // ── the T01 authorization verdict, computed now ────────────────────────
    let package_pin = sha256_prefixed(&package_bytes);
    let authorization = authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: READINESS_ACCEPTANCE_PACKAGE_REL.to_owned(),
            declared_sha256: Some(package_pin.clone()),
            actual_sha256: Some(package_pin.clone()),
        },
        human_release: human_release.clone(),
        lifecycle_state_authorizes: false,
        locks: rendered_locks(&package),
        required_families,
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
    });

    // ── the body-axis residual stays named ─────────────────────────────────
    let body_gate = gate_rows
        .iter()
        .find(|row| {
            row.get("gate_id").and_then(serde_json::Value::as_str)
                == Some(NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID)
        })
        .ok_or("the S08 package carries no body-axis gate")?;
    let body_reason = body_gate
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !body_reason
        .split(',')
        .any(|part| part.trim() == READINESS_BODY_RESIDUAL_TOKEN)
    {
        return Err(format!(
            "the body-axis gate lost the named `{READINESS_BODY_RESIDUAL_TOKEN}` residual"
        )
        .into());
    }
    let acceptance_carrier = evidence
        .iter()
        .find(|binding| binding.manifest == READINESS_ACCEPTANCE_PACKAGE_REL)
        .ok_or("the acceptance-package carrier is not bound")?
        .clone();
    let body_residual = BodyResidualBinding {
        token: READINESS_BODY_RESIDUAL_TOKEN.to_owned(),
        gate_id: NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID.to_owned(),
        evidence: acceptance_carrier,
    };

    // ── the human-gate invariance, as read from the committed carriers ─────
    let human_gate = HumanGateInvariance {
        reviewer_named: human_release
            .reviewer
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        decision: human_release.decision.clone(),
        decision_record: human_release.decision_record.clone().unwrap_or_default(),
        package_status: human_release.package_status.clone(),
        m513_package_status: m513_package_status.clone(),
        release_recorded: authorization.human_decision_recorded,
    };

    let next_required_action = NextRequiredAction {
        human_release_decision: READINESS_HUMAN_DECISION_ACTION.to_owned(),
        decision_record: human_gate.decision_record.clone(),
        java_disabled_smoke: READINESS_JAVA_DISABLED_SMOKE_ACTION.to_owned(),
        java_disabled_smoke_receipt: READINESS_JAVA_DISABLED_SMOKE_REL.to_owned(),
        steps: vec![
            "record a named human release decision against doc/review/HUMAN-REVIEW-TEMPLATE.md and \
             fill release_decision_package.human_release_review"
                .to_owned(),
            "rerun the T01 retirement authorization gate; only a recorded decision with every \
             required family gate met or accepted_residual authorizes a switch"
                .to_owned(),
            "run the Java-disabled smoke for the authorized families and publish \
             artifacts/parse_compat/native_java_disabled_smoke.json"
                .to_owned(),
        ],
    };

    let expectation = RetirementReadinessExpectation {
        evidence: evidence.clone(),
        required_family_ids: authorization
            .families
            .iter()
            .map(|family| family.family_id.clone())
            .collect(),
        body_residual_token: READINESS_BODY_RESIDUAL_TOKEN.to_owned(),
    };

    let receipt = build_retirement_readiness_receipt(&RetirementReadinessInput {
        evidence,
        authorization,
        human_gate,
        body_residual,
        next_required_action,
    })?;
    validate_retirement_readiness_receipt(&receipt, &expectation)?;

    let bytes = receipt.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "retirement readiness `{}`: claim_scope={} product_coverage_effect={} goal_unmet={} \
         families_switched={} blocking_gates={} required_families={} authorization={}/{} \
         human_reviewer_named={} human_decision_recorded={}; canonical run",
        receipt.schema_version,
        receipt.claim_scope,
        receipt.product_coverage_effect,
        receipt.goal_unmet,
        receipt.families_switched.len(),
        receipt.authorization.blocking_gates.len(),
        receipt.family_plan.len(),
        receipt.authorization.verdict,
        receipt.authorization.basis,
        receipt.authorization.human_reviewer_named,
        receipt.authorization.human_decision_recorded,
    ))
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ──────────────────────────────── self-check ────────────────────────────────

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

/// Build a hermetic, refused input plus the expectation it must satisfy.
fn synthetic_fixtures()
-> Result<(RetirementReadinessInput, RetirementReadinessExpectation), Box<dyn Error>> {
    let evidence = vec![CarrierBinding {
        manifest: READINESS_ACCEPTANCE_PACKAGE_REL.to_owned(),
        pointer: "mandatory_gates".to_owned(),
        revision_sha256: SYNTHETIC_PIN.to_owned(),
    }];
    let authorization = authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: READINESS_ACCEPTANCE_PACKAGE_REL.to_owned(),
            declared_sha256: Some(SYNTHETIC_PIN.to_owned()),
            actual_sha256: Some(SYNTHETIC_PIN.to_owned()),
        },
        human_release: HumanReleaseRecord {
            reviewer: None,
            decision: READINESS_HUMAN_DECISION_PENDING.to_owned(),
            decision_record: Some("doc/review/HUMAN-REVIEW-TEMPLATE.md".to_owned()),
            prose_alone_authorizes_release: false,
            package_status: READINESS_PACKAGE_STATUS_PENDING.to_owned(),
        },
        lifecycle_state_authorizes: false,
        locks: closed_locks(),
        required_families: vec![SYNTHETIC_FAMILY.to_owned()],
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
                    "required_scope.stage.header_composition",
                    "threshold_pending_family",
                    "threshold_pending",
                    Some(SYNTHETIC_FAMILY),
                ),
                synthetic_gate(
                    "h20_frozen_external_holdout_decision",
                    "human_release",
                    "pending",
                    None,
                ),
            ],
        },
    });
    if authorization.s09_authorized {
        return Err("the synthetic authorization must refuse".into());
    }
    let input = RetirementReadinessInput {
        evidence: evidence.clone(),
        authorization,
        human_gate: HumanGateInvariance {
            reviewer_named: false,
            decision: READINESS_HUMAN_DECISION_PENDING.to_owned(),
            decision_record: "doc/review/HUMAN-REVIEW-TEMPLATE.md".to_owned(),
            package_status: READINESS_PACKAGE_STATUS_PENDING.to_owned(),
            m513_package_status: READINESS_M513_PACKAGE_STATUS_PENDING.to_owned(),
            release_recorded: false,
        },
        body_residual: BodyResidualBinding {
            token: READINESS_BODY_RESIDUAL_TOKEN.to_owned(),
            gate_id: NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID.to_owned(),
            evidence: evidence[0].clone(),
        },
        next_required_action: NextRequiredAction {
            human_release_decision: READINESS_HUMAN_DECISION_ACTION.to_owned(),
            decision_record: "doc/review/HUMAN-REVIEW-TEMPLATE.md".to_owned(),
            java_disabled_smoke: READINESS_JAVA_DISABLED_SMOKE_ACTION.to_owned(),
            java_disabled_smoke_receipt: READINESS_JAVA_DISABLED_SMOKE_REL.to_owned(),
            steps: vec!["record the human release decision".to_owned()],
        },
    };
    let expectation = RetirementReadinessExpectation {
        evidence,
        required_family_ids: vec![SYNTHETIC_FAMILY.to_owned()],
        body_residual_token: READINESS_BODY_RESIDUAL_TOKEN.to_owned(),
    };
    Ok((input, expectation))
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

/// Deterministic negative controls, all hermetic (no carrier, no write). Each
/// mutation must be refused **by name**; any mutation that slips through fails.
fn run_self_check() -> Result<String, Box<dyn Error>> {
    let (input, expectation) = synthetic_fixtures()?;
    let base = build_retirement_readiness_receipt(&input)?;
    validate_retirement_readiness_receipt(&base, &expectation)?;

    // The positive invariants of the blocked receipt.
    assert!(
        base.goal_unmet,
        "the blocked receipt must record the unmet goal"
    );
    assert!(
        base.families_switched.is_empty(),
        "no family may be switched"
    );
    assert_eq!(base.claim_scope, "blocked_pending_human_authorization");
    assert_eq!(base.product_coverage_effect, "none");
    assert!(!base.authorization.s09_authorized);
    assert!(
        base.family_plan
            .iter()
            .all(|row| !row.in_plan && !row.switched)
    );

    let mut checked = 0usize;

    let mut m = base.clone();
    m.family_plan.clear();
    expect_refusal("dropped_family", &m, &expectation, "FamilyPlanDropped")?;
    checked += 1;

    let mut m = base.clone();
    m.families_switched.push(SYNTHETIC_FAMILY.to_owned());
    expect_refusal(
        "family_switched_without_authorization",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.family_plan[0].switched = true;
    expect_refusal(
        "plan_row_switched",
        &m,
        &expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.family_plan[0].in_plan = true;
    expect_refusal(
        "in_plan_while_refused",
        &m,
        &expectation,
        "InPlanWhileRefused",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.family_plan[0].blocking_gates.clear();
    expect_refusal(
        "plan_blocking_drift",
        &m,
        &expectation,
        "FamilyPlanBlockingDrift",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.goal_unmet = false;
    expect_refusal("goal_marked_met", &m, &expectation, "GoalClaimedMet")?;
    checked += 1;

    let mut m = base.clone();
    m.body_residual.token = "p1_localization_unknown".to_owned();
    expect_refusal(
        "renamed_body_residual",
        &m,
        &expectation,
        "BodyResidualRenamed",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;
    checked += 1;

    let mut m = base.clone();
    m.product_coverage_effect = "product".to_owned();
    expect_refusal(
        "product_coverage_claim",
        &m,
        &expectation,
        "ProductCoverageClaimed",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.claim_scope = "diagnostic_only".to_owned();
    expect_refusal("claim_scope_drift", &m, &expectation, "ClaimScopeDrift")?;
    checked += 1;

    let mut m = base.clone();
    m.human_gate.reviewer_named = true;
    expect_refusal(
        "human_record_changed",
        &m,
        &expectation,
        "HumanReleaseRecordDrift",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.human_gate.decision = "recorded".to_owned();
    expect_refusal(
        "human_decision_claimed",
        &m,
        &expectation,
        "HumanReleaseRecordDrift",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.human_gate.m513_package_status = "human_release_recorded".to_owned();
    expect_refusal(
        "m513_status_drift",
        &m,
        &expectation,
        "M513PackageStatusDrift",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.authorization.s09_authorized = true;
    expect_refusal(
        "authorized_verdict_recorded_as_blocked",
        &m,
        &expectation,
        "AuthorizationNotRefused",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.authorization.lifecycle_state_authorizes = true;
    expect_refusal(
        "lifecycle_authorization",
        &m,
        &expectation,
        "LifecycleAuthorization",
    )?;
    checked += 1;

    let mut m = base.clone();
    m.next_required_action.java_disabled_smoke_receipt = "elsewhere.json".to_owned();
    expect_refusal("next_action_drift", &m, &expectation, "NextActionDrift")?;
    checked += 1;

    let mut drifted = expectation.clone();
    drifted
        .required_family_ids
        .push("stage.reference_segmentation".to_owned());
    expect_refusal(
        "dropped_family_against_expectation",
        &base,
        &drifted,
        "FamilyPlanDropped",
    )?;
    checked += 1;

    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &base, &drifted, "EvidenceDrift")?;
    checked += 1;

    // A redaction leak must be refused by the serializer itself.
    let mut m = base.clone();
    m.next_required_action.steps = vec!["http://leak".to_owned()];
    assert!(
        matches!(
            m.artifact_json_bytes(),
            Err(RetirementReadinessError::RedactionLeak { .. })
        ),
        "a payload leak was not refused"
    );
    checked += 1;

    // FamilyPlanRow is constructed by the module; keep the type referenced so
    // the import list documents the row shape the plan carries.
    let _row = FamilyPlanRow {
        family_id: SYNTHETIC_FAMILY.to_owned(),
        blocking_gates: vec!["required_scope.stage.header_composition".to_owned()],
        in_plan: false,
        switched: false,
    };

    Ok(format!(
        "retirement readiness self-check: {checked} named refusals executed, all refused"
    ))
}
