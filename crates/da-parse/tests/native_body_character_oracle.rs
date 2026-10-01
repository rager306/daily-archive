//! Character-coverage oracle for the native body text (S05/T01).
//!
//! S03 named but did not resolve the token-coverage gap between the
//! `pdf_inspector_positioned` lane and the `pdfalto` lane (aggregate ×3.29 over
//! 202 pages, `doc/review/grobid-absorption-s03-spike.md` §3.5). Tokens cannot
//! separate granularity from real loss, and neither can a length, a ratio, or a
//! total. This test is the structurally independent oracle:
//!
//! 1. a **lexical scanner** (below, outside the SUT, importing no production
//!    helper) reads the three sha256-pinned preserved TEI fixtures, verifies the
//!    pin against the bytes, extracts the body prose and counts **characters**
//!    at each rung of the normalization ladder;
//! 2. the SUT assembles the native body text from a layout plane and the two
//!    sides are compared through the **scope-bound lexical unit alignment**, not
//!    through counts;
//! 3. the typed verdict (`GranularityOnly` / `ContentLoss` / `NotAssessable`),
//!    the D498 reopen flag, the P1/P2 separation, the population completeness,
//!    and the negative mutations are asserted by name.
//!
//! The layout plane here is a **tracked-TEI projection** and this test's claim
//! scope is therefore limited to **assembly conservation**: it shows the SUT
//! joins and normalizes the characters it is given without loss, and it asserts
//! the verdict machinery on **synthetic** planes. It deliberately does NOT issue
//! a body-gap verdict from TEI-projected inputs. The real
//! `pdf_inspector_positioned` measurement runs in the detached probe
//! `crates/da-cli/examples/native_body_character_probe.rs` (the donor lives in
//! `da-adapters` and is unreachable from a `da-parse` test) and is written to
//! `artifacts/parse_compat/native_body_character_scope_bound.json`.

use da_parse::{
    BODY_ABSENCE_POLICY, BodyAggregateInput, BodyBoundaryVerification, BodyCasePolicy,
    BodyCoverageGap, BodyCoverageVerdict, BodyDocumentOutcome, BodyGapReason, BodyJoinPolicy,
    BodyNormalization, BodyOracleCounts, BodyOracleIndependence, BodyOracleProvenance,
    BodyPlaneVerdict, BodyScope, BodyScopeClass, BodyScopeSide, BodyThirdRouteVerdict,
    BodyUnitInventory, BodyUnitRung, CoordinateDialect, FULL_SEGMENTATION_SCHEMA_VERSION,
    FullLocation, FullSegmentationIdentity, FullSegmentationOutcome, FullSemanticRegion,
    FullTokenAssignment, FurnitureEvidence, HeadingMatch, LatticeBlock, LatticeLine, LatticePage,
    LatticeToken, LayoutBBox, LayoutDocument, LayoutLattice, LayoutPage, LayoutProvider,
    LayoutSpan, LayoutStyleHints, LayoutToken, LayoutTokenKind, LexemeRow,
    NATIVE_BODY_TEXT_GENERATOR_ID, NATIVE_BODY_TEXT_SCHEMA_VERSION, NativeBodyLengths,
    NativeBodyText, NativeBodyTextError, NativeBodyThirdRouteCandidate, ReadingOrder,
    SegmentationConfig, aggregate_body_coverage, build_native_body_assignment,
    build_native_body_text, classify_body_coverage, classify_body_plane,
    evaluate_body_third_route_candidate, localize_body_plane_checked, plane_verdict_for_population,
    require_independent_body_oracle, scope_covers, segment_document,
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

/// The tokenization rung the SUT and the oracle are measured at. It is declared
/// on every [`BodyScope`] and every inventory is built at the same rung, so the
/// comparison cannot silently apply a rung the scope did not name.
fn rung() -> BodyUnitRung {
    BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    )
}

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
    plane_from_text_with_source_ordinals(document_id, text, |index| index as u32)
}

/// Like [`plane_from_text`], but the donor's extraction `source_ordinal` of the
/// token at each reading-order index is supplied separately. The document and
/// the lexeme sidecar keep that (possibly permuted) identity while the reading
/// order — `token_ordinal`, geometry and lexeme text — stays text order. The
/// lattice is ordered by `source_ordinal` because a lattice line must be
/// monotonic in it; nothing in the heading route reads the lattice token order.
fn plane_from_text_with_source_ordinals(
    document_id: &str,
    text: &str,
    source_ordinal_of: impl Fn(usize) -> u32,
) -> Plane {
    let provider = LayoutProvider::new("preserved-tei-projection", "1", "a".repeat(64)).unwrap();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let segments: Vec<&str> = text.split('\n').collect();
    let mut ordinal = 0u32;
    let mut push = |segment: &str, tokens: &mut Vec<LayoutToken>, lexemes: &mut Vec<LexemeRow>| {
        let text_digest = digest(segment);
        let source_ordinal = source_ordinal_of(ordinal as usize);
        let y = 10.0 + ordinal as f64 * 12.0;
        let bbox = LayoutBBox::new(10.0, y, 590.0, y + 10.0).unwrap();
        tokens.push(
            LayoutToken::new(
                0,
                source_ordinal,
                ordinal,
                bbox,
                text_digest.clone(),
                LayoutSpan::present(source_ordinal, source_ordinal + 1).unwrap(),
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
            source_ordinal,
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
    let mut lattice_tokens: Vec<LatticeToken> = tokens
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
    lattice_tokens.sort_by_key(|token| token.source_ordinal);
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

/// A segmentation that assigns every token by `region_of(ordinal)`.
fn segmentation_with(
    plane: &Plane,
    region_of: impl Fn(u32) -> FullSemanticRegion,
) -> FullSegmentationOutcome {
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
            region: region_of(ordinal as u32),
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

fn segmentation_for(plane: &Plane, region: FullSemanticRegion) -> FullSegmentationOutcome {
    segmentation_with(plane, |_| region)
}

fn native_body(plane: &Plane) -> Result<NativeBodyText, NativeBodyTextError> {
    let segmentation = segmentation_for(plane, FullSemanticRegion::Body);
    build_native_body_text(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &segmentation,
        rung(),
    )
}

/// The native body text when only the first `body_tokens` lexemes are Body and
/// the remainder are References: the plane keeps everything, the body does not.
fn native_body_prefix(
    plane: &Plane,
    body_tokens: u32,
) -> Result<NativeBodyText, NativeBodyTextError> {
    let segmentation = segmentation_with(plane, |ordinal| {
        if ordinal < body_tokens {
            FullSemanticRegion::Body
        } else {
            FullSemanticRegion::References
        }
    });
    build_native_body_text(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &segmentation,
        rung(),
    )
}

fn body_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativeBodyRegion,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::VerbatimTokenConcat,
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    )
}

fn body_word_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativeBodyRegion,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    )
}

fn native_plane_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativePlane,
        vec![
            BodyScopeClass::BodyProse,
            BodyScopeClass::VisualEvidence,
            BodyScopeClass::References,
            BodyScopeClass::BackMatter,
            BodyScopeClass::PageFurniture,
            BodyScopeClass::HeaderMetadata,
        ],
        BodyJoinPolicy::VerbatimTokenConcat,
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    )
}

fn oracle_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::PreservedBodyProse,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::VerbatimTokenConcat,
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
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

/// Classify a synthetic plane: the whole native text is the plane, and the body
/// is assembled from all of it. `oracle_text` is independent of the plane.
fn gap_for(document_id: &str, native_text: &str, oracle_text: &str) -> BodyCoverageGap {
    let plane = plane_from_text(document_id, native_text);
    let native = native_body(&plane).expect("the plane assembles");
    let native_plane_units = BodyUnitInventory::from_text(native_text, rung());
    let oracle_units = BodyUnitInventory::from_text(oracle_text, rung());
    classify_body_coverage(
        document_id,
        &native,
        &native_plane_units,
        &oracle_for(oracle_text),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .expect("independent oracle adjudicates")
}

/// Classify a synthetic plane at an explicit rung. Both inventories and both
/// scopes carry the rung, so the comparison is honest at either rung and the
/// result reflects tokenization, not a rung-mismatch refusal.
fn gap_for_at_rung(
    document_id: &str,
    native_text: &str,
    oracle_text: &str,
    rung: BodyUnitRung,
) -> BodyCoverageGap {
    let plane = plane_from_text(document_id, native_text);
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Body);
    let native = build_native_body_text(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &segmentation,
        rung,
    )
    .expect("the plane assembles at the declared rung");
    let native_plane_units = BodyUnitInventory::from_text(native_text, rung);
    let oracle_units = BodyUnitInventory::from_text(oracle_text, rung);
    let mut native_scope = body_scope();
    native_scope.normalization = rung.normalization;
    native_scope.case = rung.case;
    let mut oracle_scope = oracle_scope();
    oracle_scope.normalization = rung.normalization;
    oracle_scope.case = rung.case;
    classify_body_coverage(
        document_id,
        &native,
        &native_plane_units,
        &oracle_for(oracle_text),
        &oracle_units,
        &independent_provenance(),
        &native_scope,
        &oracle_scope,
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
fn granularity_only_requires_every_oracle_unit_in_the_native_body() {
    let text = "alpha beta gamma delta epsilon";
    let gap = gap_for("matched", text, text);
    assert_eq!(gap.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(gap.reason, BodyGapReason::Matched);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.in_native_body, gap.alignment.oracle_units);
    assert!(!gap.d498_reopen_required);
    assert_eq!(gap.thresholds, "not_accepted");
    // Occurrence conservation: the five classes are a partition of the oracle.
    assert_eq!(
        gap.alignment.in_native_plane
            + gap.alignment.boundary_variant_from_plane
            + gap.alignment.uncertain_present_in_token_from_plane
            + gap.alignment.uncertain_spanning_tokens_from_plane
            + gap.alignment.unresolved_from_plane,
        gap.alignment.oracle_units
    );
}

#[test]
fn equal_length_different_content_is_content_loss_not_granularity_only() {
    // Equal character totals, entirely different content: a length rule would
    // call this conservation; the unit alignment locates the loss.
    let native = "aaaaaaaaaa bbbbbbbbbb";
    let oracle = "cccccccccc dddddddddd";
    assert_eq!(native.chars().count(), oracle.chars().count());
    let gap = gap_for("equal-length", native, oracle);
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 2 }
    );
    assert!(gap.d498_reopen_required);
}

#[test]
fn a_surplus_cannot_mask_a_selective_loss() {
    // The native side is LONGER than the oracle yet missing `beta`: a
    // total/surplus rule would report a clean surplus, the alignment reports loss.
    let native = "alpha gamma delta epsilon zeta eta";
    let oracle = "alpha beta gamma";
    assert!(native.chars().count() > oracle.chars().count());
    let gap = gap_for("surplus", native, oracle);
    assert!(
        gap.native.zero_width_normalized > gap.oracle.zero_width_normalized,
        "the native side must genuinely be the longer one"
    );
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(matches!(
        gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    ));
    assert!(gap.d498_reopen_required);
}

#[test]
fn content_in_the_plane_but_not_in_the_body_is_a_region_gap_not_content_loss() {
    // The whole native plane carries the oracle text; only a prefix is Body, so
    // the deficit is a P2 assignment gap, not P1 loss.
    let full = "alpha beta\ngamma delta";
    let plane = plane_from_text("p2-case", full);
    let native = native_body_prefix(&plane, 2).expect("the prefix body assembles");
    let plane_units = BodyUnitInventory::from_text(full, rung());
    let oracle_units = BodyUnitInventory::from_text(full, rung());
    let gap = classify_body_coverage(
        "p2-case",
        &native,
        &plane_units,
        &oracle_for(full),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .expect("independent oracle adjudicates");
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert!(gap.alignment.in_plane_not_in_body > 0);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(matches!(
        gap.reason,
        BodyGapReason::P2RegionAssignmentGap { .. }
    ));
    assert!(!gap.d498_reopen_required);
}

#[test]
fn a_plane_absent_occurrence_is_not_recounted_as_a_body_deficit() {
    // `gamma` is absent from the plane (P1) and from the body; `beta` is present
    // in the plane but absent from the body (P2). The occurrence-conserving
    // deficit counts only `beta`: an occurrence consumed as P1 is never
    // re-counted as P2, and an unbound occurrence is uncertainty rather than a
    // deficit.
    let plane = plane_from_text("consumed-once", "alpha\nbeta");
    let native = native_body_prefix(&plane, 1).expect("the prefix body assembles");
    let plane_units = BodyUnitInventory::from_text("alpha\nbeta", rung());
    let oracle_units = BodyUnitInventory::from_text("alpha beta gamma", rung());
    let gap = classify_body_coverage(
        "consumed-once",
        &native,
        &plane_units,
        &oracle_for("alpha beta gamma"),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.alignment.unresolved_from_plane, 1);
    assert_eq!(gap.alignment.unresolved_from_body, 2);
    assert_eq!(gap.alignment.in_plane_not_in_body, 1);
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    );
    assert!(gap.d498_reopen_required);
    assert_eq!(gap.plane_verdict.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        gap.body_verdict.reason,
        BodyGapReason::P2RegionAssignmentGap {
            units_in_plane_not_in_body: 1,
        }
    );
}

#[test]
fn a_narrower_native_scope_is_not_covered() {
    let text = "alpha beta gamma";
    // The oracle includes visual evidence; the native body scope does not.
    let mut narrow = body_scope();
    narrow.included_classes = vec![BodyScopeClass::BodyProse];
    let mut wide_oracle = oracle_scope();
    wide_oracle.included_classes = vec![BodyScopeClass::BodyProse, BodyScopeClass::VisualEvidence];
    assert!(!scope_covers(&narrow, &wide_oracle));
    let plane = plane_from_text("scope-case", text);
    let native = native_body(&plane).unwrap();
    let plane_units = BodyUnitInventory::from_text(text, rung());
    let oracle_units = BodyUnitInventory::from_text(text, rung());
    let gap = classify_body_coverage(
        "scope-case",
        &native,
        &plane_units,
        &oracle_for(text),
        &oracle_units,
        &independent_provenance(),
        &narrow,
        &wide_oracle,
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.reason, BodyGapReason::ScopeMismatch);

    // A native superset (the plane) is covered.
    assert!(scope_covers(&native_plane_scope(), &wide_oracle));
}

#[test]
fn aggregate_fold_reports_the_worst_verdict() {
    let full = "alpha beta gamma delta";
    let granular = gap_for("granular", full, full);
    let loss = gap_for("loss", "alpha beta", "alpha beta gamma delta");
    assert_eq!(granular.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(loss.verdict, BodyCoverageVerdict::ContentLoss);
    let input = BodyAggregateInput {
        corpus_id: "synthetic".to_owned(),
        declared_documents: vec!["granular".to_owned(), "loss".to_owned()],
        outcomes: vec![
            BodyDocumentOutcome::Measured(granular),
            BodyDocumentOutcome::Measured(loss),
        ],
    };
    let aggregate = aggregate_body_coverage(&input);
    assert_eq!(aggregate.documents_declared, 2);
    assert_eq!(aggregate.documents_contributing, 2);
    assert_eq!(aggregate.documents_absent, 0);
    assert!(aggregate.population_complete);
    assert_eq!(aggregate.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(aggregate.d498_reopen_required);
    assert_eq!(aggregate.thresholds, "not_accepted");
}

#[test]
fn an_incomplete_population_cannot_claim_a_clean_corpus() {
    let full = "alpha beta gamma delta";
    let granular = gap_for("a", full, full);
    let input = BodyAggregateInput {
        corpus_id: "synthetic".to_owned(),
        declared_documents: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
        outcomes: vec![BodyDocumentOutcome::Measured(granular)],
    };
    let aggregate = aggregate_body_coverage(&input);
    assert_eq!(aggregate.documents_declared, 3);
    assert_eq!(aggregate.documents_contributing, 1);
    assert_eq!(aggregate.documents_absent, 2);
    assert!(!aggregate.population_complete);
    let absent_ids: Vec<&str> = aggregate
        .absent
        .iter()
        .map(|entry| entry.document_id.as_str())
        .collect();
    assert_eq!(absent_ids, vec!["b", "c"]);
    assert_eq!(aggregate.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!aggregate.d498_reopen_required);

    // A real ContentLoss survives an incomplete population.
    let loss = gap_for("a", "alpha beta", full);
    let with_loss = BodyAggregateInput {
        corpus_id: "synthetic".to_owned(),
        declared_documents: vec!["a".to_owned()],
        outcomes: vec![
            BodyDocumentOutcome::Measured(loss),
            BodyDocumentOutcome::Absent {
                document_id: "b".to_owned(),
                reason: "unknown_document_binding".to_owned(),
            },
        ],
    };
    let aggregate = aggregate_body_coverage(&with_loss);
    assert!(!aggregate.population_complete);
    assert_eq!(aggregate.documents_absent, 1);
    assert_eq!(aggregate.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(aggregate.d498_reopen_required);
}

#[test]
fn an_unknown_document_is_absent_not_silently_dropped() {
    let input = BodyAggregateInput {
        corpus_id: "synthetic".to_owned(),
        declared_documents: vec!["a".to_owned()],
        outcomes: vec![BodyDocumentOutcome::Absent {
            document_id: "zzz".to_owned(),
            reason: "unknown_document_binding".to_owned(),
        }],
    };
    let aggregate = aggregate_body_coverage(&input);
    let ids: Vec<&str> = aggregate
        .absent
        .iter()
        .map(|entry| entry.document_id.as_str())
        .collect();
    assert!(
        ids.contains(&"zzz"),
        "the undeclared outcome must be visible"
    );
    assert!(
        ids.contains(&"a"),
        "the declared document must be reported absent"
    );
    assert!(!aggregate.population_complete);
    assert_eq!(aggregate.verdict, BodyCoverageVerdict::NotAssessable);
}

#[test]
fn a_boundary_variant_is_located_as_granularity_not_loss() {
    // The oracle merged two words the native lane keeps separate: the units are
    // located as a boundary variant, so the gap is granularity, not loss.
    let oracle_merged = gap_for("oracle-merged", "average case", "averagecase");
    assert_eq!(oracle_merged.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(oracle_merged.alignment.unresolved_from_plane, 0);
    assert_eq!(oracle_merged.alignment.boundary_variant_from_plane, 1);

    // And the reverse: the native lane merged two words the oracle keeps separate.
    let native_merged = gap_for("native-merged", "averagecase", "average case");
    assert_eq!(native_merged.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(native_merged.alignment.unresolved_from_plane, 0);
    assert_eq!(native_merged.alignment.boundary_variant_from_plane, 2);
}

// ───────── occurrence-conserving / token-boundary-grounded negatives ─────────

#[test]
fn an_arbitrary_substring_inside_a_token_is_uncertain_not_loss() {
    // `he` sits inside `the`, but `the` is not a tile of `he`: the characters are
    // present, the token boundary is not. Neither loss nor granularity follows.
    let gap = gap_for("he-the", "the", "he");
    assert_eq!(gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_from_plane(), 1);
    assert_eq!(gap.alignment.uncertain_present_in_token_from_plane, 1);
    assert_eq!(gap.alignment.uncertain_spanning_tokens_from_plane, 0);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(
        gap.reason,
        BodyGapReason::P1LocalizationUncertain {
            present_in_token_units: 1,
            spanning_token_units: 0,
            character_material_units: 0,
        }
    );
    assert!(!gap.d498_reopen_required);
}

#[test]
fn repeated_oracle_occurrences_are_not_excused_by_one_candidate() {
    // Three oracle occurrences of `the`, one native occurrence. A boundary bool
    // would excuse every residual occurrence; occurrence conservation consumes
    // the one token once and leaves two occurrences unresolved and uncertain.
    let gap = gap_for("repeated-the", "the", "the the the");
    assert_eq!(gap.alignment.oracle_units, 3);
    assert_eq!(gap.alignment.in_native_plane, 1);
    assert_eq!(gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_present_in_token_from_plane, 2);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!gap.d498_reopen_required);
}

#[test]
fn reordered_content_is_not_a_granularity_pass() {
    // The same units in a different order carry no content loss, but the ordered
    // locator does not prove coverage either: it stays NotAssessable.
    let gap = gap_for("reordered", "gamma alpha", "alpha gamma");
    assert_eq!(gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_present_in_token_from_plane, 1);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(matches!(
        gap.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
}

#[test]
fn genuine_boundaries_refuse_an_invented_fragment() {
    // The word plane knows each word is its own fragment. `averagecase` is then
    // not tileable from `average` + `case` (that would invent a boundary the
    // caller said exists), so it must not be reported as a boundary variant.
    let native = NativeBodyText::from_fragments(
        "word-plane",
        "c".repeat(64),
        2,
        None,
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        ["average", "case"],
        rung(),
    )
    .unwrap();
    let plane_units = BodyUnitInventory::from_fragments(["average", "case"], rung());
    let oracle_units = BodyUnitInventory::from_text("averagecase", rung());
    let gap = classify_body_coverage(
        "word-plane",
        &native,
        &plane_units,
        &oracle_for("averagecase"),
        &oracle_units,
        &independent_provenance(),
        &body_word_scope(),
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_from_plane(), 1);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);

    // Contrast: the same content inside one fragment does tile, and the existing
    // `a_boundary_variant_is_located_as_granularity_not_loss` proves it.
}

#[test]
fn a_normalization_rung_mismatch_is_not_assessable() {
    // Each inventory is tokenized at the rung its own scope declares; the two
    // scopes declare different rungs, so the sides are not comparable.
    let text = "alpha beta gamma";
    let plane = plane_from_text("normalization", text);
    let raw = BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded);
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Body);
    let native = build_native_body_text(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &segmentation,
        raw,
    )
    .unwrap();
    let plane_units = BodyUnitInventory::from_text(text, raw);
    let oracle_units = BodyUnitInventory::from_text(text, rung());
    let mut raw_native = body_scope();
    raw_native.normalization = BodyNormalization::Raw;
    let gap = classify_body_coverage(
        "normalization",
        &native,
        &plane_units,
        &oracle_for(text),
        &oracle_units,
        &independent_provenance(),
        &raw_native,
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.reason, BodyGapReason::NormalizationMismatch);
    assert!(!gap.d498_reopen_required);
}

#[test]
fn an_inventory_tokenized_at_another_rung_is_refused() {
    // The inventory carries its rung, so a caller cannot declare one rung on the
    // scope and tokenize at another; the mismatch is a typed refusal, not a
    // silently wrong comparison.
    let text = "alpha beta";
    let plane = plane_from_text("rung-refusal", text);
    let native = native_body(&plane).unwrap();
    let plane_units = BodyUnitInventory::from_text(text, rung());
    let raw = BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded);
    let oracle_units = BodyUnitInventory::from_text(text, raw);
    let error = classify_body_coverage(
        "rung-refusal",
        &native,
        &plane_units,
        &oracle_for(text),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        NativeBodyTextError::InventoryRungMismatch { side: "oracle" }
    );
}

#[test]
fn a_checked_plane_localization_refuses_a_normalization_rung_mismatch() {
    // The P1 path carries no scope, so the checked entry point compares the two
    // inventories' own rungs. The same characters at Raw and at the folded rung
    // tokenize differently; the pair is not comparable and the checked path
    // refuses instead of localizing across the rungs.
    let raw = BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded);
    let plane = BodyUnitInventory::from_text("a\u{200b}b", raw);
    let oracle = BodyUnitInventory::from_text("ab", rung());
    assert_ne!(plane.rung(), oracle.rung());
    assert_eq!(
        localize_body_plane_checked(&plane, &oracle),
        Err(NativeBodyTextError::PlaneOracleRungMismatch)
    );
}

#[test]
fn a_checked_plane_localization_refuses_a_case_policy_mismatch() {
    // Same normalization rung, different case policy: the inventories are still
    // not comparable, and the checked path refuses the pair rather than
    // tokenizing one side the other never declared.
    let folded = BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    );
    let sensitive = BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseSensitive,
    );
    let plane = BodyUnitInventory::from_text("alpha beta", folded);
    let oracle = BodyUnitInventory::from_text("Alpha Beta", sensitive);
    assert_eq!(plane.rung().normalization, oracle.rung().normalization);
    assert_ne!(plane.rung().case, oracle.rung().case);
    assert_eq!(
        localize_body_plane_checked(&plane, &oracle),
        Err(NativeBodyTextError::PlaneOracleRungMismatch)
    );
}

#[test]
fn a_case_policy_mismatch_is_not_assessable() {
    // Both inventories sit at the rung they were built at; the two scopes declare
    // different case policies, so the sides are not comparable.
    let text = "alpha beta";
    let plane = plane_from_text("case-mismatch", text);
    let native = native_body(&plane).unwrap();
    let plane_units = BodyUnitInventory::from_text(text, rung());
    let sensitive = BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseSensitive,
    );
    let oracle_units = BodyUnitInventory::from_text("Alpha Beta", sensitive);
    let mut sensitive_oracle = oracle_scope();
    sensitive_oracle.case = BodyCasePolicy::CaseSensitive;
    let gap = classify_body_coverage(
        "case-mismatch",
        &native,
        &plane_units,
        &oracle_for("Alpha Beta"),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &sensitive_oracle,
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.reason, BodyGapReason::NormalizationMismatch);
}

#[test]
fn the_declared_rung_changes_tokenization_and_the_located_class() {
    // Same bytes, two declared rungs. At Raw the interior zero-width control
    // separates two units; at the normalized rung it is removed and the two join.
    // The rung therefore changes the tokenization and the located class, not just
    // whether the verdict is assessable.
    let native_text = "a\u{200b}b";
    let oracle_text = "ab";
    let raw = BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded);
    assert_eq!(
        BodyUnitInventory::from_text(native_text, raw).total(),
        2,
        "the zero-width control must split the run at Raw"
    );
    assert_eq!(
        BodyUnitInventory::from_text(native_text, rung()).total(),
        1,
        "the declared rung removes the zero-width control"
    );

    let raw_gap = gap_for_at_rung("rung-raw", native_text, oracle_text, raw);
    assert_eq!(raw_gap.alignment.in_native_plane, 0);
    assert_eq!(raw_gap.alignment.boundary_variant_from_plane, 1);
    assert_eq!(raw_gap.verdict, BodyCoverageVerdict::GranularityOnly);

    let normalized_gap = gap_for_at_rung("rung-normalized", native_text, oracle_text, rung());
    assert_eq!(normalized_gap.alignment.in_native_plane, 1);
    assert_eq!(normalized_gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(normalized_gap.verdict, BodyCoverageVerdict::GranularityOnly);
}

#[test]
fn a_ligature_is_material_only_at_the_rung_that_folds_it() {
    // The declared rung decides whether `ﬁ` counts as `fi` material. At Raw the
    // ligature is its own unit and the oracle's `f` is genuinely absent from the
    // ASCII material; at the ligature rung it expands and matches exactly.
    let native_text = "a\u{fb01}b";
    let oracle_text = "afib";
    let raw = BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded);

    let raw_gap = gap_for_at_rung("ligature-raw", native_text, oracle_text, raw);
    assert_eq!(raw_gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        raw_gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    );
    assert!(raw_gap.d498_reopen_required);

    let folded_gap = gap_for_at_rung("ligature-folded", native_text, oracle_text, rung());
    assert_eq!(folded_gap.verdict, BodyCoverageVerdict::GranularityOnly);
    assert!(!folded_gap.d498_reopen_required);
}

#[test]
fn explicit_case_policy_is_declared_not_inferred() {
    // `alpha` vs `Alpha` is a case question the scope must answer. Case-folded,
    // the units coincide exactly; case-sensitive, the uppercase ASCII material is
    // genuinely absent, so the declared policy decides.
    let native_text = "alpha beta";
    let oracle_text = "Alpha Beta";
    let folded = BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseFolded,
    );
    let sensitive = BodyUnitRung::new(
        BodyNormalization::ZeroWidthAndLigatureNormalized,
        BodyCasePolicy::CaseSensitive,
    );

    let folded_gap = gap_for_at_rung("case-folded", native_text, oracle_text, folded);
    assert_eq!(folded_gap.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(folded_gap.alignment.in_native_plane, 2);

    let sensitive_gap = gap_for_at_rung("case-sensitive", native_text, oracle_text, sensitive);
    assert_eq!(sensitive_gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(sensitive_gap.d498_reopen_required);
}

#[test]
fn greedy_exact_anchors_are_conservative_when_a_tile_would_win() {
    // Two-phase greedy decomposition, not a joint optimum: the exact anchor on
    // `x` consumes the side token that a joint exact-plus-tile optimum would have
    // tiled (`x y` onto `xy`). The conservative outcome is uncertainty, never a
    // resolution fabricated by undoing the anchor.
    let gap = gap_for("greedy-anchor", "x xy", "x y");
    assert_eq!(gap.alignment.in_native_plane, 1, "only `x` anchors exactly");
    assert_eq!(gap.alignment.boundary_variant_from_plane, 0);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_present_in_token_from_plane, 1);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(
        gap.reason,
        BodyGapReason::P1LocalizationUncertain {
            present_in_token_units: 1,
            spanning_token_units: 0,
            character_material_units: 0,
        }
    );
    assert!(!gap.d498_reopen_required);
    // The tile the joint optimum would use is genuinely available: the oracle run
    // `x y` concatenates to the side token `xy`.
    assert_eq!(BodyUnitInventory::from_text("x y", rung()).total(), 2);
    assert_eq!(
        BodyUnitInventory::from_text("x xy", rung()).count_of("xy"),
        1
    );
}

#[test]
fn a_zero_absent_plane_with_uncertainty_is_not_preservation() {
    // The measured corpus shape: no absent material, but occurrences are present
    // only as characters. A zero absent count must never read as preservation.
    let plane = BodyUnitInventory::from_text("the", rung());
    let oracle = BodyUnitInventory::from_text("he he", rung());
    let localization =
        localize_body_plane_checked(&plane, &oracle).expect("same-rung plane and oracle localize");
    assert_eq!(localization.absent_from_plane, 0);
    assert!(localization.uncertain_from_plane() > 0);
    let verdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(matches!(
        verdict.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
    assert!(!verdict.d498_reopen_required);
}

#[test]
fn a_proven_plane_material_deficit_is_still_content_loss() {
    let plane = BodyUnitInventory::from_text("alpha gamma", rung());
    let oracle = BodyUnitInventory::from_text("alpha beta", rung());
    let localization =
        localize_body_plane_checked(&plane, &oracle).expect("same-rung plane and oracle localize");
    assert_eq!(localization.absent_from_plane, 1);
    let verdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        verdict.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    );
    assert!(verdict.d498_reopen_required);
}

#[test]
fn a_fully_located_plane_is_the_only_plane_preservation() {
    let plane = BodyUnitInventory::from_text("alpha beta", rung());
    let oracle = BodyUnitInventory::from_text("alpha beta", rung());
    let localization =
        localize_body_plane_checked(&plane, &oracle).expect("same-rung plane and oracle localize");
    let verdict: BodyPlaneVerdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(verdict.reason, BodyGapReason::Matched);
    assert!(!verdict.d498_reopen_required);
}

#[test]
fn an_incomplete_plane_population_withholds_a_clean_reading() {
    let plane = BodyUnitInventory::from_text("alpha beta", rung());
    let oracle = BodyUnitInventory::from_text("alpha beta", rung());
    let localization =
        localize_body_plane_checked(&plane, &oracle).expect("same-rung plane and oracle localize");
    assert_eq!(
        classify_body_plane(&localization).verdict,
        BodyCoverageVerdict::GranularityOnly
    );
    let withheld = plane_verdict_for_population(&localization, 3, 1);
    assert_eq!(withheld.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(
        withheld.reason,
        BodyGapReason::PopulationIncomplete {
            documents_absent: 2
        }
    );
    assert!(!withheld.d498_reopen_required);
    // A complete population keeps the located plane reading.
    let complete = plane_verdict_for_population(&localization, 3, 3);
    assert_eq!(complete.verdict, BodyCoverageVerdict::GranularityOnly);
}

#[test]
fn a_non_ascii_or_encoding_difference_is_uncertain_not_loss() {
    // `café` against `cafe`: the declared rung does not fold diacritics, so the
    // difference is an encoding question, not proven character loss. The ASCII
    // material is all present, and the policy is named.
    let gap = gap_for("encoding", "cafe", "café");
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.alignment.uncertain_character_material_from_plane, 1);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!gap.d498_reopen_required);
    assert!(matches!(
        gap.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
    assert_eq!(
        BODY_ABSENCE_POLICY,
        "non_ascii_material_is_encoding_uncertainty_not_loss"
    );
}

#[test]
fn an_ascii_character_deficit_is_still_proven_loss() {
    // The contrast to the encoding case: a missing ASCII character is material
    // the extraction genuinely does not carry, so ContentLoss stays reachable.
    let gap = gap_for("ascii-deficit", "alpha gamma", "alpha beta gamma");
    assert_eq!(
        gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    );
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert!(gap.d498_reopen_required);
}

#[test]
fn plane_localization_carries_no_body_or_p2_field() {
    // P1 plane evidence is a separate type: it must not fabricate P2 fields for a
    // body that was never assembled.
    let plane = BodyUnitInventory::from_text("alpha beta", rung());
    let oracle = BodyUnitInventory::from_text("alpha beta gamma", rung());
    let localization =
        localize_body_plane_checked(&plane, &oracle).expect("same-rung plane and oracle localize");
    assert_eq!(localization.oracle_units, 3);
    assert_eq!(localization.exact_in_plane, 2);
    assert_eq!(localization.absent_from_plane, 1);
    let json = serde_json::to_string(&localization).unwrap();
    assert!(json.contains("absent_from_plane"));
    assert!(!json.contains("body"), "{json}");
    assert!(!json.contains("in_plane_not_in_body"), "{json}");
    assert!(!json.contains("unresolved_from_body"), "{json}");
}

#[test]
fn the_ordered_anchor_pass_scales_to_a_long_document() {
    // Positive control at document scale: an ordered side must resolve every
    // occurrence exactly, so the locator cannot be collapsing on length alone.
    let words: Vec<String> = (0..2_000).map(|index| format!("w{index:04}")).collect();
    let text = words.join(" ");
    let gap = gap_for("long-ordered", &text, &text);
    assert_eq!(gap.alignment.oracle_units, 2_000);
    assert_eq!(gap.alignment.in_native_plane, 2_000);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert_eq!(gap.verdict, BodyCoverageVerdict::GranularityOnly);
}

#[test]
fn a_large_reordered_side_is_not_granularity_and_not_loss() {
    // The side carries every oracle token but in an order the oracle never had.
    // The ordered locator must not call that granularity, and the characters are
    // present, so it must not call them loss either: it is uncertainty.
    let words: Vec<String> = (0..64).map(|index| format!("t{index:03}")).collect();
    let oracle_text = words.join(" ");
    let mut reversed = words.clone();
    reversed.reverse();
    let native_text = reversed.join(" ");
    let gap = gap_for("large-reordered", &native_text, &oracle_text);
    assert_ne!(gap.verdict, BodyCoverageVerdict::GranularityOnly);
    assert_eq!(gap.alignment.unresolved_from_plane, 0);
    assert!(gap.alignment.uncertain_from_plane() > 0);
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!gap.d498_reopen_required);
}

#[test]
fn a_missing_character_run_is_not_excused_by_the_boundary_locator() {
    // `averagecase` cannot be explained from `average` alone: the characters
    // after the prefix are absent, so the locator must report unresolved loss.
    let gap = gap_for("partial-merge", "average", "averagecase");
    assert_eq!(gap.verdict, BodyCoverageVerdict::ContentLoss);
    assert_eq!(
        gap.reason,
        BodyGapReason::P1NativeTextAbsent { missing_units: 1 }
    );
    assert!(gap.d498_reopen_required);
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
    let units = BodyUnitInventory::from_text("body text", rung());
    assert_eq!(
        classify_body_coverage(
            "same-generator",
            &native,
            &units,
            &oracle_for("body text"),
            &units,
            &named_sut,
            &body_scope(),
            &oracle_scope()
        ),
        Err(NativeBodyTextError::OracleNotIndependent)
    );
}

#[test]
fn an_empty_oracle_is_not_assessable_not_a_zero_zero_pass() {
    let plane = plane_from_text("empty-oracle", "body text");
    let native = native_body(&plane).unwrap();
    let plane_units = BodyUnitInventory::from_text("body text", rung());
    let empty_units = BodyUnitInventory::empty(rung());
    let gap = classify_body_coverage(
        "empty-oracle",
        &native,
        &plane_units,
        &oracle_for(""),
        &empty_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.reason, BodyGapReason::OracleEmpty);
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
            &segmentation,
            rung(),
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

// ─────────────── S05/T01 additive Body extraction/assignment ───────────────

/// A plane whose second line is a whole-line Body vocabulary entry
/// (`introduction`), so the geometry/heading route enters `Body` while a
/// marker-only segmentation that never sees a numbered heading does not.
fn additive_plane() -> Plane {
    plane_from_text("additive", "title\nintroduction\nalpha beta gamma")
}

fn additive_assignment(
    plane: &Plane,
    segmentation: &FullSegmentationOutcome,
) -> da_parse::NativeBodyAssignment {
    build_native_body_assignment(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        segmentation,
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        rung(),
    )
    .expect("the additive assignment evaluates")
}

#[test]
fn the_additive_assignment_refuses_a_single_route_boundary_even_when_it_is_header() {
    let plane = additive_plane();
    // The marker route stays in Header for every token: there is no numbered
    // heading, so `segment_document_full` never enters Body. Only the heading
    // route enters Body, and the marker route labels its boundary token `Header`
    // — the exact hole where a header token used to be accepted. One route's own
    // entry evidence is not independent corroboration, so the boundary is
    // refused and no body is adopted.
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Header);
    let boundary = segmentation
        .assignments
        .iter()
        .find(|assignment| assignment.source_ordinal == 2)
        .expect("the introduction token is admitted");
    assert_eq!(boundary.region, FullSemanticRegion::Header);
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::SingleRouteUncorroborated
    );
    assert_eq!(assignment.adopted_route, None);
    assert_eq!(assignment.segmentation_body_tokens, 0);
    assert!(assignment.heading_body_tokens > 0);
    assert_eq!(assignment.segmentation_start, None);
    assert_eq!(assignment.segmentation_end, None);
    assert_eq!(assignment.heading_start, Some((0, 2)));
    assert_eq!(assignment.heading_end, Some((0, 4)));
    assert_eq!(assignment.entry_match, Some(HeadingMatch::NormalizedLine));
    assert_eq!(assignment.adopted_bounds, None);
    assert!(
        assignment.body.is_none(),
        "a single-route boundary must not be adopted as independent corroboration"
    );
}

#[test]
fn a_shared_start_with_a_different_end_is_not_whole_run_agreement() {
    let plane = additive_plane();
    // Header for the title and separator, Body only for `introduction` and its
    // separator, References from the prose: the marker route's Body ends earlier
    // than the heading route's. The starts agree, but only the intersection of
    // the two runs is corroborated — the full heading run is never adopted.
    let segmentation = segmentation_with(&plane, |ordinal| {
        if ordinal < 2 {
            FullSemanticRegion::Header
        } else if ordinal < 4 {
            FullSemanticRegion::Body
        } else {
            FullSemanticRegion::References
        }
    });
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::ConservativeExtentIntersection
    );
    assert_eq!(assignment.segmentation_start, assignment.heading_start);
    assert_ne!(assignment.segmentation_end, assignment.heading_end);
    assert_eq!(assignment.segmentation_end, Some((0, 3)));
    assert_eq!(assignment.heading_end, Some((0, 4)));
    // The marker route's last Body token is ordinal 3, a whitespace separator
    // that projects no word; the word-level heading route has no token for it, so
    // the corroborated identity set is `{introduction}` and both ends sit on
    // `(0, 2)` — not the scalar interval `(0, 2)–(0, 3)`.
    assert_eq!(assignment.adopted_bounds, Some(((0, 2), (0, 2))));
    assert!(
        assignment.body.is_some(),
        "the corroborated intersection is adopted"
    );
    assert_ne!(
        assignment.verification,
        BodyBoundaryVerification::TwoIndependentRoutesAgree
    );
}

#[test]
fn the_additive_assignment_adopts_only_the_intersection_when_starts_differ() {
    let plane = additive_plane();
    // The marker route assigns Body from the very first token, while the heading
    // route enters Body only at the `introduction` line. The starts differ, so
    // the whole heading run is not a shared extent; the identity intersection
    // (introduction onward) is.
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Body);
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::ConservativeExtentIntersection
    );
    assert_eq!(assignment.segmentation_start, Some((0, 0)));
    assert_eq!(assignment.heading_start, Some((0, 2)));
    assert_eq!(assignment.adopted_bounds, Some(((0, 2), (0, 4))));
    assert!(
        assignment.body.is_some(),
        "the intersection both routes carry is adopted"
    );
}

#[test]
fn the_additive_assignment_agrees_when_both_routes_carry_the_same_complete_extent() {
    let plane = additive_plane();
    // Header for the title and its separator, Body from `introduction` on: the
    // two routes do not share their entry rule, but both routes carry the same
    // word-bearing parent identities, so the boundary is corroborated.
    let segmentation = segmentation_with(&plane, |ordinal| {
        if ordinal < 2 {
            FullSemanticRegion::Header
        } else {
            FullSemanticRegion::Body
        }
    });
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::TwoIndependentRoutesAgree
    );
    assert_eq!(assignment.segmentation_start, assignment.heading_start);
    assert_eq!(assignment.segmentation_end, assignment.heading_end);
    assert!(assignment.body.is_some());
}

/// A plane whose heading route enters `Body` at the whole-line marker
/// `introduction` and stays there. Reading ordinals: 0 `title`, 1 separator,
/// 2 `introduction`, 3 separator, 4 `name`, 5 separator, 6 `tail`.
fn interior_plane() -> Plane {
    plane_from_text("additive-interior", "title\nintroduction\nname\ntail")
}

#[test]
fn equal_extent_endpoints_with_a_different_interior_are_not_whole_run_agreement() {
    let plane = interior_plane();
    // The marker route marks `name` (ordinal 4) `Header` and only the two ends
    // `Body`, so both routes' first and last identities agree while the interior
    // differs by identity. Equal endpoints are not complete membership.
    let segmentation = segmentation_with(&plane, |ordinal| match ordinal {
        2 | 3 | 6 => FullSemanticRegion::Body,
        _ => FullSemanticRegion::Header,
    });
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(assignment.segmentation_start, Some((0, 2)));
    assert_eq!(assignment.segmentation_end, Some((0, 6)));
    assert_eq!(assignment.heading_start, Some((0, 2)));
    assert_eq!(assignment.heading_end, Some((0, 6)));
    assert_ne!(
        assignment.verification,
        BodyBoundaryVerification::TwoIndependentRoutesAgree,
        "equal endpoints over a different interior must not read as complete agreement"
    );
    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::ConservativeExtentIntersection
    );
    assert_eq!(assignment.adopted_bounds, Some(((0, 2), (0, 6))));
}

#[test]
fn a_header_parent_inside_the_extent_is_never_adopted() {
    let plane = interior_plane();
    let segmentation = segmentation_with(&plane, |ordinal| match ordinal {
        2 | 3 | 6 => FullSemanticRegion::Body,
        _ => FullSemanticRegion::Header,
    });
    let assignment = additive_assignment(&plane, &segmentation);
    let body = assignment
        .body
        .as_ref()
        .expect("the corroborated ends are adopted");

    assert_eq!(
        body.units().count_of("name"),
        0,
        "a Header parent inside the extent is not body"
    );
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("tail"), 1);
    assert_eq!(body.units().total(), 2);
}

#[test]
fn disjoint_body_identity_sets_are_a_named_refusal() {
    let plane = interior_plane();
    // The marker route carries only whitespace separators in `Body`, while the
    // heading route carries the three prose lines. The scalar tuple intervals
    // overlap, but the word-bearing parent identity sets are disjoint, so
    // nothing is corroborated and no body is adopted.
    let segmentation = segmentation_with(&plane, |ordinal| match ordinal {
        1 | 3 | 5 => FullSemanticRegion::Body,
        _ => FullSemanticRegion::Header,
    });
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::RoutesDisagree
    );
    assert_eq!(assignment.adopted_route, None);
    assert_eq!(assignment.adopted_bounds, None);
    assert!(assignment.body.is_none());
    assert_eq!(assignment.segmentation_start, Some((0, 1)));
    assert_eq!(assignment.segmentation_end, Some((0, 5)));
    assert_eq!(assignment.heading_start, Some((0, 2)));
    assert_eq!(assignment.heading_end, Some((0, 6)));
}

#[test]
fn a_duplicated_parent_contributes_all_or_none_of_its_words() {
    // `alpha beta gamma` is one lexical parent span projected into three words,
    // so the heading route carries three tokens with the same parent identity.
    // When the parent is corroborated, every one of its words is adopted; when
    // it is not, none of them is.
    let plane = additive_plane();

    let agreed = segmentation_with(&plane, |ordinal| {
        if ordinal < 2 {
            FullSemanticRegion::Header
        } else {
            FullSemanticRegion::Body
        }
    });
    let assignment = additive_assignment(&plane, &agreed);
    let body = assignment
        .body
        .as_ref()
        .expect("the membership agreement adopts a body");
    assert_eq!(body.units().count_of("alpha"), 1);
    assert_eq!(body.units().count_of("beta"), 1);
    assert_eq!(body.units().count_of("gamma"), 1);
    assert_eq!(body.units().total(), 4);

    let introduction_only = segmentation_with(&plane, |ordinal| {
        if ordinal == 2 {
            FullSemanticRegion::Body
        } else {
            FullSemanticRegion::Header
        }
    });
    let assignment = additive_assignment(&plane, &introduction_only);
    let body = assignment
        .body
        .as_ref()
        .expect("the shared parent is adopted");
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 0);
    assert_eq!(body.units().count_of("beta"), 0);
    assert_eq!(body.units().count_of("gamma"), 0);
    assert_eq!(body.units().total(), 1);
}

#[test]
fn a_permuted_source_ordinal_keeps_the_identity_intersection() {
    // Reading order is text order; the donor's `source_ordinal` is permuted
    // against it (`title`=3, separator=0, `introduction`=4, separator=1,
    // `alpha beta gamma`=2). Both routes carry the same word-bearing parents, so
    // the extents agree — yet the first/last identity pair is non-monotonic and
    // a scalar `(page, source_ordinal)` interval would filter every token out.
    let plane = plane_from_text_with_source_ordinals(
        "additive-permuted",
        "title\nintroduction\nalpha beta gamma",
        |index| [3u32, 0, 4, 1, 2][index],
    );
    let segmentation = segmentation_with(&plane, |ordinal| {
        if ordinal < 2 {
            FullSemanticRegion::Header
        } else {
            FullSemanticRegion::Body
        }
    });
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::TwoIndependentRoutesAgree,
        "the same word-bearing parent set agrees even when source_ordinal is permuted"
    );
    // First and last adopted parent identities in admitted reading order.
    assert_eq!(assignment.adopted_bounds, Some(((0, 4), (0, 2))));
    let body = assignment
        .body
        .as_ref()
        .expect("the agreement adopts a body");
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 1);
    assert_eq!(body.units().count_of("beta"), 1);
    assert_eq!(body.units().count_of("gamma"), 1);
    assert_eq!(body.units().total(), 4);
    assert_eq!(body.text(), "introduction alpha beta gamma");
}

#[test]
fn the_additive_assignment_is_a_typed_absence_when_neither_route_enters_body() {
    let plane = plane_from_text("additive-none", "alpha beta gamma delta");
    let segmentation = segmentation_for(&plane, FullSemanticRegion::Header);
    let assignment = additive_assignment(&plane, &segmentation);

    assert_eq!(
        assignment.verification,
        BodyBoundaryVerification::NoRouteEnteredBody
    );
    assert_eq!(assignment.adopted_route, None);
    assert_eq!(assignment.segmentation_body_tokens, 0);
    assert_eq!(assignment.heading_body_tokens, 0);
    assert_eq!(assignment.adopted_bounds, None);
    assert!(assignment.body.is_none());
}

#[test]
fn a_scope_that_misnames_the_join_policy_is_refused() {
    // The body was assembled with a word-space join; the scope declares a
    // verbatim concatenation. The declaration describes a different text, so the
    // comparison is refused instead of believed.
    let native = NativeBodyText::from_fragments(
        "join-mismatch",
        "d".repeat(64),
        2,
        None,
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        ["alpha", "beta"],
        rung(),
    )
    .unwrap();
    assert_eq!(native.join, BodyJoinPolicy::SingleSpaceBetweenWordTokens);
    let plane_units = BodyUnitInventory::from_fragments(["alpha", "beta"], rung());
    let oracle_units = BodyUnitInventory::from_text("alpha beta", rung());
    let gap = classify_body_coverage(
        "join-mismatch",
        &native,
        &plane_units,
        &oracle_for("alpha beta"),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert_eq!(gap.reason, BodyGapReason::ScopeJoinMismatch);
    assert!(!gap.d498_reopen_required);
    // Both axes are still published, independent of the scope refusal.
    assert_eq!(
        gap.plane_verdict.verdict,
        BodyCoverageVerdict::GranularityOnly
    );
    assert_eq!(
        gap.body_verdict.verdict,
        BodyCoverageVerdict::GranularityOnly
    );
}

#[test]
fn both_axes_are_published_when_the_plane_is_uncertain_and_the_body_is_short() {
    // The plane carries `alpha` only as characters inside `alphabeta`, so
    // the P1 axis is uncertain; the body (the `gamma` prefix) is absent for
    // `alpha`, so the P2 axis is a region gap. The combined reason names P1, but
    // the body axis is published beside it rather than hidden.
    let plane = plane_from_text("both-axes", "gamma\nalphabeta");
    let native = native_body_prefix(&plane, 1).expect("the prefix body assembles");
    let plane_units = BodyUnitInventory::from_text("gamma\nalphabeta", rung());
    let oracle_units = BodyUnitInventory::from_text("alpha gamma", rung());
    let gap = classify_body_coverage(
        "both-axes",
        &native,
        &plane_units,
        &oracle_for("alpha gamma"),
        &oracle_units,
        &independent_provenance(),
        &body_scope(),
        &oracle_scope(),
    )
    .unwrap();
    assert_eq!(gap.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(matches!(
        gap.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
    assert!(matches!(
        gap.plane_verdict.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
    assert!(matches!(
        gap.body_verdict.reason,
        BodyGapReason::P2RegionAssignmentGap { .. }
    ));
    assert_eq!(gap.body_verdict.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!gap.d498_reopen_required);
}

// ─────────── S05/T01 third-route candidate (v1 `segment_document`) ───────────

/// Evaluate the real v1 `NativeSegmenter` (`segment_document`) as the additive
/// third `Body` candidate against the same heading route the primary engine
/// uses. The synthetic planes here are tracked projections, so this file's claim
/// stays limited to typed candidacy behaviour, not a corpus body verdict.
fn third_route_candidate(plane: &Plane) -> NativeBodyThirdRouteCandidate {
    let third = segment_document(
        &plane.document,
        &plane.lexemes,
        SegmentationConfig::default(),
    )
    .expect("the v1 segmenter accepts the native plane");
    evaluate_body_third_route_candidate(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &third,
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        rung(),
    )
    .expect("the third-route candidate evaluates")
}

#[test]
fn a_third_route_that_agrees_with_the_heading_route_adopts_the_shared_set() {
    // An ALL-CAPS vocabulary heading is entered by the v1 marker
    // (`heading_depth`) and by the heading route (normalized `full_marker`), so
    // the two routes carry the same word-bearing parent identity set.
    let plane = plane_from_text("third-agree", "title\nINTRODUCTION\nalpha beta\nReferences");
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::CorroboratedHeadingAgreement
    );
    assert!(!candidate.provenance.reads_tei);
    assert_eq!(candidate.provenance.function, "segment_document");
    assert_eq!(
        candidate.provenance.accepted_input,
        "layout_document+lexeme_rows"
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the agreement adopts a body");
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 1);
    assert_eq!(body.units().count_of("beta"), 1);
    assert_eq!(body.units().count_of("references"), 0);
}

#[test]
fn a_third_route_that_enters_body_alone_is_a_named_refusal() {
    // `1 Experimental Setup` is entered by the v1 numeric `heading_depth` rule
    // but is not a heading-route vocabulary entry and is not prominent, so the
    // heading route never enters Body. A boundary only the candidate carries is
    // never adopted as if both routes corroborated it.
    let plane = plane_from_text(
        "third-heading-absent",
        "title\n1 Experimental Setup\nalpha beta",
    );
    let candidate = third_route_candidate(&plane);
    assert_eq!(candidate.verdict, BodyThirdRouteVerdict::HeadingRouteAbsent);
    assert_eq!(candidate.heading_body_parent_ids, 0);
    assert!(candidate.third_route_word_body_parent_ids > 0);
    assert_eq!(candidate.adopted_bounds, None);
    assert!(candidate.body.is_none());
}

#[test]
fn a_heading_route_that_enters_body_alone_is_a_named_refusal() {
    // `Introduction` is a heading-route vocabulary entry but carries no numeric
    // prefix and is not ALL-CAPS, so the v1 candidate never enters Body.
    let plane = plane_from_text("third-absent", "title\nIntroduction\nalpha beta");
    let candidate = third_route_candidate(&plane);
    assert_eq!(candidate.verdict, BodyThirdRouteVerdict::ThirdRouteAbsent);
    assert_eq!(candidate.third_route_body_tokens, 0);
    assert!(candidate.heading_body_parent_ids > 0);
    assert_eq!(candidate.adopted_bounds, None);
    assert!(candidate.body.is_none());
}

#[test]
fn an_uppercase_banner_premature_start_is_intersected_away() {
    // `PREPRINT` is an ALL-CAPS banner, so the v1 candidate enters Body there
    // before the real heading. The heading route enters at `INTRODUCTION`. The
    // adopted bounded body is the identity intersection, so the banner is not
    // silently adopted as consensus.
    let plane = plane_from_text("third-banner", "title\nPREPRINT\nINTRODUCTION\nalpha beta");
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::HeadingSubsetOfThirdRoute
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the intersection adopts a body");
    assert_eq!(body.units().count_of("preprint"), 0);
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 1);
}

#[test]
fn a_bare_numeric_line_is_not_a_corroborated_body_start() {
    // `2024` is an ALL-digit line, so the v1 numeric rule enters Body there; the
    // heading route does not. The intersection starts at the real heading and the
    // numeric line is not adopted.
    let plane = plane_from_text("third-numeric", "title\n2024\nINTRODUCTION\nalpha beta");
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::HeadingSubsetOfThirdRoute
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the intersection adopts a body");
    assert_eq!(body.units().count_of("2024"), 0);
    assert_eq!(body.units().count_of("introduction"), 1);
}

#[test]
fn a_repeated_margin_line_is_furniture_and_is_not_adopted_as_body() {
    // The v1 route excludes repeated page-margin text as `PageFurniture`; the
    // heading route has no furniture notion. A furniture parent that the heading
    // route carries must not enter the adopted body, because the candidate route
    // never corroborated it.
    let plane = plane_from_text(
        "third-furniture",
        "preprint\nINTRODUCTION\nalpha\nbeta\ngamma\npreprint\ndelta",
    );
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::ThirdRouteSubsetOfHeading
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the intersection adopts a body");
    assert_eq!(body.units().count_of("preprint"), 0);
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("delta"), 1);
}

#[test]
fn equal_endpoints_with_a_different_interior_are_not_whole_run_agreement() {
    // Both routes first enter Body at `INTRODUCTION` and last carry
    // `delta epsilon`, but the v1 route keeps `Acknowledgement`/`gamma` in Body
    // while the heading route assigns them to `Acknowledgement`. Equal endpoints
    // over a different interior must be the conservative intersection, never
    // whole-run agreement.
    let plane = plane_from_text(
        "third-interior",
        "title\nINTRODUCTION\nalpha beta\nAcknowledgement\ngamma\nMETHODS\ndelta epsilon\nReferences",
    );
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::HeadingSubsetOfThirdRoute
    );
    assert!(candidate.adopted_bounds.is_some());
    let body = candidate
        .body
        .as_ref()
        .expect("the intersection adopts a body");
    assert_eq!(body.units().count_of("acknowledgement"), 0);
    assert_eq!(body.units().count_of("gamma"), 0);
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("methods"), 1);
    assert_eq!(body.units().count_of("delta"), 1);
}

#[test]
fn a_two_sided_boundary_disagreement_is_the_conservative_intersection() {
    // The heading route enters `Body` earlier (`Abstract`) while the v1 route
    // stays `Header` until `INTRODUCTION`; the v1 route keeps
    // `Acknowledgement`/`gamma` in `Body` while the heading route assigns them
    // to `Acknowledgement`. Each route therefore carries a token the other does
    // not, so the adopted set is the proper intersection, never either whole
    // run.
    let plane = plane_from_text(
        "third-proper",
        "title\nAbstract\nINTRODUCTION\nalpha\nAcknowledgement\ngamma\nMETHODS\ndelta\nReferences",
    );
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::CorroboratedHeadingIntersection
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the intersection adopts a body");
    assert_eq!(body.units().count_of("abstract"), 0);
    assert_eq!(body.units().count_of("acknowledgement"), 0);
    assert_eq!(body.units().count_of("gamma"), 0);
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 1);
    assert_eq!(body.units().count_of("methods"), 1);
    assert_eq!(body.units().count_of("delta"), 1);
}

#[test]
fn a_permuted_source_ordinal_keeps_the_third_route_identity_intersection() {
    // Reading order is text order; the donor's `source_ordinal` is permuted
    // against it. The candidate compares parent identity sets, not a scalar
    // tuple interval, so the shared identities still corroborate and the adopted
    // stream keeps the heading route's admitted reading order.
    let plane = plane_from_text_with_source_ordinals(
        "third-permuted",
        "title\nINTRODUCTION\nalpha beta",
        |index| [3u32, 0, 4, 1, 2][index],
    );
    let candidate = third_route_candidate(&plane);
    assert_eq!(
        candidate.verdict,
        BodyThirdRouteVerdict::CorroboratedHeadingAgreement
    );
    let body = candidate
        .body
        .as_ref()
        .expect("the agreement adopts a body");
    assert_eq!(body.units().count_of("introduction"), 1);
    assert_eq!(body.units().count_of("alpha"), 1);
    assert_eq!(body.units().count_of("beta"), 1);
    assert_eq!(body.text(), "INTRODUCTION alpha beta");
}

#[test]
fn a_foreign_third_route_receipt_is_refused_before_any_comparison() {
    // A receipt that names another document is not the SUT's candidate route; it
    // is refused with provenance rather than compared.
    let plane = plane_from_text("third-foreign", "title\nINTRODUCTION\nalpha beta");
    let mut third = segment_document(
        &plane.document,
        &plane.lexemes,
        SegmentationConfig::default(),
    )
    .expect("the v1 segmenter accepts the native plane");
    third.document_id = "another-document".to_owned();
    let error = evaluate_body_third_route_candidate(
        &plane.document,
        &plane.lattice,
        &plane.lexemes,
        &third,
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        rung(),
    )
    .expect_err("a foreign receipt is refused");
    assert!(matches!(
        error,
        NativeBodyTextError::ThirdRouteInputMismatch("document_id")
    ));
}
