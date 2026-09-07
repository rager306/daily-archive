//! Integration contract for the source-bound P2 -> native body accountant handoff.
//! Expected totals are frozen DATA rows, not values derived by the accountant.

use da_parse::{
    AccountingBucket, DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider,
    LayoutSpan, LayoutSpanAbsence, LayoutStyleHints, LayoutToken, LayoutTokenKind,
    NATIVE_BODY_SCHEMA_VERSION, NATIVE_BODY_SEQUENCE_SCHEMA_VERSION, NativeBodyAccountant,
    NativeBodySequenceInput, SegmentationOutcome, TokenAssignment, run_native_body_sequence,
};
use serde_yaml::Value;

const ORACLE: &str = include_str!("fixtures/native_body_accounting_oracle.yaml");

fn text<'a>(map: &'a serde_yaml::Mapping, key: &str) -> &'a str {
    map.get(Value::String(key.into()))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing oracle field {key}"))
}

fn number(map: &serde_yaml::Mapping, key: &str) -> usize {
    map.get(Value::String(key.into()))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing oracle number {key}")) as usize
}

fn region(value: &str) -> DocumentRegion {
    match value {
        "header" => DocumentRegion::Header,
        "body" => DocumentRegion::Body,
        "references" => DocumentRegion::References,
        "annex" => DocumentRegion::Annex,
        "page_furniture" => DocumentRegion::PageFurniture,
        other => panic!("unknown oracle region {other}"),
    }
}

fn kind(value: &str) -> LayoutTokenKind {
    match value {
        "word" => LayoutTokenKind::Word,
        "formula" => LayoutTokenKind::Formula,
        "image" => LayoutTokenKind::Image,
        "other" => LayoutTokenKind::Other,
        other => panic!("unknown oracle token kind {other}"),
    }
}

fn bucket(value: &str) -> AccountingBucket {
    match value {
        "body_blocks" => AccountingBucket::BodyBlocks,
        "visual_content" => AccountingBucket::VisualContent,
        "annex" => AccountingBucket::Annex,
        "references" => AccountingBucket::References,
        "page_furniture" => AccountingBucket::PageFurniture,
        "unsupported" => AccountingBucket::Unsupported,
        other => panic!("unknown oracle bucket {other}"),
    }
}

#[test]
fn exact_p2_receipts_close_to_accountant_without_aggregate_only_checks() {
    let oracle: Value = serde_yaml::from_str(ORACLE).expect("oracle YAML parses");
    assert_eq!(oracle["schema_version"], "native-body-accounting-oracle.v1");
    let receipt = oracle["receipts"]
        .as_sequence()
        .expect("receipts sequence")
        .first()
        .expect("one receipt");
    assert_eq!(receipt["terminal_status"], "pass");
    assert_eq!(
        receipt["boundary_digest"],
        receipt["terminal_boundary_digest"]
    );

    let document_id = receipt["document_id"].as_str().unwrap();
    let provider_id = receipt["provider_id"].as_str().unwrap();
    let source_sha256 = receipt["source_pdf_sha256"].as_str().unwrap();
    let boundary_digest = receipt["boundary_digest"].as_str().unwrap();
    let rows = receipt["tokens"].as_sequence().unwrap();
    let provider = LayoutProvider::new(provider_id, "p2-v1", "a".repeat(64)).unwrap();

    let tokens = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let row = row.as_mapping().unwrap();
            let page = row[Value::String("page_ordinal".into())].as_u64().unwrap() as u32;
            let source = row[Value::String("source_ordinal".into())]
                .as_u64()
                .unwrap() as u32;
            let ordinal = row[Value::String("token_ordinal".into())].as_u64().unwrap() as u32;
            LayoutToken::new(
                page,
                source,
                ordinal,
                LayoutBBox::new(
                    10.0,
                    10.0 + index as f64 * 10.0,
                    90.0,
                    18.0 + index as f64 * 10.0,
                )
                .unwrap(),
                text(row, "text_digest").to_owned(),
                LayoutSpan::absent(LayoutSpanAbsence::NonText),
                kind(text(row, "kind")),
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        document_id,
        source_sha256.to_owned(),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens).unwrap()],
    )
    .unwrap();

    let assignments = rows
        .iter()
        .map(|row| {
            let row = row.as_mapping().unwrap();
            TokenAssignment {
                page_ordinal: row[Value::String("page_ordinal".into())].as_u64().unwrap() as u32,
                source_ordinal: row[Value::String("source_ordinal".into())]
                    .as_u64()
                    .unwrap() as u32,
                token_ordinal: row[Value::String("token_ordinal".into())].as_u64().unwrap() as u32,
                text_digest: text(row, "text_digest").to_owned(),
                region: region(text(row, "region")),
            }
        })
        .collect();
    let segmentation = SegmentationOutcome {
        schema_version: da_parse::NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        document_id: document_id.to_owned(),
        provider_id: provider_id.to_owned(),
        assignments,
        sections: Vec::new(),
        boundary_digest: boundary_digest.to_owned(),
    };

    let accounting = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .expect("source-bound P2 receipt must be accepted");
    let expected = receipt["expected"].as_mapping().unwrap();
    assert_eq!(accounting.admitted, number(expected, "admitted"));
    assert_eq!(accounting.conserved, number(expected, "conserved"));
    assert_eq!(accounting.unsupported, number(expected, "unsupported"));
    assert_eq!(accounting.loss, number(expected, "loss"));
    assert_eq!(accounting.assignments, segmentation.assignments);
    assert_eq!(
        accounting.source_boundary_digest,
        text(receipt.as_mapping().unwrap(), "terminal_boundary_digest")
    );

    let expected_buckets = expected[Value::String("buckets".into())]
        .as_mapping()
        .unwrap();
    for (name, expected_count) in expected_buckets {
        let name = name.as_str().unwrap();
        let expected_count = expected_count.as_mapping().unwrap();
        let actual = accounting
            .buckets
            .get(&bucket(name))
            .expect("expected bucket");
        assert_eq!(
            actual.admitted,
            number(expected_count, "admitted"),
            "bucket {name}"
        );
        assert_eq!(
            actual.conserved,
            number(expected_count, "conserved"),
            "bucket {name}"
        );
        assert_eq!(
            actual.unsupported,
            number(expected_count, "unsupported"),
            "bucket {name}"
        );
        assert_eq!(actual.loss, number(expected_count, "loss"), "bucket {name}");
    }

    let actual_order: Vec<_> = accounting
        .units
        .iter()
        .map(|unit| (unit.page_ordinal, unit.source_ordinal, unit.token_ordinal))
        .collect();
    let expected_order: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row["page_ordinal"].as_u64().unwrap() as u32,
                row["source_ordinal"].as_u64().unwrap() as u32,
                row["token_ordinal"].as_u64().unwrap() as u32,
            )
        })
        .collect();
    assert_eq!(actual_order, expected_order);

    let body = accounting.buckets[&AccountingBucket::BodyBlocks].admitted;
    let visual = accounting.buckets[&AccountingBucket::VisualContent].admitted;
    let annex = accounting.buckets[&AccountingBucket::Annex].admitted;
    assert_eq!(
        body + visual,
        3,
        "body prose and opaque visuals stay in body"
    );
    assert_eq!(annex, 1, "annex remains outside body totals");

    let sequence = run_native_body_sequence(NativeBodySequenceInput {
        document: &document,
        segmentation: Some(&segmentation),
        p2_status: "pass",
        p2_failure_code: None,
    });
    assert_eq!(sequence.schema_version, NATIVE_BODY_SEQUENCE_SCHEMA_VERSION);
    assert_eq!(sequence.status, "pass");
    assert_eq!(sequence.identity.document_id, document_id);
    assert_eq!(sequence.identity.source_sha256, source_sha256);
    assert_eq!(sequence.identity.provider_id, provider_id);
    assert_eq!(sequence.identity.boundary_digest, boundary_digest);
    assert_eq!(
        sequence.identity.accounting_schema_version,
        NATIVE_BODY_SCHEMA_VERSION
    );
    assert_eq!(sequence.steps.len(), 2);
    assert!(sequence.steps.iter().all(|step| step.status == "pass"));
    assert_eq!(sequence.counts.admitted, accounting.admitted);
    assert_eq!(sequence.counts.conserved, accounting.conserved);
    assert_eq!(sequence.counts.unsupported, accounting.unsupported);
    assert_eq!(sequence.counts.loss, accounting.loss);
    assert_eq!(
        sequence.java_fallback.disposition,
        "preserved_evidence_only"
    );
    assert!(sequence.redaction.values().all(|redacted| !redacted));
    let wire = serde_json::to_string(&sequence).unwrap();
    assert!(!wire.contains("raw_text_payload"));
    assert!(!wire.contains("pdf_bytes"));
}

#[test]
fn terminal_p2_failure_stops_before_accounting_and_keeps_java_evidence() {
    let document = LayoutDocument::new("terminal-p2", "a".repeat(64), vec![]).unwrap();
    let sequence = run_native_body_sequence(NativeBodySequenceInput {
        document: &document,
        segmentation: None,
        p2_status: "terminal_failure",
        p2_failure_code: Some("malformed_layout"),
    });
    assert_eq!(sequence.status, "terminal_failure");
    assert_eq!(sequence.steps[0].status, "terminal_failure");
    assert_eq!(sequence.steps[1].status, "not_admitted");
    assert_eq!(
        sequence.steps[1].failure_code.as_deref(),
        Some("p2_terminal_failure")
    );
    assert_eq!(sequence.counts.admitted, 0);
    assert_eq!(sequence.java_fallback.target, "java_grobid");
}

#[test]
fn oracle_rows_are_source_bound_and_not_payload_backed() {
    assert!(!ORACLE.contains("raw_text"));
    assert!(!ORACLE.contains("pdf_bytes"));
    assert!(ORACLE.contains("terminal_boundary_digest"));
    assert!(ORACLE.contains("text_digest"));
}
