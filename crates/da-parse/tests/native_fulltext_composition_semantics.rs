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
//! This file is the measurement, deliberately built before the repair, exactly
//! as S02/T01 built the A-REF oracle before S02/T02 repaired the composer:
//!
//! * the source-side facts come from a real XML parser (`roxmltree`, a
//!   dev-dependency only — the composer's production import root set stays
//!   `{crate, serde, sha2, std}` and nothing in `src/` can reach it);
//! * the judge of emitted output is the same independent parser, so `Accepted`
//!   and "the emitted document still carries the admitted semantics" stay
//!   separate facts;
//! * the oracle reads **only bytes**: it takes the emitted document and frozen
//!   expectation constants.  It never inspects `candidate`, its manifest, or the
//!   ledger — a composer cannot be the judge of its own emission.
//!
//! ## What T01 measures (the RED pin)
//!
//! Every admitted semantic field is injected with a distinctive frozen sentinel
//! so the measurement cannot be confused with the semantics the *preserved
//! source* already carries in its own header subtrees (which the composer
//! conserves, and which include real `<title>`, `<email>`, `<idno>` and
//! `<affiliation>` elements).  Measured on the three sha256-pinned preserved TEI
//! (re-verified on every run):
//!
//! | fixture | admitted header title/email/ids | admitted affiliation link | admitted citation role | author line | reference texts |
//! |---|---|---|---|---|---|
//! | hub_2507.19457 | 0 / 0 / 0 | 0 | 0 | present | 80 |
//! | rlm_2512.24601 | 0 / 0 / 0 | 0 | 0 | present | 49 |
//! | reca_2605.26525v1 | 0 / 0 / 0 | 0 | 0 | present | 78 |
//!
//! The RED is asserted as *data*, never as a failing or `#[ignore]`d test (the
//! auto-mode gate has no expected-failure mechanism: `passed = checks.every(exit
//! Code === 0)`), so the pin fails on deviation in **either** direction — a
//! regression that loses an author line or a reference text, and the T02 repair
//! that starts carrying the fields.  T02 replaces the pin with the GREEN
//! contract and consumes the same vocabulary.
//!
//! ## Class vocabulary (shared with the T02 production gate)
//!
//! The `Class*` constants below name the typed semantic-loss class and the
//! denominator names the fixture, so T02's composer gate and this oracle cannot
//! drift into near-miss spellings.

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

/// The admitted header band: one author carrying an affiliation link, plus the
/// projected native title/email/ids fields.  Everything is built from the frozen
/// admitted constants, never from the source document.
fn admitted_header() -> HeaderCompositionOutcome {
    let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, "O. Oracle");
    compose_header_with_fields(
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
                    links: vec![NativeAffiliationLink {
                        key: ADMITTED_AUTHOR_KEY.into(),
                        affiliation_ordinal: 0,
                        affiliation_span: AffiliationSpan {
                            start: 0,
                            end: ADMITTED_AFFILIATION.len(),
                        },
                    }],
                },
            }],
            affiliation_components: vec![
                NativeAffiliationParser::new().parse(ADMITTED_AFFILIATION),
            ],
        },
        NativeHeaderFields {
            title: vec![ADMITTED_TITLE.into()],
            email: vec![ADMITTED_EMAIL.into()],
            ids: vec![ADMITTED_ID.into()],
        },
    )
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

fn admitted_references(expected: &AdmittedSemantics) -> ReferenceCompositionOutcome {
    let references = expected
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
            provenance: "preserved-tei-semantics-oracle".into(),
        })
        .collect();
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
    compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized { references },
        fields,
        anchors: NativeCitationAnchorOutcome::Empty,
    })
}

/// Compose one preserved TEI with both bands admitted and return the emitted
/// document text — the bytes the oracle judges.
///
/// A rejection is a measurement failure, not a pass: the audit configuration is
/// "the admitted band is in the emission", which is the only configuration in
/// which the admitted semantics can be observed at all.
fn emit_admitted_semantics(fixture: &str, xml: &str, expected: &AdmittedSemantics) -> String {
    let input = NativeFulltextCompositionInput {
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
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap_or_else(|error| {
                panic!("{fixture}: the emitted document is not UTF-8 ({error})")
            })
        }
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

/// The measurement.  Every admitted semantic field is absent from the emission
/// while the properties the old behaviour *did* conserve (the author line of the
/// admitted header band and every admitted reference text) are present.
///
/// Frozen as data, not as an expected failure: `--nocapture` prints the profile
/// per fixture, and any deviation in either direction fails the pin.
#[test]
fn admitted_semantic_fields_are_absent_from_the_emission_today() {
    for (name, sha256, references) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let expected = admitted_semantics(name, source_reference_texts(name, &xml));
        assert_eq!(
            expected.reference_texts.len(),
            references,
            "{name}: the source listBibl entry denominator drifted"
        );
        let emission = emit_admitted_semantics(name, &xml, &expected);
        let profile = oracle_semantic_field_profile(name, &emission, &expected);

        eprintln!(
            "A-FIELDS RED {name}: titles={} emails={} ids={} affiliations={} author_links={} citation_values={} citation_roles={} | controls: author_line={} reference_texts={}/{} | source-carried: titles={} emails={} affiliations={}",
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

        // The measured RED: not one admitted semantic class reaches the bytes.
        assert_eq!(profile.admitted_header_titles, 0, "{name}: header title");
        assert_eq!(profile.admitted_header_emails, 0, "{name}: header email");
        assert_eq!(profile.admitted_header_ids, 0, "{name}: header ids");
        assert_eq!(
            profile.admitted_affiliations, 0,
            "{name}: affiliation value"
        );
        assert_eq!(profile.admitted_author_links, 0, "{name}: author link");
        assert_eq!(
            profile.admitted_citation_values, 0,
            "{name}: citation value"
        );
        assert_eq!(profile.admitted_citation_roles, 0, "{name}: citation role");

        // The controls: the measurement is not "the emitter produces nothing".
        assert!(
            profile.author_line_present,
            "{name}: the admitted author line must survive the header splice"
        );
        assert_eq!(
            profile.reference_texts_present, references,
            "{name}: every admitted reference text must survive the bibliography splice"
        );
        // The same element kinds are visible in the emission where the source
        // carries them, so the zero above is about the admitted band.
        assert!(
            profile.source_carried_title_elements >= 1,
            "{name}: the emission must still expose the source's <title> elements"
        );

        let expected_findings: Vec<String> = SEMANTIC_FIELD_CLASSES
            .iter()
            .map(|class| format!("{name}: {class}:"))
            .collect();
        let findings = semantic_field_findings(name, &expected, &profile);
        assert_eq!(
            findings.len(),
            SEMANTIC_FIELD_CLASSES.len(),
            "{name}: the measured RED must name every semantic-loss class exactly once: {findings:?}"
        );
        for (finding, expected_prefix) in findings.iter().zip(&expected_findings) {
            assert!(
                finding.starts_with(expected_prefix),
                "{name}: finding {finding:?} must name the fixture and the typed class {expected_prefix:?}"
            );
        }
    }
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
