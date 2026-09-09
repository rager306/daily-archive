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
    let p = LayoutProvider::new("property", "1", "a".repeat(64)).unwrap();
    let words = ["Introduction", "body", "Annex"];
    let tokens: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0 + i as f64 * 20.0, 10.0, 25.0 + i as f64 * 20.0, 25.0)
                    .unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                p.clone(),
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
    let lattice_tokens = tokens
        .iter()
        .map(|t| LatticeToken {
            source_ordinal: t.source_ordinal,
            token_ordinal: t.token_ordinal,
            bbox: t.bbox,
            text_digest: t.text_digest.clone(),
            source_span: t.source_span.clone(),
            kind: t.kind,
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
                source_span: LayoutSpan::present(0, 3).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 200.0, 100.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 3).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 200.0, 100.0).unwrap(),
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

proptest! {
    #[test]
    fn replay_is_byte_stable_and_worker_independent(max_blocks in 1usize..=MAX_FULLTEXT_BLOCKS) {
        let (doc, lattice, segmentation, rows) = fixture();
        let receipt = account_full_regions(&doc, &lattice, &segmentation, NativeBodyLimits::default()).unwrap();
        let config = NativeFulltextBlocksConfig { max_blocks, ..Default::default() };
        let first = run_native_fulltext_blocks(&receipt, &lattice, &rows, &config);
        let second = run_native_fulltext_blocks(&receipt, &lattice, &rows, &config);
        prop_assert_eq!(&first, &second);
        if max_blocks < 2 {
            let oversize = matches!(&first, Err(NativeFulltextBlocksError::Oversize { .. }));
            prop_assert!(oversize);
        } else {
            let first_bytes = serde_json::to_vec(first.as_ref().unwrap()).unwrap();
            let second_bytes = serde_json::to_vec(second.as_ref().unwrap()).unwrap();
            prop_assert_eq!(first_bytes, second_bytes);
        }
    }
}

#[test]
fn split_merge_and_terminal_conditions_are_typed() {
    let (doc, lattice, segmentation, rows) = fixture();
    let receipt =
        account_full_regions(&doc, &lattice, &segmentation, NativeBodyLimits::default()).unwrap();
    let baseline = run_native_fulltext_blocks(
        &receipt,
        &lattice,
        &rows,
        &NativeFulltextBlocksConfig::default(),
    )
    .unwrap();
    let mut split = NativeFulltextBlocksConfig::default();
    split.role_hints.push(FulltextRoleHint {
        page_ordinal: 0,
        source_ordinal: 1,
        role: FulltextLeafRole::Paragraph,
        marker_subtype: None,
    });
    let changed = run_native_fulltext_blocks(&receipt, &lattice, &rows, &split).unwrap();
    assert_eq!(baseline.leaves.len(), changed.leaves.len());
    assert_eq!(baseline.annex_runs, changed.annex_runs);

    let mut duplicate = split.clone();
    duplicate.role_hints.push(duplicate.role_hints[0].clone());
    assert!(matches!(
        run_native_fulltext_blocks(&receipt, &lattice, &rows, &duplicate),
        Err(NativeFulltextBlocksError::DuplicateHint { .. })
    ));

    let missing = &rows[..2];
    assert!(matches!(
        run_native_fulltext_blocks(
            &receipt,
            &lattice,
            missing,
            &NativeFulltextBlocksConfig::default()
        ),
        Err(NativeFulltextBlocksError::InvalidAccountantReceipt)
    ));
}
