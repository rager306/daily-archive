//! Locator join for the native GROBID `formula` family over an already decoded
//! fulltext sequence.
//!
//! ## There is no formula model — and that is the point of this module
//!
//! GROBID 0.9.0 ships no `formula` CRF: `models/` holds no `formula` directory
//! and `GrobidModels.FORMULA` does not exist. Equations are labels of the
//! **fulltext** model — `<equation>`, `<equation_label>`, `<equation_marker>` —
//! and `FullTextParser.processEquations` joins consecutive EQUATION/
//! EQUATION_LABEL clusters, closing an equation on any other base label. This
//! module is therefore a **join over decoded labels**, never a decoder: it takes
//! the outcome of the S05 fulltext runner ([`NativeFulltextDecodeOutcome`]) and
//! has no model input, no row input, no scalar-CRF call and no row-rendering
//! helper — a contract test lexically forbids those names in this file, so the
//! absence cannot regress. Fabricating a second "formula model" here would be
//! exactly the false model this family must not contain; the family needs no
//! model bytes at all (D358).
//!
//! ## Grouping is the shared clusteror rule, never a second copy (D359)
//!
//! Equation clusters are grouped by [`cluster_family_label_runs`] — GROBID's
//! `TaggingTokenClusteror`, where a token carrying the `I-` prefix *opens* a
//! cluster. `I-<equation>` followed by `<equation>` is therefore **two**
//! clusters, while `<equation>` followed by `<equation>` is one. The D359
//! `base_label_runs` of the header/bibliography/fulltext bands answers the
//! opposite question and must not be used here; the divergence and its witness
//! live in [`crate::native_visual_family_cluster`].
//!
//! ## The join rule
//!
//! Clusters — not rows, and not the carrier's own runs — are the units:
//!
//! * an `<equation>` cluster **opens a new equation**, closing any equation that
//!   is still open;
//! * an `<equation_label>` cluster **attaches to the open equation**, and opens
//!   one when no equation is open, so a label the model emitted before its
//!   content cluster is conserved instead of dropped or refused;
//! * **any other base label closes the open equation**;
//! * `<equation_marker>` is a **marker**: not a join label, so it opens no
//!   equation and closes an open one, and every marker cluster is counted in
//!   [`NativeFormulaJoinOutcome::Joined::markers`]. Clusters, not rows, are
//!   counted: a printed marker like `(1)` is several layout tokens carrying the
//!   same spelling and merges into one cluster, so counting rows would report
//!   three markers where one was printed.
//!
//! So
//!
//! * `[<equation>, <equation_label>]` is one equation (label attached),
//! * `[<equation>, I-<equation>]` is two, and that is exactly why this family
//!   groups by the clusteror rule: the `I-` spelling opens a new cluster, and a
//!   second `<equation>` cluster opens a second equation,
//! * `[<equation>, <equation>]` is one, because bare repeats merge into one
//!   cluster,
//! * `[<equation>, <paragraph>, <equation>]` is two,
//! * `[<equation_label>, <equation>]` is two: the label opens a label-only span
//!   rather than being folded forward into a following content cluster (this
//!   join never re-orders the evidence it was handed).
//!
//! ## Absence is not zero (H12-N1)
//!
//! A decode carrying no equation cluster at all is
//! [`NativeFormulaJoinOutcome::NotObserved`], never `Joined { equations: [] }`.
//! The two remaining carrier states are refusals, not absences: an empty label
//! sequence ([`NativeFormulaJoinError::EmptySequence`]) and a carrier that the
//! S05 runner itself rejected ([`NativeFormulaJoinError::DecodeRejected`]) — a
//! rejected decode must never decay into "no equations observed".
//!
//! ## The digests are label-level identities
//!
//! [`NativeEquationSpan::content_digest`] identifies the equation's **decoded
//! label region** — every row of the span, both spellings alike — and
//! [`NativeEquationSpan::label_digest`] identifies its `<equation_label>`
//! subregion, present only when the model emitted one. Both are digests of label
//! *spellings* (`sha256:` over each spelling, NUL-separated), because this
//! surface never sees token text: the join is fed a decoded label sequence, not
//! rows. A span whose only evidence is a label therefore carries equal digests;
//! that is a property of a label-only surface, not a fabricated content.
//!
//! ## No page input
//!
//! The decoded carrier carries label identities, not page ordinals, so
//! [`NativeEquationSpan::page_span`] is always `None`. Fabricating one (a
//! `Some((0, 0))` placeholder) would be an invented fact; the field exists for the
//! adapter seam that owns page-mapped rows and stays empty here.
//!
//! ## What this join does *not* prove
//!
//! That a real `models/fulltext/model.wapiti` emits this clustering (the S05
//! runner owns that claim), and that a printed equation's typography is
//! recovered from a label sequence (it is not — see the digest note). Consuming
//! whatever decode the caller hands it, and refusing the carriers that are not a
//! decode of this dictionary at all, is all this join claims.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! model bytes, no absolute path. Every function is total on hostile input — an
//! empty decode, a rejected decode, an over-long sequence, a label outside the
//! closed fulltext dictionary, an over-long equation list and a grouping refusal
//! are typed outcomes or refusals, never a panic and never a silent truncation.
//! `import_eligible` stays false and nothing here is spliced into the production
//! parse route.

use crate::native_fulltext_sequence::{
    NativeFulltextDecodeOutcome, NativeFulltextSequenceError, fulltext_label_is_admissible,
};
use crate::native_visual_family_cluster::{
    MAX_NATIVE_VISUAL_FAMILY_ROWS, NativeVisualFamilyClusterError, cluster_family_label_runs,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the joined formula surface.
pub const NATIVE_FORMULA_JOIN_SCHEMA_VERSION: &str = "da-parse.native-formula-join/v1";

/// The three fulltext labels this family locates equations by, mirroring
/// `families.formula.locator_labels` in `data/parse/native_visual_families.yaml`
/// 1:1.
///
/// Two of them are join labels; the third ([`NATIVE_FORMULA_MARKER_LABEL`]) is a
/// marker that opens no equation. The DATA contract test fails if either side
/// drifts.
pub const NATIVE_FORMULA_LOCATOR_LABELS: [&str; 3] =
    ["<equation>", "<equation_label>", "<equation_marker>"];

/// The labels a cluster can carry to belong to an equation, mirroring
/// `families.formula.join_labels` in the DATA file.
///
/// A cluster carrying the first entry (`<equation>`, the content locator) opens
/// an equation, a cluster carrying the second (`<equation_label>`, the printed
/// number) attaches to the open one. Every other base label — including
/// [`NATIVE_FORMULA_MARKER_LABEL`] — closes it.
pub const NATIVE_FORMULA_JOIN_LABELS: [&str; 2] = ["<equation>", "<equation_label>"];

/// The fulltext label that names an equation *reference in the prose*.
///
/// It is a locator label of this family but not a join label: it neither opens
/// nor extends an equation, it closes one, and every such cluster is counted in
/// [`NativeFormulaJoinOutcome::Joined::markers`].
pub const NATIVE_FORMULA_MARKER_LABEL: &str = "<equation_marker>";

/// The content locator: a cluster with this base opens an equation.
const FORMULA_CONTENT_BASE: &str = NATIVE_FORMULA_JOIN_LABELS[0];

/// The printed number: a cluster with this base attaches to the open equation.
const FORMULA_LABEL_BASE: &str = NATIVE_FORMULA_JOIN_LABELS[1];

/// Equations admitted into one join.
///
/// Provisional and fail-closed: a decode producing more equations is refused
/// rather than truncated, so no silently partial read can be mistaken for a
/// complete one.
pub const MAX_NATIVE_FORMULA_EQUATIONS: usize = 4_096;

/// Rows admitted into one join.
///
/// The label sequence is what gets grouped, so the bound is the shared
/// clusteror's ([`MAX_NATIVE_VISUAL_FAMILY_ROWS`]) — re-exported under this
/// module's own name so this family's contract names its own limit while the two
/// values can never diverge.
pub const MAX_NATIVE_FORMULA_ROWS: usize = MAX_NATIVE_VISUAL_FAMILY_ROWS;

/// One equation located in an already decoded fulltext label sequence.
///
/// `start`/`end` are a half-open row range in the original reading order — the
/// same coordinates [`crate::FamilyLabelCluster`] uses, so a consumer can address
/// the equation's rows without re-grouping anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEquationSpan {
    /// Identity of the span's decoded label region (every row of the span).
    pub content_digest: String,
    /// Identity of the span's `<equation_label>` subregion, when the model
    /// emitted one. `None` is "no label was observed", never the digest of an
    /// empty sequence.
    pub label_digest: Option<String>,
    /// Half-open row range of the equation in the decoded sequence.
    pub start: usize,
    pub end: usize,
    /// Page range of the equation, or `None` when the carrier carries no page
    /// information — which this join never has. See the module docs: it is never
    /// fabricated.
    pub page_span: Option<(u32, u32)>,
}

/// Closed vocabulary of formula absences.
///
/// [`Self::NoEquationLabels`] is the H12-N1 witness: a decode with no equation
/// cluster is an absence, not an equation list of length zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFormulaAbsence {
    /// The decoded sequence carried no `<equation>`/`<equation_label>` cluster at
    /// all. A decode carrying only markers is this absence too: a marker names a
    /// reference, it is not an equation.
    NoEquationLabels,
}

/// Typed result of one formula join: the located equations, or the absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeFormulaJoinOutcome {
    Joined {
        /// The equations, in reading order.
        equations: Vec<NativeEquationSpan>,
        /// `<equation_marker>` clusters observed in the whole decode, including
        /// those outside every equation.
        markers: usize,
    },
    NotObserved {
        reason: NativeFormulaAbsence,
    },
}

/// Typed refusal vocabulary for the formula join.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFormulaJoinError {
    /// The carrier is not a decode: the S05 runner rejected it.
    ///
    /// Refusing rather than reporting an absence keeps a failed decode from
    /// reading as "this document has no equations".
    DecodeRejected { error: NativeFulltextSequenceError },
    /// The carrier presents no labels at all, so there is nothing to join.
    EmptySequence,
    /// The label sequence exceeds the bounded join's admission.
    TooManyRows { actual: usize, maximum: usize },
    /// The decode contains more equations than the bounded join admits.
    TooManyEquations { actual: usize, maximum: usize },
    /// A label outside the closed 24-label fulltext dictionary reached the join.
    UnmodeledLabel { label: String, row: usize },
    /// The shared `TaggingTokenClusteror` grouping pass refused the labels.
    Grouping {
        error: NativeVisualFamilyClusterError,
    },
}

impl fmt::Display for NativeFormulaJoinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecodeRejected { error } => {
                write!(formatter, "formula join refused a rejected decode: {error}")
            }
            Self::EmptySequence => write!(formatter, "formula join received no labels"),
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "formula join received {actual} labels; maximum is {maximum}"
            ),
            Self::TooManyEquations { actual, maximum } => write!(
                formatter,
                "formula join located {actual} equations; maximum is {maximum}"
            ),
            Self::UnmodeledLabel { label, row } => write!(
                formatter,
                "formula label {label:?} at row {row} is not part of the closed fulltext dictionary"
            ),
            Self::Grouping { error } => write!(formatter, "formula label grouping failed: {error}"),
        }
    }
}

impl std::error::Error for NativeFormulaJoinError {}

/// Join the equations of an already decoded fulltext carrier.
///
/// The carrier's own D359 `runs` field is deliberately **not** an input: the
/// formula family groups by the clusteror rule, and the two rules disagree on
/// `I-` spellings (see the module docs). Only the decoded label sequence is read.
///
/// A rejected carrier is refused, not treated as an absence; an accepted carrier
/// with no labels is refused as well. Equal carriers always produce an equal
/// outcome: there is no clock, no I/O and no iteration over a hash map here.
pub fn join_native_equations(
    decoded: &NativeFulltextDecodeOutcome,
) -> Result<NativeFormulaJoinOutcome, NativeFormulaJoinError> {
    match decoded {
        NativeFulltextDecodeOutcome::Rejected { error } => {
            Err(NativeFormulaJoinError::DecodeRejected {
                error: error.clone(),
            })
        }
        NativeFulltextDecodeOutcome::Accepted { decode } => {
            join_native_equation_labels(&decode.labels)
        }
    }
}

/// Join equations from the decoded label sequence alone.
///
/// This is the seam [`join_native_equations`] delegates to, kept public because
/// it owns the join rule and the H12-N1 witness: a non-empty sequence with no
/// join-label cluster is [`NativeFormulaJoinOutcome::NotObserved`] and never an
/// empty equation list.
pub fn join_native_equation_labels(
    labels: &[String],
) -> Result<NativeFormulaJoinOutcome, NativeFormulaJoinError> {
    if labels.is_empty() {
        return Err(NativeFormulaJoinError::EmptySequence);
    }
    if labels.len() > MAX_NATIVE_FORMULA_ROWS {
        return Err(NativeFormulaJoinError::TooManyRows {
            actual: labels.len(),
            maximum: MAX_NATIVE_FORMULA_ROWS,
        });
    }
    for (row, label) in labels.iter().enumerate() {
        if !fulltext_label_is_admissible(label) {
            return Err(NativeFormulaJoinError::UnmodeledLabel {
                label: label.clone(),
                row,
            });
        }
    }

    let clusters = cluster_family_label_runs(labels)
        .map_err(|error| NativeFormulaJoinError::Grouping { error })?;
    let located = equation_count(&clusters);
    if located > MAX_NATIVE_FORMULA_EQUATIONS {
        return Err(NativeFormulaJoinError::TooManyEquations {
            actual: located,
            maximum: MAX_NATIVE_FORMULA_EQUATIONS,
        });
    }
    if located == 0 {
        // The marker count of a marker-only decode is not carried: the outcome
        // shape reports the absence alone (see the module docs).
        return Ok(NativeFormulaJoinOutcome::NotObserved {
            reason: NativeFormulaAbsence::NoEquationLabels,
        });
    }

    let markers = clusters
        .iter()
        .filter(|cluster| cluster.base == NATIVE_FORMULA_MARKER_LABEL)
        .count();
    let mut equations: Vec<NativeEquationSpan> = Vec::with_capacity(located);
    let mut open: Option<OpenEquation<'_>> = None;
    for cluster in &clusters {
        let rows: Vec<&str> = labels[cluster.start..cluster.end]
            .iter()
            .map(String::as_str)
            .collect();
        if cluster.base == FORMULA_CONTENT_BASE {
            // A content cluster always opens its own equation, so the clusteror
            // boundary is the equation boundary (`<equation>, I-<equation>` is
            // two equations, one cluster each).
            if let Some(previous) = open.take() {
                equations.push(previous.finish());
            }
            open = Some(OpenEquation::open(cluster, rows));
        } else if cluster.base == FORMULA_LABEL_BASE {
            // The printed number attaches to the open equation; emitted before
            // any content cluster it opens a label-only span instead of being
            // dropped.
            match open.as_mut() {
                Some(equation) => equation.attach_label(cluster, rows),
                None => open = Some(OpenEquation::open(cluster, rows)),
            }
        } else if let Some(previous) = open.take() {
            // Any other base label closes the equation; a marker only counts.
            equations.push(previous.finish());
        }
    }
    if let Some(equation) = open.take() {
        equations.push(equation.finish());
    }

    Ok(NativeFormulaJoinOutcome::Joined { equations, markers })
}

/// Is `base` one of the two bases a cluster must carry to belong to an
/// equation?
///
/// [`NATIVE_FORMULA_MARKER_LABEL`] is deliberately not in the list: a marker is
/// a reference, so it closes an open equation instead of extending it.
pub fn is_formula_join_label(base: &str) -> bool {
    NATIVE_FORMULA_JOIN_LABELS.contains(&base)
}

/// Number of equations a cluster sequence describes.
///
/// This is the same rule the span builder applies, counted up front so an
/// over-long decode is refused rather than assembled: a content cluster always
/// opens one, and a label cluster opens one only when nothing is open. A unit
/// test pins the two passes together so they cannot drift.
fn equation_count(clusters: &[crate::FamilyLabelCluster]) -> usize {
    let mut count = 0usize;
    let mut open = false;
    for cluster in clusters {
        if cluster.base == FORMULA_CONTENT_BASE {
            count += 1;
            open = true;
        } else if cluster.base == FORMULA_LABEL_BASE {
            if !open {
                count += 1;
                open = true;
            }
        } else {
            open = false;
        }
    }
    count
}

/// One equation under construction, in the merged spelling row order.
struct OpenEquation<'a> {
    start: usize,
    end: usize,
    /// Every row of the span, in reading order.
    rows: Vec<&'a str>,
    /// Only the `<equation_label>` rows; empty when no label was emitted.
    label_rows: Vec<&'a str>,
}

impl<'a> OpenEquation<'a> {
    fn open(cluster: &crate::FamilyLabelCluster, rows: Vec<&'a str>) -> Self {
        let label_rows = if cluster.base == FORMULA_LABEL_BASE {
            rows.clone()
        } else {
            Vec::new()
        };
        Self {
            start: cluster.start,
            end: cluster.end,
            rows,
            label_rows,
        }
    }

    fn attach_label(&mut self, cluster: &crate::FamilyLabelCluster, rows: Vec<&'a str>) {
        self.end = cluster.end;
        self.label_rows.extend(rows.iter().copied());
        self.rows.extend(rows);
    }

    fn finish(self) -> NativeEquationSpan {
        NativeEquationSpan {
            content_digest: label_sequence_digest(self.rows.iter().copied()),
            label_digest: (!self.label_rows.is_empty())
                .then(|| label_sequence_digest(self.label_rows.iter().copied())),
            start: self.start,
            end: self.end,
            // The carrier carries no page ordinals; see the module docs.
            page_span: None,
        }
    }
}

/// Identity of a sequence of decoded label spellings.
///
/// `sha256:` over each spelling followed by a NUL byte — the same dialect
/// `native_consolidation_specialized` uses for its digest-only receipts, so the
/// two surfaces cannot be confused by shape. The digest covers the *spellings*,
/// not the row offsets: it is a label-level identity, and a span's digest does not
/// depend on where in the document the span sits.
fn label_sequence_digest<'a>(rows: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_fulltext_sequence::{FulltextLabelRun, NativeFulltextDecodeCandidate};

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn accepted(values: &[&str]) -> NativeFulltextDecodeOutcome {
        NativeFulltextDecodeOutcome::Accepted {
            decode: NativeFulltextDecodeCandidate {
                labels: labels(values),
                // The carrier's D359 runs are not an input; an empty list here is
                // the strongest form of that claim.
                runs: Vec::new(),
            },
        }
    }

    fn joined(values: &[&str]) -> (Vec<NativeEquationSpan>, usize) {
        match join_native_equations(&accepted(values)).expect("the synthetic decode must join") {
            NativeFormulaJoinOutcome::Joined { equations, markers } => (equations, markers),
            other => panic!("expected a join, got {other:?}"),
        }
    }

    #[test]
    fn an_equation_label_attaches_to_the_open_equation() {
        let (equations, markers) = joined(&["<equation>", "<equation_label>"]);
        assert_eq!(equations.len(), 1);
        assert_eq!(markers, 0);
        assert_eq!(equations[0].start, 0);
        assert_eq!(equations[0].end, 2);
        assert!(
            equations[0].label_digest.is_some(),
            "the label cluster was emitted, so a label digest exists"
        );
        assert_eq!(
            equations[0].content_digest,
            label_sequence_digest(["<equation>", "<equation_label>"])
        );
    }

    #[test]
    fn a_marker_opens_no_equation_and_closes_an_open_one() {
        // A marker-only decode is an absence, not one equation with no content.
        assert_eq!(
            join_native_equations(&accepted(&["<equation_marker>"])),
            Ok(NativeFormulaJoinOutcome::NotObserved {
                reason: NativeFormulaAbsence::NoEquationLabels,
            })
        );

        let (equations, markers) = joined(&["<equation>", "<equation_marker>", "<equation>"]);
        assert_eq!(equations.len(), 2);
        assert_eq!((equations[0].start, equations[0].end), (0, 1));
        assert_eq!((equations[1].start, equations[1].end), (2, 3));
        assert_eq!(markers, 1, "the marker is counted even though it closes");
        assert_eq!(equations[0].label_digest, None);
    }

    /// Markers are counted as clusters, not rows: a printed marker spelled by
    /// several identical rows merges into one cluster.
    #[test]
    fn merged_marker_rows_are_one_marker() {
        let (equations, markers) = joined(&[
            "<equation>",
            "<equation_marker>",
            "<equation_marker>",
            "<equation>",
        ]);
        assert_eq!(equations.len(), 2);
        assert_eq!(markers, 1);
    }

    #[test]
    fn a_foreign_label_closes_the_equation() {
        let (equations, markers) = joined(&["<equation>", "<paragraph>", "<equation>"]);
        assert_eq!(equations.len(), 2);
        assert_eq!((equations[0].start, equations[0].end), (0, 1));
        assert_eq!((equations[1].start, equations[1].end), (2, 3));
        assert_eq!(markers, 0);
    }

    #[test]
    fn bare_repeats_merge_and_an_inside_spelling_opens_a_new_equation() {
        let (merged, _) = joined(&["<equation>", "<equation>"]);
        assert_eq!(merged.len(), 1);
        assert_eq!((merged[0].start, merged[0].end), (0, 2));

        let (split, _) = joined(&["<equation>", "I-<equation>"]);
        assert_eq!(split.len(), 2, "the `I-` spelling opens a new cluster");
    }

    #[test]
    fn a_decode_without_equation_labels_is_not_observed() {
        assert_eq!(
            join_native_equations(&accepted(&["<paragraph>", "<section>"])),
            Ok(NativeFormulaJoinOutcome::NotObserved {
                reason: NativeFormulaAbsence::NoEquationLabels,
            })
        );
    }

    #[test]
    fn a_rejected_carrier_is_refused_and_an_empty_one_too() {
        let rejected = NativeFulltextDecodeOutcome::Rejected {
            error: NativeFulltextSequenceError::EmptySequence,
        };
        assert_eq!(
            join_native_equations(&rejected),
            Err(NativeFormulaJoinError::DecodeRejected {
                error: NativeFulltextSequenceError::EmptySequence,
            })
        );
        assert_eq!(
            join_native_equations(&accepted(&[])),
            Err(NativeFormulaJoinError::EmptySequence)
        );
    }

    #[test]
    fn a_label_outside_the_fulltext_dictionary_is_refused() {
        assert_eq!(
            join_native_equation_labels(&labels(&["<equation>", "<bogus>"])),
            Err(NativeFormulaJoinError::UnmodeledLabel {
                label: "<bogus>".to_string(),
                row: 1,
            })
        );
    }

    #[test]
    fn the_page_span_is_never_invented() {
        let (equations, _) = joined(&["<equation>", "<equation_label>", "<paragraph>"]);
        assert!(equations.iter().all(|span| span.page_span.is_none()));
    }

    #[test]
    fn the_caps_refuse_rather_than_truncate() {
        let over_rows = labels(&vec!["<paragraph>"; MAX_NATIVE_FORMULA_ROWS + 1]);
        assert_eq!(
            join_native_equation_labels(&over_rows),
            Err(NativeFormulaJoinError::TooManyRows {
                actual: MAX_NATIVE_FORMULA_ROWS + 1,
                maximum: MAX_NATIVE_FORMULA_ROWS,
            })
        );

        // 4096 equations (each `<equation>`/`<paragraph>` pair) are admissible.
        let mut at_cap = Vec::new();
        for _ in 0..MAX_NATIVE_FORMULA_EQUATIONS {
            at_cap.push("<equation>".to_string());
            at_cap.push("<paragraph>".to_string());
        }
        match join_native_equation_labels(&at_cap).expect("the cap is admissible") {
            NativeFormulaJoinOutcome::Joined { equations, .. } => {
                assert_eq!(equations.len(), MAX_NATIVE_FORMULA_EQUATIONS);
            }
            other => panic!("expected a join at the cap, got {other:?}"),
        }
        at_cap.push("<equation>".to_string());
        assert_eq!(
            join_native_equation_labels(&at_cap),
            Err(NativeFormulaJoinError::TooManyEquations {
                actual: MAX_NATIVE_FORMULA_EQUATIONS + 1,
                maximum: MAX_NATIVE_FORMULA_EQUATIONS,
            })
        );
    }

    /// The join reads the label sequence only: two carriers that agree on labels
    /// and disagree on their D359 runs (the other grouping rule) join identically.
    #[test]
    fn the_carriers_d359_runs_are_not_an_input() {
        let decode = |runs: Vec<FulltextLabelRun>| NativeFulltextDecodeOutcome::Accepted {
            decode: NativeFulltextDecodeCandidate {
                labels: labels(&["<equation>", "I-<equation>", "<equation_label>"]),
                runs,
            },
        };
        let no_runs = join_native_equations(&decode(Vec::new())).unwrap();
        let bogus_runs = join_native_equations(&decode(vec![FulltextLabelRun {
            label: "<paragraph>".to_string(),
            start: 0,
            end: 3,
        }]))
        .unwrap();
        assert_eq!(no_runs, bogus_runs);
        match no_runs {
            NativeFormulaJoinOutcome::Joined { equations, .. } => {
                assert_eq!(equations.len(), 2);
            }
            other => panic!("expected a join, got {other:?}"),
        }
    }

    /// The up-front count and the span builder implement one rule: whatever the
    /// shape, the refusal path and the assembled list must agree.
    #[test]
    fn the_pre_count_agrees_with_the_built_spans() {
        for values in [
            vec!["<equation>"],
            vec!["<equation>", "<equation_label>"],
            vec!["<equation>", "I-<equation>"],
            vec!["<equation_label>", "<equation>"],
            vec![
                "<paragraph>",
                "<equation>",
                "<equation_label>",
                "I-<equation_label>",
            ],
            vec!["<equation>", "<equation_marker>", "<equation>"],
            vec!["I-<equation_label>", "<equation_label>"],
        ] {
            let grouped = cluster_family_label_runs(&labels(&values)).unwrap();
            let (equations, _) = joined(&values);
            assert_eq!(
                equation_count(&grouped),
                equations.len(),
                "the count pass drifted for {values:?}"
            );
        }
    }

    #[test]
    fn the_row_cap_mirrors_the_shared_clusteror_bound() {
        assert_eq!(MAX_NATIVE_FORMULA_ROWS, MAX_NATIVE_VISUAL_FAMILY_ROWS);
    }

    #[test]
    fn the_schema_version_names_the_surface() {
        assert_eq!(
            NATIVE_FORMULA_JOIN_SCHEMA_VERSION,
            "da-parse.native-formula-join/v1"
        );
    }

    #[test]
    fn the_locator_labels_are_the_data_spellings() {
        assert_eq!(
            NATIVE_FORMULA_LOCATOR_LABELS,
            ["<equation>", "<equation_label>", "<equation_marker>"]
        );
        assert_eq!(
            NATIVE_FORMULA_JOIN_LABELS,
            ["<equation>", "<equation_label>"]
        );
        assert_eq!(NATIVE_FORMULA_MARKER_LABEL, "<equation_marker>");
        assert!(NATIVE_FORMULA_LOCATOR_LABELS.contains(&NATIVE_FORMULA_MARKER_LABEL));
        assert!(
            NATIVE_FORMULA_JOIN_LABELS
                .iter()
                .all(|label| NATIVE_FORMULA_LOCATOR_LABELS.contains(label))
        );
        assert!(!is_formula_join_label(NATIVE_FORMULA_MARKER_LABEL));
        assert!(is_formula_join_label("<equation>"));
        assert!(is_formula_join_label("<equation_label>"));
    }
}
