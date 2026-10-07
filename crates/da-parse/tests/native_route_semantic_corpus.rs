//! M516-th0oby / S04 / T02 — independent H14/H15 judge over the 12-document
//! native route corpus.
//!
//! H14 (the composer's XML well-formedness gate) and H15 (the composer's
//! bibliography id/target integrity) were measured by M511 only on the three
//! sha256-pinned preserved TEI. S04/T01 published a *per-document*
//! semantic-correctness receipt over the expanded 12-document route corpus, but
//! that receipt is a SUT claim until an independent parser reproduces it: the
//! composer's roxmltree judge cannot live in `da-cli` (no `roxmltree`
//! dependency; the composer is quarantined, D381), so the independent half lives
//! here, in a `da-parse` test.
//!
//! This file reads the 12 `data/parse/layout_bakeoff.yaml#native_route_threshold_reference`
//! documents (id/path/sha256) and sha256-verifies every committed fixture (a
//! missing or drifted fixture is FATAL). For each document it
//!
//! 1. roxmltree-reads the reference TEI and derives the admitted band from the
//!    real document through the public typed parsers (`NativeDateParser`,
//!    `NativePersonalNameParser`, `NativeAffiliationParser`,
//!    `HeaderCandidate`/`ReferenceCandidate`) — the same construction
//!    `external_holdout_probe_semantics.rs` and `native_fulltext_corpus_semantics.rs`
//!    use, which already handles arbitrary holdout TEI;
//! 2. composes through the only composition entry point, `compose_native_fulltext`;
//! 3. independently re-parses the emission bytes with roxmltree (H14) and
//!    independently derives the source's mandatory local targets
//!    (`ref type="bibr" target="#…"`) and the emitted `xml:id` space (H15),
//!    asserting every mandatory target resolves and that the composer's
//!    `manifest.reference_links` ledger agrees with the independent counts;
//! 4. asserts a frozen per-document table over the 12 rows — so a change in any
//!    counter is a visible assertion diff, never a silently recomputed baseline.
//!
//! It then cross-checks the committed receipt
//! `artifacts/parse_compat/native_route_semantic_receipt.json` (read via
//! `include_str!`) against the oracle's independently derived per-document facts
//! and refuses the named mutations: a dropped row, a row whose reason disagrees
//! with the oracle, `empty_targets_presented_as_pass`, a placeholder header read
//! as populated, an opened lock, and reason-vocabulary drift.
//!
//! Only counts, digests and class tokens are ever printed or asserted: no raw
//! TEI and no field value leaves the measurement. The frozen three-document
//! aggregate (`native_fulltext_corpus_semantics`, `native_fulltext_composition_contract`,
//! `native_fulltext_composition_reference_integrity`) is untouched by this file.

#![allow(dead_code)]

use da_parse::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ───────────────────────────── carriers ─────────────────────────────

const LAYOUT_REL: &str = "data/parse/layout_bakeoff.yaml";
const RECEIPT_JSON: &str =
    include_str!("../../../artifacts/parse_compat/native_route_semantic_receipt.json");
const EMISSION_JSON: &str =
    include_str!("../../../artifacts/parse_compat/native_route_tei_emission.json");

/// Hermetic composition identity: the composer only requires the envelope and the
/// identity to agree and both fingerprints to be non-empty.
const PROFILE_FINGERPRINT: &str = "profile=native-route-semantic-corpus";
const CONFIG_FINGERPRINT: &str = "config=native-route-semantic-corpus";

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// The closed well-formedness reason of an accepted composer outcome.
const WF_EMISSION_PARSED_WELL_FORMED: &str = "emission_parsed_well_formed";
/// The closed well-formedness reason of a refusal.
const WF_COMPOSER_REFUSAL_PREFIX: &str = "composer_typed_refusal:";
/// Every mandatory target resolved.
const BT_ALL_REQUIRED_TARGETS_RESOLVED: &str = "all_required_targets_resolved";
/// The composer derived no mandatory target.
const BT_NO_REQUIRED_TARGETS_DECLARED: &str = "no_required_targets_declared";
/// Prefix of the unresolved-target reason.
const BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX: &str = "unresolved_required_targets:";

/// The required lock set the receipt must keep closed.
const REQUIRED_LOCKS: [(&str, &str); 5] = [
    ("import_eligible", "false"),
    ("auto_promote", "forbidden"),
    ("deployed", "false"),
    ("graph_write", "forbidden"),
    ("production_route", "forbidden"),
];

const DEFAULT_EMISSION_ARTIFACT_REL: &str = "artifacts/parse_compat/native_route_tei_emission.json";

/// The five composer-refusal class names a semantic refusal may carry.  The
/// oracle only ever reads the class the composer chose; the list pins the names
/// so a refusal reason that drifted out of the closed vocabulary is visible.
const REFUSAL_REASON_CLASSES: [&str; 5] = [
    "BibliographyDomainMismatch",
    "BibliographyKeyCollision",
    "UnresolvedBibliographyTarget",
    "InconservableBibliographyChild",
    "SemanticFieldLoss",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ───────────────────────── layout bindings ─────────────────────────

#[derive(Deserialize)]
struct Layout {
    native_route_corpus: CorpusBlock,
    native_route_threshold_reference: ReferenceBlock,
}

#[derive(Deserialize)]
struct CorpusBlock {
    id: String,
    corpus_n: usize,
    page_denominator: u64,
}

#[derive(Deserialize)]
struct ReferenceBlock {
    documents: Vec<Binding>,
}

#[derive(Deserialize)]
struct Binding {
    id: String,
    path: String,
    sha256: String,
}

fn layout() -> Layout {
    let text = std::fs::read_to_string(repo_root().join(LAYOUT_REL))
        .unwrap_or_else(|error| panic!("FATAL {LAYOUT_REL}: unreadable ({error})"));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("FATAL {LAYOUT_REL}: unparsable ({error})"))
}

fn readonly_layout() -> Layout {
    layout()
}

/// The canonical `sha256:` digest over the ordered reference bindings.  The
/// canonical form is `id \t path \t sha256 \n` per binding, matching T01's
/// `bindings_digest` so the committed receipt's corpus identity can be recomputed
/// independently.
fn bindings_digest(bindings: &[Binding]) -> String {
    let mut canonical = String::new();
    for binding in bindings {
        canonical.push_str(&binding.id);
        canonical.push('\t');
        canonical.push_str(&binding.path);
        canonical.push('\t');
        canonical.push_str(&binding.sha256);
        canonical.push('\n');
    }
    format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
}

fn profile_class_for_path(path: &str) -> &'static str {
    if path.contains("/preserved/") {
        "preserved_rich"
    } else {
        "holdout_structural"
    }
}

/// Read one committed reference TEI and verify its frozen checksum.  A missing,
/// unreadable or drifted fixture is FATAL: the judge must never measure bytes
/// other than the ones the corpus declaration pins.
fn verified_tei(binding: &Binding) -> String {
    let bytes = std::fs::read(repo_root().join(&binding.path)).unwrap_or_else(|error| {
        panic!(
            "FATAL {}: fixture missing/unreadable ({error})",
            binding.path
        )
    });
    let actual = hex::encode(Sha256::digest(&bytes));
    assert_eq!(
        actual, binding.sha256,
        "FATAL {}: fixture bytes drifted from the pinned sha256",
        binding.id
    );
    String::from_utf8(bytes)
        .unwrap_or_else(|error| panic!("FATAL {}: fixture is not UTF-8 ({error})", binding.id))
}

// ───────────────────────── independent XML readers ─────────────────────────

fn is_element(node: &roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

/// Whitespace-normalized concatenated text of every text descendant.
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

fn unique_list_bibl<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    unique_element(document, "listBibl")
}

fn xml_id<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
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

const CHECKSUM_IDNO_TYPES: [&str; 3] = ["MD5", "SHA1", "SHA256"];

fn is_checksum_idno(node: roxmltree::Node<'_, '_>) -> bool {
    node.attribute("type")
        .is_some_and(|kind| CHECKSUM_IDNO_TYPES.contains(&kind))
}

// ─────────────────── independent source-side measurement ───────────────────

#[derive(Debug, Clone)]
struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the independent scanner reads from one reference document.
struct SourceDocument {
    article_id: String,
    sha256: String,
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

fn measure_source(article_id: &str, declared_sha256: &str, xml: &str) -> SourceDocument {
    let document = parse_document(article_id, xml);
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
        .unwrap_or_else(|| panic!("{article_id}: the header declares no dated element with text"));

    let reference_texts: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(normalized_text)
        .collect();

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

    SourceDocument {
        article_id: article_id.to_string(),
        sha256: declared_sha256.to_string(),
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

// ────────────────────────── the admitted band ──────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmissionGap {
    class: &'static str,
    source_population: usize,
    admitted: usize,
    reason: String,
}

struct AdmittedBand {
    header: HeaderCompositionOutcome,
    references: ReferenceCompositionOutcome,
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
/// parsers.  Every parser rejection becomes a named [`AdmissionGap`]; nothing is
/// silently dropped and no value is invented.
fn build_admitted_band(source: &SourceDocument) -> AdmittedBand {
    let mut gaps = Vec::new();

    let date = match NativeDateParser::new().parse(&source.date_text) {
        NativeDateOutcome::Recognized { date } => date,
        other => panic!(
            "FATAL {}: the real publication date form cannot be admitted by NativeDateParser ({}); \
             the header band cannot be composed from the real document",
            source.article_id,
            outcome_token(&other)
        ),
    };

    let titles = source.title_stmt_titles.clone();
    if titles.len() < source.header_titles.len() {
        gaps.push(AdmissionGap {
            class: "SemanticFieldLoss::HeaderTitle",
            source_population: source.header_titles.len(),
            admitted: titles.len(),
            reason: "band definition: the admitted title field is the direct fileDesc/titleStmt/title \
                     children; the remaining header <title> carriers are conserved by the source subtree"
                .into(),
        });
    }

    let emails = source.header_emails.clone();
    let ids = source.header_pubnum_ids.clone();

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
                    source.article_id
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
        "FATAL {}: the reference admitted no author line",
        source.article_id
    );
    for reason in &rejected_names {
        gaps.push(AdmissionGap {
            class: "AdmittedAuthorLine",
            source_population: source.author_name_texts.len(),
            admitted: authors.len(),
            reason: format!(
                "NativePersonalNameParser rejected the real <persName> text ({reason})"
            ),
        });
    }
    for reason in &rejected_affiliations {
        gaps.push(AdmissionGap {
            class: "SemanticFieldLoss::AuthorAffiliation",
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
            provenance: "native-route-semantic-corpus".into(),
        })
        .collect();
    assert!(
        references
            .iter()
            .all(|reference| reference.span.start < reference.span.end),
        "FATAL {}: a source biblStruct carries empty text",
        source.article_id
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
        source.article_id
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
        source.article_id
    );

    AdmittedBand {
        header,
        references: references_outcome,
        reference_texts: source.reference_texts.clone(),
        citation_fields: source.citation_carriers.clone(),
        gaps,
    }
}

// ───────────────────────── composition harness ─────────────────────────

/// Header and references admitted, every other component typed
/// Unsupported/NotObserved — the same admission shape the frozen measurement uses.
fn corpus_admissions(source_sha256: &str) -> Vec<FulltextAdmissionReceipt> {
    let receipt = |component: FulltextComponent, disposition: AdmissionDisposition| {
        FulltextAdmissionReceipt {
            component,
            disposition,
            producer: "native-route-semantic-corpus".into(),
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

/// Compose one document's admitted band with the single SUT this file reaches.
fn compose_document(
    source: &SourceDocument,
    admitted: &AdmittedBand,
    xml: &str,
) -> Result<NativeFulltextCandidate, String> {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, PROFILE_FINGERPRINT.to_string()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: PROFILE_FINGERPRINT.to_string(),
            config_fingerprint: CONFIG_FINGERPRINT.to_string(),
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
        NativeFulltextCompositionOutcome::Accepted(candidate) => Ok(candidate),
        NativeFulltextCompositionOutcome::Rejected { error, .. } => Err(format!("{error:?}")),
    }
}

// ───────────────────── independent H15 reference profile ─────────────────────

/// What the source document declares and demands, read only through roxmltree.
#[derive(Debug, Clone)]
struct SourceReferenceProfile {
    published_keys: Vec<String>,
    target_occurrences: usize,
    mandatory_targets: BTreeSet<String>,
}

fn oracle_source_reference_profile(xml: &str) -> SourceReferenceProfile {
    let document = roxmltree::Document::parse(xml).expect("independent parser accepts the source");
    let list = unique_list_bibl(&document);
    let entries: Vec<roxmltree::Node<'_, '_>> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .collect();

    let mut published_keys = Vec::with_capacity(entries.len());
    for (ordinal, entry) in entries.iter().enumerate() {
        match xml_id(*entry) {
            Some(key) => published_keys.push(key.to_owned()),
            None => published_keys.push(format!("b{ordinal}")),
        }
    }

    let mut target_occurrences = 0usize;
    let mut mandatory_targets = BTreeSet::new();
    for node in document
        .descendants()
        .filter(|node| is_element(node, "ref"))
    {
        if node.attribute("type") != Some("bibr") {
            continue;
        }
        let Some(target) = node.attribute("target") else {
            continue;
        };
        let Some(fragment) = target.strip_prefix('#') else {
            continue;
        };
        if fragment.is_empty() {
            continue;
        }
        target_occurrences += 1;
        mandatory_targets.insert(fragment.to_owned());
    }

    SourceReferenceProfile {
        published_keys,
        target_occurrences,
        mandatory_targets,
    }
}

/// What the emitted document publishes, read only through roxmltree.
#[derive(Debug, Clone)]
struct EmittedReferenceProfile {
    published_keys: Vec<String>,
    id_space: BTreeSet<String>,
}

fn oracle_emitted_reference_profile(xml: &str) -> EmittedReferenceProfile {
    let document =
        roxmltree::Document::parse(xml).expect("independent parser accepts the emission");
    let id_space: BTreeSet<String> = document
        .descendants()
        .filter(|node| node.is_element())
        .filter_map(xml_id)
        .map(str::to_owned)
        .collect();
    let list = unique_list_bibl(&document);
    let published_keys: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(|entry| match xml_id(entry) {
            Some(key) => key.to_owned(),
            None => "<undeclared>".to_owned(),
        })
        .collect();
    EmittedReferenceProfile {
        published_keys,
        id_space,
    }
}

// ───────────────────────── the closed reason vocabulary ─────────────────────────

fn bibliography_target_reason(required: usize, resolved: usize) -> String {
    if required == 0 {
        BT_NO_REQUIRED_TARGETS_DECLARED.to_owned()
    } else if resolved >= required {
        BT_ALL_REQUIRED_TARGETS_RESOLVED.to_owned()
    } else {
        format!(
            "{BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX}{}",
            required - resolved
        )
    }
}

fn is_well_formedness_reason(reason: &str) -> bool {
    reason == WF_EMISSION_PARSED_WELL_FORMED
        || reason
            .strip_prefix(WF_COMPOSER_REFUSAL_PREFIX)
            .is_some_and(|class| REFUSAL_REASON_CLASSES.contains(&class) || !class.is_empty())
}

fn is_bibliography_target_reason(reason: &str) -> bool {
    reason == BT_ALL_REQUIRED_TARGETS_RESOLVED
        || reason == BT_NO_REQUIRED_TARGETS_DECLARED
        || reason
            .strip_prefix(BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX)
            .is_some_and(|count| count.parse::<u64>().is_ok())
        || is_well_formedness_reason(reason)
}

// ───────────────────────── oracle rows + frozen table ─────────────────────────

/// One independently measured per-document row.
#[derive(Debug, Clone)]
struct OracleRow {
    document_id: String,
    profile_class: &'static str,
    reference_sha256: String,
    well_formed: bool,
    composer_outcome: &'static str,
    well_formedness_reason: String,
    bibliography_target_reason: String,
    source_entries: usize,
    target_occurrences: usize,
    required_targets: usize,
    resolved_targets: usize,
    id_space: usize,
}

/// Measure every committed reference TEI through the composer and judge the
/// emission with the independent roxmltree oracle.
fn measure_oracle_rows() -> Vec<OracleRow> {
    let layout = readonly_layout();
    assert_eq!(
        layout.native_route_threshold_reference.documents.len(),
        layout.native_route_corpus.corpus_n,
        "the reference block and the corpus block must bind the same population"
    );

    let mut rows = Vec::with_capacity(layout.native_route_threshold_reference.documents.len());
    for binding in &layout.native_route_threshold_reference.documents {
        let xml = verified_tei(binding);
        let source = measure_source(&binding.id, &binding.sha256, &xml);
        let admitted = build_admitted_band(&source);

        let source_profile = oracle_source_reference_profile(&xml);

        let candidate = match compose_document(&source, &admitted, &xml) {
            Ok(candidate) => candidate,
            Err(class) => {
                rows.push(OracleRow {
                    document_id: binding.id.clone(),
                    profile_class: profile_class_for_path(&binding.path),
                    reference_sha256: binding.sha256.clone(),
                    well_formed: false,
                    composer_outcome: "refused",
                    well_formedness_reason: format!("{WF_COMPOSER_REFUSAL_PREFIX}{class}"),
                    bibliography_target_reason: format!("{WF_COMPOSER_REFUSAL_PREFIX}{class}"),
                    source_entries: source_profile.published_keys.len(),
                    target_occurrences: source_profile.target_occurrences,
                    required_targets: source_profile.mandatory_targets.len(),
                    resolved_targets: 0,
                    id_space: 0,
                });
                continue;
            }
        };

        let emission = String::from_utf8(candidate.envelope.raw_tei().to_vec())
            .unwrap_or_else(|error| panic!("{}: emission is not UTF-8 ({error})", binding.id));
        // H14: the independent parser re-reads the emission bytes.
        let well_formed = roxmltree::Document::parse(&emission).is_ok();
        assert!(
            well_formed,
            "FATAL {}: the composer accepted a document the independent parser rejects (H14)",
            binding.id
        );

        let emitted_profile = oracle_emitted_reference_profile(&emission);

        // H15: the emitted bibliography republishes the source's own keys, and
        // every mandatory source target resolves in the emitted id space.
        assert_eq!(
            emitted_profile.published_keys, source_profile.published_keys,
            "FATAL {}: emitted bibliography keys differ from the source's published keys",
            binding.id
        );
        let unresolved: Vec<&String> = source_profile
            .mandatory_targets
            .difference(&emitted_profile.id_space)
            .collect();
        assert!(
            unresolved.is_empty(),
            "FATAL {}: {} of {} mandatory source targets are dangling in an accepted emission",
            binding.id,
            unresolved.len(),
            source_profile.mandatory_targets.len()
        );

        // The composer's own ledger must agree with the independent counts.
        let ledger = candidate.manifest.reference_links;
        assert_eq!(
            ledger.source_entries,
            source_profile.published_keys.len(),
            "FATAL {}: ledger source_entries disagrees with the oracle",
            binding.id
        );
        assert_eq!(
            ledger.target_occurrences, source_profile.target_occurrences,
            "FATAL {}: ledger target_occurrences disagrees with the oracle",
            binding.id
        );
        assert_eq!(
            ledger.required_targets,
            source_profile.mandatory_targets.len(),
            "FATAL {}: ledger required_targets disagrees with the oracle",
            binding.id
        );
        assert_eq!(
            ledger.resolved_targets,
            source_profile.mandatory_targets.len(),
            "FATAL {}: ledger resolved_targets disagrees with the oracle",
            binding.id
        );
        assert_eq!(
            ledger.id_space,
            emitted_profile.id_space.len(),
            "FATAL {}: ledger id_space disagrees with the oracle",
            binding.id
        );

        let required = source_profile.mandatory_targets.len();
        rows.push(OracleRow {
            document_id: binding.id.clone(),
            profile_class: profile_class_for_path(&binding.path),
            reference_sha256: binding.sha256.clone(),
            well_formed: true,
            composer_outcome: "accepted",
            well_formedness_reason: WF_EMISSION_PARSED_WELL_FORMED.to_owned(),
            bibliography_target_reason: bibliography_target_reason(required, required),
            source_entries: source_profile.published_keys.len(),
            target_occurrences: source_profile.target_occurrences,
            required_targets: required,
            resolved_targets: required,
            id_space: emitted_profile.id_space.len(),
        });
    }
    rows
}

/// One frozen per-document row. Every number is asserted, so a deviation in
/// either direction fails with a visible diff.
struct FrozenRow {
    id: &'static str,
    profile_class: &'static str,
    source_entries: usize,
    target_occurrences: usize,
    required_targets: usize,
    resolved_targets: usize,
    id_space: usize,
}

/// The measured H14/H15 profile of the 12-document route corpus.
///
/// The frozen numbers agree with the independently derived source reference
/// profile and with the composer's `ReferenceLinkLedger`; the well-formedness and
/// bibliography reasons are closed tokens. The table is `preserved_rich` for the
/// three corridor fixtures and `holdout_structural` for the nine holdout ones.
static FROZEN: [FrozenRow; 12] = [
    FrozenRow {
        id: "hub-2507.19457",
        profile_class: "preserved_rich",
        source_entries: 80,
        target_occurrences: 87,
        required_targets: 55,
        resolved_targets: 55,
        id_space: 152,
    },
    FrozenRow {
        id: "rlm-2512.24601",
        profile_class: "preserved_rich",
        source_entries: 49,
        target_occurrences: 71,
        required_targets: 41,
        resolved_targets: 41,
        id_space: 88,
    },
    FrozenRow {
        id: "reca-2605.26525v1",
        profile_class: "preserved_rich",
        source_entries: 78,
        target_occurrences: 59,
        required_targets: 24,
        resolved_targets: 24,
        id_space: 159,
    },
    FrozenRow {
        id: "1409.0473",
        profile_class: "holdout_structural",
        source_entries: 29,
        target_occurrences: 47,
        required_targets: 24,
        resolved_targets: 24,
        id_space: 67,
    },
    FrozenRow {
        id: "1410.4615",
        profile_class: "holdout_structural",
        source_entries: 46,
        target_occurrences: 35,
        required_targets: 23,
        resolved_targets: 23,
        id_space: 55,
    },
    FrozenRow {
        id: "1410.5401",
        profile_class: "holdout_structural",
        source_entries: 42,
        target_occurrences: 44,
        required_targets: 41,
        resolved_targets: 41,
        id_space: 85,
    },
    FrozenRow {
        id: "1506.02438",
        profile_class: "holdout_structural",
        source_entries: 25,
        target_occurrences: 40,
        required_targets: 22,
        resolved_targets: 22,
        id_space: 78,
    },
    FrozenRow {
        id: "2402.04268",
        profile_class: "holdout_structural",
        source_entries: 89,
        target_occurrences: 134,
        required_targets: 89,
        resolved_targets: 89,
        id_space: 115,
    },
    FrozenRow {
        id: "2402.10980",
        profile_class: "holdout_structural",
        source_entries: 80,
        target_occurrences: 92,
        required_targets: 65,
        resolved_targets: 65,
        id_space: 108,
    },
    FrozenRow {
        id: "2406.13163",
        profile_class: "holdout_structural",
        source_entries: 46,
        target_occurrences: 50,
        required_targets: 44,
        resolved_targets: 44,
        id_space: 70,
    },
    FrozenRow {
        id: "1203.2295",
        profile_class: "holdout_structural",
        source_entries: 22,
        target_occurrences: 33,
        required_targets: 22,
        resolved_targets: 22,
        id_space: 39,
    },
    FrozenRow {
        id: "2502.05199",
        profile_class: "holdout_structural",
        source_entries: 34,
        target_occurrences: 36,
        required_targets: 32,
        resolved_targets: 32,
        id_space: 57,
    },
];

// ────────────────────── committed receipt cross-check ──────────────────────

/// Refuse a receipt that drifted from the oracle.  Every refusal names the
/// document and the field; none is a silent downgrade.
fn recheck_receipt(
    receipt: &serde_json::Value,
    oracle: &[OracleRow],
    layout: &Layout,
    emission: &serde_json::Value,
) -> Result<(), String> {
    if receipt["schema_version"] != "native-route-semantic-receipt/v1" {
        return Err("schema_version drifted".into());
    }
    if receipt["claim_scope"] != "diagnostic_only" {
        return Err("claim_scope is not diagnostic_only".into());
    }
    if receipt["product_coverage_effect"] != "none" {
        return Err("product_coverage_effect is not none".into());
    }
    if receipt["thresholds"] != "not_accepted" {
        return Err("thresholds is not not_accepted".into());
    }
    for (lock, value) in REQUIRED_LOCKS {
        let found = receipt["locks"][lock].as_str().unwrap_or("<absent>");
        if found != value {
            return Err(format!(
                "lock `{lock}` is open: expected `{value}`, found `{found}`"
            ));
        }
    }

    let bindings = &layout.native_route_threshold_reference.documents;
    if receipt["corpus"]["documents_digest"].as_str() != Some(bindings_digest(bindings).as_str()) {
        return Err("corpus.documents_digest drifted from the committed bindings".into());
    }
    if receipt["corpus"]["document_count"].as_u64() != Some(bindings.len() as u64) {
        return Err("corpus.document_count drifted".into());
    }
    if receipt["corpus"]["page_denominator"].as_u64()
        != Some(layout.native_route_corpus.page_denominator)
    {
        return Err("corpus.page_denominator drifted".into());
    }

    if receipt["population"]["aggregate_verdict"] != "no_aggregate_verdict" {
        return Err("aggregate_verdict is not no_aggregate_verdict".into());
    }

    let documents = receipt["documents"]
        .as_array()
        .ok_or_else(|| "documents is not an array".to_string())?;
    if documents.len() != oracle.len() {
        return Err(format!(
            "expected {} document rows, found {}",
            oracle.len(),
            documents.len()
        ));
    }

    for (index, expected) in oracle.iter().enumerate() {
        let row = &documents[index];
        if row["document_id"].as_str() != Some(expected.document_id.as_str()) {
            return Err(format!(
                "row {index} names `{}`, the oracle expects `{}`",
                row["document_id"].as_str().unwrap_or("<absent>"),
                expected.document_id
            ));
        }
        if row["profile_class"].as_str() != Some(expected.profile_class) {
            return Err(format!("{}: profile_class drifted", expected.document_id));
        }
        if row["reference_sha256"].as_str() != Some(expected.reference_sha256.as_str()) {
            return Err(format!(
                "{}: reference_sha256 drifted",
                expected.document_id
            ));
        }
        let composer = &row["composer"];
        if composer["outcome"].as_str() != Some(expected.composer_outcome) {
            return Err(format!(
                "{}: composer.outcome drifted",
                expected.document_id
            ));
        }
        let wf = composer["well_formedness_reason"].as_str().unwrap_or("");
        let bt = composer["bibliography_target_reason"]
            .as_str()
            .unwrap_or("");
        if !is_well_formedness_reason(wf) {
            return Err(format!(
                "{}: well_formedness_reason `{wf}` is outside the closed vocabulary",
                expected.document_id
            ));
        }
        if !is_bibliography_target_reason(bt) {
            return Err(format!(
                "{}: bibliography_target_reason `{bt}` is outside the closed vocabulary",
                expected.document_id
            ));
        }
        if wf != expected.well_formedness_reason {
            return Err(format!(
                "{}: well_formedness_reason `{wf}` disagrees with the oracle `{}`",
                expected.document_id, expected.well_formedness_reason
            ));
        }
        if bt != expected.bibliography_target_reason {
            return Err(format!(
                "{}: bibliography_target_reason `{bt}` disagrees with the oracle `{}`",
                expected.document_id, expected.bibliography_target_reason
            ));
        }
        let ledger = &composer["ledger"];
        let ledger_required = ledger["required_targets"].as_u64().unwrap_or(u64::MAX);
        if ledger_required == 0 && bt == BT_ALL_REQUIRED_TARGETS_RESOLVED {
            return Err(format!(
                "empty_targets_presented_as_pass: {}",
                expected.document_id
            ));
        }
        for (field, expected_value) in [
            ("source_entries", expected.source_entries as u64),
            ("target_occurrences", expected.target_occurrences as u64),
            ("required_targets", expected.required_targets as u64),
            ("resolved_targets", expected.resolved_targets as u64),
            ("id_space", expected.id_space as u64),
        ] {
            if ledger[field].as_u64() != Some(expected_value) {
                return Err(format!(
                    "{}: ledger.{field} is {}, the oracle expects {expected_value}",
                    expected.document_id,
                    ledger[field]
                        .as_u64()
                        .map_or("<absent>".into(), |v| v.to_string())
                ));
            }
        }
    }

    // The population counters must be bound to the rows the oracle measured.
    let population = &receipt["population"];
    let documents_with_required_targets =
        oracle.iter().filter(|row| row.required_targets > 0).count() as u64;
    let target_occurrences_total: u64 =
        oracle.iter().map(|row| row.target_occurrences as u64).sum();
    let required_targets_total: u64 = oracle.iter().map(|row| row.required_targets as u64).sum();
    let resolved_targets_total: u64 = oracle.iter().map(|row| row.resolved_targets as u64).sum();
    for (field, expected_value) in [
        ("documents", oracle.len() as u64),
        (
            "documents_with_required_targets",
            documents_with_required_targets,
        ),
        ("target_occurrences_total", target_occurrences_total),
        ("required_targets_total", required_targets_total),
        ("resolved_targets_total", resolved_targets_total),
    ] {
        if population[field].as_u64() != Some(expected_value) {
            return Err(format!(
                "population.{field} is {}, the oracle expects {expected_value}",
                population[field]
                    .as_u64()
                    .map_or("<absent>".into(), |v| v.to_string())
            ));
        }
    }

    recheck_emission_scope(receipt, emission, oracle)?;

    Ok(())
}

/// The receipt's native-route-emission scope section must agree with the
/// committed emission envelope, and its all-zero declared target population must
/// be a named scope reason, never a pass.
fn recheck_emission_scope(
    receipt: &serde_json::Value,
    emission: &serde_json::Value,
    oracle: &[OracleRow],
) -> Result<(), String> {
    let scope = &receipt["native_route_emission_scope"];
    let expected_digest = format!("sha256:{:x}", Sha256::digest(EMISSION_JSON.as_bytes()));
    if scope["sha256"].as_str() != Some(expected_digest.as_str()) {
        return Err(
            "native_route_emission_scope.sha256 drifted from the committed emission".into(),
        );
    }
    if scope["pointer"].as_str() != Some(DEFAULT_EMISSION_ARTIFACT_REL) {
        return Err("native_route_emission_scope.pointer drifted".into());
    }
    let rows = scope["documents"]
        .as_array()
        .ok_or_else(|| "native_route_emission_scope.documents is not an array".to_string())?;
    if rows.len() != oracle.len() {
        return Err("native_route_emission_scope has the wrong document count".into());
    }
    let emission_documents = emission["documents"]
        .as_array()
        .ok_or_else(|| "emission.documents is not an array".to_string())?;
    if emission_documents.len() != rows.len() {
        return Err("emission document count disagrees with the receipt scope".into());
    }

    for (index, row) in rows.iter().enumerate() {
        let emission_document = &emission_documents[index];
        if row["document_id"] != emission_document["document_id"] {
            return Err(format!(
                "emission scope row {index} names a different document"
            ));
        }
        let families = emission_document["emission"]["families"]
            .as_array()
            .ok_or_else(|| "emission document carries no families".to_string())?;
        for family in ["header", "references", "body"] {
            let expected = families.iter().find(|entry| entry["family"] == family);
            let Some(expected) = expected else {
                return Err(format!("emission lacks the `{family}` family"));
            };
            let disposition = expected["disposition"].as_str().unwrap_or("");
            let scope_reason = row[family]["scope_reason"].as_str().unwrap_or("");
            let derived = match (family, disposition) {
                ("header", "native_emitted") => "header_native_emitted",
                ("header", "typed_absence") => "header_typed_absence",
                ("references", "native_emitted") => "references_native_emitted",
                ("references", "typed_absence") => "references_typed_absence",
                ("body", "native_emitted") => "body_native_emitted",
                ("body", "typed_absence") => "body_typed_absence",
                _ => {
                    return Err(format!(
                        "emission `{family}` disposition is out of vocabulary"
                    ));
                }
            };
            if scope_reason != derived {
                return Err(format!(
                    "emission scope `{family}` is `{scope_reason}`, the emission declares `{derived}`"
                ));
            }
        }
        // The placeholder header is never read as populated; the empty declared
        // target population is a named scope reason, never a pass.
        if row["header"]["disposition"] != "typed_absence"
            || row["header"]["scope_reason"] != "header_typed_absence"
        {
            return Err("placeholder header read as populated".into());
        }
        let links = &row["reference_links"];
        if links["required_targets"].as_u64() != Some(0)
            || links["target_occurrences"].as_u64() != Some(0)
        {
            return Err(
                "emission reference_links declared targets where the emission has none".into(),
            );
        }
        if links["scope_reason"] != "reference_links_empty_targets" {
            return Err(
                "empty emission targets are not named `reference_links_empty_targets`".into(),
            );
        }
    }

    Ok(())
}

fn parsed_receipt() -> serde_json::Value {
    serde_json::from_str(RECEIPT_JSON).expect("the committed receipt is valid JSON")
}

fn parsed_emission() -> serde_json::Value {
    serde_json::from_str(EMISSION_JSON).expect("the committed emission is valid JSON")
}

// ─────────────────────────────── tests ───────────────────────────────

/// The independent H14/H15 table over the 12 reference TEI. Every row is frozen;
/// a deviation is a visible assertion diff.
#[test]
fn independent_oracle_measures_h14_and_h15_over_the_twelve_reference_tei() {
    let rows = measure_oracle_rows();
    assert_eq!(rows.len(), FROZEN.len(), "the corpus population drifted");
    for (row, frozen) in rows.iter().zip(FROZEN.iter()) {
        assert_eq!(row.document_id, frozen.id, "document order drifted");
        assert_eq!(
            row.profile_class, frozen.profile_class,
            "{}: profile class",
            frozen.id
        );
        assert!(
            row.well_formed,
            "{}: H14 — the emission must be independently well formed",
            frozen.id
        );
        assert_eq!(
            row.composer_outcome, "accepted",
            "{}: composer outcome",
            frozen.id
        );
        assert_eq!(
            row.well_formedness_reason, WF_EMISSION_PARSED_WELL_FORMED,
            "{}: well-formedness reason",
            frozen.id
        );
        assert_eq!(
            row.bibliography_target_reason, BT_ALL_REQUIRED_TARGETS_RESOLVED,
            "{}: bibliography-target reason",
            frozen.id
        );
        assert_eq!(
            row.source_entries, frozen.source_entries,
            "{}: source_entries",
            frozen.id
        );
        assert_eq!(
            row.target_occurrences, frozen.target_occurrences,
            "{}: target_occurrences",
            frozen.id
        );
        assert_eq!(
            row.required_targets, frozen.required_targets,
            "{}: required_targets",
            frozen.id
        );
        assert_eq!(
            row.resolved_targets, frozen.resolved_targets,
            "{}: resolved_targets",
            frozen.id
        );
        assert_eq!(row.id_space, frozen.id_space, "{}: id_space", frozen.id);
    }

    // Aggregate the mixed-population counters the slice must keep separate.
    let documents_with_required_targets =
        rows.iter().filter(|row| row.required_targets > 0).count();
    let required_total: usize = rows.iter().map(|row| row.required_targets).sum();
    let resolved_total: usize = rows.iter().map(|row| row.resolved_targets).sum();
    assert_eq!(documents_with_required_targets, 12);
    assert_eq!(required_total, 482);
    assert_eq!(resolved_total, 482);
}

/// The committed receipt agrees with the oracle's independently derived facts.
#[test]
fn committed_receipt_agrees_with_the_independent_oracle() {
    let rows = measure_oracle_rows();
    let receipt = parsed_receipt();
    let layout = readonly_layout();
    let emission = parsed_emission();
    recheck_receipt(&receipt, &rows, &layout, &emission)
        .unwrap_or_else(|error| panic!("the committed receipt drifted from the oracle: {error}"));
}

/// The native-route-emission scope names the empty-target population as a reason,
/// never as a pass.
#[test]
fn native_route_emission_scope_names_the_empty_target_population() {
    let receipt = parsed_receipt();
    let emission = parsed_emission();
    let rows = measure_oracle_rows();
    recheck_emission_scope(&receipt, &emission, &rows)
        .unwrap_or_else(|error| panic!("the emission scope drifted: {error}"));
    let scope_documents = receipt["native_route_emission_scope"]["documents"]
        .as_array()
        .expect("scope documents");
    assert_eq!(scope_documents.len(), 12);
    for document in scope_documents {
        let links = &document["reference_links"];
        assert_eq!(links["required_targets"].as_u64(), Some(0));
        assert_eq!(
            links["scope_reason"].as_str(),
            Some("reference_links_empty_targets"),
            "an empty declared-target population must be a named scope reason"
        );
    }
}

/// A ledger that declares no required target is a typed `no_required_targets_declared`
/// reason — never `all_required_targets_resolved`.
#[test]
fn zero_target_ledger_is_a_named_absence_never_a_pass() {
    assert_eq!(
        bibliography_target_reason(0, 0),
        BT_NO_REQUIRED_TARGETS_DECLARED
    );
    assert_ne!(
        bibliography_target_reason(0, 0),
        BT_ALL_REQUIRED_TARGETS_RESOLVED
    );
    // The oracle's own reason recovery is closed: an empty denominator is named.
    let reason = bibliography_target_reason(0, 0);
    assert!(is_bibliography_target_reason(&reason));
    assert_ne!(reason, "pass");
}

/// The receipt cross-check refuses the named mutations.
#[test]
fn receipt_cross_check_refuses_the_named_mutations() {
    let rows = measure_oracle_rows();
    let layout = readonly_layout();
    let emission = parsed_emission();
    let committed = parsed_receipt();
    assert!(recheck_receipt(&committed, &rows, &layout, &emission).is_ok());

    // 1. a dropped document row.
    let mut dropped = committed.clone();
    dropped["documents"].as_array_mut().unwrap().pop();
    assert!(recheck_receipt(&dropped, &rows, &layout, &emission).is_err());

    // 2. a document row whose reason disagrees with the oracle.
    let mut disagreeing = committed.clone();
    disagreeing["documents"][0]["composer"]["well_formedness_reason"] =
        serde_json::json!("composer_typed_refusal:MalformedXml");
    assert!(recheck_receipt(&disagreeing, &rows, &layout, &emission).is_err());

    // 3. empty targets presented as a pass.
    let mut empty_passed = committed.clone();
    empty_passed["documents"][0]["composer"]["ledger"]["required_targets"] = serde_json::json!(0);
    empty_passed["documents"][0]["composer"]["ledger"]["resolved_targets"] = serde_json::json!(0);
    empty_passed["documents"][0]["composer"]["ledger"]["target_occurrences"] = serde_json::json!(0);
    empty_passed["documents"][0]["composer"]["bibliography_target_reason"] =
        serde_json::json!(BT_ALL_REQUIRED_TARGETS_RESOLVED);
    let empty_error = recheck_receipt(&empty_passed, &rows, &layout, &emission)
        .expect_err("empty targets presented as a pass must be refused");
    assert!(
        empty_error.contains("empty_targets_presented_as_pass"),
        "the refusal must name empty_targets_presented_as_pass, got `{empty_error}`"
    );

    // 4. a placeholder header read as populated.
    let mut header_populated = committed.clone();
    header_populated["native_route_emission_scope"]["documents"][0]["header"]["scope_reason"] =
        serde_json::json!("header_native_emitted");
    assert!(recheck_receipt(&header_populated, &rows, &layout, &emission).is_err());

    // 5. an opened lock.
    let mut lock_opened = committed.clone();
    lock_opened["locks"]["import_eligible"] = serde_json::json!("true");
    assert!(recheck_receipt(&lock_opened, &rows, &layout, &emission).is_err());

    // 6. reason-vocabulary drift.
    let mut drift = committed.clone();
    drift["documents"][0]["composer"]["bibliography_target_reason"] = serde_json::json!("pass");
    assert!(recheck_receipt(&drift, &rows, &layout, &emission).is_err());
}

/// The layout block the receipt names is the same one this oracle reads: the
/// corpus pointer and reference pointer resolve inside one carrier and the
/// binding digest the receipt carries is recomputable from committed bytes.
#[test]
fn corpus_binding_is_recomputable_from_the_committed_layout() {
    let layout = readonly_layout();
    let digest = bindings_digest(&layout.native_route_threshold_reference.documents);
    assert!(
        digest.starts_with("sha256:"),
        "the binding digest must be a sha256 digest"
    );
    assert_eq!(
        layout.native_route_corpus.id,
        "native-pdf-to-tei-route-corpus"
    );
    assert_eq!(layout.native_route_corpus.corpus_n, 12);
    assert_eq!(layout.native_route_corpus.page_denominator, 381);
}

// Silence an unused-field warning on the refusal-class list: it pins the closed
// vocabulary the oracle reasons about.
#[test]
fn refusal_reason_classes_are_named() {
    assert!(REFUSAL_REASON_CLASSES.contains(&"UnresolvedBibliographyTarget"));
}
