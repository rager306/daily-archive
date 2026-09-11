#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn assert_redacted(value: &Value) {
    const FORBIDDEN: [&str; 14] = [
        "raw_pdf",
        "pdf_bytes",
        "raw_text",
        "text",
        "raw_tei",
        "tei",
        "image",
        "physical_path",
        "path",
        "hostname",
        "executable",
        "weights",
        "credentials",
        "vendor_path",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN.contains(&key.as_str()) {
                    assert_eq!(child, &Value::Bool(false), "{key}");
                }
                assert_redacted(child);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_redacted),
        Value::String(text) => {
            for marker in ["<TEI", "/root/", "file://", ".pdf", "model.bin"] {
                assert!(!text.contains(marker), "redaction marker {marker}");
            }
        }
        _ => {}
    }
}

#[test]
fn feature_tagger_training_shadow_report_is_deterministic_and_quarantined() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("shadow.json");
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "da-cli",
            "--features",
            "pdf-layout",
            "--example",
            "feature_tagger_training_shadow",
            "--offline",
            "--",
            "--report",
            report.to_str().unwrap(),
            "--workers",
            "4",
        ])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(value["schema_version"], "feature-tagger-training-shadow.v1");
    for (key, expected) in [
        ("documents", 3),
        ("pages_per_lane", 202),
        ("lane_pages", 606),
        ("document_lane_outcomes", 9),
        ("feature_source_units", 14),
        ("features_vector_classes", 13),
        ("feature_factory_units", 1),
        ("tagger_engines", 4),
        ("training_entry_points", 2),
        ("generator_modes", 3),
        ("lifecycle", 3),
    ] {
        assert_eq!(
            value["reference_population"].get(key),
            Some(&serde_json::json!(expected)),
            "reference population {key}: {:?}",
            value["reference_population"]
        );
    }
    assert_eq!(value["executed_population"]["objects"], 4);
    assert_eq!(value["outcomes"]["accounting_contour"]["receipts"], 4);
    assert_eq!(
        value["outcomes"]["accounting_contour"]["reviewed_structural_rows"],
        4
    );
    assert_eq!(
        value["outcomes"]["accounting_contour"]["executable_structural_rows"],
        4
    );
    assert_eq!(value["outcomes"]["accounting_contour"]["admitted"], 4);
    assert_eq!(value["outcomes"]["accounting_contour"]["conserved"], 4);
    assert_eq!(value["outcomes"]["accounting_contour"]["loss"], 0);
    assert_eq!(
        value["outcomes"]["accounting_contour"]["propagated_terminal_loss"],
        4
    );
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
    assert_eq!(value["outcomes"]["quality_claims"], serde_json::json!([]));
    assert_eq!(value["receipts"].as_array().unwrap().len(), 4);
    assert_eq!(value["receipts"][0]["disposition"], "structural_pass");
    assert_eq!(value["receipts"][1]["disposition"], "propagated_terminal");
    assert_eq!(value["replay"]["replay_equal"], true);
    assert_eq!(value["replay"]["worker_one_equal"], true);
    assert_eq!(value["conservation"]["admitted_equals_conserved"], true);
    assert_eq!(value["conservation"]["unpooled"], true);
    assert_eq!(
        value["policy"]["production_authority"],
        "java-grobid-http@0.9.0-crf"
    );
    assert_eq!(value["policy"]["decision_point"], 6);
    assert_eq!(
        value["policy"]["independently_selectable"],
        serde_json::json!(["header", "references"])
    );
    assert_eq!(
        value["policy"]["disposition"],
        "defer_insufficient_evidence"
    );
    for key in [
        "production_route",
        "auto_promote",
        "graph_write",
        "dspy",
        "gepa",
        "deployment",
    ] {
        assert_eq!(value["policy"][key], "forbidden", "policy {key}");
    }
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["execution"]["java_train_evaluate"], false);
    assert_eq!(value["execution"]["model_load"], false);
    assert_redacted(&value);
}
