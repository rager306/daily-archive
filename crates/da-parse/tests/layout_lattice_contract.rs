use da_parse::{
    CoordinateDialect, FurnitureEvidence, FurnitureKind, LatticeBlock, LatticeLine, LatticePage,
    LatticeToken, LayoutBBox, LayoutDocument, LayoutLattice, LayoutLatticeError, LayoutPage,
    LayoutProvider, LayoutSpan, LayoutStyleHints, LayoutToken, LayoutTokenKind, ReadingOrder,
};

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture", "1", "a".repeat(64)).unwrap()
}

fn token(source: u32, ordinal: u32, span: (u32, u32), kind: LayoutTokenKind) -> LayoutToken {
    LayoutToken::new(
        0,
        source,
        ordinal,
        LayoutBBox::new(
            10.0 + source as f64 * 30.0,
            10.0,
            25.0 + source as f64 * 30.0,
            25.0,
        )
        .unwrap(),
        format!("{source:064x}"),
        LayoutSpan::present(span.0, span.1).unwrap(),
        kind,
        LayoutStyleHints::default(),
        provider(),
    )
    .unwrap()
}

fn document(tokens: Vec<LayoutToken>) -> LayoutDocument {
    LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 612.0, 792.0, 0, None, tokens).unwrap()],
    )
    .unwrap()
}

fn lattice_for(doc: &LayoutDocument) -> da_parse::LayoutLattice {
    let page = LatticePage {
        page_ordinal: 0,
        java_page_number: 1,
        width: 612.0,
        height: 792.0,
        rotation: 0,
        crop_box: None,
        blocks: vec![LatticeBlock {
            source_ordinal: 0,
            source_span: LayoutSpan::present(0, 2).unwrap(),
            bbox: LayoutBBox::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            source_order: 0,
            derived_order: Some(0),
            furniture: FurnitureEvidence::Classified {
                kind: FurnitureKind::Header,
            },
            lines: vec![LatticeLine {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 2).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 100.0, 100.0).unwrap(),
                newline_boundary_count: 1,
                tokens: doc.pages[0]
                    .tokens
                    .iter()
                    .map(|t| LatticeToken {
                        source_ordinal: t.source_ordinal,
                        token_ordinal: t.token_ordinal,
                        bbox: t.bbox,
                        text_digest: t.text_digest.clone(),
                        source_span: t.source_span.clone(),
                        kind: t.kind,
                    })
                    .collect(),
            }],
        }],
    };
    LayoutLattice::new(
        doc,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![page],
    )
    .unwrap()
}

#[test]
fn companion_preserves_identity_mapping_and_replay_digest() {
    let doc = document(vec![
        token(0, 0, (0, 1), LayoutTokenKind::Word),
        token(1, 1, (1, 2), LayoutTokenKind::Whitespace),
    ]);
    let lattice = lattice_for(&doc);
    assert_eq!(LayoutLattice::java_page_to_rust(1).unwrap(), 0);
    assert!(LayoutLattice::java_page_to_rust(0).is_err());
    assert_eq!(lattice.identity.document_id, "fixture-doc");
    assert_eq!(lattice.digest().unwrap(), lattice.digest().unwrap());
    let roundtrip: LayoutLattice =
        serde_json::from_slice(&serde_json::to_vec(&lattice).unwrap()).unwrap();
    assert_eq!(roundtrip, lattice);
}

#[test]
fn furniture_is_typed_and_does_not_change_body_token_admission() {
    let doc = document(vec![token(0, 0, (0, 1), LayoutTokenKind::Word)]);
    let lattice = lattice_for(&doc);
    assert!(matches!(
        lattice.pages[0].blocks[0].furniture,
        FurnitureEvidence::Classified {
            kind: FurnitureKind::Header
        }
    ));
    assert_eq!(lattice.pages[0].blocks[0].lines[0].tokens.len(), 1);
    assert!(matches!(
        FurnitureEvidence::Absent,
        FurnitureEvidence::Absent
    ));
    assert!(matches!(
        FurnitureEvidence::Unknown,
        FurnitureEvidence::Unknown
    ));
}

#[test]
fn malformed_mapping_order_spans_geometry_and_visuals_fail_closed() {
    let doc = document(vec![
        token(0, 0, (0, 1), LayoutTokenKind::Word),
        token(1, 1, (1, 2), LayoutTokenKind::Word),
    ]);
    let mut lattice = lattice_for(&doc);
    lattice.pages[0].java_page_number = 2;
    assert!(matches!(
        lattice.validate_against(&doc),
        Err(LayoutLatticeError::PageMappingMismatch { .. })
    ));

    let mut reordered = lattice_for(&doc);
    reordered.pages[0].java_page_number = 1;
    reordered.pages[0].blocks[0].lines[0].tokens.swap(0, 1);
    assert!(matches!(
        reordered.validate_against(&doc),
        Err(LayoutLatticeError::NonMonotonic { kind: "token", .. })
    ));

    let mut invalid_bbox = lattice_for(&doc);
    invalid_bbox.pages[0].blocks[0].bbox = LayoutBBox::new(600.0, 0.0, 613.0, 10.0).unwrap();
    invalid_bbox.pages[0].blocks[0].lines[0].bbox = invalid_bbox.pages[0].blocks[0].bbox;
    assert!(matches!(
        invalid_bbox.validate_against(&doc),
        Err(LayoutLatticeError::PageGeometry(0))
    ));

    let visual_doc = document(vec![token(0, 0, (0, 1), LayoutTokenKind::Word)]);
    let mut visual_lattice = lattice_for(&visual_doc);
    visual_lattice.pages[0].blocks[0].lines[0].tokens[0].kind = LayoutTokenKind::Image;
    assert!(matches!(
        visual_lattice.validate_against(&visual_doc),
        Err(LayoutLatticeError::VisualTokenInLexicalLattice { .. })
    ));
}

#[test]
fn deletion_duplication_and_overlap_are_not_silent() {
    let doc = document(vec![
        token(0, 0, (0, 1), LayoutTokenKind::Word),
        token(1, 1, (1, 2), LayoutTokenKind::Word),
    ]);
    let mut deleted = lattice_for(&doc);
    deleted.pages[0].blocks[0].lines[0].tokens.pop();
    assert!(matches!(
        deleted.validate_against(&doc),
        Err(LayoutLatticeError::MissingToken { source: 1, .. })
    ));

    let mut duplicated = lattice_for(&doc);
    let duplicate = duplicated.pages[0].blocks[0].lines[0].tokens[0].clone();
    duplicated.pages[0].blocks[0].lines[0]
        .tokens
        .push(duplicate);
    assert!(matches!(
        duplicated.validate_against(&doc),
        Err(LayoutLatticeError::NonMonotonic { kind: "token", .. })
            | Err(LayoutLatticeError::SpanOverlap { kind: "token", .. })
            | Err(LayoutLatticeError::DuplicateToken { .. })
    ));

    let mut overlap = lattice_for(&doc);
    overlap.pages[0].blocks[0].lines[0].tokens[1].source_span = LayoutSpan::present(0, 2).unwrap();
    assert!(matches!(
        overlap.validate_against(&doc),
        Err(LayoutLatticeError::SpanOverlap { kind: "token", .. })
    ));
}
