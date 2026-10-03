//! S09/T04 (M514-1lss2s) — canonical Java-disabled smoke rehearsal runner.
//!
//! This is the executable half of the T04 smoke contract. It is deliberately
//! **not a runtime route**: it is not registered in `ParserPort`, `source_cli`
//! or any `da` subcommand, and it never writes to production state.
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No carrier is read and nothing is written. It
//!   exercises the live refusal path and, with a synthetic in-memory
//!   authorization that is never written to a committed artifact, the authorized
//!   path plus its positive control (one synthetic Java consult through a real
//!   [`JavaInvocationCounter`], so `java_calls = 0` is a measurement rather than
//!   vacuity). It then rejects each named mutation by name.
//! * default — canonical run over the committed carriers. It re-runs the T01
//!   authorization gate over the committed S08 gate ledger and human release
//!   record, builds the live `not_authorized` receipt, validates it, and writes
//!   `artifacts/parse_compat/native_java_disabled_smoke.json` atomically
//!   (tmp + rename).
//!
//! Fail closed in the live state: `families_switched = []`,
//! `product_scope_confirmed = false`, `product_coverage_effect = none`, the five
//! locks closed, and the typed `not_authorized` refusal. Nothing here switches a
//! family, consults Java, writes a graph or claims product coverage.

use da_cli::native_family_receipt::{
    CarrierBinding, GuardProvenance, REQUIRED_LOCKS, sha256_prefixed,
};
use da_cli::native_java_disabled_smoke::{
    DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL, JAVA_DISABLED_SMOKE_AUTHORIZED,
    JAVA_DISABLED_SMOKE_NOT_AUTHORIZED, JavaDisabledSmokeError, JavaDisabledSmokeExpectation,
    JavaDisabledSmokeInput, JavaDisabledSmokeReceipt, PositiveControlProvenance,
    SMOKE_EVIDENCE_CARRIERS, SMOKE_REFUSAL_NOT_AUTHORIZED, build_java_disabled_smoke_receipt,
    guard_from_counter, guard_not_installed, observe_positive_control,
    validate_java_disabled_smoke_receipt,
};
use da_parse::{
    HumanReleaseRecord, JavaInvocationCounter, MandatoryGate, NATIVE_ACCEPTANCE_CLAIM_SCOPE,
    NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION, NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT,
    NativeAcceptanceEvaluation, PackageBinding, RETIREMENT_PACKAGE_STATUS_AUTHORIZED,
    RetirementAuthorizationInput, authorize_native_retirement,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// A well-formed `sha256:` pin used only by the hermetic self-check.
const SYNTHETIC_PIN: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// The synthetic contract families used only by the hermetic self-check.
const SYNTHETIC_FAMILY_PRESERVATION: &str = "parse.component.native_fulltext_composition_shadow";
const SYNTHETIC_FAMILY_HEADER: &str = "stage.header_composition";
/// The S08 acceptance package the live run re-reads by pointer.
const PACKAGE_REL: &str = "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
/// The S02 absorption-progress schema the live run re-reads by pointer.
const SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!("native_java_disabled_smoke [--root DIR] [--report FILE] [--self-check]");
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
    let _log_guard = da_cli::logging::init_pipeline_logging("native_java_disabled_smoke");
    let args = parse_args();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_java_disabled_smoke: {error}");
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
    SMOKE_EVIDENCE_CARRIERS
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

fn required_families(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let schema: serde_yaml::Value = serde_yaml::from_slice(&read_bytes(root, SCHEMA_REL)?)?;
    Ok(schema
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
        .collect())
}

fn human_release_from_root(root: &Path) -> Result<HumanReleaseRecord, Box<dyn Error>> {
    let manifest: serde_yaml::Value = serde_yaml::from_slice(&read_bytes(
        root,
        da_cli::native_retirement_readiness::READINESS_MANIFEST_REL,
    )?)?;
    let release = manifest
        .get("release_decision_package")
        .ok_or("the manifest carries no release_decision_package")?;
    let review = release
        .get("human_release_review")
        .ok_or("the manifest carries no human_release_review")?;
    Ok(HumanReleaseRecord {
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
    })
}

// ───────────────────────────── full run ─────────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }
    let root = args.root.as_path();
    let evidence = evidence_from_root(root)?;

    // ── the T01 authorization, re-run over the committed carriers ──────────
    let package_bytes = read_bytes(root, PACKAGE_REL)?;
    let package: serde_json::Value = serde_json::from_slice(&package_bytes)?;
    let gate_rows = package
        .get("mandatory_gates")
        .and_then(serde_json::Value::as_array)
        .ok_or("the S08 package carries no mandatory_gates")?;
    let gates: Vec<MandatoryGate> = gate_rows
        .iter()
        .map(map_gate)
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let families = required_families(root)?;
    let human_release = human_release_from_root(root)?;

    let package_pin = sha256_prefixed(&package_bytes);
    let authorization = authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_owned(),
            declared_sha256: Some(package_pin.clone()),
            actual_sha256: Some(package_pin),
        },
        human_release,
        lifecycle_state_authorizes: false,
        locks: rendered_locks(&package),
        required_families: families.clone(),
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

    let receipt = build_java_disabled_smoke_receipt(&JavaDisabledSmokeInput {
        evidence: evidence.clone(),
        authorization: authorization.clone(),
        selection: families.clone(),
        guard: guard_not_installed(),
        positive_control: PositiveControlProvenance {
            attempted: 0,
            observed: 0,
        },
    })?;

    let expectation = JavaDisabledSmokeExpectation {
        evidence,
        required_family_ids: families,
        authorization,
    };
    validate_java_disabled_smoke_receipt(&receipt, &expectation)?;

    let bytes = receipt.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "java-disabled smoke `{}`: verdict={} claim_scope={} product_scope_confirmed={} \
         families_switched={} selection={} java_calls={} positive_control={} refusals={}; canonical run",
        receipt.schema_version,
        receipt.verdict,
        receipt.claim_scope,
        receipt.product_scope_confirmed,
        receipt.families_switched.len(),
        receipt.selection.len(),
        receipt.guard.observed_java_calls,
        receipt.positive_control.observed,
        receipt.refusals.len(),
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

/// A fully authorized synthetic record: named reviewer, recorded decision,
/// closed locks, current package pin, every required gate `met`.
fn synthetic_authorized() -> da_parse::NativeRetirementAuthorization {
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

/// A refused synthetic record, structurally identical to the live state.
fn synthetic_refused() -> da_parse::NativeRetirementAuthorization {
    authorize_native_retirement(&RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_owned(),
            declared_sha256: Some(SYNTHETIC_PIN.to_owned()),
            actual_sha256: Some(SYNTHETIC_PIN.to_owned()),
        },
        human_release: HumanReleaseRecord {
            reviewer: None,
            decision: "pending".to_owned(),
            decision_record: Some("doc/review/HUMAN-REVIEW-TEMPLATE.md".to_owned()),
            prose_alone_authorizes_release: false,
            package_status: "pending_human_review".to_owned(),
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
            gates: vec![synthetic_gate(
                "required_scope.stage.header_composition",
                "threshold_pending_family",
                "threshold_pending",
                Some(SYNTHETIC_FAMILY_HEADER),
            )],
        },
    })
}

fn expect_build_refusal(
    label: &str,
    input: &JavaDisabledSmokeInput,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match build_java_disabled_smoke_receipt(input) {
        Ok(_) => Err(format!("{label} was admitted").into()),
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

fn expect_validate_refusal(
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

/// Deterministic negative controls, all hermetic (no carrier, no write). Each
/// mutation must be refused **by name**; any mutation that slips through fails.
fn run_self_check() -> Result<String, Box<dyn Error>> {
    let mut checked = 0usize;

    // ── the live refusal path ─────────────────────────────────────────────
    let refused = synthetic_refused();
    if refused.s09_authorized {
        return Err("the synthetic refused authorization authorized a release".into());
    }
    let refused_selection = refused
        .families
        .iter()
        .map(|family| family.family_id.clone())
        .collect::<Vec<_>>();
    let refused_input = JavaDisabledSmokeInput {
        evidence: Vec::new(),
        authorization: refused.clone(),
        selection: refused_selection.clone(),
        guard: guard_not_installed(),
        positive_control: PositiveControlProvenance {
            attempted: 0,
            observed: 0,
        },
    };
    let refused_receipt = build_java_disabled_smoke_receipt(&refused_input)?;
    let refused_expectation = JavaDisabledSmokeExpectation {
        evidence: Vec::new(),
        required_family_ids: refused_selection.clone(),
        authorization: refused,
    };
    validate_java_disabled_smoke_receipt(&refused_receipt, &refused_expectation)?;
    assert_eq!(refused_receipt.verdict, JAVA_DISABLED_SMOKE_NOT_AUTHORIZED);
    assert!(refused_receipt.families_switched.is_empty());
    assert!(!refused_receipt.product_scope_confirmed);
    assert!(!refused_receipt.guard.installed);
    assert!(
        refused_receipt
            .refusals
            .iter()
            .any(|refusal| refusal.code == SMOKE_REFUSAL_NOT_AUTHORIZED)
    );

    // ── the authorized path with a non-vacuous positive control ───────────
    let authorized = synthetic_authorized();
    if !authorized.s09_authorized {
        return Err("the synthetic authorized record did not authorize".into());
    }
    let selection = authorized
        .families
        .iter()
        .map(|family| family.family_id.clone())
        .collect::<Vec<_>>();
    let run_counter = JavaInvocationCounter::new();
    let guard = guard_from_counter(&run_counter, true);
    let control_counter = JavaInvocationCounter::new();
    let control_observed = observe_positive_control(&control_counter);
    assert_eq!(control_observed, 1, "the positive control must observe one");
    let authorized_input = JavaDisabledSmokeInput {
        evidence: Vec::new(),
        authorization: authorized.clone(),
        selection: selection.clone(),
        guard,
        positive_control: PositiveControlProvenance {
            attempted: 1,
            observed: control_observed,
        },
    };
    let authorized_receipt = build_java_disabled_smoke_receipt(&authorized_input)?;
    let authorized_expectation = JavaDisabledSmokeExpectation {
        evidence: Vec::new(),
        required_family_ids: selection.clone(),
        authorization: authorized.clone(),
    };
    validate_java_disabled_smoke_receipt(&authorized_receipt, &authorized_expectation)?;
    assert_eq!(authorized_receipt.verdict, JAVA_DISABLED_SMOKE_AUTHORIZED);
    assert!(authorized_receipt.product_scope_confirmed);
    assert_eq!(authorized_receipt.guard.observed_java_calls, 0);
    assert!(authorized_receipt.guard.installed);
    assert!(authorized_receipt.guard.identity_isolated);
    assert!(authorized_receipt.positive_control.non_vacuous);
    assert_eq!(authorized_receipt.families_switched, selection);
    assert!(authorized_receipt.refusals.is_empty());
    checked += 1;

    // ── builder refusals ──────────────────────────────────────────────────
    let mut m = refused_input.clone();
    m.guard = GuardProvenance {
        installed: true,
        identity_isolated: true,
        observed_java_calls: 0,
    };
    expect_build_refusal(
        "guard_installed_while_refused",
        &m,
        "ProductScopeClaimedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = refused_input.clone();
    m.positive_control = PositiveControlProvenance {
        attempted: 1,
        observed: 1,
    };
    expect_build_refusal(
        "positive_control_while_refused",
        &m,
        "ProductScopeClaimedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = authorized_input.clone();
    m.guard = guard_not_installed();
    expect_build_refusal("guard_not_installed", &m, "GuardNotInstalled")?;
    checked += 1;

    let mut m = authorized_input.clone();
    m.guard = GuardProvenance {
        installed: true,
        identity_isolated: false,
        observed_java_calls: 0,
    };
    expect_build_refusal(
        "guard_identity_not_isolated",
        &m,
        "GuardIdentityNotIsolated",
    )?;
    checked += 1;

    let mut m = authorized_input.clone();
    m.guard = GuardProvenance {
        installed: true,
        identity_isolated: true,
        observed_java_calls: 1,
    };
    expect_build_refusal(
        "java_consulted_on_disabled_families",
        &m,
        "ObservedJavaOnDisabledFamilies",
    )?;
    checked += 1;

    let mut m = authorized_input.clone();
    m.positive_control = PositiveControlProvenance {
        attempted: 0,
        observed: 0,
    };
    expect_build_refusal(
        "positive_control_not_observed",
        &m,
        "PositiveControlNotObserved",
    )?;
    checked += 1;

    let mut m = authorized_input.clone();
    m.positive_control = PositiveControlProvenance {
        attempted: 1,
        observed: 0,
    };
    expect_build_refusal(
        "positive_control_inconsistent",
        &m,
        "PositiveControlInconsistent",
    )?;
    checked += 1;

    // ── validator refusals on the live receipt ────────────────────────────
    let mut m = refused_receipt.clone();
    m.product_scope_confirmed = true;
    expect_validate_refusal(
        "product_scope_claimed",
        &m,
        &refused_expectation,
        "ProductScopeClaimedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.families_switched.push(SYNTHETIC_FAMILY_HEADER.to_owned());
    expect_validate_refusal(
        "family_switched_without_authorization",
        &m,
        &refused_expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.selection[0].switched = true;
    m.selection[0].java_disabled = true;
    expect_validate_refusal(
        "selection_row_switched",
        &m,
        &refused_expectation,
        "FamilySwitchedWithoutAuthorization",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_validate_refusal("opened_lock", &m, &refused_expectation, "LockOpened")?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.claim_scope = "authorized_retirement_release".to_owned();
    expect_validate_refusal(
        "claim_scope_drift",
        &m,
        &refused_expectation,
        "ClaimScopeDrift",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.product_coverage_effect = "product".to_owned();
    expect_validate_refusal(
        "product_coverage_claimed",
        &m,
        &refused_expectation,
        "ProductCoverageClaimed",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.refusals.clear();
    expect_validate_refusal(
        "missing_not_authorized_refusal",
        &m,
        &refused_expectation,
        "MissingTypedRefusal",
    )?;
    checked += 1;

    let mut m = refused_receipt.clone();
    m.authorization.verdict = "authorized".to_owned();
    expect_validate_refusal(
        "authorization_drift",
        &m,
        &refused_expectation,
        "AuthorizationDrift",
    )?;
    checked += 1;

    // ── validator refusals on the authorized receipt ──────────────────────
    let mut m = authorized_receipt.clone();
    m.guard.observed_java_calls = 1;
    expect_validate_refusal(
        "authorized_java_calls",
        &m,
        &authorized_expectation,
        "ObservedJavaOnDisabledFamilies",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.guard.installed = false;
    expect_validate_refusal(
        "authorized_guard_absent",
        &m,
        &authorized_expectation,
        "GuardNotInstalled",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.guard.identity_isolated = false;
    expect_validate_refusal(
        "authorized_guard_not_isolated",
        &m,
        &authorized_expectation,
        "GuardIdentityNotIsolated",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.positive_control.observed = 0;
    expect_validate_refusal(
        "authorized_vacuous_control",
        &m,
        &authorized_expectation,
        "PositiveControlNotObserved",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.positive_control.non_vacuous = false;
    expect_validate_refusal(
        "authorized_control_not_reported_non_vacuous",
        &m,
        &authorized_expectation,
        "PositiveControlInconsistent",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.product_scope_confirmed = false;
    expect_validate_refusal(
        "authorized_scope_unconfirmed",
        &m,
        &authorized_expectation,
        "ProductScopeNotConfirmed",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.families_switched.push("stage.absent_family".to_owned());
    expect_validate_refusal(
        "switched_family_not_selected",
        &m,
        &authorized_expectation,
        "FamilyNotSelected",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    let blocking = "required_scope.stage.header_composition.blocking".to_owned();
    m.authorization.families[0].blocking_gates = vec![blocking.clone()];
    m.selection[0].blocking_gates = vec![blocking];
    expect_validate_refusal(
        "switched_family_with_blocking_gate",
        &m,
        &authorized_expectation,
        "FamilyGateBlocks",
    )?;
    checked += 1;

    let mut m = authorized_receipt.clone();
    m.selection[0].switched = false;
    m.selection[0].java_disabled = false;
    expect_validate_refusal(
        "switched_family_row_not_switched",
        &m,
        &authorized_expectation,
        "FamilyNotSwitched",
    )?;
    checked += 1;

    let mut drifted = authorized_expectation.clone();
    drifted
        .required_family_ids
        .push("stage.absent_family".to_owned());
    expect_validate_refusal(
        "dropped_family_against_expectation",
        &authorized_receipt,
        &drifted,
        "SelectionDropped",
    )?;
    checked += 1;

    let mut drifted = refused_expectation.clone();
    drifted.evidence.push(CarrierBinding {
        manifest: PACKAGE_REL.to_owned(),
        pointer: "mandatory_gates".to_owned(),
        revision_sha256: sha256_prefixed(b"drifted"),
    });
    expect_validate_refusal(
        "evidence_drift",
        &refused_receipt,
        &drifted,
        "EvidenceDrift",
    )?;
    checked += 1;

    // ── redaction is refused by the serializer itself ─────────────────────
    let mut m = refused_receipt.clone();
    m.refusals[0].detail = "http://leak".to_owned();
    assert!(
        matches!(
            m.artifact_json_bytes(),
            Err(JavaDisabledSmokeError::RedactionLeak { .. })
        ),
        "a payload leak was not refused"
    );
    checked += 1;

    Ok(format!(
        "java-disabled smoke self-check: {checked} named refusals executed, all refused; \
         authorized java_calls=0 with positive control observed={control_observed}"
    ))
}
