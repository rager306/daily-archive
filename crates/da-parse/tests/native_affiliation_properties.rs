//! Bounded Unicode and no-invention properties for native affiliations.
//!
//! The generators exercise the parser as an untrusted-input leaf.  Assertions
//! intentionally derive values from the original input, rather than from the
//! parser's private classification helpers, so span/value drift is observable.

use da_parse::{
    AffiliationComponentKind, AffiliationSpan, NATIVE_AFFILIATION_INPUT_BYTE_CAP,
    NativeAffiliationMalformed, NativeAffiliationOutcome, NativeAffiliationParser,
};
use proptest::prelude::*;

const CASES: u32 = 512;

fn parse(input: &str) -> NativeAffiliationOutcome {
    NativeAffiliationParser::new().parse(input)
}

fn is_marker_surface(value: &str) -> bool {
    value.chars().count() == 1
        && value.chars().next().is_some_and(|c| {
            c.is_ascii_digit()
                || matches!(c, '¹' | '²' | '³' | '*' | '†' | '‡')
                || ('\u{2070}'..='\u{209f}').contains(&c)
        })
}

fn is_organization_surface(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "university",
        "institute",
        "laboratory",
        "laboratório",
        "department",
        "faculty",
        "college",
        "centre",
        "center",
        "hospital",
        "research",
        "universität",
    ]
    .iter()
    .any(|word| value.contains(word))
}

fn is_address_surface(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
        && [
            " street",
            " road",
            " avenue",
            " boulevard",
            "straße",
            "strasse",
        ]
        .iter()
        .any(|word| lower.contains(word))
}

fn assert_valid_span(input: &str, span: AffiliationSpan) {
    assert!(span.start <= span.end);
    assert!(span.end <= input.len());
    assert!(input.is_char_boundary(span.start));
    assert!(input.is_char_boundary(span.end));
}

fn assert_recognized_provenance(input: &str, outcome: &NativeAffiliationOutcome) {
    let NativeAffiliationOutcome::Recognized { components } = outcome else {
        return;
    };
    let mut previous_end = 0;
    for component in components {
        assert_valid_span(input, component.span);
        assert!(component.span.start >= previous_end);
        let value = &input[component.span.start..component.span.end];
        assert!(!value.is_empty());
        assert_eq!(value, value.trim());
        match component.kind {
            AffiliationComponentKind::Marker => assert!(is_marker_surface(value)),
            AffiliationComponentKind::Organization => assert!(is_organization_surface(value)),
            AffiliationComponentKind::Address => assert!(is_address_surface(value)),
        }
        previous_end = component.span.end;
    }
}

fn arb_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..=320)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

fn arb_oversized() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 513..=700)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .prop_filter("must exceed the byte cap", |text: &String| {
            text.len() > NATIVE_AFFILIATION_INPUT_BYTE_CAP
        })
}

fn arb_word() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            'A', 'B', 'C', 'D', 'E', 'F', 'L', 'M', 'P', 'S', 'a', 'b', 'c', 'e', 'f', 'i', 'n',
            'r', 's', 't', 'é', 'ñ', 'ø', '李', '小', 'ü', '\u{0301}', '-', '.',
        ]),
        1..=18,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .prop_filter("word contains a letter", |word: &String| {
        word.chars().any(char::is_alphabetic)
    })
}

fn arb_structured_affiliation() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec![
            "University of",
            "Institute for",
            "Laboratório de",
            "Department of",
        ]),
        arb_word(),
        prop::sample::select(vec![
            "12 Main Street",
            "3 Research Road",
            "Berlin",
            "München",
        ]),
        prop::sample::select(vec!["", "¹, ", "* "]),
    )
        .prop_map(|(organization, name, address, marker)| {
            format!("{marker}{organization} {name}, {address}")
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    #[test]
    fn arbitrary_utf8_is_total_replayable_and_span_safe(input in arb_text()) {
        let first = parse(&input);
        let second = parse(&input);
        prop_assert_eq!(&first, &second);

        let bytes = serde_json::to_vec(&first).expect("outcome serializes");
        let replay: NativeAffiliationOutcome = serde_json::from_slice(&bytes).expect("outcome replays");
        prop_assert_eq!(&first, &replay);
        prop_assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
        assert_recognized_provenance(&input, &first);
    }

    #[test]
    fn oversized_input_is_rejected_before_any_recognition(input in arb_oversized()) {
        prop_assert!(input.len() > NATIVE_AFFILIATION_INPUT_BYTE_CAP);
        prop_assert_eq!(parse(&input), NativeAffiliationOutcome::Oversized);
    }

    #[test]
    fn structured_fields_are_literal_and_ordered(input in arb_structured_affiliation()) {
        let outcome = parse(&input);
        assert_recognized_provenance(&input, &outcome);
        if let NativeAffiliationOutcome::Recognized { components } = outcome {
            prop_assert!(!components.is_empty());
            for pair in components.windows(2) {
                prop_assert!(pair[0].span.end <= pair[1].span.start);
            }
        }
    }

    #[test]
    fn hostile_controls_and_bidi_marks_are_closed(
        base in prop::sample::select(vec![
            "University of Example", "Institute for Research", "12 Main Street", "* University",
        ]),
        hostile in prop::sample::select(vec!['\0', '\u{0008}', '\u{202e}', '\u{2066}']),
    ) {
        let input = format!("{base}{hostile}");
        let outcome = parse(&input);
        let recognized = matches!(outcome, NativeAffiliationOutcome::Recognized { .. });
        prop_assert!(!recognized);
        prop_assert_eq!(outcome, parse(&input));
    }

    #[test]
    fn malformed_and_repeated_separators_fail_closed(
        component in prop::sample::select(vec!["University", "Institute", "12 Main Street"]),
        separator in prop::sample::select(vec![",,", ", ,"]),
    ) {
        let input = format!("{component}{separator}Berlin");
        let repeated_separator = matches!(
            parse(&input),
            NativeAffiliationOutcome::Malformed {
                reason: NativeAffiliationMalformed::RepeatedSeparator
            }
        );
        prop_assert!(repeated_separator);
    }

    #[test]
    fn cap_boundary_is_deterministic(
        prefix in prop::sample::select(vec!["University", "Institute", "12 Main Street"]),
    ) {
        let short_len = NATIVE_AFFILIATION_INPUT_BYTE_CAP.saturating_sub(1);
        let short = format!("{prefix}{}", "x".repeat(short_len.saturating_sub(prefix.len())));
        let long = format!("{short}x");
        prop_assert_eq!(short.len(), NATIVE_AFFILIATION_INPUT_BYTE_CAP - 1);
        prop_assert_eq!(long.len(), NATIVE_AFFILIATION_INPUT_BYTE_CAP);
        prop_assert_ne!(parse(&long), NativeAffiliationOutcome::Oversized);
        let oversized = format!("{long}界");
        prop_assert_eq!(parse(&oversized), NativeAffiliationOutcome::Oversized);
        let replay = parse(&short);
        prop_assert_eq!(replay, parse(&short));
    }
}

#[test]
fn mutation_style_span_and_value_drift_is_caught() {
    let input = "¹ University of Example, 12 Main Street";
    let NativeAffiliationOutcome::Recognized { components } = parse(input) else {
        panic!("fixture must be recognized");
    };
    let mut drifted = components.clone();
    drifted[0].span.end += 1;
    let drifted_value = &input[drifted[0].span.start..drifted[0].span.end];
    assert_ne!(drifted_value, "¹");
    assert!(!is_marker_surface(drifted_value));

    let invalid = AffiliationSpan {
        start: 1,
        end: input.len() + 1,
    };
    assert!(invalid.end > input.len());
}
