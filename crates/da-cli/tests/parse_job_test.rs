//! Real binary smoke tests for the opt-in parse-job command surface.

use std::process::Command;
use tempfile::tempdir;

#[test]
fn parse_job_subcommands_are_exposed_by_real_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_da"))
        .args(["parse-job", "--help"])
        .output()
        .expect("da binary must launch");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in ["submit", "run", "status", "cancel"] {
        assert!(
            text.contains(command),
            "missing parse-job command {command}: {text}"
        );
    }
}

#[test]
fn submit_rejects_missing_manifest_before_opening_ledger() {
    let dir = tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_da"))
        .args([
            "parse-job",
            "submit",
            "--manifest",
            dir.path().join("missing.json").to_str().unwrap(),
            "--ledger",
            dir.path().join("ledger").to_str().unwrap(),
        ])
        .output()
        .expect("da binary must launch");
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("parse-job failed"));
    assert!(
        !dir.path().join("ledger").exists(),
        "invalid input opened durable state"
    );
}
