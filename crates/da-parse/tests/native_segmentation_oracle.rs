use da_parse::{
    DocumentRegion, LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind, LexemeRow, SegmentationConfig,
    segment_document,
};
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn provider() -> LayoutProvider {
    LayoutProvider::new("oracle-donor", "1", "a".repeat(64)).unwrap()
}
fn make_token(page: u32, source: u32, ordinal: u32, text: &str, y: f64) -> LayoutToken {
    LayoutToken::new(
        page,
        source,
        ordinal,
        LayoutBBox::new(20.0, y, 100.0, y + 8.0).unwrap(),
        digest(text),
        LayoutSpan::absent(da_parse::LayoutSpanAbsence::NotAvailable),
        LayoutTokenKind::Word,
        LayoutStyleHints::default(),
        provider(),
    )
    .unwrap()
}

#[test]
fn independent_fixture_oracle_matches_all_document_regions_and_sections() {
    let pages_texts: &[&[&str]] = &[
        &[
            "running header",
            "Title",
            "Abstract",
            "1 Introduction",
            "1.1 Method",
        ],
        &["running header", "References", "[1] Citation"],
        &["running header", "Appendix", "A DETAILS"],
    ];
    let mut pages = Vec::new();
    let mut rows = Vec::new();
    let mut source = 0u32;
    for (page_ordinal, texts) in pages_texts.iter().enumerate() {
        let page = page_ordinal as u32;
        let tokens = texts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let y = if i == 0 { 0.0 } else { 50.0 + i as f64 * 20.0 };
                let token = make_token(page, source, i as u32, text, y);
                rows.push(LexemeRow::new(
                    "oracle-doc",
                    "oracle-donor",
                    page,
                    source,
                    digest(text),
                    *text,
                ));
                source += 1;
                token
            })
            .collect();
        pages.push(LayoutPage::new(page, 300.0, 400.0, 0, None, tokens).unwrap());
    }
    let document = LayoutDocument::new("oracle-doc", "b".repeat(64), pages).unwrap();
    let outcome = segment_document(&document, &rows, SegmentationConfig::default()).unwrap();

    // Expected values are the reviewer's fixture oracle, not a call to the
    // classifier's transition helpers.
    let expected = [
        DocumentRegion::PageFurniture,
        DocumentRegion::Header,
        DocumentRegion::Header,
        DocumentRegion::Body,
        DocumentRegion::Body,
        DocumentRegion::PageFurniture,
        DocumentRegion::References,
        DocumentRegion::References,
        DocumentRegion::PageFurniture,
        DocumentRegion::Annex,
        DocumentRegion::Annex,
    ];
    let actual: Vec<_> = outcome.assignments.iter().map(|a| a.region).collect();
    assert_eq!(actual, expected);
    assert_eq!(
        outcome.assignments.len(),
        document.pages.iter().map(|p| p.tokens.len()).sum::<usize>()
    );
    assert_eq!(outcome.sections.len(), 3);
    assert_eq!(outcome.sections[0].parent_region, DocumentRegion::Body);
    assert_eq!(outcome.sections[0].depth, 1);
    assert_eq!(outcome.sections[1].parent_region, DocumentRegion::Body);
    assert_eq!(outcome.sections[1].depth, 2);
    assert_eq!(
        outcome.sections[1].parent_section_id,
        Some(outcome.sections[0].section_id.clone())
    );
    assert_eq!(outcome.sections[2].parent_region, DocumentRegion::Annex);
    assert_eq!(outcome.sections[2].depth, 1);
    assert_eq!(outcome.sections[2].parent_section_id, None);
    assert!(outcome.assignments.windows(2).all(|w| {
        w[0].page_ordinal < w[1].page_ordinal
            || (w[0].page_ordinal == w[1].page_ordinal && w[0].token_ordinal < w[1].token_ordinal)
    }));
}

#[test]
fn caption_like_text_is_not_a_section_boundary() {
    let text = "Figure 1 overview";
    let token = make_token(0, 0, 0, text, 100.0);
    let document = LayoutDocument::new(
        "caption-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 400.0, 0, None, vec![token]).unwrap()],
    )
    .unwrap();
    let row = LexemeRow::new("caption-doc", "oracle-donor", 0, 0, digest(text), text);
    let outcome = segment_document(&document, &[row], SegmentationConfig::default()).unwrap();
    assert!(outcome.sections.is_empty());
    assert_eq!(outcome.assignments[0].region, DocumentRegion::Header);
}
