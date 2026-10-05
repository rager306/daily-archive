//! M516-th0oby / S03 / T03 — recompute the three frozen absorption-progress
//! thresholds on the expanded native PDF-to-TEI route corpus.
//!
//! This runner reads `data/parse/layout_bakeoff.yaml#native_route_corpus.documents`
//! and the sibling `native_route_threshold_reference` block, folds each
//! document's reference TEI through the shared composition and class-parity
//! projections, and emits
//! `artifacts/parse_compat/absorption_progress_threshold_candidates.json`
//! atomically (tmp + rename).
//!
//! The artifact publishes the retained frozen pin and the newly measured
//! candidate side by side. It never re-pins the reference, never claims a shared
//! corpus identity it does not have, and leaves `thresholds = not_accepted`
//! (D545 / R140). Diagnostic only: no product coverage, no graph/import/
//! promotion, no production route.
//!
//! It compiles in the default feature set (the measurement modules need neither
//! `pdf-layout` nor `pdf-classifier`), but it is runnable under the verification
//! feature set too.

#[path = "support/native_threshold_measure.rs"]
mod native_threshold_measure;

use da_cli::native_route_threshold_candidates::DEFAULT_THRESHOLD_CANDIDATES_ARTIFACT_REL;
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
    let mut report = PathBuf::from(DEFAULT_THRESHOLD_CANDIDATES_ARTIFACT_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!(
                    "native_route_threshold_candidates [--root DIR] [--report FILE]\n\
                     \n\
                     Recomputes the three frozen absorption-progress thresholds over the\n\
                     native PDF-to-TEI route corpus's reference TEI and writes the\n\
                     threshold-candidate artifact atomically.\n\
                     \n\
                     Defaults: --root . --report {DEFAULT_THRESHOLD_CANDIDATES_ARTIFACT_REL}"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args { root, report })
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_route_threshold_candidates");
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("native_route_threshold_candidates: {error}");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_route_threshold_candidates: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    let artifact = build_threshold_candidates_artifact(&args.root)?;
    write_artifact_atomically(&args.report, &artifact.bytes)?;

    let receipt = &artifact.receipt;
    let mut table = String::new();
    for row in &receipt.threshold_candidates {
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
            row.verdict,
            row.comparability,
        ));
    }

    Ok(format!(
        "threshold candidates `{}`: corpus `{}` ({} documents / {} pages), reference population \
         `{}` ({} documents), comparability `{}`; {} thresholds published ({}); locks closed{}",
        receipt.schema_version,
        receipt.corpus.id,
        receipt.corpus.document_count,
        receipt.corpus.page_denominator,
        receipt.corpus.population.id,
        receipt.corpus.population.document_count,
        receipt.corpus.comparability.verdict,
        receipt.threshold_candidates.len(),
        receipt.thresholds,
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
