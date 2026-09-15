//! Pure sequence runner for the native GROBID header CRF.
//!
//! Layout providers own model and lexicon I/O. This module only validates the
//! already-rendered rows, maps them through a supplied scalar CRF model, and
//! groups the resulting BIO-like (plain/I-) labels into bounded field spans.
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
    /// The unprefixed GROBID field name, e.g. `<title>`.
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
    InvalidContinuation {
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
            Self::InvalidContinuation { label, row } => {
                write!(
                    f,
                    "native header continuation {label:?} is invalid at row {row}"
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
        if let Some(field) = label.strip_prefix("I-") {
            let expected = format!("<{field}>");
            match current.as_mut() {
                Some((active, _)) if active == &expected => {}
                _ => {
                    return rejected(NativeHeaderDecodeError::InvalidContinuation {
                        label: label.clone(),
                        row,
                    });
                }
            }
        } else {
            if let Some((label, start)) = current.take() {
                fields.push(HeaderFieldSpan {
                    label,
                    start,
                    end: row,
                });
            }
            current = Some((label.clone(), row));
        }
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

    fn row(value: &str) -> HeaderFeatureRow {
        HeaderFeatureRow {
            columns: std::iter::once(value.to_string())
                .chain((1..NATIVE_HEADER_FEATURE_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        }
    }

    fn model() -> CrfModel {
        let labels = ["<title>", "<author>"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        CrfModel {
            model_type: 2,
            labels,
            observations: vec!["u:title".into(), "u:author".into()],
            patterns: vec![da_ports::scalar_crf::CrfPattern {
                source: "u:%x[0,0]".into(),
                kind: 'u',
            }],
            u_offsets: vec![Some(0), Some(2)],
            b_offsets: vec![None, None],
            weights: BTreeMap::from([(0, 3.0), (3, 3.0)]),
            nftr: 4,
        }
    }

    #[test]
    fn decodes_and_groups_title_and_author_runs() {
        let outcome = decode_header_sequence(&[row("title"), row("author")], &model());
        let NativeHeaderDecodeOutcome::Accepted { decode } = outcome else {
            panic!("synthetic header should decode");
        };
        assert_eq!(decode.labels, ["<title>", "<author>"]);
        assert_eq!(
            decode.fields,
            [
                HeaderFieldSpan {
                    label: "<title>".into(),
                    start: 0,
                    end: 1
                },
                HeaderFieldSpan {
                    label: "<author>".into(),
                    start: 1,
                    end: 2
                },
            ]
        );
    }

    #[test]
    fn rejects_empty_bad_arity_unknown_and_invalid_continuation() {
        assert!(matches!(
            decode_header_sequence(&[], &model()),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::EmptySequence
            }
        ));
        let mut bad = row("title");
        bad.columns.pop();
        assert!(matches!(
            decode_header_sequence(&[bad], &model()),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::WrongArity { .. }
            }
        ));
        let unknown = CrfModel {
            labels: vec!["<not-declared>".into(), "<author>".into()],
            ..model()
        };
        assert!(matches!(
            decode_header_sequence(&[row("title")], &unknown),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::UnknownLabel { .. }
            }
        ));
        let continuation = CrfModel {
            labels: vec!["I-<title>".into(), "<author>".into()],
            ..model()
        };
        assert!(matches!(
            decode_header_sequence(&[row("title")], &continuation),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::InvalidContinuation { .. }
            }
        ));
    }

    #[test]
    fn rejects_oversized_input_before_model_work() {
        let rows = vec![row("title"); MAX_NATIVE_HEADER_SEQUENCE_ROWS + 1];
        assert!(matches!(
            decode_header_sequence(&rows, &model()),
            NativeHeaderDecodeOutcome::Rejected {
                error: NativeHeaderDecodeError::TooManyRows { .. }
            }
        ));
    }

    #[test]
    fn decode_is_deterministic() {
        let rows = [row("title"), row("author")];
        assert_eq!(
            decode_header_sequence(&rows, &model()),
            decode_header_sequence(&rows, &model())
        );
    }
}
