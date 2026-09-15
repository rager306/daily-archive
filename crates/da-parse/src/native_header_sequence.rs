//! Pure sequence runner for the native GROBID header CRF.
//!
//! Layout providers own model and lexicon I/O. This module only validates the
//! already-rendered rows, maps them through a supplied scalar CRF model, and
//! groups the emitted labels into bounded field runs.
//!
//! ## Runs are grouped by base name, exactly as GROBID's `writeField` does
//!
//! Primary source: `grobid-core/src/main/java/org/grobid/core/engines/`
//! `HeaderParser.java`, `writeField`, whose continuation test is
//! `s1.equals(lastTag0) || s1.equals("I-" + lastTag0)`; `lastTag0` is the
//! *base* name of the previous label, already stripped of its `I-` prefix a few
//! hundred lines above the call sites (`lastTag0`/`currentTag0`). Substituting
//! the two possible spellings of the current label (`base` and `I-` + `base`)
//! collapses that test to `base(current) == base(previous)`:
//!
//! - `<field>` and `I-<field>` are two spellings of one field and never open a
//!   run against each other;
//! - a run is a maximal sequence of rows whose base names are equal;
//! - the `I-` prefix alone can neither open nor continue anything.
//!
//! `507-03-RESEARCH.md` §3 asserted the inverse (a BIO-like scheme where the
//! bare label opens and `I-` continues). That premise is wrong, and the runner
//! followed it: it refused `["I-<author>", "<author>", ...]` — the shape the
//! real 0.9.0 header model emits for an author line — with "continuation
//! `I-<author>` is invalid at row 0", i.e. it rejected every real header
//! sequence. Under the corrected rule no continuation can be invalid, so this
//! module deliberately has no such refusal.
use crate::native_header_features::{HeaderFeatureRow, NATIVE_HEADER_FEATURE_COLUMNS};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NATIVE_HEADER_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-header-sequence/v1";
pub const MAX_NATIVE_HEADER_SEQUENCE_ROWS: usize = 32_768;

/// Labels emitted by GROBID's 0.9.0 header model, in model-independent order.
pub const NATIVE_HEADER_LABELS: &[&str] = &[
    "I-<other>",
    "<other>",
    "I-<title>",
    "<title>",
    "I-<author>",
    "<author>",
    "I-<affiliation>",
    "<affiliation>",
    "I-<address>",
    "<address>",
    "I-<email>",
    "<email>",
    "I-<abstract>",
    "<abstract>",
    "I-<reference>",
    "<reference>",
    "I-<doctype>",
    "<doctype>",
    "I-<copyright>",
    "<copyright>",
    "I-<pubnum>",
    "<pubnum>",
    "I-<submission>",
    "<submission>",
    "I-<keyword>",
    "<keyword>",
    "I-<date>",
    "<date>",
    "I-<group>",
    "<group>",
    "I-<web>",
    "<web>",
    "I-<editor>",
    "<editor>",
    "I-<availability>",
    "<availability>",
    "I-<funding>",
    "<funding>",
    "I-<conflict>",
    "<conflict>",
    "I-<meeting>",
    "<meeting>",
    "I-<contribution>",
    "<contribution>",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderFieldSpan {
    /// The base GROBID field name with any `I-` prefix removed, e.g. `<title>`.
    pub label: String,
    /// Half-open row range in the original reading-order input.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeaderDecodeCandidate {
    pub labels: Vec<String>,
    pub fields: Vec<HeaderFieldSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeHeaderDecodeOutcome {
    Accepted { decode: NativeHeaderDecodeCandidate },
    Rejected { error: NativeHeaderDecodeError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHeaderDecodeError {
    EmptySequence,
    TooManyRows {
        actual: usize,
        maximum: usize,
    },
    WrongArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    UnknownLabel {
        label: String,
        row: usize,
    },
    Model {
        message: String,
    },
}

impl fmt::Display for NativeHeaderDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(f, "native header sequence is empty"),
            Self::TooManyRows { actual, maximum } => {
                write!(
                    f,
                    "native header sequence has {actual} rows; maximum is {maximum}"
                )
            }
            Self::WrongArity {
                row,
                actual,
                expected,
            } => {
                write!(
                    f,
                    "native header row {row} has {actual} columns; expected {expected}"
                )
            }
            Self::UnknownLabel { label, row } => {
                write!(
                    f,
                    "native header model emitted unknown label {label:?} at row {row}"
                )
            }
            Self::Model { message } => write!(f, "native header CRF failed: {message}"),
        }
    }
}
impl std::error::Error for NativeHeaderDecodeError {}

/// Decode rendered header rows through the supplied scalar CRF model.
///
/// No model loading, filesystem access, environment lookup, or fallback is
/// performed here. Equal inputs and model values always produce equal output.
pub fn decode_header_sequence(
    rows: &[HeaderFeatureRow],
    model: &CrfModel,
) -> NativeHeaderDecodeOutcome {
    if rows.is_empty() {
        return rejected(NativeHeaderDecodeError::EmptySequence);
    }
    if rows.len() > MAX_NATIVE_HEADER_SEQUENCE_ROWS {
        return rejected(NativeHeaderDecodeError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_NATIVE_HEADER_SEQUENCE_ROWS,
        });
    }
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_HEADER_FEATURE_COLUMNS {
            return rejected(NativeHeaderDecodeError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_HEADER_FEATURE_COLUMNS,
            });
        }
    }

    let input = rows
        .iter()
        .map(HeaderFeatureRow::as_text)
        .collect::<Vec<_>>()
        .join("\n");
    let positions = match map_input(model, &input) {
        Ok(positions) => positions,
        Err(error) => return rejected(model_error(error)),
    };
    let decoded = match viterbi_decode(model, &positions) {
        Ok(decoded) => decoded,
        Err(error) => return rejected(model_error(error)),
    };
    if decoded.labels.len() != rows.len() {
        return rejected(model_error(CrfModelError::CountMismatch));
    }

    let mut fields = Vec::new();
    let mut current: Option<(String, usize)> = None;
    for (row, label) in decoded.labels.iter().enumerate() {
        if !NATIVE_HEADER_LABELS.contains(&label.as_str()) {
            return rejected(NativeHeaderDecodeError::UnknownLabel {
                label: label.clone(),
                row,
            });
        }
        // GROBID groups runs by base name; the `I-` spelling is not a
        // continuation marker (see the module docs).
        let base = label.strip_prefix("I-").unwrap_or(label.as_str());
        let continues = current
            .as_ref()
            .is_some_and(|(active, _)| active.as_str() == base);
        if continues {
            continue;
        }
        if let Some((closed, start)) = current.take() {
            fields.push(HeaderFieldSpan {
                label: closed,
                start,
                end: row,
            });
        }
        current = Some((base.to_owned(), row));
    }
    if let Some((label, start)) = current {
        fields.push(HeaderFieldSpan {
            label,
            start,
            end: rows.len(),
        });
    }

    NativeHeaderDecodeOutcome::Accepted {
        decode: NativeHeaderDecodeCandidate {
            labels: decoded.labels,
            fields,
        },
    }
}

fn rejected(error: NativeHeaderDecodeError) -> NativeHeaderDecodeOutcome {
    NativeHeaderDecodeOutcome::Rejected { error }
}

fn model_error(error: CrfModelError) -> NativeHeaderDecodeError {
    NativeHeaderDecodeError::Model {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// One 29-column row whose first column carries the token text the
    /// synthetic model template reads; the remaining columns are inert.
    fn row(value: &str) -> HeaderFeatureRow {
        HeaderFeatureRow {
            columns: std::iter::once(value.to_string())
                .chain((1..NATIVE_HEADER_FEATURE_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        }
    }

    /// Synthetic one-token-per-row model that forces `labels[row]` at row
    /// `row`: every position owns a distinct unigram observation, and that
    /// observation weights exactly one label strictly higher than the rest, so
    /// the decoded sequence is the requested one rather than an artefact of
    /// tie-breaking.
    fn model_emitting(labels: &[&str]) -> (Vec<HeaderFeatureRow>, CrfModel) {
        let rows = (0..labels.len())
            .map(|index| row(&format!("t{index}")))
            .collect::<Vec<_>>();
        let mut vocabulary: Vec<String> = Vec::new();
        for label in labels {
            if !vocabulary.iter().any(|known| known == label) {
                vocabulary.push((*label).to_string());
            }
        }
        let label_count = vocabulary.len();
        let observations = (0..labels.len())
            .map(|index| format!("u:t{index}"))
            .collect::<Vec<_>>();
        let mut u_offsets = Vec::with_capacity(observations.len());
        let mut weights = BTreeMap::new();
        for (position, label) in labels.iter().enumerate() {
            let offset = position * label_count;
            u_offsets.push(Some(offset));
            let target = vocabulary
                .iter()
                .position(|known| known == label)
                .expect("requested label is part of the synthesized vocabulary");
            weights.insert(offset + target, 3.0);
        }
        let model = CrfModel {
            model_type: 2,
            labels: vocabulary,
            observations,
            patterns: vec![da_ports::scalar_crf::CrfPattern {
                source: "u:%x[0,0]".into(),
                kind: 'u',
            }],
            u_offsets,
            b_offsets: vec![None; labels.len()],
            weights,
            nftr: labels.len() * label_count,
        };
        (rows, model)
    }

    fn decoded(labels: &[&str]) -> NativeHeaderDecodeCandidate {
        let (rows, model) = model_emitting(labels);
        match decode_header_sequence(&rows, &model) {
            NativeHeaderDecodeOutcome::Accepted { decode } => decode,
            NativeHeaderDecodeOutcome::Rejected { error } => {
                panic!("synthetic sequence {labels:?} must decode, got {error}")
            }
        }
    }

    fn spans(labels: &[&str]) -> Vec<HeaderFieldSpan> {
        decoded(labels).fields
    }

    fn span(label: &str, start: usize, end: usize) -> HeaderFieldSpan {
        HeaderFieldSpan {
            label: label.to_owned(),
            start,
            end,
        }
    }

    /// `writeField` compares base names, so `<field>` and `I-<field>` are one
    /// field and never open a run against each other (research §3 claimed the
    /// inverse).
    #[test]
    fn both_label_spellings_of_one_field_form_a_single_run() {
        let fields = spans(&["I-<title>", "<title>", "I-<title>", "<title>"]);
        assert_eq!(fields, [span("<title>", 0, 4)]);
    }

    /// The exact shape the real GROBID 0.9.0 header model emitted for an author
    /// line (T06 evidence run): one `I-<author>` opening followed by nine bare
    /// `<author>` rows. The previous rule refused this with `continuation
    /// "I-<author>" is invalid at row 0`, i.e. it rejected every real header
    /// sequence.
    #[test]
    fn decodes_the_real_model_author_run_without_a_continuation_refusal() {
        let labels = [
            "I-<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
        ];
        let decode = decoded(&labels);
        assert_eq!(decode.labels, labels);
        assert_eq!(decode.fields, [span("<author>", 0, 10)]);
        assert!(
            decode
                .fields
                .iter()
                .all(|field| !field.label.starts_with("I-")),
            "a span label is the base field name, never the I- spelling"
        );
    }

    #[test]
    fn a_base_name_change_closes_the_run_and_opens_the_next() {
        let fields = spans(&["<title>", "I-<author>", "<author>", "<affiliation>"]);
        assert_eq!(
            fields,
            [
                span("<title>", 0, 1),
                span("<author>", 1, 3),
                span("<affiliation>", 3, 4),
            ]
        );
    }

    /// Whatever the label spelling, the runs tile the rows in reading order and
    /// every span is labelled by the base name of its first row.
    #[test]
    fn runs_are_a_contiguous_tiling_of_the_rows() {
        let labels = [
            "<title>",
            "I-<title>",
            "<author>",
            "I-<author>",
            "<author>",
            "<date>",
        ];
        let decode = decoded(&labels);
        assert!(!decode.fields.is_empty());
        let mut cursor = 0usize;
        for field in &decode.fields {
            assert_eq!(field.start, cursor, "runs tile the rows in reading order");
            assert!(field.start < field.end);
            let first = decode.labels[field.start].as_str();
            assert_eq!(
                field.label,
                first.strip_prefix("I-").unwrap_or(first),
                "a run carries the base name of its first row"
            );
            cursor = field.end;
        }
        assert_eq!(cursor, decode.labels.len(), "the last run reaches the end");
    }

    #[test]
    fn refuses_empty_sequence_before_any_model_work() {
        let (_, model) = model_emitting(&["<title>"]);
        assert!(matches!(
            decode_header_sequence(&[], &model),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::EmptySequence
            }
        ));
    }

    #[test]
    fn refuses_rows_that_do_not_carry_the_29_column_abi() {
        let (_, model) = model_emitting(&["<title>"]);
        let mut bad = row("t0");
        bad.columns.pop();
        assert!(matches!(
            decode_header_sequence(&[bad], &model),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::WrongArity {
                    row: 0,
                    actual: 28,
                    expected: NATIVE_HEADER_FEATURE_COLUMNS
                }
            }
        ));
    }

    #[test]
    fn refuses_labels_outside_the_declared_vocabulary() {
        let (rows, mut model) = model_emitting(&["<title>"]);
        model.labels = vec!["<not-declared>".into()];
        assert!(matches!(
            decode_header_sequence(&rows, &model),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::UnknownLabel { .. }
            }
        ));
    }

    #[test]
    fn refuses_oversized_input_before_model_work() {
        let (_, model) = model_emitting(&["<title>"]);
        let rows = vec![row("t0"); MAX_NATIVE_HEADER_SEQUENCE_ROWS + 1];
        assert!(matches!(
            decode_header_sequence(&rows, &model),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::TooManyRows { .. }
            }
        ));
    }

    /// A malformed model template is a typed `Model` refusal, not a panic.
    #[test]
    fn surfaces_model_failures_as_a_typed_refusal() {
        let (rows, mut model) = model_emitting(&["<title>"]);
        model.patterns = vec![da_ports::scalar_crf::CrfPattern {
            source: "u:%x[0,0".into(),
            kind: 'u',
        }];
        assert!(matches!(
            decode_header_sequence(&rows, &model),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::Model { .. }
            }
        ));
    }

    #[test]
    fn decode_is_deterministic() {
        let (rows, model) = model_emitting(&["<title>", "<author>"]);
        assert_eq!(
            decode_header_sequence(&rows, &model),
            decode_header_sequence(&rows, &model)
        );
    }
}
