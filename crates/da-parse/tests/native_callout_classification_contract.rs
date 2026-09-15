//! Contract for the native callout-marker classifier and body linkage (S04/T06).
//!
//! The oracle is the Java format authority, counted by hand from
//! `CalloutAnalyzer.getCalloutType(List<LayoutToken>)` (109 lines): six regexes
//! plus a superscript flag, checked in a fixed order. Every expectation below
//! is derived from that source and written out in the test that uses it — none
//! of them is produced by running the implementation under test, and no test
//! here runs Java or loads a real GROBID model.
//!
//! What this file has to pin, in the order the task's `DONE WHEN` lists it:
//!
//! 1. all eight marker types, with the Java decision order (not the enum order);
//! 2. majority style: `Selected` / `Tie` / `Empty`;
//! 3. typed unresolved for all six `CalloutUnresolvedReason` values;
//! 4. a property test over adjacency and the illegal boundary;
//! 5. a targetless-heavy negative that cannot be reported as success, with the
//!    two MEM992 denominators kept separate and checksum-bound.

use da_parse::{
    AdjacencyDisposition, BibliographyAlignment, BibliographyTokenFacts, BibliographyTokenInput,
    CalloutDenominator, CalloutInventory, CalloutLabelOrdinalMap, CalloutLabelResolution,
    CalloutLinkageVerdict, CalloutMarkerInput, CalloutMarkerType, CalloutPosition,
    CalloutStyleMajority, CalloutTargetClass, CalloutToken, CalloutUnresolvedReason,
    FactsBlockStatus, FactsLineStatus, MAX_NATIVE_CALLOUT_MARKER_TOKENS,
    MAX_NATIVE_CALLOUT_REFERENCE_LINES, MEM992_FROZEN_TARGETED_CALLOUTS,
    MEM992_FROZEN_TARGETLESS_CALLOUTS, MEM992_FROZEN_TOTAL_CALLOUTS, MajorityStyleOutcome,
    NATIVE_CALLOUT_CLASSIFICATION_SCHEMA_VERSION, NATIVE_CALLOUT_INVENTORY_LANE,
    NATIVE_CALLOUT_TARGETED_DENOMINATOR, NATIVE_CALLOUT_TARGETLESS_DENOMINATOR,
    NativeCalloutClassificationError, NativeCalloutResolution, ObservedCalloutStyle,
    ReferenceSegmentKind, ReferenceSegmentSequence, ReferenceSegmentSpan,
    ReferenceSegmenterFeatureRow, build_callout_inventory, build_callout_label_ordinal_map,
    classify_callout_marker, decode_reference_sequence, majority_callout_style, marker_adjacency,
    reference_segmenter_row,
};
use da_ports::{CrfModel, FeatureTemplate, LexiconBundle, LexiconChannel};
use proptest::prelude::*;
use std::path::{Path, PathBuf};

const REFERENCE_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti";

// ─────────────────────────────── fixtures ───────────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_model(rel_path: &str) -> CrfModel {
    let bytes = std::fs::read(repo_root().join(rel_path))
        .unwrap_or_else(|error| panic!("FATAL {rel_path}: fixture missing/unreadable ({error})"));
    CrfModel::parse(&bytes)
        .unwrap_or_else(|error| panic!("FATAL {rel_path}: fixture does not parse ({error})"))
}

/// Same provider bundle shape the row-builder and sequence contracts use.
fn fixture_lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![
            LexiconChannel {
                key: "JournalNames".into(),
                entries: vec!["nature".into()],
            },
            LexiconChannel {
                key: "LocationNames".into(),
                entries: vec!["google".into()],
            },
            LexiconChannel {
                key: "commonName".into(),
                entries: vec!["google".into()],
            },
            LexiconChannel {
                key: "firstName".into(),
                entries: vec!["ashish".into()],
            },
            LexiconChannel {
                key: "lastName".into(),
                entries: vec!["vaswani".into()],
            },
        ],
        feature_templates: vec![FeatureTemplate {
            key: "citation".into(),
            channel_keys: vec!["commonName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn facts(text: &str, index: usize, count: usize) -> BibliographyTokenFacts {
    let line_status = match index {
        0 => FactsLineStatus::Linestart,
        _ if index + 1 == count => FactsLineStatus::Lineend,
        _ => FactsLineStatus::Linein,
    };
    let block_status = match index {
        0 => FactsBlockStatus::Blockstart,
        _ if index + 1 == count => FactsBlockStatus::Blockend,
        _ => FactsBlockStatus::Blockin,
    };
    BibliographyTokenFacts::from_token(&BibliographyTokenInput {
        text: text.to_string(),
        block_status,
        line_status,
        alignment: BibliographyAlignment::Alignedleft,
        line_length: 29,
        superscript: false,
    })
}

/// `12 Smith J Title 2019 Archive Press` through the real row builder and the
/// hand-authored reference-segmenter model. This is the T04 output the ordinal
/// map is supposed to consume — never a hand-shaped stand-in.
fn decoded_reference_line() -> ReferenceSegmentSequence {
    const TOKENS: [&str; 7] = ["12", "Smith", "J", "Title", "2019", "Archive", "Press"];
    let lexicon = fixture_lexicon();
    let count = TOKENS.len();
    let token_facts: Vec<BibliographyTokenFacts> = TOKENS
        .iter()
        .enumerate()
        .map(|(index, text)| facts(text, index, count))
        .collect();
    let rows: Vec<ReferenceSegmenterFeatureRow> =
        reference_segmenter_row(&token_facts, &lexicon, 29)
            .expect("the reference fixture line renders through the real row builder");
    decode_reference_sequence(&rows, &load_model(REFERENCE_FIXTURE))
        .expect("the synthetic reference model decodes the rendered line")
}

/// A decoded reference line fabricated for the ordinal-map tests (no model): the
/// label span is the first token, `line_text` is the joined token text.
fn labelled_line(label: &str) -> ReferenceSegmentSequence {
    let line_text = format!("{label} Smith J Title 2019 Archive Press");
    ReferenceSegmentSequence {
        labels: vec!["<label>".into(), "<reference>".into()],
        line_text,
        segments: vec![ReferenceSegmentSpan {
            kind: ReferenceSegmentKind::Label,
            label: "<label>".into(),
            start: 0,
            end: 1,
            byte_start: 0,
            byte_end: label.len(),
            text: label.to_string(),
        }],
    }
}

fn unlabelled_line() -> ReferenceSegmentSequence {
    ReferenceSegmentSequence {
        labels: vec!["<reference>".into()],
        line_text: "Smith J Title 2019 Archive Press".into(),
        segments: vec![ReferenceSegmentSpan {
            kind: ReferenceSegmentKind::Reference,
            label: "<reference>".into(),
            start: 0,
            end: 1,
            byte_start: 0,
            byte_end: 30,
            text: "Smith J Title 2019 Archive Press".into(),
        }],
    }
}

fn map_of(labels: &[&str]) -> CalloutLabelOrdinalMap {
    let lines: Vec<ReferenceSegmentSequence> =
        labels.iter().map(|label| labelled_line(label)).collect();
    build_callout_label_ordinal_map(&lines).expect("a handful of lines is within the map bounds")
}

fn empty_map() -> CalloutLabelOrdinalMap {
    build_callout_label_ordinal_map(&[]).expect("an empty line list is admitted")
}

// ─────────────────────────────── helpers ───────────────────────────────

fn site(line_ordinal: u32, token_ordinal: u32) -> CalloutPosition {
    CalloutPosition {
        page_ordinal: 0,
        block_ordinal: 0,
        line_ordinal,
        token_ordinal,
        annex_parent: None,
    }
}

fn tokens_at(texts: &[&str], line_ordinal: u32, start: u32) -> Vec<CalloutToken> {
    texts
        .iter()
        .enumerate()
        .map(|(offset, text)| CalloutToken {
            text: text.to_string(),
            superscript: false,
            position: site(line_ordinal, start + offset as u32),
        })
        .collect()
}

fn classify(texts: &[&str]) -> CalloutMarkerType {
    classify_callout_marker(&tokens_at(texts, 0, 0)).marker_type
}

fn marker(
    target_class: CalloutTargetClass,
    texts: &[&str],
    line: u32,
    start: u32,
) -> CalloutMarkerInput {
    CalloutMarkerInput {
        target_class,
        tokens: tokens_at(texts, line, start),
    }
}

fn citation(texts: &[&str], line: u32, start: u32) -> CalloutMarkerInput {
    marker(CalloutTargetClass::Citation, texts, line, start)
}

fn document() -> String {
    "doc-1".to_string()
}

fn source() -> String {
    "a".repeat(64)
}

fn build(
    markers: &[CalloutMarkerInput],
    labels: &CalloutLabelOrdinalMap,
) -> Result<CalloutInventory, NativeCalloutClassificationError> {
    build_callout_inventory(&document(), &source(), markers, labels)
}

fn reasons(inventory: &CalloutInventory) -> Vec<CalloutUnresolvedReason> {
    inventory
        .observations
        .iter()
        .filter_map(|observation| observation.resolution.reason())
        .collect()
}

// ────────────────────── 1. the eight marker types ──────────────────────

/// Every Java `MarkerType` constant, each reached by one hand-derived input.
///
/// Derivation (per marker, from the six Java patterns in decision order):
/// `12` → no bracket/parenthesis, `NUMBER`; superscript `12` → `NUMBER` at the
/// first check, so `SUPERSCRIPT_NUMBER`; `[12]` → `BRACKET_NUMBER`; `(12)` →
/// `PARENTHESIS_NUMBER`; `[etal]` → the bracket-number list needs a digit
/// immediately after `[`, so `BRACKET_TEXT`; `(Smith2019)` → the
/// parenthesis-number list needs a digit immediately after `(`, so
/// `PARENTHESIS_TEXT`; `IV` → matches the roman alternative; blank → `UNKNOWN`.
#[test]
fn all_eight_marker_types_are_reached_with_hand_derived_expectations() {
    let cases: [(&[&str], CalloutMarkerType); 7] = [
        (&["12"], CalloutMarkerType::Number),
        (&["[", "12", "]"], CalloutMarkerType::BracketNumber),
        (&["(", "12", ")"], CalloutMarkerType::ParenthesisNumber),
        (&["[", "etal", "]"], CalloutMarkerType::BracketText),
        (&["(", "Smith2019", ")"], CalloutMarkerType::ParenthesisText),
        (&["IV"], CalloutMarkerType::Roman),
        (&[" "], CalloutMarkerType::Unknown),
    ];
    for (texts, expected) in cases {
        assert_eq!(classify(texts), expected, "marker {texts:?}");
    }

    let superscript = vec![CalloutToken {
        text: "12".into(),
        superscript: true,
        position: site(0, 0),
    }];
    assert_eq!(
        classify_callout_marker(&superscript).marker_type,
        CalloutMarkerType::SuperscriptNumber
    );

    // The declared order is the Java `MarkerType` declaration order (the model's
    // label ids), and the check order is a different, published sequence.
    assert_eq!(
        CalloutMarkerType::ALL,
        [
            CalloutMarkerType::Unknown,
            CalloutMarkerType::BracketText,
            CalloutMarkerType::BracketNumber,
            CalloutMarkerType::ParenthesisText,
            CalloutMarkerType::ParenthesisNumber,
            CalloutMarkerType::SuperscriptNumber,
            CalloutMarkerType::Number,
            CalloutMarkerType::Roman,
        ]
    );
    assert_eq!(
        CalloutMarkerType::CHECK_ORDER,
        [
            CalloutMarkerType::SuperscriptNumber,
            CalloutMarkerType::BracketNumber,
            CalloutMarkerType::ParenthesisNumber,
            CalloutMarkerType::BracketText,
            CalloutMarkerType::ParenthesisText,
            CalloutMarkerType::Number,
            CalloutMarkerType::Roman,
            CalloutMarkerType::Unknown,
        ]
    );
    assert_eq!(
        CalloutMarkerType::SuperscriptNumber.style(),
        ObservedCalloutStyle::Numeric
    );
    assert_eq!(
        CalloutMarkerType::BracketText.style(),
        ObservedCalloutStyle::AuthorYear
    );
    assert_eq!(
        CalloutMarkerType::Roman.style(),
        ObservedCalloutStyle::Roman
    );
    assert_eq!(
        CalloutMarkerType::Unknown.style(),
        ObservedCalloutStyle::Unknown
    );
}

/// Java fidelity details that a "reasonable" reimplementation would get wrong:
/// the first check is unanchored, a blank token never clears the superscript
/// flag, only ASCII spaces are removed, and the roman pattern is a catch-all.
#[test]
fn java_fidelity_details_are_reproduced_not_corrected() {
    // Unanchored first check: a superscript bracket is a superscript number.
    let superscript_bracket = vec![
        CalloutToken {
            text: "[".into(),
            superscript: true,
            position: site(0, 0),
        },
        CalloutToken {
            text: "12".into(),
            superscript: true,
            position: site(0, 1),
        },
        CalloutToken {
            text: "]".into(),
            superscript: true,
            position: site(0, 2),
        },
    ];
    assert_eq!(
        classify_callout_marker(&superscript_bracket).marker_type,
        CalloutMarkerType::SuperscriptNumber
    );
    // A non-superscript bracket still lands on the bracket check.
    assert_eq!(
        classify(&["[", "12", "]"]),
        CalloutMarkerType::BracketNumber
    );

    // Blank tokens are skipped by the superscript loop, they do not clear it.
    let with_blank = vec![
        CalloutToken {
            text: "12".into(),
            superscript: true,
            position: site(0, 0),
        },
        CalloutToken {
            text: " ".into(),
            superscript: false,
            position: site(0, 1),
        },
    ];
    let classification = classify_callout_marker(&with_blank);
    assert!(classification.is_superscript);
    assert_eq!(
        classification.marker_type,
        CalloutMarkerType::SuperscriptNumber
    );

    // `replace(" ", "")` — spaces go, tabs stay.
    let spaced = classify_callout_marker(&tokens_at(&["[", "1", " ", "2", "]"], 0, 0));
    assert_eq!(spaced.text, "[1 2]");
    assert_eq!(spaced.compact_text, "[12]");

    // `ROMAN_PATTERN`'s third alternative matches the empty string, so the check
    // is a catch-all and `UNKNOWN` is reachable only for blank text.
    assert_eq!(classify(&["xyz"]), CalloutMarkerType::Roman);
    assert_eq!(classify(&["[]"]), CalloutMarkerType::Roman);
    assert_eq!(classify(&[]), CalloutMarkerType::Unknown);
    assert_eq!(classify(&[""]), CalloutMarkerType::Unknown);
    assert_eq!(classify(&["\t"]), CalloutMarkerType::Unknown);

    // Bracket-number list shapes: comma/`and`/`et` separators and the lowercase
    // `[a-f]` suffix, and the list is checked before bracket text.
    assert_eq!(
        classify(&["[", "12", ",", "13", "]"]),
        CalloutMarkerType::BracketNumber
    );
    assert_eq!(classify(&["[12", "a]"]), CalloutMarkerType::BracketNumber);
    assert_eq!(
        classify(&["(", "12", "&", "13", ")"]),
        CalloutMarkerType::ParenthesisNumber
    );
    // The `and`/`et` alternatives are reached through a separator, because the
    // greedy `[a-f]?` inside the atomic group swallows their leading letter
    // first: `12and13` never matches the list, `12,and13` does.
    assert_eq!(
        classify(&["(", "12", "and", "13", ")"]),
        CalloutMarkerType::ParenthesisText
    );
    assert_eq!(
        classify(&["(", "12", ",", "and", "13", ")"]),
        CalloutMarkerType::ParenthesisNumber
    );
    assert_eq!(
        classify(&["[", "Fig", "1", "]"]),
        CalloutMarkerType::BracketText
    );
}

// ───────────────────── 2. the label → ordinal map ─────────────────────

/// The ordinal source is the T04 reference-segmenter output, not a hand-shaped
/// map: decode the real synthetic model, then read the `<label>` span.
#[test]
fn reference_segmenter_output_supplies_the_ordinal_map() {
    let line = decoded_reference_line();
    let label = line
        .segments
        .iter()
        .find(|segment| segment.kind == ReferenceSegmentKind::Label)
        .expect("the decoded line carries a <label> span");
    assert_eq!(label.text, "12");

    let map = build_callout_label_ordinal_map(std::slice::from_ref(&line))
        .expect("one decoded line is within the map bounds");
    assert_eq!(map.len(), 1);
    assert!(map.unlabeled_lines.is_empty());
    assert!(map.conflicts.is_empty());
    assert_eq!(map.entries[0].ordinal, 1);
    assert_eq!(map.entries[0].label_text, "12");
    assert_eq!(map.entries[0].label_key, "12");
    assert_eq!(
        (map.entries[0].byte_start, map.entries[0].byte_end),
        (label.byte_start, label.byte_end)
    );

    // `[12] → bibl #12`, whatever punctuation the document printed.
    for printed in ["[12]", "(12)", "12.", "12"] {
        assert_eq!(
            map.resolve_label(printed),
            CalloutLabelResolution::Resolved {
                ordinal: 1,
                key: "12".into()
            },
            "printed {printed}"
        );
    }
    assert_eq!(
        map.resolve_label("[99]"),
        CalloutLabelResolution::Unmapped { key: "99".into() }
    );
    assert_eq!(
        map.resolve_label("[]"),
        CalloutLabelResolution::Unmapped { key: String::new() }
    );
}

/// Lines without a usable label are recorded, and a key two lines claim is a
/// reported conflict — never a guess.
#[test]
fn unlabeled_lines_are_recorded_and_conflicts_are_never_guessed() {
    let lines = vec![
        labelled_line("[12]"),
        unlabelled_line(),
        labelled_line("(12)"),
        labelled_line("12."),
    ];
    let map = build_callout_label_ordinal_map(&lines).expect("four lines are admitted");
    assert_eq!(map.len(), 3);
    assert_eq!(map.unlabeled_lines, vec![1]);
    assert_eq!(map.conflicts.len(), 1);
    assert_eq!(map.conflicts[0].key, "12");
    assert_eq!(map.conflicts[0].ordinals, vec![1, 3, 4]);
    assert_eq!(
        map.resolve_label("[12]"),
        CalloutLabelResolution::Ambiguous {
            key: "12".into(),
            ordinals: vec![1, 3, 4]
        }
    );

    let empty = empty_map();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(
        empty.resolve_label("[12]"),
        CalloutLabelResolution::Unmapped { key: "12".into() }
    );

    // Admission bound: a 100k-line bibliography is not silently truncated.
    let oversized: Vec<ReferenceSegmentSequence> =
        vec![unlabelled_line(); MAX_NATIVE_CALLOUT_REFERENCE_LINES + 1];
    assert!(matches!(
        build_callout_label_ordinal_map(&oversized),
        Err(NativeCalloutClassificationError::LimitsExceeded {
            resource: "reference_line",
            ..
        })
    ));
}

// ────────────────────── 3. majority style voting ──────────────────────

#[test]
fn majority_style_is_selected_tie_or_empty() {
    let numeric = [
        citation(&["[", "1", "]"], 0, 0),
        citation(&["[", "2", "]"], 0, 4),
    ];
    let classifications: Vec<_> = numeric
        .iter()
        .map(|marker| classify_callout_marker(&marker.tokens))
        .collect();
    let selected = majority_callout_style(&classifications);
    assert_eq!(selected.outcome, MajorityStyleOutcome::Selected);
    assert_eq!(selected.style, Some(ObservedCalloutStyle::Numeric));
    assert_eq!(selected.candidate_count, 2);
    assert_eq!(
        selected.counts,
        vec![da_parse::CalloutStyleCount {
            style: ObservedCalloutStyle::Numeric,
            count: 2
        }]
    );

    let mixed: Vec<_> = [
        citation(&["[", "1", "]"], 0, 0),
        citation(&["(", "Smith2019", ")"], 0, 4),
    ]
    .iter()
    .map(|marker| classify_callout_marker(&marker.tokens))
    .collect();
    let tie = majority_callout_style(&mixed);
    assert_eq!(tie.outcome, MajorityStyleOutcome::Tie);
    assert_eq!(tie.style, None);
    assert_eq!(tie.candidate_count, 2);
    // Counts are ordered by `ObservedCalloutStyle`, so replay is byte-identical.
    assert_eq!(tie.counts.len(), 2);

    let empty = majority_callout_style(&[]);
    assert_eq!(empty.outcome, MajorityStyleOutcome::Empty);
    assert_eq!(empty.style, None);
    assert_eq!(empty.candidate_count, 0);
    assert!(empty.counts.is_empty());

    // A blank marker votes `Unknown` and is still counted: dropping an
    // occurrence is exactly the erosion MEM992 forbids.
    let with_blank = [citation(&[" "], 0, 0)];
    let unknown: Vec<_> = with_blank
        .iter()
        .map(|marker| classify_callout_marker(&marker.tokens))
        .collect();
    let all_unknown = majority_callout_style(&unknown);
    assert_eq!(all_unknown.style, Some(ObservedCalloutStyle::Unknown));
    assert_eq!(all_unknown.candidate_count, 1);
}

// ───────────────────── 4. typed unresolved reasons ─────────────────────

/// All six reasons the reused vocabulary carries are reachable, each by one
/// distinct construction. A reason that no input can produce would be dead
/// vocabulary; a marker that resolves through two rules would be a silent
/// double count. Neither is allowed here.
#[test]
fn all_six_unresolved_reasons_are_reachable() {
    // NoTarget: the bibliography inventory is empty.
    let no_target = build(&[citation(&["[", "12", "]"], 0, 0)], &empty_map()).unwrap();
    assert_eq!(reasons(&no_target), vec![CalloutUnresolvedReason::NoTarget]);

    // TargetNotInInventory: the inventory is non-empty, this key is not in it.
    let absent = build(&[citation(&["[", "9", "]"], 0, 0)], &map_of(&["[12]"])).unwrap();
    assert_eq!(
        reasons(&absent),
        vec![CalloutUnresolvedReason::TargetNotInInventory]
    );

    // Ambiguous: two reference lines claim the same printed ordinal.
    let ambiguous = build(
        &[citation(&["[", "12", "]"], 0, 0)],
        &map_of(&["[12]", "(12)"]),
    )
    .unwrap();
    assert_eq!(
        reasons(&ambiguous),
        vec![CalloutUnresolvedReason::Ambiguous]
    );

    // IllegalAdjacency: the marker's own tokens cross a line boundary, so it is
    // not one marker — even though its text would resolve to ordinal 1.
    let mut crossing = citation(&["[", "12", "]"], 0, 0);
    crossing.tokens[1].position.line_ordinal = 1;
    crossing.tokens[2].position.line_ordinal = 1;
    let illegal = build(&[crossing], &map_of(&["[12]"])).unwrap();
    assert_eq!(
        reasons(&illegal),
        vec![CalloutUnresolvedReason::IllegalAdjacency]
    );
    assert_eq!(
        illegal.observations[0].marker.adjacency,
        AdjacencyDisposition::IllegalBoundaryCrossing
    );

    // UnsupportedEquation and SourceNotEstablished: the other classes this lane
    // does not establish an inventory for.
    let equation = build(
        &[marker(CalloutTargetClass::Equation, &["3"], 0, 0)],
        &map_of(&["[12]"]),
    )
    .unwrap();
    assert_eq!(
        reasons(&equation),
        vec![CalloutUnresolvedReason::UnsupportedEquation]
    );
    let figure = build(
        &[marker(CalloutTargetClass::Figure, &["Fig", "1"], 0, 0)],
        &map_of(&["[12]"]),
    )
    .unwrap();
    assert_eq!(
        reasons(&figure),
        vec![CalloutUnresolvedReason::SourceNotEstablished]
    );
    let blank = build(&[citation(&[" "], 0, 0)], &map_of(&["[12]"])).unwrap();
    assert_eq!(
        reasons(&blank),
        vec![CalloutUnresolvedReason::SourceNotEstablished]
    );
    assert_eq!(
        blank.observations[0].marker.marker_type,
        CalloutMarkerType::Unknown
    );
}

// ───────────── 5. inventory: every occurrence, two denominators ─────────────

#[test]
fn every_occurrence_lands_in_exactly_one_checksum_bound_denominator() {
    let markers = vec![
        citation(&["[", "12", "]"], 0, 0),
        citation(&["[", "99", "]"], 0, 5),
        marker(CalloutTargetClass::Figure, &["Fig", "1"], 0, 10),
        marker(CalloutTargetClass::Equation, &["3"], 0, 20),
    ];
    let inventory = build(&markers, &map_of(&["[12]"])).unwrap();

    assert_eq!(
        inventory.schema_version,
        NATIVE_CALLOUT_CLASSIFICATION_SCHEMA_VERSION
    );
    assert_eq!(inventory.document_id, document());
    assert_eq!(inventory.source_sha256, source());
    assert_eq!(inventory.lane, NATIVE_CALLOUT_INVENTORY_LANE);
    assert_eq!(inventory.total_count(), 4);
    assert_eq!(inventory.targeted_count(), 1);
    assert_eq!(inventory.targetless_count(), 3);
    assert_eq!(
        inventory.targeted_count() + inventory.targetless_count(),
        inventory.total_count(),
        "the denominators must partition the admitted occurrences"
    );
    assert_eq!(
        inventory.targeted.label,
        NATIVE_CALLOUT_TARGETED_DENOMINATOR
    );
    assert_eq!(
        inventory.targetless.label,
        NATIVE_CALLOUT_TARGETLESS_DENOMINATOR
    );
    assert!(inventory.targeted.checksum.starts_with("sha256:"));
    assert!(inventory.targetless.checksum.starts_with("sha256:"));
    assert_ne!(
        inventory.targeted.checksum, inventory.targetless.checksum,
        "the two denominators are checksum-disjoint by construction, never poolable"
    );
    assert_eq!(inventory.verdict(), CalloutLinkageVerdict::Partial);
    assert!(!inventory.is_complete());
    assert_eq!(
        inventory.observations[0].resolution,
        NativeCalloutResolution::Targeted {
            ordinal: 1,
            label_key: "12".into()
        }
    );
    assert_eq!(inventory.observations[0].position, site(0, 0));
    assert_eq!(
        inventory.majority.style,
        Some(ObservedCalloutStyle::Numeric)
    );
    inventory.validate().expect("a built inventory validates");
}

#[test]
fn a_fully_targeted_inventory_is_the_only_complete_verdict() {
    let markers = vec![
        citation(&["[", "12", "]"], 0, 0),
        citation(&["(", "12", ")"], 0, 5),
    ];
    let inventory = build(&markers, &map_of(&["[12]"])).unwrap();
    assert_eq!(inventory.targeted_count(), 2);
    assert_eq!(inventory.targetless_count(), 0);
    assert_eq!(inventory.verdict(), CalloutLinkageVerdict::Complete);
    assert!(inventory.is_complete());
}

// ───────────────── 6. the targetless-heavy negative ─────────────────

/// MEM992: the Java lifter skips targetless bibr refs, and the frozen rich-TEI
/// corpus therefore carries 217 targeted + 163 targetless callouts = 380. A
/// document where every admitted callout is targetless must never be reportable
/// as success, and its 163 occurrences must still be counted — erosion into a
/// smaller, prettier denominator is the failure mode this test exists to block.
#[test]
fn targetless_heavy_inventory_cannot_be_reported_as_success() {
    assert_eq!(
        MEM992_FROZEN_TARGETED_CALLOUTS + MEM992_FROZEN_TARGETLESS_CALLOUTS,
        MEM992_FROZEN_TOTAL_CALLOUTS
    );

    // 49 labelled references, 163 in-body markers that name none of them.
    let labels: Vec<String> = (1..=49).map(|ordinal| format!("[{ordinal}]")).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let map = map_of(&label_refs);
    let markers: Vec<CalloutMarkerInput> = (0..MEM992_FROZEN_TARGETLESS_CALLOUTS)
        .map(|index| {
            let text = format!("[{}]", 50 + index);
            let owned = [text];
            let borrowed: Vec<&str> = owned.iter().map(String::as_str).collect();
            citation(&borrowed, 0, index as u32)
        })
        .collect();

    let inventory = build(&markers, &map).unwrap();
    inventory
        .validate()
        .expect("the targetless inventory validates");

    assert_eq!(inventory.total_count(), MEM992_FROZEN_TARGETLESS_CALLOUTS);
    assert_eq!(
        inventory.targetless_count(),
        MEM992_FROZEN_TARGETLESS_CALLOUTS
    );
    assert_eq!(inventory.targeted_count(), 0);
    assert_eq!(inventory.verdict(), CalloutLinkageVerdict::Targetless);
    assert!(
        !inventory.is_complete(),
        "a targetless-heavy inventory must not be reportable as success"
    );
    assert_eq!(
        inventory.targeted_count() + inventory.targetless_count(),
        inventory.total_count(),
        "no occurrence may be dropped to make the denominators look better"
    );
    assert_eq!(
        reasons(&inventory)
            .into_iter()
            .filter(|reason| *reason == CalloutUnresolvedReason::TargetNotInInventory)
            .count(),
        MEM992_FROZEN_TARGETLESS_CALLOUTS
    );
    assert_ne!(inventory.targeted.checksum, inventory.targetless.checksum);

    // The same rules hold for the mixed frozen split: 217 targeted would still
    // not be a success while 163 occurrences are targetless.
    let mixed = da_parse::callout_linkage_verdict(
        MEM992_FROZEN_TARGETED_CALLOUTS,
        MEM992_FROZEN_TARGETLESS_CALLOUTS,
    );
    assert_eq!(mixed, CalloutLinkageVerdict::Partial);
    assert_eq!(
        da_parse::callout_linkage_verdict(0, 0),
        CalloutLinkageVerdict::Empty,
        "an empty denominator is not a success either"
    );
}

// ───────────────── 7. tampering is refused, not averaged ─────────────────

#[test]
fn tampered_denominators_and_observations_are_refused() {
    let markers = vec![
        citation(&["[", "12", "]"], 0, 0),
        citation(&["[", "99", "]"], 0, 5),
    ];
    let base = build(&markers, &map_of(&["[12]"])).unwrap();
    base.validate().expect("the untampered inventory validates");

    let mut eroded = base.clone();
    eroded.targeted.count += 1;
    assert!(matches!(
        eroded.validate(),
        Err(NativeCalloutClassificationError::DenominatorCountMismatch { .. })
    ));

    let mut wrong_checksum = base.clone();
    wrong_checksum.targeted.checksum = "sha256:0".into();
    assert!(matches!(
        wrong_checksum.validate(),
        Err(NativeCalloutClassificationError::DenominatorChecksumMismatch { .. })
    ));

    let mut pooled = base.clone();
    pooled.targeted.checksum = pooled.targetless.checksum.clone();
    assert!(matches!(
        pooled.validate(),
        Err(NativeCalloutClassificationError::PooledDenominator)
    ));

    let mut relabelled = base.clone();
    relabelled.targetless.label = "figure_n".into();
    assert!(matches!(
        relabelled.validate(),
        Err(NativeCalloutClassificationError::DenominatorLabelMismatch { .. })
    ));

    let mut tampered_digest = base.clone();
    tampered_digest.observations[1].digest = "sha256:deadbeef".into();
    assert!(matches!(
        tampered_digest.validate(),
        Err(NativeCalloutClassificationError::ObservationDigestMismatch { index: 1 })
    ));

    let mut wrong_index = base.clone();
    wrong_index.observations[0].index = 7;
    assert!(matches!(
        wrong_index.validate(),
        Err(NativeCalloutClassificationError::ObservationIndexMismatch { index: 0, .. })
    ));

    let mut wrong_style = base.clone();
    wrong_style.observations[0].marker.style = ObservedCalloutStyle::Roman;
    assert!(matches!(
        wrong_style.validate(),
        Err(NativeCalloutClassificationError::StyleMismatch { index: 0 })
    ));

    let mut wrong_majority = base.clone();
    wrong_majority.majority = CalloutStyleMajority {
        outcome: MajorityStyleOutcome::Selected,
        style: Some(ObservedCalloutStyle::Roman),
        counts: Vec::new(),
        candidate_count: 2,
    };
    assert!(matches!(
        wrong_majority.validate(),
        Err(NativeCalloutClassificationError::MajorityMismatch)
    ));

    let mut no_identity = base.clone();
    no_identity.document_id = String::new();
    assert!(matches!(
        no_identity.validate(),
        Err(NativeCalloutClassificationError::MissingIdentity {
            field: "document_id"
        })
    ));

    let mut emptied = base.clone();
    emptied.observations.clear();
    assert!(matches!(
        emptied.validate(),
        Err(NativeCalloutClassificationError::EmptyInventory)
    ));
}

#[test]
fn admission_refusals_are_typed() {
    let map = map_of(&["[12]"]);

    // A marker with no tokens is not an occurrence.
    assert_eq!(
        build(
            &[CalloutMarkerInput {
                target_class: CalloutTargetClass::Citation,
                tokens: Vec::new()
            }],
            &map
        ),
        Err(NativeCalloutClassificationError::EmptyMarker { index: 0 })
    );
    // No markers at all: no inventory, no claim.
    assert_eq!(
        build(&[], &map),
        Err(NativeCalloutClassificationError::EmptyInventory)
    );
    // Identity is mandatory.
    assert_eq!(
        build_callout_inventory("", &source(), &[citation(&["[", "12", "]"], 0, 0)], &map),
        Err(NativeCalloutClassificationError::MissingIdentity {
            field: "document_id"
        })
    );
    assert_eq!(
        build_callout_inventory(&document(), "", &[citation(&["[", "12", "]"], 0, 0)], &map),
        Err(NativeCalloutClassificationError::MissingIdentity {
            field: "source_sha256"
        })
    );
    // A single marker cannot smuggle unbounded work past the token bound.
    let oversized: Vec<CalloutToken> = (0..=MAX_NATIVE_CALLOUT_MARKER_TOKENS)
        .map(|ordinal| CalloutToken {
            text: "1".into(),
            superscript: false,
            position: site(0, ordinal as u32),
        })
        .collect();
    assert!(matches!(
        build(
            &[CalloutMarkerInput {
                target_class: CalloutTargetClass::Citation,
                tokens: oversized
            }],
            &map
        ),
        Err(NativeCalloutClassificationError::LimitsExceeded {
            resource: "marker_token",
            ..
        })
    ));
    // Document order is the only order admitted.
    assert!(matches!(
        build(
            &[
                citation(&["[", "12", "]"], 0, 10),
                citation(&["[", "12", "]"], 0, 5)
            ],
            &map
        ),
        Err(NativeCalloutClassificationError::NonMonotonic { index: 1 })
    ));
}

// ───────────────── 8. property: adjacency / illegal boundary ─────────────────

#[derive(Debug, Clone, Copy)]
enum Deviation {
    /// Same boundary, a gap in the token ordinals.
    Jump,
    /// A line change.
    Cross,
}

/// Build a marker whose consecutive token pairs follow a deviation script; the
/// script — not the implementation — is the expectation's source.
fn scripted_marker(start: u32, script: &[Option<Deviation>]) -> Vec<CalloutToken> {
    let mut tokens = Vec::new();
    let mut line_ordinal = 0u32;
    let mut token_ordinal = start;
    tokens.push(CalloutToken {
        text: "1".into(),
        superscript: false,
        position: site(line_ordinal, token_ordinal),
    });
    for (index, deviation) in script.iter().enumerate() {
        match deviation {
            None => token_ordinal += 1,
            Some(Deviation::Jump) => token_ordinal += 5,
            Some(Deviation::Cross) => {
                line_ordinal += 1;
                token_ordinal += 1;
            }
        }
        tokens.push(CalloutToken {
            text: (index + 2).to_string(),
            superscript: false,
            position: site(line_ordinal, token_ordinal),
        });
    }
    tokens
}

fn deviation_script() -> impl Strategy<Value = Vec<Option<Deviation>>> {
    proptest::collection::vec(
        proptest::option::of(prop_oneof![Just(Deviation::Jump), Just(Deviation::Cross)]),
        0..6,
    )
}

proptest! {
    /// A boundary crossing anywhere is illegal, whatever else the script does;
    /// otherwise the first gap makes the marker non-adjacent. Boundary crossings
    /// dominate (fail-closed), so a marker that crosses can never resolve.
    #[test]
    fn adjacency_follows_the_deviation_script(start in 0u32..64, script in deviation_script()) {
        let tokens = scripted_marker(start, &script);
        let expected = if script.iter().any(|step| matches!(step, Some(Deviation::Cross))) {
            AdjacencyDisposition::IllegalBoundaryCrossing
        } else if script.iter().any(|step| matches!(step, Some(Deviation::Jump))) {
            AdjacencyDisposition::NonAdjacent
        } else {
            AdjacencyDisposition::Adjacent
        };
        prop_assert_eq!(marker_adjacency(&tokens), expected);
    }

    /// Safety: `Adjacent` implies every consecutive pair shares its boundary and
    /// is consecutive in token order; a cross is never reported as adjacent.
    #[test]
    fn adjacency_never_hides_a_boundary_crossing(start in 0u32..64, script in deviation_script()) {
        let tokens = scripted_marker(start, &script);
        let disposition = marker_adjacency(&tokens);
        if script.iter().any(|step| matches!(step, Some(Deviation::Cross))) {
            prop_assert_ne!(disposition, AdjacencyDisposition::Adjacent);
        }
        if disposition == AdjacencyDisposition::Adjacent {
            for pair in tokens.windows(2) {
                prop_assert_eq!(pair[0].position.line_ordinal, pair[1].position.line_ordinal);
                prop_assert_eq!(pair[0].position.page_ordinal, pair[1].position.page_ordinal);
                prop_assert_eq!(pair[0].position.block_ordinal, pair[1].position.block_ordinal);
                prop_assert_eq!(pair[0].position.annex_parent, pair[1].position.annex_parent);
                prop_assert_eq!(
                    pair[0].position.token_ordinal + 1,
                    pair[1].position.token_ordinal
                );
            }
        }
    }
}

#[test]
fn adjacency_boundary_cases_are_typed() {
    // No tokens: no source, so no adjacency claim.
    assert_eq!(
        marker_adjacency(&[]),
        AdjacencyDisposition::SourceNotEstablished
    );
    // One token is trivially contiguous.
    assert_eq!(
        marker_adjacency(&tokens_at(&["12"], 0, 3)),
        AdjacencyDisposition::Adjacent
    );
    // A same-line gap.
    let gap = vec![
        CalloutToken {
            text: "1".into(),
            superscript: false,
            position: site(0, 0),
        },
        CalloutToken {
            text: "2".into(),
            superscript: false,
            position: site(0, 4),
        },
    ];
    assert_eq!(marker_adjacency(&gap), AdjacencyDisposition::NonAdjacent);
    // A gap followed by a crossing is still illegal: boundaries dominate.
    let gap_then_cross = vec![
        CalloutToken {
            text: "1".into(),
            superscript: false,
            position: site(0, 0),
        },
        CalloutToken {
            text: "2".into(),
            superscript: false,
            position: site(0, 4),
        },
        CalloutToken {
            text: "3".into(),
            superscript: false,
            position: site(1, 5),
        },
    ];
    assert_eq!(
        marker_adjacency(&gap_then_cross),
        AdjacencyDisposition::IllegalBoundaryCrossing
    );
    // An annex crossing is a boundary crossing too.
    let annex = vec![
        CalloutToken {
            text: "1".into(),
            superscript: false,
            position: CalloutPosition {
                annex_parent: Some(0),
                ..site(0, 0)
            },
        },
        CalloutToken {
            text: "2".into(),
            superscript: false,
            position: CalloutPosition {
                annex_parent: Some(1),
                ..site(0, 1)
            },
        },
    ];
    assert_eq!(
        marker_adjacency(&annex),
        AdjacencyDisposition::IllegalBoundaryCrossing
    );
}

// ───────────────────── 9. replay and roundtrip ─────────────────────

#[test]
fn the_inventory_replays_byte_identically_and_roundtrips_json() {
    let markers = vec![
        citation(&["[", "12", "]"], 0, 0),
        citation(&["[", "99", "]"], 0, 5),
        marker(CalloutTargetClass::Figure, &["Fig", "1"], 0, 10),
    ];
    let map = map_of(&["[12]", "13."]);
    let first = build(&markers, &map).unwrap();
    let second = build(&markers, &map).unwrap();
    assert_eq!(first, second, "equal inputs must produce equal inventories");
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );

    let encoded = serde_json::to_string(&first).unwrap();
    let decoded: CalloutInventory = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, first);
    decoded
        .validate()
        .expect("a roundtripped inventory still validates");
    assert_eq!(decoded.targeted, first.targeted);
    assert_eq!(decoded.targetless, first.targetless);

    // The denominators are self-describing: a reader never has to guess which
    // lane or which identity a count belongs to.
    let denominators: [&CalloutDenominator; 2] = [&first.targeted, &first.targetless];
    assert_eq!(
        denominators
            .iter()
            .map(|denominator| denominator.label.as_str())
            .collect::<Vec<_>>(),
        vec!["targeted", "targetless"]
    );
    assert!(first.lane.contains("callout"));
    assert_ne!(first.lane, "figure_n");
    assert_ne!(first.lane, "citation_n");
}
