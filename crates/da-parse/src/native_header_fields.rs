//! Pure projection of decoded GROBID header spans into composition fields.
//!
//! The CRF calls bibliographic identifiers `<pubnum>`.  This module exposes
//! that vocabulary as the composition-level `ids` field, without claiming a
//! narrower identifier type or inventing values absent from the decoded rows.
//!
//! Field text is reconstructed from the renderer's own row form: the first
//! column of each 29-column row is the raw token, so a run is the token texts
//! joined by a single space — exactly `HeaderFeatureRow::as_text` semantics.
//! It is deliberately *not* the original PDF micro-spacing: the ABI does not
//! carry it, and guessing it would be invention.
use crate::native_header_features::HeaderFeatureRow;
use crate::native_header_sequence::{HeaderFieldSpan, MAX_NATIVE_HEADER_SEQUENCE_ROWS};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NATIVE_HEADER_FIELDS_SCHEMA_VERSION: &str = "da-parse.native-header-fields/v1";

/// Rows the projection accepts.  Bound to the decode runner's own bound so the
/// projection can never be the looser of the two entry points.
pub const MAX_NATIVE_HEADER_PROJECTION_ROWS: usize = MAX_NATIVE_HEADER_SEQUENCE_ROWS;
/// Maximum decoded runs carried per field (e.g. several `<title>` runs).
pub const MAX_NATIVE_HEADER_FIELD_VALUES: usize = 4_096;
/// Maximum characters carried per single field value.
pub const MAX_NATIVE_HEADER_FIELD_CHARS: usize = 65_536;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeaderFields {
    pub title: Vec<String>,
    pub email: Vec<String>,
    /// Values decoded as `<pubnum>` by GROBID's header model.
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHeaderFieldsError {
    EmptyRows,
    TooManyRows { rows: usize },
    InvalidSpan { start: usize, end: usize },
    TooManyValues { label: String },
    FieldTooLong { label: String, chars: usize },
    EmptyField { label: String },
}

impl fmt::Display for NativeHeaderFieldsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRows => write!(f, "native header field projection has no rows"),
            Self::TooManyRows { rows } => write!(
                f,
                "native header field projection got {rows} rows, limit {}",
                MAX_NATIVE_HEADER_PROJECTION_ROWS
            ),
            Self::InvalidSpan { start, end } => {
                write!(f, "invalid header field span {start}..{end}")
            }
            Self::TooManyValues { label } => write!(
                f,
                "header field {label:?} carries more than {MAX_NATIVE_HEADER_FIELD_VALUES} values"
            ),
            Self::FieldTooLong { label, chars } => write!(
                f,
                "header field {label:?} value of {chars} chars exceeds {MAX_NATIVE_HEADER_FIELD_CHARS}"
            ),
            Self::EmptyField { label } => write!(f, "projected header field {label:?} is empty"),
        }
    }
}
impl std::error::Error for NativeHeaderFieldsError {}

/// Project only title, email, and `<pubnum>` spans; all other CRF fields stay
/// available in the decode outcome and are deliberately not guessed here.
///
/// Spans are expected in reading order, as produced by
/// [`crate::native_header_sequence::decode_header_sequence`]. Values are
/// preserved verbatim and in order; nothing is deduplicated, merged or
/// reordered.
pub fn project_native_header_fields(
    rows: &[HeaderFeatureRow],
    spans: &[HeaderFieldSpan],
) -> Result<NativeHeaderFields, NativeHeaderFieldsError> {
    if rows.is_empty() {
        return Err(NativeHeaderFieldsError::EmptyRows);
    }
    if rows.len() > MAX_NATIVE_HEADER_PROJECTION_ROWS {
        return Err(NativeHeaderFieldsError::TooManyRows { rows: rows.len() });
    }
    let mut fields = NativeHeaderFields {
        title: Vec::new(),
        email: Vec::new(),
        ids: Vec::new(),
    };
    for span in spans {
        if span.start >= span.end || span.end > rows.len() {
            return Err(NativeHeaderFieldsError::InvalidSpan {
                start: span.start,
                end: span.end,
            });
        }
        let label = span.label.as_str();
        let target = match label {
            "<title>" => &mut fields.title,
            "<email>" => &mut fields.email,
            "<pubnum>" => &mut fields.ids,
            _ => continue,
        };
        if target.len() >= MAX_NATIVE_HEADER_FIELD_VALUES {
            return Err(NativeHeaderFieldsError::TooManyValues {
                label: span.label.clone(),
            });
        }
        let value = rows[span.start..span.end]
            .iter()
            .map(|row| row.columns.first().map(String::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        let value = value.trim().to_owned();
        let chars = value.chars().count();
        if value.is_empty() {
            return Err(NativeHeaderFieldsError::EmptyField {
                label: span.label.clone(),
            });
        }
        if chars > MAX_NATIVE_HEADER_FIELD_CHARS {
            return Err(NativeHeaderFieldsError::FieldTooLong {
                label: span.label.clone(),
                chars,
            });
        }
        target.push(value);
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> HeaderFeatureRow {
        HeaderFeatureRow {
            columns: vec![text.to_owned()],
        }
    }

    fn span(label: &str, start: usize, end: usize) -> HeaderFieldSpan {
        HeaderFieldSpan {
            label: label.to_owned(),
            start,
            end,
        }
    }

    #[test]
    fn projects_title_email_and_pubnum_to_ids() {
        let rows = vec![
            row("Native"),
            row("Header"),
            row("a@b.test"),
            row("arXiv:1"),
        ];
        let fields = project_native_header_fields(
            &rows,
            &[
                span("<title>", 0, 2),
                span("<email>", 2, 3),
                span("<pubnum>", 3, 4),
            ],
        )
        .unwrap();
        assert_eq!(fields.title, vec!["Native Header"]);
        assert_eq!(fields.email, vec!["a@b.test"]);
        assert_eq!(fields.ids, vec!["arXiv:1"]);
    }

    #[test]
    fn ignores_fields_outside_the_projected_set() {
        let rows = vec![row("Ashish"), row("Vaswani"), row("Google")];
        let fields = project_native_header_fields(
            &rows,
            &[span("<author>", 0, 2), span("<affiliation>", 2, 3)],
        )
        .unwrap();
        assert_eq!(fields, NativeHeaderFields::default());
    }

    #[test]
    fn keeps_repeated_runs_in_reading_order() {
        let rows = vec![row("First"), row("a@b.test"), row("Second")];
        let fields = project_native_header_fields(
            &rows,
            &[
                span("<title>", 0, 1),
                span("<email>", 1, 2),
                span("<title>", 2, 3),
            ],
        )
        .unwrap();
        assert_eq!(fields.title, vec!["First", "Second"]);
        assert_eq!(fields.email, vec!["a@b.test"]);
    }

    #[test]
    fn rejects_empty_rows() {
        assert_eq!(
            project_native_header_fields(&[], &[span("<title>", 0, 1)]).unwrap_err(),
            NativeHeaderFieldsError::EmptyRows
        );
    }

    #[test]
    fn rejects_out_of_bounds_span_without_invention() {
        let error =
            project_native_header_fields(&[row("x")], &[span("<title>", 0, 2)]).unwrap_err();
        assert!(matches!(error, NativeHeaderFieldsError::InvalidSpan { .. }));
        let inverted = project_native_header_fields(
            &[row("x"), row("y")],
            &[span("<title>", 1, 1), span("<title>", 1, 0)],
        )
        .unwrap_err();
        assert!(matches!(
            inverted,
            NativeHeaderFieldsError::InvalidSpan { start: 1, end: 1 }
        ));
    }

    #[test]
    fn rejects_empty_field_value() {
        let error =
            project_native_header_fields(&[row("   ")], &[span("<title>", 0, 1)]).unwrap_err();
        assert_eq!(
            error,
            NativeHeaderFieldsError::EmptyField {
                label: "<title>".to_owned()
            }
        );
    }

    #[test]
    fn rejects_too_many_rows_before_projecting() {
        let rows = vec![row("x"); MAX_NATIVE_HEADER_PROJECTION_ROWS + 1];
        assert_eq!(
            project_native_header_fields(&rows, &[span("<title>", 0, 1)]).unwrap_err(),
            NativeHeaderFieldsError::TooManyRows {
                rows: MAX_NATIVE_HEADER_PROJECTION_ROWS + 1
            }
        );
    }

    #[test]
    fn rejects_too_many_values_per_field() {
        let rows = vec![row("x")];
        let spans = vec![span("<title>", 0, 1); MAX_NATIVE_HEADER_FIELD_VALUES + 1];
        assert_eq!(
            project_native_header_fields(&rows, &spans).unwrap_err(),
            NativeHeaderFieldsError::TooManyValues {
                label: "<title>".to_owned()
            }
        );
    }

    #[test]
    fn rejects_overlong_field_value() {
        let value = "a".repeat(MAX_NATIVE_HEADER_FIELD_CHARS + 1);
        let error =
            project_native_header_fields(&[row(&value)], &[span("<title>", 0, 1)]).unwrap_err();
        assert_eq!(
            error,
            NativeHeaderFieldsError::FieldTooLong {
                label: "<title>".to_owned(),
                chars: MAX_NATIVE_HEADER_FIELD_CHARS + 1
            }
        );
    }

    #[test]
    fn accepts_a_value_exactly_at_the_character_bound() {
        let value = "a".repeat(MAX_NATIVE_HEADER_FIELD_CHARS);
        let fields =
            project_native_header_fields(&[row(&value)], &[span("<title>", 0, 1)]).unwrap();
        assert_eq!(fields.title.len(), 1);
        assert_eq!(
            fields.title[0].chars().count(),
            MAX_NATIVE_HEADER_FIELD_CHARS
        );
    }
}
