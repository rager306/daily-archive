//! Character-coverage oracle for the native body text (S05/T01).
//!
//! S03 named but did not resolve the token-coverage gap between the
//! `pdf_inspector_positioned` lane and the `pdfalto` lane (aggregate ×3.29 over
//! 202 pages, `doc/review/grobid-absorption-s03-spike.md` §3.5). Tokens cannot
//! separate granularity from real loss; characters can. This test is the
//! structurally independent oracle:
//!
//! 1. a **lexical scanner** (below, outside the SUT, importing no production
//!    helper) reads the three sha256-pinned preserved TEI fixtures, verifies the
//!    pin against the bytes, extracts `<body>` and counts **characters** at each
//!    rung of the normalization ladder;
//! 2. the SUT assembles the native body text from a layout plane derived from
//!    the same body characters and the two sides are compared per document and
//!    as an aggregate;
//! 3. the typed verdict (`GranularityOnly` / `ContentLoss` / `NotAssessable`),
//!    the D498 reopen flag, and the negative mutations are asserted by name.
//!
//! The plane here is a **tracked-TEI projection** and this test's claim scope is
//! therefore limited to **assembly conservation**: it shows the SUT joins and
//! normalizes the characters it is given without loss, and it asserts the typed
//! verdict machinery on **synthetic** inputs. It deliberately does NOT issue a
//! body-gap verdict from TEI-projected inputs. The real
//! `pdf_inspector_positioned` measurement runs in the detached probe
//! `crates/da-cli/examples/native_body_character_probe.rs` (the donor lives in
//! `da-adapters` and is unreachable from a `da-parse` test) and is written to
//! `artifacts/parse_compat/native_body_character_shadow.json`.

use da_parse::{
    BodyCoverageGap, BodyCoverageVerdict, BodyNormalization, BodyOracleCounts,
    BodyOracleIndependence, BodyOracleProvenance, CoordinateDialect,
    FULL_SEGMENTATION_SCHEMA_VERSION, FullLocation, FullSegmentationIdentity,
    FullSegmentationOutcome, FullSemanticRegion, FullTokenAssignment, FurnitureEvidence,
    LatticeBlock, LatticeLine, LatticePage, LatticeToken, LayoutBBox, LayoutDocument,
    LayoutLattice, LayoutPage, LayoutProvider, LayoutSpan, LayoutStyleHints, LayoutToken,
    LayoutTokenKind, LexemeRow, NATIVE_BODY_TEXT_GENERATOR_ID, NATIVE_BODY_TEXT_SCHEMA_VERSION,
    NativeBodyLengths, NativeBodyText, NativeBodyTextError, ReadingOrder, aggregate_body_coverage,
    build_native_body_text, classify_body_coverage, require_independent_body_oracle,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

// ───────────────────────── independent lexical oracle ─────────────────────────

const PRESERVED: [(&str, &str, &str); 3] = [
    (
        "hub-2507.19457",
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    ),
    (
        "rlm-2512.24601",
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    ),
    (
        "reca-2605.26525v1",
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    ),
];

/// The oracle's generator identity. Deliberately not the SUT's.
const ORACLE_GENERATOR: &str = "preserved-java-tei-lexical-scanner";

/// Subtree element names whose text is visual/callout evidence, not body prose.
const EXCLUDED_SUBTREES: [&str; 5] = ["figure", "table", "formula", "note", "figDesc"];

/// Tags at which a text boundary is materialized as a newline.
const BLOCK_TAGS: [&str; 10] = [
    "p", "head", "div", "l", "item", "label", "list", "ab", "seg", "s",
];

fn fixture_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(file)
}

/// Read a preserved fixture and fail on a sha256 pin drift.
fn pinned_source(file: &str, pin: &str) -> Result<String, String> {
    let bytes = std::fs::read(fixture_path(file)).map_err(|error| error.to_string())?;
    let observed = format!("{:x}", Sha256::digest(&bytes));
    if observed != pin {
        return Err(format!("sha256 drift for {file}: {observed} != {pin}"));
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn tag_name(tag: &str) -> (&str, bool) {
    let is_close = tag.starts_with('/');
    let rest = tag.strip_prefix('/').unwrap_or(tag);
    let name = rest
        .split(|c: char| c.is_whitespace() || c == '/')
        .find(|token| !token.is_empty())
        .unwrap_or("");
    (name, is_close)
}

fn decode_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => Some("&".to_owned()),
        "lt" => Some("<".to_owned()),
        "gt" => Some(">".to_owned()),
        "quot" => Some("\"".to_owned()),
        "apos" => Some("'".to_owned()),
        _ => {
            let digits = entity.strip_prefix('#')?;
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse::<u32>().ok()?
            };
            char::from_u32(code).map(String::from)
        }
    }
}

/// Strip tags and decode entities from a TEI fragment, materializing a newline
/// at each block boundary. Excluded visual/callout subtrees are skipped.
fn strip_tags_and_decode(fragment: &str) -> String {
    let mut out = String::new();
    let mut excluded: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < fragment.len() {
        let ch = fragment[index..].chars().next().expect("in-bounds char");
        if ch == '<' {
            let rest = &fragment[index..];
            let close = match rest.find('>') {
                Some(close) => close,
                None => break,
            };
            let tag = &rest[1..close];
            let (name, is_close) = tag_name(tag);
            if is_close {
                if excluded.last().map(String::as_str) == Some(name) {
                    excluded.pop();
                }
            } else if EXCLUDED_SUBTREES.contains(&name) {
                if !tag.ends_with('/') {
                    excluded.push(name.to_owned());
                }
            } else if excluded.is_empty() && BLOCK_TAGS.contains(&name) {
                out.push('\n');
            }
            index += close + 1;
        } else if ch == '&' && excluded.is_empty() {
            let rest = &fragment[index..];
            if let Some(semi) = rest.find(';')
                && let Some(decoded) = decode_entity(&rest[1..semi])
            {
                out.push_str(&decoded);
                index += semi + 1;
                continue;
            }
            out.push(ch);
            index += ch.len_utf8();
        } else {
            if excluded.is_empty() {
                out.push(ch);
            }
            index += ch.len_utf8();
        }
    }
    out
}

/// Extract the `<body>…</body>` text from a TEI document.
fn extract_body_text(tei: &str) -> Option<String> {
    let start = tei.find("<body")?;
    let content_start = tei[start..].find('>')? + start + 1;
    let end = tei[content_start..].find("</body>")? + content_start;
    Some(strip_tags_and_decode(&tei[content_start..end]))
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

// ───────────────────────────── SUT input plane ─────────────────────────────

struct Plane {
    document: LayoutDocument,
    lattice: LayoutLattice,
    lexemes: Vec<LexemeRow>,
}

/// Build a coarse-span layout plane from `text`: one lexical span per
/// newline-delimited block plus the newline separator tokens, so a verbatim
/// join reproduces `text` exactly. This mirrors the positioned donor's lexical
/// unit (the show-operator span may carry a whole line), not a word.
fn plane_from_text(document_id: &str, text: &str) -> Plane {
    let provider = LayoutProvider::new("preserved-tei-projection", "1", "a".repeat(64)).unwrap();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let segments: Vec<&str> = text.split('\n').collect();
    let mut ordinal = 0u32;
    let mut push = |segment: &str, tokens: &mut Vec<LayoutToken>, lexemes: &mut Vec<LexemeRow>| {
        let text_digest = digest(segment);
        let y = 10.0 + ordinal as f64 * 12.0;
        let bbox = LayoutBBox::new(10.0, y, 590.0, y + 10.0).unwrap();
        tokens.push(
            LayoutToken::new(
                0,
                ordinal,
                ordinal,
                bbox,
                text_digest.clone(),
                LayoutSpan::present(ordinal, ordinal + 1).unwrap(),
                LayoutTokenKind::from_lexical_text(segment),
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap(),
        );
        lexemes.push(LexemeRow::new(
            document_id,
            "preserved-tei-projection",
            0,
            ordinal,
            text_digest,
            segment,
        ));
        ordinal += 1;
    };
    for (position, segment) in segments.iter().enumerate() {
        if position > 0 {
            push("\n", &mut tokens, &mut lexemes);
        }
        push(segment, &mut tokens, &mut lexemes);
    }

    let count = tokens.len() as u32;
    let height = 12.0 * count as f64 + 40.0;
    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, height, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
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
    let block_box = LayoutBBox::new(0.0, 0.0, 600.0, height).unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        None,
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 600.0,
            height,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: block_box,
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, count).unwrap(),
                    bbox: block_box,
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    Plane {
        document,
        lattice,
        lexemes,
    }
}

/// A segmentation that assigns every token to `region`, in reading order.
fn segmentation_for(plane: &Plane, region: FullSemanticRegion) -> FullSegmentationOutcome {
    let provider_id = plane.document.pages[0].tokens[0].provider.id.clone();
    let assignments = plane
        .lexemes
        .iter()
        .enumerate()
        .map(|(ordinal, lexeme)| FullTokenAssignment {
            location: FullLocation {
                page_ordinal: lexeme.page_ordinal,
                block_ordinal: 0,
                line_ordinal: ordinal as u32,
                token_ordinal: ordinal as u32,
            },
            source_ordinal: lexeme.source_ordinal,
            text_digest: lexeme.text_digest.clone(),
            region,
        })
        .collect();
    FullSegmentationOutcome {
        schema_version: FULL_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        identity: FullSegmentationIdentity {
            document_id: plane.document.document_id.clone(),
            provider_id,
            source_pdf_sha256: plane.document.source_pdf_sha256.clone(),
            lattice_fingerprint: plane.lattice.digest().unwrap(),
            config_fingerprint: "body-character-oracle".to_owned(),
        },
        assignments,
        runs: Vec::new(),
        furniture: Vec::new(),
        dispositions: Vec::new(),
        order_digest: "body-character-oracle".to_owned(),
    }
}

fn native_body(plane: &Plane) -> Result<NativeBodyText, NativeBodyTextError> {
    let segmentation = segmentation_for(plane, FullSemanticRegion::Body);
    build_native_body_text(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &segmentation,
    )
}

fn oracle_for(text: &str) -> BodyOracleCounts {
    BodyOracleCounts {
        generator_id: ORACLE_GENERATOR.to_owned(),
        lengths: NativeBodyLengths::measure(text),
    }
}

fn independent_provenance() -> BodyOracleProvenance {
    BodyOracleProvenance {
        generator_id: ORACLE_GENERATOR.to_owned(),
        independence: BodyOracleIndependence::Independent,
    }
}

fn measure_verdict(text: &str, oracle_text: &str) -> BodyCoverageGap {
    let plane = plane_from_text("oracle-case", text);
    let native = native_body(&plane).expect("the plane assembles");
    classify_body_coverage(
        "oracle-case",
        &native,
        &oracle_for(oracle_text),
        &independent_provenance(),
    )
    .expect("independent oracle adjudicates")
}

// ────────────────────────────────── tests ──────────────────────────────────

#[test]
fn tracked_tei_projection_is_only_an_assembly_conservation_check() {
    // LIMITED CLAIM SCOPE. The plane is a tracked-TEI projection, so this can
    // only show that the SUT assembly conserves the characters it is given. It
    // MUST NOT emit a body-gap verdict from TEI-projected inputs; the real
    // measurement is the detached probe
    // `crates/da-cli/examples/native_body_character_probe.rs`.
    let mut oracle_total_raw = 0usize;
    let mut native_total_raw = 0usize;
    for (document_id, file, pin) in PRESERVED {
        let tei = pinned_source(file, pin).expect("the preserved fixture matches its pin");
        let body = extract_body_text(&tei).expect("the fixture carries a <body>");
        let plane = plane_from_text(document_id, &body);
        let native = native_body(&plane).expect("the native body assembles");
        assert_eq!(
            native.text().len(),
            body.len(),
            "{document_id}: the assembly must conserve every character it was given"
        );
        let oracle = NativeBodyLengths::measure(&body);
        println!(
            "body-assembly-conservation {document_id}: oracle raw {} native raw {} oracle zero-width {} native zero-width {}",
            oracle.raw,
            native.lengths.raw,
            oracle.zero_width_normalized,
            native.lengths.zero_width_normalized,
        );
        oracle_total_raw += oracle.raw;
        native_total_raw += native.lengths.raw;
    }
    assert_eq!(native_total_raw, oracle_total_raw);
    println!(
        "body-assembly-conservation aggregate: oracle raw {oracle_total_raw} native raw {native_total_raw}"
    );
}

#[test]
fn aggregate_fold_reports_the_worst_verdict() {
    let full = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".repeat(40);
    let loss = measure_verdict(&full[..full.len() / 2], &full);
    let granular = measure_verdict(&full, &full);
    assert_eq!(granular.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(loss.verdict, BodyCoverageVerdict::ContentLoss);
    let aggregate = aggregate_body_coverage(&[granular, loss]);
    assert_eq!(aggregate.documents, 2);
    assert_eq!(aggregate.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(aggregate.d498_reopen_required);
    assert_eq!(aggregate.thresholds, "not_accepted");
}

#[test]
fn content_loss_is_distinguishable_and_raises_the_d498_flag() {
    let full = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".repeat(40);
    let truncated = &full[..full.len() / 2];
    let gap = measure_verdict(truncated, &full);
    assert!(gap.native.zero_width_normalized < gap.oracle.zero_width_normalized);
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(gap.coverage_milli.unwrap() < 980);
    assert!(
        gap.d498_reopen_required,
        "ContentLoss must require a D498 reopen"
    );

    // The same text is granularity-only, so the two verdicts are not conflated.
    let same = measure_verdict(&full, &full);
    assert_eq!(same.verdict, BodyCoverageVerdict::GranularityOnly);
    assert!(!same.d498_reopen_required);
}

#[test]
fn a_same_generator_oracle_is_refused_by_name() {
    let declared = BodyOracleProvenance {
        generator_id: ORACLE_GENERATOR.to_owned(),
        independence: BodyOracleIndependence::SameGeneratorAsSut,
    };
    assert_eq!(
        require_independent_body_oracle(&declared),
        Err(NativeBodyTextError::OracleNotIndependent)
    );
    let named_sut = BodyOracleProvenance {
        generator_id: NATIVE_BODY_TEXT_GENERATOR_ID.to_owned(),
        independence: BodyOracleIndependence::Independent,
    };
    assert_eq!(
        require_independent_body_oracle(&named_sut),
        Err(NativeBodyTextError::OracleNotIndependent)
    );

    // And the classifier refuses the same oracle rather than issuing a verdict.
    let plane = plane_from_text("same-generator", "body text");
    let native = native_body(&plane).unwrap();
    assert_eq!(
        classify_body_coverage(
            "same-generator",
            &native,
            &oracle_for("body text"),
            &named_sut
        ),
        Err(NativeBodyTextError::OracleNotIndependent)
    );
}

#[test]
fn an_empty_oracle_is_not_assessable_not_a_zero_zero_pass() {
    let plane = plane_from_text("empty-oracle", "body text");
    let native = native_body(&plane).unwrap();
    let gap = classify_body_coverage(
        "empty-oracle",
        &native,
        &oracle_for(""),
        &independent_provenance(),
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.coverage_milli, None);
    assert!(!gap.d498_reopen_required);

    // An empty <body> is a typed absence too, not a 0/0 pass.
    let empty = extract_body_text("<TEI><body></body></TEI>").unwrap();
    assert!(empty.is_empty());
    assert_eq!(NativeBodyLengths::measure(&empty).raw, 0);
}

#[test]
fn an_empty_native_region_is_a_typed_absence() {
    // Every token is attributed to Header, so the requested Body region admits
    // nothing: a typed NoBodyTokens, never a zero-character success.
    let plane = plane_from_text("empty-region", "body text");
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Header);
    assert_eq!(
        build_native_body_text(
            &plane.document,
            &plane.lattice,
            &plane.lexemes,
            &segmentation
        ),
        Err(NativeBodyTextError::NoBodyTokens)
    );
}

#[test]
fn a_tampered_fixture_pin_is_refused() {
    let (_, file, _) = PRESERVED[0];
    let zeroed = "0".repeat(64);
    assert!(pinned_source(file, &zeroed).is_err());
    assert!(pinned_source(file, PRESERVED[0].2).is_ok());
}

#[test]
fn the_serialized_body_text_carries_counts_but_not_characters() {
    let secret = "UNIQUE-ORACLE-MARKER-STRING";
    let plane = plane_from_text("redaction", secret);
    let native = native_body(&plane).unwrap();
    assert_eq!(native.schema_version, NATIVE_BODY_TEXT_SCHEMA_VERSION);
    let json = serde_json::to_string(&native).unwrap();
    assert!(!json.contains(secret), "the serialized body text leaked");
    assert!(json.contains("text_digest"));
    assert!(json.contains("whitespace_collapsed"));
    assert!(!format!("{native:?}").contains(secret));
}

#[test]
fn the_normalization_ladder_has_named_rungs() {
    let raw = "a\u{200b}b\u{fb01} c\u{00ad}d";
    assert_eq!(BodyNormalization::Raw.apply(raw), raw);
    assert_eq!(
        BodyNormalization::WhitespaceCollapsed.apply(" a\tb  c "),
        "a b c"
    );
    assert_eq!(
        BodyNormalization::ZeroWidthAndLigatureNormalized.apply(raw),
        "abfi cd"
    );
    let lengths = NativeBodyLengths::measure(raw);
    assert_eq!(lengths.raw, raw.chars().count());
    assert!(lengths.zero_width_normalized < lengths.raw);
}
