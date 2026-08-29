//! Native scholarly date recognition (M464-z84in2 S08, shadow candidate).
//!
//! Pure, deterministic, dependency-free recognizer/normalizer for the
//! scholarly date display forms observed in the checksum-bound Java GROBID
//! TEI oracle corpus (the three preserved hub/rlm/reca fixtures: 173 date
//! occurrences). S08 keeps this component strictly outside the parse
//! composition: it never touches `TeiParseIr`, `ParsedArticle`,
//! `ParserPort`, header composition, caches, tracing, clock, environment,
//! filesystem or network. The outcome is a closed, serializable, redacting
//! model that S08 T02/T03 consume only as shadow evidence.
//!
//! Policy (documented here, asserted by `tests/native_date_contract.rs`):
//!
//! - **Input cap.** Inputs longer than [`NATIVE_DATE_INPUT_BYTE_CAP`] bytes
//!   are [`NativeDateOutcome::Oversized`] and are never scanned; per-call
//!   work is therefore bounded by a small constant (linear scan, no regex,
//!   no backtracking, no unbounded allocation).
//! - **Character policy.** The grammar is ASCII-only (digits, ASCII letters,
//!   `- / . ,`); any Unicode whitespace is a token separator (GROBID display
//!   text carries NBSP); any control character, non-ASCII non-whitespace
//!   character (non-Latin digits, bidi marks, mixed script) or other
//!   out-of-grammar ASCII character is a closed [`NativeDateMalformed`]
//!   outcome — never skipped, never guessed.
//! - **Closed shapes.** `YYYY`; `YYYY-MM` and `YYYY-MM-DD` with `-`, `/` or
//!   `.` as equivalent sanctioned separators (numeric ISO precision forms,
//!   month/day must be zero-padded two-digit); `Month YYYY`; `Month D[,] YYYY`;
//!   `D Month YYYY` (observed month-name display forms: full English names
//!   and standard abbreviations, case-insensitive).
//! - **Calendar validation.** Proleptic Gregorian, full leap rule, checked
//!   with independent pure arithmetic — no `chrono` dependency.
//! - **No invention.** Absent components stay `None`; precision is exactly
//!   [`DatePrecision::Year`], [`DatePrecision::YearMonth`] or
//!   [`DatePrecision::FullDate`] per explicitly present components. Two-digit
//!   or 0-padded sub-1000 years, 5+-digit years, ranges, decades, seasons,
//!   quarters, ordinal-suffixed days and non-ISO numeric orders (`D/M/YYYY`,
//!   `M/D/YYYY`) are typed Unsupported/Ambiguous outcomes — century, locale,
//!   timezone and ordering are never guessed.
//! - **Selection.** The parser tokenizes linearly and takes the leftmost
//!   match whose closed shape consumes the entire remaining input (ties at
//!   the same start prefer the most specific/longest shape); a full-input
//!   reading that fails component or calendar validation is reported with
//!   its closed reason, never silently re-read as a weaker match. This
//!   reproduces
//!   the oracle corpus exactly, including display forms with prefix context
//!   noise (`"2025. July 16, 2025"`, `"Aime-2025, February 2025"`), while a
//!   valid date prefix followed by garbage is rejected
//!   ([`NativeDateMalformed::TrailingGarbage`]) and a glued token cluster
//!   (`"February2025"`) is never split mid-cluster.
//! - **Redaction.** Outcomes carry closed reason codes only; no source text,
//!   offsets or display strings ever leave the parser.
//!
//! Wire stability: the serde representation (internally tagged `outcome`,
//! snake_case variants, fixed field names/order) is a public contract pinned
//! byte-for-byte by the contract tests; [`NATIVE_DATE_OUTCOME_SCHEMA_VERSION`]
//! names it. Repeated calls on the same input are deterministic and serde
//! roundtrips are byte-stable.

use serde::{Deserialize, Serialize};

/// Shadow-candidate parser identity (parse-compat evidence consumes this).
pub const NATIVE_DATE_PARSER_ID: &str = "da-parse.native_date";

/// Shadow-candidate implementation version.
pub const NATIVE_DATE_PARSER_VERSION: &str = "1.0.0";

/// Stable wire schema tag for the serialized [`NativeDateOutcome`] model.
pub const NATIVE_DATE_OUTCOME_SCHEMA_VERSION: &str = "da-parse.native-date.outcome/v1";

/// Documented input byte cap: longer inputs are
/// [`NativeDateOutcome::Oversized`] and are never scanned, so worst-case
/// per-call work stays a small bounded constant.
pub const NATIVE_DATE_INPUT_BYTE_CAP: usize = 256;

/// Precision implied exactly by the explicitly recognized components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePrecision {
    /// Only an explicit year was recognized.
    Year,
    /// Explicit year and month; no day.
    YearMonth,
    /// Explicit year, month and calendar-valid day.
    FullDate,
}

/// An explicitly recognized scholarly date.
///
/// Only components declared by the input are stored; nothing is synthesized
/// and no century/timezone/locale inference happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognizedDate {
    /// Explicit four-digit year (1000..=9999); never inferred from a
    /// two-digit form or a century marker.
    pub year: i32,
    /// Explicit month 1..=12, present iff declared by the input.
    pub month: Option<u8>,
    /// Explicit day 1..=days-in-month, present iff declared by the input.
    pub day: Option<u8>,
    /// Precision exactly implied by the present components.
    pub precision: DatePrecision,
}

impl RecognizedDate {
    /// Closed constructor: returns `None` unless the combination is fully
    /// valid (year domain, month domain, day-without-month, calendar-valid
    /// day). Never panics; never invents components.
    pub fn new(year: i32, month: Option<u8>, day: Option<u8>) -> Option<Self> {
        if !(1000..=9999).contains(&year) {
            return None;
        }
        if let Some(month) = month {
            if !(1..=12).contains(&month) {
                return None;
            }
        } else if day.is_some() {
            // A day without a month would be invented precision.
            return None;
        }
        if let Some(day) = day {
            let month = month?;
            if day == 0 || day > Self::days_in_month(year, month)? {
                return None;
            }
        }
        let precision = match (month, day) {
            (None, _) => DatePrecision::Year,
            (Some(_), None) => DatePrecision::YearMonth,
            (Some(_), Some(_)) => DatePrecision::FullDate,
        };
        Some(Self {
            year,
            month,
            day,
            precision,
        })
    }

    /// Proleptic-Gregorian leap rule (divisible by 4, except century years
    /// not divisible by 400). Independent calendar validation — no `chrono`.
    pub fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// Month length in days; `None` for months outside 1..=12.
    pub fn days_in_month(year: i32, month: u8) -> Option<u8> {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
            4 | 6 | 9 | 11 => Some(30),
            2 if Self::is_leap_year(year) => Some(29),
            2 => Some(28),
            _ => None,
        }
    }

    /// Invariant helper: `true` iff `precision` is exactly the precision
    /// implied by the stored components (contract/property suites pin this).
    pub fn precision_matches_components(&self) -> bool {
        let derived = match (self.month, self.day) {
            (None, None) => DatePrecision::Year,
            (Some(_), None) => DatePrecision::YearMonth,
            (Some(_), Some(_)) => DatePrecision::FullDate,
            (None, Some(_)) => return false,
        };
        self.precision == derived
    }
}

/// Closed malformed vocabulary: the input tried to be a date but violates
/// the byte-level grammar or value domains. Carries no source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeDateMalformed {
    /// Control character (including NUL) anywhere in the input.
    ControlCharacter,
    /// Non-ASCII non-whitespace character where the closed grammar requires
    /// ASCII (non-Latin digits, bidi marks, mixed script, CJK, ...).
    NonAsciiScript,
    /// ASCII character outside the closed grammar (`:`, `?`, `(`, ...).
    UnexpectedCharacter,
    /// Empty numeric component (`2025-`, `-2025`, `2025--05`, `/2025/05`).
    EmptyComponent,
    /// Numeric component outside its value domain (month 13, day 32, ...).
    ComponentOutOfRange,
    /// Calendar-invalid combination (Feb 30, non-leap Feb 29, Apr 31, ...).
    CalendarInvalid,
    /// A valid date-shaped prefix is followed by garbage; the input is not
    /// accepted as a date (no full-input reading exists).
    TrailingGarbage,
    /// Tokens exist but no closed shape explains them.
    GrammarMismatch,
}

/// Closed unsupported vocabulary: recognizable scholarly date semantics that
/// this cut deliberately does not normalize, because doing so would invent
/// components or precision below/above what the input declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeDateUnsupported {
    /// A range of dates (`2018-2020`, en/em dash, `2018/2022`): no single
    /// instant is declared.
    DateRange,
    /// Decade form (`1990s`): a year would have to be invented.
    Decade,
    /// Season form (`Spring 2019`): a month would have to be invented.
    Season,
    /// Quarter form (`Q3 2019`, `... quarter ...`): a month would have to be
    /// invented.
    Quarter,
    /// Ordinal day suffix (`15th January 2019`): display decoration this cut
    /// does not normalize.
    OrdinalDaySuffix,
    /// Explicit year shorter than four digits (including 0-padded sub-1000
    /// forms): guessing the century is forbidden.
    YearTooShort,
    /// Explicit year longer than four digits (bounded huge-year input).
    YearTooLong,
}

/// Closed ambiguous vocabulary: multiple calendar readings exist under the
/// policy and none is preferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeDateAmbiguous {
    /// Non-ISO numeric order (`03/04/2021`, `15-01-2019`, `3.4.2021`):
    /// day-first vs month-first depends on locale; ordering is never guessed
    /// in this cut, even when one reading is eliminable.
    NumericComponentOrder,
}

/// Closed outcome of one parse call. Deterministic, `Copy`, and serializable
/// with stable wire tags; carries no source text (redacting by construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeDateOutcome {
    /// A date was recognized with exact declared precision.
    Recognized {
        /// The explicitly recognized components.
        date: RecognizedDate,
    },
    /// The input is empty or whitespace-only.
    Empty,
    /// The input violates the closed grammar or value domains.
    Malformed {
        /// Closed redacting reason.
        reason: NativeDateMalformed,
    },
    /// Recognizable scholarly date semantics outside this cut's policy.
    Unsupported {
        /// Closed redacting reason.
        reason: NativeDateUnsupported,
    },
    /// Multiple calendar readings exist; none is preferred.
    Ambiguous {
        /// Closed redacting reason.
        reason: NativeDateAmbiguous,
    },
    /// The input exceeds [`NATIVE_DATE_INPUT_BYTE_CAP`] and was not scanned.
    Oversized,
}

impl NativeDateOutcome {
    /// The recognized date, iff this outcome is [`NativeDateOutcome::Recognized`].
    pub fn recognized(&self) -> Option<RecognizedDate> {
        match self {
            Self::Recognized { date } => Some(*date),
            _ => None,
        }
    }
}

/// Stateless pure parser. Holds no configuration, no caches, no I/O.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDateParser;

impl NativeDateParser {
    /// Creates the parser (constant; the parser is stateless).
    pub const fn new() -> Self {
        Self
    }

    /// Parses one scholarly date display string into a closed outcome.
    ///
    /// Purity contract: no filesystem, environment, clock, network, cache,
    /// graph or tracing access; repeated calls on the same input return
    /// identical values.
    pub fn parse(&self, input: &str) -> NativeDateOutcome {
        parse(input)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SepKind {
    Dash,
    Slash,
    Dot,
    Comma,
    RangeDash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordKind {
    Season,
    QuarterWord,
    /// The literal letter "q"; a quarter index must follow as a glued
    /// one-digit number (`Q3`).
    QuarterLetter,
    OrdinalSuffix,
    DecadeMarker,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Num { value: u32, digits: u8, glued: bool },
    Month { month: u8, glued: bool },
    Word { kind: WordKind, glued: bool },
    Sep { kind: SepKind },
}

impl Tok {
    fn glued(&self) -> bool {
        match self {
            Tok::Num { glued, .. } | Tok::Month { glued, .. } | Tok::Word { glued, .. } => *glued,
            Tok::Sep { .. } => false,
        }
    }

    fn num(&self) -> Option<(u32, u8)> {
        match self {
            Tok::Num { value, digits, .. } => Some((*value, *digits)),
            _ => None,
        }
    }

    /// `-`, `/`, `.` are equivalent sanctioned numeric separators.
    fn is_year_sep(&self) -> bool {
        matches!(
            self,
            Tok::Sep {
                kind: SepKind::Dash | SepKind::Slash | SepKind::Dot
            }
        )
    }

    /// Separators whose duplication or edge position means an empty
    /// component (RangeDash is a range, handled earlier).
    fn is_component_sep(&self) -> bool {
        matches!(
            self,
            Tok::Sep {
                kind: SepKind::Dash | SepKind::Slash | SepKind::Dot | SepKind::Comma
            }
        )
    }
}

const MONTH_NAMES: &[(&str, u8)] = &[
    ("january", 1),
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
];

fn month_from_name(word: &str) -> Option<u8> {
    MONTH_NAMES
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, month)| *month)
}

fn is_season_word(word: &str) -> bool {
    matches!(word, "spring" | "summer" | "autumn" | "fall" | "winter")
}

/// Linear tokenizer: one pass over chars, no regex, no backtracking. Runs of
/// ASCII digits/letters become single tokens; `glued` marks tokens that began
/// directly adjacent to the previous token with no separator character
/// between (`February2025`, `15x`, `1990s`, `Q3`).
fn tokenize(input: &str) -> Result<Vec<Tok>, NativeDateMalformed> {
    let mut toks = Vec::new();
    let mut chars = input.chars().peekable();
    let mut prev_token = false;
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            prev_token = false;
            continue;
        }
        let glued = prev_token;
        if c.is_ascii_digit() {
            let mut digits: u32 = 1;
            let mut value: u32 = u32::from(c as u8 - b'0');
            while let Some(&d) = chars.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                chars.next();
                digits = digits.saturating_add(1);
                if digits <= 9 {
                    value = value
                        .saturating_mul(10)
                        .saturating_add(u32::from(d as u8 - b'0'));
                }
            }
            toks.push(Tok::Num {
                value,
                digits: digits.min(u32::from(u8::MAX)) as u8,
                glued,
            });
            prev_token = true;
        } else if c.is_ascii_alphabetic() {
            let mut word = String::with_capacity(16);
            word.push(c.to_ascii_lowercase());
            while let Some(&w) = chars.peek() {
                if !w.is_ascii_alphabetic() {
                    break;
                }
                chars.next();
                word.push(w.to_ascii_lowercase());
            }
            if let Some(month) = month_from_name(&word) {
                toks.push(Tok::Month { month, glued });
            } else {
                let kind = if is_season_word(&word) {
                    WordKind::Season
                } else if word == "quarter" {
                    WordKind::QuarterWord
                } else if word == "q" {
                    WordKind::QuarterLetter
                } else if matches!(word.as_str(), "st" | "nd" | "rd" | "th") {
                    WordKind::OrdinalSuffix
                } else if word == "s" {
                    WordKind::DecadeMarker
                } else {
                    WordKind::Other
                };
                toks.push(Tok::Word { kind, glued });
            }
            prev_token = true;
        } else {
            prev_token = false;
            let kind = match c {
                '-' => SepKind::Dash,
                '/' => SepKind::Slash,
                '.' => SepKind::Dot,
                ',' => SepKind::Comma,
                '\u{2013}' | '\u{2014}' => SepKind::RangeDash,
                _ if c.is_control() => return Err(NativeDateMalformed::ControlCharacter),
                _ if !c.is_ascii() => return Err(NativeDateMalformed::NonAsciiScript),
                _ => return Err(NativeDateMalformed::UnexpectedCharacter),
            };
            toks.push(Tok::Sep { kind });
        }
    }
    Ok(toks)
}

/// Failure of one shape attempt: the shape matched positionally but a value
/// is outside its domain (or the year outside the four-digit policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptInvalid {
    Malformed(NativeDateMalformed),
    Unsupported(NativeDateUnsupported),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attempt {
    start: usize,
    end: usize,
    result: Result<RecognizedDate, AttemptInvalid>,
}

fn year_of(value: u32) -> Result<i32, AttemptInvalid> {
    match i32::try_from(value) {
        Ok(year) if (1000..=9999).contains(&year) => Ok(year),
        // Shape matching enforces exactly four digits; reaching here with a
        // rejected value means a 0-padded sub-1000 year.
        _ => Err(AttemptInvalid::Unsupported(
            NativeDateUnsupported::YearTooShort,
        )),
    }
}

fn validated(
    new: Option<RecognizedDate>,
    calendar_fault: bool,
) -> Result<RecognizedDate, AttemptInvalid> {
    match new {
        Some(date) => Ok(date),
        None if calendar_fault => Err(AttemptInvalid::Malformed(
            NativeDateMalformed::CalendarInvalid,
        )),
        None => Err(AttemptInvalid::Malformed(
            NativeDateMalformed::GrammarMismatch,
        )),
    }
}

fn validate_year_only(value: u32) -> Result<RecognizedDate, AttemptInvalid> {
    let year = year_of(value)?;
    validated(RecognizedDate::new(year, None, None), false)
}

fn validate_year_month(value: u32, month: u32) -> Result<RecognizedDate, AttemptInvalid> {
    let year = year_of(value)?;
    if !(1..=12).contains(&month) {
        return Err(AttemptInvalid::Malformed(
            NativeDateMalformed::ComponentOutOfRange,
        ));
    }
    let month = month as u8;
    validated(RecognizedDate::new(year, Some(month), None), false)
}

/// Validates month/day domains first (out-of-range), then the calendar
/// (invalid dates like Feb 30). `month` arrives from a `Month` token and is
/// always 1..=12.
fn validate_year_month_day(
    year_value: u32,
    month: u32,
    day: u32,
) -> Result<RecognizedDate, AttemptInvalid> {
    let year = year_of(year_value)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(AttemptInvalid::Malformed(
            NativeDateMalformed::ComponentOutOfRange,
        ));
    }
    let (month, day) = (month as u8, day as u8);
    let calendar_fault = day > RecognizedDate::days_in_month(year, month).unwrap_or(0);
    validated(
        RecognizedDate::new(year, Some(month), Some(day)),
        calendar_fault,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    IsoYmd,
    MonthDayYear,
    DayMonthYear,
    IsoYm,
    MonthYear,
    Year,
}

/// Tries one shape at `start`; `Some((end, result))` iff the tokens match the
/// shape positionally. Most-specific shapes are tried first by the caller
/// order in [`shape_at`].
fn try_shape(
    toks: &[Tok],
    start: usize,
    shape: Shape,
) -> Option<(usize, Result<RecognizedDate, AttemptInvalid>)> {
    match shape {
        Shape::Year => {
            let (value, digits) = toks.get(start)?.num()?;
            (digits == 4).then(|| (start + 1, validate_year_only(value)))
        }
        Shape::IsoYm => {
            let (value, digits) = toks.get(start)?.num()?;
            if digits != 4 || !toks.get(start + 1)?.is_year_sep() {
                return None;
            }
            let (month, month_digits) = toks.get(start + 2)?.num()?;
            if month_digits != 2 {
                return None;
            }
            Some((start + 3, validate_year_month(value, month)))
        }
        Shape::IsoYmd => {
            let (value, digits) = toks.get(start)?.num()?;
            if digits != 4 || !toks.get(start + 1)?.is_year_sep() {
                return None;
            }
            let (month, month_digits) = toks.get(start + 2)?.num()?;
            if month_digits != 2 || !toks.get(start + 3)?.is_year_sep() {
                return None;
            }
            let (day, day_digits) = toks.get(start + 4)?.num()?;
            if day_digits != 2 {
                return None;
            }
            Some((start + 5, validate_year_month_day(value, month, day)))
        }
        Shape::MonthYear => {
            let month = match toks.get(start)? {
                Tok::Month { month, .. } => *month,
                _ => return None,
            };
            let year_tok = toks.get(start + 1)?;
            let (value, digits) = year_tok.num()?;
            if digits != 4 || year_tok.glued() {
                return None;
            }
            Some((start + 2, validate_year_month(value, u32::from(month))))
        }
        Shape::MonthDayYear => {
            let month = match toks.get(start)? {
                Tok::Month { month, .. } => *month,
                _ => return None,
            };
            let day_tok = toks.get(start + 1)?;
            let (day, day_digits) = day_tok.num()?;
            if day_digits > 2 || day_tok.glued() {
                return None;
            }
            let mut cursor = start + 2;
            if matches!(
                toks.get(cursor),
                Some(Tok::Sep {
                    kind: SepKind::Comma
                })
            ) {
                cursor += 1;
            }
            let year_tok = toks.get(cursor)?;
            let (value, digits) = year_tok.num()?;
            if digits != 4 || year_tok.glued() {
                return None;
            }
            Some((
                cursor + 1,
                validate_year_month_day(value, u32::from(month), day),
            ))
        }
        Shape::DayMonthYear => {
            let (day, day_digits) = toks.get(start)?.num()?;
            if day_digits > 2 {
                return None;
            }
            let month = match toks.get(start + 1)? {
                Tok::Month { month, glued } if !*glued => *month,
                _ => return None,
            };
            let year_tok = toks.get(start + 2)?;
            let (value, digits) = year_tok.num()?;
            if digits != 4 || year_tok.glued() {
                return None;
            }
            Some((
                start + 3,
                validate_year_month_day(value, u32::from(month), day),
            ))
        }
    }
}

/// Tries all closed shapes at `start` in most-specific-first order and
/// returns the first positional match.
fn shape_at(toks: &[Tok], start: usize) -> Option<Attempt> {
    for shape in [
        Shape::IsoYmd,
        Shape::MonthDayYear,
        Shape::DayMonthYear,
        Shape::IsoYm,
        Shape::MonthYear,
        Shape::Year,
    ] {
        if let Some((end, result)) = try_shape(toks, start, shape) {
            return Some(Attempt { start, end, result });
        }
    }
    None
}

fn unsupported_outcome(reason: NativeDateUnsupported) -> NativeDateOutcome {
    NativeDateOutcome::Unsupported { reason }
}

/// Structural detectors that outrank shape matching, in documented priority
/// order. Each fires only on whole-input structure, so legitimate corpus
/// forms (which contain none of these patterns) are never misclassified.
fn structural_outcome(toks: &[Tok]) -> Option<NativeDateOutcome> {
    // 1. DateRange: explicit en/em dash, or a dash/slash directly between
    //    two four-digit numbers.
    if toks.iter().any(|t| {
        matches!(
            t,
            Tok::Sep {
                kind: SepKind::RangeDash
            }
        )
    }) {
        return Some(unsupported_outcome(NativeDateUnsupported::DateRange));
    }
    for w in toks.windows(3) {
        if let (Some((_, left)), Some((_, right))) = (w[0].num(), w[2].num())
            && left == 4
            && right == 4
            && let Tok::Sep { kind } = w[1]
            && matches!(kind, SepKind::Dash | SepKind::Slash)
        {
            return Some(unsupported_outcome(NativeDateUnsupported::DateRange));
        }
    }
    // 2. Decade: four-digit year glued to "s" (1990s).
    for w in toks.windows(2) {
        if let Some((_, digits)) = w[0].num()
            && digits == 4
            && matches!(
                w[1],
                Tok::Word {
                    kind: WordKind::DecadeMarker,
                    glued: true
                }
            )
        {
            return Some(unsupported_outcome(NativeDateUnsupported::Decade));
        }
    }
    // 3. Quarter: glued "Q" + index 1..=4, or the word "quarter".
    for w in toks.windows(2) {
        if matches!(
            w[0],
            Tok::Word {
                kind: WordKind::QuarterLetter,
                ..
            }
        ) && let Tok::Num {
            value,
            digits: 1,
            glued: true,
        } = w[1]
            && (1..=4).contains(&value)
        {
            return Some(unsupported_outcome(NativeDateUnsupported::Quarter));
        }
    }
    if toks.iter().any(|t| {
        matches!(
            t,
            Tok::Word {
                kind: WordKind::QuarterWord,
                ..
            }
        )
    }) {
        return Some(unsupported_outcome(NativeDateUnsupported::Quarter));
    }
    // 4. Season words.
    if toks.iter().any(|t| {
        matches!(
            t,
            Tok::Word {
                kind: WordKind::Season,
                ..
            }
        )
    }) {
        return Some(unsupported_outcome(NativeDateUnsupported::Season));
    }
    // 5. Ordinal day suffix: 1-2 digit number glued to st/nd/rd/th.
    for w in toks.windows(2) {
        if let Tok::Num { digits, .. } = w[0]
            && (1..=2).contains(&digits)
            && matches!(
                w[1],
                Tok::Word {
                    kind: WordKind::OrdinalSuffix,
                    glued: true
                }
            )
        {
            return Some(unsupported_outcome(NativeDateUnsupported::OrdinalDaySuffix));
        }
    }
    // 6. Empty component: separator at an edge or duplicated.
    for (i, tok) in toks.iter().enumerate() {
        if tok.is_component_sep() {
            let at_edge = i == 0 || i + 1 == toks.len();
            let adjacent = (i > 0 && toks[i - 1].is_component_sep())
                || toks.get(i + 1).is_some_and(|next| next.is_component_sep());
            if at_edge || adjacent {
                return Some(NativeDateOutcome::Malformed {
                    reason: NativeDateMalformed::EmptyComponent,
                });
            }
        }
    }
    // 7. Non-ISO numeric order: n(1-2) sep n(1-2) sep yyyy. Day-first vs
    //    month-first is locale-dependent; never resolved here. If neither
    //    reading is even domain-plausible, the input is out-of-range instead.
    for w in toks.windows(5) {
        let Some((a, a_digits)) = w[0].num() else {
            continue;
        };
        let Some((b, b_digits)) = w[2].num() else {
            continue;
        };
        let Some((_, year_digits)) = w[4].num() else {
            continue;
        };
        if a_digits > 2 || b_digits > 2 || year_digits != 4 {
            continue;
        }
        if !w[1].is_year_sep() || !w[3].is_year_sep() {
            continue;
        }
        if a > 31 && b > 31 {
            return Some(NativeDateOutcome::Malformed {
                reason: NativeDateMalformed::ComponentOutOfRange,
            });
        }
        return Some(NativeDateOutcome::Ambiguous {
            reason: NativeDateAmbiguous::NumericComponentOrder,
        });
    }
    None
}

fn parse(input: &str) -> NativeDateOutcome {
    // Byte cap first: oversized inputs are never scanned.
    if input.len() > NATIVE_DATE_INPUT_BYTE_CAP {
        return NativeDateOutcome::Oversized;
    }
    let toks = match tokenize(input) {
        Ok(toks) => toks,
        Err(reason) => return NativeDateOutcome::Malformed { reason },
    };
    if toks.is_empty() {
        return NativeDateOutcome::Empty;
    }
    if let Some(outcome) = structural_outcome(&toks) {
        return outcome;
    }

    // Shape attempts from every startable boundary. A match may only start
    // after a separator or at input start (a glued token is part of the
    // preceding cluster and is never split).
    let mut attempts = Vec::new();
    for start in 0..toks.len() {
        if toks[start].glued() {
            continue;
        }
        if let Some(attempt) = shape_at(&toks, start) {
            attempts.push(attempt);
        }
    }

    // Selection: the leftmost attempt that consumes the entire remaining
    // input decides — recognized if valid, otherwise its closed failure
    // reason (a full-input reading never silently falls back to a later,
    // weaker reading). Ties at the same start were already resolved to the
    // most specific shape. This reproduces the oracle corpus, including
    // display forms with prefix context noise.
    for attempt in &attempts {
        if attempt.end == toks.len() {
            return match attempt.result {
                Ok(date) => NativeDateOutcome::Recognized { date },
                Err(AttemptInvalid::Malformed(reason)) => NativeDateOutcome::Malformed { reason },
                Err(AttemptInvalid::Unsupported(reason)) => {
                    NativeDateOutcome::Unsupported { reason }
                }
            };
        }
    }
    // No full reading: the first invalid attempt (leftmost, then most
    // specific) carries the closed reason.
    for attempt in &attempts {
        if let Err(invalid) = attempt.result {
            return match invalid {
                AttemptInvalid::Malformed(reason) => NativeDateOutcome::Malformed { reason },
                AttemptInvalid::Unsupported(reason) => NativeDateOutcome::Unsupported { reason },
            };
        }
    }
    // A date-shaped prefix exists but the rest is not a sanctioned
    // continuation: trailing garbage is never accepted.
    if attempts.iter().any(|attempt| attempt.end < toks.len()) {
        return NativeDateOutcome::Malformed {
            reason: NativeDateMalformed::TrailingGarbage,
        };
    }
    // Lone number that no shape explained: a year outside the four-digit
    // domain (bounded huge-year inputs included).
    if let [Tok::Num { digits, .. }] = toks[..] {
        return if digits > 4 {
            unsupported_outcome(NativeDateUnsupported::YearTooLong)
        } else {
            unsupported_outcome(NativeDateUnsupported::YearTooShort)
        };
    }
    NativeDateOutcome::Malformed {
        reason: NativeDateMalformed::GrammarMismatch,
    }
}
