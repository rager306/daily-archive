//! M511-j7wz2f / S05 / T03 — the mutation matrix over the REAL preserved corpus.
//!
//! S05/T01 measured the independent semantic profile of the three sha256-pinned
//! preserved TEI and S05/T02 froze it as DATA.  A frozen measurement is only as
//! good as the mutations it rejects: on a healthy input the oracle is
//! indistinguishable from a blind one.  This file therefore applies the DATA
//! `mutation_matrix` one mutation at a time and proves each is caught by exactly
//! the path DATA declares — a typed refusal, a numeric precision deviation, a
//! byte-accounting violation, or an identity failure.
//!
//! The oracle below is mirrored from `native_fulltext_corpus_semantics.rs`
//! (S05/T01) so the mutations are judged by the same independent measurement; the
//! anti-drift test `mirrored_harness_reproduces_the_frozen_corpus_aggregate`
//! fails if the mirror and the frozen corpus aggregate (TP=573/FN=0/FP=0) ever
//! diverge.  No production refusal is weakened to let a mutation through: a
//! surviving or misclassified mutation is a FAIL.
//!
//! Shadow/offline only; D127 stays closed; `crates/da-parse/src` is not touched.

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
#[derive(Debug, Clone)]
struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the independent scanner reads from one real preserved document.
/// These values are the *source population* side of the profile; nothing here is
/// produced by the composer.
#[derive(Clone)]
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
#[derive(Clone)]
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
    /// The admitted-band narrowings.  Retained from the S05/T01 mirror for
    /// fidelity; this file reads the healthy profiles, not the gap reasons.
    #[allow(dead_code)]
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

/// Metrics retained from the S05/T01 mirror.  This file judges the mutated
/// profiles on `precision()` alone, but the full metric set stays for fidelity
/// with the oracle it mirrors.
#[allow(dead_code)]
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

// ═══════════════════════════════════════════════════════════════════════════
// S05/T03 — the mutation matrix, read from DATA and proven by NAME.
//
// The dictionary (id → declared detection path / class) lives in
// `data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.corpus_semantic_regression.mutation_matrix`
// (frozen by S05/T02).  This file supplies only the applier for each id; the
// EXPECTED detection is never re-stated here, so the test and the DATA carrier
// cannot drift into near-miss expectations.
//
// Every mutation is applied in the test surface only: an admitted band is
// mutated, or the emission bytes are mutated, or the manifest's byte scope is
// mutated, or the identity set is collapsed.  No production refusal is weakened
// to let a mutation through — a surviving or misclassified mutation fails.
// ═══════════════════════════════════════════════════════════════════════════

const COMPARISON_DATA: &str = "data/parse/comparison_contract.yaml";

/// The detection paths the DATA dictionary may declare.
const DETECTIONS: [&str; 4] = [
    "typed_refusal",
    "metric_deviation",
    "byte_accounting",
    "identity_failure",
];

/// The mutation ids this file implements.  The matrix test proves this set equals
/// the DATA set, so a declared mutation with no applier fails, and an applier with
/// no declaration fails too.  The EXPECTED detection for every id still comes from
/// DATA — this is the applier registry, not a second copy of the dictionary.
const IMPLEMENTED_IDS: [&str; 12] = [
    "drop_admitted_header_title",
    "drop_admitted_header_email",
    "drop_admitted_header_ids",
    "drop_admitted_author_affiliation",
    "substitute_citation_field_role",
    "truncate_admitted_references",
    "inject_fabricated_band_note_group",
    "inject_fabricated_header_title",
    "unbalanced_xml",
    "raw_ampersand_xml",
    "substitute_byte_scope",
    "collapse_identity_to_one_sha",
];

#[derive(Debug, Clone, Deserialize)]
struct MutationEntry {
    id: String,
    detection: String,
    #[serde(default)]
    refusal_class: Option<String>,
    #[serde(default)]
    metric: Option<String>,
    #[serde(default)]
    expectation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Aggregate {
    tp: usize,
    fp: usize,
    #[serde(rename = "fn")]
    fn_count: usize,
}

#[derive(Deserialize)]
struct RegressionBody {
    mutation_matrix: Vec<MutationEntry>,
    aggregate: Aggregate,
}

#[derive(Deserialize)]
struct ShadowBody {
    corpus_semantic_regression: RegressionBody,
}

#[derive(Deserialize)]
struct ShadowRoot {
    native_fulltext_composition_shadow: ShadowBody,
}

fn regression_body() -> RegressionBody {
    let text = std::fs::read_to_string(repo_root().join(COMPARISON_DATA))
        .unwrap_or_else(|error| panic!("FATAL {COMPARISON_DATA}: unreadable ({error})"));
    let root: ShadowRoot = serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("FATAL {COMPARISON_DATA}: unparsable ({error})"));
    assert!(
        root.native_fulltext_composition_shadow
            .corpus_semantic_regression
            .mutation_matrix
            .len()
            >= 11,
        "the DATA mutation matrix must declare at least eleven mutations"
    );
    root.native_fulltext_composition_shadow
        .corpus_semantic_regression
}

fn mutation_matrix() -> Vec<MutationEntry> {
    regression_body().mutation_matrix
}

/// The declared detection, spelled as one comparable name.
fn expected_detection(entry: &MutationEntry) -> String {
    match entry.detection.as_str() {
        "typed_refusal" | "byte_accounting" => entry.refusal_class.clone().unwrap_or_else(|| {
            panic!(
                "{}: {} declares no refusal_class",
                entry.id, entry.detection
            )
        }),
        "metric_deviation" => format!(
            "{}:{}",
            entry.metric.clone().unwrap_or_default(),
            entry.expectation.clone().unwrap_or_default()
        ),
        "identity_failure" => entry
            .expectation
            .clone()
            .unwrap_or_else(|| panic!("{}: identity_failure declares no expectation", entry.id)),
        other => panic!("{}: unknown detection path `{other}`", entry.id),
    }
}

/// The production refusal class as a comparable name.
fn error_class(error: &NativeFulltextCompositionError) -> String {
    match error {
        NativeFulltextCompositionError::SemanticFieldLoss { class, .. } => {
            class.token().to_string()
        }
        other => {
            let debug = format!("{other:?}");
            debug
                .split(['(', ' ', '{'])
                .next()
                .unwrap_or(&debug)
                .to_string()
        }
    }
}

fn metric_name(profile: &ClassProfile) -> String {
    if profile.precision() < 1.0 {
        "precision:below_one".to_string()
    } else {
        "precision:one".to_string()
    }
}

// ── the corpus under mutation ───────────────────────────────────────────────

/// One preserved document with its independently measured source, admitted band,
/// healthy composition and healthy per-class profile.
struct CorpusDoc {
    fixture: &'static str,
    xml: String,
    source: CorpusSource,
    admitted: AdmittedBand,
    candidate: NativeFulltextCandidate,
    emission: String,
    healthy: Vec<ClassProfile>,
}

fn corpus_docs() -> Vec<CorpusDoc> {
    let provenance = provenance();
    let mut docs = Vec::with_capacity(PRESERVED_TEI.len());
    for (name, sha256) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = measure_source(name, sha256, &xml, &provenance);
        let admitted = build_admitted_band(&source);
        let candidate = compose_corpus_document(&source, &admitted, &xml);
        let emission = String::from_utf8(candidate.envelope.raw_tei().to_vec())
            .unwrap_or_else(|error| panic!("{name}: emitted document is not UTF-8 ({error})"));
        let healthy = oracle_class_profiles(&source, &xml, &admitted, &emission);
        docs.push(CorpusDoc {
            fixture: name,
            xml,
            source,
            admitted,
            candidate,
            emission,
            healthy,
        });
    }
    docs
}

/// The first preserved document whose healthy measurement really admits `class`.
/// A mutation applied to a document that never carried the class would prove
/// nothing, so the choice is measured rather than assumed.
fn doc_admitting<'a>(docs: &'a [CorpusDoc], class: &str) -> &'a CorpusDoc {
    docs.iter()
        .find(|doc| {
            doc.healthy
                .iter()
                .any(|profile| profile.class == class && profile.admitted > 0)
        })
        .unwrap_or_else(|| panic!("no preserved document admits class `{class}`"))
}

fn corpus_input(doc: &CorpusDoc, band: &AdmittedBand, xml: &str) -> NativeFulltextCompositionInput {
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(
                GROBID_HTTP_BACKEND_ID,
                doc.source.profile_fingerprint.clone(),
            ),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: doc.source.sha256.clone(),
            profile_fingerprint: doc.source.profile_fingerprint.clone(),
            config_fingerprint: doc.source.config_fingerprint.clone(),
        },
        header: band.header.clone(),
        references: band.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: corpus_admissions(&doc.source.sha256),
        s17_lineage: None,
    }
}

struct Outcome {
    name: String,
    note: String,
}

/// Compose a mutated band and report the typed refusal class, or `Accepted` when
/// the mutation survives.  A refusal must hand back the byte-identical Java
/// envelope: a mutation may never produce a partial emission.
fn refusal_outcome(doc: &CorpusDoc, band: &AdmittedBand, xml: &str) -> Outcome {
    let input = corpus_input(doc, band, xml);
    let java = input.java_fulltext.raw_tei().to_vec();
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert_eq!(
                fallback.raw_tei(),
                java.as_slice(),
                "{}: a refusal must return the byte-identical Java envelope",
                doc.fixture
            );
            Outcome {
                name: error_class(&error),
                note: format!("{error:?}"),
            }
        }
        NativeFulltextCompositionOutcome::Accepted(candidate) => Outcome {
            name: "Accepted".to_string(),
            note: format!(
                "SURVIVED: accepted {} emitted bytes",
                candidate.envelope.raw_tei().len()
            ),
        },
    }
}

/// The DATA matrix's `raw_ampersand_xml` entry declares the structural scanner's
/// `MalformedXml` class.  A raw `&` in the document element's name is markup the
/// scanner must read, and it cannot read it.
///
/// The *other* ampersand class — a bare `&` in character data — is a
/// well-formedness refusal (`ComposedOutputNotWellFormed`) and is covered by the
/// existing `raw_ampersand_in_source_is_rejected_with_the_byte_identical_java_envelope`
/// contract test, not by this matrix entry.
fn raw_ampersand_in_markup(xml: &str) -> String {
    let bytes = xml.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'<'
            && bytes
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_alphabetic())
        {
            let mut out = String::with_capacity(xml.len() + 1);
            out.push_str(&xml[..index + 1]);
            out.push('&');
            out.push_str(&xml[index + 1..]);
            return out;
        }
    }
    panic!("the preserved document declares no element open tag");
}

fn unbalanced(xml: &str) -> String {
    let close = xml
        .find("</title>")
        .expect("the preserved document declares a <title>");
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..close]);
    out.push_str(&xml[close + "</title>".len()..]);
    out
}

fn observe_mutation(entry: &MutationEntry, docs: &[CorpusDoc]) -> Option<Outcome> {
    match entry.id.as_str() {
        "drop_admitted_header_title" => {
            let doc = doc_admitting(docs, CLASS_HEADER_TITLE);
            let mut band = doc.admitted.clone();
            if let HeaderCompositionOutcome::Accepted { composition } = &mut band.header {
                composition.candidate.fields.title = vec![String::new()];
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "drop_admitted_header_email" => {
            let doc = doc_admitting(docs, CLASS_HEADER_EMAIL);
            let mut band = doc.admitted.clone();
            if let HeaderCompositionOutcome::Accepted { composition } = &mut band.header {
                composition.candidate.fields.email = vec![String::new()];
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "drop_admitted_header_ids" => {
            let doc = doc_admitting(docs, CLASS_HEADER_IDS);
            let mut band = doc.admitted.clone();
            if let HeaderCompositionOutcome::Accepted { composition } = &mut band.header {
                composition.candidate.fields.ids = vec![String::new()];
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "drop_admitted_author_affiliation" => {
            let doc = doc_admitting(docs, CLASS_AUTHOR_AFFILIATION);
            let mut band = doc.admitted.clone();
            if let HeaderCompositionOutcome::Accepted { composition } = &mut band.header {
                let author = composition
                    .candidate
                    .authors
                    .iter_mut()
                    .find(|author| !author.affiliations.is_empty())
                    .expect("the document admits at least one author affiliation binding");
                author.affiliations[0].affiliation_span = AffiliationSpan { start: 0, end: 0 };
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "substitute_citation_field_role" => {
            let doc = doc_admitting(docs, CLASS_CITATION_DOI);
            let mut band = doc.admitted.clone();
            if let ReferenceCompositionOutcome::Accepted { composition } = &mut band.references {
                let field = composition
                    .candidate
                    .fields
                    .first_mut()
                    .expect("the admitted bibliography carries at least one citation field");
                // The role is substituted while the VALUE is preserved.  The
                // corpus emitter writes the role token verbatim, so a role swap
                // with a valid span is unobservable through the emitter; the
                // substituted role is therefore made observable through the
                // admitted span, which the composer validates before any splice.
                field.role = if field.role == CitationFieldRole::Url {
                    CitationFieldRole::Doi
                } else {
                    CitationFieldRole::Url
                };
                field.span = CitationSpan { start: 1, end: 0 };
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "truncate_admitted_references" => {
            let doc = doc_admitting(docs, CLASS_REFERENCE_DOMAIN);
            let mut band = doc.admitted.clone();
            if let ReferenceCompositionOutcome::Accepted { composition } = &mut band.references {
                composition.candidate.references.pop();
            }
            Some(refusal_outcome(doc, &band, &doc.xml))
        }
        "inject_fabricated_band_note_group" => {
            let doc = doc_admitting(docs, CLASS_CITATION_URL);
            let role = citation_field_role_token(CitationFieldRole::Url);
            let mutated = doc.emission.replacen(
                "<noteGrp>",
                &format!("<noteGrp><note role=\"{role}\">MUTANT FABRICATED CITATION 0001</note>"),
                1,
            );
            assert_ne!(
                mutated, doc.emission,
                "{}: the emitted bibliography must carry a note group to inject into",
                doc.fixture
            );
            let profiles = oracle_class_profiles(&doc.source, &doc.xml, &doc.admitted, &mutated);
            let profile = profiles
                .iter()
                .find(|profile| profile.class == CLASS_CITATION_URL)
                .expect("the url class is measured");
            Some(Outcome {
                name: metric_name(profile),
                note: format!(
                    "url precision={:.3} fabricated={}",
                    profile.precision(),
                    profile.fabricated
                ),
            })
        }
        "inject_fabricated_header_title" => {
            let doc = doc_admitting(docs, CLASS_HEADER_TITLE);
            let mutated = doc.emission.replacen(
                "</titleStmt>",
                "<title>MUTANT FABRICATED HEADER TITLE 0001</title></titleStmt>",
                1,
            );
            assert_ne!(
                mutated, doc.emission,
                "{}: the emitted header must carry a title statement to inject into",
                doc.fixture
            );
            let profiles = oracle_class_profiles(&doc.source, &doc.xml, &doc.admitted, &mutated);
            let profile = profiles
                .iter()
                .find(|profile| profile.class == CLASS_HEADER_TITLE)
                .expect("the header title class is measured");
            Some(Outcome {
                name: metric_name(profile),
                note: format!(
                    "title precision={:.3} fabricated={}",
                    profile.precision(),
                    profile.fabricated
                ),
            })
        }
        "unbalanced_xml" => {
            let doc = &docs[0];
            let mutated = unbalanced(&doc.xml);
            Some(refusal_outcome(doc, &doc.admitted, &mutated))
        }
        "raw_ampersand_xml" => {
            let doc = &docs[0];
            let mutated = raw_ampersand_in_markup(&doc.xml);
            Some(refusal_outcome(doc, &doc.admitted, &mutated))
        }
        "substitute_byte_scope" => {
            let doc = &docs[0];
            let declared = entry
                .refusal_class
                .clone()
                .expect("byte_accounting declares a refusal_class");
            let variant = declared.rsplit("::").next().unwrap_or(declared.as_str());
            let mut manifest = doc.candidate.manifest.clone();
            let victim = manifest
                .components
                .iter_mut()
                .find(|component| component.byte_scope == ByteCountScope::NativeFragment)
                .expect("the healthy manifest carries a native fragment component");
            victim.byte_scope = ByteCountScope::WholeDocumentReference;
            let violations = manifest.byte_accounting_violations();
            let present = violations
                .iter()
                .any(|violation| format!("{violation:?}").starts_with(variant));
            Some(Outcome {
                name: if present {
                    declared
                } else {
                    "byte_accounting:clean".to_string()
                },
                note: format!("{violations:?}"),
            })
        }
        "collapse_identity_to_one_sha" => {
            let healthy: Vec<&str> = PRESERVED_TEI.iter().map(|(_, sha256)| *sha256).collect();
            let collapsed = vec![PRESERVED_TEI[0].1; PRESERVED_TEI.len()];
            let unique_healthy = healthy.iter().collect::<BTreeSet<_>>().len();
            let unique_collapsed = collapsed.iter().collect::<BTreeSet<_>>().len();
            assert_eq!(
                unique_healthy, 3,
                "the healthy identity is three distinct digests"
            );
            Some(Outcome {
                name: if unique_collapsed != 3 {
                    "unique_source_sha256_not_three".to_string()
                } else {
                    "unique_source_sha256_three".to_string()
                },
                note: format!(
                    "healthy unique={unique_healthy} collapsed unique={unique_collapsed}"
                ),
            })
        }
        _ => None,
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

/// The dictionary is DATA, and every declared entry names a known path and a
/// non-empty class.
#[test]
fn mutation_matrix_is_read_from_data_and_declares_a_known_path() {
    let matrix = mutation_matrix();
    assert!(
        matrix.len() >= 11,
        "the DATA mutation matrix must declare at least eleven mutations"
    );
    let mut ids = BTreeSet::new();
    for entry in &matrix {
        assert!(!entry.id.is_empty(), "a mutation must be named");
        assert!(
            ids.insert(entry.id.clone()),
            "duplicate mutation id `{}`",
            entry.id
        );
        assert!(
            DETECTIONS.contains(&entry.detection.as_str()),
            "{}: unknown detection path `{}`",
            entry.id,
            entry.detection
        );
        assert!(
            !expected_detection(entry).is_empty(),
            "{}: the declared detection must name a class",
            entry.id
        );
    }
    assert_eq!(
        ids.len(),
        matrix.len(),
        "every declared mutation id must be unique"
    );
}

/// No undeclared mutation: the applier registry and the DATA dictionary are the
/// same set.
#[test]
fn every_implemented_mutation_is_declared_in_data() {
    let declared: BTreeSet<String> = mutation_matrix()
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    let implemented: BTreeSet<String> =
        IMPLEMENTED_IDS.iter().map(|id| (*id).to_string()).collect();
    assert_eq!(
        implemented, declared,
        "the implemented applier registry and the DATA dictionary must be the same set"
    );
}

/// Anti-drift guard for the harness mirrored from S05/T01: the copied oracle must
/// reproduce the frozen corpus aggregate exactly (TP=573, FN=0, FP=0).  If the
/// mirror and the frozen measurement diverge, this fails before any mutation is
/// judged.
#[test]
fn mirrored_harness_reproduces_the_frozen_corpus_aggregate() {
    let docs = corpus_docs();
    let (mut tp, mut fn_, mut fp) = (0usize, 0usize, 0usize);
    for doc in &docs {
        for profile in &doc.healthy {
            tp += profile.conserved;
            fn_ += profile.lost;
            fp += profile.fabricated;
        }
    }
    let aggregate = regression_body().aggregate;
    assert_eq!(
        (tp, fn_, fp),
        (aggregate.tp, aggregate.fn_count, aggregate.fp),
        "the mutation harness must reproduce the frozen corpus aggregate \
         (measured TP={tp} FN={fn_} FP={fp}, frozen TP={} FN={} FP={})",
        aggregate.tp,
        aggregate.fn_count,
        aggregate.fp
    );
    assert!(tp > 0, "the mirrored corpus metric must be non-vacuous");
}

/// The matrix: every declared mutation is applied, detected and classified by the
/// name DATA declares.  A surviving or misclassified mutation fails.
#[test]
fn mutation_matrix_every_declared_mutation_is_detected_by_its_declared_path() {
    let matrix = mutation_matrix();
    let docs = corpus_docs();
    let mut report: Vec<(String, String)> = Vec::with_capacity(matrix.len());
    for entry in &matrix {
        let observed = observe_mutation(entry, &docs).unwrap_or_else(|| {
            panic!(
                "{}: declared in DATA but no applier is implemented in this file",
                entry.id
            )
        });
        let expected = expected_detection(entry);
        eprintln!(
            "MUTATION {} -> {} (expected {}) [{}]",
            entry.id, observed.name, expected, observed.note
        );
        assert_eq!(
            observed.name, expected,
            "mutation `{}` must be detected exactly by its declared path \
             (observed `{}`, declared `{}`)",
            entry.id, observed.name, expected
        );
        report.push((entry.id.clone(), observed.name));
    }
    assert_eq!(
        report.len(),
        matrix.len(),
        "every declared mutation must be applied and detected"
    );
    eprintln!(
        "MUTATION MATRIX: {} declared mutations, {} detected, 0 survived",
        matrix.len(),
        report.len()
    );
}

/// Determinism: the same mutation twice yields an equal typed refusal and a
/// byte-identical Java envelope.
#[test]
fn typed_refusals_are_deterministic_and_return_the_byte_identical_java_envelope() {
    let docs = corpus_docs();
    let doc = doc_admitting(&docs, CLASS_HEADER_TITLE);
    let mut band = doc.admitted.clone();
    if let HeaderCompositionOutcome::Accepted { composition } = &mut band.header {
        composition.candidate.fields.title = vec![String::new()];
    }
    let input = corpus_input(doc, &band, &doc.xml);
    let java = input.java_fulltext.raw_tei().to_vec();
    let first = compose_native_fulltext(input.clone());
    let second = compose_native_fulltext(input.clone());
    assert_eq!(
        first, second,
        "the same mutation twice must give the same typed refusal"
    );
    match (&first, &second) {
        (
            NativeFulltextCompositionOutcome::Rejected {
                fallback: a,
                error: first_error,
            },
            NativeFulltextCompositionOutcome::Rejected {
                fallback: b,
                error: second_error,
            },
        ) => {
            assert_eq!(first_error, second_error);
            assert_eq!(a.raw_tei(), java.as_slice());
            assert_eq!(b.raw_tei(), java.as_slice());
            assert_eq!(
                error_class(first_error),
                CLASS_HEADER_TITLE,
                "the deterministic refusal keeps its declared class"
            );
        }
        _ => panic!("the mutated admitted band must be refused twice"),
    }
}
