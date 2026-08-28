use super::*;
use da_ports::parser::FormulaRepresentation;
use sha2::{Digest, Sha256};

const PRESERVED_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/preserved");
const HUB_SHA: &str = "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4";
const RLM_SHA: &str = "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3";
const RECA_SHA: &str = "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040";

/// Load a tracked preserved TEI fixture as raw bytes. Missing, empty or
/// checksum-mismatched files FAIL VISIBLY — silent skip is the regression
/// class S01/T02 removes.
fn load_preserved_tei_bytes(file_name: &str, expected_sha256: &str) -> Vec<u8> {
    let path = format!("{PRESERVED_DIR}/{file_name}");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("missing preserved TEI fixture `{path}`: {e} (silent skip is forbidden)")
    });
    assert!(!bytes.is_empty(), "preserved TEI fixture `{path}` is empty");
    let actual = hex::encode(Sha256::digest(&bytes));
    assert_eq!(
        actual, expected_sha256,
        "preserved TEI fixture `{path}` sha256 mismatch"
    );
    bytes
}

/// Load a tracked preserved TEI fixture as a UTF-8 string (compatibility
/// surface). Byte-level parity tests use [`load_preserved_tei_bytes`].
fn load_preserved_tei(file_name: &str, expected_sha256: &str) -> String {
    let bytes = load_preserved_tei_bytes(file_name, expected_sha256);
    String::from_utf8(bytes).unwrap_or_else(|e| panic!("fixture `{file_name}` is not UTF-8: {e}"))
}

fn tei_header(tei: &str) -> &str {
    tei.split("<text>").next().unwrap_or(tei)
}

/// Hub paper for large bibliography regression: arXiv **2507.19457** (GEPA).
/// Real GROBID TEI has ~80 `<biblStruct>` in `<back>` (largest in m033 probe).
/// Changelog historically called this out as "80 citations with DOI/arXiv id".
/// Previous pin was only `citations.len() > 5` — too weak to catch bibliography drops.
#[test]
fn test_real_grobid_tei_gepa_hub_bibliography_2507_19457() {
    let tei = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let sections = TeiLifter::extract_sections(&tei);
    let citations = TeiLifter::extract_citations(&tei);
    println!(
        "GEPA hub TEI 2507.19457: {} sections, {} citations",
        sections.len(),
        citations.len()
    );
    assert!(
        sections.len() >= 80,
        "GEPA hub expected ≥80 sections, got {}",
        sections.len()
    );
    // Pin the large bibliography: must stay at full hub scale (not toy >5).
    assert!(
        citations.len() >= 80,
        "GEPA hub expected ≥80 bibliography citations, got {} — bibliography extract regressed",
        citations.len()
    );
    // First section should be INTRODUCTION
    assert_eq!(sections[0].title, "INTRODUCTION");
    // At least some resolvable identifiers in the hub bibliography.
    let with_arxiv = citations.iter().filter(|c| c.arxiv_id.is_some()).count();
    let with_doi = citations.iter().filter(|c| c.doi.is_some()).count();
    let with_title = citations.iter().filter(|c| c.title.is_some()).count();
    println!("  arxiv_id={with_arxiv} doi={with_doi} title={with_title}");
    // Real GEPA TEI: ~23 arXiv + ~6 DOI (many refs are monographs/venues without idno).
    // Pin floor against total extract collapse, not against optimistic id coverage.
    assert!(
        with_arxiv >= 20,
        "expected ≥20 arXiv ids in hub bibliography, got {with_arxiv}"
    );
    assert!(
        with_arxiv + with_doi >= 25,
        "expected ≥25 DOI/arXiv ids combined, arxiv={with_arxiv} doi={with_doi}"
    );
    assert!(
        with_title >= 15,
        "expected ≥15 citation titles (analytic title present), got {with_title}"
    );
    // raw_text must be non-empty for Reference-node path honesty
    assert!(
        citations.iter().all(|c| !c.raw_text.trim().is_empty()),
        "every citation must carry non-empty raw_text for Reference nodes"
    );
    // T03: in-text `<ref type="bibr">` anchors parsed from the real hub
    // TEI. Every targetable ref resolves against back-matter xml:id; a
    // minority sits in top-level gaps (no section path) and must surface
    // as path-unresolved anchors instead of being dropped.
    let total_anchors: usize = citations.iter().map(|c| c.anchors.len()).sum();
    let resolved_paths = citations
        .iter()
        .flat_map(|c| c.anchors.iter())
        .filter(|a| a.section_path_id.is_some())
        .count();
    let unresolved_paths = total_anchors - resolved_paths;
    println!(
        "  anchors total={total_anchors} resolved_paths={resolved_paths} unresolved_paths={unresolved_paths}"
    );
    assert!(
        total_anchors >= 60,
        "expected ≥60 in-text anchors in hub TEI, got {total_anchors}"
    );
    assert!(
        resolved_paths >= 50,
        "expected ≥50 anchors with a resolved section path, got {resolved_paths}"
    );
    // This fixed hub TEI has every targetable ref inside a section text
    // window (footnote refs included) — none may be dropped or left
    // path-unresolved. The unresolved path is exercised by unit tests
    // (test_extract_citations_ref_outside_section_path_unresolved,
    // test_extract_citations_anchor_in_top_level_gap_path_unresolved).
    assert_eq!(
        unresolved_paths, 0,
        "hub TEI must resolve every anchor to a section path, got {unresolved_paths} unresolved"
    );
    // Anchor offsets must be stable within the section's stored text and
    // every resolved path must exist among the section extractor's paths.
    let section_paths: std::collections::HashSet<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    for c in &citations {
        for a in &c.anchors {
            if let Some(p) = a.section_path_id.as_deref() {
                assert!(
                    section_paths.contains(p),
                    "anchor path '{p}' must exist among section paths (anchor {} on '{}')",
                    a.ref_id,
                    c.raw_text.chars().take(40).collect::<String>()
                );
                if let Some(offset) = a.offset {
                    // Offset must point at the quote inside the section text.
                    let sec = sections
                        .iter()
                        .find(|s| s.path_id.as_deref() == Some(p))
                        .expect("path found");
                    let chars: Vec<char> = sec.text.chars().collect();
                    let start = offset as usize;
                    let end = (start + a.quote.chars().count()).min(chars.len());
                    assert_eq!(
                        chars.get(start..end).map(|s| s.iter().collect::<String>()),
                        Some(a.quote.clone()),
                        "offset {offset} must point at quote '{}' in section '{p}'",
                        a.quote
                    );
                }
            }
        }
    }
}

/// Second large-bibliography pin (m033 probe): 2605.26525v1 ~78 back biblStruct.
#[test]
fn test_real_grobid_tei_large_bibliography_2605_26525() {
    let tei = load_preserved_tei("reca_2605.26525v1.tei.xml", RECA_SHA);
    let citations = TeiLifter::extract_citations(&tei);
    println!("2605.26525v1 TEI: {} citations", citations.len());
    assert!(
        citations.len() >= 70,
        "expected large bibliography ≥70 citations, got {}",
        citations.len()
    );
}

/// M400 S02 T01 real-TEI pin (hub 2507.19457): the three `<figure
/// type="table">` tables must extract with their GROBID `xml:id`, cell
/// counts, captions and 64-hex digests, deterministically across runs,
/// and every resolved section identity must exist among the section
/// extractor's paths with the matching ordinal.
#[test]
fn test_real_grobid_tei_tables_2507_19457() {
    let tei = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let (tables, missingness) = TeiLifter::extract_tables(&tei);
    let (tables_again, _) = TeiLifter::extract_tables(&tei);
    assert_eq!(
        tables, tables_again,
        "table extraction must be deterministic across runs"
    );
    assert_eq!(missingness, None, "hub TEI has <table> elements");
    assert_eq!(
        tables.len(),
        3,
        "hub TEI expected 3 tables, got {}",
        tables.len()
    );
    // tab_0 is the benchmark table: caption, 10 rows, 79 cells.
    let tab_0 = tables
        .iter()
        .find(|t| t.table_id == "tab_0")
        .expect("tab_0 present");
    assert_eq!(tab_0.caption, "Table 1 :");
    assert_eq!(tab_0.cells.len(), 79, "tab_0 expected 79 cells");
    let rows: std::collections::HashSet<u32> = tab_0.cells.iter().map(|c| c.row).collect();
    assert_eq!(rows.len(), 10, "tab_0 expected 10 rows");
    // Every table and cell digest is a 64-hex SHA-256.
    for table in &tables {
        assert_eq!(table.source_digest.len(), 64);
        assert!(table.source_digest.bytes().all(|b| b.is_ascii_hexdigit()));
        for cell in &table.cells {
            assert_eq!(cell.digest.len(), 64);
            assert!(cell.digest.bytes().all(|b| b.is_ascii_hexdigit()));
        }
    }
    // Section identity consistency: every resolved path exists among the
    // section extractor's paths and the ordinal points at that section.
    let sections = TeiLifter::extract_sections(&tei);
    let section_paths: std::collections::HashSet<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    for table in &tables {
        if let Some(path) = table.section_path_id.as_deref() {
            assert!(
                section_paths.contains(path),
                "table '{}' path '{path}' must exist among section paths",
                table.table_id
            );
            let ordinal = table.section_ordinal.expect("ordinal with path");
            assert_eq!(
                sections[ordinal].path_id.as_deref(),
                Some(path),
                "table '{}' ordinal must point at its section",
                table.table_id
            );
        }
    }
    println!(
        "hub TEI tables: {} tables, cells={}, ids={:?}",
        tables.len(),
        tables.iter().map(|t| t.cells.len()).sum::<usize>(),
        tables
            .iter()
            .map(|t| t.table_id.clone())
            .collect::<Vec<_>>()
    );
}

/// M400 S03 T01 real-TEI pin (hub 2507.19457): the 24 `<formula>`
/// elements must extract with their GROBID `xml:id` (formula_0..formula_24
/// with formula_2 absent — GROBID numbering), PlainText representations
/// (this corpus has no math extraction), deterministic 64-hex digests, and
/// every resolved section identity must exist among the section
/// extractor's paths with the matching ordinal.
#[test]
fn test_real_grobid_tei_formulas_2507_19457() {
    let tei = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let (formulas, missingness) = TeiLifter::extract_formulas(&tei);
    let (formulas_again, _) = TeiLifter::extract_formulas(&tei);
    assert_eq!(
        formulas, formulas_again,
        "formula extraction must be deterministic across runs"
    );
    assert_eq!(missingness, None, "hub TEI has <formula> elements");
    assert_eq!(
        formulas.len(),
        24,
        "hub TEI expected 24 formulas, got {}",
        formulas.len()
    );
    let expected_ids: std::collections::HashSet<String> = (0..=24u32)
        .filter(|i| *i != 2)
        .map(|i| format!("formula_{i}"))
        .collect();
    let actual_ids: std::collections::HashSet<String> =
        formulas.iter().map(|f| f.formula_id.clone()).collect();
    assert_eq!(
        actual_ids, expected_ids,
        "hub TEI formula ids must be formula_0..formula_24 minus formula_2"
    );
    // Only formula_0 carries an equation number label in this corpus.
    assert_eq!(
        formulas.iter().filter(|f| f.label.is_some()).count(),
        1,
        "hub TEI expected exactly one labeled formula"
    );
    assert_eq!(
        formulas[0].label.as_deref(),
        Some("(1)"),
        "formula_0 label is the equation number (1)"
    );
    // Representations are the GROBID PDF-layout approximation (no
    // <math>/<texMath> extraction in this corpus) and every digest is a
    // 64-hex SHA-256.
    for formula in &formulas {
        assert!(
            matches!(formula.representation, FormulaRepresentation::PlainText(_)),
            "hub TEI formulas are PlainText representations"
        );
        assert_eq!(formula.missingness, None);
        assert_eq!(formula.source_digest.len(), 64);
        assert!(formula.source_digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }
    // Section identity consistency: every resolved path exists among the
    // section extractor's paths and the ordinal points at that section.
    let sections = TeiLifter::extract_sections(&tei);
    let section_paths: std::collections::HashSet<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    for formula in &formulas {
        if let Some(path) = formula.section_path_id.as_deref() {
            assert!(
                section_paths.contains(path),
                "formula '{}' path '{path}' must exist among section paths",
                formula.formula_id
            );
            let ordinal = formula.section_ordinal.expect("ordinal with path");
            assert_eq!(
                sections[ordinal].path_id.as_deref(),
                Some(path),
                "formula '{}' ordinal must point at its section",
                formula.formula_id
            );
        }
    }
    println!(
        "hub TEI formulas: {} formulas, labels={}, resolved_paths={}",
        formulas.len(),
        formulas.iter().filter(|f| f.label.is_some()).count(),
        formulas
            .iter()
            .filter(|f| f.section_path_id.is_some())
            .count()
    );
}

/// Backward-compatible alias of the hub pin (historical test name).
#[test]
fn test_real_grobid_tei_sections_and_citations() {
    test_real_grobid_tei_gepa_hub_bibliography_2507_19457();
}

/// T02: preserved TEI bytes still carry authors, article-level idno and
/// non-table figures. This cut does not lift those families (see
/// `this_cut_ir_omits_authors_idno_and_non_table_figures`); the pin is on
/// the fixture, not on a new IR field.
#[test]
fn test_preserved_tei_bytes_cover_authors_idno_and_figures() {
    let hub = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let rlm = load_preserved_tei("rlm_2512.24601.tei.xml", RLM_SHA);
    let reca = load_preserved_tei("reca_2605.26525v1.tei.xml", RECA_SHA);
    for (name, tei, min_authors, min_nontable_figures) in [
        ("hub", hub.as_str(), 21usize, 20usize),
        ("rlm", rlm.as_str(), 4, 20),
        ("reca", reca.as_str(), 11, 40),
    ] {
        let header = tei_header(tei);
        let authors = header.matches("<author").count();
        assert!(
            authors >= min_authors,
            "{name} header expected ≥{min_authors} <author, got {authors}"
        );
        assert!(
            header.contains("<idno"),
            "{name} header must carry at least one <idno>"
        );
        let figures = tei.matches("<figure").count();
        let fig_ids = tei.matches("xml:id=\"fig_").count();
        assert!(
            fig_ids >= min_nontable_figures,
            "{name} expected ≥{min_nontable_figures} non-table figure xml:id, got figures={figures} fig_ids={fig_ids}"
        );
    }
}

/// T02: GROBID restarts `n` among sibling divs on the hub. The walker must
/// uniquify `path_id` (suffix `~2`, `~3`, …) instead of colliding.
#[test]
fn test_hub_restarted_numbering_yields_unique_path_ids() {
    let tei = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let sections = TeiLifter::extract_sections(&tei);
    let mut paths = std::collections::HashSet::new();
    let mut suffixed = 0usize;
    for s in &sections {
        let p = s
            .path_id
            .as_deref()
            .expect("nested GROBID TEI must assign path_id");
        assert!(
            paths.insert(p.to_string()),
            "duplicate section path_id `{p}` — restarted numbering was not uniquified"
        );
        if p.contains('~') {
            suffixed += 1;
        }
    }
    assert!(
        suffixed >= 1,
        "hub TEI restarts n among siblings; expected at least one `~` suffix, got 0 (sections={})",
        sections.len()
    );
    println!(
        "hub restarted numbering: {} sections, {} collision suffixes",
        sections.len(),
        suffixed
    );
}

/// T02: this-cut IR still omits authors / article idno / non-table figures
/// even when the real hub TEI contains them. Lift expansion is later slices.
#[test]
fn test_this_cut_ir_still_omits_authors_figures_idno_on_real_hub() {
    let tei = load_preserved_tei("hub_2507.19457.tei.xml", HUB_SHA);
    let ir = GrobidParser::lift_tei(tei, "2507.19457", "fixture");
    let article = GrobidParser::project_article(ir);
    let v = serde_json::to_value(&article).unwrap();
    assert!(v.get("authors").is_none());
    assert!(v.get("figures").is_none());
    assert!(v.get("idno").is_none());
    assert!(!article.tables.is_empty(), "tables ARE lifted this cut");
    assert!(!article.formulas.is_empty(), "formulas ARE lifted this cut");
}

/// T02: reca/rlm TEI carry GROBID `coords` on `<graphic>` (not on formulas
/// or tables in this probe). This cut does not lift graphics; the pin is on
/// the fixture bytes so later visual-head slices have source-bound coords.
#[test]
fn test_preserved_tei_bytes_cover_graphic_coordinates() {
    let reca = load_preserved_tei("reca_2605.26525v1.tei.xml", RECA_SHA);
    let rlm = load_preserved_tei("rlm_2512.24601.tei.xml", RLM_SHA);
    for (name, tei, min_coords) in [("reca", reca.as_str(), 30usize), ("rlm", rlm.as_str(), 20)] {
        let coords = tei.matches("coords=").count();
        let graphic_coords = tei.matches("<graphic").count();
        assert!(
            coords >= min_coords,
            "{name} expected ≥{min_coords} coords= attributes, got {coords}"
        );
        assert!(
            graphic_coords >= min_coords,
            "{name} coords live on <graphic>; expected ≥{min_coords} graphics, got {graphic_coords}"
        );
        println!("{name} coords={coords} graphics={graphic_coords}");
    }
    let (formulas, _) = TeiLifter::extract_formulas(&reca);
    let (tables, _) = TeiLifter::extract_tables(&reca);
    assert!(
        formulas.iter().all(|f| f.coordinates.is_none())
            && tables.iter().all(|t| t.coordinates.is_none()),
        "this probe stamps coords on <graphic>, not on lifted formula/table elements"
    );
}

/// S02/T02: the provider-neutral [`TeiLifter`] envelope path must reproduce
/// the compatibility path exactly on all three checksum-bound S01 fixtures:
/// raw TEI SHA/bytes preserved, serialized `TeiParseIr` byte-equal,
/// projected `ParsedArticle` byte-equal, and two offline replays byte-equal.
/// Envelope identity (backend id / profile fingerprint) is provenance only
/// and never changes lift output.
#[test]
fn tei_lifter_envelope_parity_with_compatibility_path_on_all_preserved_fixtures() {
    for (name, file, sha, paper_id) in [
        ("hub", "hub_2507.19457.tei.xml", HUB_SHA, "2507.19457"),
        ("rlm", "rlm_2512.24601.tei.xml", RLM_SHA, "2512.24601"),
        (
            "reca",
            "reca_2605.26525v1.tei.xml",
            RECA_SHA,
            "2605.26525v1",
        ),
    ] {
        let bytes = load_preserved_tei_bytes(file, sha);

        // Compatibility path (recorded `parse.stage.lift_tei` surface).
        let compat = GrobidParser::lift_tei(
            std::str::from_utf8(&bytes)
                .expect("fixture utf-8")
                .to_string(),
            paper_id,
            "fixture-pdf-hash",
        );

        // Provider-neutral envelope path (pure: no HTTP, no filesystem,
        // no runtime infrastructure).
        let envelope = TeiEnvelope::new(
            bytes.clone(),
            TeiIdentity::cached_tei(GrobidProfile::rich()),
        );
        // Raw TEI bytes preserved: the envelope carries exactly the fixture
        // bytes (checksum-bound), unmodified by the lift path.
        assert_eq!(
            hex::encode(Sha256::digest(envelope.raw_tei())),
            sha,
            "{name}: envelope must carry the fixture bytes verbatim"
        );

        let ir_new = TeiLifter::new().lift(&envelope, paper_id, "fixture-pdf-hash");
        assert_eq!(
            ir_new.tei_xml.as_bytes(),
            bytes.as_slice(),
            "{name}: lifted IR must embed the raw TEI bytes verbatim"
        );

        // Serialized IR parity, byte for byte.
        let compat_json = serde_json::to_vec(&compat).expect("serialize compat IR");
        let new_json = serde_json::to_vec(&ir_new).expect("serialize envelope IR");
        assert_eq!(
            compat_json, new_json,
            "{name}: serialized TeiParseIr diverges between envelope and compatibility paths"
        );

        // Projected ParsedArticle parity, byte for byte.
        let compat_article = GrobidParser::project_article(compat);
        let new_article = GrobidParser::project_article(ir_new);
        assert_eq!(
            serde_json::to_vec(&compat_article).expect("serialize compat article"),
            serde_json::to_vec(&new_article).expect("serialize envelope article"),
            "{name}: projected ParsedArticle diverges between paths"
        );

        // Two offline replays are byte-equal (determinism of the pure lift).
        let replay_json = serde_json::to_vec(&TeiLifter::new().lift(
            &TeiEnvelope::new(
                bytes.clone(),
                TeiIdentity::cached_tei(GrobidProfile::rich()),
            ),
            paper_id,
            "fixture-pdf-hash",
        ))
        .expect("serialize replay IR");
        assert_eq!(
            replay_json, new_json,
            "{name}: two envelope-path replays must be byte-equal"
        );

        // Identity independence: a different backend id and profile
        // fingerprint (provenance) must not change the lift output.
        let http_identity_ir = TeiLifter::new().lift(
            &TeiEnvelope::new(
                bytes.clone(),
                TeiIdentity::grobid_http(GrobidProfile::structural()),
            ),
            paper_id,
            "fixture-pdf-hash",
        );
        assert_eq!(
            serde_json::to_vec(&http_identity_ir).expect("serialize http-identity IR"),
            new_json,
            "{name}: lift output must not depend on envelope identity"
        );

        println!(
            "{name}: parity OK — ir={} bytes, article={} bytes, sections={}, citations={}",
            new_json.len(),
            serde_json::to_vec(&new_article).unwrap().len(),
            new_article.sections.len(),
            new_article.citations.len()
        );
    }
}

/// S02/T02: cached-TEI backend → envelope → pure lift is byte-equal to the
/// compatibility lift without any HTTP or runtime infrastructure (offline
/// replay of preserved S01 bytes through the R105 seam).
#[tokio::test]
async fn tei_lifter_via_cached_backend_replay_is_byte_equal_to_compatibility_path() {
    let bytes = load_preserved_tei_bytes("hub_2507.19457.tei.xml", HUB_SHA);
    let compat = GrobidParser::lift_tei(
        std::str::from_utf8(&bytes)
            .expect("fixture utf-8")
            .to_string(),
        "2507.19457",
        "fixture-pdf-hash",
    );

    let backend = CachedTeiBackend::new(bytes, GrobidProfile::rich());
    // The request carries unrelated source bytes: a cached replay serves
    // already-available TEI and never transports the PDF.
    let envelope = backend
        .fetch_tei(ParseBackendRequest::new(
            "2507.19457",
            b"unused-pdf-bytes".to_vec(),
        ))
        .await
        .expect("cached backend is infallible");
    let ir = TeiLifter::new().lift(&envelope, "2507.19457", "fixture-pdf-hash");

    assert_eq!(
        serde_json::to_vec(&ir).expect("serialize replay IR"),
        serde_json::to_vec(&compat).expect("serialize compat IR"),
        "cached-backend replay must be byte-equal to the compatibility lift"
    );
}

/// S02/T02: each fixture's genuine structural bytes drive its lift. The
/// three checksum-bound fixtures produce pairwise-distinct serialized IRs,
/// so the parity suite cannot pass by substituting the rich hub fixture
/// for the other two.
#[test]
fn tei_lifter_lifts_each_fixtures_genuine_bytes_not_a_substituted_rich_fixture() {
    let mut jsons = Vec::new();
    for (file, sha, paper_id) in [
        ("hub_2507.19457.tei.xml", HUB_SHA, "2507.19457"),
        ("rlm_2512.24601.tei.xml", RLM_SHA, "2512.24601"),
        ("reca_2605.26525v1.tei.xml", RECA_SHA, "2605.26525v1"),
    ] {
        let bytes = load_preserved_tei_bytes(file, sha);
        let envelope = TeiEnvelope::new(bytes, TeiIdentity::cached_tei(GrobidProfile::rich()));
        let ir = TeiLifter::new().lift(&envelope, paper_id, "fixture-pdf-hash");
        jsons.push(serde_json::to_vec(&ir).expect("serialize IR"));
    }
    assert_ne!(jsons[0], jsons[1], "hub and rlm lifts must differ");
    assert_ne!(jsons[0], jsons[2], "hub and reca lifts must differ");
    assert_ne!(jsons[1], jsons[2], "rlm and reca lifts must differ");
}

/// S02/T02: empty, malformed and non-UTF-8 TEI bytes through the envelope
/// path stay total and non-panicking and observe exactly the compatibility
/// behavior — there is one code path behind both entries, and nothing is
/// silently skipped (typed missingness keeps the absence honest).
#[test]
fn tei_lifter_empty_and_malformed_envelopes_match_compatibility_behavior() {
    for (name, bytes) in [
        ("empty", Vec::new()),
        ("malformed", b"<TEI><text><body><div broken".to_vec()),
        ("non-utf8", vec![0xFF, 0xFE, 0x00, b'<']),
    ] {
        let envelope = TeiEnvelope::new(
            bytes.clone(),
            TeiIdentity::cached_tei(GrobidProfile::rich()),
        );
        // Must not panic.
        let ir = TeiLifter::new().lift(&envelope, "p", "h");
        let compat = GrobidParser::lift_tei(String::from_utf8_lossy(&bytes).into_owned(), "p", "h");
        assert_eq!(
            serde_json::to_vec(&ir).expect("serialize envelope IR"),
            serde_json::to_vec(&compat).expect("serialize compat IR"),
            "{name}: envelope path must match compatibility path on degraded input"
        );
        let article = GrobidParser::project_article(ir);
        assert_eq!(article.paper_id, "p", "{name}");
        assert_eq!(article.pdf_hash, "h", "{name}");
    }

    // Empty TEI pins (mirrors tests.rs empty_tei_projects_success_with_tei_xml_present,
    // now on the envelope path).
    let empty = TeiEnvelope::new(Vec::new(), TeiIdentity::cached_tei(GrobidProfile::rich()));
    let article = GrobidParser::project_article(TeiLifter::new().lift(&empty, "p", "h"));
    assert_eq!(article.tei_xml.as_deref(), Some(""));
    assert!(article.title.is_empty());
    assert!(article.body_text.is_empty());
    assert!(article.sections.is_empty());
    assert!(article.citations.is_empty());
}
