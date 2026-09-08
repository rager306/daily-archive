//! Independent source-bound formula same-n contract.
//! Reviewed rows are typed absence; expectations are not derived from the SUT.

use da_parse::*;
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
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
const HEADS: [&str; 3] = ["figure", "table", "formula"];
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
        &fs::read_to_string(root().join(ORACLE)).expect("reviewed formula oracle must exist"),
    )
    .expect("reviewed formula oracle must be valid YAML")
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

/// Independent validation keeps identity, source and typed absence checks out of the detector.
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
        let declared = data["forbidden_fields"]
            .as_sequence()
            .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(field)));
        if !declared || forbidden(data, field) {
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
        let document = row["document_id"].as_str().unwrap_or("");
        let lane = row["lane_id"].as_str().unwrap_or("");
        let key = format!("{document}/{lane}");
        if !identities.insert(key.clone()) {
            errors.push(format!("duplicate identity {key}"));
        }
        let Some((document_index, (_, expected_sha, pages))) = DOCS
            .iter()
            .enumerate()
            .find(|(_, (id, _, _))| *id == document)
        else {
            errors.push(format!("unknown document {key}"));
            continue;
        };
        let Some(lane_index) = LANES.iter().position(|candidate| *candidate == lane) else {
            errors.push(format!("unknown lane {key}"));
            continue;
        };
        if index != document_index * LANES.len() + lane_index {
            errors.push(format!("ordinal drift {key}"));
        }
        if row["source_sha256"].as_str() != Some(*expected_sha)
            || !is_sha256(row["source_sha256"].as_str().unwrap_or(""))
        {
            errors.push(format!("checksum drift {key}"));
        }
        if row["page_denominator"].as_u64() != Some(*pages) {
            errors.push(format!("pooled denominator {key}"));
        }
        let range = row["page_range"].as_sequence();
        if range.and_then(|r| r.first()).and_then(Value::as_u64) != Some(1)
            || range.and_then(|r| r.get(1)).and_then(Value::as_u64) != Some(*pages)
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
                if value != "absent" && (!is_sha256(value) || ABSENCES.contains(&status)) {
                    errors.push(format!("invalid typed digest {key}/{label}/{field}"));
                }
            }
        }
        if heads.len() != HEADS.len() {
            errors.push(format!("head denominator drift {key}"));
        }
        if row["formula_labels"]["oracle_n"].as_i64() != Some(0)
            || row["formula_labels"]["status"].as_str() != Some("not_observed")
        {
            errors.push(format!("formula label zero is not typed absence {key}"));
        }
    }
    if identities.len() != 9 {
        errors.push("identity coverage is not 9/9".into());
    }
    errors
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn formula_status(row: &Value) -> &str {
    row["heads"]
        .as_sequence()
        .expect("heads sequence")
        .iter()
        .find(|head| head["head"].as_str() == Some("formula"))
        .and_then(|head| head["status"].as_str())
        .expect("formula status")
}

fn observation(row: &Value) -> NativeVisualObservation {
    let document = row["document_id"].as_str().expect("document identity");
    let source_sha256 = row["source_sha256"].as_str().expect("source SHA");
    let lane = row["lane_id"].as_str().expect("lane identity");
    let status = formula_status(row);
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: document.into(),
        source_sha256: source_sha256.into(),
        provider: NativeVisualProvider {
            id: format!("reviewed-formula-same-n/{lane}"),
            version: "v1".into(),
        },
        primitives: vec![],
        figure_captions: vec![],
        figure_callouts: vec![],
        table_captions: vec![],
        table_callouts: vec![],
        table_cells: vec![],
        formula_labels: vec![],
        formula_callouts: vec![],
        lane_absence: (status == "unsupported").then_some(NativeVisualAbsence::Unsupported),
    }
}

#[test]
fn nine_formula_rows_have_exact_source_bound_terminal_parity_and_denominators() {
    let data = oracle();
    let errors = validate_oracle(&data);
    assert!(
        errors.is_empty(),
        "independent oracle violations:\n{}",
        errors.join("\n")
    );

    let rows = data["receipts"].as_sequence().expect("receipts sequence");
    for row in rows {
        let document = row["document_id"].as_str().unwrap();
        let lane = row["lane_id"].as_str().unwrap();
        let pages = row["page_denominator"].as_u64().unwrap();
        let outcome = detect_formula_heads(&observation(row), Default::default())
            .unwrap_or_else(|error| panic!("{document}/{lane} ({pages} pages): {error:?}"));
        let expected = match formula_status(row) {
            "unsupported" => NativeVisualAbsence::Unsupported,
            "not_observed" => NativeVisualAbsence::NotObserved,
            other => panic!("unexpected formula status {other} for {document}/{lane}"),
        };
        match &outcome {
            NativeVisualHeadOutcome::Terminal { reason, metrics } => {
                assert_eq!(
                    *reason, expected,
                    "reason {document}/{lane} ({pages} pages)"
                );
                assert_eq!((metrics.admitted, metrics.terminal), (0, 1));
                assert_eq!(
                    (
                        metrics.caption_conservation,
                        metrics.content_conservation,
                        metrics.callout_conservation,
                        metrics.geometry_conservation,
                    ),
                    (0, 0, 0, 0),
                    "conservation {document}/{lane} ({pages} pages)"
                );
            }
            other => panic!("{document}/{lane} ({pages} pages) became formula success: {other:?}"),
        }
        let replay: NativeVisualHeadOutcome =
            serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
        assert_eq!(replay, outcome, "serde replay {document}/{lane}");
        assert_eq!(row["formula_labels"]["oracle_n"].as_i64(), Some(0));
    }
    assert_eq!(rows.len(), 9, "same-n must not pool the nine identities");
}

#[test]
fn mutations_fail_closed_for_formula_status_identity_denominator_and_typed_zero() {
    let base = oracle();

    let mut pooled = base.clone();
    pooled["receipts"][1]["page_denominator"] = 202.into();
    assert!(
        !validate_oracle(&pooled).is_empty(),
        "pooled denominator must fail"
    );

    let mut status = base.clone();
    status["receipts"][0]["heads"][2]["status"] = "formula".into();
    assert!(
        !validate_oracle(&status).is_empty(),
        "status drift must fail"
    );

    let mut missing = base.clone();
    missing["receipts"][0]["heads"]
        .as_sequence_mut()
        .unwrap()
        .retain(|head| head["head"].as_str() != Some("formula"));
    assert!(
        !validate_oracle(&missing).is_empty(),
        "missing head must fail"
    );

    let mut duplicate = base.clone();
    duplicate["receipts"][1] = duplicate["receipts"][0].clone();
    assert!(
        !validate_oracle(&duplicate).is_empty(),
        "duplicate identity must fail"
    );

    let mut unknown = base.clone();
    unknown["receipts"][0]["lane_id"] = "unknown".into();
    assert!(
        !validate_oracle(&unknown).is_empty(),
        "unknown identity must fail"
    );

    let mut checksum = base.clone();
    checksum["receipts"][0]["source_sha256"] = "0".repeat(64).into();
    assert!(
        !validate_oracle(&checksum).is_empty(),
        "checksum drift must fail"
    );

    let mut ordinal = base.clone();
    ordinal["receipts"][0]["heads"][2]["head_ordinal"] = 1.into();
    assert!(
        !validate_oracle(&ordinal).is_empty(),
        "ordinal drift must fail"
    );

    let mut silent_zero = base.clone();
    silent_zero["receipts"][0]["formula_labels"]["status"] = "unsupported".into();
    assert!(
        !validate_oracle(&silent_zero).is_empty(),
        "silent zero must fail"
    );
}
