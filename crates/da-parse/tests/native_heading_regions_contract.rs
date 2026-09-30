//! Contract tests for the additive heading-aware region assignment (S04/T04 repair).
//!
//! The S03 region walk promotes a region when a *single token* equals a marker
//! word, over a lattice that collapses each page into one block and one line.
//! Measured on the declared corpus that walk is both too strict (a heading
//! carried inside a multi-word span never matches) and too loose (a standalone
//! marker word anywhere, e.g. `Methods` inside a reference, flips the rest of
//! the document). These tests pin the heading-aware replacement: geometry-derived
//! lines, closed-vocabulary evidence levels, a max-words gate, typed absences
//! for regions never entered, and the end-to-end contract that the T02/T03
//! native bands accept projected words.

use da_parse::*;
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
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

fn document_and_projection(
    document_id: &str,
    spans: &[Span],
) -> (LayoutDocument, LayoutLattice, NativeWordProjection) {
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
    (document, lattice, projection)
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

/// A twenty-word line that *leads* with a marker word. It is bold, so only the
/// max-words gate can stop it from opening the body region.
const LONG_BOLD_LINE: &str = "Methods for evaluation are described in considerable detail here and elsewhere in this work and the next one to follow";

fn abstract_fixture() -> Vec<Span> {
    vec![
        Span {
            text: "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("Jane Doe, John Smith", 30.0),
        Span {
            text: LONG_BOLD_LINE,
            y_min: 50.0,
            style: style(true, BODY_FONT),
        },
        heading("Abstract", 70.0),
        body("We study something interesting", 82.0),
        heading("1 Introduction", 100.0),
        body("Body text continues here", 112.0),
        heading("References", 140.0),
        body("[1] Smith 2020. Journal of Methods", 152.0),
    ]
}

#[test]
fn lines_come_from_geometry_and_the_body_opens_at_the_abstract() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .expect("the fixture assigns");

    assert_eq!(outcome.lines.len(), 9, "each span sits on its own line");
    assert_eq!(outcome.body_font_size_milli_pt, Some(BODY_FONT));

    let header = outcome
        .evidence(FullSemanticRegion::Header)
        .expect("header");
    assert_eq!(header.matched_by, HeadingMatch::PreambleDefault);
    assert_eq!(header.line_ordinal, 0);

    let body_evidence = outcome.evidence(FullSemanticRegion::Body).expect("body");
    assert_eq!(body_evidence.matched_by, HeadingMatch::NormalizedLine);
    assert_eq!(body_evidence.region, FullSemanticRegion::Body);

    let references = outcome
        .evidence(FullSemanticRegion::References)
        .expect("references");
    assert_eq!(references.matched_by, HeadingMatch::NormalizedLine);

    // The title, the author line and the long bold line are the preamble.
    for assignment in outcome
        .assignments
        .iter()
        .filter(|entry| entry.source_ordinal <= 2)
    {
        assert_eq!(assignment.region, FullSemanticRegion::Header);
    }
    // The abstract introduces the body; the numbered introduction stays body.
    for assignment in outcome
        .assignments
        .iter()
        .filter(|entry| (3..=6).contains(&entry.source_ordinal))
    {
        assert_eq!(assignment.region, FullSemanticRegion::Body);
    }
    // A reference line that merely contains the word `Methods` does not reopen
    // the body region.
    for assignment in outcome
        .assignments
        .iter()
        .filter(|entry| entry.source_ordinal >= 7)
    {
        assert_eq!(assignment.region, FullSemanticRegion::References);
    }
}

#[test]
fn the_max_words_gate_stops_a_long_bold_line_from_opening_the_body() {
    let spans = vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        Span {
            text: LONG_BOLD_LINE,
            y_min: 40.0,
            style: style(true, BODY_FONT),
        },
        heading("Abstract", 70.0),
    ];
    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    let long_line = outcome
        .lines
        .iter()
        .find(|line| line.word_count > 12)
        .expect("the long line is derived");
    assert_eq!(long_line.bbox.y_min, 40.0);
    let body_evidence = outcome.evidence(FullSemanticRegion::Body).expect("body");
    assert_eq!(
        body_evidence.matched_by,
        HeadingMatch::NormalizedLine,
        "the body opens at the abstract, not at the long bold line"
    );
    assert_eq!(body_evidence.line_ordinal, 2);
}

#[test]
fn a_short_prominent_heading_outside_the_vocabulary_opens_the_body() {
    let spans = vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("An Author", 30.0),
        heading("Related Work", 60.0),
        body("Prior art is discussed at length", 72.0),
        body("Another paragraph of body text follows", 84.0),
        heading("References", 100.0),
    ];
    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    // Two body-font spans outvote the two heading-font lines, so the document's
    // measured body size is unambiguous and the heading is measurably larger.
    assert_eq!(outcome.body_font_size_milli_pt, Some(BODY_FONT));
    let body_evidence = outcome.evidence(FullSemanticRegion::Body).expect("body");
    assert_eq!(
        body_evidence.matched_by,
        HeadingMatch::ProminentSectionHeading
    );
    assert!(body_evidence.font_above_body_size);
    assert!(body_evidence.bold);
    for assignment in outcome
        .assignments
        .iter()
        .filter(|entry| (3..=4).contains(&entry.source_ordinal))
    {
        assert_eq!(assignment.region, FullSemanticRegion::Body);
    }
    assert_eq!(
        outcome
            .assignments
            .iter()
            .filter(|entry| entry.source_ordinal == 5)
            .map(|entry| entry.region)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([FullSemanticRegion::References])
    );
}

#[test]
fn regions_never_entered_are_typed_absences() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    for region in [
        FullSemanticRegion::Annex,
        FullSemanticRegion::Acknowledgement,
        FullSemanticRegion::Toc,
        FullSemanticRegion::Funding,
        FullSemanticRegion::Availability,
        FullSemanticRegion::PageFurniture,
    ] {
        assert_eq!(
            outcome.absence(region),
            Some(&NativeHeadingAbsence::RegionNeverEntered { region }),
            "{region:?} must be a typed absence, not an implied default"
        );
        assert!(outcome.evidence(region).is_none());
    }
    assert!(outcome.evidence(FullSemanticRegion::Header).is_some());
    assert_eq!(HEADING_REGION_ORDER.len(), 9);
}

#[test]
fn every_s03_marker_spelling_still_opens_its_region() {
    let markers: [&'static str; 13] = [
        "introduction",
        "methods",
        "results",
        "discussion",
        "conclusion",
        "references",
        "bibliography",
        "acknowledgements",
        "appendix",
        "funding",
        "data availability",
        "contents",
        "abstract",
    ];
    let mut spans = vec![Span {
        text: "A Title",
        y_min: 10.0,
        style: style(true, TITLE_FONT),
    }];
    for (index, marker) in markers.iter().enumerate() {
        spans.push(heading(marker, 40.0 + index as f64 * 20.0));
    }

    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    for region in [
        FullSemanticRegion::Body,
        FullSemanticRegion::References,
        FullSemanticRegion::Acknowledgement,
        FullSemanticRegion::Annex,
        FullSemanticRegion::Funding,
        FullSemanticRegion::Availability,
        FullSemanticRegion::Toc,
    ] {
        let evidence = outcome
            .evidence(region)
            .unwrap_or_else(|| panic!("{region:?} was never opened by its marker"));
        assert_eq!(evidence.matched_by, HeadingMatch::NormalizedLine);
    }
    assert!(outcome.absence(FullSemanticRegion::PageFurniture).is_some());
}

#[test]
fn an_ordinal_prefix_is_stripped_before_the_vocabulary_is_consulted() {
    let spans = vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("An Author", 30.0),
        heading("3.1 Discussion", 60.0),
    ];
    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    let body_evidence = outcome.evidence(FullSemanticRegion::Body).expect("body");
    assert_eq!(body_evidence.matched_by, HeadingMatch::OrdinalStrippedLine);
}

#[test]
fn assignment_is_deterministic_and_covers_every_projected_word() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let first = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    let second = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    assert_eq!(first.digest().unwrap(), second.digest().unwrap());

    let assigned: std::collections::BTreeSet<(u32, u32, u32)> = first
        .assignments
        .iter()
        .map(|entry| (entry.page_ordinal, entry.source_ordinal, entry.word_ordinal))
        .collect();
    assert_eq!(assigned, native_word_keys(&projection));
    assert_eq!(assigned.len(), projection.identity.word_count as usize);
    assert_eq!(first.schema_version, NATIVE_HEADING_REGIONS_SCHEMA_VERSION);
    assert!(first.digest().unwrap().starts_with("sha256:"));
}

#[test]
fn a_heading_sharing_its_line_with_an_entry_is_read_from_the_leading_span() {
    // The positioned donor emits one span per show operator, so a heading and
    // the entry that follows it can share a baseline and therefore group into a
    // single visual line. The whole line then never equals the marker, but the
    // line *opens* with a span that does.
    let spans = vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("An Author", 30.0),
        Span {
            text: "References",
            y_min: 100.0,
            style: style(true, HEADING_FONT),
        },
        body("[1] Smith 2020. Some Journal of Methods", 100.0),
    ];
    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    let references = outcome
        .evidence(FullSemanticRegion::References)
        .expect("the references heading is read from the leading span");
    assert_eq!(references.matched_by, HeadingMatch::LeadingSpanLine);
    for assignment in outcome
        .assignments
        .iter()
        .filter(|entry| entry.source_ordinal >= 2)
    {
        assert_eq!(assignment.region, FullSemanticRegion::References);
    }
}

#[test]
fn a_body_font_leading_marker_word_does_not_promote_a_region() {
    // The inverse defect: a marker word at the start of an ordinary line must
    // not flip the region. A real heading is visually distinct, so the
    // leading-span rule demands the line look like a heading.
    let spans = vec![
        Span {
            text: "A Title",
            y_min: 10.0,
            style: style(true, TITLE_FONT),
        },
        body("An Author", 30.0),
        body("Methods for evaluation are described", 60.0),
        body("and the results are discussed", 60.0),
    ];
    let (document, lattice, projection) = document_and_projection("fixture-doc", &spans);
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    assert!(outcome.evidence(FullSemanticRegion::Body).is_none());
    assert!(outcome.absence(FullSemanticRegion::Body).is_some());
    for assignment in outcome.assignments.iter() {
        assert_eq!(assignment.region, FullSemanticRegion::Header);
    }
}

#[test]
fn redaction_keeps_line_text_out_of_debug_and_serde() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();

    let line = &outcome.lines[0];
    let debug = format!("{line:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("GEPA"));

    let serialized = serde_json::to_string(&outcome).unwrap();
    for secret in ["GEPA", "Reflective", "Jane", "considerable", "Smith"] {
        assert!(
            !serialized.contains(secret),
            "the serialized outcome must not carry line text: {secret}"
        );
    }
    assert!(serialized.contains(&line.text_digest));
    assert!(!serialized.contains("pub text"));
}

#[test]
fn invalid_config_and_foreign_identity_are_refused() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());

    let invalid = HeadingRegionConfig {
        heading_max_words: 0,
        ..HeadingRegionConfig::default()
    };
    assert_eq!(
        assign_heading_regions(&document, &lattice, &projection, invalid).unwrap_err(),
        NativeHeadingRegionError::InvalidConfig
    );

    let (other_document, other_lattice, _) =
        document_and_projection("other-doc", &abstract_fixture());
    assert_eq!(
        assign_heading_regions(
            &other_document,
            &other_lattice,
            &projection,
            HeadingRegionConfig::default()
        )
        .unwrap_err(),
        NativeHeadingRegionError::IdentityMismatch("document_id")
    );
}

// ───────────────── end-to-end: the bands accept projected words ─────────────────

fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["doe".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

#[test]
fn projected_words_satisfy_the_native_band_contract() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    let carrier = build_native_word_region_carrier(
        &document,
        &lattice,
        &projection,
        &outcome,
        &[FullSemanticRegion::Header, FullSemanticRegion::References],
    )
    .expect("the word carrier builds");

    for region in [FullSemanticRegion::Header, FullSemanticRegion::References] {
        let run = carrier.run(region).expect("the fixture has both runs");
        assert!(!run.tokens.is_empty());
        for token in &run.tokens {
            assert!(
                !token.text().chars().any(char::is_whitespace),
                "no carrier token may carry internal whitespace: {:?}",
                token.text()
            );
        }
    }
    assert!(carrier.absence(FullSemanticRegion::Header).is_none());
    assert!(carrier.absence(FullSemanticRegion::References).is_none());

    let header = build_native_header_input(&carrier, &lexicon())
        .expect("the header band accepts projected words");
    assert!(!header.atoms.is_empty());
    let bibliography = build_native_bibliography_input(&carrier, &lexicon())
        .expect("the bibliography band accepts projected words");
    assert!(!bibliography.atoms.is_empty());
    assert!(!bibliography.citation_names_pooled_with_biblstruct);

    // The carrier is byte-stable across a rebuild.
    let rebuilt = build_native_word_region_carrier(
        &document,
        &lattice,
        &projection,
        &outcome,
        &[FullSemanticRegion::References, FullSemanticRegion::Header],
    )
    .unwrap();
    assert_eq!(carrier.digest().unwrap(), rebuilt.digest().unwrap());
}

#[test]
fn the_word_carrier_refuses_a_foreign_projection() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    let (other_document, other_lattice, other_projection) =
        document_and_projection("other-doc", &abstract_fixture());

    assert_eq!(
        build_native_word_region_carrier(
            &other_document,
            &other_lattice,
            &projection,
            &outcome,
            &[FullSemanticRegion::Header],
        )
        .unwrap_err(),
        NativeRegionError::WordProjectionMismatch("document_id")
    );
    assert_eq!(
        build_native_word_region_carrier(
            &document,
            &lattice,
            &other_projection,
            &outcome,
            &[FullSemanticRegion::Header],
        )
        .unwrap_err(),
        NativeRegionError::WordProjectionMismatch("document_id")
    );
    assert_eq!(
        build_native_word_region_carrier(&document, &lattice, &projection, &outcome, &[])
            .unwrap_err(),
        NativeRegionError::EmptyRegionRequest
    );
}

#[test]
fn the_word_carrier_reports_a_typed_absence_for_an_unentered_region() {
    let (document, lattice, projection) =
        document_and_projection("fixture-doc", &abstract_fixture());
    let outcome = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    let carrier = build_native_word_region_carrier(
        &document,
        &lattice,
        &projection,
        &outcome,
        &[FullSemanticRegion::Header, FullSemanticRegion::Funding],
    )
    .unwrap();

    assert!(
        carrier
            .absence(FullSemanticRegion::Funding)
            .is_some_and(|_| carrier.run(FullSemanticRegion::Funding).is_none())
    );
    assert!(matches!(
        build_native_header_input(
            &build_native_word_region_carrier(
                &document,
                &lattice,
                &projection,
                &outcome,
                &[FullSemanticRegion::References],
            )
            .unwrap(),
            &lexicon()
        )
        .unwrap_err(),
        NativeHeaderInputError::RegionNotRequested { .. }
    ));
}
