//! Independent binding contract for the reviewed digest-only visual oracle.
//! This test intentionally does not import production detector helpers.

use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const FIXTURE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const SOURCE: &str = "artifacts/parse_compat/native_visual_oracle_source.json";
const MANIFEST: &str = "data/parse/native_visual_heads.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const HEADS: [&str; 3] = ["figure", "table", "formula"];
const DOCS: [(&str, &str, u64); 3] = [
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
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && value.bytes().collect::<BTreeSet<_>>().len() > 1
}
fn forbidden(value: &Value, key: &str) -> bool {
    match value {
        Value::Mapping(m) => m
            .iter()
            .any(|(k, v)| k.as_str() == Some(key) || forbidden(v, key)),
        Value::Sequence(s) => s.iter().any(|v| forbidden(v, key)),
        _ => false,
    }
}
fn source_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate(
    fixture: &Value,
    source: &serde_json::Value,
    manifest: &Value,
    source_bytes: &[u8],
) -> Result<(), String> {
    if source["schema_version"].as_str() != Some("native-visual-oracle-source.v1")
        || source["representation"].as_str() != Some("digest_only")
        || source["review_status"].as_str() != Some("independently_reviewed")
    {
        return Err("source metadata drift".into());
    }
    if source["policy"]["promotion_state"].as_str() != Some("candidate_only")
        || source["policy"]["import_eligible"].as_bool() != Some(false)
        || source["policy"]["graph_mutation"].as_str() != Some("forbidden")
        || source["policy"]["expected_rows_not_from_sut_helpers"].as_bool() != Some(true)
    {
        return Err("source policy drift".into());
    }
    if manifest["oracle"]["source_artifact_sha256"].as_str() != Some(&source_digest(source_bytes)) {
        return Err("source artifact SHA binding drift".into());
    }
    let source_rows = source["receipts"]
        .as_array()
        .ok_or("missing source receipts")?;
    let rows = fixture["receipts"]
        .as_sequence()
        .ok_or("missing fixture receipts")?;
    if source_rows.len() != 9 || rows.len() != 9 {
        return Err("expected nine receipts".into());
    }
    let mut identities = BTreeSet::new();
    let mut source_by_identity = std::collections::BTreeMap::new();
    for source_row in source_rows {
        let id = source_row["document_id"]
            .as_str()
            .ok_or("source missing document_id")?;
        let lane = source_row["lane_id"]
            .as_str()
            .ok_or("source missing lane_id")?;
        if source_by_identity.insert((id, lane), source_row).is_some() {
            return Err(format!("duplicate source identity {id}/{lane}"));
        }
    }
    for (fixture_index, row) in rows.iter().enumerate() {
        let id = row["document_id"].as_str().ok_or("missing document_id")?;
        let lane = row["lane_id"].as_str().ok_or("missing lane_id")?;
        let key = format!("{id}/{lane}");
        if !identities.insert(key.clone()) {
            return Err(format!("duplicate identity {key}"));
        }
        let (_, expected_sha, expected_pages) = DOCS
            .iter()
            .find(|(d, _, _)| *d == id)
            .ok_or("unknown document")?;
        let source_row = source_by_identity
            .get(&(id, lane))
            .ok_or_else(|| format!("missing source identity {key}"))?;
        if !LANES.contains(&lane)
            || source_row["document_id"].as_str() != Some(id)
            || source_row["lane_id"].as_str() != Some(lane)
        {
            return Err(format!("identity mismatch {key}"));
        }
        let expected_document_index = DOCS
            .iter()
            .position(|(document, _, _)| *document == id)
            .ok_or("unknown document")?;
        let expected_lane_index = LANES
            .iter()
            .position(|candidate| *candidate == lane)
            .ok_or("unknown lane")?;
        let expected_ordinal = expected_document_index * LANES.len() + expected_lane_index;
        if fixture_index != expected_ordinal
            || source_row["ordinal"].as_u64() != Some(expected_ordinal as u64)
        {
            return Err(format!("receipt order drift {key}"));
        }
        if row["source_sha256"].as_str() != Some(*expected_sha)
            || source_row["source_sha256"].as_str() != Some(*expected_sha)
        {
            return Err(format!("checksum mismatch {key}"));
        }
        if row["page_denominator"].as_i64() != Some(*expected_pages as i64)
            || source_row["page_denominator"].as_u64() != Some(*expected_pages)
        {
            return Err(format!("page denominator mismatch {key}"));
        }
        if row["page_range"]
            .as_sequence()
            .and_then(|r| Some((r.first()?.as_i64()?, r.get(1)?.as_i64()?)))
            != Some((1, *expected_pages as i64))
        {
            return Err(format!("page range mismatch {key}"));
        }
        let heads = row["heads"].as_sequence().ok_or("missing heads")?;
        if heads.len() != 3 {
            return Err(format!("head denominator mismatch {key}"));
        }
        let mut labels = BTreeSet::new();
        for head in heads {
            let label = head["head"].as_str().ok_or("missing head label")?;
            if !HEADS.contains(&label) || !labels.insert(label) {
                return Err(format!("unknown or duplicate head {key}/{label}"));
            }
            if head["head_ordinal"].as_i64() != Some(0) {
                return Err(format!("ordinal drift {key}/{label}"));
            }
            let status = head["status"].as_str().ok_or("empty status")?;
            if !ABSENCES.contains(&status) {
                return Err(format!("open status {key}/{label}"));
            }
            for field in [
                "bbox_72dpi",
                "caption_digest",
                "content_digest",
                "callout_digest",
            ] {
                let value = head[field].as_str().ok_or("missing typed field")?;
                let absent = value == "absent";
                if absent {
                    continue;
                }
                if !digest(value) {
                    return Err(format!("invalid digest {key}/{label}/{field}"));
                }
                if ABSENCES.contains(&status) {
                    return Err(format!("digest on closed absence {key}/{label}"));
                }
            }
        }
        for sibling in ["table_cells", "formula_labels"] {
            let item = &row[sibling];
            let n = item["oracle_n"]
                .as_u64()
                .ok_or("missing sibling denominator")?;
            let status = item["status"].as_str().ok_or("empty sibling status")?;
            if n == 0 && status != "not_observed" {
                return Err(format!("silent zero {key}/{sibling}"));
            }
            if n > 0 && status == "not_observed" {
                return Err(format!(
                    "typed absence/conservation mismatch {key}/{sibling}"
                ));
            }
        }
    }
    for field in FORBIDDEN {
        if !fixture["forbidden_fields"]
            .as_sequence()
            .map(|x| x.iter().any(|v| v.as_str() == Some(field)))
            .unwrap_or(false)
            || forbidden(fixture, field)
        {
            return Err(format!("forbidden payload {field}"));
        }
    }
    if identities.len() != 9 {
        return Err("missing receipt identity".into());
    }
    if manifest["gates"]["promotion_state"].as_str() != Some("candidate") {
        return Err("manifest promotion state drift".into());
    }
    Ok(())
}

fn inputs() -> (Value, serde_json::Value, Value, Vec<u8>) {
    let bytes = fs::read(root().join(SOURCE)).expect(SOURCE);
    (
        yaml(FIXTURE),
        serde_json::from_slice(&bytes).expect(SOURCE),
        yaml(MANIFEST),
        bytes,
    )
}

#[test]
fn independent_real_oracle_binds_nine_receipts_and_twenty_seven_heads() {
    let (f, s, m, b) = inputs();
    validate(&f, &s, &m, &b).unwrap();
}

#[test]
fn mutations_fail_closed_for_identity_denominator_and_order() {
    let (fixture, source, manifest, bytes) = inputs();
    let mut pooled = fixture.clone();
    pooled["receipts"][1]["page_denominator"] = 202.into();
    assert!(validate(&pooled, &source, &manifest, &bytes).is_err());
    let mut missing = fixture.clone();
    missing["receipts"][0]["heads"]
        .as_sequence_mut()
        .unwrap()
        .pop();
    assert!(validate(&missing, &source, &manifest, &bytes).is_err());
    let mut duplicate = fixture.clone();
    duplicate["receipts"][0]["heads"][1]["head"] = "figure".into();
    assert!(validate(&duplicate, &source, &manifest, &bytes).is_err());
    let mut unknown = fixture.clone();
    unknown["receipts"][0]["heads"][0]["head"] = "chart".into();
    assert!(validate(&unknown, &source, &manifest, &bytes).is_err());
    let mut order = fixture.clone();
    order["receipts"][0]["heads"][0]["head_ordinal"] = 1.into();
    assert!(validate(&order, &source, &manifest, &bytes).is_err());
    let mut duplicate_receipt = fixture.clone();
    duplicate_receipt["receipts"][1] = duplicate_receipt["receipts"][0].clone();
    assert!(validate(&duplicate_receipt, &source, &manifest, &bytes).is_err());
    let mut range = fixture.clone();
    range["receipts"][4]["page_range"][1] = 202.into();
    assert!(validate(&range, &source, &manifest, &bytes).is_err());
    let mut source_pin = manifest.clone();
    source_pin["oracle"]["source_artifact_sha256"] = "0".repeat(64).into();
    assert!(validate(&fixture, &source, &source_pin, &bytes).is_err());
    let mut source_identity = source.clone();
    source_identity["receipts"][4]["source_sha256"] = "0".repeat(64).into();
    assert!(validate(&fixture, &source_identity, &manifest, &bytes).is_err());
}

#[test]
fn mutations_fail_closed_for_redaction_status_digest_and_sibling_zero() {
    let (fixture, source, manifest, bytes) = inputs();
    let mut raw = fixture.clone();
    raw["raw_pdf"] = "leak".into();
    assert!(validate(&raw, &source, &manifest, &bytes).is_err());
    let mut status = fixture.clone();
    status["receipts"][0]["heads"][0]["status"] = "".into();
    assert!(validate(&status, &source, &manifest, &bytes).is_err());
    let mut digest_mut = fixture.clone();
    digest_mut["receipts"][0]["heads"][0]["caption_digest"] = "placeholder".into();
    assert!(validate(&digest_mut, &source, &manifest, &bytes).is_err());
    let mut silent = fixture.clone();
    silent["receipts"][0]["table_cells"]["status"] = "unsupported".into();
    assert!(validate(&silent, &source, &manifest, &bytes).is_err());
    let mut promotion = manifest.clone();
    promotion["gates"]["promotion_state"] = "promoted".into();
    assert!(validate(&fixture, &source, &promotion, &bytes).is_err());
}
