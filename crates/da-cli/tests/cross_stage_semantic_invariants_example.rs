use serde_json::Value;
use std::process::Command;

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn bounded_digest_keeps_families_distinct_and_redacted() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "-p",
            "da-cli",
            "--example",
            "cross_stage_semantic_invariants",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run offline cross-stage example");
    assert!(
        output.status.success(),
        "example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 digest");
    let value: Value = serde_json::from_str(&stdout).expect("JSON digest");

    assert_eq!(
        value["schema_version"],
        "da-parse.cross-stage-semantic-invariants/v1"
    );
    assert_eq!(value["identity_match"], true);
    assert_eq!(value["empty_count_not_success"], true);
    assert_eq!(value["java_p4"]["java_citation_n"], 3);
    assert_eq!(value["java_p6"]["java_figure_n"], 1);
    assert_eq!(value["java_p6"]["java_table_n"], 2);
    assert_eq!(value["java_p6"]["java_formula_n"], 1);
    assert_eq!(value["java_p7"]["java_callout_n"], 2);
    assert_eq!(value["native_sequence"]["admitted"], 2);
    assert_eq!(value["native_buckets"]["body_blocks"]["admitted"], 2);
    for head in ["figure", "table", "formula"] {
        assert!(
            value["native_visual"][head].is_object(),
            "missing visual head {head}"
        );
        assert_eq!(value["native_visual"][head]["supplied"], true);
    }

    for forbidden in [
        "pooled_n",
        "p5_success",
        "p6_success",
        "p7_success",
        "body_text",
        "source_path",
        "tei_xml",
        "pdf_path",
        "body_text",
        "caption_digest",
        "callout_digest",
    ] {
        assert!(
            !stdout.contains(forbidden),
            "forbidden field leaked: {forbidden}"
        );
    }
    let second: Value = serde_json::from_str(&stdout).expect("repeatable JSON digest");
    assert_eq!(value, second, "digest must be stable after JSON round-trip");
}
