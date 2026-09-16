//! S06/T04 contract for the native GROBID `formula` locator join.
//!
//! Three independent gates live here:
//!
//! 1. the **DATA binding** — `families.formula` in
//!    `data/parse/native_visual_families.yaml` must agree 1:1 with the Rust
//!    locator/join label constants, so a transcription edit on either side fails
//!    here;
//! 2. the **join behaviour** — the `TaggingTokenClusteror` `I-` boundary,
//!    `<equation_label>` attaching to the open equation, any other base label
//!    closing it, `<equation_marker>` opening nothing and being counted, the
//!    half-open row bounds, and the caps;
//! 3. the **H12-N1 witnesses** — a decode with no equation label is an absence
//!    (never an equation list of length zero), a rejected carrier is a refusal
//!    (never an absence), and an absent page span is never fabricated.
//!
//! The join is fed decoded label sequences, so this file needs no model at all:
//! GROBID 0.9.0 ships no `formula` model, and `the_join_has_no_decoder` pins that
//! absence lexically.

use da_parse::{
    FulltextLabelRun, MAX_NATIVE_FORMULA_EQUATIONS, MAX_NATIVE_FORMULA_ROWS,
    NATIVE_FORMULA_JOIN_LABELS, NATIVE_FORMULA_JOIN_SCHEMA_VERSION, NATIVE_FORMULA_LOCATOR_LABELS,
    NATIVE_FORMULA_MARKER_LABEL, NativeEquationSpan, NativeFormulaAbsence, NativeFormulaJoinError,
    NativeFormulaJoinOutcome, NativeFulltextDecodeCandidate, NativeFulltextDecodeOutcome,
    NativeFulltextSequenceError, is_formula_join_label, join_native_equation_labels,
    join_native_equations,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const DATA: &str = "data/parse/native_visual_families.yaml";
const SOURCE: &str = include_str!("../src/native_formula_join.rs");

/// Independently recomputed identity of a label-spelling sequence.
///
/// The test owns its own copy of the dialect (`sha256:` over each spelling
/// followed by a NUL byte) so a change in the SUT's digest cannot silently move
/// the expectation with it.
fn expected_digest(rows: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn data() -> serde_yaml::Value {
    let path = root().join(DATA);
    serde_yaml::from_str(&std::fs::read_to_string(&path).expect(DATA)).expect(DATA)
}

fn get<'a>(value: &'a serde_yaml::Value, path: &[&str]) -> &'a serde_yaml::Value {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"));
    }
    current
}

fn text(value: &serde_yaml::Value, path: &[&str]) -> String {
    get(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a string"))
        .to_string()
}

fn flag(value: &serde_yaml::Value, path: &[&str]) -> bool {
    get(value, path)
        .as_bool()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a boolean"))
}

fn integer(value: &serde_yaml::Value, path: &[&str]) -> i64 {
    get(value, path)
        .as_i64()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not an integer"))
}

fn strings(value: &serde_yaml::Value, path: &[&str]) -> Vec<String> {
    get(value, path)
        .as_sequence()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a sequence"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("DATA key {path:?} carries a non-string"))
                .to_string()
        })
        .collect()
}

fn labels(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// A carrier whose labels are the given sequence. The `runs` field is left empty
/// on purpose: the join must not read it (see
/// `the_carriers_d359_runs_are_not_an_input`).
fn accepted(values: &[&str]) -> NativeFulltextDecodeOutcome {
    accepted_with_runs(values, Vec::new())
}

fn accepted_with_runs(values: &[&str], runs: Vec<FulltextLabelRun>) -> NativeFulltextDecodeOutcome {
    NativeFulltextDecodeOutcome::Accepted {
        decode: NativeFulltextDecodeCandidate {
            labels: labels(values),
            runs,
        },
    }
}

fn joined(values: &[&str]) -> (Vec<NativeEquationSpan>, usize) {
    match join_native_equations(&accepted(values)).expect("the synthetic decode must join") {
        NativeFormulaJoinOutcome::Joined { equations, markers } => (equations, markers),
        other => panic!("expected a join, got {other:?}"),
    }
}

fn not_observed(values: &[&str]) -> NativeFormulaAbsence {
    match join_native_equations(&accepted(values))
        .expect("a sequence without equations still joins")
    {
        NativeFormulaJoinOutcome::NotObserved { reason } => reason,
        other => panic!("expected an absence, got {other:?}"),
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

/// The Rust locator/join labels are the DATA transcription, verbatim and in
/// order, and the marker is a locator that is deliberately not a join label.
#[test]
fn the_rust_locator_labels_are_the_data_transcription() {
    let value = data();
    assert_eq!(
        strings(&value, &["families", "formula", "locator_labels"]),
        NATIVE_FORMULA_LOCATOR_LABELS
    );
    assert_eq!(
        strings(&value, &["families", "formula", "join_labels"]),
        NATIVE_FORMULA_JOIN_LABELS
    );
    assert_eq!(
        NATIVE_FORMULA_MARKER_LABEL, NATIVE_FORMULA_LOCATOR_LABELS[2],
        "the marker is the third locator label"
    );
    assert!(
        !is_formula_join_label(NATIVE_FORMULA_MARKER_LABEL),
        "a marker closes an equation, it does not extend one"
    );
    for label in NATIVE_FORMULA_JOIN_LABELS {
        assert!(is_formula_join_label(label));
        assert!(NATIVE_FORMULA_LOCATOR_LABELS.contains(&label));
    }
}

/// The family is a locator join, not a decoder, and the lane stays shadow locked.
#[test]
fn the_family_is_a_locator_join_on_a_shadow_lane() {
    let value = data();
    assert!(
        get(&value, &["families", "formula", "model_id"]).is_null(),
        "GROBID 0.9.0 ships no formula model"
    );
    assert_eq!(
        text(&value, &["families", "formula", "kind"]),
        "locator_join"
    );
    assert_eq!(
        text(&value, &["families", "formula", "locator"]),
        "fulltext"
    );
    assert!(!flag(&value, &["deployed"]));
    assert_eq!(text(&value, &["production_route"]), "forbidden");
    assert_eq!(text(&value, &["graph_write"]), "forbidden");
    assert_eq!(text(&value, &["auto_promote"]), "forbidden");
    assert!(!flag(&value, &["import_eligible"]));
    assert_eq!(integer(&value, &["decision_point"]), 6);
    assert_eq!(
        strings(&value, &["independently_selectable"]),
        ["header", "references"]
    );
}

#[test]
fn the_schema_version_names_the_surface() {
    assert_eq!(
        NATIVE_FORMULA_JOIN_SCHEMA_VERSION,
        "da-parse.native-formula-join/v1"
    );
}

/// `<equation_label>` attaches to the open equation instead of opening a second
/// one, and the span covers both rows.
#[test]
fn an_equation_label_attaches_to_the_open_equation() {
    let (equations, markers) = joined(&["<equation>", "<equation_label>"]);
    assert_eq!(markers, 0);
    assert_eq!(equations.len(), 1);
    assert_eq!((equations[0].start, equations[0].end), (0, 2));
    assert_eq!(
        equations[0].label_digest.as_deref(),
        Some(expected_digest(&["<equation_label>"]).as_str())
    );
}

/// A bare repeat merges (one cluster) and the `I-` spelling opens a new cluster,
/// which under this rule opens a new equation.
#[test]
fn bare_repeats_merge_and_an_inside_spelling_opens_a_new_span() {
    let (merged, _) = joined(&["<equation>", "<equation>"]);
    assert_eq!(merged.len(), 1);
    assert_eq!((merged[0].start, merged[0].end), (0, 2));

    let (split, _) = joined(&["<equation>", "I-<equation>"]);
    assert_eq!(split.len(), 2);
    assert_eq!((split[0].start, split[0].end), (0, 1));
    assert_eq!((split[1].start, split[1].end), (1, 2));
}

/// Any base label outside the join list closes the equation — here a paragraph,
/// so two equations are located.
#[test]
fn a_foreign_label_closes_the_equation() {
    let (equations, markers) = joined(&["<equation>", "<paragraph>", "<equation>"]);
    assert_eq!(markers, 0);
    assert_eq!(equations.len(), 2);
    assert_eq!((equations[0].start, equations[0].end), (0, 1));
    assert_eq!((equations[1].start, equations[1].end), (2, 3));
    assert_eq!(equations[0].label_digest, None);
    assert_eq!(equations[1].label_digest, None);
}

/// A marker is a reference, not an equation: on its own it yields the absence,
/// and next to an equation it closes it and is counted.
#[test]
fn a_marker_opens_no_equation_and_is_counted() {
    assert_eq!(
        not_observed(&["<equation_marker>"]),
        NativeFormulaAbsence::NoEquationLabels
    );
    assert_eq!(
        not_observed(&["<paragraph>", "<equation_marker>"]),
        NativeFormulaAbsence::NoEquationLabels
    );

    let (equations, markers) = joined(&["<equation>", "<equation_marker>", "<equation>"]);
    assert_eq!(equations.len(), 2);
    assert_eq!((equations[0].start, equations[0].end), (0, 1));
    assert_eq!((equations[1].start, equations[1].end), (2, 3));
    assert_eq!(markers, 1, "the marker is counted even though it closes");
}

/// Markers are counted as clusters: identical rows merge into one cluster, while
/// the `I-` spelling opens a second one.
#[test]
fn markers_are_counted_as_clusters_not_rows() {
    let (equations, markers) = joined(&[
        "<equation>",
        "<equation_marker>",
        "<equation_marker>",
        "<equation>",
    ]);
    assert_eq!(equations.len(), 2);
    assert_eq!(markers, 1, "two identical marker rows are one cluster");

    let (_, split_markers) = joined(&[
        "<equation>",
        "<equation_marker>",
        "I-<equation_marker>",
        "<equation>",
    ]);
    assert_eq!(split_markers, 2, "the `I-` spelling opens a second cluster");
}

/// H12-N1: a decode with no equation label is an absence, never an empty list.
#[test]
fn a_decode_without_equation_labels_is_not_observed() {
    assert_eq!(
        not_observed(&["<paragraph>", "<section>"]),
        NativeFormulaAbsence::NoEquationLabels
    );
    assert_eq!(
        join_native_equations(&accepted(&["<paragraph>"])),
        Ok(NativeFormulaJoinOutcome::NotObserved {
            reason: NativeFormulaAbsence::NoEquationLabels,
        })
    );
}

/// A rejected carrier is a refusal, not an absence: a failed decode must not read
/// as "this document has no equations".
#[test]
fn a_rejected_carrier_is_refused_not_reported_as_an_absence() {
    let rejected = NativeFulltextDecodeOutcome::Rejected {
        error: NativeFulltextSequenceError::EmptySequence,
    };
    assert_eq!(
        join_native_equations(&rejected),
        Err(NativeFormulaJoinError::DecodeRejected {
            error: NativeFulltextSequenceError::EmptySequence,
        })
    );
}

#[test]
fn an_empty_carrier_is_a_typed_refusal() {
    assert_eq!(
        join_native_equations(&accepted(&[])),
        Err(NativeFormulaJoinError::EmptySequence)
    );
    assert_eq!(
        join_native_equation_labels(&[]),
        Err(NativeFormulaJoinError::EmptySequence)
    );
}

/// The join refuses a label the closed fulltext dictionary does not name rather
/// than treating it as a closing label.
#[test]
fn a_label_outside_the_fulltext_dictionary_is_refused() {
    assert_eq!(
        join_native_equation_labels(&labels(&["<equation>", "<bogus>"])),
        Err(NativeFormulaJoinError::UnmodeledLabel {
            label: "<bogus>".to_string(),
            row: 1,
        })
    );
    assert_eq!(
        join_native_equation_labels(&labels(&[""])),
        Err(NativeFormulaJoinError::UnmodeledLabel {
            label: String::new(),
            row: 0,
        })
    );
}

/// The carrier carries no page ordinals, so no span may invent one.
#[test]
fn the_page_span_is_never_invented() {
    let (equations, _) = joined(&[
        "<paragraph>",
        "<equation>",
        "<equation_label>",
        "<paragraph>",
        "<equation>",
    ]);
    assert_eq!(equations.len(), 2);
    assert!(
        equations.iter().all(|span| span.page_span.is_none()),
        "a label-level join has no page input, so it reports none"
    );
}

/// The digests are the label-level identity of the span's own region: the content
/// covers every row of the span, the label only its `<equation_label>` rows, and
/// neither depends on where in the document the span sits.
#[test]
fn the_digests_are_the_label_level_identity_of_the_span() {
    let (equations, _) = joined(&["<equation>", "<equation_label>"]);
    assert_eq!(
        equations[0].content_digest,
        expected_digest(&["<equation>", "<equation_label>"])
    );
    assert_eq!(
        equations[0].content_digest,
        "sha256:f9067479db72dcfa17d7f8b7832be25a9e2d2b610f6cd69473d9e955f2d8f93e"
    );
    assert_eq!(
        equations[0].label_digest.as_deref(),
        Some("sha256:caf9765fae86094e29aeeb3e70b2cc9e280e9db3ced4e4504e9c8cc455b05e72")
    );

    let (unlabelled, _) = joined(&["<equation>"]);
    assert_eq!(
        unlabelled[0].content_digest,
        expected_digest(&["<equation>"])
    );
    assert_eq!(
        unlabelled[0].content_digest,
        "sha256:48a6d92e3e2739274f8f6e0c23d52e9ad15b53403b5edbf018fbbf03859c5a22"
    );
    assert_eq!(unlabelled[0].label_digest, None);
}

/// An irrelevant label may move a span's row offsets, but never its evidence: the
/// digest of an equation is a function of that equation's own labels alone.
#[test]
fn the_join_is_deterministic_under_irrelevant_label_permutation() {
    let (first, _) = joined(&["<paragraph>", "<equation>", "<equation_label>"]);
    let (second, _) = joined(&["<equation>", "<equation_label>", "<paragraph>"]);
    assert_eq!(first[0].content_digest, second[0].content_digest);
    assert_eq!(first[0].label_digest, second[0].label_digest);
    assert_eq!((first[0].start, first[0].end), (1, 3));
    assert_eq!((second[0].start, second[0].end), (0, 2));

    // Byte-identical input is byte-identical output through the stable projection.
    let first = join_native_equations(&accepted(&["<equation>", "I-<equation_label>"])).unwrap();
    let second = join_native_equations(&accepted(&["<equation>", "I-<equation_label>"])).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

/// An `<equation_label>` emitted before its content cluster opens a label-only
/// span: the evidence is conserved, never folded forward into a later cluster.
#[test]
fn an_equation_label_before_its_content_opens_a_label_only_span() {
    let (equations, markers) = joined(&["<equation_label>", "<equation>"]);
    assert_eq!(markers, 0);
    assert_eq!(equations.len(), 2);
    assert_eq!((equations[0].start, equations[0].end), (0, 1));
    assert_eq!((equations[1].start, equations[1].end), (1, 2));
    assert_eq!(
        equations[0].label_digest.as_deref(),
        Some(expected_digest(&["<equation_label>"]).as_str()),
        "the label-only span carries its label evidence"
    );
    assert_eq!(
        equations[0].content_digest,
        equations[0].label_digest.clone().unwrap(),
        "a label-only span's whole observed region is the label"
    );
    assert_eq!(equations[1].label_digest, None);
}

/// The half-open bounds tile the equation list in reading order and stay inside
/// the decoded sequence.
#[test]
fn the_bounds_tile_the_equations_in_reading_order() {
    let (equations, _) = joined(&[
        "<paragraph>",
        "<equation>",
        "<equation_label>",
        "<paragraph>",
        "<equation>",
        "<paragraph>",
        "<equation>",
        "<equation_label>",
    ]);
    assert_eq!(equations.len(), 3);
    for (index, span) in equations.iter().enumerate() {
        assert!(span.start < span.end, "span {index} is never empty");
        assert!(span.end <= 8, "span {index} stays inside the sequence");
    }
    assert!(
        equations
            .windows(2)
            .all(|pair| pair[0].end <= pair[1].start),
        "the spans never overlap and stay in reading order"
    );
    assert_eq!((equations[0].start, equations[0].end), (1, 3));
    assert_eq!((equations[1].start, equations[1].end), (4, 5));
    assert_eq!((equations[2].start, equations[2].end), (6, 8));
}

#[test]
fn the_equation_cap_refuses_rather_than_truncating() {
    let mut at_cap = Vec::new();
    for _ in 0..MAX_NATIVE_FORMULA_EQUATIONS {
        at_cap.push("<equation>".to_string());
        at_cap.push("<paragraph>".to_string());
    }
    match join_native_equation_labels(&at_cap).expect("the cap is admissible") {
        NativeFormulaJoinOutcome::Joined { equations, markers } => {
            assert_eq!(equations.len(), MAX_NATIVE_FORMULA_EQUATIONS);
            assert_eq!(markers, 0);
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

#[test]
fn the_row_cap_refuses_rather_than_truncating() {
    let over = labels(&vec!["<paragraph>"; MAX_NATIVE_FORMULA_ROWS + 1]);
    assert_eq!(
        join_native_equation_labels(&over),
        Err(NativeFormulaJoinError::TooManyRows {
            actual: MAX_NATIVE_FORMULA_ROWS + 1,
            maximum: MAX_NATIVE_FORMULA_ROWS,
        })
    );
}

/// The join reads the decoded label sequence only: two carriers that agree on the
/// labels and disagree on their D359 `runs` (the *other* grouping rule) join
/// identically. Reading `runs` would silently apply the wrong grouping.
#[test]
fn the_carriers_d359_runs_are_not_an_input() {
    let values = ["<equation>", "<equation_label>", "I-<equation>"];
    let without_runs = join_native_equations(&accepted_with_runs(&values, Vec::new())).unwrap();
    let with_bogus_runs = join_native_equations(&accepted_with_runs(
        &values,
        vec![FulltextLabelRun {
            label: "<paragraph>".to_string(),
            start: 0,
            end: 3,
        }],
    ))
    .unwrap();
    assert_eq!(without_runs, with_bogus_runs);
    match without_runs {
        NativeFormulaJoinOutcome::Joined { equations, .. } => {
            assert_eq!(
                equations.len(),
                2,
                "the `I-` spelling opens a second equation"
            );
        }
        other => panic!("expected a join, got {other:?}"),
    }
}

/// The absence of a formula decoder is a claim about this file, so it is pinned
/// lexically: no model entry, no row input, no scalar-CRF call, no row rendering.
#[test]
fn the_join_has_no_decoder_and_no_row_input() {
    for forbidden in [
        "CrfModel",
        "map_input",
        "viterbi_decode",
        "scalar_crf",
        "da_ports",
        "FulltextFeatureRow",
        "render_fulltext_feature_text",
        "decode_fulltext_sequence",
        "check_fulltext_label_block",
        "#rdr#",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "the formula join must not reference {forbidden}: the family is a locator join only"
        );
    }
    assert!(
        SOURCE.contains("cluster_family_label_runs"),
        "the join must group by the shared clusteror rule, not by a second copy"
    );
    // The D359 base-name rule would merge `I-<equation>` into its predecessor. It
    // may be *named* in the docs (to warn against it) but never imported or called.
    assert!(!SOURCE.contains("base_label_runs("));
    assert!(!SOURCE.contains("use crate::native_bibliography_sequence"));
}

/// The span and outcome wire shapes are pinned: field order, the `None` label
/// digest, the typed absence and the always-`null` page span are all part of the
/// projection a consumer reads.
#[test]
fn the_span_projection_is_stable() {
    let (equations, _) = joined(&["<equation>"]);
    assert_eq!(
        serde_json::to_string(&equations[0]).unwrap(),
        concat!(
            r#"{"content_digest":"sha256:48a6d92e3e2739274f8f6e0c23d52e9ad15b53403b5edbf0"#,
            r#"18fbbf03859c5a22","label_digest":null,"start":0,"end":1,"page_span":null}"#
        )
    );

    let outcome = join_native_equations(&accepted(&["<paragraph>"])).unwrap();
    assert_eq!(
        serde_json::to_string(&outcome).unwrap(),
        r#"{"outcome":"not_observed","reason":"no_equation_labels"}"#
    );

    let joined_outcome =
        join_native_equations(&accepted(&["<equation>", "<equation_label>"])).unwrap();
    assert_eq!(
        serde_json::to_string(&joined_outcome).unwrap(),
        concat!(
            r#"{"outcome":"joined","equations":[{"content_digest":"#,
            r#""sha256:f9067479db72dcfa17d7f8b7832be25a9e2d2b610f6cd69473d9e955f2d8f93e","#,
            r#""label_digest":"sha256:caf9765fae86094e29aeeb3e70b2cc9e280e9db3ced4e4504e9c8cc455b05e72","#,
            r#""start":0,"end":2,"page_span":null}],"markers":0}"#
        )
    );
}

#[test]
fn the_outcome_and_the_error_round_trip() {
    let joined_outcome = join_native_equations(&accepted(&["<equation>"])).unwrap();
    roundtrip(&joined_outcome);
    roundtrip(&join_native_equations(&accepted(&["<paragraph>"])).unwrap());
    roundtrip(&join_native_equations(&accepted(&[])).unwrap_err());
    roundtrip(&NativeFulltextDecodeOutcome::Rejected {
        error: NativeFulltextSequenceError::TooManyRows {
            actual: 5,
            maximum: 4,
        },
    });
}
