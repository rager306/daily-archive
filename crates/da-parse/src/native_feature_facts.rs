//! Pure per-token categorical facts for the native GROBID bibliography feature ABI.
//!
//! S01's `FeatureAbi` emits a provider-neutral `f32` channel vector and S03's
//! [`crate::native_header_features`] consumes ordered strings for the header
//! model, but neither exposes the *categorical* facts the bibliography rows are
//! built from. The citation row is 30 whitespace-delimited columns and the
//! reference-segmenter row is 28; almost every column is a category spelling
//! (`LINESTART`, `INITCAP`, `ALLDIGIT`, `NOPUNCT`, ...) or a bounded integer,
//! never a float. Those facts live here and nowhere else, so the two row
//! builders (and the nested `name/citation` and `date` rows) share one home
//! instead of copying private helpers.
//!
//! ## Source of truth (format authority)
//!
//! Ported from the vendored GROBID 0.9.1 tree, read for format only and never
//! linked, vendored, or copied:
//!
//! - `grobid-core/src/main/java/org/grobid/core/features/FeaturesVectorCitation.java`
//!   (`printVector`, `addFeaturesCitation`) — the 30-column citation row and the
//!   `capitalisation` / `digit` / `punctType` / `singleChar` / `year` / `month`
//!   lanes as they are *emitted*;
//! - `grobid-core/src/main/java/org/grobid/core/features/FeaturesVectorReferenceSegmenter.java`
//!   (`printVector`) — the 28-column reference-segmenter row (note: `email` is
//!   commented out upstream and therefore must not be emitted);
//! - `grobid-core/src/main/java/org/grobid/core/features/FeatureFactory.java`
//!   (`test_all_capital`, `test_digit`, `test_common`, `test_names`,
//!   `test_first_names`, `test_last_names`, `test_month`, `linearScaling`,
//!   `MONTHS`) — the predicates and the scaling primitive;
//! - `grobid-core/src/main/java/org/grobid/core/utilities/TextUtilities.java`
//!   (`prefix`, `suffix`, `punctuationProfile`, `fullPunctuations`);
//! - `grobid-core/src/main/java/org/grobid/core/engines/ReferenceSegmenterParser.java`
//!   (`LINESCALE`, the `lineLength` / `relativePosition` call sites) and
//!   `.../engines/CitationParser.java` (the `nbBins` call site).
//!
//! ## Emitted values, not natural intent
//!
//! `printVector` overrides `capitalisation` with `NOCAPS` whenever
//! `digit == "ALLDIGIT"`, and a token with no lowercase character at all —
//! including pure punctuation such as `.` — lands on `ALLCAP` because
//! `test_all_capital` only rejects lowercase characters. Both quirks are
//! reproduced here and pinned by tests: this layer must produce the row the
//! model was trained on, not the tidy value a reader would guess.
//!
//! ## Known residuals (contract-level proof, not source-bound parity)
//!
//! - Java indexes UTF-16 code units (`String.length()`, `charAt`, `prefix`,
//!   `suffix`, `substring`); this module indexes Unicode scalar values. The two
//!   agree for every BMP token, which is what the frozen models were trained
//!   on; they differ for astral-plane tokens only.
//! - `Character.isDigit` (Java, Unicode decimal digits) is approximated by
//!   `char::is_numeric`, which also covers `Nl`/`No` (e.g. `²`). `ALLDIGIT` uses
//!   ASCII digits, matching Java's default `\d`.
//! - Java regex `$` also matches before a trailing line terminator; tokens
//!   carrying whitespace are rejected by the row builders, so the divergence is
//!   unreachable on the ABI path.
//! - `commonName` reproduces `Lexicon.inDictionary` exactly as far as the
//!   trailing-punctuation strip and the single hyphen/space split; GROBID's
//!   wordform morphology is *not* modelled and the real dictionary stays behind
//!   the opt-in resource path (D358).
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! regex, no absolute path, no model bytes, no real lexicon bytes. Every
//! function is total — untrusted tokens (empty, whitespace, multibyte) must
//! yield facts, never a panic. The band is candidate-only: `import_eligible`
//! stays false and nothing here is spliced into the production parse route.

use da_ports::LexiconBundle;
use serde::{Deserialize, Serialize};

/// Schema identity for the typed fact layer.
pub const NATIVE_FEATURE_FACTS_SCHEMA_VERSION: &str = "da-parse.native-feature-facts/v1";

/// `FeaturesVectorCitation.nbBins` — relative-position bins for the citation row.
pub const CITATION_RELATIVE_POSITION_BINS: i32 = 12;

/// `ReferenceSegmenterParser.LINESCALE` — scaling for the reference-segmenter row.
pub const REFERENCE_SEGMENTER_LINE_SCALE: i32 = 10;

/// Maximum emitted punctuation-profile value; longer profiles print `10`.
pub const BIBLIOGRAPHY_MAX_PUNCTUATION_PROFILE: usize = 10;

/// Lexicon channel that carries the `commonName` (`Lexicon.inDictionary`) lane.
pub const BIBLIOGRAPHY_COMMON_NAME_CHANNEL: &str = "commonName";

/// Lexicon channel that carries the `firstName` (`Lexicon.inFirstNames`) lane.
pub const BIBLIOGRAPHY_FIRST_NAME_CHANNEL: &str = "firstName";

/// Lexicon channel that carries the `lastName` (`Lexicon.inLastNames`) lane.
pub const BIBLIOGRAPHY_LAST_NAME_CHANNEL: &str = "lastName";

/// `FeatureFactory.MONTHS`, verbatim: twelve full names followed by twelve
/// abbreviations. `may` occurs once in each half and `sept` is *absent*
/// (`sep` is the abbreviation GROBID knows), so the list cannot be derived.
pub const BIBLIOGRAPHY_MONTHS: [&str; 24] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "may",
    "jun",
    "jul",
    "aug",
    "sep",
    "oct",
    "nov",
    "dec",
];

/// `TextUtilities.fullPunctuations`, verbatim (43 characters). Two members are
/// easy to mistranscribe and are pinned by tests: an ordinary space and `]`
/// both belong to the set, and `\u{a0}` is a *non-breaking* space, so it is
/// collected by [`punctuation_profile`] even though `' '` is skipped.
pub const BIBLIOGRAPHY_FULL_PUNCTUATIONS: &str =
    "(（[ •*,:;?.!/)）-−–‐«»„\"“”‘’'`$#@]*♦♥♣♠\u{a0}。、，・";

/// Token line status, spelled exactly as `printVector` emits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FactsLineStatus {
    Linestart,
    Linein,
    Lineend,
}

impl FactsLineStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linestart => "LINESTART",
            Self::Linein => "LINEIN",
            Self::Lineend => "LINEEND",
        }
    }
}

/// Token block status, spelled exactly as `printVector` emits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FactsBlockStatus {
    Blockstart,
    Blockin,
    Blockend,
}

impl FactsBlockStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blockstart => "BLOCKSTART",
            Self::Blockin => "BLOCKIN",
            Self::Blockend => "BLOCKEND",
        }
    }
}

/// Line alignment, spelled as `FeaturesVectorReferenceSegmenter` emits it.
///
/// Only [`BibliographyAlignment::Alignedleft`] and
/// [`BibliographyAlignment::Lineindent`] are produced by the 0.9.1 bibliography
/// paths (`ReferenceSegmenterParser` never sets a third value). `Centered`
/// belongs to the wider GROBID layout vocabulary and is carried so the typed
/// alignment fact is not silently narrower than `printVector`, which prints
/// whatever the producer supplied. Note the emitted spelling is `LINEINDENT`,
/// never `INDENT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BibliographyAlignment {
    Alignedleft,
    Lineindent,
    Centered,
}

impl BibliographyAlignment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alignedleft => "ALIGNEDLEFT",
            Self::Lineindent => "LINEINDENT",
            Self::Centered => "CENTERED",
        }
    }
}

/// `capitalisation` lane: `INITCAP` / `ALLCAP` / `NOCAPS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Capitalisation {
    Initcap,
    Allcap,
    Nocaps,
}

impl Capitalisation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initcap => "INITCAP",
            Self::Allcap => "ALLCAP",
            Self::Nocaps => "NOCAPS",
        }
    }
}

/// `digit` lane: `ALLDIGIT` / `CONTAINSDIGITS` / `NODIGIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DigitStatus {
    Alldigit,
    Containsdigits,
    Nodigit,
}

impl DigitStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alldigit => "ALLDIGIT",
            Self::Containsdigits => "CONTAINSDIGITS",
            Self::Nodigit => "NODIGIT",
        }
    }
}

/// `punctType` lane, in the exact order `printVector`'s producers assign it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PunctType {
    Punct,
    Openbracket,
    Endbracket,
    Dot,
    Comma,
    Hyphen,
    Quote,
    Nopunct,
}

impl PunctType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Punct => "PUNCT",
            Self::Openbracket => "OPENBRACKET",
            Self::Endbracket => "ENDBRACKET",
            Self::Dot => "DOT",
            Self::Comma => "COMMA",
            Self::Hyphen => "HYPHEN",
            Self::Quote => "QUOTE",
            Self::Nopunct => "NOPUNCT",
        }
    }
}

/// Typed input token for one bibliography position.
///
/// `text` is the normalised token text (the producer applies GROBID's
/// `UnicodeUtil.normaliseTextAndRemoveSpaces` before handing the token over);
/// the status/alignment values are supplied by the layout producer because the
/// Java original derives them from neighbouring tokens and line geometry that a
/// single token cannot see. `line_length` is the raw line length;
/// `superscript` is carried for the callout marker classifier (S04/T06), which
/// consumes the same token shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BibliographyTokenInput {
    pub text: String,
    pub block_status: FactsBlockStatus,
    pub line_status: FactsLineStatus,
    pub alignment: BibliographyAlignment,
    pub line_length: usize,
    pub superscript: bool,
}

/// The four lexicon-backed name predicates of the citation row (columns 14-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BibliographyNameFlags {
    /// `FeatureFactory.test_names` — first *or* last name.
    pub proper_name: bool,
    /// `FeatureFactory.test_common` — `Lexicon.inDictionary`.
    pub common_name: bool,
    /// `FeatureFactory.test_first_names`.
    pub first_name: bool,
    /// `FeatureFactory.test_last_names`.
    pub last_name: bool,
}

/// Categorical facts for one token, before any lexicon-backed lane is applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BibliographyTokenFacts {
    /// Column 0 of both rows, before normalisation is re-applied.
    pub text: String,
    /// Column 1 of both rows.
    pub lowercase: String,
    /// Columns 2-5: first 1..4 characters.
    pub prefix: [String; 4],
    /// Columns 6-9: last 1..4 characters.
    pub suffix: [String; 4],
    pub line_status: FactsLineStatus,
    pub block_status: FactsBlockStatus,
    pub alignment_status: BibliographyAlignment,
    /// The `INITCAP`/`ALLCAP`/`NOCAPS` ladder *before* the `ALLDIGIT`
    /// substitution; use [`BibliographyTokenFacts::printed_capitalisation`] for
    /// the value the row emits.
    pub capitalisation: Capitalisation,
    pub digit: DigitStatus,
    pub single_char: bool,
    pub punct_type: PunctType,
    /// Raw line length supplied by the producer. The reference-segmenter's
    /// printed `lineLength` column is *not* this value: it is
    /// `linear_scaling(line_length, max_line_length, LINESCALE)`, applied by the
    /// row builder with its block-level maximum.
    pub line_length: usize,
    /// `Pattern("[1,2][0-9][0-9][0-9]")` applied with `find()`.
    pub year: bool,
    /// `FeatureFactory.test_month`.
    pub month: bool,
    /// `Pattern("http(s)?")` applied with `find()`.
    pub http: bool,
    /// Columns 14-17; all false until a lexicon is supplied.
    pub name_flags: BibliographyNameFlags,
}

impl BibliographyTokenFacts {
    /// Derive every lexicon-independent fact for one token. Total: any input
    /// string yields facts, and empty or hostile tokens never panic.
    pub fn from_token(input: &BibliographyTokenInput) -> Self {
        let text = input.text.as_str();
        Self {
            text: input.text.clone(),
            lowercase: text.to_lowercase(),
            prefix: std::array::from_fn(|index| token_prefix(text, index + 1)),
            suffix: std::array::from_fn(|index| token_suffix(text, index + 1)),
            line_status: input.line_status,
            block_status: input.block_status,
            alignment_status: input.alignment,
            capitalisation: capitalisation(text),
            digit: digit_status(text),
            single_char: is_single_char(text),
            punct_type: punct_type(text),
            line_length: input.line_length,
            year: is_year_shape(text),
            month: is_month_name(text),
            http: contains_http_shape(text),
            name_flags: BibliographyNameFlags::default(),
        }
    }

    /// Attach the lexicon-backed name lanes (see [`name_flags`]).
    pub fn with_name_flags(mut self, flags: BibliographyNameFlags) -> Self {
        self.name_flags = flags;
        self
    }

    /// The value `printVector` emits for the `capitalisation` column: `NOCAPS`
    /// whenever the `digit` lane is `ALLDIGIT`, otherwise the ladder result.
    pub const fn printed_capitalisation(&self) -> Capitalisation {
        capitalisation_as_printed(self.capitalisation, self.digit)
    }
}

/// `TextUtilities.prefix`: the first `count` characters, or the whole string
/// when it is shorter than `count`.
pub fn token_prefix(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

/// `TextUtilities.suffix`: the last `count` characters, or the whole string
/// when it is shorter than `count`.
pub fn token_suffix(text: &str, count: usize) -> String {
    let characters: Vec<char> = text.chars().collect();
    characters[characters.len().saturating_sub(count)..]
        .iter()
        .collect()
}

/// `FeatureFactory.test_all_capital`: true when the string is non-empty and
/// contains no lowercase character (digits and punctuation therefore pass).
pub fn is_all_capital(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(|c| c.is_lowercase())
}

/// The `capitalisation` ladder as `addFeaturesCitation` builds it: `INITCAP`
/// when the first character is uppercase, overridden by `ALLCAP` when no
/// character is lowercase, and `NOCAPS` when neither fires.
pub fn capitalisation(text: &str) -> Capitalisation {
    let mut value = None;
    if text.chars().next().is_some_and(|c| c.is_uppercase()) {
        value = Some(Capitalisation::Initcap);
    }
    if is_all_capital(text) {
        value = Some(Capitalisation::Allcap);
    }
    value.unwrap_or(Capitalisation::Nocaps)
}

/// The `ALLDIGIT` substitution performed by both `printVector`s.
pub const fn capitalisation_as_printed(
    capitalisation: Capitalisation,
    digit: DigitStatus,
) -> Capitalisation {
    match digit {
        DigitStatus::Alldigit => Capitalisation::Nocaps,
        DigitStatus::Containsdigits | DigitStatus::Nodigit => capitalisation,
    }
}

/// The `digit` ladder: `CONTAINSDIGITS` when any character is numeric,
/// overridden by `ALLDIGIT` when every character is an ASCII digit (`^\d+$`,
/// so at least one digit is required), and `NODIGIT` when neither fires.
pub fn digit_status(text: &str) -> DigitStatus {
    let mut value = None;
    if text.chars().any(|c| c.is_numeric()) {
        value = Some(DigitStatus::Containsdigits);
    }
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()) {
        value = Some(DigitStatus::Alldigit);
    }
    value.unwrap_or(DigitStatus::Nodigit)
}

/// True when the token occupies exactly one character (column 13/14).
pub fn is_single_char(text: &str) -> bool {
    text.chars().count() == 1
}

/// Java `Pattern("^[,\\:;?\\.]+$")` tested with `find()`: a non-empty run of
/// `,`, `:`, `;`, `?` or `.`.
pub fn is_punctuation_run(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| matches!(c, ',' | ':' | ';' | '?' | '.'))
}

/// The `punctType` ladder, in the producer's exact assignment order.
///
/// The punctuation run is tested first and *then* overridden by the literal
/// single-character cases, so `,` is `COMMA` rather than `PUNCT`, `..` stays
/// `PUNCT` rather than `DOT`, and a token such as `!` — punctuation to a human,
/// but absent from the Java set — falls through to `NOPUNCT`.
pub fn punct_type(text: &str) -> PunctType {
    let mut value = None;
    if is_punctuation_run(text) {
        value = Some(PunctType::Punct);
    }
    if text == "(" || text == "[" {
        value = Some(PunctType::Openbracket);
    } else if text == ")" || text == "]" {
        value = Some(PunctType::Endbracket);
    } else if text == "." {
        value = Some(PunctType::Dot);
    } else if text == "," {
        value = Some(PunctType::Comma);
    } else if text == "-" {
        value = Some(PunctType::Hyphen);
    } else if text == "\"" || text == "'" || text == "`" {
        value = Some(PunctType::Quote);
    }
    value.unwrap_or(PunctType::Nopunct)
}

/// `Pattern("[1,2][0-9][0-9][0-9]")` tested with `find()`.
///
/// Two properties come straight from the frozen pattern and are easy to
/// "fix" by mistake: the match is a *substring* search (not anchored), and the
/// leading character class is `{'1', ',', '2'}` — the comma is a member, not a
/// separator, so `,999` is a year shape. Kept verbatim so the row the model
/// sees is unchanged.
pub fn is_year_shape(text: &str) -> bool {
    let characters: Vec<char> = text.chars().collect();
    if characters.len() < 4 {
        return false;
    }
    characters.windows(4).any(|window| {
        matches!(window[0], '1' | ',' | '2') && window[1..].iter().all(char::is_ascii_digit)
    })
}

/// `FeatureFactory.test_month`: exact membership of `MONTHS` after lowercasing.
pub fn is_month_name(text: &str) -> bool {
    let lowercase = text.to_lowercase();
    BIBLIOGRAPHY_MONTHS.contains(&lowercase.as_str())
}

/// `Pattern("http(s)?")` tested with `find()`: does the token contain the
/// literal substring `http`? Case-sensitive and unanchored, so `https` and
/// `nothttp` both qualify while `HTTP` does not. Note this is the *fact* named
/// after the Java pattern; the citation row's own `http` column (21) is fed by
/// the gazetteer URL lane, not by this predicate.
pub fn contains_http_shape(text: &str) -> bool {
    text.contains("http")
}

/// `FeatureFactory.linearScaling(int pos, int total, int nbBins)`.
///
/// The branch order is load-bearing and is why this is not a plain ratio:
/// `pos >= total` returns `nbBins` *before* the `pos <= 0` guard, so a zero
/// total with a zero position yields `nbBins` rather than a division by zero or
/// zero. The arithmetic is deliberately `f32`: Java casts the position to
/// `float`, and `1/3 * 12` is `4` in `f32` but `3` in `f64` after truncation.
pub fn linear_scaling(pos: i32, total: i32, nb_bins: i32) -> i32 {
    if pos >= total {
        return nb_bins;
    }
    if pos <= 0 {
        return 0;
    }
    let relative = pos as f32 / total as f32;
    let scaled = relative * nb_bins as f32;
    scaled as i32
}

/// Citation column 28: the token position scaled against the token count with
/// `nbBins = 12`.
pub fn relative_position_citation(position: usize, sentence_length: usize) -> i32 {
    linear_scaling(
        saturating_i32(position),
        saturating_i32(sentence_length),
        CITATION_RELATIVE_POSITION_BINS,
    )
}

/// Reference-segmenter column 23: the token position scaled against its line
/// length with `LINESCALE = 10`. The same scale also produces column 24's
/// `lineLength` value from a caller-supplied block maximum — see
/// [`linear_scaling`].
pub fn relative_position_reference_segmenter(position: usize, line_length: usize) -> i32 {
    linear_scaling(
        saturating_i32(position),
        saturating_i32(line_length),
        REFERENCE_SEGMENTER_LINE_SCALE,
    )
}

/// `TextUtilities.punctuationProfile`: the punctuation characters of a line,
/// in order, with `' '` skipped. `\u{a0}` is a member of the set and is
/// *not* skipped, because the Java guard only special-cases the space
/// character.
pub fn punctuation_profile(line: &str) -> String {
    line.chars()
        .filter(|c| *c != ' ' && BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains(*c))
        .collect()
}

/// The reference-segmenter's punctuation-profile column: `no` for an empty
/// profile, otherwise the profile length capped at
/// [`BIBLIOGRAPHY_MAX_PUNCTUATION_PROFILE`].
pub fn punctuation_profile_field(profile: &str) -> String {
    let length = profile.chars().count();
    if length == 0 {
        "no".to_string()
    } else {
        length.min(BIBLIOGRAPHY_MAX_PUNCTUATION_PROFILE).to_string()
    }
}

/// The four lexicon-backed name predicates for one token.
///
/// Mirrors `FeatureFactory.test_names` / `test_first_names` / `test_last_names`
/// (exact membership of `firstNames` / `lastNames` after lowercasing) and
/// `test_common` (`Lexicon.inDictionary`). This is *single-token membership*,
/// the same mechanism S03 uses for the header lanes; the positional gazetteer
/// that feeds the `isKnown*` columns is a different matcher with its own home.
pub fn name_flags(text: &str, lexicon: &LexiconBundle) -> BibliographyNameFlags {
    let lowercase = text.to_lowercase();
    let first_name = channel_contains(lexicon, BIBLIOGRAPHY_FIRST_NAME_CHANNEL, &lowercase);
    let last_name = channel_contains(lexicon, BIBLIOGRAPHY_LAST_NAME_CHANNEL, &lowercase);
    BibliographyNameFlags {
        proper_name: first_name || last_name,
        common_name: common_name(text, lexicon),
        first_name,
        last_name,
    }
}

/// `FeatureFactory.test_common` → `Lexicon.inDictionary(tok.trim().toLowerCase())`.
fn common_name(text: &str, lexicon: &LexiconBundle) -> bool {
    let mut candidate = text.trim().to_lowercase();
    if candidate.ends_with(&['.', ',', ':', ';'][..]) {
        candidate.pop();
    }
    if candidate.is_empty() {
        return false;
    }
    if let Some(index) = candidate.find('-') {
        return channel_contains(
            lexicon,
            BIBLIOGRAPHY_COMMON_NAME_CHANNEL,
            &candidate[..index],
        ) && channel_contains(
            lexicon,
            BIBLIOGRAPHY_COMMON_NAME_CHANNEL,
            &candidate[index + 1..],
        );
    }
    if let Some(index) = candidate.find(' ') {
        return channel_contains(
            lexicon,
            BIBLIOGRAPHY_COMMON_NAME_CHANNEL,
            &candidate[..index],
        ) && channel_contains(
            lexicon,
            BIBLIOGRAPHY_COMMON_NAME_CHANNEL,
            &candidate[index + 1..],
        );
    }
    channel_contains(lexicon, BIBLIOGRAPHY_COMMON_NAME_CHANNEL, &candidate)
}

fn channel_contains(lexicon: &LexiconBundle, key: &str, needle: &str) -> bool {
    !needle.is_empty()
        && lexicon
            .channels
            .iter()
            .find(|channel| channel_matches(&channel.key, key))
            .is_some_and(|channel| channel.entries.iter().any(|entry| entry == needle))
}

/// Channel keys are matched alphanumerically and case-insensitively so
/// `firstName`, `first_name` and `Firstname` resolve to one lane.
fn channel_matches(actual: &str, expected: &str) -> bool {
    normalize_channel_key(actual) == normalize_channel_key(expected)
}

fn normalize_channel_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn saturating_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::{FeatureTemplate, LexiconChannel};

    fn token(text: &str) -> BibliographyTokenInput {
        BibliographyTokenInput {
            text: text.to_string(),
            block_status: FactsBlockStatus::Blockin,
            line_status: FactsLineStatus::Linein,
            alignment: BibliographyAlignment::Alignedleft,
            line_length: 40,
            superscript: false,
        }
    }

    fn lexicon() -> LexiconBundle {
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: [
                ("commonName", vec!["acid", "water", "well", "known"]),
                ("firstName", vec!["ashish"]),
                ("lastName", vec!["vaswani"]),
            ]
            .into_iter()
            .map(|(key, entries)| LexiconChannel {
                key: key.into(),
                entries: entries.into_iter().map(String::from).collect(),
            })
            .collect(),
            feature_templates: vec![FeatureTemplate {
                key: "bibliography".into(),
                channel_keys: vec!["firstName".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }

    #[test]
    fn capitalisation_transposes_the_java_ladder() {
        assert_eq!(capitalisation("Name"), Capitalisation::Initcap);
        assert_eq!(capitalisation("NAME"), Capitalisation::Allcap);
        assert_eq!(capitalisation("name"), Capitalisation::Nocaps);
        // `A1` is INITCAP by the first-character test and ALLCAP by the
        // no-lowercase test; the second assignment wins, as in Java.
        assert_eq!(capitalisation("A1"), Capitalisation::Allcap);
        // Pure punctuation has no lowercase character, so it is ALLCAP.
        assert_eq!(capitalisation("."), Capitalisation::Allcap);
        assert_eq!(capitalisation("abc-DEF"), Capitalisation::Nocaps);
        // Empty input is rejected by the row builders; the predicate is total.
        assert_eq!(capitalisation(""), Capitalisation::Nocaps);
    }

    #[test]
    fn all_digit_substitutes_nocaps_as_print_vector_does() {
        let digits = BibliographyTokenFacts::from_token(&token("2017"));
        assert_eq!(digits.capitalisation, Capitalisation::Allcap);
        assert_eq!(digits.digit, DigitStatus::Alldigit);
        assert_eq!(digits.printed_capitalisation(), Capitalisation::Nocaps);

        // A non-ALLDIGIT token keeps the ladder value verbatim.
        let mixed = BibliographyTokenFacts::from_token(&token("2nd"));
        assert_eq!(mixed.digit, DigitStatus::Containsdigits);
        assert_eq!(mixed.capitalisation, Capitalisation::Nocaps);
        assert_eq!(mixed.printed_capitalisation(), Capitalisation::Nocaps);

        let dotted = BibliographyTokenFacts::from_token(&token("."));
        assert_eq!(dotted.digit, DigitStatus::Nodigit);
        assert_eq!(dotted.printed_capitalisation(), Capitalisation::Allcap);
    }

    #[test]
    fn digit_lane_transposes_the_java_ladder() {
        assert_eq!(digit_status("2017"), DigitStatus::Alldigit);
        assert_eq!(digit_status("2nd"), DigitStatus::Containsdigits);
        assert_eq!(digit_status("abc"), DigitStatus::Nodigit);
        assert_eq!(digit_status("."), DigitStatus::Nodigit);
        assert_eq!(digit_status(""), DigitStatus::Nodigit);
        // `test_digit` is Unicode-aware; the ASCII-only `^\d+$` is not.
        assert_eq!(digit_status("٣٤"), DigitStatus::Containsdigits);
    }

    #[test]
    fn punct_type_follows_the_java_assignment_order() {
        assert_eq!(punct_type(","), PunctType::Comma);
        assert_eq!(punct_type("."), PunctType::Dot);
        assert_eq!(punct_type(".."), PunctType::Punct);
        assert_eq!(punct_type("?:"), PunctType::Punct);
        assert_eq!(punct_type("("), PunctType::Openbracket);
        assert_eq!(punct_type("["), PunctType::Openbracket);
        assert_eq!(punct_type(")"), PunctType::Endbracket);
        assert_eq!(punct_type("]"), PunctType::Endbracket);
        assert_eq!(punct_type("-"), PunctType::Hyphen);
        assert_eq!(punct_type("\""), PunctType::Quote);
        assert_eq!(punct_type("'"), PunctType::Quote);
        assert_eq!(punct_type("`"), PunctType::Quote);
        assert_eq!(punct_type("!"), PunctType::Nopunct);
        assert_eq!(punct_type(";"), PunctType::Punct);
        assert_eq!(punct_type("word"), PunctType::Nopunct);
        assert_eq!(punct_type(""), PunctType::Nopunct);
    }

    #[test]
    fn year_shape_keeps_the_literal_comma_class_and_substring_search() {
        assert!(is_year_shape("1999"));
        assert!(is_year_shape("2020"));
        assert!(is_year_shape("2999"));
        // The Java class is {'1', ',', '2'}; the comma is a member.
        assert!(is_year_shape(",999"));
        // `find()` is not anchored.
        assert!(is_year_shape("x1999y"));
        // The class is {'1', ',', '2'} and the remaining three positions are
        // digits, so `1600` is a year shape for the frozen pattern.
        assert!(is_year_shape("1600"));
        // A multi-byte prefix does not stop the substring scan.
        assert!(is_year_shape("🙂2020"));
        assert!(!is_year_shape("0600"));
        assert!(!is_year_shape("199"));
        assert!(!is_year_shape("🙂199"));
        assert!(!is_year_shape(""));
    }

    #[test]
    fn month_list_is_the_frozen_twenty_four_entries() {
        assert_eq!(BIBLIOGRAPHY_MONTHS.len(), 24);
        assert!(is_month_name("may"));
        assert!(is_month_name("May"));
        assert!(is_month_name("SEPTEMBER"));
        assert!(is_month_name("sep"));
        // `sept` is not in the Java list; `sep` is.
        assert!(!is_month_name("sept"));
        assert!(!is_month_name("september "));
        assert!(!is_month_name(""));
    }

    #[test]
    fn http_shape_mirrors_the_unanchored_java_pattern() {
        assert!(contains_http_shape("http"));
        assert!(contains_http_shape("https://a.b"));
        assert!(contains_http_shape("nothttp"));
        assert!(!contains_http_shape("HTTP"));
        assert!(!contains_http_shape("ftp://a.b"));
    }

    #[test]
    fn linear_scaling_matches_the_java_branches() {
        // `pos >= total` is evaluated before the `pos <= 0` guard.
        assert_eq!(linear_scaling(0, 0, 12), 12);
        assert_eq!(linear_scaling(0, 5, 12), 0);
        assert_eq!(linear_scaling(5, 5, 12), 12);
        assert_eq!(linear_scaling(11, 12, 12), 11);
        assert_eq!(linear_scaling(13, 12, 12), 12);
        assert_eq!(linear_scaling(0, -1, 10), 10);
        // f32 arithmetic is load-bearing: `1/3 * 12` truncates to 4 in f32 and
        // to 3 in f64.
        assert_eq!(linear_scaling(1, 3, 12), 4);
        assert_eq!(linear_scaling(2, 3, 12), 8);
    }

    #[test]
    fn the_two_families_scale_with_different_bins() {
        // Same position and total, different declared scales.
        assert_eq!(relative_position_citation(6, 7), 10);
        assert_eq!(relative_position_reference_segmenter(6, 7), 8);
        assert_eq!(relative_position_citation(3, 4), 9);
        assert_eq!(relative_position_reference_segmenter(3, 4), 7);
    }

    #[test]
    fn punctuation_profile_matches_the_java_collector() {
        assert_eq!(punctuation_profile("a, b. c"), ",.");
        assert_eq!(punctuation_profile("no punctuation here"), "");
        // `]`, `*` and the space belong to `fullPunctuations`; the space is
        // skipped explicitly and the others are collected.
        assert_eq!(punctuation_profile("x ] * y"), "]*");
        // U+00A0 is a member and is *not* the space character.
        assert_eq!(punctuation_profile("a\u{a0}b"), "\u{a0}");
        assert_eq!(punctuation_profile(""), "");
    }

    #[test]
    fn punctuation_profile_field_caps_at_ten_or_reports_none() {
        assert_eq!(punctuation_profile_field(""), "no");
        assert_eq!(punctuation_profile_field(".,"), "2");
        assert_eq!(punctuation_profile_field(&".".repeat(10)), "10");
        assert_eq!(punctuation_profile_field(&".".repeat(11)), "10");
    }

    #[test]
    fn full_punctuations_constant_keeps_its_verbatim_members() {
        assert_eq!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.chars().count(), 43);
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains(' '));
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains(']'));
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('*'));
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('\u{a0}'));
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('\u{ff08}'));
        assert!(BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('\u{2666}'));
        assert!(!BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('A'));
        assert!(!BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('0'));
        assert!(!BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains('\n'));
    }

    #[test]
    fn token_prefix_and_suffix_transpose_text_utilities() {
        assert_eq!(token_prefix("abcd", 2), "ab");
        assert_eq!(token_prefix("ab", 4), "ab");
        assert_eq!(token_prefix("", 3), "");
        assert_eq!(token_suffix("abcd", 2), "cd");
        assert_eq!(token_suffix("ab", 4), "ab");
        assert_eq!(token_suffix("", 3), "");
        let facts = BibliographyTokenFacts::from_token(&token("Water"));
        assert_eq!(facts.prefix, ["W", "Wa", "Wat", "Wate"]);
        assert_eq!(facts.suffix, ["r", "er", "ter", "ater"]);
        assert_eq!(facts.lowercase, "water");
        assert!(!facts.single_char);
    }

    #[test]
    fn name_flags_transpose_the_java_predicates() {
        let lexicon = lexicon();
        let flags = name_flags("Vaswani", &lexicon);
        assert!(flags.proper_name);
        assert!(flags.last_name);
        assert!(!flags.first_name);
        assert!(!flags.common_name);

        let flags = name_flags("Ashish", &lexicon);
        assert!(flags.proper_name);
        assert!(flags.first_name);
        assert!(!flags.last_name);

        // `test_common` strips exactly one trailing punctuation character.
        assert!(name_flags("water", &lexicon).common_name);
        assert!(name_flags("Water,", &lexicon).common_name);
        // Hyphen and space split only when *both* halves are in the dictionary.
        assert!(name_flags("well-known", &lexicon).common_name);
        assert!(!name_flags("well-unknown", &lexicon).common_name);
        assert!(name_flags("acid water", &lexicon).common_name);
        assert!(!name_flags("acid plasma", &lexicon).common_name);
        assert!(!name_flags("nothing", &lexicon).common_name);

        let facts = BibliographyTokenFacts::from_token(&token("Vaswani"))
            .with_name_flags(name_flags("Vaswani", &lexicon));
        assert!(facts.name_flags.proper_name);
    }

    #[test]
    fn name_flags_tolerate_an_empty_lexicon() {
        let empty = LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: vec![LexiconChannel {
                key: "other".into(),
                entries: vec!["x".into()],
            }],
            feature_templates: vec![FeatureTemplate {
                key: "t".into(),
                channel_keys: vec!["other".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        };
        let flags = name_flags("Vaswani", &empty);
        assert_eq!(flags, BibliographyNameFlags::default());
    }

    #[test]
    fn facts_are_total_on_hostile_tokens() {
        let long = "x".repeat(4096);
        let cases: [&str; 10] = [
            "",
            " ",
            "\n",
            "\t",
            "  \n  ",
            "日本語",
            "🙂",
            ")(",
            "a\u{0}b",
            long.as_str(),
        ];
        for text in cases {
            let facts = BibliographyTokenFacts::from_token(&token(text));
            assert_eq!(facts.text, text);
            assert_eq!(facts.single_char, text.chars().count() == 1);
            // Re-deriving the same token must be byte-identical.
            assert_eq!(facts, BibliographyTokenFacts::from_token(&token(text)));
        }
        assert_eq!(punctuation_profile("\n\t"), "");
        assert!(!is_year_shape("🙂199"));
    }

    #[test]
    fn alignment_spellings_stay_the_emitted_ones() {
        assert_eq!(BibliographyAlignment::Alignedleft.as_str(), "ALIGNEDLEFT");
        assert_eq!(BibliographyAlignment::Lineindent.as_str(), "LINEINDENT");
        // The emitted spelling is LINEINDENT; a bare INDENT would be a
        // mistranscription of the reference-segmenter vocabulary.
        assert_ne!(BibliographyAlignment::Lineindent.as_str(), "INDENT");
        assert_eq!(FactsLineStatus::Linein.as_str(), "LINEIN");
        assert_eq!(FactsBlockStatus::Blockin.as_str(), "BLOCKIN");
        assert_eq!(PunctType::Nopunct.as_str(), "NOPUNCT");
    }

    #[test]
    fn facts_carry_the_typed_status_inputs_through() {
        let mut input = token("word");
        input.line_status = FactsLineStatus::Lineend;
        input.block_status = FactsBlockStatus::Blockend;
        input.alignment = BibliographyAlignment::Lineindent;
        input.line_length = 77;
        input.superscript = true;
        let facts = BibliographyTokenFacts::from_token(&input);
        assert_eq!(facts.line_status, FactsLineStatus::Lineend);
        assert_eq!(facts.block_status, FactsBlockStatus::Blockend);
        assert_eq!(facts.alignment_status, BibliographyAlignment::Lineindent);
        assert_eq!(facts.line_length, 77);
        assert!(!facts.year);
        assert!(!facts.month);
        assert!(!facts.http);
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    fn arbitrary_text() -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), 0..24)
            .prop_map(|characters| characters.into_iter().collect::<String>())
    }

    proptest! {
        #[test]
        fn facts_are_deterministic_for_arbitrary_tokens(text in arbitrary_text()) {
            let input = BibliographyTokenInput {
                text: text.clone(),
                block_status: FactsBlockStatus::Blockstart,
                line_status: FactsLineStatus::Linestart,
                alignment: BibliographyAlignment::Alignedleft,
                line_length: text.chars().count(),
                superscript: false,
            };
            let first = BibliographyTokenFacts::from_token(&input);
            let second = BibliographyTokenFacts::from_token(&input);
            prop_assert_eq!(&first, &second);
            prop_assert_eq!(&first.text, &text);
            prop_assert_eq!(first.lowercase, text.to_lowercase());
        }

        #[test]
        fn prefix_and_suffix_are_bounded_by_the_token_length(text in arbitrary_text(), count in 0usize..8) {
            let characters = text.chars().count();
            prop_assert_eq!(token_prefix(&text, count).chars().count(), count.min(characters));
            prop_assert_eq!(token_suffix(&text, count).chars().count(), count.min(characters));
        }

        #[test]
        fn scaling_stays_inside_its_declared_range(pos in 0i32..64, total in 0i32..64, bins in 1i32..64) {
            let value = linear_scaling(pos, total, bins);
            prop_assert!(value >= 0);
            prop_assert!(value <= bins);
            if pos >= total {
                prop_assert_eq!(value, bins);
            }
        }

        #[test]
        fn scaling_is_monotonic_in_position(total in 1i32..64, first in 0i32..64, second in 0i32..64, bins in 1i32..64) {
            let (low, high) = if first <= second { (first, second) } else { (second, first) };
            let low = low.min(total);
            let high = high.min(total);
            prop_assert!(linear_scaling(low, total, bins) <= linear_scaling(high, total, bins));
        }

        #[test]
        fn punctuation_profile_only_ever_collects_members(line in arbitrary_text()) {
            let profile = punctuation_profile(&line);
            prop_assert!(profile.chars().all(|c| c != ' ' && BIBLIOGRAPHY_FULL_PUNCTUATIONS.contains(c)));
            prop_assert!(profile.chars().count() <= line.chars().count());
            let field = punctuation_profile_field(&profile);
            prop_assert!(field == "no" || field.parse::<usize>().is_ok());
            if field != "no" {
                prop_assert!(field.parse::<usize>().unwrap() <= BIBLIOGRAPHY_MAX_PUNCTUATION_PROFILE);
            }
        }
    }
}
