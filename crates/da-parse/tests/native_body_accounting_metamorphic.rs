use da_parse::{
    AccountedKind, AccountingBucket, DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage,
    LayoutProvider, LayoutSpan, LayoutSpanAbsence, LayoutStyleHints, LayoutToken, LayoutTokenKind,
    NativeBodyAccountant, NativeBodyError, SegmentationOutcome, TokenAssignment,
};
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn fixture() -> (LayoutDocument, SegmentationOutcome) {
    let provider = LayoutProvider::new("metamorphic-fixture", "1", "a".repeat(64)).unwrap();
    let specs = [
        ("prose", LayoutTokenKind::Word, DocumentRegion::Body),
        ("formula", LayoutTokenKind::Formula, DocumentRegion::Body),
        ("image", LayoutTokenKind::Image, DocumentRegion::Body),
        ("annex", LayoutTokenKind::Other, DocumentRegion::Annex),
        (
            "reference",
            LayoutTokenKind::Word,
            DocumentRegion::References,
        ),
    ];
    let tokens = specs
        .iter()
        .enumerate()
        .map(|(ordinal, (text, kind, _))| {
            LayoutToken::new(
                0,
                ordinal as u32,
                ordinal as u32,
                LayoutBBox::new(
                    10.0,
                    10.0 + ordinal as f64 * 10.0,
                    90.0,
                    18.0 + ordinal as f64 * 10.0,
                )
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
        "metamorphic-fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let assignments = specs
        .iter()
        .enumerate()
        .map(|(ordinal, (text, _, region))| TokenAssignment {
            page_ordinal: 0,
            source_ordinal: ordinal as u32,
            token_ordinal: ordinal as u32,
            text_digest: digest(text),
            region: *region,
        })
        .collect();
    let segmentation = SegmentationOutcome {
        schema_version: da_parse::NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        document_id: "metamorphic-fixture".to_owned(),
        provider_id: "metamorphic-fixture".to_owned(),
        assignments,
        sections: Vec::new(),
        boundary_digest: "sha256:boundary".to_owned(),
    };
    (document, segmentation)
}

fn account(
    document: &LayoutDocument,
    segmentation: &SegmentationOutcome,
) -> Result<da_parse::NativeBodyAccounting, NativeBodyError> {
    NativeBodyAccountant::new().account(document, segmentation)
}

#[test]
fn deletion_is_not_silently_conserved() {
    let (document, mut segmentation) = fixture();
    segmentation.assignments.remove(2);

    assert!(matches!(
        account(&document, &segmentation),
        Err(NativeBodyError::AssignmentCountMismatch {
            expected: 5,
            actual: 4
        })
    ));
}

#[test]
fn duplication_is_rejected_even_when_the_count_is_repaired() {
    let (document, mut segmentation) = fixture();
    segmentation.assignments[3] = segmentation.assignments[2].clone();

    assert!(matches!(
        account(&document, &segmentation),
        Err(NativeBodyError::DuplicateAssignment { .. })
    ));
}

#[test]
fn count_equal_reordering_is_detected_by_ordered_identity() {
    let (document, mut segmentation) = fixture();
    segmentation.assignments.swap(1, 2);

    assert_eq!(segmentation.assignments.len(), 5);
    assert!(matches!(
        account(&document, &segmentation),
        Err(NativeBodyError::MalformedOrder { .. })
    ));
}

#[test]
fn allowed_split_and_merge_preserve_total_but_change_partition() {
    let (document, mut segmentation) = fixture();
    let baseline = account(&document, &segmentation).unwrap();
    let baseline_body = baseline.buckets[&AccountingBucket::BodyBlocks].admitted;
    let baseline_annex = baseline.buckets[&AccountingBucket::Annex].admitted;

    // A segmentation split may move a lexical unit to the annex; the unit is
    // still source-bound and conserved, while the partition changes.
    segmentation.assignments[0].region = DocumentRegion::Annex;
    let split = account(&document, &segmentation).unwrap();
    assert_eq!(split.admitted, baseline.admitted);
    assert_eq!(split.conserved, baseline.conserved);
    assert_eq!(split.loss, 0);
    assert_eq!(
        split
            .buckets
            .get(&AccountingBucket::BodyBlocks)
            .map_or(0, |count| count.admitted),
        baseline_body - 1,
    );
    assert_eq!(
        split
            .buckets
            .get(&AccountingBucket::Annex)
            .map_or(0, |count| count.admitted),
        baseline_annex + 1,
    );

    // Merging the same unit back restores the original bucket counts.
    segmentation.assignments[0].region = DocumentRegion::Body;
    let merged = account(&document, &segmentation).unwrap();
    assert_eq!(merged, baseline);
}

#[test]
fn unknown_layout_kind_is_lexical_and_never_dropped() {
    let (document, segmentation) = fixture();
    let accounting = account(&document, &segmentation).unwrap();
    let unit = &accounting.units[3];

    assert_eq!(unit.kind, AccountedKind::Lexical);
    assert_eq!(unit.bucket, AccountingBucket::Annex);
    assert_eq!(accounting.admitted, 5);
    assert_eq!(accounting.conserved, 5);
}

#[test]
fn opaque_visual_tokens_preserve_identity_without_becoming_prose() {
    let (document, segmentation) = fixture();
    let accounting = account(&document, &segmentation).unwrap();

    assert_eq!(accounting.units[1].kind, AccountedKind::Formula);
    assert_eq!(accounting.units[2].kind, AccountedKind::Image);
    assert_eq!(accounting.units[1].bucket, AccountingBucket::VisualContent);
    assert_eq!(accounting.units[2].bucket, AccountingBucket::VisualContent);
    assert_eq!(accounting.units[1].text_digest, digest("formula"));
    assert_eq!(accounting.units[2].text_digest, digest("image"));
}

#[test]
fn metamorphic_replay_is_byte_identical() {
    let (document, segmentation) = fixture();
    let accounting = account(&document, &segmentation).unwrap();
    let first = serde_json::to_vec(&accounting).unwrap();
    let replay: da_parse::NativeBodyAccounting = serde_json::from_slice(&first).unwrap();

    assert_eq!(first, serde_json::to_vec(&replay).unwrap());
}
