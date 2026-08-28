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
//!   lifter) → `coordinate_dropped` at the exact source ordinal.
//!
//! Baseline semantics on preserved GROBID TEI (pinned, deterministic): this
//! cut's lifter honestly loses a typed, stable minority — the header
//! `sourceDesc`/`biblStruct` entry (−1 citation per document), citation
//! anchors outside section windows (`linkage_loss`), and trailing caption
//! heads (rlm/hub sections). The pristine verdict is therefore
//! `loss_detected` with pinned per-reason counts, and replay equality is
//! byte-identical. Injected losses are asserted relative to that pinned
//! baseline.
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
    ConservationFamily, ConservationReasonCode, ConservationVerdict, GrobidProfile, ParseBackend,
    ParseBackendRequest, TeiEnvelope, TeiIdentity, TeiLifter, TeiParseIr,
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
    /// Pinned this-cut loss: `<head>` occurrences the lifter does not emit
    /// (trailing caption heads). Frozen deterministic finding.
    section_missing: usize,
    /// Pinned this-cut loss: the header `sourceDesc`/`biblStruct` entry is
    /// not lifted (exactly one per document). Frozen deterministic finding.
    citation_missing: usize,
    /// Pinned this-cut loss: source `<ref type="bibr">` anchors whose linkage
    /// is not carried by the IR (anchors outside section windows).
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
            section_missing: 1,
            citation_missing: 1,
            linkage_loss: 36,
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
            section_missing: 8,
            citation_missing: 1,
            linkage_loss: 29,
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
            citation_missing: 1,
            linkage_loss: 99,
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

        // Candidate inventory: IR-only — the S07 cut surfaces nowhere.
        let candidate = engine.candidate_inventory(&ir);
        for family in [
            ConservationFamily::Author,
            ConservationFamily::ArticleIdno,
            ConservationFamily::Link,
            ConservationFamily::CrossReference,
            ConservationFamily::Note,
            ConservationFamily::Figure,
        ] {
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
        // These fixtures carry coords attributes on <graphic> only, so the
        // this-cut lift holds no typed coordinates yet.
        assert_eq!(candidate.count(ConservationFamily::Coordinate), 0);

        // Baseline delta: the honest pinned conservation findings of this cut.
        let delta = engine.compare(&envelope, &ir);
        assert_eq!(
            delta.verdict,
            ConservationVerdict::LossDetected,
            "{case}: this cut loses header biblStruct citations, out-of-window \
             anchors and trailing caption heads — pinned, never silent",
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
            "{case}: header sourceDesc biblStruct loss drifted",
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

        // Typed S07 gaps: every occurrence of a deliberately-not-lifted family
        // gets an unsupported row; coordinates on <graphic> are unsupported.
        for family in [
            ConservationFamily::Author,
            ConservationFamily::ArticleIdno,
            ConservationFamily::Link,
            ConservationFamily::CrossReference,
            ConservationFamily::Note,
            ConservationFamily::Figure,
        ] {
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
                ConservationFamily::Coordinate,
                ConservationReasonCode::UnsupportedThisCut
            ),
            source.count(ConservationFamily::Coordinate),
            "{}: not-yet-lifted coordinates are typed unsupported",
            fixture.case_id
        );
        assert!(
            delta
                .entries
                .iter()
                .all(|entry| entry.family != ConservationFamily::Coordinate
                    || entry.reason == ConservationReasonCode::UnsupportedThisCut)
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
