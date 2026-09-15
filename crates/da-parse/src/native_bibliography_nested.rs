//! Pure row builders and nested-family decoder for the GROBID `name/citation`
//! and `date` CRFs.
//!
//! A citation decode (`citation/model.wapiti`, S04/T04) yields an `<author>`,
//! `<editor>` or `<date>` role cluster. GROBID then runs *nested* models over the
//! tokens of those clusters: `name/citation/model.wapiti` (9 labels) over the
//! author/editor text, and `date/model.wapiti` (7 labels) over the date text
//! (`AuthorParser.processingCitation` feeds both, and always passes the same
//! title/suffix position lists it passes for the header family). This module owns
//! exactly those two families: their model-order label tables, their feature-row
//! ABIs, and the projection of a decoded cluster into typed name/date values with
//! byte spans.
//!
//! It is candidate-only like the rest of the wave: it renders rows and decodes a
//! caller-supplied model. It never loads a model, never opens a file, never reads
//! the environment, and is never spliced into the production parse route.
//!
//! # The referenced column subset — an explicit, checked limitation
//!
//! `FeaturesVectorName.printVector()` emits **21** whitespace-separated fields
//! and `FeaturesVectorDate.printVector()` emits **18**: both append
//! `isKnownSuffix`/`punctType` and a training-label placeholder. The frozen
//! models reference fewer than that. Scanning the pattern blocks of the local
//! copies (`grobid-home/models/name/citation/model.wapiti` and
//! `.../date/model.wapiti`) finds `%x[..., c]` only for `c` in `0..=17` and
//! `0..=16`, matching the model headers `#rdr#63/17/0` and `#rdr#50/16/0`.
//!
//! The row builders therefore emit the **referenced subset only**:
//! [`NATIVE_NAME_CITATION_FEATURE_COLUMNS`] = 18 columns (`string` …
//! `isKnownTitle`) and [`NATIVE_DATE_FEATURE_COLUMNS`] = 17 (`string` …
//! `punctType`). The name family's `isKnownSuffix` (column 18), its `punctType`
//! (column 19) and both training-label placeholders are deliberately *not*
//! emitted: a column the model cannot reference cannot change a decode, and
//! emitting one would claim a parity this wave has not established.
//!
//! The citation derivation (30 columns) must **not** be generalised to these two
//! families: they sit on a different feature ladder, and S04's research called
//! this out as risk 1. The widths are pinned against the model headers and the
//! pattern blocks by the tests in this module and in
//! `tests/native_bibliography_contract.rs`.
//!
//! # `name/citation` row — 18 columns, `printVector()` fields 0..=17
//!
//! | col | value |
//! |-----|-------|
//! | 0 | normalised token text (`UnicodeUtil.normaliseTextAndRemoveSpaces`; the producer's job) |
//! | 1 | lowercase string |
//! | 2-5 | prefix 1..4 |
//! | 6-9 | suffix 1..4 |
//! | 10 | `lineStatus` |
//! | 11 | `capitalisation` **as printed** (`NOCAPS` whenever `digit == ALLDIGIT`) |
//! | 12 | `digit` |
//! | 13 | `singleChar` |
//! | 14 | `commonName` (`FeatureFactory.test_common` → lexicon) |
//! | 15 | `firstName` |
//! | 16 | `lastName` |
//! | 17 | `isKnownTitle` — person-title lexicon **positions** |
//!
//! Column 14 is `commonName`, *not* the citation row's `properName`: the name
//! model's lexical block is `commonName`/`firstName`/`lastName`/`isKnownTitle`,
//! while `citation_row`'s block is `properName`/`commonName`/`firstName`/
//! `lastName`. The two ladders are different and are not interchangeable.
//!
//! # `date` row — 17 columns, `printVector()` fields 0..=16
//!
//! | col | value |
//! |-----|-------|
//! | 0-9 | as above |
//! | 10 | `lineStatus` |
//! | 11 | `capitalisation` as printed |
//! | 12 | `digit` |
//! | 13 | `singleChar` |
//! | 14 | `year` (`FeatureFactory.year`, `[1,2][0-9][0-9][0-9]`) |
//! | 15 | `month` (`FeatureFactory.test_month`) |
//! | 16 | `punctType` |
//!
//! # `isKnownTitle` is a span lane, and this wave does not own it
//!
//! `isKnownTitle` is the only referenced column that is not derivable from the
//! S04 seam's inputs: [`crate::native_bibliography_gazetteer`] publishes the eight
//! *citation* lanes, and `Lexicon.tokenPositionsPersonTitle` is not one of them.
//! It is also a **phrase** lane — `grobid-home/lexicon/names/VincentNgPeopleTitles.txt`
//! contains multi-word entries such as `Professor Emeritus` — consumed with the
//! same inclusive `start <= n <= end` rule as every other lexicon lane, so
//! single-token membership would be a structural approximation.
//!
//! The lane is therefore carried as its own typed input,
//! [`NativePersonTitleCoverage`], built from positions by the caller that owns
//! the tokens. A block whose title lane has no spans yields an all-`0` column 17
//! (typed missingness, never a fabricated flag), and the row builder refuses a
//! coverage whose length disagrees with the token list instead of silently
//! shifting it.
//!
//! # H10-N1: an absent identifier stays absent
//!
//! [`cluster_identifier`] reads the citation row's `isKnownIdentifier` lane
//! ([`NATIVE_CITATION_IDENTIFIER_COLUMN`]) and returns `Option`. It never
//! synthesises an identifier from a token's *shape*: the column is evidence, and
//! the decoded name/date values ([`NativeNameCitation`], [`NativeDateValue`])
//! have no identifier field at all, so an absent identifier cannot appear in any
//! of their fields.
//!
//! # H10-N2: line and page geometry never rewrite identity
//!
//! Nothing in the projection reads `lineStatus` or `blockStatus`. The decoded
//! values are a function of the row *labels* and the column-0 token texts only,
//! so a value split across a line break or a page boundary projects to the same
//! identity and the same byte spans. `tests/native_bibliography_contract.rs`
//! asserts that equality directly, with a non-vacuous control proving the two
//! row sets really do differ.

use crate::native_bibliography_features::{
    CitationFeatureRow, NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_TRAINING_LABEL_PLACEHOLDER,
};
use crate::native_bibliography_sequence::{
    BibliographyLabelRun, BibliographySequenceFamily, CitationRole, CitationRoleSpan,
    NativeBibliographySequenceError, bibliography_label_runs, decode_bibliography_labels,
};
use crate::native_feature_facts::{BibliographyTokenFacts, name_flags};
use da_ports::{CrfModel, LexiconBundle};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema version of the rendered nested-family rows.
pub const NATIVE_BIBLIOGRAPHY_NESTED_SCHEMA_VERSION: &str =
    "da-parse.native-bibliography-nested/v1";

/// Highest column *index* the `name/citation` model references.
///
/// From the model header `#rdr#63/17/0` and confirmed by scanning every
/// `%x[..., c]` of its pattern block: `c` never exceeds 17.
pub const NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX: usize = 17;
/// Printed `name/citation` row width: the referenced subset, without the
/// `isKnownSuffix`, `punctType` and training-label fields `printVector` appends.
pub const NATIVE_NAME_CITATION_FEATURE_COLUMNS: usize = 18;
/// Labels declared by `name/citation/model.wapiti`.
pub const NATIVE_NAME_CITATION_LABEL_COUNT: usize = 9;

/// Label ids of `name/citation/model.wapiti`, in model order.
///
/// Verbatim from the model's label block: the order is load-bearing (label ids
/// index the weight blocks `uoff[o] + y`), so it is neither sorted nor paired.
/// Five base names appear; `<suffix>` appears only in its `I-` spelling, which is
/// why the grouping rule compares base names rather than BIO prefixes.
pub const NATIVE_NAME_CITATION_LABELS: [&str; NATIVE_NAME_CITATION_LABEL_COUNT] = [
    "I-<forename>",
    "I-<surname>",
    "I-<other>",
    "<other>",
    "I-<middlename>",
    "<surname>",
    "<forename>",
    "<middlename>",
    "I-<suffix>",
];

/// Highest column *index* the `date` model references (`#rdr#50/16/0`).
pub const NATIVE_DATE_MODEL_MAX_COLUMN_INDEX: usize = 16;
/// Printed `date` row width: the referenced subset, without the label field.
pub const NATIVE_DATE_FEATURE_COLUMNS: usize = 17;
/// Labels declared by `date/model.wapiti`.
pub const NATIVE_DATE_LABEL_COUNT: usize = 7;

/// Label ids of `date/model.wapiti`, in model order.
pub const NATIVE_DATE_LABELS: [&str; NATIVE_DATE_LABEL_COUNT] = [
    "I-<day>",
    "I-<month>",
    "I-<year>",
    "I-<other>",
    "<other>",
    "<year>",
    "<day>",
];

/// `name/citation/model.wapiti`: 9 labels, 18-column referenced rows.
pub const NATIVE_NAME_CITATION_SEQUENCE_FAMILY: BibliographySequenceFamily =
    BibliographySequenceFamily {
        name: "name/citation",
        expected_columns: NATIVE_NAME_CITATION_FEATURE_COLUMNS,
        admissible_labels: &NATIVE_NAME_CITATION_LABELS,
    };

/// `date/model.wapiti`: 7 labels, 17-column referenced rows.
pub const NATIVE_DATE_SEQUENCE_FAMILY: BibliographySequenceFamily = BibliographySequenceFamily {
    name: "date",
    expected_columns: NATIVE_DATE_FEATURE_COLUMNS,
    admissible_labels: &NATIVE_DATE_LABELS,
};

/// Column 26 of a citation row: the `isKnownIdentifier` gazetteer lane
/// (DOI/arXiv shapes), and therefore the *evidence* H10-N1 turns on.
///
/// Bound to the real citation ABI by a contract test that builds a citation row
/// through [`crate::native_bibliography_features::citation_row`] and checks this
/// column mirrors the identifier lane.
pub const NATIVE_CITATION_IDENTIFIER_COLUMN: usize = 26;

/// The five roles `name/citation/model.wapiti` distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamePartRole {
    Forename,
    Surname,
    Middlename,
    Suffix,
    Other,
}

/// Base-label spellings of [`NamePartRole::ALL`], index-for-index.
pub const NAME_PART_BASE_LABELS: [&str; 5] = [
    "<forename>",
    "<surname>",
    "<middlename>",
    "<suffix>",
    "<other>",
];

impl NamePartRole {
    /// Every role, in the order [`NAME_PART_BASE_LABELS`] spells.
    pub const ALL: [NamePartRole; 5] = [
        Self::Forename,
        Self::Surname,
        Self::Middlename,
        Self::Suffix,
        Self::Other,
    ];

    /// Wire spelling used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forename => "forename",
            Self::Surname => "surname",
            Self::Middlename => "middlename",
            Self::Suffix => "suffix",
            Self::Other => "other",
        }
    }

    /// The base label this role groups.
    pub const fn base_label(self) -> &'static str {
        match self {
            Self::Forename => "<forename>",
            Self::Surname => "<surname>",
            Self::Middlename => "<middlename>",
            Self::Suffix => "<suffix>",
            Self::Other => "<other>",
        }
    }

    /// Classify a base label. `None` outside the closed five.
    pub fn from_base_label(base: &str) -> Option<Self> {
        NAME_PART_BASE_LABELS
            .iter()
            .position(|label| *label == base)
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// The four roles `date/model.wapiti` distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePartRole {
    Day,
    Month,
    Year,
    Other,
}

/// Base-label spellings of [`DatePartRole::ALL`], index-for-index.
pub const DATE_PART_BASE_LABELS: [&str; 4] = ["<day>", "<month>", "<year>", "<other>"];

impl DatePartRole {
    /// Every role, in the order [`DATE_PART_BASE_LABELS`] spells.
    pub const ALL: [DatePartRole; 4] = [Self::Day, Self::Month, Self::Year, Self::Other];

    /// Wire spelling used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
            Self::Year => "year",
            Self::Other => "other",
        }
    }

    /// The base label this role groups.
    pub const fn base_label(self) -> &'static str {
        match self {
            Self::Day => "<day>",
            Self::Month => "<month>",
            Self::Year => "<year>",
            Self::Other => "<other>",
        }
    }

    /// Classify a base label. `None` outside the closed four.
    pub fn from_base_label(base: &str) -> Option<Self> {
        DATE_PART_BASE_LABELS
            .iter()
            .position(|label| *label == base)
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// Person-title lane coverage over the tokens of one name block.
///
/// Java fills `isKnownTitle` from `Lexicon.tokenPositionsPersonTitle(tokens)`,
/// a phrase lane over the person-title lexicon, consumed with the inclusive
/// `start <= n <= end` coverage predicate. The lane is not one of the eight
/// citation lanes published by [`crate::native_bibliography_gazetteer`], so the
/// caller that owns the tokens supplies its coverage here.
///
/// A block with no spans yields an all-`false` coverage: typed missingness, never
/// a fabricated flag.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePersonTitleCoverage {
    covered: Vec<bool>,
}

impl NativePersonTitleCoverage {
    /// Coverage of `token_count` tokens from half-open *inclusive* positions.
    ///
    /// `(start, end)` is the Java span shape: every token `n` with
    /// `start <= n <= end` is covered. Positions outside the token list are
    /// ignored rather than refused, mirroring the Java scan's bounds test.
    pub fn from_positions(token_count: usize, spans: &[(usize, usize)]) -> Self {
        let mut covered = vec![false; token_count];
        for &(start, end) in spans {
            if start > end {
                continue;
            }
            for slot in covered.iter_mut().take(end.saturating_add(1)).skip(start) {
                *slot = true;
            }
        }
        Self { covered }
    }

    /// Whether any span covers `position`.
    pub fn covers(&self, position: usize) -> bool {
        self.covered.get(position).copied().unwrap_or(false)
    }

    /// Number of tokens the coverage describes.
    pub fn len(&self) -> usize {
        self.covered.len()
    }

    /// Whether the coverage describes no tokens at all.
    pub fn is_empty(&self) -> bool {
        self.covered.is_empty()
    }
}

/// One rendered `name/citation` row
/// ([`NATIVE_NAME_CITATION_FEATURE_COLUMNS`] fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameCitationFeatureRow {
    pub columns: Vec<String>,
}

impl NameCitationFeatureRow {
    /// Space-joined row, the exact payload `map_input` consumes per token.
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    /// Field count; always [`NATIVE_NAME_CITATION_FEATURE_COLUMNS`] for a built row.
    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

/// One rendered `date` row ([`NATIVE_DATE_FEATURE_COLUMNS`] fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateFeatureRow {
    pub columns: Vec<String>,
}

impl DateFeatureRow {
    /// Space-joined row, the exact payload `map_input` consumes per token.
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    /// Field count; always [`NATIVE_DATE_FEATURE_COLUMNS`].
    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

/// One grouped name part: a maximal run of equal name base labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeNamePart {
    /// Typed role of the run.
    pub role: NamePartRole,
    /// Base label spelling (`<forename>`), never the `I-` spelling.
    pub label: String,
    /// Half-open row range inside the nested cluster's token list.
    pub start: usize,
    pub end: usize,
    /// Half-open byte range inside [`NativeNameCitation::name_text`].
    pub byte_start: usize,
    pub byte_end: usize,
    /// Member token texts joined by a single space.
    pub text: String,
}

/// A decoded `name/citation` cluster projected into typed name parts.
///
/// There is deliberately no identifier field: an identifier is not a name part,
/// and H10-N1 requires that an absent identifier has nowhere to appear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeNameCitation {
    /// One decoded label per nested row, in reading order.
    pub labels: Vec<String>,
    /// Column-0 token texts joined by one space — the byte-span coordinate system.
    pub name_text: String,
    /// Grouped name parts, in reading order.
    pub parts: Vec<NativeNamePart>,
}

/// One grouped date part: a maximal run of equal date base labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDatePart {
    /// Typed role of the run.
    pub role: DatePartRole,
    /// Base label spelling (`<year>`), never the `I-` spelling.
    pub label: String,
    /// Half-open row range inside the nested cluster's token list.
    pub start: usize,
    pub end: usize,
    /// Half-open byte range inside [`NativeDateValue::date_text`].
    pub byte_start: usize,
    pub byte_end: usize,
    /// Member token texts joined by a single space.
    pub text: String,
}

/// A decoded `date` cluster projected into typed date parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDateValue {
    /// One decoded label per nested row, in reading order.
    pub labels: Vec<String>,
    /// Column-0 token texts joined by one space — the byte-span coordinate system.
    pub date_text: String,
    /// Grouped date parts, in reading order.
    pub parts: Vec<NativeDatePart>,
}

/// Identifier evidence read from a citation cluster's `isKnownIdentifier` lane.
///
/// H10-N1: the lane is evidence, never a synthesis licence. [`cluster_identifier`]
/// returns `Ok(None)` when no row of the cluster carries it, and never derives an
/// identifier from a token's shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeIdentifierEvidence {
    /// Absolute row index inside the citation block the cluster came from.
    pub row: usize,
    /// Half-open byte range inside the citation cluster text
    /// ([`crate::native_bibliography_sequence::CitationClusterSequence::cluster_text`]).
    pub byte_start: usize,
    pub byte_end: usize,
    /// Column-0 text of the row that carries the lane.
    pub text: String,
}

/// Typed refusal vocabulary for the nested bibliography families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBibliographyNestedError {
    /// The token list is empty, so there is no block to render.
    EmptyInput,
    /// A token text is empty; `printVector` would return no row at all.
    EmptyToken { token: usize },
    /// A token text contains whitespace, which would corrupt the row arity.
    TokenContainsWhitespace { token: usize },
    /// The provider bundle behind the lexical lanes failed validation.
    InvalidLexicon(String),
    /// A rendered row is not the width the family declares.
    WrongArity { actual: usize, expected: usize },
    /// The person-title coverage describes a different token count than the block.
    TitleCoverageMismatch { coverage: usize, tokens: usize },
    /// The cluster is not an author/editor cluster, so the name family does not
    /// apply to it.
    NotANameCluster { role: CitationRole },
    /// The cluster is not a date cluster, so the date family does not apply.
    NotADateCluster { role: CitationRole },
    /// The nested rows do not correspond one-to-one with the cluster's rows.
    ClusterRowMismatch {
        span_rows: usize,
        nested_rows: usize,
    },
    /// The cluster's row range is not inside the citation rows handed over.
    ClusterRowsOutOfRange {
        span_start: usize,
        span_end: usize,
        rows: usize,
    },
    /// A run's base label is outside the family's closed role set.
    UnknownBaseLabel { label: String, row: usize },
    /// The nested decode runner refused the rows or the model.
    Sequence(NativeBibliographySequenceError),
}

impl fmt::Display for NativeBibliographyNestedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "nested bibliography input is empty"),
            Self::EmptyToken { token } => {
                write!(formatter, "nested bibliography token {token} is empty")
            }
            Self::TokenContainsWhitespace { token } => write!(
                formatter,
                "nested bibliography token {token} contains whitespace"
            ),
            Self::InvalidLexicon(error) => {
                write!(formatter, "lexicon bundle is invalid: {error}")
            }
            Self::WrongArity { actual, expected } => write!(
                formatter,
                "rendered nested bibliography row has {actual} columns; expected {expected}"
            ),
            Self::TitleCoverageMismatch { coverage, tokens } => write!(
                formatter,
                "person-title coverage describes {coverage} tokens; the block has {tokens}"
            ),
            Self::NotANameCluster { role } => write!(
                formatter,
                "citation role {} is not an author/editor cluster",
                role.as_str()
            ),
            Self::NotADateCluster { role } => write!(
                formatter,
                "citation role {} is not a date cluster",
                role.as_str()
            ),
            Self::ClusterRowMismatch {
                span_rows,
                nested_rows,
            } => write!(
                formatter,
                "citation cluster covers {span_rows} rows; {nested_rows} nested rows were supplied"
            ),
            Self::ClusterRowsOutOfRange {
                span_start,
                span_end,
                rows,
            } => write!(
                formatter,
                "citation cluster rows {span_start}..{span_end} are outside the block's {rows} rows"
            ),
            Self::UnknownBaseLabel { label, row } => write!(
                formatter,
                "nested bibliography run {label:?} at row {row} is outside the family's roles"
            ),
            Self::Sequence(error) => {
                write!(formatter, "nested bibliography decode failed: {error}")
            }
        }
    }
}

impl std::error::Error for NativeBibliographyNestedError {}

/// Build the exact 18-column `name/citation` rows for one cluster's tokens.
///
/// Returns one row per token, in input order. `title` must describe the *same*
/// token list: the lane is positional, and a misaligned coverage would silently
/// flag the wrong tokens, so a length disagreement is a typed refusal.
pub fn name_citation_row(
    tokens: &[BibliographyTokenFacts],
    title: &NativePersonTitleCoverage,
    lexicon: &LexiconBundle,
) -> Result<Vec<NameCitationFeatureRow>, NativeBibliographyNestedError> {
    if tokens.is_empty() {
        return Err(NativeBibliographyNestedError::EmptyInput);
    }
    lexicon
        .validate()
        .map_err(|error| NativeBibliographyNestedError::InvalidLexicon(error.to_string()))?;
    if title.len() != tokens.len() {
        return Err(NativeBibliographyNestedError::TitleCoverageMismatch {
            coverage: title.len(),
            tokens: tokens.len(),
        });
    }
    let mut rows = Vec::with_capacity(tokens.len());
    for (position, token) in tokens.iter().enumerate() {
        validate_token(position, token)?;
        let flags = name_flags(&token.text, lexicon);
        let columns = vec![
            // 0: normalised token text (normalisation is the producer's job).
            token.text.clone(),
            // 1: lowercase string.
            token.lowercase.clone(),
            // 2-5: prefix 1..4.
            token.prefix[0].clone(),
            token.prefix[1].clone(),
            token.prefix[2].clone(),
            token.prefix[3].clone(),
            // 6-9: suffix 1..4.
            token.suffix[0].clone(),
            token.suffix[1].clone(),
            token.suffix[2].clone(),
            token.suffix[3].clone(),
            // 10: lineStatus.
            token.line_status.as_str().to_string(),
            // 11: capitalisation as printed (`NOCAPS` when `digit == ALLDIGIT`).
            token.printed_capitalisation().as_str().to_string(),
            // 12: digit.
            token.digit.as_str().to_string(),
            // 13: singleChar.
            bool_flag(token.single_char),
            // 14: commonName — the *name* family's lexical block starts here,
            // not the citation family's `properName`.
            bool_flag(flags.common_name),
            // 15: firstName.
            bool_flag(flags.first_name),
            // 16: lastName.
            bool_flag(flags.last_name),
            // 17: isKnownTitle — person-title lane coverage (span predicate).
            bool_flag(title.covers(position)),
        ];
        let row = NameCitationFeatureRow { columns };
        if row.width() != NATIVE_NAME_CITATION_FEATURE_COLUMNS {
            return Err(NativeBibliographyNestedError::WrongArity {
                actual: row.width(),
                expected: NATIVE_NAME_CITATION_FEATURE_COLUMNS,
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Build the exact 17-column `date` rows for one cluster's tokens.
pub fn date_row(
    tokens: &[BibliographyTokenFacts],
    lexicon: &LexiconBundle,
) -> Result<Vec<DateFeatureRow>, NativeBibliographyNestedError> {
    if tokens.is_empty() {
        return Err(NativeBibliographyNestedError::EmptyInput);
    }
    lexicon
        .validate()
        .map_err(|error| NativeBibliographyNestedError::InvalidLexicon(error.to_string()))?;
    let mut rows = Vec::with_capacity(tokens.len());
    for (position, token) in tokens.iter().enumerate() {
        validate_token(position, token)?;
        let columns = vec![
            token.text.clone(),
            token.lowercase.clone(),
            token.prefix[0].clone(),
            token.prefix[1].clone(),
            token.prefix[2].clone(),
            token.prefix[3].clone(),
            token.suffix[0].clone(),
            token.suffix[1].clone(),
            token.suffix[2].clone(),
            token.suffix[3].clone(),
            // 10: lineStatus.
            token.line_status.as_str().to_string(),
            // 11: capitalisation as printed.
            token.printed_capitalisation().as_str().to_string(),
            // 12: digit.
            token.digit.as_str().to_string(),
            // 13: singleChar.
            bool_flag(token.single_char),
            // 14: year.
            bool_flag(token.year),
            // 15: month.
            bool_flag(token.month),
            // 16: punctType.
            token.punct_type.as_str().to_string(),
        ];
        let row = DateFeatureRow { columns };
        if row.width() != NATIVE_DATE_FEATURE_COLUMNS {
            return Err(NativeBibliographyNestedError::WrongArity {
                actual: row.width(),
                expected: NATIVE_DATE_FEATURE_COLUMNS,
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Serialize `name/citation` rows exactly as `map_input` expects.
pub fn render_name_citation_feature_text(
    rows: &[NameCitationFeatureRow],
) -> Result<String, NativeBibliographyNestedError> {
    render_rows(
        rows.iter().map(NameCitationFeatureRow::as_text).collect(),
        rows.iter().map(NameCitationFeatureRow::width).collect(),
        NATIVE_NAME_CITATION_FEATURE_COLUMNS,
    )
}

/// Serialize `date` rows exactly as `map_input` expects.
pub fn render_date_feature_text(
    rows: &[DateFeatureRow],
) -> Result<String, NativeBibliographyNestedError> {
    render_rows(
        rows.iter().map(DateFeatureRow::as_text).collect(),
        rows.iter().map(DateFeatureRow::width).collect(),
        NATIVE_DATE_FEATURE_COLUMNS,
    )
}

/// Decode one `<author>`/`<editor>` citation cluster through `name/citation`.
///
/// `rows` must be the nested rows for exactly the tokens the cluster covers, so
/// `rows.len()` has to equal `family_span.end - family_span.start`; a mismatch is
/// refused rather than silently decoding the wrong token list. The decode itself
/// goes through S04/T04's runner, which also refuses a model whose declared label
/// block is not the family's expected [`NATIVE_NAME_CITATION_LABEL_COUNT`].
pub fn decode_name_citation_cluster(
    family_span: &CitationRoleSpan,
    rows: &[NameCitationFeatureRow],
    model: &CrfModel,
) -> Result<NativeNameCitation, NativeBibliographyNestedError> {
    let expected = cluster_row_count(family_span)?;
    if rows.len() != expected {
        return Err(NativeBibliographyNestedError::ClusterRowMismatch {
            span_rows: expected,
            nested_rows: rows.len(),
        });
    }
    if !matches!(
        family_span.role,
        CitationRole::Author | CitationRole::Editor
    ) {
        return Err(NativeBibliographyNestedError::NotANameCluster {
            role: family_span.role,
        });
    }
    let row_texts: Vec<String> = rows.iter().map(NameCitationFeatureRow::as_text).collect();
    let decoded =
        decode_bibliography_labels(&row_texts, &NATIVE_NAME_CITATION_SEQUENCE_FAMILY, model)
            .map_err(NativeBibliographyNestedError::Sequence)?;
    let mut parts = Vec::new();
    for run in runs(&decoded)? {
        let Some(role) = NamePartRole::from_base_label(&run.label) else {
            return Err(NativeBibliographyNestedError::UnknownBaseLabel {
                label: run.label,
                row: run.start,
            });
        };
        parts.push(NativeNamePart {
            role,
            label: run.label,
            start: run.start,
            end: run.end,
            byte_start: run.byte_start,
            byte_end: run.byte_end,
            text: run.text,
        });
    }
    Ok(NativeNameCitation {
        labels: decoded.labels,
        name_text: decoded.text,
        parts,
    })
}

/// Decode one `<date>` citation cluster through `date`.
pub fn decode_date_cluster(
    family_span: &CitationRoleSpan,
    rows: &[DateFeatureRow],
    model: &CrfModel,
) -> Result<NativeDateValue, NativeBibliographyNestedError> {
    let expected = cluster_row_count(family_span)?;
    if rows.len() != expected {
        return Err(NativeBibliographyNestedError::ClusterRowMismatch {
            span_rows: expected,
            nested_rows: rows.len(),
        });
    }
    if family_span.role != CitationRole::Date {
        return Err(NativeBibliographyNestedError::NotADateCluster {
            role: family_span.role,
        });
    }
    let row_texts: Vec<String> = rows.iter().map(DateFeatureRow::as_text).collect();
    let decoded = decode_bibliography_labels(&row_texts, &NATIVE_DATE_SEQUENCE_FAMILY, model)
        .map_err(NativeBibliographyNestedError::Sequence)?;
    let mut parts = Vec::new();
    for run in runs(&decoded)? {
        let Some(role) = DatePartRole::from_base_label(&run.label) else {
            return Err(NativeBibliographyNestedError::UnknownBaseLabel {
                label: run.label,
                row: run.start,
            });
        };
        parts.push(NativeDatePart {
            role,
            label: run.label,
            start: run.start,
            end: run.end,
            byte_start: run.byte_start,
            byte_end: run.byte_end,
            text: run.text,
        });
    }
    Ok(NativeDateValue {
        labels: decoded.labels,
        date_text: decoded.text,
        parts,
    })
}

/// Read the identifier evidence of one citation cluster, or `None` when absent.
///
/// H10-N1 in one function: the answer comes from the row's `isKnownIdentifier`
/// lane ([`NATIVE_CITATION_IDENTIFIER_COLUMN`]) and from nothing else, so a
/// token that merely *looks* like a DOI cannot become an identifier. The byte
/// range indexes the citation cluster text, exactly like
/// [`CitationRoleSpan::byte_start`].
pub fn cluster_identifier(
    rows: &[CitationFeatureRow],
    family_span: &CitationRoleSpan,
) -> Result<Option<NativeIdentifierEvidence>, NativeBibliographyNestedError> {
    let _ = cluster_row_count(family_span)?;
    if family_span.end > rows.len() {
        return Err(NativeBibliographyNestedError::ClusterRowsOutOfRange {
            span_start: family_span.start,
            span_end: family_span.end,
            rows: rows.len(),
        });
    }
    for (offset, row) in rows[family_span.start..family_span.end].iter().enumerate() {
        if row.width() < NATIVE_CITATION_FEATURE_COLUMNS {
            return Err(NativeBibliographyNestedError::WrongArity {
                actual: row.width(),
                expected: NATIVE_CITATION_FEATURE_COLUMNS,
            });
        }
        if row.columns[NATIVE_CITATION_IDENTIFIER_COLUMN] != "1" {
            continue;
        }
        let (byte_start, byte_end) = member_byte_range(family_span, offset).ok_or(
            NativeBibliographyNestedError::ClusterRowsOutOfRange {
                span_start: family_span.start,
                span_end: family_span.end,
                rows: rows.len(),
            },
        )?;
        return Ok(Some(NativeIdentifierEvidence {
            row: family_span.start + offset,
            byte_start,
            byte_end,
            text: row.columns[0].clone(),
        }));
    }
    Ok(None)
}

/// The exact width both row builders must produce, checked against the model
/// headers' `#rdr#` referenced-column count.
pub const NATIVE_NAME_CITATION_REFERENCED_COLUMNS: usize =
    NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX + 1;
/// The date family's equivalent: `date` references columns `0..=16`.
pub const NATIVE_DATE_REFERENCED_COLUMNS: usize = NATIVE_DATE_MODEL_MAX_COLUMN_INDEX + 1;

/// The training-label placeholder neither family references, kept as the shared
/// constant so a future family cannot invent a different spelling.
pub const NATIVE_NESTED_UNREFERENCED_LABEL_PLACEHOLDER: &str = NATIVE_TRAINING_LABEL_PLACEHOLDER;

// The referenced subsets are strictly narrower than `printVector`'s field counts
// (21 for `name/citation`, 18 for `date`): the limitation this module documents is
// a compile-time invariant, not a hope.
const _: () = assert!(NATIVE_NAME_CITATION_FEATURE_COLUMNS < 21);
const _: () = assert!(NATIVE_DATE_FEATURE_COLUMNS < 18);
const _: () = assert!(NATIVE_NAME_CITATION_REFERENCED_COLUMNS < NATIVE_CITATION_FEATURE_COLUMNS);

fn cluster_row_count(
    family_span: &CitationRoleSpan,
) -> Result<usize, NativeBibliographyNestedError> {
    family_span
        .end
        .checked_sub(family_span.start)
        .filter(|count| *count > 0)
        .ok_or(NativeBibliographyNestedError::ClusterRowsOutOfRange {
            span_start: family_span.start,
            span_end: family_span.end,
            rows: family_span.end.saturating_sub(family_span.start),
        })
}

fn runs(
    decoded: &crate::native_bibliography_sequence::BibliographyLabelSequence,
) -> Result<Vec<BibliographyLabelRun>, NativeBibliographyNestedError> {
    bibliography_label_runs(decoded).map_err(NativeBibliographyNestedError::Sequence)
}

/// Byte range of member `member` inside the cluster text, using the span's own
/// reconstructed text so the caller never re-derives offsets.
fn member_byte_range(span: &CitationRoleSpan, member: usize) -> Option<(usize, usize)> {
    let mut cursor = span.byte_start;
    for (index, token) in span.text.split(' ').enumerate() {
        let end = cursor + token.len();
        if index == member {
            return Some((cursor, end));
        }
        cursor = end + 1;
    }
    None
}

fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.into()
}

/// A token the row format cannot carry is a typed refusal, never a silent skip:
/// an empty text would emit no row at all (`printVector` returns `null`) and a
/// whitespace character would shift every following field.
fn validate_token(
    index: usize,
    token: &BibliographyTokenFacts,
) -> Result<(), NativeBibliographyNestedError> {
    if token.text.is_empty() {
        return Err(NativeBibliographyNestedError::EmptyToken { token: index });
    }
    if token.text.chars().any(char::is_whitespace) {
        return Err(NativeBibliographyNestedError::TokenContainsWhitespace { token: index });
    }
    Ok(())
}

fn render_rows(
    texts: Vec<String>,
    widths: Vec<usize>,
    expected: usize,
) -> Result<String, NativeBibliographyNestedError> {
    for width in widths {
        if width != expected {
            return Err(NativeBibliographyNestedError::WrongArity {
                actual: width,
                expected,
            });
        }
    }
    Ok(texts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_feature_facts::{
        BibliographyAlignment, BibliographyTokenInput, FactsBlockStatus, FactsLineStatus,
    };
    use da_ports::{FeatureTemplate, LexiconChannel};

    fn fixture_lexicon() -> LexiconBundle {
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: vec![
                LexiconChannel {
                    key: "commonName".into(),
                    entries: vec!["google".into()],
                },
                LexiconChannel {
                    key: "firstName".into(),
                    entries: vec!["ashish".into()],
                },
                LexiconChannel {
                    key: "lastName".into(),
                    entries: vec!["vaswani".into()],
                },
            ],
            feature_templates: vec![FeatureTemplate {
                key: "name".into(),
                channel_keys: vec!["firstName".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }

    fn facts(text: &str, index: usize, count: usize) -> BibliographyTokenFacts {
        BibliographyTokenFacts::from_token(&BibliographyTokenInput {
            text: text.to_string(),
            block_status: match index {
                0 => FactsBlockStatus::Blockstart,
                _ if index + 1 == count => FactsBlockStatus::Blockend,
                _ => FactsBlockStatus::Blockin,
            },
            line_status: match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            },
            alignment: BibliographyAlignment::Alignedleft,
            line_length: 20,
            superscript: false,
        })
    }

    fn block(tokens: &[&str]) -> Vec<BibliographyTokenFacts> {
        tokens
            .iter()
            .enumerate()
            .map(|(index, text)| facts(text, index, tokens.len()))
            .collect()
    }

    fn fields(columns: &[String]) -> Vec<&str> {
        columns.iter().map(String::as_str).collect()
    }

    #[test]
    fn name_row_emits_the_referenced_eighteen_column_subset() {
        let lexicon = fixture_lexicon();
        let tokens = block(&["Ashish", "Vaswani"]);
        let title = NativePersonTitleCoverage::from_positions(tokens.len(), &[]);
        let rows = name_citation_row(&tokens, &title, &lexicon).expect("fixture renders");
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| row.width() == NATIVE_NAME_CITATION_FEATURE_COLUMNS),
            "the referenced subset is 18 columns, not printVector's 21"
        );

        let first = fields(&rows[0].columns);
        assert_eq!(
            first[..11],
            [
                "Ashish",
                "ashish",
                "A",
                "As",
                "Ash",
                "Ashi",
                "h",
                "sh",
                "ish",
                "hish",
                "LINESTART"
            ]
        );
        assert_eq!(first[11], "INITCAP");
        assert_eq!(first[12], "NODIGIT");
        assert_eq!(first[13], "0");
        // 14 is `commonName` (0 for `Ashish`), 15 is `firstName` (1): the name
        // family's block is not the citation family's `properName` block.
        assert_eq!(first[14..18], ["0", "1", "0", "0"]);

        let second = fields(&rows[1].columns);
        assert_eq!(second[14..18], ["0", "0", "1", "0"]);
        assert_eq!(second[10], "LINEEND");
    }

    #[test]
    fn name_row_capitalisation_is_the_printed_value_and_title_is_a_span_predicate() {
        let lexicon = fixture_lexicon();
        let tokens = block(&["2017", "Professor", "Emeritus"]);
        // `Professor Emeritus` is one multi-word title span: both tokens are
        // covered, the year before it is not.
        let title = NativePersonTitleCoverage::from_positions(3, &[(1, 2)]);
        let rows = name_citation_row(&tokens, &title, &lexicon).expect("fixture renders");
        let columns: Vec<&str> = fields(&rows[0].columns);
        assert_eq!(columns[11], "NOCAPS", "ALLDIGIT prints NOCAPS");
        assert_eq!(columns[12], "ALLDIGIT");
        assert_eq!(columns[17], "0");
        assert_eq!(fields(&rows[1].columns)[17], "1");
        assert_eq!(fields(&rows[2].columns)[17], "1");
        // A coverage built from the same span without the phrase is a different
        // value: the predicate is positional, not a membership lookup.
        assert!(title.covers(1) && title.covers(2) && !title.covers(0));
    }

    #[test]
    fn date_row_emits_the_referenced_seventeen_column_subset() {
        let lexicon = fixture_lexicon();
        let tokens = block(&["June", "12", "2017"]);
        let rows = date_row(&tokens, &lexicon).expect("fixture renders");
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .all(|row| row.width() == NATIVE_DATE_FEATURE_COLUMNS),
            "the referenced subset is 17 columns"
        );
        let june = fields(&rows[0].columns);
        assert_eq!(june[10], "LINESTART");
        assert_eq!(june[11], "INITCAP");
        assert_eq!(june[12], "NODIGIT");
        assert_eq!(&june[14..17], ["0", "1", "NOPUNCT"]);
        let twelve = fields(&rows[1].columns);
        assert_eq!(&twelve[14..17], ["0", "0", "NOPUNCT"]);
        assert_eq!(twelve[12], "ALLDIGIT");
        let year = fields(&rows[2].columns);
        assert_eq!(&year[14..17], ["1", "0", "NOPUNCT"]);
        assert_eq!(year[11], "NOCAPS");
        assert_eq!(year[10], "LINEEND");
        // A punctuated token prints its punctuation class in the last column.
        let punct = date_row(&block(&[","]), &lexicon).expect("fixture renders");
        assert_eq!(fields(&punct[0].columns)[16], "COMMA");
    }

    #[test]
    fn row_builders_refuse_malformed_inputs() {
        let lexicon = fixture_lexicon();
        let empty_coverage = NativePersonTitleCoverage::default();
        assert_eq!(
            name_citation_row(&[], &empty_coverage, &lexicon),
            Err(NativeBibliographyNestedError::EmptyInput)
        );
        assert_eq!(
            date_row(&[], &lexicon),
            Err(NativeBibliographyNestedError::EmptyInput)
        );

        let tokens = block(&["Ashish", "Vaswani"]);
        // A coverage for a different token count would silently shift the lane.
        let short = NativePersonTitleCoverage::from_positions(1, &[(0, 0)]);
        assert_eq!(
            name_citation_row(&tokens, &short, &lexicon),
            Err(NativeBibliographyNestedError::TitleCoverageMismatch {
                coverage: 1,
                tokens: 2
            })
        );

        let empty = block(&[""]);
        assert_eq!(
            name_citation_row(
                &empty,
                &NativePersonTitleCoverage::from_positions(1, &[]),
                &lexicon
            ),
            Err(NativeBibliographyNestedError::EmptyToken { token: 0 })
        );
        assert_eq!(
            date_row(&empty, &lexicon),
            Err(NativeBibliographyNestedError::EmptyToken { token: 0 })
        );

        let spaced = block(&["bad token"]);
        assert_eq!(
            name_citation_row(
                &spaced,
                &NativePersonTitleCoverage::from_positions(1, &[]),
                &lexicon
            ),
            Err(NativeBibliographyNestedError::TokenContainsWhitespace { token: 0 })
        );
        assert_eq!(
            date_row(&spaced, &lexicon),
            Err(NativeBibliographyNestedError::TokenContainsWhitespace { token: 0 })
        );
    }

    #[test]
    fn row_builders_refuse_an_invalid_lexicon() {
        let mut lexicon = fixture_lexicon();
        lexicon.schema_version = "grobid-lexicon.v2".into();
        let tokens = block(&["Ashish"]);
        assert!(matches!(
            name_citation_row(
                &tokens,
                &NativePersonTitleCoverage::from_positions(1, &[]),
                &lexicon
            ),
            Err(NativeBibliographyNestedError::InvalidLexicon(_))
        ));
        assert!(matches!(
            date_row(&tokens, &lexicon),
            Err(NativeBibliographyNestedError::InvalidLexicon(_))
        ));
    }

    #[test]
    fn title_coverage_is_inclusive_and_ignores_out_of_range_positions() {
        let coverage = NativePersonTitleCoverage::from_positions(4, &[(1, 2), (3, 1), (99, 120)]);
        assert_eq!(coverage.len(), 4);
        assert!(coverage.covers(1) && coverage.covers(2));
        assert!(!coverage.covers(0) && !coverage.covers(3));
        assert!(!coverage.covers(7), "out-of-range positions are absent");
        let no_spans = NativePersonTitleCoverage::from_positions(2, &[]);
        assert_eq!(no_spans.len(), 2);
        assert!(
            !no_spans.covers(0) && !no_spans.covers(1),
            "an empty lane is typed missingness, not a fabricated flag"
        );
        assert!(NativePersonTitleCoverage::from_positions(0, &[(0, 3)]).is_empty());
    }

    #[test]
    fn role_tables_are_the_model_label_tables_up_to_the_spelling() {
        let mut name_bases: Vec<&str> = NATIVE_NAME_CITATION_LABELS
            .iter()
            .map(|label| label.strip_prefix("I-").unwrap_or(label))
            .collect();
        name_bases.sort_unstable();
        name_bases.dedup();
        let mut role_bases: Vec<&str> = NAME_PART_BASE_LABELS.to_vec();
        role_bases.sort_unstable();
        assert_eq!(name_bases, role_bases);
        assert_eq!(NamePartRole::ALL.len(), NAME_PART_BASE_LABELS.len());
        for role in NamePartRole::ALL {
            assert_eq!(NamePartRole::from_base_label(role.base_label()), Some(role));
            // `<suffix>` appears only in its `I-` spelling in the model table, so
            // the comparison is on base names, not on the raw labels.
            assert!(
                NATIVE_NAME_CITATION_LABELS
                    .iter()
                    .any(|label| label.strip_prefix("I-").unwrap_or(label) == role.base_label()),
                "role {} has no model base label",
                role.as_str()
            );
        }
        assert!(NamePartRole::from_base_label("<not-a-role>").is_none());

        let mut date_bases: Vec<&str> = NATIVE_DATE_LABELS
            .iter()
            .map(|label| label.strip_prefix("I-").unwrap_or(label))
            .collect();
        date_bases.sort_unstable();
        date_bases.dedup();
        let mut date_roles: Vec<&str> = DATE_PART_BASE_LABELS.to_vec();
        date_roles.sort_unstable();
        assert_eq!(date_bases, date_roles);
        for role in DatePartRole::ALL {
            assert_eq!(DatePartRole::from_base_label(role.base_label()), Some(role));
        }
        assert!(DatePartRole::from_base_label("<not-a-role>").is_none());
    }

    /// The two widths are the models' referenced-column counts, not
    /// `printVector`'s field counts: 18 and 17, not 21 and 18.
    #[test]
    fn declared_widths_are_the_referenced_column_counts() {
        assert_eq!(NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX, 17);
        assert_eq!(NATIVE_NAME_CITATION_FEATURE_COLUMNS, 18);
        assert_eq!(
            NATIVE_NAME_CITATION_FEATURE_COLUMNS,
            NATIVE_NAME_CITATION_REFERENCED_COLUMNS
        );
        assert_eq!(NATIVE_DATE_MODEL_MAX_COLUMN_INDEX, 16);
        assert_eq!(NATIVE_DATE_FEATURE_COLUMNS, 17);
        assert_eq!(NATIVE_DATE_FEATURE_COLUMNS, NATIVE_DATE_REFERENCED_COLUMNS);
        assert_eq!(
            NATIVE_NAME_CITATION_LABELS.len(),
            NATIVE_NAME_CITATION_LABEL_COUNT
        );
        assert_eq!(NATIVE_DATE_LABELS.len(), NATIVE_DATE_LABEL_COUNT);
        assert_eq!(NATIVE_NESTED_UNREFERENCED_LABEL_PLACEHOLDER, "0");
        assert_eq!(
            NATIVE_BIBLIOGRAPHY_NESTED_SCHEMA_VERSION,
            "da-parse.native-bibliography-nested/v1"
        );
    }

    #[test]
    fn renderers_refuse_a_row_of_the_wrong_width() {
        let name = NameCitationFeatureRow {
            columns: vec!["0".into()],
        };
        assert_eq!(
            render_name_citation_feature_text(&[name]),
            Err(NativeBibliographyNestedError::WrongArity {
                actual: 1,
                expected: NATIVE_NAME_CITATION_FEATURE_COLUMNS
            })
        );
        let date = DateFeatureRow {
            columns: vec!["0".into(); NATIVE_DATE_FEATURE_COLUMNS + 1],
        };
        assert_eq!(
            render_date_feature_text(&[date]),
            Err(NativeBibliographyNestedError::WrongArity {
                actual: NATIVE_DATE_FEATURE_COLUMNS + 1,
                expected: NATIVE_DATE_FEATURE_COLUMNS
            })
        );
    }

    #[test]
    fn decoders_refuse_an_empty_cluster_span() {
        let span = CitationRoleSpan {
            role: CitationRole::Author,
            label: "<author>".into(),
            start: 3,
            end: 3,
            byte_start: 0,
            byte_end: 0,
            text: String::new(),
        };
        assert!(matches!(
            cluster_identifier(&[], &span),
            Err(NativeBibliographyNestedError::ClusterRowsOutOfRange { .. })
        ));
    }
}
