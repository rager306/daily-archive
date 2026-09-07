//! S16 DATA contract: three independent visual heads and a digest-only PDF oracle.
//! Expected rows live in a reviewed fixture and are deliberately not produced by
//! da-parse or any layout donor helper.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_yaml::Value;

const MANIFEST: &str = "data/parse/native_visual_heads.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const S14: &str = "data/parse/layout_bakeoff.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const HEADS: [&str; 3] = ["figure", "table", "formula"];
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
const FORBIDDEN: [&str; 10] = [
    "raw_text",
    "raw_pdf",
    "pdf_bytes",
    "tei_xml",
    "image",
    "page_image",
    "assets",
    "physical_path",
    "hostname",
    "executable",
];
const ABSENCES: [&str; 10] = [
    "unsupported",
    "not_observed",
    "missing_identity",
    "checksum_mismatch",
    "malformed_bbox",
    "invalid_digest",
    "duplicate_receipt",
    "missing_receipt",
    "oversize",
    "interrupted",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |current, key| {
        current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"))
    })
}

fn strings(value: &Value, path: &[&str]) -> Vec<String> {
    at(value, path)
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|item| item.as_str().expect("DATA string").to_owned())
        .collect()
}

fn contains_key(value: &Value, forbidden: &str) -> bool {
    match value {
        Value::Mapping(map) => map
            .iter()
            .any(|(key, child)| key.as_str() == Some(forbidden) || contains_key(child, forbidden)),
        Value::Sequence(items) => items.iter().any(|item| contains_key(item, forbidden)),
        _ => false,
    }
}

fn validate(manifest: &Value, oracle: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if at(manifest, &["schema_version"]).as_str() != Some("native-visual-heads.v1") {
        errors.push("[schema] unknown native visual schema".into());
    }
    if at(manifest, &["source_authority", "manifest"]).as_str() != Some(S14)
        || at(manifest, &["source_authority", "exact_sha256_binding"]).as_str() != Some("required")
    {
        errors.push("[source-authority] S14 identity/topology authority drifted".into());
    }
    if strings(manifest, &["lanes", "required_ids"])
        != LANES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-lane] lane vocabulary changed".into());
    }
    if strings(manifest, &["heads", "closed_labels"])
        != HEADS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-head] head vocabulary changed".into());
    }
    for (path, expected) in [
        (&["source_authority", "exact_document_count"][..], 3),
        (&["source_authority", "page_denominator"][..], 202),
        (&["source_authority", "lane_page_denominator"][..], 606),
        (&["lanes", "pages_per_lane"][..], 202),
        (&["lanes", "total_lane_pages"][..], 606),
    ] {
        if at(manifest, path).as_i64() != Some(expected) {
            errors.push(format!("[denominator] {path:?} must remain {expected}"));
        }
    }
    if at(manifest, &["heads", "pooled_score"]).as_str() != Some("forbidden")
        || at(manifest, &["metrics", "pooled_metrics"]).as_str() != Some("forbidden")
        || at(manifest, &["metrics", "missing_head_denominator"]).as_str() != Some("hard_failure")
    {
        errors.push("[independence] pooled or missing-head scoring was enabled".into());
    }
    for (path, expected) in [
        (&["gates", "state"][..], "shadow"),
        (&["gates", "promotion_state"][..], "candidate"),
        (&["gates", "auto_promote"][..], "forbidden"),
        (&["gates", "donor_selection"][..], "forbidden"),
        (&["gates", "graph_mutation"][..], "forbidden"),
        (&["gates", "fulltext_composition"][..], "forbidden"),
    ] {
        if at(manifest, path).as_str() != Some(expected) {
            errors.push(format!("[policy-lock] {path:?} changed"));
        }
    }
    if at(manifest, &["gates", "import_eligible"]).as_bool() != Some(false) {
        errors.push("[policy-lock] import_eligible must remain false".into());
    }
    let docs = at(manifest, &["source_documents"])
        .as_sequence()
        .expect("source documents");
    if docs.len() != DOCUMENTS.len() {
        errors.push("[source-binding] document denominator eroded".into());
    }
    for ((id, sha, pages), doc) in DOCUMENTS.iter().zip(docs) {
        if doc["id"].as_str() != Some(*id)
            || doc["sha256"].as_str() != Some(*sha)
            || doc["pages"].as_i64() != Some(*pages)
        {
            errors.push(format!("[source-binding] {id} SHA or pages drifted"));
        }
    }
    validate_oracle(oracle, &mut errors);
    errors
}

fn validate_oracle(oracle: &Value, errors: &mut Vec<String>) {
    if at(oracle, &["schema_version"]).as_str() != Some("native-visual-oracle.v1")
        || at(oracle, &["review_status"]).as_str() != Some("independently_reviewed")
        || at(oracle, &["representation"]).as_str() != Some("digest_only")
        || at(oracle, &["coordinate_dialect"]).as_str() != Some("top_left_72dpi_points")
    {
        errors.push("[oracle-schema] review, representation, or coordinate dialect drifted".into());
    }
    for field in FORBIDDEN {
        if !strings(oracle, &["forbidden_fields"])
            .iter()
            .any(|item| item == field)
            || contains_key(oracle, field)
        {
            errors.push(format!("[redaction] forbidden oracle field {field} leaked"));
        }
    }
    let receipts = at(oracle, &["receipts"])
        .as_sequence()
        .expect("oracle receipts");
    if receipts.len() != DOCUMENTS.len() * LANES.len() {
        errors.push("[oracle-coverage] expected 9 document/lane receipts".into());
    }
    let mut keys = BTreeSet::new();
    for receipt in receipts {
        let id = receipt["document_id"].as_str().unwrap_or("");
        let lane = receipt["lane_id"].as_str().unwrap_or("");
        let key = format!("{id}/{lane}");
        if !keys.insert(key.clone()) {
            errors.push(format!("[duplicate-receipt] {key}"));
        }
        let expected_sha = DOCUMENTS
            .iter()
            .find(|(expected_id, _, _)| *expected_id == id)
            .map(|(_, sha, _)| *sha);
        let expected_pages = DOCUMENTS
            .iter()
            .find(|(expected_id, _, _)| *expected_id == id)
            .map(|(_, _, pages)| *pages);
        if expected_sha != receipt["source_sha256"].as_str()
            || !LANES.contains(&lane)
            || expected_pages != receipt["page_denominator"].as_i64()
        {
            errors.push(format!("[source-binding] invalid receipt binding {key}"));
        }
        let range = receipt["page_range"].as_sequence().expect("page range");
        if range.len() != 2 || range[0].as_i64() != Some(1) || expected_pages != range[1].as_i64() {
            errors.push(format!("[topology] invalid document page range for {key}"));
        }
        let heads = receipt["heads"].as_sequence().expect("head receipts");
        if heads.len() != HEADS.len() {
            errors.push(format!("[head-denominator] missing head row for {key}"));
        }
        let mut seen_heads = BTreeSet::new();
        for head in heads {
            let label = head["head"].as_str().unwrap_or("");
            if !HEADS.contains(&label) || !seen_heads.insert(label) {
                errors.push(format!("[head-label] unknown or duplicate head in {key}"));
            }
            if head["head_ordinal"].as_i64() != Some(0) {
                errors.push(format!("[ordinal] invalid head ordinal in {key}"));
            }
            let status = head["status"].as_str().unwrap_or("");
            if !ABSENCES.contains(&status) {
                errors.push(format!("[absence] status is not closed for {key}/{label}"));
            }
            for field in [
                "bbox_72dpi",
                "caption_digest",
                "content_digest",
                "callout_digest",
            ] {
                if head[field].as_str() != Some("absent") {
                    errors.push(format!(
                        "[typed-absence] {field} is not absent for {key}/{label}"
                    ));
                }
            }
        }
    }
}

#[test]
fn native_visual_data_contract_is_source_bound_and_independent() {
    let errors = validate(&yaml(MANIFEST), &yaml(ORACLE));
    assert!(
        errors.is_empty(),
        "S16 DATA violations:\n{}",
        errors.join("\n")
    );
}

#[test]
fn rejects_pooling_missing_denominator_unknown_label_and_checksum_drift() {
    let oracle = yaml(ORACLE);
    let mut manifest = yaml(MANIFEST);
    manifest["heads"]["pooled_score"] = Value::String("allowed".into());
    assert!(!validate(&manifest, &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["source_documents"]
        .as_sequence_mut()
        .unwrap()
        .pop();
    assert!(!validate(&manifest, &oracle).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["heads"][0]["head"] = Value::String("chart".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["source_sha256"] = Value::String("0".repeat(64));
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());
}

#[test]
fn rejects_payload_leakage_promotion_and_untyped_absence() {
    let mut oracle = yaml(ORACLE);
    oracle["raw_text"] = Value::String("must never be stored".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["gates"]["promotion_state"] = Value::String("promoted".into());
    assert!(!validate(&manifest, &yaml(ORACLE)).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["heads"][0]["status"] = Value::String("".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());
}

#[test]
fn preserves_s14_authority_and_all_three_head_denominators() {
    let s14 = yaml(S14);
    assert_eq!(s14["schema_version"].as_str(), Some("layout-bakeoff.v1"));
    assert_eq!(s14["corpus"]["page_denominator"].as_i64(), Some(202));
    assert_eq!(s14["lanes"]["required"].as_sequence().unwrap().len(), 3);
    let manifest = yaml(MANIFEST);
    assert_eq!(strings(&manifest, &["heads", "closed_labels"]).len(), 3);
    assert_eq!(
        manifest["heads"]["independent_authorities"].as_str(),
        Some("required")
    );
    assert_eq!(
        manifest["gates"]["s14_durable_evidence_mutation"].as_str(),
        Some("forbidden")
    );
}
