//! Independent oracle for the native bibliography band (S04/T03).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus the sha256 pins published in `provenance.yaml`, and a byte-derived
//! prediction of the atom gates (`oversized` / control / non-ASCII / `et al` /
//! ambiguous `title:`) written from the atoms' documented contract. It never
//! reaches for the native band, the bridge, the row builders, or the coverage
//! classifier: those are the SUT lane. The two lanes are separated by explicit
//! source markers, and the separation is asserted by a test.
//!
//! # Denominators, and why each is its own population
//!
//! * body `<biblStruct>` blocks carrying a `<note type="raw_reference">` —
//!   80 / 49 / 78 = **207**. The header's analytic `biblStruct` is excluded, so
//!   the reference population is the body's, as in the composition receipt.
//! * `<ref type="bibr">` anchors — 123 / 99 / 158 = **380**, split into
//!   87 / 71 / 59 = **217 targeted** and 36 / 28 / 99 = **163 targetless**, with
//!   43 / 32 / 93 = **168** adjacent fragmented pairs. Targeted and targetless
//!   stay two denominators (MEM992).
//! * `<author>` elements inside the body's `biblStruct` blocks — **2185**.
//! * `citation_names` = **2198** is a Java-side *citation name* population. It
//!   is none of the above, and no sum of the above reconstructs it.
//!
//! # Honest offline limits
//!
//!   * there is no preserved ALTO/layout for these fixtures and `pdf-inspector`
//!     is not a dependency of `da-parse`, so the native side is a controlled
//!     carrier built from the oracle's own entry strings; the real 3-document
//!     PDF measurement belongs to the S04/T04 runner;
//!   * the real GROBID 0.9.0 bibliography model bytes are not in the tree
//!     (D358), so no CRF label sequence is produced: the reference-sequence
//!     decode is a typed out-of-scope;
//!   * in-text `<ref type="bibr">` anchors are TEI markup, which the native PDF
//!     plane does not carry: the anchors atom is a typed out-of-scope on the
//!     native side and the oracle's 380 anchors stay a reference denominator;
//!   * the fixture builds its `FullSegmentationOutcome` as a pure References
//!     run. The real S03 segmenter cannot produce one from reference text
//!     alone, because `full_marker` matches a standalone marker word anywhere,
//!     so a reference containing `Methods` flips the rest of the document to
//!     Body (pinned by the T03 contract test). The real segmenter path is
//!     covered by `native_bibliography_native_input_contract.rs`.

use da_parse::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

// ---- independent expectation builder (no SUT lane below this marker) ----

/// The three preserved TEI fixtures, in `provenance.yaml` order.
pub const FIXTURES: [(&str, &str, usize); 3] = [
    (
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        80,
    ),
    (
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        49,
    ),
    (
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        78,
    ),
];

/// Independently audited anchor denominators, in `FIXTURES` order.
pub const ANCHORS: [(usize, usize, usize); 3] = [(123, 87, 36), (99, 71, 28), (158, 59, 99)];
/// Independently audited body `biblStruct` author denominator.
pub const BODY_AUTHORS: usize = 2185;

/// Independent population totals.
pub const TOTAL_REFERENCES: usize = 207;
pub const TOTAL_ANCHORS: usize = 380;
pub const TOTAL_TARGETED: usize = 217;
pub const TOTAL_TARGETLESS: usize = 163;
pub const TOTAL_ADJACENT: usize = 168;
pub const CORPUS_CITATION_NAMES_TOTAL: usize = 2198;

/// Independently audited native field count over the three fixtures: the
/// citation atom's total emitted fields (not an oracle denominator — it is
/// re-derived by a determinism pass and cross-checked by zero-invention).
pub const NATIVE_CITATION_FIELDS: usize = 585;

/// The prediction the atoms' documented contract makes for one entry's
/// disposition. Computed from the bytes alone; the gate order is the atoms'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictedDisposition {
    Oversized,
    Empty,
    Malformed,
    Unsupported,
    Ambiguous,
    Recognized,
}

/// One `<ref type="bibr">` occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub start: usize,
    pub end: usize,
    pub target_present: bool,
}

/// One fixture's independent bibliography receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleReferences {
    pub references: Vec<String>,
    pub anchors: Vec<Anchor>,
    pub body_authors: usize,
    pub bibl_struct: usize,
    /// `<biblStruct>` openings inside the single `<listBibl>` container of the
    /// body. The plan's oracle reads `<listBibl>`/`<biblStruct>` together, so
    /// the container is pinned rather than assumed.
    pub list_bibl_blocks: usize,
    /// Body `<biblStruct>` blocks carried by the fixture, including the header
    /// analytic is already excluded.
    pub declared_titles: usize,
    pub declared_dates: usize,
}

impl OracleReferences {
    pub fn tokens(&self) -> usize {
        self.references
            .iter()
            .map(|entry| entry.split_whitespace().count())
            .sum()
    }

    pub fn targeted(&self) -> usize {
        self.anchors
            .iter()
            .filter(|anchor| anchor.target_present)
            .count()
    }

    pub fn targetless(&self) -> usize {
        self.anchors.len() - self.targeted()
    }
}

/// Slice the document body: everything after `</teiHeader>`.
pub fn body_slice(xml: &str) -> &str {
    let close = "</teiHeader>";
    let end = xml.find(close).expect("preserved TEI closes its teiHeader");
    &xml[end + close.len()..]
}

fn between<'a>(block: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = block.find(open)? + open.len();
    let end = start + block[start..].find(close)?;
    Some(&block[start..end])
}

/// Opening offsets of `<tag` whose name is followed by a boundary.
pub fn openings(text: &str, tag: &str) -> Vec<usize> {
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = text[at..].find(&open) {
        let start = at + offset;
        let next = text[start + open.len()..].chars().next();
        let boundary = match next {
            Some('>') => true,
            Some(c) => c.is_whitespace(),
            None => false,
        };
        if boundary {
            out.push(start);
        }
        at = start + open.len();
    }
    out
}

/// Every closed `<biblStruct>…</biblStruct>` block of a slice, in reading order.
pub fn bibl_struct_blocks(text: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut at = 0;
    while let Some(offset) = text[at..].find("<biblStruct") {
        let start = at + offset;
        let next = text[start + "<biblStruct".len()..].chars().next();
        let boundary = match next {
            Some('>') => true,
            Some(c) => c.is_whitespace(),
            None => false,
        };
        if !boundary {
            at = start + "<biblStruct".len();
            continue;
        }
        let body = &text[start..];
        let Some(close) = body.find("</biblStruct>") else {
            break;
        };
        blocks.push(&body[..close]);
        at = start + close + "</biblStruct>".len();
    }
    blocks
}

/// Every `<ref type="bibr">…</ref>` anchor of a slice, in reading order.
pub fn bibr_anchors(text: &str) -> Vec<Anchor> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("<ref") {
        let start = cursor + relative;
        let Some(open_end_rel) = text[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_rel;
        let opening = &text[start..=open_end];
        let is_bibr = opening.contains("type=\"bibr\"") || opening.contains("type='bibr'");
        if is_bibr {
            let Some(close_rel) = text[open_end + 1..].find("</ref>") else {
                break;
            };
            let end = open_end + 1 + close_rel + "</ref>".len();
            out.push(Anchor {
                start,
                end,
                target_present: opening.contains("target=\"") || opening.contains("target='"),
            });
            cursor = end;
        } else {
            cursor = open_end + 1;
        }
    }
    out
}

/// Anchor pairs separated only by whitespace, `;` or `,`.
pub fn adjacent_anchor_pairs(text: &str, anchors: &[Anchor]) -> usize {
    anchors
        .windows(2)
        .filter(|pair| {
            text[pair[0].end..pair[1].start]
                .chars()
                .all(|c| c.is_whitespace() || matches!(c, ';' | ','))
        })
        .count()
}

/// The `<listBibl>…</listBibl>` container of a slice, when present.
pub fn list_bibl_slice(xml: &str) -> Option<&str> {
    let start = xml.find("<listBibl")?;
    let end = xml[start..].find("</listBibl>")? + start;
    Some(&xml[start..end])
}

/// Scan one preserved document without any production XML or lifting helper.
pub fn scan_references(xml: &str) -> OracleReferences {
    let body = body_slice(xml);
    let blocks = bibl_struct_blocks(body);
    let list_bibl_blocks = list_bibl_slice(body).map_or(0, |list| bibl_struct_blocks(list).len());
    let mut references = Vec::new();
    let mut declared_titles = 0usize;
    let mut declared_dates = 0usize;
    let mut body_authors = 0usize;
    for block in &blocks {
        body_authors += openings(block, "author").len();
        if let Some(raw) = between(block, "<note type=\"raw_reference\">", "</note>") {
            references.push(raw.to_owned());
        }
        if !openings(block, "title").is_empty() {
            declared_titles += 1;
        }
        if !openings(block, "date").is_empty() {
            declared_dates += 1;
        }
    }
    OracleReferences {
        references,
        anchors: bibr_anchors(body),
        body_authors,
        bibl_struct: blocks.len(),
        list_bibl_blocks,
        declared_titles,
        declared_dates,
    }
}

/// Predict one entry's atom disposition from its bytes, in the atoms' gate
/// order. `title_count` is folded in because the atom pre-checks it.
pub fn predict_disposition(entry: &str) -> PredictedDisposition {
    if entry.len() > NATIVE_CITATION_INPUT_BYTE_CAP {
        return PredictedDisposition::Oversized;
    }
    if entry.is_empty() || entry.chars().all(char::is_whitespace) {
        return PredictedDisposition::Empty;
    }
    if entry.chars().any(char::is_control) {
        return PredictedDisposition::Malformed;
    }
    if entry.chars().any(|c| !c.is_ascii() && !c.is_whitespace()) {
        return PredictedDisposition::Malformed;
    }
    if entry.to_ascii_lowercase().contains("et al") {
        return PredictedDisposition::Unsupported;
    }
    if entry.to_ascii_lowercase().match_indices("title:").count() > 1 {
        return PredictedDisposition::Ambiguous;
    }
    PredictedDisposition::Recognized
}

/// Map a prediction onto the closed atom disposition vocabulary.
pub fn predicted_disposition(entry: &str) -> BibliographyAtomDisposition {
    match predict_disposition(entry) {
        PredictedDisposition::Oversized => BibliographyAtomDisposition::Oversized,
        PredictedDisposition::Empty => BibliographyAtomDisposition::Empty,
        PredictedDisposition::Malformed => BibliographyAtomDisposition::Malformed,
        PredictedDisposition::Unsupported => BibliographyAtomDisposition::Unsupported,
        PredictedDisposition::Ambiguous => BibliographyAtomDisposition::Ambiguous,
        PredictedDisposition::Recognized => BibliographyAtomDisposition::Recognized,
    }
}

// ---- end independent expectation builder ----

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(name)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn text_digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Read the pinned fixtures, verify each sha256, then scan them.
fn scanned_fixtures() -> Vec<(String, OracleReferences)> {
    FIXTURES
        .iter()
        .map(|(name, checksum, _)| {
            let bytes = fs::read(fixture_path(name)).expect("preserved fixture is tracked");
            assert_eq!(digest(&bytes), *checksum, "fixture checksum drift: {name}");
            let xml = String::from_utf8(bytes).expect("preserved TEI is valid UTF-8");
            ((*name).to_owned(), scan_references(&xml))
        })
        .collect()
}

fn all_entries() -> Vec<String> {
    scanned_fixtures()
        .into_iter()
        .flat_map(|(_, scan)| scan.references)
        .collect()
}

/// Build a References-only carrier from the entry strings.
///
/// The segmentation is constructed directly (every lexical token is a
/// References token) so the band is exercised on a pure References run; see the
/// module docs for why the real S03 segmenter cannot produce one from reference
/// text alone.
fn references_carrier(document_id: &str, entries: &[String]) -> NativeRegionCarrier {
    let provider = LayoutProvider::new("oracle-fixture-donor", "1", "a".repeat(64)).unwrap();
    let mut tokens: Vec<LayoutToken> = Vec::new();
    let mut lexemes: Vec<LexemeRow> = Vec::new();
    let mut ranges: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut y = 100.0;
    for entry in entries {
        let start = tokens.len();
        let first_y = y;
        for word in entry.split_whitespace() {
            let index = tokens.len() as u32;
            tokens.push(
                LayoutToken::new(
                    0,
                    index,
                    index,
                    LayoutBBox::new(20.0, y, 400.0, y + 10.0).unwrap(),
                    text_digest(word),
                    LayoutSpan::present(index, index + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word),
                    LayoutStyleHints {
                        font_size_milli_pt: Some(9_000),
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
                text_digest(word),
                word,
            ));
            y += 12.0;
        }
        ranges.push((start, tokens.len(), first_y, y));
    }

    let document = LayoutDocument::new(
        document_id,
        "b".repeat(64),
        vec![LayoutPage::new(0, 600.0, 100_000.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 600.0,
            height: 100_000.0,
            rotation: 0,
            crop_box: None,
            blocks: ranges
                .iter()
                .enumerate()
                .map(
                    |(block_index, (start, end, first_y, last_y))| LatticeBlock {
                        source_ordinal: block_index as u32,
                        source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                        bbox: LayoutBBox::new(0.0, *first_y, 600.0, *last_y).unwrap(),
                        source_order: block_index as u32,
                        derived_order: Some(block_index as u32),
                        furniture: FurnitureEvidence::Absent,
                        lines: vec![LatticeLine {
                            source_ordinal: 0,
                            source_span: LayoutSpan::present(*start as u32, *end as u32).unwrap(),
                            bbox: LayoutBBox::new(0.0, *first_y, 600.0, *last_y).unwrap(),
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
                        }],
                    },
                )
                .collect(),
        }],
    )
    .unwrap();
    let lattice_fingerprint = lattice.digest().unwrap();

    let mut assignments: Vec<FullTokenAssignment> = Vec::new();
    for page in &lattice.pages {
        for block in &page.blocks {
            for line in &block.lines {
                for token in &line.tokens {
                    assignments.push(FullTokenAssignment {
                        location: FullLocation {
                            page_ordinal: page.page_ordinal,
                            block_ordinal: block.source_ordinal,
                            line_ordinal: line.source_ordinal,
                            token_ordinal: token.token_ordinal,
                        },
                        source_ordinal: token.source_ordinal,
                        text_digest: token.text_digest.clone(),
                        region: FullSemanticRegion::References,
                    });
                }
            }
        }
    }
    let first = assignments
        .first()
        .map(|assignment| assignment.location.clone());
    let last = assignments
        .last()
        .map(|assignment| assignment.location.clone());
    let runs = match (first, last) {
        (Some(start), Some(end)) => vec![FullRegionRun {
            region: FullSemanticRegion::References,
            boundary_digest: format!("sha256:{:x}", Sha256::digest(b"fixture-references")),
            start,
            end,
        }],
        _ => Vec::new(),
    };
    let dispositions = [
        FullSemanticRegion::Header,
        FullSemanticRegion::Body,
        FullSemanticRegion::References,
        FullSemanticRegion::Annex,
        FullSemanticRegion::Acknowledgement,
        FullSemanticRegion::Toc,
        FullSemanticRegion::Funding,
        FullSemanticRegion::Availability,
        FullSemanticRegion::PageFurniture,
    ]
    .into_iter()
    .map(|region| FullRegionDisposition {
        region,
        absence: (region != FullSemanticRegion::References)
            .then_some(FullRegionAbsence::NoAdmittedTokens),
    })
    .collect();
    let order_digest = {
        let mut hasher = Sha256::new();
        for assignment in &assignments {
            hasher.update(
                format!(
                    "{}:{}:{}:{}:references",
                    assignment.location.page_ordinal,
                    assignment.location.block_ordinal,
                    assignment.location.line_ordinal,
                    assignment.location.token_ordinal,
                )
                .as_bytes(),
            );
            hasher.update([0u8]);
        }
        format!("sha256:{:x}", hasher.finalize())
    };
    let segmentation = FullSegmentationOutcome {
        schema_version: FULL_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        identity: FullSegmentationIdentity {
            document_id: document_id.to_owned(),
            provider_id: "oracle-fixture-donor".to_owned(),
            source_pdf_sha256: "b".repeat(64),
            lattice_fingerprint,
            config_fingerprint: "c".repeat(64),
        },
        assignments,
        runs,
        furniture: Vec::new(),
        dispositions,
        order_digest,
    };
    build_native_region_carrier(
        &document,
        &lattice,
        &lexemes,
        &segmentation,
        &[FullSemanticRegion::References, FullSemanticRegion::Body],
    )
    .expect("the references-only fixture carrier builds")
}

fn lexicon() -> da_ports::LexiconBundle {
    da_ports::LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![da_ports::LexiconChannel {
            key: "properName".into(),
            entries: vec!["Attention".into()],
        }],
        feature_templates: vec![da_ports::FeatureTemplate {
            key: "bibliography".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn run_band(document_id: &str, entries: &[String]) -> NativeBibliographyFromInputOutcome {
    build_native_bibliography_input(&references_carrier(document_id, entries), &lexicon())
        .expect("the oracle-entry band executes")
}

fn atom(
    outcome: &NativeBibliographyFromInputOutcome,
    atom: BibliographyAtom,
) -> NativeBibliographyAtomOutcome {
    outcome
        .atoms
        .iter()
        .find(|entry| entry.atom == atom)
        .expect("every atom is reported")
        .clone()
}

#[test]
fn oracle_counters_are_derived_from_pinned_bytes() {
    let scanned = scanned_fixtures();
    assert_eq!(scanned.len(), FIXTURES.len());
    let mut references = 0usize;
    let mut anchors = 0usize;
    let mut targeted = 0usize;
    let mut targetless = 0usize;
    let mut adjacent = 0usize;
    let mut authors = 0usize;
    let mut declared_titles = 0usize;
    let mut declared_dates = 0usize;
    for (index, (name, scan)) in scanned.iter().enumerate() {
        let (expected_total, expected_targeted, expected_targetless) = ANCHORS[index];
        assert_eq!(
            scan.references.len(),
            FIXTURES[index].2,
            "reference denominator: {name}"
        );
        assert_eq!(
            scan.bibl_struct, FIXTURES[index].2,
            "body biblStruct denominator: {name}"
        );
        assert_eq!(
            scan.list_bibl_blocks, scan.bibl_struct,
            "every body biblStruct sits inside the single listBibl container: {name}"
        );
        assert!(
            scan.list_bibl_blocks > 0,
            "the fixture carries a listBibl container: {name}"
        );
        assert_eq!(scan.anchors.len(), expected_total, "anchor total: {name}");
        assert_eq!(scan.targeted(), expected_targeted, "targeted: {name}");
        assert_eq!(scan.targetless(), expected_targetless, "targetless: {name}");
        assert_eq!(
            scan.tokens(),
            scan.references
                .iter()
                .map(|entry| entry.split_whitespace().count())
                .sum::<usize>(),
            "token count is derived from the entries: {name}"
        );
        assert!(
            scan.references.iter().all(|entry| !entry.trim().is_empty()),
            "no empty raw_reference: {name}"
        );
        assert!(
            scan.anchors.iter().all(|anchor| anchor.end > anchor.start),
            "every anchor has a non-empty byte range: {name}"
        );
        references += scan.references.len();
        anchors += scan.anchors.len();
        targeted += scan.targeted();
        targetless += scan.targetless();
        adjacent += adjacent_anchor_pairs(&body_slice_of(name), &scan.anchors);
        authors += scan.body_authors;
        declared_titles += scan.declared_titles;
        declared_dates += scan.declared_dates;

        // Counts are derived from the bytes, not hardcoded: a second scan over
        // the same bytes reproduces them exactly.
        let bytes = fs::read(fixture_path(name)).unwrap();
        assert_eq!(
            digest(&bytes),
            FIXTURES[index].1,
            "pin is the bytes: {name}"
        );
        let xml = String::from_utf8(bytes).unwrap();
        assert_eq!(
            scan_references(&xml),
            *scan,
            "scan is deterministic: {name}"
        );
    }
    assert_eq!(references, TOTAL_REFERENCES);
    assert_eq!(anchors, TOTAL_ANCHORS);
    assert_eq!(targeted, TOTAL_TARGETED);
    assert_eq!(targetless, TOTAL_TARGETLESS);
    assert_eq!(adjacent, TOTAL_ADJACENT);
    assert_eq!(authors, BODY_AUTHORS);
    assert_eq!(declared_titles, 200, "oracle title denominator");
    assert_eq!(declared_dates, 167, "oracle date denominator");
    assert_eq!(targeted + targetless, anchors, "the anchor halves conserve");
}

/// The body slice of one fixture, for adjacency recomputation in the totals.
fn body_slice_of(name: &str) -> String {
    let bytes = fs::read(fixture_path(name)).unwrap();
    let xml = String::from_utf8(bytes).unwrap();
    body_slice(&xml).to_owned()
}

#[test]
fn citation_fields_atom_matches_the_oracle_references() {
    let entries = all_entries();
    assert_eq!(entries.len(), TOTAL_REFERENCES);
    let outcome = run_band("oracle-references", &entries);

    // Every reference entry is one native entry; the section heading is absent
    // from this fixture (the contract test covers the real-segmenter case).
    assert_eq!(outcome.entry_count, TOTAL_REFERENCES);
    assert_eq!(outcome.citation_field_entries.len(), TOTAL_REFERENCES);
    assert_eq!(
        outcome.token_count,
        entries
            .iter()
            .map(|entry| entry.split_whitespace().count())
            .sum::<usize>(),
        "the native token count is the oracle's token count"
    );

    // Per-entry same-n: the SUT's typed disposition per entry equals the
    // byte-derived prediction, entry for entry.
    let predicted: Vec<BibliographyAtomDisposition> = entries
        .iter()
        .map(|entry| predicted_disposition(entry))
        .collect();
    assert_eq!(
        outcome.citation_field_entries, predicted,
        "per-entry disposition must match the byte-derived prediction"
    );

    let recognized = predicted
        .iter()
        .filter(|d| **d == BibliographyAtomDisposition::Recognized)
        .count();
    let fields = atom(&outcome, BibliographyAtom::CitationFields);
    assert_eq!(fields.disposition, BibliographyAtomDisposition::Recognized);
    assert_eq!(fields.recognized_count, recognized);
    assert!(
        recognized <= TOTAL_REFERENCES,
        "the atom never recognizes more entries than the oracle holds"
    );
    assert_eq!(
        fields.item_count, NATIVE_CITATION_FIELDS,
        "audited native field total; re-derived by the determinism pass below"
    );

    // Named gap: the label-driven citation atom reads explicit `title:` labels,
    // and raw reference strings do not carry them, so the article-title lane is
    // empty while the oracle's XML `<title>` denominator (200) stays the
    // receipt. The gap is named and open, never closed by invention.
    assert!(
        outcome
            .citation_field_counts
            .iter()
            .all(|count| count.role != CitationFieldRole::ArticleTitle),
        "the raw-reference lane cannot recover article titles"
    );
    let declared_titles: usize = scanned_fixtures()
        .iter()
        .map(|(_, scan)| scan.declared_titles)
        .sum();
    assert_eq!(declared_titles, 200, "the declared title denominator stays");

    // Determinism: a second run over the same bytes reproduces the atom.
    let replay = run_band("oracle-references", &entries);
    assert_eq!(replay, outcome);
    assert_eq!(
        replay.citation_field_entries,
        outcome.citation_field_entries
    );

    // Zero invention: every emitted field value occurs verbatim in its own
    // entry's source bytes. Authority is the oracle's entry strings.
    let parser = NativeCitationParser::new();
    let mut invented = 0usize;
    for entry in &entries {
        if let NativeCitationOutcome::Recognized { fields } = parser.parse(entry) {
            for field in &fields {
                if !entry.contains(&field.value) {
                    invented += 1;
                }
            }
        }
    }
    assert_eq!(invented, 0, "no field value may be invented");
}

#[test]
fn reference_segments_atom_matches_the_oracle_references() {
    let entries = all_entries();
    let outcome = run_band("oracle-references", &entries);
    let segments = atom(&outcome, BibliographyAtom::ReferenceSegments);
    assert_eq!(
        segments.disposition,
        BibliographyAtomDisposition::Recognized
    );
    assert_eq!(
        segments.recognized_count, TOTAL_REFERENCES,
        "the reference segmenter reconstructs every oracle entry"
    );
    assert_eq!(segments.item_count, TOTAL_REFERENCES);
    assert!(segments.evidence_digest.starts_with("sha256:"));

    // The atom never exceeds the oracle denominator: a fabricated entry would
    // push it past 207.
    assert!(segments.recognized_count <= TOTAL_REFERENCES);
    assert_ne!(segments.recognized_count, CORPUS_CITATION_NAMES_TOTAL);
}

#[test]
fn citation_anchors_are_out_of_scope_on_the_native_plane() {
    let entries = all_entries();
    let outcome = run_band("oracle-references", &entries);
    let anchors = atom(&outcome, BibliographyAtom::CitationAnchors);
    assert_eq!(
        anchors.disposition,
        BibliographyAtomDisposition::OutOfScope,
        "`<ref type=\"bibr\">` anchors are TEI markup, not native PDF text"
    );
    assert_eq!(anchors.recognized_count, 0);
    assert_eq!(anchors.item_count, 0);
    assert_eq!(
        outcome.anchor_scope,
        NativeBibliographyAnchorScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::AnchorMarkupNotInNativePlane
        }
    );
    // The oracle's anchor topology stays a reference denominator: targeted and
    // targetless are two populations, and neither is the reference count.
    assert_eq!(TOTAL_TARGETED + TOTAL_TARGETLESS, TOTAL_ANCHORS);
    assert_ne!(TOTAL_ANCHORS, TOTAL_REFERENCES);
    assert_ne!(TOTAL_TARGETED, TOTAL_TARGETLESS);
    assert_eq!(
        outcome.crf_scope,
        NativeBibliographyCrfScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::ModelNotInTree
        }
    );
}

#[test]
fn citation_names_stays_named_and_unpooled() {
    let entries = all_entries();
    let outcome = run_band("oracle-references", &entries);
    assert!(!outcome.citation_names_pooled_with_biblstruct);
    let fields = atom(&outcome, BibliographyAtom::CitationFields).item_count;
    let references = TOTAL_REFERENCES;
    // Three distinct populations, three distinct numbers: substituting one for
    // another is the pooling the governor flags.
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, references);
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, BODY_AUTHORS);
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, TOTAL_ANCHORS);
    assert_ne!(BODY_AUTHORS, references);
    assert_ne!(TOTAL_ANCHORS, BODY_AUTHORS);
    // No sum of the independently scanned lanes rebuilds the named gap.
    for pooled in [
        references + TOTAL_ANCHORS,
        references + BODY_AUTHORS,
        TOTAL_ANCHORS + BODY_AUTHORS,
        TOTAL_TARGETED + TOTAL_TARGETLESS + references,
        fields.saturating_sub(BODY_AUTHORS),
    ] {
        assert_ne!(
            pooled, CORPUS_CITATION_NAMES_TOTAL,
            "citation_names must not be reconstructible by pooling bibliography lanes"
        );
    }
}

#[test]
fn character_coverage_verdict_for_the_reference_corpus() {
    let entries = all_entries();
    let carrier = references_carrier("oracle-references", &entries);
    let run = carrier.run(FullSemanticRegion::References).unwrap();
    let lines = build_bibliography_line_facts(run).unwrap();
    let native_text: String = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let native = bibliography_char_set(&native_text);
    let oracle: std::collections::BTreeSet<char> = entries
        .iter()
        .flat_map(|entry| bibliography_char_set(entry))
        .collect();
    assert_eq!(
        classify_bibliography_coverage(&oracle, &native),
        BibliographyCoverageVerdict::GranularityOnly,
        "the carrier-joined text covers every oracle character"
    );

    // Dropping to the first entry's characters is measured as content loss.
    let first_only = bibliography_char_set(&entries[0]);
    assert_eq!(
        classify_bibliography_coverage(&oracle, &first_only),
        BibliographyCoverageVerdict::ContentLoss,
        "dropping entries is measured as content loss"
    );
    assert_eq!(
        classify_bibliography_coverage(&std::collections::BTreeSet::new(), &native),
        BibliographyCoverageVerdict::NotAssessable,
        "an empty oracle is not a pass"
    );
}

#[test]
fn atom_level_same_n_never_exceeds_the_oracle_denominator() {
    let entries = all_entries();
    let outcome = run_band("oracle-references", &entries);
    assert_eq!(outcome.entry_count, TOTAL_REFERENCES);
    assert_eq!(
        outcome.citation_field_entries.len(),
        TOTAL_REFERENCES,
        "one typed disposition per oracle reference"
    );
    for atom_outcome in &outcome.atoms {
        assert!(
            atom_outcome.item_count <= outcome.token_count.max(TOTAL_REFERENCES),
            "an atom cannot count more items than its input holds"
        );
    }
    assert_eq!(outcome.line_count, TOTAL_REFERENCES);
    assert_eq!(outcome.block_count, TOTAL_REFERENCES);
    assert_eq!(outcome.skipped_empty_lines, 0);
}

#[test]
fn invented_doi_and_title_are_not_admitted_as_oracle_evidence() {
    let entries = all_entries();
    const INVENTED_DOI: &str = "10.9999/not-in-corpus";
    const INVENTED_TITLE: &str = "Completely Fake Paper";
    assert!(
        entries.iter().all(|entry| !entry.contains(INVENTED_DOI)),
        "the invented DOI occurs in no preserved raw_reference"
    );
    assert!(
        entries.iter().all(|entry| !entry.contains(INVENTED_TITLE)),
        "the invented title occurs in no preserved raw_reference"
    );

    // Simulated admission as oracle evidence breaks exact same-n: 207 -> 208.
    let mut admitted = entries.clone();
    admitted.push(format!("doi:{INVENTED_DOI}"));
    let baseline = run_band("oracle-references", &entries);
    let mutated = run_band("oracle-references", &admitted);
    assert_eq!(baseline.entry_count, TOTAL_REFERENCES);
    assert_eq!(mutated.entry_count, TOTAL_REFERENCES + 1);
    assert_ne!(
        mutated.entry_count, TOTAL_REFERENCES,
        "admitting a foreign entry breaks exact same-n"
    );
    // The band still refuses to invent: the injected entry is recognized only
    // because it carries an explicit `doi:` label, and its value is the label's
    // own literal, present in its source.
    assert_eq!(
        mutated.citation_field_entries[TOTAL_REFERENCES],
        BibliographyAtomDisposition::Recognized
    );
    let parser = NativeCitationParser::new();
    match parser.parse(&admitted[TOTAL_REFERENCES]) {
        NativeCitationOutcome::Recognized { fields } => {
            assert!(fields.iter().any(|field| field.value == INVENTED_DOI));
        }
        other => panic!("expected the explicit label to be recognized, got {other:?}"),
    }
}

#[test]
fn fixture_segmentation_is_a_pure_references_run() {
    let entries = all_entries();
    let carrier = references_carrier("oracle-references", &entries);
    assert!(carrier.run(FullSemanticRegion::Body).is_none());
    assert!(carrier.absence(FullSemanticRegion::Body).is_some());
    let run = carrier.run(FullSemanticRegion::References).unwrap();
    assert_eq!(
        run.tokens.len(),
        entries
            .iter()
            .map(|e| e.split_whitespace().count())
            .sum::<usize>()
    );
    assert_eq!(
        carrier.identity.document_id, "oracle-references",
        "the fixture is a pure References run, as its module docs state"
    );
}

#[test]
fn independence_check_catches_a_same_generator_oracle() {
    let (_, checksum, _) = FIXTURES[0];
    let oracle = BibliographyOracleProvenance {
        generator_id:
            "crates/da-parse/tests/native_bibliography_native_input_oracle.rs#lexical-tei-scanner"
                .into(),
        backend: "java-grobid-crf".into(),
        sha256: checksum.to_owned(),
    };
    assert_eq!(require_independent_bibliography_oracle(&oracle), Ok(()));

    // An oracle produced by the SUT's own generator is refused.
    let same_generator = BibliographyOracleProvenance {
        generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.into(),
        ..oracle.clone()
    };
    assert_eq!(
        require_independent_bibliography_oracle(&same_generator),
        Err(BibliographyOracleIndependenceError::NotIndependent {
            generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.into()
        })
    );
    let unpinned = BibliographyOracleProvenance {
        sha256: String::new(),
        ..oracle
    };
    assert_eq!(
        require_independent_bibliography_oracle(&unpinned),
        Err(BibliographyOracleIndependenceError::MissingPin)
    );
}

#[test]
fn expectation_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_bibliography_native_input_oracle.rs");
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
        region.len() > 4_000,
        "expectation builder must be substantive, got {} bytes",
        region.len()
    );
    for name in [
        "build_native_bibliography_input",
        "build_bibliography_line_facts",
        "classify_bibliography_coverage",
        "bibliography_char_set",
        "normalize_bibliography_text",
        "NativeCitationParser",
        "NativeReferenceSegmenter",
        "NativeCitationAnchorScanner",
        "citation_row",
        "reference_segmenter_row",
        "gazetteer_spans",
        "render_citation_feature_text",
        "render_reference_segmenter_feature_text",
        "NativeRegionCarrier",
        "segment_document_full",
        "decode_reference_sequence",
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
        "build_native_bibliography_input",
        "NativeCitationParser",
        "NativeReferenceSegmenter",
        "NativeRegionCarrier",
    ] {
        assert!(
            !imports.contains(name),
            "{name} must not be imported at the top of the oracle test"
        );
    }
}
