//! Native date component contract (M464-z84in2 S08/T01).
//!
//! Pins the public [`da_parse::NativeDateParser`] component contract before
//! any integration: exact precision normalization for every sanctioned form
//! (including every display-form class observed in the three checksum-bound
//! preserved TEI fixtures), equivalent separator forms, leap-year boundaries,
//! no-invention of month/day, the closed malformed/unsupported/ambiguous
//! vocabularies, deterministic replay, byte-stable serde roundtrips, stable
//! public wire tags and the redaction guarantee (no source text ever appears
//! in a serialized outcome).
//!
//! Offline and dependency-free: no fixture files, no clock, no network. The
//! oracle corpus parity itself is S08/T02; this contract is the standalone
//! component basis it builds on.

use da_parse::{
    DatePrecision, NATIVE_DATE_INPUT_BYTE_CAP, NATIVE_DATE_OUTCOME_SCHEMA_VERSION,
    NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION, NativeDateAmbiguous, NativeDateMalformed,
    NativeDateOutcome, NativeDateParser, NativeDateUnsupported, RecognizedDate,
};

fn parse(input: &str) -> NativeDateOutcome {
    NativeDateParser::new().parse(input)
}

fn d(year: i32, month: Option<u8>, day: Option<u8>) -> RecognizedDate {
    RecognizedDate::new(year, month, day)
        .unwrap_or_else(|| panic!("test date must be valid: {year} {month:?} {day:?}"))
}

fn json_of(outcome: &NativeDateOutcome) -> String {
    serde_json::to_string(outcome).expect("outcome must serialize")
}

/// Asserts the exact outcome, determinism across repeated calls and a
/// byte-stable serde roundtrip.
fn assert_closed_replay(input: &str, expected: &NativeDateOutcome) {
    let first = parse(input);
    assert_eq!(&first, expected, "outcome mismatch for the pinned input");
    let second = parse(input);
    assert_eq!(second, first, "repeated call must be deterministic");
    let bytes = serde_json::to_vec(&first).expect("serialize");
    let restored: NativeDateOutcome = serde_json::from_slice(&bytes).expect("deserialize");
    let bytes_again = serde_json::to_vec(&restored).expect("re-serialize");
    assert_eq!(bytes, bytes_again, "serde roundtrip must be byte-stable");
}

fn assert_recognized(input: &str, year: i32, month: Option<u8>, day: Option<u8>) {
    let expected = NativeDateOutcome::Recognized {
        date: d(year, month, day),
    };
    assert_closed_replay(input, &expected);
    let date = expected.recognized().expect("recognized");
    assert!(
        date.precision_matches_components(),
        "precision must exactly track present components"
    );
    if let Some(m) = date.month {
        assert!((1..=12).contains(&m), "month domain");
    }
    if let Some(day) = date.day {
        assert!(day >= 1, "day domain");
        assert!(
            day <= RecognizedDate::days_in_month(date.year, date.month.expect("month present"))
                .expect("month domain"),
            "day must be calendar-valid"
        );
    }
}

#[test]
fn identity_and_wire_constants_are_stable() {
    assert_eq!(NATIVE_DATE_PARSER_ID, "da-parse.native_date");
    assert_eq!(NATIVE_DATE_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_DATE_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-date.outcome/v1"
    );
    assert_eq!(NATIVE_DATE_INPUT_BYTE_CAP, 256);
}

#[test]
fn wire_tags_are_exact_for_every_outcome_kind() {
    assert_eq!(
        json_of(&parse("2025")),
        r#"{"outcome":"recognized","date":{"year":2025,"month":null,"day":null,"precision":"year"}}"#
    );
    assert_eq!(json_of(&parse("   ")), r#"{"outcome":"empty"}"#);
    assert_eq!(
        json_of(&parse("2025\u{0000}")),
        r#"{"outcome":"malformed","reason":"control_character"}"#
    );
    assert_eq!(
        json_of(&parse("1990s")),
        r#"{"outcome":"unsupported","reason":"decade"}"#
    );
    assert_eq!(
        json_of(&parse("03/04/2021")),
        r#"{"outcome":"ambiguous","reason":"numeric_component_order"}"#
    );
    assert_eq!(
        json_of(&parse(&"9".repeat(NATIVE_DATE_INPUT_BYTE_CAP + 1))),
        r#"{"outcome":"oversized"}"#
    );
}

#[test]
fn oracle_corpus_display_form_classes_normalize_with_exact_precision() {
    // Year-only forms (the dominant corpus class).
    for input in ["2025", "  2025  ", "2025. 2025"] {
        assert_recognized(input, 2025, None, None);
    }
    assert_recognized("1999", 1999, None, None);
    assert_recognized("2015", 2015, None, None);
    assert_recognized("2025. 2025", 2025, None, None);

    // Year-month forms (full month names).
    assert_recognized("February 2026", 2026, Some(2), None);
    assert_recognized("November 2020", 2020, Some(11), None);
    assert_recognized("April 2009", 2009, Some(4), None);
    // ... with prefix context noise, exactly as in the preserved TEI.
    assert_recognized("2025. April 2025", 2025, Some(4), None);
    assert_recognized("Aime-2025, February 2025", 2025, Some(2), None);
    // ... and numeric ISO precision forms with equivalent separators.
    assert_recognized("2026-02", 2026, Some(2), None);
    assert_recognized("2026/02", 2026, Some(2), None);
    assert_recognized("2026.02", 2026, Some(2), None);

    // Full-date forms (day month-name year, abbreviated and full).
    assert_recognized("14 Feb 2026", 2026, Some(2), Some(14));
    assert_recognized("26 May 2026", 2026, Some(5), Some(26));
    assert_recognized("11 May 2026", 2026, Some(5), Some(11));
    // ... month-day-year order, optional comma.
    assert_recognized("July 16, 2025", 2025, Some(7), Some(16));
    assert_recognized("July 16 2025", 2025, Some(7), Some(16));
    assert_recognized("16 July 2025", 2025, Some(7), Some(16));
    // ... with prefix context noise, exactly as in the preserved TEI.
    assert_recognized("2025. July 16, 2025", 2025, Some(7), Some(16));
    assert_recognized("2026. 2026-05-06", 2026, Some(5), Some(6));
    // ... numeric ISO full form with equivalent separators.
    assert_recognized("2026-05-06", 2026, Some(5), Some(6));
    assert_recognized("2026/05/06", 2026, Some(5), Some(6));
    assert_recognized("2026.05.06", 2026, Some(5), Some(6));
}

#[test]
fn equivalent_sanctioned_forms_produce_identical_outcomes() {
    let iso = parse("2026-05-06");
    for equivalent in ["2026/05/06", "2026.05.06"] {
        assert_eq!(parse(equivalent), iso, "{equivalent} must equal ISO form");
    }
    let bytes = serde_json::to_vec(&iso).expect("serialize");
    for equivalent in ["2026/05/06", "2026.05.06"] {
        assert_eq!(
            serde_json::to_vec(&parse(equivalent)).expect("serialize"),
            bytes,
            "wire bytes must be identical across equivalent separators"
        );
    }
}

#[test]
fn month_names_full_abbreviated_and_case_insensitive() {
    for (name, month) in [
        ("january", 1u8),
        ("february", 2),
        ("march", 3),
        ("april", 4),
        ("may", 5),
        ("june", 6),
        ("july", 7),
        ("august", 8),
        ("september", 9),
        ("october", 10),
        ("november", 11),
        ("december", 12),
        ("jan", 1),
        ("feb", 2),
        ("mar", 3),
        ("apr", 4),
        ("jun", 6),
        ("jul", 7),
        ("aug", 8),
        ("sep", 9),
        ("sept", 9),
        ("oct", 10),
        ("nov", 11),
        ("dec", 12),
    ] {
        assert_recognized(&format!("{name} 2025"), 2025, Some(month), None);
        assert_recognized(&format!("5 {name} 2025"), 2025, Some(month), Some(5));
        let upper = name.to_uppercase();
        assert_recognized(&format!("{upper} 2025"), 2025, Some(month), None);
    }
}

#[test]
fn leap_year_boundaries_are_exact() {
    assert_recognized("2024-02-29", 2024, Some(2), Some(29));
    // 2000 is a leap year (divisible by 400).
    assert_recognized("2000-02-29", 2000, Some(2), Some(29));
    // 1900 is not (century not divisible by 400).
    assert_closed_replay(
        "1900-02-29",
        &NativeDateOutcome::Malformed {
            reason: NativeDateMalformed::CalendarInvalid,
        },
    );
    assert_closed_replay(
        "2023-02-29",
        &NativeDateOutcome::Malformed {
            reason: NativeDateMalformed::CalendarInvalid,
        },
    );
}

#[test]
fn iso_calendar_validation_is_exhaustive_over_domain_boundaries() {
    for year in [1900i32, 1999, 2000, 2023, 2024] {
        for month in 1..=12u8 {
            for day in 0..=32u8 {
                let input = format!("{year:04}-{month:02}-{day:02}");
                let dim = RecognizedDate::days_in_month(year, month).expect("month domain");
                let outcome = parse(&input);
                if (1..=dim).contains(&day) {
                    let date = outcome
                        .recognized()
                        .unwrap_or_else(|| panic!("{input} must be recognized"));
                    assert_eq!(date, d(year, Some(month), Some(day)));
                    assert_eq!(date.precision, DatePrecision::FullDate);
                } else if day == 0 || day > 31 {
                    assert_eq!(
                        outcome,
                        NativeDateOutcome::Malformed {
                            reason: NativeDateMalformed::ComponentOutOfRange
                        },
                        "{input} is outside the day domain"
                    );
                } else {
                    assert_eq!(
                        outcome,
                        NativeDateOutcome::Malformed {
                            reason: NativeDateMalformed::CalendarInvalid
                        },
                        "{input} must be calendar-rejected"
                    );
                }
            }
        }
    }
}

#[test]
fn no_month_or_day_is_ever_invented() {
    for input in ["2025", " 2025 ", "2025. 2025", "1999", "2015"] {
        let date = parse(input)
            .recognized()
            .unwrap_or_else(|| panic!("{input} must be recognized"));
        assert_eq!(
            date.month, None,
            "{input}: year-only must not invent a month"
        );
        assert_eq!(date.day, None, "{input}: year-only must not invent a day");
        assert_eq!(date.precision, DatePrecision::Year);
    }
    for input in ["2026-02", "2026/02", "2026.02", "February 2026", "feb 2026"] {
        let date = parse(input)
            .recognized()
            .unwrap_or_else(|| panic!("{input} must be recognized"));
        assert_eq!(date.day, None, "{input}: year-month must not invent a day");
        assert_eq!(date.precision, DatePrecision::YearMonth);
        assert!(date.month.is_some(), "{input}: declared month is kept");
    }
}

#[test]
fn empty_and_whitespace_only_inputs_are_empty() {
    assert_closed_replay("", &NativeDateOutcome::Empty);
    assert_closed_replay("   ", &NativeDateOutcome::Empty);
    assert_closed_replay("\t\n\r ", &NativeDateOutcome::Empty);
}

#[test]
fn malformed_inputs_have_closed_non_panicking_outcomes() {
    let cases: &[(&str, NativeDateMalformed)] = &[
        // Control characters anywhere (NUL, bell, escape, embedded).
        ("2025\u{0000}", NativeDateMalformed::ControlCharacter),
        ("\u{0007}2025", NativeDateMalformed::ControlCharacter),
        ("2025-\u{0001}05", NativeDateMalformed::ControlCharacter),
        // Non-ASCII script: fullwidth and Arabic-Indic digits, bidi, mixed.
        (
            "\u{FF11}\u{FF12}\u{FF13}\u{FF14}",
            NativeDateMalformed::NonAsciiScript,
        ),
        (
            "\u{0662}\u{0660}\u{0662}\u{0665}",
            NativeDateMalformed::NonAsciiScript,
        ),
        ("2025-0\u{0661}-15", NativeDateMalformed::NonAsciiScript),
        ("2025\u{202E}35", NativeDateMalformed::NonAsciiScript),
        // ASCII characters outside the closed grammar.
        ("2025?", NativeDateMalformed::UnexpectedCharacter),
        ("2025:05:06", NativeDateMalformed::UnexpectedCharacter),
        // Empty numeric components.
        ("2025-", NativeDateMalformed::EmptyComponent),
        ("-2025", NativeDateMalformed::EmptyComponent),
        ("2025--05", NativeDateMalformed::EmptyComponent),
        ("2025..05", NativeDateMalformed::EmptyComponent),
        ("/2025/05", NativeDateMalformed::EmptyComponent),
        ("2025,", NativeDateMalformed::EmptyComponent),
        // Component value domains.
        ("2025-13-01", NativeDateMalformed::ComponentOutOfRange),
        ("2026-00-10", NativeDateMalformed::ComponentOutOfRange),
        ("2026-01-32", NativeDateMalformed::ComponentOutOfRange),
        ("2026-01-00", NativeDateMalformed::ComponentOutOfRange),
        // Calendar validity.
        ("February 30, 2025", NativeDateMalformed::CalendarInvalid),
        ("31 April 2025", NativeDateMalformed::CalendarInvalid),
        // Valid date prefix followed by garbage is rejected.
        ("2019-01-15x", NativeDateMalformed::TrailingGarbage),
        ("2019-01-15 x", NativeDateMalformed::TrailingGarbage),
        ("2025-05 garbage", NativeDateMalformed::TrailingGarbage),
        ("2025 05", NativeDateMalformed::TrailingGarbage),
        // No closed shape explains the tokens.
        ("hello world", NativeDateMalformed::GrammarMismatch),
        ("February2025", NativeDateMalformed::GrammarMismatch),
    ];
    for (input, reason) in cases {
        assert_closed_replay(input, &NativeDateOutcome::Malformed { reason: *reason });
    }
}

#[test]
fn unsupported_scholarly_forms_have_closed_outcomes() {
    let cases: &[(&str, NativeDateUnsupported)] = &[
        ("2018-2020", NativeDateUnsupported::DateRange),
        ("2018\u{2013}2022", NativeDateUnsupported::DateRange),
        ("2018/2022", NativeDateUnsupported::DateRange),
        ("1990s", NativeDateUnsupported::Decade),
        ("1990s.", NativeDateUnsupported::Decade),
        ("Q3 2019", NativeDateUnsupported::Quarter),
        ("q3 2019", NativeDateUnsupported::Quarter),
        ("1st quarter 2019", NativeDateUnsupported::Quarter),
        ("Spring 2019", NativeDateUnsupported::Season),
        ("winter 2020", NativeDateUnsupported::Season),
        ("15th January 2019", NativeDateUnsupported::OrdinalDaySuffix),
        ("98", NativeDateUnsupported::YearTooShort),
        ("999", NativeDateUnsupported::YearTooShort),
        ("0999", NativeDateUnsupported::YearTooShort),
        ("0999-05-06", NativeDateUnsupported::YearTooShort),
        ("123456", NativeDateUnsupported::YearTooLong),
        ("1234567890", NativeDateUnsupported::YearTooLong),
        ("12345678", NativeDateUnsupported::YearTooLong),
    ];
    for (input, reason) in cases {
        assert_closed_replay(input, &NativeDateOutcome::Unsupported { reason: *reason });
    }
}

#[test]
fn non_iso_numeric_orders_are_ambiguous_not_guessed() {
    let ambiguous = NativeDateOutcome::Ambiguous {
        reason: NativeDateAmbiguous::NumericComponentOrder,
    };
    for input in ["03/04/2021", "15-01-2019", "3.4.2021", "12-11-2013"] {
        assert_closed_replay(input, &ambiguous);
    }
    // Neither reading is domain-plausible: out-of-range, not ambiguous.
    assert_closed_replay(
        "45/45/2021",
        &NativeDateOutcome::Malformed {
            reason: NativeDateMalformed::ComponentOutOfRange,
        },
    );
}

#[test]
fn input_byte_cap_bounds_all_work() {
    // One byte over the cap: oversized, never scanned.
    assert_closed_replay(
        &"9".repeat(NATIVE_DATE_INPUT_BYTE_CAP + 1),
        &NativeDateOutcome::Oversized,
    );
    assert_closed_replay(
        &format!("{}2025", " ".repeat(NATIVE_DATE_INPUT_BYTE_CAP - 3)),
        &NativeDateOutcome::Oversized,
    );
    // Exactly at the cap the input is still processed normally.
    assert_recognized(
        &format!("{}2025", " ".repeat(NATIVE_DATE_INPUT_BYTE_CAP - 4)),
        2025,
        None,
        None,
    );
    // A 200-digit number is under the cap but bounded: closed unsupported.
    assert_closed_replay(
        &"9".repeat(200),
        &NativeDateOutcome::Unsupported {
            reason: NativeDateUnsupported::YearTooLong,
        },
    );
}

#[test]
fn glued_token_clusters_are_never_split() {
    // "February2025" is one glued cluster; the year is not extracted.
    assert_closed_replay(
        "February2025",
        &NativeDateOutcome::Malformed {
            reason: NativeDateMalformed::GrammarMismatch,
        },
    );
    // "14Feb 2026": the glued cluster is prefix context; the trailing year
    // consumes the rest of the input (documented selection policy).
    assert_recognized("14Feb 2026", 2026, None, None);
}

#[test]
fn outcomes_never_carry_source_text() {
    let outcome = parse("Aime-2025, February 2025");
    assert_eq!(
        outcome,
        NativeDateOutcome::Recognized {
            date: d(2025, Some(2), None)
        }
    );
    let json = json_of(&outcome);
    assert!(!json.contains("Aime"), "prefix noise must be redacted");
    assert!(!json.contains("February"), "display text must be redacted");
    let malformed_json = json_of(&parse("hello world 2025?"));
    assert!(
        !malformed_json.contains("hello"),
        "malformed outcomes must not echo input"
    );
}

#[test]
fn adversarial_inputs_stay_closed_and_deterministic() {
    let adversarial = [
        "\u{0000}".repeat(10),
        "2025\u{0001}".repeat(5),
        "\u{0661}\u{0662}".repeat(30),
        "2025-".repeat(40),
        "-".repeat(256),
        ".".repeat(250),
        "9".repeat(256),
        "0".repeat(255),
        format!("{}2025", "\u{00A0}".repeat(10)),
        format!("2025{}", "\u{202E}".repeat(4)),
        "2025-02-29".repeat(25),
        format!("2025.{}", " 2025".repeat(20)),
    ];
    for input in adversarial {
        assert!(
            input.len() <= NATIVE_DATE_INPUT_BYTE_CAP
                || parse(&input) == NativeDateOutcome::Oversized
        );
        let first = parse(&input);
        let second = parse(&input);
        assert_eq!(
            first, second,
            "adversarial input must parse deterministically"
        );
        // Roundtrip every adversarial outcome: closed vocabulary survives.
        let bytes = serde_json::to_vec(&first).expect("serialize");
        let restored: NativeDateOutcome = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(restored, first, "outcome must survive a serde roundtrip");
        if let NativeDateOutcome::Recognized { date } = first {
            assert!(date.precision_matches_components());
        }
    }
}

#[test]
fn recognized_constructor_is_closed() {
    // Year domain.
    assert!(RecognizedDate::new(999, None, None).is_none());
    assert!(RecognizedDate::new(10_000, None, None).is_none());
    assert!(RecognizedDate::new(1000, None, None).is_some());
    assert!(RecognizedDate::new(9999, None, None).is_some());
    // Month domain.
    assert!(RecognizedDate::new(2025, Some(0), None).is_none());
    assert!(RecognizedDate::new(2025, Some(13), None).is_none());
    // No day without month (would be invented precision).
    assert!(RecognizedDate::new(2025, None, Some(15)).is_none());
    // Calendar-valid day domain.
    assert!(RecognizedDate::new(2024, Some(2), Some(29)).is_some());
    assert!(RecognizedDate::new(2023, Some(2), Some(29)).is_none());
    assert!(RecognizedDate::new(2025, Some(4), Some(31)).is_none());
    // Leap-rule helpers are independent of any dependency.
    assert!(RecognizedDate::is_leap_year(2000));
    assert!(RecognizedDate::is_leap_year(2024));
    assert!(!RecognizedDate::is_leap_year(1900));
    assert!(!RecognizedDate::is_leap_year(2023));
    assert_eq!(RecognizedDate::days_in_month(2024, 2), Some(29));
    assert_eq!(RecognizedDate::days_in_month(2023, 2), Some(28));
    assert_eq!(RecognizedDate::days_in_month(2025, 4), Some(30));
    assert_eq!(RecognizedDate::days_in_month(2025, 13), None);
}
