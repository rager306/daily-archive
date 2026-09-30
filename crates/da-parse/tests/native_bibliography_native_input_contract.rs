//! Contract tests for the native bibliography band (S04/T03).
//!
//! The band consumes the T01 [`NativeRegionCarrier`] References run, rebuilds
//! the ordered native reference entries, closes the `BibliographyTokenFacts` →
//! `citation_row` / `reference_segmenter_row` bridge, and runs the pure
//! bibliography atoms (citation fields / reference segments / citation anchors)
//! on the native input. These tests pin the happy path, the typed refusals, the
//! redaction discipline, the deterministic replay, the two typed out-of-scopes,
//! and the negative mutations named in the task plan (invented DOI/title,
//! anchor markup in native text, empty region as typed absence).
//!
//! The carrier is built through the **real** S03 segmenter here: the first
//! block is the `references` section marker, then one block per reference
//! entry. Entry text is chosen so it contains no standalone S03 region marker
//! word; the marker sensitivity itself is pinned by its own test below.

use da_parse::*;
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

/// One synthetic block: a single line of words, in the document order given.
/// `blocks[0]` is expected to be the `references` marker.
fn references_carrier(document_id: &str, blocks: &[Vec<&str>]) -> NativeRegionCarrier {
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut ranges: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut y = 100.0;
    for block in blocks {
        let start = tokens.len();
        let first_y = y;
        for word in block {
            let index = tokens.len() as u32;
            tokens.push(
                LayoutToken::new(
                    0,
                    index,
                    index,
                    LayoutBBox::new(20.0, y, 400.0, y + 10.0).unwrap(),
                    digest(word),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word),
                    LayoutStyleHints {
                        font_size_milli_pt: Some(9_000),
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
                digest(word),
                *word,
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
    build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::References, FullSemanticRegion::Body],
    )
    .expect("the fixture carrier builds")
}

/// The three hand-authored reference entries. None of the words is an S03
/// region marker, so the section stays References.
const ENTRIES: [&str; 3] = [
    "Doe, J. A bounded citation parser. doi:10.1000/xyz.",
    "Roe, K. Another bounded study. arXiv:2401.00001",
    "Poe, L. A third bounded note. https://example.test/paper",
];

fn entries_carrier(document_id: &str, entries: &[&str]) -> NativeRegionCarrier {
    let mut blocks: Vec<Vec<&str>> = vec![vec!["references"]];
    for entry in entries {
        blocks.push(entry.split_whitespace().collect());
    }
    references_carrier(document_id, &blocks)
}

/// Minimal project-owned lexicon. Contents only flip binary ABI columns; no
/// GROBID lexicon bytes are read or committed (D358).
fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["Doe".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "bibliography".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn outcome(document_id: &str) -> NativeBibliographyFromInputOutcome {
    build_native_bibliography_input(&entries_carrier(document_id, &ENTRIES), &lexicon())
        .expect("the happy-path fixture must execute the bibliography band")
}

fn atom(
    outcome: &NativeBibliographyFromInputOutcome,
    atom: BibliographyAtom,
) -> NativeBibliographyAtomOutcome {
    outcome
        .atoms
        .iter()
        .find(|entry| entry.atom == atom)
        .expect("every atom is reported")
        .clone()
}

#[test]
fn bridge_builds_the_bibliography_abi_from_the_native_run() {
    let carrier = entries_carrier("fixture-doc", &ENTRIES);
    let run = carrier.run(FullSemanticRegion::References).unwrap();
    let lines = build_bibliography_line_facts(run).unwrap();

    // Marker block + three entries.
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].text, "references");
    assert_eq!(lines[0].facts.len(), 1);
    assert_eq!(lines[1].text, ENTRIES[0]);
    assert_eq!(lines[2].text, ENTRIES[1]);
    assert_eq!(lines[3].text, ENTRIES[2]);

    // Line/block statuses come from the lattice grouping: every entry is its
    // own single-line block, so the first token opens and the last closes.
    let entry = &lines[1];
    assert_eq!(entry.facts[0].line_status, FactsLineStatus::Linestart);
    assert_eq!(
        entry.facts.last().unwrap().line_status,
        FactsLineStatus::Lineend
    );
    assert_eq!(entry.facts[0].block_status, FactsBlockStatus::Blockstart);
    assert_eq!(
        entry.facts.last().unwrap().block_status,
        FactsBlockStatus::Blockend
    );
    if entry.facts.len() > 2 {
        assert_eq!(entry.facts[1].line_status, FactsLineStatus::Linein);
        assert_eq!(entry.facts[1].block_status, FactsBlockStatus::Blockin);
    }
    // `line_length` is the line's admitted token count, and every token in the
    // line agrees about it.
    assert!(
        entry
            .facts
            .iter()
            .all(|fact| fact.line_length == entry.facts.len())
    );

    // The bridge feeds the real row builders, and they emit the exact ABIs.
    let lexicon = lexicon();
    let texts: Vec<String> = entry.facts.iter().map(|f| f.text.clone()).collect();
    let spans = gazetteer_spans(&lexicon, &texts).unwrap();
    let citation = citation_row(&entry.facts, &spans, &lexicon).unwrap();
    assert_eq!(citation.len(), entry.facts.len());
    assert!(
        citation
            .iter()
            .all(|row| row.width() == NATIVE_CITATION_FEATURE_COLUMNS)
    );
    let reference = reference_segmenter_row(&entry.facts, &lexicon, entry.facts.len()).unwrap();
    assert_eq!(reference.len(), entry.facts.len());
    assert!(
        reference
            .iter()
            .all(|row| row.width() == NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS)
    );
}

#[test]
fn atoms_execute_on_the_native_carrier() {
    let result = outcome("fixture-doc");
    assert_eq!(
        result.schema_version,
        NATIVE_BIBLIOGRAPHY_INPUT_SCHEMA_VERSION
    );
    assert_eq!(result.region, FullSemanticRegion::References);
    assert_eq!(result.entry_count, 4);
    assert_eq!(result.line_count, 4);
    assert_eq!(result.block_count, 4);
    assert_eq!(result.citation_field_entries.len(), 4);
    assert_eq!(result.citation_row_count, result.token_count);
    assert_eq!(result.reference_row_count, result.token_count);
    assert!(result.citation_feature_digest.starts_with("sha256:"));
    assert!(result.reference_feature_digest.starts_with("sha256:"));
    assert_eq!(result.citation_feature_digest.len(), "sha256:".len() + 64);

    // The section marker is its own entry and is not a citation; the three
    // reference entries each carry an explicit identifier.
    assert_eq!(
        result.citation_field_entries[0],
        BibliographyAtomDisposition::Malformed
    );
    assert_eq!(
        result
            .citation_field_entries
            .iter()
            .filter(|d| **d == BibliographyAtomDisposition::Recognized)
            .count(),
        3
    );
    let fields = atom(&result, BibliographyAtom::CitationFields);
    assert_eq!(fields.disposition, BibliographyAtomDisposition::Recognized);
    assert_eq!(fields.recognized_count, 3);
    assert!(fields.item_count >= 3);

    let segments = atom(&result, BibliographyAtom::ReferenceSegments);
    assert_eq!(
        segments.disposition,
        BibliographyAtomDisposition::Recognized
    );
    assert_eq!(segments.recognized_count, 4);

    let anchors = atom(&result, BibliographyAtom::CitationAnchors);
    assert_eq!(
        anchors.disposition,
        BibliographyAtomDisposition::OutOfScope,
        "anchor markup does not exist on the native plane"
    );
    assert_eq!(anchors.item_count, 0);

    assert_eq!(
        result.crf_scope,
        NativeBibliographyCrfScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::ModelNotInTree
        },
        "the reference-sequence decode is a typed out-of-scope, not a fabricated decode"
    );
    assert_eq!(
        result.anchor_scope,
        NativeBibliographyAnchorScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::AnchorMarkupNotInNativePlane
        }
    );
}

#[test]
fn outcome_provenance_comes_from_the_carrier() {
    let carrier = entries_carrier("fixture-doc", &ENTRIES);
    let result = build_native_bibliography_input(&carrier, &lexicon()).unwrap();
    assert_eq!(result.identity.document_id, "fixture-doc");
    assert_eq!(result.identity.provider_id, "fixture-donor");
    assert_eq!(result.identity.source_pdf_sha256, "b".repeat(64));
    assert_eq!(result.identity.order_digest, carrier.identity.order_digest);
    assert_eq!(result.identity.carrier_digest, carrier.digest().unwrap());
}

#[test]
fn outcome_redaction_hides_the_native_text() {
    let result = outcome("fixture-doc");
    let json = serde_json::to_string(&result).unwrap();
    let debug = format!("{result:?}");
    for word in ["Doe", "bounded", "xyz", "Roe", "Poe", "example"] {
        assert!(!json.contains(word), "serde leaked `{word}`");
        assert!(!debug.contains(word), "Debug leaked `{word}`");
    }
    assert!(json.contains(&result.citation_feature_digest));
    assert!(json.contains(&result.identity.carrier_digest));
}

#[test]
fn outcome_replay_is_byte_stable() {
    let first = outcome("fixture-doc");
    let second = outcome("fixture-doc");
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
    let roundtrip: NativeBibliographyFromInputOutcome =
        serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    assert_eq!(roundtrip, first);
}

#[test]
fn empty_references_region_is_a_typed_absence_not_a_zero() {
    let carrier = references_carrier("fixture-doc", &[vec!["introduction"]]);
    assert!(carrier.run(FullSemanticRegion::References).is_none());
    assert!(carrier.absence(FullSemanticRegion::References).is_some());
    assert_eq!(
        build_native_bibliography_input(&carrier, &lexicon()).unwrap_err(),
        NativeBibliographyInputError::NoAdmittedTokens {
            region: FullSemanticRegion::References
        }
    );
}

#[test]
fn token_with_internal_whitespace_is_typed() {
    // A lexeme that carries two words cannot become one ABI token; splitting it
    // would be invention.
    let carrier = references_carrier("fixture-doc", &[vec!["references"], vec!["Two Words"]]);
    assert_eq!(
        build_native_bibliography_input(&carrier, &lexicon()).unwrap_err(),
        NativeBibliographyInputError::TokenContainsWhitespace { page: 0, source: 1 }
    );
}

#[test]
fn whitespace_only_block_is_skipped_and_visible() {
    let carrier = references_carrier(
        "fixture-doc",
        &[
            vec!["references"],
            vec![" "],
            ENTRIES[0].split_whitespace().collect(),
        ],
    );
    let result = build_native_bibliography_input(&carrier, &lexicon()).unwrap();
    assert_eq!(result.skipped_empty_lines, 1, "the skip is reported");
    assert_eq!(result.entry_count, 2, "marker + one reference entry");
    assert_eq!(result.line_count, 3, "the empty line is still a line");
}

#[test]
fn anchor_markup_inside_native_text_is_refused() {
    // Native PDF text is not TEI markup. If the reconstructed text walks to a
    // recognized anchor, the input is refused rather than promoted.
    let carrier = references_carrier(
        "fixture-doc",
        &[
            vec!["references"],
            vec!["<ref", "type=\"bibr\"", "target=\"#b0\">Cited</ref>"],
        ],
    );
    match build_native_bibliography_input(&carrier, &lexicon()).unwrap_err() {
        NativeBibliographyInputError::AnchorScanNotNative { outcome, anchors } => {
            assert_eq!(outcome, "recognized");
            assert_eq!(anchors, 1);
        }
        other => panic!("expected AnchorScanNotNative, got {other:?}"),
    }
}

#[test]
fn invented_doi_and_title_are_not_invented_by_the_band() {
    // An entry with no explicit field is refused, never guessed into a field.
    let carrier = references_carrier(
        "fixture-doc",
        &[vec!["references"], vec!["Completely", "Fake", "Paper"]],
    );
    let result = build_native_bibliography_input(&carrier, &lexicon()).unwrap();
    let fields = atom(&result, BibliographyAtom::CitationFields);
    assert_ne!(fields.disposition, BibliographyAtomDisposition::Recognized);
    assert_eq!(fields.recognized_count, 0);
    assert_eq!(
        result.citation_field_entries[1],
        BibliographyAtomDisposition::Malformed,
        "a fieldless entry is Malformed, never an invented title"
    );
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("Fake"), "no text ever reaches the outcome");
}

#[test]
fn region_marker_words_inside_reference_text_flip_the_region() {
    // Measured S03 property, pinned here so the band's References run is never
    // read as "all reference text": `full_marker` matches a standalone marker
    // word anywhere, so a reference containing `Methods` flips the rest of the
    // document to Body. The band faithfully reports the carrier it is given.
    let carrier = references_carrier(
        "fixture-doc",
        &[
            vec!["references"],
            vec!["Doe", "Methods", "study"],
            vec!["Roe", "Another", "study"],
        ],
    );
    let references = carrier.run(FullSemanticRegion::References).unwrap();
    assert_eq!(
        references.tokens.len(),
        2,
        "only `references` and `Doe` stay in the References run"
    );
    let body = carrier.run(FullSemanticRegion::Body).unwrap();
    assert_eq!(
        body.tokens
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec!["Methods", "study", "Roe", "Another", "study"]
    );
    let result = build_native_bibliography_input(&carrier, &lexicon()).unwrap();
    assert_eq!(
        result.entry_count, 2,
        "the marker block plus the truncated first entry"
    );
    assert_eq!(result.token_count, 2);
}

#[test]
fn reference_segments_follow_carrier_order_and_never_sort() {
    // Contract counter-sense `reordered`: the segmenter must preserve carrier
    // order, so swapping two entries keeps the population but changes the
    // ordered identity the segment evidence digest folds.
    let mut swapped = ENTRIES;
    swapped.swap(0, 1);
    let forward =
        build_native_bibliography_input(&entries_carrier("fixture-doc", &ENTRIES), &lexicon())
            .unwrap();
    let reversed =
        build_native_bibliography_input(&entries_carrier("fixture-doc", &swapped), &lexicon())
            .unwrap();
    let forward_segments = atom(&forward, BibliographyAtom::ReferenceSegments);
    let reversed_segments = atom(&reversed, BibliographyAtom::ReferenceSegments);
    assert_eq!(
        forward_segments.recognized_count, reversed_segments.recognized_count,
        "reorder preserves cardinality"
    );
    assert_ne!(
        forward_segments.evidence_digest, reversed_segments.evidence_digest,
        "order is part of the segment evidence; the band never sorts or repairs it"
    );
    assert_eq!(
        forward.entry_count, reversed.entry_count,
        "reorder preserves the entry population"
    );
}

#[test]
fn citation_names_stays_unpooled_with_the_bibliography_lanes() {
    let result = outcome("fixture-doc");
    assert!(
        !result.citation_names_pooled_with_biblstruct,
        "the Java-side citation_names population is a distinct denominator"
    );
    // 2198 is the Java-side citation-name population over the three
    // checksum-bound fixtures; no lane here reconstructs it, and this band's
    // populations stay strictly smaller.
    let references = atom(&result, BibliographyAtom::CitationFields).recognized_count;
    let segments = atom(&result, BibliographyAtom::ReferenceSegments).recognized_count;
    for value in [
        references,
        segments,
        result.entry_count,
        result.token_count,
        result.citation_row_count,
    ] {
        assert_ne!(
            value, 2198,
            "no bibliography lane may reconstruct the citation_names denominator"
        );
    }
}

#[test]
fn coverage_verdict_distinguishes_granularity_from_loss() {
    let chars = |text: &str| bibliography_char_set(text);
    let oracle = chars("Doe bounded parser");
    let superset = chars("Doe bounded parser 2025");
    let subset = chars("Doe");

    assert_eq!(
        classify_bibliography_coverage(&oracle, &superset),
        BibliographyCoverageVerdict::GranularityOnly
    );
    assert_eq!(
        classify_bibliography_coverage(&oracle, &subset),
        BibliographyCoverageVerdict::ContentLoss
    );
    assert_eq!(
        classify_bibliography_coverage(&chars(""), &superset),
        BibliographyCoverageVerdict::NotAssessable,
        "an empty oracle is not a pass"
    );
    assert_eq!(
        classify_bibliography_coverage(&oracle, &chars("")),
        BibliographyCoverageVerdict::ContentLoss,
        "an empty native side against a populated oracle is measured loss"
    );
    // Whitespace and punctuation are granularity, not content.
    assert_eq!(chars("Ab-c d!"), chars("abcd"));
    assert_eq!(normalize_bibliography_text("  Doe   J.  "), "doe j.");
}

#[test]
fn independence_requires_a_distinct_generator() {
    let independent = BibliographyOracleProvenance {
        generator_id:
            "crates/da-parse/tests/native_bibliography_native_input_oracle.rs#lexical-tei-scanner"
                .into(),
        backend: "java-grobid-crf".into(),
        sha256: "a".repeat(64),
    };
    assert_eq!(
        require_independent_bibliography_oracle(&independent),
        Ok(())
    );

    let same_generator = BibliographyOracleProvenance {
        generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.into(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_bibliography_oracle(&same_generator),
        Err(BibliographyOracleIndependenceError::NotIndependent {
            generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.into()
        })
    );

    let no_generator = BibliographyOracleProvenance {
        generator_id: "  ".into(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_bibliography_oracle(&no_generator),
        Err(BibliographyOracleIndependenceError::MissingGenerator)
    );

    let no_backend = BibliographyOracleProvenance {
        backend: String::new(),
        ..independent.clone()
    };
    assert_eq!(
        require_independent_bibliography_oracle(&no_backend),
        Err(BibliographyOracleIndependenceError::MissingBackend)
    );

    let no_pin = BibliographyOracleProvenance {
        sha256: "not-a-digest".into(),
        ..independent
    };
    assert_eq!(
        require_independent_bibliography_oracle(&no_pin),
        Err(BibliographyOracleIndependenceError::MissingPin)
    );
}

#[test]
fn band_never_imports_a_tei_or_java_type() {
    // Source-level assertion: the band's input class is the native carrier and
    // the pure atoms. No TEI envelope, IR, lifter, or parser type may appear.
    let source = include_str!("../src/native_bibliography_native_input.rs");
    for name in [
        "TeiEnvelope",
        "TeiSpan",
        "TeiParseIr",
        "TeiLifter",
        "ScholarlyParser",
        "GrobidParser",
        "ParserPort",
        "CitationEntry",
    ] {
        assert!(
            !source.contains(name),
            "{name} must not appear in the native bibliography band"
        );
    }
    // And the module is registered with a re-export.
    let lib = include_str!("../src/lib.rs");
    assert!(lib.contains("mod native_bibliography_native_input;"));
    assert!(lib.contains("pub use native_bibliography_native_input::"));
}
