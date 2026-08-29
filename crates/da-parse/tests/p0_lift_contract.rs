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
//! T02/T03 extend this contract with lifted-output assertions (exact counts,
//! order, byte-equality of the two public lift paths); T04 converts these
//! families from `unsupported_this_cut` to conservation evidence.
//!
//! Reports are redacting by construction: nothing here prints raw TEI,
//! author names, captions, quotes, URLs or physical paths.

use std::path::{Path, PathBuf};

use da_parse::{TeiLifter, TeiParseIr};
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
    non_table_figures: usize,
    table_wrappers: usize,
    total_figure_tags: usize,
    coords: usize,
    declared_idno_raw: &'static str,
    figure_refs: usize,
    figure_refs_without_target: usize,
    table_refs: usize,
    table_refs_without_target: usize,
    body_persname_not_authors: usize,
}

fn pins_for(case_id: &str) -> S07BytePins {
    match case_id {
        "reg.tei.hub-bibliography-2507.19457" => S07BytePins {
            header_authors: 21,
            non_table_figures: 22,
            table_wrappers: 3,
            total_figure_tags: 25,
            coords: 1,
            declared_idno_raw: "arXiv:2507.19457v2[cs.CL]",
            figure_refs: 24,
            figure_refs_without_target: 10,
            table_refs: 5,
            table_refs_without_target: 2,
            body_persname_not_authors: 686,
        },
        "reg.tei.rlm-2512.24601" => S07BytePins {
            header_authors: 4,
            non_table_figures: 29,
            table_wrappers: 2,
            total_figure_tags: 31,
            coords: 25,
            declared_idno_raw: "arXiv:2512.24601v3[cs.AI]",
            figure_refs: 25,
            figure_refs_without_target: 10,
            table_refs: 11,
            table_refs_without_target: 4,
            body_persname_not_authors: 998,
        },
        "reg.tei.reca-bibliography-2605.26525" => S07BytePins {
            header_authors: 11,
            non_table_figures: 55,
            table_wrappers: 3,
            total_figure_tags: 58,
            coords: 34,
            declared_idno_raw: "arXiv:2605.26525v1[cs.CV]",
            figure_refs: 30,
            figure_refs_without_target: 5,
            table_refs: 11,
            table_refs_without_target: 1,
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
/// counted separately from non-table figures, and the two never overlap.
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

/// T01 boundary state: the additive wire fields exist and decode, but the
/// single lifter does not populate them yet (S07 T02/T03 implement the
/// lifters). Until then the families stay at typed emptiness — nothing is
/// invented. T02/T03 replace the emptiness assertions here with exact
/// lifted-output assertions (21/4/11 authors, 22/29/55 figures, 1/25/34
/// coordinates, 29/36/41 callouts and the pinned declared idno values).
#[test]
fn s07_families_stay_uninvented_until_the_lifter_implements() {
    for fixture in preserved_bound_fixtures() {
        let case = &fixture.case_id;
        let tei = as_tei(&fixture.bytes, case);
        let ir = TeiLifter::lift_tei(tei, fixture.paper_id.clone(), "s07-t01-contract-hash");
        assert!(
            ir.declared_arxiv_id.is_none() && ir.declared_id_missingness.is_none(),
            "{case}: declared identity must not be invented before T02"
        );
        assert!(
            ir.header_authors.is_empty(),
            "{case}: header authors must not be invented before T02"
        );
        assert!(
            ir.figures.is_empty() && ir.visual_callouts.is_empty(),
            "{case}: figures/callouts must not be invented before T03"
        );
        // The additive fields did not disturb the existing lift algorithm.
        assert!(!ir.title.is_empty(), "{case}: title still lifts");
        assert!(!ir.sections.is_empty(), "{case}: sections still lift");
        assert!(!ir.citations.is_empty(), "{case}: citations still lift");
    }
}
