//! Independent source-bound table same-n contract.
//! The reviewed YAML is parsed as data; expectations are never obtained from detector helpers.

use da_parse::*;
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const HEADS: [&str; 3] = ["figure", "table", "formula"];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const DOCS: [(&str, &str, u32); 3] = [
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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle() -> Value {
    serde_yaml::from_str(
        &fs::read_to_string(root().join(ORACLE)).expect("reviewed table oracle must exist"),
    )
    .expect("reviewed table oracle must be valid YAML")
}

fn sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn forbidden(value: &Value, key: &str) -> bool {
    match value {
        Value::Mapping(map) => map
            .iter()
            .any(|(k, v)| k.as_str() == Some(key) || forbidden(v, key)),
        Value::Sequence(items) => items.iter().any(|item| forbidden(item, key)),
        _ => false,
    }
}

/// Independent oracle validation used by both the positive binding and mutation tests.
fn validate_oracle(data: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if data["schema_version"].as_str() != Some("native-visual-oracle.v1")
        || data["review_status"].as_str() != Some("independently_reviewed")
        || data["representation"].as_str() != Some("digest_only")
        || data["expected_rows_not_from_sut_helpers"].as_bool() != Some(true)
    {
        errors.push("oracle metadata drift".into());
    }
    for field in FORBIDDEN {
        let listed = data["forbidden_fields"]
            .as_sequence()
            .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(field)));
        if !listed || forbidden(data, field) {
            errors.push(format!("forbidden payload field {field}"));
        }
    }
    let Some(rows) = data["receipts"].as_sequence() else {
        return ["missing receipts".to_owned()].into();
    };
    if rows.len() != 9 {
        errors.push(format!("expected 9 rows, got {}", rows.len()));
    }
    let mut identities = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let id = row["document_id"].as_str().unwrap_or("");
        let lane = row["lane_id"].as_str().unwrap_or("");
        let key = format!("{id}/{lane}");
        if !identities.insert(key.clone()) {
            errors.push(format!("duplicate identity {key}"));
        }
        let Some((doc_index, (_, expected_sha, pages))) =
            DOCS.iter().enumerate().find(|(_, (doc, _, _))| *doc == id)
        else {
            errors.push(format!("unknown document {key}"));
            continue;
        };
        let Some(lane_index) = LANES.iter().position(|candidate| *candidate == lane) else {
            errors.push(format!("unknown lane {key}"));
            continue;
        };
        if index != doc_index * LANES.len() + lane_index {
            errors.push(format!("ordinal drift {key}"));
        }
        if row["source_sha256"].as_str() != Some(*expected_sha)
            || !sha256(row["source_sha256"].as_str().unwrap_or(""))
        {
            errors.push(format!("checksum drift {key}"));
        }
        if row["page_denominator"].as_u64() != Some(*pages as u64) {
            errors.push(format!("pooled denominator {key}"));
        }
        let range = row["page_range"].as_sequence();
        if range.and_then(|r| r.first()).and_then(Value::as_u64) != Some(1)
            || range.and_then(|r| r.get(1)).and_then(Value::as_u64) != Some(*pages as u64)
        {
            errors.push(format!("page range drift {key}"));
        }
        let Some(heads) = row["heads"].as_sequence() else {
            errors.push(format!("missing heads {key}"));
            continue;
        };
        let mut labels = BTreeSet::new();
        for head in heads {
            let label = head["head"].as_str().unwrap_or("");
            if !HEADS.contains(&label) || !labels.insert(label) {
                errors.push(format!("unknown or duplicate head {key}/{label}"));
            }
            if head["head_ordinal"].as_u64() != Some(0) {
                errors.push(format!("head ordinal drift {key}/{label}"));
            }
            let status = head["status"].as_str().unwrap_or("");
            if !ABSENCES.contains(&status) {
                errors.push(format!("open status {key}/{label}"));
            }
            for field in [
                "bbox_72dpi",
                "caption_digest",
                "content_digest",
                "callout_digest",
            ] {
                let value = head[field].as_str().unwrap_or("");
                if value != "absent" && (!sha256(value) || ABSENCES.contains(&status)) {
                    errors.push(format!("invalid typed digest {key}/{label}/{field}"));
                }
            }
        }
        if heads.len() != HEADS.len() {
            errors.push(format!("head denominator drift {key}"));
        }
        let cells = &row["table_cells"];
        let n = cells["oracle_n"].as_i64();
        let status = cells["status"].as_str().unwrap_or("");
        if n != Some(0) || status != "not_observed" {
            errors.push(format!("table cell zero is not typed absence {key}"));
        }
    }
    if identities.len() != 9 {
        errors.push("identity coverage is not 9/9".into());
    }
    errors
}

fn primitive(
    document_id: &str,
    source_sha256: &str,
    page: u32,
    source: u32,
) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: document_id.into(),
        source_sha256: source_sha256.into(),
        provider: NativeVisualProvider {
            id: "oracle-binding".into(),
            version: "1".into(),
        },
        page_ordinal: page,
        source_ordinal: source,
        kind: NativeVisualKind::Table,
        bbox: None,
        content_digest: None,
        absence: Some(NativeVisualAbsence::NotObserved),
    }
}

fn observation(row: &Value) -> NativeVisualObservation {
    let id = row["document_id"].as_str().unwrap();
    let sha = row["source_sha256"].as_str().unwrap();
    let status = row["heads"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|head| head["head"].as_str() == Some("table"))
        .unwrap()["status"]
        .as_str()
        .unwrap();
    let (primitives, lane_absence) = match status {
        "unsupported" => (vec![], Some(NativeVisualAbsence::Unsupported)),
        "not_observed" => (vec![], None),
        other => panic!("table fixture unexpectedly needs positive binding: {other}"),
    };
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: id.into(),
        source_sha256: sha.into(),
        provider: NativeVisualProvider {
            id: "oracle-binding".into(),
            version: "1".into(),
        },
        primitives,
        figure_captions: vec![],
        figure_callouts: vec![],
        table_captions: vec![],
        table_callouts: vec![],
        table_cells: vec![],
        formula_labels: vec![],
        formula_callouts: vec![],
        lane_absence,
    }
}

#[test]
fn nine_table_rows_have_exact_source_bound_terminal_parity_and_denominators() {
    let data = oracle();
    let errors = validate_oracle(&data);
    assert!(
        errors.is_empty(),
        "independent oracle violations:\n{}",
        errors.join("\n")
    );
    for row in data["receipts"].as_sequence().unwrap() {
        let outcome = detect_table_heads(&observation(row), Default::default()).unwrap();
        let status = row["heads"]
            .as_sequence()
            .unwrap()
            .iter()
            .find(|head| head["head"].as_str() == Some("table"))
            .unwrap()["status"]
            .as_str()
            .unwrap();
        let expected = match status {
            "unsupported" => NativeVisualAbsence::Unsupported,
            "not_observed" => NativeVisualAbsence::NotObserved,
            _ => unreachable!(),
        };
        assert!(
            matches!(&outcome, NativeVisualHeadOutcome::Terminal { reason, metrics }
            if *reason == expected && metrics.admitted == 0 && metrics.terminal == 1),
            "{} / {}: expected Terminal::{expected:?}, got {outcome:?}",
            row["document_id"].as_str().unwrap(),
            row["lane_id"].as_str().unwrap()
        );
        assert_eq!(
            row["page_denominator"].as_u64(),
            Some(
                DOCS.iter()
                    .find(|(id, _, _)| *id == row["document_id"].as_str().unwrap())
                    .unwrap()
                    .2 as u64
            )
        );
        assert_eq!(row["table_cells"]["oracle_n"].as_i64(), Some(0));
        assert_eq!(row["table_cells"]["status"].as_str(), Some("not_observed"));
    }
}

#[test]
fn mutations_fail_closed_for_status_identity_denominator_and_typed_zero() {
    let mut pooled = oracle();
    pooled["receipts"][1]["page_denominator"] = 202.into();
    assert!(
        !validate_oracle(&pooled).is_empty(),
        "mutation must fail closed: pooled 202"
    );

    let mut wrong_status = oracle();
    wrong_status["receipts"][0]["heads"][1]["status"] = "".into();
    assert!(
        !validate_oracle(&wrong_status).is_empty(),
        "mutation must fail closed: wrong status"
    );

    let mut duplicate = oracle();
    duplicate["receipts"][1]["document_id"] = duplicate["receipts"][0]["document_id"].clone();
    duplicate["receipts"][1]["lane_id"] = duplicate["receipts"][0]["lane_id"].clone();
    assert!(
        !validate_oracle(&duplicate).is_empty(),
        "mutation must fail closed: duplicate identity"
    );

    let mut unknown = oracle();
    unknown["receipts"][0]["lane_id"] = "unknown".into();
    assert!(
        !validate_oracle(&unknown).is_empty(),
        "mutation must fail closed: unknown identity"
    );

    let mut ordinal = oracle();
    ordinal["receipts"][0]["heads"][1]["head_ordinal"] = 1.into();
    assert!(
        !validate_oracle(&ordinal).is_empty(),
        "mutation must fail closed: bad ordinal"
    );

    let mut range = oracle();
    range["receipts"][4]["page_range"][1] = 202.into();
    assert!(
        !validate_oracle(&range).is_empty(),
        "mutation must fail closed: bad range"
    );

    let mut checksum = oracle();
    checksum["receipts"][0]["source_sha256"] = "0".repeat(64).into();
    assert!(
        !validate_oracle(&checksum).is_empty(),
        "mutation must fail closed: bad checksum"
    );

    let mut silent_zero = oracle();
    silent_zero["receipts"][0]["table_cells"]["status"] = "unsupported".into();
    assert!(
        !validate_oracle(&silent_zero).is_empty(),
        "mutation must fail closed: silent table zero"
    );

    let mut payload = oracle();
    payload["raw_pdf"] = "leak".into();
    assert!(
        !validate_oracle(&payload).is_empty(),
        "mutation must fail closed: raw payload"
    );
}

#[test]
fn terminal_receipts_replay_exactly_and_never_serialize_forbidden_payloads() {
    let data = oracle();
    for row in data["receipts"].as_sequence().unwrap() {
        let outcome = detect_table_heads(&observation(row), Default::default()).unwrap();
        let bytes = serde_json::to_vec(&outcome).unwrap();
        let replay: NativeVisualHeadOutcome = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(outcome, replay);
        let text = String::from_utf8_lossy(&bytes);
        for marker in FORBIDDEN {
            assert!(!text.contains(marker), "forbidden marker {marker} leaked");
        }
    }
}

#[test]
fn malformed_observation_inputs_fail_closed_before_detection() {
    let data = oracle();
    let row = &data["receipts"][0];
    let mut input = observation(row);
    input.source_sha256 = "not-a-sha".into();
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::InvalidDigest)
    );
    let mut input = observation(row);
    input.lane_absence = None;
    input.primitives.push(primitive(
        row["document_id"].as_str().unwrap(),
        row["source_sha256"].as_str().unwrap(),
        1,
        0,
    ));
    assert!(input.validate(Default::default()).is_ok());
    let outcome = detect_table_heads(&input, Default::default()).unwrap();
    assert!(matches!(outcome, NativeVisualHeadOutcome::Terminal { .. }));
}
