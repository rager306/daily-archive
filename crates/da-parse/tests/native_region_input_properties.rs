//! Property tests for the additive native region carrier (S04/T01).
//!
//! The properties that must hold for every call: selection is an ordered
//! subset of the S03 segmentation, the caller's region ordering is irrelevant,
//! the carrier is byte-stable on replay, and the joined text/digest is exact.

use da_parse::*;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn fixture() -> (
    LayoutDocument,
    LayoutLattice,
    FullSegmentationOutcome,
    Vec<LexemeRow>,
) {
    let provider = LayoutProvider::new("property", "1", "a".repeat(64)).unwrap();
    let words = [
        "Title",
        "Author",
        "introduction",
        "body",
        "references",
        "[1]",
        "Smith",
        "2020",
    ];
    let tokens: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let y = 10.0 + i as f64 * 12.0;
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(20.0, y, 80.0, y + 10.0).unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::from_lexical_text(text),
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let doc = LayoutDocument::new(
        "property",
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 400.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let count = tokens.len() as u32;
    let lattice_tokens = tokens
        .iter()
        .map(|token| LatticeToken {
            source_ordinal: token.source_ordinal,
            token_ordinal: token.token_ordinal,
            bbox: token.bbox,
            text_digest: token.text_digest.clone(),
            source_span: token.source_span.clone(),
            kind: token.kind,
        })
        .collect();
    let lattice = LayoutLattice::new(
        &doc,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 300.0,
            height: 400.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, count).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let rows = words
        .iter()
        .enumerate()
        .map(|(i, text)| LexemeRow::new("property", "property", 0, i as u32, digest(text), *text))
        .collect::<Vec<_>>();
    let full = segment_document_full(&doc, &lattice, &rows, SegmentationConfig::default()).unwrap();
    (doc, lattice, full, rows)
}

fn region_of(index: u8) -> FullSemanticRegion {
    match index % 9 {
        0 => FullSemanticRegion::Header,
        1 => FullSemanticRegion::Body,
        2 => FullSemanticRegion::References,
        3 => FullSemanticRegion::Annex,
        4 => FullSemanticRegion::Acknowledgement,
        5 => FullSemanticRegion::Toc,
        6 => FullSemanticRegion::Funding,
        7 => FullSemanticRegion::Availability,
        _ => FullSemanticRegion::PageFurniture,
    }
}

proptest! {
    #[test]
    fn carrier_is_an_ordered_subset_of_the_segmentation(
        header in any::<bool>(),
        body in any::<bool>(),
        references in any::<bool>(),
        annex in any::<bool>(),
        acknowledgement in any::<bool>(),
    ) {
        let (doc, lattice, segmentation, rows) = fixture();
        let mut regions = Vec::new();
        if header { regions.push(FullSemanticRegion::Header); }
        if body { regions.push(FullSemanticRegion::Body); }
        if references { regions.push(FullSemanticRegion::References); }
        if annex { regions.push(FullSemanticRegion::Annex); }
        if acknowledgement { regions.push(FullSemanticRegion::Acknowledgement); }
        if regions.is_empty() { regions.push(FullSemanticRegion::Header); }

        let first = build_native_region_carrier(&doc, &lattice, &rows, &segmentation, &regions).unwrap();
        let second = build_native_region_carrier(&doc, &lattice, &rows, &segmentation, &regions).unwrap();
        prop_assert_eq!(&first, &second);
        prop_assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );

        // Per requested region, the run is exactly the segmentation's tokens
        // for that region, in segmentation order; an empty region is a typed
        // absence.
        for region in &regions {
            let expected: Vec<(u32, u32)> = segmentation
                .assignments
                .iter()
                .filter(|assignment| assignment.region == *region)
                .map(|assignment| (assignment.location.page_ordinal, assignment.source_ordinal))
                .collect();
            match first.run(*region) {
                Some(run) => {
                    let got: Vec<(u32, u32)> = run
                        .tokens
                        .iter()
                        .map(|token| (token.location.page_ordinal, token.source_ordinal))
                        .collect();
                    prop_assert_eq!(got, expected);
                    prop_assert!(first.absence(*region).is_none());
                }
                None => {
                    prop_assert!(expected.is_empty(), "a run is missing for a non-empty region");
                    prop_assert!(first.absence(*region).is_some(), "an empty region must be a typed absence");
                }
            }
        }

        // Text and digest fidelity: the joined lexeme is the token's text and
        // the digest recomputes from the bytes.
        for run in &first.runs {
            for token in &run.tokens {
                let row = rows
                    .iter()
                    .find(|row| {
                        row.page_ordinal == token.location.page_ordinal
                            && row.source_ordinal == token.source_ordinal
                    })
                    .expect("every admitted token has a lexeme");
                prop_assert_eq!(token.text(), row.text());
                prop_assert_eq!(digest(token.text()), token.text_digest.clone());
            }
        }
    }

    #[test]
    fn request_order_and_duplicates_do_not_change_the_carrier(
        order in prop::collection::vec(0u8..9, 1..10)
    ) {
        let (doc, lattice, segmentation, rows) = fixture();
        let regions: Vec<FullSemanticRegion> = order.iter().map(|index| region_of(*index)).collect();
        let mut reversed = regions.clone();
        reversed.reverse();
        let forward = build_native_region_carrier(&doc, &lattice, &rows, &segmentation, &regions).unwrap();
        let backward = build_native_region_carrier(&doc, &lattice, &rows, &segmentation, &reversed).unwrap();
        prop_assert_eq!(forward, backward);
    }
}

#[test]
fn an_empty_region_request_is_typed_not_an_empty_carrier() {
    let (doc, lattice, segmentation, rows) = fixture();
    assert_eq!(
        build_native_region_carrier(&doc, &lattice, &rows, &segmentation, &[]),
        Err(NativeRegionError::EmptyRegionRequest)
    );
}
