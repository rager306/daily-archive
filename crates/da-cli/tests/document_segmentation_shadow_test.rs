#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn runner() -> PathBuf {
    std::env::var_os("DOCUMENT_SEGMENTATION_SHADOW_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .expect("integration test executable")
                .parent()
                .expect("deps directory")
                .parent()
                .expect("debug directory")
                .join("examples/document_segmentation_shadow")
        })
}

fn assert_redacted(value: &Value, path: &str, in_redaction: bool) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                let key_is_sensitive = [
                    "raw_text",
                    "raw_pdf",
                    "pdf_bytes",
                    "tei_xml",
                    "images",
                    "page_image",
                    "physical_paths",
                    "hostname",
                    "executable",
                    "figure_head",
                    "table_head",
                    "formula_head",
                    "caption",
                    "callout",
                    "fulltext",
                    "parsed_article",
                    "graph_payload",
                ]
                .contains(&key.as_str());
                if key_is_sensitive && !in_redaction && key != "redaction" {
                    panic!("sensitive report field escaped redaction block at {child_path}");
                }
                if in_redaction && key_is_sensitive {
                    assert!(
                        child == &Value::Bool(false),
                        "redaction marker must be false at {child_path}"
                    );
                }
                assert_redacted(child, &child_path, in_redaction || key == "redaction");
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_redacted(child, &format!("{path}[{index}]"), in_redaction);
            }
        }
        Value::String(text) => {
            for marker in [
                "data/sources",
                "file://",
                "/root/",
                ".pdf",
                "<tei",
                "<TEI",
                "FigureHead",
                "TableHead",
                "FormulaHead",
                "VisualCaption",
                "Callout",
                "ParsedArticle",
            ] {
                assert!(
                    !text.contains(marker),
                    "forbidden report marker `{marker}` at {path}"
                );
            }
        }
        _ => {}
    }
}

#[test]
fn source_bound_shadow_report_is_terminal_redacted_and_unpooled() {
    let dir = tempdir().expect("temporary report directory");
    let report = dir.path().join("document-segmentation-shadow.json");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let layout_manifest = repo_root.join("data/parse/layout_bakeoff.yaml");
    let segmentation_manifest = repo_root.join("data/parse/document_segmentation.yaml");
    let output = Command::new(runner())
        .args([
            "--layout-manifest",
            layout_manifest.to_str().expect("layout manifest path"),
            "--segmentation-manifest",
            segmentation_manifest
                .to_str()
                .expect("segmentation manifest path"),
            "--report",
            report.to_str().expect("report path"),
        ])
        .output()
        .expect("run feature-gated shadow example");
    assert!(
        output.status.success(),
        "runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(&report).expect("canonical report");
    let value: Value = serde_json::from_str(&text).expect("valid JSON report");
    assert_eq!(value["schema_version"], "document-segmentation-shadow.v1");
    assert_eq!(value["denominators"]["corpus_documents"], 3);
    assert_eq!(value["denominators"]["lanes"], 3);
    assert_eq!(value["denominators"]["pages_per_lane"], 202);
    assert_eq!(value["denominators"]["lane_pages"], 606);
    assert_eq!(value["denominators"]["matrix_rows"], 45);
    assert_eq!(value["outcomes"].as_array().expect("outcomes").len(), 9);
    assert_eq!(value["matrix"].as_array().expect("matrix").len(), 45);
    assert_eq!(value["replay"]["repetitions"], 2);
    assert_eq!(value["replay"]["semantic_equal"], true);
    assert_eq!(value["replay"]["worker_one_equal"], true);
    assert_eq!(value["policy"]["observation_mode"], "shadow");
    assert_eq!(value["policy"]["promotion_state"], "candidate");
    assert_eq!(value["policy"]["auto_promote"], "forbidden");
    assert_eq!(value["policy"]["donor_selection"], "forbidden");
    assert_eq!(value["policy"]["import_eligible"], false);

    let redaction = &value["redaction"];
    for key in [
        "raw_text",
        "raw_pdf",
        "tei_xml",
        "images",
        "physical_paths",
        "hostname",
        "executable",
        "visual_structure_heads",
    ] {
        assert_eq!(redaction[key], false, "redaction flag {key}");
    }
    assert_redacted(&value, "$", false);
    assert_eq!(value["gate"]["no_selection"], true);
    assert_eq!(value["policy"]["selection"], "forbidden");
    assert!(
        !text.contains("data/sources")
            && !text.contains(".pdf")
            && !text.contains("/root/")
            && !text.contains("file://")
            && !text.contains("ParsedArticle")
            && !text.contains("fulltext")
            && !text.contains("graph_payload"),
        "durable report leaked forbidden source, host, or downstream payload"
    );

    let outcomes = value["outcomes"].as_array().expect("outcomes");
    let ordinals = outcomes
        .iter()
        .map(|outcome| outcome["ordinal"].as_u64().expect("ordinal"))
        .collect::<Vec<_>>();
    assert_eq!(ordinals, (0..9).collect::<Vec<_>>());
    assert!(outcomes.iter().all(|outcome| {
        outcome["document_id"].is_string()
            && outcome["lane"].is_string()
            && (outcome["failure_code"].is_null() || outcome["failure_code"].is_string())
    }));
}
