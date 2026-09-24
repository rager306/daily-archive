//! M511-j7wz2f / S05 / T04 — the runtime corpus shadow runner and its frozen
//! `preserved_three_sha` artifact (H18).
//!
//! This example is the *runtime* half of the S05 corpus regression.  The
//! structural, independent oracle is `crates/da-parse/tests/native_fulltext_corpus_semantics.rs`
//! (S05/T01): it is a dev-only test target and is not reachable from this crate,
//! and `da-cli` has no `roxmltree` dependency (adding one is out of scope).  What
//! this runner therefore provides is the runtime evidence the governor and H18
//! need:
//!
//! 1. it reads the same three sha256-pinned preserved TEI and **fails FATAL** on
//!    drift, absence or non-UTF-8 bytes, so the artifact can never describe
//!    anything but the pinned corpus;
//! 2. it derives the admitted band **from the real document** — through
//!    `TeiLifter::lift_tei` where the production lifter already publishes real
//!    data (header authors, citations) and through a compact in-file tag/substring
//!    scan for the remaining carriers (title / email / pubnum / affiliation key /
//!    reference texts / citation role carriers);
//! 3. it composes every document through `compose_native_fulltext` and measures
//!    the *emission bytes* with the same scan, per class:
//!    `source_population / admitted / conserved / fabricated / P / R / F1 /
//!    admission_coverage`;
//! 4. it emits `artifacts/parse_compat/native_fulltext_corpus_shadow.json` carrying
//!    `source_identity.identity_kind = preserved_three_sha`,
//!    `unique_source_sha256 = 3`, the GAR-006 population split
//!    (`reference_population` identity-only with its own `documents` count,
//!    `executed_population` with `corpus_execution`/`execution_evidence`, and
//!    `evaluated_population` with the measured `status`), three rows with
//!    three distinct sha256, the per-class counters and the corpus aggregate,
//!    plus the T03 mutation dictionary and the policy locks — and no raw TEI, no
//!    field values, no e-mail and no author names.
//!
//!    T07: the report must satisfy the GAR-006 shadow-population contract
//!    (`data/parse/acceptance_manifest.yaml` -> `population_contract`), which
//!    requires a distinguishable reference/executed/evaluated split and FORBIDS
//!    the undifferentiated legacy blocks `denominators` and `corpus`.  Every
//!    measured number is unchanged by that re-shaping: no field is re-derived,
//!    only re-homed.
//!
//! The measurement rule is the one S05/T01 froze (occurrence-net conservation:
//! `conserved = emitted_matches - source_matches` for the header classes, because
//! the composer re-inserts the source header subtrees verbatim; the reference and
//! citation bands are rebuilt from the admitted band and are not source-conserved).
//! The runner additionally binds its measurement to the frozen DATA carrier
//! `data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.corpus_semantic_regression`
//! and fails closed on any disagreement, so the artifact and the DATA block cannot
//! drift apart silently.
//!
//! Shadow/offline only.  No production route, no graph mutation, no import, no
//! promotion: `import_eligible=false`, D127 stays closed.
#![cfg(feature = "pdf-layout")]

use clap::Parser;
use da_parse::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const REPORT_SCHEMA: &str = "native-fulltext-corpus-shadow.v1";
const CORPUS_ID: &str = "preserved_three_sha";

// GAR-006 population split (T07).  The reference population is identity-only so
// its numbers cannot be read as executed; the executed population names the
// evidence that produced it; the evaluated population carries the measured
// status.  The evidence token's kind must be a member of the governor's
// declared `evidence_kinds` vocabulary
// (`scripts/governor/checks/executed_population_binding.yaml#populations`).
const REFERENCE_POPULATION_ROLE: &str = "identity_only";
const REFERENCE_POPULATION_IDENTITY: &str = "reviewed_three_document_corpus_semantic_reference";
const EXECUTION_EVIDENCE: &str = "tracked_offline_fixture:preserved_three_sha_tei";
const EVALUATED_STATUS: &str = "evaluated";
const EVALUATED_ORACLE: &str = "preserved_three_sha_semantic_oracle";
const DATA: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const PROVENANCE: &str = include_str!("../../da-parse/tests/fixtures/preserved/provenance.yaml");

/// The three sha256-pinned preserved TEI: `(document_id, fixture file, sha256)`.
const PRESERVED_TEI: [(&str, &str, &str); 3] = [
    (
        "hub_2507.19457",
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    ),
    (
        "rlm_2512.24601",
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    ),
    (
        "reca_2605.26525v1",
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    ),
];

// The class vocabulary is the production vocabulary, not a second literal copy:
// the five loss tokens come from `SemanticFieldLoss` and the four citation tokens
// from `citation_field_role_token`.
const CLASS_AUTHOR_LINE: &str = "AdmittedAuthorLine";
const CLASS_HEADER_TITLE: &str = "SemanticFieldLoss::HeaderTitle";
const CLASS_HEADER_EMAIL: &str = "SemanticFieldLoss::HeaderEmail";
const CLASS_HEADER_IDS: &str = "SemanticFieldLoss::HeaderIds";
const CLASS_AUTHOR_AFFILIATION: &str = "SemanticFieldLoss::AuthorAffiliation";
const CLASS_REFERENCE_DOMAIN: &str = NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION;
const CLASS_CITATION_DOI: &str = "CitationFieldRole::doi";
const CLASS_CITATION_ARXIV: &str = "CitationFieldRole::arxiv_id";
const CLASS_CITATION_URL: &str = "CitationFieldRole::url";
const CLASS_CITATION_ARTICLE_TITLE: &str = "CitationFieldRole::article_title";

const CLASS_ORDER: [&str; 10] = [
    CLASS_AUTHOR_LINE,
    CLASS_HEADER_TITLE,
    CLASS_HEADER_EMAIL,
    CLASS_HEADER_IDS,
    CLASS_AUTHOR_AFFILIATION,
    CLASS_REFERENCE_DOMAIN,
    CLASS_CITATION_DOI,
    CLASS_CITATION_ARXIV,
    CLASS_CITATION_URL,
    CLASS_CITATION_ARTICLE_TITLE,
];

const CORPUS_CITATION_ROLES: [(CitationFieldRole, &str); 4] = [
    (CitationFieldRole::Doi, CLASS_CITATION_DOI),
    (CitationFieldRole::ArxivId, CLASS_CITATION_ARXIV),
    (CitationFieldRole::Url, CLASS_CITATION_URL),
    (
        CitationFieldRole::ArticleTitle,
        CLASS_CITATION_ARTICLE_TITLE,
    ),
];

/// `<idno>` types that are checksum identifiers rather than bibliographic ones:
/// GROBID's header CRF calls the latter `<pubnum>`.
const CHECKSUM_IDNO_TYPES: [&str; 3] = ["MD5", "SHA1", "SHA256"];

#[derive(Parser, Debug)]
struct Args {
    /// Output path for the frozen report.  Required for the canonical run;
    /// `--self-check` is the only mode that may omit it.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Prove every fail-closed guard rejects a violating input, then exit 0
    /// without touching the artifact.  Used by the pin test as objective
    /// evidence for the guard set; it is not part of the canonical run.
    #[arg(long, default_value_t = false)]
    self_check: bool,
}

// ───────────────────────────── the repo-root reader ─────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pure checksum/encoding predicate behind [`preserved_tei_source`], so the
/// `--self-check` arm can prove drifted and non-UTF-8 bytes are rejected without
/// touching a pinned fixture.
fn verify_fixture_bytes<'a>(
    name: &str,
    bytes: &'a [u8],
    declared_sha256: &str,
) -> Result<&'a str, String> {
    if format!("{:x}", Sha256::digest(bytes)) != declared_sha256 {
        return Err(format!(
            "FATAL {name}: preserved fixture bytes drifted from the frozen checksum"
        ));
    }
    std::str::from_utf8(bytes)
        .map_err(|error| format!("FATAL {name}: preserved fixture is not UTF-8 ({error})"))
}

/// Read a preserved TEI fixture and verify its frozen checksum.  Missing,
/// unreadable, drifted or non-UTF-8 bytes are FATAL: the runner must never
/// measure anything but the pinned corpus.
fn preserved_tei_source(name: &str, declared_sha256: &str) -> String {
    let rel = format!("crates/da-parse/tests/fixtures/preserved/{name}");
    let bytes = std::fs::read(repo_root().join(&rel)).unwrap_or_else(|error| {
        panic!("FATAL {rel}: preserved fixture missing/unreadable ({error})")
    });
    verify_fixture_bytes(name, &bytes, declared_sha256)
        .unwrap_or_else(|reason| panic!("{reason}"))
        .to_string()
}

// ───────────────────────── the compact XML tag scanner ──────────────────────
//
// `da-cli` has no XML parser and adding one is out of scope, so this file carries
// a minimal, non-validating scanner: it builds the element list with attributes
// and content ranges, and reconstructs an element's normalized text with XML
// entity decoding.  It is deliberately nothing more than that — it is not a
// substitute for the `roxmltree` oracle in `native_fulltext_corpus_semantics.rs`
// (the authoritative structural oracle), it does not validate well-formedness,
// and it is never used on untrusted input.  Its only job is to let the runtime
// runner read the same carriers the oracle reads.

struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
    content_start: usize,
    end: usize,
    parent: Option<usize>,
}

struct XmlScan {
    elems: Vec<Elem>,
    source: String,
}

/// Byte offset of the `>` that closes the tag opened at `start`, skipping `>`
/// characters inside quoted attribute values.
fn find_tag_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let byte = bytes[i];
        match quote {
            Some(q) => {
                if byte == q {
                    quote = None;
                }
            }
            None => {
                if byte == b'"' || byte == b'\'' {
                    quote = Some(byte);
                } else if byte == b'>' {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

fn find_after(source: &str, start: usize, marker: &str) -> usize {
    source[start..]
        .find(marker)
        .map(|offset| start + offset + marker.len())
        .unwrap_or(source.len())
}

fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.'
}

fn tag_name(inner: &str) -> &str {
    let start = inner
        .find(|c: char| !c.is_whitespace() && c != '/')
        .unwrap_or(inner.len());
    let rest = &inner[start..];
    let end = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
    &rest[..end]
}

/// Decode the five predefined XML entities plus numeric character references.
/// The composer re-escapes every emitted value (`esc`), so both sides of the
/// measurement must decode to compare; this is the same projection `roxmltree`
/// applies.
fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let rest = &input[index + 1..];
        let Some(semi) = rest.find(';') else {
            out.push(ch);
            continue;
        };
        if semi > 12 {
            out.push(ch);
            continue;
        }
        let entity = &rest[..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(decoded) => {
                out.push(decoded);
                for _ in 0..=semi {
                    chars.next();
                }
            }
            None => out.push(ch),
        }
    }
    out
}

fn parse_attrs(inner: &str) -> Vec<(String, String)> {
    let bytes = inner.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = inner[name_start..i].to_string();
        if name.is_empty() {
            i += 1;
            continue;
        }
        i = skip_whitespace(bytes, i);
        if i >= bytes.len() || bytes[i] != b'=' {
            attrs.push((name, String::new()));
            continue;
        }
        i = skip_whitespace(bytes, i + 1);
        if i >= bytes.len() {
            attrs.push((name, String::new()));
            break;
        }
        let quote = bytes[i];
        if quote == b'"' || quote == b'\'' {
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            attrs.push((name, decode_entities(&inner[value_start..i])));
            if i < bytes.len() {
                i += 1;
            }
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            attrs.push((name, decode_entities(&inner[value_start..i])));
        }
    }
    attrs
}

fn scan_xml(source: &str) -> XmlScan {
    let bytes = source.as_bytes();
    let mut elems: Vec<Elem> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &source[i..];
        if rest.starts_with("<!--") {
            i = find_after(source, i, "-->");
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = find_after(source, i, "]]>");
            continue;
        }
        if rest.starts_with("<?") {
            i = find_after(source, i, "?>");
            continue;
        }
        if rest.starts_with("<!") {
            i = find_after(source, i, ">");
            continue;
        }
        let Some(gt) = find_tag_end(source, i) else {
            break;
        };
        if rest.starts_with("</") {
            let name = tag_name(&source[i + 2..gt]).to_string();
            while let Some(top) = stack.pop() {
                elems[top].end = i;
                if elems[top].name == name {
                    break;
                }
            }
            i = gt + 1;
            continue;
        }
        let inner = &source[i + 1..gt];
        let self_closing = inner.trim_end().ends_with('/');
        let name = tag_name(inner).to_string();
        let attrs = parse_attrs(inner);
        let parent = stack.last().copied();
        let index = elems.len();
        elems.push(Elem {
            name,
            attrs,
            content_start: gt + 1,
            end: gt + 1,
            parent,
        });
        if self_closing {
            elems[index].end = gt + 1;
        } else {
            stack.push(index);
        }
        i = gt + 1;
    }
    for top in stack {
        elems[top].end = source.len();
    }
    XmlScan {
        elems,
        source: source.to_string(),
    }
}

impl XmlScan {
    /// The unique element named `name` anywhere in the document.
    fn unique(&self, name: &str) -> usize {
        let mut found = self
            .elems
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name == name);
        let (index, _) = found
            .next()
            .unwrap_or_else(|| panic!("the document declares no <{name}> element"));
        assert!(
            found.next().is_none(),
            "the scanner requires a unique <{name}> element"
        );
        index
    }

    fn is_descendant(&self, node: usize, root: usize) -> bool {
        let mut current = self.elems[node].parent;
        while let Some(parent) = current {
            if parent == root {
                return true;
            }
            current = self.elems[parent].parent;
        }
        false
    }

    fn descendants(&self, root: usize, name: &str) -> Vec<usize> {
        (0..self.elems.len())
            .filter(|&index| self.elems[index].name == name && self.is_descendant(index, root))
            .collect()
    }

    fn children(&self, root: usize, name: &str) -> Vec<usize> {
        (0..self.elems.len())
            .filter(|&index| {
                self.elems[index].name == name && self.elems[index].parent == Some(root)
            })
            .collect()
    }

    fn attr(&self, index: usize, name: &str) -> Option<&str> {
        self.elems[index]
            .attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Normalized, entity-decoded text of an element: tags become a space (the
    /// join `roxmltree`'s text-descendant join performs) and whitespace runs
    /// collapse.
    fn text(&self, index: usize) -> String {
        let raw = &self.source[self.elems[index].content_start..self.elems[index].end];
        let mut stripped = String::with_capacity(raw.len());
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                if let Some(gt) = find_tag_end(raw, i) {
                    stripped.push(' ');
                    i = gt + 1;
                    continue;
                }
                i += 1;
                continue;
            }
            let ch = raw[i..].chars().next().expect("in-bounds char");
            stripped.push(ch);
            i += ch.len_utf8();
        }
        decode_entities(&stripped)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn text_list(&self, indices: &[usize]) -> Vec<String> {
        indices.iter().map(|&index| self.text(index)).collect()
    }
}

// ───────────────────────── the independent source measure ───────────────────

struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the scanner reads from one real preserved document: the source
/// population side of the profile.  Nothing here is produced by the composer.
struct CorpusSource {
    fixture: &'static str,
    sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    header_titles: Vec<String>,
    title_stmt_titles: Vec<String>,
    header_emails: Vec<String>,
    header_idnos: Vec<String>,
    header_pubnum_ids: Vec<String>,
    author_name_texts: Vec<String>,
    affiliation_occurrences: Vec<AffiliationOccurrence>,
    date_text: String,
    reference_texts: Vec<String>,
    citation_carriers: Vec<(CitationFieldRole, String)>,
}

fn is_checksum_idno(scan: &XmlScan, index: usize) -> bool {
    scan.attr(index, "type")
        .is_some_and(|kind| CHECKSUM_IDNO_TYPES.contains(&kind))
}

fn measure_source(
    document_id: &'static str,
    fixture: &'static str,
    declared_sha256: &str,
    xml: &str,
    provenance: &Provenance,
) -> CorpusSource {
    let provenance_fixture = provenance
        .fixtures
        .iter()
        .find(|entry| entry.id == document_id)
        .unwrap_or_else(|| panic!("FATAL {document_id}: provenance.yaml declares no such fixture"));
    assert_eq!(
        provenance_fixture.sha256, declared_sha256,
        "FATAL {document_id}: provenance.yaml declares a different sha256 for this fixture"
    );

    let scan = scan_xml(xml);
    let header = scan.unique("teiHeader");
    let list = scan.unique("listBibl");

    let title_stmt = scan
        .children(header, "fileDesc")
        .first()
        .and_then(|&desc| scan.children(desc, "titleStmt").first().copied());
    let title_stmt_titles: Vec<String> = title_stmt
        .map(|stmt| scan.text_list(&scan.descendants(stmt, "title")))
        .unwrap_or_default();
    let header_titles = scan.text_list(&scan.descendants(header, "title"));
    let header_emails = scan.text_list(&scan.descendants(header, "email"));
    let header_idnos = scan.text_list(&scan.descendants(header, "idno"));
    let header_pubnum_ids: Vec<String> = scan
        .descendants(header, "idno")
        .into_iter()
        .filter(|&index| !is_checksum_idno(&scan, index))
        .map(|index| scan.text(index))
        .collect();

    let author_elements = scan.descendants(header, "author");
    let author_name_texts: Vec<String> = author_elements
        .iter()
        .map(|&author| {
            scan.children(author, "persName")
                .first()
                .map(|&pers_name| scan.text(pers_name))
                .unwrap_or_default()
        })
        .collect();

    let mut affiliation_occurrences = Vec::new();
    for (author_index, &author) in author_elements.iter().enumerate() {
        for affiliation in scan.children(author, "affiliation") {
            let Some(key) = scan.attr(affiliation, "key") else {
                continue;
            };
            affiliation_occurrences.push(AffiliationOccurrence {
                author_index,
                key: key.to_string(),
                text: scan.text(affiliation),
            });
        }
    }

    let date_text = scan
        .descendants(header, "date")
        .into_iter()
        .map(|index| scan.text(index))
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| panic!("{fixture}: the header declares no dated element with text"));

    let reference_texts: Vec<String> = scan.text_list(&scan.children(list, "biblStruct"));

    let mut citation_carriers = Vec::new();
    for index in scan.descendants(list, "idno") {
        match scan.attr(index, "type") {
            Some("DOI") => citation_carriers.push((CitationFieldRole::Doi, scan.text(index))),
            Some("arXiv") => citation_carriers.push((CitationFieldRole::ArxivId, scan.text(index))),
            _ => {}
        }
    }
    for index in scan.descendants(list, "ptr") {
        if let Some(target) = scan.attr(index, "target")
            && target.starts_with("http")
        {
            citation_carriers.push((CitationFieldRole::Url, target.to_string()));
        }
    }
    for index in scan.descendants(list, "title") {
        if scan.attr(index, "level") == Some("a") {
            citation_carriers.push((CitationFieldRole::ArticleTitle, scan.text(index)));
        }
    }

    CorpusSource {
        fixture,
        sha256: declared_sha256.to_string(),
        profile_fingerprint: format!("profile={}", provenance.assumed_grobid_profile),
        config_fingerprint: provenance.configuration_fingerprint.clone(),
        header_titles,
        title_stmt_titles,
        header_emails,
        header_idnos,
        header_pubnum_ids,
        author_name_texts,
        affiliation_occurrences,
        date_text,
        reference_texts,
        citation_carriers,
    }
}

// ───────────────────────────── the admitted band ────────────────────────────

struct AdmissionGap {
    class: &'static str,
    source_population: usize,
    admitted: usize,
    reason: String,
}

/// The typed composition input plus the admitted-side values the measurement
/// compares the emission against.  All of it is derived from the real document.
struct AdmittedBand {
    header: HeaderCompositionOutcome,
    references: ReferenceCompositionOutcome,
    titles: Vec<String>,
    emails: Vec<String>,
    ids: Vec<String>,
    author_lines: Vec<(String, String)>,
    affiliation_bindings: Vec<(String, String)>,
    reference_texts: Vec<String>,
    citation_fields: Vec<(CitationFieldRole, String)>,
    gaps: Vec<AdmissionGap>,
}

/// The serde wire tag and reason of a rejected parser outcome, e.g.
/// `ambiguous:single_token`.
fn outcome_token<T: serde::Serialize>(outcome: &T) -> String {
    let value = serde_json::to_value(outcome).expect("a parser outcome serializes");
    let tag = value
        .get("outcome")
        .and_then(|tag| tag.as_str())
        .unwrap_or("unknown")
        .to_string();
    match value.get("reason").and_then(|reason| reason.as_str()) {
        Some(reason) => format!("{tag}:{reason}"),
        None => tag,
    }
}

/// Assemble the admitted band from the real document through the public typed
/// parsers.  Every parser rejection becomes a named gap; nothing is silently
/// dropped and no value is invented.
fn build_admitted_band(source: &CorpusSource) -> AdmittedBand {
    let mut gaps = Vec::new();

    let date = match NativeDateParser::new().parse(&source.date_text) {
        NativeDateOutcome::Recognized { date } => date,
        other => panic!(
            "FATAL {}: the real publication date form cannot be admitted ({})",
            source.fixture,
            outcome_token(&other)
        ),
    };

    let titles = source.title_stmt_titles.clone();
    if titles.len() < source.header_titles.len() {
        gaps.push(AdmissionGap {
            class: CLASS_HEADER_TITLE,
            source_population: source.header_titles.len(),
            admitted: titles.len(),
            reason: format!(
                "band definition: the admitted title field is the direct fileDesc/titleStmt/title \
                 children; the remaining {} of {} header <title> carriers are conserved by the \
                 source subtree and are not produced by the admitted band",
                source.header_titles.len() - titles.len(),
                source.header_titles.len()
            ),
        });
    }

    let emails = source.header_emails.clone();
    let ids = source.header_pubnum_ids.clone();

    let affiliation_parser = NativeAffiliationParser::new();
    let mut components: Vec<Option<Vec<RecognizedAffiliationComponent>>> = Vec::new();
    let mut occurrence_component: Vec<Option<usize>> = Vec::new();
    let mut rejected_affiliations = Vec::new();
    for occurrence in &source.affiliation_occurrences {
        match affiliation_parser.parse(&occurrence.text) {
            NativeAffiliationOutcome::Recognized {
                components: recognized,
            } => {
                assert!(
                    !recognized.is_empty(),
                    "{}: a recognized affiliation must carry at least one component",
                    source.fixture
                );
                occurrence_component.push(Some(components.len()));
                components.push(Some(recognized));
            }
            other => {
                occurrence_component.push(None);
                rejected_affiliations.push(outcome_token(&other));
            }
        }
    }

    let name_parser = NativePersonalNameParser::new();
    let mut authors: Vec<HeaderAuthor> = Vec::new();
    let mut author_lines: Vec<(String, String)> = Vec::new();
    let mut affiliation_bindings: Vec<(String, String)> = Vec::new();
    let mut rejected_names = Vec::new();

    for (author_index, name_text) in source.author_name_texts.iter().enumerate() {
        let name = match name_parser.parse(PersonalNameContext::Header, name_text) {
            NativePersonalNameOutcome::Recognized { name } => name,
            other => {
                rejected_names.push(outcome_token(&other));
                continue;
            }
        };
        let ordinal = authors.len();
        let owner = format!("author-{ordinal}");
        let mut links = Vec::new();
        for (occurrence_index, occurrence) in source.affiliation_occurrences.iter().enumerate() {
            if occurrence.author_index != author_index {
                continue;
            }
            let Some(Some(recognized)) = occurrence_component
                .get(occurrence_index)
                .and_then(|index| index.and_then(|index| components.get(index)))
            else {
                continue;
            };
            let span = recognized
                .first()
                .expect("a recognized affiliation carries a component")
                .span;
            links.push(NativeAffiliationLink {
                key: occurrence.key.clone(),
                affiliation_ordinal: occurrence_index,
                affiliation_span: span,
            });
        }
        author_lines.push((owner.clone(), name.surname.clone()));
        affiliation_bindings.extend(links.iter().map(|link| (owner.clone(), link.key.clone())));
        authors.push(HeaderAuthor {
            ordinal,
            span: CompositionSpan {
                start: ordinal,
                end: ordinal + 1,
            },
            name,
            affiliations: links,
        });
    }

    assert!(
        !authors.is_empty(),
        "FATAL {}: the real corpus admitted no author line",
        source.fixture
    );
    for reason in &rejected_names {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_LINE,
            source_population: source.author_name_texts.len(),
            admitted: authors.len(),
            reason: format!(
                "NativePersonalNameParser rejected the real <persName> text ({reason})"
            ),
        });
    }
    for reason in &rejected_affiliations {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_AFFILIATION,
            source_population: source.affiliation_occurrences.len(),
            admitted: affiliation_bindings.len(),
            reason: format!(
                "NativeAffiliationParser rejected the real <affiliation key> carrier text ({reason})"
            ),
        });
    }

    let references: Vec<NativeReference> = source
        .reference_texts
        .iter()
        .enumerate()
        .map(|(ordinal, text)| NativeReference {
            ordinal,
            source: text.clone(),
            span: CitationSpan {
                start: 0,
                end: text.len(),
            },
            provenance: "preserved-tei-corpus-shadow".into(),
        })
        .collect();
    assert!(
        references
            .iter()
            .all(|reference| reference.span.start < reference.span.end),
        "FATAL {}: a source biblStruct carries empty text",
        source.fixture
    );

    let citation_fields: Vec<CitationField> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| CitationField {
            role: *role,
            value: value.clone(),
            span: CitationSpan {
                start: 0,
                end: value.len(),
            },
        })
        .collect();
    assert!(
        citation_fields
            .iter()
            .all(|field| field.span.start < field.span.end),
        "FATAL {}: a citation carrier carries an empty value",
        source.fixture
    );

    let header = HeaderCompositionOutcome::Accepted {
        composition: HeaderCompositionCandidate {
            candidate: HeaderCandidate {
                date,
                authors,
                fields: NativeHeaderFields {
                    title: titles.clone(),
                    email: emails.clone(),
                    ids: ids.clone(),
                },
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::Date,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Names,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Affiliations,
                    disposition: ComponentDisposition::Accepted,
                },
            ],
        },
    };
    let references_outcome = ReferenceCompositionOutcome::Accepted {
        composition: ReferenceCompositionCandidate {
            candidate: ReferenceCandidate {
                references,
                fields: citation_fields.clone(),
                anchors: Vec::new(),
                adjacent_pairs: Vec::new(),
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::References,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::CitationFields,
                    disposition: if citation_fields.is_empty() {
                        ComponentDisposition::Empty
                    } else {
                        ComponentDisposition::Accepted
                    },
                },
                ComponentReceipt {
                    component: CompositionComponent::Anchors,
                    disposition: ComponentDisposition::Empty,
                },
            ],
        },
    };

    assert!(
        affiliation_bindings.len() <= source.affiliation_occurrences.len(),
        "{}: the admitted band cannot carry more bindings than the source declares",
        source.fixture
    );

    AdmittedBand {
        header,
        references: references_outcome,
        titles,
        emails,
        ids,
        author_lines,
        affiliation_bindings,
        reference_texts: source.reference_texts.clone(),
        citation_fields: source.citation_carriers.clone(),
        gaps,
    }
}

// ──────────────────────────── the profile counters ──────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassProfile {
    class: &'static str,
    source_population: usize,
    admitted: usize,
    conserved: usize,
    lost: usize,
    fabricated: usize,
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}

impl ClassProfile {
    fn admission_coverage(&self) -> f64 {
        ratio(self.admitted, self.source_population)
    }
    fn precision(&self) -> f64 {
        ratio(self.conserved, self.conserved + self.fabricated)
    }
    fn recall(&self) -> f64 {
        ratio(self.conserved, self.conserved + self.lost)
    }
    fn f1(&self) -> f64 {
        let (precision, recall) = (self.precision(), self.recall());
        if precision + recall == 0.0 {
            1.0
        } else {
            2.0 * precision * recall / (precision + recall)
        }
    }
}

/// Occurrence-counted conservation.  `conserved` is `admitted` net of the
/// occurrences the conserved source subtrees already contribute; `fabricated`
/// counts emitted carriers matching neither an admitted nor a source value.
fn class_profile(
    fixture: &str,
    class: &'static str,
    source_population: usize,
    admitted: &[String],
    emitted: &[String],
    source_carriers: &[String],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted
        .iter()
        .filter(|carrier| admitted.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted
        .iter()
        .filter(|carrier| {
            !admitted.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    let _ = fixture;
    ClassProfile {
        class,
        source_population,
        admitted: admitted.len(),
        conserved,
        lost: admitted.len() - conserved,
        fabricated,
    }
}

/// Occurrence-counted conservation of binding-valued carriers (an owner element
/// plus a value).
fn binding_profile(
    class: &'static str,
    source_population: usize,
    admitted: &[(String, String)],
    emitted: &[(String, String)],
    source_carriers: &[(String, String)],
) -> ClassProfile {
    let emitted_match = emitted
        .iter()
        .filter(|carrier| admitted.iter().any(|value| value == *carrier))
        .count();
    let conserved = admitted.len().min(emitted_match);
    let fabricated = emitted
        .iter()
        .filter(|carrier| {
            !admitted.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        class,
        source_population,
        admitted: admitted.len(),
        conserved,
        lost: admitted.len() - conserved,
        fabricated,
    }
}

/// The emitted header's author-line carriers: `author-{ordinal}` → surname.
/// Only a synthesized author carries an `xml:id`, which is exactly the
/// distinction between a band carrier and a conserved source carrier.
fn emitted_author_lines(scan: &XmlScan) -> Vec<(String, String)> {
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .filter_map(|author| {
            let id = scan.attr(author, "xml:id")?;
            let surname = scan
                .children(author, "surname")
                .first()
                .map(|&index| scan.text(index))
                .unwrap_or_default();
            Some((id.to_string(), surname))
        })
        .collect()
}

fn source_author_lines(xml: &str) -> Vec<(String, String)> {
    let scan = scan_xml(xml);
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .map(|author| {
            let surname = scan
                .children(author, "surname")
                .first()
                .map(|&index| scan.text(index))
                .unwrap_or_default();
            (String::new(), surname)
        })
        .collect()
}

fn emitted_affiliation_bindings(scan: &XmlScan) -> Vec<(String, String)> {
    let header = scan.unique("teiHeader");
    let mut bindings = Vec::new();
    for author in scan.descendants(header, "author") {
        let Some(owner) = scan.attr(author, "xml:id") else {
            continue;
        };
        for affiliation in scan.descendants(author, "affiliation") {
            if let Some(key) = scan.attr(affiliation, "key") {
                bindings.push((owner.to_string(), key.to_string()));
            }
        }
    }
    bindings
}

fn source_affiliation_bindings(xml: &str) -> Vec<(String, String)> {
    let scan = scan_xml(xml);
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .flat_map(|author| {
            scan.descendants(author, "affiliation")
                .into_iter()
                .filter_map(|index| {
                    scan.attr(index, "key")
                        .map(|key| (String::new(), key.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The emitted `listBibl` carriers of the measured citation roles.
fn emitted_citation_carriers(scan: &XmlScan) -> Vec<(String, String)> {
    let list = scan.unique("listBibl");
    scan.descendants(list, "note")
        .into_iter()
        .filter_map(|index| {
            let role = scan.attr(index, "role")?;
            let known = CORPUS_CITATION_ROLES
                .iter()
                .any(|(candidate, _)| citation_field_role_token(*candidate) == role);
            known.then(|| (role.to_string(), scan.text(index)))
        })
        .collect()
}

/// Every class's measured profile for one document, from the emission bytes.
fn oracle_class_profiles(
    source: &CorpusSource,
    source_xml: &str,
    admitted: &AdmittedBand,
    emission_xml: &str,
) -> Vec<ClassProfile> {
    let scan = scan_xml(emission_xml);
    let header = scan.unique("teiHeader");
    let list = scan.unique("listBibl");

    let emitted_titles = scan.text_list(&scan.descendants(header, "title"));
    let emitted_emails = scan.text_list(&scan.descendants(header, "email"));
    let emitted_ids = scan.text_list(&scan.descendants(header, "idno"));
    let emitted_reference_texts = scan.text_list(&scan.children(list, "biblStruct"));

    let emitted_lines = emitted_author_lines(&scan);
    let emitted_affiliations = emitted_affiliation_bindings(&scan);
    let source_author_carriers = source_author_lines(source_xml);
    let source_affiliation_carriers = source_affiliation_bindings(source_xml);

    let emitted_citations = emitted_citation_carriers(&scan);
    let source_citations: Vec<(String, String)> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();
    let admitted_citations: Vec<(String, String)> = admitted
        .citation_fields
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();

    let mut profiles = Vec::with_capacity(CLASS_ORDER.len());
    profiles.push(binding_profile(
        CLASS_AUTHOR_LINE,
        source.author_name_texts.len(),
        &admitted.author_lines,
        &emitted_lines,
        &source_author_carriers,
    ));
    profiles.push(class_profile(
        source.fixture,
        CLASS_HEADER_TITLE,
        source.header_titles.len(),
        &admitted.titles,
        &emitted_titles,
        &source.header_titles,
        true,
    ));
    profiles.push(class_profile(
        source.fixture,
        CLASS_HEADER_EMAIL,
        source.header_emails.len(),
        &admitted.emails,
        &emitted_emails,
        &source.header_emails,
        true,
    ));
    profiles.push(class_profile(
        source.fixture,
        CLASS_HEADER_IDS,
        source.header_pubnum_ids.len(),
        &admitted.ids,
        &emitted_ids,
        &source.header_idnos,
        true,
    ));
    profiles.push(binding_profile(
        CLASS_AUTHOR_AFFILIATION,
        source.affiliation_occurrences.len(),
        &admitted.affiliation_bindings,
        &emitted_affiliations,
        &source_affiliation_carriers,
    ));
    profiles.push(class_profile(
        source.fixture,
        CLASS_REFERENCE_DOMAIN,
        source.reference_texts.len(),
        &admitted.reference_texts,
        &emitted_reference_texts,
        &source.reference_texts,
        false,
    ));
    for (role, class) in CORPUS_CITATION_ROLES {
        let token = citation_field_role_token(role);
        let admitted_values: Vec<String> = admitted_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        let source_values: Vec<String> = source_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        let emitted_values: Vec<String> = emitted_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        profiles.push(class_profile(
            source.fixture,
            class,
            source_values.len(),
            &admitted_values,
            &emitted_values,
            &source_values,
            false,
        ));
    }
    assert_eq!(
        profiles.len(),
        CLASS_ORDER.len(),
        "every declared class must be measured"
    );
    profiles
}

// ─────────────────────────── the composition harness ────────────────────────

fn compose_corpus_document(
    source: &CorpusSource,
    admitted: &AdmittedBand,
    xml: &str,
) -> NativeFulltextCompositionOutcome {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, source.profile_fingerprint.clone()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: source.profile_fingerprint.clone(),
            config_fingerprint: source.config_fingerprint.clone(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: corpus_admissions(&source.sha256),
        s17_lineage: None,
    };
    compose_native_fulltext(input)
}

fn corpus_admissions(source_sha256: &str) -> Vec<FulltextAdmissionReceipt> {
    let receipt = |component: FulltextComponent, disposition: AdmissionDisposition| {
        FulltextAdmissionReceipt {
            component,
            disposition,
            producer: "preserved-tei-corpus-shadow".into(),
            evidence_state: if disposition == AdmissionDisposition::Passed {
                "passing"
            } else {
                "unsupported"
            }
            .into(),
            schema_version: "native-composition.outcome/v1".into(),
            source_sha256: source_sha256.to_string(),
            artifact_sha256: String::new(),
            fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
        }
    };
    vec![
        receipt(FulltextComponent::Header, AdmissionDisposition::Passed),
        receipt(FulltextComponent::References, AdmissionDisposition::Passed),
        receipt(
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::InvalidEvidence,
        ),
        receipt(FulltextComponent::Figure, AdmissionDisposition::Unsupported),
        receipt(FulltextComponent::Table, AdmissionDisposition::NotObserved),
        receipt(
            FulltextComponent::Formula,
            AdmissionDisposition::Unsupported,
        ),
        receipt(
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
        ),
    ]
}

// ────────────────────────────── the DATA carrier ────────────────────────────

#[derive(Deserialize)]
struct ContractFile {
    native_fulltext_composition_shadow: ShadowBlock,
}

#[derive(Deserialize)]
struct ShadowBlock {
    corpus_semantic_regression: Regression,
}

#[derive(Deserialize)]
struct Regression {
    identity_kind: String,
    unique_source_sha256: usize,
    fallback_target: String,
    class_order: Vec<String>,
    classes: BTreeMap<String, DataClass>,
    aggregate: DataAggregate,
    denominators: DataDenominators,
    documents: Vec<DataDocument>,
    mutation_matrix: Vec<DataMutation>,
    refusal_classes: Vec<String>,
    locks: DataLocks,
}

#[derive(Deserialize)]
struct DataClass {
    source_population: usize,
    admitted: usize,
    conserved: usize,
    fabricated: usize,
}

#[derive(Deserialize)]
struct DataAggregate {
    tp: usize,
    fp: usize,
    #[serde(rename = "fn")]
    fn_count: usize,
}

#[derive(Deserialize)]
struct DataDenominators {
    documents: usize,
    unique_source_sha256: usize,
}

#[derive(Deserialize)]
struct DataDocument {
    id: String,
    sha256: String,
}

#[derive(Deserialize)]
struct DataMutation {
    id: String,
    detection: String,
    refusal_class: Option<String>,
    metric: Option<String>,
    expectation: Option<String>,
}

#[derive(Deserialize)]
struct DataLocks {
    import_eligible: bool,
    production_route: String,
    graph_write: String,
    auto_promote: String,
}

#[derive(Deserialize)]
struct Provenance {
    assumed_grobid_profile: String,
    configuration_fingerprint: String,
    fixtures: Vec<ProvenanceFixture>,
}

#[derive(Deserialize)]
struct ProvenanceFixture {
    id: String,
    sha256: String,
}

fn load_contract() -> Regression {
    let file: ContractFile = serde_yaml::from_str(DATA)
        .unwrap_or_else(|error| panic!("FATAL data/parse/comparison_contract.yaml: {error}"));
    file.native_fulltext_composition_shadow
        .corpus_semantic_regression
}

fn load_provenance() -> Provenance {
    serde_yaml::from_str(PROVENANCE)
        .unwrap_or_else(|error| panic!("FATAL provenance.yaml: {error}"))
}

fn mutation_detector(mutation: &DataMutation) -> String {
    if let Some(class) = &mutation.refusal_class {
        return format!("{}:{class}", mutation.detection);
    }
    match (&mutation.metric, &mutation.expectation) {
        (Some(metric), Some(expectation)) => {
            format!("{}:{metric}:{expectation}", mutation.detection)
        }
        (Some(metric), None) => format!("{}:{metric}", mutation.detection),
        (None, Some(expectation)) => format!("{}:{expectation}", mutation.detection),
        (None, None) => mutation.detection.clone(),
    }
}

// ────────────────────────────── the report shape ────────────────────────────

#[derive(Serialize)]
struct SourceIdentity {
    identity_kind: String,
    unique_source_sha256: usize,
    source_sha256: Vec<String>,
    profile_fingerprint: String,
    config_fingerprint: String,
    fallback_target: String,
}

#[derive(Serialize)]
struct DenominatorRow {
    source_population: usize,
    admitted: usize,
}

/// GAR-006 reference population: identity-only.  It names the referenced corpus
/// and carries its own document count plus the per-class *source* population, so
/// a referenced count cannot be consumed as an executed one.
#[derive(Serialize)]
struct ReferencePopulation {
    role: &'static str,
    identity: &'static str,
    documents: usize,
    unique_source_sha256: usize,
    per_class: BTreeMap<String, DenominatorRow>,
}

#[derive(Serialize)]
struct ClassRow {
    class: String,
    source_population: usize,
    admitted: usize,
    conserved: usize,
    fabricated: usize,
    precision: f64,
    recall: f64,
    f1: f64,
    admission_coverage: f64,
}

#[derive(Serialize)]
struct Outcome {
    document_id: String,
    source_sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    disposition: String,
    lift: LiftCounters,
    classes: Vec<ClassRow>,
}

#[derive(Serialize)]
struct LiftCounters {
    header_authors: usize,
    citations: usize,
}

#[derive(Serialize)]
struct Aggregate {
    tp: usize,
    fp: usize,
    #[serde(rename = "fn")]
    fn_count: usize,
    precision: f64,
    recall: f64,
    f1: f64,
    admission_coverage: f64,
}

/// GAR-006 executed population: what actually ran.  `corpus_execution` is
/// literally true because the runner composes the three sha256-pinned preserved
/// TEI, and `execution_evidence` is the declared evidence kind that produced it.
#[derive(Serialize)]
struct ExecutedPopulation {
    corpus_execution: bool,
    execution_evidence: &'static str,
    documents: usize,
    accepted: usize,
}

/// GAR-006 evaluated population: the measured quality claim, kept separate from
/// what was executed so a corpus-scale number cannot be read as an outcome of a
/// smaller run (and vice versa).
#[derive(Serialize)]
struct EvaluatedPopulation {
    status: &'static str,
    oracle: &'static str,
    documents: usize,
    aggregate: Aggregate,
    per_class: BTreeMap<String, Aggregate>,
}

#[derive(Serialize)]
struct PolicyLocks {
    observation_mode: &'static str,
    promotion_state: &'static str,
    import_eligible: bool,
    production_route: &'static str,
    graph_mutation: &'static str,
    auto_promote: &'static str,
    deployed: bool,
    pooled_winner: &'static str,
}

#[derive(Serialize)]
struct Redaction {
    raw_tei: bool,
    raw_pdf: bool,
    raw_text: bool,
    field_values: bool,
    names: bool,
    email_addresses: bool,
    captions: bool,
    references: bool,
    physical_paths: bool,
    hostname: bool,
    executable: bool,
    secrets: bool,
    pdf_bytes: bool,
    graph_payload: bool,
    fulltext: bool,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    corpus_id: &'static str,
    source_identity: SourceIdentity,
    reference_population: ReferencePopulation,
    executed_population: ExecutedPopulation,
    evaluated_population: EvaluatedPopulation,
    outcomes: Vec<Outcome>,
    mutation_summary: BTreeMap<String, String>,
    policy: PolicyLocks,
    redaction: Redaction,
}

// ─────────────────────────────── the guards ─────────────────────────────────

/// Every guard is a pure predicate over the assembled measurement so the
/// `--self-check` arm can prove it rejects a violating input without touching a
/// fixture.
fn guard_identity(shas: &BTreeSet<String>, expected: usize) -> Result<(), String> {
    if shas.len() != expected {
        return Err(format!(
            "unique_source_sha256 must be {expected}, measured {}",
            shas.len()
        ));
    }
    Ok(())
}

fn guard_documents(rows: &[Outcome], expected: usize) -> Result<(), String> {
    if rows.len() != expected {
        return Err(format!(
            "reference_population.documents must be {expected}, measured {}",
            rows.len()
        ));
    }
    let accepted = rows
        .iter()
        .filter(|row| row.disposition == "accepted")
        .count();
    if accepted < expected {
        return Err(format!(
            "accepted rows must be {expected}, measured {accepted} (a typed refusal is not an accepted row)"
        ));
    }
    Ok(())
}

fn guard_denominators(per_class: &BTreeMap<String, ClassProfile>) -> Result<(), String> {
    for (class, profile) in per_class {
        if profile.source_population == 0 && profile.admitted == 0 {
            return Err(format!(
                "{class}: the corpus-wide denominator is zero, so the class proves nothing"
            ));
        }
    }
    Ok(())
}

fn guard_scope(class_order: &[&str], observed: &[String]) -> Result<(), String> {
    let declared: BTreeSet<&str> = class_order.iter().copied().collect();
    let seen: BTreeSet<&str> = observed.iter().map(String::as_str).collect();
    if declared != seen {
        return Err(format!(
            "unknown scope: declared {declared:?}, observed {seen:?}"
        ));
    }
    Ok(())
}

fn guard_aggregate(
    recorded: &Aggregate,
    profiles: &BTreeMap<String, ClassProfile>,
) -> Result<(), String> {
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_count = 0usize;
    let mut admitted = 0usize;
    let mut population = 0usize;
    for profile in profiles.values() {
        tp += profile.conserved;
        fp += profile.fabricated;
        fn_count += profile.lost;
        admitted += profile.admitted;
        population += profile.source_population;
    }
    let expected = Aggregate {
        tp,
        fp,
        fn_count,
        precision: ratio(tp, tp + fp),
        recall: ratio(tp, tp + fn_count),
        f1: {
            let (precision, recall) = (ratio(tp, tp + fp), ratio(tp, tp + fn_count));
            if precision + recall == 0.0 {
                1.0
            } else {
                2.0 * precision * recall / (precision + recall)
            }
        },
        admission_coverage: ratio(admitted, population),
    };
    if recorded.tp != expected.tp
        || recorded.fp != expected.fp
        || recorded.fn_count != expected.fn_count
        || recorded.precision != expected.precision
        || recorded.recall != expected.recall
        || recorded.f1 != expected.f1
        || recorded.admission_coverage != expected.admission_coverage
    {
        return Err(format!(
            "aggregate metric disagreement: recorded TP={} FP={} FN={} P={} R={} F1={} coverage={}, re-derived TP={} FP={} FN={} P={} R={} F1={} coverage={}",
            recorded.tp,
            recorded.fp,
            recorded.fn_count,
            recorded.precision,
            recorded.recall,
            recorded.f1,
            recorded.admission_coverage,
            expected.tp,
            expected.fp,
            expected.fn_count,
            expected.precision,
            expected.recall,
            expected.f1,
            expected.admission_coverage,
        ));
    }
    Ok(())
}

/// The runtime measurement must agree with the frozen DATA carrier: a divergence
/// is a fail-closed error, never a silently divergent artifact.
fn guard_data_agreement(
    profiles: &BTreeMap<String, ClassProfile>,
    recorded: &Aggregate,
    contract: &Regression,
) -> Result<(), String> {
    if contract.identity_kind != CORPUS_ID {
        return Err(format!(
            "DATA identity_kind `{}` is not `{CORPUS_ID}`",
            contract.identity_kind
        ));
    }
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_count = 0usize;
    for class in CLASS_ORDER {
        let Some(measured) = profiles.get(class) else {
            return Err(format!(
                "DATA declares class `{class}` but nothing was measured"
            ));
        };
        let Some(declared) = contract.classes.get(class) else {
            return Err(format!("measured class `{class}` is absent from DATA"));
        };
        if measured.source_population != declared.source_population
            || measured.admitted != declared.admitted
            || measured.conserved != declared.conserved
            || measured.fabricated != declared.fabricated
        {
            return Err(format!(
                "{class}: runtime (source={} admitted={} conserved={} fabricated={}) disagrees with DATA (source={} admitted={} conserved={} fabricated={})",
                measured.source_population,
                measured.admitted,
                measured.conserved,
                measured.fabricated,
                declared.source_population,
                declared.admitted,
                declared.conserved,
                declared.fabricated,
            ));
        }
        tp += measured.conserved;
        fp += measured.fabricated;
        fn_count += measured.lost;
    }
    if tp != contract.aggregate.tp
        || fp != contract.aggregate.fp
        || fn_count != contract.aggregate.fn_count
    {
        return Err(format!(
            "corpus aggregate TP/FP/FN {tp}/{fp}/{fn_count} disagrees with DATA {}/{}/{}",
            contract.aggregate.tp, contract.aggregate.fp, contract.aggregate.fn_count
        ));
    }
    if recorded.tp != tp || recorded.fp != fp || recorded.fn_count != fn_count {
        return Err("recorded aggregate disagrees with the DATA-agreed aggregate".into());
    }
    Ok(())
}

/// Prove each guard rejects a violating input.  Each case is the smallest
/// violation of the guard's contract.
fn self_check() {
    let mut cases: Vec<(&str, String)> = Vec::new();

    let mut one_sha = BTreeSet::new();
    one_sha.insert("a".repeat(64));
    match guard_identity(&one_sha, 3) {
        Ok(()) => panic!("FATAL guard_identity accepted a single sha"),
        Err(reason) => cases.push(("identity_unique_sha", reason)),
    }
    let mut three_sha = BTreeSet::new();
    for byte in ["a", "b", "c"] {
        three_sha.insert(byte.repeat(64));
    }
    guard_identity(&three_sha, 3).expect("three distinct sha is the valid case");

    let refused = Outcome {
        document_id: "d".into(),
        source_sha256: "a".repeat(64),
        profile_fingerprint: "profile=rich".into(),
        config_fingerprint: "profile=rich;".into(),
        disposition: "rejected:MalformedXml".into(),
        lift: LiftCounters {
            header_authors: 0,
            citations: 0,
        },
        classes: Vec::new(),
    };
    match guard_documents(std::slice::from_ref(&refused), 3) {
        Ok(()) => panic!("FATAL guard_documents accepted a short/refused row set"),
        Err(reason) => cases.push(("documents_and_accepted_rows", reason)),
    }

    let empty = ClassProfile {
        class: CLASS_AUTHOR_LINE,
        source_population: 0,
        admitted: 0,
        conserved: 0,
        lost: 0,
        fabricated: 0,
    };
    let mut zero = BTreeMap::new();
    zero.insert(CLASS_AUTHOR_LINE.to_string(), empty);
    match guard_denominators(&zero) {
        Ok(()) => panic!("FATAL guard_denominators accepted a zero denominator"),
        Err(reason) => cases.push(("zero_class_denominator", reason)),
    }

    match guard_scope(
        &CLASS_ORDER,
        &[CLASS_AUTHOR_LINE.to_string(), "UnknownScope".to_string()],
    ) {
        Ok(()) => panic!("FATAL guard_scope accepted an unknown class"),
        Err(reason) => cases.push(("unknown_scope", reason)),
    }

    let mut valid_profiles = BTreeMap::new();
    valid_profiles.insert(
        CLASS_AUTHOR_LINE.to_string(),
        ClassProfile {
            class: CLASS_AUTHOR_LINE,
            source_population: 2,
            admitted: 2,
            conserved: 2,
            lost: 0,
            fabricated: 0,
        },
    );
    let wrong = Aggregate {
        tp: 1,
        fp: 0,
        fn_count: 0,
        precision: 1.0,
        recall: 1.0,
        f1: 1.0,
        admission_coverage: 1.0,
    };
    match guard_aggregate(&wrong, &valid_profiles) {
        Ok(()) => panic!("FATAL guard_aggregate accepted a mismatched metric"),
        Err(reason) => cases.push(("aggregate_metric_mismatch", reason)),
    }

    let contract = load_contract();
    let mut single_class = BTreeMap::new();
    single_class.insert(
        CLASS_AUTHOR_LINE.to_string(),
        ClassProfile {
            class: CLASS_AUTHOR_LINE,
            source_population: 1,
            admitted: 1,
            conserved: 1,
            lost: 0,
            fabricated: 0,
        },
    );
    let declared = contract
        .classes
        .get(CLASS_AUTHOR_LINE)
        .expect("DATA declares the author-line class");
    let mismatched = if declared.source_population == 1 {
        2
    } else {
        1
    };
    single_class.insert(
        CLASS_AUTHOR_LINE.to_string(),
        ClassProfile {
            class: CLASS_AUTHOR_LINE,
            source_population: mismatched,
            admitted: mismatched,
            conserved: mismatched,
            lost: 0,
            fabricated: 0,
        },
    );
    let aggregate_ok = Aggregate {
        tp: mismatched,
        fp: 0,
        fn_count: 0,
        precision: 1.0,
        recall: 1.0,
        f1: 1.0,
        admission_coverage: 1.0,
    };
    match guard_data_agreement(&single_class, &aggregate_ok, &contract) {
        Ok(()) => panic!("FATAL guard_data_agreement accepted a DATA divergence"),
        Err(reason) => cases.push(("data_carrier_agreement", reason)),
    }

    // The read-time checksum guard is the one that makes the whole regression
    // corpus-bound, so it is proven against the real pinned fixture: the frozen
    // pin must still match the bytes on disk, a single flipped byte must be
    // rejected, and non-UTF-8 bytes must be rejected even with a matching digest.
    let real = std::fs::read(
        repo_root().join("crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml"),
    )
    .expect("the pinned hub fixture is readable");
    verify_fixture_bytes("hub_2507.19457", &real, PRESERVED_TEI[0].2)
        .expect("the frozen checksum must still match the pinned fixture on disk");
    let mut drifted = real.clone();
    drifted[0] ^= 0x01;
    match verify_fixture_bytes("hub_2507.19457", &drifted, PRESERVED_TEI[0].2) {
        Ok(_) => panic!("FATAL the checksum guard accepted a drifted fixture"),
        Err(reason) => cases.push(("fixture_checksum_drift", reason)),
    }
    let non_utf8 = [0xff_u8, 0xfe];
    let non_utf8_digest = format!("{:x}", Sha256::digest(non_utf8));
    match verify_fixture_bytes("synthetic-non-utf8", &non_utf8, &non_utf8_digest) {
        Ok(_) => panic!("FATAL the encoding guard accepted non-UTF-8 fixture bytes"),
        Err(reason) => cases.push(("fixture_non_utf8", reason)),
    }

    let guard_count = cases.len();
    for (name, reason) in cases {
        println!("guard {name}: rejected ({reason})");
    }
    println!(
        "self-check: {} guards rejected their violating inputs",
        guard_count
    );
}

// ─────────────────────────────────── main ───────────────────────────────────

fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_fulltext_corpus_shadow");
    let args = Args::parse();
    let contract = load_contract();
    let provenance = load_provenance();

    if args.self_check {
        self_check();
        return;
    }
    let report_path = args
        .report
        .as_deref()
        .unwrap_or_else(|| panic!("FATAL --report <path> is required for the canonical run"));

    // Identity: the corpus is exactly these three sha256 pins, and the DATA
    // carrier and the provenance register must name the same three.
    let declared_shas: BTreeSet<String> = contract
        .documents
        .iter()
        .map(|document| document.sha256.clone())
        .collect();
    let provenance_shas: BTreeSet<String> = provenance
        .fixtures
        .iter()
        .map(|fixture| fixture.sha256.clone())
        .collect();
    guard_identity(&declared_shas, contract.denominators.unique_source_sha256)
        .unwrap_or_else(|reason| panic!("FATAL data carrier identity: {reason}"));
    assert_eq!(
        contract.unique_source_sha256, contract.denominators.unique_source_sha256,
        "FATAL the DATA carrier's unique_source_sha256 disagrees with its denominators"
    );
    assert_eq!(
        contract.unique_source_sha256,
        PRESERVED_TEI.len(),
        "FATAL the DATA carrier's unique_source_sha256 must be the pinned corpus size"
    );
    for (document_id, _, sha) in PRESERVED_TEI {
        assert!(
            declared_shas.contains(sha),
            "FATAL {document_id}: the DATA carrier does not declare the pinned sha256"
        );
        assert!(
            provenance_shas.contains(sha),
            "FATAL {document_id}: provenance.yaml does not declare the pinned sha256"
        );
        assert!(
            contract
                .documents
                .iter()
                .any(|document| document.id == document_id && document.sha256 == sha),
            "FATAL {document_id}: the DATA carrier declares a different sha256 for this document"
        );
    }
    let expected_classes: Vec<String> = contract.class_order.clone();
    let order: Vec<&str> = CLASS_ORDER.to_vec();
    guard_scope(
        &order,
        &expected_classes
            .iter()
            .map(|class| class.as_str())
            .collect::<Vec<_>>()
            .iter()
            .map(|class| (*class).to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|reason| panic!("FATAL class vocabulary: {reason}"));
    assert_eq!(
        contract
            .refusal_classes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        SemanticFieldLoss::ALL
            .iter()
            .map(|class| class.token())
            .collect::<Vec<_>>(),
        "FATAL refusal_classes must be the production SemanticFieldLoss vocabulary"
    );

    let mut rows: Vec<Outcome> = Vec::new();
    let mut measured: BTreeMap<String, ClassProfile> = BTreeMap::new();
    let mut shas: BTreeSet<String> = BTreeSet::new();

    for (document_id, file, sha) in PRESERVED_TEI {
        let xml = preserved_tei_source(file, sha);
        shas.insert(sha.to_string());
        let fixture = format!("crates/da-parse/tests/fixtures/preserved/{file}");
        let source = measure_source(
            document_id,
            Box::leak(fixture.into_boxed_str()),
            sha,
            &xml,
            &provenance,
        );
        // Runtime non-vacuity: the production lifter must see the same real header
        // author population the independent scan sees, and at least one real
        // citation.
        let lifted = TeiLifter::lift_tei(xml.clone(), document_id, sha);
        assert_eq!(
            lifted.header_authors.len(),
            source.author_name_texts.len(),
            "FATAL {document_id}: TeiLifter header authors disagree with the independent scan"
        );
        assert!(
            !lifted.citations.is_empty(),
            "FATAL {document_id}: TeiLifter lifted no citation from a real bibliography"
        );
        let admitted = build_admitted_band(&source);
        for gap in &admitted.gaps {
            println!(
                "gap {} {document_id}: source_population={} admitted={} ({})",
                gap.class, gap.source_population, gap.admitted, gap.reason
            );
        }
        let outcome = compose_corpus_document(&source, &admitted, &xml);
        let (disposition, emission) = match &outcome {
            NativeFulltextCompositionOutcome::Accepted(candidate) => (
                "accepted".to_string(),
                String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap_or_else(|error| {
                    panic!("FATAL {document_id}: emission is not UTF-8 ({error})")
                }),
            ),
            NativeFulltextCompositionOutcome::Rejected { error, .. } => {
                (format!("rejected:{error:?}"), String::new())
            }
        };
        let profiles = if disposition == "accepted" {
            oracle_class_profiles(&source, &xml, &admitted, &emission)
        } else {
            Vec::new()
        };
        for profile in &profiles {
            let entry = measured
                .entry(profile.class.to_string())
                .or_insert(ClassProfile {
                    class: profile.class,
                    source_population: 0,
                    admitted: 0,
                    conserved: 0,
                    lost: 0,
                    fabricated: 0,
                });
            entry.source_population += profile.source_population;
            entry.admitted += profile.admitted;
            entry.conserved += profile.conserved;
            entry.lost += profile.lost;
            entry.fabricated += profile.fabricated;
        }
        rows.push(Outcome {
            document_id: document_id.to_string(),
            source_sha256: sha.to_string(),
            profile_fingerprint: source.profile_fingerprint.clone(),
            config_fingerprint: source.config_fingerprint.clone(),
            disposition,
            lift: LiftCounters {
                header_authors: lifted.header_authors.len(),
                citations: lifted.citations.len(),
            },
            classes: profiles
                .iter()
                .map(|profile| ClassRow {
                    class: profile.class.to_string(),
                    source_population: profile.source_population,
                    admitted: profile.admitted,
                    conserved: profile.conserved,
                    fabricated: profile.fabricated,
                    precision: profile.precision(),
                    recall: profile.recall(),
                    f1: profile.f1(),
                    admission_coverage: profile.admission_coverage(),
                })
                .collect(),
        });
    }

    guard_identity(&shas, contract.denominators.unique_source_sha256)
        .unwrap_or_else(|reason| panic!("FATAL corpus identity: {reason}"));
    guard_documents(&rows, contract.denominators.documents)
        .unwrap_or_else(|reason| panic!("FATAL corpus rows: {reason}"));
    guard_denominators(&measured).unwrap_or_else(|reason| panic!("FATAL denominators: {reason}"));
    let observed: Vec<String> = measured.keys().cloned().collect();
    guard_scope(&order, &observed).unwrap_or_else(|reason| panic!("FATAL scope: {reason}"));

    let recorded = aggregate(&measured);
    guard_aggregate(&recorded, &measured)
        .unwrap_or_else(|reason| panic!("FATAL aggregate: {reason}"));
    guard_data_agreement(&measured, &recorded, &contract)
        .unwrap_or_else(|reason| panic!("FATAL DATA agreement: {reason}"));

    println!(
        "corpus: documents={} accepted={} TP={} FP={} FN={} P={:.6} R={:.6} F1={:.6} admission_coverage={:.6}",
        rows.len(),
        rows.iter()
            .filter(|row| row.disposition == "accepted")
            .count(),
        recorded.tp,
        recorded.fp,
        recorded.fn_count,
        recorded.precision,
        recorded.recall,
        recorded.f1,
        recorded.admission_coverage,
    );
    for row in &rows {
        println!(
            "  {} sha={} disposition={} lift(authors={}, citations={})",
            row.document_id,
            &row.source_sha256[..12],
            row.disposition,
            row.lift.header_authors,
            row.lift.citations
        );
    }

    // `accepted` is the number of accepted rows, not the admitted-carrier sum:
    // the rows already carry the per-class admitted counters.  It is computed
    // here because `outcomes: rows` below moves the vector.
    let accepted_rows = rows
        .iter()
        .filter(|row| row.disposition == "accepted")
        .count();

    let report = Report {
        schema_version: REPORT_SCHEMA,
        corpus_id: CORPUS_ID,
        source_identity: SourceIdentity {
            identity_kind: contract.identity_kind.clone(),
            unique_source_sha256: shas.len(),
            source_sha256: PRESERVED_TEI
                .iter()
                .map(|(_, _, sha)| (*sha).to_string())
                .collect(),
            profile_fingerprint: rows[0].profile_fingerprint.clone(),
            config_fingerprint: rows[0].config_fingerprint.clone(),
            fallback_target: contract.fallback_target.clone(),
        },
        reference_population: ReferencePopulation {
            role: REFERENCE_POPULATION_ROLE,
            identity: REFERENCE_POPULATION_IDENTITY,
            documents: rows.len(),
            unique_source_sha256: shas.len(),
            per_class: measured
                .iter()
                .map(|(class, profile)| {
                    (
                        class.clone(),
                        DenominatorRow {
                            source_population: profile.source_population,
                            admitted: profile.admitted,
                        },
                    )
                })
                .collect(),
        },
        outcomes: rows,
        executed_population: ExecutedPopulation {
            corpus_execution: true,
            execution_evidence: EXECUTION_EVIDENCE,
            documents: PRESERVED_TEI.len(),
            accepted: accepted_rows,
        },
        evaluated_population: EvaluatedPopulation {
            status: EVALUATED_STATUS,
            oracle: EVALUATED_ORACLE,
            documents: PRESERVED_TEI.len(),
            aggregate: recorded,
            per_class: measured
                .iter()
                .map(|(class, profile)| (class.clone(), aggregate_of(profile)))
                .collect(),
        },
        mutation_summary: contract
            .mutation_matrix
            .iter()
            .map(|mutation| (mutation.id.clone(), mutation_detector(mutation)))
            .collect(),
        policy: PolicyLocks {
            observation_mode: "shadow",
            promotion_state: "candidate",
            import_eligible: contract.locks.import_eligible,
            production_route: "forbidden",
            graph_mutation: "forbidden",
            auto_promote: "forbidden",
            deployed: false,
            pooled_winner: "forbidden",
        },
        redaction: Redaction {
            raw_tei: false,
            raw_pdf: false,
            raw_text: false,
            field_values: false,
            names: false,
            email_addresses: false,
            captions: false,
            references: false,
            physical_paths: false,
            hostname: false,
            executable: false,
            secrets: false,
            pdf_bytes: false,
            graph_payload: false,
            fulltext: false,
        },
    };
    assert!(
        contract.locks.production_route == "forbidden"
            && contract.locks.graph_write == "forbidden"
            && contract.locks.auto_promote == "forbidden"
            && !contract.locks.import_eligible,
        "FATAL the DATA carrier's policy locks must keep the runner shadow-only"
    );
    assert!(
        !report.source_identity.profile_fingerprint.is_empty()
            && !report.source_identity.config_fingerprint.is_empty(),
        "FATAL identity fingerprints must be non-empty"
    );

    fs_create_and_write(
        report_path,
        &serde_json::to_vec_pretty(&report).expect("report JSON"),
    );
}

fn aggregate(per_class: &BTreeMap<String, ClassProfile>) -> Aggregate {
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_count = 0usize;
    let mut admitted = 0usize;
    let mut population = 0usize;
    for profile in per_class.values() {
        tp += profile.conserved;
        fp += profile.fabricated;
        fn_count += profile.lost;
        admitted += profile.admitted;
        population += profile.source_population;
    }
    let precision = ratio(tp, tp + fp);
    let recall = ratio(tp, tp + fn_count);
    let f1 = if precision + recall == 0.0 {
        1.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    Aggregate {
        tp,
        fp,
        fn_count,
        precision,
        recall,
        f1,
        admission_coverage: ratio(admitted, population),
    }
}

fn aggregate_of(profile: &ClassProfile) -> Aggregate {
    Aggregate {
        tp: profile.conserved,
        fp: profile.fabricated,
        fn_count: profile.lost,
        precision: profile.precision(),
        recall: profile.recall(),
        f1: profile.f1(),
        admission_coverage: profile.admission_coverage(),
    }
}

fn fs_create_and_write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("report directory");
    }
    std::fs::write(path, bytes).expect("report write");
}
