//! Pure sequence runner for the native GROBID fulltext (body) CRF.
//!
//! Layout providers own model and row I/O. This module only validates the
//! already-rendered 27-column rows, maps them through a supplied scalar CRF
//! model, and groups the emitted labels into bounded runs — the same division of
//! labour as [`crate::native_header_sequence`] and
//! [`crate::native_bibliography_sequence`]. It opens no file, reads no
//! environment variable, and has no fallback producer.
//!
//! ## The closed 24-label dictionary
//!
//! `models/fulltext/model.wapiti` labels the body flow with twelve **base
//! labels** — `<paragraph>`, `<item>`, `<section>`, `<citation_marker>`,
//! `<figure_marker>`, `<table_marker>`, `<equation_marker>`, `<figure>`,
//! `<table>`, `<equation>`, `<equation_label>`, `<other>` — transcribed in
//! `data/parse/native_body_accounting.yaml` (`fulltext_labels`). Every base
//! label has two spellings (`<x>` and `I-<x>`), so the admissible decoded
//! vocabulary is exactly 24 labels. A model whose declared label block is not
//! that set is refused up front
//! ([`NativeFulltextSequenceError::ModelLabelCount`] /
//! [`NativeFulltextSequenceError::ModelLabelMismatch`]); the row-by-row
//! [`NativeFulltextSequenceError::UnknownLabel`] guard is retained as
//! defense-in-depth behind that block check, so no decode path can ever carry a
//! label the dictionary does not name.
//!
//! ## Grouping is the shared rule, never a second copy (D359 / MEM1382)
//!
//! Runs are grouped by **base-name equality**: `I-` is a *spelling* of a label,
//! not a continuation marker, so a run continues while
//! `base(current) == base(previous)` and nothing else opens or closes one
//! (a base-name change closes the run and opens the next). That rule already
//! lives in [`crate::native_bibliography_sequence::base_label_runs`]; this module
//! calls it instead of restating it, so the rule cannot drift between the
//! header, bibliography, nested and fulltext families. S03's research asserted
//! the inverse (a BIO-like scheme where the bare label opens and `I-` continues)
//! and that reading rejected every real 0.9.0 sequence — the mirror-image rule
//! is a trap, not an option, which is exactly why there is one implementation.
//!
//! ## What this runner does *not* prove
//!
//! The real GROBID model bytes are not in the tree (D358), so the contract test
//! decodes a hand-authored synthetic model. Decoding whatever frozen model the
//! caller hands it — and refusing the ones that do not declare this dictionary —
//! is all this runner claims. Binding a real model's bytes to the transcribed
//! dictionary is the opt-in real-model harness's job (S05/T05).
//!
//! ## Determinism
//!
//! Equal rows and equal model values always produce equal output: weight
//! selection, tie-breaking and run detection are pure functions of the inputs.
//! There is no clock, no iteration over a hash map, and no I/O.

use crate::native_bibliography_sequence::base_label_runs;
use crate::native_fulltext_features::{
    FulltextFeatureRow, NATIVE_FULLTEXT_FEATURE_COLUMNS, render_fulltext_feature_text,
};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema identity of the decoded fulltext sequence surface.
pub const NATIVE_FULLTEXT_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-fulltext-sequence/v1";

/// Rows admitted into one decode, mirroring the header and bibliography runners'
/// bound.
pub const MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS: usize = 32_768;

/// The twelve base labels of `models/fulltext/model.wapiti`, in the order
/// `data/parse/native_body_accounting.yaml` transcribes them.
///
/// Each entry carries `<x>` spelling; the `I-` spelling is
/// [`NATIVE_FULLTEXT_LABELS`]' odd index (a unit test pins the interleaving).
pub const NATIVE_FULLTEXT_BASE_LABELS: [&str; 12] = [
    "<paragraph>",
    "<item>",
    "<section>",
    "<citation_marker>",
    "<figure_marker>",
    "<table_marker>",
    "<equation_marker>",
    "<figure>",
    "<table>",
    "<equation>",
    "<equation_label>",
    "<other>",
];

/// The closed decoded-label vocabulary: both spellings of every base label.
///
/// Layout: `[2i]` is `NATIVE_FULLTEXT_BASE_LABELS[i]` and `[2i + 1]` is its
/// `I-` spelling. The order is model-independent — the model's own block is
/// compared as a *set* — so a real model that lists the two spellings in another
/// order still decodes, while one that adds, drops or renames a label is
/// refused.
pub const NATIVE_FULLTEXT_LABELS: [&str; 24] = [
    "<paragraph>",
    "I-<paragraph>",
    "<item>",
    "I-<item>",
    "<section>",
    "I-<section>",
    "<citation_marker>",
    "I-<citation_marker>",
    "<figure_marker>",
    "I-<figure_marker>",
    "<table_marker>",
    "I-<table_marker>",
    "<equation_marker>",
    "I-<equation_marker>",
    "<figure>",
    "I-<figure>",
    "<table>",
    "I-<table>",
    "<equation>",
    "I-<equation>",
    "<equation_label>",
    "I-<equation_label>",
    "<other>",
    "I-<other>",
];

/// One maximal run of equal base names over a decoded body sequence.
///
/// A run continues while `base(current) == base(previous)`, where `base` strips
/// the `I-` spelling prefix — the shared D359 rule (see the module docs). A
/// single-row run is legal: a base name change closes the previous run and opens
/// the next, so a repeated base after another base yields two runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FulltextLabelRun {
    /// Base label of the run (`<paragraph>`, `<section>`), never the `I-`
    /// spelling.
    pub label: String,
    /// Half-open row range in the original reading order.
    pub start: usize,
    pub end: usize,
}

/// A decoded body sequence plus its grouped runs, in reading order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFulltextDecodeCandidate {
    /// One decoded label per row, in reading order, exactly as the model emitted
    /// it (spelling preserved).
    pub labels: Vec<String>,
    /// Maximal base-name runs, tiling `labels` end to end.
    pub runs: Vec<FulltextLabelRun>,
}

/// Typed result of one fulltext decode: an accepted candidate, or the refusal
/// that stopped it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeFulltextDecodeOutcome {
    Accepted {
        decode: NativeFulltextDecodeCandidate,
    },
    Rejected {
        error: NativeFulltextSequenceError,
    },
}

/// Typed refusal vocabulary for the fulltext sequence runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFulltextSequenceError {
    /// No rows were supplied, so there is nothing to decode.
    EmptySequence,
    /// The sequence exceeds the bounded runner's admission.
    TooManyRows { actual: usize, maximum: usize },
    /// A rendered row does not present the printed 27-column width (checked on
    /// both the `' '`-separated and the whitespace-separated view, so a column
    /// that smuggled a separator is refused rather than shifting the grid).
    WrongArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    /// The model emitted a label outside the closed native vocabulary.
    UnknownLabel { label: String, row: usize },
    /// The model declares a label block of the wrong cardinality.
    ModelLabelCount { expected: usize, actual: usize },
    /// The model declares the right number of labels but not the native set.
    ModelLabelMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    /// The 27-column row serializer refused the rows handed to the runner.
    Feature { message: String },
    /// The decoder produced a different number of labels than there were rows.
    CountMismatch { expected: usize, actual: usize },
    /// The scalar CRF itself refused the input.
    Model { message: String },
}

impl fmt::Display for NativeFulltextSequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(formatter, "native fulltext sequence is empty"),
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "native fulltext sequence has {actual} rows; maximum is {maximum}"
            ),
            Self::WrongArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "native fulltext row {row} has {actual} columns; expected {expected}"
            ),
            Self::UnknownLabel { label, row } => write!(
                formatter,
                "native fulltext model emitted unknown label {label:?} at row {row}"
            ),
            Self::ModelLabelCount { expected, actual } => write!(
                formatter,
                "fulltext model declares {actual} labels; the closed dictionary has {expected}"
            ),
            Self::ModelLabelMismatch {
                missing,
                unexpected,
            } => write!(
                formatter,
                "fulltext model label block diverges from the native dictionary \
                 (missing {missing:?}, unexpected {unexpected:?})"
            ),
            Self::Feature { message } => {
                write!(formatter, "fulltext row rendering failed: {message}")
            }
            Self::CountMismatch { expected, actual } => write!(
                formatter,
                "fulltext decode produced {actual} labels for {expected} rows"
            ),
            Self::Model { message } => write!(formatter, "fulltext CRF failed: {message}"),
        }
    }
}

impl std::error::Error for NativeFulltextSequenceError {}

/// Is `label` (either spelling) part of the closed native vocabulary?
pub fn fulltext_label_is_admissible(label: &str) -> bool {
    NATIVE_FULLTEXT_LABELS.contains(&label)
}

/// Fail closed when a model's declared label block is not the native set.
///
/// Two distinct conditions are reported separately so the diagnostic names the
/// actual defect: a wrong cardinality, and a same-sized block that still diverges
/// from the dictionary (which the emitted-label check could only catch one row at
/// a time). The comparison is order-independent: a model may list the two
/// spellings in any order.
pub fn check_fulltext_label_block(model: &CrfModel) -> Result<(), NativeFulltextSequenceError> {
    if model.labels.len() != NATIVE_FULLTEXT_LABELS.len() {
        return Err(NativeFulltextSequenceError::ModelLabelCount {
            expected: NATIVE_FULLTEXT_LABELS.len(),
            actual: model.labels.len(),
        });
    }
    let mut missing: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .filter(|label| !model.labels.iter().any(|declared| declared == *label))
        .map(|label| (*label).to_string())
        .collect();
    let mut unexpected: Vec<String> = model
        .labels
        .iter()
        .filter(|declared| !NATIVE_FULLTEXT_LABELS.contains(&declared.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    unexpected.sort_unstable();
    unexpected.dedup();
    Err(NativeFulltextSequenceError::ModelLabelMismatch {
        missing,
        unexpected,
    })
}

/// Decode rendered fulltext rows through the supplied scalar CRF model and group
/// the emitted labels into base-name runs.
///
/// No model loading, filesystem access, environment lookup, or fallback is
/// performed here. The rows are re-serialized through
/// [`render_fulltext_feature_text`] so the text the model sees is exactly the
/// ABI the row builder produced, and that text is what `da_ports::map_input`
/// takes. Equal inputs and model values always produce equal output.
pub fn decode_fulltext_sequence(
    rows: &[FulltextFeatureRow],
    model: &CrfModel,
) -> NativeFulltextDecodeOutcome {
    match decode(rows, model) {
        Ok(decode) => NativeFulltextDecodeOutcome::Accepted { decode },
        Err(error) => NativeFulltextDecodeOutcome::Rejected { error },
    }
}

fn decode(
    rows: &[FulltextFeatureRow],
    model: &CrfModel,
) -> Result<NativeFulltextDecodeCandidate, NativeFulltextSequenceError> {
    if rows.is_empty() {
        return Err(NativeFulltextSequenceError::EmptySequence);
    }
    if rows.len() > MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS {
        return Err(NativeFulltextSequenceError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS,
        });
    }
    check_fulltext_label_block(model)?;
    for (row, feature_row) in rows.iter().enumerate() {
        // Two views of the same row must agree with the printed width: the
        // `' '`-separated view `render_fulltext_feature_text` writes, and the
        // whitespace-separated view `map_input` will take. A column that
        // smuggled a separator would otherwise shift the model's column grid
        // without changing the row count, which is exactly the injection the
        // ABI must refuse.
        if feature_row.width() != NATIVE_FULLTEXT_FEATURE_COLUMNS {
            return Err(NativeFulltextSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
            });
        }
        let text = feature_row.as_text();
        let spaced = text.split(' ').count();
        let model_view = text.split_whitespace().count();
        if spaced != NATIVE_FULLTEXT_FEATURE_COLUMNS
            || model_view != NATIVE_FULLTEXT_FEATURE_COLUMNS
        {
            let actual = if spaced != NATIVE_FULLTEXT_FEATURE_COLUMNS {
                spaced
            } else {
                model_view
            };
            return Err(NativeFulltextSequenceError::WrongArity {
                row,
                actual,
                expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
            });
        }
    }

    let input = render_fulltext_feature_text(rows).map_err(|error| {
        NativeFulltextSequenceError::Feature {
            message: error.to_string(),
        }
    })?;
    let positions = map_input(model, &input).map_err(model_error)?;
    if positions.len() != rows.len() {
        return Err(NativeFulltextSequenceError::CountMismatch {
            expected: rows.len(),
            actual: positions.len(),
        });
    }
    let decoded = viterbi_decode(model, &positions).map_err(model_error)?;
    if decoded.labels.len() != rows.len() {
        return Err(NativeFulltextSequenceError::CountMismatch {
            expected: rows.len(),
            actual: decoded.labels.len(),
        });
    }
    for (row, label) in decoded.labels.iter().enumerate() {
        if !fulltext_label_is_admissible(label) {
            return Err(NativeFulltextSequenceError::UnknownLabel {
                label: label.clone(),
                row,
            });
        }
    }

    // The shared D359 rule: `I-` is a spelling, not a continuation marker. Not a
    // second copy of the grouping loop.
    let runs = base_label_runs(&decoded.labels)
        .into_iter()
        .map(|(label, start, end)| FulltextLabelRun { label, start, end })
        .collect();

    Ok(NativeFulltextDecodeCandidate {
        labels: decoded.labels,
        runs,
    })
}

fn model_error(error: CrfModelError) -> NativeFulltextSequenceError {
    NativeFulltextSequenceError::Model {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_bibliography_sequence::base_label;
    use da_ports::scalar_crf::CrfPattern;
    use std::collections::BTreeMap;

    /// One 27-column row whose first column carries the token text the synthetic
    /// model template reads; the remaining columns are inert.
    fn row(value: &str) -> FulltextFeatureRow {
        FulltextFeatureRow {
            columns: std::iter::once(value.to_string())
                .chain((1..NATIVE_FULLTEXT_FEATURE_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        }
    }

    /// Synthetic one-token-per-row model over the closed 24-label dictionary that
    /// forces `labels[row]` at row `row`: every position owns a distinct unigram
    /// observation and that observation weights exactly one label strictly
    /// higher than the rest, so the decoded sequence is the requested one rather
    /// than an artefact of tie-breaking (no `b:` observation is declared, so all
    /// transitions are zero).
    fn model_emitting(labels: &[&str]) -> (Vec<FulltextFeatureRow>, CrfModel) {
        let rows = (0..labels.len())
            .map(|index| row(&format!("t{index}")))
            .collect::<Vec<_>>();
        let vocabulary: Vec<String> = NATIVE_FULLTEXT_LABELS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        let label_count = vocabulary.len();
        let observations = (0..labels.len())
            .map(|index| format!("u:t{index}"))
            .collect::<Vec<_>>();
        let mut u_offsets = Vec::with_capacity(observations.len());
        let mut weights = BTreeMap::new();
        for (position, label) in labels.iter().enumerate() {
            let offset = position * label_count;
            u_offsets.push(Some(offset));
            let target = NATIVE_FULLTEXT_LABELS
                .iter()
                .position(|known| known == label)
                .expect("requested label is part of the closed dictionary");
            weights.insert(offset + target, 3.0);
        }
        let model = CrfModel {
            model_type: 2,
            labels: vocabulary,
            observations,
            patterns: vec![CrfPattern {
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

    fn decoded(labels: &[&str]) -> NativeFulltextDecodeCandidate {
        let (rows, model) = model_emitting(labels);
        match decode_fulltext_sequence(&rows, &model) {
            NativeFulltextDecodeOutcome::Accepted { decode } => decode,
            NativeFulltextDecodeOutcome::Rejected { error } => {
                panic!("synthetic sequence {labels:?} must decode, got {error}")
            }
        }
    }

    fn runs(labels: &[&str]) -> Vec<FulltextLabelRun> {
        decoded(labels).runs
    }

    fn run(label: &str, start: usize, end: usize) -> FulltextLabelRun {
        FulltextLabelRun {
            label: label.to_owned(),
            start,
            end,
        }
    }

    /// The closed dictionary is twelve base labels in both spellings, index for
    /// index; the `I-` spelling of `base` is `base` behind the shared prefix
    /// (the same `base_label` the run grouping uses — no local copy).
    #[test]
    fn the_dictionary_is_twelve_bases_in_both_spellings() {
        assert_eq!(NATIVE_FULLTEXT_BASE_LABELS.len(), 12);
        assert_eq!(NATIVE_FULLTEXT_LABELS.len(), 24);
        for (index, base) in NATIVE_FULLTEXT_BASE_LABELS.iter().enumerate() {
            assert_eq!(NATIVE_FULLTEXT_LABELS[2 * index], *base);
            assert_ne!(NATIVE_FULLTEXT_LABELS[2 * index + 1], *base);
            assert_eq!(
                base_label(NATIVE_FULLTEXT_LABELS[2 * index + 1]),
                *base,
                "the odd index is the I- spelling of {base}"
            );
            assert_eq!(NATIVE_FULLTEXT_LABELS[2 * index + 1], format!("I-{base}"));
            assert!(fulltext_label_is_admissible(base));
        }
        assert!(!fulltext_label_is_admissible("<not-declared>"));
        assert!(!fulltext_label_is_admissible("I-<not-declared>"));
    }

    /// `I-<paragraph>` first is one run, exactly as the real 0.9.0 family emits
    /// for a body paragraph: the `I-` spelling is not a continuation marker, so
    /// no row can be "invalid" by position.
    #[test]
    fn an_inside_spelling_first_opens_a_run_of_the_same_base() {
        let runs = runs(&["I-<paragraph>", "<paragraph>", "<paragraph>"]);
        assert_eq!(runs, [run("<paragraph>", 0, 3)]);
    }

    /// The reverse order — bare then `I-` — is the same run, not a new one.
    #[test]
    fn a_bare_label_then_its_inside_spelling_stays_one_run() {
        let runs = runs(&["<paragraph>", "I-<paragraph>"]);
        assert_eq!(runs, [run("<paragraph>", 0, 2)]);
    }

    #[test]
    fn a_base_name_change_closes_the_run_and_opens_the_next() {
        let runs = runs(&["<paragraph>", "I-<section>", "<section>", "I-<other>"]);
        assert_eq!(
            runs,
            [
                run("<paragraph>", 0, 1),
                run("<section>", 1, 3),
                run("<other>", 3, 4),
            ]
        );
    }

    /// Whatever the spelling, runs tile the rows in reading order and every run
    /// is labelled by the base name of its first row.
    #[test]
    fn runs_are_a_contiguous_tiling_of_the_rows() {
        let decode = decoded(&[
            "<paragraph>",
            "I-<paragraph>",
            "<section>",
            "I-<section>",
            "<section>",
            "<other>",
        ]);
        assert!(!decode.runs.is_empty());
        let mut cursor = 0usize;
        for run in &decode.runs {
            assert_eq!(run.start, cursor, "runs tile the rows in reading order");
            assert!(run.start < run.end);
            assert_eq!(run.label, base_label(&decode.labels[run.start]));
            assert!(!run.label.starts_with("I-"));
            cursor = run.end;
        }
        assert_eq!(cursor, decode.labels.len(), "the last run reaches the end");
    }

    #[test]
    fn refuses_empty_sequence_before_any_model_work() {
        let (_, model) = model_emitting(&["<paragraph>"]);
        assert!(matches!(
            decode_fulltext_sequence(&[], &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::EmptySequence
            }
        ));
    }

    #[test]
    fn refuses_rows_that_do_not_carry_the_27_column_abi() {
        let (_, model) = model_emitting(&["<paragraph>"]);
        let mut bad = row("t0");
        bad.columns.pop();
        assert!(matches!(
            decode_fulltext_sequence(&[bad], &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::WrongArity {
                    row: 0,
                    actual: 26,
                    expected: NATIVE_FULLTEXT_FEATURE_COLUMNS
                }
            }
        ));
    }

    /// A column whose *value* carries whitespace keeps the `' '`-field count but
    /// gives the model's whitespace split one field more: the row is refused
    /// instead of silently shifting the column grid under the decoder.
    #[test]
    fn refuses_a_row_that_smuggles_a_separator_into_a_column() {
        let (_, model) = model_emitting(&["<paragraph>"]);
        let mut bad = row("t0");
        bad.columns[1] = "0\t0".into();
        assert_eq!(
            bad.as_text().split(' ').count(),
            NATIVE_FULLTEXT_FEATURE_COLUMNS
        );
        assert_eq!(
            bad.as_text().split_whitespace().count(),
            NATIVE_FULLTEXT_FEATURE_COLUMNS + 1
        );
        assert!(matches!(
            decode_fulltext_sequence(&[bad], &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::WrongArity {
                    row: 0,
                    actual: 28,
                    expected: NATIVE_FULLTEXT_FEATURE_COLUMNS
                }
            }
        ));
    }

    /// The model's declared label block is checked before any row work, so a
    /// wrong cardinality is refused before a single row is serialized.
    #[test]
    fn refuses_a_model_whose_label_block_has_the_wrong_size() {
        let (rows, mut model) = model_emitting(&["<paragraph>"]);
        model.labels.pop();
        assert!(matches!(
            decode_fulltext_sequence(&rows, &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::ModelLabelCount {
                    expected: 24,
                    actual: 23
                }
            }
        ));
    }

    /// A same-sized block that still diverges cannot hide behind the count
    /// check: the row-by-row guard alone would need one foreign row to notice,
    /// so the block check reports both directions at once.
    #[test]
    fn refuses_a_same_sized_block_that_diverges_from_the_dictionary() {
        let (rows, mut model) = model_emitting(&["<paragraph>"]);
        let index = model
            .labels
            .iter()
            .position(|label| label == "<other>")
            .expect("the dictionary carries <other>");
        model.labels[index] = "<not-declared>".into();
        assert_eq!(model.labels.len(), 24);
        match decode_fulltext_sequence(&rows, &model) {
            NativeFulltextDecodeOutcome::Rejected {
                error:
                    NativeFulltextSequenceError::ModelLabelMismatch {
                        missing,
                        unexpected,
                    },
            } => {
                assert_eq!(missing, ["<other>"]);
                assert_eq!(unexpected, ["<not-declared>"]);
            }
            other => panic!("a diverging label block must be refused, got {other:?}"),
        }
    }

    /// The block comparison is order-independent: a model that lists the same
    /// 24 spellings in another order still declares the native dictionary.
    #[test]
    fn accepts_a_model_that_lists_the_same_set_in_another_order() {
        let (rows, mut model) = model_emitting(&["<paragraph>"]);
        model.labels.reverse();
        assert_eq!(check_fulltext_label_block(&model), Ok(()));
        // The weight table is indexed by position, so the reversed table makes
        // the same weight select another label; the decode still succeeds and
        // every emitted label is a dictionary member.
        match decode_fulltext_sequence(&rows, &model) {
            NativeFulltextDecodeOutcome::Accepted { decode } => assert!(
                decode
                    .labels
                    .iter()
                    .all(|label| fulltext_label_is_admissible(label))
            ),
            other => panic!("a reordered dictionary block must still decode, got {other:?}"),
        }
    }

    #[test]
    fn refuses_oversized_input_before_model_work() {
        let (_, model) = model_emitting(&["<paragraph>"]);
        let rows = vec![row("t0"); MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS + 1];
        assert!(matches!(
            decode_fulltext_sequence(&rows, &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::TooManyRows { .. }
            }
        ));
    }

    /// A malformed model template is a typed `Model` refusal, not a panic.
    #[test]
    fn surfaces_model_failures_as_a_typed_refusal() {
        let (rows, mut model) = model_emitting(&["<paragraph>"]);
        model.patterns = vec![CrfPattern {
            source: "u:%x[0,0".into(),
            kind: 'u',
        }];
        assert!(matches!(
            decode_fulltext_sequence(&rows, &model),
            NativeFulltextDecodeOutcome::Rejected {
                error: NativeFulltextSequenceError::Model { .. }
            }
        ));
    }

    #[test]
    fn decode_is_deterministic() {
        let (rows, model) = model_emitting(&["<paragraph>", "I-<section>"]);
        assert_eq!(
            decode_fulltext_sequence(&rows, &model),
            decode_fulltext_sequence(&rows, &model)
        );
    }
}
