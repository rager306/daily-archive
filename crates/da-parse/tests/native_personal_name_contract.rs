use da_parse::{
    NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP, NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
    NATIVE_PERSONAL_NAME_PARSER_ID, NATIVE_PERSONAL_NAME_PARSER_VERSION,
    NativePersonalNameAmbiguous, NativePersonalNameMalformed, NativePersonalNameOutcome,
    NativePersonalNameParser, NativePersonalNameUnsupported, PersonalNameContext,
    RecognizedPersonalName,
};

fn parse(context: PersonalNameContext, input: &str) -> NativePersonalNameOutcome {
    NativePersonalNameParser::new().parse(context, input)
}

fn recognized(context: PersonalNameContext, input: &str) -> RecognizedPersonalName {
    match parse(context, input) {
        NativePersonalNameOutcome::Recognized { name } => name,
        other => panic!("expected recognized, got {other:?}"),
    }
}

#[test]
fn public_identity_context_and_wire_contract_are_frozen() {
    assert_eq!(
        NATIVE_PERSONAL_NAME_PARSER_ID,
        "da-parse.native_personal_name"
    );
    assert_eq!(NATIVE_PERSONAL_NAME_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-personal-name.outcome/v1"
    );
    assert_eq!(NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP, 256);
    assert_eq!(PersonalNameContext::Header, PersonalNameContext::Header);
    assert_eq!(PersonalNameContext::Citation, PersonalNameContext::Citation);
}

#[test]
fn header_and_citation_dialects_keep_roles_explicit() {
    let header = recognized(PersonalNameContext::Header, "J. Smith");
    assert_eq!(header.given_name.as_deref(), Some("J"));
    assert_eq!(header.initials, vec!["J"]);
    assert_eq!(header.surname, "Smith");

    let citation = recognized(PersonalNameContext::Citation, "Smith, J.");
    assert_eq!(citation.given_name.as_deref(), Some("J"));
    assert_eq!(citation.initials, vec!["J"]);
    assert_eq!(citation.surname, "Smith");

    let middle = recognized(PersonalNameContext::Header, "Jane Q. Public");
    assert_eq!(middle.given_name.as_deref(), Some("Jane"));
    assert_eq!(middle.middle_names, vec!["Q"]);
    assert_eq!(middle.initials, vec!["Q"]);
    assert_eq!(middle.surname, "Public");
}

#[test]
fn particles_suffixes_and_surface_are_explicit_without_invention() {
    for (input, particles, surname) in [
        ("Ludwig van der Waals", vec!["van", "der"], "van der Waals"),
        ("Gabriel de la Cruz", vec!["de", "la"], "de la Cruz"),
        ("John von Neumann Jr.", vec!["von"], "von Neumann"),
    ] {
        let name = recognized(PersonalNameContext::Header, input);
        assert_eq!(name.surname_particles, particles);
        assert_eq!(name.surname, surname);
        assert_eq!(name.recomposed_surname, surname);
    }
    assert_eq!(
        recognized(PersonalNameContext::Header, "George Smith III")
            .suffix
            .as_deref(),
        Some("III")
    );
    assert_eq!(
        recognized(PersonalNameContext::Citation, "de la Cruz, Gabriel").surname,
        "de la Cruz"
    );
}

#[test]
fn unicode_hyphen_apostrophe_and_initials_are_preserved() {
    let name = recognized(PersonalNameContext::Header, "Élodie M.-C. O’Neill");
    assert_eq!(name.given_name.as_deref(), Some("Élodie"));
    assert_eq!(name.surname, "O’Neill");
    assert_eq!(name.middle_names, vec!["M.-C."]);
    assert_eq!(name.initials, vec!["M", "C"]);
    let non_latin = recognized(PersonalNameContext::Header, "李 小龙");
    assert_eq!(non_latin.surname, "小龙");
    assert_eq!(non_latin.given_name.as_deref(), Some("李"));
}

#[test]
fn closed_failures_are_typed_redacted_and_byte_stable() {
    let cases = [
        ("", NativePersonalNameOutcome::Empty),
        ("   ", NativePersonalNameOutcome::Empty),
        (
            "Smith",
            NativePersonalNameOutcome::Ambiguous {
                reason: NativePersonalNameAmbiguous::SingleToken,
            },
        ),
        (
            "John  Smith",
            NativePersonalNameOutcome::Malformed {
                reason: NativePersonalNameMalformed::RepeatedSeparator,
            },
        ),
        (
            "John\0 Smith",
            NativePersonalNameOutcome::Malformed {
                reason: NativePersonalNameMalformed::ControlCharacter,
            },
        ),
        (
            "Smith, et al.",
            NativePersonalNameOutcome::Unsupported {
                reason: NativePersonalNameUnsupported::EtAl,
            },
        ),
    ];
    for (input, expected) in cases {
        let actual = parse(PersonalNameContext::Header, input);
        assert_eq!(actual, expected);
        let bytes = serde_json::to_vec(&actual).unwrap();
        if !input.is_empty() {
            assert!(!String::from_utf8_lossy(&bytes).contains(input));
        }
        let replay: NativePersonalNameOutcome = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
    }
    assert_eq!(
        parse(
            PersonalNameContext::Citation,
            &"A".repeat(NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP + 1)
        ),
        NativePersonalNameOutcome::Oversized
    );
}

#[test]
fn parser_is_stateless_and_context_is_required() {
    let parser = NativePersonalNameParser::new();
    assert_eq!(
        parser.parse(PersonalNameContext::Header, "J. Smith"),
        parser.parse(PersonalNameContext::Header, "J. Smith")
    );
    assert_ne!(
        parser.parse(PersonalNameContext::Header, "Smith, J."),
        parser.parse(PersonalNameContext::Citation, "Smith, J.")
    );
}
