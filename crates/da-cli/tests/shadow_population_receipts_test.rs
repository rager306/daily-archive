//! GAR-006: the offline JSON-only shadow receipts (`p5_shadow_join`,
//! `native_body_accounting_shadow`) must publish the same
//! reference/executed/evaluated population split as the file-based shadow
//! reports, so a zero-count or hand-built fixture can never be read as a corpus
//! execution. Both examples write their report with `--report FILE`, and the
//! populated enumeration of every shadow report lives in
//! `data/parse/acceptance_manifest.yaml` (`population_contract.report_paths`).

use serde_json::Value;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn run_receipt_example(example: &str, report: &Path) -> Value {
    let output = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "-q",
            "-p",
            "da-cli",
            "--example",
            example,
            "--",
            "--report",
            report.to_str().expect("report path"),
        ])
        .output()
        .expect("run offline shadow receipt example");
    assert!(
        output.status.success(),
        "{example} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(report).expect("shadow receipt report");
    serde_json::from_str(&text).expect("valid report JSON")
}

fn non_empty(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn assert_population_split(value: &Value) {
    // Reference population: identity only, with its own document count.
    assert_eq!(value["reference_population"]["role"], "identity_only");
    assert!(non_empty(&value["reference_population"]["identity"]));
    assert!(value["reference_population"]["documents"].is_u64());
    // Executed population: its own execution evidence and document count.
    assert!(value["executed_population"]["corpus_execution"].is_boolean());
    assert!(non_empty(
        &value["executed_population"]["execution_evidence"]
    ));
    assert!(value["executed_population"]["documents"].is_u64());
    // Evaluated population: an explicit status and its own document count.
    assert!(non_empty(&value["evaluated_population"]["status"]));
    assert!(value["evaluated_population"]["documents"].is_u64());
    // The three populations must stay distinguishable.
    assert_ne!(
        value["reference_population"], value["executed_population"],
        "reference and executed populations are the same object"
    );
    assert_ne!(
        value["reference_population"], value["evaluated_population"],
        "reference and evaluated populations are the same object"
    );
    assert_ne!(
        value["executed_population"], value["evaluated_population"],
        "executed and evaluated populations are the same object"
    );
    // No undifferentiated legacy block and no quality claim.
    assert!(value.get("denominators").is_none());
    assert!(value.get("corpus").is_none());
    assert_eq!(value["outcomes"]["quality_claims"], serde_json::json!([]));
}

#[test]
fn p5_shadow_join_receipt_publishes_the_population_split() {
    let dir = tempdir().expect("receipt directory");
    let report = dir.path().join("p5_shadow_join.json");
    let value = run_receipt_example("p5_shadow_join", &report);
    assert_population_split(&value);
    assert_eq!(value["reference_population"]["documents"], 0);
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["executed_population"]["documents"], 1);
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
    assert_eq!(
        value["evaluated_population"]["reason"],
        "offline_shadow_join_without_corpus_execution"
    );
}

#[test]
fn native_body_accounting_shadow_receipt_publishes_the_population_split() {
    let dir = tempdir().expect("receipt directory");
    let report = dir.path().join("native_body_accounting_shadow.json");
    let value = run_receipt_example("native_body_accounting_shadow", &report);
    assert_population_split(&value);
    // The receipt itself stays quoted inside the report rather than flattened
    // into the population blocks.
    assert_eq!(
        value["receipt"]["schema_version"],
        da_parse::NATIVE_BODY_SEQUENCE_SCHEMA_VERSION
    );
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["executed_population"]["documents"], 1);
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
}
