//! Pure renderer for the GROBID 0.9.0 CRF funding feature ABI.
//!
//! One model-facing string ABI lives here and nothing else: the
//! `FeaturesVectorFunding.printVector` row. It is whitespace separated, one row
//! per admitted token, and candidate-only: this module renders rows, it never
//! loads a model, never decodes a label sequence, and is never spliced into the
//! production parse route.
//!
//! ## Source of truth (format authority)
//!
//! Ported from the vendored GROBID 0.9.x tree, read for format only and never
//! linked, vendored, or copied:
//!
//! - `grobid-core/src/main/java/org/grobid/core/features/FeaturesVectorFunding.java`
//!   (`printVector`, `addFeatures`) — the exact emitted column list and order;
//! - `grobid-core/src/main/java/org/grobid/core/features/FeatureFactory.java`
//!   (`isDigit` = `^\d+$`, `isPunct` = `^[,\:;?\.]+$`, `test_all_capital`,
//!   `test_digit`) — already transcribed in [`crate::native_feature_facts`];
//! - `grobid-core/src/main/java/org/grobid/core/utilities/TextUtilities.java`
//!   (`prefix`, `suffix`).
//!
//! ## The 18 fields of one row, in order
//!
//! | # | Java field | source |
//! |---|---|---|
//! | 0 | `string` | token text (producer-normalised) |
//! | 1 | lowercased `string` | `String.toLowerCase` |
//! | 2-5 | `prefix(1..4)` | `TextUtilities.prefix` |
//! | 6-9 | `suffix(1..4)` | `TextUtilities.suffix` |
//! | 10 | `lineStatus` | producer (`LINESTART` / `LINEIN` / `LINEEND`) |
//! | 11 | `capitalisation` | ladder, `ALLDIGIT` overrides to `NOCAPS` |
//! | 12 | `digit` | ladder, `ALLDIGIT` overrides `CONTAINSDIGITS` |
//! | 13 | `singleChar` | length == 1 |
//! | 14 | `knownFunder` | caller-supplied coverage (see below) |
//! | 15 | `knownInfrastructure` | caller-supplied coverage (see below) |
//! | 16 | `punctType` | punctuation ladder |
//! | 17 | label slot | the literal `0` at inference time |
//!
//! Column 17 is the *only* field after `punctType`. This ABI is the inference
//! ABI: Java's `printVector` prints the gold tag there when `label != null`, and
//! the funding model is fed the literal `0` instead. The training slot is
//! deliberately not modelled — it is a named `offline_limit` of this band (D358:
//! training parity needs the frozen corpus, which is not bundled), and
//! [`NATIVE_FUNDING_LABEL_COLUMN_DUMMY`] is the only value this renderer can
//! emit there.
//!
//! ## Deliberately absent (the D363 trap)
//!
//! `FeaturesVectorFunding` declares `containDash` and `containPunct`, assigns
//! neither in `addFeatures`, and prints neither in `printVector`. They must not
//! be emitted, because the frozen model was trained on exactly the 18 fields
//! above. The family also has no font and no relative-position lanes: unlike the
//! fulltext ABI there is no `fontStatus` / `fontSize` /
//! `relativeDocumentPosition` column here, and adding one would shift every lane
//! after it.
//!
//! ## Gazetteer coverage is caller-supplied (D363), never invented
//!
//! Columns 14 and 15 are the two lexicon lanes of the funding model. Their real
//! feeds are `grobid-home/lexicon/organisations/funders.txt` (3.3 MB) and
//! `research_infrastructures.txt`, which D358 forbids bundling, so this module
//! accepts a caller-built [`NativeFundingGazetteerCoverage`] of typed positional
//! spans instead of reading a lexicon. Coverage is compared against the *admitted
//! token list* (`start <= n <= end`, both ends inclusive) — the same inclusive
//! rule the sibling bibliography gazetteer publishes
//! ([`crate::native_bibliography_gazetteer::span_covers`]).
//!
//! Java walks its span list with a stateful cursor
//! (`currentFunderPositions` / `skipTest`). That cursor is an iteration
//! optimization over the same inclusive predicate for the sorted,
//! non-overlapping spans `Lexicon` produces; here the predicate itself is
//! authoritative, so overlapping or unsorted caller spans cannot silently drop
//! coverage. [`NativeFundingGazetteerCoverage::covers_funder`] is the direct
//! predicate; [`lane_flags`] is its linear-time sweep form, pinned equal to the
//! predicate by contract test.
//!
//! **Empty coverage is an `offline_limit`, not an observation.** With no spans
//! supplied both flags print `0`, which is the value Java prints for "this token
//! is not inside any known funder". The two cases are therefore distinguishable
//! only through the coverage value itself, and this band never presents the
//! `0`-with-empty-coverage case as an observed negative: the caller owns the
//! lanes, the module never fabricates coverage, and the opt-in harness is the
//! only thing allowed to read the real lexicons (T07).
//!
//! ## Emitted value, not intent
//!
//! The printed `capitalisation` is `NOCAPS` whenever `digit == ALLDIGIT`, so
//! `2017` is `ALLCAP` by the ladder and `NOCAPS` as printed. Pinned by test.
//!
//! ## Known residuals (contract-level proof, not source-bound parity)
//!
//! - Java sends every token through
//!   `UnicodeUtil.normaliseTextAndRemoveSpaces` and then `continue`s on blank
//!   tokens; this module takes the already normalised text from the producer and
//!   *refuses* an empty or whitespace-carrying token instead of skipping it.
//!   Skipping would shift every later token index, and the coverage spans are
//!   token positions, so a refused token is the only fail-closed option.
//! - Java indexes UTF-16 code units (`String.length()`, `charAt`, `prefix`,
//!   `suffix`); this module indexes Unicode scalar values. The two agree for
//!   every BMP token, which is what the frozen model was trained on.
//! - Java's `String.toLowerCase` uses the default locale; `str::to_lowercase` is
//!   locale-independent.
//! - The `lineStatus` lookahead (`"\n"` / `"@newline"`) is the producer's: a
//!   single token cannot see the next one, so the status arrives as a typed
//!   input, exactly as in the header and fulltext bands.
//!
//! ## Redaction boundary
//!
//! A rendered row carries token text by construction — it *is* the model input.
//! Nothing here logs, persists, or reports a row; downstream evidence (fixtures,
//! same-n oracles, governor stamps) must use [`FundingFeatureRow::digest`], which
//! is why the digest lives on the row type. Funder/grant/person strings are PII
//! (D363 advisory): they may cross this API as model input and must not cross it
//! into a report verbatim.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! regex, no absolute path, no model bytes, no lexicon bytes. Every function is
//! total on hostile input — untrusted tokens (empty, whitespace, multibyte) and
//! hostile spans (inverted, unbounded count) yield a *typed* refusal, never a
//! panic and never a silent approximation. `import_eligible` stays false and
//! nothing here is spliced into the production parse route.

use crate::native_feature_facts::{
    Capitalisation, DigitStatus, FactsLineStatus, PunctType, capitalisation,
    capitalisation_as_printed, digit_status, is_single_char, punct_type, token_prefix,
    token_suffix,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the rendered funding rows.
pub const NATIVE_FUNDING_FEATURES_SCHEMA_VERSION: &str = "da-parse.native-funding-features/v1";

/// Printed *feature* columns the funding model consumes: `#rdr#100/16/0` names
/// the 17 featured columns of `FeaturesVectorFunding.printVector`.
pub const NATIVE_FUNDING_FEATURE_COLUMNS: usize = 17;

/// Highest column *index* the model references (`0..=16`).
pub const NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX: usize = NATIVE_FUNDING_FEATURE_COLUMNS - 1;

/// Index of the label slot: the field after the last printed feature column.
pub const NATIVE_FUNDING_LABEL_COLUMN_INDEX: usize = NATIVE_FUNDING_FEATURE_COLUMNS;

/// The literal the inference ABI places in the label slot.
pub const NATIVE_FUNDING_LABEL_COLUMN_DUMMY: &str = "0";

/// Printed width of one rendered row: the 17 feature columns plus the label slot.
pub const NATIVE_FUNDING_ROW_COLUMNS: usize = NATIVE_FUNDING_FEATURE_COLUMNS + 1;

/// Upper bound on `render_funding_features` input length.
///
/// Provisional and fail-closed: the funding band is not a runtime surface yet,
/// so an input beyond this bound is refused rather than truncated, and no
/// silently partial read can be mistaken for a complete one.
pub const MAX_NATIVE_FUNDING_FEATURE_ROWS: usize = 32_768;

/// Upper bound on the caller-supplied coverage spans of both lanes together.
///
/// The two real lexicons hold tens of thousands of entries; the cap keeps a
/// shadow-band call from being driven into an unbounded flag sweep.
pub const MAX_NATIVE_FUNDING_GAZETTEER_SPANS: usize = 65_536;

/// `lineStatus` lane, spelled exactly as `printVector` emits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FundingLineStatus {
    Linestart,
    Linein,
    Lineend,
}

impl FundingLineStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linestart => "LINESTART",
            Self::Linein => "LINEIN",
            Self::Lineend => "LINEEND",
        }
    }
}

impl From<FactsLineStatus> for FundingLineStatus {
    fn from(status: FactsLineStatus) -> Self {
        match status {
            FactsLineStatus::Linestart => Self::Linestart,
            FactsLineStatus::Linein => Self::Linein,
            FactsLineStatus::Lineend => Self::Lineend,
        }
    }
}

/// `capitalisation` lane: `INITCAP` / `ALLCAP` / `NOCAPS`.
///
/// The funding family carries its own spelling vocabulary, converted once from
/// the shared ladder in [`crate::native_feature_facts`], for the same reason the
/// fulltext band carries its own position scale: a change to the shared ladder
/// must not silently move a second ABI's emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FundingCapitalisation {
    Initcap,
    Allcap,
    Nocaps,
}

impl FundingCapitalisation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initcap => "INITCAP",
            Self::Allcap => "ALLCAP",
            Self::Nocaps => "NOCAPS",
        }
    }
}

impl From<Capitalisation> for FundingCapitalisation {
    fn from(value: Capitalisation) -> Self {
        match value {
            Capitalisation::Initcap => Self::Initcap,
            Capitalisation::Allcap => Self::Allcap,
            Capitalisation::Nocaps => Self::Nocaps,
        }
    }
}

/// `digit` lane: `ALLDIGIT` / `CONTAINSDIGITS` / `NODIGIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FundingDigit {
    Alldigit,
    Containsdigits,
    Nodigit,
}

impl FundingDigit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alldigit => "ALLDIGIT",
            Self::Containsdigits => "CONTAINSDIGITS",
            Self::Nodigit => "NODIGIT",
        }
    }
}

impl From<DigitStatus> for FundingDigit {
    fn from(value: DigitStatus) -> Self {
        match value {
            DigitStatus::Alldigit => Self::Alldigit,
            DigitStatus::Containsdigits => Self::Containsdigits,
            DigitStatus::Nodigit => Self::Nodigit,
        }
    }
}

/// `punctType` lane, in the exact order `printVector`'s producers assign it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FundingPunctType {
    Nopunct,
    Openbracket,
    Endbracket,
    Dot,
    Comma,
    Hyphen,
    Quote,
    Punct,
}

impl FundingPunctType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nopunct => "NOPUNCT",
            Self::Openbracket => "OPENBRACKET",
            Self::Endbracket => "ENDBRACKET",
            Self::Dot => "DOT",
            Self::Comma => "COMMA",
            Self::Hyphen => "HYPHEN",
            Self::Quote => "QUOTE",
            Self::Punct => "PUNCT",
        }
    }
}

impl From<PunctType> for FundingPunctType {
    fn from(value: PunctType) -> Self {
        match value {
            PunctType::Punct => Self::Punct,
            PunctType::Openbracket => Self::Openbracket,
            PunctType::Endbracket => Self::Endbracket,
            PunctType::Dot => Self::Dot,
            PunctType::Comma => Self::Comma,
            PunctType::Hyphen => Self::Hyphen,
            PunctType::Quote => Self::Quote,
            PunctType::Nopunct => Self::Nopunct,
        }
    }
}

/// The two gazetteer lanes of the funding model.
///
/// The spellings are the `gazetteer_lanes` spellings of
/// `data/parse/native_visual_families.yaml`, bound 1:1 by the contract test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeFundingGazetteerLane {
    KnownFunder,
    KnownInfrastructure,
}

impl NativeFundingGazetteerLane {
    /// Every lane, in the DATA declaration order.
    pub const ALL: [Self; 2] = [Self::KnownFunder, Self::KnownInfrastructure];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KnownFunder => "knownFunder",
            Self::KnownInfrastructure => "knownInfrastructure",
        }
    }

    /// Column index this lane feeds in the printed row.
    pub const fn column_index(self) -> usize {
        match self {
            Self::KnownFunder => 14,
            Self::KnownInfrastructure => 15,
        }
    }
}

/// One admitted token of the funding sequence.
///
/// `text` is the normalised token text (the producer applies GROBID's
/// `UnicodeUtil.normaliseTextAndRemoveSpaces` before handing the token over) and
/// `line_status` is supplied by the layout producer, because the Java original
/// derives it from neighbouring tokens and line geometry a single token cannot
/// see. There is deliberately no label field: the training slot is not modelled
/// (see the module doc), so a caller cannot inject a gold tag into this ABI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingFeatureToken {
    pub text: String,
    pub line_status: FundingLineStatus,
}

/// One half-open-inclusive span over the *admitted token list*.
///
/// The span covers `start`, `end`, and every token between them — the same
/// inclusive rule `FeaturesVectorFunding.addFeatures` consumes with
/// `start <= n <= end`. A span whose `start` is greater than its `end` is not an
/// empty span: it is a typed refusal, because treating it as "covers nothing"
/// would fail open and silently drop coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFundingGazetteerSpan {
    pub start: usize,
    pub end: usize,
}

impl NativeFundingGazetteerSpan {
    /// The Java consumption rule: both ends inclusive.
    pub const fn covers(self, n: usize) -> bool {
        self.start <= n && n <= self.end
    }

    /// True when the span cannot be satisfied by any index.
    pub const fn is_inverted(self) -> bool {
        self.start > self.end
    }
}

/// Caller-supplied positional coverage of the two gazetteer lanes (D363).
///
/// An empty coverage is valid and means "no lexicon was supplied": both flags
/// then print `0`, which is the documented `offline_limit` of this band, never an
/// observed "not a funder" fact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFundingGazetteerCoverage {
    pub funders: Vec<NativeFundingGazetteerSpan>,
    pub infrastructures: Vec<NativeFundingGazetteerSpan>,
}

impl NativeFundingGazetteerCoverage {
    /// Spans of one lane, in caller order.
    pub fn for_lane(&self, lane: NativeFundingGazetteerLane) -> &[NativeFundingGazetteerSpan] {
        match lane {
            NativeFundingGazetteerLane::KnownFunder => &self.funders,
            NativeFundingGazetteerLane::KnownInfrastructure => &self.infrastructures,
        }
    }

    /// Spans across both lanes.
    pub fn span_count(&self) -> usize {
        self.funders.len() + self.infrastructures.len()
    }

    /// True when no lane carries a span.
    pub fn is_empty(&self) -> bool {
        self.span_count() == 0
    }

    /// Direct inclusive predicate of the `knownFunder` lane.
    pub fn covers_funder(&self, n: usize) -> bool {
        span_covers(&self.funders, n)
    }

    /// Direct inclusive predicate of the `knownInfrastructure` lane.
    pub fn covers_infrastructure(&self, n: usize) -> bool {
        span_covers(&self.infrastructures, n)
    }
}

/// The inclusive coverage predicate of one lane, published as its own function
/// precisely because it is the thing `printVector` consumes.
pub fn span_covers(spans: &[NativeFundingGazetteerSpan], n: usize) -> bool {
    spans.iter().any(|span| span.covers(n))
}

/// One rendered funding row: 17 feature columns plus the label slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingFeatureRow {
    pub columns: Vec<String>,
}

impl FundingFeatureRow {
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// The label slot as printed: always [`NATIVE_FUNDING_LABEL_COLUMN_DUMMY`]
    /// for a row this module built.
    pub fn label_column(&self) -> Option<&str> {
        self.columns
            .get(NATIVE_FUNDING_LABEL_COLUMN_INDEX)
            .map(String::as_str)
    }

    /// Redaction-safe identity of the row: `sha256:` over the UTF-8 bytes of
    /// [`Self::as_text`].
    ///
    /// This is the only shape in which a funding row may leave the band into a
    /// fixture, report, or governor stamp — token text is raw funder/grant/person
    /// material (PII) and the model input is where it stays.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.as_text().as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }
}

/// Typed refusal of the funding row builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFundingFeatureError {
    /// No tokens were supplied; an empty row list is not a valid observation.
    EmptyInput,
    /// A token text was empty.
    EmptyToken { token: usize },
    /// A token text contains whitespace, which would corrupt the row arity by
    /// splitting one field into two.
    TokenContainsWhitespace { token: usize },
    /// The input exceeded [`MAX_NATIVE_FUNDING_FEATURE_ROWS`].
    TooManyRows { rows: usize, maximum: usize },
    /// A rendered row did not carry exactly [`NATIVE_FUNDING_ROW_COLUMNS`]
    /// fields.
    WrongArity {
        row: usize,
        expected: usize,
        actual: usize,
    },
    /// A model declared a different number of observation columns than this ABI
    /// prints; fail closed instead of widening the row.
    DeclaredColumnMismatch { declared: usize },
    /// A coverage span cannot be satisfied by any index (`start > end`).
    ///
    /// Refusing keeps an inverted span from reading as "covers nothing", which
    /// would be a silent loss of coverage (fail-open).
    InvertedGazetteerSpan {
        lane: NativeFundingGazetteerLane,
        index: usize,
        start: usize,
        end: usize,
    },
    /// The coverage carries more spans than the bounded sweep admits.
    TooManyGazetteerSpans { spans: usize, maximum: usize },
}

impl fmt::Display for NativeFundingFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "funding feature input is empty"),
            Self::EmptyToken { token } => write!(f, "funding feature token {token} is empty"),
            Self::TokenContainsWhitespace { token } => write!(
                f,
                "funding feature token {token} contains a separator or whitespace"
            ),
            Self::TooManyRows { rows, maximum } => write!(
                f,
                "funding feature input has {rows} rows; the admitted limit is {maximum}"
            ),
            Self::WrongArity {
                row,
                expected,
                actual,
            } => write!(
                f,
                "rendered funding row {row} has {actual} columns; expected {expected}"
            ),
            Self::DeclaredColumnMismatch { declared } => write!(
                f,
                "model declares {declared} funding observation columns; this ABI prints {NATIVE_FUNDING_FEATURE_COLUMNS}"
            ),
            Self::InvertedGazetteerSpan {
                lane,
                index,
                start,
                end,
            } => write!(
                f,
                "funding gazetteer span {index} of lane {lane} is inverted ({start} > {end})"
            ),
            Self::TooManyGazetteerSpans { spans, maximum } => write!(
                f,
                "funding gazetteer coverage carries {spans} spans; the admitted limit is {maximum}"
            ),
        }
    }
}

impl std::error::Error for NativeFundingFeatureError {}

impl fmt::Display for NativeFundingGazetteerLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fail closed when a model declares a different observation width than the
/// printed ABI.
///
/// Called with the number of funding observation columns a frozen model
/// declares. Widening the row to satisfy a model would emit fields the model was
/// never trained on, so a mismatch — including a model that counts the dummy
/// label slot as an observation column — is a typed refusal.
pub fn check_funding_feature_columns(declared: usize) -> Result<(), NativeFundingFeatureError> {
    if declared == NATIVE_FUNDING_FEATURE_COLUMNS {
        Ok(())
    } else {
        Err(NativeFundingFeatureError::DeclaredColumnMismatch { declared })
    }
}

/// Reject a coverage value the sweep is not allowed to consume.
///
/// Exposed separately so a caller can validate its own inventory before rendering
/// a sequence, and used by [`render_funding_features`] so the flag lanes can
/// never be built from an inverted or unbounded inventory.
pub fn validate_funding_gazetteer_coverage(
    coverage: &NativeFundingGazetteerCoverage,
) -> Result<(), NativeFundingFeatureError> {
    let spans = coverage.span_count();
    if spans > MAX_NATIVE_FUNDING_GAZETTEER_SPANS {
        return Err(NativeFundingFeatureError::TooManyGazetteerSpans {
            spans,
            maximum: MAX_NATIVE_FUNDING_GAZETTEER_SPANS,
        });
    }
    for lane in NativeFundingGazetteerLane::ALL {
        for (index, span) in coverage.for_lane(lane).iter().enumerate() {
            if span.is_inverted() {
                return Err(NativeFundingFeatureError::InvertedGazetteerSpan {
                    lane,
                    index,
                    start: span.start,
                    end: span.end,
                });
            }
        }
    }
    Ok(())
}

/// Render ordered tokens into the exact 18-field
/// `FeaturesVectorFunding.printVector` ABI.
///
/// Rows are rendered in caller order; `coverage` decides the two gazetteer lanes
/// and an empty coverage prints `0` in both (see the module doc: that is the
/// documented `offline_limit`, not an observed negative).
pub fn render_funding_features(
    tokens: &[FundingFeatureToken],
    coverage: &NativeFundingGazetteerCoverage,
) -> Result<Vec<FundingFeatureRow>, NativeFundingFeatureError> {
    if tokens.is_empty() {
        return Err(NativeFundingFeatureError::EmptyInput);
    }
    if tokens.len() > MAX_NATIVE_FUNDING_FEATURE_ROWS {
        return Err(NativeFundingFeatureError::TooManyRows {
            rows: tokens.len(),
            maximum: MAX_NATIVE_FUNDING_FEATURE_ROWS,
        });
    }
    validate_funding_gazetteer_coverage(coverage)?;
    for (index, token) in tokens.iter().enumerate() {
        validate_token(index, token)?;
    }
    let funder = lane_flags(&coverage.funders, tokens.len());
    let infrastructure = lane_flags(&coverage.infrastructures, tokens.len());
    let mut rows = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        let facts_text = token.text.as_str();
        let digit = digit_status(facts_text);
        let printed_capitalisation = FundingCapitalisation::from(capitalisation_as_printed(
            capitalisation(facts_text),
            digit,
        ));
        let columns = vec![
            token.text.clone(),
            facts_text.to_lowercase(),
            token_prefix(facts_text, 1),
            token_prefix(facts_text, 2),
            token_prefix(facts_text, 3),
            token_prefix(facts_text, 4),
            token_suffix(facts_text, 1),
            token_suffix(facts_text, 2),
            token_suffix(facts_text, 3),
            token_suffix(facts_text, 4),
            token.line_status.as_str().into(),
            printed_capitalisation.as_str().into(),
            FundingDigit::from(digit).as_str().into(),
            bool_flag(is_single_char(facts_text)),
            bool_flag(funder[index]),
            bool_flag(infrastructure[index]),
            FundingPunctType::from(punct_type(facts_text))
                .as_str()
                .into(),
            NATIVE_FUNDING_LABEL_COLUMN_DUMMY.into(),
        ];
        if columns.len() != NATIVE_FUNDING_ROW_COLUMNS {
            return Err(NativeFundingFeatureError::WrongArity {
                row: index,
                expected: NATIVE_FUNDING_ROW_COLUMNS,
                actual: columns.len(),
            });
        }
        rows.push(FundingFeatureRow { columns });
    }
    Ok(rows)
}

/// Serialize rows exactly as the funding model's `map_input` consumes them: one
/// row per line, no trailing newline (the caller joins/terminates).
pub fn render_funding_feature_text(
    rows: &[FundingFeatureRow],
) -> Result<String, NativeFundingFeatureError> {
    let mut lines = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        if row.width() != NATIVE_FUNDING_ROW_COLUMNS {
            return Err(NativeFundingFeatureError::WrongArity {
                row: index,
                expected: NATIVE_FUNDING_ROW_COLUMNS,
                actual: row.width(),
            });
        }
        lines.push(row.as_text());
    }
    Ok(lines.join("\n"))
}

/// Per-token flags of one gazetteer lane, as the linear-time form of
/// [`span_covers`].
///
/// Spans are sorted here (a copy, never the caller's vector) and consumed with a
/// running maximum end, so the result is independent of the caller's span order
/// and of span overlap. Inverted spans cannot reach this function: the coverage
/// is validated first.
fn lane_flags(spans: &[NativeFundingGazetteerSpan], token_count: usize) -> Vec<bool> {
    let mut flags = vec![false; token_count];
    let mut bounds: Vec<(usize, usize)> = spans.iter().map(|span| (span.start, span.end)).collect();
    bounds.sort_unstable();
    let mut cursor = 0usize;
    let mut reach: Option<usize> = None;
    for (n, flag) in flags.iter_mut().enumerate() {
        while cursor < bounds.len() && bounds[cursor].0 <= n {
            let end = bounds[cursor].1;
            reach = Some(reach.map_or(end, |current| current.max(end)));
            cursor += 1;
        }
        *flag = reach.is_some_and(|current| current >= n);
    }
    flags
}

fn validate_token(
    index: usize,
    token: &FundingFeatureToken,
) -> Result<(), NativeFundingFeatureError> {
    if token.text.is_empty() {
        return Err(NativeFundingFeatureError::EmptyToken { token: index });
    }
    if token.text.chars().any(char::is_whitespace) {
        return Err(NativeFundingFeatureError::TokenContainsWhitespace { token: index });
    }
    Ok(())
}

fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str) -> FundingFeatureToken {
        FundingFeatureToken {
            text: text.into(),
            line_status: FundingLineStatus::Linestart,
        }
    }

    fn coverage(
        funders: Vec<NativeFundingGazetteerSpan>,
        infrastructures: Vec<NativeFundingGazetteerSpan>,
    ) -> NativeFundingGazetteerCoverage {
        NativeFundingGazetteerCoverage {
            funders,
            infrastructures,
        }
    }

    #[test]
    fn the_row_is_the_seventeen_features_plus_the_dummy_label() {
        let rows = render_funding_features(&[token("Cambridge")], &coverage(vec![], vec![]))
            .expect("a valid token renders");
        assert_eq!(rows[0].width(), NATIVE_FUNDING_ROW_COLUMNS);
        assert_eq!(
            rows[0].label_column(),
            Some(NATIVE_FUNDING_LABEL_COLUMN_DUMMY)
        );
        assert_eq!(
            rows[0].columns[0..4],
            ["Cambridge", "cambridge", "C", "Ca"].map(String::from)
        );
        assert_eq!(rows[0].columns[10], "LINESTART");
        assert_eq!(rows[0].columns[11], "INITCAP");
        assert_eq!(rows[0].columns[12], "NODIGIT");
        assert_eq!(rows[0].columns[13], "0");
        assert_eq!(rows[0].columns[16], "NOPUNCT");
    }

    #[test]
    fn an_all_digit_token_prints_nocaps() {
        let rows = render_funding_features(&[token("2017")], &coverage(vec![], vec![]))
            .expect("a valid token renders");
        assert_eq!(rows[0].columns[11], "NOCAPS");
        assert_eq!(rows[0].columns[12], "ALLDIGIT");
    }

    #[test]
    fn coverage_lanes_are_inclusive_on_both_ends() {
        let rows = render_funding_features(
            &["a", "b", "c", "d", "e"].map(token),
            &coverage(
                vec![NativeFundingGazetteerSpan { start: 2, end: 4 }],
                vec![],
            ),
        )
        .expect("a valid sequence renders");
        let flags: Vec<&str> = rows.iter().map(|row| row.columns[14].as_str()).collect();
        assert_eq!(flags, ["0", "0", "1", "1", "1"]);
        assert!(rows.iter().all(|row| row.columns[15] == "0"));
    }

    #[test]
    fn an_empty_coverage_prints_zero_and_is_not_an_observed_negative() {
        let empty = coverage(vec![], vec![]);
        let rows = render_funding_features(&[token("Cambridge")], &empty).expect("renders");
        assert_eq!(rows[0].columns[14], "0");
        assert_eq!(rows[0].columns[15], "0");
        assert!(empty.is_empty());
        let covered = coverage(
            vec![NativeFundingGazetteerSpan { start: 0, end: 0 }],
            vec![NativeFundingGazetteerSpan { start: 0, end: 0 }],
        );
        let supplied = render_funding_features(&[token("Cambridge")], &covered).expect("renders");
        assert_eq!(supplied[0].columns[14], "1");
        assert_eq!(supplied[0].columns[15], "1");
    }

    #[test]
    fn an_inverted_span_is_refused() {
        let error = render_funding_features(
            &[token("Cambridge")],
            &coverage(
                vec![NativeFundingGazetteerSpan { start: 4, end: 2 }],
                vec![],
            ),
        )
        .expect_err("an inverted span must be refused");
        assert_eq!(
            error,
            NativeFundingFeatureError::InvertedGazetteerSpan {
                lane: NativeFundingGazetteerLane::KnownFunder,
                index: 0,
                start: 4,
                end: 2,
            }
        );
    }

    #[test]
    fn an_empty_or_whitespace_token_is_refused() {
        let empty = render_funding_features(&[token("")], &coverage(vec![], vec![]))
            .expect_err("an empty token must be refused");
        assert_eq!(empty, NativeFundingFeatureError::EmptyToken { token: 0 });
        let spaced = render_funding_features(&[token("two words")], &coverage(vec![], vec![]))
            .expect_err("a token with whitespace must be refused");
        assert_eq!(
            spaced,
            NativeFundingFeatureError::TokenContainsWhitespace { token: 0 }
        );
        let empty_input = render_funding_features(&[], &coverage(vec![], vec![]))
            .expect_err("empty input refused");
        assert_eq!(empty_input, NativeFundingFeatureError::EmptyInput);
    }

    #[test]
    fn a_multibyte_token_still_renders_the_full_row() {
        let rows = render_funding_features(&[token("Ünïcödé")], &coverage(vec![], vec![]))
            .expect("a multibyte token renders");
        assert_eq!(rows[0].width(), NATIVE_FUNDING_ROW_COLUMNS);
        assert_eq!(rows[0].columns[1], "ünïcödé");
        assert_eq!(rows[0].columns[2], "Ü");
        assert_eq!(rows[0].columns[6], "é");
        assert_eq!(rows[0].columns[7], "dé");
    }

    #[test]
    fn the_declared_feature_width_must_be_the_printed_one() {
        assert!(check_funding_feature_columns(NATIVE_FUNDING_FEATURE_COLUMNS).is_ok());
        for declared in [
            NATIVE_FUNDING_FEATURE_COLUMNS - 1,
            NATIVE_FUNDING_FEATURE_COLUMNS + 1,
            NATIVE_FUNDING_ROW_COLUMNS,
        ] {
            assert_eq!(
                check_funding_feature_columns(declared),
                Err(NativeFundingFeatureError::DeclaredColumnMismatch { declared })
            );
        }
    }

    #[test]
    fn a_row_of_the_wrong_width_is_refused_by_the_text_renderer() {
        for width in [
            NATIVE_FUNDING_FEATURE_COLUMNS,
            NATIVE_FUNDING_ROW_COLUMNS + 1,
        ] {
            let row = FundingFeatureRow {
                columns: vec!["0".into(); width],
            };
            assert_eq!(
                render_funding_feature_text(&[row]),
                Err(NativeFundingFeatureError::WrongArity {
                    row: 0,
                    expected: NATIVE_FUNDING_ROW_COLUMNS,
                    actual: width,
                })
            );
        }
        let rows = render_funding_features(&[token("a"), token("b")], &coverage(vec![], vec![]))
            .expect("renders");
        let text = render_funding_feature_text(&rows).expect("rows are the declared width");
        assert_eq!(text.lines().count(), 2);
        assert!(!text.ends_with('\n'));
    }

    #[test]
    fn the_row_digest_is_stable_and_carries_no_raw_text() {
        let rows = render_funding_features(&[token("Funder")], &coverage(vec![], vec![]))
            .expect("renders");
        let digest = rows[0].digest();
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), "sha256:".len() + 64);
        assert!(!digest.contains("Funder"));
        let again = render_funding_features(&[token("Funder")], &coverage(vec![], vec![]))
            .expect("renders");
        assert_eq!(again[0].digest(), digest);
    }

    #[test]
    fn the_sweep_agrees_with_the_inclusive_predicate() {
        let spans = vec![
            NativeFundingGazetteerSpan { start: 4, end: 6 },
            NativeFundingGazetteerSpan { start: 0, end: 1 },
            NativeFundingGazetteerSpan { start: 3, end: 3 },
            NativeFundingGazetteerSpan { start: 5, end: 9 },
        ];
        let flags = lane_flags(&spans, 12);
        for (n, flag) in flags.iter().enumerate() {
            assert_eq!(*flag, span_covers(&spans, n), "token {n}");
        }
    }
}
