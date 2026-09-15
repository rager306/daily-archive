//! Pure sequence runner for the native GROBID bibliography CRFs.
//!
//! Layout providers own model and row I/O. This module only validates the
//! already-rendered rows, maps them through a supplied scalar CRF model, and
//! groups the emitted labels into bounded spans — the same division of labour as
//! [`crate::native_header_sequence`]. It opens no file, reads no environment
//! variable, and has no fallback producer.
//!
//! ## Two families, one grouping rule
//!
//! * `reference-segmenter/model.wapiti` — 6 labels over one line of a
//!   bibliography block; the three boundary kinds are `<label>`, `<reference>`
//!   and `<other>`;
//! * `citation/model.wapiti` — 38 labels over one citation block; the 19 roles
//!   (`author`, `editor`, …, `other`) each carry a begin and an inside spelling.
//!
//! Both group by **base-name equality**. `I-` is a *spelling* of a label, not a
//! continuation marker: a run continues while
//! `base(current) == base(previous)` — where `base` strips the `I-` prefix — and
//! nothing else opens or closes one. That is the rule `507-03-RESEARCH.md` §3 got
//! backwards for the header family (D359 / MEM1372 / MEM1373: the real 0.9.0
//! header model emits `I-<author>` followed by nine bare `<author>` rows as one
//! author line). The bibliography families inherit the corrected rule
//! deliberately: under base-name equality no continuation can be invalid, so
//! this module has no "invalid continuation" refusal to write.
//!
//! ## Byte spans
//!
//! Every span carries its half-open row range and the half-open byte range of
//! the same span inside the **reconstructed sequence text** — the column-0 token
//! texts joined by one space, reported as [`BibliographyLabelSequence::text`],
//! [`ReferenceSegmentSequence::line_text`] or
//! [`CitationClusterSequence::cluster_text`]. The two are consistent by
//! construction: `&text[byte_start..byte_end] == span.text`. A consumer that
//! needs provenance into a reference line therefore never re-derives offsets and
//! never guesses which separator was used.
//!
//! ## What this runner does *not* prove
//!
//! The declared label *block* is checked for cardinality
//! ([`NativeBibliographySequenceError::WrongLabelCount`]) and every decoded label
//! is checked against the family's closed set
//! ([`NativeBibliographySequenceError::UnknownLabel`]). Binding a real model's
//! declared block to the transcribed order is the opt-in real-model harness's
//! job (S04/T07) — this runner decodes whatever frozen model the caller hands it
//! and never claims more.
//!
//! ## Determinism
//!
//! Equal rows and equal model values always produce equal output. Weight
//! selection, tie-breaking and run detection are all pure functions of the
//! inputs; there is no clock, no iteration over a hash map, and no I/O.

use crate::native_bibliography_features::{
    CitationFeatureRow, NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_CITATION_LABELS,
    NATIVE_CITATION_ROLE_COUNT, NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
    NATIVE_REFERENCE_SEGMENTER_LABELS, ReferenceSegmenterFeatureRow,
};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NATIVE_BIBLIOGRAPHY_SEQUENCE_SCHEMA_VERSION: &str =
    "da-parse.native-bibliography-sequence/v1";

/// Rows admitted into one decode, mirroring the header runner's bound.
pub const MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS: usize = 32_768;

/// One CRF family's decode contract: the printed row width and the closed label
/// set the model may emit.
///
/// The label table is in *model order*. This runner uses it as the admissible
/// decoded-label vocabulary (and for its cardinality); a family added later
/// (name/date, S04/T05) supplies its own table rather than editing this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BibliographySequenceFamily {
    /// Stable family name, used in refusal messages.
    pub name: &'static str,
    /// Printed feature-row width the family's rows must present.
    pub expected_columns: usize,
    /// The family's closed label set, in model order.
    pub admissible_labels: &'static [&'static str],
}

/// `reference-segmenter/model.wapiti`: 6 labels, 28-column rows.
pub const NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY: BibliographySequenceFamily =
    BibliographySequenceFamily {
        name: "reference-segmenter",
        expected_columns: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
        admissible_labels: &NATIVE_REFERENCE_SEGMENTER_LABELS,
    };

/// `citation/model.wapiti`: 38 labels, 30-column rows.
pub const NATIVE_CITATION_SEQUENCE_FAMILY: BibliographySequenceFamily =
    BibliographySequenceFamily {
        name: "citation",
        expected_columns: NATIVE_CITATION_FEATURE_COLUMNS,
        admissible_labels: &NATIVE_CITATION_LABELS,
    };

/// The three boundary kinds the reference segmenter distinguishes, mirroring the
/// three base labels of its 6-label model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSegmentKind {
    /// `<label>` — the in-text marker (`[12]`, `1.`), the ordinal source for the
    /// callout label map.
    Label,
    /// `<reference>` — the reference body.
    Reference,
    /// `<other>` — anything the segmenter keeps outside both.
    Other,
}

impl ReferenceSegmentKind {
    /// Every kind, in the order the base labels appear in the model table.
    pub const ALL: [ReferenceSegmentKind; 3] = [Self::Other, Self::Label, Self::Reference];

    /// Wire spelling used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::Reference => "reference",
            Self::Other => "other",
        }
    }

    /// The base label (angle brackets included) this kind groups.
    pub const fn base_label(self) -> &'static str {
        match self {
            Self::Label => "<label>",
            Self::Reference => "<reference>",
            Self::Other => "<other>",
        }
    }

    /// Classify a base label. `None` for anything outside the closed set; the
    /// caller turns that into a typed refusal rather than a default.
    pub fn from_base_label(base: &str) -> Option<Self> {
        match base {
            "<label>" => Some(Self::Label),
            "<reference>" => Some(Self::Reference),
            "<other>" => Some(Self::Other),
            _ => None,
        }
    }
}

/// The 19 citation roles of `citation/model.wapiti`, in the order the reference
/// implementation enumerates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationRole {
    Author,
    Editor,
    Title,
    Journal,
    Booktitle,
    Publisher,
    Institution,
    Series,
    Volume,
    Issue,
    Pages,
    Location,
    Date,
    Web,
    Tech,
    Pubnum,
    Collaboration,
    Note,
    Other,
}

/// Base-label spellings of [`CitationRole::ALL`], index-for-index.
///
/// Kept as a separate table — rather than a `format!` in `base_label` — so the
/// role/bracket correspondence is one greppable literal and cheap to assert
/// against `NATIVE_CITATION_LABELS`.
pub const CITATION_ROLE_BASE_LABELS: [&str; NATIVE_CITATION_ROLE_COUNT] = [
    "<author>",
    "<editor>",
    "<title>",
    "<journal>",
    "<booktitle>",
    "<publisher>",
    "<institution>",
    "<series>",
    "<volume>",
    "<issue>",
    "<pages>",
    "<location>",
    "<date>",
    "<web>",
    "<tech>",
    "<pubnum>",
    "<collaboration>",
    "<note>",
    "<other>",
];

impl CitationRole {
    /// Every role, in the order [`CITATION_ROLE_BASE_LABELS`] spells.
    pub const ALL: [CitationRole; NATIVE_CITATION_ROLE_COUNT] = [
        Self::Author,
        Self::Editor,
        Self::Title,
        Self::Journal,
        Self::Booktitle,
        Self::Publisher,
        Self::Institution,
        Self::Series,
        Self::Volume,
        Self::Issue,
        Self::Pages,
        Self::Location,
        Self::Date,
        Self::Web,
        Self::Tech,
        Self::Pubnum,
        Self::Collaboration,
        Self::Note,
        Self::Other,
    ];

    /// Wire spelling used in reports (`author`, `booktitle`, …).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Editor => "editor",
            Self::Title => "title",
            Self::Journal => "journal",
            Self::Booktitle => "booktitle",
            Self::Publisher => "publisher",
            Self::Institution => "institution",
            Self::Series => "series",
            Self::Volume => "volume",
            Self::Issue => "issue",
            Self::Pages => "pages",
            Self::Location => "location",
            Self::Date => "date",
            Self::Web => "web",
            Self::Tech => "tech",
            Self::Pubnum => "pubnum",
            Self::Collaboration => "collaboration",
            Self::Note => "note",
            Self::Other => "other",
        }
    }

    /// Classify a base label (`<author>`, …). `None` outside the closed 19.
    pub fn from_base_label(base: &str) -> Option<Self> {
        CITATION_ROLE_BASE_LABELS
            .iter()
            .position(|label| *label == base)
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// A decoded label sequence plus the token texts it was decoded from.
///
/// This is the family-independent output of [`decode_bibliography_labels`]; the
/// family wrappers group it into typed spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BibliographyLabelSequence {
    /// One decoded label per row, in reading order.
    pub labels: Vec<String>,
    /// Column-0 token text per row, in reading order.
    pub tokens: Vec<String>,
    /// `tokens` joined by a single space; span byte ranges index this text.
    pub text: String,
    /// Half-open `(start, end)` byte range of each row's token inside `text`.
    pub token_bytes: Vec<(usize, usize)>,
}

/// One maximal run of equal base names over a decoded sequence.
///
/// This is the single grouping primitive every native bibliography family shares
/// (reference segmenter, citation, and the nested `name/citation` / `date`
/// families added by S04/T05): a run continues while
/// `base(current) == base(previous)`, where `base` strips the `I-` spelling
/// prefix, and nothing else opens or closes one. Byte ranges index the
/// reconstructed text of the sequence the run came from
/// ([`BibliographyLabelSequence::text`]).
///
/// A family wrapper maps this into its own span type so the rule cannot drift
/// between families; the bibliography families never re-implement it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BibliographyLabelRun {
    /// Base label of the run (`<reference>`, `<author>`, `<forename>`), never the
    /// `I-` spelling.
    pub label: String,
    /// Half-open row range in the original reading order.
    pub start: usize,
    pub end: usize,
    /// Half-open byte range inside the sequence's reconstructed text.
    pub byte_start: usize,
    pub byte_end: usize,
    /// Member token texts joined by a single space.
    pub text: String,
}

/// Group a decoded sequence into maximal runs of equal base names.
///
/// Total for any sequence [`decode_bibliography_labels`] produced: the label and
/// byte vectors are built together, so a disagreement can only come from a
/// hand-constructed [`BibliographyLabelSequence`] and is refused rather than
/// yielding a wrong byte range.
pub fn bibliography_label_runs(
    decoded: &BibliographyLabelSequence,
) -> Result<Vec<BibliographyLabelRun>, NativeBibliographySequenceError> {
    let mut runs = Vec::new();
    for (base, start, end) in base_label_runs(&decoded.labels) {
        let (byte_start, byte_end) = run_bytes(decoded, start, end)?;
        runs.push(BibliographyLabelRun {
            label: base,
            start,
            end,
            byte_start,
            byte_end,
            text: join_tokens(&decoded.tokens, start, end),
        });
    }
    Ok(runs)
}

/// One grouped run of the reference segmenter, in reading order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSegmentSpan {
    /// Boundary kind of the run.
    pub kind: ReferenceSegmentKind,
    /// Base label spelling of the run (`<reference>`), never the `I-` spelling.
    pub label: String,
    /// Half-open row range in the original reading order.
    pub start: usize,
    pub end: usize,
    /// Half-open byte range inside [`ReferenceSegmentSequence::line_text`].
    pub byte_start: usize,
    pub byte_end: usize,
    /// Member token texts joined by a single space.
    pub text: String,
}

/// One grouped citation run: a role cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CitationRoleSpan {
    /// Role of the run.
    pub role: CitationRole,
    /// Base label spelling of the run (`<author>`), never the `I-` spelling.
    pub label: String,
    /// Half-open row range in the original reading order.
    pub start: usize,
    pub end: usize,
    /// Half-open byte range inside [`CitationClusterSequence::cluster_text`].
    pub byte_start: usize,
    pub byte_end: usize,
    /// Member token texts joined by a single space.
    pub text: String,
}

/// Decoded reference-segmenter line: labels, reconstructed text, grouped spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSegmentSequence {
    pub labels: Vec<String>,
    /// Column-0 token texts joined by one space — the byte-span coordinate system.
    pub line_text: String,
    /// Grouped boundary spans, in reading order.
    pub segments: Vec<ReferenceSegmentSpan>,
}

/// Decoded citation block: labels, reconstructed text, grouped role clusters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CitationClusterSequence {
    pub labels: Vec<String>,
    /// Column-0 token texts joined by one space — the byte-span coordinate system.
    pub cluster_text: String,
    /// Grouped role clusters, in reading order.
    pub roles: Vec<CitationRoleSpan>,
}

/// Typed refusal vocabulary for the bibliography sequence runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBibliographySequenceError {
    /// No rows were supplied, so there is nothing to decode.
    EmptySequence,
    /// The sequence exceeds the bounded runner's admission.
    LimitsExceeded {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    /// A rendered row does not present the family's printed width.
    WrongArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    /// The model declares a label block of the wrong size for the family.
    WrongLabelCount {
        family: &'static str,
        expected: usize,
        actual: usize,
    },
    /// The model emitted a label outside the family's closed set.
    UnknownLabel { label: String, row: usize },
    /// The decoder produced a different number of labels than there were rows.
    CountMismatch { expected: usize, actual: usize },
    /// The scalar CRF itself refused the input.
    Model { message: String },
}

impl fmt::Display for NativeBibliographySequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(formatter, "bibliography sequence is empty"),
            Self::LimitsExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "bibliography sequence {resource} count {actual} exceeds the maximum {maximum}"
            ),
            Self::WrongArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "bibliography row {row} has {actual} columns; expected {expected}"
            ),
            Self::WrongLabelCount {
                family,
                expected,
                actual,
            } => write!(
                formatter,
                "{family} model declares {actual} labels; expected {expected}"
            ),
            Self::UnknownLabel { label, row } => write!(
                formatter,
                "bibliography model emitted unknown label {label:?} at row {row}"
            ),
            Self::CountMismatch { expected, actual } => write!(
                formatter,
                "bibliography decode produced {actual} labels for {expected} rows"
            ),
            Self::Model { message } => write!(formatter, "bibliography CRF failed: {message}"),
        }
    }
}

impl std::error::Error for NativeBibliographySequenceError {}

/// Decode rendered rows through the supplied scalar CRF model.
///
/// Family-independent core: it validates the rows, decodes them with
/// `da_ports::{map_input, viterbi_decode}`, and returns one row per label along
/// with the reconstructed text the family wrappers index. No model loading,
/// filesystem access, environment lookup, or fallback is performed here.
pub fn decode_bibliography_labels(
    row_texts: &[String],
    family: &BibliographySequenceFamily,
    model: &CrfModel,
) -> Result<BibliographyLabelSequence, NativeBibliographySequenceError> {
    if row_texts.is_empty() {
        return Err(NativeBibliographySequenceError::EmptySequence);
    }
    if row_texts.len() > MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS {
        return Err(NativeBibliographySequenceError::LimitsExceeded {
            resource: "row",
            actual: row_texts.len(),
            maximum: MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS,
        });
    }
    if model.labels.len() != family.admissible_labels.len() {
        return Err(NativeBibliographySequenceError::WrongLabelCount {
            family: family.name,
            expected: family.admissible_labels.len(),
            actual: model.labels.len(),
        });
    }

    let mut tokens = Vec::with_capacity(row_texts.len());
    for (row, text) in row_texts.iter().enumerate() {
        // Two views of the same row must agree with the family width: the
        // `' '`-separated view this module built, and the whitespace-separated
        // view `map_input` will take. A column that smuggled a separator would
        // otherwise shift the model's column grid without changing the row
        // count, which is exactly the injection the ABI must refuse.
        let fields: Vec<&str> = text.split(' ').collect();
        if fields.len() != family.expected_columns {
            return Err(NativeBibliographySequenceError::WrongArity {
                row,
                actual: fields.len(),
                expected: family.expected_columns,
            });
        }
        let model_fields = text.split_whitespace().count();
        if model_fields != family.expected_columns {
            return Err(NativeBibliographySequenceError::WrongArity {
                row,
                actual: model_fields,
                expected: family.expected_columns,
            });
        }
        tokens.push(fields[0].to_string());
    }

    let input = row_texts.join("\n");
    let positions = map_input(model, &input).map_err(model_error)?;
    if positions.len() != row_texts.len() {
        return Err(NativeBibliographySequenceError::CountMismatch {
            expected: row_texts.len(),
            actual: positions.len(),
        });
    }
    let decoded = viterbi_decode(model, &positions).map_err(model_error)?;
    if decoded.labels.len() != row_texts.len() {
        return Err(NativeBibliographySequenceError::CountMismatch {
            expected: row_texts.len(),
            actual: decoded.labels.len(),
        });
    }
    for (row, label) in decoded.labels.iter().enumerate() {
        if !family
            .admissible_labels
            .iter()
            .any(|admissible| admissible == label)
        {
            return Err(NativeBibliographySequenceError::UnknownLabel {
                label: label.clone(),
                row,
            });
        }
    }

    let text = tokens.join(" ");
    let token_bytes = token_byte_offsets(&tokens);
    Ok(BibliographyLabelSequence {
        labels: decoded.labels,
        tokens,
        text,
        token_bytes,
    })
}

/// Decode one reference-segmenter line and group it into boundary spans.
pub fn decode_reference_sequence(
    rows: &[ReferenceSegmenterFeatureRow],
    model: &CrfModel,
) -> Result<ReferenceSegmentSequence, NativeBibliographySequenceError> {
    let row_texts: Vec<String> = rows
        .iter()
        .map(ReferenceSegmenterFeatureRow::as_text)
        .collect();
    let decoded = decode_bibliography_labels(
        &row_texts,
        &NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
        model,
    )?;
    let mut segments = Vec::new();
    for run in bibliography_label_runs(&decoded)? {
        let Some(kind) = ReferenceSegmentKind::from_base_label(&run.label) else {
            // Unreachable while the family's admissible set is exactly the three
            // base labels; total anyway, so no input can panic the runner.
            return Err(NativeBibliographySequenceError::UnknownLabel {
                label: run.label,
                row: run.start,
            });
        };
        segments.push(ReferenceSegmentSpan {
            kind,
            label: run.label,
            start: run.start,
            end: run.end,
            byte_start: run.byte_start,
            byte_end: run.byte_end,
            text: run.text,
        });
    }
    Ok(ReferenceSegmentSequence {
        labels: decoded.labels,
        line_text: decoded.text,
        segments,
    })
}

/// Decode one citation block and group it into the 19 typed role clusters.
pub fn decode_citation_sequence(
    rows: &[CitationFeatureRow],
    model: &CrfModel,
) -> Result<CitationClusterSequence, NativeBibliographySequenceError> {
    let row_texts: Vec<String> = rows.iter().map(CitationFeatureRow::as_text).collect();
    let decoded = decode_bibliography_labels(&row_texts, &NATIVE_CITATION_SEQUENCE_FAMILY, model)?;
    let mut roles = Vec::new();
    for run in bibliography_label_runs(&decoded)? {
        let Some(role) = CitationRole::from_base_label(&run.label) else {
            return Err(NativeBibliographySequenceError::UnknownLabel {
                label: run.label,
                row: run.start,
            });
        };
        roles.push(CitationRoleSpan {
            role,
            label: run.label,
            start: run.start,
            end: run.end,
            byte_start: run.byte_start,
            byte_end: run.byte_end,
            text: run.text,
        });
    }
    Ok(CitationClusterSequence {
        labels: decoded.labels,
        cluster_text: decoded.text,
        roles,
    })
}

/// Strip the `I-` spelling prefix. The result is the *base name* GROBID's
/// continuation test compares (see the module docs).
fn base_label(label: &str) -> &str {
    label.strip_prefix("I-").unwrap_or(label)
}

/// Maximal runs of equal base names, as `(base, start, end)` in reading order.
///
/// This is the single implementation of the grouping rule; both families map
/// its output into their own span type, so the rule cannot drift between them.
fn base_label_runs(labels: &[String]) -> Vec<(String, usize, usize)> {
    let mut runs: Vec<(String, usize, usize)> = Vec::new();
    for (row, label) in labels.iter().enumerate() {
        let base = base_label(label);
        match runs.last_mut() {
            Some((active, _, end)) if active.as_str() == base => *end = row + 1,
            _ => runs.push((base.to_string(), row, row + 1)),
        }
    }
    runs
}

/// Half-open byte range of each token inside the single-space joined text.
fn token_byte_offsets(tokens: &[String]) -> Vec<(usize, usize)> {
    let mut offsets = Vec::with_capacity(tokens.len());
    let mut cursor = 0usize;
    for token in tokens {
        let start = cursor;
        let end = start + token.len();
        offsets.push((start, end));
        // One separator byte after every token; the trailing one is never read.
        cursor = end + 1;
    }
    offsets
}

/// Byte range covered by rows `start..end`, or a typed refusal if the byte
/// vector and the label vector disagree (they are built together, so this is a
/// structural guard rather than an input-dependent branch).
fn run_bytes(
    decoded: &BibliographyLabelSequence,
    start: usize,
    end: usize,
) -> Result<(usize, usize), NativeBibliographySequenceError> {
    let first = decoded.token_bytes.get(start);
    let last = end
        .checked_sub(1)
        .and_then(|index| decoded.token_bytes.get(index));
    match (first, last) {
        (Some(first), Some(last)) => Ok((first.0, last.1)),
        _ => Err(NativeBibliographySequenceError::CountMismatch {
            expected: decoded.labels.len(),
            actual: decoded.token_bytes.len(),
        }),
    }
}

fn join_tokens(tokens: &[String], start: usize, end: usize) -> String {
    tokens
        .get(start..end)
        .map(|members| members.join(" "))
        .unwrap_or_default()
}

fn model_error(error: CrfModelError) -> NativeBibliographySequenceError {
    NativeBibliographySequenceError::Model {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_bibliography_features::{
        NATIVE_CITATION_LABEL_COUNT, NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT,
    };
    use da_ports::scalar_crf::CrfPattern;
    use std::collections::BTreeMap;

    /// Row text carrying `token` in column 0 and inert `0` in every other
    /// column — the shape a fully rendered family row has.
    fn row_text(token: &str, columns: usize) -> String {
        std::iter::once(token.to_string())
            .chain((1..columns).map(|_| "0".to_string()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn rows(family: &BibliographySequenceFamily, tokens: &[&str]) -> Vec<String> {
        tokens
            .iter()
            .map(|token| row_text(token, family.expected_columns))
            .collect()
    }

    fn reference_rows(tokens: &[&str]) -> Vec<ReferenceSegmenterFeatureRow> {
        rows(&NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY, tokens)
            .into_iter()
            .map(|text| ReferenceSegmenterFeatureRow {
                columns: text.split(' ').map(str::to_string).collect(),
            })
            .collect()
    }

    fn citation_rows(tokens: &[&str]) -> Vec<CitationFeatureRow> {
        rows(&NATIVE_CITATION_SEQUENCE_FAMILY, tokens)
            .into_iter()
            .map(|text| CitationFeatureRow {
                columns: text.split(' ').map(str::to_string).collect(),
            })
            .collect()
    }

    /// Synthetic one-token-per-row model that forces `targets[row]` at row
    /// `row`: every position owns a distinct unigram observation, and that
    /// observation weights exactly one label strictly higher than the rest, so
    /// the decoded sequence is the requested one rather than an artefact of
    /// tie-breaking (no `b:` observation is declared, so transitions are zero).
    fn model_for<'a>(
        family: &BibliographySequenceFamily,
        tokens: impl AsRef<[&'a str]>,
        targets: impl AsRef<[&'a str]>,
    ) -> CrfModel {
        let tokens = tokens.as_ref();
        let targets = targets.as_ref();
        assert_eq!(tokens.len(), targets.len(), "one target per token");
        let labels: Vec<String> = family
            .admissible_labels
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        let label_count = labels.len();
        let observations: Vec<String> = tokens.iter().map(|token| format!("u:{token}")).collect();
        let mut u_offsets = Vec::with_capacity(observations.len());
        let mut weights = BTreeMap::new();
        for (position, target) in targets.iter().enumerate() {
            let offset = position * label_count;
            u_offsets.push(Some(offset));
            let target_id = labels
                .iter()
                .position(|label| label == target)
                .expect("requested target is part of the family vocabulary");
            weights.insert(offset + target_id, 3.0);
        }
        let nftr = observations.len() * label_count;
        CrfModel {
            model_type: 2,
            labels,
            observations,
            patterns: vec![CrfPattern {
                source: "u:%x[0,0]".into(),
                kind: 'u',
            }],
            u_offsets,
            b_offsets: vec![None; tokens.len()],
            weights,
            nftr,
        }
    }

    fn decode_reference<'a>(
        tokens: impl AsRef<[&'a str]>,
        targets: impl AsRef<[&'a str]>,
    ) -> Result<ReferenceSegmentSequence, NativeBibliographySequenceError> {
        let tokens = tokens.as_ref();
        let targets = targets.as_ref();
        let model = model_for(&NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY, tokens, targets);
        decode_reference_sequence(&reference_rows(tokens), &model)
    }

    fn decode_citation<'a>(
        tokens: impl AsRef<[&'a str]>,
        targets: impl AsRef<[&'a str]>,
    ) -> Result<CitationClusterSequence, NativeBibliographySequenceError> {
        let tokens = tokens.as_ref();
        let targets = targets.as_ref();
        let model = model_for(&NATIVE_CITATION_SEQUENCE_FAMILY, tokens, targets);
        decode_citation_sequence(&citation_rows(tokens), &model)
    }

    /// Whether a `I-` prefix opens or continues is decided by base-name
    /// equality, so a run may begin with the `I-` spelling — the shape the real
    /// 0.9.0 header model emits, and the rule S03's research inverted.
    #[test]
    fn both_label_spellings_of_one_kind_form_a_single_run() {
        let decoded = decode_reference(
            ["12", "Smith", "J", "Title", "2019"],
            [
                "I-<label>",
                "I-<reference>",
                "<reference>",
                "I-<reference>",
                "<reference>",
            ],
        )
        .expect("synthetic reference line decodes");
        assert_eq!(decoded.segments.len(), 2);
        assert_eq!(decoded.segments[0].kind, ReferenceSegmentKind::Label);
        assert_eq!(decoded.segments[0].start, 0);
        assert_eq!(decoded.segments[0].end, 1);
        assert_eq!(decoded.segments[1].kind, ReferenceSegmentKind::Reference);
        assert_eq!(decoded.segments[1].start, 1);
        assert_eq!(decoded.segments[1].end, 5);
        assert!(
            decoded
                .segments
                .iter()
                .all(|segment| !segment.label.starts_with("I-")),
            "a span carries the base name, never the I- spelling"
        );
        assert_eq!(decoded.segments[0].label, "<label>");
        assert_eq!(decoded.segments[0].text, "12");
        assert_eq!(decoded.segments[1].text, "Smith J Title 2019");
    }

    #[test]
    fn a_base_name_change_closes_a_run_and_opens_the_next() {
        let decoded = decode_reference(
            ["12", "Smith", "2019", "Archive"],
            ["<label>", "I-<reference>", "<other>", "I-<other>"],
        )
        .expect("synthetic reference line decodes");
        let spans: Vec<(&str, usize, usize)> = decoded
            .segments
            .iter()
            .map(|segment| (segment.label.as_str(), segment.start, segment.end))
            .collect();
        assert_eq!(
            spans,
            [("<label>", 0, 1), ("<reference>", 1, 2), ("<other>", 2, 4)]
        );
    }

    /// Runs tile the rows in reading order and every span is labelled by the base
    /// name of its first row.
    #[test]
    fn runs_are_a_contiguous_tiling_of_the_rows() {
        let decoded = decode_reference(
            ["12", "Smith", "2019", "Archive"],
            ["<label>", "I-<reference>", "<reference>", "<other>"],
        )
        .expect("synthetic reference line decodes");
        let mut cursor = 0usize;
        for segment in &decoded.segments {
            assert_eq!(segment.start, cursor, "runs tile the rows in reading order");
            assert!(segment.start < segment.end);
            let first = decoded.labels[segment.start].as_str();
            assert_eq!(segment.label, base_label(first));
            cursor = segment.end;
        }
        assert_eq!(cursor, decoded.labels.len(), "the last run reaches the end");
    }

    /// A repeated role opens a new cluster instead of merging with the first one,
    /// so `author · title · author` is three spans and not one.
    #[test]
    fn citation_roles_group_by_base_name_and_repeats_open_new_clusters() {
        let decoded = decode_citation(
            ["Vaswani", "Attention", "Doe"],
            ["I-<author>", "<title>", "<author>"],
        )
        .expect("synthetic citation block decodes");
        let roles: Vec<(CitationRole, usize, usize)> = decoded
            .roles
            .iter()
            .map(|span| (span.role, span.start, span.end))
            .collect();
        assert_eq!(
            roles,
            [
                (CitationRole::Author, 0, 1),
                (CitationRole::Title, 1, 2),
                (CitationRole::Author, 2, 3)
            ]
        );
    }

    #[test]
    fn byte_spans_index_the_reconstructed_text() {
        let decoded = decode_citation(
            ["Vaswani", "Ashish", "Attention", "2017"],
            ["I-<author>", "<author>", "<title>", "<date>"],
        )
        .expect("synthetic citation block decodes");
        assert_eq!(decoded.cluster_text, "Vaswani Ashish Attention 2017");
        assert_eq!(decoded.roles[0].text, "Vaswani Ashish");
        assert_eq!(decoded.roles[1].text, "Attention");
        assert_eq!(decoded.roles[2].text, "2017");
        for span in &decoded.roles {
            assert_eq!(
                &decoded.cluster_text[span.byte_start..span.byte_end],
                span.text,
                "the reported byte range must reproduce the span text"
            );
        }
        let line = decode_reference(
            ["12", "Smith", "2019"],
            ["<label>", "<reference>", "<reference>"],
        )
        .expect("synthetic reference line decodes");
        assert_eq!(line.line_text, "12 Smith 2019");
        for segment in &line.segments {
            assert_eq!(
                &line.line_text[segment.byte_start..segment.byte_end],
                segment.text
            );
        }
    }

    #[test]
    fn refuses_empty_sequence_before_any_model_work() {
        let family = NATIVE_CITATION_SEQUENCE_FAMILY;
        let model = model_for(&family, ["Vaswani"], ["<author>"]);
        assert!(matches!(
            decode_bibliography_labels(&[], &family, &model),
            Err(NativeBibliographySequenceError::EmptySequence)
        ));
    }

    #[test]
    fn refuses_rows_that_do_not_carry_the_printed_abi() {
        let family = NATIVE_CITATION_SEQUENCE_FAMILY;
        let model = model_for(&family, ["Vaswani"], ["<author>"]);
        let short = row_text("Vaswani", family.expected_columns - 1);
        assert!(matches!(
            decode_bibliography_labels(&[short], &family, &model),
            Err(NativeBibliographySequenceError::WrongArity {
                row: 0,
                actual: 29,
                expected: NATIVE_CITATION_FEATURE_COLUMNS
            })
        ));
        let reference = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let reference_model = model_for(&reference, ["12"], ["<label>"]);
        let wide = row_text("12", reference.expected_columns + 1);
        assert!(matches!(
            decode_bibliography_labels(&[wide], &reference, &reference_model),
            Err(NativeBibliographySequenceError::WrongArity {
                row: 0,
                actual: 29,
                expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS
            })
        ));
    }

    /// A column whose *value* carries whitespace keeps the `' '`-field count but
    /// gives the model's whitespace split one field more: the row is refused
    /// instead of silently shifting the column grid under the decoder.
    #[test]
    fn refuses_a_row_that_smuggles_a_separator_into_a_column() {
        let family = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let model = model_for(&family, ["12"], ["<label>"]);
        let mut columns: Vec<String> = (0..family.expected_columns)
            .map(|_| "0".to_string())
            .collect();
        columns[0] = "12".to_string();
        columns[1] = "0\t0".to_string();
        let text = columns.join(" ");
        assert_eq!(text.split(' ').count(), family.expected_columns);
        assert_eq!(text.split_whitespace().count(), family.expected_columns + 1);
        assert!(matches!(
            decode_bibliography_labels(&[text], &family, &model),
            Err(NativeBibliographySequenceError::WrongArity {
                row: 0,
                actual: 29,
                expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS
            })
        ));
    }

    #[test]
    fn refuses_a_model_with_the_wrong_label_block_size() {
        let reference = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let model = model_for(&NATIVE_CITATION_SEQUENCE_FAMILY, ["Vaswani"], ["<author>"]);
        assert!(matches!(
            decode_bibliography_labels(&rows(&reference, &["12"]), &reference, &model),
            Err(NativeBibliographySequenceError::WrongLabelCount {
                family: "reference-segmenter",
                expected: NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT,
                actual: NATIVE_CITATION_LABEL_COUNT
            })
        ));
    }

    #[test]
    fn refuses_labels_outside_the_declared_vocabulary() {
        let family = NATIVE_CITATION_SEQUENCE_FAMILY;
        let mut model = model_for(&family, ["Vaswani"], ["<author>"]);
        // Same cardinality, one foreign label: the block check cannot catch it,
        // so the decoded-label check must. The row's weighted label id is the
        // target's position in the table (`<author>` is id 10).
        let target_id = family
            .admissible_labels
            .iter()
            .position(|label| *label == "<author>")
            .expect("the fixture target is part of the family vocabulary");
        assert_eq!(target_id, 10);
        let understudy = model.labels[target_id].clone();
        model.labels[target_id] = "<not-declared>".into();
        assert!(matches!(
            decode_bibliography_labels(&rows(&family, &["Vaswani"]), &family, &model),
            Err(NativeBibliographySequenceError::UnknownLabel { label, row: 0 })
                if label == "<not-declared>"
        ));
        // Sanity: the swap is a real one, not a no-op edit of an unused id.
        assert_ne!(understudy, model.labels[target_id]);
        assert_eq!(model.labels.len(), NATIVE_CITATION_LABEL_COUNT);
    }

    #[test]
    fn refuses_oversized_input_before_model_work() {
        let family = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let model = model_for(&family, ["12"], ["<label>"]);
        let oversized = vec![
            row_text("12", family.expected_columns);
            MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS + 1
        ];
        assert!(matches!(
            decode_bibliography_labels(&oversized, &family, &model),
            Err(NativeBibliographySequenceError::LimitsExceeded {
                resource: "row",
                ..
            })
        ));
    }

    /// A malformed model template is a typed `Model` refusal, not a panic.
    #[test]
    fn surfaces_model_failures_as_a_typed_refusal() {
        let family = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let mut model = model_for(&family, ["12"], ["<label>"]);
        model.patterns = vec![CrfPattern {
            source: "u:%x[0,0".into(),
            kind: 'u',
        }];
        assert!(matches!(
            decode_bibliography_labels(&rows(&family, &["12"]), &family, &model),
            Err(NativeBibliographySequenceError::Model { .. })
        ));
    }

    #[test]
    fn decode_is_deterministic() {
        let family = NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY;
        let model = model_for(&family, ["12", "Smith"], ["<label>", "<reference>"]);
        let input = rows(&family, &["12", "Smith"]);
        assert_eq!(
            decode_bibliography_labels(&input, &family, &model),
            decode_bibliography_labels(&input, &family, &model)
        );
    }

    /// The 19 roles and the 38 model labels are two views of one closed set: the
    /// role table must reproduce every base label of the model table exactly
    /// twice (begin + inside), with no label left over and no role invented.
    #[test]
    fn the_role_table_is_the_model_label_table_up_to_the_spelling() {
        assert_eq!(CitationRole::ALL.len(), NATIVE_CITATION_ROLE_COUNT);
        assert_eq!(CITATION_ROLE_BASE_LABELS.len(), NATIVE_CITATION_ROLE_COUNT);
        let mut model_bases: Vec<&str> = NATIVE_CITATION_LABELS
            .iter()
            .map(|label| base_label(label))
            .collect();
        model_bases.sort_unstable();
        model_bases.dedup();
        let mut role_bases: Vec<&str> = CITATION_ROLE_BASE_LABELS.to_vec();
        role_bases.sort_unstable();
        assert_eq!(model_bases, role_bases);
        for base in CITATION_ROLE_BASE_LABELS {
            let spellings = NATIVE_CITATION_LABELS
                .iter()
                .filter(|label| **label == base || base_label(label) == base)
                .count();
            assert_eq!(spellings, 2, "role {base} must have a begin and an inside");
            assert!(
                CitationRole::from_base_label(base).is_some(),
                "base label {base} must classify into a role"
            );
        }
        assert!(CitationRole::from_base_label("<not-a-role>").is_none());
    }

    #[test]
    fn the_reference_kind_table_covers_the_three_base_labels() {
        let kinds: Vec<&str> = ReferenceSegmentKind::ALL
            .iter()
            .map(|kind| kind.base_label())
            .collect();
        let mut sorted = kinds.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, ["<label>", "<other>", "<reference>"]);
        for kind in ReferenceSegmentKind::ALL {
            assert_eq!(
                ReferenceSegmentKind::from_base_label(kind.base_label()),
                Some(kind)
            );
        }
        assert!(ReferenceSegmentKind::from_base_label("<unknown>").is_none());
    }
}
