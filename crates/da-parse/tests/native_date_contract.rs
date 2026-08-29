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

// ───── S08/T04: shadow-boundary closure on the component itself ─────

#[test]
fn s08_component_stays_stateless_across_instances_and_call_sequences() {
    // Freshly constructed parsers agree on every outcome class: no lazy
    // init, no environment dependence, no hidden construction state.
    let inputs = [
        "2025",
        "2025-07",
        "2024-02-29",
        "July 16, 2025",
        "16 July 2025",
        "2025-02-30",
        "1990s",
        "03/04/2021",
        "",
        "   ",
    ];
    let a = NativeDateParser::new();
    let b = NativeDateParser;
    for input in inputs {
        assert_eq!(
            a.parse(input),
            b.parse(input),
            "independent instances diverged on {input:?}"
        );
    }

    // A long-lived shared instance produces exactly the fresh-instance
    // outcomes for an interleaved call sequence: parsing never depends on
    // prior calls (no cache, no accumulated state, no environment reads).
    let sequence = [
        "2025",
        "Feb 3, 2021",
        "2021-02-29",
        "not a date",
        "2020/07/04",
        "September 30, 2024",
    ];
    let shared = NativeDateParser::new();
    let via_shared: Vec<NativeDateOutcome> =
        sequence.iter().map(|input| shared.parse(input)).collect();
    let via_fresh: Vec<NativeDateOutcome> = sequence
        .iter()
        .map(|input| NativeDateParser::new().parse(input))
        .collect();
    assert_eq!(
        via_shared, via_fresh,
        "a shared parser instance must stay stateless across calls"
    );

    // The candidate identity stays the versioned shadow pair the registry
    // and the baseline bind against.
    assert_eq!(NATIVE_DATE_PARSER_ID, "da-parse.native_date");
    assert_eq!(NATIVE_DATE_PARSER_VERSION, "1.0.0");
}

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

// ==================================================================== S08/T02 ===
// Checksum-bound oracle corpus parity (same-n) harness.
//
// Independence rule: oracle expected values come ONLY from the Java-produced
// `when` attributes of the preserved rich-profile TEI fixtures, parsed by a
// deliberately separate calendar model below — the production date parser is
// never consulted for expected values. The scanner is a byte-level XML walker
// with a strict element stack; malformed XML, missing/invalid `when`, entities
// or non-UTF-8 date text are typed failures, never skips.
//
// Redaction rule: no raw TEI, no display text and no physical paths enter any
// error value, digest or report — per-occurrence evidence is fixture id,
// ordinal, span, scope tag and SHA-256 hashes only.

mod oracle_harness {
    use super::*;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    const PROVENANCE_REL: &str = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
    /// Corpus denominator pinned by the slice plan: hub 80 + rlm 50 + reca 43.
    const EXPECTED_TOTAL: usize = 173;
    /// Sanity bound for a single `<date>` element span in bytes.
    const MAX_ELEMENT_SPAN: usize = 4096;

    #[derive(Deserialize)]
    struct ProvenanceDoc {
        fixtures: Vec<ProvenanceFixture>,
    }

    #[derive(Deserialize)]
    struct ProvenanceFixture {
        id: String,
        sha256: String,
        bytes: u64,
    }

    struct FixtureSpec {
        id: &'static str,
        rel: &'static str,
        sha256: &'static str,
        bytes: u64,
        /// Pinned per-scope split: (header article, header bibliography, bibliography).
        scope_split: (usize, usize, usize),
    }

    const FIXTURES: [FixtureSpec; 3] = [
        FixtureSpec {
            id: "hub_2507.19457",
            rel: "crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml",
            sha256: "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
            bytes: 404_197,
            scope_split: (1, 1, 78),
        },
        FixtureSpec {
            id: "rlm_2512.24601",
            rel: "crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml",
            sha256: "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
            bytes: 288_029,
            scope_split: (1, 1, 48),
        },
        FixtureSpec {
            id: "reca_2605.26525v1",
            rel: "crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
            sha256: "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
            bytes: 304_349,
            scope_split: (1, 1, 41),
        },
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum OracleScope {
        /// TEI/teiHeader/fileDesc/publicationStmt.
        HeaderArticle,
        /// TEI/teiHeader/fileDesc/sourceDesc.
        HeaderBibliography,
        /// TEI/text/back/listBibl.
        Bibliography,
    }

    impl OracleScope {
        fn index(self) -> usize {
            match self {
                OracleScope::HeaderArticle => 0,
                OracleScope::HeaderBibliography => 1,
                OracleScope::Bibliography => 2,
            }
        }

        fn tag(self) -> &'static str {
            [
                "header_article_date",
                "header_bibliography_date",
                "bibliography_date",
            ][self.index()]
        }
    }

    /// Independent oracle `when` model: strict `####` | `####-##` | `####-##-##`
    /// with its own calendar math. Never reuses production date parsing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum OracleWhen {
        Y(i32),
        YM(i32, u8),
        Ymd(i32, u8, u8),
    }

    fn oracle_is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    fn oracle_days_in_month(year: i32, month: u8) -> Option<u8> {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
            4 | 6 | 9 | 11 => Some(30),
            2 if oracle_is_leap(year) => Some(29),
            2 => Some(28),
            _ => None,
        }
    }

    impl OracleWhen {
        fn parse(raw: &str) -> Option<Self> {
            let digits = |part: &[u8]| !part.is_empty() && part.iter().all(u8::is_ascii_digit);
            let num = |part: &[u8]| -> Option<i64> {
                if digits(part) {
                    std::str::from_utf8(part).ok()?.parse().ok()
                } else {
                    None
                }
            };
            let parts: Vec<&[u8]> = raw.as_bytes().split(|b| *b == b'-').collect();
            match parts.len() {
                1 => {
                    let year = num(parts[0])?;
                    if parts[0].len() == 4 && (1000..=9999).contains(&year) {
                        return Some(OracleWhen::Y(year as i32));
                    }
                    None
                }
                2 => {
                    let (year, month) = (num(parts[0])?, num(parts[1])?);
                    let month = u8::try_from(month).ok()?;
                    if parts[0].len() == 4
                        && parts[1].len() == 2
                        && (1000..=9999).contains(&year)
                        && (1..=12).contains(&month)
                    {
                        return Some(OracleWhen::YM(year as i32, month));
                    }
                    None
                }
                3 => {
                    let (year, month, day) = (num(parts[0])?, num(parts[1])?, num(parts[2])?);
                    let (month, day) = (u8::try_from(month).ok()?, u8::try_from(day).ok()?);
                    if parts[0].len() == 4
                        && parts[1].len() == 2
                        && parts[2].len() == 2
                        && (1000..=9999).contains(&year)
                        && (1..=12).contains(&month)
                        && day >= 1
                        && matches!(oracle_days_in_month(year as i32, month), Some(max) if day <= max)
                    {
                        return Some(OracleWhen::Ymd(year as i32, month, day));
                    }
                    None
                }
                _ => None,
            }
        }

        fn as_recognized(self) -> RecognizedDate {
            match self {
                OracleWhen::Y(y) => RecognizedDate::new(y, None, None),
                OracleWhen::YM(y, m) => RecognizedDate::new(y, Some(m), None),
                OracleWhen::Ymd(y, m, d) => RecognizedDate::new(y, Some(m), Some(d)),
            }
            .expect("oracle when was calendar-validated at parse time")
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WhenFailure {
        Missing,
        Malformed,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RowFailure {
        SpanOverflow,
        DuplicateOrdinal,
        Reordered,
        EmptyText,
        EntityInText,
        NonUtf8Text,
    }

    /// Typed oracle harness failures. Never carries raw TEI or display text.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum OracleError {
        ChecksumMismatch {
            fixture: &'static str,
        },
        XmlMalformed {
            fixture: &'static str,
            offset: usize,
            reason: &'static str,
        },
        OracleWhen {
            fixture: &'static str,
            ordinal: usize,
            reason: WhenFailure,
        },
        UnexpectedScope {
            fixture: &'static str,
            ordinal: usize,
        },
        RowInvariants {
            fixture: &'static str,
            ordinal: usize,
            reason: RowFailure,
        },
        SameNDrift {
            fixture: &'static str,
            oracle_n: usize,
            candidate_n: usize,
        },
        DenominatorDrift {
            fixture: &'static str,
            expected: usize,
            actual: usize,
        },
        ScopeMix {
            fixture: &'static str,
            scope: &'static str,
            expected: usize,
            actual: usize,
        },
        ParityMismatch {
            fixture: &'static str,
            ordinal: usize,
        },
        ProvenanceIdentity {
            reason: &'static str,
        },
    }

    #[derive(Clone)]
    struct ScannedDate {
        span: (usize, usize),
        scope: OracleScope,
        when: String,
        text: String,
    }

    #[derive(Clone)]
    struct OracleRow {
        fixture: &'static str,
        ordinal: usize,
        span: (usize, usize),
        scope: OracleScope,
        input_sha256: [u8; 32],
        when_sha256: [u8; 32],
        oracle: OracleWhen,
        candidate: NativeDateOutcome,
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode(sha256_raw(bytes))
    }

    fn sha256_raw(bytes: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }

    fn find(data: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || from >= data.len() {
            return None;
        }
        data[from..]
            .windows(needle.len())
            .position(|window| window == needle)
            .map(|position| position + from)
    }

    fn starts(data: &[u8], at: usize, prefix: &[u8]) -> bool {
        data.len() >= at + prefix.len() && &data[at..at + prefix.len()] == prefix
    }

    /// True iff `at` starts an opening/closing tag exactly named `name`
    /// (the following byte must not extend the name).
    fn is_tag_start(data: &[u8], at: usize, name: &str, closing: bool) -> bool {
        let mut prefix = String::with_capacity(name.len() + 2);
        prefix.push('<');
        if closing {
            prefix.push('/');
        }
        prefix.push_str(name);
        if !starts(data, at, prefix.as_bytes()) {
            return false;
        }
        match data.get(at + prefix.len()) {
            None => false,
            Some(b) => {
                !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-' || *b == b'.' || *b == b':')
            }
        }
    }

    fn element_name(tag: &[u8]) -> Option<String> {
        let mut i = usize::from(tag.first() == Some(&b'/'));
        let start = i;
        if i < tag.len() && (tag[i].is_ascii_alphabetic() || tag[i] == b'_') {
            while i < tag.len()
                && (tag[i].is_ascii_alphanumeric()
                    || tag[i] == b'_'
                    || tag[i] == b'.'
                    || tag[i] == b'-'
                    || tag[i] == b':')
            {
                i += 1;
            }
            return Some(String::from_utf8_lossy(&tag[start..i]).into_owned());
        }
        None
    }

    /// Bounded attribute reader for one tag body (`name="value"` / `'value'`).
    /// Known simplification: tag end is the first `>` byte; attribute values in
    /// the pinned oracle fixtures never contain `>` (checksum-bound corpus).
    fn attr_map(tag: &[u8]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < tag.len() {
            let is_name_start = |b: u8| b.is_ascii_alphabetic() || b == b'_' || b == b':';
            let is_name_byte = |b: u8| {
                b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'.' || b == b'-'
            };
            if is_name_start(tag[i]) {
                let start = i;
                while i < tag.len() && is_name_byte(tag[i]) {
                    i += 1;
                }
                let name = String::from_utf8_lossy(&tag[start..i]).into_owned();
                while i < tag.len() && tag[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < tag.len() && tag[i] == b'=' {
                    i += 1;
                    while i < tag.len() && tag[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if i < tag.len() && (tag[i] == b'"' || tag[i] == b'\'') {
                        let quote = tag[i];
                        i += 1;
                        let value_start = i;
                        while i < tag.len() && tag[i] != quote {
                            i += 1;
                        }
                        let value = String::from_utf8_lossy(&tag[value_start..i]).into_owned();
                        if i < tag.len() {
                            i += 1;
                        }
                        out.push((name, value));
                    }
                }
            } else {
                i += 1;
            }
        }
        out
    }

    struct ScanStats {
        published: Vec<ScannedDate>,
        /// Every `<date>` element seen regardless of `type` — the pristine
        /// corpus must contain zero non-published date elements so that a
        /// re-typed date can never be silently excluded from the denominator.
        date_elements_total: usize,
    }

    /// Byte-level scan for all and only `<date type="published">` occurrences
    /// in the three sanctioned TEI scopes, in document order.
    fn scan_published(fixture: &'static str, data: &[u8]) -> Result<ScanStats, OracleError> {
        let n = data.len();
        let mut i = 0usize;
        let mut stack: Vec<String> = Vec::new();
        let mut stats = ScanStats {
            published: Vec::new(),
            date_elements_total: 0,
        };
        while i < n {
            let lt = match find(data, i, b"<") {
                Some(p) => p,
                None => break,
            };
            if starts(data, lt, b"<!--") {
                i = match find(data, lt, b"-->") {
                    Some(e) => e + 3,
                    None => {
                        return Err(OracleError::XmlMalformed {
                            fixture,
                            offset: lt,
                            reason: "unterminated_comment",
                        });
                    }
                };
                continue;
            }
            if starts(data, lt, b"<![CDATA[") {
                i = match find(data, lt, b"]]>") {
                    Some(e) => e + 3,
                    None => {
                        return Err(OracleError::XmlMalformed {
                            fixture,
                            offset: lt,
                            reason: "unterminated_cdata",
                        });
                    }
                };
                continue;
            }
            if starts(data, lt, b"<?") || starts(data, lt, b"<!") {
                i = match find(data, lt, b">") {
                    Some(e) => e + 1,
                    None => {
                        return Err(OracleError::XmlMalformed {
                            fixture,
                            offset: lt,
                            reason: "unterminated_decl",
                        });
                    }
                };
                continue;
            }
            let gt = match find(data, lt, b">") {
                Some(g) => g,
                None => {
                    return Err(OracleError::XmlMalformed {
                        fixture,
                        offset: lt,
                        reason: "unterminated_tag",
                    });
                }
            };
            let tag = &data[lt + 1..gt];
            i = gt + 1;
            let closing = tag.first() == Some(&b'/');
            let name = match element_name(tag) {
                Some(nm) => nm,
                None => continue,
            };
            let selfclose = tag.last() == Some(&b'/');
            if closing {
                match stack.last() {
                    Some(top) if *top == name => {
                        stack.pop();
                    }
                    _ => {
                        return Err(OracleError::XmlMalformed {
                            fixture,
                            offset: lt,
                            reason: "mismatched_close",
                        });
                    }
                }
                continue;
            }
            if name == "date" {
                stats.date_elements_total += 1;
                let attrs = attr_map(tag);
                let is_published = attrs
                    .iter()
                    .any(|(key, value)| key == "type" && value == "published");
                if is_published {
                    let scope = if stack.iter().any(|s| s.as_str() == "publicationStmt") {
                        OracleScope::HeaderArticle
                    } else if stack.iter().any(|s| s.as_str() == "sourceDesc") {
                        OracleScope::HeaderBibliography
                    } else if stack.iter().any(|s| s.as_str() == "listBibl") {
                        OracleScope::Bibliography
                    } else {
                        return Err(OracleError::UnexpectedScope {
                            fixture,
                            ordinal: stats.published.len(),
                        });
                    };
                    if selfclose {
                        return Err(OracleError::XmlMalformed {
                            fixture,
                            offset: lt,
                            reason: "published_date_selfclosed",
                        });
                    }
                    let mut j = i;
                    let mut content_end: Option<usize> = None;
                    while j < n {
                        let nxt = match find(data, j, b"<") {
                            Some(p) => p,
                            None => break,
                        };
                        if is_tag_start(data, nxt, "date", true) {
                            content_end = Some(nxt);
                            j = match find(data, nxt, b">") {
                                Some(g) => g + 1,
                                None => {
                                    return Err(OracleError::XmlMalformed {
                                        fixture,
                                        offset: nxt,
                                        reason: "unterminated_date_close",
                                    });
                                }
                            };
                            break;
                        } else if is_tag_start(data, nxt, "date", false) {
                            j = match find(data, nxt, b">") {
                                Some(g) => g + 1,
                                None => {
                                    return Err(OracleError::XmlMalformed {
                                        fixture,
                                        offset: nxt,
                                        reason: "unterminated_nested_date",
                                    });
                                }
                            };
                        } else {
                            j = match find(data, nxt, b">") {
                                Some(g) => g + 1,
                                None => {
                                    return Err(OracleError::XmlMalformed {
                                        fixture,
                                        offset: nxt,
                                        reason: "unterminated_inner_tag",
                                    });
                                }
                            };
                        }
                    }
                    let content_end = match content_end {
                        Some(end) => end,
                        None => {
                            return Err(OracleError::XmlMalformed {
                                fixture,
                                offset: lt,
                                reason: "date_not_closed",
                            });
                        }
                    };
                    let inner = &data[i..content_end];
                    if inner.contains(&b'&') {
                        return Err(OracleError::RowInvariants {
                            fixture,
                            ordinal: stats.published.len(),
                            reason: RowFailure::EntityInText,
                        });
                    }
                    let text = std::str::from_utf8(inner)
                        .map_err(|_| OracleError::RowInvariants {
                            fixture,
                            ordinal: stats.published.len(),
                            reason: RowFailure::NonUtf8Text,
                        })?
                        .to_owned();
                    let when = attrs
                        .iter()
                        .find(|(key, _)| key == "when")
                        .map(|(_, value)| value.clone())
                        .unwrap_or_default();
                    stats.published.push(ScannedDate {
                        span: (lt, j),
                        scope,
                        when,
                        text,
                    });
                    // Fully consumed through `</date>`: nothing pushed on the stack.
                    i = j;
                    continue;
                }
            }
            if !selfclose {
                stack.push(name);
            }
        }
        Ok(stats)
    }

    /// Row invariants on scanned dates: bounded strictly-increasing spans,
    /// non-empty display text, then one oracle `when` + one candidate parse
    /// per row (the parser is total; a skip would be a same-n drift).
    fn build_rows(
        fixture: &'static str,
        scanned: Vec<ScannedDate>,
        data_len: usize,
    ) -> Result<Vec<OracleRow>, OracleError> {
        let mut last_end: Option<usize> = None;
        for (ordinal, date) in scanned.iter().enumerate() {
            if date.span.1 > data_len
                || date.span.0 >= date.span.1
                || date.span.1 - date.span.0 > MAX_ELEMENT_SPAN
            {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal,
                    reason: RowFailure::SpanOverflow,
                });
            }
            if let Some(prev_end) = last_end
                && date.span.0 <= prev_end
            {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal,
                    reason: RowFailure::Reordered,
                });
            }
            if date.text.trim().is_empty() {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal,
                    reason: RowFailure::EmptyText,
                });
            }
            last_end = Some(date.span.1);
        }
        let mut rows = Vec::with_capacity(scanned.len());
        for (ordinal, date) in scanned.into_iter().enumerate() {
            let oracle = OracleWhen::parse(&date.when).ok_or(OracleError::OracleWhen {
                fixture,
                ordinal,
                reason: if date.when.is_empty() {
                    WhenFailure::Missing
                } else {
                    WhenFailure::Malformed
                },
            })?;
            let candidate = NativeDateParser::new().parse(&date.text);
            rows.push(OracleRow {
                fixture,
                ordinal,
                span: date.span,
                scope: date.scope,
                input_sha256: sha256_raw(date.text.as_bytes()),
                when_sha256: sha256_raw(date.when.as_bytes()),
                oracle,
                candidate,
            });
        }
        Ok(rows)
    }

    /// Validates a crafted row list (corruption tests): ordinals must be
    /// exactly 0..n in order; spans bounded and strictly increasing.
    fn validate_crafted_rows(
        fixture: &'static str,
        rows: &[OracleRow],
        data_len: usize,
    ) -> Result<(), OracleError> {
        let mut last_end: Option<usize> = None;
        for (index, row) in rows.iter().enumerate() {
            if row.ordinal != index {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal: row.ordinal,
                    reason: RowFailure::DuplicateOrdinal,
                });
            }
            if row.span.1 > data_len
                || row.span.0 >= row.span.1
                || row.span.1 - row.span.0 > MAX_ELEMENT_SPAN
            {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal: row.ordinal,
                    reason: RowFailure::SpanOverflow,
                });
            }
            if let Some(prev_end) = last_end
                && row.span.0 <= prev_end
            {
                return Err(OracleError::RowInvariants {
                    fixture,
                    ordinal: row.ordinal,
                    reason: RowFailure::Reordered,
                });
            }
            last_end = Some(row.span.1);
        }
        Ok(())
    }

    fn check_denominator(spec: &FixtureSpec, rows: &[OracleRow]) -> Result<(), OracleError> {
        let (a, b, c) = spec.scope_split;
        let expected = a + b + c;
        if rows.len() != expected {
            return Err(OracleError::DenominatorDrift {
                fixture: spec.id,
                expected,
                actual: rows.len(),
            });
        }
        Ok(())
    }

    fn assert_same_n(
        fixture: &'static str,
        oracle_n: usize,
        candidate_n: usize,
    ) -> Result<(), OracleError> {
        if oracle_n != candidate_n {
            return Err(OracleError::SameNDrift {
                fixture,
                oracle_n,
                candidate_n,
            });
        }
        Ok(())
    }

    fn assert_scope_split(spec: &FixtureSpec, rows: &[OracleRow]) -> Result<(), OracleError> {
        let mut counts = [0usize; 3];
        for row in rows {
            counts[row.scope.index()] += 1;
        }
        let (a, b, c) = spec.scope_split;
        let expected = [a, b, c];
        if counts != expected {
            for index in 0..3 {
                if counts[index] != expected[index] {
                    return Err(OracleError::ScopeMix {
                        fixture: spec.id,
                        scope: [
                            "header_article_date",
                            "header_bibliography_date",
                            "bibliography_date",
                        ][index],
                        expected: expected[index],
                        actual: counts[index],
                    });
                }
            }
        }
        Ok(())
    }

    fn strict_parity_failures(rows: &[OracleRow]) -> Vec<usize> {
        rows.iter()
            .filter(|row| row.candidate.recognized() != Some(row.oracle.as_recognized()))
            .map(|row| row.ordinal)
            .collect()
    }

    /// Redacted deterministic digest: fixture ids, ordinals, spans, scope tags,
    /// SHA-256 hashes and closed wire outcomes only — never raw text.
    fn corpus_digest(per_fixture: &[(&'static str, Vec<OracleRow>)]) -> String {
        let mut hasher = Sha256::new();
        for (fixture, rows) in per_fixture {
            hasher.update(fixture.as_bytes());
            hasher.update([0xFF]);
            for row in rows {
                hasher.update((row.ordinal as u64).to_le_bytes());
                hasher.update((row.span.0 as u64).to_le_bytes());
                hasher.update((row.span.1 as u64).to_le_bytes());
                hasher.update(row.scope.tag().as_bytes());
                hasher.update([0x00]);
                hasher.update(row.input_sha256);
                hasher.update(row.when_sha256);
                hasher.update([0x01]);
                hasher.update(serde_json::to_vec(&row.candidate).expect("outcome must serialize"));
                hasher.update([0x02]);
            }
        }
        hex::encode(hasher.finalize())
    }

    struct Corpus {
        per_fixture: Vec<(&'static str, Vec<OracleRow>)>,
        date_elements_total: Vec<usize>,
        digest: String,
    }

    fn fixture_bytes(spec: &FixtureSpec) -> Result<Vec<u8>, OracleError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        fs::read(root.join(spec.rel)).map_err(|_| OracleError::ProvenanceIdentity {
            reason: "fixture_unreadable",
        })
    }

    fn verify_checksum(spec: &FixtureSpec, bytes: &[u8]) -> Result<(), OracleError> {
        if sha256_hex(bytes) != spec.sha256 {
            return Err(OracleError::ChecksumMismatch { fixture: spec.id });
        }
        Ok(())
    }

    fn load_provenance() -> Result<BTreeMap<String, (String, u64)>, OracleError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let raw = fs::read_to_string(root.join(PROVENANCE_REL)).map_err(|_| {
            OracleError::ProvenanceIdentity {
                reason: "provenance_unreadable",
            }
        })?;
        let doc: ProvenanceDoc =
            serde_yaml::from_str(&raw).map_err(|_| OracleError::ProvenanceIdentity {
                reason: "provenance_unparseable",
            })?;
        Ok(doc
            .fixtures
            .into_iter()
            .map(|f| (f.id, (f.sha256, f.bytes)))
            .collect())
    }

    fn verify_provenance_and_checksums() -> Result<Vec<Vec<u8>>, OracleError> {
        let provenance = load_provenance()?;
        let mut out = Vec::new();
        for spec in FIXTURES.iter() {
            let (sha, size) = provenance
                .get(spec.id)
                .ok_or(OracleError::ProvenanceIdentity {
                    reason: "fixture_missing_from_provenance",
                })?;
            if *sha != spec.sha256 || *size != spec.bytes {
                return Err(OracleError::ProvenanceIdentity {
                    reason: "provenance_identity_drift",
                });
            }
            let bytes = fixture_bytes(spec)?;
            if bytes.len() as u64 != spec.bytes {
                return Err(OracleError::ProvenanceIdentity {
                    reason: "byte_length_drift",
                });
            }
            verify_checksum(spec, &bytes)?;
            out.push(bytes);
        }
        Ok(out)
    }

    fn build_corpus() -> Result<Corpus, OracleError> {
        let bytes_all = verify_provenance_and_checksums()?;
        let mut per_fixture = Vec::new();
        let mut date_elements_total = Vec::new();
        for (spec, bytes) in FIXTURES.iter().zip(bytes_all.iter()) {
            let stats = scan_published(spec.id, bytes)?;
            let rows = build_rows(spec.id, stats.published, bytes.len())?;
            date_elements_total.push(stats.date_elements_total);
            per_fixture.push((spec.id, rows));
        }
        let digest = corpus_digest(&per_fixture);
        Ok(Corpus {
            per_fixture,
            date_elements_total,
            digest,
        })
    }

    #[test]
    fn oracle_corpus_is_checksum_bound_with_pinned_denominator_173() {
        let provenance = load_provenance().expect("provenance must parse");
        for spec in FIXTURES.iter() {
            let (sha, size) = provenance
                .get(spec.id)
                .expect("fixture must be recorded in provenance");
            assert_eq!(
                sha, spec.sha256,
                "provenance sha must match pinned identity"
            );
            assert_eq!(
                *size, spec.bytes,
                "provenance byte length must match pinned identity"
            );
        }
        let corpus =
            build_corpus().expect("pristine oracle corpus must build without typed failures");
        for ((spec, (fixture, rows)), date_total) in FIXTURES
            .iter()
            .zip(corpus.per_fixture.iter())
            .zip(corpus.date_elements_total.iter())
        {
            assert_eq!(*fixture, spec.id);
            check_denominator(spec, rows).expect("pinned denominator must hold");
            assert_scope_split(spec, rows).expect("pinned per-scope split must hold");
            assert_eq!(
                *date_total,
                rows.len(),
                "every <date> element must be type=published; a re-typed date must never be silently excluded"
            );
        }
        let total: usize = corpus.per_fixture.iter().map(|(_, rows)| rows.len()).sum();
        assert_eq!(
            total, EXPECTED_TOTAL,
            "corpus denominator must stay exactly 173"
        );
    }

    #[test]
    fn oracle_candidate_strict_parity_173_of_173() {
        let corpus = build_corpus().expect("pristine oracle corpus must build");
        let mut strict_by_scope = [0usize; 3];
        let mut candidate_n = 0usize;
        let mut oracle_n = 0usize;
        for (spec, (_, rows)) in FIXTURES.iter().zip(corpus.per_fixture.iter()) {
            oracle_n += rows.len();
            for row in rows {
                candidate_n += 1; // one candidate evaluation per oracle occurrence
                let expected = row.oracle.as_recognized();
                assert_eq!(
                    row.candidate.recognized(),
                    Some(expected),
                    "{:?}",
                    OracleError::ParityMismatch {
                        fixture: row.fixture,
                        ordinal: row.ordinal
                    }
                );
                strict_by_scope[row.scope.index()] += 1;
            }
            assert_same_n(spec.id, rows.len(), rows.len()).expect("same-n must hold per fixture");
        }
        assert_eq!(oracle_n, EXPECTED_TOTAL);
        assert_eq!(candidate_n, EXPECTED_TOTAL);
        assert_eq!(
            strict_by_scope,
            [3, 3, 167],
            "per-scope strict parity must not collapse header vs bibliography scopes"
        );
    }

    #[test]
    fn oracle_replay_digest_is_deterministic_and_redacted() {
        let first = build_corpus().expect("pristine oracle corpus must build");
        let second = build_corpus().expect("pristine oracle corpus must build");
        assert_eq!(
            first.digest, second.digest,
            "replay must be digest-identical"
        );
        assert_eq!(first.digest.len(), 64);
        assert!(first.digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn oracle_corruptions_turn_the_harness_red() {
        let hub = &FIXTURES[0];
        let bytes = fixture_bytes(hub).expect("fixture must be readable");
        verify_checksum(hub, &bytes).expect("pristine bytes must match provenance checksum");

        // (a) Checksum substitution is detected.
        let mut substituted = bytes.clone();
        let mid = bytes.len() / 2;
        substituted[mid] ^= 0x01;
        assert!(matches!(
            verify_checksum(hub, &substituted),
            Err(OracleError::ChecksumMismatch { .. })
        ));

        // (b) Missing occurrence: denominator and same-n drift turn red.
        let pristine_scan = scan_published(hub.id, &bytes).expect("pristine scan");
        let pristine_rows = build_rows(hub.id, pristine_scan.published.clone(), bytes.len())
            .expect("pristine rows");
        validate_crafted_rows(hub.id, &pristine_rows, bytes.len())
            .expect("pristine rows must validate");
        let victim = pristine_scan.published[7].span;
        let mut removed = bytes.clone();
        removed.drain(victim.0..victim.1);
        let rescanned =
            scan_published(hub.id, &removed).expect("removal of one element stays well-formed");
        let drifted_rows = build_rows(hub.id, rescanned.published, removed.len())
            .expect("drifted rows must still build");
        assert_eq!(drifted_rows.len(), pristine_rows.len() - 1);
        assert!(matches!(
            check_denominator(hub, &drifted_rows),
            Err(OracleError::DenominatorDrift { .. })
        ));
        assert!(matches!(
            assert_same_n(hub.id, drifted_rows.len(), drifted_rows.len() - 1),
            Err(OracleError::SameNDrift { .. })
        ));

        // (c) Malformed/missing oracle `when` is a typed failure, not a skip.
        let target = &pristine_rows[3];
        let open_end = find(&bytes, target.span.0, b">").expect("open tag must end");
        let tag_zone = bytes[target.span.0..open_end].to_vec();
        let wq = find(&tag_zone, 0, b"when=\"").expect("when attr must be present");
        let value_start = target.span.0 + wq + 6;
        let value_end = value_start
            + bytes[value_start..]
                .iter()
                .position(|b| *b == b'"')
                .expect("closing quote");
        let mut malformed = bytes.clone();
        malformed.splice(value_start..value_end, b"1999-99-99".iter().copied());
        let rescanned = scan_published(hub.id, &malformed).expect("scan must stay well-formed");
        assert!(matches!(
            build_rows(hub.id, rescanned.published, malformed.len()),
            Err(OracleError::OracleWhen {
                reason: WhenFailure::Malformed,
                ..
            })
        ));
        let mut missing = bytes.clone();
        missing.splice(value_start - 6..value_end + 1, std::iter::empty::<u8>());
        let rescanned = scan_published(hub.id, &missing).expect("scan must stay well-formed");
        assert!(matches!(
            build_rows(hub.id, rescanned.published, missing.len()),
            Err(OracleError::OracleWhen {
                reason: WhenFailure::Missing,
                ..
            })
        ));

        // (d) Crafted row-list violations are typed red.
        let mut duplicated = pristine_rows.clone();
        duplicated.insert(4, duplicated[3].clone());
        assert!(matches!(
            validate_crafted_rows(hub.id, &duplicated, bytes.len()),
            Err(OracleError::RowInvariants {
                reason: RowFailure::DuplicateOrdinal,
                ..
            })
        ));
        let mut swapped = pristine_rows.clone();
        let (span_a, span_b) = (swapped[3].span, swapped[4].span);
        swapped[3].span = span_b;
        swapped[4].span = span_a;
        assert!(matches!(
            validate_crafted_rows(hub.id, &swapped, bytes.len()),
            Err(OracleError::RowInvariants {
                reason: RowFailure::Reordered,
                ..
            })
        ));
        let mut overflow = pristine_rows.clone();
        overflow[3].span = (overflow[3].span.0, bytes.len() + 1);
        assert!(matches!(
            validate_crafted_rows(hub.id, &overflow, bytes.len()),
            Err(OracleError::RowInvariants {
                reason: RowFailure::SpanOverflow,
                ..
            })
        ));

        // (e) Scope collapse (header vs bibliography mixing) is visible.
        let mut mixed = pristine_rows.clone();
        let biblio = mixed
            .iter()
            .position(|row| row.scope == OracleScope::Bibliography)
            .expect("bibliography rows exist");
        mixed[biblio].scope = OracleScope::HeaderArticle;
        assert!(matches!(
            assert_scope_split(hub, &mixed),
            Err(OracleError::ScopeMix { .. })
        ));

        // (f) Parity regression is visible on a crafted row.
        let mut broken = pristine_rows.clone();
        broken[2].candidate = NativeDateOutcome::Empty;
        assert_eq!(strict_parity_failures(&broken), vec![2]);
        assert!(
            strict_parity_failures(&pristine_rows).is_empty(),
            "pristine corpus must have zero strict parity failures"
        );
    }
}
