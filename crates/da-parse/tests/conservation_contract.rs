//! Frozen-corpus conservation contract (M464-z84in2 S03/T02, R106).
//!
//! Proves, over the three checksum-bound preserved rich TEI fixtures, that the
//! provider-neutral [`da_parse::ConservationEngine`] inventories every observed
//! TEI family in original source order with valid bounded byte spans, derives
//! the candidate side from the current [`da_parse::TeiParseIr`] only (typed
//! unsupported/missing reasons for the deliberately-not-lifted S07 cut),
//! replays byte-identically, and catches every injected loss class through
//! exact typed reason codes:
//!
//! - family item deletion → extra `observed_but_missing` at the exact source ordinals;
//! - source span corruption → `malformed_source` precedence at the exact corrupted ordinal;
//! - section / citation structure permutation → `reordered` with the stable
//!   structure key and moved linkage positions;
//! - coordinate deletion (given a lifted-owner coordinate produced by the real
//!   lifter) → `coordinate_dropped` at the exact source ordinal;
//! - header-author / figure / callout / declared-idno deletion (S07/T04:
//!   lifted P0 families) → `observed_but_missing` (+ count mismatch) at the
//!   exact source ordinal with a bounded span and a moved digest;
//! - declared-idno malformed source (an arXiv idno value that parses to no
//!   arXiv id) → typed lifter missingness and an `observed_but_missing` row;
//! - figure-owned graphic-coordinate deletion → `coordinate_dropped` at the
//!   exact figure-owned source ordinal without disturbing the figure's own
//!   ordinal-aligned retention.
//!
//! Baseline semantics on preserved GROBID TEI (pinned, deterministic): the
//! scanner does not count the paper's own header `sourceDesc`/`biblStruct`
//! as a citation, and does not count figure caption `<head>` as a section.
//! Remaining typed loss after three-region scanner honesty is CountMismatch
//! on Section: the flat GROBID `<head>` scan still emits figure caption heads
//! as sections (S02). Annex `<head>` in `<back>` is not a section; a targeted
//! bibr in `<back>` is a typed CrossReference remainder, not linkage loss.
//! Back `biblStruct` stays CitationEntry. The pristine verdict stays
//! `loss_detected` while caption heads inflate the IR. Replay equality is
//! byte-identical. Injected losses are asserted relative to that pinned
//! baseline.
//!
//! Since S07/T04 the lifted P0 families (header authors, non-table figures,
//! the declared arXiv identity, figure/table callouts, figure-owned graphic
//! coordinates) surface as exact retained rows instead of blanket
//! `unsupported_this_cut` pins; unsupported remainders stay exact and typed
//! for what the cut still does not lift: `<ptr>` links, `<note>` entries,
//! non-arXiv header idnos (MD5) and non-callout cross references.
//!
//! Offline only: envelopes are served by [`da_parse::CachedTeiBackend`] (no
//! HTTP), lifted by [`da_parse::TeiLifter`], compared by the stateless engine.
//! Fixture absence or checksum drift is fail-visible (hard panic), never a
//! skip. S01 lift output hashes and cache identities are not touched: no
//! fixture bytes are modified on disk and mutations are in-memory only.
//!
//! Reports are redacting by construction: the serialized
//! [`da_parse::ConservationDelta`] must carry identity/counts/ordinals/ranges/
//! digests only — the tests assert no raw TEI markup and no title/abstract/
//! body text ever reaches the delta JSON.

use std::path::{Path, PathBuf};

use da_parse::{
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, ConservationDelta, ConservationEngine,
    ConservationFamily, ConservationReasonCode, ConservationSubtype, ConservationVerdict,
    GrobidProfile, ParseBackend, ParseBackendRequest, TeiEnvelope, TeiIdentity, TeiLifter,
    TeiParseIr,
};
use serde_yaml::Value;
use sha2::{Digest, Sha256};

/// Sanctioned DATA manifest binding fixture paths to frozen SHA-256 checksums
/// (data/parse/regression_cases.yaml; `binding_status: bound` rows only).
const REGRESSION_MANIFEST_REL: &str = "data/parse/regression_cases.yaml";

/// Rich profile fingerprint every bound preserved fixture is pinned to
/// (D204: rich and structural identities must never mix).
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";

// ─── Fail-visible fixture integrity ─────────────────────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a repo-relative file. Missing files are FATAL here: the contract must
/// never silently shrink because a fixture disappeared.
fn read_repo_file(rel_path: &str) -> Vec<u8> {
    let full = repo_root().join(rel_path);
    std::fs::read(&full).unwrap_or_else(|error| {
        panic!("FATAL {rel_path}: fixture missing/unreadable ({error}) — conservation contract hard-fails, never skips")
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Fail-visible checksum gate shared by every run in this file.
fn verify_fixture_sha(bytes: &[u8], declared_sha256: &str, case_id: &str) {
    let actual = sha256_hex(bytes);
    assert_eq!(
        actual, declared_sha256,
        "FATAL {case_id}: preserved fixture bytes drifted from the frozen checksum"
    );
}

fn manifest_field<'a>(value: &'a Value, key: &str, case_id: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("FATAL {case_id}: manifest field `{key}` missing"))
}

/// One bound preserved fixture, integrity-verified (existence + SHA-256).
struct PreservedFixture {
    case_id: String,
    paper_id: String,
    declared_sha256: String,
    fingerprint: String,
    bytes: Vec<u8>,
}

/// Load the three bound preserved rich TEI fixtures through the manifest.
/// Missing manifests, missing bindings, missing bytes, or checksum drift all
/// hard-fail — they can never silently reduce coverage.
fn preserved_bound_fixtures() -> Vec<PreservedFixture> {
    let manifest: Value = serde_yaml::from_slice(&read_repo_file(REGRESSION_MANIFEST_REL))
        .unwrap_or_else(|error| {
            panic!("FATAL {REGRESSION_MANIFEST_REL}: unreadable manifest ({error})")
        });
    let cases = manifest
        .get("cases")
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| panic!("FATAL {REGRESSION_MANIFEST_REL}: no cases sequence"));

    let mut out = Vec::new();
    for case in cases {
        if manifest_field(case, "source_kind", "case") != "preserved_grobid_tei" {
            continue;
        }
        if manifest_field(case, "binding_status", "case") != "bound" {
            continue;
        }
        let case_id = manifest_field(case, "id", "case").to_string();
        let fixture = case
            .get("fixture")
            .unwrap_or_else(|| panic!("FATAL {case_id}: no fixture block"));
        let path = manifest_field(fixture, "path", &case_id).to_string();
        let declared_sha256 = manifest_field(fixture, "sha256", &case_id).to_string();
        let fingerprint =
            manifest_field(case, "expected_cache_config_fingerprint", &case_id).to_string();

        let bytes = read_repo_file(&path);
        assert!(
            !bytes.is_empty(),
            "FATAL {case_id}: preserved fixture {path} is empty"
        );
        verify_fixture_sha(&bytes, &declared_sha256, &case_id);
        assert_eq!(
            fingerprint, RICH_FP,
            "FATAL {case_id}: preserved fixture must stay bound to the rich profile fingerprint"
        );

        let paper_id = case_id
            .rsplit('-')
            .next()
            .unwrap_or_else(|| panic!("FATAL {case_id}: no arXiv id suffix"))
            .to_string();
        out.push(PreservedFixture {
            case_id,
            paper_id,
            declared_sha256,
            fingerprint,
            bytes,
        });
    }

    assert_eq!(
        out.len(),
        3,
        "FATAL: expected exactly three bound preserved fixtures, found {}",
        out.len()
    );
    out
}

fn fixture_by_paper(paper_id: &str) -> PreservedFixture {
    preserved_bound_fixtures()
        .into_iter()
        .find(|fixture| fixture.paper_id == paper_id)
        .unwrap_or_else(|| panic!("FATAL: no bound fixture for paper {paper_id}"))
}

/// Serve the fixture through the real cached-TEI backend seam (no HTTP) and
/// lift it with the real lifter. Identity binding is asserted every time.
async fn load_pair(fixture: &PreservedFixture) -> (TeiEnvelope, TeiParseIr) {
    let backend = CachedTeiBackend::new(fixture.bytes.clone(), GrobidProfile::rich());
    let request = ParseBackendRequest::new("conservation-contract", Vec::new());
    let envelope = backend
        .fetch_tei(request)
        .await
        .expect("FATAL: cached-tei backend cannot fail");

    assert_eq!(envelope.identity().backend_id(), CACHED_TEI_BACKEND_ID);
    assert_eq!(
        envelope.identity().profile_fingerprint(),
        fixture.fingerprint
    );

    let ir = TeiLifter::new().lift(
        &envelope,
        fixture.paper_id.clone(),
        fixture.declared_sha256.clone(),
    );
    (envelope, ir)
}

// ─── Manifest-derived pins (data/parse/regression_cases.yaml notes) ────────

struct Pins {
    /// Lifted section floor (behavioral floor from the manifest note).
    sections_min: Option<usize>,
    /// Lifted citation floor.
    citations_min: Option<usize>,
    /// Exact lifted/source table pin.
    tables: Option<usize>,
    /// Exact lifted/source formula pin.
    formulas: Option<usize>,
    /// Exact TEI byte-level header author pin (`<persName>` in the header,
    /// all inside `<sourceDesc><biblStruct>` in GROBID TEI).
    authors: usize,
    /// Exact TEI byte-level non-table figure pin.
    figures: usize,
    /// Exact TEI byte-level `coords` attribute pin.
    coordinates: Option<usize>,
    /// Header `<idno type="arXiv">` occurrences (lifted as the declared
    /// identity since S07/T02).
    idnos_arxiv: usize,
    /// Header idno occurrences of other types (MD5) — stay unsupported.
    idnos_other: usize,
    /// Source body/back non-bibr cross references (ref/xref outside bibl).
    cross_refs: usize,
    /// Visual callouts lifted since S07/T03 (figure + table refs).
    visual_callouts: usize,
    /// Pinned this-cut loss: body `<head>` occurrences the lifter does not
    /// emit. Zero after three-region honesty (annex heads are not sections).
    section_missing: usize,
    /// Pinned this-cut loss: header `sourceDesc`/`biblStruct` counted as a
    /// cited work. Zero after scanner honesty.
    citation_missing: usize,
    /// Pinned this-cut loss: body `<ref type="bibr" target>` anchors whose
    /// linkage is not carried by the IR. Zero after three-region honesty
    /// (back targeted bibr is a CrossReference remainder).
    linkage_loss: usize,
}

fn pins_for(paper_id: &str) -> Pins {
    match paper_id {
        // "Behavioral floors: sections >= 80, citations >= 80, 3 tables, 24
        // formulas; 21 header authors and 22 non-table figures present in TEI
        // bytes (not lifted this cut)."
        "2507.19457" => Pins {
            sections_min: Some(80),
            citations_min: Some(80),
            tables: Some(3),
            formulas: Some(24),
            authors: 21,
            figures: 22,
            coordinates: Some(1),
            section_missing: 0,
            citation_missing: 0,
            linkage_loss: 0,
            idnos_arxiv: 1,
            idnos_other: 1,
            cross_refs: 67,
            visual_callouts: 29,
        },
        // "4 header authors, 29 non-table figures, 2 tables, 25 coords
        // attributes in TEI bytes."
        "2512.24601" => Pins {
            sections_min: None,
            citations_min: None,
            tables: Some(2),
            formulas: None,
            authors: 4,
            figures: 29,
            coordinates: Some(25),
            section_missing: 0,
            citation_missing: 0,
            linkage_loss: 0,
            idnos_arxiv: 1,
            idnos_other: 1,
            cross_refs: 67,
            visual_callouts: 36,
        },
        // "Large bibliography pin: citations >= 70. 11 header authors, 55
        // non-table figures, 34 coords attributes in TEI bytes."
        "2605.26525" => Pins {
            sections_min: None,
            citations_min: Some(70),
            tables: None,
            formulas: None,
            authors: 11,
            figures: 55,
            coordinates: Some(34),
            section_missing: 0,
            citation_missing: 0,
            linkage_loss: 0,
            idnos_arxiv: 1,
            idnos_other: 1,
            cross_refs: 140,
            visual_callouts: 41,
        },
        other => panic!("FATAL: no manifest pins for paper {other}"),
    }
}

// ─── Byte-level mutation helpers (test-only, in-memory) ────────────────────

fn find_sub(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| from + position)
}

/// M2 — source span corruption: break the first `</head>` close tag so the
/// section span it terminates becomes structurally malformed. Same byte
/// length, single site, deterministic.
fn corrupt_first_head_close_tag(bytes: &[u8]) -> Vec<u8> {
    let position = find_sub(bytes, 0, b"</head>")
        .unwrap_or_else(|| panic!("FATAL: fixture carries no <head> close tag"));
    let mut mutated = bytes.to_vec();
    mutated.splice(position..position + 7, b"</heab>".iter().copied());
    mutated
}

/// Value of the first `coords="..."` attribute in the raw bytes.
fn first_coords_value(bytes: &[u8]) -> Vec<u8> {
    let start = find_sub(bytes, 0, b"coords=\"")
        .unwrap_or_else(|| panic!("FATAL: fixture carries no coords attribute"))
        + b"coords=\"".len();
    let end = bytes[start..]
        .iter()
        .position(|&byte| byte == b'"')
        .map(|position| start + position)
        .unwrap_or_else(|| panic!("FATAL: unterminated coords attribute"));
    bytes[start..end].to_vec()
}

/// Byte offset of the `>` closing the first `<figure ...>` opening tag whose
/// attributes contain `type="table"` (body-region wrappers only).
fn first_table_figure_tag_end(bytes: &[u8]) -> usize {
    let mut from = 0usize;
    while let Some(start) = find_sub(bytes, from, b"<figure") {
        let end = bytes[start..]
            .iter()
            .position(|&byte| byte == b'>')
            .map(|position| start + position)
            .unwrap_or_else(|| panic!("FATAL: unterminated <figure> tag"));
        if find_sub(&bytes[start..=end], 0, b"type=\"table\"").is_some() {
            return end;
        }
        from = start + 7;
    }
    panic!("FATAL: fixture carries no <figure type=\"table\"> wrapper")
}

/// Count entries matching one (family, reason) pair.
fn count_rows(
    delta: &ConservationDelta,
    family: ConservationFamily,
    reason: ConservationReasonCode,
) -> usize {
    delta
        .entries
        .iter()
        .filter(|entry| entry.family == family && entry.reason == reason)
        .count()
}

/// Redaction helper.
fn snippet_of(text: &str) -> String {
    text.chars().take(24).collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// Fixture integrity is fail-visible: missing files and checksum drift are
/// hard failures, never skips, and can never silently reduce coverage.
#[test]
fn preserved_fixture_integrity_is_fail_visible() {
    let fixtures = preserved_bound_fixtures();
    assert_eq!(fixtures.len(), 3);
    for fixture in &fixtures {
        assert!(
            !fixture.bytes.is_empty(),
            "{}: empty fixture",
            fixture.case_id
        );
        assert_eq!(fixture.declared_sha256.len(), 64);
        assert_eq!(fixture.fingerprint, RICH_FP);
    }
}

#[test]
#[should_panic(expected = "drifted from the frozen checksum")]
fn preserved_checksum_drift_hard_fails_instead_of_skipping() {
    let fixture = fixture_by_paper("2507.19457");
    let drifted = format!("{}0", &fixture.declared_sha256[..63]);
    verify_fixture_sha(&fixture.bytes, &drifted, &fixture.case_id);
}

#[test]
#[should_panic(expected = "fixture missing/unreadable")]
fn preserved_missing_fixture_hard_fails_instead_of_skipping() {
    read_repo_file("crates/da-parse/tests/fixtures/preserved/does_not_exist.tei.xml");
}

/// Complete source inventory in original order with valid bounded spans,
/// candidate inventory derived from the current IR only, pinned baseline
/// conservation findings, exact identity binding, typed S07 gaps, full
/// per-occurrence row coverage, and a redacting report.
#[tokio::test]
async fn preserved_source_inventory_is_complete_ordered_and_bounded() {
    let engine = ConservationEngine::new();
    for fixture in preserved_bound_fixtures() {
        let (envelope, ir) = load_pair(&fixture).await;
        let raw_len = envelope.raw_tei().len();
        let pins = pins_for(&fixture.paper_id);

        // Source inventory: every observed family is present with pinned
        // byte-level counts, contiguous ordinals, and ordered bounded spans.
        let source = engine.source_inventory(&envelope);
        let required = [
            ConservationFamily::Title,
            ConservationFamily::Abstract,
            ConservationFamily::Author,
            ConservationFamily::ArticleIdno,
            ConservationFamily::Section,
            ConservationFamily::CitationEntry,
            ConservationFamily::CitationAnchor,
            ConservationFamily::CrossReference,
            ConservationFamily::Link,
            ConservationFamily::Note,
            ConservationFamily::Figure,
            ConservationFamily::Table,
            ConservationFamily::TableCell,
            ConservationFamily::Formula,
            ConservationFamily::Coordinate,
        ];
        for family in required {
            assert!(
                source.count(family) > 0,
                "{case}: family {family:?} unobserved in source bytes",
                case = fixture.case_id
            );
        }
        assert_eq!(
            source.count(ConservationFamily::Malformed),
            0,
            "{}: pristine fixture must scan clean",
            fixture.case_id
        );
        assert_eq!(source.count(ConservationFamily::Author), pins.authors);
        assert_eq!(source.count(ConservationFamily::Figure), pins.figures);
        assert!(source.count(ConservationFamily::ArticleIdno) >= 1);
        if let Some(tables) = pins.tables {
            assert_eq!(source.count(ConservationFamily::Table), tables);
        }
        if let Some(formulas) = pins.formulas {
            assert_eq!(source.count(ConservationFamily::Formula), formulas);
        }
        if let Some(coordinates) = pins.coordinates {
            assert_eq!(source.count(ConservationFamily::Coordinate), coordinates);
        }
        for (family, observations) in &source.families {
            for (index, observation) in observations.iter().enumerate() {
                assert_eq!(observation.ordinal as usize, index);
            }
            for pair in observations.windows(2) {
                let first = pair[0].byte_range.expect("source span");
                let second = pair[1].byte_range.expect("source span");
                assert!(
                    first.start <= second.start,
                    "{case}: family {family:?} spans must stay in document order",
                    case = fixture.case_id
                );
            }
            for observation in observations {
                let range = observation.byte_range.expect("source span");
                assert!(range.is_valid(raw_len));
                if !observation.malformed {
                    assert!(range.start < range.end, "clean spans are non-empty");
                }
            }
        }

        // Lifted behavioral floors from the manifest.
        assert!(!ir.sections.is_empty() && !ir.citations.is_empty());
        if let Some(sections_min) = pins.sections_min {
            assert!(ir.sections.len() >= sections_min);
        }
        if let Some(citations_min) = pins.citations_min {
            assert!(ir.citations.len() >= citations_min);
        }
        if let Some(tables) = pins.tables {
            assert_eq!(ir.tables.len(), tables);
        }
        if let Some(formulas) = pins.formulas {
            assert_eq!(ir.formulas.len(), formulas);
        }
        assert!(!ir.title.is_empty() && !ir.abstract_text.is_empty());

        // Candidate inventory: IR-only. Since S07/T02–T03 the lifted P0
        // families appear with exact lifted counts; the still-unlifted
        // families (links, notes) never appear.
        let candidate = engine.candidate_inventory(&ir);
        for family in [ConservationFamily::Link, ConservationFamily::Note] {
            assert_eq!(
                candidate.count(family),
                0,
                "{case}: candidate must not invent {family:?}",
                case = fixture.case_id
            );
        }
        assert_eq!(candidate.count(ConservationFamily::Title), 1);
        assert_eq!(candidate.count(ConservationFamily::Abstract), 1);
        assert_eq!(
            candidate.count(ConservationFamily::Section),
            ir.sections.len()
        );
        assert_eq!(
            candidate.count(ConservationFamily::Author),
            pins.authors,
            "{case}: header authors lift 1:1 in document order",
            case = fixture.case_id
        );
        assert_eq!(
            candidate.count(ConservationFamily::Figure),
            pins.figures,
            "{case}: non-table figures lift 1:1 (no double-count)",
            case = fixture.case_id
        );
        assert_eq!(
            candidate.count(ConservationFamily::ArticleIdno),
            pins.idnos_arxiv,
            "{case}: the declared arXiv identity lifts",
            case = fixture.case_id
        );
        assert_eq!(
            candidate.count(ConservationFamily::CrossReference),
            pins.visual_callouts,
            "{case}: figure/table callouts lift",
            case = fixture.case_id
        );
        // These fixtures carry coords attributes on <graphic> only, so every
        // coordinate is figure-owned and lifts with its figure.
        assert_eq!(
            candidate.count(ConservationFamily::Coordinate),
            pins.coordinates
                .unwrap_or_else(|| source.count(ConservationFamily::Coordinate)),
            "{case}: graphic coordinates lift with their figures",
            case = fixture.case_id
        );

        // Baseline delta: the honest pinned conservation findings of this cut.
        let delta = engine.compare(&envelope, &ir);
        assert_eq!(
            delta.verdict,
            ConservationVerdict::LossDetected,
            "{case}: this cut still mismatches caption heads the flat lifter emits as sections — pinned, never silent",
            case = fixture.case_id
        );
        assert_eq!(delta.identity.backend_id, CACHED_TEI_BACKEND_ID);
        assert_eq!(delta.identity.profile_fingerprint, RICH_FP);
        assert_eq!(delta.identity.paper_id, fixture.paper_id);
        assert_eq!(delta.identity.raw_tei_sha256, fixture.declared_sha256);
        assert_eq!(delta.identity.raw_tei_len as usize, raw_len);

        // Pinned typed losses (frozen-corpus conservation evidence).
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Section,
                ConservationReasonCode::ObservedButMissing
            ),
            pins.section_missing,
            "{case}: section losses drifted",
            case = fixture.case_id
        );
        let citation_missing = count_rows(
            &delta,
            ConservationFamily::CitationEntry,
            ConservationReasonCode::ObservedButMissing,
        );
        assert_eq!(
            citation_missing,
            pins.citation_missing,
            "{case}: citation ObservedButMissing drifted after scanner honesty",
            case = fixture.case_id
        );
        if pins.citation_missing > 0 {
            let last = source.count(ConservationFamily::CitationEntry) - 1;
            assert!(
                delta
                    .entries
                    .iter()
                    .any(|entry| entry.family == ConservationFamily::CitationEntry
                        && entry.reason == ConservationReasonCode::ObservedButMissing
                        && entry.source_ordinal == Some(last as u32))
            );
        }
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::CitationAnchor,
                ConservationReasonCode::LinkageLoss
            ),
            pins.linkage_loss,
            "{case}: out-of-window anchor linkage loss drifted",
            case = fixture.case_id
        );

        // Typed S07 remainders: links and notes stay fully unsupported; the
        // partially-lifted families keep exact unsupported remainders
        // (non-arXiv header idnos, non-callout cross references).
        for family in [ConservationFamily::Link, ConservationFamily::Note] {
            assert_eq!(
                count_rows(&delta, family, ConservationReasonCode::UnsupportedThisCut),
                source.count(family),
                "{case}: every {family:?} occurrence needs a typed gap row",
                case = fixture.case_id
            );
        }
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::ArticleIdno,
                ConservationReasonCode::UnsupportedThisCut
            ),
            pins.idnos_other,
            "{case}: non-arXiv header idnos stay typed unsupported",
            case = fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::CrossReference,
                ConservationReasonCode::UnsupportedThisCut
            ),
            pins.cross_refs - pins.visual_callouts,
            "{case}: non-callout cross references stay typed unsupported",
            case = fixture.case_id
        );

        // Lifted P0 families retain exactly (S07/T04): header authors,
        // non-table figures, the declared arXiv identity, the visual callouts
        // and every graphic coordinate are retained rows — never unsupported,
        // never silently lost.
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Author,
                ConservationReasonCode::Retained
            ),
            pins.authors,
            "{case}: header authors retain",
            case = fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Figure,
                ConservationReasonCode::Retained
            ),
            pins.figures,
            "{case}: non-table figures retain",
            case = fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::ArticleIdno,
                ConservationReasonCode::Retained
            ),
            pins.idnos_arxiv,
            "{case}: declared arXiv identity retains",
            case = fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::CrossReference,
                ConservationReasonCode::Retained
            ),
            pins.visual_callouts,
            "{case}: visual callouts retain (unresolved state is not loss)",
            case = fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Coordinate,
                ConservationReasonCode::Retained
            ),
            source.count(ConservationFamily::Coordinate),
            "{}: every graphic coordinate retains",
            fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Coordinate,
                ConservationReasonCode::UnsupportedThisCut
            ),
            0,
            "{}: lifted graphic coordinates are no longer typed unsupported",
            fixture.case_id
        );
        assert_eq!(
            count_rows(
                &delta,
                ConservationFamily::Coordinate,
                ConservationReasonCode::CoordinateDropped
            ),
            0,
            "{}: pristine lift drops no coordinate",
            fixture.case_id
        );

        // Completeness: every source occurrence of every family is rowed.
        for (family, observations) in &source.families {
            let mut covered: Vec<u32> = delta
                .entries
                .iter()
                .filter(|entry| entry.family == *family)
                .filter_map(|entry| entry.source_ordinal)
                .collect();
            covered.sort_unstable();
            covered.dedup();
            let expected: Vec<u32> = (0..observations.len() as u32).collect();
            assert_eq!(
                covered,
                expected,
                "{case}: family {family:?} occurrences missing from the delta",
                case = fixture.case_id
            );
        }
        // Lifted families retain rows.
        for family in [
            ConservationFamily::Section,
            ConservationFamily::CitationEntry,
            ConservationFamily::TableCell,
            ConservationFamily::Formula,
            ConservationFamily::Author,
            ConservationFamily::Figure,
            ConservationFamily::ArticleIdno,
            ConservationFamily::CrossReference,
            ConservationFamily::Coordinate,
        ] {
            assert!(
                delta.entries.iter().any(|entry| {
                    entry.family == family && entry.reason == ConservationReasonCode::Retained
                }),
                "{case}: lifted family {family:?} must retain rows",
                case = fixture.case_id
            );
        }
        assert!(
            delta.entries.len() >= 100,
            "rich fixtures row every occurrence"
        );

        // Redaction: identity/counts/offsets/digests only — never raw TEI or
        // title/abstract/body text.
        let json = serde_json::to_string(&delta).expect("delta serializes");
        assert!(!json.contains('<'), "raw TEI markup leaked into the delta");
        for leak in [
            snippet_of(&ir.title),
            snippet_of(&ir.abstract_text),
            snippet_of(&ir.body_text),
        ] {
            assert!(
                !json.contains(&leak),
                "{case}: document text leaked into the conservation report",
                case = fixture.case_id
            );
        }
        for entry in &delta.entries {
            assert!(entry.digest.is_empty() || entry.digest.len() == 64);
        }
    }
}

/// Untouched replay — two independent offline runs through fresh
/// backend/envelope instances — is fully equal and byte-identical.
#[tokio::test]
async fn preserved_untouched_replay_is_byte_identical() {
    let engine = ConservationEngine::new();
    for fixture in preserved_bound_fixtures() {
        let (envelope_a, ir_a) = load_pair(&fixture).await;
        let (envelope_b, ir_b) = load_pair(&fixture).await;

        let first = engine.compare(&envelope_a, &ir_a);
        let second = engine.compare(&envelope_b, &ir_b);
        let replay = engine.compare(&envelope_a, &ir_a);

        assert_eq!(first, second, "{}: replay diverged", fixture.case_id);
        assert_eq!(first, replay);
        assert_eq!(
            serde_json::to_vec(&first).expect("delta serializes"),
            serde_json::to_vec(&second).expect("replayed delta serializes")
        );
        assert_eq!(first.delta_digest, second.delta_digest);
        assert!(first.entries.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}

/// M1 — injected family item loss: dropping one lifted citation item from the
/// candidate IR adds exact `observed_but_missing` rows at the exact source
/// ordinals and changes the delta digest.
#[tokio::test]
async fn preserved_injected_family_item_loss_is_typed() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let baseline = engine.compare(&envelope, &ir);
    assert_eq!(baseline.verdict, ConservationVerdict::LossDetected);

    let source_count = engine
        .source_inventory(&envelope)
        .count(ConservationFamily::CitationEntry) as u32;
    let candidate_len = ir.citations.len() as u32;
    ir.citations.pop();

    let mutated = engine.compare(&envelope, &ir);
    assert_eq!(
        mutated.verdict,
        ConservationVerdict::LossDetected,
        "{}: deleting a lifted citation item must stay loss",
        fixture.case_id
    );
    assert_ne!(
        mutated.delta_digest, baseline.delta_digest,
        "{}: injected loss must change the digest",
        fixture.case_id
    );
    // Exactly one more missing row than the pinned baseline, at the exact
    // ordinals the ordinal-aligned strategy leaves unmatched.
    let baseline_missing = count_rows(
        &baseline,
        ConservationFamily::CitationEntry,
        ConservationReasonCode::ObservedButMissing,
    );
    let missing_ordinals: Vec<u32> = mutated
        .entries
        .iter()
        .filter(|entry| {
            entry.family == ConservationFamily::CitationEntry
                && entry.reason == ConservationReasonCode::ObservedButMissing
        })
        .filter_map(|entry| entry.source_ordinal)
        .collect();
    assert_eq!(missing_ordinals.len(), baseline_missing + 1);
    assert!(missing_ordinals.contains(&(candidate_len - 1)));
    assert!(missing_ordinals.contains(&(source_count - 1)));
    assert!(
        count_rows(
            &mutated,
            ConservationFamily::CitationEntry,
            ConservationReasonCode::CountMismatch
        ) >= 1
    );
}

/// M2 — injected source span corruption: breaking one `</head>` close tag
/// makes that section span malformed; the engine stays total (no panic, all
/// ranges bounded), flips the verdict to `malformed_source` precedence, and
/// reports the exact family, ordinal, and corrupted span.
#[tokio::test]
async fn preserved_injected_source_span_corruption_is_typed() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, ir) = load_pair(&fixture).await;
    let baseline = engine.compare(&envelope, &ir);
    assert_eq!(baseline.verdict, ConservationVerdict::LossDetected);

    let corrupted_bytes = corrupt_first_head_close_tag(envelope.raw_tei());
    let corrupted = TeiEnvelope::new(
        corrupted_bytes.clone(),
        TeiIdentity::cached_tei(GrobidProfile::rich()),
    );
    let corrupted_ir = TeiLifter::new().lift(
        &corrupted,
        fixture.paper_id.clone(),
        fixture.declared_sha256.clone(),
    );

    let source = engine.source_inventory(&corrupted);
    let damaged = source
        .families
        .get(&ConservationFamily::Section)
        .and_then(|observations| {
            observations
                .iter()
                .find(|observation| observation.malformed)
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: corrupted close tag must surface as a malformed section span",
                fixture.case_id
            )
        })
        .clone();

    let mutated = engine.compare(&corrupted, &corrupted_ir);
    assert_eq!(
        mutated.verdict,
        ConservationVerdict::MalformedSource,
        "{}: span corruption must take malformed precedence (verdict flips)",
        fixture.case_id
    );
    assert_ne!(mutated.entries, baseline.entries);
    assert_ne!(mutated.delta_digest, baseline.delta_digest);
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Section
                && entry.reason == ConservationReasonCode::MalformedSource
                && entry.source_ordinal == Some(damaged.ordinal)
                && entry.byte_range == damaged.byte_range)
    );
    // Total on adversarial bytes: every reported span stays bounded.
    assert!(mutated.entries.iter().all(|entry| {
        entry
            .byte_range
            .is_none_or(|range| range.is_valid(corrupted_bytes.len()))
    }));
}

/// M3 — injected structure permutation: reordering lifted sections adds an
/// exact `reordered` row with the stable structure key (absent from the
/// baseline) and moves linkage positions; permuting citation anchors inside
/// one lifted entry moves retained candidate ordinals without losing items.
#[tokio::test]
async fn preserved_injected_structure_permutation_is_reordered() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let baseline = engine.compare(&envelope, &ir);
    assert_eq!(baseline.verdict, ConservationVerdict::LossDetected);
    assert_eq!(
        count_rows(
            &baseline,
            ConservationFamily::Section,
            ConservationReasonCode::Reordered
        ),
        0,
        "{}: baseline sections must be order-clean for this injection",
        fixture.case_id
    );

    // Section structure: swap the first adjacent pair with distinct titles.
    let swap_at = (0..ir.sections.len() - 1)
        .find(|&index| ir.sections[index].title != ir.sections[index + 1].title)
        .unwrap_or_else(|| {
            panic!(
                "{}: fixture needs distinct adjacent sections",
                fixture.case_id
            )
        });
    let (first, second) = (swap_at as u32, swap_at as u32 + 1);
    ir.sections.swap(swap_at, swap_at + 1);

    let mutated = engine.compare(&envelope, &ir);
    assert_eq!(mutated.verdict, ConservationVerdict::LossDetected);
    assert_ne!(
        mutated.delta_digest, baseline.delta_digest,
        "{}: structure permutation must change the digest",
        fixture.case_id
    );
    assert_eq!(
        count_rows(
            &mutated,
            ConservationFamily::Section,
            ConservationReasonCode::Reordered
        ),
        1
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Section
                && entry.reason == ConservationReasonCode::Reordered
                && entry.structure_key.as_deref() == Some("section_document_order"))
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Section
                && entry.reason == ConservationReasonCode::Retained
                && entry.source_ordinal == Some(first)
                && entry.candidate_ordinal == Some(second))
    );

    // Citation anchor structure: move one anchor linkage between two distinct
    // citations — every anchor stays present (no new linkage loss) but the
    // candidate linkage order changes, so retained positions move.
    let with_anchors: Vec<usize> = ir
        .citations
        .iter()
        .enumerate()
        .filter(|(_, citation)| !citation.anchors.is_empty())
        .map(|(index, _)| index)
        .collect();
    let pair = with_anchors.iter().find_map(|&first| {
        with_anchors.iter().find_map(|&second| {
            (first != second
                && ir.citations[first].anchors[0].ref_id != ir.citations[second].anchors[0].ref_id)
                .then_some((first, second))
        })
    });
    let (host, donor) = pair.unwrap_or_else(|| {
        panic!(
            "{}: fixture needs two distinct-anchor citations",
            fixture.case_id
        )
    });
    let anchors_before = count_rows(
        &mutated,
        ConservationFamily::CitationAnchor,
        ConservationReasonCode::Retained,
    );
    let linkage_before = count_rows(
        &mutated,
        ConservationFamily::CitationAnchor,
        ConservationReasonCode::LinkageLoss,
    );
    let retained_candidate_of = |delta: &ConservationDelta, ordinal: u32| {
        delta
            .entries
            .iter()
            .find(|entry| {
                entry.family == ConservationFamily::CitationAnchor
                    && entry.reason == ConservationReasonCode::Retained
                    && entry.source_ordinal == Some(ordinal)
            })
            .and_then(|entry| entry.candidate_ordinal)
    };
    if donor > host {
        let (left, right) = ir.citations.split_at_mut(donor);
        std::mem::swap(&mut left[host].anchors[0], &mut right[0].anchors[0]);
    } else {
        let (left, right) = ir.citations.split_at_mut(host);
        std::mem::swap(&mut right[0].anchors[0], &mut left[donor].anchors[0]);
    }

    let anchor_permuted = engine.compare(&envelope, &ir);
    assert_eq!(anchor_permuted.verdict, ConservationVerdict::LossDetected);
    assert_ne!(anchor_permuted.delta_digest, baseline.delta_digest);
    assert_ne!(anchor_permuted.delta_digest, mutated.delta_digest);
    assert_eq!(
        count_rows(
            &anchor_permuted,
            ConservationFamily::CitationAnchor,
            ConservationReasonCode::Retained
        ),
        anchors_before,
        "permutation must not drop anchors"
    );
    assert_eq!(
        count_rows(
            &anchor_permuted,
            ConservationFamily::CitationAnchor,
            ConservationReasonCode::LinkageLoss
        ),
        linkage_before,
        "permutation must not invent linkage loss"
    );
    let source_anchor_total = engine
        .source_inventory(&envelope)
        .count(ConservationFamily::CitationAnchor) as u32;
    assert!(
        (0..source_anchor_total).any(|ordinal| retained_candidate_of(&anchor_permuted, ordinal)
            != retained_candidate_of(&mutated, ordinal)),
        "{}: anchor permutation must move linkage positions",
        fixture.case_id
    );
}

/// M4 — injected coordinate loss: give one real coords attribute a lifted
/// owner through real bytes (real lifter retains it — a row absent from the
/// pristine baseline), then delete the coordinate from the candidate IR. The
/// engine reports `coordinate_dropped` at the exact source ordinal and the
/// digest changes.
#[tokio::test]
async fn preserved_injected_coordinate_loss_is_typed() {
    let fixture = fixture_by_paper("2512.24601");
    let engine = ConservationEngine::new();

    // Byte-level prep: attach the first coords value to the first
    // <figure type="table"> opening tag so the real lifter lifts it onto the
    // first table (in-memory only — frozen fixture bytes are never written).
    let mut bytes = fixture.bytes.clone();
    let coords_value = first_coords_value(&bytes);
    let insert_at = first_table_figure_tag_end(&bytes);
    let adjusted = if bytes[insert_at - 1] == b'/' {
        insert_at - 1
    } else {
        insert_at
    };
    let attribute = format!(" coords=\"{}\"", String::from_utf8_lossy(&coords_value));
    bytes.splice(adjusted..adjusted, attribute.into_bytes().iter().copied());

    let envelope = TeiEnvelope::new(
        bytes.clone(),
        TeiIdentity::cached_tei(GrobidProfile::rich()),
    );
    let ir = TeiLifter::new().lift(
        &envelope,
        fixture.paper_id.clone(),
        fixture.declared_sha256.clone(),
    );
    let coords_digest = sha256_hex(&coords_value);

    // The real lifter now carries the coordinate; the engine retains it.
    assert!(
        ir.tables
            .first()
            .and_then(|table| table.coordinates.as_deref())
            .is_some(),
        "{}: attached coords must be lifted onto the first table",
        fixture.case_id
    );
    let baseline = engine.compare(&envelope, &ir);
    assert!(
        baseline
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Coordinate
                && entry.reason == ConservationReasonCode::Retained
                && entry.digest == coords_digest)
    );
    assert_eq!(
        count_rows(
            &baseline,
            ConservationFamily::Coordinate,
            ConservationReasonCode::CoordinateDropped
        ),
        0,
        "{}: retained baseline must hold no dropped coordinate",
        fixture.case_id
    );

    // Inject the loss: drop the lifted coordinate from the candidate IR.
    let source = engine.source_inventory(&envelope);
    let expected_ordinal = source
        .families
        .get(&ConservationFamily::Coordinate)
        .and_then(|observations| {
            observations
                .iter()
                .find(|observation| observation.owner_family == Some(ConservationFamily::Table))
        })
        .map(|observation| observation.ordinal)
        .unwrap_or_else(|| {
            panic!(
                "{}: table-owned coordinate missing from source",
                fixture.case_id
            )
        });

    let mut lossy = ir.clone();
    lossy.tables[0].coordinates = None;

    let mutated = engine.compare(&envelope, &lossy);
    assert_eq!(
        mutated.verdict,
        ConservationVerdict::LossDetected,
        "{}: deleting a lifted coordinate must be loss",
        fixture.case_id
    );
    assert_ne!(
        mutated.delta_digest, baseline.delta_digest,
        "{}: coordinate deletion must change the digest",
        fixture.case_id
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Coordinate
                && entry.reason == ConservationReasonCode::CoordinateDropped
                && entry.source_ordinal == Some(expected_ordinal)
                && entry.digest == coords_digest)
    );
}

/// M5 — injected header-author deletion: dropping the last lifted header
/// author adds an exact `observed_but_missing` row at the last source ordinal
/// (with its bounded span) plus the family count mismatch, and moves the
/// delta digest; the remaining authors stay retained.
#[tokio::test]
async fn preserved_injected_author_deletion_is_typed() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let pins = pins_for("2507.19457");
    let baseline = engine.compare(&envelope, &ir);
    assert_eq!(
        count_rows(
            &baseline,
            ConservationFamily::Author,
            ConservationReasonCode::Retained
        ),
        pins.authors
    );

    let last_ordinal = engine
        .source_inventory(&envelope)
        .count(ConservationFamily::Author) as u32
        - 1;
    ir.header_authors.pop();

    let mutated = engine.compare(&envelope, &ir);
    assert_eq!(mutated.verdict, ConservationVerdict::LossDetected);
    assert_ne!(
        mutated.delta_digest, baseline.delta_digest,
        "{}: author deletion must change the digest",
        fixture.case_id
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Author
                && entry.reason == ConservationReasonCode::ObservedButMissing
                && entry.source_ordinal == Some(last_ordinal)
                && entry.byte_range.is_some())
    );
    assert!(
        count_rows(
            &mutated,
            ConservationFamily::Author,
            ConservationReasonCode::CountMismatch
        ) >= 1
    );
    assert_eq!(
        count_rows(
            &mutated,
            ConservationFamily::Author,
            ConservationReasonCode::Retained
        ),
        pins.authors - 1,
        "{}: remaining authors stay retained",
        fixture.case_id
    );
}

/// M6 — injected declared-idno deletion: clearing the lifted identity adds an
/// `observed_but_missing` row at the exact arXiv idno source ordinal while
/// the non-arXiv header idno keeps its typed unsupported row; the digest
/// moves.
#[tokio::test]
async fn preserved_injected_declared_idno_deletion_is_typed() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let pins = pins_for("2507.19457");
    let baseline = engine.compare(&envelope, &ir);

    let arxiv_ordinal = engine
        .source_inventory(&envelope)
        .families
        .get(&ConservationFamily::ArticleIdno)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.subtype == Some(ConservationSubtype::ArxivIdno))
        })
        .map(|row| row.ordinal)
        .expect("fixture carries a declared arXiv idno");
    ir.declared_arxiv_id = None;

    let mutated = engine.compare(&envelope, &ir);
    assert_ne!(
        mutated.delta_digest, baseline.delta_digest,
        "{}: idno deletion must change the digest",
        fixture.case_id
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::ArticleIdno
                && entry.reason == ConservationReasonCode::ObservedButMissing
                && entry.source_ordinal == Some(arxiv_ordinal)
                && entry.byte_range.is_some())
    );
    assert_eq!(
        count_rows(
            &mutated,
            ConservationFamily::ArticleIdno,
            ConservationReasonCode::UnsupportedThisCut
        ),
        pins.idnos_other
    );
}

/// M7 — injected declared-idno malformed source: an arXiv idno whose value
/// parses to no arXiv id yields typed missingness in the lifter (no invented
/// identity) and an `observed_but_missing` conservation row; the digest moves
/// relative to the pristine baseline.
#[tokio::test]
async fn preserved_injected_malformed_declared_idno_surfaces_as_loss() {
    let fixture = fixture_by_paper("2507.19457");
    let engine = ConservationEngine::new();
    let (envelope, ir) = load_pair(&fixture).await;
    let pristine = engine.compare(&envelope, &ir);
    assert!(ir.declared_arxiv_id.is_some());

    let declared = b"arXiv:2507.19457v2[cs.CL]";
    let position = find_sub(envelope.raw_tei(), 0, declared).unwrap_or_else(|| {
        panic!(
            "{}: declared idno value missing from bytes",
            fixture.case_id
        )
    });
    let mut bytes = envelope.raw_tei().to_vec();
    let replacement = b"arXiv:not-an-arxiv-id".to_vec();
    bytes.splice(
        position..position + declared.len(),
        replacement.iter().copied(),
    );
    let mutated_envelope = TeiEnvelope::new(bytes, TeiIdentity::cached_tei(GrobidProfile::rich()));
    let mutated_ir = TeiLifter::new().lift(
        &mutated_envelope,
        fixture.paper_id.clone(),
        fixture.declared_sha256.clone(),
    );
    assert!(
        mutated_ir.declared_arxiv_id.is_none(),
        "{}: unusable declared value must not be repaired",
        fixture.case_id
    );
    assert!(mutated_ir.declared_id_missingness.is_some());

    let mutated = engine.compare(&mutated_envelope, &mutated_ir);
    assert_ne!(mutated.delta_digest, pristine.delta_digest);
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::ArticleIdno
                && entry.reason == ConservationReasonCode::ObservedButMissing),
        "{}: malformed declared idno must surface as typed loss",
        fixture.case_id
    );
}

/// M8 — injected figure and callout deletion: dropping the last lifted
/// non-table figure and the last visual callout adds exact
/// `observed_but_missing` rows at their source ordinals and moves the digest.
/// Unresolved callouts on the pristine fixture stay retained: typed candidate
/// unresolved state is never manufactured into conservation loss.
#[tokio::test]
async fn preserved_injected_figure_and_callout_deletion_are_typed() {
    let fixture = fixture_by_paper("2512.24601");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let pins = pins_for("2512.24601");
    let pristine = engine.compare(&envelope, &ir);
    assert_eq!(
        count_rows(
            &pristine,
            ConservationFamily::CrossReference,
            ConservationReasonCode::Retained
        ),
        pins.visual_callouts,
        "{}: every lifted callout retains, unresolved ones included",
        fixture.case_id
    );

    let source = engine.source_inventory(&envelope);
    let figure_ordinal = source.count(ConservationFamily::Figure) as u32 - 1;
    let last_callout_ordinal = source
        .families
        .get(&ConservationFamily::CrossReference)
        .and_then(|rows| rows.iter().rev().find(|row| row.subtype.is_some()))
        .map(|row| row.ordinal)
        .expect("fixture carries visual callouts");

    ir.figures.pop();
    let figure_loss = engine.compare(&envelope, &ir);
    assert_ne!(figure_loss.delta_digest, pristine.delta_digest);
    assert!(
        figure_loss
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Figure
                && entry.reason == ConservationReasonCode::ObservedButMissing
                && entry.source_ordinal == Some(figure_ordinal))
    );

    // Isolated callout deletion from a fresh pair: an exact missing row at
    // the last lifted callout's source ordinal, distinct from both priors.
    let (_, mut callout_ir) = load_pair(&fixture).await;
    callout_ir.visual_callouts.pop();
    let callout_loss = engine.compare(&envelope, &callout_ir);
    assert_ne!(callout_loss.delta_digest, pristine.delta_digest);
    assert_ne!(callout_loss.delta_digest, figure_loss.delta_digest);
    assert!(
        callout_loss
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::CrossReference
                && entry.reason == ConservationReasonCode::ObservedButMissing
                && entry.source_ordinal == Some(last_callout_ordinal))
    );
}

/// M9 — injected figure-owned coordinate loss: clearing a lifted figure's
/// graphic coordinates reports `coordinate_dropped` at the exact figure-owned
/// source ordinal without disturbing the figure's own retention (its ordinal
/// alignment does not compare digests).
#[tokio::test]
async fn preserved_injected_figure_coordinate_loss_is_typed() {
    let fixture = fixture_by_paper("2512.24601");
    let engine = ConservationEngine::new();
    let (envelope, mut ir) = load_pair(&fixture).await;
    let pristine = engine.compare(&envelope, &ir);

    let (figure_index, coords) = ir
        .figures
        .iter()
        .enumerate()
        .find_map(|(index, figure)| {
            figure
                .coordinates
                .as_deref()
                .map(|coords| (index, coords.to_string()))
        })
        .expect("fixture carries a graphic coordinate");
    let coords_digest = sha256_hex(coords.as_bytes());
    let expected_ordinal = engine
        .source_inventory(&envelope)
        .families
        .get(&ConservationFamily::Coordinate)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.owner_family == Some(ConservationFamily::Figure) && row.digest == coords_digest
            })
        })
        .map(|row| row.ordinal)
        .expect("figure-owned coordinate missing from source");

    let figure_losses_before = count_rows(
        &pristine,
        ConservationFamily::Figure,
        ConservationReasonCode::ObservedButMissing,
    );
    ir.figures[figure_index].coordinates = None;

    let mutated = engine.compare(&envelope, &ir);
    assert_eq!(mutated.verdict, ConservationVerdict::LossDetected);
    assert_ne!(
        mutated.delta_digest, pristine.delta_digest,
        "{}: figure coordinate deletion must change the digest",
        fixture.case_id
    );
    assert!(
        mutated
            .entries
            .iter()
            .any(|entry| entry.family == ConservationFamily::Coordinate
                && entry.reason == ConservationReasonCode::CoordinateDropped
                && entry.source_ordinal == Some(expected_ordinal)
                && entry.digest == coords_digest)
    );
    assert_eq!(
        count_rows(
            &mutated,
            ConservationFamily::Figure,
            ConservationReasonCode::ObservedButMissing
        ),
        figure_losses_before,
        "{}: figure retention is ordinal-aligned and unaffected by coord loss",
        fixture.case_id
    );
}
