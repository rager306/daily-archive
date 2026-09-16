//! S06/T01 contract: the shared `TaggingTokenClusteror` grouping rule and the
//! caption-link lane of the native visual/funding families.
//!
//! The interesting assertions are the divergence cases: the D359
//! `base_label_runs` rule (used by the header/bibliography/fulltext bands) groups
//! by base name, so `I-<figure>` followed by `<figure>` is one run there. GROBID's
//! `TaggingTokenClusteror.isBeginning()` opens a new cluster at every `I-` label,
//! so the same input must be two clusters here.

use da_parse::{
    FamilyLabelCluster, MAX_NATIVE_VISUAL_FAMILY_ROWS, NATIVE_VISUAL_FAMILY_NAME_PREFIX,
    NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker,
    NativeVisualFamilyClusterError, caption_links_from_markers, cluster_family_label_runs,
};

fn labels(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn cluster(base: &str, start: usize, end: usize, opened_by_i_prefix: bool) -> FamilyLabelCluster {
    FamilyLabelCluster {
        base: base.to_string(),
        start,
        end,
        opened_by_i_prefix,
        spelled_identically: true,
    }
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

fn digest(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn groups(values: &[&str]) -> Vec<FamilyLabelCluster> {
    cluster_family_label_runs(&labels(values)).expect("the synthetic sequence must group")
}

/// The four cases the plan pins. Under the D359 base-name rule the first two
/// would each be a single run; under the clusteror rule they are two clusters,
/// because the `I-` prefix always opens one.
#[test]
fn the_inside_prefix_always_opens_a_cluster() {
    assert_eq!(
        groups(&["<figure>", "<figure>"]),
        [cluster("<figure>", 0, 2, false)]
    );
    assert_eq!(
        groups(&["I-<figure>", "<figure>"]),
        [
            cluster("<figure>", 0, 1, true),
            cluster("<figure>", 1, 2, false),
        ]
    );
    assert_eq!(
        groups(&["I-<figure>", "I-<figure>"]),
        [
            cluster("<figure>", 0, 1, true),
            cluster("<figure>", 1, 2, true),
        ]
    );
    assert_eq!(
        groups(&["<figure>", "I-<figure>"]),
        [
            cluster("<figure>", 0, 1, false),
            cluster("<figure>", 1, 2, true),
        ]
    );
}

/// Three different bare spellings never merge, and a repeated *bare* spelling
/// does: the merge key is the full spelling, not the base name.
#[test]
fn distinct_spellings_do_not_merge_and_equal_spellings_do() {
    assert_eq!(
        groups(&["<content>", "<figure_head>", "<figDesc>"]),
        [
            cluster("<content>", 0, 1, false),
            cluster("<figure_head>", 1, 2, false),
            cluster("<figDesc>", 2, 3, false),
        ]
    );
    assert_eq!(
        groups(&["<content>", "<content>", "I-<content>"]),
        [
            cluster("<content>", 0, 2, false),
            cluster("<content>", 2, 3, true),
        ]
    );
}

/// Whatever the spellings, the clusters tile the rows end to end, every base is
/// the cluster spelling without the prefix, and no cluster mixes spellings.
#[test]
fn clusters_tile_the_rows_in_reading_order() {
    let sequence = [
        "I-<figure_head>",
        "<figure_head>",
        "<content>",
        "I-<content>",
        "<content>",
        "I-<label>",
        "<figDesc>",
    ];
    let clusters = groups(&sequence);
    let mut cursor = 0usize;
    for family_cluster in &clusters {
        assert_eq!(family_cluster.start, cursor, "clusters tile the input");
        assert!(family_cluster.start < family_cluster.end);
        assert!(family_cluster.end <= sequence.len());
        assert_eq!(
            family_cluster.opened_by_i_prefix,
            sequence[family_cluster.start].starts_with(NATIVE_VISUAL_FAMILY_NAME_PREFIX)
        );
        assert_eq!(
            family_cluster.base,
            sequence[family_cluster.start]
                .strip_prefix(NATIVE_VISUAL_FAMILY_NAME_PREFIX)
                .unwrap_or(sequence[family_cluster.start])
        );
        assert!(
            !family_cluster
                .base
                .starts_with(NATIVE_VISUAL_FAMILY_NAME_PREFIX)
        );
        assert!(
            family_cluster.spelled_identically,
            "the clusteror rule never merges two spellings"
        );
        assert!(
            sequence[family_cluster.start..family_cluster.end]
                .iter()
                .all(|row| *row == sequence[family_cluster.start])
        );
        cursor = family_cluster.end;
    }
    assert_eq!(cursor, sequence.len(), "the last cluster reaches the end");
}

/// A cluster that opens with the bare spelling and whose next row is the `I-`
/// spelling stays open: switching spelling forward does not extend a cluster,
/// so only the `I-`-opened row is its own cluster.
#[test]
fn a_bare_cluster_is_closed_by_the_next_inside_spelling() {
    assert_eq!(
        groups(&["<figDesc>", "<figDesc>", "I-<figDesc>", "<figDesc>"]),
        [
            cluster("<figDesc>", 0, 2, false),
            cluster("<figDesc>", 2, 3, true),
            cluster("<figDesc>", 3, 4, false),
        ]
    );
}

#[test]
fn empty_inputs_are_typed_refusals_before_any_grouping() {
    assert_eq!(
        cluster_family_label_runs(&[]),
        Err(NativeVisualFamilyClusterError::EmptySequence)
    );
    assert_eq!(
        cluster_family_label_runs(&labels(&["<content>", "", "I-<content>"])),
        Err(NativeVisualFamilyClusterError::EmptyLabel { row: 1 })
    );
}

/// The row cap refuses instead of truncating a silently partial read: exactly
/// the cap groups, one row above it is a typed refusal.
#[test]
fn the_row_cap_is_a_refusal_not_a_truncation() {
    let capped = vec!["<content>".to_string(); MAX_NATIVE_VISUAL_FAMILY_ROWS];
    assert_eq!(
        cluster_family_label_runs(&capped),
        Ok(vec![cluster(
            "<content>",
            0,
            MAX_NATIVE_VISUAL_FAMILY_ROWS,
            false
        )])
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

#[test]
fn grouping_is_deterministic() {
    let sequence = labels(&["I-<content>", "<content>", "<figDesc>", "I-<note>"]);
    assert_eq!(
        cluster_family_label_runs(&sequence),
        cluster_family_label_runs(&sequence)
    );
}

#[test]
fn cluster_and_error_surfaces_roundtrip() {
    roundtrip(&groups(&["I-<content>", "<content>"]));
    roundtrip(&FamilyLabelCluster {
        base: "<figure_head>".to_string(),
        start: 0,
        end: 3,
        opened_by_i_prefix: false,
        spelled_identically: true,
    });
    roundtrip(&NativeVisualFamilyClusterError::EmptySequence);
    roundtrip(&NativeVisualFamilyClusterError::TooManyRows {
        rows: 4,
        maximum: MAX_NATIVE_VISUAL_FAMILY_ROWS,
    });
    roundtrip(&NativeVisualFamilyClusterError::EmptyLabel { row: 2 });
}

/// H12-N1: an empty inventory is a *typed absence*, never an empty success. The
/// caller must not be able to read "no markers arrived" as "zero captions".
#[test]
fn an_empty_caption_inventory_is_not_observed() {
    assert_eq!(
        caption_links_from_markers(Vec::new()),
        NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(NativeCaptionLinkAbsence::NotObserved),
        }
    );
}

#[test]
fn a_valid_caption_inventory_keeps_its_order_and_absence_stays_none() {
    let first = digest('a');
    let second = digest('0');
    let links = caption_links_from_markers(vec![
        NativeCaptionMarker {
            digest: first.clone(),
            cluster_index: 3,
        },
        NativeCaptionMarker {
            digest: second.clone(),
            cluster_index: 1,
        },
    ]);
    assert_eq!(links.absence, None);
    assert_eq!(
        links.markers,
        [
            NativeCaptionMarker {
                digest: first,
                cluster_index: 3,
            },
            NativeCaptionMarker {
                digest: second,
                cluster_index: 1,
            },
        ]
    );
    roundtrip(&links);
}

/// A malformed digest and a duplicated digest are both refused, and the refusal
/// is total: no partially accepted prefix escapes as a success.
#[test]
fn malformed_and_duplicate_caption_digests_refuse_the_whole_inventory() {
    for bad in [
        String::new(),
        digest('a')[1..].to_string(),
        "A".repeat(64),
        format!("{}Z", "a".repeat(63)),
        format!(" {} ", digest('a')),
    ] {
        let links = caption_links_from_markers(vec![NativeCaptionMarker {
            digest: bad.clone(),
            cluster_index: 0,
        }]);
        assert_eq!(
            links.absence,
            Some(NativeCaptionLinkAbsence::InvalidDigest),
            "digest {bad:?} must be refused"
        );
        assert!(links.markers.is_empty());
    }

    let repeated = digest('b');
    let links = caption_links_from_markers(vec![
        NativeCaptionMarker {
            digest: repeated.clone(),
            cluster_index: 0,
        },
        NativeCaptionMarker {
            digest: repeated,
            cluster_index: 7,
        },
    ]);
    assert_eq!(links.absence, Some(NativeCaptionLinkAbsence::InvalidDigest));
    assert!(links.markers.is_empty());
}

/// The closed absence vocabulary survives serialization unchanged, so a consumer
/// can never see an unknown state.
#[test]
fn the_caption_absence_vocabulary_is_closed() {
    for absence in [
        NativeCaptionLinkAbsence::Unsupported,
        NativeCaptionLinkAbsence::NotObserved,
        NativeCaptionLinkAbsence::MissingIdentity,
        NativeCaptionLinkAbsence::InvalidDigest,
    ] {
        roundtrip(&absence);
        roundtrip(&NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(absence),
        });
    }
}
