//! Bounded Unicode and no-invention properties for the native personal-name parser.
//!
//! The generators deliberately include hostile Unicode, controls, bidi marks,
//! punctuation, and structured names.  This suite is offline and checks only
//! the public parser contract: totality, deterministic redacted replay,
//! explicit context handling, and provenance of every recognized component.

use da_parse::{
    NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP, NativePersonalNameMalformed, NativePersonalNameOutcome,
    NativePersonalNameParser, PersonalNameContext,
};
use proptest::prelude::*;

const CASES: u32 = 512;

fn parse(context: PersonalNameContext, input: &str) -> NativePersonalNameOutcome {
    NativePersonalNameParser::new().parse(context, input)
}

fn arb_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..=300).prop_map(|chars| chars.into_iter().collect())
}

fn arb_oversize() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP + 1..=420)
        .prop_map(|chars| chars.into_iter().collect())
}

fn arb_word() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            'A', 'B', 'C', 'D', 'E', 'J', 'M', 'Q', 'S', 'a', 'b', 'c', 'd', 'e', 'j', 'm', 'q',
            's', 'é', 'ñ', 'ø', '李', '小', '龙', '\u{0301}', '’', '-', '.',
        ]),
        1..=12,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .prop_filter("word contains a letter", |word: &String| {
        word.chars().any(char::is_alphabetic)
    })
}

fn arb_structured_name() -> impl Strategy<Value = String> {
    (arb_word(), arb_word(), arb_word(), prop::bool::ANY).prop_map(
        |(given, middle, surname, citation)| {
            if citation {
                format!("{surname}, {given} {middle}")
            } else {
                format!("{given} {middle} {surname}")
            }
        },
    )
}

fn contains_surface(input: &str, value: &str) -> bool {
    value.is_empty() || input.contains(value)
}

fn assert_recognized_provenance(input: &str, outcome: &NativePersonalNameOutcome) {
    let NativePersonalNameOutcome::Recognized { name } = outcome else {
        return;
    };
    assert!(contains_surface(
        input,
        name.given_name.as_deref().unwrap_or("")
    ));
    assert!(
        name.middle_names
            .iter()
            .all(|part| contains_surface(input, part))
    );
    assert!(contains_surface(input, &name.surname));
    assert!(contains_surface(input, &name.recomposed_surname));
    assert!(
        name.surname_particles
            .iter()
            .all(|part| contains_surface(input, part))
    );
    assert!(contains_surface(
        input,
        name.suffix.as_deref().unwrap_or("")
    ));
    assert!(name.initials.iter().all(|initial| {
        let mut chars = initial.chars();
        chars.next().is_some_and(|first| first.is_alphabetic())
            && chars.next().is_none()
            && input.contains(initial)
    }));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// Arbitrary valid UTF-8 must never panic or depend on hidden mutable state.
    /// The serialized outcome is a stable, redacted replay of input+context.
    #[test]
    fn arbitrary_utf8_is_total_deterministic_and_byte_stable(input in arb_text()) {
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            let first = parse(context, &input);
            let second = parse(context, &input);
            prop_assert_eq!(&first, &second);

            let bytes = serde_json::to_vec(&first).expect("outcome must serialize");
            let replay: NativePersonalNameOutcome =
                serde_json::from_slice(&bytes).expect("outcome must deserialize");
            prop_assert_eq!(&first, &replay);
            prop_assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes.clone());
            if !input.is_empty() {
                prop_assert!(!String::from_utf8_lossy(&bytes).contains(&input));
            }
            assert_recognized_provenance(&input, &first);
        }
    }

    /// Oversized data is rejected before tokenization, in both dialects.
    #[test]
    fn byte_cap_is_hard_and_content_independent(input in arb_oversize()) {
        prop_assert!(input.len() > NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP);
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            prop_assert_eq!(parse(context, &input), NativePersonalNameOutcome::Oversized);
        }
    }

    /// Structured names may be recognized, but all emitted pieces must be
    /// literal surfaces from the input: no transliteration or invented suffix.
    #[test]
    fn structured_names_never_invent_components(input in arb_structured_name()) {
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            let outcome = parse(context, &input);
            assert_recognized_provenance(&input, &outcome);
            if let NativePersonalNameOutcome::Recognized { name } = &outcome {
                prop_assert!(name.suffix.is_none() || input.contains(name.suffix.as_deref().unwrap()));
                for particle in &name.surname_particles {
                    prop_assert!(input.contains(particle));
                }
            }
        }
    }

    /// Header and citation are not interchangeable: comma syntax is accepted
    /// only by Citation and is rejected as an unexpected character in Header.
    #[test]
    fn context_selection_is_explicit(given in arb_word(), surname in arb_word()) {
        let input = format!("{surname}, {given}");
        let header = parse(PersonalNameContext::Header, &input);
        let citation = parse(PersonalNameContext::Citation, &input);
        let header_rejects_comma = matches!(
            header,
            NativePersonalNameOutcome::Malformed {
                reason: NativePersonalNameMalformed::UnexpectedCharacter
            }
        );
        prop_assert!(header_rejects_comma);
        assert_recognized_provenance(&input, &citation);
    }

    /// Hostile controls, bidi formatting marks, NUL, and non-ASCII digits stay
    /// typed failures and never become recognized name components.
    #[test]
    fn hostile_scalars_remain_closed(
        base in prop::collection::vec(0x20u8..=0x7eu8, 0..=80),
        kind in 0usize..=3,
    ) {
        let base = String::from_utf8(base).unwrap();
        let injected = match kind {
            0 => '\0',
            1 => '\u{202e}',
            2 => '\u{0661}',
            _ => '\u{2066}',
        };
        let input = format!("{base}Smith {injected}Jones");
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            let outcome = parse(context, &input);
            let recognized = matches!(outcome, NativePersonalNameOutcome::Recognized { .. });
            prop_assert!(!recognized);
            prop_assert_eq!(outcome, parse(context, &input));
        }
    }

    /// A token made only of combining marks or punctuation has no letter from
    /// which a person-name component could be derived.
    #[test]
    fn non_letter_tokens_are_not_recognized(mark in prop::sample::select(vec!["\u{0301}", "-", "’"])) {
        let input = format!("Ada {mark} Lovelace");
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            let recognized = matches!(
                parse(context, &input),
                NativePersonalNameOutcome::Recognized { .. }
            );
            prop_assert!(!recognized);
        }
    }
}
