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

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        && value.chars().any(|c| c != '0')
        && value
            .chars()
            .any(|c| c != value.chars().next().unwrap_or('0'))
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
    let oracle = dir.path().join("document-segmentation-oracle-source.json");
    let mut args = vec![
        "--layout-manifest".to_owned(),
        layout_manifest
            .to_str()
            .expect("layout manifest path")
            .to_owned(),
        "--segmentation-manifest".to_owned(),
        segmentation_manifest
            .to_str()
            .expect("segmentation manifest path")
            .to_owned(),
        "--report".to_owned(),
        report.to_str().expect("report path").to_owned(),
        "--oracle-source".to_owned(),
        oracle.to_str().expect("oracle path").to_owned(),
    ];
    let runtime_pdfalto = repo_root.join(".gsd/tmp/pdfalto-segmentation-runtime");
    if runtime_pdfalto.is_file() {
        args.extend([
            "--pdfalto".to_owned(),
            ".gsd/tmp/pdfalto-segmentation-runtime".to_owned(),
        ]);
    }
    let output = Command::new(runner())
        .args(args)
        .current_dir(&repo_root)
        .output()
        .expect("run feature-gated shadow example");
    assert!(
        output.status.success(),
        "runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(&report).expect("canonical report");
    let value: Value = serde_json::from_str(&text).expect("valid JSON report");
    let oracle_text = fs::read_to_string(&oracle).expect("oracle source report");
    let oracle_value: Value = serde_json::from_str(&oracle_text).expect("valid oracle source JSON");
    assert_eq!(
        oracle_value["schema_version"],
        "document-segmentation-oracle-source.v1"
    );
    let receipts = oracle_value["receipts"]
        .as_array()
        .expect("oracle receipts");
    assert_eq!(receipts.len(), 9);
    let expected_keys = [
        ("hub-2507.19457", "pdfalto"),
        ("hub-2507.19457", "pdf_inspector_positioned"),
        ("hub-2507.19457", "pdf_inspector_positioned_xy_cut"),
        ("rlm-2512.24601", "pdfalto"),
        ("rlm-2512.24601", "pdf_inspector_positioned"),
        ("rlm-2512.24601", "pdf_inspector_positioned_xy_cut"),
        ("reca-2605.26525v1", "pdfalto"),
        ("reca-2605.26525v1", "pdf_inspector_positioned"),
        ("reca-2605.26525v1", "pdf_inspector_positioned_xy_cut"),
    ];
    let actual_keys = receipts
        .iter()
        .map(|receipt| {
            (
                receipt["document_id"].as_str().expect("document id"),
                receipt["lane"].as_str().expect("lane"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_keys, expected_keys);
    let expected_source_sha = [
        (
            "hub-2507.19457",
            "ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
            96,
        ),
        (
            "rlm-2512.24601",
            "8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca",
            43,
        ),
        (
            "reca-2605.26525v1",
            "a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8",
            63,
        ),
    ];
    assert!(receipts.iter().all(|receipt| {
        let (document_id, source_sha256, page_count) = expected_source_sha
            .iter()
            .find(|(id, _, _)| receipt["document_id"] == *id)
            .expect("source-bound document");
        receipt["status"] == "pass"
            && receipt["document_id"] == *document_id
            && receipt["source_sha256"] == *source_sha256
            && receipt["page_count"] == *page_count
            && receipt["page_ordinal"]
                .as_u64()
                .is_some_and(|page| page > 0)
            && receipt["token_ordinal"].as_u64().is_some()
            && receipt["token_digest"].as_str().is_some_and(is_digest)
            && receipt["boundary_digest"].as_str().is_some_and(is_digest)
            && receipt["section_ordinal"].as_u64().is_some()
            && receipt["section_boundary_digest"]
                .as_str()
                .is_some_and(is_digest)
    }));
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
