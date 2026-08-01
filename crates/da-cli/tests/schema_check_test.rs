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
