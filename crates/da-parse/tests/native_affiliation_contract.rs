use da_parse::{
    AffiliationComponentKind, NATIVE_AFFILIATION_INPUT_BYTE_CAP,
    NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION, NATIVE_AFFILIATION_PARSER_ID,
    NATIVE_AFFILIATION_PARSER_VERSION, NativeAffiliationAmbiguous, NativeAffiliationMalformed,
    NativeAffiliationOutcome, NativeAffiliationParser, NativeAffiliationUnsupported,
};

fn parse(input: &str) -> NativeAffiliationOutcome {
    NativeAffiliationParser::new().parse(input)
}

fn recognized(input: &str) -> Vec<(AffiliationComponentKind, &str)> {
    match parse(input) {
        NativeAffiliationOutcome::Recognized { components } => components
            .iter()
            .map(|component| {
                (
                    component.kind,
                    &input[component.span.start..component.span.end],
                )
            })
            .collect(),
        other => panic!("expected recognized, got {other:?}"),
    }
}

#[test]
fn public_identity_and_schema_are_frozen() {
    assert_eq!(NATIVE_AFFILIATION_PARSER_ID, "da-parse.native_affiliation");
    assert_eq!(NATIVE_AFFILIATION_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-affiliation.outcome/v1"
    );
    assert_eq!(NATIVE_AFFILIATION_INPUT_BYTE_CAP, 512);
}

#[test]
fn recognized_components_are_literal_spans_and_keep_order() {
    let input = "¹ Department of Physics, University of Example, 12 Main Street, Berlin";
    let components = recognized(input);
    assert_eq!(
        components,
        vec![
            (AffiliationComponentKind::Marker, "¹"),
            (
                AffiliationComponentKind::Organization,
                "Department of Physics"
            ),
            (
                AffiliationComponentKind::Organization,
                "University of Example"
            ),
            (AffiliationComponentKind::Address, "12 Main Street"),
        ]
    );
}

#[test]
fn laboratory_and_unicode_text_are_not_normalized_or_invented() {
    let input = "Laboratório de Física, Universität München";
    assert_eq!(
        recognized(input),
        vec![
            (
                AffiliationComponentKind::Organization,
                "Laboratório de Física"
            ),
            (
                AffiliationComponentKind::Organization,
                "Universität München"
            ),
        ]
    );
    assert!(matches!(
        parse("plain scholarly prose"),
        NativeAffiliationOutcome::Unsupported {
            reason: NativeAffiliationUnsupported::NoRecognizableComponent
        }
    ));
}

#[test]
fn closed_failures_are_redacted_and_serde_replay_is_byte_stable() {
    let cases = [
        ("", NativeAffiliationOutcome::Empty),
        ("   ", NativeAffiliationOutcome::Empty),
        (
            "University, Institute",
            NativeAffiliationOutcome::Ambiguous {
                reason: NativeAffiliationAmbiguous::MultipleOrganizationCandidates,
            },
        ),
        (
            "University,, Berlin",
            NativeAffiliationOutcome::Malformed {
                reason: NativeAffiliationMalformed::RepeatedSeparator,
            },
        ),
        (
            "University\0 Berlin",
            NativeAffiliationOutcome::Malformed {
                reason: NativeAffiliationMalformed::ControlCharacter,
            },
        ),
        (
            "University; Berlin",
            NativeAffiliationOutcome::Unsupported {
                reason: NativeAffiliationUnsupported::UnsupportedPunctuation,
            },
        ),
    ];
    for (input, expected) in cases {
        let actual = parse(input);
        assert_eq!(actual, expected);
        let bytes = serde_json::to_vec(&actual).unwrap();
        if !input.is_empty() {
            assert!(!String::from_utf8_lossy(&bytes).contains(input));
        }
        let replay: NativeAffiliationOutcome = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
    }
}

#[test]
fn oversized_input_is_rejected_before_recognition() {
    assert_eq!(
        parse(&"University ".repeat(100)),
        NativeAffiliationOutcome::Oversized
    );
}

#[test]
fn parser_is_stateless_and_has_no_author_or_link_surface() {
    let parser = NativeAffiliationParser::new();
    assert_eq!(
        parser.parse("University of Example"),
        parse("University of Example")
    );
}
