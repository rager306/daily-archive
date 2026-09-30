//! Contract tests for the additive native region carrier (S04/T01).
//!
//! The carrier is the join between S03's layout plane (`LayoutDocument` +
//! `LayoutLattice` + `FullSegmentationOutcome` + `LexemeRow`) and the ordered
//! region token runs the header/bibliography atoms consume. These tests pin the
//! happy path, the typed refusals, the redaction discipline, and determinism.

use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

fn header_style() -> LayoutStyleHints {
    LayoutStyleHints {
        bold: true,
        italic: false,
        monospace: false,
        font_size_milli_pt: Some(14_000),
    }
}

/// Default words of the fixture: header tokens, a body marker, and a
/// references block. `Title`/`Author` default to Header, `introduction` opens
/// Body, and `references` opens References (S03's semantic markers).
const WORDS: [&str; 8] = [
    "Title",
    "Author",
    "introduction",
    "body",
    "references",
    "[1]",
    "Smith",
    "2020",
];

/// One page with a header block, a body block, and a references block.
fn fixture() -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    fixture_with(WORDS)
}

fn fixture_with(
    words: [&str; 8],
) -> (
    LayoutDocument,
    LayoutLattice,
    Vec<LexemeRow>,
    FullSegmentationOutcome,
) {
    let tokens: Vec<LayoutToken> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let y = 10.0 + i as f64 * 12.0;
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(20.0, y, 80.0, y + 10.0).unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::from_lexical_text(text),
                if i == 0 {
                    header_style()
                } else {
                    LayoutStyleHints::default()
                },
                provider(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "fixture-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 400.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let count = tokens.len() as u32;
    let lattice_tokens = tokens
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
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 300.0,
            height: 400.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, count).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes: Vec<LexemeRow> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                "fixture-doc",
                "fixture-donor",
                0,
                i as u32,
                digest(text),
                *text,
            )
        })
        .collect();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    (document, lattice, lexemes, segmentation)
}

fn build_header_references() -> NativeRegionCarrier {
    let (document, lattice, lexemes, segmentation) = fixture();
    build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header, FullSemanticRegion::References],
    )
    .expect("the fixture carrier builds")
}

fn texts(run: &NativeRegionRun) -> Vec<&str> {
    run.tokens.iter().map(NativeRegionToken::text).collect()
}

#[test]
fn header_and_references_runs_join_text_and_layout() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let carrier = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header, FullSemanticRegion::References],
    )
    .unwrap();

    assert_eq!(carrier.schema_version, NATIVE_REGION_INPUT_SCHEMA_VERSION);
    assert_eq!(carrier.identity.document_id, "fixture-doc");
    assert_eq!(carrier.identity.provider_id, "fixture-donor");
    assert_eq!(carrier.identity.source_pdf_sha256, "b".repeat(64));
    assert_eq!(carrier.identity.order_digest, segmentation.order_digest);
    assert_eq!(carrier.identity.lattice_fingerprint.len(), 71);
    assert!(carrier.absences.is_empty());
    assert_eq!(carrier.runs.len(), 2);

    let header = carrier.run(FullSemanticRegion::Header).unwrap();
    assert_eq!(header.region, FullSemanticRegion::Header);
    assert_eq!(texts(header), ["Title", "Author"]);
    assert_eq!(header.start, header.tokens[0].location);
    assert_eq!(header.end, header.tokens[1].location);
    assert!(header.boundary_digest.starts_with("sha256:"));

    // Layout facts come from the LayoutToken under the same key.
    assert_eq!(header.tokens[0].kind, LayoutTokenKind::Word);
    assert_eq!(header.tokens[0].style, header_style());
    assert_eq!(header.tokens[0].bbox, document.pages[0].tokens[0].bbox);
    assert_eq!(header.tokens[0].text_digest, digest("Title"));
    assert_eq!(header.tokens[0].source_ordinal, 0);
    assert_eq!(header.tokens[0].location.token_ordinal, 0);
    assert_eq!(header.tokens[1].source_ordinal, 1);

    let references = carrier.run(FullSemanticRegion::References).unwrap();
    assert_eq!(texts(references), ["references", "[1]", "Smith", "2020"]);
    assert_eq!(references.tokens[0].text_digest, digest("references"));
    assert_eq!(references.end.token_ordinal, 7);

    // The digest is a pure function of the carrier.
    assert_eq!(carrier.digest().unwrap(), carrier.digest().unwrap());
}

#[test]
fn requested_region_selection_is_exact() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let body = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Body],
    )
    .unwrap();
    assert_eq!(body.runs.len(), 1);
    assert_eq!(
        texts(body.run(FullSemanticRegion::Body).unwrap()),
        ["introduction", "body"]
    );
    assert!(body.run(FullSemanticRegion::Header).is_none());
    assert!(
        body.absence(FullSemanticRegion::Header).is_none(),
        "an unrequested region is neither a run nor an absence"
    );
}

#[test]
fn absent_requested_region_is_a_typed_absence() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let carrier = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header, FullSemanticRegion::Annex],
    )
    .unwrap();
    assert_eq!(carrier.runs.len(), 1);
    assert!(carrier.run(FullSemanticRegion::Annex).is_none());
    assert_eq!(
        carrier.absence(FullSemanticRegion::Annex),
        Some(&NativeRegionAbsence::NoAdmittedTokens {
            region: FullSemanticRegion::Annex
        })
    );
}

#[test]
fn caller_region_order_and_duplicates_do_not_change_the_carrier() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let forward = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header, FullSemanticRegion::References],
    )
    .unwrap();
    let reversed = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[
            FullSemanticRegion::References,
            FullSemanticRegion::Header,
            FullSemanticRegion::References,
        ],
    )
    .unwrap();
    assert_eq!(forward, reversed);
    assert_eq!(
        serde_json::to_vec(&forward).unwrap(),
        serde_json::to_vec(&reversed).unwrap()
    );
}

#[test]
fn replay_is_byte_stable() {
    let first = build_header_references();
    let second = build_header_references();
    assert_eq!(first, second);
    assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn redaction_hides_raw_text_from_debug_and_serde() {
    let carrier = build_header_references();
    let json = serde_json::to_string(&carrier).unwrap();
    let debug = format!("{carrier:?}");
    for word in ["Title", "Author", "introduction", "Smith", "2020"] {
        assert!(!json.contains(word), "serde leaked `{word}`");
        assert!(!debug.contains(word), "Debug leaked `{word}`");
    }
    assert!(debug.contains("<redacted>"));
    // The digest is carried, so the projection is still content-sensitive.
    assert!(json.contains(&digest("Title")));
    // A different text in the same position changes the carrier digest, even
    // though the redacted serde projection carries only its hash.
    let mut other_words = WORDS;
    other_words[0] = "Different";
    let (document, lattice, lexemes, segmentation) = fixture_with(other_words);
    let changed = build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::Header],
    )
    .unwrap();
    assert_ne!(carrier.digest().unwrap(), changed.digest().unwrap());
}

#[test]
fn empty_region_request_is_refused() {
    let (document, lattice, lexemes, segmentation) = fixture();
    assert_eq!(
        build_native_region_carrier(&document, &lattice, &lexemes, &segmentation, &[]),
        Err(NativeRegionError::EmptyRegionRequest)
    );
}

#[test]
fn duplicate_lexeme_is_typed() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let mut duplicated = lexemes.clone();
    duplicated.push(duplicated[0].clone());
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &duplicated,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::DuplicateLexeme { page: 0, source: 0 }
    );
}

#[test]
fn missing_lexeme_is_typed() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let missing = &lexemes[1..];
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            missing,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::MissingLexeme { page: 0, source: 0 }
    );
}

#[test]
fn digest_mismatch_is_typed() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let mut tampered = lexemes.clone();
    tampered[0] = LexemeRow::new(
        "fixture-doc",
        "fixture-donor",
        0,
        0,
        digest("Title"),
        "Tampered",
    );
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &tampered,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::DigestMismatch { page: 0, source: 0 }
    );
}

#[test]
fn missing_lattice_token_is_typed() {
    let (document, lattice, lexemes, mut segmentation) = fixture();
    segmentation.assignments.push(FullTokenAssignment {
        location: FullLocation {
            page_ordinal: 0,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: 999,
        },
        source_ordinal: 999,
        text_digest: digest("ghost"),
        region: FullSemanticRegion::Header,
    });
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &lexemes,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::MissingLatticeToken {
            page: 0,
            source: 999
        }
    );
}

#[test]
fn malformed_order_is_typed() {
    let (document, lattice, lexemes, mut segmentation) = fixture();
    segmentation.assignments.swap(0, 1);
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &lexemes,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::MalformedOrder { page: 0, source: 0 }
    );
}

#[test]
fn duplicate_assignment_is_typed() {
    let (document, lattice, lexemes, mut segmentation) = fixture();
    let duplicate = segmentation.assignments[0].clone();
    segmentation.assignments.push(duplicate);
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &lexemes,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::DuplicateAssignment { page: 0, source: 0 }
    );
}

#[test]
fn identity_drift_is_typed() {
    let (document, lattice, lexemes, mut segmentation) = fixture();
    segmentation.identity.lattice_fingerprint = "0".repeat(64);
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &lexemes,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::IdentityMismatch("lattice_fingerprint")
    );

    let (document, lattice, lexemes, mut segmentation) = fixture();
    segmentation.identity.document_id = "other-doc".into();
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &lexemes,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::IdentityMismatch("document_id")
    );
}

#[test]
fn provider_and_document_drift_are_typed() {
    let (document, lattice, lexemes, segmentation) = fixture();
    let mut wrong_provider = lexemes.clone();
    wrong_provider[0] = LexemeRow::new(
        "fixture-doc",
        "other-provider",
        0,
        0,
        digest("Title"),
        "Title",
    );
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &wrong_provider,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::ProviderMismatch
    );

    let mut wrong_document = lexemes.clone();
    wrong_document[0] =
        LexemeRow::new("other-doc", "fixture-donor", 0, 0, digest("Title"), "Title");
    assert_eq!(
        build_native_region_carrier(
            &document,
            &lattice,
            &wrong_document,
            &segmentation,
            &[FullSemanticRegion::Header]
        )
        .unwrap_err(),
        NativeRegionError::DocumentMismatch
    );
}
