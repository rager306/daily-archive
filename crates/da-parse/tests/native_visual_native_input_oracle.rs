//! Independent oracle for the native visual-family band (S05/T03).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus the sha256 pins published in `provenance.yaml`. It never reaches
//! for the native band, the head detectors, or the plane builders: those are the
//! SUT lane, never the receipt. The two lanes are separated by explicit source
//! markers, and the separation is itself asserted by a test.
//!
//! Honest offline limits:
//!   * there is no preserved ALTO/layout for the three fixtures and
//!     `pdf-inspector` is not a dependency of `da-parse`, so this oracle is
//!     TEI-to-families, not PDF-to-families. The native side here is a controlled
//!     S03-shaped plane; the real three-PDF measurement belongs to the S05
//!     receipt runner, which owns the `pdf-layout` feature;
//!   * the committed digest-only oracle (`native_visual_oracle_source.json`) is
//!     all-absence, so evaluating against it is vacuous. This oracle is the
//!     non-vacuous replacement: it carries at least one positive row per family
//!     and per document, derived from the bytes, and refuses itself when a
//!     family is absent.

use std::fs;
use std::path::{Path, PathBuf};

// ---- independent expectation builder (no SUT lane below this marker) ----

/// The three preserved TEI fixtures, in `provenance.yaml` order.
pub const FIXTURE_IDS: [&str; 3] = ["hub_2507.19457", "rlm_2512.24601", "reca_2605.26525v1"];

/// Independently audited positive row denominators per document, in
/// `FIXTURE_IDS` order: `(figure, table, formula)`.
///
/// A `<figure type="table">` is a table (it carries the `<table>` grid); a plain
/// `<figure>` is a figure; a `<formula>` is a formula. `<table>` elements are
/// never counted separately — they are the body of their `type="table"` figure.
pub const POSITIVE_ROWS: [[usize; 3]; 3] = [[22, 3, 24], [29, 2, 2], [55, 3, 12]];

/// One published fixture pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub id: String,
    pub path: String,
    pub sha256: String,
}

/// One positive row read from the bytes by the lexical scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedRow {
    pub family: &'static str,
    pub ordinal: u32,
    pub page_ordinal: Option<u32>,
    pub coords: Option<String>,
    pub caption: Option<String>,
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

/// All un-nested `<tag ...>...</tag>` blocks, in document order.
fn element_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = xml[at..].find(&open) {
        let start = at + offset;
        let rest = &xml[start + open.len()..];
        if !(rest.starts_with('>') || rest.starts_with(|c: char| c.is_whitespace())) {
            at = start + open.len();
            continue;
        }
        let gt = start + xml[start..].find('>').expect("opening tag is closed");
        let body = &xml[gt + 1..];
        let body_end = body.find(&close).expect("element is closed");
        out.push(&xml[start..gt + 1 + body_end + close.len()]);
        at = gt + 1 + body_end + close.len();
    }
    out
}

/// The opening tag of a block, e.g. `<figure ...>`.
fn opening_tag<'a>(block: &'a str, tag: &str) -> &'a str {
    assert!(
        block.starts_with(&format!("<{tag}")),
        "block starts with its tag"
    );
    let gt = block.find('>').expect("opening tag is closed");
    &block[..=gt]
}

/// The trimmed inner text of the first `<tag>` element, when present.
fn inner(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    let body = &block[start + open.len()..];
    let end = body.find(&close)?;
    Some(body[..end].trim().to_owned())
}

/// The `coords` attribute of the first element that carries one.
fn coords_of(block: &str) -> Option<String> {
    let marker = "coords=\"";
    let start = block.find(marker)?;
    let body = &block[start + marker.len()..];
    let end = body.find('"')?;
    Some(body[..end].to_owned())
}

/// The zero-based page of a GROBID `page,x,y,width,height` coordinate string.
fn page_of(coords: &str) -> Option<u32> {
    let first = coords.split(',').next()?;
    let page: u32 = first.trim().parse().ok()?;
    Some(page.saturating_sub(1))
}

/// The content bytes of a figure/table: its `<graphic .../>` element when
/// present, else the whole block. Digested only, never carried raw.
fn figure_content(block: &str) -> String {
    match block.find("<graphic") {
        Some(start) => {
            let end = block[start..]
                .find('>')
                .map(|gt| start + gt + 1)
                .unwrap_or(block.len());
            block[start..end].to_owned()
        }
        None => block.to_owned(),
    }
}

/// A block bbox row: `(page, block_index, x_min, x_max, lines)`.
type BlockRow = (usize, usize, f64, f64, Vec<(usize, usize, f64, f64)>);

/// Lexically scan the preserved TEI for positive figure/table/formula rows.
pub fn scan_visual_rows(xml: &str) -> Vec<ScannedRow> {
    let mut rows = Vec::new();
    let mut figure_ordinal = 0u32;
    let mut table_ordinal = 0u32;
    for block in element_blocks(xml, "figure") {
        let opening = opening_tag(block, "figure");
        let is_table = opening.contains("type=\"table\"");
        let (family, ordinal) = if is_table {
            let ordinal = table_ordinal;
            table_ordinal += 1;
            ("table", ordinal)
        } else {
            let ordinal = figure_ordinal;
            figure_ordinal += 1;
            ("figure", ordinal)
        };
        let coords = coords_of(block);
        let page_ordinal = coords.as_deref().and_then(page_of);
        let caption = inner(block, "figDesc")
            .or_else(|| inner(block, "head"))
            .filter(|text| !text.is_empty());
        rows.push(ScannedRow {
            family,
            ordinal,
            page_ordinal,
            coords,
            caption,
            content: figure_content(block),
        });
    }
    for (formula_ordinal, block) in element_blocks(xml, "formula").into_iter().enumerate() {
        let coords = coords_of(block);
        let page_ordinal = coords.as_deref().and_then(page_of);
        let caption = inner(block, "label").filter(|text| !text.is_empty());
        rows.push(ScannedRow {
            family: "formula",
            ordinal: formula_ordinal as u32,
            page_ordinal,
            coords,
            caption,
            content: block.to_owned(),
        });
    }
    rows
}

// ---- end independent expectation builder ----

use da_parse::*;
use sha2::{Digest, Sha256};

/// Identity of this test-only scanner. It must never equal the SUT generator.
const ORACLE_GENERATOR_ID: &str =
    "crates/da-parse/tests/native_visual_native_input_oracle.rs#lexical-tei-visual-scanner";

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
) -> NativeVisualOracle {
    let mut rows = Vec::new();
    for (document_id, xml) in documents {
        for row in scan_visual_rows(xml) {
            let family = match row.family {
                "figure" => NativeVisualFamily::Figure,
                "table" => NativeVisualFamily::Table,
                _ => NativeVisualFamily::Formula,
            };
            rows.push(NativeVisualOracleRow {
                document_id: document_id.clone(),
                family,
                ordinal: row.ordinal,
                page_ordinal: row.page_ordinal,
                coords: row.coords,
                caption_digest: row.caption.map(|caption| sha256_text(&caption)),
                content_digest: sha256_text(&row.content),
            });
        }
    }
    NativeVisualOracle {
        schema_version: NATIVE_VISUAL_ORACLE_SCHEMA_VERSION.to_owned(),
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: backend.to_owned(),
        pins: pins
            .iter()
            .map(|pin| NativeVisualOraclePin {
                document_id: pin.id.clone(),
                sha256: pin.sha256.clone(),
            })
            .collect(),
        rows,
    }
}

// ───────────────────────────── inline S03 planes ─────────────────────────────

const PROVIDER_ID: &str = "native-fixture-donor";

struct PToken {
    kind: LayoutTokenKind,
    x_min: f64,
    x_max: f64,
    text: &'static str,
}

impl PToken {
    fn word(text: &'static str, x_min: f64, x_max: f64) -> Self {
        Self {
            kind: LayoutTokenKind::Word,
            x_min,
            x_max,
            text,
        }
    }
    fn visual(kind: LayoutTokenKind, text: &'static str, x_min: f64, x_max: f64) -> Self {
        Self {
            kind,
            x_min,
            x_max,
            text,
        }
    }
}

fn build_plane(
    document_id: &str,
    blocks: Vec<Vec<Vec<PToken>>>,
) -> (LayoutDocument, LayoutLattice, FullSegmentationOutcome) {
    let provider = LayoutProvider::new(PROVIDER_ID, "1", "a".repeat(64)).unwrap();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut block_rows: Vec<BlockRow> = Vec::new();
    let mut y = 10.0;
    for block in &blocks {
        let block_start = tokens.len();
        let block_first_y = y;
        let mut lines = Vec::new();
        for line in block {
            let line_start = tokens.len();
            let line_first_y = y;
            for token in line {
                let index = tokens.len() as u32;
                tokens.push(
                    LayoutToken::new(
                        0,
                        index,
                        index,
                        LayoutBBox::new(token.x_min, y, token.x_max, y + 10.0).unwrap(),
                        sha256_text(token.text),
                        LayoutSpan::present(index, index + 1).unwrap(),
                        token.kind,
                        LayoutStyleHints {
                            font_size_milli_pt: Some(11_000),
                            ..LayoutStyleHints::default()
                        },
                        provider.clone(),
                    )
                    .unwrap(),
                );
                if token.kind.is_lexical() {
                    lexemes.push(LexemeRow::new(
                        document_id,
                        PROVIDER_ID,
                        0,
                        index,
                        sha256_text(token.text),
                        token.text,
                    ));
                }
                y += 12.0;
            }
            lines.push((line_start, tokens.len(), line_first_y, y));
        }
        block_rows.push((block_start, tokens.len(), block_first_y, y, lines));
    }
    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 2000.0, 0, None, tokens.clone()).unwrap()],
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
            width: 300.0,
            height: 2000.0,
            rotation: 0,
            crop_box: None,
            blocks: block_rows
                .iter()
                .enumerate()
                .map(
                    |(block_index, (start, end, first_y, last_y, lines))| LatticeBlock {
                        source_ordinal: block_index as u32,
                        source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                        bbox: LayoutBBox::new(0.0, *first_y, 300.0, *last_y).unwrap(),
                        source_order: block_index as u32,
                        derived_order: Some(block_index as u32),
                        furniture: FurnitureEvidence::Absent,
                        lines: lines
                            .iter()
                            .enumerate()
                            .map(|(line_index, (ls, le, lfy, lly))| LatticeLine {
                                source_ordinal: line_index as u32,
                                source_span: LayoutSpan::present(*ls as u32, *le as u32).unwrap(),
                                bbox: LayoutBBox::new(0.0, *lfy, 300.0, *lly).unwrap(),
                                newline_boundary_count: 1,
                                tokens: tokens[*ls..*le]
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
                            })
                            .collect(),
                    },
                )
                .collect(),
        }],
    )
    .unwrap();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    (document, lattice, segmentation)
}

fn lexical_plane(document_id: &str) -> (LayoutDocument, LayoutLattice, FullSegmentationOutcome) {
    build_plane(
        document_id,
        vec![vec![
            vec![PToken::word("alpha", 10.0, 30.0)],
            vec![PToken::word("beta", 10.0, 30.0)],
        ]],
    )
}

fn visual_plane(document_id: &str) -> (LayoutDocument, LayoutLattice, FullSegmentationOutcome) {
    build_plane(
        document_id,
        vec![
            vec![
                vec![
                    PToken::word("cell", 10.0, 30.0),
                    PToken::word("cell", 60.0, 80.0),
                ],
                vec![
                    PToken::word("cell", 10.0, 30.0),
                    PToken::word("cell", 60.0, 80.0),
                ],
            ],
            vec![vec![PToken::visual(
                LayoutTokenKind::Image,
                "img",
                10.0,
                40.0,
            )]],
            vec![vec![PToken::visual(
                LayoutTokenKind::Formula,
                "eq",
                10.0,
                40.0,
            )]],
        ],
    )
}

fn empty_plane(document_id: &str) -> (LayoutDocument, LayoutLattice, FullSegmentationOutcome) {
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
    (document, lattice, segmentation)
}

fn run_plane(document_id: &str) -> NativeVisualFromInputOutcome {
    let (document, lattice, segmentation) = lexical_plane(document_id);
    build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap()
}

fn disposition(
    evaluations: &[NativeVisualFamilyEvaluation],
    family: NativeVisualFamily,
) -> NativeVisualFamilyDisposition {
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
        let rows = scan_visual_rows(xml);
        let counts = ["figure", "table", "formula"]
            .map(|family| rows.iter().filter(|row| row.family == family).count());
        assert_eq!(
            counts, POSITIVE_ROWS[index],
            "byte-derived positive denominators for {document_id}"
        );
        // Counts are derived from the bytes, not hardcoded: a second scan
        // reproduces them exactly, and every row carries a content digest.
        assert_eq!(scan_visual_rows(xml), rows, "scan is deterministic");
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
fn non_vacuous_positive_oracle_is_accepted() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    require_non_vacuous_visual_oracle(&oracle).expect("every family carries positive rows");
    for family in NativeVisualFamily::ALL {
        assert!(
            visual_oracle_positive_rows(&oracle, None, family) > 0,
            "{family:?} must be positive"
        );
    }
}

#[test]
fn lexical_only_native_plane_is_honest_unmet() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    for document_id in FIXTURE_IDS {
        let outcome = run_plane(document_id);
        let evaluations = evaluate_visual_family_rows(&outcome, &oracle).unwrap();
        assert_eq!(evaluations.len(), 3);
        for family in NativeVisualFamily::ALL {
            assert_eq!(
                disposition(&evaluations, family),
                NativeVisualFamilyDisposition::Unmet {
                    reason: NativeVisualUnmetReason::NativeKindUnreachable
                },
                "{document_id}/{family:?} is a typed unmet, never out_of_scope"
            );
        }
        let serialized = serde_json::to_string(&evaluations).unwrap();
        assert!(
            !serialized.contains("out_of_scope"),
            "a required family can never be moved out of scope: {serialized}"
        );
    }
}

#[test]
fn visual_kind_native_plane_is_measured() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    let (document, lattice, segmentation) = visual_plane("visual-doc");
    let outcome = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    let evaluations = evaluate_visual_family_rows(&outcome, &oracle).unwrap();
    for family in NativeVisualFamily::ALL {
        assert!(
            matches!(
                disposition(&evaluations, family),
                NativeVisualFamilyDisposition::Measured { .. }
            ),
            "a plane carrying the visual kind is measured for {family:?}"
        );
    }
}

#[test]
fn same_generator_oracle_is_refused() {
    let (backend, pins, documents) = scanned_documents();
    let mut oracle = oracle_from_scan(&backend, &pins, &documents);
    oracle.generator_id = NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID.to_owned();
    assert!(matches!(
        require_non_vacuous_visual_oracle(&oracle),
        Err(NativeVisualOracleError::NotIndependent(
            NativeVisualOracleIndependenceError::NotIndependent { .. }
        ))
    ));
}

#[test]
fn a_family_without_a_positive_row_is_vacuous() {
    let (backend, pins, documents) = scanned_documents();
    let mut oracle = oracle_from_scan(&backend, &pins, &documents);
    oracle
        .rows
        .retain(|row| row.family != NativeVisualFamily::Formula);
    assert_eq!(
        require_non_vacuous_visual_oracle(&oracle),
        Err(NativeVisualOracleError::OracleVacuous {
            family: NativeVisualFamily::Formula
        })
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
fn empty_document_is_a_typed_lane_absence() {
    let (backend, pins, documents) = scanned_documents();
    let oracle = oracle_from_scan(&backend, &pins, &documents);
    let (document, lattice, segmentation) = empty_plane(FIXTURE_IDS[0]);
    let outcome = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    assert!(outcome.observation.is_none());
    assert_eq!(outcome.absence, Some(NativeVisualAbsence::NotObserved));
    let evaluations = evaluate_visual_family_rows(&outcome, &oracle).unwrap();
    for family in NativeVisualFamily::ALL {
        assert_eq!(
            disposition(&evaluations, family),
            NativeVisualFamilyDisposition::Unmet {
                reason: NativeVisualUnmetReason::LaneAbsent
            }
        );
    }
}

#[test]
fn a_document_without_a_family_is_not_observed() {
    let (backend, pins, documents) = scanned_documents();
    let mut oracle = oracle_from_scan(&backend, &pins, &documents);
    // Remove only one document's formula rows; the family stays positive on the
    // other two documents, so the oracle is not vacuous — but for this document
    // there is nothing to meet.
    let doc = FIXTURE_IDS[0];
    oracle
        .rows
        .retain(|row| !(row.document_id == doc && row.family == NativeVisualFamily::Formula));
    require_non_vacuous_visual_oracle(&oracle).expect("the family is still positive elsewhere");
    let outcome = run_plane(doc);
    let evaluations = evaluate_visual_family_rows(&outcome, &oracle).unwrap();
    assert_eq!(
        disposition(&evaluations, NativeVisualFamily::Formula),
        NativeVisualFamilyDisposition::NotObserved
    );
    assert_eq!(
        disposition(&evaluations, NativeVisualFamily::Figure),
        NativeVisualFamilyDisposition::Unmet {
            reason: NativeVisualUnmetReason::NativeKindUnreachable
        }
    );
}

#[test]
fn expectation_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_visual_native_input_oracle.rs");
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
        "build_native_visual_input",
        "build_native_visual_observation",
        "detect_figure_heads",
        "detect_table_heads",
        "detect_formula_heads",
        "NativeVisualFamily",
        "NativeVisualHeadOutcome",
        "NativeVisualOracleRow",
        "NativeVisualOracle",
        "evaluate_visual_family_rows",
        "require_non_vacuous_visual_oracle",
        "NativeRegionCarrier",
        "segment_document_full",
    ] {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }
}
