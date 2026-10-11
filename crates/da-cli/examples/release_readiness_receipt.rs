//! M516-th0oby / S12 / T02 — publish the release-readiness receipt from committed
//! bytes.
//!
//! This runner *projects* the release-readiness receipt: it reads the committed
//! carriers via `derive_release_readiness_expectation`, which recomputes every
//! evidence sha256 from the exact committed bytes (a declared digest is never
//! copied), builds the receipt, validates it against the re-derived expectation
//! and only then writes
//! `artifacts/parse_compat/grobid_retirement_release_readiness.json` atomically
//! (tmp + rename). A tampered or stale carrier therefore fails closed instead of
//! being copied into the artifact.
//!
//! `--self-check` builds the receipt, applies one in-memory mutation per named
//! refusal, asserts the validator refuses each, prints a summary and writes
//! nothing.
//!
//! The receipt prepares a release; it never authorises one: `claim_scope` stays
//! `release_readiness_preparation_only`, `thresholds` stay `not_accepted`,
//! `product_coverage_effect` stays `none`, the human decision stays pending and
//! the five locks stay closed.
//!
//! It compiles in the default feature set (the projection needs neither
//! `pdf-layout` nor `pdf-classifier`) but is runnable under the verification
//! feature set too.

use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::release_readiness_receipt::{
    ACCEPTANCE_MANIFEST_REL, DEFAULT_RELEASE_READINESS_ARTIFACT_REL, POINTER_GATE_QUALITY,
    ReleaseGateRow, ReleaseReadinessError, ReleaseReadinessReceipt,
    build_release_readiness_receipt, derive_release_readiness_expectation,
    validate_release_readiness_receipt,
};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_RELEASE_READINESS_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!(
                    "release_readiness_receipt [--root DIR] [--report FILE] [--self-check]\n\
                     \n\
                     Projects the release-readiness receipt from the committed carriers,\n\
                     recomputing every evidence sha256 from committed bytes, validating before\n\
                     writing, and writing atomically. `--self-check` builds the receipt, applies\n\
                     one in-memory mutation per named refusal, asserts the validator refuses\n\
                     each, prints a summary and writes nothing.\n\
                     \n\
                     Defaults: --root . --report {DEFAULT_RELEASE_READINESS_ARTIFACT_REL}"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        root,
        report,
        self_check,
    })
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("release_readiness_receipt");
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("release_readiness_receipt: {error}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = if args.self_check {
        run_self_check(&args)
    } else {
        run(&args)
    };
    match outcome {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("release_readiness_receipt: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Project the receipt from committed bytes and write it atomically.
fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    // 1. Read the committed carriers and recompute every sha256 from committed
    //    bytes. A tampered or stale carrier fails closed here.
    let expectation = derive_release_readiness_expectation(&args.root)?;

    // 2. Assemble the receipt from the re-derived expectation (the envelope comes
    //    from the module constants, never from the input).
    let receipt = build_release_readiness_receipt(&expectation)?;

    // 3. Validate once more against the expectation before serializing, then
    //    serialize canonically (redaction swept).
    validate_release_readiness_receipt(&expectation, &receipt)?;
    let bytes = receipt.artifact_json_bytes()?;

    // 4. Write atomically: tmp + rename.
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "release-readiness receipt `{}` written to {} ({} bytes)\n\
         \x20 gates: {} | claim_scope {} | thresholds {} | product_coverage_effect {} | \
         nothing_here_authorizes_a_release {}\n\
         \x20 human decision: reviewer {} decision {} record {}\n\
         \x20 machine authorization: s09_authorized={} gsd_lifecycle_authorizes={} \
         human_release_recorded={} locks closed {}\n\
         \x20 milestone evidence: {}",
        receipt.schema_version,
        args.report.display(),
        bytes.len(),
        gate_summary(&receipt),
        receipt.claim_scope,
        receipt.thresholds,
        receipt.product_coverage_effect,
        receipt.nothing_here_authorizes_a_release,
        match &receipt.human_decision.reviewer {
            Some(name) => format!("{name:?}"),
            None => "null".to_owned(),
        },
        receipt.human_decision.decision,
        receipt.human_decision.decision_record,
        receipt.machine_authorization.s09_authorized,
        receipt.machine_authorization.gsd_lifecycle_authorizes,
        receipt.machine_authorization.human_release_recorded,
        receipt.machine_authorization.locks.len(),
        milestone_summary(&receipt),
    ))
}

fn gate_summary(receipt: &ReleaseReadinessReceipt) -> String {
    let mut rows: Vec<String> = Vec::new();
    for row in &receipt.release_gates {
        let shas: Vec<String> = row
            .evidence
            .iter()
            .map(|binding| short_sha(&binding.revision_sha256))
            .collect();
        rows.push(format!(
            "{}={} [{}] evidence={}",
            row.gate_id,
            row.status,
            row.release_effect,
            shas.join(",")
        ));
    }
    rows.join("; ")
}

fn milestone_summary(receipt: &ReleaseReadinessReceipt) -> String {
    let rows: Vec<String> = receipt
        .milestone_evidence
        .iter()
        .map(|row| {
            format!(
                "{}#{}@{}",
                row.slice,
                row.pointer,
                short_sha(&row.revision_sha256)
            )
        })
        .collect();
    rows.join("; ")
}

fn short_sha(sha256: &str) -> String {
    sha256.chars().take(19).collect()
}

/// Build the receipt, mutate it one refusal at a time, and assert the validator
/// refuses every mutation. Writes nothing.
fn run_self_check(args: &Args) -> Result<String, Box<dyn Error>> {
    let expectation = derive_release_readiness_expectation(&args.root)?;
    let receipt = build_release_readiness_receipt(&expectation)?;

    // The clean receipt must validate (the build already validated it; this is the
    // explicit positive half of the self-check).
    validate_release_readiness_receipt(&expectation, &receipt)?;

    let mutations = mutations();
    let mut failures: Vec<String> = Vec::new();
    for (label, expected, mutate) in &mutations {
        let mut mutated = receipt.clone();
        mutate(&mut mutated);
        match validate_release_readiness_receipt(&expectation, &mutated) {
            Ok(()) => failures.push(format!(
                "{label}: validator ACCEPTED a mutation that must be refused as {expected}"
            )),
            Err(error) => {
                let found = variant_name(&error);
                if found != *expected {
                    failures.push(format!("{label}: refused as {found}, expected {expected}"));
                }
            }
        }
    }

    if !failures.is_empty() {
        return Err(format!(
            "self-check failed ({} of {} mutations not refused as expected):\n  {}",
            failures.len(),
            mutations.len(),
            failures.join("\n  ")
        )
        .into());
    }

    Ok(format!(
        "release-readiness receipt self-check: clean receipt validates; {} of {} mutations \
         refused as expected; nothing written",
        mutations.len(),
        mutations.len()
    ))
}

/// The variant name of a refusal, for the self-check assertion.
fn variant_name(error: &ReleaseReadinessError) -> &'static str {
    use ReleaseReadinessError as E;
    match error {
        E::MalformedCarrier { .. } => "MalformedCarrier",
        E::SchemaDrift { .. } => "SchemaDrift",
        E::ClaimScopeDrift { .. } => "ClaimScopeDrift",
        E::ProductCoverageClaimed { .. } => "ProductCoverageClaimed",
        E::ThresholdClaimed { .. } => "ThresholdClaimed",
        E::NothingAuthorizesFlagFalse => "NothingAuthorizesFlagFalse",
        E::ReleaseGateMissing { .. } => "ReleaseGateMissing",
        E::ReleaseGateUnexpected { .. } => "ReleaseGateUnexpected",
        E::DuplicateReleaseGate { .. } => "DuplicateReleaseGate",
        E::ReleaseGateEvidenceEmpty { .. } => "ReleaseGateEvidenceEmpty",
        E::ReleaseGateFieldEmpty { .. } => "ReleaseGateFieldEmpty",
        E::UnboundEvidence { .. } => "UnboundEvidence",
        E::DuplicateEvidence { .. } => "DuplicateEvidence",
        E::HumanDecisionClaimed { .. } => "HumanDecisionClaimed",
        E::DecisionRecordEmpty => "DecisionRecordEmpty",
        E::LockOpened { .. } => "LockOpened",
        E::MachineAuthorizationClaimed { .. } => "MachineAuthorizationClaimed",
        E::MilestoneSliceMissing { .. } => "MilestoneSliceMissing",
        E::DuplicateMilestoneSlice { .. } => "DuplicateMilestoneSlice",
        E::MilestoneSliceUnexpected { .. } => "MilestoneSliceUnexpected",
        E::UnboundMilestoneEvidence { .. } => "UnboundMilestoneEvidence",
        E::ReleaseGateDrift { .. } => "ReleaseGateDrift",
        E::GateStatusAlignmentDrift { .. } => "GateStatusAlignmentDrift",
        E::HumanDecisionDrift { .. } => "HumanDecisionDrift",
        E::MilestoneEvidenceDrift { .. } => "MilestoneEvidenceDrift",
        E::EvidenceDrift { .. } => "EvidenceDrift",
        E::RedactionLeak { .. } => "RedactionLeak",
    }
}

type Mutation = fn(&mut ReleaseReadinessReceipt);

/// One in-memory mutation per named refusal.
fn mutations() -> Vec<(&'static str, &'static str, Mutation)> {
    vec![
        ("schema drift", "SchemaDrift", |r| {
            r.schema_version = "release-readiness.v2".to_owned();
        }),
        ("widened claim scope", "ClaimScopeDrift", |r| {
            r.claim_scope = "release_authorized".to_owned();
        }),
        ("claimed product coverage", "ProductCoverageClaimed", |r| {
            r.product_coverage_effect = "full".to_owned();
        }),
        ("accepted threshold", "ThresholdClaimed", |r| {
            r.thresholds = "accepted".to_owned();
        }),
        (
            "nothing-authorizes flag false",
            "NothingAuthorizesFlagFalse",
            |r| r.nothing_here_authorizes_a_release = false,
        ),
        ("redaction flag set", "RedactionLeak", |r| {
            r.redaction.raw_text = true;
        }),
        ("dropped release gate", "ReleaseGateMissing", |r| {
            r.release_gates.retain(|row| row.gate_id != "cost");
        }),
        ("extra release gate", "ReleaseGateUnexpected", |r| {
            r.release_gates.push(ReleaseGateRow {
                gate_id: "speed".to_owned(),
                status: "met".to_owned(),
                release_effect: "does_not_authorize_anything".to_owned(),
                evidence: vec![CarrierBinding {
                    manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
                    pointer: POINTER_GATE_QUALITY.to_owned(),
                    revision_sha256: sha256_prefixed(b"speed"),
                }],
            });
        }),
        ("duplicate release gate", "DuplicateReleaseGate", |r| {
            let duplicate = r.release_gates[0].clone();
            r.release_gates.push(duplicate);
        }),
        ("empty gate evidence", "ReleaseGateEvidenceEmpty", |r| {
            gate_mut(&mut r.release_gates, "rights").evidence.clear();
        }),
        ("empty gate status", "ReleaseGateFieldEmpty", |r| {
            gate_mut(&mut r.release_gates, "quality").status = String::new();
        }),
        ("non-repo-relative evidence path", "UnboundEvidence", |r| {
            gate_mut(&mut r.release_gates, "quality").evidence[0].manifest =
                "/absolute/path.yaml".to_owned();
        }),
        ("duplicate evidence binding", "DuplicateEvidence", |r| {
            let duplicate = gate_mut(&mut r.release_gates, "quality").evidence[0].clone();
            gate_mut(&mut r.release_gates, "quality")
                .evidence
                .push(duplicate);
        }),
        ("named reviewer", "HumanDecisionClaimed", |r| {
            r.human_decision.reviewer = Some("Ada".to_owned());
        }),
        ("recorded decision", "HumanDecisionClaimed", |r| {
            r.human_decision.decision = "approved".to_owned();
        }),
        ("empty decision record", "DecisionRecordEmpty", |r| {
            r.human_decision.decision_record = String::new();
        }),
        ("opened lock", "LockOpened", |r| {
            r.machine_authorization
                .locks
                .insert("import_eligible".to_owned(), "true".to_owned());
        }),
        (
            "claimed s09 authorization",
            "MachineAuthorizationClaimed",
            |r| {
                r.machine_authorization.s09_authorized = true;
            },
        ),
        ("dropped milestone slice", "MilestoneSliceMissing", |r| {
            r.milestone_evidence.retain(|row| row.slice != "S11");
        }),
        (
            "duplicate milestone slice",
            "DuplicateMilestoneSlice",
            |r| {
                let duplicate = r.milestone_evidence[0].clone();
                r.milestone_evidence.push(duplicate);
            },
        ),
        (
            "unexpected milestone slice",
            "MilestoneSliceUnexpected",
            |r| r.milestone_evidence[0].slice = "S99".to_owned(),
        ),
        (
            "unbound milestone evidence",
            "UnboundMilestoneEvidence",
            |r| r.milestone_evidence[0].revision_sha256 = "sha256:0".to_owned(),
        ),
        (
            "gate status alignment drift",
            "GateStatusAlignmentDrift",
            |r| {
                r.human_decision
                    .gate_status
                    .insert("quality".to_owned(), "met".to_owned());
            },
        ),
        ("release gate drift", "ReleaseGateDrift", |r| {
            gate_mut(&mut r.release_gates, "quality").status = "met".to_owned();
            r.human_decision
                .gate_status
                .insert("quality".to_owned(), "met".to_owned());
        }),
        ("stale evidence sha256", "EvidenceDrift", |r| {
            gate_mut(&mut r.release_gates, "quality").evidence[0].revision_sha256 =
                sha256_prefixed(b"stale");
        }),
        ("human decision drift", "HumanDecisionDrift", |r| {
            r.human_decision.decision_record = "doc/review/other.md".to_owned();
        }),
        ("milestone evidence drift", "MilestoneEvidenceDrift", |r| {
            milestone_mut(&mut r.milestone_evidence, "S03").revision_sha256 =
                sha256_prefixed(b"drifted");
        }),
    ]
}

fn gate_mut<'a>(gates: &'a mut Vec<ReleaseGateRow>, gate_id: &str) -> &'a mut ReleaseGateRow {
    gates
        .iter_mut()
        .find(|row| row.gate_id == gate_id)
        .expect("the gate is present in the committed receipt")
}

fn milestone_mut<'a>(
    rows: &'a mut Vec<da_cli::release_readiness_receipt::MilestoneEvidence>,
    slice: &str,
) -> &'a mut da_cli::release_readiness_receipt::MilestoneEvidence {
    rows.iter_mut()
        .find(|row| row.slice == slice)
        .expect("the milestone slice is present in the committed receipt")
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
