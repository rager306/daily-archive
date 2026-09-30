//! Property tests for the additive native word-level projection (S04/T04 repair).
//!
//! The contract the repair rests on is a *partition*: the split of a
//! show-operator span into words must conserve every character, invent none,
//! produce no word with internal whitespace, and derive geometry honestly. These
//! tests assert that over generated text rather than over a hand-picked fixture.

use da_parse::*;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn project_one(text: &str) -> NativeWordProjection {
    let token = LayoutToken::new(
        0,
        0,
        0,
        LayoutBBox::new(20.0, 10.0, 280.0, 20.0).unwrap(),
        digest(text),
        LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
        LayoutTokenKind::from_lexical_text(text),
        LayoutStyleHints {
            bold: false,
            italic: false,
            monospace: false,
            font_size_milli_pt: Some(10_500),
        },
        LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap(),
    )
    .unwrap();
    let document = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 800.0, 0, None, vec![token]).unwrap()],
    )
    .unwrap();
    let lexeme = LexemeRow::new("fixture-doc", "fixture-donor", 0, 0, digest(text), text);
    project_native_words(&document, &[lexeme]).unwrap()
}

/// Characters that matter for a whitespace partition: letters, digits, ASCII
/// punctuation and four spellings of whitespace (space, tab, newline, NBSP).
fn text_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            proptest::char::range('a', 'z'),
            proptest::char::range('A', 'Z'),
            proptest::char::range('0', '9'),
            Just(' '),
            Just('\t'),
            Just('\n'),
            Just('\u{a0}'),
            Just('.'),
            Just('['),
            Just(']'),
        ],
        0..30,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

proptest! {
    #[test]
    fn partition_conserves_every_character(text in text_strategy()) {
        let projection = project_one(&text);
        let stripped: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
        let Some(parent) = projection.parents.first() else {
            // Whitespace-only and empty text project to a typed absence.
            prop_assert!(stripped.is_empty());
            prop_assert_eq!(projection.identity.word_count, 0);
            return Ok(());
        };
        let word_chars: u32 = parent
            .words
            .iter()
            .map(|word| word.geometry.char_end - word.geometry.char_start)
            .sum();
        prop_assert_eq!(word_chars + parent.separator_chars, parent.parent_char_len);
        prop_assert_eq!(parent.parent_char_len as usize, text.chars().count());

        let rebuilt: String = parent.words.iter().map(|word| word.text()).collect();
        prop_assert_eq!(rebuilt, stripped, "no character is invented or dropped");
    }

    #[test]
    fn no_projected_word_carries_whitespace(text in text_strategy()) {
        let projection = project_one(&text);
        for parent in &projection.parents {
            for word in &parent.words {
                prop_assert!(!word.text().is_empty());
                prop_assert!(
                    !word.text().chars().any(char::is_whitespace),
                    "projected word carried whitespace: {:?}",
                    word.text()
                );
            }
        }
    }

    #[test]
    fn projection_is_deterministic(text in text_strategy()) {
        let first = project_one(&text);
        let second = project_one(&text);
        prop_assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        prop_assert_eq!(first, second);
    }

    #[test]
    fn empty_and_whitespace_only_text_is_a_typed_absence(text in text_strategy()) {
        let projection = project_one(&text);
        let stripped: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
        if stripped.is_empty() {
            prop_assert!(projection.parents.is_empty());
            prop_assert_eq!(projection.identity.word_count, 0);
            prop_assert_eq!(projection.absences.len(), 1);
            let expected = if text.is_empty() {
                NativeWordAbsence::EmptyText {
                    page_ordinal: 0,
                    source_ordinal: 0,
                }
            } else {
                NativeWordAbsence::WhitespaceOnly {
                    page_ordinal: 0,
                    source_ordinal: 0,
                }
            };
            prop_assert_eq!(projection.absences[0], expected);
        } else {
            prop_assert!(projection.absences.is_empty());
            prop_assert!(!projection.parents.is_empty());
        }
    }

    #[test]
    fn geometry_origin_matches_the_word_count(text in text_strategy()) {
        let projection = project_one(&text);
        let Some(parent) = projection.parents.first() else {
            return Ok(());
        };
        let parent_bbox = parent.words[0].geometry.parent_bbox;
        let mut previous_x: Option<f64> = None;
        for word in &parent.words {
            prop_assert!(word.geometry.char_start < word.geometry.char_end);
            prop_assert_eq!(word.geometry.parent_bbox, parent_bbox);
            if parent.words.len() == 1 {
                prop_assert_eq!(
                    word.geometry.origin,
                    NativeWordGeometryOrigin::MeasuredParentSpan
                );
                prop_assert!(word.geometry.estimated_bbox.is_none());
            } else {
                prop_assert_eq!(
                    word.geometry.origin,
                    NativeWordGeometryOrigin::ProportionalEstimateFromParentSpan
                );
                let estimate = word.geometry.estimated_bbox.expect("an estimate is reported");
                prop_assert!(estimate.x_min >= parent_bbox.x_min - 1e-9);
                prop_assert!(estimate.x_max <= parent_bbox.x_max + 1e-9);
                prop_assert!(estimate.x_max > estimate.x_min);
                prop_assert_eq!(estimate.y_min, parent_bbox.y_min);
                prop_assert_eq!(estimate.y_max, parent_bbox.y_max);
                if let Some(previous) = previous_x {
                    prop_assert!(estimate.x_min >= previous - 1e-9);
                }
                previous_x = Some(estimate.x_max);
            }
        }
        if parent.words.len() > 1 {
            let last = parent.words.last().unwrap();
            prop_assert_eq!(
                last.geometry.estimated_bbox.unwrap().x_max,
                parent_bbox.x_max
            );
        }
    }

    #[test]
    fn word_digests_recompute_and_reconstruction_is_stable(
        left in text_strategy(),
        right in text_strategy(),
    ) {
        let projection = project_one(&left);
        for parent in &projection.parents {
            for word in &parent.words {
                prop_assert_eq!(word.text_digest.clone(), digest(word.text()));
            }
            prop_assert_eq!(
                parent.reconstruction_digest.clone(),
                native_word_reconstruction_digest(&parent.words)
            );
        }
        // Separator width is a property of the span bytes, not of the word
        // line: the reconstruction digest identifies the word sequence, the
        // parent digest identifies the span.
        let other = project_one(&right);
        if left.split_whitespace().collect::<Vec<_>>()
            == right.split_whitespace().collect::<Vec<_>>()
        {
            prop_assert_eq!(
                projection
                    .parents
                    .first()
                    .map(|parent| parent.reconstruction_digest.clone()),
                other
                    .parents
                    .first()
                    .map(|parent| parent.reconstruction_digest.clone())
            );
        }
    }
}
