use da_parse::{
    LAYOUT_SCHEMA_VERSION, LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind,
};

fn provider() -> LayoutProvider {
    LayoutProvider::new("test-donor", "1.0", "a".repeat(64)).unwrap()
}

fn token(source: u32, ordinal: u32, x: f64) -> LayoutToken {
    LayoutToken::new(
        0,
        source,
        ordinal,
        LayoutBBox::new(x, 10.0, x + 20.0, 22.0).unwrap(),
        "c".repeat(64),
        LayoutSpan::present(0, 5).unwrap(),
        LayoutTokenKind::Word,
        LayoutStyleHints::default(),
        provider(),
    )
    .unwrap()
}

fn document(tokens: Vec<LayoutToken>) -> LayoutDocument {
    LayoutDocument::new(
        "paper-1",
        "b".repeat(64),
        vec![LayoutPage::new(0, 612.0, 792.0, 0, None, tokens).unwrap()],
    )
    .unwrap()
}

#[test]
fn document_is_versioned_and_has_deterministic_digest() {
    let doc = document(vec![token(0, 0, 10.0), token(1, 1, 40.0)]);
    assert_eq!(doc.schema_version, LAYOUT_SCHEMA_VERSION);
    assert_eq!(doc.digest().unwrap(), doc.digest().unwrap());
    assert_eq!(
        serde_json::to_vec(&doc).unwrap(),
        serde_json::to_vec(&doc).unwrap()
    );
    assert!(doc.digest().unwrap().starts_with("sha256:"));
}

#[test]
fn coordinate_round_trip_is_top_left_and_finite() {
    let bbox = LayoutBBox::new(72.0, 100.0, 144.0, 172.0).unwrap();
    let bottom = bbox.to_bottom_left(792.0).unwrap();
    assert_eq!(bottom, (72.0, 620.0, 144.0, 692.0));
    assert_eq!(
        LayoutBBox::from_bottom_left(bottom.0, bottom.1, bottom.2, bottom.3, 792.0).unwrap(),
        bbox
    );
}

#[test]
fn malformed_geometry_order_and_utf8_fail_closed() {
    assert!(LayoutBBox::new(1.0, 2.0, 0.0, 3.0).is_err());
    assert!(LayoutBBox::new(f64::NAN, 0.0, 1.0, 1.0).is_err());
    assert!(LayoutSpan::present(5, 4).is_err());
    let span = LayoutSpan::present(1, 2).unwrap();
    assert!(span.validate_against("é").is_err());

    let mut adjacent = document(vec![token(0, 0, 10.0), token(1, 1, 40.0)]);
    adjacent.pages[0].tokens[1].source_ordinal = 0;
    assert!(adjacent.validate().is_err());

    let non_adjacent = LayoutPage::new(
        0,
        612.0,
        792.0,
        0,
        None,
        vec![token(0, 0, 10.0), token(2, 1, 40.0), token(0, 2, 70.0)],
    );
    assert!(matches!(
        non_adjacent,
        Err(da_parse::LayoutError::DuplicateOrdinal {
            page: 0,
            ordinal: 0
        })
    ));
}

#[test]
fn absent_span_is_typed_not_an_empty_present_span() {
    let absent = LayoutSpan::absent(da_parse::LayoutSpanAbsence::ProviderUnavailable);
    assert!(absent.is_absent());
    let json = serde_json::to_string(&absent).unwrap();
    assert!(json.contains("provider_unavailable"));
}

#[test]
fn unknown_schema_does_not_deserialize() {
    let json = r#"{"schema_version":999,"document_id":"x","source_pdf_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","pages":[]}"#;
    assert!(serde_json::from_str::<LayoutDocument>(json).is_err());
}

#[test]
fn limits_reject_oversize_documents() {
    let mut doc = document(vec![token(0, 0, 10.0)]);
    doc.pages[0].tokens = (0..100_001).map(|i| token(i, i, 10.0)).collect();
    for item in &mut doc.pages[0].tokens {
        item.page_ordinal = 0;
    }
    doc.pages[0].tokens.sort_by_key(|item| item.token_ordinal);
    assert!(matches!(
        doc.validate(),
        Err(da_parse::LayoutError::TooManyTokens { .. })
    ));
}
