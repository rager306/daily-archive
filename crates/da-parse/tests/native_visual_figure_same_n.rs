//! Figure-only same-n binding against the independently reviewed fixture.
//!
//! The reviewed rows are all typed absence. Positive geometry and digest
//! conservation below are deliberately synthetic and never derived from the
//! reviewed fixture.

use da_parse::*;
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const FIXTURE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const DOCS: [(&str, &str, i64); 3] = [
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
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CONTENT_A: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CONTENT_B: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CAPTION_A: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const CALLOUT_B: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
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

fn fixture() -> Value {
    serde_yaml::from_str(
        &fs::read_to_string(root().join(FIXTURE)).expect("reviewed fixture must be present"),
    )
    .expect("reviewed fixture must be valid YAML")
}

fn figure_head(row: &Value) -> &Value {
    row["heads"]
        .as_sequence()
        .expect("heads must be a sequence")
        .iter()
        .find(|head| head["head"].as_str() == Some("figure"))
        .expect("each receipt must contain one figure row")
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

fn validate_reviewed_fixture(value: &Value) -> Result<(), String> {
    if value["schema_version"].as_str() != Some("native-visual-oracle.v1")
        || value["review_status"].as_str() != Some("independently_reviewed")
        || value["representation"].as_str() != Some("digest_only")
    {
        return Err("review metadata drift".into());
    }
    let rows = value["receipts"]
        .as_sequence()
        .ok_or_else(|| "receipts must be a sequence".to_owned())?;
    if rows.len() != 9 {
        return Err(format!("expected 9 receipts, got {}", rows.len()));
    }
    let declared_forbidden = value["forbidden_fields"]
        .as_sequence()
        .ok_or_else(|| "forbidden_fields must be a sequence".to_owned())?;
    for field in FORBIDDEN {
        if !declared_forbidden
            .iter()
            .any(|item| item.as_str() == Some(field))
            || contains_key(value, field)
        {
            return Err(format!("forbidden payload field {field}"));
        }
    }
    let mut identities = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let document = row["document_id"].as_str().ok_or("missing document")?;
        let lane = row["lane_id"].as_str().ok_or("missing lane")?;
        if !identities.insert((document, lane)) {
            return Err(format!("duplicate identity {document}/{lane}"));
        }
        let document_index = DOCS
            .iter()
            .position(|(id, _, _)| *id == document)
            .ok_or_else(|| format!("unknown document {document}"))?;
        let lane_index = LANES
            .iter()
            .position(|candidate| *candidate == lane)
            .ok_or_else(|| format!("unknown lane {lane}"))?;
        if index != document_index * LANES.len() + lane_index {
            return Err(format!("receipt order drift {document}/{lane}"));
        }
        let (_, expected_sha, expected_pages) = DOCS[document_index];
        if row["source_sha256"].as_str() != Some(expected_sha)
            || row["page_denominator"].as_i64() != Some(expected_pages)
            || row["page_range"].as_sequence().map(Vec::len) != Some(2)
            || row["page_range"][0].as_i64() != Some(1)
            || row["page_range"][1].as_i64() != Some(expected_pages)
        {
            return Err(format!("source binding drift {document}/{lane}"));
        }
        let heads = row["heads"]
            .as_sequence()
            .ok_or_else(|| format!("missing heads {document}/{lane}"))?;
        if heads
            .iter()
            .filter(|candidate| candidate["head"].as_str() == Some("figure"))
            .count()
            != 1
        {
            return Err(format!(
                "missing or duplicate figure identity {document}/{lane}"
            ));
        }
        let head = figure_head(row);
        let status = head["status"].as_str().ok_or("missing figure status")?;
        if !matches!(status, "unsupported" | "not_observed") {
            return Err(format!("open figure status {document}/{lane}: {status}"));
        }
        for field in [
            "bbox_72dpi",
            "caption_digest",
            "content_digest",
            "callout_digest",
        ] {
            if head[field].as_str() != Some("absent") {
                return Err(format!(
                    "figure {field} is not typed absent {document}/{lane}"
                ));
            }
        }
    }
    if identities.len() != 9 {
        return Err(format!(
            "missing receipt identity, got {}",
            identities.len()
        ));
    }
    Ok(())
}

fn reviewed_rows() -> Vec<(String, String, i64, String, String)> {
    let value = fixture();
    validate_reviewed_fixture(&value).expect("reviewed fixture must pass strict validation");
    assert_eq!(
        value["schema_version"].as_str(),
        Some("native-visual-oracle.v1")
    );
    assert_eq!(
        value["review_status"].as_str(),
        Some("independently_reviewed")
    );
    assert_eq!(value["representation"].as_str(), Some("digest_only"));

    let rows = value["receipts"].as_sequence().expect("receipts sequence");
    assert_eq!(rows.len(), 9, "figure same-n must have exactly nine rows");
    let mut identities = BTreeSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let document = row["document_id"].as_str().expect("document identity");
        let lane = row["lane_id"].as_str().expect("lane identity");
        let doc_index = DOCS
            .iter()
            .position(|(id, _, _)| *id == document)
            .expect("unknown reviewed document");
        let lane_index = LANES
            .iter()
            .position(|candidate| *candidate == lane)
            .expect("unknown reviewed lane");
        assert_eq!(index, doc_index * LANES.len() + lane_index, "receipt order");
        assert!(
            identities.insert((document, lane)),
            "duplicate row {document}/{lane}"
        );

        let (_, expected_sha, expected_pages) = DOCS[doc_index];
        assert_eq!(
            row["source_sha256"].as_str(),
            Some(expected_sha),
            "SHA {document}/{lane}"
        );
        assert_eq!(
            row["page_denominator"].as_i64(),
            Some(expected_pages),
            "pages {document}/{lane}"
        );
        assert_eq!(row["page_range"].as_sequence().map(Vec::len), Some(2));
        assert_eq!(row["page_range"][0].as_i64(), Some(1));
        assert_eq!(row["page_range"][1].as_i64(), Some(expected_pages));

        let head = figure_head(row);
        assert_eq!(
            head["head_ordinal"].as_i64(),
            Some(0),
            "ordinal {document}/{lane}"
        );
        let status = head["status"].as_str().expect("figure status");
        assert!(
            matches!(status, "unsupported" | "not_observed"),
            "open status {status}"
        );
        for field in [
            "bbox_72dpi",
            "caption_digest",
            "content_digest",
            "callout_digest",
        ] {
            assert_eq!(
                head[field].as_str(),
                Some("absent"),
                "{field} must be typed absent"
            );
        }
        result.push((
            document.to_owned(),
            lane.to_owned(),
            expected_pages,
            expected_sha.to_owned(),
            status.to_owned(),
        ));
    }
    assert_eq!(identities.len(), 9);
    result
}

fn absence_observation(
    document: &str,
    source_sha256: &str,
    lane: &str,
    status: &str,
) -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: document.into(),
        source_sha256: source_sha256.into(),
        provider: NativeVisualProvider {
            id: format!("reviewed-figure-same-n/{lane}"),
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

fn synthetic_primitive(
    kind: NativeVisualKind,
    page: u32,
    source: u32,
    bbox: LayoutBBox,
    content_digest: &str,
) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: "synthetic-figure-positive".into(),
        source_sha256: SHA.into(),
        provider: NativeVisualProvider {
            id: "synthetic-figure-provider".into(),
            version: "v1".into(),
        },
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(bbox),
        content_digest: Some(content_digest.into()),
        absence: None,
    }
}

fn link(page: u32, source: u32, digest: &str) -> NativeVisualDigestReceipt {
    NativeVisualDigestReceipt {
        page_ordinal: page,
        source_ordinal: source,
        digest: Some(digest.into()),
        absence: None,
    }
}

fn positive_observation(with_siblings: bool, reverse_figures: bool) -> NativeVisualObservation {
    let first = synthetic_primitive(
        NativeVisualKind::Graphic,
        4,
        9,
        LayoutBBox::new(1.0, 2.0, 11.0, 12.0).unwrap(),
        CONTENT_A,
    );
    let second = synthetic_primitive(
        NativeVisualKind::Image,
        2,
        3,
        LayoutBBox::new(20.0, 5.0, 32.0, 17.0).unwrap(),
        CONTENT_B,
    );
    let figures = if reverse_figures {
        vec![first.clone(), second.clone()]
    } else {
        vec![second.clone(), first.clone()]
    };
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: "synthetic-figure-positive".into(),
        source_sha256: SHA.into(),
        provider: NativeVisualProvider {
            id: "synthetic-figure-provider".into(),
            version: "v1".into(),
        },
        primitives: if with_siblings {
            let mut all = figures;
            all.extend([
                synthetic_primitive(
                    NativeVisualKind::Table,
                    1,
                    1,
                    LayoutBBox::new(40.0, 1.0, 50.0, 11.0).unwrap(),
                    SHA,
                ),
                synthetic_primitive(
                    NativeVisualKind::Formula,
                    3,
                    2,
                    LayoutBBox::new(41.0, 2.0, 51.0, 12.0).unwrap(),
                    SHA,
                ),
                synthetic_primitive(
                    NativeVisualKind::Rect,
                    5,
                    4,
                    LayoutBBox::new(42.0, 3.0, 52.0, 13.0).unwrap(),
                    SHA,
                ),
            ]);
            all
        } else {
            figures
        },
        figure_captions: vec![link(4, 9, CAPTION_A)],
        figure_callouts: vec![link(2, 3, CALLOUT_B)],
        table_captions: vec![],
        table_callouts: vec![],
        table_cells: vec![],
        formula_labels: vec![],
        formula_callouts: vec![],
        lane_absence: None,
    }
}

#[test]
fn reviewed_figure_rows_are_independently_evaluated_as_exact_typed_absence() {
    let rows = reviewed_rows();
    let mut evaluated = 0;
    for (document, lane, pages, sha, status) in rows {
        let input = absence_observation(&document, &sha, &lane, &status);
        let outcome = detect_figure_heads(&input, Default::default())
            .unwrap_or_else(|error| panic!("{document}/{lane} ({pages} pages): {error:?}"));
        let expected_reason = if status == "unsupported" {
            NativeVisualAbsence::Unsupported
        } else {
            NativeVisualAbsence::NotObserved
        };
        match &outcome {
            NativeVisualHeadOutcome::Terminal { reason, metrics } => {
                assert_eq!(*reason, expected_reason, "reason {document}/{lane}");
                assert_eq!((metrics.admitted, metrics.terminal), (0, 1));
                assert_eq!(
                    (
                        metrics.caption_conservation,
                        metrics.content_conservation,
                        metrics.callout_conservation,
                        metrics.geometry_conservation
                    ),
                    (0, 0, 0, 0)
                );
            }
            other => panic!("{document}/{lane} became a figure success: {other:?}"),
        }
        let replay: NativeVisualHeadOutcome =
            serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
        assert_eq!(replay, outcome, "serde replay {document}/{lane}");
        evaluated += 1;
    }
    assert_eq!(
        evaluated, 9,
        "no pooled denominator may replace nine evaluations"
    );
}

#[test]
fn synthetic_figures_conserve_geometry_digests_and_order_without_sibling_contamination() {
    let base =
        detect_figure_heads(&positive_observation(false, false), Default::default()).unwrap();
    let permuted =
        detect_figure_heads(&positive_observation(false, true), Default::default()).unwrap();
    let with_siblings =
        detect_figure_heads(&positive_observation(true, false), Default::default()).unwrap();
    assert_eq!(
        serde_json::to_vec(&base).unwrap(),
        serde_json::to_vec(&permuted).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&base).unwrap(),
        serde_json::to_vec(&with_siblings).unwrap()
    );

    let NativeVisualHeadOutcome::Figure { receipts, metrics } = base else {
        panic!("synthetic positive must be Figure");
    };
    assert_eq!(receipts.len(), 2);
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| (
                receipt.head_ordinal,
                receipt.page_ordinal,
                receipt.source_ordinal
            ))
            .collect::<Vec<_>>(),
        vec![(0, 2, 3), (1, 4, 9)]
    );
    assert_eq!(
        receipts[0].bbox,
        LayoutBBox::new(20.0, 5.0, 32.0, 17.0).unwrap()
    );
    assert_eq!(
        receipts[1].bbox,
        LayoutBBox::new(1.0, 2.0, 11.0, 12.0).unwrap()
    );
    assert_eq!(receipts[0].content_digest, CONTENT_B);
    assert_eq!(receipts[1].content_digest, CONTENT_A);
    assert_eq!(receipts[0].caption_digest, None);
    assert_eq!(receipts[1].caption_digest.as_deref(), Some(CAPTION_A));
    assert_eq!(receipts[0].callout_digest.as_deref(), Some(CALLOUT_B));
    assert_eq!(receipts[1].callout_digest, None);
    assert_eq!(
        (
            metrics.admitted,
            metrics.terminal,
            metrics.caption_conservation,
            metrics.content_conservation,
            metrics.callout_conservation,
            metrics.geometry_conservation,
        ),
        (2, 0, 1, 2, 1, 2)
    );

    let replay: NativeVisualHeadOutcome =
        serde_json::from_slice(&serde_json::to_vec(&with_siblings).unwrap()).unwrap();
    assert_eq!(replay, with_siblings);
}

#[test]
fn malformed_review_status_and_pooled_denominator_fail_closed() {
    let mut status = fixture();
    status["receipts"][0]["heads"][0]["status"] = Value::String("figure".into());
    assert!(validate_reviewed_fixture(&status).is_err());

    let mut pooled = fixture();
    pooled["receipts"][1]["page_denominator"] = Value::Number(202.into());
    assert!(validate_reviewed_fixture(&pooled).is_err());

    let mut malformed_sha = fixture();
    malformed_sha["receipts"][0]["source_sha256"] = Value::String("not-a-sha".into());
    assert!(validate_reviewed_fixture(&malformed_sha).is_err());

    let mut payload = fixture();
    payload["receipts"][0]["raw_pdf"] = Value::String("must not enter digest-only receipt".into());
    assert!(validate_reviewed_fixture(&payload).is_err());

    let mut duplicate = fixture();
    duplicate["receipts"][1] = duplicate["receipts"][0].clone();
    assert!(validate_reviewed_fixture(&duplicate).is_err());

    let mut missing = fixture();
    missing["receipts"][0]["heads"]
        .as_sequence_mut()
        .unwrap()
        .retain(|head| head["head"].as_str() != Some("figure"));
    assert!(validate_reviewed_fixture(&missing).is_err());
    assert_eq!(reviewed_rows().len(), 9, "fixture itself remains unchanged");
}
