//! Contract tests for the additive native section / reading-order surface
//! (S05/T02).
//!
//! The surface derives a document's section tree and reading-order identity from
//! the S03 plane (`LayoutDocument` + `LayoutLattice` + `LexemeRow` +
//! `FullSegmentationOutcome`) joined with the heading-aware region walk and the
//! word-level region carrier. These tests pin the closed vocabularies, the
//! one-region-per-admitted-token invariant, boundary monotonicity, the typed
//! absences, the typed `segmenter_rejected_lattice` refusal for the `xy_cut`
//! lane, determinism, and the named negative mutations.
//!
//! The corpus measurement itself belongs to the gated runner of S05/T06: this
//! test drives the derivation over tracked inline planes only.

use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

fn style(bold: bool, font_size_milli_pt: u32) -> LayoutStyleHints {
    LayoutStyleHints {
        bold,
        italic: false,
        monospace: false,
        font_size_milli_pt: Some(font_size_milli_pt),
    }
}

const BODY_FONT: u32 = 10_500;
const HEADING_FONT: u32 = 12_000;
const TITLE_FONT: u32 = 14_000;

#[derive(Clone, Copy)]
struct Span {
    text: &'static str,
    y_min: f64,
    style: LayoutStyleHints,
}

fn heading(text: &'static str, y_min: f64) -> Span {
    Span {
        text,
        y_min,
        style: style(true, HEADING_FONT),
    }
}

fn body(text: &'static str, y_min: f64) -> Span {
    Span {
        text,
        y_min,
        style: style(false, BODY_FONT),
    }
}

/// The section fixture: a header, a numbered body heading, a numbered
/// sub-heading, and a references region. All spans are single tokens on page 0;
/// spans sharing a `y_min` form one visual line.
fn section_spans() -> Vec<Span> {
    vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("An Author", 30.0),
        heading("1 Introduction", 60.0),
        body("Body one", 80.0),
        heading("3.1 Discussion", 100.0),
        body("Body two", 120.0),
        heading("References", 140.0),
        body("[1] Smith", 160.0),
    ]
}

/// The mismatch fixture: S03's token-level marker opens Body on an inline
/// `introduction` word* (the known "too loose" walk), while the line-based
/// heading walk never enters Body. The derivation must then record
/// `NoHeadingEvidence` rather than invent a heading.
fn inline_marker_spans() -> Vec<Span> {
    vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("the", 30.0),
        body("introduction", 30.0),
        body("of", 30.0),
        body("methods", 30.0),
        heading("References", 60.0),
        body("[1] Smith", 80.0),
    ]
}

/// Mirror the positioned donor's lattice: one block, one line, all tokens.
fn lattice_for(document: &LayoutDocument) -> LayoutLattice {
    let pages = document
        .pages
        .iter()
        .map(|page| {
            let tokens: Vec<LatticeToken> = page
                .tokens
                .iter()
                .map(|token| LatticeToken {
                    source_ordinal: token.source_ordinal,
                    token_ordinal: token.token_ordinal,
                    bbox: token.bbox,
                    text_digest: token.text_digest.clone(),
                    source_span: token.source_span.clone(),
                    kind: token.kind,
                })
                .collect();
            let bbox = tokens
                .iter()
                .map(|token| token.bbox)
                .reduce(|left, right| {
                    LayoutBBox::new(
                        left.x_min.min(right.x_min),
                        left.y_min.min(right.y_min),
                        left.x_max.max(right.x_max),
                        left.y_max.max(right.y_max),
                    )
                    .unwrap()
                })
                .unwrap();
            LatticePage {
                page_ordinal: page.page_ordinal,
                java_page_number: page.page_ordinal + 1,
                width: page.width,
                height: page.height,
                rotation: page.rotation,
                crop_box: page.crop_box,
                blocks: vec![LatticeBlock {
                    source_ordinal: 0,
                    source_span: LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                    bbox,
                    source_order: 0,
                    derived_order: Some(0),
                    furniture: FurnitureEvidence::Absent,
                    lines: vec![LatticeLine {
                        source_ordinal: 0,
                        source_span: LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                        bbox,
                        newline_boundary_count: 1,
                        tokens,
                    }],
                }],
            }
        })
        .collect();
    LayoutLattice::new(
        document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        pages,
    )
    .unwrap()
}

struct Fixture {
    document: LayoutDocument,
    lattice: LayoutLattice,
    lexemes: Vec<LexemeRow>,
    projection: NativeWordProjection,
}

fn fixture(document_id: &str, spans: &[Span]) -> Fixture {
    let tokens: Vec<LayoutToken> = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let source = index as u32;
            LayoutToken::new(
                0,
                source,
                source,
                LayoutBBox::new(20.0, span.y_min, 280.0, span.y_min + 10.0).unwrap(),
                digest(span.text),
                LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                LayoutTokenKind::from_lexical_text(span.text),
                span.style,
                provider(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 800.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let lattice = lattice_for(&document);
    let lexemes: Vec<LexemeRow> = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            LexemeRow::new(
                document_id,
                "fixture-donor",
                0,
                index as u32,
                digest(span.text),
                span.text,
            )
        })
        .collect();
    let projection = project_native_words(&document, &lexemes).expect("the fixture projects");
    Fixture {
        document,
        lattice,
        lexemes,
        projection,
    }
}

fn heading_outcome(fixture: &Fixture) -> NativeHeadingRegionOutcome {
    assign_heading_regions(
        &fixture.document,
        &fixture.lattice,
        &fixture.projection,
        HeadingRegionConfig::default(),
    )
    .expect("the fixture heading walk succeeds")
}

fn word_carrier(fixture: &Fixture) -> NativeRegionCarrier {
    let outcome = heading_outcome(fixture);
    build_native_word_region_carrier(
        &fixture.document,
        &fixture.lattice,
        &fixture.projection,
        &outcome,
        &[
            FullSemanticRegion::Header,
            FullSemanticRegion::Body,
            FullSemanticRegion::References,
            FullSemanticRegion::Annex,
        ],
    )
    .expect("the fixture word carrier builds")
}

fn parent_carrier(fixture: &Fixture) -> NativeRegionCarrier {
    let segmentation = segment_document_full(
        &fixture.document,
        &fixture.lattice,
        &fixture.lexemes,
        SegmentationConfig::default(),
    )
    .expect("the fixture segments");
    build_native_region_carrier(
        &fixture.document,
        &fixture.lattice,
        &fixture.lexemes,
        &segmentation,
        &[
            FullSemanticRegion::Header,
            FullSemanticRegion::Body,
            FullSemanticRegion::References,
        ],
    )
    .expect("the fixture parent carrier builds")
}

fn derive(fixture: &Fixture, carrier: &NativeRegionCarrier) -> NativeSectionOutcome {
    derive_native_section_reading_order(
        &fixture.lattice,
        carrier,
        &heading_outcome(fixture),
        NativeSectionReadingOrderConfig::default(),
    )
    .expect("the fixture derives")
}

fn location_key(location: &FullLocation) -> (u32, u32, u32, u32) {
    (
        location.page_ordinal,
        location.block_ordinal,
        location.line_ordinal,
        location.token_ordinal,
    )
}

// ───────────────────────────── happy path ─────────────────────────────

#[test]
fn regions_and_sections_are_the_closed_declared_vocabularies() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));

    assert_eq!(
        outcome.section_parent_regions,
        SECTION_PARENT_REGIONS.to_vec()
    );
    assert_eq!(outcome.section_parent_regions.len(), 2);
    for section in &outcome.sections {
        assert!(
            SECTION_PARENT_REGIONS.contains(&section.parent_region),
            "a section may only live in a section-bearing region"
        );
    }
    let regions = native_section_regions();
    assert_eq!(regions.len(), NATIVE_SECTION_REGION_COUNT);
    for region in &outcome.reading_order.region_sequence {
        assert!(regions.contains(region), "unknown region {region:?}");
    }
    assert_eq!(outcome.reading_order.region_sequence.len(), 3);
}

#[test]
fn every_admitted_token_carries_exactly_one_region_and_order_is_preserved() {
    let fixture = fixture("fixture-doc", &section_spans());
    let carrier = word_carrier(&fixture);
    let expected: u32 = carrier.runs.iter().map(|run| run.tokens.len() as u32).sum();
    let outcome = derive(&fixture, &carrier);

    assert_eq!(outcome.reading_order.admitted_token_count, expected);
    let counted: u32 = outcome
        .reading_order
        .region_rows
        .iter()
        .map(|row| row.admitted_tokens)
        .sum();
    assert_eq!(counted, expected, "the rows partition the admitted tokens");

    let mut previous: Option<(u32, u32, u32, u32)> = None;
    for row in &outcome.reading_order.region_rows {
        if let Some(previous) = previous {
            assert!(
                previous <= location_key(&row.start),
                "region rows must follow admitted reading order"
            );
        }
        previous = Some(location_key(&row.start));
    }

    // One region per admitted token: no region appears twice in the rows.
    let mut regions: Vec<FullSemanticRegion> = outcome
        .reading_order
        .region_rows
        .iter()
        .map(|row| row.region)
        .collect();
    let unique = regions.len();
    regions.sort();
    regions.dedup();
    assert_eq!(regions.len(), unique, "a region must not admit two runs");
}

#[test]
fn section_boundaries_are_monotonic_and_well_formed() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));

    assert!(!outcome.sections.is_empty());
    let mut previous: Option<(u32, u32, u32, u32)> = None;
    for section in &outcome.sections {
        assert!(
            location_key(&section.start) <= location_key(&section.end),
            "a section start precedes or equals its end"
        );
        if let Some(previous) = previous {
            assert!(
                previous <= location_key(&section.start),
                "sections are published in reading order"
            );
        }
        previous = Some(location_key(&section.start));
    }
    // The numbered sub-heading nests under the numbered body heading.
    let sub = outcome
        .sections
        .iter()
        .find(|section| section.depth == 2)
        .expect("3.1 Discussion is depth 2");
    assert!(sub.parent_section_id.is_some());
    let parent = outcome
        .sections
        .iter()
        .find(|section| Some(section.section_id.clone()) == sub.parent_section_id)
        .expect("the parent exists");
    assert_eq!(parent.depth, 1);
    assert!(parent.order_ordinal < sub.order_ordinal);
}

#[test]
fn page_spans_partition_the_admitted_tokens_in_document_order() {
    let fixture = fixture("fixture-doc", &section_spans());
    let carrier = word_carrier(&fixture);
    let outcome = derive(&fixture, &carrier);

    let total: u32 = outcome
        .reading_order
        .page_spans
        .iter()
        .map(|span| span.admitted_tokens)
        .sum();
    assert_eq!(total, outcome.reading_order.admitted_token_count);
    for span in &outcome.reading_order.page_spans {
        assert!(location_key(&span.first) <= location_key(&span.last));
    }
}

#[test]
fn the_reading_order_identity_is_the_plane_identity() {
    let fixture = fixture("fixture-doc", &section_spans());
    let carrier = word_carrier(&fixture);
    let outcome = derive(&fixture, &carrier);
    assert_eq!(
        outcome.reading_order.admitted_order_digest,
        carrier.identity.order_digest
    );
    assert_eq!(outcome.document_id, carrier.identity.document_id);
    assert_eq!(
        outcome.lattice_fingerprint,
        carrier.identity.lattice_fingerprint
    );
}

#[test]
fn derivation_is_deterministic_and_byte_stable() {
    let fixture = fixture("fixture-doc", &section_spans());
    let first = derive(&fixture, &word_carrier(&fixture));
    let second = derive(&fixture, &word_carrier(&fixture));
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
}

// ───────────────────────────── typed absences ─────────────────────────────

#[test]
fn a_region_never_entered_is_a_typed_absence_not_an_empty_section() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));
    assert!(
        outcome
            .absences
            .contains(&NativeSectionAbsence::RegionNeverEntered {
                region: FullSemanticRegion::Annex,
            }),
        "annex was never entered and must say so"
    );
    assert!(
        !outcome
            .sections
            .iter()
            .any(|section| section.parent_region == FullSemanticRegion::Annex)
    );
}

#[test]
fn a_section_run_admitted_without_heading_evidence_is_a_typed_absence() {
    let fixture = fixture("fixture-doc", &inline_marker_spans());
    let outcome = derive(&fixture, &parent_carrier(&fixture));
    assert!(
        outcome
            .absences
            .contains(&NativeSectionAbsence::NoHeadingEvidence {
                region: FullSemanticRegion::Body,
            }),
        "an S03 body run the heading walk never entered must say NoHeadingEvidence"
    );
    let body: Vec<&NativeSectionRun> = outcome
        .sections
        .iter()
        .filter(|section| section.parent_region == FullSemanticRegion::Body)
        .collect();
    assert_eq!(body.len(), 1, "one unlabelled section covers the run");
    assert!(body[0].heading_evidence.is_none());
}

// ────────────────────── segmenter rejection / no silent skip ──────────────────────

#[test]
fn the_xy_cut_lane_is_a_typed_segmenter_rejection_never_counted() {
    assert_eq!(
        native_lane_disposition(SEGMENTER_REJECTED_LANE_IDS[0]),
        NativeLaneDisposition::SegmenterRejectedLattice
    );
    assert_eq!(
        native_lane_disposition(NATIVE_MEASURED_LANE_ID),
        NativeLaneDisposition::Measured
    );
    assert_eq!(
        native_lane_disposition(JAVA_AUTHORITY_LANE_ID),
        NativeLaneDisposition::JavaAuthority
    );
    assert_eq!(
        native_lane_disposition("unlisted-lane"),
        NativeLaneDisposition::UnknownLane
    );

    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));
    let oracle = oracle_rows_for(&outcome, NATIVE_MEASURED_LANE_ID);
    let verdicts =
        evaluate_native_section_rows(&outcome, SEGMENTER_REJECTED_LANE_IDS[0], &oracle, true)
            .expect("the evaluator runs");
    assert_eq!(verdicts.len(), NATIVE_SECTION_REGION_COUNT);
    for row in &verdicts {
        assert_eq!(
            row.verdict,
            NativeSectionRowVerdict::NotAssessable {
                reason: NativeSectionNotAssessable::LaneNotMeasuredBySegmenter,
            },
            "the xy_cut lane is never silently counted for {region:?}",
            region = row.region
        );
    }
}

// ───────────────────────────── oracle evaluation ─────────────────────────────

/// The native outcome's own rows transposed into the oracle shape: the
/// conformance input for the evaluator (the corpus measurement runs in S05/T06).
fn oracle_rows_for(outcome: &NativeSectionOutcome, lane_id: &str) -> Vec<NativeSectionOracleRow> {
    native_section_regions()
        .iter()
        .map(|region| {
            let run = outcome
                .reading_order
                .region_rows
                .iter()
                .find(|row| row.region == *region);
            NativeSectionOracleRow {
                document_id: outcome.document_id.clone(),
                lane_id: lane_id.to_owned(),
                region: *region,
                boundary: run.map(|run| NativeSectionOracleBoundary {
                    start: run.start.clone(),
                    end: run.end.clone(),
                }),
            }
        })
        .collect()
}

#[test]
fn a_conforming_native_outcome_is_an_agreement_and_mutations_are_typed() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));
    let oracle = oracle_rows_for(&outcome, NATIVE_MEASURED_LANE_ID);

    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &oracle, true)
        .expect("the evaluator runs");
    for row in &verdicts {
        assert_eq!(row.verdict, NativeSectionRowVerdict::Agreement);
    }

    // Boundary mutation on the body row.
    let mut displaced = oracle.clone();
    let body = displaced
        .iter_mut()
        .find(|row| row.region == FullSemanticRegion::Body)
        .expect("body row");
    if let Some(boundary) = body.boundary.as_mut() {
        boundary.end.page_ordinal += 1;
    }
    let verdicts =
        evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &displaced, true)
            .expect("the evaluator runs");
    let body_verdict = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Body)
        .expect("body row");
    assert!(matches!(
        body_verdict.verdict,
        NativeSectionRowVerdict::BoundaryDisplaced { .. }
    ));

    // Count mutation: the oracle claims a body run the native outcome has.
    let mut counted = oracle.clone();
    let annex = counted
        .iter_mut()
        .find(|row| row.region == FullSemanticRegion::Annex)
        .expect("annex row");
    annex.boundary = Some(NativeSectionOracleBoundary {
        start: FullLocation {
            page_ordinal: 1,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: 0,
        },
        end: FullLocation {
            page_ordinal: 1,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: 1,
        },
    });
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &counted, true)
        .expect("the evaluator runs");
    let annex_verdict = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Annex)
        .expect("annex row");
    assert_eq!(
        annex_verdict.verdict,
        NativeSectionRowVerdict::CountMismatch {
            oracle_runs: 1,
            native_runs: 0,
        }
    );

    // A missing oracle row is not an agreement.
    let mut missing = oracle.clone();
    missing.retain(|row| row.region != FullSemanticRegion::Funding);
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &missing, true)
        .expect("the evaluator runs");
    let funding = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Funding)
        .expect("funding row");
    assert_eq!(
        funding.verdict,
        NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::OracleRowMissing,
        }
    );
}

#[test]
fn a_non_independent_oracle_is_not_assessable_by_name() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));
    let oracle = oracle_rows_for(&outcome, NATIVE_MEASURED_LANE_ID);
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &oracle, false)
        .expect("the evaluator runs");
    for row in &verdicts {
        assert_eq!(
            row.verdict,
            NativeSectionRowVerdict::NotAssessable {
                reason: NativeSectionNotAssessable::OracleNotIndependent,
            }
        );
    }
    assert_eq!(
        require_independent_section_oracle(NATIVE_SECTION_SUT_GENERATOR_ID),
        Err(NativeSectionReadingOrderError::NotIndependent)
    );
}

// ────────────────────── structural separation from T01 ──────────────────────

#[test]
fn a_section_region_row_carries_no_character_level_field() {
    let fixture = fixture("fixture-doc", &section_spans());
    let outcome = derive(&fixture, &word_carrier(&fixture));
    let row = serde_json::to_value(&outcome.reading_order.region_rows[0]).unwrap();
    let object = row.as_object().expect("a region row is an object");
    for forbidden in [
        "characters",
        "character_counts",
        "denominator",
        "content_loss",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "the boundary/count section row must not carry {forbidden}"
        );
    }
    // The closed verdict vocabulary is the structural guard.
    let verdict = serde_json::to_value(NativeSectionRowVerdict::Agreement).unwrap();
    assert_eq!(verdict["verdict"], "agreement");
    assert_eq!(NATIVE_SECTION_ROW_VERDICT_VOCABULARY.len(), 4);
    assert!(!NATIVE_SECTION_ROW_VERDICT_VOCABULARY.contains(&"content_loss"));
    assert!(!NATIVE_SECTION_ROW_VERDICT_VOCABULARY.contains(&"granularity_only"));
}

// ───────────────────────────── negative mutations ─────────────────────────────

#[test]
fn a_foreign_heading_outcome_is_refused_by_name() {
    let native = fixture("fixture-doc", &section_spans());
    let other = fixture("other-doc", &section_spans());
    let carrier = word_carrier(&native);
    assert_eq!(
        derive_native_section_reading_order(
            &native.lattice,
            &carrier,
            &heading_outcome(&other),
            NativeSectionReadingOrderConfig::default(),
        )
        .unwrap_err(),
        NativeSectionReadingOrderError::IdentityMismatch("document_id")
    );
}

#[test]
fn a_foreign_lattice_is_refused_by_name() {
    let native = fixture("fixture-doc", &section_spans());
    let other = fixture("other-doc", &section_spans());
    let carrier = word_carrier(&native);
    assert_eq!(
        derive_native_section_reading_order(
            &other.lattice,
            &carrier,
            &heading_outcome(&native),
            NativeSectionReadingOrderConfig::default(),
        )
        .unwrap_err(),
        NativeSectionReadingOrderError::IdentityMismatch("lattice_document_id")
    );
}

#[test]
fn an_excessive_section_depth_is_refused_by_name() {
    let fixture = fixture("fixture-doc", &section_spans());
    let carrier = word_carrier(&fixture);
    assert_eq!(
        derive_native_section_reading_order(
            &fixture.lattice,
            &carrier,
            &heading_outcome(&fixture),
            NativeSectionReadingOrderConfig {
                max_sections: MAX_NATIVE_SECTION_RUNS,
                max_depth: 1,
            },
        )
        .unwrap_err(),
        NativeSectionReadingOrderError::SectionDepthExceeded { depth: 2 }
    );
}

#[test]
fn a_section_budget_of_zero_refuses_the_first_section_by_name() {
    let fixture = fixture("fixture-doc", &inline_marker_spans());
    let carrier = parent_carrier(&fixture);
    assert_eq!(
        derive_native_section_reading_order(
            &fixture.lattice,
            &carrier,
            &heading_outcome(&fixture),
            NativeSectionReadingOrderConfig {
                max_sections: 0,
                max_depth: MAX_NATIVE_SECTION_DEPTH,
            },
        )
        .unwrap_err(),
        NativeSectionReadingOrderError::TooManySections { count: 1 }
    );
}
