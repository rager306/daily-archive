use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn runner() -> PathBuf {
    std::env::var_os("LAYOUT_BAKEOFF_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let current = std::env::current_exe().unwrap();
            current
                .parent()
                .and_then(|p| p.parent())
                .unwrap()
                .join("examples/layout_bakeoff")
        })
}

fn invoke(
    binary: &std::path::Path,
    manifest_path: &std::path::Path,
    report_path: &std::path::Path,
    queue_capacity: &str,
    extra: &[&str],
) -> std::process::Output {
    Command::new(binary)
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--report",
            report_path.to_str().unwrap(),
            "--workers",
            "2",
            "--queue-capacity",
            queue_capacity,
            "--max-in-flight",
            "1",
        ])
        .args(extra)
        .output()
        .unwrap()
}

fn invoke_interrupt(
    binary: &std::path::Path,
    manifest_path: &std::path::Path,
    report_path: &std::path::Path,
    queue_capacity: &str,
    interrupt_after: usize,
) -> std::process::Output {
    Command::new(binary)
        .env(
            "DA_LAYOUT_BAKEOFF_INTERRUPT_AFTER",
            interrupt_after.to_string(),
        )
        .args([
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--report",
            report_path.to_str().unwrap(),
            "--workers",
            "2",
            "--queue-capacity",
            queue_capacity,
            "--max-in-flight",
            "1",
        ])
        .output()
        .unwrap()
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn manifest(source: &std::path::Path, checksum: &str) -> String {
    format!(
        r#"schema_version: layout-bakeoff.v1
corpus:
  id: test-corpus
  corpus_n: 1
  page_denominator: 1
  documents:
    - id: test-paper
      source_pdf: {}
      sha256: {}
      pages: 1
lanes:
  required: [pdfalto, pdf_inspector_positioned, pdf_inspector_positioned_xy_cut]
  same_input: true
  all_lanes_required: true
"#,
        source.display(),
        checksum
    )
}

#[test]
fn bakeoff_emits_one_redacted_receipt_per_lane_and_replays_semantics() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input.pdf");
    let manifest_path = dir.path().join("manifest.yaml");
    let report_path = dir.path().join("report.json");
    let bytes = b"not-a-pdf-but-a-checksum-bound-test-source";
    fs::write(&source, bytes).unwrap();
    fs::write(&manifest_path, manifest(&source, &digest(bytes))).unwrap();

    let binary = runner();
    assert!(
        binary.is_file(),
        "prebuilt runner missing: {}",
        binary.display()
    );
    let first = invoke(&binary, &manifest_path, &report_path, "2", &[]);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(first_report["corpus_n"], 1);
    assert_eq!(first_report["outcomes"].as_array().unwrap().len(), 3);
    assert_eq!(first_report["gate"]["result"], "no_selection");
    assert_eq!(first_report["gate"]["semantic_replays_equal"], false);
    assert_eq!(first_report["promotion_state"], "candidate");
    assert_eq!(first_report["auto_promote"], "forbidden");
    assert_eq!(first_report["redaction"]["raw_pdf"], false);
    assert_eq!(first_report["redaction"]["raw_text"], false);
    assert_eq!(first_report["redaction"]["tei_xml"], false);
    assert_eq!(first_report["redaction"]["physical_paths"], false);
    assert_eq!(first_report["redaction"]["token_text"], false);
    assert_eq!(first_report["redaction"]["executable"], false);
    assert_eq!(first_report["redaction"]["hostname"], false);
    assert_eq!(first_report["redaction"]["topology_path"], false);
    assert_eq!(first_report["optimization"]["cpu_only"], true);
    assert_eq!(
        first_report["optimization"]["subprocess_io_included"],
        false
    );
    assert_eq!(
        first_report["optimization"]["optimization_state"],
        "not_applicable"
    );
    assert_eq!(first_report["optimization"]["selection_forbidden"], true);
    assert!(first_report["optimization"]["scalar_oracle_digest"].is_string());
    assert!(first_report["optimization"]["semantic_digest"].is_string());
    // Honest evidence binding: exact source identity, real argv digest (not a
    // constant), full runtime-config fingerprint, and a declared-only clean-tree
    // flag (binding proof lives in the durable evidence reference).
    assert_eq!(first_report["report_state"], "final");
    let binding = &first_report["evidence_binding"];
    assert!(
        binding["tested_source_revision"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
        "tested_source_revision must be an exact digest of the tested binary"
    );
    assert_eq!(binding["declared_clean_tree"], false);
    assert!(
        binding["command_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        binding["runtime_config_fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
    );
    assert!(
        binding["configuration_fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        binding["topology_fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(binding["durable_evidence_ref"].is_null());
    let composition = &first_report["representative_donor_composition"];
    assert_eq!(composition["scope"], "single_configured_invocation");
    assert_eq!(composition["cpu_matrix_excludes_subprocess_io"], true);
    assert_eq!(composition["acceptance_denominator_separate"], true);
    assert_eq!(composition["lanes"].as_array().unwrap().len(), 3);
    assert_eq!(first_report["acceptance_denominator"]["corpus_n"], 1);
    assert_eq!(first_report["acceptance_denominator"]["lanes_n"], 3);
    assert_eq!(
        first_report["benchmark_denominator"]["pooled_with_acceptance"],
        false
    );
    // F02 regression guard: queue/in-flight gauges are runtime observations,
    // never echoes of configured capacities.
    let metrics = &first_report["metrics"];
    let in_hw = metrics["input_queue_high_water"].as_u64().unwrap();
    let out_hw = metrics["result_queue_high_water"].as_u64().unwrap();
    let in_flight_hw = metrics["max_observed_in_flight"].as_u64().unwrap();
    assert!(
        (1..=4).contains(&in_hw),
        "input high-water {in_hw} out of observed bounds"
    );
    assert!(
        (1..=4).contains(&out_hw),
        "result high-water {out_hw} out of observed bounds"
    );
    assert_eq!(
        in_flight_hw, 1,
        "in-flight high-water must respect max_in_flight=1"
    );
    // CPU matrix rows must be scoped to in-process lanes only (no pdfalto
    // subprocess attribution to CPU scaling).
    for row in first_report["worker_matrix"].as_array().unwrap() {
        assert_eq!(row["lane_scope"], "cpu_in_process");
        assert!(row["latency_p50_ms"].is_u64());
        assert_ne!(row["breakpoint"], "inconclusive");
    }
    assert!(
        first_report
            .to_string()
            .find(source.to_str().unwrap())
            .is_none()
    );
    for outcome in first_report["outcomes"].as_array().unwrap() {
        assert_eq!(outcome["document_id"], "test-paper");
        assert_eq!(outcome["source_sha256"], digest(bytes));
        assert!(outcome["failure_code"].is_string());
        assert_eq!(outcome["status"], "terminal_failure");
    }

    let second = invoke(&binary, &manifest_path, &report_path, "2", &[]);
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(
        second_report["manifest_fingerprint"],
        first_report["manifest_fingerprint"]
    );
    assert_eq!(second_report["gate"]["semantic_replays_equal"], true);
    assert_eq!(second_report["gate"]["result"], "no_selection");
}

#[test]
fn bakeoff_interruption_writes_partial_and_resume_completes_without_duplicates() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input.pdf");
    let manifest_path = dir.path().join("manifest.yaml");
    let report_path = dir.path().join("report.json");
    let partial_path = report_path.with_extension("partial.json");
    let bytes = b"not-a-pdf-for-interruption-proof";
    fs::write(&source, bytes).unwrap();
    fs::write(&manifest_path, manifest(&source, &digest(bytes))).unwrap();

    let binary = runner();
    assert!(
        binary.is_file(),
        "prebuilt runner missing: {}",
        binary.display()
    );
    let interrupted = invoke_interrupt(&binary, &manifest_path, &report_path, "2", 1);
    assert!(!interrupted.status.success());
    assert!(!report_path.exists());
    let partial: Value = serde_json::from_slice(&fs::read(&partial_path).unwrap()).unwrap();
    assert_eq!(partial["outcomes"].as_array().unwrap().len(), 1);
    assert_eq!(
        partial["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["unit_key"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );

    let resumed = invoke(&binary, &manifest_path, &report_path, "2", &["--resume"]);
    assert!(
        resumed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    let outcomes = report["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 3);
    let keys = outcomes
        .iter()
        .map(|o| o["unit_key"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), 3);
    assert!(!partial_path.exists());
}

#[test]
fn bakeoff_resume_rejects_partial_when_queue_capacity_changes() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input.pdf");
    let manifest_path = dir.path().join("manifest.yaml");
    let report_path = dir.path().join("report.json");
    let partial_path = report_path.with_extension("partial.json");
    let bytes = b"not-a-pdf-for-queue-fingerprint-proof";
    fs::write(&source, bytes).unwrap();
    fs::write(&manifest_path, manifest(&source, &digest(bytes))).unwrap();

    let binary = runner();
    assert!(
        binary.is_file(),
        "prebuilt runner missing: {}",
        binary.display()
    );
    let interrupted = invoke_interrupt(&binary, &manifest_path, &report_path, "2", 1);
    assert!(!interrupted.status.success());
    assert!(partial_path.exists());
    let old_partial: Value = serde_json::from_slice(&fs::read(&partial_path).unwrap()).unwrap();

    let resumed = invoke(&binary, &manifest_path, &report_path, "3", &["--resume"]);
    assert!(
        resumed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(report["metrics"]["queue_capacity"], 3);
    assert_ne!(
        report["donor_fingerprint"],
        old_partial["donor_fingerprint"]
    );
    assert_eq!(report["outcomes"].as_array().unwrap().len(), 3);
    assert!(!partial_path.exists());
}

#[test]
fn bakeoff_rejects_checksum_drift_before_writing_report() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input.pdf");
    let manifest_path = dir.path().join("manifest.yaml");
    let report_path = dir.path().join("report.json");
    fs::write(&source, b"changed-source").unwrap();
    fs::write(&manifest_path, manifest(&source, &"0".repeat(64))).unwrap();
    let binary = runner();
    assert!(
        binary.is_file(),
        "prebuilt runner missing: {}",
        binary.display()
    );
    let output = invoke(&binary, &manifest_path, &report_path, "2", &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checksum mismatch")
            || String::from_utf8_lossy(&output.stdout).contains("checksum mismatch")
    );
    assert!(!report_path.exists());
}
