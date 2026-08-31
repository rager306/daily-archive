use da_parse::{
    DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind, LexemeRow, SegmentationConfig,
    SegmentationError, segment_document,
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
        if text == "[formula]" {
            LayoutTokenKind::Formula
        } else {
            LayoutTokenKind::Word
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
