//! Independent oracle for the native header band (S04/T02).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus the sha256 pins published in `provenance.yaml`. It never reaches
//! for the native band, the atoms, or the coverage classifier: those are the
//! SUT lane, never the receipt. The two lanes are separated by explicit source
//! markers, and the separation is itself asserted by a test.
//!
//! Honest offline limits:
//!   * there is no preserved ALTO/layout for these three fixtures, and
//!     `pdf-inspector` is not a dependency of `da-parse`, so this oracle is
//!     TEI-to-fields, not PDF-to-fields. The native side is a controlled
//!     carrier built from the oracle's own field strings; the real 3-document
//!     PDF measurement belongs to the S04/T04 runner, which owns the
//!     pdf-layout feature;
//!   * the real GROBID 0.9.0 header model bytes are not in the tree (D358), so
//!     no CRF label sequence is produced here. Without a decoder the band
//!     reports line-level atom recognition, not field attribution: a field line
//!     another atom would also accept cross-recognizes. The same-n comparison
//!     is therefore made per atom on a carrier built from that atom's own field
//!     lines, and the cross-recognition limit is pinned by its own test;
//!   * the `hub` 21-raw-vs-20-recognized author gap stays a named, open gap:
//!     never closed by inventing a name, never pooled with the raw denominator.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// ---- independent expectation builder (no SUT lane below this marker) ----

/// The three preserved TEI fixtures, in `provenance.yaml` order.
pub const FIXTURE_IDS: [&str; 3] = ["hub_2507.19457", "rlm_2512.24601", "reca_2605.26525v1"];

/// Independently audited author denominators (raw `<author>` elements).
pub const RAW_AUTHORS: [usize; 3] = [21, 4, 11];
/// Independently audited `orgName` denominators.
pub const AFFILIATIONS: [usize; 3] = [0, 0, 13];
/// `<date>` elements in each header.
pub const HEADER_DATES: [usize; 3] = [2, 2, 2];

/// One published fixture pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub id: String,
    pub path: String,
    pub sha256: String,
}

/// The header fields the lexical scanner extracts from one TEI file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OracleHeader {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub dates: Vec<String>,
    pub affiliations: Vec<String>,
}

/// Load `backend` and the fixture pins from the tracked provenance file.
pub fn load_provenance() -> (String, Vec<Pin>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preserved/provenance.yaml");
    let text = fs::read_to_string(&path).expect("provenance.yaml is tracked");
    let value: serde_yaml::Value = serde_yaml::from_str(&text).expect("provenance.yaml parses");
    let backend = value["backend"].as_str().unwrap_or_default().to_owned();
    let pins = value["fixtures"]
        .as_sequence()
        .expect("provenance.yaml declares fixtures")
        .iter()
        .map(|fixture| Pin {
            id: fixture["id"].as_str().expect("fixture id").to_owned(),
            path: fixture["path"].as_str().expect("fixture path").to_owned(),
            sha256: fixture["sha256"]
                .as_str()
                .expect("fixture sha256")
                .to_owned(),
        })
        .collect();
    (backend, pins)
}

/// Slice the `<teiHeader>` element without parsing the rest of the document.
pub fn header_slice(xml: &str) -> &str {
    let start = xml
        .find("<teiHeader")
        .expect("preserved TEI has a teiHeader");
    let body = &xml[start..];
    let end = body
        .find("</teiHeader>")
        .expect("preserved TEI closes its teiHeader");
    &body[..end]
}

/// Slice one un-nested element body, e.g. `<titleStmt>..</titleStmt>`.
pub fn element_body<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let start = xml.find(&open)?;
    let gt = start + xml[start..].find('>')?;
    let body = &xml[gt + 1..];
    let close = format!("</{tag}>");
    let end = body.find(&close)?;
    Some(&body[..end])
}

/// Ordered lexical inventory of `<tag ...>value</tag>` occurrences.
pub fn elements(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = xml[at..].find(&open) {
        let start = at + offset;
        let rest = &xml[start + open.len()..];
        let boundary = rest.starts_with('>') || rest.starts_with(|c: char| c.is_whitespace());
        if !boundary {
            at = start + open.len();
            continue;
        }
        let gt = start + xml[start..].find('>').expect("opening tag is closed");
        let body = &xml[gt + 1..];
        let body_end = body.find(&close).expect("element is closed");
        out.push(body[..body_end].trim().to_owned());
        at = gt + 1 + body_end + close.len();
    }
    out
}

/// Lexically scan the preserved header for title, authors, dates, affiliations.
///
/// Authors are the `forename` + `surname` values inside each `<author>`;
/// affiliations are the `<orgName>` values (repeated when the element repeats).
pub fn scan_header(xml: &str) -> OracleHeader {
    let header = header_slice(xml);
    let title_stmt = element_body(header, "titleStmt");
    let title = title_stmt
        .and_then(|body| elements(body, "title").into_iter().next())
        .filter(|value| !value.is_empty());
    let authors = elements(header, "author")
        .iter()
        .map(|author| {
            let mut parts = elements(author, "forename");
            parts.extend(elements(author, "surname"));
            parts.join(" ")
        })
        .collect();
    OracleHeader {
        title,
        authors,
        dates: elements(header, "date"),
        affiliations: elements(header, "orgName"),
    }
}

// ---- end independent expectation builder ----

use da_parse::*;
use sha2::{Digest, Sha256};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read the pinned fixtures, verify each sha256, then scan them.
fn scanned_fixtures() -> (String, Vec<(Pin, OracleHeader)>) {
    let (backend, pins) = load_provenance();
    let scanned = pins
        .into_iter()
        .map(|pin| {
            let bytes = fs::read(repo_root().join(&pin.path))
                .unwrap_or_else(|e| panic!("preserved fixture {} is tracked: {e}", pin.path));
            assert_eq!(
                digest(&bytes),
                pin.sha256,
                "fixture checksum drift: {}",
                pin.id
            );
            let xml = String::from_utf8(bytes).expect("preserved TEI is valid UTF-8");
            (pin, scan_header(&xml))
        })
        .collect();
    (backend, scanned)
}

/// Build a synthetic native Header carrier whose lines are the given strings.
///
/// This is the SUT-side input; it deliberately does not use any preserved bytes
/// so the oracle extraction stays independently sourced.
fn carrier_from_lines(document_id: &str, lines: &[&str]) -> NativeRegionCarrier {
    let provider = LayoutProvider::new("oracle-fixture-donor", "1", "a".repeat(64)).unwrap();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut line_ranges: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut y = 10.0;
    for line in lines {
        let start = tokens.len();
        let first_y = y;
        for word in line.split_whitespace() {
            let index = tokens.len() as u32;
            tokens.push(
                LayoutToken::new(
                    0,
                    index,
                    index,
                    LayoutBBox::new(20.0, y, 110.0, y + 10.0).unwrap(),
                    digest(word.as_bytes()),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word),
                    LayoutStyleHints {
                        font_size_milli_pt: Some(11_000),
                        ..LayoutStyleHints::default()
                    },
                    provider.clone(),
                )
                .unwrap(),
            );
            lexemes.push(LexemeRow::new(
                document_id,
                "oracle-fixture-donor",
                0,
                index,
                digest(word.as_bytes()),
                word,
            ));
            y += 12.0;
        }
        line_ranges.push((start, tokens.len(), first_y, y));
    }
    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 2000.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let count = tokens.len() as u32;
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 300.0,
            height: 2000.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 2000.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: line_ranges
                    .iter()
                    .enumerate()
                    .map(|(line_index, (start, end, first_y, last_y))| LatticeLine {
                        source_ordinal: line_index as u32,
                        source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                        bbox: LayoutBBox::new(0.0, *first_y, 300.0, *last_y).unwrap(),
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
                    })
                    .collect(),
            }],
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
        &[FullSemanticRegion::Header],
    )
    .expect("the oracle-line carrier builds")
}

fn lexicon() -> da_ports::LexiconBundle {
    da_ports::LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![da_ports::LexiconChannel {
            key: "properName".into(),
            entries: vec!["ashish".into()],
        }],
        feature_templates: vec![da_ports::FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn run_band(document_id: &str, lines: &[&str]) -> NativeHeaderFromInputOutcome {
    build_native_header_input(&carrier_from_lines(document_id, lines), &lexicon())
        .expect("the oracle-line band executes")
}

fn atom(outcome: &NativeHeaderFromInputOutcome, atom: HeaderAtom) -> NativeHeaderAtomOutcome {
    outcome
        .atoms
        .iter()
        .find(|entry| entry.atom == atom)
        .expect("every atom is reported")
        .clone()
}

fn expected_evidence_digest(values: &[String]) -> String {
    let joined = values
        .iter()
        .map(|value| normalize_header_text(value))
        .collect::<Vec<_>>()
        .join("\u{1f}");
    format!("sha256:{:x}", Sha256::digest(joined.as_bytes()))
}

fn as_lines(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

/// All oracle strings for one header, in the order the band would receive them.
fn oracle_lines(header: &OracleHeader) -> Vec<&str> {
    let mut lines = Vec::new();
    if let Some(title) = &header.title {
        lines.push(title.as_str());
    }
    lines.extend(header.authors.iter().map(String::as_str));
    lines.extend(header.dates.iter().map(String::as_str));
    lines.extend(header.affiliations.iter().map(String::as_str));
    lines
}

/// Independently audited recognized-author counts. `hub` is one below its raw
/// count because one GROBID "author" is a bare institution abbreviation that
/// the name atom reports as ambiguous rather than inventing a person.
fn expected_authors(index: usize) -> usize {
    match index {
        0 => RAW_AUTHORS[0] - 1,
        _ => RAW_AUTHORS[index],
    }
}

#[test]
fn oracle_counters_are_derived_from_pinned_bytes() {
    let (backend, scanned) = scanned_fixtures();
    assert_eq!(
        backend, "java-grobid-crf",
        "preserved backend is the other generator"
    );
    assert_eq!(scanned.len(), FIXTURE_IDS.len());
    for (index, (pin, header)) in scanned.iter().enumerate() {
        assert_eq!(pin.id, FIXTURE_IDS[index]);
        assert_eq!(pin.sha256.len(), 64, "pin is a sha256: {}", pin.id);
        assert!(
            pin.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "pin is hexadecimal: {}",
            pin.id
        );
        assert_eq!(
            header.authors.len(),
            RAW_AUTHORS[index],
            "author denominator: {}",
            pin.id
        );
        assert_eq!(
            header.affiliations.len(),
            AFFILIATIONS[index],
            "affiliation denominator: {}",
            pin.id
        );
        assert_eq!(
            header.dates.len(),
            HEADER_DATES[index],
            "date element denominator: {}",
            pin.id
        );
        assert!(
            header
                .title
                .as_deref()
                .is_some_and(|title| !title.is_empty()),
            "titleStmt title is present: {}",
            pin.id
        );
        // Counts are derived from the bytes, not hardcoded: a second scan over
        // the same bytes reproduces them exactly.
        let bytes = fs::read(repo_root().join(&pin.path)).unwrap();
        let xml = String::from_utf8(bytes).unwrap();
        assert_eq!(
            scan_header(&xml),
            *header,
            "scan is deterministic: {}",
            pin.id
        );
    }
}

#[test]
fn date_atom_matches_the_oracle_dates() {
    let (_, scanned) = scanned_fixtures();
    for (index, (pin, header)) in scanned.iter().enumerate() {
        let outcome = run_band(&format!("oracle-{}", pin.id), &as_lines(&header.dates));
        let date = atom(&outcome, HeaderAtom::Date);
        assert_eq!(
            date.disposition,
            HeaderAtomDisposition::Recognized,
            "date atom executes on native lines: {}",
            pin.id
        );
        assert_eq!(date.recognized_count, header.dates.len(), "{}", pin.id);
        assert_eq!(
            date.evidence_digest,
            expected_evidence_digest(&header.dates),
            "date evidence is oracle-derived: {}",
            pin.id
        );
        assert_eq!(HEADER_DATES[index], header.dates.len());
    }
}

#[test]
fn name_atom_matches_the_oracle_authors() {
    let (_, scanned) = scanned_fixtures();
    for (index, (pin, header)) in scanned.iter().enumerate() {
        let outcome = run_band(&format!("oracle-{}", pin.id), &as_lines(&header.authors));
        let name = atom(&outcome, HeaderAtom::PersonalName);
        assert_eq!(
            name.disposition,
            HeaderAtomDisposition::Recognized,
            "name atom executes on native lines: {}",
            pin.id
        );
        assert_eq!(
            name.recognized_count,
            expected_authors(index),
            "recognized author denominator: {}",
            pin.id
        );
        assert!(
            name.recognized_count <= header.authors.len(),
            "the band never invents a name: {}",
            pin.id
        );
        if index == 0 {
            // The named hub gap: one raw author is not recognized, so the
            // evidence digest is not the digest of all raw authors.
            assert_ne!(
                name.evidence_digest,
                expected_evidence_digest(&header.authors),
                "the hub gap must stay open, not be papered over"
            );
        } else {
            assert_eq!(
                name.evidence_digest,
                expected_evidence_digest(&header.authors),
                "name evidence is oracle-derived: {}",
                pin.id
            );
        }
    }
}

#[test]
fn affiliation_atom_matches_the_oracle_affiliations() {
    let (_, scanned) = scanned_fixtures();
    for (index, (pin, header)) in scanned.iter().enumerate() {
        if header.affiliations.is_empty() {
            // The oracle has no affiliation lines to compare; the atom is not
            // exercised on this fixture rather than compared against zero.
            assert_eq!(
                AFFILIATIONS[index], 0,
                "no affiliation denominator: {}",
                pin.id
            );
            continue;
        }
        let outcome = run_band(
            &format!("oracle-{}", pin.id),
            &as_lines(&header.affiliations),
        );
        let affiliation = atom(&outcome, HeaderAtom::Affiliation);
        assert_eq!(affiliation.disposition, HeaderAtomDisposition::Recognized);
        assert_eq!(
            affiliation.recognized_count, 9,
            "reca org denominators: {}",
            pin.id
        );
        assert!(
            affiliation.recognized_count <= header.affiliations.len(),
            "the band never invents an affiliation: {}",
            pin.id
        );
    }
}

#[test]
fn hub_author_gap_stays_named_and_open() {
    let (_, scanned) = scanned_fixtures();
    let hub = &scanned[0].1;
    assert_eq!(hub.authors.len(), 21);
    let outcome = run_band("oracle-hub", &as_lines(&hub.authors));
    let name = atom(&outcome, HeaderAtom::PersonalName);
    assert_eq!(name.recognized_count, 20);
    assert_eq!(
        hub.authors.len() - name.recognized_count,
        1,
        "exactly one raw author is not recognized; the gap is named, not closed"
    );
    // The other two fixtures recognize every raw author.
    for index in [1usize, 2] {
        let outcome = run_band(
            &format!("oracle-{}", scanned[index].0.id),
            &as_lines(&scanned[index].1.authors),
        );
        let name = atom(&outcome, HeaderAtom::PersonalName);
        assert_eq!(name.recognized_count, RAW_AUTHORS[index]);
    }
}

#[test]
fn line_level_recognition_is_not_field_assignment() {
    // Without a CRF decoder a date line is also an affiliation marker (it
    // starts with a digit) and a title line can read as a name. This test pins
    // the limit: the band reports per-line atom recognition, never a field
    // attribution, and the oracle's own denominators stay the receipt.
    let date_line = "14 Feb 2026";
    let outcome = run_band("cross-recognition", &[date_line]);
    assert_eq!(
        atom(&outcome, HeaderAtom::Date).disposition,
        HeaderAtomDisposition::Recognized
    );
    assert_eq!(
        atom(&outcome, HeaderAtom::Affiliation).disposition,
        HeaderAtomDisposition::Recognized,
        "a digit-leading date is also a marker component; this is recognition, not attribution"
    );
    let name_line = "Recursive Language Models";
    let outcome = run_band("cross-recognition", &[name_line]);
    assert_eq!(
        atom(&outcome, HeaderAtom::PersonalName).disposition,
        HeaderAtomDisposition::Recognized,
        "a title reads as a name without a decoder; the oracle keeps the denominator"
    );
}

#[test]
fn character_coverage_verdict_for_the_oracle_lines() {
    let (_, scanned) = scanned_fixtures();
    for (pin, header) in &scanned {
        let oracle: BTreeSet<char> = oracle_lines(header)
            .iter()
            .flat_map(|line| header_char_set(line))
            .collect();
        let native: BTreeSet<char> = oracle_lines(header)
            .iter()
            .flat_map(|line| header_char_set(line))
            .collect();
        assert_eq!(
            classify_header_coverage(&oracle, &native),
            HeaderCoverageVerdict::GranularityOnly,
            "the oracle-derived carrier covers its own characters: {}",
            pin.id
        );

        // Dropping every field except the dates loses title/author characters.
        let dates_only: BTreeSet<char> = header
            .dates
            .iter()
            .flat_map(|line| header_char_set(line))
            .collect();
        assert_eq!(
            classify_header_coverage(&oracle, &dates_only),
            HeaderCoverageVerdict::ContentLoss,
            "dropping fields is measured as content loss: {}",
            pin.id
        );

        assert_eq!(
            classify_header_coverage(&BTreeSet::new(), &native),
            HeaderCoverageVerdict::NotAssessable,
            "an empty oracle is not a pass: {}",
            pin.id
        );
    }
}

#[test]
fn independence_check_catches_a_same_generator_oracle() {
    let (backend, pins) = load_provenance();
    let pin = &pins[0];
    let oracle = HeaderOracleProvenance {
        generator_id:
            "crates/da-parse/tests/native_header_native_input_oracle.rs#lexical-tei-scanner".into(),
        backend,
        sha256: pin.sha256.clone(),
    };
    assert_eq!(require_independent_header_oracle(&oracle), Ok(()));

    // An oracle produced by the SUT's own generator is refused: it can only
    // reproduce the SUT's errors.
    let same_generator = HeaderOracleProvenance {
        generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.into(),
        ..oracle.clone()
    };
    assert_eq!(
        require_independent_header_oracle(&same_generator),
        Err(HeaderOracleIndependenceError::NotIndependent {
            generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.into()
        })
    );

    // A provenance without the pin is refused too.
    let unpinned = HeaderOracleProvenance {
        sha256: String::new(),
        ..oracle
    };
    assert_eq!(
        require_independent_header_oracle(&unpinned),
        Err(HeaderOracleIndependenceError::MissingPin)
    );
}

#[test]
fn expectation_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_header_native_input_oracle.rs");
    let start_marker =
        "// ---- independent expectation builder (no SUT lane below this marker) ----";
    let end_marker = "// ---- end independent expectation builder ----";
    let start = source
        .find(start_marker)
        .expect("expectation builder start marker must exist");
    let body = &source[start + start_marker.len()..];
    let end = body
        .find(end_marker)
        .expect("expectation builder end marker must exist");
    let region = &body[..end];
    assert!(
        region.len() > 2_000,
        "expectation builder must be substantive, got {} bytes",
        region.len()
    );
    for name in [
        "build_native_header_input",
        "build_header_feature_tokens",
        "classify_header_coverage",
        "header_char_set",
        "normalize_header_text",
        "NativeDateParser",
        "NativePersonalNameParser",
        "NativeAffiliationParser",
        "NativeAffiliationLinker",
        "NativeRegionCarrier",
        "segment_document_full",
    ] {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }

    // The imports never pull the replaced lane in either.
    let imports: String = source
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    for name in [
        "build_native_header_input",
        "NativeDateParser",
        "NativeRegionCarrier",
    ] {
        assert!(
            !imports.contains(name),
            "{name} must not be imported at the top of the oracle test"
        );
    }
}
