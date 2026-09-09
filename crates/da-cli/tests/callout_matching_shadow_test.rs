#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn run_example(report: &std::path::Path) -> std::process::Output {
    Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "--offline",
            "--features",
            "pdf-layout",
            "--example",
            "callout_matching_shadow",
            "--",
            "--report",
        ])
        .arg(report)
        .output()
        .expect("run offline example")
}

fn assert_redacted(value: &Value) {
    const KEYS: &[&str] = &[
        "raw_payload",
        "raw_tei",
        "raw_pdf",
        "raw_text",
        "images",
        "physical_paths",
        "host",
        "executable",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if KEYS.contains(&key.as_str()) {
                    assert_eq!(child, &Value::Bool(false), "redaction lock: {key}");
                }
                assert_redacted(child);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_redacted),
        Value::String(text) => {
            for forbidden in [
                "<TEI",
                "raw_text",
                "Reference one",
                "/root/",
                "file://",
                ".pdf",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "forbidden report value: {forbidden}"
                );
            }
        }
        _ => {}
    }
}

#[test]
fn callout_matching_shadow_is_composed_terminal_and_quarantined() {
    let dir = tempdir().expect("temporary output");
    let report = dir.path().join("callout_matching_shadow.json");
    let output = run_example(&report);
    assert!(
        output.status.success(),
        "example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(&report).expect("report");
    let value: Value = serde_json::from_str(&text).expect("JSON report");
    assert_eq!(value["schema_version"], "callout-matching-shadow.v1");
    assert!(
        value["source_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["data_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["oracle_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(
        value["corpus"],
        serde_json::json!({"documents":3,"pages":202,"lane_pages":606,"document_lane_outcomes":9,"oracle_cells":54})
    );
    assert_eq!(
        value["accounting"]["admitted"],
        value["accounting"]["conserved"]
    );
    assert_eq!(value["accounting"]["loss"], 0);
    assert_eq!(value["matching"]["admitted"], 4);
    assert_eq!(value["matching"]["matched"], 3);
    assert_eq!(value["matching"]["conserved"], 4);
    assert_eq!(value["matching"]["loss"], 0);
    assert_eq!(value["matching"]["unresolved"]["sourcenotestablished"], 1);
    assert_eq!(value["replay_equal"], true);
    assert_eq!(value["worker_one_equal"], true);
    assert_eq!(
        value["policy_locks"]["production_authority"],
        "java-grobid-http@0.9.0-crf"
    );
    assert_eq!(value["policy_locks"]["import_eligible"], false);
    assert_eq!(value["policy_locks"]["graph_write"], "forbidden");
    assert_eq!(value["policy_locks"]["production_route"], "forbidden");
    assert_eq!(value["policy_locks"]["auto_promote"], "forbidden");
    assert_eq!(value["policy_locks"]["decision_point"], 6);
    assert_eq!(
        value["policy_locks"]["independently_selectable"],
        serde_json::json!(["header", "references"])
    );
    assert_redacted(&value);
}
