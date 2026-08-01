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

#[test]
fn test_edge_contracts_command_outputs_table() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("edge-contracts")
        .output()
        .expect("failed to spawn `da edge-contracts`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "edge-contracts exited non-zero\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("| Edge | Source | Target(s) | Rationale |"),
        "expected table header, got:\n{stdout}"
    );
    // Every pipeline edge must appear in the rendered table.
    for edge in [
        "AFFILIATED_WITH",
        "CITES",
        "FROM_SOURCE",
        "MEMBER_OF_CLUSTER",
        "MENTIONS",
        "PARTICIPATES_IN",
        "SUPERSEDES",
        "SUPPORTS",
        "authoredBy",
        "foundIn",
        "hasPart",
        "hasTopic",
        "inCategory",
    ] {
        assert!(
            stdout.contains(&format!("`{edge}`")),
            "edge `{edge}` missing from CLI output:\n{stdout}"
        );
    }
}

#[test]
fn test_schema_list_command_outputs_table() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("schema-list")
        .output()
        .expect("failed to spawn `da schema-list`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "schema-list exited non-zero\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("| Label | Required fields | Optional fields | Materialized? |"),
        "expected table header, got:\n{stdout}"
    );
    // All 29 registered schemas must appear.
    for label in [
        "ArtifactVersion", "Author", "BaselineSnapshot", "Category",
        "Citation", "Claim", "ConceptCluster", "Concept", "Entity",
        "EvidenceBundle", "ExperimentRun", "FailureEvent", "Hypothesis",
        "ImplementationAttempt", "Institution", "Intervention",
        "InterventionBundle", "MetricDefinition", "MetricObservation",
        "Paper", "Reference", "ResearchEnvironment", "ResearchIdea",
        "ResearchProblem", "ResultComparison", "SchedulerTask", "Section",
        "Source", "Topic",
    ] {
        assert!(
            stdout.contains(&format!("`{label}`")),
            "schema `{label}` missing from CLI output:\n{stdout}"
        );
    }
}

#[test]
fn test_cross_refs_command_outputs_table() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("cross-refs")
        .output()
        .expect("failed to spawn `da cross-refs`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cross-refs exited non-zero\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("| Source | Field | Target | Required? |"),
        "expected table header, got:\n{stdout}"
    );
    // Every declared cross-reference must appear.
    for source_label in [
        "Claim",
        "MetricObservation",
        "ResearchEnvironment",
        "ResearchProblem",
        "ResultComparison",
    ] {
        assert!(
            stdout.contains(&format!("`{source_label}`")),
            "source label `{source_label}` missing from CLI output:\n{stdout}"
        );
    }
}

#[test]
fn test_validate_node_accepts_valid_snapshot() {
    use std::io::Write;
    let bin = env!("CARGO_BIN_EXE_da");
    let valid = r#"{"vid":"vid:paper:1","arxiv_id":"1","title":"T","valid_from":1,"import_eligible":false,"retrieval_eligible":true,"schema_version":1}"#;
    let mut child = std::process::Command::new(bin)
        .args(["validate-node", "--label", "Paper"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    {
        let stdin = child.stdin.as_mut().expect("opened stdin");
        stdin.write_all(valid.as_bytes()).expect("write");
    }
    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "validate-node failed on valid input\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("validates cleanly"),
        "expected success marker, got: {stdout}"
    );
}

#[test]
fn test_validate_node_rejects_invalid_snapshot() {
    use std::io::Write;
    let bin = env!("CARGO_BIN_EXE_da");
    let invalid = r#"{"vid":"x","arxiv_id":"1"}"#; // missing required fields + invariants
    let mut child = std::process::Command::new(bin)
        .args(["validate-node", "--label", "Paper"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    {
        let stdin = child.stdin.as_mut().expect("opened stdin");
        stdin.write_all(invalid.as_bytes()).expect("write");
    }
    let output = child.wait_with_output().expect("wait");
    assert!(
        !output.status.success(),
        "expected non-zero exit on invalid snapshot"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CRITICAL"), "expected CRITICAL in output: {stdout}");
    assert!(stdout.contains("required-field"), "expected required-field rule: {stdout}");
}

#[test]
fn test_audit_fields_command_succeeds_when_clean() {
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .arg("audit-fields")
        .output()
        .expect("failed to spawn `da audit-fields`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "audit-fields exited non-zero (means drift exists)\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("OK — every pipeline-set field is declared"),
        "expected success marker, got:\n{stdout}"
    );
}

#[test]
fn test_validate_graph_command_runs_on_empty_store() {
    // On a fresh/empty SamyamaGraphStore the validator should report
    // 0 nodes and 0 violations, exiting 0.
    let bin = env!("CARGO_BIN_EXE_da");
    let output = Command::new(bin)
        .args(["validate-graph", "--label", "Paper"])
        .output()
        .expect("failed to spawn `da validate-graph`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "validate-graph exited non-zero on empty store\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Validated") && stdout.contains("0 node"),
        "expected validation summary, got:\n{stdout}"
    );
    assert!(
        stdout.contains("OK — graph conforms"),
        "expected success marker on empty store, got:\n{stdout}"
    );
}
