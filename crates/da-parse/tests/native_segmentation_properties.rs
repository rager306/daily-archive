use da_parse::{
    LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan, LayoutStyleHints,
    LayoutToken, LayoutTokenKind, LexemeRow, SegmentationConfig, segment_document,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn doc(texts: &[String]) -> (LayoutDocument, Vec<LexemeRow>) {
    let provider = LayoutProvider::new("property-donor", "1", "a".repeat(64)).unwrap();
    let tokens = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0, 30.0 + i as f64 * 10.0, 90.0, 38.0 + i as f64 * 10.0)
                    .unwrap(),
                digest(text),
                LayoutSpan::absent(da_parse::LayoutSpanAbsence::NotAvailable),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "property-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 600.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let rows = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                "property-doc",
                "property-donor",
                0,
                i as u32,
                digest(text),
                text.clone(),
            )
        })
        .collect();
    (document, rows)
}

proptest! {
    #[test]
    fn admitted_tokens_are_assigned_once_and_in_order(texts in prop::collection::vec("[A-Za-z]{1,12}", 1..30)) {
        let (document, rows) = doc(&texts);
        let outcome = segment_document(&document, &rows, SegmentationConfig::default()).unwrap();
        prop_assert_eq!(outcome.assignments.len(), texts.len());
        for (assignment, token) in outcome.assignments.iter().zip(document.pages[0].tokens.iter()) {
            prop_assert_eq!(assignment.source_ordinal, token.source_ordinal);
            prop_assert_eq!(assignment.token_ordinal, token.token_ordinal);
            prop_assert_eq!(&assignment.text_digest, &token.text_digest);
        }
        prop_assert!(outcome.assignments.windows(2).all(|w| w[0].token_ordinal < w[1].token_ordinal));
        prop_assert_eq!(outcome, segment_document(&document, &rows, SegmentationConfig::default()).unwrap());
    }
}

#[test]
fn malformed_sidecar_order_is_rejected_before_classification() {
    let (document, mut rows) = doc(&["one".into(), "two".into()]);
    rows.swap(0, 1);
    assert!(matches!(
        segment_document(&document, &rows, SegmentationConfig::default()),
        Err(da_parse::SegmentationError::MalformedOrder { page: 0, source: 0 })
    ));
}

#[test]
fn repeated_admitted_token_ordinal_is_rejected_fail_closed() {
    let (mut document, rows) = doc(&["one".into(), "two".into()]);
    document.pages[0].tokens[1].token_ordinal = document.pages[0].tokens[0].token_ordinal;
    assert!(matches!(
        segment_document(&document, &rows, SegmentationConfig::default()),
        Err(da_parse::SegmentationError::LayoutInvalid)
    ));
}
