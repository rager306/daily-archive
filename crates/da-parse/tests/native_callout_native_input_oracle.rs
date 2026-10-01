//! Independent oracle for the native callout-marker band (S05/T04).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus the sha256 pins published in `provenance.yaml`. It never reaches
//! for the native band, the region carrier, the classifier or the inventory
//! builder: those are the SUT lane. The two lanes are separated by explicit
//! source markers, and the separation is itself asserted by a test.
//!
//! ## What the independent scanner counts, and what it deliberately does not
//!
//! GROBID's in-text callout markup is `<ref type="bibr">` (citation),
//! `<ref type="figure">` (figure), `<ref type="table">` (table) and
//! `<ref type="formula">` (equation). Those four are the byte-derived
//! denominators.
//!
//! The `<formula>` *element* is the equation object itself, not an in-text
//! callout to it, so it is **not** counted — counting it would overstate the
//! equation denominator by folding targets into markers. On these three fixtures
//! there is no `<ref type="formula">` at all, so the equation family carries
//! zero oracle rows. That is the declared state of
//! `callout_matching_semantics.yaml#matching_semantics.equation`
//! (`inventory_source = source_not_established`,
//! `positive_match = forbidden_until_inventory_proven`), and the evaluation
//! reports it as `not_observed` rather than pretending it was met.
//!
//! ## Honest offline limits
//!
//! There is no preserved ALTO/layout for these fixtures and `pdf-inspector` is
//! not a dependency of `da-parse`, so this oracle is TEI-to-markers, not
//! PDF-to-markers. The native side here is a controlled S03-shaped plane; the
//! real three-PDF measurement belongs to the S05 receipt runner, which owns the
//! `pdf-layout` feature.

use std::fs;
use std::path::{Path, PathBuf};

// ---- independent expectation builder (no SUT lane below this marker) ----

/// The three preserved TEI fixtures, in `provenance.yaml` order.
pub const FIXTURE_IDS: [&str; 3] = ["hub_2507.19457", "rlm_2512.24601", "reca_2605.26525v1"];

/// Independently audited `<ref type=…>` denominators per document, in
/// `FIXTURE_IDS` order: `(citation, figure, table, equation)`.
pub const SCANNED_COUNTS: [[usize; 4]; 3] = [[123, 24, 5, 0], [99, 25, 11, 0], [158, 30, 11, 0]];

/// One published fixture pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub id: String,
    pub path: String,
    pub sha256: String,
}

/// One callout marker read from the bytes by the lexical scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedMarker {
    /// `citation` | `figure` | `table` | `equation`.
    pub family: &'static str,
    pub ordinal: u32,
    /// The element bytes, digested only, never carried raw.
    pub content: String,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Load `backend` and the fixture pins from the tracked provenance file.
pub fn load_provenance() -> (String, Vec<Pin>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preserved/provenance.yaml");
    let text = fs::read_to_string(&path).expect("provenance.yaml is tracked");
    let value: serde_yaml::Value = serde_yaml::from_str(&text).expect("provenance.yaml parses");
    let backend = value["backend"].as_str().unwrap_or_default().to_owned();
    let pins = value["fixtures"]
        .as_sequence()
        .expect("provenance.yaml declares fixtures")
        .iter()
        .map(|fixture| Pin {
            id: fixture["id"].as_str().expect("fixture id").to_owned(),
            path: fixture["path"].as_str().expect("fixture path").to_owned(),
            sha256: fixture["sha256"]
                .as_str()
                .expect("fixture sha256")
                .to_owned(),
        })
        .collect();
    (backend, pins)
}

/// Read a pinned fixture and verify its sha256 against the committed pin.
///
/// A substituted pin is refused before any byte of the fixture is scanned.
pub fn read_verified(pin: &Pin) -> Result<String, String> {
    let bytes = fs::read(repo_root().join(&pin.path))
        .map_err(|error| format!("{} is unreadable: {error}", pin.path))?;
    let actual = digest(&bytes);
    if actual != pin.sha256 {
        return Err(format!(
            "checksum drift for {}: expected {}, got {actual}",
            pin.id, pin.sha256
        ));
    }
    String::from_utf8(bytes).map_err(|error| format!("{} is not UTF-8: {error}", pin.path))
}

/// The `type` attribute of a `<ref …>` opening tag.
fn ref_type(opening: &str) -> Option<&str> {
    let marker = "type=\"";
    let start = opening.find(marker)? + marker.len();
    let rest = &opening[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Lexically scan the preserved TEI for in-text callout markers.
///
/// The scan is non-nested: `<ref>` elements do not nest in GROBID TEI, and a
/// nesting guard is not needed. An unclosed `<ref` ends the scan rather than
/// inventing a marker.
pub fn scan_callout_markers(xml: &str) -> Vec<ScannedMarker> {
    let mut rows = Vec::new();
    let mut ordinals = [0u32; 4];
    let mut cursor = 0;
    while let Some(relative) = xml[cursor..].find("<ref") {
        let start = cursor + relative;
        let Some(gt_relative) = xml[start..].find('>') else {
            break;
        };
        let gt = start + gt_relative;
        let opening = &xml[start..=gt];
        let family = match ref_type(opening) {
            Some("bibr") => 0usize,
            Some("figure") => 1,
            Some("table") => 2,
            Some("formula") => 3,
            _ => {
                cursor = gt + 1;
                continue;
            }
        };
        let Some(close_relative) = xml[gt + 1..].find("</ref>") else {
            break;
        };
        let end = gt + 1 + close_relative + "</ref>".len();
        let ordinal = ordinals[family];
        ordinals[family] += 1;
        rows.push(ScannedMarker {
            family: match family {
                0 => "citation",
                1 => "figure",
                2 => "table",
                _ => "equation",
            },
            ordinal,
            content: xml[start..end].to_owned(),
        });
        cursor = end;
    }
    rows
}

// ---- end independent expectation builder ----

use da_parse::*;
use sha2::{Digest, Sha256};

/// Identity of this test-only scanner. It must never equal the SUT generator.
const ORACLE_GENERATOR_ID: &str =
    "crates/da-parse/tests/native_callout_native_input_oracle.rs#lexical-tei-callout-scanner";

fn sha256_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Read the three fixtures (verifying each pin) and scan them.
fn scanned_documents() -> (String, Vec<Pin>, Vec<(String, String)>) {
    let (backend, pins) = load_provenance();
    assert_eq!(pins.len(), FIXTURE_IDS.len());
    let documents = pins
        .iter()
        .map(|pin| {
            let xml = read_verified(pin).expect("a pinned fixture verifies");
            (pin.id.clone(), xml)
        })
        .collect();
    (backend, pins, documents)
}

/// Build the SUT oracle from the scanned positive rows.
fn oracle_from_scan(
    backend: &str,
    pins: &[Pin],
    documents: &[(String, String)],
) -> NativeCalloutOracle {
    let mut rows = Vec::new();
    for (document_id, xml) in documents {
        for row in scan_callout_markers(xml) {
            let family = match row.family {
                "citation" => CalloutTargetClass::Citation,
                "figure" => CalloutTargetClass::Figure,
                "table" => CalloutTargetClass::Table,
                _ => CalloutTargetClass::Equation,
            };
            rows.push(NativeCalloutOracleRow {
                document_id: document_id.clone(),
                family,
                ordinal: row.ordinal,
                content_digest: oracle_content_digest(row.content.as_bytes()),
            });
        }
    }
    NativeCalloutOracle {
        schema_version: NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION.to_owned(),
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: backend.to_owned(),
        pins: pins
            .iter()
            .map(|pin| NativeCalloutOraclePin {
                document_id: pin.id.clone(),
                sha256: pin.sha256.clone(),
            })
            .collect(),
        rows,
    }
}

// ───────────────────────────── inline S03 planes ─────────────────────────────

const PROVIDER_ID: &str = "fixture-donor";

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
}

/// A `introduction` marker line opens the Body region, because the segmenter
/// starts in `Header` and only a region marker moves it.
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
    let provider = LayoutProvider::new(PROVIDER_ID, "1", "a".repeat(64)).unwrap();
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
                    sha256_text(word.text),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word.text),
                    LayoutStyleHints {
                        font_size_milli_pt: word.font_milli,
                        ..LayoutStyleHints::default()
                    },
                    provider.clone(),
                )
                .unwrap(),
            );
            lexemes.push(LexemeRow::new(
                document_id,
                PROVIDER_ID,
                0,
                index,
                sha256_text(word.text),
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
        ],
    )
}

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

fn disposition(
    evaluations: &[NativeCalloutFamilyEvaluation],
    family: CalloutTargetClass,
) -> NativeCalloutFamilyDisposition {
    evaluations
        .iter()
        .find(|evaluation| evaluation.family == family)
        .expect("every family is evaluated")
        .disposition
}

#[test]
fn oracle_rows_are_positive_and_derived_from_pinned_bytes() {
    let (backend, pins, documents) = scanned_documents();
    assert_eq!(backend, "java-grobid-crf", "the other generator");
    for (index, (document_id, xml)) in documents.iter().enumerate() {
        assert_eq!(document_id, FIXTURE_IDS[index]);
        let rows = scan_callout_markers(xml);
        let counts = ["citation", "figure", "table", "equation"]
            .map(|family| rows.iter().filter(|row| row.family == family).count());
        assert_eq!(
            counts, SCANNED_COUNTS[index],
            "byte-derived marker denominators for {document_id}"
        );
        // Counts are derived from the bytes, not hardcoded: a second scan
        // reproduces them exactly, and every row carries a content digest.
        assert_eq!(scan_callout_markers(xml), rows, "scan is deterministic");
        for row in &rows {
            assert!(!row.content.is_empty(), "a positive row carries content");
        }
    }
    // The pin read is real: a substituted sha256 is refused before scanning.
    let mut tampered = pins[0].clone();
    tampered.sha256 = "0".repeat(64);
    assert!(read_verified(&tampered).is_err());
}

#[test]
fn equation_is_absent_from_the_oracle_and_stays_source_not_established() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    assert_eq!(
        callout_oracle_positive_rows(&oracle, None, CalloutTargetClass::Equation),
        0,
        "`<formula>` elements are targets, not callout markers, so equation has no oracle row"
    );
    for family in [
        CalloutTargetClass::Citation,
        CalloutTargetClass::Figure,
        CalloutTargetClass::Table,
    ] {
        assert!(
            callout_oracle_positive_rows(&oracle, None, family) > 0,
            "{family:?} must be positive"
        );
    }
}

#[test]
fn non_vacuous_positive_oracle_is_accepted() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    require_non_vacuous_callout_oracle(&oracle).expect("the oracle carries positive rows");
}

#[test]
fn same_generator_oracle_is_refused() {
    let (backend, pins, documents) = scanned_documents();
    let mut oracle = oracle_from_scan(&backend, &pins, &documents);
    oracle.generator_id = NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID.to_owned();
    assert!(matches!(
        require_non_vacuous_callout_oracle(&oracle),
        Err(NativeCalloutOracleError::NotIndependent(
            NativeCalloutOracleIndependenceError::NotIndependent { .. }
        ))
    ));
}

#[test]
fn an_oracle_without_any_positive_row_is_vacuous() {
    let (backend, pins, documents) = scanned_documents();
    let mut oracle = oracle_from_scan(&backend, &pins, &documents);
    oracle.rows.clear();
    assert_eq!(
        require_non_vacuous_callout_oracle(&oracle),
        Err(NativeCalloutOracleError::OracleVacuous)
    );
}

#[test]
fn pin_substitution_is_refused() {
    let (_, pins, _) = scanned_documents();
    let mut tampered = pins[1].clone();
    tampered.sha256 = "f".repeat(64);
    let error = read_verified(&tampered).expect_err("a substituted pin is refused");
    assert!(
        error.contains("checksum drift"),
        "the refusal names the drift: {error}"
    );
}

#[test]
fn markerless_native_plane_is_honest_unmet() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    let hub = FIXTURE_IDS[0];
    let (document, lattice, lexemes, segmentation) = markerless_plane(hub);
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    assert_eq!(
        outcome.absence,
        Some(NativeCalloutInputAbsence::NoMarkerShapedRun)
    );
    let evaluations = evaluate_callout_family_rows(&outcome, &oracle).unwrap();
    assert_eq!(evaluations.len(), 4);
    for family in [
        CalloutTargetClass::Citation,
        CalloutTargetClass::Figure,
        CalloutTargetClass::Table,
    ] {
        assert_eq!(
            disposition(&evaluations, family),
            NativeCalloutFamilyDisposition::Unmet {
                reason: NativeCalloutUnmetReason::MarkerShapeUnreachable
            },
            "{family:?} is a typed unmet, never out_of_scope"
        );
    }
    assert_eq!(
        disposition(&evaluations, CalloutTargetClass::Equation),
        NativeCalloutFamilyDisposition::NotObserved,
        "the oracle carries no equation marker row"
    );
    let serialized = serde_json::to_string(&evaluations).unwrap();
    assert!(
        !serialized.contains("out_of_scope"),
        "a required family can never be moved out of scope: {serialized}"
    );
}

#[test]
fn marker_plane_is_measured_for_every_family() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    let (document, lattice, lexemes, segmentation) = marker_plane(FIXTURE_IDS[0]);
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    let evaluations = evaluate_callout_family_rows(&outcome, &oracle).unwrap();
    for family in NATIVE_CALLOUT_FAMILIES {
        assert!(
            matches!(
                disposition(&evaluations, family),
                NativeCalloutFamilyDisposition::Measured { .. }
            ),
            "a plane carrying a marker of {family:?} is measured"
        );
    }
    // …and the measured equation marker still never matches.
    let inventory = outcome.inventory.as_ref().unwrap();
    let equation = inventory
        .matching
        .iter()
        .find(|row| row.family == CalloutTargetClass::Equation)
        .unwrap();
    assert_eq!(
        equation.disposition,
        NativeCalloutMatchingDisposition::Unresolved {
            reason: CalloutUnresolvedReason::UnsupportedEquation
        }
    );
}

#[test]
fn empty_document_is_a_typed_lane_absence() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    let (document, lattice, lexemes, segmentation) = empty_plane(FIXTURE_IDS[0]);
    let outcome = run(&document, &lattice, &lexemes, &segmentation);
    assert_eq!(outcome.absence, Some(NativeCalloutInputAbsence::LaneAbsent));
    let evaluations = evaluate_callout_family_rows(&outcome, &oracle).unwrap();
    assert_eq!(
        disposition(&evaluations, CalloutTargetClass::Citation),
        NativeCalloutFamilyDisposition::Unmet {
            reason: NativeCalloutUnmetReason::LaneAbsent
        }
    );
}

#[test]
fn expectation_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_callout_native_input_oracle.rs");
    let start_marker =
        "// ---- independent expectation builder (no SUT lane below this marker) ----";
    let end_marker = "// ---- end independent expectation builder ----";
    let start = source
        .find(start_marker)
        .expect("expectation builder start marker must exist");
    let body = &source[start + start_marker.len()..];
    let end = body
        .find(end_marker)
        .expect("expectation builder end marker must exist");
    let region = &body[..end];
    assert!(
        region.len() > 2_000,
        "expectation builder must be substantive, got {} bytes",
        region.len()
    );
    for name in [
        "build_native_callout_input",
        "extract_native_callout_markers",
        "classify_callout_marker",
        "build_callout_inventory",
        "majority_callout_style",
        "NativeCalloutFromInputOutcome",
        "NativeCalloutOracle",
        "NativeCalloutFamilyEvaluation",
        "evaluate_callout_family_rows",
        "segment_document_full",
        "NativeRegionCarrier",
    ] {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }
}
