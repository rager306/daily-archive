//! Independent same-n oracle for the complete native GROBID header lane (S03/T05).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus test-local constants.  It never reaches for the Java lifter, the
//! composition entry points, or the header decode runner: those are the SUT
//! lane, never the receipt.  The two lanes are separated by explicit source
//! markers and the separation is itself asserted by a test.
//!
//! Honest offline limits (507-03 research §6):
//!   * no preserved ALTO/layout exists for these three fixtures, so this oracle
//!     is TEI-to-fields, not PDF-to-fields;
//!   * the real header model bytes are not in the tree (D358), so no CRF label
//!     sequence can be produced here.  The projection lane is therefore
//!     exercised on rows and spans derived lexically from the same TEI: that
//!     proves the projection is faithful to its input, and it does not pretend
//!     a decode happened;
//!   * the `hub` 20-vs-21 author gap stays a named, open gap — never closed by
//!     inventing a name, never pooled with the raw denominator.

use da_parse::{
    HeaderFeatureRow, HeaderFieldSpan, NativeHeaderFields, project_native_header_fields,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

// ---- independent expectation builder (no SUT lane below this marker) ----

pub const FIXTURES: [&str; 3] = [
    "hub_2507.19457.tei.xml",
    "rlm_2512.24601.tei.xml",
    "reca_2605.26525v1.tei.xml",
];

pub const FIXTURE_SHA256: [&str; 3] = [
    "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
];

/// One fixture's independent receipt.
///
/// `java_raw_authors` and `native_accepted_authors` are the two distinct
/// populations the header lane must keep apart.  They come from the independent
/// S09 audit receipts (the same denominators `java_lift_same_n_oracle.rs` and
/// `native_composition_oracle.rs` pin), not from either implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderReceipt {
    pub java_raw_authors: usize,
    pub native_accepted_authors: usize,
    pub title_stmt_titles: usize,
    pub header_title_elements: usize,
    pub emails: usize,
    pub idno_elements: usize,
    pub pubnum_elements: usize,
    pub checksum_idno_elements: usize,
    pub header_date_elements: usize,
    pub document_dates: usize,
}

/// Per-fixture receipts, in `FIXTURES` order.
pub const RECEIPTS: [HeaderReceipt; 3] = [
    HeaderReceipt {
        java_raw_authors: 21,
        native_accepted_authors: 20,
        title_stmt_titles: 1,
        header_title_elements: 2,
        emails: 0,
        idno_elements: 2,
        pubnum_elements: 1,
        checksum_idno_elements: 1,
        header_date_elements: 2,
        document_dates: 80,
    },
    HeaderReceipt {
        java_raw_authors: 4,
        native_accepted_authors: 4,
        title_stmt_titles: 1,
        header_title_elements: 2,
        emails: 3,
        idno_elements: 2,
        pubnum_elements: 1,
        checksum_idno_elements: 1,
        header_date_elements: 2,
        document_dates: 50,
    },
    HeaderReceipt {
        java_raw_authors: 11,
        native_accepted_authors: 11,
        title_stmt_titles: 1,
        header_title_elements: 2,
        emails: 0,
        idno_elements: 2,
        pubnum_elements: 1,
        checksum_idno_elements: 1,
        header_date_elements: 2,
        document_dates: 43,
    },
];

/// Independent population totals.  Each is also checked against the sum of the
/// per-fixture receipts: two independent framings must agree, and neither lane
/// may compensate the other.
pub const TOTAL_JAVA_RAW_AUTHORS: usize = 36;
pub const TOTAL_NATIVE_ACCEPTED_AUTHORS: usize = 35;
pub const TOTAL_TITLE_STMT_TITLES: usize = 3;
pub const TOTAL_HEADER_TITLE_ELEMENTS: usize = 6;
pub const TOTAL_EMAILS: usize = 3;
pub const TOTAL_IDNO_ELEMENTS: usize = 6;
pub const TOTAL_PUBNUM_ELEMENTS: usize = 3;
pub const TOTAL_CHECKSUM_IDNO_ELEMENTS: usize = 3;
pub const TOTAL_HEADER_DATE_ELEMENTS: usize = 6;
pub const TOTAL_DOCUMENT_DATES: usize = 173;

/// The named, open gap.  Never closed by invention, never pooled with the raw
/// denominator.  Java reports 21 raw `hub` authors; the native accepted
/// population is 20.  The offset exists on `hub` only.
pub const HUB_JAVA_HEADER_RAW: usize = 21;
pub const HUB_NATIVE_ACCEPTED_HEADER: usize = 20;

/// Recomputable digest of the canonical receipt text produced below.  The
/// canonicalization is documented in [`canonical_receipts`]; regenerate the pin
/// only when the tracked fixtures themselves change.
pub const EXPECTED_RECEIPT_DIGEST: &str =
    "beff81487c73f6bd76fa44dbec37b871ac5aac33bcf3e1bdba6e3b1384a7bbe0";

/// One `<tag ...>value</tag>` occurrence from the preserved TEI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub attributes: String,
    pub value: String,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        let needle = format!("{name}=\"");
        let at = self.attributes.find(&needle)? + needle.len();
        let rest = &self.attributes[at..];
        Some(&rest[..rest.find('"')?])
    }
}

/// Lexical header inventory.  Nothing here calls a parser, a lifter, or a
/// composer: it is the expected side of the comparison.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderScan {
    pub titles: Vec<String>,
    pub title_stmt_titles: Vec<String>,
    pub emails: Vec<String>,
    pub idnos: Vec<String>,
    pub pubnums: Vec<String>,
    pub checksum_idnos: Vec<String>,
    pub authors: usize,
    pub header_dates: usize,
    pub document_dates: usize,
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
///
/// The boundary check keeps `<titleStmt>` from being read as a `<title>`.
pub fn elements(xml: &str, tag: &str) -> Vec<Element> {
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
        let opening = &xml[start..=gt];
        let body = &xml[gt + 1..];
        let body_end = body.find(&close).expect("element is closed");
        out.push(Element {
            attributes: opening[open.len()..opening.len() - 1].to_owned(),
            value: body[..body_end].trim().to_owned(),
        });
        at = gt + 1 + body_end + close.len();
    }
    out
}

fn values(elements: &[Element]) -> Vec<String> {
    elements.iter().map(|e| e.value.clone()).collect()
}

fn is_checksum(element: &Element) -> bool {
    element
        .attribute("type")
        .is_some_and(|value| value.eq_ignore_ascii_case("MD5"))
}

/// Scan the preserved header lexically into independent, comparable data.
///
/// `document_dates` deliberately counts `<date` over the *whole* document, which
/// is the canonicalization the pre-existing `date_total == 173` denominator
/// uses; `header_dates` counts only the header's own `<date` elements.
pub fn scan_header(xml: &str) -> HeaderScan {
    let header = header_slice(xml);
    let title_stmt = element_body(header, "titleStmt").expect("preserved TEI has a titleStmt");
    let idnos = elements(header, "idno");
    let checksum_idnos = idnos
        .iter()
        .filter(|element| is_checksum(element))
        .map(|element| element.value.clone())
        .collect();
    let pubnums = idnos
        .iter()
        .filter(|element| !is_checksum(element))
        .map(|element| element.value.clone())
        .collect();
    HeaderScan {
        titles: values(&elements(header, "title")),
        title_stmt_titles: values(&elements(title_stmt, "title")),
        emails: values(&elements(header, "email")),
        idnos: values(&idnos),
        pubnums,
        checksum_idnos,
        authors: elements(header, "author").len(),
        header_dates: elements(header, "date").len(),
        document_dates: xml.matches("<date").count(),
    }
}

/// Canonical text for the scanned receipts.
///
/// Published canonicalization, in `FIXTURES` order, one record per fixture:
///
/// ```text
/// fixture=<file name>
/// sha256=<pinned digest>
/// authors=<raw>|accepted=<native>|title_stmt_titles=<n>|title_elements=<n>
/// emails=<n>|idnos=<n>|pubnums=<n>|checksum_idnos=<n>|header_dates=<n>|document_dates=<n>
/// title=<titleStmt title>
/// emails=<US-joined values>
/// pubnums=<US-joined values>
/// checksum_idnos=<US-joined values>
/// ---
/// ```
///
/// `US` is U+001F (unit separator) so joined values can never collide with the
/// record separators.
pub fn canonical_receipts(scans: &[HeaderScan]) -> String {
    let mut out = String::new();
    for (index, scan) in scans.iter().enumerate() {
        let receipt = RECEIPTS[index];
        out.push_str(&format!("fixture={}\n", FIXTURES[index]));
        out.push_str(&format!("sha256={}\n", FIXTURE_SHA256[index]));
        out.push_str(&format!(
            "authors={}|accepted={}|title_stmt_titles={}|title_elements={}\n",
            scan.authors,
            receipt.native_accepted_authors,
            scan.title_stmt_titles.len(),
            scan.titles.len(),
        ));
        out.push_str(&format!(
            "emails={}|idnos={}|pubnums={}|checksum_idnos={}|header_dates={}|document_dates={}\n",
            scan.emails.len(),
            scan.idnos.len(),
            scan.pubnums.len(),
            scan.checksum_idnos.len(),
            scan.header_dates,
            scan.document_dates,
        ));
        out.push_str(&format!(
            "title={}\n",
            scan.title_stmt_titles.join("\u{1f}")
        ));
        out.push_str(&format!("emails={}\n", scan.emails.join("\u{1f}")));
        out.push_str(&format!("pubnums={}\n", scan.pubnums.join("\u{1f}")));
        out.push_str(&format!(
            "checksum_idnos={}\n",
            scan.checksum_idnos.join("\u{1f}")
        ));
        out.push_str("---\n");
    }
    out
}

/// SHA-256 over [`canonical_receipts`], recomputable from the tracked fixtures.
pub fn receipts_digest(scans: &[HeaderScan]) -> String {
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
fn scanned_fixtures() -> Vec<HeaderScan> {
    FIXTURES
        .iter()
        .zip(FIXTURE_SHA256)
        .map(|(name, checksum)| {
            let bytes = fs::read(fixture_path(name)).expect("preserved fixture is tracked");
            assert_eq!(digest(&bytes), checksum, "fixture checksum drift: {name}");
            let xml = String::from_utf8(bytes).expect("preserved TEI is valid UTF-8");
            scan_header(&xml)
        })
        .collect()
}

#[test]
fn lexical_header_scan_matches_independent_receipts() {
    let scans = scanned_fixtures();
    assert_eq!(scans.len(), RECEIPTS.len());

    for (index, scan) in scans.iter().enumerate() {
        let receipt = RECEIPTS[index];
        let name = FIXTURES[index];

        assert_eq!(
            scan.authors, receipt.java_raw_authors,
            "raw author denominator: {name}"
        );
        assert_eq!(
            scan.title_stmt_titles.len(),
            receipt.title_stmt_titles,
            "titleStmt title denominator: {name}"
        );
        assert_eq!(
            scan.titles.len(),
            receipt.header_title_elements,
            "header title element denominator: {name}"
        );
        assert_eq!(
            scan.emails.len(),
            receipt.emails,
            "header email denominator: {name}"
        );
        assert_eq!(
            scan.idnos.len(),
            receipt.idno_elements,
            "header idno denominator: {name}"
        );
        assert_eq!(
            scan.pubnums.len(),
            receipt.pubnum_elements,
            "header publication-number denominator: {name}"
        );
        assert_eq!(
            scan.checksum_idnos.len(),
            receipt.checksum_idno_elements,
            "header checksum-idno denominator: {name}"
        );
        assert_eq!(
            scan.header_dates, receipt.header_date_elements,
            "header date element denominator: {name}"
        );
        assert_eq!(
            scan.document_dates, receipt.document_dates,
            "document date denominator: {name}"
        );

        // Conservation inside the idno lane: publication numbers plus the
        // GROBID-generated checksum must rebuild the raw element count.
        assert_eq!(
            scan.pubnums.len() + scan.checksum_idnos.len(),
            scan.idnos.len(),
            "idno lane must conserve its own denominator: {name}"
        );
        assert!(
            scan.checksum_idnos
                .iter()
                .all(|value| value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit())),
            "checksum idno is a 32-hex digest: {name}"
        );
        assert!(
            scan.pubnums.iter().all(|value| !value.trim().is_empty()),
            "publication numbers are non-empty: {name}"
        );
        assert!(
            scan.emails.iter().all(|value| value.contains('@')),
            "email lane only carries addresses: {name}"
        );
        assert!(
            scan.emails.iter().all(|value| !value.contains('\n')),
            "email lane carries single-line values: {name}"
        );

        // The biblStruct analytic repeats the titleStmt title verbatim.  It is a
        // copy, not a second title, so the title lane denominator is one per
        // fixture and double counting is observable.
        assert_eq!(
            scan.titles.len(),
            scan.title_stmt_titles.len() + 1,
            "one analytic copy beside the titleStmt title: {name}"
        );
        assert_eq!(
            scan.titles[0], scan.title_stmt_titles[0],
            "titleStmt title comes first in reading order: {name}"
        );
        assert_eq!(
            scan.titles[1], scan.title_stmt_titles[0],
            "analytic title is a byte-identical copy, not a new title: {name}"
        );
    }

    let sum = |pick: fn(&HeaderScan) -> usize| scans.iter().map(pick).sum::<usize>();
    assert_eq!(sum(|scan| scan.authors), TOTAL_JAVA_RAW_AUTHORS);
    assert_eq!(
        RECEIPTS
            .iter()
            .map(|receipt| receipt.native_accepted_authors)
            .sum::<usize>(),
        TOTAL_NATIVE_ACCEPTED_AUTHORS
    );
    assert_eq!(
        sum(|scan| scan.title_stmt_titles.len()),
        TOTAL_TITLE_STMT_TITLES
    );
    assert_eq!(sum(|scan| scan.titles.len()), TOTAL_HEADER_TITLE_ELEMENTS);
    assert_eq!(sum(|scan| scan.emails.len()), TOTAL_EMAILS);
    assert_eq!(sum(|scan| scan.idnos.len()), TOTAL_IDNO_ELEMENTS);
    assert_eq!(sum(|scan| scan.pubnums.len()), TOTAL_PUBNUM_ELEMENTS);
    assert_eq!(
        sum(|scan| scan.checksum_idnos.len()),
        TOTAL_CHECKSUM_IDNO_ELEMENTS
    );
    assert_eq!(sum(|scan| scan.header_dates), TOTAL_HEADER_DATE_ELEMENTS);
    assert_eq!(sum(|scan| scan.document_dates), TOTAL_DOCUMENT_DATES);

    // Distinct lanes never pool into one another: an address is never an
    // identifier, and the title lane never carries an address.
    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        assert!(
            scan.emails
                .iter()
                .all(|email| !scan.idnos.iter().any(|idno| idno == email)),
            "email and identifier lanes are disjoint: {name}"
        );
        assert!(
            scan.title_stmt_titles
                .iter()
                .all(|title| !scan.emails.iter().any(|email| title == email)),
            "title and email lanes are disjoint: {name}"
        );
        assert!(
            scan.emails
                .iter()
                .all(|email| email.contains('@') && !email.contains(' ')),
            "an email is one address, never a joined run: {name}"
        );
    }
    assert_ne!(TOTAL_DOCUMENT_DATES, TOTAL_NATIVE_ACCEPTED_AUTHORS);
    assert_ne!(TOTAL_JAVA_RAW_AUTHORS, TOTAL_NATIVE_ACCEPTED_AUTHORS);
    assert_ne!(TOTAL_IDNO_ELEMENTS, TOTAL_EMAILS);
    assert_ne!(TOTAL_HEADER_TITLE_ELEMENTS, TOTAL_TITLE_STMT_TITLES);
}

#[test]
fn hub_author_gap_stays_named_and_open() {
    let scans = scanned_fixtures();
    assert_eq!(scans[0].authors, HUB_JAVA_HEADER_RAW);
    assert_eq!(
        RECEIPTS[0].native_accepted_authors,
        HUB_NATIVE_ACCEPTED_HEADER
    );
    assert_ne!(
        HUB_JAVA_HEADER_RAW, HUB_NATIVE_ACCEPTED_HEADER,
        "hub 21 raw vs 20 native accepted is a named gap that must stay open"
    );
    // The gap is hub-only; the other two fixtures accept every raw author.
    for index in [1usize, 2] {
        assert_eq!(
            scans[index].authors, RECEIPTS[index].native_accepted_authors,
            "no un-named gap outside hub: {}",
            FIXTURES[index]
        );
    }
    // Conservation with the gap named: exactly one raw name is not accepted.
    assert_eq!(
        TOTAL_NATIVE_ACCEPTED_AUTHORS + (HUB_JAVA_HEADER_RAW - HUB_NATIVE_ACCEPTED_HEADER),
        TOTAL_JAVA_RAW_AUTHORS,
        "accepted + named gap must rebuild the raw denominator"
    );
    assert_ne!(
        TOTAL_JAVA_RAW_AUTHORS,
        TOTAL_IDNO_ELEMENTS.max(TOTAL_EMAILS),
        "author lane is never merged with the identifier or email lane"
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

fn row(text: &str) -> HeaderFeatureRow {
    HeaderFeatureRow {
        columns: vec![text.to_owned()],
    }
}

/// One span per field value: a repeated field (three emails) is three runs, not
/// one joined run.  That is the shape the decode runner produces.
fn push_run(
    rows: &mut Vec<HeaderFeatureRow>,
    spans: &mut Vec<HeaderFieldSpan>,
    label: &str,
    values: &[String],
) {
    for value in values {
        let start = rows.len();
        for token in value.split_whitespace() {
            rows.push(row(token));
        }
        if rows.len() > start {
            spans.push(HeaderFieldSpan {
                label: label.to_owned(),
                start,
                end: rows.len(),
            });
        }
    }
}

/// Lexically derived projection input: title, then emails, then publication
/// numbers, each as one run of its own rows.  No model is loaded, and no label
/// is guessed — the labels are the declared spellings of the header dictionary.
fn projection_input(scan: &HeaderScan) -> (Vec<HeaderFeatureRow>, Vec<HeaderFieldSpan>) {
    let mut rows = Vec::new();
    let mut spans = Vec::new();
    push_run(&mut rows, &mut spans, "<title>", &scan.title_stmt_titles);
    push_run(&mut rows, &mut spans, "<email>", &scan.emails);
    push_run(&mut rows, &mut spans, "<pubnum>", &scan.pubnums);
    (rows, spans)
}

#[test]
fn projected_header_fields_match_the_independent_scan() {
    let scans = scanned_fixtures();
    let mut titles = 0;
    let mut emails = 0;
    let mut ids = 0;

    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        let (rows, spans) = projection_input(scan);
        assert_eq!(
            rows.len(),
            scan.title_stmt_titles
                .iter()
                .chain(&scan.emails)
                .chain(&scan.pubnums)
                .map(|value| value.split_whitespace().count())
                .sum::<usize>(),
            "one row per projected token: {name}"
        );
        let fields = project_native_header_fields(&rows, &spans)
            .unwrap_or_else(|error| panic!("projection rejected {name}: {error}"));

        assert_eq!(fields.title, scan.title_stmt_titles, "title: {name}");
        assert_eq!(fields.email, scan.emails, "email: {name}");
        assert_eq!(fields.ids, scan.pubnums, "ids: {name}");

        // The projection is label-driven, so the checksum idno lane is carried
        // in the scan but never invented into `ids`.
        for checksum in &scan.checksum_idnos {
            assert!(
                !fields.ids.contains(checksum),
                "checksum idno must not be invented into ids: {name}"
            );
        }
        // Determinism: a second projection of the same input is identical.
        assert_eq!(
            project_native_header_fields(&rows, &spans).expect("projection is total on this input"),
            fields,
            "projection is deterministic: {name}"
        );

        titles += fields.title.len();
        emails += fields.email.len();
        ids += fields.ids.len();
    }

    assert_eq!(titles, TOTAL_TITLE_STMT_TITLES);
    assert_eq!(emails, TOTAL_EMAILS);
    assert_eq!(ids, TOTAL_PUBNUM_ELEMENTS);
    // The publication-number and checksum lanes have the same *size* here (one
    // each per fixture) but must never be the same values: they are separated
    // by content, not by count.
    assert_eq!(TOTAL_PUBNUM_ELEMENTS, TOTAL_CHECKSUM_IDNO_ELEMENTS);
    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        assert!(
            scan.pubnums
                .iter()
                .all(|pubnum| !scan.checksum_idnos.iter().any(|sum| sum == pubnum)),
            "publication numbers and checksum idnos are disjoint: {name}"
        );
        assert!(
            scan.pubnums.iter().any(|pubnum| !pubnum.is_empty()),
            "publication number lane is populated: {name}"
        );
    }
}

#[test]
fn projection_lane_never_invents_absent_fields() {
    // Labels outside the projected set contribute nothing.
    let rows = vec![row("Ashish"), row("Vaswani"), row("2026")];
    let spans = vec![
        HeaderFieldSpan {
            label: "<author>".to_owned(),
            start: 0,
            end: 2,
        },
        HeaderFieldSpan {
            label: "<date>".to_owned(),
            start: 2,
            end: 3,
        },
    ];
    assert_eq!(
        project_native_header_fields(&rows, &spans).expect("declared labels project"),
        NativeHeaderFields::default(),
        "labels outside the projected set must not fabricate title, email or ids"
    );

    // No rows means no candidate at all, rather than an empty-looking success.
    assert!(project_native_header_fields(&[], &spans).is_err());

    // A span that runs past the rows is refused instead of clamped.
    let out_of_bounds = [HeaderFieldSpan {
        label: "<title>".to_owned(),
        start: 0,
        end: 9,
    }];
    assert!(project_native_header_fields(&rows, &out_of_bounds).is_err());
}

#[test]
fn expected_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_header_same_n.rs");
    let forbidden = ["TeiLifter", "compose_header", "run_header_sequence"];

    // The file never imports the replaced lane at all.
    let imports: String = source
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    for name in forbidden {
        assert!(
            !imports.contains(name),
            "{name} must not be imported into the independent header same-n oracle"
        );
    }

    // The expectation region contains no SUT symbol of any kind, not even the
    // projection entry point it is compared against.
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
        "TeiLifter",
        "compose_header",
        "run_header_sequence",
        "project_native_header_fields",
        "decode_header_sequence",
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
