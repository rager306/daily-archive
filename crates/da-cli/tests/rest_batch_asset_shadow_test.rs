use serde_json::Value;
use std::{fs, process::Command};
use tempfile::tempdir;

fn assert_redacted(value: &Value) {
    fn visit(value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if [
                        "raw_pdf",
                        "raw_text",
                        "raw_tei",
                        "tei_xml",
                        "physical_path",
                        "hostname",
                        "credentials",
                    ]
                    .contains(&key.as_str())
                    {
                        assert_eq!(
                            child,
                            &Value::Bool(false),
                            "redaction marker must be false: {key}"
                        );
                    }
                    visit(child);
                }
            }
            Value::Array(items) => items.iter().for_each(visit),
            Value::String(text) => {
                for marker in [
                    "http://",
                    "https://",
                    "127.0.0.1",
                    "synthetic-pdf:",
                    "<TEI",
                    ".pdf",
                    "/tmp/",
                    "/root/",
                ] {
                    assert!(
                        !text.contains(marker),
                        "forbidden report marker leaked: {marker}"
                    );
                }
            }
            _ => {}
        }
    }
    visit(value);
}

#[test]
fn rest_batch_shadow_is_source_bound_bounded_and_digest_only() {
    let dir = tempdir().expect("report directory");
    let report = dir.path().join("rest_batch_asset_shadow.json");
    let output = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "-p",
            "da-cli",
            "--example",
            "rest_batch_asset_shadow",
            "--offline",
            "--",
            "--report",
            report.to_str().expect("report path"),
        ])
        .output()
        .expect("run offline shadow example");
    assert!(
        output.status.success(),
        "shadow runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(&report).expect("shadow report");
    let value: Value = serde_json::from_str(&text).expect("valid report JSON");
    assert_eq!(value["schema_version"], "rest-batch-asset-shadow.v1");
    for key in [
        "java_source_sha256",
        "demand_oracle_source_sha256",
        "data_sha256",
    ] {
        assert_eq!(
            value["source_bindings"][key].as_str().map(str::len),
            Some(64)
        );
    }
    // GAR-006: the reviewed demand inventory is the referenced population; the
    // executed population is the synthetic loopback run and says so.
    assert_eq!(value["reference_population"]["role"], "identity_only");
    assert_eq!(value["reference_population"]["documents"], 3);
    assert_eq!(value["reference_population"]["inventory"], 19);
    assert_eq!(value["reference_population"]["demand_review"], 19);
    assert_eq!(value["reference_population"]["executable"], 4);
    assert_eq!(value["reference_population"]["corpus_lane_pages"], 606);
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["executed_population"]["documents"], 3);
    assert_eq!(value["executed_population"]["operations"], 3);
    assert_eq!(
        value["executed_population"]["source_kind"],
        "synthetic_loopback_fixture"
    );
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
    assert_eq!(value["evaluated_population"]["documents"], 0);
    assert_eq!(value["outcomes"]["quality_claims"], serde_json::json!([]));
    let operations = value["operations"]
        .as_array()
        .expect("operation dispositions");
    assert_eq!(operations.len(), 4);
    assert!(
        operations[..3]
            .iter()
            .all(|row| row["disposition"] == "existing_typed_transport")
    );
    assert_eq!(
        operations[3]["disposition"],
        "existing_local_batch_composition"
    );
    let summaries = &value["run_summary"];
    assert_eq!(summaries["write_through"]["admitted"], 3);
    assert_eq!(summaries["write_through"]["completed"], 3);
    assert_eq!(summaries["write_through"]["backend_calls"], 3);
    assert_eq!(summaries["write_through"]["raw_writes"], 3);
    assert_eq!(summaries["write_through"]["projector_writes"], 3);
    assert_eq!(summaries["read_only_replay"]["backend_calls"], 0);
    assert_eq!(summaries["read_only_replay"]["raw_hits"], 3);
    assert_eq!(summaries["off_worker_one"]["backend_calls"], 3);
    assert_eq!(summaries["off_worker_one"]["raw_hits"], 0);
    assert!(
        summaries["write_through"]["max_in_flight_observed"]
            .as_u64()
            .expect("in-flight gauge")
            <= 2
    );
    assert!(
        summaries["write_through"]["max_queue_depth_observed"]
            .as_u64()
            .expect("queue gauge")
            <= 3
    );
    assert_eq!(
        value["per_document"]
            .as_array()
            .expect("terminal rows")
            .len(),
        3
    );
    assert_eq!(value["cache"]["replay_equal"], true);
    assert_eq!(value["cache"]["worker_one_equal"], true);
    assert_eq!(value["cache"]["raw_tei_published_before_projection"], true);
    assert_eq!(
        value["cache"]["projector_reprojection_without_backend"],
        true
    );
    assert_eq!(value["cache"]["identity_mismatch"], "error_not_miss");
    assert_eq!(value["policy"]["implemented_new_endpoints"], 0);
    assert_eq!(value["policy"]["generic_arbitrary_route"], "forbidden");
    assert_eq!(value["policy"]["production_route"], "forbidden");
    assert_eq!(value["policy"]["import_eligible"], false);
    assert_eq!(value["policy"]["graph_write"], "forbidden");
    assert_eq!(value["policy"]["auto_promote"], "forbidden");
    assert_redacted(&value);
}
