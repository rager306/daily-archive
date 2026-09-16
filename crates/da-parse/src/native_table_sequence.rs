//! Pure sequence runner for the native GROBID `table` CRF model.
//!
//! `TableParser.processing` labels the content block of a fulltext `<table>`
//! cluster with the **secondary** `table` model (`models/table/model.wapiti`,
//! `#mdl#2#1391`, `#rdr#85/25/0`) and turns the emitted labels into one or more
//! `Table` objects. This module is that second pass and its disposition rules: it
//! validates the already rendered rows, maps them through a supplied scalar CRF
//! model, groups the emitted labels with the shared `TaggingTokenClusteror` rule,
//! applies the Java `validateTable` rejection contract and assembles typed spans.
//!
//! ## The rows are the S05 fulltext ABI — there is no second table ABI
//!
//! Exactly as for [`crate::native_figure_sequence`], the features the secondary
//! model sees are fulltext rows *without* a predicted label, so this family
//! re-uses [`FulltextFeatureRow`] and [`NATIVE_FULLTEXT_FEATURE_COLUMNS`] verbatim
//! (27 printed columns). The frozen `table` templates reference columns `0..=25`
//! only (`template_excludes: [26]` — column 26 is `superscript`), so a model
//! whose templates reach any other column index is refused up front by
//! [`check_table_column_window`]. The guard is load-bearing rather than
//! decorative: a 27-column row really does carry column 26, so
//! `da_ports::map_input` would expand `%x[0,26]` cleanly without it.
//!
//! ## The closed ten-label dictionary
//!
//! `data/parse/native_visual_families.yaml` transcribes the frozen model's label
//! block verbatim in model order; [`NATIVE_TABLE_LABELS`] mirrors that list 1:1
//! (the contract test fails if either side drifts). The block is compared as a
//! *set*, and the two divergences are reported separately (`missing` /
//! `unexpected`) so the diagnostic names the actual defect. The table block adds
//! `<note>` and `I-<other>` to the figure roles; `java_declared_not_in_model` is
//! empty for this family, so there is no declared-but-unmodeled label to refuse.
//!
//! Role comparisons are made against the **plain** label, because
//! `TaggingLabels.labelFor` strips the `I-` prefix before constructing the
//! `TaggingLabel` the cluster carries (`GenericTaggerUtils.getPlainLabel`). Only
//! the *cluster boundary* is driven by the raw `I-` prefix.
//!
//! ## One fulltext cluster can hold several tables
//!
//! `TableParser.getExtractionResult` keeps a single `Table` open and closes it
//! when a **`<figure_head>` cluster is seen while the open table already carries
//! a header and the immediately preceding label was `<content>`, `<note>` or
//! `<figDesc>`** — the transcription [`cluster_opens_new_table`] implements,
//! including its two conditions. Two consequences are transcribed deliberately
//! rather than smoothed over: consecutive head clusters do *not* split (the
//! previous label is a head), and neither does a head after a `<label>`.
//! Grouping itself is [`cluster_family_label_runs`], never a second copy of it
//! (D359): `<content>,<content>` is one cluster, `<content>,I-<content>` is two.
//!
//! ## Rejection is typed and never silent (`TableParser.validateTable`)
//!
//! `Table.validateTable` rejects a table when any of `label`, `header` or
//! `content` is blank, when `Integer.valueOf(label.trim(), 10)` throws, or when
//! the header does not start with `tab` ignoring case. `FullTextParser` then
//! reverts exactly those tables (`badTables` → `revertResultsForBadItems`), which
//! is why the flag here is named *reverted*: the table is present and flagged
//! incomplete, never silently missing and never claiming completeness.
//!
//! Two transcription details matter and are pinned by the contract test:
//!
//! * the header `validateTable` inspects is Java's assembled header — the
//!   `<figure_head>` cluster texts appended verbatim, the `<label>` cluster text
//!   appended *wrapped in spaces* (`TableParser` calls
//!   `appendHeader(" " + content + " ")`) — so a table whose label cluster
//!   precedes its head cannot start with `tab` and is reverted. That ordering
//!   sensitivity is Java's, not a local choice;
//! * the label is trimmed before parsing (Java trims) while the header is *not*
//!   trimmed before the prefix test (Java does not trim), so a header that merely
//!   contains `tab` later is still reverted.
//!
//! `Table.validateTable` reports one combined counter for the blank case; the
//! typed [`NativeTableRevertReason`] refines that into three named defects in the
//! fixed order label → header → content. The *verdict* is identical to Java's in
//! every case; only the diagnostic is finer. The remaining rejection layers —
//! `Table.isCompleteForTEI` (a non-empty caption is also required),
//! `badTableAdvancedCheck` (page/bbox/area geometry) and
//! `LabelUtils.postProcessFulltextFixInvalidTableOrFigure` (D366) — need layout
//! geometry and are named `offline_limit`s of this wave, not silently assumed
//! true.
//!
//! ## Cells are caller-supplied geometry, and absence is not zero (H12-N1)
//!
//! Table cells do **not** come from the CRF. Java derives them from the layout
//! geometry (`Line`/`Row`/`Cell`, X/Y thresholds, `mergeMulticolumnCells`,
//! colspan). That algorithm is a named `offline_limit` and is deliberately not
//! re-invented: this runner accepts a **ready inventory** of cells from its
//! caller ([`NativeTableCell`], `row`/`column` being the layout units H12
//! counts). An absent inventory is [`NativeTableCells::NotObserved`] and never
//! "a table with zero rows or cells"; a caller that supplies an empty list gets
//! the same typed absence rather than an empty success. Because one decode can
//! hold several tables and one inventory carries no table binding, the inventory
//! is attributed to the first table only — every further table reports
//! [`NativeTableAbsence::AmbiguousInventory`] instead of silently duplicating
//! the caller's cells into a table they may not describe.
//!
//! ## The caption lane is typed, bounded, and does not resolve anything
//!
//! Every table carries the shared [`NativeCaptionLinks`] inventory
//! (`postProcessTableCaptions`), distributed by cluster range: a marker belongs
//! to the table whose clusters contain the cluster it addresses, so no marker is
//! duplicated across tables and none is dropped. An empty inventory — global or
//! per-table — is [`NativeCaptionLinkAbsence::NotObserved`] (H12-N1), a malformed
//! or duplicated digest refuses the whole decode, and a marker addressing a
//! cluster this decode does not contain is a typed refusal. Resolving a marker to
//! a reference stays with the already-shipped S04 [`crate::match_native_callouts`]
//! surface and is not repeated here.
//!
//! ## What this runner does *not* prove
//!
//! The real GROBID model bytes are not in the tree (D358), so the contract test
//! decodes a hand-authored synthetic model. Binding a real model's bytes to the
//! transcribed dictionary is the opt-in real-model harness's job (S06/T07).
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! model bytes, no absolute path. Every function is total on hostile input — an
//! empty sequence, a wrong-arity row, a foreign label block, a declared column
//! window outside `0..=25`, an over-long sequence, a malformed template, a
//! malformed cell or caption inventory, and a marker addressing a cluster that
//! does not exist are typed refusals, never a panic and never a silent
//! truncation. `import_eligible` stays false and nothing here is spliced into the
//! production parse route.

use crate::native_fulltext_features::{
    FulltextFeatureRow, NATIVE_FULLTEXT_FEATURE_COLUMNS, render_fulltext_feature_text,
};
use crate::native_visual_family_cluster::{
    FamilyLabelCluster, NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker,
    NativeVisualFamilyClusterError, caption_links_from_markers, cluster_family_label_runs,
};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Schema identity of the decoded table sequence surface.
pub const NATIVE_TABLE_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-table-sequence/v1";

/// Printed row width of the rows this family decodes.
///
/// A derived alias, not a second ABI: the secondary `table` model consumes the
/// same 27-column fulltext row the S05 builder prints.
pub const NATIVE_TABLE_EMITTED_COLUMNS: usize = NATIVE_FULLTEXT_FEATURE_COLUMNS;

/// Highest column index the frozen `table` templates reference
/// (`#rdr#85/25/0` → columns `0..=25`).
///
/// Column 26 (`superscript`) is printed by the ABI but never read by this model
/// (`template_excludes: [26]`), which is exactly the window
/// [`check_table_column_window`] enforces.
pub const NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX: usize = 25;

/// Rows admitted into one table decode, mirroring the S05 runner's bound.
///
/// Provisional and fail-closed: an input beyond this bound is refused rather
/// than truncated, so no silently partial read can be mistaken for a complete
/// one.
pub const MAX_NATIVE_TABLE_ROWS: usize = 32_768;

/// The closed decoded-label vocabulary of `models/table/model.wapiti`.
///
/// Transcribed in the frozen model's own order, mirroring
/// `families.table.declared_labels` in `data/parse/native_visual_families.yaml`
/// 1:1. The order is a transcription fact and carries no meaning to the decoder:
/// the label-block check compares sets, and the loaded model owns its own order.
pub const NATIVE_TABLE_LABELS: [&str; 10] = [
    "I-<other>",
    "I-<figure_head>",
    "I-<label>",
    "I-<figDesc>",
    "<figDesc>",
    "I-<content>",
    "<content>",
    "I-<note>",
    "<note>",
    "<figure_head>",
];

/// The six base roles of the table family, mirroring
/// `families.table.base_labels` in the DATA file.
pub const NATIVE_TABLE_BASE_LABELS: [&str; 6] = [
    "<other>",
    "<figure_head>",
    "<label>",
    "<figDesc>",
    "<content>",
    "<note>",
];

/// The plain label of a table head (`TaggingLabels.TBL_HEAD`).
pub const NATIVE_TABLE_HEAD_BASE: &str = "<figure_head>";

/// The plain label of a table label (`TaggingLabels.TBL_LABEL`).
pub const NATIVE_TABLE_LABEL_BASE: &str = "<label>";

/// The previous labels that make a head cluster start a new table
/// (`TableParser.getExtractionResult`).
const NATIVE_TABLE_SPLIT_PREVIOUS_BASES: [&str; 3] = ["<content>", "<note>", "<figDesc>"];

/// The `tab` prefix `Table.validateTable` requires on the header, compared
/// ignoring case (`StringUtils.startsWithIgnoreCase`).
const NATIVE_TABLE_HEAD_PREFIX: &str = "tab";

/// One table cell as a caller-supplied layout fact.
///
/// `row`/`column` are the layout *units* H12 counts (`Line`/`Row`/`Cell`), not
/// indices into this decode: the runner never derives geometry, it only carries
/// the caller's inventory. `content_digest` is the cell's digest-only identity,
/// in the same lower-hex-64 dialect as the caption markers, so no raw cell text
/// enters the shadow band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTableCell {
    pub row: u32,
    pub column: u32,
    pub content_digest: String,
}

/// Closed vocabulary of table absences.
///
/// [`Self::NoTableClusters`] is the decode-level H12-N1 witness (an empty decode
/// is an absence, not a table with zero parts); [`Self::NoCellInventory`] and
/// [`Self::AmbiguousInventory`] are the cell-level witnesses (no inventory, or an
/// inventory that cannot be attributed to one of several tables).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTableAbsence {
    /// The decoded label sequence carried no table-family cluster at all.
    NoTableClusters,
    /// The caller supplied no cell inventory for this table.
    NoCellInventory,
    /// The decode holds several tables, so one caller-supplied inventory cannot
    /// be attributed to this one.
    AmbiguousInventory,
}

/// The cell inventory of one table, or its typed absence.
///
/// A provided-but-empty vector is never representable as a success: the only
/// constructor [`table_cells_from_inventory`] maps it to [`Self::NotObserved`],
/// and [`table_spans_from_decoded`] normalizes a hand-built empty `Provided` the
/// same way (H12-N1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NativeTableCells {
    NotObserved { reason: NativeTableAbsence },
    Provided { cells: Vec<NativeTableCell> },
}

/// Why `Table.validateTable` would revert an assembled table.
///
/// The three reasons refine Java's two rules plus its combined blank guard: a
/// blank label cannot parse as an integer, a blank header cannot start with
/// `tab`. `FullTextParser` reverts such tables, hence the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTableRevertReason {
    /// The label is absent, blank, or not a 32-bit decimal integer.
    LabelNotInteger,
    /// The assembled header is absent, blank, or does not start with `tab`.
    HeadNotTabPrefixed,
    /// No content cluster carried any text.
    EmptyContent,
}

/// One table assembled from the clusters of a decoded table sequence.
///
/// A role the model did not emit stays `None` — never `Some("")` — so "the model
/// emitted no such cluster" and "the cluster was empty" stay distinguishable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTableSpan {
    /// `<label>` — the table's printed number.
    pub label: Option<String>,
    /// `<figure_head>` — the table heading line.
    pub head: Option<String>,
    /// `<figDesc>` — the table description (caption body).
    pub caption: Option<String>,
    /// `<content>` — the table body text.
    pub content: Option<String>,
    /// `<note>` — the table note.
    pub note: Option<String>,
    /// `<other>` — text Java discards (`addDiscardedPieceTokens`). Carried
    /// typed rather than dropped, so a consumer can see what the model could not
    /// place instead of it vanishing silently.
    pub other: Option<String>,
    /// `Some(reason)` when `validateTable` would revert this table.
    ///
    /// The verdict is computed from Java's assembled header — the head cluster
    /// texts appended verbatim with the label text wrapped in spaces (see the
    /// module docs) — which [`Self::head`] and [`Self::label`] expose unmerged.
    pub reverted: Option<NativeTableRevertReason>,
    /// The caller-supplied cell inventory, or its typed absence.
    pub cells: NativeTableCells,
    /// The caption-marker inventory of this table's cluster range, or its typed
    /// absence.
    pub caption_links: NativeCaptionLinks,
    /// Index of the [`FamilyLabelCluster`] that opened this table.
    pub cluster_index: usize,
}

/// Typed result of one table decode: the assembled tables, or the absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeTableDecodeOutcome {
    Decoded {
        /// One decoded label per row, in reading order, spelling preserved.
        labels: Vec<String>,
        /// The assembled tables, in reading order. One decode can hold several.
        tables: Vec<NativeTableSpan>,
    },
    NotObserved {
        reason: NativeTableAbsence,
    },
}

/// Typed refusal vocabulary for the table sequence runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTableSequenceError {
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
    /// The supplied cell inventory was malformed: a digest outside the shared
    /// lower-hex-64 dialect, or two cells claiming the same `(row, column)`.
    InvalidCellInventory,
    /// The shared `TaggingTokenClusteror` grouping pass refused the labels.
    Grouping {
        error: NativeVisualFamilyClusterError,
    },
    /// The 27-column row serializer refused the rows handed to the runner.
    Feature { message: String },
    /// The scalar CRF itself refused the input.
    Model { message: String },
}

impl fmt::Display for NativeTableSequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(formatter, "native table sequence is empty"),
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "native table sequence has {actual} rows; maximum is {maximum}"
            ),
            Self::WrongArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "native table row {row} has {actual} columns; expected {expected}"
            ),
            Self::ModelLabelCount { expected, actual } => write!(
                formatter,
                "table model declares {actual} labels; the closed dictionary has {expected}"
            ),
            Self::ModelLabelMismatch {
                missing,
                unexpected,
            } => write!(
                formatter,
                "table model label block diverges from the native dictionary \
                 (missing {missing:?}, unexpected {unexpected:?})"
            ),
            Self::DeclaredColumnMismatch { declared, expected } => write!(
                formatter,
                "table model references column {declared:?}; the frozen window ends at {expected}"
            ),
            Self::UnmodeledLabel { label, row } => write!(
                formatter,
                "table label {label:?} at row {row} is not part of the closed dictionary"
            ),
            Self::CountMismatch { expected, actual } => write!(
                formatter,
                "table decode produced {actual} values for {expected} positions"
            ),
            Self::InvalidCaptionInventory => {
                write!(formatter, "table caption-marker inventory is malformed")
            }
            Self::DanglingCaptionCluster {
                cluster_index,
                clusters,
            } => write!(
                formatter,
                "table caption marker addresses cluster {cluster_index}, but the decode has \
                 {clusters} clusters"
            ),
            Self::InvalidCellInventory => write!(formatter, "table cell inventory is malformed"),
            Self::Grouping { error } => write!(formatter, "table label grouping failed: {error}"),
            Self::Feature { message } => {
                write!(formatter, "table row rendering failed: {message}")
            }
            Self::Model { message } => write!(formatter, "table CRF failed: {message}"),
        }
    }
}

impl std::error::Error for NativeTableSequenceError {}

/// Is `label` (either spelling) part of the closed native vocabulary?
pub fn table_label_is_admissible(label: &str) -> bool {
    NATIVE_TABLE_LABELS.contains(&label)
}

/// Fail closed when a model's declared label block is not the native set.
///
/// Two distinct conditions are reported separately so the diagnostic names the
/// actual defect: a wrong cardinality, and a same-sized block that still
/// diverges from the dictionary. The comparison is order-independent: a model
/// may list the spellings in any order, because the loaded model owns the label
/// ids its weight blocks index.
pub fn check_table_label_block(model: &CrfModel) -> Result<(), NativeTableSequenceError> {
    if model.labels.len() != NATIVE_TABLE_LABELS.len() {
        return Err(NativeTableSequenceError::ModelLabelCount {
            expected: NATIVE_TABLE_LABELS.len(),
            actual: model.labels.len(),
        });
    }
    let mut missing: Vec<String> = NATIVE_TABLE_LABELS
        .iter()
        .filter(|label| !model.labels.iter().any(|declared| declared == *label))
        .map(|label| (*label).to_string())
        .collect();
    let mut unexpected: Vec<String> = model
        .labels
        .iter()
        .filter(|declared| !NATIVE_TABLE_LABELS.contains(&declared.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    unexpected.sort_unstable();
    unexpected.dedup();
    Err(NativeTableSequenceError::ModelLabelMismatch {
        missing,
        unexpected,
    })
}

/// Fail closed when the model's referenced column window is not the frozen one.
///
/// The frozen `table` model declares `#rdr#85/25/0`: its templates reference
/// columns `0..=25` of the printed 27-column fulltext row and never column 26
/// (`superscript`). Because a 27-column row *does* carry column 26,
/// `da_ports::map_input` would expand `%x[0,26]` successfully — so this check is
/// the only thing standing between a drifted model and the silent consumption of
/// a column the Java side never feeds this model. The window is read off the
/// model's own templates, which is the only place a loaded [`CrfModel`] declares
/// a column at all.
pub fn check_table_column_window(model: &CrfModel) -> Result<(), NativeTableSequenceError> {
    let declared = template_max_column_index(model).map_err(model_error)?;
    if declared == Some(NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX) {
        return Ok(());
    }
    Err(NativeTableSequenceError::DeclaredColumnMismatch {
        declared,
        expected: NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
    })
}

/// Does this cluster start a new table?
///
/// The transcription of `TableParser.getExtractionResult`: a `<figure_head>`
/// cluster starts a new table exactly when the open table **already has a
/// header** and the immediately preceding cluster was a `<content>`, `<note>` or
/// `<figDesc>`. Both conditions are Java's; the first is why a head after a
/// `<label>`/head pair does not split, and the second is why two consecutive
/// head clusters merge into the same table.
///
/// `base` is the *plain* label (the `I-` spelling is stripped before the
/// comparison, as `TaggingLabels.labelFor` does); the `I-` prefix only matters
/// for the cluster boundary itself.
pub fn cluster_opens_new_table(
    base: &str,
    open_table_has_header: bool,
    previous_base: Option<&str>,
) -> bool {
    base == NATIVE_TABLE_HEAD_BASE
        && open_table_has_header
        && previous_base
            .is_some_and(|previous| NATIVE_TABLE_SPLIT_PREVIOUS_BASES.contains(&previous))
}

/// Why `Table.validateTable` would revert a table, or `None` when it is valid.
///
/// `label`, `header` and `content` are Java's `Table` fields, so `header` is the
/// **assembled** header (head cluster texts verbatim, label text wrapped in
/// spaces) and not merely the span's `head` field. The rules, in the fixed order
/// they are applied (only the first failure is reported, so the flag is a reason
/// and not a bitmask):
///
/// 1. a blank label, header or content reverts — Java's combined
///    `isAnyBlank(label, header, content)` guard, refined here into the closest
///    of the three typed reasons;
/// 2. the label must parse as a 32-bit decimal integer after trimming
///    (`Integer.valueOf(getLabel().trim(), 10)`);
/// 3. the header must start with `tab` ignoring case, and is **not** trimmed
///    first — Java does not trim either, so a label cluster preceding the head
///    (`" 1 tab ..."`) is reverted rather than repaired.
pub fn table_revert_reason(
    label: Option<&str>,
    header: Option<&str>,
    content: Option<&str>,
) -> Option<NativeTableRevertReason> {
    let label = non_blank(label);
    let header = non_blank(header);
    let content = non_blank(content);
    // Java's blank guard reports one combined counter; naming the closest typed
    // reason keeps the identical verdict (any blank reverts) with a finer signal.
    let Some(label) = label else {
        return Some(NativeTableRevertReason::LabelNotInteger);
    };
    let Some(header) = header else {
        return Some(NativeTableRevertReason::HeadNotTabPrefixed);
    };
    if label.parse::<i32>().is_err() {
        return Some(NativeTableRevertReason::LabelNotInteger);
    }
    if !header_starts_with_table_word(header) {
        return Some(NativeTableRevertReason::HeadNotTabPrefixed);
    }
    if content.is_none() {
        return Some(NativeTableRevertReason::EmptyContent);
    }
    None
}

/// Type a caller-supplied cell inventory, applying the H12-N1 rule.
///
/// * an empty vector is [`NativeTableCells::NotObserved`] with
///   [`NativeTableAbsence::NoCellInventory`] — never an empty success, because
///   "absent" and "zero" must stay distinguishable;
/// * every `content_digest` must be one lower-hex-64 digest in the shared
///   caption-marker dialect, and no two cells may claim the same
///   `(row, column)`: a duplicated position is not two cells.
///
/// Validation is all-or-nothing, so a caller can never act on a partially valid
/// inventory.
pub fn table_cells_from_inventory(
    cells: Vec<NativeTableCell>,
) -> Result<NativeTableCells, NativeTableSequenceError> {
    if cells.is_empty() {
        return Ok(NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        });
    }
    validate_cells(&cells)?;
    Ok(NativeTableCells::Provided { cells })
}

/// Decode rendered fulltext rows through the supplied `table` model.
///
/// No model loading, filesystem access, environment lookup, or fallback is
/// performed here. The rows are re-serialized through
/// [`render_fulltext_feature_text`] so the text the model sees is exactly the ABI
/// the S05 row builder produced, and that text is what `da_ports::map_input`
/// takes. Equal inputs and model values always produce equal output.
///
/// `markers` is the caller's caption inventory and `cells` the caller's cell
/// inventory; neither is resolved or derived here.
pub fn decode_table_sequence(
    rows: &[FulltextFeatureRow],
    model: &CrfModel,
    markers: &[NativeCaptionMarker],
    cells: NativeTableCells,
) -> Result<NativeTableDecodeOutcome, NativeTableSequenceError> {
    if rows.is_empty() {
        return Err(NativeTableSequenceError::EmptySequence);
    }
    if rows.len() > MAX_NATIVE_TABLE_ROWS {
        return Err(NativeTableSequenceError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_NATIVE_TABLE_ROWS,
        });
    }
    check_table_label_block(model)?;
    check_table_column_window(model)?;
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_TABLE_EMITTED_COLUMNS {
            return Err(NativeTableSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_TABLE_EMITTED_COLUMNS,
            });
        }
    }

    let input =
        render_fulltext_feature_text(rows).map_err(|error| NativeTableSequenceError::Feature {
            message: error.to_string(),
        })?;
    let positions = map_input(model, &input).map_err(model_error)?;
    if positions.len() != rows.len() {
        return Err(NativeTableSequenceError::CountMismatch {
            expected: rows.len(),
            actual: positions.len(),
        });
    }
    let decoded = viterbi_decode(model, &positions).map_err(model_error)?;
    if decoded.labels.len() != rows.len() {
        return Err(NativeTableSequenceError::CountMismatch {
            expected: rows.len(),
            actual: decoded.labels.len(),
        });
    }
    table_spans_from_decoded(&decoded.labels, rows, markers, cells)
}

/// Assemble the tables of an already-decoded label sequence.
///
/// This is the seam the decoder above calls, kept public because it owns the
/// H12-N1 rule and the split rule, and because a caller that assembled labels
/// elsewhere still needs the same assembly. `rows` supplies the token text of
/// each decoded position (column 0 of the printed fulltext row); `markers` and
/// `cells` are the caller's inventories.
///
/// Roles map one-to-one onto the span fields: `<label>` → `label`,
/// `<figure_head>` → `head`, `<figDesc>` → `caption`, `<content>` → `content`,
/// `<note>` → `note`, `<other>` → `other`. A role the decode did not emit stays
/// `None`; a role the decode split across two clusters joins its parts with a
/// single space rather than dropping one of them. The split and the Java header
/// accumulation happen in the same single pass, so the two can never disagree.
pub fn table_spans_from_decoded(
    labels: &[String],
    rows: &[FulltextFeatureRow],
    markers: &[NativeCaptionMarker],
    cells: NativeTableCells,
) -> Result<NativeTableDecodeOutcome, NativeTableSequenceError> {
    if labels.is_empty() {
        return Ok(NativeTableDecodeOutcome::NotObserved {
            reason: NativeTableAbsence::NoTableClusters,
        });
    }
    if labels.len() != rows.len() {
        return Err(NativeTableSequenceError::CountMismatch {
            expected: labels.len(),
            actual: rows.len(),
        });
    }
    for (row, label) in labels.iter().enumerate() {
        if !table_label_is_admissible(label) {
            return Err(NativeTableSequenceError::UnmodeledLabel {
                label: label.clone(),
                row,
            });
        }
    }
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_TABLE_EMITTED_COLUMNS {
            return Err(NativeTableSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_TABLE_EMITTED_COLUMNS,
            });
        }
    }
    let clusters = cluster_family_label_runs(labels)
        .map_err(|error| NativeTableSequenceError::Grouping { error })?;

    // The caption inventory is validated as a whole first: a malformed digest
    // must not decay into "no links observed", and a duplicated digest is not two
    // captions. The per-table subsets below therefore only re-run the shared
    // normalizer on already valid markers.
    let whole_inventory = caption_links_from_markers(markers.to_vec());
    if whole_inventory.absence == Some(NativeCaptionLinkAbsence::InvalidDigest) {
        return Err(NativeTableSequenceError::InvalidCaptionInventory);
    }
    for marker in markers {
        if marker.cluster_index >= clusters.len() {
            return Err(NativeTableSequenceError::DanglingCaptionCluster {
                cluster_index: marker.cluster_index,
                clusters: clusters.len(),
            });
        }
    }

    // H12-N1, defensively as well as at the constructor: a provided-but-empty
    // inventory is an absence, never "a table with zero cells".
    let cells = match cells {
        NativeTableCells::Provided { cells } if cells.is_empty() => NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        },
        provided => {
            if let NativeTableCells::Provided { cells } = &provided {
                validate_cells(cells)?;
            }
            provided
        }
    };

    let mut pending_cells = Some(cells);
    let mut tables: Vec<NativeTableSpan> = Vec::new();
    let mut current = empty_table(0);
    let mut current_start = 0usize;
    // Java's assembled header: head cluster texts verbatim, label text wrapped in
    // spaces (`TableParser` calls `appendHeader(" " + content + " ")`). It is both
    // the split condition and the value `validateTable` inspects.
    let mut header = String::new();
    let mut previous: Option<&str> = None;

    for (index, cluster) in clusters.iter().enumerate() {
        if index > 0 && cluster_opens_new_table(&cluster.base, !header.is_empty(), previous) {
            let closed = close_table(
                current,
                header.as_str(),
                current_start,
                index,
                markers,
                pending_cells.take(),
                tables.len(),
            );
            tables.push(closed);
            current = empty_table(index);
            current_start = index;
            header.clear();
        }
        let text = cluster_text(rows, cluster)?;
        match cluster.base.as_str() {
            NATIVE_TABLE_LABEL_BASE => {
                append_role_text(&mut current.label, &text);
                header.push(' ');
                header.push_str(&text);
                header.push(' ');
            }
            NATIVE_TABLE_HEAD_BASE => {
                append_role_text(&mut current.head, &text);
                header.push_str(&text);
            }
            "<figDesc>" => append_role_text(&mut current.caption, &text),
            "<content>" => append_role_text(&mut current.content, &text),
            "<note>" => append_role_text(&mut current.note, &text),
            "<other>" => append_role_text(&mut current.other, &text),
            // The admissibility guard above already proves this branch cannot be
            // taken; it stays a typed refusal instead of a panic so hostile input
            // can never abort a caller.
            other => {
                return Err(NativeTableSequenceError::UnmodeledLabel {
                    label: other.to_string(),
                    row: cluster.start,
                });
            }
        };
        previous = Some(cluster.base.as_str());
    }

    let last = close_table(
        current,
        header.as_str(),
        current_start,
        clusters.len(),
        markers,
        pending_cells.take(),
        tables.len(),
    );
    tables.push(last);

    Ok(NativeTableDecodeOutcome::Decoded {
        labels: labels.to_vec(),
        tables,
    })
}

/// An assembled table with no role filled and no inventory attributed yet.
fn empty_table(cluster_index: usize) -> NativeTableSpan {
    NativeTableSpan {
        label: None,
        head: None,
        caption: None,
        content: None,
        note: None,
        other: None,
        reverted: None,
        cells: NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        },
        caption_links: NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(NativeCaptionLinkAbsence::NotObserved),
        },
        cluster_index,
    }
}

/// Finish one table: attribute the inventories and apply `validateTable`.
///
/// `pending_cells` is the caller's single inventory, taken by the first table
/// only; every further table reports the typed ambiguity instead of silently
/// duplicating cells into a table they may not describe. `caption_links` is the
/// marker subset addressing a cluster in `start..end`, and `reverted` is
/// `validateTable` evaluated on Java's assembled `header`.
fn close_table(
    mut table: NativeTableSpan,
    header: &str,
    start: usize,
    end: usize,
    markers: &[NativeCaptionMarker],
    pending_cells: Option<NativeTableCells>,
    table_index: usize,
) -> NativeTableSpan {
    table.cells = if table_index == 0 {
        pending_cells.unwrap_or(NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        })
    } else {
        NativeTableCells::NotObserved {
            reason: NativeTableAbsence::AmbiguousInventory,
        }
    };
    table.caption_links = caption_links_in_range(markers, start, end);
    table.reverted = table_revert_reason(
        table.label.as_deref(),
        Some(header),
        table.content.as_deref(),
    );
    table
}

/// The caption inventory of one table: the markers addressing a cluster in
/// `start..end`, re-normalized through the shared lane.
///
/// Distributing rather than duplicating is the point: a marker belongs to exactly
/// one table, so no marker is counted twice and none is dropped. A table whose
/// range holds no marker reports [`NativeCaptionLinkAbsence::NotObserved`] — an
/// absence, never "zero links".
fn caption_links_in_range(
    markers: &[NativeCaptionMarker],
    start: usize,
    end: usize,
) -> NativeCaptionLinks {
    let subset: Vec<NativeCaptionMarker> = markers
        .iter()
        .filter(|marker| marker.cluster_index >= start && marker.cluster_index < end)
        .cloned()
        .collect();
    caption_links_from_markers(subset)
}

/// `Some(trimmed)` when the text carries a non-blank character, else `None`.
///
/// This is `StringUtils.isBlank`'s semantics, which `Table.validateTable` uses
/// for its combined guard: whitespace-only is blank, not merely empty.
fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

/// Does `header` start with `tab` ignoring case (`startsWithIgnoreCase`)?
///
/// The header is deliberately not trimmed first, because Java does not trim it
/// either: the check is on the assembled string as it stands.
fn header_starts_with_table_word(header: &str) -> bool {
    header
        .get(..NATIVE_TABLE_HEAD_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(NATIVE_TABLE_HEAD_PREFIX))
}

/// Validate a non-empty cell inventory: shared digest dialect, unique positions.
fn validate_cells(cells: &[NativeTableCell]) -> Result<(), NativeTableSequenceError> {
    let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
    for cell in cells {
        // The digest dialect is the caption lane's, validated through the shared
        // band so the crate keeps exactly one definition of a digest.
        if !is_shared_digest(&cell.content_digest) || !seen.insert((cell.row, cell.column)) {
            return Err(NativeTableSequenceError::InvalidCellInventory);
        }
    }
    Ok(())
}

/// Is `value` one accepted digest in the shared caption-marker dialect?
fn is_shared_digest(value: &str) -> bool {
    caption_links_from_markers(vec![NativeCaptionMarker {
        digest: value.to_string(),
        cluster_index: 0,
    }])
    .absence
    .is_none()
}

/// The printed token text of one cluster: column 0 of every row in its range,
/// joined by a single space (GROBID's own `TextUtilities.join` separator).
///
/// Empty tokens are skipped, so an all-empty cluster yields the empty string and
/// the caller stores `None` rather than `Some("")`.
fn cluster_text(
    rows: &[FulltextFeatureRow],
    cluster: &FamilyLabelCluster,
) -> Result<String, NativeTableSequenceError> {
    let mut text = String::new();
    for row in cluster.start..cluster.end {
        let Some(feature_row) = rows.get(row) else {
            return Err(NativeTableSequenceError::CountMismatch {
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
/// same refusal `da_ports::map_input` would raise — a malformed template must not
/// be skipped, or it could hide a column reference the window check exists to
/// catch.
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

fn model_error(error: CrfModelError) -> NativeTableSequenceError {
    NativeTableSequenceError::Model {
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
                .chain((1..NATIVE_TABLE_EMITTED_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        }
    }

    /// The template set of the frozen model: column 0 carries the token, column
    /// 25 closes the declared window.
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

    /// Synthetic one-token-per-row model over the closed ten-label dictionary
    /// that forces `labels[row]` at row `row`.
    fn model_emitting(labels: &[&str]) -> (Vec<FulltextFeatureRow>, CrfModel) {
        let rows = (0..labels.len())
            .map(|index| row(&format!("t{index}")))
            .collect::<Vec<_>>();
        let vocabulary: Vec<String> = NATIVE_TABLE_LABELS
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
            let target = NATIVE_TABLE_LABELS
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

    fn tables(labels: &[&str]) -> Vec<NativeTableSpan> {
        let (rows, model) = model_emitting(labels);
        let cells = table_cells_from_inventory(Vec::new()).unwrap();
        match decode_table_sequence(&rows, &model, &[], cells).expect("synthetic sequence decodes")
        {
            NativeTableDecodeOutcome::Decoded { tables, .. } => tables,
            other => panic!("a table sequence must decode, got {other:?}"),
        }
    }

    #[test]
    fn the_dictionary_is_the_six_native_roles_in_both_spellings() {
        assert_eq!(NATIVE_TABLE_LABELS.len(), 10);
        assert_eq!(NATIVE_TABLE_BASE_LABELS.len(), 6);
        for label in NATIVE_TABLE_LABELS {
            assert!(table_label_is_admissible(label));
        }
        // Declared by Java but not in the frozen block, so unreachable here.
        assert!(!table_label_is_admissible("<label>"));
        assert!(!table_label_is_admissible("<table>"));
        assert!(!table_label_is_admissible("<figure>"));
        assert!(!table_label_is_admissible(""));
        assert_eq!(NATIVE_TABLE_EMITTED_COLUMNS, 27);
        assert_eq!(NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, 25);
    }

    /// The Java split rule, in unit form: head cluster, open table already has a
    /// header, previous label was content/note/desc.
    #[test]
    fn a_head_cluster_splits_only_after_a_body_with_a_header_present() {
        assert!(cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<content>")
        ));
        assert!(cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<note>")
        ));
        assert!(cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<figDesc>")
        ));
        // The open table has no header yet.
        assert!(!cluster_opens_new_table(
            "<figure_head>",
            false,
            Some("<content>")
        ));
        // A head after a head/label/other does not split.
        assert!(!cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<figure_head>")
        ));
        assert!(!cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<label>")
        ));
        assert!(!cluster_opens_new_table(
            "<figure_head>",
            true,
            Some("<other>")
        ));
        assert!(!cluster_opens_new_table("<figure_head>", true, None));
        // Only the head base splits.
        assert!(!cluster_opens_new_table(
            "<content>",
            true,
            Some("<content>")
        ));
        assert!(!cluster_opens_new_table("<note>", true, Some("<content>")));
    }

    #[test]
    fn one_decode_holds_every_table_it_contains() {
        assert_eq!(tables(&["<content>"]).len(), 1);
        // A bare repeat merges into one cluster, so it stays one table.
        assert_eq!(tables(&["<content>", "<content>"]).len(), 1);
        // An `I-` repeat opens a second cluster of the same base: still no split,
        // because the split trigger is the head, not the body.
        assert_eq!(tables(&["<content>", "I-<content>"]).len(), 1);
        // A head after a body, with a header present, starts a second table.
        let split = tables(&["<figure_head>", "<content>", "I-<figure_head>"]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].cluster_index, 0);
        assert_eq!(split[1].cluster_index, 2);
        assert_eq!(split[0].content.as_deref(), Some("t1"));
        assert_eq!(split[1].head.as_deref(), Some("t2"));
        // The `I-` spelling of the split head is irrelevant: the plain-label
        // comparison is the same.
        assert_eq!(
            tables(&["<figure_head>", "<content>", "<figure_head>"]).len(),
            2
        );
        // A label alone also makes the header non-empty, so it can be split from.
        assert_eq!(
            tables(&["I-<label>", "<content>", "I-<figure_head>"]).len(),
            2
        );
        // Two consecutive heads stay in one table (previous label is a head).
        assert_eq!(tables(&["<figure_head>", "I-<figure_head>"]).len(), 1);
    }

    /// `Table.validateTable` in unit form, including the two transcription
    /// details the contract test pins.
    #[test]
    fn the_revert_reason_follows_the_java_rule_order() {
        assert_eq!(
            table_revert_reason(Some("1"), Some("tab. x"), Some("a")),
            None
        );
        // The header prefix test ignores case.
        assert_eq!(
            table_revert_reason(Some("1"), Some("TABELLE 1"), Some("a")),
            None
        );
        // The label is trimmed before parsing, as Java trims it.
        assert_eq!(
            table_revert_reason(Some(" 1 "), Some("tab. x"), Some("a")),
            None
        );
        assert_eq!(
            table_revert_reason(Some("Figure 1"), Some("tab. x"), Some("a")),
            Some(NativeTableRevertReason::LabelNotInteger)
        );
        // Java does not trim the header before the prefix test.
        assert_eq!(
            table_revert_reason(Some("1"), Some(" 1 tab. x"), Some("a")),
            Some(NativeTableRevertReason::HeadNotTabPrefixed)
        );
        // Blank/absent label, header or content each revert.
        assert_eq!(
            table_revert_reason(None, Some("tab. x"), Some("a")),
            Some(NativeTableRevertReason::LabelNotInteger)
        );
        assert_eq!(
            table_revert_reason(Some("1"), Some("   "), Some("a")),
            Some(NativeTableRevertReason::HeadNotTabPrefixed)
        );
        assert_eq!(
            table_revert_reason(Some("1"), Some("tab. x"), None),
            Some(NativeTableRevertReason::EmptyContent)
        );
        // Order: the label defect wins over the header defect.
        assert_eq!(
            table_revert_reason(Some("x"), Some("y"), None),
            Some(NativeTableRevertReason::LabelNotInteger)
        );
    }
}
