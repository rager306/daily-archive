use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture", "1", "a".repeat(64)).unwrap()
}

fn fixture() -> (
    LayoutDocument,
    LayoutLattice,
    FullSegmentationOutcome,
    Vec<LexemeRow>,
) {
    let texts = ["Introduction", "prose", "Annex", "citation"];
    let tokens = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0 + i as f64 * 30.0, 10.0, 30.0 + i as f64 * 30.0, 25.0)
                    .unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let document = LayoutDocument::new(
        "fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 612.0, 792.0, 0, None, tokens.clone()).unwrap()],
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
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 612.0,
            height: 792.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 4).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 200.0, 100.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 4).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 200.0, 100.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes = texts
        .iter()
        .enumerate()
        .map(|(i, text)| LexemeRow::new("fixture", "fixture", 0, i as u32, digest(text), *text))
        .collect::<Vec<_>>();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    (document, lattice, segmentation, lexemes)
}

#[test]
fn accountant_bound_sequence_preserves_conservation_and_annex_parent_scope() {
    let (document, lattice, segmentation, lexemes) = fixture();
    let receipt = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    assert_eq!(receipt.admitted, receipt.conserved);
    assert_eq!(receipt.loss, 0);
    let config = NativeFulltextBlocksConfig {
        role_hints: vec![
            FulltextRoleHint {
                page_ordinal: 0,
                source_ordinal: 1,
                role: FulltextLeafRole::Paragraph,
                marker_subtype: None,
            },
            FulltextRoleHint {
                page_ordinal: 0,
                source_ordinal: 3,
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Citation),
            },
        ],
        config_fingerprint: "sha256:config".into(),
        ..Default::default()
    };
    let first = run_native_fulltext_blocks(&receipt, &lattice, &lexemes, &config).unwrap();
    let second = run_native_fulltext_blocks(&receipt, &lattice, &lexemes, &config).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.annex_runs.len(), 1);
    assert!(
        first
            .leaves
            .iter()
            .any(|l| l.role == FulltextLeafRole::Paragraph)
    );
    let marker = first.leaves.iter().find(|l| l.source_ordinal == 3).unwrap();
    assert_eq!(marker.marker_subtype, Some(MarkerSubtype::Citation));
    assert!(marker.parent_annex.is_some());
}

#[test]
fn serialization_is_digest_only_and_visuals_are_not_lexical() {
    let (document, lattice, segmentation, lexemes) = fixture();
    let receipt = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    let output = run_native_fulltext_blocks(
        &receipt,
        &lattice,
        &lexemes,
        &NativeFulltextBlocksConfig::default(),
    )
    .unwrap();
    let encoded = serde_json::to_string(&output).unwrap();
    assert!(!encoded.contains("Introduction") && !encoded.contains("citation"));
    assert!(encoded.contains("text_digest"));
    assert_eq!(receipt.visual_evidence.iter().filter(|u| u.kind == FullRegionUnitKind::Image || u.kind == FullRegionUnitKind::Formula).count(), 0);
    assert_eq!(
        document.pages[0]
            .tokens
            .iter()
            .filter(|t| t.kind.is_visual())
            .count(),
        0
    );
}

#[test]
fn note_and_marker_contracts_fail_closed() {
    let (document, lattice, segmentation, lexemes) = fixture();
    let receipt = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    let note = NativeFulltextBlocksConfig {
        role_hints: vec![FulltextRoleHint {
            page_ordinal: 0,
            source_ordinal: 1,
            role: FulltextLeafRole::Note,
            marker_subtype: None,
        }],
        ..Default::default()
    };
    assert!(matches!(
        run_native_fulltext_blocks(&receipt, &lattice, &lexemes, &note),
        Err(NativeFulltextBlocksError::NotePositiveInvention)
    ));
    let marker = NativeFulltextBlocksConfig {
        role_hints: vec![FulltextRoleHint {
            page_ordinal: 0,
            source_ordinal: 1,
            role: FulltextLeafRole::Marker,
            marker_subtype: None,
        }],
        ..Default::default()
    };
    assert!(matches!(
        run_native_fulltext_blocks(&receipt, &lattice, &lexemes, &marker),
        Err(NativeFulltextBlocksError::MarkerSubtypeMissing)
    ));
    let mut altered = receipt.clone();
    altered.lattice_fingerprint = "sha256:wrong".into();
    assert!(matches!(
        run_native_fulltext_blocks(
            &altered,
            &lattice,
            &lexemes,
            &NativeFulltextBlocksConfig::default()
        ),
        Err(NativeFulltextBlocksError::IdentityMismatch("lattice"))
    ));
}
