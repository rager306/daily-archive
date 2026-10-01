//! Contract tests for the additive native visual-input band (S05/T03).
//!
//! The tests build the S03 plane directly (document + lattice + segmentation)
//! with tracked inline fixtures, so the band is exercised without any donor, TEI
//! or gitignored asset. The real corpus measurement belongs to the S05 receipt
//! runner; here the surface, the closed vocabularies, the typed absences and the
//! fail-closed limits are pinned.

use da_parse::*;
use sha2::{Digest, Sha256};

/// Provider identity used by the inline planes.
const PROVIDER_ID: &str = "native-fixture-donor";

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// One token of an inline plane.
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

/// A block bbox row: `(page, block_index, x_min, x_max, lines)`.
type BlockRow = (usize, usize, f64, f64, Vec<(usize, usize, f64, f64)>);

/// Build the S03 plane from inline blocks → lines → tokens.
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
                        digest(token.text),
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
                        digest(token.text),
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
            // A two-row, two-column grid → a table.
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

fn strip_line_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(cut) => &line[..cut],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn closed_vocabularies_have_no_out_of_scope() {
    assert_eq!(
        NativeVisualFamily::ALL.map(NativeVisualFamily::token),
        NATIVE_VISUAL_FAMILY_VOCABULARY
    );
    assert_eq!(
        NATIVE_VISUAL_FAMILY_DISPOSITION_VOCABULARY,
        ["measured", "unmet", "not_observed"]
    );
    assert!(!NATIVE_VISUAL_FAMILY_DISPOSITION_VOCABULARY.contains(&"out_of_scope"));
    for disposition in [
        NativeVisualFamilyDisposition::Measured { native_heads: 1 },
        NativeVisualFamilyDisposition::Unmet {
            reason: NativeVisualUnmetReason::NativeKindUnreachable,
        },
        NativeVisualFamilyDisposition::NotObserved,
    ] {
        let bytes = serde_json::to_vec(&disposition).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            !text.contains("scope"),
            "a required-family disposition must never carry a scope reduction: {text}"
        );
    }
}

#[test]
fn native_plane_surface_names_no_donor_or_tei_type() {
    let source = include_str!("../src/native_visual_native_input.rs");
    assert!(source.len() > 10_000, "the surface must stay substantive");
    let code = strip_line_comments(source);
    for marker in [
        "da_adapters",
        "layout_visual",
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
    ] {
        assert!(
            !code.contains(marker),
            "the native visual-input surface must not name `{marker}` in code"
        );
    }
    for required in [
        "build_native_visual_observation",
        "build_native_visual_input",
        "run_native_visual_heads",
        "evaluate_visual_family_rows",
        "require_independent_visual_oracle",
        "require_non_vacuous_visual_oracle",
    ] {
        assert!(source.contains(required), "the API lost {required}");
    }
}

#[test]
fn consecutive_builds_are_byte_identical() {
    let (document, lattice, segmentation) = visual_plane("determinism-doc");
    let first = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    let second = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn empty_document_is_a_typed_absence_never_a_zero() {
    let (document, lattice, segmentation) = empty_plane("empty-doc");
    let outcome = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    assert!(
        outcome.observation.is_none(),
        "an empty plane is not an observation"
    );
    assert_eq!(outcome.absence, Some(NativeVisualAbsence::NotObserved));
    assert!(outcome.heads.is_empty(), "no head ran on an empty plane");
    for family in NativeVisualFamily::ALL {
        assert_eq!(outcome.admitted_heads(family), 0);
    }
}

#[test]
fn lexical_only_plane_yields_typed_terminal_heads() {
    let (document, lattice, segmentation) = lexical_plane("lexical-doc");
    let outcome = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    let observation = outcome.observation.as_ref().expect("a non-empty plane");
    assert!(
        observation.primitives.is_empty(),
        "no lexical token may manufacture a visual primitive"
    );
    for family in NativeVisualFamily::ALL {
        assert!(
            matches!(
                &outcome.head(family).unwrap().outcome,
                NativeVisualHeadOutcome::Terminal {
                    reason: NativeVisualAbsence::NotObserved,
                    ..
                }
            ),
            "{family:?} must be a typed terminal on a lexical-only plane"
        );
    }
}

#[test]
fn visual_kind_tokens_and_a_table_grid_are_admitted() {
    let (document, lattice, segmentation) = visual_plane("visual-doc");
    let outcome = build_native_visual_input(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    assert!(matches!(
        outcome.head(NativeVisualFamily::Figure).unwrap().outcome,
        NativeVisualHeadOutcome::Figure { .. }
    ));
    assert!(matches!(
        outcome.head(NativeVisualFamily::Table).unwrap().outcome,
        NativeVisualHeadOutcome::Table { .. }
    ));
    assert!(matches!(
        outcome.head(NativeVisualFamily::Formula).unwrap().outcome,
        NativeVisualHeadOutcome::Formula { .. }
    ));
    assert_eq!(outcome.admitted_heads(NativeVisualFamily::Figure), 1);
    assert_eq!(outcome.admitted_heads(NativeVisualFamily::Table), 1);
    assert_eq!(outcome.admitted_heads(NativeVisualFamily::Formula), 1);
}

#[test]
fn table_heuristic_is_conservative_on_prose() {
    let (document, lattice, segmentation) = build_plane(
        "prose-doc",
        vec![vec![
            vec![PToken::word("the", 10.0, 40.0)],
            vec![PToken::word("quick", 10.0, 45.0)],
            vec![PToken::word("brown", 10.0, 50.0)],
        ]],
    );
    let observation = build_native_visual_observation(
        &document,
        &lattice,
        &segmentation,
        NativeVisualFromInputConfig::default(),
    )
    .unwrap();
    assert!(
        !observation
            .primitives
            .iter()
            .any(|primitive| primitive.kind == NativeVisualKind::Table),
        "a single-cell prose block is never a table grid"
    );
}

#[test]
fn line_and_primitive_limits_are_enforced() {
    let (document, lattice, segmentation) = visual_plane("limits-doc");
    let no_lines = NativeVisualFromInputConfig::new(0, NativeVisualConfig::default());
    assert!(matches!(
        build_native_visual_input(&document, &lattice, &segmentation, no_lines),
        Err(NativeVisualInputError::LineLimit { .. })
    ));

    let no_primitives = NativeVisualFromInputConfig::new(
        MAX_NATIVE_VISUAL_LINES,
        NativeVisualConfig::new(0, 128, 128),
    );
    assert!(matches!(
        build_native_visual_input(&document, &lattice, &segmentation, no_primitives),
        Err(NativeVisualInputError::NativeVisual(
            NativeVisualError::Oversize("primitives")
        ))
    ));
}

#[test]
fn plane_identity_drift_is_refused() {
    let (document_a, lattice_a, _) = lexical_plane("doc-a");
    let (_, _, segmentation_b) = lexical_plane("doc-b");
    assert!(matches!(
        build_native_visual_input(
            &document_a,
            &lattice_a,
            &segmentation_b,
            NativeVisualFromInputConfig::default()
        ),
        Err(NativeVisualInputError::IdentityMismatch("document_id"))
    ));
}

#[test]
fn a_same_generator_oracle_is_refused() {
    let provenance = NativeVisualOracleProvenance {
        generator_id: NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID.into(),
        backend: "java-grobid-crf".into(),
        sha256: "a".repeat(64),
    };
    assert_eq!(
        require_independent_visual_oracle(&provenance),
        Err(NativeVisualOracleIndependenceError::NotIndependent {
            generator_id: NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID.into()
        })
    );
}

#[test]
fn an_all_absence_oracle_is_vacuous() {
    let oracle = NativeVisualOracle {
        schema_version: NATIVE_VISUAL_ORACLE_SCHEMA_VERSION.into(),
        generator_id: "independent-lexical-scanner".into(),
        backend: "java-grobid-crf".into(),
        pins: vec![NativeVisualOraclePin {
            document_id: "doc".into(),
            sha256: "b".repeat(64),
        }],
        rows: Vec::new(),
    };
    assert_eq!(
        require_non_vacuous_visual_oracle(&oracle),
        Err(NativeVisualOracleError::OracleVacuous {
            family: NativeVisualFamily::Figure
        })
    );
    let _ = (
        NATIVE_VISUAL_TABLE_MIN_ROWS,
        NATIVE_VISUAL_TABLE_MIN_COLUMNS,
        NATIVE_VISUAL_TABLE_CELL_GAP_PT,
        NATIVE_VISUAL_TABLE_ANCHOR_TOLERANCE_PT,
        NATIVE_VISUAL_NATIVE_INPUT_SCHEMA_VERSION,
    );
}
