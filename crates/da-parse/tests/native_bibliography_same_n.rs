//! Independent same-n oracle for the native GROBID bibliography lane (S04/T07).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus test-local constants. It never reaches for the Java lifter, the
//! native composition entry points, the citation parser, or the reference
//! decode runner: those are the SUT lane, never the receipt. The two lanes are
//! separated by explicit source markers and the separation is asserted below.
//!
//! # Denominators, and why each is its own population
//!
//! * `biblStruct` blocks **inside the document body** — 80 / 49 / 78 = **207**.
//!   Every preserved fixture also carries one `biblStruct` inside its
//!   `teiHeader`: that is the analytic description of the paper itself, not a
//!   reference. Slicing the header off is the canonicalization that reproduces
//!   the composition receipt's `references_total` = 207, and it is the reason
//!   the lexical rule is documented rather than hidden.
//! * `<ref type="bibr">` anchors — 123 / 99 / 158 = **380**, split into
//!   87 / 71 / 59 = **217 targeted** (the opening tag declares a `target`) and
//!   36 / 28 / 99 = **163 targetless**. Targeted and targetless stay two
//!   denominators: a single pooled "anchors" number could hide the erosion of
//!   either side (MEM992).
//! * adjacent fragmented pairs — 43 / 32 / 93 = **168**, reproducing
//!   `component_receipts.anchors.fragmented_adjacency`.
//! * `<author>` elements inside the body's `biblStruct` blocks — 665 / 947 /
//!   573 = **2185**; and the header's own `<author>` elements — 21 / 4 / 11 =
//!   **36**, the `header_raw_total` denominator the header oracle pins
//!   independently (`native_header_same_n.rs`, `java_lift_same_n_oracle.rs`).
//! * bibliography text reconstruction — the same lexical rule yields
//!   1036 / 1164 / 935 = **3135** non-empty lines and 5355 / 5325 / 4735 =
//!   **15415** tokens. That token denominator is what the native ABI row
//!   builders are driven with on the SUT side, so a "one row per token" claim
//!   is anchored to a number this scan produced, not to the builders.
//!
//! # Named gaps — never closed by invention, never pooled
//!
//! * `citation_names` = **2198** is a Java-side *citation name* population. It
//!   is not the raw `<author>`-element denominator (2185 over these fixtures),
//!   not the `biblStruct` reference denominator (207), and not the header
//!   author denominator (36 raw / 35 native accepted). Substituting one for
//!   another is exactly the pooling the governor flags as
//!   `acceptance_same_n_drift` (MEM1049 / MEM1104 / MEM1112).
//! * the hub header gap (Java raw 21 vs native accepted 20) stays open here as
//!   it does in the header oracle; the bibliography scan may not compensate it.
//!
//! # Honest offline limits (507-04 research)
//!
//! * There is no preserved ALTO/layout beside these fixtures, so this oracle is
//!   TEI-to-denominators, not PDF-to-denominators.
//! * The real GROBID model bytes are not in the tree (D358), so no bibliography
//!   label sequence can be produced here. The SUT side therefore exercises the
//!   row ABI (width, totality, one row per token) over lexically derived corpus
//!   input; the decode itself is proven on hand-authored synthetic models
//!   (`native_bibliography_contract.rs`) and probed against real models only
//!   opt-in (`crates/da-adapters/tests/native_bibliography_real_model.rs`).
//! * The gazetteer lanes are supplied from a minimal project-owned bundle, so
//!   the lexicon-backed columns are not a claim about GROBID's own lexicons.

use da_parse::{
    BibliographyAlignment, BibliographyTokenFacts, BibliographyTokenInput, FactsBlockStatus,
    FactsLineStatus, NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
    citation_row, gazetteer_spans, reference_segmenter_row, render_citation_feature_text,
    render_reference_segmenter_feature_text,
};
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

// ---- independent expectation builder (no SUT lane below this marker) ----

/// Preserved fixtures, in `FIXTURES` order.
pub const FIXTURES: [&str; 3] = [
    "hub_2507.19457.tei.xml",
    "rlm_2512.24601.tei.xml",
    "reca_2605.26525v1.tei.xml",
];

/// SHA-256 of the tracked bytes, copied from `fixtures/preserved/provenance.yaml`.
pub const FIXTURE_SHA256: [&str; 3] = [
    "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
];

/// One fixture's independent bibliography receipt.
///
/// Every field is a *lexical* denominator: a count of literal element openings,
/// not a count any parser produced. `bibl_struct` and `bibl_authors` are
/// body-only (see the module docs for why the header's analytic `biblStruct` is
/// excluded); `header_authors` is the header's own author lane, kept separate so
/// it can never be swapped for the reference population.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BibliographyReceipt {
    pub bibl_struct: usize,
    pub bibr_total: usize,
    pub bibr_targeted: usize,
    pub bibr_targetless: usize,
    pub bibr_adjacent: usize,
    pub bibl_authors: usize,
    pub header_authors: usize,
    pub bibl_lines: usize,
    pub bibl_tokens: usize,
}

/// Per-fixture receipts, in `FIXTURES` order.
pub const RECEIPTS: [BibliographyReceipt; 3] = [
    BibliographyReceipt {
        bibl_struct: 80,
        bibr_total: 123,
        bibr_targeted: 87,
        bibr_targetless: 36,
        bibr_adjacent: 43,
        bibl_authors: 665,
        header_authors: 21,
        bibl_lines: 1036,
        bibl_tokens: 5355,
    },
    BibliographyReceipt {
        bibl_struct: 49,
        bibr_total: 99,
        bibr_targeted: 71,
        bibr_targetless: 28,
        bibr_adjacent: 32,
        bibl_authors: 947,
        header_authors: 4,
        bibl_lines: 1164,
        bibl_tokens: 5325,
    },
    BibliographyReceipt {
        bibl_struct: 78,
        bibr_total: 158,
        bibr_targeted: 59,
        bibr_targetless: 99,
        bibr_adjacent: 93,
        bibl_authors: 573,
        header_authors: 11,
        bibl_lines: 935,
        bibl_tokens: 4735,
    },
];

/// Independent population totals. Each is also checked against the sum of the
/// per-fixture receipts: two independent framings must agree, and no lane may
/// compensate another.
pub const TOTAL_BIBL_STRUCT: usize = 207;
pub const TOTAL_BIBR_ANCHORS: usize = 380;
pub const TOTAL_BIBR_TARGETED: usize = 217;
pub const TOTAL_BIBR_TARGETLESS: usize = 163;
pub const TOTAL_BIBR_ADJACENT: usize = 168;
pub const TOTAL_BIBL_AUTHORS: usize = 2185;
pub const TOTAL_HEADER_AUTHORS: usize = 36;
pub const TOTAL_BIBL_LINES: usize = 3135;
pub const TOTAL_BIBL_TOKENS: usize = 15415;

/// The `citation_names` population the composition receipt names.
///
/// This is a **named gap**, not a bibliography denominator: it counts citation
/// *names* on the Java side, while this oracle counts raw `<author>` element
/// openings (2185) and `biblStruct` references (207). The three numbers describe
/// three different things; pooling any pair is the failure mode MEM1049 /
/// MEM1104 / MEM1112 record. Nothing here closes the gap by invention.
pub const CORPUS_CITATION_NAMES_TOTAL: usize = 2198;

/// The hub header gap: Java reports 21 raw header authors, the native accepted
/// population is 20. Named here so the bibliography lane can never be asked to
/// compensate it (the bibliography scan is not a header scan).
pub const HUB_HEADER_RAW_AUTHORS: usize = 21;
pub const HUB_NATIVE_ACCEPTED_AUTHORS: usize = 20;

/// Recomputable digest of the canonical receipt text produced below. The
/// canonicalization is documented in [`canonical_receipts`]; regenerate the pin
/// only when the tracked fixtures themselves change.
pub const EXPECTED_RECEIPT_DIGEST: &str =
    "c13b3afb49d1d08805862a7de8d83feadb61387f2de4ec6487c065a69cf3aac3";

/// One `<ref type="bibr">` occurrence, with its body-slice byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BibliographyAnchor {
    pub start: usize,
    pub end: usize,
    pub target_present: bool,
}

/// Independent lexical inventory of the bibliography lane.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BibliographyScan {
    /// The scanned bytes, kept so the canonical receipt can recompute the
    /// adjacency topology over the same slice the counts came from.
    pub document: String,
    pub bibl_struct: usize,
    pub anchors: Vec<BibliographyAnchor>,
    /// `<author>` element openings inside the body's `biblStruct` blocks.
    pub bibl_authors: usize,
    /// `<author>` element openings inside the `teiHeader`.
    pub header_authors: usize,
    /// Line/token reconstruction of the body's `biblStruct` blocks.
    pub lines: Vec<Vec<String>>,
}

/// Slice the document body: everything after `</teiHeader>`.
///
/// The header's analytic `biblStruct` describes the paper itself, so excluding
/// the whole header region is the canonicalization that yields the pinned
/// references denominator. A preserved TEI without a `teiHeader` is refused
/// rather than silently scanned as if the whole document were body.
pub fn body_slice(xml: &str) -> &str {
    let close = "</teiHeader>";
    let end = xml.find(close).expect("preserved TEI closes its teiHeader");
    &xml[end + close.len()..]
}

/// Slice the `<teiHeader>…</teiHeader>` region.
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

/// The body slice of an already scanned document.
pub fn body_slice_of(scan: &BibliographyScan) -> &str {
    body_slice(&scan.document)
}

/// Raw `biblStruct` opening count over a whole document, header included.
///
/// The gap between this and [`BibliographyScan::bibl_struct`] is exactly the
/// header's analytic description of the paper itself.
pub fn raw_bibl_struct_openings(xml: &str) -> usize {
    openings(xml, "biblStruct").len()
}

/// Opening offsets of `<tag` whose name is followed by a boundary.
///
/// The boundary check keeps `<titleStmt>` from being read as a `<title>`, and
/// `<author>` from being read as `<authorGroup>`.
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

/// Every `<biblStruct>…</biblStruct>` block of a slice, in reading order.
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
///
/// A `target` attribute on the opening tag is the whole targeted/targetless
/// distinction: it is a property of the anchor's own bytes, not of any
/// resolution decision.
pub fn bibr_anchors(text: &str) -> Vec<BibliographyAnchor> {
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
            out.push(BibliographyAnchor {
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
///
/// This is the `fragmented_adjacency` topology: two anchors nothing but a
/// separator apart are one fragmented citation, not two.
pub fn adjacent_anchor_pairs(text: &str, anchors: &[BibliographyAnchor]) -> usize {
    anchors
        .windows(2)
        .filter(|pair| {
            text[pair[0].end..pair[1].start]
                .chars()
                .all(|c| c.is_whitespace() || matches!(c, ';' | ','))
        })
        .count()
}

/// Replace every `<…>` tag with one space, mirroring the lexical rule
/// `/<[^>]*>/ → " "`. An unterminated `<` is text, never a tag.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(relative) = text[at..].find('<') {
        let lt = at + relative;
        out.push_str(&text[at..lt]);
        match text[lt..].find('>') {
            Some(rel) => {
                out.push(' ');
                at = lt + rel + 1;
            }
            None => {
                out.push_str(&text[lt..]);
                return out;
            }
        }
    }
    out.push_str(&text[at..]);
    out
}

/// Decode the five standard XML entities the preserved TEI uses. `&amp;` is
/// decoded last so `&amp;lt;` cannot become `<`.
fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Lexical line/token reconstruction of one `biblStruct` block: tags become one
/// space, the standard entities are decoded, then each newline-separated line is
/// split on whitespace. Empty lines are dropped — a line with no tokens is not a
/// reference line.
pub fn block_lines(block: &str) -> Vec<Vec<String>> {
    let text = decode_entities(&strip_tags(block));
    text.split('\n')
        .map(|line| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

/// Scan the preserved document lexically into independent, comparable data.
pub fn scan_bibliography(xml: &str) -> BibliographyScan {
    let body = body_slice(xml);
    let blocks = bibl_struct_blocks(body);
    let mut lines = Vec::new();
    let mut bibl_authors = 0;
    for block in &blocks {
        bibl_authors += openings(block, "author").len();
        lines.extend(block_lines(block));
    }
    BibliographyScan {
        document: xml.to_owned(),
        bibl_struct: blocks.len(),
        anchors: bibr_anchors(body),
        bibl_authors,
        header_authors: openings(header_slice(xml), "author").len(),
        lines,
    }
}

/// Canonical text for the scanned receipts.
///
/// Published canonicalization, in `FIXTURES` order, one record per fixture:
///
/// ```text
/// fixture=<file name>
/// sha256=<pinned digest>
/// bibl_struct=<n>|bibr=<n>|targeted=<n>|targetless=<n>|adjacent=<n>
/// authors=<n>|header_authors=<n>|lines=<n>|tokens=<n>
/// ---
/// ```
///
/// Every field is a count the scan produced; the digest therefore moves if a
/// fixture, the scan rule, or the receipts drift.
pub fn canonical_receipts(scans: &[BibliographyScan]) -> String {
    let mut out = String::new();
    for (index, scan) in scans.iter().enumerate() {
        let targeted = scan
            .anchors
            .iter()
            .filter(|anchor| anchor.target_present)
            .count();
        let targetless = scan.anchors.len() - targeted;
        let adjacent = adjacent_anchor_pairs(body_slice_of(scan), &scan.anchors);
        out.push_str(&format!("fixture={}\n", FIXTURES[index]));
        out.push_str(&format!("sha256={}\n", FIXTURE_SHA256[index]));
        out.push_str(&format!(
            "bibl_struct={}|bibr={}|targeted={}|targetless={}|adjacent={}\n",
            scan.bibl_struct,
            scan.anchors.len(),
            targeted,
            targetless,
            adjacent,
        ));
        out.push_str(&format!(
            "authors={}|header_authors={}|lines={}|tokens={}\n",
            scan.bibl_authors,
            scan.header_authors,
            scan.lines.len(),
            scan.lines.iter().map(Vec::len).sum::<usize>(),
        ));
        out.push_str("---\n");
    }
    out
}

/// SHA-256 over [`canonical_receipts`], recomputable from the tracked fixtures.
pub fn receipts_digest(scans: &[BibliographyScan]) -> String {
    format!("{:x}", Sha256::digest(canonical_receipts(scans).as_bytes()))
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

/// Read the preserved fixtures, verify their checksums, then scan them.
fn scanned_fixtures() -> Vec<BibliographyScan> {
    FIXTURES
        .iter()
        .zip(FIXTURE_SHA256)
        .map(|(name, checksum)| {
            let bytes = fs::read(fixture_path(name)).expect("preserved fixture is tracked");
            assert_eq!(digest(&bytes), checksum, "fixture checksum drift: {name}");
            let xml = String::from_utf8(bytes).expect("preserved TEI is valid UTF-8");
            scan_bibliography(&xml)
        })
        .collect()
}

#[test]
fn lexical_bibliography_scan_matches_independent_receipts() {
    let scans = scanned_fixtures();
    assert_eq!(scans.len(), RECEIPTS.len());

    for (index, scan) in scans.iter().enumerate() {
        let receipt = RECEIPTS[index];
        let name = FIXTURES[index];
        let targeted = scan
            .anchors
            .iter()
            .filter(|anchor| anchor.target_present)
            .count();
        let targetless = scan.anchors.len() - targeted;

        assert_eq!(
            scan.bibl_struct, receipt.bibl_struct,
            "body biblStruct denominator: {name}"
        );
        assert_eq!(
            scan.anchors.len(),
            receipt.bibr_total,
            "bibr anchor denominator: {name}"
        );
        assert_eq!(targeted, receipt.bibr_targeted, "targeted anchors: {name}");
        assert_eq!(
            targetless, receipt.bibr_targetless,
            "targetless anchors: {name}"
        );
        assert_eq!(
            targeted + targetless,
            scan.anchors.len(),
            "the two anchor halves must conserve the anchor denominator: {name}"
        );
        assert_eq!(
            adjacent_anchor_pairs(body_slice_of(scan), &scan.anchors),
            receipt.bibr_adjacent,
            "fragmented adjacency pairs: {name}"
        );
        assert_eq!(
            scan.bibl_authors, receipt.bibl_authors,
            "body biblStruct author elements: {name}"
        );
        assert_eq!(
            scan.header_authors, receipt.header_authors,
            "header author elements: {name}"
        );
        assert_eq!(
            scan.lines.len(),
            receipt.bibl_lines,
            "bibliography lines: {name}"
        );
        assert_eq!(
            scan.lines.iter().map(Vec::len).sum::<usize>(),
            receipt.bibl_tokens,
            "bibliography tokens: {name}"
        );

        // Lanes are separated by content, not by coincidence of size.
        assert!(
            scan.anchors.iter().all(|anchor| anchor.end > anchor.start),
            "every anchor has a non-empty byte range: {name}"
        );
        assert!(
            scan.lines.iter().all(|line| !line.is_empty()),
            "no empty reconstructed line: {name}"
        );
        assert!(
            scan.lines
                .iter()
                .flatten()
                .all(|token| !token.chars().any(char::is_whitespace)),
            "reconstructed tokens carry no whitespace: {name}"
        );
        // The header's analytic biblStruct is excluded, so the reference lane is
        // strictly smaller than the raw `biblStruct`-opening count.
        assert_eq!(
            raw_bibl_struct_openings(&scan.document),
            scan.bibl_struct + 1,
            "exactly one header analytic sits outside the reference receipt: {name}"
        );
    }

    let sum = |pick: fn(&BibliographyScan) -> usize| scans.iter().map(pick).sum::<usize>();
    assert_eq!(sum(|scan| scan.bibl_struct), TOTAL_BIBL_STRUCT);
    assert_eq!(sum(|scan| scan.anchors.len()), TOTAL_BIBR_ANCHORS);
    assert_eq!(
        scans
            .iter()
            .map(|scan| scan
                .anchors
                .iter()
                .filter(|anchor| anchor.target_present)
                .count())
            .sum::<usize>(),
        TOTAL_BIBR_TARGETED
    );
    assert_eq!(
        scans
            .iter()
            .map(|scan| scan
                .anchors
                .iter()
                .filter(|anchor| !anchor.target_present)
                .count())
            .sum::<usize>(),
        TOTAL_BIBR_TARGETLESS
    );
    assert_eq!(
        scans
            .iter()
            .map(|scan| adjacent_anchor_pairs(body_slice_of(scan), &scan.anchors))
            .sum::<usize>(),
        TOTAL_BIBR_ADJACENT
    );
    assert_eq!(sum(|scan| scan.bibl_authors), TOTAL_BIBL_AUTHORS);
    assert_eq!(sum(|scan| scan.header_authors), TOTAL_HEADER_AUTHORS);
    assert_eq!(sum(|scan| scan.lines.len()), TOTAL_BIBL_LINES);
    assert_eq!(
        scans
            .iter()
            .map(|scan| scan.lines.iter().map(Vec::len).sum::<usize>())
            .sum::<usize>(),
        TOTAL_BIBL_TOKENS
    );
}

#[test]
fn anchor_topology_stays_two_denominators() {
    let scans = scanned_fixtures();
    assert_eq!(
        TOTAL_BIBR_TARGETED + TOTAL_BIBR_TARGETLESS,
        TOTAL_BIBR_ANCHORS
    );
    assert_ne!(
        TOTAL_BIBR_TARGETED, TOTAL_BIBR_TARGETLESS,
        "the two halves are distinct populations, not two spellings of one"
    );
    assert_ne!(TOTAL_BIBR_ANCHORS, TOTAL_BIBL_STRUCT);
    assert_ne!(TOTAL_BIBR_ADJACENT, TOTAL_BIBR_TARGETED);
    assert_ne!(TOTAL_BIBR_ADJACENT, TOTAL_BIBR_TARGETLESS);

    // Each fixture declares anchors on both sides: neither half is a
    // degenerate restatement of the total.
    for (index, scan) in scans.iter().enumerate() {
        let targeted = scan
            .anchors
            .iter()
            .filter(|anchor| anchor.target_present)
            .count();
        let targetless = scan.anchors.len() - targeted;
        assert!(targeted > 0, "targeted anchors exist: {}", FIXTURES[index]);
        assert!(
            targetless > 0,
            "targetless anchors exist: {}",
            FIXTURES[index]
        );
        assert!(
            scan.anchors
                .iter()
                .any(|anchor| anchor.target_present && anchor.end > anchor.start),
            "a targeted anchor carries a byte range: {}",
            FIXTURES[index]
        );
    }
}

#[test]
fn citation_names_gap_stays_named_and_unpooled() {
    let scans = scanned_fixtures();
    let authors: usize = scans.iter().map(|scan| scan.bibl_authors).sum();
    let references: usize = scans.iter().map(|scan| scan.bibl_struct).sum();
    assert_eq!(authors, TOTAL_BIBL_AUTHORS);
    assert_eq!(references, TOTAL_BIBL_STRUCT);

    // Three different populations, three different numbers. A substitution of
    // one for another is the pooling the governor flags.
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, references);
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, TOTAL_HEADER_AUTHORS);
    assert_ne!(CORPUS_CITATION_NAMES_TOTAL, authors);
    assert_ne!(TOTAL_HEADER_AUTHORS, references);
    assert_ne!(TOTAL_HEADER_AUTHORS, authors);
    assert_ne!(authors, references);
    assert_ne!(HUB_HEADER_RAW_AUTHORS, HUB_NATIVE_ACCEPTED_AUTHORS);

    // No sum of the independently computed lanes rebuilds the named gap: the
    // gap is not a conservation identity waiting to be closed.
    for pooled in [
        references + TOTAL_HEADER_AUTHORS,
        authors + references,
        TOTAL_HEADER_AUTHORS + authors,
        TOTAL_BIBL_STRUCT + TOTAL_BIBR_ANCHORS,
        TOTAL_BIBR_ANCHORS + authors,
        TOTAL_BIBL_TOKENS.saturating_sub(authors),
    ] {
        assert_ne!(
            pooled, CORPUS_CITATION_NAMES_TOTAL,
            "citation_names must not be reconstructible by pooling bibliography lanes"
        );
    }

    // The hub gap is a header population: the bibliography lane neither sees it
    // nor compensates it.
    let hub_authors = scans[0].bibl_authors;
    assert_ne!(hub_authors, HUB_HEADER_RAW_AUTHORS);
    assert_ne!(hub_authors, HUB_NATIVE_ACCEPTED_AUTHORS);
    assert_eq!(
        TOTAL_HEADER_AUTHORS - HUB_HEADER_RAW_AUTHORS,
        RECEIPTS[1].header_authors + RECEIPTS[2].header_authors
    );
}

#[test]
fn receipt_canonicalization_is_recomputable_and_stable() {
    let scans = scanned_fixtures();
    let first = receipts_digest(&scans);
    let second = receipts_digest(&scanned_fixtures());
    assert_eq!(first, second, "canonicalization is deterministic");
    assert_eq!(first.len(), 64, "sha256 hex digest");
    assert!(
        first.chars().all(|c| c.is_ascii_hexdigit()),
        "digest is hexadecimal"
    );
    assert_eq!(
        first, EXPECTED_RECEIPT_DIGEST,
        "canonical receipt digest drifted; recomputed value"
    );

    // Swapping two fixtures changes the digest: the canonicalization is
    // order-sensitive rather than a multiset.
    let mut swapped = scans.clone();
    swapped.swap(0, 1);
    assert_ne!(
        receipts_digest(&swapped),
        first,
        "fixture order is part of the canonical receipt"
    );
}

// ---- SUT lane: the row ABI is driven with the corpus the scan reconstructed ----

/// Minimal project-owned bundle. Only the lexical channels are named; no GROBID
/// lexicon bytes are read or committed (D358).
fn bibliography_lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["Attention".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "bibliography".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

/// One reconstructed line's facts, with the line/block statuses the row ABI
/// carries. `line_length` is the line's own token count, as the producer would
/// report it.
fn line_facts(line: &[String], block_status: FactsBlockStatus) -> Vec<BibliographyTokenFacts> {
    let last = line.len().saturating_sub(1);
    line.iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = if index == 0 {
                FactsLineStatus::Linestart
            } else if index == last {
                FactsLineStatus::Lineend
            } else {
                FactsLineStatus::Linein
            };
            BibliographyTokenFacts::from_token(&BibliographyTokenInput {
                text: text.clone(),
                block_status,
                line_status,
                alignment: BibliographyAlignment::Alignedleft,
                line_length: line.len(),
                superscript: false,
            })
        })
        .collect()
}

/// Reconstruct one block's tokens with block-level statuses: the first token of
/// the block opens it, the last closes it, everything else is inside.
fn block_facts(lines: &[Vec<String>]) -> Vec<BibliographyTokenFacts> {
    let token_count: usize = lines.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(token_count);
    for (index, line) in lines.iter().enumerate() {
        let block_status = if index == 0 {
            FactsBlockStatus::Blockstart
        } else if index == lines.len() - 1 {
            FactsBlockStatus::Blockend
        } else {
            FactsBlockStatus::Blockin
        };
        out.extend(line_facts(line, block_status));
    }
    out
}

#[test]
fn bibliography_rows_are_total_on_the_scanned_corpus() {
    let lexicon = bibliography_lexicon();
    let scans = scanned_fixtures();
    let mut citation_rows = 0usize;
    let mut reference_rows = 0usize;
    let mut blocks = 0usize;

    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        assert_eq!(
            raw_bibl_struct_openings(&scan.document),
            scan.bibl_struct + 1,
            "the header analytic stays out of the driven reference blocks: {name}"
        );
        blocks += scan.bibl_struct;

        // The citation model runs over a block, so the block boundary is
        // re-derived lexically here rather than guessed.
        let body = body_slice_of(scan);
        for block in bibl_struct_blocks(body) {
            let lines = block_lines(block);
            assert!(!lines.is_empty(), "a reference block has text: {name}");
            let tokens = block_facts(&lines);
            let texts: Vec<String> = tokens.iter().map(|token| token.text.clone()).collect();
            let spans = gazetteer_spans(&lexicon, &texts)
                .unwrap_or_else(|error| panic!("gazetteer pass failed on {name}: {error}"));
            let rows = citation_row(&tokens, &spans, &lexicon).unwrap_or_else(|error| {
                panic!("citation ABI refused corpus tokens in {name}: {error}")
            });
            assert_eq!(
                rows.len(),
                tokens.len(),
                "one citation row per token: {name}"
            );
            assert!(
                rows.iter()
                    .all(|row| row.width() == NATIVE_CITATION_FEATURE_COLUMNS),
                "every citation row carries the 30-column ABI: {name}"
            );
            let text = render_citation_feature_text(&rows)
                .unwrap_or_else(|error| panic!("citation render refused {name}: {error}"));
            assert_eq!(
                text.lines().count(),
                tokens.len(),
                "rendered citation text has one line per token: {name}"
            );

            let max_line_length = lines.iter().map(Vec::len).max().unwrap_or(1);
            for line in &lines {
                let facts = line_facts(line, FactsBlockStatus::Blockin);
                let line_rows = reference_segmenter_row(&facts, &lexicon, max_line_length)
                    .unwrap_or_else(|error| {
                        panic!("reference-segmenter ABI refused corpus tokens in {name}: {error}")
                    });
                assert_eq!(
                    line_rows.len(),
                    line.len(),
                    "one reference-segmenter row per token: {name}"
                );
                assert!(
                    line_rows
                        .iter()
                        .all(|row| row.width() == NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS),
                    "every reference-segmenter row carries the 28-column ABI: {name}"
                );
                let text = render_reference_segmenter_feature_text(&line_rows)
                    .unwrap_or_else(|error| panic!("reference render refused {name}: {error}"));
                assert_eq!(
                    text.lines().count(),
                    line.len(),
                    "rendered reference-segmenter text has one line per token: {name}"
                );
                reference_rows += line_rows.len();
            }
            citation_rows += rows.len();
        }
    }

    // The SUT-side row counts are anchored to the independent token
    // denominator, not to themselves.
    assert_eq!(blocks, TOTAL_BIBL_STRUCT);
    assert_eq!(citation_rows, TOTAL_BIBL_TOKENS);
    assert_eq!(reference_rows, TOTAL_BIBL_TOKENS);
    assert_ne!(citation_rows, TOTAL_BIBR_ANCHORS);
}

#[test]
fn scan_negatives_fail_closed_without_inventing_denominators() {
    // (1) A truncated document must shrink the receipt, never fabricate one: an
    // unterminated `<biblStruct` contributes no block and an unterminated
    // `<ref type="bibr"` contributes no anchor.
    let truncated = "<teiHeader></teiHeader><text><body>\
        <biblStruct xml:id=\"b0\"><ref type=\"bibr\" target=\"#b1\">x</ref></biblStruct>\
        <biblStruct xml:id=\"b1\"><ref type=\"bibr\"";
    let scan = scan_bibliography(truncated);
    assert_eq!(scan.bibl_struct, 1, "only the closed block is counted");
    assert_eq!(scan.anchors.len(), 1, "only the closed anchor is counted");
    assert!(scan.anchors[0].target_present);
    assert_eq!(scan.bibl_authors, 0);
    assert_eq!(scan.lines.len(), 1);

    // (2) A document without a `teiHeader` is refused rather than scanned as if
    // the whole document were body — otherwise the header's analytic
    // `biblStruct` would silently enter the reference denominator.
    assert!(std::panic::catch_unwind(|| body_slice("<text><biblStruct/></text>")).is_err());

    // (3) Targeted/targetless is a partition on any input: an anchor without a
    // `target` attribute never counts as targeted.
    let targetless = "<teiHeader></teiHeader><text><ref type=\"bibr\">y</ref></text>";
    let scan = scan_bibliography(targetless);
    assert_eq!(scan.anchors.len(), 1);
    assert!(!scan.anchors[0].target_present);

    // (4) A `ref` that is not a bibliography anchor is not counted at all, a
    // self-closing `<biblStruct/>` is not a block, and an unterminated `<` is
    // text rather than a tag.
    let other = "<teiHeader></teiHeader><text><ref type=\"figure\">z</ref><biblStruct/></text>";
    let scan = scan_bibliography(other);
    assert!(scan.anchors.is_empty());
    assert_eq!(scan.bibl_struct, 0, "a self-closing block is not a block");
    assert_eq!(
        block_lines("a < b"),
        vec![vec!["a".to_owned(), "<".to_owned(), "b".to_owned()]]
    );
}

#[test]
fn expected_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_bibliography_same_n.rs");
    let forbidden = [
        "TeiLifter",
        "compose_references",
        "run_reference_sequence",
        "NativeCitationParser",
        "decode_reference_sequence",
    ];

    // The file never imports the replaced lane at all.
    let imports: String = source
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    for name in forbidden {
        assert!(
            !imports.contains(name),
            "{name} must not be imported into the independent bibliography same-n oracle"
        );
    }

    // The expectation region contains no SUT symbol of any kind — not even the
    // row builders it is compared against.
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
        region.len() > 8_000,
        "expectation builder must be substantive, got {} bytes",
        region.len()
    );
    for name in [
        "TeiLifter",
        "compose_references",
        "run_reference_sequence",
        "NativeCitationParser",
        "decode_reference_sequence",
        "citation_row",
        "reference_segmenter_row",
        "gazetteer_spans",
        "render_citation_feature_text",
        "render_reference_segmenter_feature_text",
    ] {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }
    for name in forbidden {
        assert!(
            !region.contains(&format!("{name}(")),
            "{name} must not be called from the independent expectation builder"
        );
    }
}
