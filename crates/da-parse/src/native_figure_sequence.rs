//! Pure sequence runner for the native GROBID `figure` CRF model.
//!
//! `FullTextParser.processFigures` cuts the fulltext clusters labelled
//! `<figure>` and hands each cluster's feature block to the **secondary**
//! `figure` model (`models/figure/model.wapiti`, `#mdl#2#521`, `#rdr#85/25/0`).
//! This module is that second pass and nothing else: it validates the already
//! rendered rows, maps them through a supplied scalar CRF model, groups the
//! emitted labels and assembles one figure span.
//!
//! ## The rows are the S05 fulltext ABI — there is no second figure ABI
//!
//! The features the secondary model sees are fulltext rows *without* a predicted
//! label, so this family re-uses [`FulltextFeatureRow`] and
//! [`NATIVE_FULLTEXT_FEATURE_COLUMNS`] verbatim (27 printed columns) and never
//! invents a figure-specific row. The dead `FeaturesVectorFigure.printVector`
//! (32 columns) is called nowhere in `grobid-core`/`grobid-trainer` and is
//! deliberately not resurrected here. What *does* differ is the referenced
//! window: the frozen `figure` templates read columns `0..=25` only
//! (`template_excludes: [26]` — column 26 is `superscript`), so a model whose
//! templates reach any other column index is refused up front
//! ([`check_figure_column_window`]) instead of silently consuming a column the
//! Java side never fed to this model. Note that the guard is load-bearing: a
//! 27-column row *has* a column 26, so `da_ports::map_input` would happily
//! expand `%x[0,26]` without it.
//!
//! ## The closed seven-label dictionary
//!
//! `data/parse/native_visual_families.yaml` transcribes the frozen model's label
//! block verbatim in model order; [`NATIVE_FIGURE_LABELS`] mirrors that list 1:1
//! (the contract test fails if either side drifts). The block is compared as a
//! *set* — a model that lists the same spellings in another order decodes, one
//! that adds, drops or renames a label is refused with both directions reported
//! (`missing` and `unexpected`). `<other>` is declared by the Java label enum but
//! absent from the frozen model block, so no decode path can produce it and no
//! branch maps it.
//!
//! ## Grouping is the shared clusteror rule, never a second copy (D359)
//!
//! Figure labels are grouped by [`cluster_family_label_runs`] — GROBID's
//! `TaggingTokenClusteror`, where a token carrying the `I-` prefix *opens* a
//! cluster. `I-<figure_head>` followed by `<figure_head>` is therefore **two**
//! clusters while `<figure_head>` followed by `<figure_head>` is one. The D359
//! `base_label_runs` of the header/bibliography/fulltext bands answers the
//! opposite question and must not be used here; the divergence and its witness
//! live in [`crate::native_visual_family_cluster`].
//!
//! ## The caption lane is typed, bounded, and does not resolve anything
//!
//! Every span carries the shared [`NativeCaptionLinks`] inventory. The marker
//! inventory is **caller-supplied**: an empty inventory is
//! [`NativeCaptionLinkAbsence::NotObserved`] and never "zero links" (H12-N1), a
//! malformed or duplicated digest refuses the whole decode instead of decaying
//! into an absence, and a marker addressing a cluster this decode does not
//! contain is a typed refusal rather than a silently dropped caption. Resolving
//! a marker to a reference belongs to the already-shipped S04
//! [`crate::match_native_callouts`] surface and is deliberately *not* repeated
//! here.
//!
//! ## Absence is not zero (H12-N1)
//!
//! A decode that carries no figure label at all is
//! [`NativeFigureDecodeOutcome::NotObserved`], never `Decoded { spans: [] }`.
//! Every row of a non-empty sequence receives exactly one label from the closed
//! dictionary, so the decoder itself cannot produce an empty decode;
//! [`figure_spans_from_decoded`] is the seam that keeps the rule testable (and
//! available to a caller that assembled labels elsewhere).
//!
//! ## What this runner does *not* prove
//!
//! The real GROBID model bytes are not in the tree (D358), so the contract test
//! decodes a hand-authored synthetic model. Decoding whatever frozen model the
//! caller hands it — and refusing the ones that do not declare this dictionary
//! or this column window — is all this runner claims. Binding a real model's
//! bytes to the transcribed dictionary is the opt-in real-model harness's job
//! (S06/T07). Row/Cell geometry, `TableParser.validateTable` and
//! `inferAcronyms` are out of scope here.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! model bytes, no absolute path. Every function is total on hostile input —
//! an empty sequence, a wrong-arity row, a foreign label block, a declared
//! column window outside `0..=25`, an over-long sequence and a malformed
//! template are typed refusals, never a panic and never a silent truncation.
//! `import_eligible` stays false and nothing here is spliced into the production
//! parse route.

use crate::native_fulltext_features::{
    FulltextFeatureRow, NATIVE_FULLTEXT_FEATURE_COLUMNS, render_fulltext_feature_text,
};
use crate::native_visual_family_cluster::{
    FamilyLabelCluster, NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker,
    NativeVisualFamilyClusterError, caption_links_from_markers, cluster_family_label_runs,
};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema identity of the decoded figure sequence surface.
pub const NATIVE_FIGURE_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-figure-sequence/v1";

/// Printed row width of the rows this family decodes.
///
/// A derived alias, not a second ABI: the secondary `figure` model consumes the
/// same 27-column fulltext row the S05 builder prints.
pub const NATIVE_FIGURE_EMITTED_COLUMNS: usize = NATIVE_FULLTEXT_FEATURE_COLUMNS;

/// Highest column index the frozen `figure` templates reference
/// (`#rdr#85/25/0` → columns `0..=25`).
///
/// Column 26 (`superscript`) is printed by the ABI but never read by this model
/// (`template_excludes: [26]`), which is exactly the window
/// [`check_figure_column_window`] enforces.
pub const NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX: usize = 25;

/// Rows admitted into one figure decode, mirroring the S05 runner's bound.
///
/// Provisional and fail-closed: an input beyond this bound is refused rather
/// than truncated, so no silently partial read can be mistaken for a complete
/// one.
pub const MAX_NATIVE_FIGURE_ROWS: usize = 32_768;

/// The closed decoded-label vocabulary of `models/figure/model.wapiti`.
///
/// Transcribed in the frozen model's own order, mirroring
/// `families.figure.declared_labels` in
/// `data/parse/native_visual_families.yaml` 1:1 (both bare and `I-` spellings).
/// The order is a transcription fact and carries no meaning to the decoder: the
/// label-block check compares sets, and the loaded model owns its own order.
pub const NATIVE_FIGURE_LABELS: [&str; 7] = [
    "I-<figure_head>",
    "I-<label>",
    "I-<figDesc>",
    "<figDesc>",
    "I-<content>",
    "<content>",
    "<figure_head>",
];

/// The four base roles of the figure family, mirroring
/// `families.figure.base_labels` in the DATA file.
pub const NATIVE_FIGURE_BASE_LABELS: [&str; 4] =
    ["<figure_head>", "<label>", "<figDesc>", "<content>"];

/// One figure assembled from the clusters of a decoded figure sequence.
///
/// A file-level absence of the model leaves `None` on the role fields — never
/// `Some("")`, so "the model emitted no such cluster" and "the cluster was
/// empty" stay distinguishable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureSpan {
    /// `<label>` — the figure's printed number, when the model emitted one.
    pub label: Option<String>,
    /// `<figure_head>` — the figure heading line.
    pub head: Option<String>,
    /// `<figDesc>` — the figure description (caption body).
    pub caption: Option<String>,
    /// `<content>` — the textual trace of the graphical content.
    pub content: Option<String>,
    /// The caller-supplied caption-marker inventory of this span, or its typed
    /// absence.
    pub caption_links: NativeCaptionLinks,
    /// Index of the [`FamilyLabelCluster`] that opened this span.
    ///
    /// A figure decode covers exactly one fulltext `<figure>` cluster (Java
    /// calls the secondary model once per figure cluster), so the span opens at
    /// cluster 0; caption markers address the cluster they were observed at
    /// inside this span.
    pub cluster_index: usize,
}

/// Closed vocabulary of figure absences.
///
/// [`Self::NoFigureClusters`] is the H12-N1 witness: an empty decode is an
/// absence, not a figure with zero parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFigureAbsence {
    /// The decoded label sequence carried no figure-family cluster at all.
    NoFigureClusters,
}

/// Typed result of one figure decode: assembled spans, or the absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeFigureDecodeOutcome {
    Decoded {
        /// One decoded label per row, in reading order, spelling preserved.
        labels: Vec<String>,
        /// The assembled spans. A figure decode yields exactly one.
        spans: Vec<FigureSpan>,
    },
    NotObserved {
        reason: NativeFigureAbsence,
    },
}

/// Typed refusal vocabulary for the figure sequence runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFigureSequenceError {
    /// No rows were supplied, so there is nothing to decode.
    EmptySequence,
    /// The sequence exceeds the bounded runner's admission.
    TooManyRows { actual: usize, maximum: usize },
    /// A rendered row does not present the printed 27-column width.
    WrongArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    /// The model declares a label block of the wrong cardinality.
    ModelLabelCount { expected: usize, actual: usize },
    /// The model declares the right number of labels but not the native set.
    ModelLabelMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    /// The model's templates do not reference exactly columns `0..=25`.
    ///
    /// `declared` is the highest column index seen in a `%x[offset,column]`
    /// template, or `None` when no template references a column at all.
    DeclaredColumnMismatch {
        declared: Option<usize>,
        expected: usize,
    },
    /// A label outside the closed native vocabulary reached the span builder.
    UnmodeledLabel { label: String, row: usize },
    /// The label/row parallelism the builder requires is broken.
    CountMismatch { expected: usize, actual: usize },
    /// The supplied caption-marker inventory was malformed (bad digest width,
    /// non-lexical hex, or a duplicated digest).
    InvalidCaptionInventory,
    /// A caption marker addresses a cluster this decode does not contain.
    DanglingCaptionCluster {
        cluster_index: usize,
        clusters: usize,
    },
    /// The shared `TaggingTokenClusteror` grouping pass refused the labels.
    Grouping {
        error: NativeVisualFamilyClusterError,
    },
    /// The 27-column row serializer refused the rows handed to the runner.
    Feature { message: String },
    /// The scalar CRF itself refused the input.
    Model { message: String },
}

impl fmt::Display for NativeFigureSequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(formatter, "native figure sequence is empty"),
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "native figure sequence has {actual} rows; maximum is {maximum}"
            ),
            Self::WrongArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "native figure row {row} has {actual} columns; expected {expected}"
            ),
            Self::ModelLabelCount { expected, actual } => write!(
                formatter,
                "figure model declares {actual} labels; the closed dictionary has {expected}"
            ),
            Self::ModelLabelMismatch {
                missing,
                unexpected,
            } => write!(
                formatter,
                "figure model label block diverges from the native dictionary \
                 (missing {missing:?}, unexpected {unexpected:?})"
            ),
            Self::DeclaredColumnMismatch { declared, expected } => write!(
                formatter,
                "figure model references column {declared:?}; the frozen window ends at {expected}"
            ),
            Self::UnmodeledLabel { label, row } => write!(
                formatter,
                "figure label {label:?} at row {row} is not part of the closed dictionary"
            ),
            Self::CountMismatch { expected, actual } => write!(
                formatter,
                "figure decode produced {actual} values for {expected} positions"
            ),
            Self::InvalidCaptionInventory => {
                write!(formatter, "figure caption-marker inventory is malformed")
            }
            Self::DanglingCaptionCluster {
                cluster_index,
                clusters,
            } => write!(
                formatter,
                "figure caption marker addresses cluster {cluster_index}, but the decode has \
                 {clusters} clusters"
            ),
            Self::Grouping { error } => write!(formatter, "figure label grouping failed: {error}"),
            Self::Feature { message } => {
                write!(formatter, "figure row rendering failed: {message}")
            }
            Self::Model { message } => write!(formatter, "figure CRF failed: {message}"),
        }
    }
}

impl std::error::Error for NativeFigureSequenceError {}

/// Is `label` (either spelling) part of the closed native vocabulary?
pub fn figure_label_is_admissible(label: &str) -> bool {
    NATIVE_FIGURE_LABELS.contains(&label)
}

/// Fail closed when a model's declared label block is not the native set.
///
/// Two distinct conditions are reported separately so the diagnostic names the
/// actual defect: a wrong cardinality, and a same-sized block that still
/// diverges from the dictionary (which the emitted-label check could only catch
/// one row at a time). The comparison is order-independent: a model may list the
/// two spellings in any order.
pub fn check_figure_label_block(model: &CrfModel) -> Result<(), NativeFigureSequenceError> {
    if model.labels.len() != NATIVE_FIGURE_LABELS.len() {
        return Err(NativeFigureSequenceError::ModelLabelCount {
            expected: NATIVE_FIGURE_LABELS.len(),
            actual: model.labels.len(),
        });
    }
    let mut missing: Vec<String> = NATIVE_FIGURE_LABELS
        .iter()
        .filter(|label| !model.labels.iter().any(|declared| declared == *label))
        .map(|label| (*label).to_string())
        .collect();
    let mut unexpected: Vec<String> = model
        .labels
        .iter()
        .filter(|declared| !NATIVE_FIGURE_LABELS.contains(&declared.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    unexpected.sort_unstable();
    unexpected.dedup();
    Err(NativeFigureSequenceError::ModelLabelMismatch {
        missing,
        unexpected,
    })
}

/// Fail closed when the model's referenced column window is not the frozen one.
///
/// The frozen `figure` model declares `#rdr#85/25/0`: its templates reference
/// columns `0..=25` of the printed 27-column fulltext row and never column 26
/// (`superscript`). Because a 27-column row *does* carry column 26,
/// `da_ports::map_input` would expand `%x[0,26]` successfully — so this check is
/// the only thing standing between a drifted model and the silent consumption of
/// a column the Java side never feeds this model. The window is read off the
/// model's own templates, which is the only place a loaded [`CrfModel`] declares
/// a column at all.
pub fn check_figure_column_window(model: &CrfModel) -> Result<(), NativeFigureSequenceError> {
    let declared = template_max_column_index(model).map_err(model_error)?;
    if declared == Some(NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX) {
        return Ok(());
    }
    Err(NativeFigureSequenceError::DeclaredColumnMismatch {
        declared,
        expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
    })
}

/// Decode rendered fulltext rows through the supplied `figure` model.
///
/// No model loading, filesystem access, environment lookup, or fallback is
/// performed here. The rows are re-serialized through
/// [`render_fulltext_feature_text`] so the text the model sees is exactly the ABI
/// the S05 row builder produced, and that text is what `da_ports::map_input`
/// takes. Equal inputs and model values always produce equal output.
///
/// The caption-marker inventory is the caller's: it is validated as a whole and
/// attributed to the span, never resolved to a reference (that boundary belongs
/// to [`crate::match_native_callouts`]).
pub fn decode_figure_sequence(
    rows: &[FulltextFeatureRow],
    model: &CrfModel,
    markers: &[NativeCaptionMarker],
) -> Result<NativeFigureDecodeOutcome, NativeFigureSequenceError> {
    if rows.is_empty() {
        return Err(NativeFigureSequenceError::EmptySequence);
    }
    if rows.len() > MAX_NATIVE_FIGURE_ROWS {
        return Err(NativeFigureSequenceError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_NATIVE_FIGURE_ROWS,
        });
    }
    check_figure_label_block(model)?;
    check_figure_column_window(model)?;
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_FIGURE_EMITTED_COLUMNS {
            return Err(NativeFigureSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FIGURE_EMITTED_COLUMNS,
            });
        }
    }

    let input =
        render_fulltext_feature_text(rows).map_err(|error| NativeFigureSequenceError::Feature {
            message: error.to_string(),
        })?;
    let positions = map_input(model, &input).map_err(model_error)?;
    if positions.len() != rows.len() {
        return Err(NativeFigureSequenceError::CountMismatch {
            expected: rows.len(),
            actual: positions.len(),
        });
    }
    let decoded = viterbi_decode(model, &positions).map_err(model_error)?;
    if decoded.labels.len() != rows.len() {
        return Err(NativeFigureSequenceError::CountMismatch {
            expected: rows.len(),
            actual: decoded.labels.len(),
        });
    }
    figure_spans_from_decoded(&decoded.labels, rows, markers)
}

/// Assemble a figure span from an already-decoded label sequence.
///
/// This is the seam the decoder above calls, kept public because it owns the
/// H12-N1 rule: an empty decode is [`NativeFigureDecodeOutcome::NotObserved`]
/// and never a figure with zero parts. `rows` supplies the token text of each
/// decoded position (column 0 of the printed fulltext row); `markers` is the
/// caller's caption inventory.
///
/// Roles map one-to-one onto the span fields: `<label>` → `label`,
/// `<figure_head>` → `head`, `<figDesc>` → `caption`, `<content>` → `content`.
/// A role the decode did not emit stays `None`. A role the decode split across
/// two clusters (a `<figDesc>` between two `<content>` clusters, for instance)
/// joins its parts with a single space rather than dropping one of them.
pub fn figure_spans_from_decoded(
    labels: &[String],
    rows: &[FulltextFeatureRow],
    markers: &[NativeCaptionMarker],
) -> Result<NativeFigureDecodeOutcome, NativeFigureSequenceError> {
    if labels.is_empty() {
        return Ok(NativeFigureDecodeOutcome::NotObserved {
            reason: NativeFigureAbsence::NoFigureClusters,
        });
    }
    if labels.len() != rows.len() {
        return Err(NativeFigureSequenceError::CountMismatch {
            expected: labels.len(),
            actual: rows.len(),
        });
    }
    for (row, label) in labels.iter().enumerate() {
        if !figure_label_is_admissible(label) {
            return Err(NativeFigureSequenceError::UnmodeledLabel {
                label: label.clone(),
                row,
            });
        }
    }
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_FIGURE_EMITTED_COLUMNS {
            return Err(NativeFigureSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FIGURE_EMITTED_COLUMNS,
            });
        }
    }
    let clusters = cluster_family_label_runs(labels)
        .map_err(|error| NativeFigureSequenceError::Grouping { error })?;

    // The inventory is validated as a whole first: a malformed digest must not
    // decay into "no links observed", and a duplicated digest is not two
    // captions.
    let caption_links = caption_links_from_markers(markers.to_vec());
    if caption_links.absence == Some(NativeCaptionLinkAbsence::InvalidDigest) {
        return Err(NativeFigureSequenceError::InvalidCaptionInventory);
    }
    for marker in markers {
        if marker.cluster_index >= clusters.len() {
            return Err(NativeFigureSequenceError::DanglingCaptionCluster {
                cluster_index: marker.cluster_index,
                clusters: clusters.len(),
            });
        }
    }

    let mut span = FigureSpan {
        label: None,
        head: None,
        caption: None,
        content: None,
        caption_links,
        // A figure decode opens at its first cluster; see the field docs.
        cluster_index: 0,
    };
    for cluster in &clusters {
        let text = cluster_text(rows, cluster)?;
        let slot = match cluster.base.as_str() {
            "<label>" => &mut span.label,
            "<figure_head>" => &mut span.head,
            "<figDesc>" => &mut span.caption,
            "<content>" => &mut span.content,
            // The admissibility guard above already proves this branch cannot be
            // taken; it stays a typed refusal instead of a panic so hostile
            // input can never abort a caller.
            other => {
                return Err(NativeFigureSequenceError::UnmodeledLabel {
                    label: other.to_string(),
                    row: cluster.start,
                });
            }
        };
        append_role_text(slot, &text);
    }

    Ok(NativeFigureDecodeOutcome::Decoded {
        labels: labels.to_vec(),
        spans: vec![span],
    })
}

/// The printed token text of one cluster: column 0 of every row in its range,
/// joined by a single space (GROBID's own `TextUtilities.join` separator).
///
/// Empty tokens are skipped, so an all-empty cluster yields the empty string and
/// the caller stores `None` rather than `Some("")`.
fn cluster_text(
    rows: &[FulltextFeatureRow],
    cluster: &FamilyLabelCluster,
) -> Result<String, NativeFigureSequenceError> {
    let mut text = String::new();
    for row in cluster.start..cluster.end {
        let Some(feature_row) = rows.get(row) else {
            return Err(NativeFigureSequenceError::CountMismatch {
                expected: cluster.end,
                actual: rows.len(),
            });
        };
        let token = feature_row
            .columns
            .first()
            .map(String::as_str)
            .unwrap_or_default();
        if token.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(token);
    }
    Ok(text)
}

/// Fold one cluster's text into its role slot.
///
/// An empty cluster leaves the slot `None` (never `Some("")`), and a repeated
/// role joins with a single space so neither part is dropped.
fn append_role_text(slot: &mut Option<String>, text: &str) {
    if text.is_empty() {
        return;
    }
    match slot {
        Some(existing) => {
            existing.push(' ');
            existing.push_str(text);
        }
        None => *slot = Some(text.to_string()),
    }
}

/// The highest `%x[offset,column]` column index any template references.
///
/// `None` means no template references a column at all. A template whose
/// `%x[...]` spec is malformed is a [`CrfModelError::MalformedTemplate`], the
/// same refusal `da_ports::map_input` would raise — a malformed template must
/// not be skipped, or it could hide a column reference the window check exists
/// to catch.
fn template_max_column_index(model: &CrfModel) -> Result<Option<usize>, CrfModelError> {
    let mut maximum: Option<usize> = None;
    for pattern in &model.patterns {
        let source = pattern.source.as_str();
        let mut cursor = 0usize;
        while let Some(relative) = source[cursor..].find("%x[") {
            let start = cursor + relative;
            let end = source[start + 3..]
                .find(']')
                .map(|offset| start + 3 + offset)
                .ok_or(CrfModelError::MalformedTemplate)?;
            let spec = &source[start + 3..end];
            let column = spec
                .split_once(',')
                .filter(|(_, column)| !column.contains(','))
                .map(|(_, column)| column)
                .ok_or(CrfModelError::MalformedTemplate)?;
            let column = column
                .parse::<usize>()
                .map_err(|_| CrfModelError::MalformedTemplate)?;
            maximum = Some(maximum.map_or(column, |current| current.max(column)));
            cursor = end + 1;
        }
    }
    Ok(maximum)
}

fn model_error(error: CrfModelError) -> NativeFigureSequenceError {
    NativeFigureSequenceError::Model {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::scalar_crf::CrfPattern;
    use std::collections::BTreeMap;

    /// One 27-column row whose first column carries the token text; the remaining
    /// columns are inert.
    fn row(value: &str) -> FulltextFeatureRow {
        FulltextFeatureRow {
            columns: std::iter::once(value.to_string())
                .chain((1..NATIVE_FIGURE_EMITTED_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        }
    }

    /// The template set of the frozen model: column 0 carries the token, column
    /// 25 closes the declared window. Without the second template the synthetic
    /// model would declare a narrower window and be refused.
    fn frozen_templates() -> Vec<CrfPattern> {
        vec![
            CrfPattern {
                source: "u:%x[0,0]".into(),
                kind: 'u',
            },
            CrfPattern {
                source: "u:%x[0,25]".into(),
                kind: 'u',
            },
        ]
    }

    /// Synthetic one-token-per-row model over the closed seven-label dictionary
    /// that forces `labels[row]` at row `row`: every position owns a distinct
    /// unigram observation and that observation weights exactly one label
    /// strictly higher than the rest (no `b:` observation is declared, so all
    /// transitions are zero).
    fn model_emitting(labels: &[&str]) -> (Vec<FulltextFeatureRow>, CrfModel) {
        let rows = (0..labels.len())
            .map(|index| row(&format!("t{index}")))
            .collect::<Vec<_>>();
        let vocabulary: Vec<String> = NATIVE_FIGURE_LABELS
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
            let target = NATIVE_FIGURE_LABELS
                .iter()
                .position(|known| known == label)
                .expect("requested label is part of the closed dictionary");
            weights.insert(offset + target, 3.0);
        }
        let model = CrfModel {
            model_type: 2,
            labels: vocabulary,
            observations,
            patterns: frozen_templates(),
            u_offsets,
            b_offsets: vec![None; labels.len()],
            weights,
            nftr: labels.len() * label_count,
        };
        (rows, model)
    }

    fn decoded(labels: &[&str]) -> NativeFigureDecodeOutcome {
        let (rows, model) = model_emitting(labels);
        decode_figure_sequence(&rows, &model, &[]).expect("synthetic sequence must decode")
    }

    fn span(labels: &[&str]) -> FigureSpan {
        match decoded(labels) {
            NativeFigureDecodeOutcome::Decoded { spans, .. } => {
                spans.into_iter().next().expect("one span per decode")
            }
            other => panic!("a figure sequence must decode, got {other:?}"),
        }
    }

    /// The closed dictionary is exactly the DATA transcription, and every
    /// spelling strips to one of the four base roles.
    #[test]
    fn the_dictionary_is_the_four_native_roles_in_both_spellings() {
        assert_eq!(NATIVE_FIGURE_LABELS.len(), 7);
        assert_eq!(NATIVE_FIGURE_BASE_LABELS.len(), 4);
        for label in NATIVE_FIGURE_LABELS {
            assert!(figure_label_is_admissible(label));
        }
        assert!(!figure_label_is_admissible("<other>"));
        assert!(!figure_label_is_admissible("<note>"));
        assert!(!figure_label_is_admissible(""));
        assert_eq!(NATIVE_FIGURE_EMITTED_COLUMNS, 27);
        assert_eq!(NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, 25);
        // The `I-` spelling of `<label>` is the only one the frozen model
        // declares, so the dictionary is not the simple base×2 cross product.
        assert!(NATIVE_FIGURE_LABELS.contains(&"I-<label>"));
        assert!(!NATIVE_FIGURE_LABELS.contains(&"<label>"));
    }

    #[test]
    fn an_inside_spelling_opens_a_new_cluster_and_a_bare_repeat_does_not() {
        let split = span(&["I-<content>", "<content>"]);
        assert_eq!(split.content.as_deref(), Some("t0 t1"));
        assert_eq!(split.cluster_index, 0);

        // A bare repeat merges, so the joined value carries both tokens exactly
        // once — the two rules must not collapse into one.
        let merged = span(&["<figure_head>", "<figure_head>"]);
        assert_eq!(merged.head.as_deref(), Some("t0 t1"));
    }

    #[test]
    fn every_role_maps_to_its_own_field_and_absent_roles_stay_none() {
        let filled = span(&["I-<label>", "<figure_head>", "<figDesc>", "<content>"]);
        assert_eq!(filled.label.as_deref(), Some("t0"));
        assert_eq!(filled.head.as_deref(), Some("t1"));
        assert_eq!(filled.caption.as_deref(), Some("t2"));
        assert_eq!(filled.content.as_deref(), Some("t3"));

        let partial = span(&["<figDesc>"]);
        assert_eq!(partial.caption.as_deref(), Some("t0"));
        assert_eq!(partial.label, None);
        assert_eq!(partial.head, None);
        assert_eq!(partial.content, None);
    }

    /// A role the decode split across two clusters keeps both parts; an
    /// all-empty cluster stays `None` instead of becoming the empty string.
    #[test]
    fn repeated_roles_join_and_empty_text_is_none() {
        let split = span(&["<content>", "<figDesc>", "<content>"]);
        assert_eq!(split.content.as_deref(), Some("t0 t2"));
        assert_eq!(split.caption.as_deref(), Some("t1"));

        let (rows, model) = model_emitting(&["<content>"]);
        let mut blank = rows;
        blank[0].columns[0] = String::new();
        let outcome = decode_figure_sequence(&blank, &model, &[]).unwrap();
        match outcome {
            NativeFigureDecodeOutcome::Decoded { spans, .. } => {
                assert_eq!(spans[0].content, None, "empty text is None, not Some(\"\")");
            }
            other => panic!("expected a decode, got {other:?}"),
        }
    }

    #[test]
    fn refuses_empty_sequence_before_any_model_work() {
        let (_, model) = model_emitting(&["<content>"]);
        assert_eq!(
            decode_figure_sequence(&[], &model, &[]),
            Err(NativeFigureSequenceError::EmptySequence)
        );
    }

    #[test]
    fn refuses_rows_that_do_not_carry_the_27_column_abi() {
        let (_, model) = model_emitting(&["<content>"]);
        for width in [26usize, 28] {
            let mut bad = row("t0");
            while bad.width() < width {
                bad.columns.push("0".into());
            }
            bad.columns.truncate(width);
            assert_eq!(
                decode_figure_sequence(&[bad], &model, &[]),
                Err(NativeFigureSequenceError::WrongArity {
                    row: 0,
                    actual: width,
                    expected: NATIVE_FIGURE_EMITTED_COLUMNS,
                })
            );
        }
    }

    #[test]
    fn refuses_a_model_whose_label_block_has_the_wrong_size() {
        let (rows, mut model) = model_emitting(&["<content>"]);
        model.labels.pop();
        assert_eq!(
            decode_figure_sequence(&rows, &model, &[]),
            Err(NativeFigureSequenceError::ModelLabelCount {
                expected: 7,
                actual: 6,
            })
        );
    }

    /// Both directions of a same-sized divergence are reported at once, and the
    /// count check alone cannot hide them.
    #[test]
    fn refuses_a_same_sized_block_that_diverges_from_the_dictionary() {
        let (rows, mut model) = model_emitting(&["<content>"]);
        let index = model
            .labels
            .iter()
            .position(|label| label == "<content>")
            .expect("the dictionary carries <content>");
        model.labels[index] = "<other>".into();
        assert_eq!(model.labels.len(), 7);
        assert_eq!(
            decode_figure_sequence(&rows, &model, &[]),
            Err(NativeFigureSequenceError::ModelLabelMismatch {
                missing: vec!["<content>".to_string()],
                unexpected: vec!["<other>".to_string()],
            })
        );

        let (rows, mut model) = model_emitting(&["<content>"]);
        model.labels[0] = "<nothing>".into();
        match decode_figure_sequence(&rows, &model, &[]) {
            Err(NativeFigureSequenceError::ModelLabelMismatch {
                missing,
                unexpected,
            }) => {
                assert_eq!(missing, ["I-<figure_head>"]);
                assert_eq!(unexpected, ["<nothing>"]);
            }
            other => panic!("a diverging label block must be refused, got {other:?}"),
        }
    }

    /// The frozen window is `0..=25`: a narrower model (max 24) and a model that
    /// reaches column 26 are both refused, and the refusal happens *before* the
    /// decode — the 26-case would otherwise expand cleanly, because a 27-column
    /// row really does carry column 26.
    #[test]
    fn refuses_a_model_whose_referenced_window_is_not_the_frozen_one() {
        for declared in [24usize, 26] {
            let (rows, mut model) = model_emitting(&["<content>"]);
            model.patterns[1] = CrfPattern {
                source: format!("u:%x[0,{declared}]"),
                kind: 'u',
            };
            assert_eq!(
                decode_figure_sequence(&rows, &model, &[]),
                Err(NativeFigureSequenceError::DeclaredColumnMismatch {
                    declared: Some(declared),
                    expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
                })
            );
        }

        // The guard is load-bearing: the decoder would accept this row.
        let (rows, mut model) = model_emitting(&["<content>"]);
        model.patterns[1] = CrfPattern {
            source: "u:%x[0,26]".into(),
            kind: 'u',
        };
        let rendered = render_fulltext_feature_text(&rows).unwrap();
        assert_eq!(da_ports::map_input(&model, &rendered).unwrap().len(), 1);

        // A model that references no column at all cannot be the frozen model.
        let (rows, mut model) = model_emitting(&["<content>"]);
        model.patterns = vec![CrfPattern {
            source: "u:literal".into(),
            kind: 'u',
        }];
        assert_eq!(
            decode_figure_sequence(&rows, &model, &[]),
            Err(NativeFigureSequenceError::DeclaredColumnMismatch {
                declared: None,
                expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
            })
        );
    }

    /// A malformed template is a typed `Model` refusal, not a panic and not a
    /// silently skipped reference.
    #[test]
    fn surfaces_a_malformed_template_as_a_typed_refusal() {
        for source in ["u:%x[0,25", "u:%x[0]", "u:%x[0,2,5]", "u:%x[0,x]"] {
            let (rows, mut model) = model_emitting(&["<content>"]);
            model.patterns[1] = CrfPattern {
                source: source.into(),
                kind: 'u',
            };
            assert!(
                matches!(
                    decode_figure_sequence(&rows, &model, &[]),
                    Err(NativeFigureSequenceError::Model { .. })
                ),
                "{source} must be a typed model refusal"
            );
        }
    }

    #[test]
    fn refuses_a_label_outside_the_dictionary_in_the_span_seam() {
        let rows = vec![row("t0")];
        assert_eq!(
            figure_spans_from_decoded(&["<other>".to_string()], &rows, &[]),
            Err(NativeFigureSequenceError::UnmodeledLabel {
                label: "<other>".to_string(),
                row: 0,
            })
        );
        assert_eq!(
            figure_spans_from_decoded(&["<content>".to_string()], &[], &[]),
            Err(NativeFigureSequenceError::CountMismatch {
                expected: 1,
                actual: 0,
            })
        );
    }

    /// H12-N1: no figure label at all is an absence, never a figure with zero
    /// parts — and never an error either.
    #[test]
    fn an_empty_decode_is_not_observed_rather_than_zero_figures() {
        assert_eq!(
            figure_spans_from_decoded(&[], &[], &[]),
            Ok(NativeFigureDecodeOutcome::NotObserved {
                reason: NativeFigureAbsence::NoFigureClusters,
            })
        );
    }

    fn marker(digest_char: char, cluster_index: usize) -> NativeCaptionMarker {
        NativeCaptionMarker {
            digest: digest_char.to_string().repeat(64),
            cluster_index,
        }
    }

    #[test]
    fn a_valid_caption_inventory_is_provided_and_an_empty_one_is_not_observed() {
        let inventory = [marker('a', 0), marker('b', 1)];
        let (rows, model) = model_emitting(&["<content>", "<figDesc>"]);
        let provided = match decode_figure_sequence(&rows, &model, &inventory).unwrap() {
            NativeFigureDecodeOutcome::Decoded { mut spans, .. } => {
                assert_eq!(spans.len(), 1, "one span per decode");
                spans.remove(0)
            }
            other => panic!("expected a decode, got {other:?}"),
        };
        assert_eq!(provided.caption_links.absence, None);
        assert_eq!(
            provided
                .caption_links
                .markers
                .iter()
                .map(|marker| marker.digest.clone())
                .collect::<Vec<_>>(),
            [marker('a', 0).digest, marker('b', 1).digest]
        );

        let absent = span(&["<content>", "<figDesc>"]);
        assert_eq!(
            absent.caption_links,
            NativeCaptionLinks {
                markers: Vec::new(),
                absence: Some(NativeCaptionLinkAbsence::NotObserved),
            },
            "an empty inventory is an absence, not zero links"
        );
    }

    #[test]
    fn a_malformed_or_dangling_caption_inventory_is_refused() {
        let (rows, model) = model_emitting(&["<content>", "<figDesc>"]);

        for bad in [
            NativeCaptionMarker {
                digest: "abc".into(),
                cluster_index: 0,
            },
            NativeCaptionMarker {
                digest: "A".repeat(64),
                cluster_index: 0,
            },
        ] {
            assert_eq!(
                decode_figure_sequence(&rows, &model, &[bad]),
                Err(NativeFigureSequenceError::InvalidCaptionInventory)
            );
        }

        let duplicated = [marker('c', 0), marker('c', 1)];
        assert_eq!(
            decode_figure_sequence(&rows, &model, &duplicated),
            Err(NativeFigureSequenceError::InvalidCaptionInventory)
        );

        assert_eq!(
            decode_figure_sequence(&rows, &model, &[marker('d', 2)]),
            Err(NativeFigureSequenceError::DanglingCaptionCluster {
                cluster_index: 2,
                clusters: 2,
            })
        );
        // The last cluster of the decode is admissible.
        assert!(decode_figure_sequence(&rows, &model, &[marker('d', 1)]).is_ok());
    }

    /// The run cap refuses instead of truncating: exactly the cap is admissible,
    /// cap+1 is a typed refusal.
    #[test]
    fn the_row_cap_refuses_rather_than_truncating() {
        let (_, model) = model_emitting(&["<content>"]);
        let over = vec![row("t0"); MAX_NATIVE_FIGURE_ROWS + 1];
        assert_eq!(
            decode_figure_sequence(&over, &model, &[]),
            Err(NativeFigureSequenceError::TooManyRows {
                actual: MAX_NATIVE_FIGURE_ROWS + 1,
                maximum: MAX_NATIVE_FIGURE_ROWS,
            })
        );
    }

    /// Equal inputs produce an equal decode, byte for byte through the stable
    /// serde projection.
    #[test]
    fn the_decode_is_deterministic_byte_for_byte() {
        let (rows, model) = model_emitting(&["<content>", "I-<figDesc>", "I-<label>"]);
        let first = decode_figure_sequence(&rows, &model, &[marker('e', 0)]).unwrap();
        let second = decode_figure_sequence(&rows, &model, &[marker('e', 0)]).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    /// The span wire shape is pinned: field order, the `None` roles and the
    /// typed caption absence are all part of the projection a consumer reads.
    #[test]
    fn the_span_projection_is_stable() {
        let absent = span(&["<content>"]);
        assert_eq!(
            serde_json::to_string(&absent).unwrap(),
            concat!(
                r#"{"label":null,"head":null,"caption":null,"content":"t0","#,
                r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}"#
            )
        );
        let round_tripped: FigureSpan =
            serde_json::from_str(&serde_json::to_string(&absent).unwrap())
                .expect("the span projection round-trips");
        assert_eq!(round_tripped, absent);

        let outcome = decoded(&["<content>", "<figDesc>"]);
        assert_eq!(
            serde_json::to_string(&outcome).unwrap(),
            concat!(
                r#"{"outcome":"decoded","labels":["<content>","<figDesc>"],"spans":["#,
                r#"{"label":null,"head":null,"caption":"t1","content":"t0","#,
                r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}]}"#
            )
        );
    }

    #[test]
    fn the_schema_version_names_the_surface() {
        assert_eq!(
            NATIVE_FIGURE_SEQUENCE_SCHEMA_VERSION,
            "da-parse.native-figure-sequence/v1"
        );
    }
}
