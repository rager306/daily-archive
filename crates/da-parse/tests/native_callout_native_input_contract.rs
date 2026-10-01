//! Contract tests for the additive native callout-marker band (S05/T04).
//!
//! The tests build the S03 plane directly (document + lattice + lexemes +
//! segmentation) with tracked inline fixtures, so the band is exercised without
//! any donor, TEI or gitignored asset. The real corpus measurement belongs to the
//! S05 receipt runner; here the surface, the closed vocabularies, the typed
//! absences, the typed-unresolved matching, the fail-closed rules and the
//! redaction discipline are pinned.
//!
//! The Java format authority for the *classifier* is pinned by
//! `native_callout_classification_contract.rs`; this file pins the *input band*
//! and its rules, each negative case by its named failure.

use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

/// One token of an inline plane line.
struct PWord {
    text: &'static str,
    font_milli: Option<u32>,
}

impl PWord {
    fn word(text: &'static str) -> Self {
        Self {
            text,
            font_milli: Some(10_000),
        }
    }

    fn small(text: &'static str) -> Self {
        Self {
            text,
            font_milli: Some(7_000),
        }
    }
}

/// Build the S03 plane from inline lines; every line is one block. A standalone
/// `introduction` line (the S03 Body marker) opens the Body region, because the
/// segmenter starts in `Header` and only a region marker moves it.
fn plane(
    document_id: &str,
    lines: Vec<Vec<PWord>>,
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    let lines: Vec<Vec<PWord>> = std::iter::once(vec![PWord::word("introduction")])
        .chain(lines)
        .collect();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut ranges: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut y = 100.0;
    for line in &lines {
        let start = tokens.len();
        let first_y = y;
        for word in line {
            let index = tokens.len() as u32;
            tokens.push(
                LayoutToken::new(
                    0,
                    index,
                    index,
                    LayoutBBox::new(20.0, y, 400.0, y + 10.0).unwrap(),
                    digest(word.text),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word.text),
                    LayoutStyleHints {
                        font_size_milli_pt: word.font_milli,
                        ..LayoutStyleHints::default()
                    },
                    provider(),
                )
                .unwrap(),
            );
            lexemes.push(LexemeRow::new(
                document_id,
                "fixture-donor",
                0,
                index,
                digest(word.text),
                word.text,
            ));
            y += 12.0;
        }
        ranges.push((start, tokens.len(), first_y, y));
    }
    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 100_000.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 600.0,
            height: 100_000.0,
            rotation: 0,
            crop_box: None,
            blocks: ranges
                .iter()
                .enumerate()
                .map(
                    |(block_index, (start, end, first_y, last_y))| LatticeBlock {
                        source_ordinal: block_index as u32,
                        source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                        bbox: LayoutBBox::new(0.0, *first_y, 600.0, *last_y).unwrap(),
                        source_order: block_index as u32,
                        derived_order: Some(block_index as u32),
                        furniture: FurnitureEvidence::Absent,
                        lines: vec![LatticeLine {
                            source_ordinal: 0,
                            source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                            bbox: LayoutBBox::new(0.0, *first_y, 600.0, *last_y).unwrap(),
                            newline_boundary_count: 1,
                            tokens: tokens[*start..*end]
                                .iter()
                                .filter(|token| token.kind.is_lexical())
                                .map(|token| LatticeToken {
                                    source_ordinal: token.source_ordinal,
                                    token_ordinal: token.token_ordinal,
                                    bbox: token.bbox,
                                    text_digest: token.text_digest.clone(),
                                    source_span: token.source_span.clone(),
                                    kind: token.kind,
                                })
                                .collect(),
                        }],
                    },
                )
                .collect(),
        }],
    )
    .unwrap();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    (document, lattice, lexemes, segmentation)
}

/// The four marker families plus one author-year citation, one line each.
fn marker_plane(
    document_id: &str,
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    plane(
        document_id,
        vec![
            vec![PWord::word("alpha"), PWord::word("[12]")],
            vec![PWord::word("beta"), PWord::word("Fig."), PWord::word("3")],
            vec![PWord::word("gamma"), PWord::word("Table"), PWord::word("2")],
            vec![PWord::word("delta"), PWord::word("Eq."), PWord::word("(4)")],
            vec![PWord::word("epsilon"), PWord::word("[Smith2019]")],
        ],
    )
}

/// A plane whose callout styles tie: one author-year, one numeric.
fn tie_plane(
    document_id: &str,
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    plane(
        document_id,
        vec![
            vec![PWord::word("alpha"), PWord::word("[Smith2019]")],
            vec![PWord::word("beta"), PWord::word("Fig."), PWord::word("3")],
        ],
    )
}

/// A plane with tokens but no callout-shaped run.
fn markerless_plane(
    document_id: &str,
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    plane(
        document_id,
        vec![
            vec![PWord::word("alpha"), PWord::word("beta")],
            vec![PWord::word("gamma"), PWord::word("delta")],
        ],
    )
}

fn empty_plane(
    document_id: &str,
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    let document = LayoutDocument::new(document_id, "b".repeat(64), vec![]).unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![],
    )
    .unwrap();
    let segmentation =
        segment_document_full(&document, &lattice, &[], SegmentationConfig::default()).unwrap();
    (document, lattice, Vec::new(), segmentation)
}

fn run(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    segmentation: &FullSegmentationOutcome,
) -> NativeCalloutFromInputOutcome {
    build_native_callout_input(
        document,
        lattice,
        lexemes,
        segmentation,
        &NATIVE_CALLOUT_REGIONS,
    )
    .expect("the fixture plane must execute the callout band")
}

fn strip_line_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(cut) => &line[..cut],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn callout_token(text: &str, line: u32, token: u32) -> CalloutToken {
    CalloutToken {
        text: text.to_owned(),
        superscript: false,
        position: CalloutPosition {
            page_ordinal: 0,
            block_ordinal: 0,
            line_ordinal: line,
            token_ordinal: token,
            annex_parent: None,
        },
    }
}

#[test]
fn closed_vocabularies_have_no_out_of_scope() {
    let families = NATIVE_CALLOUT_FAMILIES.map(|family| match family {
        CalloutTargetClass::Citation => "citation",
        CalloutTargetClass::Figure => "figure",
        CalloutTargetClass::Table => "table",
        CalloutTargetClass::Equation => "equation",
    });
    assert_eq!(families, NATIVE_CALLOUT_FAMILY_VOCABULARY);
    assert_eq!(
        NATIVE_CALLOUT_FAMILY_DISPOSITION_VOCABULARY,
        ["measured", "unmet", "not_observed"]
    );
    assert!(!NATIVE_CALLOUT_FAMILY_DISPOSITION_VOCABULARY.contains(&"out_of_scope"));
    for disposition in [
        NativeCalloutFamilyDisposition::Measured { native_markers: 1 },
        NativeCalloutFamilyDisposition::Unmet {
            reason: NativeCalloutUnmetReason::MarkerShapeUnreachable,
        },
        NativeCalloutFamilyDisposition::NotObserved,
    ] {
        let text = serde_json::to_string(&disposition).unwrap();
        assert!(
            !text.contains("scope"),
            "a required-family disposition must never carry a scope reduction: {text}"
        );
    }
}

#[test]
fn native_input_surface_names_no_donor_or_tei_type() {
    let source = include_str!("../src/native_callout_native_input.rs");
    assert!(source.len() > 20_000, "the surface must stay substantive");
    let code = strip_line_comments(source);
    for marker in [
        "da_adapters",
        "LayoutVisual",
        "ParserPort",
        "ParsedArticle",
        "TeiEnvelope",
        "TeiLifter",
        "GraphStore",
        "Samyama",
        "std::fs",
        "std::env",
        "std::process",
        "Command::",
        "reqwest",
        "println!",
        "tracing",
        // The target-inventory path is TEI-derived and must never be entered.
        "build_callout_label_ordinal_map",
        "ReferenceSegmentSequence",
    ] {
        assert!(
            !code.contains(marker),
            "the native callout-input surface must not name `{marker}` in code"
        );
    }
    for required in [
        "build_native_callout_input",
        "extract_native_callout_markers",
        "validate_native_callout_marker",
        "evaluate_callout_family_rows",
        "require_independent_callout_oracle",
        "require_non_vacuous_callout_oracle",
        "classify_callout_marker",
        "majority_callout_style",
        "build_callout_inventory",
    ] {
        assert!(source.contains(required), "the API lost {required}");
    }
}

#[test]
fn consecutive_builds_are_byte_identical() {
    let (document, lattice, lexemes, segmentation) = marker_plane("determinism-doc");
    let first = run(&document, &lattice, &lexemes, &segmentation);
    let second = run(&document, &lattice, &lexemes, &segmentation);
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn empty_document_is_a_typed_lane_absence() {
    let (document, lattice, lexemes, segmentation) = empty_plane("empty-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    assert!(
        outcome.inventory.is_none(),
        "an empty plane is not an inventory"
    );
    assert_eq!(outcome.absence, Some(NativeCalloutInputAbsence::LaneAbsent));
    assert_eq!(outcome.total_markers(), 0);
    for family in NATIVE_CALLOUT_FAMILIES {
        assert_eq!(outcome.admitted_markers(family), 0);
    }
}

#[test]
fn a_plane_without_markers_is_a_typed_absence_not_a_zero() {
    let (document, lattice, lexemes, segmentation) = markerless_plane("markerless-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    assert!(outcome.inventory.is_none());
    assert_eq!(
        outcome.absence,
        Some(NativeCalloutInputAbsence::NoMarkerShapedRun)
    );
}

#[test]
fn markers_are_classified_and_stay_typed_unresolved() {
    let (document, lattice, lexemes, segmentation) = marker_plane("markers-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    assert_eq!(
        outcome.target_inventory_status,
        NativeCalloutTargetInventoryStatus::SourceNotEstablished
    );
    assert_eq!(outcome.total_markers(), 5);
    assert_eq!(
        outcome.admitted_markers(CalloutTargetClass::Citation),
        2,
        "a bracketed number and a bracketed author-year are both citations"
    );
    assert_eq!(outcome.admitted_markers(CalloutTargetClass::Figure), 1);
    assert_eq!(outcome.admitted_markers(CalloutTargetClass::Table), 1);
    assert_eq!(outcome.admitted_markers(CalloutTargetClass::Equation), 1);

    let inventory = outcome.inventory.as_ref().expect("markers were admitted");
    assert_eq!(inventory.markers.len(), inventory.matching.len());
    for (marker, matching) in inventory.markers.iter().zip(inventory.matching.iter()) {
        assert_eq!(marker.index, matching.index);
        assert_eq!(marker.family, matching.family);
        assert!(
            matches!(
                matching.disposition,
                NativeCalloutMatchingDisposition::Unresolved { .. }
            ),
            "every marker stays typed unresolved while the inventory is not established"
        );
        assert!(marker.digest.starts_with("sha256:"));
    }
    assert_eq!(inventory.verdict, CalloutLinkageVerdict::Targetless);
}

#[test]
fn each_family_produces_its_own_typed_unresolved_reason() {
    let (document, lattice, lexemes, segmentation) = marker_plane("reasons-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    let reason = |family: CalloutTargetClass| {
        inventory
            .markers
            .iter()
            .find(|marker| marker.family == family)
            .map(|marker| match &marker.resolution {
                NativeCalloutResolution::Unresolved { reason } => *reason,
                NativeCalloutResolution::Targeted { .. } => panic!("no marker may resolve"),
            })
            .expect("every family is present")
    };
    assert_eq!(
        reason(CalloutTargetClass::Citation),
        CalloutUnresolvedReason::NoTarget
    );
    assert_eq!(
        reason(CalloutTargetClass::Figure),
        CalloutUnresolvedReason::SourceNotEstablished
    );
    assert_eq!(
        reason(CalloutTargetClass::Table),
        CalloutUnresolvedReason::SourceNotEstablished
    );
    assert_eq!(
        reason(CalloutTargetClass::Equation),
        CalloutUnresolvedReason::UnsupportedEquation
    );
}

#[test]
fn equation_marker_is_unsupported_and_never_matched() {
    let (document, lattice, lexemes, segmentation) = marker_plane("equation-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    let equation = inventory
        .markers
        .iter()
        .find(|marker| marker.family == CalloutTargetClass::Equation)
        .expect("the equation marker was extracted");
    assert_eq!(
        equation.marker_type,
        CalloutMarkerType::ParenthesisNumber,
        "`Eq. (4)` is a parenthesized number, not a bare number"
    );
    assert_eq!(
        equation.resolution,
        NativeCalloutResolution::Unresolved {
            reason: CalloutUnresolvedReason::UnsupportedEquation
        }
    );
}

#[test]
fn majority_tie_is_reported_and_never_matched() {
    let (document, lattice, lexemes, segmentation) = tie_plane("tie-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    assert_eq!(inventory.majority.outcome, MajorityStyleOutcome::Tie);
    assert_eq!(inventory.majority.style, None);
    for matching in &inventory.matching {
        assert!(
            matches!(
                matching.disposition,
                NativeCalloutMatchingDisposition::Unresolved { .. }
            ),
            "a tie can never produce a match"
        );
    }
}

#[test]
fn fuzzy_target_claim_is_refused_by_name() {
    let resolution = NativeCalloutResolution::Targeted {
        ordinal: 12,
        label_key: "12".to_owned(),
    };
    assert_eq!(
        NativeCalloutMatchingDisposition::try_from_resolution(
            3,
            CalloutTargetClass::Citation,
            &resolution,
            NativeCalloutTargetInventoryStatus::SourceNotEstablished,
        ),
        Err(NativeCalloutInputError::FuzzyTargetClaimed { index: 3 })
    );
}

#[test]
fn equation_positive_match_is_refused_by_name() {
    let resolution = NativeCalloutResolution::Targeted {
        ordinal: 4,
        label_key: "4".to_owned(),
    };
    assert_eq!(
        NativeCalloutMatchingDisposition::try_from_resolution(
            1,
            CalloutTargetClass::Equation,
            &resolution,
            NativeCalloutTargetInventoryStatus::SourceNotEstablished,
        ),
        Err(NativeCalloutInputError::EquationPositiveMatch { index: 1 })
    );
    // A proven inventory is the only state in which a positive match survives.
    assert_eq!(
        NativeCalloutMatchingDisposition::try_from_resolution(
            1,
            CalloutTargetClass::Equation,
            &resolution,
            NativeCalloutTargetInventoryStatus::Established,
        ),
        Ok(NativeCalloutMatchingDisposition::Matched { ordinal: 4 })
    );
}

#[test]
fn cross_boundary_marker_is_refused_by_name() {
    let marker = CalloutMarkerInput {
        target_class: CalloutTargetClass::Citation,
        tokens: vec![
            callout_token("[", 0, 0),
            callout_token("12", 0, 1),
            callout_token("]", 1, 2),
        ],
    };
    assert_eq!(
        validate_native_callout_marker(0, &marker),
        Err(NativeCalloutInputError::CrossBoundaryMarker { index: 0 })
    );
    // The same tokens on one line are one marker.
    let contiguous = CalloutMarkerInput {
        tokens: vec![
            callout_token("[", 0, 0),
            callout_token("12", 0, 1),
            callout_token("]", 0, 2),
        ],
        ..marker
    };
    assert_eq!(validate_native_callout_marker(0, &contiguous), Ok(()));
}

#[test]
fn empty_marker_is_refused_by_name() {
    let marker = CalloutMarkerInput {
        target_class: CalloutTargetClass::Citation,
        tokens: Vec::new(),
    };
    assert_eq!(
        validate_native_callout_marker(0, &marker),
        Err(NativeCalloutInputError::EmptyMarker { index: 0 })
    );
}

#[test]
fn outcome_is_redaction_safe() {
    let (document, lattice, lexemes, segmentation) = marker_plane("redaction-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let encoded = serde_json::to_string(&outcome).unwrap();
    for raw in [
        "[12]",
        "Fig.",
        "Table",
        "Eq.",
        "[Smith2019]",
        "alpha",
        "beta",
    ] {
        assert!(
            !encoded.contains(raw),
            "the outcome must not carry marker text `{raw}`: {encoded}"
        );
    }
    assert!(
        !encoded.contains("\"text\""),
        "the outcome must not carry a text field"
    );
}

#[test]
fn denominators_partition_the_observations_and_are_checksum_bound() {
    let (document, lattice, lexemes, segmentation) = marker_plane("partition-doc");
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    assert_eq!(
        inventory.targeted.count + inventory.targetless.count,
        inventory.markers.len()
    );
    // No marker may resolve, so the targeted denominator is empty and the
    // targetless one carries every occurrence.
    assert_eq!(inventory.targeted.count, 0);
    assert_eq!(inventory.targetless.count, inventory.markers.len());
    assert_ne!(inventory.targeted.checksum, inventory.targetless.checksum);
}

#[test]
fn superscript_is_a_derived_signal_not_a_java_flag() {
    let (document, lattice, lexemes, segmentation) = plane(
        "superscript-doc",
        vec![vec![
            PWord::word("alpha"),
            PWord::small("12"),
            PWord::word("beta"),
        ]],
    );
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    assert_eq!(inventory.markers.len(), 1);
    assert_eq!(
        inventory.markers[0].marker_type,
        CalloutMarkerType::SuperscriptNumber
    );
    assert!(inventory.markers[0].superscript);
}

#[test]
fn a_bare_number_before_a_group_is_admitted_in_document_order() {
    // Extraction emits closed groups before bare numbers so a group's own tokens
    // shield the numbers inside it, but a bare number that precedes a group on
    // the same line must still be admitted in document order: the inventory
    // refuses a non-monotonic stream by name (`NonMonotonic`). Without the
    // ordering pass in `extract_native_callout_markers` this plane does not
    // build at all.
    let (document, lattice, lexemes, segmentation) = plane(
        "ordering-doc",
        vec![vec![
            PWord::word("alpha"),
            PWord::small("3"),
            PWord::word("[12]"),
        ]],
    );
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let inventory = outcome.inventory.as_ref().unwrap();
    assert_eq!(inventory.markers.len(), 2);
    assert!(
        inventory
            .markers
            .windows(2)
            .all(|pair| pair[0].position <= pair[1].position),
        "markers must be admitted in document order: {:?}",
        inventory
            .markers
            .iter()
            .map(|marker| marker.position)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        inventory.markers[0].marker_type,
        CalloutMarkerType::SuperscriptNumber,
        "the superscript bare number precedes the bracket group"
    );
}
