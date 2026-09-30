//! Contract tests for the additive native word-level projection (S04/T04 repair).
//!
//! The projection exists because the positioned donor's lexical unit is the PDF
//! show-operator span, not a word, so the T02/T03 native bands — whose contract
//! is word granular — refused every real document with
//! `TokenContainsWhitespace`. These tests pin the partition, the honest
//! geometry origin, the digest lineage, the typed refusals and the redaction
//! discipline.

use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

fn style(bold: bool, font_size_milli_pt: u32) -> LayoutStyleHints {
    LayoutStyleHints {
        bold,
        italic: false,
        monospace: false,
        font_size_milli_pt: Some(font_size_milli_pt),
    }
}

/// One span of the fixture: text, horizontal extent, vertical offset, style.
struct Span {
    text: &'static str,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    style: LayoutStyleHints,
}

fn span(text: &'static str, y_min: f64, style: LayoutStyleHints) -> Span {
    Span {
        text,
        x_min: 20.0,
        x_max: 280.0,
        y_min,
        style,
    }
}

/// Build a one-page document plus its lexical sidecar from positioned spans.
/// `y_min` values are explicit so a caller can place spans on distinct lines.
fn fixture(spans: &[Span]) -> (LayoutDocument, Vec<LexemeRow>) {
    let tokens: Vec<LayoutToken> = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let source = index as u32;
            LayoutToken::new(
                0,
                source,
                source,
                LayoutBBox::new(span.x_min, span.y_min, span.x_max, span.y_min + 10.0).unwrap(),
                digest(span.text),
                LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                LayoutTokenKind::from_lexical_text(span.text),
                span.style,
                provider(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 800.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let lexemes = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            LexemeRow::new(
                "fixture-doc",
                "fixture-donor",
                0,
                index as u32,
                digest(span.text),
                span.text,
            )
        })
        .collect();
    (document, lexemes)
}

/// The blocker fixture: multi-word show-operator spans, exactly the shape the
/// positioned donor produces and the bands used to refuse.
fn blocker_spans() -> Vec<Span> {
    vec![
        span(
            "GEPA: Reflective Prompt Evolution",
            10.0,
            style(true, 14_000),
        ),
        span("Jane Doe, John Smith", 30.0, style(false, 10_500)),
        span("1 Introduction", 60.0, style(true, 12_000)),
        span("[1] Smith 2020", 90.0, style(false, 10_500)),
        span("*** ***", 120.0, style(false, 10_500)),
    ]
}

/// The words of the blocker fixture: 4 + 4 + 2 + 3 + 2.
const BLOCKER_WORDS: usize = 15;

#[test]
fn multi_word_spans_project_into_word_tokens_without_whitespace() {
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).expect("projection must succeed");

    let mut total = 0usize;
    for parent in &projection.parents {
        assert!(
            parent.word_count() >= 2,
            "the blocker fixture is made of multi-word spans"
        );
        for word in &parent.words {
            assert!(
                !word.text().chars().any(char::is_whitespace),
                "a projected word must never carry internal whitespace: {:?}",
                word.text()
            );
            assert!(!word.text().is_empty(), "a projected word is never empty");
        }
        total += parent.words.len();
    }
    assert_eq!(projection.identity.word_count as usize, total);
    assert_eq!(projection.identity.parent_count as usize, 5);
    assert_eq!(total, BLOCKER_WORDS);
    assert!(projection.absences.is_empty());
}

#[test]
fn geometry_states_its_origin_and_multi_word_boxes_stay_inside_the_parent() {
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).expect("projection must succeed");

    for parent in &projection.parents {
        let parent_bbox = parent.words[0].geometry.parent_bbox;
        let mut previous_x: Option<f64> = None;
        for word in &parent.words {
            let geometry = word.geometry;
            assert_eq!(geometry.parent_bbox, parent_bbox);
            assert!(
                geometry.char_start < geometry.char_end,
                "character spans are strictly increasing"
            );
            match geometry.origin {
                NativeWordGeometryOrigin::MeasuredParentSpan => {
                    assert_eq!(parent.words.len(), 1);
                    assert!(geometry.estimated_bbox.is_none());
                }
                NativeWordGeometryOrigin::ProportionalEstimateFromParentSpan => {
                    let estimate = geometry.estimated_bbox.expect("an estimate is reported");
                    assert!(
                        estimate.x_min >= parent_bbox.x_min - 1e-9
                            && estimate.x_max <= parent_bbox.x_max + 1e-9,
                        "an estimated word box stays inside its parent span"
                    );
                    assert_eq!(estimate.y_min, parent_bbox.y_min);
                    assert_eq!(estimate.y_max, parent_bbox.y_max);
                    if let Some(previous) = previous_x {
                        assert!(
                            estimate.x_min >= previous - 1e-9,
                            "estimated word boxes keep reading order"
                        );
                    }
                    previous_x = Some(estimate.x_max);
                }
            }
        }
        if parent.words.len() > 1 {
            let last = parent.words.last().unwrap();
            assert_eq!(
                last.geometry.estimated_bbox.unwrap().x_max,
                parent_bbox.x_max,
                "the last estimated word closes the parent span"
            );
        }
    }
}

#[test]
fn projection_is_deterministic_and_independent_of_sidecar_row_order() {
    let (document, lexemes) = fixture(&blocker_spans());
    let first = project_native_words(&document, &lexemes).unwrap();
    let second = project_native_words(&document, &lexemes).unwrap();
    assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    assert_eq!(first, second);

    let mut reversed = lexemes.clone();
    reversed.reverse();
    let reordered = project_native_words(&document, &reversed).unwrap();
    assert_eq!(
        first.digest().unwrap(),
        reordered.digest().unwrap(),
        "the projection follows document order, not sidecar row order"
    );
}

#[test]
fn style_is_inherited_verbatim_and_kind_is_derived_from_the_word() {
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).unwrap();

    let title = projection.parent(0, 0).expect("title span");
    let title_token = &title.words[0];
    assert_eq!(title_token.style, style(true, 14_000));
    assert_eq!(title_token.kind, LayoutTokenKind::Word);

    // No invented font data: every word inherits its parent's hints verbatim.
    let references = projection.parent(0, 3).expect("reference span");
    for word in &references.words {
        assert_eq!(word.style, style(false, 10_500));
        assert_eq!(word.kind, LayoutTokenKind::Word);
    }

    // The kind is derived from each word, not inherited from the parent span:
    // the parent `*** ***` is one symbol run, while each projected word is
    // punctuation.
    let parent_token = document.pages[0]
        .tokens
        .iter()
        .find(|token| token.source_ordinal == 4)
        .expect("the symbol span");
    assert_eq!(parent_token.kind, LayoutTokenKind::Symbol);
    let symbols = projection.parent(0, 4).expect("symbol span");
    assert_eq!(symbols.word_count(), 2);
    for word in &symbols.words {
        assert_eq!(word.kind, LayoutTokenKind::Punctuation);
    }
}

#[test]
fn conservation_accounting_closes_for_every_parent() {
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).unwrap();

    for parent in &projection.parents {
        let word_chars: u32 = parent
            .words
            .iter()
            .map(|word| word.geometry.char_end - word.geometry.char_start)
            .sum();
        assert_eq!(
            word_chars + parent.separator_chars,
            parent.parent_char_len,
            "the partition closes over the parent text"
        );
        assert_eq!(
            parent.reconstruction_digest,
            native_word_reconstruction_digest(&parent.words),
            "the reconstruction digest recomputes from the word line"
        );
        let mut expected_start = 0u32;
        for word in &parent.words {
            assert!(
                word.geometry.char_start >= expected_start,
                "word character spans do not overlap"
            );
            expected_start = word.geometry.char_end;
        }
        assert!(expected_start <= parent.parent_char_len);
    }
}

#[test]
fn redaction_keeps_word_text_out_of_debug_and_serde() {
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).unwrap();
    let word = &projection.parents[0].words[0];

    let debug = format!("{word:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("GEPA"));

    let serialized = serde_json::to_string(&projection).unwrap();
    for secret in ["GEPA", "Reflective", "Jane", "Introduction", "Smith"] {
        assert!(
            !serialized.contains(secret),
            "the serialized projection must not carry text: {secret}"
        );
    }
    assert!(serialized.contains(&word.text_digest));
    assert!(!serialized.contains("pub text"));
}

#[test]
fn empty_and_whitespace_only_spans_are_typed_absences() {
    let spans = vec![
        span("", 10.0, style(false, 10_500)),
        span("   ", 30.0, style(false, 10_500)),
        span("word", 50.0, style(false, 10_500)),
    ];
    let (document, lexemes) = fixture(&spans);
    let projection = project_native_words(&document, &lexemes).unwrap();

    assert_eq!(projection.parents.len(), 1);
    assert_eq!(projection.identity.word_count, 1);
    assert!(projection.absences.contains(&NativeWordAbsence::EmptyText {
        page_ordinal: 0,
        source_ordinal: 0
    }));
    assert!(
        projection
            .absences
            .contains(&NativeWordAbsence::WhitespaceOnly {
                page_ordinal: 0,
                source_ordinal: 1
            })
    );
}

#[test]
fn typed_refusals_cover_the_join_vocabulary() {
    let (document, lexemes) = fixture(&blocker_spans());

    let mut duplicate = lexemes.clone();
    duplicate.push(lexemes[0].clone());
    assert_eq!(
        project_native_words(&document, &duplicate).unwrap_err(),
        NativeWordProjectionError::DuplicateLexeme { page: 0, source: 0 }
    );

    let mut missing = lexemes.clone();
    missing.remove(2);
    assert_eq!(
        project_native_words(&document, &missing).unwrap_err(),
        NativeWordProjectionError::MissingLexeme { page: 0, source: 2 }
    );

    let mut extra = lexemes.clone();
    extra.push(LexemeRow::new(
        "fixture-doc",
        "fixture-donor",
        0,
        99,
        digest("ghost"),
        "ghost",
    ));
    assert_eq!(
        project_native_words(&document, &extra).unwrap_err(),
        NativeWordProjectionError::MissingLayoutToken {
            page: 0,
            source: 99
        }
    );

    let mut drifted = lexemes.clone();
    drifted[0].text_digest = digest("something else");
    assert_eq!(
        project_native_words(&document, &drifted).unwrap_err(),
        NativeWordProjectionError::DigestMismatch { page: 0, source: 0 }
    );

    let mut foreign_document = lexemes.clone();
    foreign_document[0].document_id = "other-doc".into();
    assert_eq!(
        project_native_words(&document, &foreign_document).unwrap_err(),
        NativeWordProjectionError::DocumentMismatch
    );

    let mut foreign_provider = lexemes.clone();
    foreign_provider[0].provider_id = "other-provider".into();
    assert_eq!(
        project_native_words(&document, &foreign_provider).unwrap_err(),
        NativeWordProjectionError::ProviderMismatch
    );

    let empty_document = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 800.0, 0, None, Vec::new()).unwrap()],
    )
    .unwrap();
    assert_eq!(
        project_native_words(&empty_document, &[]).unwrap_err(),
        NativeWordProjectionError::EmptyDocument
    );
}

#[test]
fn the_surface_bounds_are_sane() {
    assert_eq!(
        NATIVE_WORD_PROJECTION_SCHEMA_VERSION,
        "da-parse.native-word-projection/v1"
    );
    let (document, lexemes) = fixture(&blocker_spans());
    let projection = project_native_words(&document, &lexemes).unwrap();
    assert_eq!(
        projection.schema_version,
        NATIVE_WORD_PROJECTION_SCHEMA_VERSION
    );
    assert_eq!(native_word_keys(&projection).len(), BLOCKER_WORDS);
    assert!(native_word_keys(&projection).contains(&(0, 0, 0)));
    assert_eq!(
        native_word_order_key(&projection.parents[0].words[1]),
        (0, 0, 1)
    );
    assert!(projection.digest().unwrap().starts_with("sha256:"));
}
