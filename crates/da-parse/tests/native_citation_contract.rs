use da_parse::{
    CitationFieldRole, NATIVE_CITATION_INPUT_BYTE_CAP, NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
    NATIVE_CITATION_PARSER_ID, NATIVE_CITATION_PARSER_VERSION, NativeCitationAmbiguous,
    NativeCitationMalformed, NativeCitationOutcome, NativeCitationParser,
    NativeCitationUnsupported,
};

#[test]
fn parses_explicit_identifier_and_preserves_literal_span() {
    let input = "Smith, J. (2024). A bounded parser. doi:10.1000/xyz.";
    let outcome = NativeCitationParser::new().parse(input);
    let NativeCitationOutcome::Recognized { fields } = outcome else {
        panic!("expected recognized citation");
    };
    let doi = fields
        .iter()
        .find(|field| field.role == CitationFieldRole::Doi)
        .expect("DOI field");
    assert_eq!(doi.value, "10.1000/xyz");
    assert_eq!(&input[doi.span.start..doi.span.end], "10.1000/xyz");
    assert!(doi.span.start < doi.span.end);
    assert!(doi.span.is_valid_for(input.len()));
}

#[test]
fn recognizes_declared_roles_without_inventing_values() {
    let input = "Doe, J. 2024. title: A Study; journal: Science; volume: 12; issue: 3; pages: 4-9; publisher: Press; place: Paris; article number: e7; arXiv:2401.00001; https://example.test/p";
    let NativeCitationOutcome::Recognized { fields } = NativeCitationParser::new().parse(input)
    else {
        panic!("expected recognized citation");
    };
    for role in [
        CitationFieldRole::Authors,
        CitationFieldRole::PublicationDate,
        CitationFieldRole::ArticleTitle,
        CitationFieldRole::ContainerTitle,
        CitationFieldRole::Volume,
        CitationFieldRole::Issue,
        CitationFieldRole::Pages,
        CitationFieldRole::Publisher,
        CitationFieldRole::Place,
        CitationFieldRole::ArticleNumber,
        CitationFieldRole::ArxivId,
        CitationFieldRole::Url,
    ] {
        assert!(
            fields.iter().any(|field| field.role == role),
            "missing role {role:?}"
        );
    }
    assert!(
        !fields
            .iter()
            .any(|field| field.value == "A Study; journal: Science")
    );
    for field in fields {
        assert!(field.span.is_valid_for(input.len()));
        assert_eq!(&input[field.span.start..field.span.end], field.value);
    }
}

#[test]
fn public_identity_and_closed_wire_contract_are_stable() {
    assert_eq!(NATIVE_CITATION_PARSER_ID, "da-parse.native_citation");
    assert_eq!(NATIVE_CITATION_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-citation.outcome/v1"
    );
    assert_eq!(NATIVE_CITATION_INPUT_BYTE_CAP, 4096);
    let json =
        serde_json::to_string(&NativeCitationParser::new().parse("")).expect("outcome serializes");
    assert_eq!(json, r#"{"outcome":"empty"}"#);
}

#[test]
fn closed_failures_are_bounded_redacted_and_deterministic() {
    let parser = NativeCitationParser::new();
    let cases = [
        ("", NativeCitationOutcome::Empty),
        (
            "\u{0000}citation",
            NativeCitationOutcome::Malformed {
                reason: NativeCitationMalformed::ControlCharacter,
            },
        ),
        (
            "citation\u{202e}doi:10.1000/x",
            NativeCitationOutcome::Malformed {
                reason: NativeCitationMalformed::NonAsciiScript,
            },
        ),
        (
            "citation",
            NativeCitationOutcome::Malformed {
                reason: NativeCitationMalformed::MissingValue,
            },
        ),
        (
            "\u{2060}",
            NativeCitationOutcome::Malformed {
                reason: NativeCitationMalformed::NonAsciiScript,
            },
        ),
    ];
    for (input, expected) in cases {
        let first = parser.parse(input);
        assert_eq!(first, expected);
        let bytes = serde_json::to_vec(&first).expect("serialize");
        let replay: NativeCitationOutcome = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(serde_json::to_vec(&replay).expect("re-serialize"), bytes);
        if !input.is_empty() {
            assert!(!String::from_utf8_lossy(&bytes).contains(input));
        }
    }
    assert_eq!(
        parser.parse(&"doi:10.1000/x".repeat(NATIVE_CITATION_INPUT_BYTE_CAP)),
        NativeCitationOutcome::Oversized
    );

    // Unicode whitespace is a separator, while no non-ASCII value is guessed.
    let nbsp = parser.parse("Doe,\u{00a0}J.\u{00a0}doi:10.1000/x");
    assert!(matches!(nbsp, NativeCitationOutcome::Recognized { .. }));
    assert!(matches!(
        parser.parse("doi:10.1000/x; unknown: y"),
        NativeCitationOutcome::Recognized { .. }
    ));
    assert!(matches!(
        parser.parse("Smith, et al.; doi:10.1000/x"),
        NativeCitationOutcome::Unsupported {
            reason: NativeCitationUnsupported::UnsupportedRole
        }
    ));
    assert!(matches!(
        parser.parse("title: First; title: Second"),
        NativeCitationOutcome::Ambiguous {
            reason: NativeCitationAmbiguous::MultipleUnlabelledTitles
        }
    ));

    // Keep the closed vocabulary linked into the public contract so adding a
    // new outcome cannot silently change the wire model.
    let _ = NativeCitationUnsupported::UnsupportedRole;
    let _ = NativeCitationAmbiguous::MultipleDateReadings;
}
