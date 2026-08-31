//! S15 DATA contract gate: segmentation manifest + independent redacted oracle.
//! The oracle is intentionally decoded as generic DATA here; it must not call
//! native segmentation or any S13 donor helper.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_yaml::Value;

const MANIFEST: &str = "data/parse/document_segmentation.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml";
const S14: &str = "data/parse/layout_bakeoff.yaml";
const REGIONS: [&str; 5] = ["header", "body", "references", "annex", "page_furniture"];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const DOCUMENTS: [(&str, &str, i64); 3] = [
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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn scalar<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |current, key| {
        current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"))
    })
}

fn strings(value: &Value, path: &[&str]) -> Vec<String> {
    scalar(value, path)
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|v| v.as_str().expect("DATA string").to_owned())
        .collect()
}

fn expect_string(errors: &mut Vec<String>, value: &Value, path: &[&str], expected: &str) {
    if scalar(value, path).as_str() != Some(expected) {
        errors.push(format!("[data-drift] {path:?} must remain {expected}"));
    }
}

fn expect_i64(errors: &mut Vec<String>, value: &Value, path: &[&str], expected: i64) {
    if scalar(value, path).as_i64() != Some(expected) {
        errors.push(format!("[data-drift] {path:?} must remain {expected}"));
    }
}

fn validate_manifest(manifest: &Value, oracle: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if scalar(manifest, &["schema_version"]).as_str() != Some("document-segmentation.v1") {
        errors.push("[schema] unknown segmentation schema".into());
    }
    for (path, expected) in [
        (&["policy_locks", "observation_mode"][..], "shadow"),
        (&["policy_locks", "promotion_state"][..], "candidate"),
        (&["policy_locks", "auto_promote"][..], "forbidden"),
        (&["policy_locks", "donor_selection"][..], "forbidden"),
    ] {
        if scalar(manifest, path).as_str() != Some(expected) {
            errors.push(format!("[policy-lock] {path:?} changed"));
        }
    }
    if scalar(manifest, &["policy_locks", "import_eligible"]).as_bool() != Some(false) {
        errors.push("[policy-lock] import_eligible must remain false".into());
    }

    if strings(manifest, &["regions", "closed_labels"])
        != REGIONS
            .iter()
            .map(|label| (*label).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-region] closed region vocabulary changed".into());
    }
    if strings(manifest, &["lanes", "required_ids"])
        != LANES
            .iter()
            .map(|lane| (*lane).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-lane] required lane vocabulary changed".into());
    }
    expect_i64(&mut errors, manifest, &["source_corpus", "corpus_n"], 3);
    expect_i64(&mut errors, manifest, &["source_corpus", "page_sum"], 202);
    expect_i64(&mut errors, manifest, &["lanes", "pages_per_lane"], 202);
    expect_i64(&mut errors, manifest, &["lanes", "total_lane_pages"], 606);
    expect_i64(&mut errors, manifest, &["evaluation", "required_rows"], 45);
    expect_string(
        &mut errors,
        manifest,
        &["evaluation", "row_formula"],
        "3_lanes_x_1_reviewed_genre_x_5_regions",
    );
    if scalar(manifest, &["evaluation", "pooled_metrics"]).as_str() != Some("forbidden") {
        errors.push("[pooled-metrics] pooling must be forbidden".into());
    }
    if scalar(manifest, &["source_bound_genres", "stratification_only"]).as_bool() != Some(true)
        || scalar(manifest, &["source_bound_genres", "classifier_branching"]).as_str()
            != Some("forbidden")
        || scalar(
            manifest,
            &["source_bound_genres", "reviewed_annotation_required"],
        )
        .as_bool()
            != Some(true)
    {
        errors.push("[genre-policy] genre must remain reviewed stratification only".into());
    }

    let docs = scalar(manifest, &["source_corpus", "documents"])
        .as_sequence()
        .unwrap();
    if docs.len() != DOCUMENTS.len() {
        errors.push("[source-binding] source document denominator eroded".into());
    }
    for ((id, sha, pages), doc) in DOCUMENTS.iter().zip(docs) {
        if doc["id"].as_str() != Some(*id)
            || doc["source_sha256"].as_str() != Some(*sha)
            || doc["pages"].as_i64() != Some(*pages)
        {
            errors.push(format!(
                "[source-binding] document {id} drifted from S13 authority"
            ));
        }
    }

    let rows = scalar(manifest, &["evaluation", "matrix_axes"])
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if rows != ["lane", "source_bound_genre", "region"] {
        errors.push("[matrix] evaluation axes are incomplete or reordered".into());
    }

    let forbidden = strings(manifest, &["oracle", "forbidden_fields"]);
    for required in [
        "raw_text",
        "pdf_bytes",
        "tei_xml",
        "page_image",
        "physical_path",
    ] {
        if !forbidden.iter().any(|field| field == required) {
            errors.push(format!("[redaction] oracle no longer forbids {required}"));
        }
    }

    validate_oracle(oracle, &mut errors);
    errors
}

fn validate_oracle(oracle: &Value, errors: &mut Vec<String>) {
    if scalar(oracle, &["schema_version"]).as_str() != Some("segmentation-oracle.v1") {
        errors.push("[oracle-schema] wrong oracle schema".into());
    }
    let forbidden = strings(oracle, &["forbidden_fields"]);
    for forbidden_field in [
        "raw_text",
        "pdf_bytes",
        "tei_xml",
        "page_image",
        "physical_path",
    ] {
        if !forbidden.iter().any(|field| field == forbidden_field) {
            errors.push(format!(
                "[raw-payload-leakage] {forbidden_field} not forbidden"
            ));
        }
    }
    let mut keys = BTreeSet::new();
    let receipts = scalar(oracle, &["receipts"]).as_sequence().unwrap();
    if receipts.len() != 9 {
        errors.push(
            "[oracle-coverage] expected one independent receipt for each document/lane".into(),
        );
    }
    for receipt in receipts {
        let id = receipt["document_id"].as_str().unwrap_or("");
        let lane = receipt["lane_id"].as_str().unwrap_or("");
        let key = format!("{id}/{lane}");
        if !keys.insert(key.clone()) {
            errors.push(format!("[duplicate-oracle-key] {key}"));
        }
        if !DOCUMENTS.iter().any(|(expected, _, _)| *expected == id) || !LANES.contains(&lane) {
            errors.push(format!("[oracle-binding] unknown receipt {key}"));
        }
        let pages = receipt["page_ordinals"].as_sequence().unwrap();
        if pages.len() != 2 || pages[0].as_i64() >= pages[1].as_i64() {
            errors.push(format!("[overlapping-range] invalid page range for {key}"));
        }
        let token = &receipt["token_receipts"][0];
        if !REGIONS
            .iter()
            .any(|region| token["region"].as_str() == Some(*region))
        {
            errors.push(format!("[unknown-region] oracle token in {key}"));
        }
        if token["token_digest"].as_str().is_none_or(|s| s.len() != 64) {
            errors.push(format!("[digest-mismatch] token digest for {key}"));
        }
        let section = &receipt["section_receipts"][0];
        if !["body", "annex"].contains(&section["parent_region"].as_str().unwrap_or("")) {
            errors.push(format!("[section-parent] invalid parent for {key}"));
        }
    }
}

#[test]
fn s15_data_contract_is_complete_and_source_bound() {
    let manifest = yaml(MANIFEST);
    let oracle = yaml(ORACLE);
    let errors = validate_manifest(&manifest, &oracle);
    assert!(
        errors.is_empty(),
        "S15 DATA violations:\n{}",
        errors.join("\n")
    );
}

#[test]
fn rejects_unknown_labels_overlap_and_denominator_erosion() {
    let mut manifest = yaml(MANIFEST);
    manifest["regions"]["closed_labels"]
        .as_sequence_mut()
        .unwrap()
        .push(Value::String("figure".into()));
    assert!(!validate_manifest(&manifest, &yaml(ORACLE)).is_empty());

    let mut oracle = yaml(ORACLE);
    let duplicate = oracle["receipts"][0].clone();
    oracle["receipts"]
        .as_sequence_mut()
        .unwrap()
        .push(duplicate);
    assert!(!validate_manifest(&yaml(MANIFEST), &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["lanes"]["total_lane_pages"] = Value::Number(605.into());
    assert!(!validate_manifest(&manifest, &yaml(ORACLE)).is_empty());
}

#[test]
fn rejects_pooling_payload_leakage_and_promotion() {
    let mut manifest = yaml(MANIFEST);
    manifest["evaluation"]["pooled_metrics"] = Value::String("allowed".into());
    assert!(!validate_manifest(&manifest, &yaml(ORACLE)).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["forbidden_fields"]
        .as_sequence_mut()
        .unwrap()
        .retain(|field| field.as_str() != Some("raw_text"));
    assert!(!validate_manifest(&yaml(MANIFEST), &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["policy_locks"]["promotion_state"] = Value::String("promoted".into());
    assert!(!validate_manifest(&manifest, &yaml(ORACLE)).is_empty());
}

#[test]
fn preserves_s14_authority_and_declares_opaque_visual_tokens() {
    let s14 = yaml(S14);
    assert_eq!(s14["schema_version"].as_str(), Some("layout-bakeoff.v1"));
    assert_eq!(s14["corpus"]["page_denominator"].as_i64(), Some(202));
    assert_eq!(s14["lanes"]["required"].as_sequence().unwrap().len(), 3);
    let manifest = yaml(MANIFEST);
    assert_eq!(
        strings(&manifest, &["regions", "opaque_token_kinds"]),
        vec!["Formula".to_owned(), "Image".to_owned()]
    );
    assert_eq!(
        manifest["ownership"]["figures"].as_str(),
        Some("out_of_scope")
    );
    assert_eq!(
        manifest["ownership"]["tables"].as_str(),
        Some("out_of_scope")
    );
    assert_eq!(
        manifest["ownership"]["formulas"].as_str(),
        Some("out_of_scope")
    );
    assert_eq!(
        manifest["ownership"]["fulltext_composition"].as_str(),
        Some("forbidden")
    );
}
