use super::*;
use da_ports::parser::{FormulaMissingReason, FormulaRepresentation, TableMissingReason};
use std::collections::HashSet;

#[test]
fn structural_profile_disables_remote_consolidation() {
    let parser = GrobidParser::new(None).with_profile(GrobidProfile::structural());
    assert_eq!(parser.profile(), GrobidProfile::structural());
    assert!(!parser.profile().consolidate_header());
    assert!(!parser.profile().consolidate_citations());
}

#[test]
fn profile_fingerprints_bind_semantics_and_exclude_timeout() {
    assert_eq!(GrobidProfile::rich().name(), "rich");
    assert_eq!(
        GrobidProfile::rich().configuration_fingerprint(),
        "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula"
    );
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula"
    );
    assert_ne!(
        GrobidProfile::rich().configuration_fingerprint(),
        GrobidProfile::structural().configuration_fingerprint()
    );
    let custom = GrobidProfile::structural()
        .with_consolidation(Consolidation::try_new(1, 0, 0).expect("sanctioned levels"));
    assert_eq!(custom.name(), "custom");
    assert!(
        custom
            .configuration_fingerprint()
            .contains("profile=custom")
    );
    let with_timeout = GrobidParser::new(None)
        .with_timeout(std::time::Duration::from_secs(1))
        .profile();
    assert_eq!(
        with_timeout.configuration_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint(),
        "timeout is operational policy, not a content-key input"
    );
}

#[test]
fn identity_and_fingerprint_ignore_worker_count() {
    let fingerprint = GrobidProfile::structural().configuration_fingerprint();
    for leak in ["worker", "smp", "concurrency", "thread"] {
        assert!(
            !fingerprint.to_ascii_lowercase().contains(leak),
            "D204 fingerprint leaked operational {leak}: {fingerprint}"
        );
    }
    let a = TeiIdentity::grobid_http(GrobidProfile::structural());
    let b = TeiIdentity::grobid_http(GrobidProfile::structural());
    assert_eq!(a, b);
    assert_eq!(a.backend_id(), b.backend_id());
    assert_eq!(a.profile_fingerprint(), fingerprint);
    assert_eq!(a.profile_fingerprint(), b.profile_fingerprint());
    assert!(a.backend_version().is_none());
}

#[test]
fn tei_identity_backend_version_is_additive() {
    let base = TeiIdentity::grobid_http(GrobidProfile::structural());
    assert!(base.backend_version().is_none());
    assert_eq!(base, TeiIdentity::grobid_http(GrobidProfile::structural()));
    let stamped = base.clone().with_backend_version("0.9.0-crf");
    assert_eq!(stamped.backend_version(), Some("0.9.0-crf"));
    assert_ne!(stamped, base);
    assert_eq!(
        stamped.profile_fingerprint(),
        GrobidProfile::structural().configuration_fingerprint(),
        "version must not rewrite the D204 profile fingerprint"
    );
    assert_eq!(
        base.clone().with_backend_version("   "),
        base,
        "whitespace-only version must not invent an identity component"
    );
    let http = GrobidHttpBackend::new(Some("http://127.0.0.1:1"));
    assert!(http.identity().backend_version().is_none());
    let with_ver = http.clone().with_backend_version("0.9.0-crf");
    assert_eq!(with_ver.identity().backend_version(), Some("0.9.0-crf"));
    assert_ne!(with_ver.identity(), http.identity());
    assert_eq!(
        http.clone()
            .with_timeout(std::time::Duration::from_secs(9))
            .identity(),
        http.identity()
    );
}

#[test]
fn tei_coordinates_json_matches_fingerprint_and_includes_formula() {
    // M400 S03 T01: the REST body must request formula coordinates so
    // GROBID emits `coords` on `<formula>` elements — the request can
    // never silently drop a coordinate group the fingerprint claims.
    assert_eq!(
        GrobidProfile::tei_coordinates_json(),
        "[\"figure\",\"table\",\"note\",\"formula\"]"
    );
    // Rendered from the same constant the fingerprint uses: splitting the
    // JSON back out must reproduce the constant exactly.
    let json = GrobidProfile::tei_coordinates_json();
    let inner = json.trim_start_matches('[').trim_end_matches(']');
    let items: Vec<&str> = inner.split(',').map(|s| s.trim_matches('"')).collect();
    assert_eq!(items.join(","), GrobidProfile::TEI_COORDINATES);
    assert!(
        GrobidProfile::rich()
            .configuration_fingerprint()
            .contains("teiCoordinates=figure,table,note,formula"),
        "fingerprint keeps the formula coordinate group"
    );
}

#[test]
fn test_extract_title() {
    let tei = r#"<TEI><teiHeader><fileDesc><titleStmt><title>Seq2Seq Models</title></titleStmt></fileDesc></teiHeader></TEI>"#;
    assert_eq!(TeiLifter::extract_title(tei), "Seq2Seq Models");
}

#[test]
fn test_extract_title_with_attributes() {
    // GROBID TEI uses attributes: <title level="a" type="main">
    let tei = r#"<title level="a" type="main">A Joint Model of Language and Perception</title>"#;
    assert_eq!(
        TeiLifter::extract_title(tei),
        "A Joint Model of Language and Perception"
    );
}

#[test]
fn test_strip_xml_tags() {
    let s = "<p>Hello <ref>world</ref></p>";
    assert_eq!(TeiLifter::strip_xml_tags(s), "Hello world");
}

#[test]
fn test_pdf_hash() {
    let bytes = b"test pdf content";
    let hash = TeiLifter::pdf_hash(bytes);
    assert_eq!(hash.len(), 64);
}

#[test]
fn test_extract_sections() {
    let tei = r#"<TEI><text><body>
            <div><head n="1">INTRODUCTION</head><p>Intro text here.</p></div>
            <div><head n="2">Methods</head><p>Method details.</p></div>
        </body></text></TEI>"#;
    let sections = TeiLifter::extract_sections(tei);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].title, "INTRODUCTION");
    assert_eq!(sections[0].level, 1);
    assert!(sections[0].text.contains("Intro text"));
    assert_eq!(sections[1].title, "Methods");
    assert_eq!(sections[1].level, 2);
}

#[test]
fn test_extract_sections_no_body() {
    let tei = r#"<TEI><teiHeader><title>No body</title></teiHeader></TEI>"#;
    let sections = TeiLifter::extract_sections(tei);
    assert!(sections.is_empty());
}

#[test]
fn test_extract_citations() {
    let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct><analytic><title level="a" type="main">First Citation</title></analytic>
            <idno type="DOI">10.1000/cite.1</idno></biblStruct>
            <biblStruct><analytic><title level="a" type="main">Second Citation</title></analytic>
            <idno type="arXiv">arXiv:2001.00002v1</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].title.as_deref(), Some("First Citation"));
    assert_eq!(citations[0].doi.as_deref(), Some("10.1000/cite.1"));
    assert_eq!(citations[1].title.as_deref(), Some("Second Citation"));
    assert_eq!(citations[1].arxiv_id.as_deref(), Some("2001.00002"));
}

#[test]
fn test_extract_citations_no_back() {
    let tei = r#"<TEI><text><body><p>no refs</p></body></text></TEI>"#;
    let citations = TeiLifter::extract_citations(tei);
    assert!(citations.is_empty());
}

#[test]
fn test_extract_citations_with_attributes() {
    // Real GROBID output: <biblStruct status="extracted" xml:id="b0">
    let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct status="extracted" xml:id="b0"><analytic>
            <title level="a" type="main">Cited With Attrs</title></analytic>
            <idno type="DOI">10.2000/x</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].title.as_deref(), Some("Cited With Attrs"));
    assert_eq!(citations[0].doi.as_deref(), Some("10.2000/x"));
    // No in-text refs in this TEI → anchors stay empty.
    assert!(citations[0].anchors.is_empty());
}

// ─── T03 (M398-cz4pzx/S03): in-text citation anchors ───

/// Flat TEI: `<head n="1">` window contains a `<ref type="bibr">`
/// targeting a back-matter biblStruct xml:id. The anchor must carry the
/// resolved section path "1" and the char offset of the quote in the
/// section's stripped text ("Alpha [1] Beta" → offset 6).
#[test]
fn test_extract_citations_in_text_anchors_flat() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>Alpha <ref type="bibr" target="#b0">[1]</ref> Beta</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic><idno type="DOI">10.1000/cite.1</idno></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 1, "one in-text anchor attached");
    let anchor = &citations[0].anchors[0];
    assert_eq!(anchor.ref_id, "b0");
    assert_eq!(anchor.section_path_id.as_deref(), Some("1"));
    assert_eq!(anchor.quote, "[1]");
    assert_eq!(
        anchor.offset,
        Some(6),
        "offset of '[1]' in 'Alpha [1] Beta'"
    );
}

/// Nested TEI: the same citation referenced in two sections keeps two
/// anchors with distinct section paths and offsets (multi-span, D249).
#[test]
fn test_extract_citations_anchors_nested_div_multi_span() {
    let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha <ref type="bibr" target="#b1">(Jones, 2020)</ref></p><div n="1.1"><head>Setup</head><p>See <ref type="bibr" target="#b1">[1]</ref></p></div></div></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
<biblStruct xml:id="b1"><analytic><title level="a" type="main">Jones 2020</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 2);
    assert!(citations[0].anchors.is_empty(), "b0 has no in-text refs");
    let anchors = &citations[1].anchors;
    assert_eq!(anchors.len(), 2, "Jones cited in sections 1 and 1.1");
    assert_eq!(anchors[0].ref_id, "b1");
    assert_eq!(anchors[0].section_path_id.as_deref(), Some("1"));
    assert_eq!(anchors[0].quote, "(Jones, 2020)");
    assert_eq!(anchors[0].offset, Some(6));
    assert_eq!(anchors[1].section_path_id.as_deref(), Some("1.1"));
    assert_eq!(anchors[1].quote, "[1]");
    assert_eq!(anchors[1].offset, Some(4), "offset of '[1]' in 'See [1]'");
}

/// No in-text refs in the body → every citation stays anchor-less
/// (ingest then records typed missingness no_in_text_anchor).
#[test]
fn test_extract_citations_no_in_text_refs_anchors_empty() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>plain text</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert!(citations[0].anchors.is_empty());
}

/// A `<ref type="bibr">` in the body before the first `<head>` is not
/// inside any section text window → anchor attaches with section_path_id
/// None (typed missingness anchor_section_path_unresolved at ingest).
#[test]
fn test_extract_citations_ref_outside_section_path_unresolved() {
    let tei = r##"<TEI><text><body><p>Preamble <ref type="bibr" target="#b0">[1]</ref></p><head n="1">Intro</head><p>Body</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 1);
    let anchor = &citations[0].anchors[0];
    assert_eq!(anchor.ref_id, "b0");
    assert!(
        anchor.section_path_id.is_none(),
        "no section window → None path"
    );
    assert!(anchor.offset.is_none(), "no section text → no offset");
}

/// Refs without a `target` attribute (GROBID emits them for merged ref
/// groups) cannot be mapped to a bibliography entry and are dropped;
/// targeted refs in the same window still attach.
#[test]
fn test_extract_citations_ref_without_target_skipped() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>A <ref type="bibr">(no target)</ref> B <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(
        citations[0].anchors.len(),
        1,
        "only the targeted ref attaches"
    );
    assert_eq!(citations[0].anchors[0].quote, "[1]");
}

/// Non-bibliographic refs (`type="note"`) are not citation anchors.
#[test]
fn test_extract_citations_ignores_non_bibr_refs() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="note" target="#n1">footnote</ref> and <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 1);
    assert_eq!(citations[0].anchors[0].quote, "[1]");
}

/// biblStruct without xml:id falls back to its ordinal key ("b{index}")
/// so synthetic/hand-written TEI with ordinal targets still resolves.
#[test]
fn test_extract_citations_ordinal_fallback_without_xml_id() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct><analytic><title level="a" type="main">NoId</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 1);
    assert_eq!(citations[0].anchors[0].ref_id, "b0");
}

/// Flat TEI with a numbering restart (two `<head n="1">`, e.g. an
/// appendix restarting section numbers): anchors from the second window
/// must resolve to the same uniquified path the section pass produces
/// ("1~2"), never the first window's "1" (regression: a global per-path
/// cache collapsed both windows onto the first window's path, so the
/// appendix anchors pointed into the INTRODUCTION section text).
#[test]
fn test_extract_citations_flat_numbering_restart_paths() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="bibr" target="#b0">[1]</ref></p><head n="2">Methods</head><p>X</p><head n="1">Appendix</head><p>See <ref type="bibr" target="#b0">[1]</ref> again</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let sections = TeiLifter::extract_sections(tei);
    let citations = TeiLifter::extract_citations(tei);
    // Section paths: "1", "2", then the appendix restart becomes "1~2".
    let paths: Vec<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    assert_eq!(paths, vec!["1", "2", "1~2"]);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 2);
    let p0 = citations[0].anchors[0].section_path_id.clone();
    let p1 = citations[0].anchors[1].section_path_id.clone();
    assert_eq!(p0.as_deref(), Some("1"), "first window keeps path 1");
    assert_eq!(
        p1.as_deref(),
        Some("1~2"),
        "restarted window resolves to the section's uniquified path"
    );
    // Offsets must point at the quote inside each section's own text.
    let s1 = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1"))
        .expect("section 1");
    let sa = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1~2"))
        .expect("section 1~2");
    let a0 = &citations[0].anchors[0];
    let a1 = &citations[0].anchors[1];
    let off0 = a0.offset.expect("anchor 0 offset");
    let off1 = a1.offset.expect("anchor 1 offset");
    assert_eq!(
        &s1.text[off0 as usize..off0 as usize + a0.quote.len()],
        a0.quote,
        "anchor 0 offset must land on its quote in section 1"
    );
    assert_eq!(
        &sa.text[off1 as usize..off1 as usize + a1.quote.len()],
        a1.quote,
        "anchor 1 offset must land on its quote in section 1~2"
    );
}

/// Anchors in a top-level gap (between sibling divs) of a NESTED body are
/// attached with section_path_id None rather than being dropped: the gap
/// belongs to no section text window.
#[test]
fn test_extract_citations_anchor_in_top_level_gap_path_unresolved() {
    // div n="1" contains a nested child so the body counts as nested; the
    // ref sits between div 1's close and div 2's open — outside any window.
    let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha</p><div n="1.1"><head>Sub</head><p>x</p></div></div><p>Between <ref type="bibr" target="#b0">[1]</ref></p><div n="2"><head>Methods</head><p>Beta</p></div></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
    let citations = TeiLifter::extract_citations(tei);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].anchors.len(), 1);
    assert!(
        citations[0].anchors[0].section_path_id.is_none(),
        "inter-div gap belongs to no section → None path"
    );
    assert!(citations[0].anchors[0].offset.is_none());
}

#[test]
fn test_extract_head_level() {
    assert_eq!(TeiLifter::extract_head_level(r#"<head n="3">"#), 3);
    assert_eq!(TeiLifter::extract_head_level("<head>"), 1);
    assert_eq!(TeiLifter::extract_head_level(r#"<head n="abc">"#), 1);
}

// ─── ADR-053 Layer A T02: recursive TEI div walk for path_id ───

/// Fixture: minimal TEI body with nested `<div>` elements matching GROBID
/// output structure. `<div n="N">` gives the section number; nesting gives
/// the tree. Text content is in `<p>` children of each div.
const NESTED_DIV_TEI: &str = r#"<body>
      <div n="1"><head>Introduction</head><p>Intro text.</p>
        <div n="1.1"><head>Background</head><p>Background text.</p></div>
        <div n="1.2"><head>Motivation</head><p>Motivation text.</p></div>
      </div>
      <div n="2"><head>Methods</head><p>Methods overview.</p>
        <div n="2.1"><head>Model</head><p>Model text.</p>
          <div n="2.1.1"><head>Encoder</head><p>Encoder text.</p></div>
        </div>
        <div n="2.2"><head>Training</head><p>Training text.</p></div>
      </div>
      <div n="3"><head>Results</head><p>Results text.</p></div>
    </body>"#;

#[test]
fn test_extract_sections_nested_div_path_id_populated() {
    let sections = TeiLifter::extract_sections(NESTED_DIV_TEI);
    let path_ids: Vec<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    assert!(
        path_ids.contains(&"1"),
        "missing root 1, got: {:?}",
        path_ids
    );
    assert!(path_ids.contains(&"1.1"));
    assert!(path_ids.contains(&"1.2"));
    assert!(path_ids.contains(&"2"));
    assert!(path_ids.contains(&"2.1"));
    assert!(path_ids.contains(&"2.1.1"));
    assert!(path_ids.contains(&"2.2"));
    assert!(path_ids.contains(&"3"));
}

#[test]
fn test_duplicate_div_numbers_receive_deterministic_unique_paths() {
    let tei = r#"<body>
          <div n="1"><head>Introduction</head><p>Intro.</p></div>
          <div n="1"><head>Appendix A</head><p>First appendix.</p>
            <div n="1.1"><head>Details</head><p>Details.</p></div>
          </div>
          <div n="1"><head>Appendix B</head><p>Second appendix.</p></div>
        </body>"#;
    let sections = TeiLifter::extract_sections(tei);
    let paths: Vec<_> = sections
        .iter()
        .map(|section| section.path_id.as_deref().unwrap())
        .collect();
    assert_eq!(paths, vec!["1", "1~2", "1~2.1", "1~3"]);
    let unique: HashSet<_> = paths.iter().copied().collect();
    assert_eq!(unique.len(), paths.len());
    assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-1~2"));
}

#[test]
fn test_extract_sections_nested_div_parent_section_id() {
    let sections = TeiLifter::extract_sections(NESTED_DIV_TEI);
    // Section 2.1.1 parent should be section 2.1
    let encoder = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("2.1.1"))
        .expect("section 2.1.1 present");
    assert_eq!(
        encoder.parent_section_id.as_deref(),
        Some("sec-2.1"),
        "parent_section_id must point at the parent div's section_id"
    );
    // Root sections have no parent.
    let intro = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1"))
        .expect("section 1 present");
    assert!(
        intro.parent_section_id.is_none(),
        "root section has no parent"
    );
}

#[test]
fn test_extract_sections_nested_div_order_in_parent() {
    let sections = TeiLifter::extract_sections(NESTED_DIV_TEI);
    // Section 1.1 has order 0, section 1.2 has order 1 in parent 1.
    let bg = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1.1"))
        .expect("section 1.1 present");
    assert_eq!(bg.order_in_parent, Some(0));
    let motiv = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1.2"))
        .expect("section 1.2 present");
    assert_eq!(motiv.order_in_parent, Some(1));
    // Training is the second child of Methods (after Model).
    let training = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("2.2"))
        .expect("section 2.2 present");
    assert_eq!(training.order_in_parent, Some(1));
}

#[test]
fn test_extract_sections_nested_div_section_id_stable() {
    let sections = TeiLifter::extract_sections(NESTED_DIV_TEI);
    // Every section must have a section_id of form "sec-<path_id>".
    for s in &sections {
        let sid = s
            .section_id
            .as_ref()
            .expect("section_id populated for nested TEI");
        let pid = s
            .path_id
            .as_ref()
            .expect("path_id populated for nested TEI");
        assert!(sid.starts_with("sec-"), "section_id should start with sec-");
        assert!(sid.ends_with(pid), "section_id should end with path_id");
    }
}

#[test]
fn test_extract_sections_nested_div_depth_matches_level() {
    let sections = TeiLifter::extract_sections(NESTED_DIV_TEI);
    // depth (0-based) for section 2.1.1 is 2; level (1-based) is 3.
    let encoder = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("2.1.1"))
        .expect("section 2.1.1 present");
    // level field is populated from existing logic — recursive walk must set it.
    assert_eq!(
        encoder.level, 3,
        "level = path_id component count (1-based)"
    );
    // Root section has level 1.
    let root = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1"))
        .expect("section 1 present");
    assert_eq!(root.level, 1);
}

#[test]
fn test_flat_numbering_restart_preserves_unique_tree_identity() {
    let tei = r#"<body>
          <head n="1">Introduction</head><p>Intro.</p>
          <head n="2">Methods</head><p>Methods.</p>
          <head n="1">Appendix</head><p>Appendix root.</p>
          <head n="1.1">Appendix Detail</head><p>Detail.</p>
          <head n="1.1">Appendix Detail Two</head><p>Detail two.</p>
        </body>"#;
    let sections = TeiLifter::extract_sections(tei);
    let paths: Vec<_> = sections
        .iter()
        .map(|section| section.path_id.as_deref().unwrap())
        .collect();
    assert_eq!(paths, vec!["1", "2", "1~2", "1~2.1", "1~2.1~2"]);
    let unique: HashSet<_> = paths.iter().copied().collect();
    assert_eq!(unique.len(), sections.len());
    assert_eq!(sections[3].parent_section_id.as_deref(), Some("sec-1~2"));
}

#[test]
fn test_extract_sections_flat_legacy_tei_still_works() {
    // Legacy TEI with no nested divs — flat <head n="N"> scan.
    // After MEM654 fix: path_id IS populated from <head n="N"> attribute
    // (GROBID encodes section hierarchy lexically, not via XML nesting).
    let tei = r#"<body>
          <head n="1">Intro</head><p>Intro text.</p>
          <head n="2">Methods</head><p>Methods text.</p>
        </body>"#;
    let sections = TeiLifter::extract_sections(tei);
    assert_eq!(sections.len(), 2, "flat TEI yields 2 sections");
    // path_id is now populated from n attribute (MEM654).
    assert_eq!(sections[0].path_id.as_deref(), Some("1"));
    assert_eq!(sections[1].path_id.as_deref(), Some("2"));
}

// ─── M400 S02 T01: deterministic TEI table extraction ───

/// Flat TEI: a `<figure type="table">` (no caption head, so it creates
/// no section boundary) inside the Results window maps to section "1";
/// cells carry row/column/colspan/text, spans land on the section's
/// stripped text, and digests are 64-hex.
#[test]
fn test_extract_tables_flat_section_resolved() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p>Benchmarks.</p><figure xmlns="http://www.tei-c.org/ns/1.0" type="table" xml:id="tab_0"><table><row><cell>Model</cell><cell>Score</cell></row><row><cell>GEPA</cell><cell>54.85</cell></row></table></figure></body></text></TEI>"##;
    let (tables, missingness) = TeiLifter::extract_tables(tei);
    assert_eq!(
        missingness, None,
        "tables found → no article-level missingness"
    );
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.table_id, "tab_0");
    assert_eq!(table.section_path_id.as_deref(), Some("1"));
    assert_eq!(table.section_ordinal, Some(0));
    assert_eq!(table.caption, "", "no <head> in figure → empty caption");
    assert_eq!(table.cells.len(), 4);
    assert_eq!(table.cells[0].row, 0);
    assert_eq!(table.cells[0].column, 0);
    assert_eq!(table.cells[0].text, "Model");
    assert_eq!(table.cells[1].row, 0);
    assert_eq!(table.cells[1].column, 1);
    assert_eq!(table.cells[1].text, "Score");
    assert_eq!(table.cells[3].text, "54.85");
    assert_eq!(table.source_digest.len(), 64);
    for cell in &table.cells {
        assert_eq!(cell.digest.len(), 64);
    }
    // Spans must land on the section's stripped text.
    let sections = TeiLifter::extract_sections(tei);
    let sec = &sections[0];
    assert_eq!(sec.path_id.as_deref(), Some("1"));
    for cell in &table.cells {
        let span = cell.span.expect("resolved table cells carry spans");
        let slice = &sec.text[span.start as usize..span.end as usize];
        assert_eq!(
            slice.trim(),
            cell.text,
            "span must point at the cell text in the section"
        );
    }
    // Section identity must be consistent with the section extractor.
    assert_eq!(sec.title, "Results");
}

/// Nested TEI: a table in a div's direct content maps to that div's
/// path; a table inside a nested div maps to the nested path. Spans are
/// verified against each section's stripped text.
#[test]
fn test_extract_tables_nested_section_mapping_and_spans() {
    let tei = r##"<TEI><text><body><div n="1"><head>Introduction</head><p>Intro text.</p><figure type="table" xml:id="tab_0"><head>Table 1 :</head><table><row><cell>h1</cell><cell>h2</cell></row><row><cell>42</cell><cell>7</cell></row></table></figure><div n="1.1"><head>Setup</head><p>Setup text.</p></div></div></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.section_path_id.as_deref(), Some("1"));
    assert_eq!(table.section_ordinal, Some(0));
    assert_eq!(table.caption, "Table 1 :");
    assert_eq!(table.cells.len(), 4);
    let sections = TeiLifter::extract_sections(tei);
    let sec = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1"))
        .expect("section 1");
    assert!(sec.text.contains("Intro text."));
    for cell in &table.cells {
        let span = cell.span.expect("nested resolved cells carry spans");
        let slice = &sec.text[span.start as usize..span.end as usize];
        assert_eq!(slice.trim(), cell.text);
    }
}

/// Flat TEI with a caption `<head>` inside the figure: the caption head
/// becomes a section boundary (mirroring `extract_sections_flat`), so the
/// table lands in its own caption window with a collision-resolved path
/// that exists in the section list — consistency, never invention.
#[test]
fn test_extract_tables_caption_head_creates_consistent_section() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p>Benchmarks.</p><figure xmlns="http://www.tei-c.org/ns/1.0" type="table" xml:id="tab_0"><head>Table 1 :</head><table><row><cell>a</cell></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    let sections = TeiLifter::extract_sections(tei);
    assert_eq!(sections.len(), 2, "caption head is a flat section boundary");
    assert_eq!(sections[1].title, "Table 1 :");
    let section_paths: std::collections::HashSet<&str> = sections
        .iter()
        .filter_map(|s| s.path_id.as_deref())
        .collect();
    let table = &tables[0];
    assert_eq!(table.caption, "Table 1 :");
    let path = table
        .section_path_id
        .as_deref()
        .expect("table resolves to its caption window");
    assert!(
        section_paths.contains(path),
        "table path '{path}' must exist among section paths"
    );
    let ordinal = table.section_ordinal.expect("ordinal resolved");
    assert_eq!(
        sections[ordinal].path_id.as_deref(),
        Some(path),
        "ordinal must point at the section with the table's path"
    );
}

/// A table in a top-level gap (outside every section text window) is
/// reported with `section_path_id: None` and no spans — typed
/// missingness, never guessed into a neighbour section.
#[test]
fn test_extract_tables_gap_table_path_unresolved() {
    let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha</p><div n="1.1"><head>Sub</head><p>x</p></div></div><figure type="table" xml:id="tab_0"><table><row><cell>gap</cell></row></table></figure><div n="2"><head>Methods</head><p>Beta</p></div></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert!(
        table.section_path_id.is_none(),
        "inter-div gap belongs to no section"
    );
    assert!(table.section_ordinal.is_none());
    assert!(
        table.cells[0].span.is_none(),
        "no section text window → no source span"
    );
    assert_eq!(table.cells[0].text, "gap");
}

/// TEI without any `<table>` element: article-level typed missingness
/// `NoTableElements` (R087 — an empty Vec alone cannot distinguish this
/// from a parser that did not populate tables).
#[test]
fn test_extract_tables_no_tables_typed_missingness() {
    let tei =
        r##"<TEI><text><body><head n="1">Intro</head><p>No tables here.</p></body></text></TEI>"##;
    let (tables, missingness) = TeiLifter::extract_tables(tei);
    assert!(tables.is_empty());
    assert_eq!(missingness, Some(TableMissingReason::NoTableElements));

    // No body at all → also typed missingness, not a crash.
    let header_only = r##"<TEI><teiHeader><title>No body</title></teiHeader></TEI>"##;
    let (tables, missingness) = TeiLifter::extract_tables(header_only);
    assert!(tables.is_empty());
    assert_eq!(missingness, Some(TableMissingReason::NoTableElements));
}

/// A `<table>` element with zero rows / zero cells is fail-closed: typed
/// per-table missingness `Malformed`, never invented cells.
#[test]
fn test_extract_tables_malformed_zero_rows() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><figure type="table" xml:id="tab_0"><table></table></figure></body></text></TEI>"##;
    let (tables, article_missingness) = TeiLifter::extract_tables(tei);
    assert_eq!(tables.len(), 1);
    assert_eq!(article_missingness, None, "a <table> element was found");
    let table = &tables[0];
    assert!(table.cells.is_empty());
    assert_eq!(
        table.missingness,
        Some(TableMissingReason::Malformed("zero rows".into()))
    );

    // Rows present but no cells.
    let tei = r##"<TEI><text><body><head n="1">Intro</head><figure type="table" xml:id="tab_0"><table><row></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    assert_eq!(
        tables[0].missingness,
        Some(TableMissingReason::Malformed(
            "rows found but zero cells".into()
        ))
    );
}

/// `cols="N"` colspan drives the cumulative column index; empty cells
/// are preserved so the grid stays aligned for fidelity comparison.
#[test]
fn test_extract_tables_colspan_column_accounting_and_empty_cells() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><figure type="table" xml:id="tab_0"><table><row><cell cols="2">span</cell><cell>b</cell></row><row><cell>x</cell><cell></cell><cell>z</cell></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    let table = &tables[0];
    assert_eq!(table.cells.len(), 5);
    let c0 = &table.cells[0];
    assert_eq!(
        (c0.row, c0.column, c0.colspan, c0.text.as_str()),
        (0, 0, 2, "span")
    );
    let c1 = &table.cells[1];
    assert_eq!(
        (c1.row, c1.column, c1.colspan, c1.text.as_str()),
        (0, 2, 1, "b"),
        "column accounts for the preceding colspan"
    );
    let empty = &table.cells[3];
    assert_eq!((empty.row, empty.column, empty.text.as_str()), (1, 1, ""));
    assert_eq!(
        table.cells[4].column, 2,
        "empty cell still occupies a grid column"
    );
}

/// Nested tags inside a cell (`<ref>`/`<p>`) are stripped for text but
/// the span still covers only visible chars in the section text.
#[test]
fn test_extract_tables_cell_nested_tags_span() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><figure type="table" xml:id="tab_0"><table><row><cell>See <ref type="bibr" target="#b1">[1]</ref> now</cell></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    let cell = &tables[0].cells[0];
    assert_eq!(cell.text, "See [1] now");
    let sections = TeiLifter::extract_sections(tei);
    let sec = &sections[0];
    let span = cell.span.expect("span present");
    let slice = &sec.text[span.start as usize..span.end as usize];
    assert_eq!(slice.trim(), "See [1] now");
}

/// Extraction is deterministic: the same TEI yields identical digests;
/// a changed cell text changes the cell and table digests.
#[test]
fn test_extract_tables_digest_determinism() {
    let tei_a = r##"<TEI><text><body><head n="1">Results</head><figure type="table" xml:id="tab_0"><table><row><cell>GEPA</cell><cell>54.85</cell></row></table></figure></body></text></TEI>"##;
    let tei_b = r##"<TEI><text><body><head n="1">Results</head><figure type="table" xml:id="tab_0"><table><row><cell>GEPA</cell><cell>55.00</cell></row></table></figure></body></text></TEI>"##;
    let (tables_a, _) = TeiLifter::extract_tables(tei_a);
    let (tables_a2, _) = TeiLifter::extract_tables(tei_a);
    let (tables_b, _) = TeiLifter::extract_tables(tei_b);
    assert_eq!(tables_a, tables_a2, "identical TEI → identical tables");
    assert_eq!(tables_a[0].source_digest, tables_a2[0].source_digest);
    assert_ne!(
        tables_a[0].source_digest, tables_b[0].source_digest,
        "changed cell text must change the source digest"
    );
    assert_ne!(tables_a[0].cells[1].digest, tables_b[0].cells[1].digest);
}

/// Without an `xml:id`, the table id falls back to the deterministic
/// ordinal key "table-{index}".
#[test]
fn test_extract_tables_ordinal_fallback_without_xml_id() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><figure type="table"><table><row><cell>a</cell></row></table></figure><figure type="table"><table><row><cell>b</cell></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].table_id, "table-1");
    assert_eq!(tables[1].table_id, "table-2");
}

/// Cell-level `coords` attribute (rare in GROBID) is preserved and
/// participates in the cell digest; a figure `coords` becomes the
/// table's coordinates.
#[test]
fn test_extract_tables_coordinates_preserved() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><figure type="table" xml:id="tab_0" coords="50,100,550,300;1"><table><row><cell coords="60,110,120,130;1">a</cell></row></table></figure></body></text></TEI>"##;
    let (tables, _) = TeiLifter::extract_tables(tei);
    let table = &tables[0];
    assert_eq!(table.coordinates.as_deref(), Some("50,100,550,300;1"));
    assert_eq!(
        table.cells[0].coordinates.as_deref(),
        Some("60,110,120,130;1")
    );
    // Table id extraction must not confuse `id=` inside `xml:id=`.
    assert_eq!(table.table_id, "tab_0");
}

// ─── M400 S03 T01: deterministic TEI formula extraction ───

/// Flat TEI: a `<formula>` inline inside a `<p>` in the Results window
/// maps to section "1"; label, coordinates, PlainText representation and
/// the source span are resolved, and the digest is 64-hex. The span must
/// point at the formula's visible content (text + label) in the section's
/// stripped text.
#[test]
fn test_extract_formulas_flat_section_resolved() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p>We optimize <formula xml:id="formula_0" coords="90,120,428,140;1">E = mc2<label>(1)</label></formula>.</p></body></text></TEI>"##;
    let (formulas, missingness) = TeiLifter::extract_formulas(tei);
    assert_eq!(
        missingness, None,
        "formula found → no article-level missingness"
    );
    assert_eq!(formulas.len(), 1);
    let formula = &formulas[0];
    assert_eq!(formula.formula_id, "formula_0");
    assert_eq!(formula.section_path_id.as_deref(), Some("1"));
    assert_eq!(formula.section_ordinal, Some(0));
    assert_eq!(formula.label.as_deref(), Some("(1)"));
    assert_eq!(formula.coordinates.as_deref(), Some("90,120,428,140;1"));
    assert_eq!(
        formula.representation,
        FormulaRepresentation::PlainText("E = mc2(1)".into()),
        "no <math>/<texMath> → stripped visible text"
    );
    assert_eq!(formula.missingness, None);
    assert_eq!(formula.source_digest.len(), 64);
    // Spans must land on the section's stripped text.
    let sections = TeiLifter::extract_sections(tei);
    let sec = &sections[0];
    assert_eq!(sec.path_id.as_deref(), Some("1"));
    let span = formula.span.expect("resolved formula carries a span");
    let slice = &sec.text[span.start as usize..span.end as usize];
    assert_eq!(slice.trim(), "E = mc2(1)");
    assert_eq!(sec.title, "Results");
}

/// A `<math>` subtree inside the formula becomes the MathML
/// representation (raw subtree, tags included); the label is still
/// extracted from the `<label>` child.
#[test]
fn test_extract_formulas_mathml_representation() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p><formula xml:id="formula_0"><math xmlns="http://www.w3.org/1998/Math/MathML"><mi>x</mi><mo>=</mo><mn>1</mn></math><label>(2)</label></formula></p></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 1);
    let formula = &formulas[0];
    assert_eq!(
            formula.representation,
            FormulaRepresentation::MathML(
                "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi><mo>=</mo><mn>1</mn></math>"
                    .into()
            )
        );
    assert_eq!(formula.label.as_deref(), Some("(2)"));
    assert_eq!(formula.missingness, None);
    // The span covers the MathML text nodes plus the label in the
    // section's stripped text ("x=1(2)").
    let sections = TeiLifter::extract_sections(tei);
    let span = formula.span.expect("span present");
    let slice = &sections[0].text[span.start as usize..span.end as usize];
    assert_eq!(slice.trim(), "x=1(2)");
}

/// A `<texMath>` element (other TEI producers) becomes the TeXMath
/// representation with the LaTeX source content.
#[test]
fn test_extract_formulas_texmath_representation() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p><formula xml:id="formula_0"><texMath>x = \gamma</texMath></formula></p></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 1);
    assert_eq!(
        formulas[0].representation,
        FormulaRepresentation::TeXMath("x = \\gamma".into())
    );
    assert_eq!(formulas[0].missingness, None);
}

/// Nested TEI: a formula in a div's direct content maps to that div's
/// path; spans are verified against that section's stripped text.
#[test]
fn test_extract_formulas_nested_section_mapping_and_spans() {
    let tei = r##"<TEI><text><body><div n="1"><head>Introduction</head><p>Equation <formula xml:id="formula_0">y = mx + b</formula> here.</p><div n="1.1"><head>Setup</head><p>x</p></div></div></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 1);
    let formula = &formulas[0];
    assert_eq!(formula.section_path_id.as_deref(), Some("1"));
    assert_eq!(formula.section_ordinal, Some(0));
    let sections = TeiLifter::extract_sections(tei);
    let sec = sections
        .iter()
        .find(|s| s.path_id.as_deref() == Some("1"))
        .expect("section 1");
    let span = formula
        .span
        .expect("nested resolved formula carries a span");
    let slice = &sec.text[span.start as usize..span.end as usize];
    assert_eq!(slice.trim(), "y = mx + b");
}

/// A formula in a top-level gap (outside every section text window) is
/// reported with `section_path_id: None`, no span and no ordinal — typed
/// missingness, never guessed into a neighbour section.
#[test]
fn test_extract_formulas_gap_path_unresolved() {
    let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha</p><div n="1.1"><head>Sub</head><p>x</p></div></div><p>Gap formula <formula xml:id="formula_0">gap</formula> here.</p><div n="2"><head>Methods</head><p>Beta</p></div></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 1);
    let formula = &formulas[0];
    assert!(
        formula.section_path_id.is_none(),
        "inter-div gap belongs to no section"
    );
    assert!(formula.section_ordinal.is_none());
    assert!(
        formula.span.is_none(),
        "no section text window → no source span"
    );
    assert_eq!(
        formula.representation,
        FormulaRepresentation::PlainText("gap".into())
    );
}

/// TEI without any `<formula>` element: article-level typed missingness
/// `NoFormulaElements` (R087 — an empty Vec alone cannot distinguish this
/// from a parser that did not populate formulas).
#[test]
fn test_extract_formulas_no_formulas_typed_missingness() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p>No formulas here.</p></body></text></TEI>"##;
    let (formulas, missingness) = TeiLifter::extract_formulas(tei);
    assert!(formulas.is_empty());
    assert_eq!(missingness, Some(FormulaMissingReason::NoFormulaElements));

    // No body at all → also typed missingness, not a crash.
    let header_only = r##"<TEI><teiHeader><title>No body</title></teiHeader></TEI>"##;
    let (formulas, missingness) = TeiLifter::extract_formulas(header_only);
    assert!(formulas.is_empty());
    assert_eq!(missingness, Some(FormulaMissingReason::NoFormulaElements));
}

/// A `<formula>` element with empty content is fail-closed: typed
/// per-formula missingness `Malformed` with an empty PlainText
/// representation — never an invented representation. Unclosed `<math>`
/// is also malformed.
#[test]
fn test_extract_formulas_malformed_empty() {
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p><formula xml:id="formula_0"></formula></p></body></text></TEI>"##;
    let (formulas, article_missingness) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 1);
    assert_eq!(article_missingness, None, "a <formula> element was found");
    let formula = &formulas[0];
    assert_eq!(
        formula.representation,
        FormulaRepresentation::PlainText(String::new())
    );
    assert_eq!(
        formula.missingness,
        Some(FormulaMissingReason::Malformed(
            "empty formula content".into()
        ))
    );

    // Unclosed <math> is fail-closed malformed too.
    let tei = r##"<TEI><text><body><head n="1">Intro</head><p><formula xml:id="formula_0"><math><mi>x</mi></formula></p></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(
        formulas[0].missingness,
        Some(FormulaMissingReason::Malformed(
            "math element without closing tag".into()
        ))
    );
}

/// Extraction is deterministic: the same TEI yields identical formulas;
/// a changed formula text changes the representation and the digest.
#[test]
fn test_extract_formulas_digest_determinism() {
    let tei_a = r##"<TEI><text><body><head n="1">Results</head><p><formula xml:id="formula_0">E = mc2</formula></p></body></text></TEI>"##;
    let tei_b = r##"<TEI><text><body><head n="1">Results</head><p><formula xml:id="formula_0">E = mc2!</formula></p></body></text></TEI>"##;
    let (formulas_a, _) = TeiLifter::extract_formulas(tei_a);
    let (formulas_a2, _) = TeiLifter::extract_formulas(tei_a);
    let (formulas_b, _) = TeiLifter::extract_formulas(tei_b);
    assert_eq!(
        formulas_a, formulas_a2,
        "identical TEI → identical formulas"
    );
    assert_eq!(formulas_a[0].source_digest, formulas_a2[0].source_digest);
    assert_ne!(
        formulas_a[0].source_digest, formulas_b[0].source_digest,
        "changed formula text must change the source digest"
    );
}

/// Without an `xml:id`, the formula id falls back to the deterministic
/// ordinal key "formula-{index}".
#[test]
fn test_extract_formulas_ordinal_fallback_without_xml_id() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p><formula>a</formula> <formula>b</formula></p></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    assert_eq!(formulas.len(), 2);
    assert_eq!(formulas[0].formula_id, "formula-1");
    assert_eq!(formulas[1].formula_id, "formula-2");
}

/// The `coords` attribute on the `<formula>` element is preserved and
/// participates in the digest; `xml:id` extraction must not confuse
/// `id=` inside `xml:id=`.
#[test]
fn test_extract_formulas_coordinates_preserved() {
    let tei = r##"<TEI><text><body><head n="1">Results</head><p><formula xml:id="formula_0" coords="90,120,428,140;1">a</formula></p></body></text></TEI>"##;
    let (formulas, _) = TeiLifter::extract_formulas(tei);
    let formula = &formulas[0];
    assert_eq!(formula.coordinates.as_deref(), Some("90,120,428,140;1"));
    assert_eq!(formula.formula_id, "formula_0");
}

#[test]
fn s07_t03_figures_lift_and_header_families_stay_typed_on_synthetic_shape() {
    let tei = concat!(
        r#"<TEI><teiHeader><fileDesc><titleStmt><title>T</title>"#,
        r#"<author>Alice</author></titleStmt>"#,
        r#"<sourceDesc><biblStruct><idno type="DOI">10.1/x</idno></biblStruct></sourceDesc>"#,
        r#"</fileDesc><profileDesc><abstract><p>A</p></abstract></profileDesc></teiHeader>"#,
        r#"<text><body><figure xml:id="fig_0"><head>Figure 1</head></figure><p>Body</p></body></text></TEI>"#,
    );
    let ir = GrobidParser::lift_tei(tei.to_string(), "paper-1", "hash");
    assert_eq!(ir.paper_id, "paper-1");
    assert_eq!(ir.title, "T");
    assert_eq!(ir.abstract_text, "A");
    let article = GrobidParser::project_article(ir);
    let v = serde_json::to_value(&article).unwrap();
    // S07/T02: declared identity and header authors lift through the single
    // pure lifter, header-scoped to `<persName>` only. This synthetic TEI has
    // no persName (plain-text titleStmt `<author>` is outside the declared
    // family — GROBID always emits persName) and no arXiv idno (only a DOI
    // idno), so the honest lift output is typed emptiness for authors and
    // typed `no_idno` missingness for the identity — never a guessed value.
    assert!(
        v.get("authors").is_none(),
        "no legacy-named authors field; the family is `header_authors`"
    );
    assert!(
        v.get("idno").is_none(),
        "no article-level idno; the family is `declared_arxiv_id`"
    );
    assert_eq!(
        v.get("header_authors"),
        Some(&serde_json::Value::Array(vec![])),
        "titleStmt plain-text `<author>` is outside the declared persName family"
    );
    // S07/T03: the body figure lifts (no graphic child → typed
    // no_graphic_child), and a document with no figure/table refs lifts
    // typed-empty callouts — nothing invented.
    let figures = v
        .get("figures")
        .and_then(|f| f.as_array())
        .expect("figures family present");
    assert_eq!(figures.len(), 1, "the body figure lifts since S07/T03");
    assert_eq!(
        figures[0].get("figure_id"),
        Some(&serde_json::Value::String("fig_0".to_string()))
    );
    assert_eq!(
        figures[0].get("graphic_missingness"),
        Some(&serde_json::Value::String("no_graphic_child".to_string())),
        "figure without a graphic child keeps typed missingness (R087)"
    );
    assert_eq!(
        v.get("visual_callouts"),
        Some(&serde_json::Value::Array(vec![])),
        "no refs → typed-empty callouts"
    );
    assert_eq!(v.get("declared_arxiv_id"), Some(&serde_json::Value::Null));
    assert_eq!(
        v.get("declared_id_missingness"),
        Some(&serde_json::Value::String("no_idno".to_string())),
        "header without an arXiv idno types the absence (R087), never guesses"
    );
    assert_eq!(article.layout_json, None);
    assert_eq!(article.page_text_missingness, None);
}

#[test]
fn empty_tei_projects_success_with_tei_xml_present() {
    let ir = GrobidParser::lift_tei(String::new(), "p", "h");
    let article = GrobidParser::project_article(ir);
    assert_eq!(article.paper_id, "p");
    assert_eq!(article.pdf_hash, "h");
    assert_eq!(article.tei_xml.as_deref(), Some(""));
    assert!(article.title.is_empty());
    assert!(article.sections.is_empty());
    assert!(article.citations.is_empty());
}

// ─── S02/T03 (M464-z84in2): ScholarlyParser composition units ───
//
// Full composition assertions (exactly-once, error short-circuit, facade
// parity over a stub transport) live in tests/parser_composition.rs. These
// units pin the crate-internal invariants of the pure stage.

mod composition_units {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Stateful fake recording every backend invocation.
    struct CountingBackend {
        calls: AtomicUsize,
        requests: Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl CountingBackend {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn last_request(&self) -> (String, Vec<u8>) {
            self.requests
                .lock()
                .expect("request log")
                .last()
                .cloned()
                .expect("at least one recorded request")
        }
    }

    #[async_trait::async_trait]
    impl ParseBackend for CountingBackend {
        async fn fetch_tei(
            &self,
            request: ParseBackendRequest,
        ) -> Result<TeiEnvelope, ParseBackendFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.requests
                .lock()
                .expect("request log")
                .push((request.paper_id().to_string(), request.pdf_bytes().to_vec()));
            Ok(TeiEnvelope::new(
                b"<TEI><text><body><div><head n=\"1\">S</head><p>x</p></div></body></text></TEI>"
                    .to_vec(),
                TeiIdentity::new("unit-fake", "profile=unit"),
            ))
        }
    }

    #[test]
    fn lift_and_project_is_pure_and_backend_free() {
        let parser = ScholarlyParser::new(CountingBackend::new());
        let envelope = TeiEnvelope::new(
            b"<TEI><text><body><div><head n=\"1\">Intro</head><p>Body text.</p></div></body></text></TEI>"
                .to_vec(),
            TeiIdentity::new("unit-fake", "profile=unit"),
        );

        let article = parser.lift_and_project("pure-paper", "hash-1", envelope);

        assert_eq!(
            parser.backend().call_count(),
            0,
            "the pure lift+project stage must never touch the backend"
        );
        assert_eq!(article.paper_id, "pure-paper");
        assert_eq!(article.pdf_hash, "hash-1");
        assert_eq!(article.layout_json, None);
        assert_eq!(article.page_text_missingness, None);
        assert!(
            article
                .tei_xml
                .as_deref()
                .is_some_and(|t| t.contains("Intro")),
            "tei_xml must stay Some(raw TEI)"
        );
        assert_eq!(article.sections.len(), 1);
    }

    #[tokio::test]
    async fn fetch_envelope_once_passes_logical_request_verbatim() {
        let parser = ScholarlyParser::new(CountingBackend::new());

        let envelope = parser
            .fetch_envelope_once("unit-paper", b"unit source bytes".to_vec())
            .await
            .expect("fake backend must succeed");

        assert_eq!(parser.backend().call_count(), 1);
        let (paper_id, bytes) = parser.backend().last_request();
        assert_eq!(paper_id, "unit-paper");
        assert_eq!(bytes, b"unit source bytes");
        assert_eq!(
            envelope.identity().backend_id(),
            "unit-fake",
            "envelope identity flows through the composition untouched"
        );
    }
}
