//! Contract tests for the native header band (S04/T02).
//!
//! The band consumes the T01 [`NativeRegionCarrier`] Header run, rebuilds the
//! ordered native text lines, closes the `HeaderFeatureToken` →
//! `render_header_features` bridge, and runs the pure header atoms
//! (date / personal name / affiliation / affiliation link) on the native
//! input. These tests pin the happy path, the typed refusals, the redaction
//! discipline, the deterministic replay, and the negative mutations named in
//! the task plan (invented author/org, empty region as typed absence).

use da_parse::*;
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

/// One synthetic header row: words, font size in milli-points, and the line's
/// left edge. `None` font size models a donor that declared no style.
struct Row {
    words: Vec<&'static str>,
    font_milli: Option<u32>,
    x_min: f64,
}

fn row(words: &[&'static str], font_milli: u32) -> Row {
    Row {
        words: words.to_vec(),
        font_milli: Some(font_milli),
        x_min: 20.0,
    }
}

fn styled_row(words: &[&'static str], font_milli: u32, x_min: f64) -> Row {
    Row {
        words: words.to_vec(),
        font_milli: Some(font_milli),
        x_min,
    }
}

/// The happy-path header: title / author / affiliation / date, then a body
/// marker so the S03 segmentation closes the Header region.
///
/// Without a CRF decoder the band reports line-level atom recognition, not
/// field attribution, so a field line that another atom would also accept
/// cross-recognizes. This fixture is chosen so each field line is accepted by
/// exactly its own atom: the title carries `:` (rejected by all three), the
/// affiliation carries `,` (rejected by the header name grammar), and the date
/// is `Feb 2026` (rejected as an affiliation marker because it starts with a
/// letter).
fn happy_rows() -> Vec<Row> {
    vec![
        row(&["GEPA:", "Reflective", "Prompt", "Evolution"], 14_000),
        row(&["Ashish", "Vaswani"], 11_000),
        row(&["Monash", "University,", "Australia"], 9_000),
        row(&["Feb", "2026"], 9_000),
        row(&["introduction"], 11_000),
    ]
}

fn build(
    rows: &[Row],
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut line_ranges: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut y = 10.0;
    for (row_index, item) in rows.iter().enumerate() {
        let start = tokens.len();
        let first_y = y;
        for word in &item.words {
            let index = tokens.len() as u32;
            let style = LayoutStyleHints {
                bold: item.font_milli.is_some_and(|milli| milli >= 13_000),
                italic: false,
                monospace: false,
                font_size_milli_pt: item.font_milli,
            };
            tokens.push(
                LayoutToken::new(
                    0,
                    index,
                    index,
                    LayoutBBox::new(item.x_min, y, item.x_min + 90.0, y + 10.0).unwrap(),
                    digest(word),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word),
                    style,
                    provider(),
                )
                .unwrap(),
            );
            lexemes.push(LexemeRow::new(
                "fixture-doc",
                "fixture-donor",
                0,
                index,
                digest(word),
                *word,
            ));
            y += 12.0;
        }
        let end = tokens.len();
        line_ranges.push((start, end, first_y, y));
        let _ = row_index;
    }
    let document = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 900.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();

    let lattice_lines: Vec<LatticeLine> = line_ranges
        .iter()
        .enumerate()
        .map(|(line_index, (start, end, first_y, last_y))| LatticeLine {
            source_ordinal: line_index as u32,
            source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
            bbox: LayoutBBox::new(0.0, *first_y, 300.0, *last_y).unwrap(),
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
        })
        .collect();
    let count = tokens.len() as u32;
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 300.0,
            height: 900.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 900.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: lattice_lines,
            }],
        }],
    )
    .unwrap();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    (document, lattice, lexemes, segmentation)
}

fn header_carrier(rows: &[Row]) -> NativeRegionCarrier {
    let (document, lattice, lexemes, segmentation) = build(rows);
    build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header],
    )
    .expect("the fixture carrier builds")
}

/// Minimal project-owned lexicon. Contents only flip binary ABI columns; no
/// GROBID lexicon bytes are read or committed.
fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["ashish".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn outcome(rows: &[Row]) -> NativeHeaderFromInputOutcome {
    build_native_header_input(&header_carrier(rows), &lexicon())
        .expect("the happy-path fixture must execute the header band")
}

fn atom(outcome: &NativeHeaderFromInputOutcome, atom: HeaderAtom) -> NativeHeaderAtomOutcome {
    outcome
        .atoms
        .iter()
        .find(|entry| entry.atom == atom)
        .expect("every atom is reported")
        .clone()
}

#[test]
fn bridge_renders_the_crf_feature_abi_from_the_native_run() {
    let carrier = header_carrier(&happy_rows());
    let run = carrier.run(FullSemanticRegion::Header).unwrap();
    let tokens = build_header_feature_tokens(run).unwrap();

    // Title 4 words, author 2, affiliation 3, date 2.
    assert_eq!(tokens.len(), 11);
    assert_eq!(tokens[0].text, "GEPA:");
    assert_eq!(tokens[0].font_size, 14.0);
    assert!(tokens[0].bold, "a 14pt row is the fixture's bold row");
    assert_eq!(tokens[4].font_size, 11.0);
    assert_eq!(tokens[6].font_size, 9.0);
    assert_eq!(tokens[9].font_size, 9.0);

    // One block, four lines: the block opens and closes once, each line opens
    // and closes once.
    assert_eq!(tokens[0].block_status, BlockStatus::Blockstart);
    assert_eq!(tokens[10].block_status, BlockStatus::Blockend);
    assert_eq!(tokens[1].block_status, BlockStatus::Blockin);
    assert_eq!(tokens[0].line_status, LineStatus::Linestart);
    assert_eq!(tokens[3].line_status, LineStatus::Lineend);
    assert_eq!(tokens[1].line_status, LineStatus::Linein);
    assert_eq!(tokens[4].line_status, LineStatus::Linestart);
    assert_eq!(tokens[5].line_status, LineStatus::Lineend);
    assert_eq!(tokens[6].line_status, LineStatus::Linestart);
    assert_eq!(tokens[10].line_status, LineStatus::Lineend);

    // All lines share the same left edge, so nothing is indented.
    assert!(
        tokens
            .iter()
            .all(|token| token.alignment == HeaderAlignment::AlignedLeft)
    );

    // The font statistics are derived from the tokens, not supplied.
    let fonts = derive_header_font_statistics(&tokens).unwrap();
    assert_eq!(fonts.largest_font, 14.0);
    assert_eq!(fonts.smallest_font, 9.0);
    assert!(fonts.smallest_font <= fonts.average_font);
    assert!(fonts.average_font <= fonts.largest_font);

    // And the bridge renders the exact 29-column ABI.
    let rows = render_header_features(&tokens, fonts, &lexicon()).unwrap();
    assert_eq!(rows.len(), 11);
    assert!(
        rows.iter()
            .all(|r| r.width() == NATIVE_HEADER_FEATURE_COLUMNS)
    );
}

#[test]
fn indented_line_is_reported_as_line_indent() {
    let rows = vec![
        row(&["Title", "Line"], 14_000),
        styled_row(&["Indented"], 11_000, 60.0),
        row(&["introduction"], 11_000),
    ];
    let carrier = header_carrier(&rows);
    let tokens = build_header_feature_tokens(carrier.run(FullSemanticRegion::Header).unwrap())
        .expect("the indentation fixture builds");
    // Line 0 starts at x=20 (the leftmost); its tokens are aligned-left.
    assert_eq!(tokens[0].alignment, HeaderAlignment::AlignedLeft);
    assert_eq!(tokens[1].alignment, HeaderAlignment::AlignedLeft);
    // Line 1 starts at x=60, well beyond the epsilon.
    assert_eq!(tokens[2].alignment, HeaderAlignment::LineIndent);
}

#[test]
fn header_atoms_recognize_the_native_lines() {
    let result = outcome(&happy_rows());
    assert_eq!(result.schema_version, NATIVE_HEADER_INPUT_SCHEMA_VERSION);
    assert_eq!(result.region, FullSemanticRegion::Header);
    assert_eq!(result.token_count, 11);
    assert_eq!(result.line_count, 4);
    assert_eq!(result.block_count, 1);
    assert_eq!(result.feature_row_count, 11);
    assert_eq!(result.feature_digest.len(), 71);
    assert!(result.feature_digest.starts_with("sha256:"));

    let date = atom(&result, HeaderAtom::Date);
    assert_eq!(date.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(date.recognized_count, 1);

    let name = atom(&result, HeaderAtom::PersonalName);
    assert_eq!(name.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(name.recognized_count, 1);

    let affiliation = atom(&result, HeaderAtom::Affiliation);
    assert_eq!(affiliation.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(affiliation.recognized_count, 1);

    let link = atom(&result, HeaderAtom::AffiliationLink);
    assert_eq!(link.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(link.recognized_count, 1);

    assert_eq!(
        result.crf_scope,
        NativeHeaderCrfScope::OutOfScope {
            reason: NativeHeaderOutOfScope::ModelNotInTree
        },
        "the CRF decode is a typed out-of-scope, not a fabricated decode"
    );
}

#[test]
fn outcome_provenance_comes_from_the_carrier() {
    let carrier = header_carrier(&happy_rows());
    let result = build_native_header_input(&carrier, &lexicon()).unwrap();
    assert_eq!(result.identity.document_id, "fixture-doc");
    assert_eq!(result.identity.provider_id, "fixture-donor");
    assert_eq!(result.identity.source_pdf_sha256, "b".repeat(64));
    assert_eq!(result.identity.order_digest, carrier.identity.order_digest);
    assert_eq!(result.identity.carrier_digest, carrier.digest().unwrap());
}

#[test]
fn outcome_redaction_hides_the_native_text() {
    let result = outcome(&happy_rows());
    let json = serde_json::to_string(&result).unwrap();
    let debug = format!("{result:?}");
    for word in [
        "GEPA",
        "Reflective",
        "Ashish",
        "Vaswani",
        "Monash",
        "University",
        "Australia",
    ] {
        assert!(!json.contains(word), "serde leaked `{word}`");
        assert!(!debug.contains(word), "Debug leaked `{word}`");
    }
    assert!(json.contains(&result.feature_digest));
    assert!(json.contains(&result.identity.carrier_digest));
}

#[test]
fn outcome_replay_is_byte_stable() {
    let first = outcome(&happy_rows());
    let second = outcome(&happy_rows());
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
    let roundtrip: NativeHeaderFromInputOutcome =
        serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    assert_eq!(roundtrip, first);
}

#[test]
fn empty_header_region_is_a_typed_absence_not_a_zero() {
    let rows = vec![row(&["introduction"], 11_000), row(&["Materials"], 11_000)];
    let carrier = header_carrier(&rows);
    assert!(carrier.run(FullSemanticRegion::Header).is_none());
    assert!(carrier.absence(FullSemanticRegion::Header).is_some());
    assert_eq!(
        build_native_header_input(&carrier, &lexicon()).unwrap_err(),
        NativeHeaderInputError::NoAdmittedTokens {
            region: FullSemanticRegion::Header
        }
    );
}

#[test]
fn missing_font_size_is_typed() {
    let rows = vec![
        Row {
            words: vec!["Ashish", "Vaswani"],
            font_milli: None,
            x_min: 20.0,
        },
        row(&["introduction"], 11_000),
    ];
    let carrier = header_carrier(&rows);
    assert!(matches!(
        build_native_header_input(&carrier, &lexicon()).unwrap_err(),
        NativeHeaderInputError::MissingFontSize { .. }
    ));
}

#[test]
fn zero_font_size_is_typed() {
    let rows = vec![
        Row {
            words: vec!["Ashish", "Vaswani"],
            font_milli: Some(0),
            x_min: 20.0,
        },
        row(&["introduction"], 11_000),
    ];
    let carrier = header_carrier(&rows);
    assert_eq!(
        build_native_header_input(&carrier, &lexicon()).unwrap_err(),
        NativeHeaderInputError::InvalidFontSize {
            page: 0,
            source: 0,
            milli_pt: 0
        }
    );
}

#[test]
fn token_with_internal_whitespace_is_typed() {
    // A lexeme that carries two words cannot become one CRF token; splitting it
    // would be invention.
    let rows = vec![row(&["Two Words"], 11_000), row(&["introduction"], 11_000)];
    let carrier = header_carrier(&rows);
    assert_eq!(
        build_native_header_input(&carrier, &lexicon()).unwrap_err(),
        NativeHeaderInputError::TokenContainsWhitespace { page: 0, source: 0 }
    );
}

#[test]
fn whitespace_tokens_are_skipped_not_errors() {
    let rows = vec![
        row(&["Ashish", " ", "Vaswani"], 11_000),
        row(&["introduction"], 11_000),
    ];
    let carrier = header_carrier(&rows);
    let result = build_native_header_input(&carrier, &lexicon()).unwrap();
    // The whitespace token is admitted by the carrier but is not a CRF word.
    assert_eq!(result.token_count, 3);
    let name = atom(&result, HeaderAtom::PersonalName);
    assert_eq!(name.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(name.recognized_count, 1, "the line joins back to one name");
}

#[test]
fn invented_author_and_org_are_not_recognized() {
    // A single-token "name" is ambiguous, and an unknown string is not an
    // affiliation organization. Neither may be invented into a recognition.
    let rows = vec![row(&["Zzz"], 11_000), row(&["introduction"], 11_000)];
    let carrier = header_carrier(&rows);
    let result = build_native_header_input(&carrier, &lexicon()).unwrap();
    let name = atom(&result, HeaderAtom::PersonalName);
    assert_ne!(name.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(name.disposition, HeaderAtomDisposition::Ambiguous);
    assert_eq!(name.recognized_count, 0);
    let affiliation = atom(&result, HeaderAtom::Affiliation);
    assert_ne!(affiliation.disposition, HeaderAtomDisposition::Recognized);
    assert_eq!(affiliation.disposition, HeaderAtomDisposition::Unsupported);
    assert_eq!(affiliation.recognized_count, 0);
}

#[test]
fn coverage_verdict_distinguishes_granularity_from_loss() {
    let chars = |text: &str| header_char_set(text);
    let oracle = chars("GEPA Evolution");
    let superset = chars("GEPA Evolution 2026");
    let subset = chars("GEPA");

    assert_eq!(
        classify_header_coverage(&oracle, &superset),
        HeaderCoverageVerdict::GranularityOnly
    );
    assert_eq!(
        classify_header_coverage(&oracle, &subset),
        HeaderCoverageVerdict::ContentLoss
    );
    assert_eq!(
        classify_header_coverage(&chars(""), &superset),
        HeaderCoverageVerdict::NotAssessable,
        "an empty oracle is not a pass"
    );
    assert_eq!(
        classify_header_coverage(&oracle, &chars("")),
        HeaderCoverageVerdict::ContentLoss,
        "an empty native side against a populated oracle is measured loss"
    );
    // Whitespace and punctuation are granularity, not content.
    assert_eq!(chars("Ab-c d!"), chars("abcd"));
    assert_eq!(
        normalize_header_text("  Ashish   Vaswani "),
        "ashish vaswani"
    );
}

#[test]
fn independence_requires_a_distinct_generator() {
    let independent = HeaderOracleProvenance {
        generator_id:
            "crates/da-parse/tests/native_header_native_input_oracle.rs#lexical-tei-scanner".into(),
        backend: "java-grobid-crf".into(),
        sha256: "a".repeat(64),
    };
    assert_eq!(require_independent_header_oracle(&independent), Ok(()));

    let same_generator = HeaderOracleProvenance {
        generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.into(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_header_oracle(&same_generator),
        Err(HeaderOracleIndependenceError::NotIndependent {
            generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.into()
        })
    );

    let no_generator = HeaderOracleProvenance {
        generator_id: "  ".into(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_header_oracle(&no_generator),
        Err(HeaderOracleIndependenceError::MissingGenerator)
    );

    let no_backend = HeaderOracleProvenance {
        backend: String::new(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_header_oracle(&no_backend),
        Err(HeaderOracleIndependenceError::MissingBackend)
    );

    let no_pin = HeaderOracleProvenance {
        sha256: "not-a-digest".into(),
        ..independent
    };
    assert_eq!(
        require_independent_header_oracle(&no_pin),
        Err(HeaderOracleIndependenceError::MissingPin)
    );
}
