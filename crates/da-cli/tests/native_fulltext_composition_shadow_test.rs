#![cfg(feature = "pdf-layout")]

use serde_json::Value;
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn runner() -> PathBuf {
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
                        name.starts_with("native_fulltext_composition_shadow-")
                            && !name.ends_with(".d")
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
        .expect("built native fulltext shadow example")
}

fn assert_redacted(value: &Value, path: &str) {
    let sensitive = [
        "raw_tei",
        "raw_pdf",
        "raw_text",
        "names",
        "captions",
        "references",
        "physical_paths",
        "hostname",
        "executable",
        "secrets",
        "pdf_bytes",
        "graph_payload",
        "fulltext",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                if sensitive.contains(&key.as_str()) {
                    assert_eq!(
                        child,
                        &Value::Bool(false),
                        "redaction marker at {child_path}"
                    );
                }
                assert_redacted(child, &child_path);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_redacted(child, &format!("{path}[{index}]"));
            }
        }
        Value::String(text) => {
            for marker in [
                "<TEI",
                "<tei",
                "data/sources",
                "file://",
                "/root/",
                ".pdf",
                "ParsedArticle",
                "graph_payload",
                "hostname",
                "executable",
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
fn source_bound_shadow_report_is_terminal_replayed_and_quarantined() {
    let dir = tempdir().expect("temporary report directory");
    let report = dir.path().join("native_fulltext_composition_shadow.json");
    let output = Command::new(runner())
        .args([
            "--report",
            report.to_str().expect("report path"),
            "--workers",
            "1",
            "--queue-capacity",
            "1",
        ])
        .output()
        .expect("run feature-gated offline example");
    assert!(
        output.status.success(),
        "runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(&report).expect("canonical report");
    let value: Value = serde_json::from_str(&text).expect("valid JSON report");
    assert_eq!(
        value["schema_version"],
        "native-fulltext-composition-shadow.v1"
    );
    assert_eq!(value["data_schema"], "native-fulltext-composition.v1");
    assert_eq!(value["source_identity"]["identity_kind"], "synthetic_clone");
    assert_eq!(value["source_identity"]["unique_source_sha256"], 1);
    // GAR-006: the referenced corpus identity is separate from the executed
    // population and from the evaluated population, so a 3-document / 27-head
    // reference cannot be read as this synthetic run's execution scope.
    assert_eq!(value["reference_population"]["role"], "identity_only");
    // S03/T04: the semantic-field block must be non-vacuous and independently
    // observed in the emitted bytes; a zero admitted count would make the block
    // an empty proof (the S02/T05 `required_targets == 0` lesson).
    let semantic = &value["semantic_fields"];
    for class in [
        "header_titles",
        "header_emails",
        "header_ids",
        "author_affiliations",
        "citation_fields",
    ] {
        assert_eq!(
            semantic["ledger"][format!("admitted_{class}")],
            1,
            "admitted_{class} must be non-vacuous"
        );
        assert_eq!(
            semantic["ledger"][format!("conserved_{class}")],
            1,
            "conserved_{class} must match the admitted count"
        );
    }
    for observed in [
        "header_title",
        "header_email",
        "header_id",
        "author_affiliation",
        "citation_role_value",
    ] {
        assert_eq!(
            semantic["observed"][observed], true,
            "the emitted bytes must carry the {observed} sentinel"
        );
    }
    assert_eq!(semantic["observed"]["observed_fields"], 5);
    assert_eq!(value["reference_population"]["documents"], 3);
    assert_eq!(value["reference_population"]["lanes"], 3);
    assert_eq!(value["reference_population"]["referent_rows"], 9);
    assert_eq!(value["reference_population"]["s12_named"], 3);
    assert_eq!(value["reference_population"]["s15_terminal"], 9);
    assert_eq!(value["reference_population"]["s16_visual_heads"], 27);
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["executed_population"]["documents"], 1);
    assert_eq!(value["executed_population"]["document_identities"], 3);
    assert_eq!(value["executed_population"]["objects"], 9);
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
    assert_eq!(value["evaluated_population"]["documents"], 0);
    assert_eq!(value["execution"]["admitted"], 9);
    assert_eq!(value["execution"]["terminal"], 9);
    assert_eq!(value["outcomes"].as_array().expect("outcomes").len(), 9);
    assert_eq!(value["replay"]["semantic_replays"], 2);
    assert_eq!(value["replay"]["semantic_equal"], true);
    assert_eq!(value["replay"]["worker_one_equal"], true);
    assert_eq!(value["conservation"]["admitted_equals_terminal"], true);
    assert_eq!(value["conservation"]["all_rows_terminal"], true);
    assert_eq!(value["conservation"]["no_silent_loss"], true);
    assert_eq!(value["policy"]["native_segmentation_usage"], 0);
    assert_eq!(value["policy"]["native_visual_usage"], 0);
    assert_eq!(value["policy"]["output_affecting_learning_selection"], 0);
    assert_eq!(value["policy"]["auto_promote"], "forbidden");
    assert_eq!(value["policy"]["production_route"], "forbidden");
    assert_eq!(value["policy"]["graph_mutation"], "forbidden");
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["policy"]["pooled_winner"], "forbidden");
    assert_eq!(value["typed_failures"]["invalid_evidence"], 3);
    assert_eq!(value["typed_failures"]["unsupported"], 6);
    assert_eq!(value["typed_failures"]["not_observed"], 6);
    let outcomes = value["outcomes"].as_array().expect("outcomes");
    assert!(outcomes.iter().all(|row| {
        row["status"] == "terminal"
            && row["truncated"] == false
            && row["native_components"] == 2
            && row["fallback_components"] == 5
            && row["dispositions"]
                .as_array()
                .is_some_and(|items| items.len() == 7)
            && row["semantic_fields"] == *semantic
    }));
    let components: HashSet<&str> = outcomes[0]["dispositions"]
        .as_array()
        .expect("component dispositions")
        .iter()
        .map(|item| item["component"].as_str().expect("component wire name"))
        .collect();
    assert_eq!(
        components,
        HashSet::from([
            "header",
            "references",
            "document_segmentation",
            "figure",
            "table",
            "formula",
            "learning_policy",
        ])
    );
    let dispositions: HashSet<&str> = outcomes[0]["dispositions"]
        .as_array()
        .expect("component dispositions")
        .iter()
        .map(|item| item["disposition"].as_str().expect("disposition wire name"))
        .collect();
    assert_eq!(
        dispositions,
        HashSet::from(["passed", "invalid_evidence", "unsupported", "not_observed"])
    );
    assert!(!text.contains("documentsegmentation"));
    assert!(!text.contains("invalidevidence"));
    assert!(!text.contains("notobserved"));
    assert!(!text.contains("learningpolicy"));
    assert_redacted(&value, "$");
    assert!(
        !text.contains("<TEI") && !text.contains("body bytes") && !text.contains("Reference one")
    );
}
