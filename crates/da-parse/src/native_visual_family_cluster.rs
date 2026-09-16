//! Shared grouping rule and caption-link lane of the native visual/funding
//! families (figure, table, formula, funding).
//!
//! This module owns exactly two things, because all four families need them and
//! a second copy would drift:
//!
//! 1. **The `TaggingTokenClusteror` grouping rule.** GROBID's
//!    `TaggingTokenClusteror.isBeginning(label)` opens a new cluster at every
//!    label that carries the `I-` prefix — unconditionally, whatever the previous
//!    label was. Inside such a segment only *consecutive identical* spellings
//!    merge. The figure, table and funding families are grouped by that rule.
//! 2. **The caption-link lane** shared by figure and table
//!    (`postProcessFigureCaptions` / `postProcessTableCaptions`), which is a
//!    typed inventory of caller-supplied marker digests — never a resolved
//!    marker→reference link (that belongs to the already-shipped S04
//!    `native_callout_matching` surface, [`crate::match_native_callouts`]).
//!
//! ## Divergence from `base_label_runs` (D359) — do not unify the two
//!
//! [`crate::native_bibliography_sequence::base_label_runs`] is the authority for
//! the header, bibliography and fulltext bands. It groups by **base-name
//! equality**, so `I-<figure>` followed by `<figure>` is *one* run there. That
//! rule is wrong for these families: `TaggingTokenClusteror.isBeginning()`
//! treats the `I-` prefix as "a cluster begins here", so the same two labels are
//! *two* clusters, while `<figure>` followed by `<figure>` is one. The two rules
//! are therefore kept as two implementations with two contracts —
//! `I-<figure>,<figure>` must yield two clusters here (see
//! `native_visual_family_cluster_contract.rs`), and reusing `base_label_runs`
//! for these families would silently merge a head into its own content.
//!
//! Only the `I-` *stripping* is shared: [`base_label`] is re-used from the
//! bibliography module rather than re-written, so the spelling prefix has one
//! definition in the crate.
//!
//! ## Totality
//!
//! No filesystem, no environment, no clock, no model bytes, no logging. Every
//! function is total on hostile input: an empty sequence, an empty label, an
//! over-long sequence, a non-hex digest and a duplicate digest are typed
//! refusals, never a panic and never a silent success. `import_eligible` stays
//! false and nothing here is spliced into the production parse route.

use crate::native_bibliography_sequence::base_label;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The spelling prefix GROBID's clusteror treats as "a new cluster begins here".
pub const NATIVE_VISUAL_FAMILY_NAME_PREFIX: &str = "I-";

/// Rows admitted into one grouping pass.
///
/// Provisional and fail-closed: the visual/funding bands are not a runtime
/// surface yet, so an input beyond this bound is refused rather than truncated,
/// and no silently partial read can be mistaken for a complete one.
pub const MAX_NATIVE_VISUAL_FAMILY_ROWS: usize = 32_768;

/// Digest width of a caption marker identity: one lower-hex SHA-256.
const CAPTION_DIGEST_HEX_BYTES: usize = 64;

/// One cluster of the `TaggingTokenClusteror` rule: a maximal run of one full
/// label spelling that is never entered through an `I-` row.
///
/// `start`/`end` are a half-open row range in the original reading order, and
/// the clusters tile the input end to end. `base` is the base name with the
/// spelling prefix stripped (so a cluster opened by `I-<content>` reports
/// `<content>`), and `opened_by_i_prefix` records which spelling opened it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyLabelCluster {
    /// Base name of the cluster: the cluster's spelling with `I-` stripped.
    pub base: String,
    /// Half-open row range in the original reading order.
    pub start: usize,
    pub end: usize,
    /// True when the cluster's first row carried the `I-` spelling.
    pub opened_by_i_prefix: bool,
    /// True when every row of the cluster carries the identical full spelling.
    ///
    /// Computed rather than asserted: under this rule the value holds for every
    /// cluster by construction, which is exactly the witness that separates it
    /// from the D359 base-name merge — that rule would have produced one run
    /// whose rows carry two different spellings.
    pub spelled_identically: bool,
}

/// Typed refusal vocabulary of the family grouping rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualFamilyClusterError {
    /// No rows were supplied, so there is nothing to group.
    EmptySequence,
    /// The sequence exceeds the bounded grouping pass.
    TooManyRows { rows: usize, maximum: usize },
    /// A row carries an empty label, which is not a name.
    EmptyLabel { row: usize },
}

impl fmt::Display for NativeVisualFamilyClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => {
                write!(formatter, "native visual family label sequence is empty")
            }
            Self::TooManyRows { rows, maximum } => write!(
                formatter,
                "native visual family label sequence has {rows} rows; maximum is {maximum}"
            ),
            Self::EmptyLabel { row } => {
                write!(
                    formatter,
                    "native visual family label at row {row} is empty"
                )
            }
        }
    }
}

impl std::error::Error for NativeVisualFamilyClusterError {}

/// Group decoded family labels with GROBID's `TaggingTokenClusteror` rule.
///
/// The rule is applied in two steps, and the order matters:
///
/// 1. cut before **every** label that starts with `I-` — the prefix opens a new
///    cluster whatever preceded it, even the identical `I-` spelling;
/// 2. inside the resulting segment, merge consecutive identical full labels.
///
/// So `<figure>,<figure>` is one cluster, while `I-<figure>,<figure>`,
/// `I-<figure>,I-<figure>` and `<figure>,I-<figure>` are two each. This is
/// deliberately *not* [`crate::native_bibliography_sequence::base_label_runs`]:
/// see the module docs for why the two families need two rules.
///
/// The label strings are opaque here — the closed dictionaries live in
/// `data/parse/native_visual_families.yaml` and are checked by the family
/// runners, not by this shared pass.
pub fn cluster_family_label_runs(
    labels: &[String],
) -> Result<Vec<FamilyLabelCluster>, NativeVisualFamilyClusterError> {
    if labels.is_empty() {
        return Err(NativeVisualFamilyClusterError::EmptySequence);
    }
    if labels.len() > MAX_NATIVE_VISUAL_FAMILY_ROWS {
        return Err(NativeVisualFamilyClusterError::TooManyRows {
            rows: labels.len(),
            maximum: MAX_NATIVE_VISUAL_FAMILY_ROWS,
        });
    }
    for (row, label) in labels.iter().enumerate() {
        if label.is_empty() {
            return Err(NativeVisualFamilyClusterError::EmptyLabel { row });
        }
    }

    let mut clusters = Vec::new();
    let mut start = 0usize;
    while start < labels.len() {
        let spelling = &labels[start];
        let opened_by_i_prefix = spelling.starts_with(NATIVE_VISUAL_FAMILY_NAME_PREFIX);
        let mut end = start + 1;
        // Step 2 of the rule, with step 1 folded in: a row extends the cluster
        // only when it repeats the identical spelling *and* does not itself open
        // a new cluster through the `I-` prefix.
        while end < labels.len()
            && labels[end] == *spelling
            && !labels[end].starts_with(NATIVE_VISUAL_FAMILY_NAME_PREFIX)
        {
            end += 1;
        }
        let rows = &labels[start..end];
        clusters.push(FamilyLabelCluster {
            base: base_label(spelling).to_string(),
            start,
            end,
            opened_by_i_prefix,
            spelled_identically: rows.iter().all(|row| row == spelling),
        });
        start = end;
    }
    Ok(clusters)
}

/// One caption marker the caller observed for a figure or table.
///
/// The digest is the marker's identity (one lower-hex SHA-256); `cluster_index`
/// is the index of the family cluster the marker belongs to, so a consumer can
/// address a caption without this module ever resolving it to a reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCaptionMarker {
    pub digest: String,
    pub cluster_index: usize,
}

/// Closed vocabulary of caption-link absences.
///
/// [`Self::Unsupported`] and [`Self::MissingIdentity`] are carried for the
/// caller's own upstream status (a family that has no caption lane at all, or a
/// marker whose identity never arrived); [`NativeCaptionLinks::absence`] is
/// never `None` when the inventory is empty, so "no links were observed" can
/// never be read as "zero links" (H12-N1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCaptionLinkAbsence {
    Unsupported,
    NotObserved,
    MissingIdentity,
    InvalidDigest,
}

/// The caption-link inventory of one figure/table cluster, or its typed absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCaptionLinks {
    /// Accepted markers, in the order the caller supplied them.
    pub markers: Vec<NativeCaptionMarker>,
    /// Why no inventory is reported. `None` means the markers are the inventory.
    pub absence: Option<NativeCaptionLinkAbsence>,
}

/// Type a caller-supplied caption-marker inventory.
///
/// * an empty inventory is [`NativeCaptionLinkAbsence::NotObserved`] — never an
///   empty success, because "absent" and "zero" must stay distinguishable
///   (H12-N1);
/// * every digest must be 64 lower-hex characters;
/// * duplicate digests are refused as well: the same marker cannot be two
///   captions.
///
/// Validation is all-or-nothing: a single invalid or duplicated digest refuses
/// the whole inventory (the returned `markers` is empty) rather than reporting a
/// partial success the caller might act on. Accepted markers keep their input
/// order.
///
/// This function never resolves a marker to a reference: that ownership stays
/// with the already-shipped S04 `native_callout_matching` surface.
pub fn caption_links_from_markers(markers: Vec<NativeCaptionMarker>) -> NativeCaptionLinks {
    if markers.is_empty() {
        return NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(NativeCaptionLinkAbsence::NotObserved),
        };
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for marker in &markers {
        let digest = marker.digest.as_str();
        if !is_lower_hex_digest(digest) || !seen.insert(digest) {
            return NativeCaptionLinks {
                markers: Vec::new(),
                absence: Some(NativeCaptionLinkAbsence::InvalidDigest),
            };
        }
    }
    NativeCaptionLinks {
        markers,
        absence: None,
    }
}

/// One lower-hex SHA-256 digest: exactly 64 hexadecimal characters, no upper
/// case (the same dialect the reviewed oracle fixtures pin).
fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == CAPTION_DIGEST_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn cluster(base: &str, start: usize, end: usize, opener: bool) -> FamilyLabelCluster {
        FamilyLabelCluster {
            base: base.to_string(),
            start,
            end,
            opened_by_i_prefix: opener,
            spelled_identically: true,
        }
    }

    /// The witness of the D359 divergence, in unit form: the base-name rule would
    /// merge these two rows, the clusteror rule must not.
    #[test]
    fn an_inside_spelling_always_opens_a_new_cluster() {
        assert_eq!(
            cluster_family_label_runs(&labels(&["I-<figure>", "<figure>"])).unwrap(),
            [
                cluster("<figure>", 0, 1, true),
                cluster("<figure>", 1, 2, false),
            ]
        );
    }

    #[test]
    fn identical_bare_spellings_merge_into_one_cluster() {
        assert_eq!(
            cluster_family_label_runs(&labels(&["<figure>", "<figure>"])).unwrap(),
            [cluster("<figure>", 0, 2, false)]
        );
    }

    #[test]
    fn clusters_tile_the_rows_and_name_their_opener() {
        let clusters = cluster_family_label_runs(&labels(&[
            "I-<content>",
            "<content>",
            "<figDesc>",
            "I-<figure_head>",
        ]))
        .unwrap();
        let mut cursor = 0usize;
        for family_cluster in &clusters {
            assert_eq!(family_cluster.start, cursor);
            assert!(family_cluster.start < family_cluster.end);
            assert!(
                !family_cluster
                    .base
                    .starts_with(NATIVE_VISUAL_FAMILY_NAME_PREFIX)
            );
            assert!(family_cluster.spelled_identically);
            cursor = family_cluster.end;
        }
        assert_eq!(cursor, 4);
    }

    #[test]
    fn empty_inputs_are_typed_refusals() {
        assert_eq!(
            cluster_family_label_runs(&[]),
            Err(NativeVisualFamilyClusterError::EmptySequence)
        );
        assert_eq!(
            cluster_family_label_runs(&labels(&["<content>", ""])),
            Err(NativeVisualFamilyClusterError::EmptyLabel { row: 1 })
        );
    }

    #[test]
    fn the_row_cap_refuses_rather_than_truncating() {
        let capped = vec!["<content>".to_string(); MAX_NATIVE_VISUAL_FAMILY_ROWS];
        assert_eq!(
            cluster_family_label_runs(&capped).unwrap(),
            [cluster(
                "<content>",
                0,
                MAX_NATIVE_VISUAL_FAMILY_ROWS,
                false
            )]
        );
        let over = vec!["<content>".to_string(); MAX_NATIVE_VISUAL_FAMILY_ROWS + 1];
        assert_eq!(
            cluster_family_label_runs(&over),
            Err(NativeVisualFamilyClusterError::TooManyRows {
                rows: MAX_NATIVE_VISUAL_FAMILY_ROWS + 1,
                maximum: MAX_NATIVE_VISUAL_FAMILY_ROWS,
            })
        );
    }

    /// H12-N1 in unit form: absence is never encoded as an empty inventory.
    #[test]
    fn an_empty_caption_inventory_is_not_observed_not_an_empty_success() {
        assert_eq!(
            caption_links_from_markers(Vec::new()),
            NativeCaptionLinks {
                markers: Vec::new(),
                absence: Some(NativeCaptionLinkAbsence::NotObserved),
            }
        );
    }

    #[test]
    fn a_valid_inventory_keeps_its_order_and_reports_no_absence() {
        let digest_a = "0123456789abcdef".repeat(4);
        let digest_b = "fedcba9876543210".repeat(4);
        let links = caption_links_from_markers(vec![
            NativeCaptionMarker {
                digest: digest_a.clone(),
                cluster_index: 1,
            },
            NativeCaptionMarker {
                digest: digest_b.clone(),
                cluster_index: 0,
            },
        ]);
        assert_eq!(links.absence, None);
        assert_eq!(
            links
                .markers
                .iter()
                .map(|marker| marker.digest.as_str())
                .collect::<Vec<_>>(),
            [digest_a.as_str(), digest_b.as_str()]
        );
    }

    #[test]
    fn malformed_and_duplicate_digests_refuse_the_whole_inventory() {
        let digest = "0123456789abcdef".repeat(4);
        for bad in [
            String::new(),
            "abc".to_string(),
            "A".repeat(64),
            format!("{}z", "0".repeat(63)),
        ] {
            let links = caption_links_from_markers(vec![NativeCaptionMarker {
                digest: bad,
                cluster_index: 0,
            }]);
            assert_eq!(links.absence, Some(NativeCaptionLinkAbsence::InvalidDigest));
            assert!(links.markers.is_empty());
        }
        let duplicated = caption_links_from_markers(vec![
            NativeCaptionMarker {
                digest: digest.clone(),
                cluster_index: 0,
            },
            NativeCaptionMarker {
                digest,
                cluster_index: 1,
            },
        ]);
        assert_eq!(
            duplicated.absence,
            Some(NativeCaptionLinkAbsence::InvalidDigest)
        );
        assert!(duplicated.markers.is_empty());
    }
}
