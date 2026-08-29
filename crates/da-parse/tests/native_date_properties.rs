//! Native date property suite (M464-z84in2 S08/T02, ADR-051 Contour 2/4).
//!
//! Bounded proptest over arbitrary UTF-8: totality (no panic), stable output,
//! closed wire outcomes that never carry source text, recognized calendar
//! validity, precision exactly implied by present components, no component
//! invention, valid-prefix-with-garbage-suffix rejection, and bounded closed
//! handling of huge years, control characters, NUL, bidi marks, non-ASCII
//! digits and date ranges. Metamorphic checks are limited to the explicitly
//! sanctioned whitespace/separator equivalences. Offline, dependency-free.

use da_parse::{
    DatePrecision, NATIVE_DATE_INPUT_BYTE_CAP, NativeDateMalformed, NativeDateOutcome,
    NativeDateParser, NativeDateUnsupported, RecognizedDate,
};
use proptest::prelude::*;

fn parse(input: &str) -> NativeDateOutcome {
    NativeDateParser::new().parse(input)
}

/// Every lowercase token permitted on the outcome wire (closed vocabulary).
const WIRE_VOCABULARY: &[&str] = &[
    // Field names.
    "outcome",
    "date",
    "year",
    "month",
    "day",
    "precision",
    "reason",
    // Outcome variants.
    "recognized",
    "empty",
    "malformed",
    "unsupported",
    "ambiguous",
    "oversized",
    // Precision values ("year" already covered above).
    "year_month",
    "full_date",
    // Malformed reasons.
    "control_character",
    "non_ascii_script",
    "unexpected_character",
    "empty_component",
    "component_out_of_range",
    "calendar_invalid",
    "trailing_garbage",
    "grammar_mismatch",
    // Unsupported reasons.
    "date_range",
    "decade",
    "season",
    "quarter",
    "ordinal_day_suffix",
    "year_too_short",
    "year_too_long",
    // Ambiguous reasons.
    "numeric_component_order",
];

/// The wire carries only closed-vocabulary tokens. Because arbitrary input may
/// contain arbitrary words, this simultaneously proves that no source text can
/// ever leak into a serialized outcome (redaction by construction).
fn wire_is_closed(json: &str) -> bool {
    let mut token = String::new();
    for ch in json.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_lowercase() || ch == '_' {
            token.push(ch);
        } else if !token.is_empty() {
            if !WIRE_VOCABULARY.contains(&token.as_str()) {
                return false;
            }
            token.clear();
        }
    }
    true
}

/// Arbitrary UTF-8 text, bounded so worst-case work stays small.
fn arb_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..=300).prop_map(|chars| chars.into_iter().collect())
}

/// Inputs guaranteed to exceed the documented byte cap.
fn arb_oversize() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), NATIVE_DATE_INPUT_BYTE_CAP + 1..=420)
        .prop_map(|chars| chars.into_iter().collect())
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// ASCII valid date display forms, all far below the byte cap.
fn arb_valid_form() -> impl Strategy<Value = String> {
    (1000i32..=9999, 1u8..=12, 1u8..=28, 0usize..=3).prop_map(|(year, month, day, form)| match form
    {
        0 => format!("{year}"),
        1 => format!("{year}-{month:02}"),
        2 => format!("{year}-{month:02}-{day:02}"),
        _ => format!("{day} {} {year}", MONTHS[(month - 1) as usize]),
    })
}

fn garbage_suffix() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "x", "xx", "zzz", "?", "!", "#", "$", "%", "(", ")", "*", "@", "+", "=", "~", "|", "\\",
        "<", ">", "[", "]", "{", "}", ":", ";", "'", "\"", "q", "s", "Q3", "\u{0001}", "\u{202E}",
        "\u{0661}", "…", "€",
    ])
}

fn whitespace_run() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![" ", "  ", "\t", "\n", "\u{00A0}", "\u{2003}"])
}

fn has_month_word(input_lower: &str) -> bool {
    input_lower
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| (3..=9).contains(&word.len()))
        .any(|word| {
            MONTHS
                .iter()
                .any(|name| name.to_lowercase().starts_with(word))
        })
}

fn decimal_runs(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_ascii_digit())
        .filter(|run| !run.is_empty())
        .map(str::to_owned)
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Totality: any UTF-8 input yields a stable, closed, bounded, redacting
    /// outcome that roundtrips byte-stably. Never panics.
    #[test]
    fn total_closed_stable_and_redacting(input in arb_text()) {
        let first = parse(&input);
        let second = parse(&input);
        prop_assert_eq!(first, second, "repeated parses must be identical");
        let json = serde_json::to_string(&first).expect("outcome must serialize");
        prop_assert!(json.len() <= 256, "outcome wire must stay bounded: {json}");
        prop_assert!(wire_is_closed(&json), "wire vocabulary must be closed: {json}");
        let restored: NativeDateOutcome =
            serde_json::from_str(&json).expect("closed outcomes must deserialize");
        prop_assert_eq!(first, restored, "serde roundtrip must be byte-stable");
    }

    /// Recognized outcomes: calendar-valid, precision exactly implied by the
    /// stored components, and no invented component — every stored component
    /// must be derivable from the input (month word or digit runs).
    #[test]
    fn recognized_is_calendar_valid_with_exact_precision(input in arb_text()) {
        if let Some(date) = parse(&input).recognized() {
            prop_assert!((1000..=9999).contains(&date.year));
            prop_assert_eq!(RecognizedDate::new(date.year, date.month, date.day), Some(date));
            prop_assert!(date.precision_matches_components());
            match date.precision {
                DatePrecision::Year => {
                    prop_assert!(date.month.is_none() && date.day.is_none());
                }
                DatePrecision::YearMonth => {
                    prop_assert!(date.month.is_some() && date.day.is_none());
                }
                DatePrecision::FullDate => {
                    prop_assert!(date.month.is_some() && date.day.is_some());
                }
            }
            let lower = input.to_lowercase();
            let runs = decimal_runs(&input);
            let month_ok = match date.month {
                None => true,
                Some(month) => {
                    has_month_word(&lower)
                        || runs.iter().any(|run| {
                            run == &month.to_string() || run == &format!("{month:02}")
                        })
                }
            };
            prop_assert!(month_ok, "recognized month {:?} must be present in input", date.month);
            let day_ok = match date.day {
                None => true,
                Some(day) => runs
                    .iter()
                    .any(|run| run == &day.to_string() || run == &format!("{day:02}")),
            };
            prop_assert!(day_ok, "recognized day {:?} must be present in input", date.day);
            prop_assert!(
                runs.iter().any(|run| run == &date.year.to_string()),
                "recognized year must be present as a digit run in input"
            );
        }
    }

    /// A valid date prefix followed by non-whitespace garbage is never
    /// accepted as a date: precision is never silently weakened, and the
    /// outcome stays inside the closed vocabulary.
    #[test]
    fn valid_prefix_with_garbage_suffix_is_never_recognized(
        form in arb_valid_form(),
        garbage in garbage_suffix(),
    ) {
        let input = format!("{form}{garbage}");
        let outcome = parse(&input);
        prop_assert!(outcome.recognized().is_none(), "garbage suffix must not parse: {input:?}");
        prop_assert!(
            !matches!(outcome, NativeDateOutcome::Empty),
            "non-empty garbage-suffixed input must not be Empty"
        );
        prop_assert!(wire_is_closed(&serde_json::to_string(&outcome).expect("serializable")));
    }

    /// All-digit inputs stay inside the closed year-domain outcomes.
    #[test]
    fn all_digit_inputs_stay_in_closed_year_domains(
        digits in prop::collection::vec(0u8..=9, 0..=12),
    ) {
        let input: String = digits.iter().map(|d| (b'0' + d) as char).collect();
        let outcome = parse(&input);
        let value: u32 = input.parse().unwrap_or(u32::MAX);
        match digits.len() {
            0 => prop_assert_eq!(outcome, NativeDateOutcome::Empty),
            4 if (1000..=9999).contains(&value) => {
                prop_assert_eq!(outcome.recognized().map(|d| d.year), Some(value as i32));
            }
            1..=4 => {
                let year_too_short = matches!(
                    outcome,
                    NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::YearTooShort }
                );
                prop_assert!(year_too_short);
            }
            _ => {
                let year_too_long = matches!(
                    outcome,
                    NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::YearTooLong }
                );
                prop_assert!(year_too_long);
            }
        }
    }

    /// Inputs above the documented byte cap are Oversized and never scanned,
    /// regardless of content.
    #[test]
    fn oversized_inputs_are_never_scanned(input in arb_oversize()) {
        prop_assert!(input.len() > NATIVE_DATE_INPUT_BYTE_CAP);
        prop_assert_eq!(parse(&input), NativeDateOutcome::Oversized);
    }

    /// Control characters (including NUL), bidi marks and non-ASCII digits
    /// are classified into exactly the closed malformed reasons and never
    /// panic or hang, wherever an ASCII date-ish prefix sits in front.
    #[test]
    fn injected_control_and_non_ascii_stay_closed(
        base in prop::collection::vec(0x20u8..=0x7E, 0..=60)
            .prop_map(|bytes| String::from_utf8(bytes).expect("ascii bytes")),
        kind in 0usize..=2,
    ) {
        let injected = match kind {
            0 => '\u{0000}',
            1 => '\u{202E}',
            _ => '\u{0661}',
        };
        let input = format!("{base}{injected}");
        let outcome = parse(&input);
        match kind {
            0 => {
                let control = matches!(
                    outcome,
                    NativeDateOutcome::Malformed { reason: NativeDateMalformed::ControlCharacter }
                );
                prop_assert!(control);
            }
            _ => {
                let non_ascii = matches!(
                    outcome,
                    NativeDateOutcome::Malformed { reason: NativeDateMalformed::NonAsciiScript }
                );
                prop_assert!(non_ascii);
            }
        }
        prop_assert_eq!(outcome, parse(&input), "classification must be stable");
    }

    /// Sanctioned metamorphs only: `-` / `/` / `.` separator equivalence for
    /// ISO forms and Unicode-whitespace-run equivalence. No other transform
    /// is claimed equivalent.
    #[test]
    fn sanctioned_separator_and_whitespace_equivalence(
        year in 1000i32..=9999,
        month in 1u8..=12,
        ws in whitespace_run(),
        ws2 in whitespace_run(),
    ) {
        let ymd_dash = format!("{year}-{month:02}-06");
        let ym_dash = format!("{year}-{month:02}");
        prop_assert_eq!(parse(&ymd_dash), parse(&format!("{year}/{month:02}/06")));
        prop_assert_eq!(parse(&ymd_dash), parse(&format!("{year}.{month:02}.06")));
        prop_assert_eq!(parse(&ym_dash), parse(&format!("{year}/{month:02}")));
        prop_assert_eq!(parse(&ym_dash), parse(&format!("{year}.{month:02}")));
        prop_assert_eq!(parse("17 March 2021".replace(' ', ws).as_str()), parse("17 March 2021"));
        prop_assert_eq!(parse(&format!("{ws}{ymd_dash}{ws2}")), parse(&ymd_dash));
    }

    /// Date ranges, decades, seasons, quarters and ordinal day suffixes stay
    /// bounded and are never normalized into invented single instants.
    #[test]
    fn unsupported_scholarly_semantics_stay_bounded(
        year in 1000i32..=9700,
        delta in 1i32..=299,
        month in 1u8..=12,
    ) {
        let other = year + delta;
        for separator in ["-", "\u{2013}", "\u{2014}", "/"] {
            let outcome = parse(&format!("{year}{separator}{other}"));
            let __ok5 = matches!(
                outcome,
                NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::DateRange }
            );
        prop_assert!(__ok5);
        }
        let __ok6 = matches!(
            parse(&format!("{year}s")),
            NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::Decade }
        );
        prop_assert!(__ok6);
        let __ok7 = matches!(
            parse(&format!("Spring {year}")),
            NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::Season }
        );
        prop_assert!(__ok7);
        let __ok8 = matches!(
            parse(&format!("Q3 {year}")),
            NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::Quarter }
        );
        prop_assert!(__ok8);
        let __ok9 = matches!(
            parse(&format!("15th {} {year}", MONTHS[(month - 1) as usize])),
            NativeDateOutcome::Unsupported { reason: NativeDateUnsupported::OrdinalDaySuffix }
        );
        prop_assert!(__ok9);
    }
}
