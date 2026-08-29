//! Bounded, candidate-independent properties for the citation leaf.
use da_parse::{
    CitationField, NATIVE_CITATION_INPUT_BYTE_CAP, NativeCitationOutcome, NativeCitationParser,
};
use proptest::prelude::*;

fn assert_spans(input: &str, fields: &[CitationField]) {
    for field in fields {
        assert!(field.span.start < field.span.end);
        assert!(field.span.end <= input.len());
        assert!(input.is_char_boundary(field.span.start));
        assert!(input.is_char_boundary(field.span.end));
        assert_eq!(&input[field.span.start..field.span.end], field.value);
    }
}

proptest! {
    #[test]
    fn parser_is_total_deterministic_and_never_invents(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let input = String::from_utf8_lossy(&bytes).into_owned();
        let parser = NativeCitationParser::new();
        let first = parser.parse(&input);
        let second = parser.parse(&input);
        prop_assert_eq!(&first, &second);
        if let NativeCitationOutcome::Recognized { fields } = &first {
            assert_spans(&input, fields);
            for field in fields {
                prop_assert!(!field.value.is_empty());
                prop_assert!(input.contains(&field.value));
            }
        }
        if input.len() > NATIVE_CITATION_INPUT_BYTE_CAP {
            prop_assert_eq!(first, NativeCitationOutcome::Oversized);
        }
    }

    #[test]
    fn unicode_scalar_boundaries_are_safe(chars in prop::collection::vec(any::<char>(), 0..128)) {
        let input: String = chars.into_iter().collect();
        let outcome = NativeCitationParser::new().parse(&input);
        if let NativeCitationOutcome::Recognized { fields } = outcome {
            assert_spans(&input, &fields);
        }
    }
}

#[test]
fn regression_seeds_cover_closed_negative_surface() {
    let parser = NativeCitationParser::new();
    for input in [
        "",
        "   ",
        "title: one; title: two",
        "Smith, et al.",
        "x\0doi:10.1/x",
        "doi:10.1/x\u{202e}",
    ] {
        let first = parser.parse(input);
        assert_eq!(first, parser.parse(input));
        let encoded = serde_json::to_vec(&first).unwrap();
        let replay: NativeCitationOutcome = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(encoded, serde_json::to_vec(&replay).unwrap());
        if !input.is_empty() && !matches!(first, NativeCitationOutcome::Recognized { .. }) {
            assert!(!String::from_utf8_lossy(&encoded).contains(input));
        }
    }
}
