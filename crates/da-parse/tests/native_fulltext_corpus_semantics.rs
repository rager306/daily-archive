//! M511-j7wz2f / S05 / T01 — the independent semantic profile of the REAL
//! preserved corpus (H18).
//!
//! S01/S03/S04 drive the three sha256-pinned preserved TEI through
//! `compose_native_fulltext`, but the *admitted* band they hand it is synthetic:
//! S03 injects sentinel constants (`S03 ORACLE ADMITTED TITLE`, …) precisely so
//! that "the admitted field reached the emission" cannot be confused with the
//! source's own header subtrees, which the composer conserves.  That is the right
//! device for a fault-localizing RED, and it is not a corpus regression: no
//! admitted band anywhere is derived from the real documents, so nothing measures
//! whether the composer conserves the semantics the real corpus actually carries.
//!
//! This file measures exactly that.  For each of the three preserved documents it
//!
//! 1. reads the document with an independent XML parser (`roxmltree`, a
//!    dev-dependency only — the composer's production import root set stays
//!    `{crate, serde, sha2, std}` and nothing in `src/` can reach it),
//! 2. derives the admitted band **from the real document** — never from a
//!    sentinel — through the public typed parsers
//!    (`NativePersonalNameParser`, `NativeAffiliationParser`, `NativeDateParser`),
//! 3. hands that band to `compose_native_fulltext` (the only composition entry
//!    point this file reaches; the guard below is a real invariant of this file,
//!    not a comment),
//! 4. re-reads the **emission bytes** with the same independent parser and counts,
//!    per semantic class,
//!
//! | counter | meaning |
//! |---|---|
//! | `source_population` | independent `roxmltree` count of the carrier elements the real document declares |
//! | `admitted` | occurrences the admitted band really carried (a parser rejection is a **named** gap, never a silent narrowing) |
//! | `conserved` | admitted occurrences the emitted band carries, net of the occurrences the conserved source subtree already contributes |
//! | `lost` | `admitted - conserved` (a false negative: the band carried it, the emission does not) |
//! | `fabricated` | emitted band carriers matching neither an admitted value nor an independently measured source value (a false positive) |
//!
//! and reports `TP=conserved`, `FN=lost`, `FP=fabricated`,
//! `P=TP/(TP+FP)`, `R=TP/(TP+FN)`, `F1`, plus the honest
//! `admission_coverage = admitted/source_population`.
//!
//! ## Why the source-occurrence rule is load-bearing
//!
//! The composer re-inserts the source `titleStmt`/`fileDesc`/header content
//! verbatim, so the real `<title>`, `<email>`, `<idno>` and `<affiliation>`
//! carriers of `sourceDesc` are present in every emission *whatever the admitted
//! band did*.  A membership-only oracle would therefore report a clean `1.0` for a
//! composer that dropped the admitted band entirely.  `conserved` is therefore
//! computed as `emitted_matches - source_matches` (saturating), which is `0` when
//! the band contributed nothing and the only occurrences in the emission are the
//! conserved source subtrees — see
//! [`oracle_is_not_fooled_by_source_conserved_occurrences`].
//!
//! The reference and citation bands are the opposite case: the emitted `listBibl`
//! is **rebuilt** from the admitted band (the source's own `listBibl` is replaced
//! by the splice), so nothing there is conserved from the source and
//! `source_conserved` is `false` for those classes.  Their source carriers are
//! still used as the fabrication exclusion set, per the plan's rule that a real
//! value already carried by the source is not a fabrication.
//!
//! ## Frozen measurement
//!
//! Every measured number is an assertion ([`FROZEN_PROFILE`],
//! [`FROZEN_ADMISSION_GAPS`], the corpus aggregate), so the profile fails on
//! deviation in **either** direction: a composer that loses an admitted field
//! fails `conserved == admitted`, and a composer that fabricates one fails
//! `fabricated == 0`.  The independent `roxmltree` counts are additionally
//! cross-checked against the already declared independent denominators in
//! `data/parse/native_fulltext_composition.yaml#source_evidence.s12_named_denominators`
//! — a mismatch is a named failure carrying both numbers.
//!
//! Only counters are ever printed or asserted.  Field values (titles, e-mail
//! addresses, author names) never leave the measurement: this file publishes
//! counts, class tokens and digests, exactly like the DATA carrier it agrees with.
//!
//! ## Independence guard
//!
//! The admitted band is assembled here from document-derived values; no upstream
//! composition helper is reachable from this file, and
//! [`expected_side_does_not_call_a_composition_entry_point`] enforces that by
//! scanning this file's own source.  The judge of the emission is the independent
//! parser alone: the composer's manifest ledger is cross-checked *against* the
//! oracle, never used as its input.

use da_parse::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ─────────────────────────── preserved corpus identity ──────────────────────

/// The three sha256-pinned preserved TEI, in `provenance.yaml` order.
///
/// `identity_kind = preserved_three_sha` and `unique_source_sha256 = 3`: the
/// corpus identity of this measurement *is* these three digests.
const PRESERVED_TEI: [(&str, &str); 3] = [
    (
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    ),
    (
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    ),
    (
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    ),
];

/// The direct `listBibl` `biblStruct` denominator each document must keep.  Read
/// from the document by the independent scanner and cross-checked against the
/// DATA carrier; the two must agree.
const BIBL_STRUCT_DENOMINATOR: [usize; 3] = [80, 49, 78];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a preserved TEI fixture and verify its frozen checksum.  A missing,
/// unreadable or drifted fixture is FATAL: the measurement must never silently
/// profile bytes other than the pinned ones.
fn preserved_tei_source(name: &str, declared_sha256: &str) -> String {
    let rel = format!("crates/da-parse/tests/fixtures/preserved/{name}");
    let bytes = std::fs::read(repo_root().join(&rel)).unwrap_or_else(|error| {
        panic!("FATAL {rel}: preserved fixture missing/unreadable ({error})")
    });
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        actual, declared_sha256,
        "FATAL {name}: preserved fixture bytes drifted from the frozen checksum"
    );
    String::from_utf8(bytes)
        .unwrap_or_else(|error| panic!("FATAL {name}: preserved fixture is not UTF-8 ({error})"))
}

#[derive(Deserialize)]
struct Provenance {
    assumed_grobid_profile: String,
    configuration_fingerprint: String,
}

fn provenance() -> Provenance {
    let rel = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
    let text = std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|error| panic!("FATAL {rel}: unreadable ({error})"));
    serde_yaml::from_str(&text).unwrap_or_else(|error| panic!("FATAL {rel}: unparsable ({error})"))
}

// ───────────────── the declared independent denominators (DATA) ──────────────

/// The already-declared independent denominators this measurement must agree
/// with.  Only the fields this corpus profile consumes are declared: serde
/// ignores the rest of the block.
#[derive(Deserialize)]
struct DeclaredDenominators {
    header_title_elements_total: usize,
    header_emails_total: usize,
    header_pubnum_elements_total: usize,
    header_idno_elements_total: usize,
    header_checksum_idno_elements_total: usize,
    header_raw_total: usize,
    header_accepted_total: usize,
    affiliation_linking_total: usize,
    references_total: usize,
    bibl_struct_hub_total: usize,
    bibl_struct_rlm_total: usize,
    bibl_struct_reca_total: usize,
    citation_names_total: usize,
}

#[derive(Deserialize)]
struct SourceEvidence {
    s12_named_denominators: DeclaredDenominators,
}

#[derive(Deserialize)]
struct CompositionData {
    source_evidence: SourceEvidence,
}

fn declared_denominators() -> DeclaredDenominators {
    let rel = "data/parse/native_fulltext_composition.yaml";
    let text = std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|error| panic!("FATAL {rel}: unreadable ({error})"));
    let data: CompositionData = serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("FATAL {rel}: unparsable ({error})"));
    data.source_evidence.s12_named_denominators
}

// ─────────────────────────── independent XML readers ────────────────────────

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

fn is_element(node: &roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

/// Whitespace-normalized concatenated text of every text descendant.
///
/// The joined-with-a-single-space form is the same reconstruction the harness
/// helpers use; collapsing runs of whitespace additionally removes a difference
/// that is not a semantic difference when the source document and the emission
/// spell the same characters across a different element boundary.
fn normalized_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_element<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    name: &str,
) -> roxmltree::Node<'a, 'input> {
    let mut found = document.descendants().filter(|node| is_element(node, name));
    let first = found
        .next()
        .unwrap_or_else(|| panic!("the document declares no <{name}> element"));
    assert!(
        found.next().is_none(),
        "the independent scanner requires a unique <{name}> element"
    );
    first
}

/// The unique `listBibl` element — the reference band's own address space.
fn unique_list_bibl<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    unique_element(document, "listBibl")
}

/// `roxmltree` attribute access for the reserved XML namespace.
fn xml_id_of<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute((XML_NAMESPACE, "id"))
}

fn direct_child<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    parent.children().find(|child| is_element(child, name))
}

fn descendants_named<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
    name: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    root.descendants()
        .filter(move |node| is_element(node, name))
}

fn parse_document<'a>(fixture: &str, xml: &'a str) -> roxmltree::Document<'a> {
    roxmltree::Document::parse(xml)
        .unwrap_or_else(|error| panic!("{fixture}: the document does not parse: {error}"))
}

// ───────────────────────── the class vocabulary ─────────────────────────────

// The five production loss tokens are derived from the production enum, the
// reference class token from the composer's own declared derivation and the four
// citation class tokens from the composer's role mapping: none of these is a
// second literal copy, so the oracle and the composer gate cannot drift into
// near-miss spellings.

/// The admitted author line (the header band's own ordinal space, which every
/// affiliation binding is keyed by).  It is not one of the production loss
/// classes because the composer does not gate on it directly — a rejected author
/// is dropped by the admission step *before* a band exists — but it is the carrier
/// whose real-corpus narrowing (36 raw → 35 admitted) must be a measured, named
/// result rather than a silent shrink.
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

const CLASS_COUNT: usize = 10;

/// The four citation roles this corpus can really express, with the `biblStruct`
/// carrier form each is read from.  Nothing outside this table is invented: a
/// `CitationFieldRole` with no carrier form in the corpus is simply not part of
/// the measurement.
const CORPUS_CITATION_ROLES: [(CitationFieldRole, &str); 4] = [
    (CitationFieldRole::Doi, CLASS_CITATION_DOI),
    (CitationFieldRole::ArxivId, CLASS_CITATION_ARXIV),
    (CitationFieldRole::Url, CLASS_CITATION_URL),
    (
        CitationFieldRole::ArticleTitle,
        CLASS_CITATION_ARTICLE_TITLE,
    ),
];

fn citation_class_token(role: CitationFieldRole) -> &'static str {
    CORPUS_CITATION_ROLES
        .iter()
        .find(|(candidate, _)| *candidate == role)
        .map(|(_, class)| *class)
        .expect("every measured citation role has a class token")
}

/// `<idno>` types that are checksum identifiers rather than bibliographic
/// identifiers.  GROBID's header CRF calls the latter `<pubnum>`; the preserved
/// carrier spells them `type="arXiv"` and the checksum identifier `type="MD5"`,
/// which is why the declared denominators name both
/// (`header_pubnum_elements_total` vs `header_checksum_idno_elements_total`).
const CHECKSUM_IDNO_TYPES: [&str; 3] = ["MD5", "SHA1", "SHA256"];

fn is_checksum_idno(node: roxmltree::Node<'_, '_>) -> bool {
    node.attribute("type")
        .is_some_and(|kind| CHECKSUM_IDNO_TYPES.contains(&kind))
}

// ─────────────────── the independent source-side measurement ─────────────────

/// One real `<author>`/`<affiliation key>` binding, in document order.
struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the independent scanner reads from one real preserved document.
/// These values are the *source population* side of the profile; nothing here is
/// produced by the composer.
struct CorpusSource {
    fixture: &'static str,
    sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    /// Every `<title>` inside `teiHeader` (carrier population).
    header_titles: Vec<String>,
    /// The direct `fileDesc/titleStmt/title` children — the band's title field.
    title_stmt_titles: Vec<String>,
    header_emails: Vec<String>,
    /// Every `<idno>` inside `teiHeader`.
    header_idnos: Vec<String>,
    /// The non-checksum `<idno>`s — the `<pubnum>` projection.
    header_pubnum_ids: Vec<String>,
    author_name_texts: Vec<String>,
    affiliation_occurrences: Vec<AffiliationOccurrence>,
    date_text: String,
    reference_texts: Vec<String>,
    citation_carriers: Vec<(CitationFieldRole, String)>,
}

fn measure_source(
    fixture: &'static str,
    declared_sha256: &str,
    xml: &str,
    provenance: &Provenance,
) -> CorpusSource {
    let document = parse_document(fixture, xml);
    let header = unique_element(&document, "teiHeader");
    let list = unique_list_bibl(&document);

    let title_stmt =
        direct_child(header, "fileDesc").and_then(|desc| direct_child(desc, "titleStmt"));
    let title_stmt_titles: Vec<String> = title_stmt
        .map(|stmt| {
            descendants_named(stmt, "title")
                .map(normalized_text)
                .collect()
        })
        .unwrap_or_default();
    let header_titles: Vec<String> = descendants_named(header, "title")
        .map(normalized_text)
        .collect();
    let header_emails: Vec<String> = descendants_named(header, "email")
        .map(normalized_text)
        .collect();
    let header_idnos: Vec<String> = descendants_named(header, "idno")
        .map(normalized_text)
        .collect();
    let header_pubnum_ids: Vec<String> = descendants_named(header, "idno")
        .filter(|node| !is_checksum_idno(*node))
        .map(normalized_text)
        .collect();

    let author_elements: Vec<roxmltree::Node<'_, '_>> =
        descendants_named(header, "author").collect();
    let author_name_texts: Vec<String> = author_elements
        .iter()
        .map(|author| {
            author
                .children()
                .find(|child| is_element(child, "persName"))
                .map(normalized_text)
                .unwrap_or_default()
        })
        .collect();

    // The real affiliation bindings: an `<affiliation key=…>` child of a real
    // `<author>`, in document order.  The occurrence text is the carrier's own
    // text; the only span the band may carry is the one the public recognizer
    // reports for it.
    let mut affiliation_occurrences = Vec::new();
    for (author_index, author) in author_elements.iter().enumerate() {
        for affiliation in author
            .children()
            .filter(|child| is_element(child, "affiliation"))
        {
            let Some(key) = affiliation.attribute("key") else {
                continue;
            };
            affiliation_occurrences.push(AffiliationOccurrence {
                author_index,
                key: key.to_string(),
                text: normalized_text(affiliation),
            });
        }
    }

    let date_text = descendants_named(header, "date")
        .map(normalized_text)
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| panic!("{fixture}: the header declares no dated element with text"));

    let reference_texts: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(normalized_text)
        .collect();

    // The citation carrier forms the real `biblStruct` truth expresses.  Each
    // value comes from the carrier itself: element text for `idno`/`title`, the
    // `target` attribute for `ptr`.
    let mut citation_carriers = Vec::new();
    for node in list.descendants() {
        if !node.is_element() {
            continue;
        }
        match node.tag_name().name() {
            "idno" => match node.attribute("type") {
                Some("DOI") => {
                    citation_carriers.push((CitationFieldRole::Doi, normalized_text(node)))
                }
                Some("arXiv") => {
                    citation_carriers.push((CitationFieldRole::ArxivId, normalized_text(node)))
                }
                _ => {}
            },
            "ptr" => {
                if let Some(target) = node.attribute("target")
                    && target.starts_with("http")
                {
                    citation_carriers.push((CitationFieldRole::Url, target.to_string()));
                }
            }
            "title" if node.attribute("level") == Some("a") => {
                citation_carriers.push((CitationFieldRole::ArticleTitle, normalized_text(node)));
            }
            _ => {}
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

// ────────────────────────── the admitted band (input) ───────────────────────

/// One named, typed admission narrowing: a class whose real carrier population
/// could not be admitted in full, with the reason.  `admitted < source_population`
/// is only allowed when a row of this table explains it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmissionGap {
    class: &'static str,
    source_population: usize,
    admitted: usize,
    reason: String,
}

/// The admitted band: the typed composition input plus the admitted-side values
/// the independent oracle measures the emission against.  All of it is derived
/// from the real document; none of it is a sentinel.
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
/// `ambiguous:single_token`.  Derived from the serde form so the recorded reason
/// cannot drift into prose that no longer matches the parser's own vocabulary.
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
/// parsers.  Every parser rejection becomes a named [`AdmissionGap`]; nothing is
/// silently dropped and no value is invented.
fn build_admitted_band(source: &CorpusSource) -> AdmittedBand {
    let mut gaps = Vec::new();

    // The header date: the document's own first dated element with text.
    let date = match NativeDateParser::new().parse(&source.date_text) {
        NativeDateOutcome::Recognized { date } => date,
        other => panic!(
            "FATAL {}: the real publication date form cannot be admitted by NativeDateParser ({}); \
             the header band cannot be composed from the real document",
            source.fixture,
            outcome_token(&other)
        ),
    };

    // The admitted title field is the band's definition: the direct
    // `fileDesc/titleStmt/title` children.  The other real `<title>` carriers are
    // conserved by the source subtree the composer re-inserts, so they are a
    // source-carried, not a band-carried, occurrence.
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

    // The affiliation components: one per real occurrence the public recognizer
    // accepts, in document order.  A rejected occurrence drops the binding that
    // points at it, and the drop is named once, with the final admitted count.
    let affiliation_parser = NativeAffiliationParser::new();
    let mut components: Vec<NativeAffiliationOutcome> = Vec::new();
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
                components.push(NativeAffiliationOutcome::Recognized {
                    components: recognized,
                });
            }
            other => {
                occurrence_component.push(None);
                rejected_affiliations.push(outcome_token(&other));
            }
        }
    }

    // Names, then the affiliation bindings they key into.
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
        // The band's own ordinal address: the composer validates only that an
        // author span is non-empty and non-overlapping, and the independent oracle
        // never reads it — the binding's identity is the owner id and the key.
        let owner = format!("author-{ordinal}");
        let mut links = Vec::new();
        for (occurrence_index, occurrence) in source.affiliation_occurrences.iter().enumerate() {
            if occurrence.author_index != author_index {
                continue;
            }
            let Some(component) = occurrence_component[occurrence_index] else {
                continue;
            };
            let NativeAffiliationOutcome::Recognized {
                components: recognized,
            } = &components[component]
            else {
                continue;
            };
            let span = recognized
                .first()
                .expect("a recognized affiliation carries a component")
                .span;
            links.push(NativeAffiliationLink {
                key: occurrence.key.clone(),
                affiliation_ordinal: component,
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

    // The reference band: every direct `listBibl` child, ordinal = index.  The
    // composer refuses a domain that does not equal the source's own entries, so
    // this is the honest domain, not a narrowed one.
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
            provenance: "preserved-tei-corpus-semantics".into(),
        })
        .collect();
    assert!(
        references
            .iter()
            .all(|reference| reference.span.start < reference.span.end),
        "FATAL {}: a source biblStruct carries empty text, so no honest reference span exists",
        source.fixture
    );

    // The citation fields: only roles the real `biblStruct` forms express, with
    // the value read from the carrier itself.
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

// ─────────────────────────── the independent oracle ────────────────────────

/// One class's measured profile: `TP = conserved`, `FN = lost`, `FP = fabricated`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassProfile {
    fixture: String,
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
    /// Honest coverage of the real carrier population by the admitted band.  The
    /// `0/0` case has no population to cover and is reported as `1.0`; such a class
    /// is asserted to carry no admitted occurrence either, so the convention
    /// cannot hide a loss.
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

/// Occurrence-counted conservation of text-valued carriers.
///
/// * `conserved` is `admitted` net of the occurrences the conserved source
///   subtrees already contribute: when the source's own carriers match the
///   admitted values, subtracting them is what makes a band that contributed
///   nothing read as `0` instead of a vacuous `1.0`.
/// * `fabricated` counts emitted band carriers matching neither an admitted value
///   nor a source value.
fn text_class(
    fixture: &str,
    class: &'static str,
    source_population: usize,
    admitted_values: &[String],
    emitted_carriers: &[String],
    source_carriers: &[String],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted_values
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        fixture: fixture.to_string(),
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
    }
}

/// Occurrence-counted conservation of binding-valued carriers (an owner element
/// plus a value: the author line's surname, or an affiliation key).
fn binding_class(
    fixture: &str,
    class: &'static str,
    source_population: usize,
    admitted_values: &[(String, String)],
    emitted_carriers: &[(String, String)],
    source_carriers: &[(String, String)],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted_values
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        fixture: fixture.to_string(),
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
    }
}

/// The emitted header's author-line carriers: `author-{ordinal}` → surname.  Only
/// authors the composer synthesized carry an `xml:id`, which is exactly the
/// distinction between a band carrier and a conserved source carrier.
fn emitted_author_lines(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let header = unique_element(document, "teiHeader");
    descendants_named(header, "author")
        .filter_map(|author| {
            let id = xml_id_of(author)?;
            let surname = author
                .children()
                .find(|child| is_element(child, "surname"))
                .map(normalized_text)
                .unwrap_or_default();
            Some((id.to_string(), surname))
        })
        .collect()
}

/// The emitted header's affiliation bindings: `author-{ordinal}` → key.  Only a
/// synthesized author carries an `xml:id`, so a conserved source affiliation can
/// never enter this band's carrier set.
fn emitted_affiliation_bindings(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let header = unique_element(document, "teiHeader");
    let mut bindings = Vec::new();
    for author in descendants_named(header, "author") {
        let Some(owner) = xml_id_of(author) else {
            continue;
        };
        for affiliation in descendants_named(author, "affiliation") {
            if let Some(key) = affiliation.attribute("key") {
                bindings.push((owner.to_string(), key.to_string()));
            }
        }
    }
    bindings
}

/// The emitted `listBibl` carriers of the measured citation roles, as
/// `(role wire token, value)` pairs.  The band is the emitted bibliography: the
/// source's own `listBibl` is replaced by the splice, so nothing there is
/// conserved.
fn emitted_citation_carriers(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let list = unique_list_bibl(document);
    list.descendants()
        .filter(|node| is_element(node, "note"))
        .filter_map(|node| {
            let role = node.attribute("role")?;
            let known = CORPUS_CITATION_ROLES
                .iter()
                .any(|(candidate, _)| citation_field_role_token(*candidate) == role);
            known.then(|| (role.to_string(), normalized_text(node)))
        })
        .collect()
}

/// The source document's own author-line carriers, keyed by the empty owner (a
/// source `<author>` carries no `xml:id`), so they can never satisfy a synthesized
/// `author-{ordinal}` binding.
fn source_author_lines(xml: &str) -> Vec<(String, String)> {
    let document = parse_document("source-author-lines", xml);
    let header = unique_element(&document, "teiHeader");
    descendants_named(header, "author")
        .map(|author| {
            let surname = author
                .children()
                .find(|child| is_element(child, "surname"))
                .map(normalized_text)
                .unwrap_or_default();
            (String::new(), surname)
        })
        .collect()
}

/// The source document's own affiliation bindings, keyed by the empty owner for
/// the same reason.
fn source_affiliation_bindings(xml: &str) -> Vec<(String, String)> {
    let document = parse_document("source-affiliation-bindings", xml);
    let header = unique_element(&document, "teiHeader");
    descendants_named(header, "author")
        .flat_map(|author| {
            descendants_named(author, "affiliation")
                .filter_map(|node| node.attribute("key").map(str::to_string))
                .map(|key| (String::new(), key))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Every class's measured profile for one document, from the emission bytes only.
///
/// `source`/`source_xml` and `admitted` are the frozen expectations and inputs;
/// the judged document is `emission_xml`.  The composer's manifest ledger is
/// deliberately not a parameter: the oracle cannot consult the object it judges.
fn oracle_class_profiles(
    source: &CorpusSource,
    source_xml: &str,
    admitted: &AdmittedBand,
    emission_xml: &str,
) -> Vec<ClassProfile> {
    let document = parse_document(source.fixture, emission_xml);
    let header = unique_element(&document, "teiHeader");
    let list = unique_list_bibl(&document);
    let fixture = source.fixture;

    let emitted_titles: Vec<String> = descendants_named(header, "title")
        .map(normalized_text)
        .collect();
    let emitted_emails: Vec<String> = descendants_named(header, "email")
        .map(normalized_text)
        .collect();
    let emitted_ids: Vec<String> = descendants_named(header, "idno")
        .map(normalized_text)
        .collect();
    let emitted_reference_texts: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(normalized_text)
        .collect();

    let emitted_author_lines = emitted_author_lines(&document);
    let emitted_affiliations = emitted_affiliation_bindings(&document);
    let source_author_line_carriers = source_author_lines(source_xml);
    let source_affiliation_carriers = source_affiliation_bindings(source_xml);

    let emitted_citations = emitted_citation_carriers(&document);
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

    let mut profiles = Vec::with_capacity(CLASS_COUNT);

    // The author line: the synthesized authors, keyed by their `author-{ordinal}`
    // owner and their surname.
    profiles.push(binding_class(
        fixture,
        CLASS_AUTHOR_LINE,
        source.author_name_texts.len(),
        &admitted.author_lines,
        &emitted_author_lines,
        &source_author_line_carriers,
        false,
    ));

    // The header text classes: the conserved source subtrees really are inside the
    // emitted header, so their matching occurrences are subtracted.
    profiles.push(text_class(
        fixture,
        CLASS_HEADER_TITLE,
        source.header_titles.len(),
        &admitted.titles,
        &emitted_titles,
        &source.header_titles,
        true,
    ));
    profiles.push(text_class(
        fixture,
        CLASS_HEADER_EMAIL,
        source.header_emails.len(),
        &admitted.emails,
        &emitted_emails,
        &source.header_emails,
        true,
    ));
    profiles.push(text_class(
        fixture,
        CLASS_HEADER_IDS,
        source.header_pubnum_ids.len(),
        &admitted.ids,
        &emitted_ids,
        &source.header_idnos,
        true,
    ));

    // The affiliation bindings: source bindings hang off authors without an
    // `xml:id`, so they are not band carriers; they stay in the exclusion set.
    profiles.push(binding_class(
        fixture,
        CLASS_AUTHOR_AFFILIATION,
        source.affiliation_occurrences.len(),
        &admitted.affiliation_bindings,
        &emitted_affiliations,
        &source_affiliation_carriers,
        false,
    ));

    // The reference domain: the emitted `listBibl` is rebuilt from the admitted
    // band, so `source_is_conserved` is false; the source texts stay in the
    // fabrication exclusion set.
    profiles.push(text_class(
        fixture,
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
        profiles.push(text_class(
            fixture,
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
        CLASS_COUNT,
        "every declared class must be measured"
    );
    profiles
}

// ─────────────────────────── composition harness ───────────────────────────

/// Compose one document's admitted band with the single SUT this file reaches.
fn compose_corpus_document(
    source: &CorpusSource,
    admitted: &AdmittedBand,
    xml: &str,
) -> NativeFulltextCandidate {
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
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => candidate,
        NativeFulltextCompositionOutcome::Rejected { error, .. } => panic!(
            "FATAL {}: the real-corpus admitted band must compose; typed refusal: {error:?}",
            source.fixture
        ),
    }
}

/// Header and references admitted, every other component typed
/// Unsupported/NotObserved — the same admission shape the S01/S03 fixtures use.
fn corpus_admissions(source_sha256: &str) -> Vec<FulltextAdmissionReceipt> {
    let receipt = |component: FulltextComponent, disposition: AdmissionDisposition| {
        FulltextAdmissionReceipt {
            component,
            disposition,
            producer: "preserved-tei-corpus".into(),
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

// ───────────────────────────── frozen measurement ──────────────────────────

/// One frozen per-class row: `(class, source_population, admitted, conserved,
/// fabricated)`.  Every number is asserted, so a deviation in either direction
/// fails.
struct FrozenFixture {
    name: &'static str,
    classes: [(&'static str, usize, usize, usize, usize); CLASS_COUNT],
}

fn frozen(name: &'static str) -> &'static FrozenFixture {
    FROZEN_PROFILE
        .iter()
        .find(|fixture| fixture.name == name)
        .unwrap_or_else(|| panic!("{name}: no frozen profile"))
}

/// The measured profile of 2026-09-23 on the three sha256-pinned preserved TEI.
///
/// `source_population` is the independent `roxmltree` count of the real carrier
/// elements; `admitted` is what the band really carried through the public
/// parsers; `conserved`/`fabricated` are the emission measurement.  `conserved ==
/// admitted` and `fabricated == 0` everywhere is the frozen claim.
///
/// The table is written out rather than derived so that a change in any counter is
/// visible as a diff in the assertion, not as a silently recomputed baseline.
static FROZEN_PROFILE: [FrozenFixture; 3] = [
    FrozenFixture {
        name: "hub_2507.19457.tei.xml",
        classes: [
            (CLASS_AUTHOR_LINE, 21, 20, 20, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 0, 0, 0, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 0, 0, 0, 0),
            (CLASS_REFERENCE_DOMAIN, 80, 80, 80, 0),
            (CLASS_CITATION_DOI, 6, 6, 6, 0),
            (CLASS_CITATION_ARXIV, 23, 23, 23, 0),
            (CLASS_CITATION_URL, 66, 66, 66, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 21, 21, 21, 0),
        ],
    },
    FrozenFixture {
        name: "rlm_2512.24601.tei.xml",
        classes: [
            (CLASS_AUTHOR_LINE, 4, 4, 4, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 3, 3, 3, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 0, 0, 0, 0),
            (CLASS_REFERENCE_DOMAIN, 49, 49, 49, 0),
            (CLASS_CITATION_DOI, 1, 1, 1, 0),
            (CLASS_CITATION_ARXIV, 1, 1, 1, 0),
            (CLASS_CITATION_URL, 45, 45, 45, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 6, 6, 6, 0),
        ],
    },
    FrozenFixture {
        name: "reca_2605.26525v1.tei.xml",
        classes: [
            (CLASS_AUTHOR_LINE, 11, 11, 11, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 0, 0, 0, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 13, 13, 13, 0),
            (CLASS_REFERENCE_DOMAIN, 78, 78, 78, 0),
            (CLASS_CITATION_DOI, 53, 53, 53, 0),
            (CLASS_CITATION_ARXIV, 11, 11, 11, 0),
            (CLASS_CITATION_URL, 37, 37, 37, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 39, 39, 39, 0),
        ],
    },
];

/// The frozen, named admission narrowings: `(fixture, class, source_population,
/// admitted)`.  Every class whose admitted band is smaller than the real carrier
/// population must appear here with a reason; a narrowing that is not declared is
/// a silent shrink and fails.
const FROZEN_ADMISSION_GAPS: [(&str, &str, usize, usize); 4] = [
    ("hub_2507.19457.tei.xml", CLASS_AUTHOR_LINE, 21, 20),
    ("hub_2507.19457.tei.xml", CLASS_HEADER_TITLE, 2, 1),
    ("rlm_2512.24601.tei.xml", CLASS_HEADER_TITLE, 2, 1),
    ("reca_2605.26525v1.tei.xml", CLASS_HEADER_TITLE, 2, 1),
];

// ──────────────────────────────── tests ────────────────────────────────────

/// Identity: three distinct sha256 digests, re-verified from disk on every run.
#[test]
fn preserved_corpus_identity_is_three_unique_sha256_digests() {
    let mut seen = BTreeSet::new();
    for (name, sha256) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        assert!(
            !xml.is_empty(),
            "{name}: a preserved fixture must stay non-empty"
        );
        assert!(
            seen.insert(sha256),
            "{name}: the corpus identity must be three distinct digests"
        );
    }
    assert_eq!(seen.len(), 3, "unique_source_sha256 stays 3");
}

/// The oracle's class vocabulary is the live production vocabulary, not a second
/// literal copy: the five typed loss tokens come from the production enum and the
/// four citation tokens from the composer's own role mapping.
#[test]
fn class_vocabulary_matches_the_production_tokens() {
    assert_eq!(CLASS_HEADER_TITLE, SemanticFieldLoss::HeaderTitle.token());
    assert_eq!(CLASS_HEADER_EMAIL, SemanticFieldLoss::HeaderEmail.token());
    assert_eq!(CLASS_HEADER_IDS, SemanticFieldLoss::HeaderIds.token());
    assert_eq!(
        CLASS_AUTHOR_AFFILIATION,
        SemanticFieldLoss::AuthorAffiliation.token()
    );
    assert_eq!(
        SemanticFieldLoss::ALL
            .iter()
            .map(|class| class.token())
            .collect::<Vec<_>>(),
        vec![
            CLASS_HEADER_TITLE,
            CLASS_HEADER_EMAIL,
            CLASS_HEADER_IDS,
            CLASS_AUTHOR_AFFILIATION,
            "SemanticFieldLoss::CitationRole",
        ],
        "every production loss class stays in the oracle vocabulary"
    );
    assert_eq!(
        CLASS_REFERENCE_DOMAIN,
        NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION
    );
    for (role, class) in CORPUS_CITATION_ROLES {
        assert_eq!(class, citation_class_token(role));
        assert_eq!(
            citation_field_role_token(role),
            serde_json::to_value(role)
                .expect("CitationFieldRole serializes")
                .as_str()
                .expect("CitationFieldRole serializes as a string"),
            "the emitted role token must equal the serde wire form for {role:?}"
        );
    }
}

/// The independent source-side populations agree with the independently declared
/// denominators, and the header / references / Java-lift populations stay separate
/// keys: pooling them is a named failure, not a rounding.
#[test]
fn independent_source_populations_match_the_declared_denominators() {
    let declared = declared_denominators();
    let provenance = provenance();
    let mut header_authors = 0usize;
    let mut header_titles = 0usize;
    let mut header_emails = 0usize;
    let mut header_idnos = 0usize;
    let mut header_pubnum = 0usize;
    let mut header_checksum = 0usize;
    let mut affiliations = 0usize;
    let mut references = 0usize;

    for (index, (name, sha256)) in PRESERVED_TEI.iter().enumerate() {
        let xml = preserved_tei_source(name, sha256);
        let source = measure_source(name, sha256, &xml, &provenance);
        header_authors += source.author_name_texts.len();
        header_titles += source.header_titles.len();
        header_emails += source.header_emails.len();
        header_idnos += source.header_idnos.len();
        header_pubnum += source.header_pubnum_ids.len();
        header_checksum += source.header_idnos.len() - source.header_pubnum_ids.len();
        affiliations += source.affiliation_occurrences.len();
        references += source.reference_texts.len();

        let declared_bibl = [
            declared.bibl_struct_hub_total,
            declared.bibl_struct_rlm_total,
            declared.bibl_struct_reca_total,
        ][index];
        assert_eq!(
            source.reference_texts.len(),
            declared_bibl,
            "{name}: independent biblStruct population vs declared denominator \
             (measured {}, declared {declared_bibl})",
            source.reference_texts.len()
        );
        assert_eq!(
            source.reference_texts.len(),
            BIBL_STRUCT_DENOMINATOR[index],
            "{name}: independent biblStruct population vs the frozen fixture denominator"
        );
    }

    let named = |what: &str, measured: usize, declared: usize| {
        assert_eq!(
            measured, declared,
            "{what}: independent measurement disagrees with the declared denominator \
             (measured {measured}, declared {declared})"
        );
    };
    named(
        "header_title_elements_total",
        header_titles,
        declared.header_title_elements_total,
    );
    named(
        "header_emails_total",
        header_emails,
        declared.header_emails_total,
    );
    named(
        "header_pubnum_elements_total",
        header_pubnum,
        declared.header_pubnum_elements_total,
    );
    named(
        "header_idno_elements_total",
        header_idnos,
        declared.header_idno_elements_total,
    );
    named(
        "header_checksum_idno_elements_total",
        header_checksum,
        declared.header_checksum_idno_elements_total,
    );
    named(
        "header_raw_total",
        header_authors,
        declared.header_raw_total,
    );
    named(
        "affiliation_linking_total",
        affiliations,
        declared.affiliation_linking_total,
    );
    named("references_total", references, declared.references_total);

    // Three separate populations, three separate keys, explicitly not pooled.  The
    // Java-lift citation-name population is the declared third key; it is not
    // recomputed here because the lift is not part of this measurement, and the
    // assertions below are about the keys staying distinct — a pooled report would
    // have one number where three are owed.
    let java_lift_citation_names = declared.citation_names_total;
    assert_ne!(
        header_authors, references,
        "the header and reference populations must stay distinct"
    );
    assert_ne!(
        header_authors, java_lift_citation_names,
        "the header and java-lift populations must stay distinct"
    );
    assert_ne!(
        references, java_lift_citation_names,
        "the reference and java-lift populations must stay distinct"
    );
    assert_ne!(
        header_authors + references,
        header_authors,
        "pooling references into the header population is forbidden"
    );
}

/// The corpus measurement: the admitted band is derived from the real document,
/// composed through the single SUT, re-read from the emission bytes by the
/// independent parser, and compared with the frozen per-class profile.
#[test]
fn real_corpus_semantic_profile_is_frozen_and_conserves_every_admitted_field() {
    let provenance = provenance();
    let declared = declared_denominators();
    let mut totals = (0usize, 0usize, 0usize);
    let mut admitted_author_lines = 0usize;
    let mut all_gaps: Vec<(&str, AdmissionGap)> = Vec::new();
    let mut documents = 0usize;

    for (name, sha256) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = measure_source(name, sha256, &xml, &provenance);
        let admitted = build_admitted_band(&source);
        let candidate = compose_corpus_document(&source, &admitted, &xml);
        let emission = String::from_utf8(candidate.envelope.raw_tei().to_vec())
            .unwrap_or_else(|error| panic!("{name}: emitted document is not UTF-8 ({error})"));
        let profiles = oracle_class_profiles(&source, &xml, &admitted, &emission);

        let expected = frozen(name);
        assert_eq!(
            profiles.len(),
            expected.classes.len(),
            "{name}: the measured class count must equal the frozen class count"
        );
        documents += 1;

        for (profile, row) in profiles.iter().zip(expected.classes.iter()) {
            let (class, population, admitted_n, conserved_n, fabricated_n) = *row;
            eprintln!(
                "CORPUS {name} {class}: source_population={population} admitted={admitted_n} \
                 conserved={conserved_n} lost={} fabricated={fabricated_n} coverage={:.3} \
                 P={:.3} R={:.3} F1={:.3}",
                profile.lost,
                profile.admission_coverage(),
                profile.precision(),
                profile.recall(),
                profile.f1(),
            );
            assert_eq!(
                profile.class, class,
                "{name}: the class order must match the frozen profile"
            );
            assert_eq!(
                profile.source_population, population,
                "{name}/{class}: source_population drifted (measured {}, frozen {population})",
                profile.source_population
            );
            assert_eq!(
                profile.admitted, admitted_n,
                "{name}/{class}: admitted drifted (measured {}, frozen {admitted_n})",
                profile.admitted
            );
            assert_eq!(
                profile.conserved, conserved_n,
                "{name}/{class}: conserved drifted (measured {}, frozen {conserved_n})",
                profile.conserved
            );
            assert_eq!(
                profile.lost,
                admitted_n - conserved_n,
                "{name}/{class}: lost must be admitted - conserved"
            );
            assert_eq!(
                profile.fabricated, fabricated_n,
                "{name}/{class}: fabricated drifted (measured {}, frozen {fabricated_n})",
                profile.fabricated
            );

            // The GREEN claim: the emission carries every admitted occurrence of
            // this class and fabricates none.
            assert_eq!(
                profile.conserved, profile.admitted,
                "{name}/{class}: the emitted band must conserve every admitted occurrence \
                 (admitted {}, conserved {}, lost {})",
                profile.admitted, profile.conserved, profile.lost
            );
            assert_eq!(
                profile.fabricated, 0,
                "{name}/{class}: the emitted band must fabricate nothing"
            );
            assert_eq!(
                profile.admission_coverage(),
                ratio(profile.admitted, profile.source_population),
                "{name}/{class}: admission_coverage is admitted/source_population, not a claim"
            );
            assert_eq!(
                profile.admission_coverage() == 1.0,
                profile.admitted == profile.source_population,
                "{name}/{class}: coverage reaches 1.0 exactly when the band admitted the whole population"
            );
            assert_eq!(profile.precision(), 1.0, "{name}/{class}: precision");
            assert_eq!(profile.recall(), 1.0, "{name}/{class}: recall");
            assert_eq!(profile.f1(), 1.0, "{name}/{class}: f1");

            totals.0 += profile.conserved;
            totals.1 += profile.lost;
            totals.2 += profile.fabricated;
            if profile.class == CLASS_AUTHOR_LINE {
                admitted_author_lines += profile.admitted;
            }
        }

        // The composer's own ledger is cross-checked against the independent
        // oracle — never the other way round.
        let ledger = candidate.manifest.semantic_fields;
        let class_conserved = |class: &str| {
            profiles
                .iter()
                .find(|profile| profile.class == class)
                .map(|profile| profile.conserved)
                .unwrap_or(0)
        };
        assert_eq!(
            ledger.admitted_header_titles,
            admitted.titles.len(),
            "{name}: ledger admitted titles vs oracle"
        );
        assert_eq!(
            ledger.admitted_header_emails,
            admitted.emails.len(),
            "{name}: ledger admitted emails vs oracle"
        );
        assert_eq!(
            ledger.admitted_header_ids,
            admitted.ids.len(),
            "{name}: ledger admitted ids vs oracle"
        );
        assert_eq!(
            ledger.admitted_author_affiliations,
            admitted.affiliation_bindings.len(),
            "{name}: ledger admitted affiliations vs oracle"
        );
        assert_eq!(
            ledger.admitted_citation_fields,
            admitted.citation_fields.len(),
            "{name}: ledger admitted citation fields vs oracle"
        );
        assert_eq!(
            ledger.conserved_header_titles,
            class_conserved(CLASS_HEADER_TITLE),
            "{name}: ledger conserved titles vs oracle"
        );
        assert_eq!(
            ledger.conserved_header_emails,
            class_conserved(CLASS_HEADER_EMAIL),
            "{name}: ledger conserved emails vs oracle"
        );
        assert_eq!(
            ledger.conserved_header_ids,
            class_conserved(CLASS_HEADER_IDS),
            "{name}: ledger conserved ids vs oracle"
        );
        assert_eq!(
            ledger.conserved_author_affiliations,
            class_conserved(CLASS_AUTHOR_AFFILIATION),
            "{name}: ledger conserved affiliations vs oracle"
        );
        let citation_conserved: usize = CORPUS_CITATION_ROLES
            .iter()
            .map(|(_, class)| class_conserved(class))
            .sum();
        assert_eq!(
            ledger.conserved_citation_fields, citation_conserved,
            "{name}: ledger conserved citation roles vs oracle"
        );

        all_gaps.extend(admitted.gaps.iter().map(|gap| (name, gap.clone())));
    }

    assert_eq!(
        documents, 3,
        "the measured corpus is the three sha256-pinned documents"
    );

    // The admitted author lines across the corpus agree with the independently
    // declared `header_accepted_total` — the same narrowing (36 raw -> 35
    // admitted) the DATA carrier names, measured here through the public parsers.
    assert_eq!(
        admitted_author_lines, declared.header_accepted_total,
        "admitted author lines vs declared header_accepted_total \
         (measured {admitted_author_lines}, declared {})",
        declared.header_accepted_total
    );

    // The corpus aggregate: micro P/R/F1 over every class of every document.
    let (tp, fn_, fp) = totals;
    let aggregate_precision = ratio(tp, tp + fp);
    let aggregate_recall = ratio(tp, tp + fn_);
    eprintln!(
        "CORPUS AGGREGATE: documents={documents} TP={tp} FN={fn_} FP={fp} \
         P={aggregate_precision:.6} R={aggregate_recall:.6} F1={:.6}",
        if aggregate_precision + aggregate_recall == 0.0 {
            1.0
        } else {
            2.0 * aggregate_precision * aggregate_recall / (aggregate_precision + aggregate_recall)
        }
    );
    assert!(tp > 0, "the corpus metric must be non-vacuous");
    assert_eq!(fn_, 0, "no admitted occurrence may be lost");
    assert_eq!(fp, 0, "no emitted band carrier may be fabricated");
    assert_eq!(aggregate_precision, 1.0, "corpus precision");
    assert_eq!(aggregate_recall, 1.0, "corpus recall");

    // Every narrowing is named and frozen; nothing is silently shrunk.
    let mut measured: Vec<(&str, &str, usize, usize)> = all_gaps
        .iter()
        .map(|(fixture, gap)| (*fixture, gap.class, gap.source_population, gap.admitted))
        .collect();
    let mut expected: Vec<(&str, &str, usize, usize)> = FROZEN_ADMISSION_GAPS.to_vec();
    measured.sort();
    expected.sort();
    assert_eq!(
        measured, expected,
        "the admitted band's narrowings must be exactly the declared, named ones"
    );
    for (fixture, gap) in &all_gaps {
        assert!(
            !gap.reason.is_empty(),
            "{fixture}/{}: a narrowing must carry its reason",
            gap.class
        );
        assert!(
            gap.admitted < gap.source_population,
            "{fixture}/{}: a gap is only declared where the band really narrowed",
            gap.class
        );
        eprintln!(
            "CORPUS GAP {fixture} {}: {}/{} admitted — {}",
            gap.class, gap.admitted, gap.source_population, gap.reason
        );
    }
}

/// Non-vacuity of the oracle core: it reports a loss when the band's occurrence is
/// missing, a fabrication when the band adds one, and it is **not** fooled by an
/// occurrence the conserved source subtree already carries.
#[test]
fn oracle_is_not_fooled_by_source_conserved_occurrences() {
    let value = vec!["V".to_string()];
    let other = vec!["W".to_string()];
    let v = |values: &[&str]| {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    };

    // The band contributed its occurrence: emitted = source + band.
    let conserved = text_class(
        "syn",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &v(&["V", "V"]),
        &value,
        true,
    );
    assert_eq!(
        (conserved.admitted, conserved.conserved, conserved.lost),
        (1, 1, 0)
    );
    assert_eq!(conserved.fabricated, 0);
    assert_eq!(conserved.precision(), 1.0);
    assert_eq!(conserved.recall(), 1.0);

    // The same emission without the band's contribution: only the conserved source
    // occurrence remains.  A membership-only oracle would still report a clean 1.0
    // here; the occurrence-net reading reports the loss.
    let lost = text_class(
        "syn",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &v(&["V"]),
        &value,
        true,
    );
    assert_eq!((lost.conserved, lost.lost), (0, 1));
    assert!(lost.recall() < 1.0);

    // A fabricated band carrier: matching neither the admitted nor a source value.
    let fabricated = text_class(
        "syn",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &v(&["V", "V", "X"]),
        &value,
        true,
    );
    assert_eq!((fabricated.conserved, fabricated.fabricated), (1, 1));
    assert!(fabricated.precision() < 1.0);

    // A real source value the band did not admit and the emission re-carries is not
    // a fabrication: only the source exclusion set decides that.
    let source_only = text_class(
        "syn",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &v(&["V", "V", "W"]),
        &[value.clone(), other.clone()].concat(),
        true,
    );
    assert_eq!(
        source_only.fabricated, 0,
        "a source-carried value is not fabricated"
    );

    // The same for a binding class: the owner key is part of the identity, so a
    // source binding (no owner id) can never satisfy a synthesized binding.
    let admitted = vec![("author-0".to_string(), "aff0".to_string())];
    let emitted = vec![
        ("author-0".to_string(), "aff0".to_string()),
        (String::new(), "aff0".to_string()),
    ];
    let bindings = binding_class(
        "syn",
        CLASS_AUTHOR_AFFILIATION,
        1,
        &admitted,
        &emitted,
        &[],
        false,
    );
    assert_eq!((bindings.conserved, bindings.fabricated), (1, 1));
    let missing = binding_class(
        "syn",
        CLASS_AUTHOR_AFFILIATION,
        1,
        &admitted,
        &[(String::new(), "aff0".to_string())],
        &[],
        false,
    );
    assert_eq!((missing.conserved, missing.lost), (0, 1));
}

/// End-to-end non-vacuity: the oracle judges the emission bytes.  Removing the
/// admitted bibliography's own field group from the emission must show up as a
/// measured loss of the citation classes, proving the oracle reads the emission
/// and not the admitted band it was handed.
#[test]
fn oracle_reads_the_emission_and_reports_the_citation_loss() {
    let provenance = provenance();
    let (name, sha256) = PRESERVED_TEI[0];
    let xml = preserved_tei_source(name, sha256);
    let source = measure_source(name, sha256, &xml, &provenance);
    let admitted = build_admitted_band(&source);
    let candidate = compose_corpus_document(&source, &admitted, &xml);
    let emission =
        String::from_utf8(candidate.envelope.raw_tei().to_vec()).expect("emission is UTF-8");

    let healthy = oracle_class_profiles(&source, &xml, &admitted, &emission);
    for (_, class) in CORPUS_CITATION_ROLES {
        let profile = healthy
            .iter()
            .find(|profile| profile.class == class)
            .expect("every class is measured");
        assert!(
            profile.admitted > 0,
            "{class}: the corpus fixture must admit this class"
        );
        assert_eq!(
            profile.conserved, profile.admitted,
            "{class}: healthy emission"
        );
    }

    let open = emission
        .find("<noteGrp>")
        .expect("the admitted bibliography carries its field group");
    let close = emission
        .find("</noteGrp>")
        .expect("the field group is closed");
    let mut damaged = String::with_capacity(emission.len());
    damaged.push_str(&emission[..open]);
    damaged.push_str(&emission[close + "</noteGrp>".len()..]);

    let damaged_profiles = oracle_class_profiles(&source, &xml, &admitted, &damaged);
    for (_, class) in CORPUS_CITATION_ROLES {
        let profile = damaged_profiles
            .iter()
            .find(|profile| profile.class == class)
            .expect("every class is measured");
        assert_eq!(
            profile.conserved, 0,
            "{class}: removing the emitted field group must read as a total loss"
        );
        assert_eq!(
            profile.lost, profile.admitted,
            "{class}: lost must equal admitted"
        );
        assert!(profile.recall() < 1.0, "{class}: recall must drop");
    }
    // The header classes are untouched by that removal, so the oracle discriminates
    // between bands rather than reporting a blanket failure.
    let title = damaged_profiles
        .iter()
        .find(|profile| profile.class == CLASS_HEADER_TITLE)
        .expect("the header title class is measured");
    assert_eq!(
        title.conserved, title.admitted,
        "the header band is untouched"
    );
}

/// The guard: this file reaches exactly one composition entry point — the SUT —
/// and never the upstream composition helpers, so the admitted band cannot be a
/// co-simulation of the thing it measures.  The check scans this file's own bytes,
/// exactly like the Java-lift same-n guard it mirrors.
#[test]
fn expected_side_does_not_call_a_composition_entry_point() {
    let src = include_str!("native_fulltext_corpus_semantics.rs");
    let use_block: String = src
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect();
    for name in [
        "compose_header",
        "compose_references",
        "run_header_sequence",
        "run_reference_sequence",
    ] {
        assert!(
            !use_block.contains(name),
            "{name} must not be imported into the corpus expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the corpus expected builder"
        );
    }
    assert!(
        src.contains("compose_native_fulltext("),
        "the corpus expected builder must reach the single SUT"
    );
    // Non-vacuity of the guard itself: the source it scans is non-trivial, so a
    // zero is a real zero rather than an empty scan.
    assert!(
        src.len() > 4096,
        "the guard must scan the whole expected-builder source"
    );
}
