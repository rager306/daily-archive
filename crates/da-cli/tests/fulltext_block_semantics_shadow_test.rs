#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn runner() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/fulltext_block_semantics_shadow");
    assert!(
        path.is_file(),
        "build example before running test: {}",
        path.display()
    );
    path
}

fn assert_redacted(value: &Value) {
    fn visit(value: &Value, in_redaction: bool) {
        let forbidden_keys = [
            "pdf_bytes",
            "tei_xml",
            "raw_text",
            "body_text",
            "page_image",
            "text",
            "path",
            "physical_path",
            "hostname",
            "executable",
            "vendor_path",
        ];
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if forbidden_keys.contains(&key.as_str()) {
                        assert!(
                            in_redaction,
                            "raw field leaked outside redaction marker: {key}"
                        );
                        assert_eq!(
                            child,
                            &Value::Bool(false),
                            "redaction marker must be false: {key}"
                        );
                    }
                    visit(child, in_redaction || key == "redaction");
                }
            }
            Value::Array(items) => items.iter().for_each(|child| visit(child, in_redaction)),
            Value::String(text) => {
                for marker in ["<TEI", "/root/", "file://", ".pdf", "raw source"] {
                    assert!(!text.contains(marker), "redaction marker leaked: {marker}");
                }
            }
            _ => {}
        }
    }

    visit(value, false);
}

#[test]
fn fulltext_shadow_is_real_accountant_bound_deterministic_and_quarantined() {
    let dir = tempdir().expect("report directory");
    let report = dir.path().join("fulltext_block_semantics_shadow.json");
    let output = Command::new(runner())
        .args(["--report", report.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "runner stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(report).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value["schema_version"],
        "fulltext-block-semantics-shadow.v1"
    );
    assert_eq!(value["denominators"]["unpooled_cells"], 54);
    assert_eq!(value["denominators"]["lane_pages"], 606);
    assert_eq!(
        value["upstream"]["admitted"],
        value["upstream"]["conserved"]
    );
    assert_eq!(value["upstream"]["loss"], 0);
    assert_eq!(value["semantic"]["replay_equal"], true);
    assert_eq!(value["semantic"]["worker_one_equal"], true);
    assert_eq!(value["policy"]["production_authority"], "java_http");
    assert_eq!(value["policy"]["decision_points"], 6);
    assert_eq!(
        value["policy"]["independently_selectable"],
        serde_json::json!(["header", "references"])
    );
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["policy"]["graph_write"], "forbidden");
    assert_eq!(value["policy"]["production_route"], "forbidden");
    assert_redacted(&value);
}
