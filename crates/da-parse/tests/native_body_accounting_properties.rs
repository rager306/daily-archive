use da_parse::{
    DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind, NativeBodyAccountant, NativeBodyError,
    NativeBodyLimits, SegmentationOutcome, TokenAssignment,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn fixture() -> (LayoutDocument, SegmentationOutcome) {
    let provider = LayoutProvider::new("property-fixture", "1", "a".repeat(64)).unwrap();
    let tokens = (0..3)
        .map(|i| {
            LayoutToken::new(
                0,
                i,
                i,
                LayoutBBox::new(1.0, i as f64 + 1.0, 20.0, i as f64 + 2.0).unwrap(),
                digest(&format!("token-{i}")),
                LayoutSpan::absent(da_parse::LayoutSpanAbsence::NonText),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "property-fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 100.0, 100.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let assignments = (0..3)
        .map(|i| TokenAssignment {
            page_ordinal: 0,
            source_ordinal: i,
            token_ordinal: i,
            text_digest: digest(&format!("token-{i}")),
            region: DocumentRegion::Body,
        })
        .collect();
    let segmentation = SegmentationOutcome {
        schema_version: da_parse::NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        document_id: "property-fixture".into(),
        provider_id: "property-fixture".into(),
        assignments,
        sections: vec![],
        boundary_digest: "sha256:boundary".into(),
    };
    (document, segmentation)
}

#[test]
fn rejects_empty_identity_and_checksum_mismatch_without_payload_in_diagnostics() {
    let (document, mut segmentation) = fixture();
    segmentation.document_id.clear();
    let error = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .unwrap_err();
    assert!(matches!(error, NativeBodyError::MissingIdentity { .. }));
    assert!(!error.to_string().contains("token-"));

    let (mut document, segmentation) = fixture();
    document.source_pdf_sha256 = "not-a-digest".into();
    let error = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .unwrap_err();
    assert!(matches!(error, NativeBodyError::SourceDigestMismatch));

    let (document, mut segmentation) = fixture();
    segmentation.boundary_digest = "bad-checksum".into();
    let error = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .unwrap_err();
    assert!(matches!(
        error,
        NativeBodyError::ChecksumMismatch {
            field: "boundary_digest"
        }
    ));
}

#[test]
fn rejects_duplicate_overlap_and_malformed_assignment_ranges() {
    let (document, mut segmentation) = fixture();
    segmentation.assignments[1].source_ordinal = 0;
    assert!(matches!(
        NativeBodyAccountant::new().account(&document, &segmentation),
        Err(NativeBodyError::DuplicateAssignment { .. })
    ));

    let (_, mut segmentation) = fixture();
    segmentation.assignments[1].page_ordinal = 0;
    segmentation.assignments[1].token_ordinal = 0;
    segmentation.assignments[1].source_ordinal = 1;
    assert!(matches!(
        NativeBodyAccountant::new().account(&document, &segmentation),
        Err(NativeBodyError::Overlap { .. }) | Err(NativeBodyError::MalformedRange { .. })
    ));

    let (_, mut segmentation) = fixture();
    segmentation.assignments[1].token_ordinal = 99;
    assert!(matches!(
        NativeBodyAccountant::new().account(&document, &segmentation),
        Err(NativeBodyError::MalformedRange { .. })
    ));
}

#[test]
fn rejects_configured_oversize_without_serializing_payload() {
    let (document, segmentation) = fixture();
    let error = NativeBodyAccountant::new()
        .account_with_limits(
            &document,
            &segmentation,
            NativeBodyLimits { max_assignments: 2 },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        NativeBodyError::Oversize { count: 3, max: 2 }
    ));
    assert!(!error.to_string().contains("token-"));
}

#[test]
fn replay_is_stable_for_generated_assignment_orders() {
    let (document, segmentation) = fixture();
    let accountant = NativeBodyAccountant::new();
    let first = accountant.account(&document, &segmentation).unwrap();
    let bytes = serde_json::to_vec(&first).unwrap();
    let replay: da_parse::NativeBodyAccounting = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(first, replay);
}

#[test]
fn bounded_accounting_preserves_conservation_and_ordered_identity() {
    let (document, segmentation) = fixture();
    let accounting = NativeBodyAccountant::new()
        .account(&document, &segmentation)
        .unwrap();

    assert_eq!(accounting.admitted, segmentation.assignments.len());
    assert_eq!(accounting.conserved, accounting.admitted);
    assert_eq!(accounting.loss, 0);
    assert_eq!(accounting.units.len(), segmentation.assignments.len());
    assert_eq!(accounting.units[0].source_ordinal, 0);
    assert_eq!(accounting.units[2].source_ordinal, 2);
}

proptest! {
    #[test]
    fn malformed_token_ordinals_never_panic(ordinal in any::<u32>()) {
        let (document, mut segmentation) = fixture();
        segmentation.assignments[1].token_ordinal = ordinal;
        let _ = NativeBodyAccountant::new().account(&document, &segmentation);
    }

    #[test]
    fn bounded_assignment_limits_never_admit_more_than_configured(max in 0usize..4) {
        let (document, segmentation) = fixture();
        let result = NativeBodyAccountant::new().account_with_limits(
            &document,
            &segmentation,
            NativeBodyLimits { max_assignments: max },
        );
        if max < segmentation.assignments.len() {
            prop_assert!(result.is_err());
        } else {
            prop_assert!(result.is_ok());
        }
    }
}
