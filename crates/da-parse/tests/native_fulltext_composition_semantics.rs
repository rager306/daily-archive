//! M511-j7wz2f / S03 — the independent post-serialization semantic-field oracle.
//!
//! The 2026-09-21 audit (A-FIELDS, P1) measured that the composer's emitters
//! carry far less than the candidates they are handed: `header_xml` reads only
//! `authors` and `date`, and `references_xml` reads only `reference.source`.
//! The structured semantics the leaves already produced —
//! `NativeHeaderFields{title,email,ids}`, `HeaderAuthor.affiliations` and
//! `ReferenceCandidate.fields` (`CitationFieldRole`) — are not serialized by
//! those functions at all.  Conserving *text* is not semantic equivalence, so
//! neither S01 (well-formedness) nor S02 (link resolve) could see the defect.
//!
//! T01 measured the A-FIELDS RED (0 of every admitted header/reference field in
//! the emission).  T02 repaired the emitter and turned that measurement into the
//! GREEN contract this file now asserts:
//!
//! * the source-side facts come from a real XML parser (`roxmltree`, a
//!   dev-dependency only — the composer's production import root set stays
//!   `{crate, serde, sha2, std}` and nothing in `src/` can reach it);
//! * the judge of emitted output is the same independent parser, so `Accepted`
//!   and "the emitted document still carries the admitted semantics" stay
//!   separate facts;
//! * the oracle reads **only bytes**: profiling takes the emitted document and
//!   frozen expectation constants, never `candidate`.  The composer's manifest
//!   ledger is cross-checked *against* the oracle, never used as its input.
//!
//! ## What the GREEN contract asserts
//!
//! Every admitted semantic field is injected with a distinctive frozen sentinel
//! so the measurement cannot be confused with the semantics the *preserved
//! source* already carries in its own header subtrees (which the composer
//! conserves, and which include real `<title>`, `<email>`, `<idno>` and
//! `<affiliation>` elements).  After the T02 repair, on the three sha256-pinned
//! preserved TEI (re-verified on every run):
//!
//! | fixture | admitted header title/email/ids | admitted affiliation binding | admitted citation role | author line | reference texts |
//! |---|---|---|---|---|---|
//! | hub_2507.19457 | 1 / 1 / 1 | 1 | 1 | present | 80 |
//! | rlm_2512.24601 | 1 / 1 / 1 | 1 | 1 | present | 49 |
//! | reca_2605.26525v1 | 1 / 1 / 1 | 1 | 1 | present | 78 |
//!
//! The affiliation *text* is not part of the composer's typed input
//! (`RecognizedAffiliationComponent` carries a span, not its text), so the
//! binding — the key the independent oracle reads — is what is conserved; the
//! `affiliations` counter stays informational.  The contract is asserted as
//! data (`exit 0`, never `#[ignore]`), exactly like the RED pin it replaces, so
//! it fails on deviation in either direction.
//!
//! ## Class vocabulary (shared with the T02 production gate)
//!
//! The `Class*` constants below name the typed semantic-loss class and
//! [`SemanticFieldLoss::token`] produces exactly them, so the oracle and the
//! composer gate cannot drift into near-miss spellings.  The negative matrix at
//! the end of this file drives the production gate through each class and asserts
//! the byte-identical Java fallback.

use da_parse::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// `xml:id` lives in the reserved XML namespace; the oracle resolves it as a
/// namespaced attribute through the real parser.
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// Typed semantic-loss classes (S03).  T02's composer gate reports the same
/// tokens when an admitted semantic field cannot be conserved, so a future test
/// cannot invent a near-miss spelling of this boundary.
const CLASS_HEADER_TITLE: &str = "SemanticFieldLoss::HeaderTitle";
const CLASS_HEADER_EMAIL: &str = "SemanticFieldLoss::HeaderEmail";
const CLASS_HEADER_IDS: &str = "SemanticFieldLoss::HeaderIds";
const CLASS_AUTHOR_AFFILIATION: &str = "SemanticFieldLoss::AuthorAffiliation";
const CLASS_CITATION_ROLE: &str = "SemanticFieldLoss::CitationRole";

/// The five classes one admitted semantics fixture exercises, in report order.
const SEMANTIC_FIELD_CLASSES: [&str; 5] = [
    CLASS_HEADER_TITLE,
    CLASS_HEADER_EMAIL,
    CLASS_HEADER_IDS,
    CLASS_AUTHOR_AFFILIATION,
    CLASS_CITATION_ROLE,
];

/// Frozen admitted semantics.  The values are synthetic sentinels, deliberately
/// unlike anything the preserved sources carry, so "the field is in the
/// emission" is decided by the admitted band and not by conserved source text.
const ADMITTED_TITLE: &str = "S03 ORACLE ADMITTED TITLE";
const ADMITTED_EMAIL: &str = "s03-oracle-admitted@example.invalid";
const ADMITTED_ID: &str = "S03-ORACLE-ID-0001";
const ADMITTED_AUTHOR_KEY: &str = "s03-oracle-aff-0";
const ADMITTED_AFFILIATION: &str = "S03 Oracle Admitted University";
const ADMITTED_AUTHOR_SURNAME: &str = "Oracle";
const ADMITTED_CITATION_VALUE: &str = "S03-ORACLE-CITATION-VALUE-0001";
const ADMITTED_CITATION_ROLE: CitationFieldRole = CitationFieldRole::Doi;

/// The three sha256-pinned preserved TEI, in `provenance.yaml` order, with the
/// measured direct `listBibl` entry count the reference band must conserve:
/// `(file, sha256, listBibl entries)`.
const PRESERVED_TEI: [(&str, &str, usize); 3] = [
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

/// What the preserved sources really carry inside their own `teiHeader`, read
/// independently (roxmltree) rather than by tag scanning:
/// `(file, <title> elements, <email> elements, <affiliation> elements)`.
/// This is the source-side proof that the oracle is not blind: the same element
/// kinds are visible where the source carries them, so a zero admitted count in
/// the emission is a statement about the admitted band, not about emptiness.
const PRESERVED_SOURCE_SEMANTICS: [(&str, usize, usize, usize); 3] = [
    ("hub_2507.19457.tei.xml", 2, 0, 0),
    ("rlm_2512.24601.tei.xml", 2, 3, 0),
    ("reca_2605.26525v1.tei.xml", 2, 0, 13),
];

fn preserved_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a preserved TEI fixture and verify its frozen checksum.  A missing or
/// drifted fixture is FATAL: the oracle must never silently measure other bytes
/// than the ones the audit measured.
fn preserved_tei_source(name: &str, declared_sha256: &str) -> String {
    let rel = format!("crates/da-parse/tests/fixtures/preserved/{name}");
    let bytes = std::fs::read(preserved_repo_root().join(&rel)).unwrap_or_else(|error| {
        panic!("FATAL {rel}: preserved fixture missing/unreadable ({error})")
    });
    let actual = hex::encode(Sha256::digest(&bytes));
    assert_eq!(
        actual, declared_sha256,
        "FATAL {name}: preserved fixture bytes drifted from the frozen checksum"
    );
    String::from_utf8(bytes)
        .unwrap_or_else(|error| panic!("FATAL {name}: preserved fixture is not UTF-8 ({error})"))
}

/// The admitted semantics of one composition, with the frozen sentinel values
/// the oracle searches the emission for.  `reference_texts` is derived from the
/// *source* document by the independent parser, never from the composer.
struct AdmittedSemantics {
    fixture: &'static str,
    header_titles: Vec<&'static str>,
    header_emails: Vec<&'static str>,
    header_ids: Vec<&'static str>,
    author_ordinal: usize,
    author_surname: &'static str,
    affiliation_key: &'static str,
    affiliation_text: &'static str,
    citation_fields: Vec<(CitationFieldRole, &'static str)>,
    reference_texts: Vec<String>,
}

fn admitted_semantics(fixture: &'static str, reference_texts: Vec<String>) -> AdmittedSemantics {
    AdmittedSemantics {
        fixture,
        header_titles: vec![ADMITTED_TITLE],
        header_emails: vec![ADMITTED_EMAIL],
        header_ids: vec![ADMITTED_ID],
        author_ordinal: 0,
        author_surname: ADMITTED_AUTHOR_SURNAME,
        affiliation_key: ADMITTED_AUTHOR_KEY,
        affiliation_text: ADMITTED_AFFILIATION,
        citation_fields: vec![(ADMITTED_CITATION_ROLE, ADMITTED_CITATION_VALUE)],
        reference_texts,
    }
}

// ───────────────────────── independent XML readers ─────────────────────────

fn is_element(node: &roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

fn xml_id_of<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute((XML_NAMESPACE, "id"))
}

/// The concatenated text of every text descendant, joined by a single space —
/// the same form the reference band's source text is built from.
fn element_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_element<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    name: &str,
) -> roxmltree::Node<'a, 'input> {
    let mut found = document.descendants().filter(|node| is_element(node, name));
    let first = found.next().unwrap_or_else(|| {
        panic!("the document declares no <{name}> element");
    });
    assert!(
        found.next().is_none(),
        "the oracle requires a unique <{name}> element"
    );
    first
}

fn element_count(root: roxmltree::Node<'_, '_>, name: &str) -> usize {
    root.descendants()
        .filter(|node| is_element(node, name))
        .count()
}

/// Elements `name` whose whole trimmed text equals `value` — the shape the
/// admitted text fields must be serialized in (`title`, `email`, `idno`).
fn exact_text_element_count(root: roxmltree::Node<'_, '_>, names: &[&str], value: &str) -> usize {
    root.descendants()
        .filter(|node| node.is_element() && names.contains(&node.tag_name().name()))
        .filter(|node| element_text(*node).trim() == value)
        .count()
}

/// Elements of any name whose whole trimmed text equals `value`.  Used for the
/// affiliation component value, whose TEI carrier (`orgName`, `affiliation`,
/// `note`) the emitter may choose.
fn any_exact_text_element_count(root: roxmltree::Node<'_, '_>, value: &str) -> usize {
    root.descendants()
        .filter(|node| node.is_element())
        .filter(|node| element_text(*node).trim() == value)
        .count()
}

/// The unique `listBibl` element.  More than one means the oracle cannot decide
/// which bibliography the emitted citation roles belong to, which is itself a
/// finding.
fn unique_list_bibl<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    unique_element(document, "listBibl")
}

/// Whether the emitted `<author xml:id="author-{ordinal}">` carries the admitted
/// link to `key`: an `affiliation` (or any element) attribute referencing the
/// key, or the admitted affiliation text somewhere in the author subtree.
fn author_link_present(
    header: roxmltree::Node<'_, '_>,
    ordinal: usize,
    key: &str,
    affiliation_text: &str,
) -> bool {
    let author_id = format!("author-{ordinal}");
    header
        .descendants()
        .filter(|node| is_element(node, "author"))
        .filter(|node| xml_id_of(*node) == Some(author_id.as_str()))
        .any(|author| {
            let key_bound = author
                .descendants()
                .filter(|node| node.is_element())
                .any(|node| {
                    ["key", "ref", "target", "href"].iter().any(|attribute| {
                        node.attribute(*attribute)
                            .map(|value| value.trim_start_matches('#') == key)
                            .unwrap_or(false)
                    })
                });
            key_bound || element_text(author).contains(affiliation_text)
        })
}

/// Whether the emitted bibliography binds `value` to its admitted `role`: the
/// element whose trimmed text equals the value, or one of its three nearest
/// ancestors, carries an attribute whose name or value is the role's wire token.
fn citation_role_bound(
    list: roxmltree::Node<'_, '_>,
    role: CitationFieldRole,
    value: &str,
) -> bool {
    let token = serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    list.descendants()
        .filter(|node| node.is_element())
        .filter(|node| element_text(*node).trim() == value)
        .any(|node| {
            let mut current = Some(node);
            for _ in 0..4 {
                let Some(element) = current else {
                    return false;
                };
                if element
                    .attributes()
                    .any(|attribute| attribute.name() == token || attribute.value() == token)
                {
                    return true;
                }
                current = element.parent().filter(|parent| parent.is_element());
            }
            false
        })
}

// ─────────────────────────────── the oracle ────────────────────────────────

/// What the *emitted* document carries, read only through `roxmltree`.
///
/// The admitted counters are the measurement: `0` means the emission does not
/// carry that admitted semantic class at all.  The control fields are the
/// pre-existing behaviour the RED is measured *against* — if a repair silently
/// dropped the author line or a reference text, the pin must fail rather than
/// report a clean zero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SemanticFieldProfile {
    admitted_header_titles: usize,
    admitted_header_emails: usize,
    admitted_header_ids: usize,
    admitted_affiliations: usize,
    admitted_author_links: usize,
    admitted_citation_values: usize,
    admitted_citation_roles: usize,
    /// The admitted author line (`<author xml:id="author-0">` carrying the
    /// admitted surname) survived the header splice.
    author_line_present: bool,
    /// Admitted reference texts still present in the emitted bibliography.
    reference_texts_present: usize,
    /// Source-carried `<title>`/`<email>`/`<affiliation>` elements still visible
    /// in the emitted header — the non-vacuity control for the measurement.
    source_carried_title_elements: usize,
    source_carried_email_elements: usize,
    source_carried_affiliation_elements: usize,
}

/// Extract the admitted semantic profile from the emitted document.
///
/// `emission_xml` is the only evidence: the expectation is frozen test data, and
/// the profile is derived by re-parsing the emitted bytes.
fn oracle_semantic_field_profile(
    fixture: &str,
    emission_xml: &str,
    expected: &AdmittedSemantics,
) -> SemanticFieldProfile {
    assert_eq!(
        fixture, expected.fixture,
        "the oracle is judging a different fixture than it was configured for"
    );
    let document = roxmltree::Document::parse(emission_xml).unwrap_or_else(|error| {
        panic!("{fixture}: independent parser rejects the emission: {error}")
    });
    let header = unique_element(&document, "teiHeader");
    let list = unique_list_bibl(&document);

    let admitted_header_titles = expected
        .header_titles
        .iter()
        .copied()
        .filter(|title| exact_text_element_count(header, &["title"], title) > 0)
        .count();
    let admitted_header_emails = expected
        .header_emails
        .iter()
        .copied()
        .filter(|email| exact_text_element_count(header, &["email"], email) > 0)
        .count();
    let admitted_header_ids = expected
        .header_ids
        .iter()
        .copied()
        .filter(|id| exact_text_element_count(header, &["idno", "pubnum"], id) > 0)
        .count();
    let admitted_affiliations =
        usize::from(any_exact_text_element_count(header, expected.affiliation_text) > 0);
    let admitted_author_links = usize::from(author_link_present(
        header,
        expected.author_ordinal,
        expected.affiliation_key,
        expected.affiliation_text,
    ));
    let admitted_citation_values = expected
        .citation_fields
        .iter()
        .copied()
        .filter(|(_, value)| {
            list.descendants()
                .any(|node| node.is_element() && element_text(node).trim() == *value)
        })
        .count();
    let admitted_citation_roles = expected
        .citation_fields
        .iter()
        .copied()
        .filter(|(role, value)| citation_role_bound(list, *role, value))
        .count();

    let author_line_present = header
        .descendants()
        .filter(|node| is_element(node, "author"))
        .filter(|node| {
            xml_id_of(*node) == Some(format!("author-{}", expected.author_ordinal).as_str())
        })
        .any(|author| element_text(author).contains(expected.author_surname));
    let emitted_entries: Vec<String> = list
        .descendants()
        .filter(|node| is_element(node, "biblStruct"))
        .map(element_text)
        .collect();
    let reference_texts_present = expected
        .reference_texts
        .iter()
        .filter(|text| {
            let text = text.trim();
            !text.is_empty() && emitted_entries.iter().any(|entry| entry.contains(text))
        })
        .count();

    SemanticFieldProfile {
        admitted_header_titles,
        admitted_header_emails,
        admitted_header_ids,
        admitted_affiliations,
        admitted_author_links,
        admitted_citation_values,
        admitted_citation_roles,
        author_line_present,
        reference_texts_present,
        source_carried_title_elements: element_count(header, "title"),
        source_carried_email_elements: element_count(header, "email"),
        source_carried_affiliation_elements: element_count(header, "affiliation"),
    }
}

/// Typed findings for one fixture: one line per semantic class the admitted band
/// failed to conserve, naming the fixture and the class.  T02's GREEN contract is
/// this returning nothing.
fn semantic_field_findings(
    fixture: &str,
    expected: &AdmittedSemantics,
    profile: &SemanticFieldProfile,
) -> Vec<String> {
    let mut findings = Vec::new();
    let mut class = |token: &str, observed: usize, total: usize, what: &str| {
        if observed < total {
            findings.push(format!(
                "{fixture}: {token}: {observed}/{total} admitted {what} present in the emission"
            ));
        }
    };
    class(
        CLASS_HEADER_TITLE,
        profile.admitted_header_titles,
        expected.header_titles.len(),
        "header title values",
    );
    class(
        CLASS_HEADER_EMAIL,
        profile.admitted_header_emails,
        expected.header_emails.len(),
        "header email values",
    );
    class(
        CLASS_HEADER_IDS,
        profile.admitted_header_ids,
        expected.header_ids.len(),
        "header id values",
    );
    class(
        CLASS_AUTHOR_AFFILIATION,
        profile.admitted_author_links,
        1,
        "author affiliation link",
    );
    class(
        CLASS_CITATION_ROLE,
        profile.admitted_citation_roles,
        expected.citation_fields.len(),
        "bibliography citation roles",
    );
    findings
}

// ─────────────────────────────── harness ───────────────────────────────────

fn receipt(
    component: FulltextComponent,
    disposition: AdmissionDisposition,
) -> FulltextAdmissionReceipt {
    FulltextAdmissionReceipt {
        component,
        producer: "fixture".into(),
        disposition,
        evidence_state: if disposition == AdmissionDisposition::Passed {
            "passing"
        } else {
            "unsupported"
        }
        .into(),
        schema_version: "native-composition.outcome/v1".into(),
        source_sha256: SOURCE.into(),
        artifact_sha256: String::new(),
        fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
    }
}

fn admissions() -> Vec<FulltextAdmissionReceipt> {
    [
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
    .into()
}

/// The admitted header input: one author carrying an affiliation link.  Built
/// from the frozen admitted constants, never from the source document.
fn header_input_with_links(links: Vec<NativeAffiliationLink>) -> HeaderCompositionInput {
    let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, "O. Oracle");
    HeaderCompositionInput {
        date: NativeDateParser::new().parse("2026-09-23"),
        authors: vec![HeaderAuthorInput {
            ordinal: 0,
            span: CompositionSpan { start: 1, end: 9 },
            name,
            affiliations: NativeAffiliationLinkOutcome::Linked {
                author: NativeAffiliationAuthorEvidence {
                    ordinal: 0,
                    span: AffiliationSpan { start: 0, end: 1 },
                },
                links,
            },
        }],
        affiliation_components: vec![NativeAffiliationParser::new().parse(ADMITTED_AFFILIATION)],
    }
}

/// The admitted affiliation link: the binding the independent oracle reads.
fn admitted_link() -> NativeAffiliationLink {
    NativeAffiliationLink {
        key: ADMITTED_AUTHOR_KEY.into(),
        affiliation_ordinal: 0,
        affiliation_span: AffiliationSpan {
            start: 0,
            end: ADMITTED_AFFILIATION.len(),
        },
    }
}

fn admitted_header_input() -> HeaderCompositionInput {
    header_input_with_links(vec![admitted_link()])
}

/// The admitted projected header fields.
fn admitted_fields() -> NativeHeaderFields {
    NativeHeaderFields {
        title: vec![ADMITTED_TITLE.into()],
        email: vec![ADMITTED_EMAIL.into()],
        ids: vec![ADMITTED_ID.into()],
    }
}

/// The admitted header band: one author carrying an affiliation link, plus the
/// projected native title/email/ids fields.
fn admitted_header() -> HeaderCompositionOutcome {
    compose_header_with_fields(admitted_header_input(), admitted_fields())
}

/// The source's own direct `listBibl` child texts, read with the independent
/// parser.  The reference band's domain must equal this list (S02), so the
/// measurement admits exactly the source entries and nothing is invented.
fn source_reference_texts(fixture: &str, xml: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(xml)
        .unwrap_or_else(|error| panic!("{fixture}: the source document does not parse: {error}"));
    let list = unique_list_bibl(&document);
    list.children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(element_text)
        .collect()
}

/// A reference band for the given source texts and admitted citation fields.
fn reference_outcome(
    reference_texts: &[String],
    fields: Vec<NativeCitationOutcome>,
) -> ReferenceCompositionOutcome {
    let references = reference_texts
        .iter()
        .enumerate()
        .map(|(ordinal, text)| NativeReference {
            ordinal,
            source: text.clone(),
            span: CitationSpan {
                start: 0,
                end: text.len(),
            },
            provenance: "preserved-tei-semantics-oracle".into(),
        })
        .collect();
    compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized { references },
        fields,
        anchors: NativeCitationAnchorOutcome::Empty,
    })
}

fn admitted_references(expected: &AdmittedSemantics) -> ReferenceCompositionOutcome {
    let fields = expected
        .citation_fields
        .iter()
        .map(|(role, value)| NativeCitationOutcome::Recognized {
            fields: vec![CitationField {
                role: *role,
                value: (*value).into(),
                span: CitationSpan {
                    start: 0,
                    end: value.len(),
                },
            }],
        })
        .collect();
    reference_outcome(&expected.reference_texts, fields)
}

/// The admitted-semantics composition input for one preserved fixture.
fn admitted_input(xml: &str, expected: &AdmittedSemantics) -> NativeFulltextCompositionInput {
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, "profile=fixture"),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: SOURCE.into(),
            profile_fingerprint: "profile=fixture".into(),
            config_fingerprint: "config=fixture".into(),
        },
        header: admitted_header(),
        references: admitted_references(expected),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: admissions(),
        s17_lineage: Some(S17LineageReceipt {
            source: "immutable-s17".into(),
            artifact_sha256: ARTIFACT.into(),
            live_ruvllm_receipt: false,
            output_affecting: false,
        }),
    }
}

/// Compose the admitted-semantics configuration and return the candidate.
///
/// A rejection is a measurement failure, not a pass: the audit configuration is
/// "the admitted band is in the emission", the only configuration in which the
/// admitted semantics can be observed at all.
fn compose_admitted_semantics(
    fixture: &str,
    xml: &str,
    expected: &AdmittedSemantics,
) -> NativeFulltextCandidate {
    match compose_native_fulltext(admitted_input(xml, expected)) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => candidate,
        NativeFulltextCompositionOutcome::Rejected { error, .. } => panic!(
            "{fixture}: the admitted-semantics configuration must be Accepted so the emission can be measured; got {error:?}"
        ),
    }
}

// ──────────────────────────────── tests ────────────────────────────────────

#[test]
fn preserved_fixture_bytes_are_sha256_pinned() {
    for (name, sha256, _) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        assert!(
            !xml.is_empty(),
            "{name}: a preserved fixture must stay non-empty"
        );
    }
}

/// The class vocabulary the oracle reports and the production refusal emits are
/// the same tokens, and the emission role token is the serde wire form.  Neither
/// can drift without this test failing.
#[test]
fn production_loss_classes_match_the_oracle_vocabulary() {
    assert_eq!(SemanticFieldLoss::HeaderTitle.token(), CLASS_HEADER_TITLE);
    assert_eq!(SemanticFieldLoss::HeaderEmail.token(), CLASS_HEADER_EMAIL);
    assert_eq!(SemanticFieldLoss::HeaderIds.token(), CLASS_HEADER_IDS);
    assert_eq!(
        SemanticFieldLoss::AuthorAffiliation.token(),
        CLASS_AUTHOR_AFFILIATION
    );
    assert_eq!(SemanticFieldLoss::CitationRole.token(), CLASS_CITATION_ROLE);
    let serde_token = |role: CitationFieldRole| {
        serde_json::to_value(role)
            .expect("CitationFieldRole serializes")
            .as_str()
            .expect("CitationFieldRole serializes as a string")
            .to_owned()
    };
    for role in [
        CitationFieldRole::Authors,
        CitationFieldRole::PublicationDate,
        CitationFieldRole::ArticleTitle,
        CitationFieldRole::ContainerTitle,
        CitationFieldRole::Volume,
        CitationFieldRole::Issue,
        CitationFieldRole::Pages,
        CitationFieldRole::ArticleNumber,
        CitationFieldRole::Publisher,
        CitationFieldRole::Place,
        CitationFieldRole::Doi,
        CitationFieldRole::ArxivId,
        CitationFieldRole::Url,
    ] {
        assert_eq!(
            citation_field_role_token(role),
            serde_token(role),
            "the emitted role token must equal the serde wire form for {role:?}"
        );
    }
}

/// The repaired contract: every admitted semantic field reaches the emitted
/// bytes, the pre-existing controls survive, and the composer's own ledger agrees
/// with the independent extraction.
#[test]
fn admitted_semantic_fields_are_conserved_in_the_emission() {
    for (name, sha256, references) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let expected = admitted_semantics(name, source_reference_texts(name, &xml));
        assert_eq!(
            expected.reference_texts.len(),
            references,
            "{name}: the source listBibl entry denominator drifted"
        );
        let candidate = compose_admitted_semantics(name, &xml, &expected);
        let emission = String::from_utf8(candidate.envelope.raw_tei().to_vec())
            .unwrap_or_else(|error| panic!("{name}: emitted document is not UTF-8 ({error})"));
        let profile = oracle_semantic_field_profile(name, &emission, &expected);

        eprintln!(
            "A-FIELDS GREEN {name}: titles={} emails={} ids={} affiliations={} author_links={} citation_values={} citation_roles={} | controls: author_line={} reference_texts={}/{} | source-carried: titles={} emails={} affiliations={}",
            profile.admitted_header_titles,
            profile.admitted_header_emails,
            profile.admitted_header_ids,
            profile.admitted_affiliations,
            profile.admitted_author_links,
            profile.admitted_citation_values,
            profile.admitted_citation_roles,
            profile.author_line_present,
            profile.reference_texts_present,
            references,
            profile.source_carried_title_elements,
            profile.source_carried_email_elements,
            profile.source_carried_affiliation_elements,
        );

        // The GREEN contract: every admitted class is in the bytes.
        assert_eq!(
            profile.admitted_header_titles,
            expected.header_titles.len(),
            "{name}: header title"
        );
        assert_eq!(
            profile.admitted_header_emails,
            expected.header_emails.len(),
            "{name}: header email"
        );
        assert_eq!(
            profile.admitted_header_ids,
            expected.header_ids.len(),
            "{name}: header ids"
        );
        assert_eq!(
            profile.admitted_author_links, 1,
            "{name}: author affiliation binding"
        );
        assert_eq!(
            profile.admitted_citation_values,
            expected.citation_fields.len(),
            "{name}: citation value"
        );
        assert_eq!(
            profile.admitted_citation_roles,
            expected.citation_fields.len(),
            "{name}: citation role"
        );

        // The controls: the repair did not trade a field for the pre-existing
        // behaviour.
        assert!(
            profile.author_line_present,
            "{name}: the admitted author line must survive the header splice"
        );
        assert_eq!(
            profile.reference_texts_present, references,
            "{name}: every admitted reference text must survive the bibliography splice"
        );
        assert!(
            profile.source_carried_title_elements >= 1,
            "{name}: the emission must still expose the source's <title> elements"
        );

        assert_eq!(
            semantic_field_findings(name, &expected, &profile),
            Vec::<String>::new(),
            "{name}: the emission must conserve every admitted semantic field"
        );

        // The composer's ledger agrees with the independent extraction.  The
        // oracle is the judge; the ledger is checked against it, never the other
        // way round.
        let ledger = candidate.manifest.semantic_fields;
        assert_eq!(
            ledger.admitted_header_titles,
            expected.header_titles.len(),
            "{name}: ledger admitted titles"
        );
        assert_eq!(
            ledger.admitted_header_emails,
            expected.header_emails.len(),
            "{name}: ledger admitted emails"
        );
        assert_eq!(
            ledger.admitted_header_ids,
            expected.header_ids.len(),
            "{name}: ledger admitted ids"
        );
        assert_eq!(
            ledger.admitted_author_affiliations, 1,
            "{name}: ledger admitted affiliations"
        );
        assert_eq!(
            ledger.admitted_citation_fields,
            expected.citation_fields.len(),
            "{name}: ledger admitted citation fields"
        );
        assert_eq!(
            ledger.conserved_header_titles, profile.admitted_header_titles,
            "{name}: ledger conserved titles vs oracle"
        );
        assert_eq!(
            ledger.conserved_header_emails, profile.admitted_header_emails,
            "{name}: ledger conserved emails vs oracle"
        );
        assert_eq!(
            ledger.conserved_header_ids, profile.admitted_header_ids,
            "{name}: ledger conserved ids vs oracle"
        );
        assert_eq!(
            ledger.conserved_author_affiliations, profile.admitted_author_links,
            "{name}: ledger conserved affiliations vs oracle"
        );
        assert_eq!(
            ledger.conserved_citation_fields, profile.admitted_citation_roles,
            "{name}: ledger conserved citation roles vs oracle"
        );
    }
}

/// Backward compatibility: a manifest serialized before S03/T02 (no
/// `semantic_fields` key) decodes to the all-zero ledger instead of failing —
/// the same device `reference_links` uses.
#[test]
fn pre_s03_manifests_decode_with_an_all_zero_semantic_ledger() {
    let (xml, expected) = harness_fixture();
    let candidate = compose_admitted_semantics("pre-s03", &xml, &expected);
    let mut value = serde_json::to_value(&candidate.manifest).expect("manifest serializes");
    let object = value.as_object_mut().expect("manifest is a JSON object");
    assert!(
        object.remove("semantic_fields").is_some(),
        "the manifest must serialize the semantic ledger"
    );
    let decoded: NativeFulltextManifest =
        serde_json::from_value(value).expect("a pre-S03 manifest still decodes");
    assert_eq!(
        decoded.semantic_fields,
        Box::new(SemanticFieldLedger::default()),
        "a missing semantic ledger must degrade to the all-zero record"
    );
}

// ───────────────────────── negative matrix (T02) ───────────────────────────

/// The first preserved fixture with its frozen expectation.
fn harness_fixture() -> (String, AdmittedSemantics) {
    let (name, sha256, _) = PRESERVED_TEI[0];
    let xml = preserved_tei_source(name, sha256);
    let expected = admitted_semantics(name, source_reference_texts(name, &xml));
    (xml, expected)
}

/// Compose an adversarial input that must be refused, asserting the fallback is
/// the byte-identical Java envelope, and return the typed error.
fn expect_typed_refusal(input: &NativeFulltextCompositionInput) -> NativeFulltextCompositionError {
    let fallback = input.java_fulltext.raw_tei().to_vec();
    match compose_native_fulltext(input.clone()) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => panic!(
            "the adversarial semantic configuration must be Rejected; got Accepted with {} bytes",
            candidate.envelope.raw_tei().len()
        ),
        NativeFulltextCompositionOutcome::Rejected {
            fallback: envelope,
            error,
        } => {
            assert_eq!(
                envelope.raw_tei(),
                fallback.as_slice(),
                "a semantic refusal must return the byte-identical Java envelope"
            );
            error
        }
    }
}

/// The typed class token of a refusal, failing loudly on any other error shape.
fn refusal_class(error: &NativeFulltextCompositionError) -> &'static str {
    match error {
        NativeFulltextCompositionError::SemanticFieldLoss { class, .. } => class.token(),
        other => panic!("expected a typed SemanticFieldLoss refusal, got {other:?}"),
    }
}

/// (a) An admitted header title that cannot be carried into XML character data is
/// a typed refusal, not a silently dropped `<title>`.
#[test]
fn loss_of_an_admitted_title_is_a_typed_refusal() {
    let (xml, expected) = harness_fixture();
    let mut input = admitted_input(&xml, &expected);
    let mut fields = admitted_fields();
    fields.title = vec!["illegal-\u{1}-title".into()];
    input.header = compose_header_with_fields(admitted_header_input(), fields);

    let error = expect_typed_refusal(&input);
    assert_eq!(refusal_class(&error), CLASS_HEADER_TITLE);
}

/// (b) A bibliography field whose role cannot be conserved (an invalid span)
/// while every reference text is preserved is a typed refusal.
#[test]
fn loss_of_a_citation_role_beside_preserved_reference_text_is_a_typed_refusal() {
    let (xml, expected) = harness_fixture();
    let mut input = admitted_input(&xml, &expected);
    input.references = reference_outcome(
        &expected.reference_texts,
        vec![NativeCitationOutcome::Recognized {
            fields: vec![CitationField {
                role: ADMITTED_CITATION_ROLE,
                value: ADMITTED_CITATION_VALUE.into(),
                span: CitationSpan { start: 5, end: 5 },
            }],
        }],
    );

    let error = expect_typed_refusal(&input);
    assert_eq!(refusal_class(&error), CLASS_CITATION_ROLE);
}

/// (c) An affiliation link whose span is not valid is a typed refusal.
#[test]
fn invalid_affiliation_span_is_a_typed_refusal() {
    let (xml, expected) = harness_fixture();
    let mut input = admitted_input(&xml, &expected);
    input.header = compose_header_with_fields(
        header_input_with_links(vec![NativeAffiliationLink {
            key: ADMITTED_AUTHOR_KEY.into(),
            affiliation_ordinal: 0,
            affiliation_span: AffiliationSpan { start: 5, end: 5 },
        }]),
        admitted_fields(),
    );

    let error = expect_typed_refusal(&input);
    assert_eq!(refusal_class(&error), CLASS_AUTHOR_AFFILIATION);
}

/// (d) The same adversarial input twice yields the same typed error and the same
/// byte-identical fallback: the refusal is deterministic, not timing-dependent.
#[test]
fn semantic_refusals_are_deterministic() {
    let (xml, expected) = harness_fixture();
    let mut input = admitted_input(&xml, &expected);
    let mut fields = admitted_fields();
    fields.title = vec!["illegal-\u{1}-title".into()];
    input.header = compose_header_with_fields(admitted_header_input(), fields);

    let fallback = input.java_fulltext.raw_tei().to_vec();
    let first = expect_typed_refusal(&input);
    let second = expect_typed_refusal(&input);
    assert_eq!(first, second, "the typed refusal must be deterministic");

    let fallback_once =
        |input: &NativeFulltextCompositionInput| match compose_native_fulltext(input.clone()) {
            NativeFulltextCompositionOutcome::Rejected {
                fallback: envelope, ..
            } => envelope.raw_tei().to_vec(),
            NativeFulltextCompositionOutcome::Accepted(_) => panic!("must be rejected"),
        };
    assert_eq!(
        fallback_once(&input),
        fallback,
        "the fallback is the source Java envelope"
    );
    assert_eq!(
        fallback_once(&input),
        fallback_once(&input),
        "the fallback bytes must be identical across runs"
    );
}

/// Non-vacuity on the emission side: the oracle is a discriminating function, not
/// a constant zero.  Each admitted semantic class is injected into a synthetic
/// document in the shape the emitter must produce, and the oracle must see it.
#[test]
fn oracle_detects_each_semantic_class_when_the_emission_carries_it() {
    let expected = admitted_semantics("synthetic-emission", vec!["Reference one".into()]);

    let header_title = r##"<TEI><teiHeader><fileDesc><titleStmt><title>S03 ORACLE ADMITTED TITLE</title></titleStmt></fileDesc></teiHeader><text><body/><listBibl/></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-emission", header_title, &expected);
    assert_eq!(profile.admitted_header_titles, 1, "title");
    assert_eq!(
        profile.admitted_header_emails, 0,
        "email must stay unmeasured"
    );

    let header_email = r##"<TEI><teiHeader><fileDesc><titleStmt><email>s03-oracle-admitted@example.invalid</email></titleStmt></fileDesc></teiHeader><text><body/><listBibl/></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-emission", header_email, &expected);
    assert_eq!(profile.admitted_header_emails, 1, "email");

    let header_ids = r##"<TEI><teiHeader><fileDesc><titleStmt><idno type="pubnum">S03-ORACLE-ID-0001</idno></titleStmt></fileDesc></teiHeader><text><body/><listBibl/></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-emission", header_ids, &expected);
    assert_eq!(profile.admitted_header_ids, 1, "ids");

    let header_author_link = r##"<TEI><teiHeader><fileDesc><titleStmt><author xml:id="author-0"><surname>Oracle</surname><affiliation key="s03-oracle-aff-0">S03 Oracle Admitted University</affiliation></author></titleStmt></fileDesc></teiHeader><text><body/><listBibl/></text></TEI>"##;
    let profile =
        oracle_semantic_field_profile("synthetic-emission", header_author_link, &expected);
    assert_eq!(profile.admitted_affiliations, 1, "affiliation value");
    assert_eq!(profile.admitted_author_links, 1, "author affiliation link");
    assert!(profile.author_line_present, "author line");

    let citation = r##"<TEI><teiHeader><fileDesc><titleStmt/></fileDesc></teiHeader><text><body/><listBibl><biblStruct xml:id="b0">Reference one<note role="doi">S03-ORACLE-CITATION-VALUE-0001</note></biblStruct></listBibl></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-emission", citation, &expected);
    assert_eq!(profile.admitted_citation_values, 1, "citation value");
    assert_eq!(profile.admitted_citation_roles, 1, "citation role");
    assert_eq!(profile.reference_texts_present, 1, "reference text control");

    // All five classes at once: the emission a repaired composer must produce,
    // and the findings must collapse to nothing.
    let complete = r##"<TEI><teiHeader><fileDesc><titleStmt><author xml:id="author-0"><surname>Oracle</surname><affiliation key="s03-oracle-aff-0">S03 Oracle Admitted University</affiliation></author><title>S03 ORACLE ADMITTED TITLE</title><email>s03-oracle-admitted@example.invalid</email><idno type="pubnum">S03-ORACLE-ID-0001</idno></titleStmt></fileDesc></teiHeader><text><body/><listBibl><biblStruct xml:id="b0">Reference one<note role="doi">S03-ORACLE-CITATION-VALUE-0001</note></biblStruct></listBibl></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-emission", complete, &expected);
    assert_eq!(
        semantic_field_findings("synthetic-emission", &expected, &profile),
        Vec::<String>::new(),
        "a complete emission must leave no semantic-loss finding"
    );
}

/// Negative controls: the oracle names exactly the classes that are missing, and
/// a value that is merely *present as bytes* does not satisfy a field whose role
/// was not serialized.
///
/// The `title` value here appears only as an attribute value, and the citation
/// value appears as plain text beside no role — the two shapes a repair might use
/// to look complete while the semantics are still gone.  Both must stay findings.
#[test]
fn oracle_reports_exactly_the_missing_classes_and_is_not_fooled_by_raw_text() {
    let expected = admitted_semantics("synthetic-negative", vec!["Reference one".into()]);
    let value_only = r##"<TEI><teiHeader><fileDesc><titleStmt><title type="S03 ORACLE ADMITTED TITLE"/><email>s03-oracle-admitted@example.invalid</email><idno type="pubnum">S03-ORACLE-ID-0001</idno><author xml:id="author-0"><surname>Oracle</surname><affiliation key="s03-oracle-aff-0">S03 Oracle Admitted University</affiliation></author></titleStmt></fileDesc></teiHeader><text><body/><listBibl><biblStruct xml:id="b0">Reference one<note>S03-ORACLE-CITATION-VALUE-0001</note></biblStruct></listBibl></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-negative", value_only, &expected);
    assert_eq!(
        profile.admitted_header_titles, 0,
        "a title value that only appears as an attribute is not a serialized title field"
    );
    assert_eq!(profile.admitted_header_emails, 1, "email");
    assert_eq!(profile.admitted_header_ids, 1, "ids");
    assert_eq!(profile.admitted_author_links, 1, "author link");
    assert_eq!(
        profile.admitted_citation_values, 1,
        "the citation value text is present"
    );
    assert_eq!(
        profile.admitted_citation_roles, 0,
        "but the role it belongs to is not serialized"
    );
    let findings = semantic_field_findings("synthetic-negative", &expected, &profile);
    assert_eq!(
        findings.len(),
        2,
        "only two classes are missing: {findings:?}"
    );
    assert!(
        findings[0].starts_with(&format!("synthetic-negative: {CLASS_HEADER_TITLE}")),
        "{findings:?}"
    );
    assert!(
        findings[1].starts_with(&format!("synthetic-negative: {CLASS_CITATION_ROLE}")),
        "{findings:?}"
    );
}

/// Boundary: an emission without the admitted author line and with an empty
/// bibliography must be visible as zero controls, so a repair that silently
/// dropped the pre-existing behaviour cannot pass as "no findings".
#[test]
fn oracle_reports_absent_controls_instead_of_a_silent_pass() {
    let expected = admitted_semantics("synthetic-controls", vec!["Reference one".into()]);
    let empty = r##"<TEI><teiHeader><fileDesc><titleStmt/></fileDesc></teiHeader><text><body/><listBibl/></text></TEI>"##;
    let profile = oracle_semantic_field_profile("synthetic-controls", empty, &expected);
    assert!(
        !profile.author_line_present,
        "author line must read as absent"
    );
    assert_eq!(
        profile.reference_texts_present, 0,
        "an empty bibliography cannot conserve a reference text"
    );
    assert_eq!(
        semantic_field_findings("synthetic-controls", &expected, &profile).len(),
        SEMANTIC_FIELD_CLASSES.len()
    );
}

/// Non-vacuity on the source side: the preserved sources really do carry the
/// semantic element kinds.  An oracle that reported zero because it cannot see
/// `<title>`/`<email>`/`<affiliation>` at all would fail here.
#[test]
fn oracle_reads_the_semantics_the_preserved_source_really_carries() {
    for (name, sha256, _) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let document = roxmltree::Document::parse(&xml)
            .unwrap_or_else(|error| panic!("{name}: source does not parse: {error}"));
        let header = unique_element(&document, "teiHeader");
        let (_, titles, emails, affiliations) = PRESERVED_SOURCE_SEMANTICS
            .iter()
            .find(|(fixture, _, _, _)| *fixture == name)
            .unwrap_or_else(|| panic!("{name}: no frozen source-semantics denominator"));
        assert_eq!(element_count(header, "title"), *titles, "{name}: titles");
        assert_eq!(element_count(header, "email"), *emails, "{name}: emails");
        assert_eq!(
            element_count(header, "affiliation"),
            *affiliations,
            "{name}: affiliations"
        );
        assert!(
            *titles >= 1,
            "{name}: the source-side measurement must not be vacuous"
        );
    }
    let non_vacuous = PRESERVED_SOURCE_SEMANTICS
        .iter()
        .any(|(_, _, emails, _)| *emails > 0)
        && PRESERVED_SOURCE_SEMANTICS
            .iter()
            .any(|(_, _, _, affiliations)| *affiliations > 0);
    assert!(
        non_vacuous,
        "the preserved corpus must carry at least one <email> and one <affiliation>"
    );
}

/// The frozen inline oracle fixture carries a real `<title>`, so the same
/// independent extraction sees it there too — the `<title>Java title</title>`
/// example the task plan names.
#[test]
fn oracle_reads_the_frozen_inline_fixture_title() {
    #[derive(Deserialize)]
    struct OracleFixture {
        source_xml: String,
    }
    let fixture: OracleFixture = serde_yaml::from_str(include_str!(
        "fixtures/native_fulltext_composition_oracle.yaml"
    ))
    .expect("the frozen oracle fixture parses");
    assert!(
        fixture.source_xml.contains("<title>Java title</title>"),
        "the frozen oracle fixture must keep its <title> example"
    );
    let document = roxmltree::Document::parse(&fixture.source_xml)
        .expect("the frozen oracle fixture source parses");
    let header = unique_element(&document, "teiHeader");
    assert_eq!(
        exact_text_element_count(header, &["title"], "Java title"),
        1,
        "the oracle must see the fixture's own <title> text through the real parser"
    );
}
