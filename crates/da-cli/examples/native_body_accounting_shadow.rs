//! Deterministic, redacted P2 → native body-accounting shadow receipt.
//! This example is intentionally not a production CLI command or ParserPort path.
//!
//! GAR-006: the emitted report separates the referenced population, the
//! executed population and the evaluated population, so a zero-count terminal
//! receipt can never be read as a corpus execution.

use da_parse::{
    LayoutDocument, NativeBodySequenceInput, NativeBodySequenceReceipt, run_native_body_sequence,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct ShadowReport<'a> {
    schema_version: &'static str,
    // The three populations are separate objects with their own `documents`
    // counts: what the P2/P5 registry references, what this offline example
    // actually executed, and what (if anything) was evaluated.
    reference_population: serde_json::Value,
    executed_population: serde_json::Value,
    evaluated_population: serde_json::Value,
    outcomes: serde_json::Value,
    receipt: &'a NativeBodySequenceReceipt,
    redaction: &'a BTreeMap<String, bool>,
}

fn parse_report_path() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    // Every arm below either returns or terminates, so this is a single
    // dispatch rather than a loop (`clippy::never_loop`).
    if let Some(arg) = args.next() {
        match arg.as_str() {
            "--report" => return args.next().map(std::path::PathBuf::from),
            "--help" => {
                println!("native_body_accounting_shadow [--report FILE]");
                std::process::exit(0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_body_accounting_shadow");
    let report_path = parse_report_path();
    let document = LayoutDocument::new("shadow-example", "a".repeat(64), vec![])?;
    let receipt = run_native_body_sequence(NativeBodySequenceInput {
        document: &document,
        segmentation: None,
        p2_status: "terminal_failure",
        p2_failure_code: Some("example_requires_source_bound_p2_receipt"),
    });
    let report = ShadowReport {
        schema_version: "native-body-accounting-shadow.v1",
        reference_population: serde_json::json!({
            "identity": "native_body_accounting_p2_registry",
            "role": "identity_only",
            "documents": 0,
            "pages": 0,
            "document_lane_outcomes": 0,
            "note": "p2_receipt_registry_not_a_document_corpus",
        }),
        executed_population: serde_json::json!({
            "corpus_execution": false,
            "documents": 1,
            "pages": 0,
            "objects": 0,
            "execution_evidence": "tracked_offline_fixture:native_body_accounting_shadow",
        }),
        evaluated_population: serde_json::json!({
            "status": "not_evaluated",
            "documents": 0,
            "oracle": "none",
            "reason": "terminal_failure_receipt_without_corpus_execution",
        }),
        outcomes: serde_json::json!({
            "accounting_contour": {
                "admitted": receipt.counts.admitted,
                "conserved": receipt.counts.conserved,
                "unsupported": receipt.counts.unsupported,
                "loss": receipt.counts.loss,
                "receipt_status": receipt.status,
            },
            "quality_claims": [],
        }),
        receipt: &receipt,
        redaction: &receipt.redaction,
    };
    let serialized = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &serialized)?;
    }
    println!("{}", String::from_utf8(serialized)?);
    Ok(())
}
