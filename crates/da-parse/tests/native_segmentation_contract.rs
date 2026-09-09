use da_parse::{
    CoordinateDialect, DocumentRegion, FullSegmentationError, FullSemanticRegion,
    FurnitureEvidence, FurnitureKind, LatticeBlock, LatticeLine, LatticePage, LatticeToken,
    LayoutBBox, LayoutDocument, LayoutLattice, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind, LexemeRow, ReadingOrder, SegmentationConfig,
    SegmentationError, segment_document, segment_document_full,
};
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}
fn token(page: u32, source: u32, ordinal: u32, text: &str, y: f64) -> LayoutToken {
    LayoutToken::new(
        page,
        source,
        ordinal,
        LayoutBBox::new(20.0, y, 80.0, y + 10.0).unwrap(),
        digest(text),
        LayoutSpan::absent(da_parse::LayoutSpanAbsence::NotAvailable),
        match text {
            "[formula]" => LayoutTokenKind::Formula,
            "[image]" => LayoutTokenKind::Image,
            _ => LayoutTokenKind::from_lexical_text(text),
        },
        LayoutStyleHints::default(),
        provider(),
    )
    .unwrap()
}
fn document(items: &[(&str, f64)]) -> LayoutDocument {
    let tokens = items
        .iter()
        .enumerate()
        .map(|(i, (text, y))| token(0, i as u32, i as u32, text, *y))
        .collect();
    LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens).unwrap()],
    )
    .unwrap()
}
fn lattice_for(doc: &LayoutDocument) -> LayoutLattice {
    let tokens = doc.pages[0]
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
        .collect();
    LayoutLattice::new(
        doc,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 200.0,
            height: 300.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, doc.pages[0].tokens.len() as u32).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, doc.pages[0].tokens.len() as u32).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                    tokens,
                    newline_boundary_count: 1,
                }],
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Classified {
                    kind: FurnitureKind::Header,
                },
            }],
        }],
    )
    .unwrap()
}

fn lexemes(texts: &[&str]) -> Vec<LexemeRow> {
    texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                "fixture-doc",
                "fixture-donor",
                0,
                i as u32,
                digest(text),
                *text,
            )
        })
        .collect()
}

#[test]
fn rejects_missing_duplicate_and_digest_mismatched_lexemes() {
    let doc = document(&[("Title", 40.0), ("Introduction", 80.0)]);
    let rows = lexemes(&["Title"]);
    assert!(matches!(
        segment_document(&doc, &rows, SegmentationConfig::default()),
        Err(SegmentationError::MissingLexeme { page: 0, source: 1 })
    ));

    let mut duplicate = lexemes(&["Title", "Introduction"]);
    duplicate.push(duplicate[1].clone());
    assert!(matches!(
        segment_document(&doc, &duplicate, SegmentationConfig::default()),
        Err(SegmentationError::DuplicateLexeme { page: 0, source: 1 })
    ));

    let mut bad = lexemes(&["Title", "Introduction"]);
    bad[1] = LexemeRow::new(
        "fixture-doc",
        "fixture-donor",
        0,
        1,
        digest("wrong"),
        "wrong",
    );
    assert!(matches!(
        segment_document(&doc, &bad, SegmentationConfig::default()),
        Err(SegmentationError::DigestMismatch { page: 0, source: 1 })
    ));
}

#[test]
fn rejects_unmatched_sidecar_and_invalid_bounds() {
    let doc = document(&[("Title", 40.0)]);
    let mut rows = lexemes(&["Title"]);
    rows.push(LexemeRow::new(
        "fixture-doc",
        "fixture-donor",
        0,
        99,
        digest("extra"),
        "extra",
    ));
    assert!(matches!(
        segment_document(&doc, &rows, SegmentationConfig::default()),
        Err(SegmentationError::InvalidLexeme)
    ));

    let mut config = SegmentationConfig::default();
    config.limits.max_tokens = 0;
    assert!(matches!(
        segment_document(&doc, &lexemes(&["Title"]), config),
        Err(SegmentationError::TooManyTokens { count: 1 })
    ));
    config = SegmentationConfig::default();
    config.limits.max_regions = 4;
    assert!(matches!(
        segment_document(&doc, &lexemes(&["Title"]), config),
        Err(SegmentationError::InvalidConfig)
    ));
}

#[test]
fn mixed_word_formula_image_assigns_exactly_one_region_per_token() {
    let tokens = vec![
        token(0, 0, 0, "Title", 20.0),
        token(0, 1, 1, "[formula]", 40.0),
        token(0, 2, 2, "body", 60.0),
        token(0, 3, 3, "[image]", 80.0),
    ];
    let doc = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let rows = vec![
        LexemeRow::new(
            "fixture-doc",
            "fixture-donor",
            0,
            0,
            digest("Title"),
            "Title",
        ),
        LexemeRow::new("fixture-doc", "fixture-donor", 0, 2, digest("body"), "body"),
    ];
    // Sidecar covers lexical tokens only; visual kinds stay opaque.
    let outcome = segment_document(&doc, &rows, SegmentationConfig::default()).unwrap();
    let admitted = doc.pages.iter().map(|p| p.tokens.len()).sum::<usize>();
    assert_eq!(outcome.assignments.len(), admitted);
    assert_eq!(admitted, 4);
    let keys: Vec<_> = outcome
        .assignments
        .iter()
        .map(|a| (a.source_ordinal, a.region))
        .collect();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0].0, 0);
    assert_eq!(keys[1].0, 1);
    assert_eq!(keys[2].0, 2);
    assert_eq!(keys[3].0, 3);
    for assignment in &outcome.assignments {
        assert!(matches!(
            assignment.region,
            DocumentRegion::Header
                | DocumentRegion::Body
                | DocumentRegion::References
                | DocumentRegion::Annex
                | DocumentRegion::PageFurniture
        ));
    }
    assert!(doc.pages[0].tokens[1].kind.is_visual());
    assert!(doc.pages[0].tokens[3].kind.is_visual());
    assert!(doc.pages[0].tokens[0].kind.is_lexical());
    assert!(doc.pages[0].tokens[2].kind.is_lexical());
}

#[test]
fn full_companion_is_lattice_bound_and_keeps_furniture_out_of_body_runs() {
    let doc = document(&[("Title", 40.0), ("Introduction", 80.0)]);
    let lattice = lattice_for(&doc);
    let outcome = segment_document_full(
        &doc,
        &lattice,
        &lexemes(&["Title", "Introduction"]),
        SegmentationConfig::default(),
    )
    .unwrap();

    assert_eq!(outcome.schema_version, "da-parse.native-segmentation/v2");
    assert_eq!(outcome.assignments.len(), 2);
    assert_eq!(outcome.furniture.len(), 2);
    assert!(
        outcome
            .assignments
            .iter()
            .all(|a| a.region == FullSemanticRegion::PageFurniture)
    );
    assert!(
        outcome
            .dispositions
            .iter()
            .any(|d| d.region == FullSemanticRegion::Body && d.absence.is_some())
    );
    assert_eq!(
        outcome,
        segment_document_full(
            &doc,
            &lattice,
            &lexemes(&["Title", "Introduction"]),
            SegmentationConfig::default(),
        )
        .unwrap()
    );
    let encoded = serde_json::to_vec(&outcome).unwrap();
    assert_eq!(outcome, serde_json::from_slice(&encoded).unwrap());
}

#[test]
fn full_companion_rejects_lattice_identity_drift_without_touching_v1() {
    let doc = document(&[("Title", 40.0)]);
    let mut lattice = lattice_for(&doc);
    lattice.pages[0].java_page_number = 2;
    assert!(matches!(
        segment_document_full(
            &doc,
            &lattice,
            &lexemes(&["Title"]),
            SegmentationConfig::default(),
        ),
        Err(FullSegmentationError::LatticeInvalid)
    ));
    assert!(segment_document(&doc, &lexemes(&["Title"]), SegmentationConfig::default()).is_ok());
}

#[test]
fn formula_is_opaque_and_debug_does_not_expose_lexeme_text() {
    let doc = document(&[("[formula]", 40.0)]);
    let row = LexemeRow::new(
        "fixture-doc",
        "fixture-donor",
        0,
        0,
        digest("secret-lexeme"),
        "secret-lexeme",
    );
    let debug = format!("{row:?}");
    assert!(!debug.contains("secret-lexeme"));
    // Visual tokens have no lexical join row; they remain opaque and conserved.
    let outcome = segment_document(&doc, &[], SegmentationConfig::default()).unwrap();
    assert_eq!(outcome.assignments.len(), 1);
    assert!(matches!(
        outcome.assignments[0].region,
        DocumentRegion::Header
    ));
}

#[test]
fn serde_replay_is_stable_and_error_is_redacted() {
    let doc = document(&[("Title", 40.0), ("Introduction", 80.0)]);
    let outcome = segment_document(
        &doc,
        &lexemes(&["Title", "Introduction"]),
        SegmentationConfig::default(),
    )
    .unwrap();
    let encoded = serde_json::to_string(&outcome).unwrap();
    let decoded: da_parse::SegmentationOutcome = serde_json::from_str(&encoded).unwrap();
    assert_eq!(outcome, decoded);
    assert_eq!(outcome.boundary_digest, decoded.boundary_digest);
    let error = SegmentationError::DigestMismatch { page: 0, source: 1 };
    assert!(!error.to_string().contains("Introduction"));
}
