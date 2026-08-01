//! CLI smoke test for `da schema-check` (ADR-045 Wave E).
//!
//! Runs the schema_check function against the actual da-application/src
//! directory and asserts it reports success. This guards against drift
//! between pipeline create_node sites and the schema registry.

use std::process::Command;

#[test]
fn test_schema_check_succeeds_on_current_codebase() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("schema-check")
        .output()
        .expect("failed to spawn `da schema-check`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "schema-check exited non-zero\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("OK — every create_node label is registered"),
        "expected success marker in stdout, got:\n{stdout}"
    );
}

#[test]
fn test_schema_check_counts_match() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("schema-check")
        .output()
        .expect("failed to spawn `da schema-check`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The pipeline currently materializes 16 node types.
    assert!(
        stdout.contains("16 distinct node labels"),
        "expected 16 materialized labels, got:\n{stdout}"
    );
    // The registry contains 29 declared node types.
    assert!(
        stdout.contains("29 declared node types"),
        "expected 29 registered schemas, got:\n{stdout}"
    );
}
