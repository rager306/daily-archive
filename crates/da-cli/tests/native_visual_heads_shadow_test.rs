#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn runner() -> PathBuf {
    if let Some(path) = std::env::var_os("NATIVE_VISUAL_HEADS_SHADOW_BIN") {
        return PathBuf::from(path);
    }
    let debug = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .canonicalize()
        .expect("debug target directory");
    debug
        .join("examples")
        .read_dir()
        .expect("examples directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("native_visual_heads_shadow-") && !name.ends_with(".d")
                    })
                && std::os::unix::fs::PermissionsExt::mode(
                    &fs::metadata(path).expect("example metadata").permissions(),
                ) & 0o111
                    != 0
        })
        .max_by_key(|path| {
            fs::metadata(path)
                .expect("example metadata")
                .modified()
                .ok()
        })
        .expect("built native visual shadow example")
}

fn assert_redacted(value: &Value, path: &str, in_redaction: bool) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                let sensitive = [
                    "raw_lexemes",
                    "raw_pdf",
                    "raw_text",
                    "captions",
                    "tei_xml",
                    "images",
                    "assets",
                    "physical_paths",
                    "hostname",
                    "executable",
                    "fulltext",
                    "parsed_article",
                    "graph_payload",
                    "pdf_bytes",
                    "page_image",
                ]
                .contains(&key.as_str());
                if sensitive && !in_redaction {
                    panic!("sensitive field escaped redaction block at {child_path}");
                }
                if sensitive && in_redaction {
                    assert_eq!(
                        child,
                        &Value::Bool(false),
                        "redaction marker at {child_path}"
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
                "ParsedArticle",
                "FigureHead",
                "TableHead",
                "FormulaHead",
                "VisualCaption",
                "Callout",
            ] {
                assert!(
                    !text.contains(marker),
                    "forbidden marker `{marker}` at {path}"
                );
            }
        }
        _ => {}
    }
}

#[test]
fn native_visual_shadow_has_nine_terminal_lane_receipts() {
    let dir = tempdir().expect("temporary report directory");
    let report = dir.path().join("native_visual_heads_shadow.json");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let output = Command::new(runner())
        .args([
            "--manifest",
            root.join("data/parse/layout_bakeoff.yaml")
                .to_str()
                .expect("manifest"),
            "--report",
            report.to_str().expect("report path"),
            "--workers",
            "1",
            "--queue-capacity",
            "1",
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
    assert_eq!(value["schema_version"], "native-visual-heads-shadow.v1");
    assert_eq!(value["denominators"]["documents"], 3);
    assert_eq!(value["denominators"]["lanes"], 3);
    assert_eq!(value["denominators"]["pages_per_lane"], 202);
    assert_eq!(value["denominators"]["lane_pages"], 606);
    assert_eq!(value["denominators"]["receipts"], 27);
    assert_eq!(value["outcomes"].as_array().expect("outcomes").len(), 9);
    assert_eq!(
        value["outcomes"]
            .as_array()
            .expect("outcomes")
            .iter()
            .map(|outcome| outcome["heads"].as_object().expect("heads").len())
            .sum::<usize>(),
        27
    );
    assert_eq!(value["replay"]["semantic_replays"], 2);
    assert_eq!(value["replay"]["semantic_equal"], true);
    assert_eq!(value["replay"]["worker_one_equal"], true);
    assert_eq!(
        value["gate"]["state"],
        "shadow_terminal_evidence_incomplete"
    );
    assert_eq!(value["gate"]["unsupported_visible"], true);
    assert_eq!(value["gate"]["no_pooled_winner"], true);
    assert_eq!(value["policy"]["observation_mode"], "shadow");
    assert_eq!(value["policy"]["promotion_state"], "candidate_only");
    assert_eq!(value["policy"]["auto_promote"], "forbidden");
    assert_eq!(value["policy"]["donor_selection"], "forbidden");
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["policy"]["graph_mutation"], "forbidden");
    assert_eq!(value["policy"]["fulltext_composition"], "forbidden");
    assert_eq!(value["redaction"]["raw_pdf"], false);
    assert_eq!(value["redaction"]["raw_text"], false);
    assert_redacted(&value, "$", false);

    let outcomes = value["outcomes"].as_array().expect("outcomes");
    // A visual head is `unsupported` when the donor/provider cannot evaluate it
    // (capability or provision gap) and `not_observed` when it was evaluated and
    // absent. Both are first-class closed-taxonomy statuses (see
    // `NativeVisualAbsence` in the shadow example), so the invariant is not
    // "every head is not_observed". The precise invariant is stronger: every
    // head carries a known status with a bound denominator, and `unsupported`
    // may appear ONLY in an outcome that declares `failure_code ==
    // "unsupported"` — never silently inside a clean-looking outcome.
    assert!(outcomes.iter().all(|outcome| {
        let outcome_unsupported = outcome["failure_code"].as_str() == Some("unsupported");
        outcome["status"] == "terminal"
            && outcome["admitted"] == true
            && outcome["terminal"] == true
            && outcome["heads"].as_object().is_some_and(|heads| {
                heads.values().all(|head| {
                    let status = head["oracle_status"].as_str().unwrap_or("");
                    matches!(status, "not_observed" | "unsupported")
                        && (status != "unsupported" || outcome_unsupported)
                        && head["oracle_denominator"] == 1
                        && head["java_supporting_denominator"].as_u64().unwrap_or(0) > 0
                })
            })
    }));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome["failure_code"] == "unsupported")
    );
}
