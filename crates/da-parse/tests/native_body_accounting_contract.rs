use da_parse::{
    AccountedKind, AccountingBucket, DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage,
    LayoutProvider, LayoutSpan, LayoutSpanAbsence, LayoutStyleHints, LayoutToken, LayoutTokenKind,
    NATIVE_BODY_SCHEMA_VERSION, NativeBodyAccountant, SegmentationOutcome, TokenAssignment,
};
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn fixture() -> (LayoutDocument, SegmentationOutcome) {
    let provider = LayoutProvider::new("body-fixture", "1", "a".repeat(64)).unwrap();
    let specs = [
        ("body prose", LayoutTokenKind::Word, DocumentRegion::Body),
        ("[formula]", LayoutTokenKind::Formula, DocumentRegion::Body),
        ("[image]", LayoutTokenKind::Image, DocumentRegion::Body),
        ("annex note", LayoutTokenKind::Word, DocumentRegion::Annex),
        (
            "reference",
            LayoutTokenKind::Word,
            DocumentRegion::References,
        ),
        (
            "running",
            LayoutTokenKind::Word,
            DocumentRegion::PageFurniture,
        ),
        ("title", LayoutTokenKind::Word, DocumentRegion::Header),
    ];
    let tokens = specs
        .iter()
        .enumerate()
        .map(|(i, (text, kind, _))| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0, 10.0 + i as f64 * 10.0, 90.0, 18.0 + i as f64 * 10.0)
                    .unwrap(),
                digest(text),
                LayoutSpan::absent(LayoutSpanAbsence::NonText),
                *kind,
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "body-fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let assignments = specs
        .iter()
        .enumerate()
        .map(|(i, (text, _, region))| TokenAssignment {
            page_ordinal: 0,
            source_ordinal: i as u32,
            token_ordinal: i as u32,
            text_digest: digest(text),
            region: *region,
        })
        .collect();
    let segmentation = SegmentationOutcome {
        schema_version: da_parse::NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        document_id: "body-fixture".to_owned(),
        provider_id: "body-fixture".to_owned(),
        assignments,
        sections: Vec::new(),
        boundary_digest: "sha256:boundary".to_owned(),
    };
    (document, segmentation)
}

#[test]
fn exact_partition_keeps_body_annex_and_opaque_visuals_separate() {
    let (document, segmentation) = fixture();
    let accounting = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .unwrap();

    assert_eq!(accounting.schema_version, NATIVE_BODY_SCHEMA_VERSION);
    assert_eq!(accounting.admitted, 7);
    assert_eq!(accounting.conserved, 7);
    assert_eq!(accounting.unsupported, 1);
    assert_eq!(accounting.loss, 0);
    assert_eq!(accounting.assignments, segmentation.assignments);
    assert_eq!(accounting.units.len(), 7);
    assert_eq!(accounting.units[1].kind, AccountedKind::Formula);
    assert_eq!(accounting.units[2].kind, AccountedKind::Image);
    assert_eq!(accounting.units[1].bucket, AccountingBucket::VisualContent);
    assert_eq!(accounting.units[2].bucket, AccountingBucket::VisualContent);
    assert_eq!(
        accounting.buckets[&AccountingBucket::BodyBlocks].admitted,
        1
    );
    assert_eq!(
        accounting.buckets[&AccountingBucket::VisualContent].conserved,
        2
    );
    assert_eq!(accounting.buckets[&AccountingBucket::Annex].admitted, 1);
    assert_eq!(
        accounting.buckets[&AccountingBucket::References].admitted,
        1
    );
    assert_eq!(
        accounting.buckets[&AccountingBucket::PageFurniture].admitted,
        1
    );
    assert_eq!(
        accounting.buckets[&AccountingBucket::Unsupported].unsupported,
        1
    );
}

#[test]
fn replay_is_deterministic_and_does_not_serialize_raw_payload() {
    let (document, segmentation) = fixture();
    let accountant = NativeBodyAccountant::new();
    let first = accountant.account(&document, &segmentation).unwrap();
    let encoded = serde_json::to_string(&first).unwrap();
    let decoded: da_parse::NativeBodyAccounting = serde_json::from_str(&encoded).unwrap();
    assert_eq!(first, decoded);
    assert_eq!(encoded, serde_json::to_string(&decoded).unwrap());
    assert!(!encoded.contains("body prose"));
    assert!(!encoded.contains("annex note"));
}
