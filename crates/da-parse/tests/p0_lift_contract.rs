//! P0 lift wire contract (M464-z84in2 S07/T01; R085, R087, R106).
//!
//! Pins the additive S07 wire model — declared arXiv identity with honest
//! optional revision and typed missingness, document-ordered header authors,
//! non-table figures with graphic-owned coordinates, and figure/table visual
//! callouts with closed kind, target and explicit unresolved state — over the
//! three checksum-bound preserved rich TEI fixtures plus small
//! malformed/legacy JSON cases.
//!
//! S07 stopping condition inputs encoded here as byte-level pins. The byte
//! scanner is an INDEPENDENT oracle (plain byte counting over frozen fixture
//! bytes — it shares no code with the lifter):
//!
//! - header-only authors: `<persName>` inside `<teiHeader>` (21 / 4 / 11);
//!   the hundreds of bibliography `persName` entries (686 / 998 / 573) must
//!   never leak into the author family;
//! - declared arXiv idno: exactly the pinned declared value per fixture,
//!   including its declared revision (`v2` / `v3` / `v1`) — the revision is
//!   kept only because the source declares it; it is never guessed;
//! - table wrappers (`<figure type="table">`) are tables, never figures:
//!   non-table figures are 25−3=22, 31−2=29, 58−3=55;
//! - coordinates: 1 / 25 / 34 `coords` attributes, every one owned by a
//!   `<graphic>` child — coordinates must never disappear;
//! - visual callouts: `<ref type="figure">` / `<ref type="table">` counts
//!   24+5 / 25+11 / 30+11, of which 10+2 / 10+4 / 5+1 carry no declared
//!   target — the typed unresolved state must be representable and such
//!   callouts must never disappear;
//! - legacy JSON predating S07 decodes with typed defaults (R085).
//!
//! S07/T02 extends this contract with lifted-output assertions for the
//! declared arXiv identity and the header authors (exact counts, order,
//! digests, typed missingness, byte-equality of the two public lift paths);
//! S07/T03 extends it with lifted-output assertions for non-table figures
//! (graphic-owned coordinates, typed graphic missingness, table-wrapper
//! exclusion) and figure/table visual callouts (closed kind, declared
//! target, typed unresolved state, bounded offsets, section context); T04
//! converts these families from `unsupported_this_cut` to conservation
//! evidence.
//!
//! Reports are redacting by construction: nothing here prints raw TEI,
//! author names, captions, quotes, URLs or physical paths.

use std::path::{Path, PathBuf};

use da_parse::{GrobidParser, GrobidProfile, TeiEnvelope, TeiIdentity, TeiLifter, TeiParseIr};
use da_ports::parser::{
    CalloutUnresolvedReason, DeclaredArxivIdentity, DeclaredIdMissingReason, ExtractedFigure,
    FigureGraphicMissingReason, HeaderAuthor, ParsedArticle, VisualCallout, VisualCalloutKind,
};
use serde_yaml::Value;
use sha2::{Digest, Sha256};

/// Sanctioned DATA manifest binding fixture paths to frozen SHA-256 checksums
/// (data/parse/regression_cases.yaml; `binding_status: bound` rows only).
const REGRESSION_MANIFEST_REL: &str = "data/parse/regression_cases.yaml";

// ─── Fail-visible fixture integrity (conservation_contract pattern) ─────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a repo-relative file. Missing files are FATAL here: the contract must
/// never silently shrink because a fixture disappeared.
fn read_repo_file(rel_path: &str) -> Vec<u8> {
    let full = repo_root().join(rel_path);
    std::fs::read(&full).unwrap_or_else(|error| {
        panic!("FATAL {rel_path}: fixture missing/unreadable ({error}) — P0 lift contract hard-fails, never skips")
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

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
    bytes: Vec<u8>,
}

/// Load the three bound preserved rich TEI fixtures through the manifest.
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
            .unwrap_or_else(|| panic!("FATAL {case_id}: manifest field `fixture` missing"));
        let path = manifest_field(fixture, "path", &case_id).to_string();
        let sha = manifest_field(fixture, "sha256", &case_id).to_string();
        // Provenance paper id: the fixture filename carries the arXiv paper
        // id (e.g. hub_2507.19457.tei.xml → 2507.19457).
        let file_name = Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("FATAL {case_id}: fixture path has no file name"));
        let paper_id = file_name
            .trim_end_matches(".tei.xml")
            .split_once('_')
            .map(|(_, id)| id.to_string())
            .unwrap_or_else(|| panic!("FATAL {case_id}: cannot derive paper id from file name"));
        let bytes = read_repo_file(&path);
        verify_fixture_sha(&bytes, &sha, &case_id);
        out.push(PreservedFixture {
            case_id,
            paper_id,
            bytes,
        });
    }
    assert_eq!(
        out.len(),
        3,
        "FATAL: expected exactly 3 bound preserved fixtures in the manifest"
    );
    out
}

// ─── Independent byte-level oracle over frozen fixture bytes ────────────────

fn as_tei(bytes: &[u8], case_id: &str) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap_or_else(|error| panic!("FATAL {case_id}: fixture is not UTF-8 ({error})"))
}

fn header_region<'a>(tei: &'a str, case_id: &str) -> &'a str {
    let start = tei
        .find("<teiHeader")
        .unwrap_or_else(|| panic!("FATAL {case_id}: no <teiHeader>"));
    let end = tei
        .find("</teiHeader>")
        .unwrap_or_else(|| panic!("FATAL {case_id}: no </teiHeader>"));
    &tei[start..end]
}

fn body_region<'a>(tei: &'a str, case_id: &str) -> &'a str {
    let end = tei
        .find("</teiHeader>")
        .unwrap_or_else(|| panic!("FATAL {case_id}: no </teiHeader>"));
    &tei[end..]
}

fn count_needle(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// All opening tags whose text starts with `prefix` (tag text includes the
/// terminating `>`), in document order.
fn opening_tags<'a>(haystack: &'a str, prefix: &str, case_id: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = haystack;
    while let Some(pos) = rest.find(prefix) {
        let after = &rest[pos..];
        let tag_end = after
            .find('>')
            .unwrap_or_else(|| panic!("FATAL {case_id}: unterminated opening tag"));
        out.push(&after[..=tag_end]);
        rest = &after[tag_end + 1..];
    }
    out
}

/// The declared arXiv idno value from the header (first
/// `<idno type="arXiv">…</idno>`), verbatim.
fn declared_arxiv_idno(header: &str, case_id: &str) -> String {
    let pos = header
        .find("<idno type=\"arXiv\"")
        .unwrap_or_else(|| panic!("FATAL {case_id}: no declared arXiv idno in header"));
    let after = &header[pos..];
    let tag_end = after
        .find('>')
        .unwrap_or_else(|| panic!("FATAL {case_id}: unterminated idno tag"));
    let rest = &after[tag_end + 1..];
    let end = rest
        .find("</idno>")
        .unwrap_or_else(|| panic!("FATAL {case_id}: unterminated idno value"));
    rest[..end].to_string()
}

/// Per-fixture S07 byte pins (stopping condition inputs), probed from the
/// frozen bytes and cross-checked against the manifest notes.
struct S07BytePins {
    header_authors: usize,
    authors_with_forename: usize,
    authors_with_surname: usize,
    non_table_figures: usize,
    table_wrappers: usize,
    total_figure_tags: usize,
    coords: usize,
    declared_idno_raw: &'static str,
    declared_bare_id: &'static str,
    declared_revision: Option<&'static str>,
    figure_refs: usize,
    figure_refs_without_target: usize,
    table_refs: usize,
    table_refs_without_target: usize,
    /// Callouts outside every section window (back-matter refs): present in
    /// the family, but with typed-unresolved section state.
    out_of_window_callouts: usize,
    body_persname_not_authors: usize,
}

fn pins_for(case_id: &str) -> S07BytePins {
    match case_id {
        "reg.tei.hub-bibliography-2507.19457" => S07BytePins {
            header_authors: 21,
            authors_with_forename: 20,
            authors_with_surname: 21,
            non_table_figures: 22,
            table_wrappers: 3,
            total_figure_tags: 25,
            coords: 1,
            declared_idno_raw: "arXiv:2507.19457v2[cs.CL]",
            declared_bare_id: "2507.19457",
            declared_revision: Some("v2"),
            figure_refs: 24,
            figure_refs_without_target: 10,
            table_refs: 5,
            table_refs_without_target: 2,
            out_of_window_callouts: 0,
            body_persname_not_authors: 686,
        },
        "reg.tei.rlm-2512.24601" => S07BytePins {
            header_authors: 4,
            authors_with_forename: 4,
            authors_with_surname: 4,
            non_table_figures: 29,
            table_wrappers: 2,
            total_figure_tags: 31,
            coords: 25,
            declared_idno_raw: "arXiv:2512.24601v3[cs.AI]",
            declared_bare_id: "2512.24601",
            declared_revision: Some("v3"),
            figure_refs: 25,
            figure_refs_without_target: 10,
            table_refs: 11,
            table_refs_without_target: 4,
            out_of_window_callouts: 14,
            body_persname_not_authors: 998,
        },
        "reg.tei.reca-bibliography-2605.26525" => S07BytePins {
            header_authors: 11,
            authors_with_forename: 11,
            authors_with_surname: 11,
            non_table_figures: 55,
            table_wrappers: 3,
            total_figure_tags: 58,
            coords: 34,
            declared_idno_raw: "arXiv:2605.26525v1[cs.CV]",
            declared_bare_id: "2605.26525",
            declared_revision: Some("v1"),
            figure_refs: 30,
            figure_refs_without_target: 5,
            table_refs: 11,
            table_refs_without_target: 1,
            out_of_window_callouts: 0,
            body_persname_not_authors: 573,
        },
        other => panic!("FATAL {other}: no S07 byte pins registered"),
    }
}

fn has_declared_target(opening_tag: &str) -> bool {
    // `target="…"` with a non-empty value; empty target = typed NoTarget.
    match opening_tag.find("target=\"") {
        Some(pos) => {
            let rest = &opening_tag[pos + "target=\"".len()..];
            !rest.starts_with('"')
        }
        None => false,
    }
}

// ─── Contract tests ──────────────────────────────────────────────────────────

/// Fixture integrity is fail-visible: missing bindings or checksum drift
/// hard-fail above, never skip.
#[test]
fn preserved_fixtures_are_checksum_bound() {
    let fixtures = preserved_bound_fixtures();
    assert_eq!(fixtures.len(), 3);
    for fixture in &fixtures {
        assert!(
            !fixture.bytes.is_empty(),
            "{}: empty fixture",
            fixture.case_id
        );
    }
}

/// Byte-level stopping condition inputs for every family S07 must lift:
/// header-only authors, declared idno, non-table figures vs table wrappers,
/// graphic-owned coordinates, and figure/table callouts (with and without a
/// declared target).
#[test]
fn fixture_bytes_pin_the_s07_stopping_condition_inputs() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let pins = pins_for(case);
        let tei = as_tei(&fixture.bytes, case);
        let header = header_region(&tei, case);
        let body = body_region(&tei, case);

        // Header-only authors: persName inside the TEI header only.
        assert_eq!(
            count_needle(header, "<persName"),
            pins.header_authors,
            "{case}: header author pin"
        );

        // Declared arXiv identity: exactly the pinned declared value.
        let idno = declared_arxiv_idno(header, case);
        assert_eq!(idno, pins.declared_idno_raw, "{case}: declared idno raw");

        // Table wrappers are excluded from the figure family.
        let figure_tags = opening_tags(body, "<figure", case);
        assert_eq!(
            figure_tags.len(),
            pins.total_figure_tags,
            "{case}: total <figure> tags"
        );
        let wrappers = figure_tags
            .iter()
            .filter(|tag| tag.contains("type=\"table\""))
            .count();
        assert_eq!(wrappers, pins.table_wrappers, "{case}: table wrappers");
        assert_eq!(
            figure_tags.len() - wrappers,
            pins.non_table_figures,
            "{case}: non-table figures"
        );

        // Every fixture figure carries a declared xml:id (the wire model's
        // figure_id convention keeps the honest ordinal fallback for TEI
        // that lacks one).
        let with_id = opening_tags(body, "<figure", case)
            .iter()
            .filter(|tag| tag.contains("xml:id="))
            .count();
        assert_eq!(with_id, pins.total_figure_tags, "{case}: figure xml:ids");

        // Coordinates: graphic-owned, never invented.
        assert_eq!(
            count_needle(body, "coords=\""),
            pins.coords,
            "{case}: coords attributes"
        );
        let graphics_with_coords = opening_tags(body, "<graphic", case)
            .iter()
            .filter(|tag| tag.contains("coords=\""))
            .count();
        assert_eq!(
            graphics_with_coords, pins.coords,
            "{case}: every coords attribute is graphic-owned"
        );

        // Visual callouts: closed kind split, with typed no-target surface.
        let figure_ref_tags = opening_tags(body, "<ref type=\"figure\"", case);
        assert_eq!(
            figure_ref_tags.len(),
            pins.figure_refs,
            "{case}: figure callouts"
        );
        assert_eq!(
            figure_ref_tags
                .iter()
                .filter(|tag| !has_declared_target(tag))
                .count(),
            pins.figure_refs_without_target,
            "{case}: figure callouts without target"
        );
        let table_ref_tags = opening_tags(body, "<ref type=\"table\"", case);
        assert_eq!(
            table_ref_tags.len(),
            pins.table_refs,
            "{case}: table callouts"
        );
        assert_eq!(
            table_ref_tags
                .iter()
                .filter(|tag| !has_declared_target(tag))
                .count(),
            pins.table_refs_without_target,
            "{case}: table callouts without target"
        );

        // Bibliography persName entries are citations, never authors — the
        // header-only rule must keep them out of the author family.
        assert!(
            pins.body_persname_not_authors > pins.header_authors,
            "{case}: bibliography authors must dwarf the header count for this pin to be meaningful"
        );
        assert_eq!(
            count_needle(body, "<persName"),
            pins.body_persname_not_authors,
            "{case}: bibliography persName byte pin"
        );
    }
}

/// The table-wrapper exclusion rule the lifter must implement: wrappers are
/// counted separately from non-table figures, and the two never overlap —
/// on the frozen bytes AND in the lifted output (no double-counting, the
/// id sets are disjoint, and the counts sum to the total figure tags).
#[test]
fn table_wrappers_are_never_counted_as_figures() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let pins = pins_for(case);
        let tei = as_tei(&fixture.bytes, case);
        let body = body_region(&tei, case);
        let figure_tags = opening_tags(body, "<figure", case);
        let wrappers = figure_tags
            .iter()
            .filter(|tag| tag.contains("type=\"table\""))
            .count();
        let non_table = figure_tags.len() - wrappers;
        assert_eq!(non_table, pins.non_table_figures, "{case}");
        assert_eq!(wrappers, pins.table_wrappers, "{case}");
        assert_eq!(
            non_table + wrappers,
            pins.total_figure_tags,
            "{case}: classification is total, nothing falls through"
        );

        // Lifted output keeps the same partition: figures and tables are
        // disjoint families whose sizes sum to the total wrapper count.
        let ir = TeiLifter::lift_tei(tei, fixture.paper_id.clone(), "s07-t03-wrapper-hash");
        assert_eq!(ir.figures.len(), non_table, "{case}: lifted figure count");
        assert_eq!(ir.tables.len(), wrappers, "{case}: lifted table count");
        let table_ids: std::collections::HashSet<&str> =
            ir.tables.iter().map(|t| t.table_id.as_str()).collect();
        for figure in &ir.figures {
            assert!(
                !table_ids.contains(figure.figure_id.as_str()),
                "{case}: figure '{}' must not double-count as a table",
                figure.figure_id
            );
        }
        assert_eq!(
            ir.figures.len() + ir.tables.len(),
            pins.total_figure_tags,
            "{case}: lifted partition covers every figure element"
        );
    }
}

/// Declared identity honesty on the wire: a revision-less declared value
/// roundtrips with `revision: null` (never guessed to v1), while a declared
/// revision survives verbatim.
#[test]
fn declared_revision_is_never_guessed() {
    let plain = DeclaredArxivIdentity {
        raw: "2507.19457".into(),
        bare_id: "2507.19457".into(),
        revision: None,
        source_digest: sha256_hex(b"2507.19457"),
    };
    let json = serde_json::to_string(&plain).unwrap();
    assert!(
        json.contains("\"revision\":null"),
        "wire keeps null revision for an undeclared revision"
    );
    let back: DeclaredArxivIdentity = serde_json::from_str(&json).unwrap();
    assert_eq!(back.revision, None, "revision is never guessed to v1");
    assert_eq!(back, plain);

    let declared = DeclaredArxivIdentity {
        raw: "arXiv:2507.19457v2[cs.CL]".into(),
        bare_id: "2507.19457".into(),
        revision: Some("v2".into()),
        source_digest: sha256_hex(b"arXiv:2507.19457v2[cs.CL]"),
    };
    let json = serde_json::to_string(&declared).unwrap();
    assert!(json.contains("\"revision\":\"v2\""));
    let back: DeclaredArxivIdentity = serde_json::from_str(&json).unwrap();
    assert_eq!(back, declared, "declared revision survives verbatim");
}

/// Header-only authors on the wire: document order is preserved and
/// incomplete names (no decomposition) survive without silent drop.
#[test]
fn header_only_authors_keep_document_order_and_incomplete_names() {
    let json = r#"[
        {"name":"Ozan İrsoy","forename":"Ozan","surname":"İrsoy","source_digest":"aaaa"},
        {"name":"Yu","source_digest":"bbbb"}
    ]"#;
    let authors: Vec<HeaderAuthor> = serde_json::from_str(json).expect("authors decode");
    assert_eq!(authors.len(), 2);
    // Order is exactly the document order of the JSON array.
    assert_eq!(authors[0].name, "Ozan İrsoy");
    assert_eq!(authors[0].forename.as_deref(), Some("Ozan"));
    assert_eq!(authors[0].surname.as_deref(), Some("İrsoy"));
    // Incomplete name: decomposition is absent, name survives untouched —
    // never dropped, never split by guesswork.
    assert_eq!(authors[1].name, "Yu");
    assert!(authors[1].forename.is_none());
    assert!(authors[1].surname.is_none());

    // Roundtrip keeps order and content byte-stable.
    let json2 = serde_json::to_string(&authors).unwrap();
    let back: Vec<HeaderAuthor> = serde_json::from_str(&json2).unwrap();
    assert_eq!(back, authors);
}

/// Coordinates and unresolved callout state survive the wire: graphic-owned
/// coordinates, typed missingness, closed kinds and explicit unresolved
/// reasons all roundtrip byte-stably (serde snake_case tags).
#[test]
fn coordinates_and_unresolved_callouts_survive_wire_roundtrip() {
    let figure = ExtractedFigure {
        figure_id: "fig_0".into(),
        label: Some("Figure 1".into()),
        caption: Some("Overview of the approach".into()),
        coordinates: Some("90,91.80,190.43,428.38,331.74".into()),
        graphic_missingness: None,
        source_digest: "d".repeat(64),
    };
    let figure_without_coords = ExtractedFigure {
        figure_id: "figure-7".into(),
        label: None,
        caption: None,
        coordinates: None,
        graphic_missingness: Some(FigureGraphicMissingReason::GraphicWithoutCoordinates),
        source_digest: "e".repeat(64),
    };
    let resolved = VisualCallout {
        kind: VisualCalloutKind::Figure,
        target: Some("fig_0".into()),
        unresolved: None,
        quote: "[12]".into(),
        offset: Some(41),
        section_path_id: Some("3.2".into()),
        section_ordinal: Some(1),
        source_digest: "f".repeat(64),
    };
    let unresolved = VisualCallout {
        kind: VisualCalloutKind::Table,
        target: None,
        unresolved: Some(CalloutUnresolvedReason::NoTarget),
        quote: "Table 3".into(),
        offset: None,
        section_path_id: None,
        section_ordinal: None,
        source_digest: "0".repeat(64),
    };

    // Structural roundtrip equality (the wire never loses coordinates or
    // unresolved state).
    let json = serde_json::to_string(&figure).unwrap();
    let back: ExtractedFigure = serde_json::from_str(&json).unwrap();
    assert_eq!(back, figure, "coordinates survive roundtrip");
    assert_eq!(
        back.coordinates.as_deref(),
        Some("90,91.80,190.43,428.38,331.74")
    );

    let json = serde_json::to_string(&figure_without_coords).unwrap();
    assert!(
        json.contains("graphic_without_coordinates"),
        "typed graphic missingness survives: {json}"
    );
    let back: ExtractedFigure = serde_json::from_str(&json).unwrap();
    assert_eq!(back, figure_without_coords);

    let json = serde_json::to_string(&unresolved).unwrap();
    assert!(
        json.contains("\"kind\":\"table\""),
        "closed kind tag: {json}"
    );
    assert!(json.contains("no_target"), "typed unresolved tag: {json}");
    let back: VisualCallout = serde_json::from_str(&json).unwrap();
    assert_eq!(back, unresolved, "unresolved state survives roundtrip");

    let json = serde_json::to_string(&resolved).unwrap();
    assert!(json.contains("\"target\":\"fig_0\""));
    assert!(
        json.contains("\"unresolved\":null"),
        "resolved state stays explicit-null on the wire"
    );
    let back: VisualCallout = serde_json::from_str(&json).unwrap();
    assert_eq!(back, resolved, "resolved callout stays resolved");
}

/// Typed idno missingness on the wire: `no_idno` and `malformed` roundtrip
/// with stable snake_case tags (R087 — the lifter must be able to SAY why
/// no identity was declared, never guess one).
#[test]
fn declared_id_missingness_is_typed_and_never_repaired() {
    for (reason, tag) in [
        (DeclaredIdMissingReason::NoIdno, "\"no_idno\"".to_string()),
        (
            DeclaredIdMissingReason::Malformed("no parseable arxiv id".into()),
            "{\"malformed\":\"no parseable arxiv id\"}".to_string(),
        ),
    ] {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, tag);
        roundtrip_assert(&reason);
    }
}

fn roundtrip_assert<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json1 = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json1).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

/// (R085) Legacy ParsedArticle JSON predating S07 decodes with typed
/// defaults — the additive families never break older cached payloads.
#[test]
fn legacy_parsed_article_json_decodes_with_s07_defaults() {
    let json = r#"{"paper_id":"2507.19457","title":"GEPA","abstract_text":"a","body_text":"b","sections":[],"citations":[],"layout_json":null,"tei_xml":null,"pdf_hash":"abc123"}"#;
    let article: ParsedArticle = serde_json::from_str(json).expect("legacy ParsedArticle decodes");
    assert!(article.declared_arxiv_id.is_none());
    assert!(article.declared_id_missingness.is_none());
    assert!(article.header_authors.is_empty());
    assert!(article.figures.is_empty());
    assert!(article.visual_callouts.is_empty());
}

/// (R085) Legacy TeiParseIr JSON (the full pre-S07 field set, no S07
/// families) decodes with typed defaults — old IR payloads stay readable.
#[test]
fn legacy_tei_parse_ir_json_decodes_with_s07_defaults() {
    let json = r#"{"tei_xml":"<tei/>","paper_id":"2507.19457","title":"T","abstract_text":"A","body_text":"B","sections":[],"citations":[],"tables":[],"table_missingness":null,"formulas":[],"formula_missingness":null,"pdf_hash":"abc123"}"#;
    let ir: TeiParseIr = serde_json::from_str(json).expect("legacy TeiParseIr decodes");
    assert!(ir.declared_arxiv_id.is_none());
    assert!(ir.declared_id_missingness.is_none());
    assert!(ir.header_authors.is_empty());
    assert!(ir.figures.is_empty());
    assert!(ir.visual_callouts.is_empty());
    assert!(ir.tables.is_empty());
    assert!(ir.formulas.is_empty());
}

// ─── S07/T02: lifted-output contract for the declared id + header authors ───

/// S07/T02 lifted-output contract: the single pure lifter produces exactly
/// the pinned declared identity (verbatim raw, parsed bare id, explicitly
/// declared revision — never guessed, never defaulted) and exactly the
/// header-scoped author count per fixture, with document order, declared
/// decomposition coverage and digest-bound display names. Since S07/T03 the
/// figure/callout families lift too (exact counts pinned here; full shape
/// pins in the T03 contract below), and every previously accepted family
/// keeps lifting unchanged.
#[test]
fn t02_lifter_lifts_declared_id_and_header_authors_exactly_on_all_fixtures() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let pins = pins_for(case);
        let ir = TeiLifter::lift_tei(
            as_tei(&fixture.bytes, case),
            fixture.paper_id.clone(),
            "s07-t02-contract-hash",
        );

        // Declared identity: exactly the oracle-extracted declared value,
        // with the bare id and the explicitly declared revision.
        let declared = ir
            .declared_arxiv_id
            .as_ref()
            .unwrap_or_else(|| panic!("{case}: declared identity must lift"));
        assert_eq!(declared.raw, pins.declared_idno_raw, "{case}: raw verbatim");
        assert_eq!(declared.bare_id, pins.declared_bare_id, "{case}: bare id");
        assert_eq!(
            declared.revision.as_deref(),
            pins.declared_revision,
            "{case}: declared revision (declared v1 stays Some, absent stays None)"
        );
        assert_eq!(
            declared.source_digest,
            sha256_hex(pins.declared_idno_raw.as_bytes()),
            "{case}: identity digest binds the declared value"
        );
        assert!(
            ir.declared_id_missingness.is_none(),
            "{case}: usable idno carries no missingness"
        );

        // Header authors: exactly the header-scoped count (the hundreds of
        // bibliography persName entries never leak in), declared-only
        // decomposition, non-empty tag-free display names, digest-bound.
        assert_eq!(
            ir.header_authors.len(),
            pins.header_authors,
            "{case}: header author count"
        );
        let with_fore = ir
            .header_authors
            .iter()
            .filter(|a| a.forename.is_some())
            .count();
        let with_sur = ir
            .header_authors
            .iter()
            .filter(|a| a.surname.is_some())
            .count();
        assert_eq!(with_fore, pins.authors_with_forename, "{case}: forenames");
        assert_eq!(with_sur, pins.authors_with_surname, "{case}: surnames");
        for author in &ir.header_authors {
            assert!(!author.name.is_empty(), "{case}: no silent author drop");
            assert!(
                !author.name.contains('<'),
                "{case}: display name is tag-free"
            );
            assert_eq!(
                author.source_digest,
                sha256_hex(author.name.as_bytes()),
                "{case}: author digest binds the display name"
            );
        }

        // Deterministic replay: a second lift is byte-equal on the wire.
        let replay = TeiLifter::lift_tei(
            as_tei(&fixture.bytes, case),
            fixture.paper_id.clone(),
            "s07-t02-contract-hash",
        );
        assert_eq!(
            serde_json::to_vec(&replay).unwrap(),
            serde_json::to_vec(&ir).unwrap(),
            "{case}: two lifts are byte-equal"
        );

        // S07/T03 families lift through the same pure algorithm: exact
        // ordered counts per fixture (shape pins in the T03 contract test).
        assert_eq!(
            ir.figures.len(),
            pins.non_table_figures,
            "{case}: non-table figures lift in document order"
        );
        assert_eq!(
            ir.visual_callouts.len(),
            pins.figure_refs + pins.table_refs,
            "{case}: figure/table callouts lift in document order"
        );
        // Previously accepted families keep lifting (projection-level pins
        // live in the real-TEI suite; byte-level baseline rotation is T05).
        assert!(!ir.title.is_empty(), "{case}: title still lifts");
        assert!(!ir.abstract_text.is_empty(), "{case}: abstract still lifts");
        assert!(!ir.body_text.is_empty(), "{case}: body still lifts");
        assert!(!ir.sections.is_empty(), "{case}: sections still lift");
        assert!(!ir.citations.is_empty(), "{case}: citations still lift");
        assert_eq!(
            ir.tables.len(),
            pins.table_wrappers,
            "{case}: table family unchanged by the S07 additions"
        );
        assert!(
            ir.formula_missingness.is_none(),
            "{case}: formula family unchanged by the S07 additions"
        );
    }
}

/// The two public lift paths — the recorded `parse.stage.lift_tei`
/// compatibility stage and the provider-neutral envelope path — share the
/// single algorithm: serialized IRs are byte-equal, the lifted S07/T02+T03
/// families match field-for-field, and the projected articles are byte-equal
/// on every fixture.
#[test]
fn t02_two_public_lift_paths_are_byte_equal_on_the_lifted_families() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let bytes = fixture.bytes.clone();
        let compat = GrobidParser::lift_tei(
            as_tei(&bytes, case),
            fixture.paper_id.clone(),
            "s07-t02-contract-hash",
        );
        let envelope_ir = TeiLifter::new().lift(
            &TeiEnvelope::new(bytes, TeiIdentity::cached_tei(GrobidProfile::rich())),
            fixture.paper_id.clone(),
            "s07-t02-contract-hash",
        );
        assert_eq!(
            serde_json::to_vec(&compat).unwrap(),
            serde_json::to_vec(&envelope_ir).unwrap(),
            "{case}: serialized IR diverges between the two public lift paths"
        );
        assert_eq!(
            compat.declared_arxiv_id, envelope_ir.declared_arxiv_id,
            "{case}: declared identity matches across paths"
        );
        assert_eq!(
            compat.header_authors, envelope_ir.header_authors,
            "{case}: header authors match across paths"
        );
        assert_eq!(
            compat.figures, envelope_ir.figures,
            "{case}: figures match across paths"
        );
        assert_eq!(
            compat.visual_callouts, envelope_ir.visual_callouts,
            "{case}: visual callouts match across paths"
        );
        let compat_article = GrobidParser::project_article(compat);
        let envelope_article = GrobidParser::project_article(envelope_ir);
        assert_eq!(
            serde_json::to_vec(&compat_article).unwrap(),
            serde_json::to_vec(&envelope_article).unwrap(),
            "{case}: projected articles diverge between the two public lift paths"
        );
    }
}

/// Typed missingness stays honest on every malformed header shape, and
/// malformed input stays TOTAL: no header at all, no arXiv idno (other idno
/// types only), empty value, value without a parseable arXiv id, digit run
/// that fails the modern id shape, unterminated element — each types the
/// absence instead of guessing or panicking. A revision-less declared value
/// keeps `revision: None` (never defaulted to v1), and namespace
/// declarations plus attribute order never block the lift.
#[test]
fn t02_declared_id_missingness_is_honest_on_malformed_headers() {
    // No header at all: total, typed absence (empty input too).
    for tei in ["", "<TEI><text><body><p>x</p></body></text></TEI>"] {
        let ir = TeiLifter::lift_tei(tei.to_string(), "p", "h");
        assert!(ir.declared_arxiv_id.is_none(), "no header: no identity");
        assert_eq!(
            ir.declared_id_missingness,
            Some(DeclaredIdMissingReason::NoIdno),
            "no header: typed no_idno"
        );
        assert!(ir.header_authors.is_empty(), "no header: no authors");
    }

    // DOI-only header: the arXiv identity is absent, typed — never guessed
    // from the DOI or the paper_id.
    let doi_only = r#"<TEI><teiHeader><fileDesc><sourceDesc><biblStruct><idno type="DOI">10.1/x</idno></biblStruct></sourceDesc></fileDesc></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(doi_only.to_string(), "p", "h");
    assert!(ir.declared_arxiv_id.is_none());
    assert_eq!(
        ir.declared_id_missingness,
        Some(DeclaredIdMissingReason::NoIdno)
    );

    // Empty idno value: declared but unusable → typed malformed.
    let empty = r#"<TEI><teiHeader><title>T</title><idno type="arXiv">  </idno></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(empty.to_string(), "p", "h");
    assert!(ir.declared_arxiv_id.is_none());
    assert_eq!(
        ir.declared_id_missingness,
        Some(DeclaredIdMissingReason::Malformed(
            "declared arXiv idno is empty".to_string()
        ))
    );
    assert_eq!(
        ir.title, "T",
        "malformed identity stays total for other families"
    );

    // Value without any parseable arXiv id: typed malformed, never repaired.
    let garbage = r#"<TEI><teiHeader><idno type="arXiv">not-an-arxiv-id</idno></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(garbage.to_string(), "p", "h");
    assert!(ir.declared_arxiv_id.is_none());
    assert_eq!(
        ir.declared_id_missingness,
        Some(DeclaredIdMissingReason::Malformed(
            "declared arXiv idno carries no parseable arXiv id".to_string()
        ))
    );

    // A digit run that fails the modern id shape (6-digit fraction) stays
    // malformed — no partial repair.
    let too_long = r#"<TEI><teiHeader><idno type="arXiv">2507.194571</idno></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(too_long.to_string(), "p", "h");
    assert!(ir.declared_arxiv_id.is_none());
    assert!(matches!(
        ir.declared_id_missingness,
        Some(DeclaredIdMissingReason::Malformed(_))
    ));

    // Unterminated idno element: typed malformed.
    let unterminated = r#"<TEI><teiHeader><idno type="arXiv">arXiv:2507.19457</teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(unterminated.to_string(), "p", "h");
    assert!(ir.declared_arxiv_id.is_none());
    assert_eq!(
        ir.declared_id_missingness,
        Some(DeclaredIdMissingReason::Malformed(
            "declared arXiv idno element is unterminated".to_string()
        ))
    );

    // Revision-less declared value: revision stays None — never guessed v1.
    let revisionless =
        r#"<TEI><teiHeader><idno type="arXiv">arXiv:2507.19457</idno></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(revisionless.to_string(), "p", "h");
    let declared = ir
        .declared_arxiv_id
        .expect("parseable declared value lifts");
    assert_eq!(declared.raw, "arXiv:2507.19457");
    assert_eq!(declared.bare_id, "2507.19457");
    assert_eq!(declared.revision, None, "revision is never defaulted to v1");
    assert!(ir.declared_id_missingness.is_none());

    // Namespace declarations and attribute order never block the lift.
    let namespaced = r#"<TEI><teiHeader><idno xmlns="http://arxiv.org/schemas/atomic" type="arXiv">arXiv:2507.19457v2</idno></teiHeader></TEI>"#;
    let ir = TeiLifter::lift_tei(namespaced.to_string(), "p", "h");
    let declared = ir
        .declared_arxiv_id
        .expect("namespaced declared value lifts");
    assert_eq!(declared.bare_id, "2507.19457");
    assert_eq!(declared.revision.as_deref(), Some("v2"));
    assert!(ir.declared_id_missingness.is_none());
}

/// Header-only author scoping, document order, Unicode/incomplete names and
/// count honesty: back-matter (bibliography) and body persName entries NEVER
/// become authors, a plain-text `<author>` without persName is outside the
/// declared family, an empty persName stays an empty-name entry, and
/// decomposition comes only from declared forename/surname children (joined
/// in document order — GROBID emits children without inter-element
/// whitespace, so boundary-joined text is the display name).
#[test]
fn t02_header_authors_scoping_order_and_incomplete_names() {
    let tei = concat!(
        r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader xml:lang="en"><fileDesc><titleStmt>"#,
        r#"<title>T</title>"#,
        // Plain-text author without persName: outside the declared family.
        r#"<author>Plain Text Author</author>"#,
        // Full Unicode decomposition with two declared forenames.
        r#"<author><persName><forename type="first">Mira</forename><forename type="middle">Ş</forename><surname>Öztürk-Güvenç</surname></persName></author>"#,
        // Incomplete name: surname only — preserved, never dropped or split.
        r#"<author><persName><surname>Mit</surname></persName></author>"#,
        // Declared-empty entry: count honesty keeps the entry.
        r#"<author><persName></persName></author>"#,
        r#"</titleStmt></fileDesc></teiHeader>"#,
        // Body persName (in-text) is not an author either.
        r#"<text><body><p>mentioned <persName><forename>Inline</forename><surname>Body</surname></persName> here</p></body>"#,
        // Bibliography persName entries (back matter) are citations.
        r#"<back><listBibl>"#,
        r#"<biblStruct><persName><forename>Bib</forename><surname>First</surname></persName></biblStruct>"#,
        r#"<biblStruct><persName><forename>Bib</forename><surname>Second</surname></persName></biblStruct>"#,
        r#"</listBibl></back></text></TEI>"#,
    );
    let ir = TeiLifter::lift_tei(tei.to_string(), "p", "h");
    assert_eq!(
        ir.header_authors.len(),
        3,
        "exactly the header persName entries lift"
    );
    // Document order is preserved and decomposition is declared-only.
    assert_eq!(ir.header_authors[0].name, "Mira Ş Öztürk-Güvenç");
    assert_eq!(ir.header_authors[0].forename.as_deref(), Some("Mira Ş"));
    assert_eq!(
        ir.header_authors[0].surname.as_deref(),
        Some("Öztürk-Güvenç")
    );
    // Incomplete name: surname-only decomposition survives untouched.
    assert_eq!(ir.header_authors[1].name, "Mit");
    assert_eq!(ir.header_authors[1].forename, None);
    assert_eq!(ir.header_authors[1].surname.as_deref(), Some("Mit"));
    // Declared-empty persName stays an empty-name entry (count honesty).
    assert_eq!(ir.header_authors[2].name, "");
    assert_eq!(ir.header_authors[2].forename, None);
    assert_eq!(ir.header_authors[2].surname, None);
    // Bibliography and body persName entries are excluded from the family.
    assert!(!ir.header_authors.iter().any(|a| a.name == "Bib First"));
    assert!(!ir.header_authors.iter().any(|a| a.name == "Bib Second"));
    assert!(!ir.header_authors.iter().any(|a| a.name == "Inline Body"));
    assert!(
        !ir.header_authors
            .iter()
            .any(|a| a.name == "Plain Text Author")
    );
    // Deterministic replay on the synthetic shape too.
    let replay = TeiLifter::lift_tei(tei.to_string(), "p", "h");
    assert_eq!(replay.header_authors, ir.header_authors);
}

// ─── S07/T03: lifted-output contract for figures + visual callouts ──────────

/// S07/T03 lifted-output contract: the single pure lifter produces exactly
/// the byte-oracle quantities per fixture — document-ordered non-table
/// figures with graphic-owned coordinates and typed graphic missingness
/// (table wrappers never double-counted), and figure/table visual callouts
/// with closed kind, declared target, typed unresolved state, bounded
/// offsets and section context. All targeted callouts resolve against their
/// family inventory on these fixtures; target-less callouts stay `no_target`
/// and rlm's back-matter callouts stay section-unresolved — present, never
/// dropped.
#[test]
fn t03_figures_and_visual_callouts_lift_exactly_on_all_fixtures() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let pins = pins_for(case);
        let ir = TeiLifter::lift_tei(
            as_tei(&fixture.bytes, case),
            fixture.paper_id.clone(),
            "s07-t03-contract-hash",
        );

        // Figures: exact ordered count, graphic-owned coordinates, typed
        // missingness, 64-hex digests, unique declared ids.
        assert_eq!(
            ir.figures.len(),
            pins.non_table_figures,
            "{case}: non-table figure count"
        );
        assert_eq!(
            ir.figures
                .iter()
                .filter(|f| f.coordinates.is_some())
                .count(),
            pins.coords,
            "{case}: graphic-owned coordinates"
        );
        assert!(
            ir.figures.iter().all(|f| f.graphic_missingness.is_none()
                || f.graphic_missingness == Some(FigureGraphicMissingReason::NoGraphicChild)),
            "{case}: fixtures carry no unusable graphic shapes"
        );
        assert_eq!(
            ir.figures
                .iter()
                .filter(
                    |f| f.graphic_missingness == Some(FigureGraphicMissingReason::NoGraphicChild)
                )
                .count(),
            pins.non_table_figures - pins.coords,
            "{case}: graphic-missing decomposition mirrors the coords pin"
        );
        let mut ids: Vec<&str> = ir.figures.iter().map(|f| f.figure_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            pins.non_table_figures,
            "{case}: unique figure ids"
        );
        for figure in &ir.figures {
            assert_eq!(figure.source_digest.len(), 64, "{case}: figure digest");
        }

        // Callouts: exact kind split, typed unresolved state, bounded
        // offsets, section context (out-of-window callouts stay typed).
        assert_eq!(
            ir.visual_callouts.len(),
            pins.figure_refs + pins.table_refs,
            "{case}: total callouts"
        );
        assert_eq!(
            ir.visual_callouts
                .iter()
                .filter(|c| c.kind == VisualCalloutKind::Figure)
                .count(),
            pins.figure_refs,
            "{case}: figure callouts"
        );
        assert_eq!(
            ir.visual_callouts
                .iter()
                .filter(|c| c.kind == VisualCalloutKind::Table)
                .count(),
            pins.table_refs,
            "{case}: table callouts"
        );
        assert_eq!(
            ir.visual_callouts
                .iter()
                .filter(|c| c.unresolved == Some(CalloutUnresolvedReason::NoTarget))
                .count(),
            pins.figure_refs_without_target + pins.table_refs_without_target,
            "{case}: target-less callouts stay typed no_target"
        );
        assert!(
            ir.visual_callouts
                .iter()
                .all(|c| c.unresolved != Some(CalloutUnresolvedReason::TargetNotInInventory)),
            "{case}: every fixture-declared target resolves in its inventory"
        );
        assert_eq!(
            ir.visual_callouts
                .iter()
                .filter(|c| c.section_path_id.is_none())
                .count(),
            pins.out_of_window_callouts,
            "{case}: section-unresolved callouts stay typed, never dropped"
        );
        for callout in &ir.visual_callouts {
            assert_eq!(callout.source_digest.len(), 64, "{case}: callout digest");
            if let (Some(path), Some(ordinal)) =
                (callout.section_path_id.as_deref(), callout.section_ordinal)
            {
                assert_eq!(
                    ir.sections[ordinal].path_id.as_deref(),
                    Some(path),
                    "{case}: callout ordinal points at its section"
                );
                if let Some(offset) = callout.offset {
                    let chars: Vec<char> = ir.sections[ordinal].text.chars().collect();
                    let start = offset as usize;
                    let end = (start + callout.quote.chars().count()).min(chars.len());
                    assert_eq!(
                        chars.get(start..end).map(|s| s.iter().collect::<String>()),
                        Some(callout.quote.clone()),
                        "{case}: offset {offset} must point at the quote in '{path}'"
                    );
                }
            } else {
                assert!(
                    callout.section_ordinal.is_none() && callout.offset.is_none(),
                    "{case}: out-of-window callouts stay fully section-unresolved"
                );
            }
        }

        // Deterministic replay on the synthetic surface too.
        let replay = TeiLifter::lift_tei(
            as_tei(&fixture.bytes, case),
            fixture.paper_id.clone(),
            "s07-t03-contract-hash",
        );
        assert_eq!(replay.figures, ir.figures, "{case}: figure replay");
        assert_eq!(
            replay.visual_callouts, ir.visual_callouts,
            "{case}: callout replay"
        );
    }
}

/// S07/T03 malformed shapes stay TOTAL and typed: figures without a graphic
/// child, graphics without coordinates and empty coords attributes surface
/// as typed graphic missingness (never invented coordinates); callouts with
/// an unresolvable target keep the typed `target_not_in_inventory` state;
/// the ordinal `figure-{index}` fallback covers figures without xml:id; a
/// target-less callout in a section text window carries the bounded offset;
/// and a document with no figures/refs at all lifts typed-empty.
#[test]
fn t03_malformed_figures_and_unresolved_callouts_stay_total_and_typed() {
    let tei = concat!(
        r##"<TEI><teiHeader><title>T</title></teiHeader><text><body>"##,
        r##"<div><head n="1">S</head>"##,
        r##"<p>See <ref type="figure" target="#fig_x">Fig</ref>, <ref type="table">Tab</ref> and <ref type="figure" target="#nope">X</ref>.</p>"##,
        // Resolved graphic-owned coords.
        r##"<figure xml:id="fig_x"><head>Figure 1</head><label>1</label><graphic coords="4,1,2,3,4" type="bitmap" /></figure>"##,
        // No graphic child at all.
        r##"<figure xml:id="fig_y"><figDesc>no graphic</figDesc></figure>"##,
        // Graphic present but coords-less.
        r##"<figure xml:id="fig_z"><graphic type="bitmap" /></figure>"##,
        // Graphic coords declared but empty: unusable shape, typed.
        r##"<figure xml:id="fig_w"><graphic coords="" /></figure>"##,
        // No xml:id: honest ordinal fallback identity.
        r##"<figure><head>No id</head></figure>"##,
        // Table wrapper: the table family's member, never a figure.
        r##"<figure type="table" xml:id="tab_0"><head>T</head><table><row><cell>c</cell></row></table></figure>"##,
        r##"</div>"##,
        r##"</body></text></TEI>"##,
    );
    let ir = TeiLifter::lift_tei(tei.to_string(), "p", "h");

    // Figures: document-ordered, ids + typed graphic missingness exact.
    let ids: Vec<&str> = ir.figures.iter().map(|f| f.figure_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["fig_x", "fig_y", "fig_z", "fig_w", "figure-4"],
        "document order with the ordinal fallback for the id-less figure"
    );
    assert_eq!(
        ir.figures[0].coordinates.as_deref(),
        Some("4,1,2,3,4"),
        "graphic-owned coords lift verbatim"
    );
    assert_eq!(ir.figures[0].graphic_missingness, None);
    assert_eq!(
        ir.figures[1].graphic_missingness,
        Some(FigureGraphicMissingReason::NoGraphicChild)
    );
    assert_eq!(
        ir.figures[2].graphic_missingness,
        Some(FigureGraphicMissingReason::GraphicWithoutCoordinates)
    );
    assert!(matches!(
        ir.figures[3].graphic_missingness,
        Some(FigureGraphicMissingReason::Malformed(_))
    ));
    assert_eq!(
        ir.figures[4].graphic_missingness,
        Some(FigureGraphicMissingReason::NoGraphicChild)
    );
    // The table wrapper never became a figure and is in the table family.
    assert!(!ids.contains(&"tab_0"), "no double-count of table wrappers");
    assert_eq!(
        ir.tables
            .iter()
            .map(|t| t.table_id.as_str())
            .collect::<Vec<_>>(),
        vec!["tab_0"]
    );

    // Callouts: resolved, no-target and dangling-target in document order.
    assert_eq!(ir.visual_callouts.len(), 3);
    assert_eq!(ir.visual_callouts[0].kind, VisualCalloutKind::Figure);
    assert_eq!(ir.visual_callouts[0].target.as_deref(), Some("fig_x"));
    assert_eq!(ir.visual_callouts[0].unresolved, None);
    assert_eq!(ir.visual_callouts[1].kind, VisualCalloutKind::Table);
    assert_eq!(
        ir.visual_callouts[1].unresolved,
        Some(CalloutUnresolvedReason::NoTarget),
        "target-less callouts stay typed no_target"
    );
    assert_eq!(
        ir.visual_callouts[2].unresolved,
        Some(CalloutUnresolvedReason::TargetNotInInventory),
        "dangling targets stay typed, never guessed away"
    );
    // Bounded offsets inside the section window; unresolved links preserved.
    for callout in &ir.visual_callouts {
        assert_eq!(callout.section_path_id.as_deref(), Some("1"));
        let offset = callout.offset.expect("in-window callout carries offset");
        let sec = ir
            .sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1"))
            .expect("section 1");
        let chars: Vec<char> = sec.text.chars().collect();
        let start = offset as usize;
        let end = (start + callout.quote.chars().count()).min(chars.len());
        assert_eq!(
            chars.get(start..end).map(|s| s.iter().collect::<String>()),
            Some(callout.quote.clone()),
            "offset {offset} points at the quote inside the section text"
        );
    }
    // ptr/notes stay outside the callout family.
    assert!(
        !tei.contains("<ptr"),
        "synthetic shape pins the exclusion surface"
    );

    // Deterministic replay on the synthetic shape.
    let replay = TeiLifter::lift_tei(tei.to_string(), "p", "h");
    assert_eq!(replay.figures, ir.figures);
    assert_eq!(replay.visual_callouts, ir.visual_callouts);

    // A document with no figures/refs at all lifts typed-empty (total).
    let empty = "<TEI><teiHeader></teiHeader><text><body><p>plain</p></body></text></TEI>";
    let ir = TeiLifter::lift_tei(empty.to_string(), "p", "h");
    assert!(ir.figures.is_empty());
    assert!(ir.visual_callouts.is_empty());
}
