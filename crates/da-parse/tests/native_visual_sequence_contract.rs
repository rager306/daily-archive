use da_parse::*;
use serde_yaml::Value;
use std::{fs, path::PathBuf};

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
