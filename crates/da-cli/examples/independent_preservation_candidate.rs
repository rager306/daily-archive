//! M516-th0oby / S11 / T02 — produce the independent preservation candidate.
//!
//! This runner performs a *fresh* measurement over the native route corpus's
//! reference TEI (it re-runs the S03 measurement plane via
//! `support/native_threshold_measure.rs`; it never copies a literal value),
//! derives the independent candidate + frozen reference series from the committed
//! carriers, resolves the three frozen absorption-progress thresholds against the
//! candidate, retains the earlier conservation result as its own disclosed
//! record, and writes
//! `artifacts/parse_compat/independent_preservation_candidate.json` atomically
//! (tmp + rename).
//!
//! The candidate series is genuinely independent: its `sha256` fingerprint
//! differs from the frozen reference, so the derived identity is
//! `independent_series` and every threshold resolves `not_comparable` /
//! `mixed_scope_comparison`. Every threshold stays `not_accepted`; all five locks
//! stay closed (D127 / D536 / D545 / R140 / R141). Nothing here writes to the
//! frozen pins, the S03 threshold candidates or the acceptance package.
//!
//! It compiles in the default feature set (the measurement modules need neither
//! `pdf-layout` nor `pdf-classifier`) but is runnable under the verification
//! feature set too.

#[path = "support/native_threshold_measure.rs"]
mod native_threshold_measure;

use da_cli::independent_preservation_candidate::{
    DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL, build_independent_preservation_receipt,
    derive_independent_preservation_expectation, validate_independent_preservation_receipt,
};
use da_cli::native_family_receipt::sha256_prefixed;
use native_threshold_measure::build_threshold_candidates_artifact;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    root: PathBuf,
    report: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!(
                    "independent_preservation_candidate [--root DIR] [--report FILE]\n\
                     \n\
                     Re-measures the three frozen absorption-progress thresholds over the\n\
                     native PDF-to-TEI route corpus's reference TEI, resolves them against an\n\
                     independent candidate series whose fingerprint differs from the frozen\n\
                     reference, retains the earlier conservation result as its own disclosed\n\
                     record, and writes the artifact atomically.\n\
                     \n\
                     Defaults: --root . --report {DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL}"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args { root, report })
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("independent_preservation_candidate");
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("independent_preservation_candidate: {error}");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("independent_preservation_candidate: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    // 1. A fresh measurement over the route corpus's reference TEI. The measured
    //    candidate bytes are summarized by their digest so the fresh run is
    //    identifiable without copying any value out of the artifact.
    let measurement = build_threshold_candidates_artifact(&args.root)?;
    let measured_candidates_sha256 = sha256_prefixed(&measurement.bytes);
    let measured_rows = &measurement.receipt.threshold_candidates;

    // 2. The committed carriers derive the candidate + reference series and the
    //    series identity.
    let expectation = derive_independent_preservation_expectation(&args.root)?;

    // 3. Assemble the receipt, validate it against the recomputed expectation
    //    before writing, then serialize canonically.
    let receipt = build_independent_preservation_receipt(&expectation, measured_rows)?;
    validate_independent_preservation_receipt(&expectation, &receipt)?;
    let bytes = receipt.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    // 4. The slice demo line: every threshold resolved against the candidate,
    //    showing the retained pin, the candidate fraction/denominator, the
    //    resolution and the reason, plus the two fingerprints and the derived
    //    series-identity verdict — with the earlier conservation result retained.
    let mut table = String::new();
    for row in &receipt.thresholds_resolved {
        table.push_str(&format!(
            "\n  {} ({} @ {}): pin {}/{} = {:.6} -> candidate {}/{} = {:.6} [{}, {}]",
            row.threshold_id,
            row.metric_id,
            row.stratum,
            row.reference.fraction.numerator,
            row.reference.fraction.denominator,
            row.reference.value,
            row.candidate.fraction.numerator,
            row.candidate.fraction.denominator,
            row.candidate.value,
            row.resolution,
            row.reason,
        ));
    }

    Ok(format!(
        "independent preservation candidate `{}`: candidate {} / reference {}; series identity \
         `{}` (basis `{}`); {} thresholds resolved, thresholds {}; measured candidates {}; \
         retained conservation `{}` ({}); locks closed{}",
        receipt.schema_version,
        receipt.candidate_series.fingerprint,
        receipt.reference_series.fingerprint,
        receipt.series_identity.verdict,
        receipt.series_identity.basis,
        receipt.thresholds_resolved.len(),
        receipt.thresholds,
        measured_candidates_sha256,
        receipt.retained_conservation.verdict,
        receipt.retained_conservation.basis,
        table,
    ))
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
