//! M516-th0oby / S04 / T01 — emit the per-document semantic-correctness receipt
//! over the expanded 12-document native route corpus.
//!
//! This runner reads `data/parse/layout_bakeoff.yaml#native_route_threshold_reference`
//! (the 12 sha256-pinned reference TEI), folds each document through the
//! composer, and writes `artifacts/parse_compat/native_route_semantic_receipt.json`
//! atomically (tmp + rename). It also reads the committed native route emission
//! envelope hermetically to disclose the native-route-emission scope beside the
//! composer's per-document facts.
//!
//! The artifact publishes one closed well-formedness reason and one closed
//! bibliography-target reason per document, plus the placeholder-header /
//! typed-absence / zero-declared-target scope as named reasons — never one
//! aggregate verdict and never an empty-target pass. It leaves every threshold
//! `not_accepted`, every lock closed, and the frozen three-document aggregate
//! byte-unchanged (D127 / ADR-076 §14 / R140).
//!
//! It compiles in the default feature set (the measurement modules need neither
//! `pdf-layout` nor `pdf-classifier`), but it is runnable under the verification
//! feature set too.

#[path = "support/native_semantic_measure.rs"]
mod native_semantic_measure;

use da_cli::native_route_semantic_receipt::DEFAULT_SEMANTIC_RECEIPT_ARTIFACT_REL;
use native_semantic_measure::build_semantic_receipt_artifact;
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
    let mut report = PathBuf::from(DEFAULT_SEMANTIC_RECEIPT_ARTIFACT_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!(
                    "native_route_semantic_receipt [--root DIR] [--report FILE]\n\
                     \n\
                     Runs the composer's H14 well-formedness and H15 bibliography-target\n\
                     checks on the 12-document route corpus's reference TEI and writes the\n\
                     per-document semantic-correctness receipt atomically.\n\
                     \n\
                     Defaults: --root . --report {DEFAULT_SEMANTIC_RECEIPT_ARTIFACT_REL}"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args { root, report })
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_route_semantic_receipt");
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("native_route_semantic_receipt: {error}");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_route_semantic_receipt: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    let artifact = build_semantic_receipt_artifact(&args.root)?;
    write_artifact_atomically(&args.report, &artifact.bytes)?;

    let receipt = &artifact.receipt;
    let mut table = String::new();
    for row in &receipt.documents {
        table.push_str(&format!(
            "\n  {} ({}): band {} / composer {} / wf {} / targets {}",
            row.document_id,
            row.profile_class,
            row.admitted_band.reason,
            row.composer.outcome,
            row.composer.well_formedness_reason,
            row.composer.bibliography_target_reason,
        ));
    }
    let mut scope = String::new();
    for row in &receipt.native_route_emission_scope.documents {
        scope.push_str(&format!(
            "\n  {}: header {} / references {} / body {} / {}",
            row.document_id,
            row.header.scope_reason,
            row.references.scope_reason,
            row.body.scope_reason,
            row.reference_links.scope_reason,
        ));
    }

    Ok(format!(
        "semantic receipt `{}`: corpus `{}` ({} documents / {} pages), composer {} composed / {} \
         refused, required targets {} resolved of {} ({} documents declare none); thresholds {}\n\
         composer rows:{}\nnative route emission scope ({}):{}",
        receipt.schema_version,
        receipt.corpus.id,
        receipt.corpus.document_count,
        receipt.corpus.page_denominator,
        receipt.population.composed,
        receipt.population.refused,
        receipt.population.resolved_targets_total,
        receipt.population.required_targets_total,
        receipt.population.documents - receipt.population.documents_with_required_targets,
        receipt.thresholds,
        table,
        receipt.native_route_emission_scope.document_count,
        scope,
    ))
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
