#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn assert_redacted(value: &Value) {
    const FORBIDDEN: [&str; 13] = [
        "raw_pdf",
        "pdf_bytes",
        "raw_text",
        "text",
        "tei",
        "image",
        "path",
        "physical_path",
        "host",
        "hostname",
        "executable",
        "credentials",
        "vendor_path",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN.contains(&key.as_str()) {
                    assert_eq!(child, false, "{key}");
                }
                assert_redacted(child);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_redacted),
        Value::String(text) => {
            for marker in ["<TEI", "/root/", "file://", ".pdf", "Reference one"] {
                assert!(!text.contains(marker), "redaction marker {marker}");
            }
        }
        _ => {}
    }
}

#[test]
fn consolidation_specialized_shadow_report_is_deterministic_and_quarantined() {
    let dir = tempdir().expect("report directory");
    let report = dir.path().join("shadow.json");
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "da-cli",
            "--features",
            "pdf-layout",
            "--example",
            "consolidation_specialized_shadow",
            "--offline",
            "--",
            "--report",
            report.to_str().unwrap(),
            "--workers",
            "2",
            "--queue-capacity",
            "2",
        ])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .expect("offline runner");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(
        value["schema_version"],
        "consolidation-specialized-shadow.v1"
    );
    assert_eq!(value["denominators"]["unpooled_cells"], 54);
    assert_eq!(value["denominators"]["lane_pages"], 606);
    assert_eq!(value["denominators"]["admitted"], 54);
    assert_eq!(value["denominators"]["conserved"], 54);
    assert_eq!(value["denominators"]["loss"], 0);
    assert_eq!(value["cells"].as_array().unwrap().len(), 54);
    assert_eq!(value["replay"]["semantic_equal"], true);
    assert_eq!(value["replay"]["worker_one_equal"], true);
    assert_eq!(value["conservation"]["positive_count"], 0);
    assert_eq!(value["policy"]["decision_point"], 6);
    assert_eq!(
        value["policy"]["independently_selectable"],
        serde_json::json!(["header", "references"])
    );
    assert_eq!(value["policy"]["structural_header_consolidation"], 0);
    assert_eq!(value["policy"]["structural_citations_consolidation"], 0);
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["policy"]["graph_write"], "forbidden");
    assert_eq!(value["policy"]["production_route"], "forbidden");
    assert_redacted(&value);
}
