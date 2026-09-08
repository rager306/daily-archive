use da_parse::*;
use serde_yaml::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle() -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(ORACLE)).unwrap()).unwrap()
}

fn bbox(x: f64) -> LayoutBBox {
    LayoutBBox::new(x, 1.0, x + 10.0, 11.0).unwrap()
}

fn provider() -> NativeVisualProvider {
    NativeVisualProvider {
        id: "reviewed-provider".into(),
        version: "v1".into(),
    }
}

fn primitive(kind: NativeVisualKind, page: u32, source: u32, x: f64) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: "doc-1".into(),
        source_sha256: SHA.into(),
        provider: provider(),
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(bbox(x)),
        content_digest: Some(SHA.into()),
        absence: None,
    }
}

fn link(page: u32, source: u32, digest: Option<&str>) -> NativeVisualDigestReceipt {
    NativeVisualDigestReceipt {
        page_ordinal: page,
        source_ordinal: source,
        digest: digest.map(str::to_owned),
        absence: digest
            .is_none()
            .then_some(NativeVisualAbsence::MissingReceipt),
    }
}

fn mixed() -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: "doc-1".into(),
        source_sha256: SHA.into(),
        provider: provider(),
        primitives: vec![
            primitive(NativeVisualKind::Image, 3, 8, 1.0),
            primitive(NativeVisualKind::Table, 5, 2, 20.0),
            primitive(NativeVisualKind::Formula, 7, 4, 40.0),
        ],
        figure_captions: vec![link(3, 8, Some(SHA))],
        figure_callouts: vec![link(3, 8, None)],
        table_captions: vec![link(5, 2, Some(SHA))],
        table_callouts: vec![link(5, 2, Some(SHA))],
        table_cells: vec![
            NativeTableCellReceipt {
                row: 0,
                column: 0,
                digest: Some(SHA.into()),
                absence: None,
            },
            NativeTableCellReceipt {
                row: 0,
                column: 1,
                digest: Some(SHA.into()),
                absence: None,
            },
        ],
        formula_labels: vec![link(7, 4, Some(SHA))],
        formula_callouts: vec![link(7, 4, None)],
        lane_absence: None,
    }
}

fn mixed_with_two_tables() -> NativeVisualObservation {
    let mut input = mixed();
    input
        .primitives
        .push(primitive(NativeVisualKind::Table, 9, 5, 60.0));
    input.table_captions.push(link(9, 5, Some(&"b".repeat(64))));
    input.table_callouts.push(link(9, 5, Some(&"c".repeat(64))));
    input
}

fn reviewed(document_id: &str, source_sha256: &str, status: &str) -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: document_id.into(),
        source_sha256: source_sha256.into(),
        provider: provider(),
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
fn reviewed_oracle_rows_run_all_three_independent_heads() {
    let data = oracle();
    assert_eq!(data["expected_rows_not_from_sut_helpers"], true);
    let rows = data["receipts"].as_sequence().unwrap();
    assert_eq!(rows.len(), 9);
    for row in rows {
        let expected_pages = match row["document_id"].as_str().unwrap() {
            "hub-2507.19457" => 96,
            "rlm-2512.24601" => 43,
            "reca-2605.26525v1" => 63,
            other => panic!("unknown reviewed document: {other}"),
        };
        assert_eq!(row["page_denominator"].as_i64(), Some(expected_pages));
        let input = reviewed(
            row["document_id"].as_str().unwrap(),
            row["source_sha256"].as_str().unwrap(),
            row["heads"][0]["status"].as_str().unwrap(),
        );
        let receipt = run_visual_heads_sequence(&input, &Default::default());
        assert_eq!(receipt.status, "pass");
        assert_eq!(
            receipt.identity.source_sha256,
            row["source_sha256"].as_str().unwrap()
        );
        assert_eq!(
            receipt
                .steps
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["validate_observation", "figure", "table", "formula"]
        );
        assert!(
            receipt.steps[1..]
                .iter()
                .all(|s| s.status == "terminal" && s.outcome.is_some())
        );
        let expected = row["heads"].as_sequence().unwrap();
        for (step, expected_head) in receipt.steps[1..].iter().zip(expected) {
            let reason = match step.outcome.as_ref().unwrap() {
                NativeVisualHeadOutcome::Terminal { reason, .. } => reason,
                _ => panic!("reviewed row must be typed terminal"),
            };
            let actual = match reason {
                NativeVisualAbsence::Unsupported => "unsupported",
                NativeVisualAbsence::NotObserved => "not_observed",
                other => panic!("unexpected reviewed absence: {other:?}"),
            };
            assert_eq!(actual, expected_head["status"].as_str().unwrap());
        }
        let replay = serde_json::to_vec(&receipt).unwrap();
        assert_eq!(
            replay,
            serde_json::to_vec(
                &serde_json::from_slice::<NativeVisualSequenceReceipt>(&replay).unwrap()
            )
            .unwrap()
        );
        assert!(receipt.redaction.values().all(|value| !value));
        assert_eq!(receipt.java_fallback.disposition, "preserved_evidence_only");
    }
}

#[test]
fn validation_failure_constructs_no_head_outcomes_and_stops_dependents() {
    let mut input = mixed();
    input.document_id.clear();
    let receipt = run_visual_heads_sequence(&input, &Default::default());
    assert_eq!(receipt.status, "terminal_failure");
    assert_eq!(
        receipt.steps[0].failure_code.as_deref(),
        Some("invalid_identity")
    );
    assert!(
        receipt.steps[1..]
            .iter()
            .all(|s| s.status == "not_admitted" && s.outcome.is_none())
    );

    let mut malformed_sha = mixed();
    malformed_sha.source_sha256 = "bad".into();
    let receipt = run_visual_heads_sequence(&malformed_sha, &Default::default());
    assert_eq!(
        receipt.steps[0].failure_code.as_deref(),
        Some("invalid_digest")
    );
}

#[test]
fn mixed_heads_are_ordered_conserved_and_replay_stable() {
    let input = mixed();
    let receipt = run_visual_heads_sequence(&input, &Default::default());
    assert_eq!(receipt.status, "pass");
    assert_eq!(
        receipt.steps[1..]
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["figure", "table", "formula"]
    );
    let NativeVisualHeadOutcome::Figure { metrics, .. } =
        receipt.steps[1].outcome.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation
        ),
        (1, 1, 0)
    );
    let NativeVisualHeadOutcome::Table { receipts, metrics } =
        receipt.steps[2].outcome.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation,
            receipts[0].cells.len()
        ),
        (1, 1, 1, 2)
    );
    let NativeVisualHeadOutcome::Formula { receipts, metrics } =
        receipt.steps[3].outcome.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation,
            receipts[0].equation_label_digest.as_deref()
        ),
        (1, 1, 0, Some(SHA))
    );
    let mut permuted = input.clone();
    permuted.primitives.reverse();
    assert_eq!(
        serde_json::to_vec(&receipt).unwrap(),
        serde_json::to_vec(&run_visual_heads_sequence(&permuted, &Default::default())).unwrap()
    );
}

#[test]
fn figure_head_limit_stops_only_dependent_steps() {
    let mut input = mixed();
    input
        .primitives
        .push(primitive(NativeVisualKind::Graphic, 3, 9, 60.0));
    let receipt = run_visual_heads_sequence(
        &input,
        &NativeVisualConfig::new(MAX_PRIMITIVES, MAX_RECEIPTS, 1),
    );
    assert_eq!(receipt.status, "terminal_failure");
    assert_eq!(receipt.steps[1].failure_code.as_deref(), Some("head_limit"));
    assert!(
        receipt.steps[2..]
            .iter()
            .all(|s| s.status == "not_admitted" && s.outcome.is_none())
    );
    assert_eq!(receipt.java_fallback.source, "production_fallback");
}

#[test]
fn mixed_sequence_preserves_geometry_links_and_cells_once_across_siblings() {
    let receipt = run_visual_heads_sequence(&mixed_with_two_tables(), &Default::default());
    assert_eq!(
        receipt.schema_version,
        NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION
    );
    assert_eq!(receipt.status, "pass");
    assert_eq!(receipt.identity.document_id, "doc-1");
    assert_eq!(receipt.identity.source_sha256, SHA);
    assert_eq!(
        receipt.identity.visual_schema_version,
        NATIVE_VISUAL_SCHEMA_VERSION
    );
    assert_eq!(receipt.java_fallback.disposition, "preserved_evidence_only");
    assert_eq!(receipt.java_fallback.target, "java_grobid");
    assert_eq!(receipt.java_fallback.source, "production_fallback");
    assert!(receipt.redaction.values().all(|value| !value));

    let NativeVisualHeadOutcome::Figure { receipts, .. } = receipt.steps[1]
        .outcome
        .as_ref()
        .expect("figure must be admitted")
    else {
        panic!("figure must be a positive receipt")
    };
    assert_eq!(
        (
            receipts[0].page_ordinal,
            receipts[0].source_ordinal,
            receipts[0].content_digest.as_str(),
            receipts[0].caption_digest.as_deref(),
            receipts[0].callout_digest.as_deref()
        ),
        (3, 8, SHA, Some(SHA), None)
    );
    assert_eq!(receipts[0].bbox, bbox(1.0));

    let NativeVisualHeadOutcome::Table { receipts, .. } = receipt.steps[2]
        .outcome
        .as_ref()
        .expect("table must be admitted")
    else {
        panic!("table must be a positive receipt")
    };
    assert_eq!(receipts.len(), 2);
    assert_eq!(
        (receipts[0].page_ordinal, receipts[0].source_ordinal),
        (5, 2)
    );
    assert_eq!(
        (receipts[1].page_ordinal, receipts[1].source_ordinal),
        (9, 5)
    );
    assert_eq!(receipts[0].bbox, bbox(20.0));
    assert_eq!(receipts[1].bbox, bbox(60.0));
    assert_eq!(receipts[0].content_digest, SHA);
    assert_eq!(receipts[1].content_digest, SHA);
    assert_eq!(receipts[0].caption_digest.as_deref(), Some(SHA));
    assert_eq!(receipts[0].callout_digest.as_deref(), Some(SHA));
    assert_eq!(
        receipts[1].caption_digest.as_deref(),
        Some("b".repeat(64).as_str())
    );
    assert_eq!(
        receipts[1].callout_digest.as_deref(),
        Some("c".repeat(64).as_str())
    );
    assert_eq!(receipts[0].cells.len(), 2);
    assert!(receipts[1].cells.is_empty());
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.cells.len())
            .sum::<usize>(),
        2
    );

    let NativeVisualHeadOutcome::Formula { receipts, .. } = receipt.steps[3]
        .outcome
        .as_ref()
        .expect("formula must be admitted")
    else {
        panic!("formula must be a positive receipt")
    };
    assert_eq!(
        (receipts[0].page_ordinal, receipts[0].source_ordinal),
        (7, 4)
    );
    assert_eq!(receipts[0].bbox, bbox(40.0));
    assert_eq!(receipts[0].content_digest, SHA);
    assert_eq!(receipts[0].equation_label_digest.as_deref(), Some(SHA));
    assert_eq!(receipts[0].callout_digest, None);
    assert!(receipt.steps.iter().all(|step| step.duration_ms == 0));
}

#[test]
fn sequence_is_canonical_under_foreign_permutation_and_serde_roundtrip() {
    let input = mixed_with_two_tables();
    let baseline =
        serde_json::to_vec(&run_visual_heads_sequence(&input, &Default::default())).unwrap();
    let mut foreign_permuted = input.clone();
    foreign_permuted
        .primitives
        .sort_by_key(|primitive| match primitive.kind {
            NativeVisualKind::Image => 3,
            NativeVisualKind::Table => 1,
            NativeVisualKind::Formula => 2,
            NativeVisualKind::Graphic | NativeVisualKind::Rect => 4,
        });
    let permuted = serde_json::to_vec(&run_visual_heads_sequence(
        &foreign_permuted,
        &Default::default(),
    ))
    .unwrap();
    assert_eq!(baseline, permuted);
    let replay: NativeVisualSequenceReceipt = serde_json::from_slice(&baseline).unwrap();
    assert_eq!(baseline, serde_json::to_vec(&replay).unwrap());
}

fn oracle_is_valid(data: &Value) -> bool {
    if data["schema_version"] != "native-visual-oracle.v1"
        || data["review_status"] != "independently_reviewed"
        || data["representation"] != "digest_only"
        || data["expected_rows_not_from_sut_helpers"] != true
    {
        return false;
    }
    let Some(rows) = data["receipts"].as_sequence() else {
        return false;
    };
    if rows.len() != 9 {
        return false;
    }
    let expected = [
        ("hub-2507.19457", 96u64),
        ("rlm-2512.24601", 43),
        ("reca-2605.26525v1", 63),
    ];
    let lanes = [
        "pdfalto",
        "pdf_inspector_positioned",
        "pdf_inspector_positioned_xy_cut",
    ];
    let mut identities = BTreeSet::new();
    let allowed_top_level = [
        "schema_version",
        "review_status",
        "representation",
        "coordinate_dialect",
        "expected_rows_not_from_sut_helpers",
        "forbidden_fields",
        "receipts",
    ];
    if data.as_mapping().is_some_and(|mapping| {
        mapping.keys().any(|key| {
            key.as_str()
                .is_some_and(|key| !allowed_top_level.contains(&key))
        })
    }) {
        return false;
    }
    for (index, row) in rows.iter().enumerate() {
        let Some(document) = row["document_id"].as_str() else {
            return false;
        };
        let Some(lane) = row["lane_id"].as_str() else {
            return false;
        };
        if !identities.insert((document, lane)) || !lanes.contains(&lane) {
            return false;
        }
        let Some((document_index, (_, pages))) = expected
            .iter()
            .enumerate()
            .find(|(_, (candidate, _))| *candidate == document)
        else {
            return false;
        };
        let Some(lane_index) = lanes.iter().position(|candidate| *candidate == lane) else {
            return false;
        };
        let expected_sha = match document {
            "hub-2507.19457" => "ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
            "rlm-2512.24601" => "8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca",
            "reca-2605.26525v1" => {
                "a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8"
            }
            _ => return false,
        };
        if index != document_index * 3 + lane_index
            || row["source_sha256"].as_str() != Some(expected_sha)
            || row["page_denominator"].as_u64() != Some(*pages)
            || row["page_range"] != Value::Sequence(vec![1.into(), (*pages).into()])
        {
            return false;
        }
        let Some(heads) = row["heads"].as_sequence() else {
            return false;
        };
        if heads.len() != 3 {
            return false;
        }
        let mut names = BTreeSet::new();
        for head in heads {
            let Some(name) = head["head"].as_str() else {
                return false;
            };
            let Some(status) = head["status"].as_str() else {
                return false;
            };
            if !["figure", "table", "formula"].contains(&name)
                || !names.insert(name)
                || !["unsupported", "not_observed"].contains(&status)
                || head["head_ordinal"] != 0
            {
                return false;
            }
        }
        if row["table_cells"]["oracle_n"] != 0
            || row["table_cells"]["status"] != "not_observed"
            || row["formula_labels"]["oracle_n"] != 0
            || row["formula_labels"]["status"] != "not_observed"
        {
            return false;
        }
    }
    identities.len() == 9
}

#[test]
fn independent_oracle_mutations_fail_closed_without_silent_zero_or_payload() {
    let base = oracle();
    assert!(oracle_is_valid(&base));

    let mut pooled = base.clone();
    pooled["receipts"][1]["page_denominator"] = 202.into();
    assert!(!oracle_is_valid(&pooled));

    let mut silent_zero = base.clone();
    silent_zero["receipts"][0]["table_cells"]["status"] = "unsupported".into();
    assert!(!oracle_is_valid(&silent_zero));

    let mut wrong_status = base.clone();
    wrong_status["receipts"][0]["heads"][0]["status"] = "success".into();
    assert!(!oracle_is_valid(&wrong_status));

    let mut duplicate_identity = base.clone();
    duplicate_identity["receipts"][1]["lane_id"] =
        duplicate_identity["receipts"][0]["lane_id"].clone();
    assert!(!oracle_is_valid(&duplicate_identity));

    let mut wrong_sha = base.clone();
    wrong_sha["receipts"][0]["source_sha256"] = "0".repeat(64).into();
    assert!(!oracle_is_valid(&wrong_sha));

    let mut forbidden = base.clone();
    forbidden["raw_pdf"] = "payload".into();
    assert!(!oracle_is_valid(&forbidden));
}

#[test]
fn empty_success_and_forbidden_sequence_payload_fail_closed() {
    let mut empty = mixed();
    empty.primitives[0].bbox = None;
    empty.primitives[0].content_digest = None;
    empty.primitives[0].absence = Some(NativeVisualAbsence::NotObserved);
    let receipt = run_visual_heads_sequence(&empty, &Default::default());
    assert_eq!(receipt.steps[1].status, "terminal");
    assert!(matches!(
        receipt.steps[1].outcome,
        Some(NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            ..
        })
    ));
    let encoded: serde_json::Value =
        serde_json::from_slice(&serde_json::to_vec(&receipt).unwrap()).unwrap();
    let redaction_keys = [
        "raw_text",
        "raw_pdf",
        "physical_paths",
        "hostname",
        "executable",
    ];
    let forbidden_payload_keys = [
        "pdf_bytes",
        "tei_xml",
        "image",
        "page_image",
        "assets",
        "physical_path",
    ];
    fn has_forbidden_payload(
        value: &serde_json::Value,
        forbidden: &[&str],
        redaction: &[&str],
    ) -> bool {
        match value {
            serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
                (forbidden.contains(&key.as_str()))
                    || (!redaction.contains(&key.as_str())
                        && has_forbidden_payload(value, forbidden, redaction))
            }),
            serde_json::Value::Array(items) => items
                .iter()
                .any(|item| has_forbidden_payload(item, forbidden, redaction)),
            _ => false,
        }
    }
    assert!(!has_forbidden_payload(
        &encoded,
        &forbidden_payload_keys,
        &redaction_keys
    ));
    for key in redaction_keys {
        assert_eq!(encoded["redaction"][key], serde_json::Value::Bool(false));
    }
}

#[test]
fn sequence_receipt_is_digest_only_and_timing_free() {
    let bytes =
        serde_json::to_vec(&run_visual_heads_sequence(&mixed(), &Default::default())).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    for forbidden in [
        "raw_text",
        "raw_pdf",
        "physical_paths",
        "hostname",
        "executable",
        "Instant",
        "SystemTime",
    ] {
        assert!(
            text.contains(forbidden)
                == (forbidden == "raw_text"
                    || forbidden == "raw_pdf"
                    || forbidden == "physical_paths"
                    || forbidden == "hostname"
                    || forbidden == "executable"),
            "unexpected payload token: {forbidden}"
        );
    }
    assert!(text.contains("duration_ms"));
    assert!(
        run_visual_heads_sequence(&mixed(), &Default::default())
            .steps
            .iter()
            .all(|step| step.duration_ms == 0)
    );
}
