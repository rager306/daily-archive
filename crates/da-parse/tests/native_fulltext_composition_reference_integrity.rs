//! M511-j7wz2f / S02 — the independent reference-integrity oracle and gate.
//!
//! A-REF (audit 2026-09-21) could only be reproduced by an archived out-of-repo
//! probe (`doc/review/grobid-composition-repro.rs`): the composer checked the
//! *supplied* anchors and never the body targets of the document it was keeping,
//! and it rewrote every published bibliography key to a synthesized
//! `bibl-{ordinal}` (`native_fulltext_composition.rs`, `references_xml`). A
//! source body target `#b0` therefore survived the splice into a document that
//! no longer published `b0`.
//!
//! T01 measured that defect and pinned it as data.  T02 repaired the composer:
//! the admitted reference band republishes the source's own published keys, and
//! `Accepted` now requires every mandatory local target to resolve in the emitted
//! document (a post-splice resolve gate), so the gate below is the GREEN contract
//! rather than a RED pin.
//!
//! * the source-side facts come from a real XML parser (`roxmltree`, a
//!   dev-dependency only — the composer's production import root set stays
//!   `{crate, serde, sha2, std}` and nothing in `src/` can reach it), never from
//!   the composer's own tag scanner;
//! * the judge of emitted output is the same independent parser, so `Accepted`
//!   and "the emitted document still resolves the kept document's own
//!   `#b0`-style targets" stay separate facts;
//! * key-text parity with the published link contract is asserted against
//!   `da_ports::citation_link`, so the composer cannot drift to a private
//!   naming scheme;
//! * the manifest ledger (`ReferenceLinkLedger`) is checked against the same
//!   independent counts, so the published record cannot be vacuous either.
//!
//! The audit configuration is reproduced exactly: the reference band is
//! admitted and its candidate is built from the source `listBibl`, while the
//! *supplied* anchors are `Empty` (the preserved TEI has no separate anchor
//! stream). That is the point — with no supplied anchors the pre-splice guard
//! has nothing to check, so any obligation the kept document carries must be
//! derived from the document itself, not from the caller.
//!
//! Measured on the three sha256-pinned preserved TEI
//! (`tests/fixtures/preserved/`, re-verified on every run):
//!
//! | fixture | `listBibl` entries | `bibr` occurrences | unique targets | emitted keys |
//! |---|---|---|---|---|
//! | hub_2507.19457 | 80 | 87 | 55 | the source's own `b0..b79` |
//! | rlm_2512.24601 | 49 | 71 | 41 | the source's own `b0..b48` |
//! | reca_2605.26525v1 | 78 | 59 | 24 | the source's own `b0..b77` |
//! | total | 207 | 217 | 120 | 0 dangling |
//!
//! The source publishes `b0..b{ordinal}` and every one of the 120 mandatory
//! targets is published by the source itself, so a composer that republishes the
//! source keys leaves none dangling — which is exactly what the gate below
//! requires of the actual emission.

use da_parse::*;
use da_ports::{CitationEntryLinkKeys, positional_entry_key};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// `xml:id` lives in the reserved XML namespace; the oracle resolves it as a
/// namespaced attribute through the real parser, exactly as any unprefixed
/// `type`/`target` attribute is resolved as an unqualified one.
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// Typed rejection classes the composer reports for reference-integrity
/// failures (S02/T02).  The reference-integrity surface keeps the vocabulary in
/// one place: the gate reports a *class* naming its fixture, never a generic
/// mismatch.  `KeyCollision` and `InconservableChild` are exercised by the S02/T03
/// negatives; they are named here so a future test cannot invent a near-miss
/// spelling.
const CLASS_DOMAIN_MISMATCH: &str = "BibliographyDomainMismatch";
const CLASS_UNRESOLVED_TARGET: &str = "UnresolvedBibliographyTarget";
#[allow(dead_code)]
const CLASS_KEY_COLLISION: &str = "BibliographyKeyCollision";
#[allow(dead_code)]
const CLASS_INCONSERVABLE_CHILD: &str = "InconservableBibliographyChild";

/// The three sha256-pinned preserved TEI, in `provenance.yaml` order, with the
/// S02/T01 measured reference-integrity profile:
/// `(file, sha256, listBibl entries, bibr occurrences, unique mandatory targets)`.
const PRESERVED_TEI: [(&str, &str, usize, usize, usize); 3] = [
    (
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        80,
        87,
        55,
    ),
    (
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        49,
        71,
        41,
    ),
    (
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        78,
        59,
        24,
    ),
];

fn preserved_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a preserved TEI fixture and verify its frozen checksum. A missing or
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

fn xml_id<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute((XML_NAMESPACE, "id"))
}

/// The unique `listBibl` element. More than one means the oracle cannot decide
/// which declaration the body targets point at, which is itself a finding.
fn unique_list_bibl<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    let mut lists = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "listBibl");
    let first = lists
        .next()
        .expect("the document declares a listBibl element");
    assert!(
        lists.next().is_none(),
        "the oracle requires a unique listBibl element"
    );
    first
}

/// What the *source* document declares and demands, read only through
/// `roxmltree`:
///
/// * `entry_texts` — the text of every direct `biblStruct` child of the unique
///   `listBibl`, in document order;
/// * `published_keys` — the keys those children publish: declared `xml:id`, else
///   the positional fallback `b{ordinal}`;
/// * `target_occurrences` / `mandatory_targets` — `ref type="bibr"` elements with
///   a `#`-local `target`, by the XML attribute grammar (so whitespace around
///   `=` and single quotes are the same as the double-quoted spelling).
#[derive(Debug, Clone)]
struct SourceReferenceProfile {
    entry_texts: Vec<String>,
    published_keys: Vec<String>,
    positional_fallbacks: usize,
    target_occurrences: usize,
    mandatory_targets: BTreeSet<String>,
}

impl SourceReferenceProfile {
    fn published_entries(&self) -> usize {
        self.published_keys.len()
    }
}

fn oracle_source_reference_profile(xml: &str) -> SourceReferenceProfile {
    let document =
        roxmltree::Document::parse(xml).expect("independent parser accepts the source document");
    let list = unique_list_bibl(&document);
    let entries: Vec<roxmltree::Node<'_, '_>> = list
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "biblStruct")
        .collect();

    let mut entry_texts = Vec::with_capacity(entries.len());
    let mut published_keys = Vec::with_capacity(entries.len());
    let mut positional_fallbacks = 0usize;
    for (ordinal, entry) in entries.iter().enumerate() {
        let text = entry
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<Vec<_>>()
            .join(" ");
        entry_texts.push(text);
        match xml_id(*entry) {
            Some(key) => published_keys.push(key.to_owned()),
            None => {
                positional_fallbacks += 1;
                published_keys.push(format!("b{ordinal}"));
            }
        }
    }

    let mut target_occurrences = 0usize;
    let mut mandatory_targets = BTreeSet::new();
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "ref")
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
        entry_texts,
        published_keys,
        positional_fallbacks,
        target_occurrences,
        mandatory_targets,
    }
}

/// What the *emitted* document publishes, read only through `roxmltree`.
#[derive(Debug, Clone)]
struct EmittedReferenceProfile {
    /// Keys of the direct `biblStruct` children of the unique `listBibl`.
    published_keys: Vec<String>,
    /// Emitted entries that declare no key at all.
    undeclared_entries: usize,
    /// Every `xml:id` in the whole emitted document — the address space a
    /// `#fragment` reference can resolve against.
    id_space: BTreeSet<String>,
}

fn oracle_emitted_reference_profile(xml: &str) -> EmittedReferenceProfile {
    let document =
        roxmltree::Document::parse(xml).expect("independent parser accepts the emitted document");
    let id_space: BTreeSet<String> = document
        .descendants()
        .filter(|node| node.is_element())
        .filter_map(xml_id)
        .map(str::to_owned)
        .collect();
    let list = unique_list_bibl(&document);
    let mut published_keys = Vec::new();
    let mut undeclared_entries = 0usize;
    for entry in list
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "biblStruct")
    {
        match xml_id(entry) {
            Some(key) => published_keys.push(key.to_owned()),
            None => {
                undeclared_entries += 1;
                published_keys.push("<undeclared>".to_owned());
            }
        }
    }
    EmittedReferenceProfile {
        published_keys,
        undeclared_entries,
        id_space,
    }
}

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

fn admissions(
    header: AdmissionDisposition,
    references: AdmissionDisposition,
) -> Vec<FulltextAdmissionReceipt> {
    [
        receipt(FulltextComponent::Header, header),
        receipt(FulltextComponent::References, references),
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

fn header() -> HeaderCompositionOutcome {
    let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, "J. Smith");
    compose_header(HeaderCompositionInput {
        date: NativeDateParser::new().parse("2024-02-29"),
        authors: vec![HeaderAuthorInput {
            ordinal: 0,
            span: CompositionSpan { start: 1, end: 8 },
            name,
            affiliations: NativeAffiliationLinkOutcome::Linked {
                author: NativeAffiliationAuthorEvidence {
                    ordinal: 0,
                    span: AffiliationSpan { start: 0, end: 1 },
                },
                links: vec![],
            },
        }],
        affiliation_components: vec![NativeAffiliationParser::new().parse("Example University")],
    })
}

/// The reference band candidate, built from the source's own direct `listBibl`
/// children. `anchors` stays `Empty` — the audit configuration — so the
/// pre-splice guard has no supplied obligation to check and the composer's
/// treatment of the kept document is what the measurement sees.
fn source_backed_references(profile: &SourceReferenceProfile) -> ReferenceCompositionOutcome {
    let references = profile
        .entry_texts
        .iter()
        .enumerate()
        .map(|(ordinal, text)| NativeReference {
            ordinal,
            source: text.clone(),
            span: CitationSpan {
                start: 0,
                end: text.len(),
            },
            provenance: "preserved-tei-oracle".into(),
        })
        .collect();
    compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized { references },
        fields: Vec::new(),
        anchors: NativeCitationAnchorOutcome::Empty,
    })
}

/// Compose with the reference band admitted, the header left to the Java
/// fallback (the source header stays in place) and `Empty` supplied anchors.
fn composition_input(
    xml: &str,
    references: ReferenceCompositionOutcome,
) -> NativeFulltextCompositionInput {
    composition_input_with_reference_admission(xml, references, AdmissionDisposition::Passed)
}

/// The same input with the reference band's admission chosen by the caller.  An
/// `Unsupported` admission splices nothing, which is the configuration that
/// isolates the post-splice resolve gate from the pre-splice key-domain check.
fn composition_input_with_reference_admission(
    xml: &str,
    references: ReferenceCompositionOutcome,
    reference_admission: AdmissionDisposition,
) -> NativeFulltextCompositionInput {
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
        header: header(),
        references,
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: admissions(AdmissionDisposition::Unsupported, reference_admission),
        s17_lineage: Some(S17LineageReceipt {
            source: "immutable-s17".into(),
            artifact_sha256: ARTIFACT.into(),
            live_ruvllm_receipt: false,
            output_affecting: false,
        }),
    }
}

/// Emit the reference splice for one preserved TEI and return the emitted
/// document text together with the manifest's `ReferenceLinkLedger`.
///
/// A rejection here is a measurement failure, not a pass: with empty supplied
/// anchors and the reference band admitted, `Accepted` is the audit
/// configuration the measurement needs — and after S02/T02 it is also the claim
/// the resolve gate must earn, not an assumption.
fn emit_preserved_references(
    name: &str,
    xml: &str,
    profile: &SourceReferenceProfile,
) -> (String, ReferenceLinkLedger) {
    assert!(
        profile.entry_texts.iter().all(|text| !text.is_empty()),
        "{name}: a direct listBibl child with no text cannot carry a valid segment span"
    );
    let references = source_backed_references(profile);
    assert!(
        matches!(&references, ReferenceCompositionOutcome::Accepted { .. }),
        "{name}: the source-backed reference band must compose before the id-domain measurement"
    );
    match compose_native_fulltext(composition_input(xml, references)) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            let document = String::from_utf8(candidate.envelope.raw_tei().to_vec())
                .unwrap_or_else(|error| panic!("{name}: emitted document is not UTF-8 ({error})"));
            (document, candidate.manifest.reference_links)
        }
        NativeFulltextCompositionOutcome::Rejected { error, .. } => panic!(
            "{name}: the audit configuration (empty supplied anchors, references admitted) must be Accepted so the emitted id domain can be measured; got {error:?}"
        ),
    }
}

/// The typed findings of the reference-integrity contract for one fixture: the
/// emitted id domain must equal the source's published keys, and every
/// mandatory source target must resolve in the emitted document.
///
/// S02/T01 built this to *measure* A-REF; S02/T02 consumes the same builder in its
/// post-splice resolve gate, and the gate below asserts it returns nothing, so the
/// oracle and the composer's own refusal vocabulary cannot drift apart.
fn reference_integrity_findings(
    name: &str,
    source: &SourceReferenceProfile,
    emitted: &EmittedReferenceProfile,
) -> Vec<String> {
    let mut findings: Vec<String> = Vec::new();
    if emitted.undeclared_entries > 0 {
        findings.push(format!(
            "{name}: {CLASS_DOMAIN_MISMATCH}: {}/{} emitted bibliography entries declare no xml:id",
            emitted.undeclared_entries,
            emitted.published_keys.len()
        ));
    }
    if emitted.published_keys != source.published_keys {
        let emitted_head = &emitted.published_keys[..emitted.published_keys.len().min(4)];
        let source_head = &source.published_keys[..source.published_keys.len().min(4)];
        findings.push(format!(
            "{name}: {CLASS_DOMAIN_MISMATCH}: emitted bibliography publishes {} keys from {:?}, the source publishes {} keys from {:?}",
            emitted.published_keys.len(),
            emitted_head,
            source.published_keys.len(),
            source_head
        ));
    }
    let unresolved: Vec<&str> = source
        .mandatory_targets
        .difference(&emitted.id_space)
        .map(String::as_str)
        .collect();
    if !unresolved.is_empty() {
        let head = &unresolved[..unresolved.len().min(8)];
        findings.push(format!(
            "{name}: {CLASS_UNRESOLVED_TARGET}: {} of {} mandatory source targets do not resolve in the emitted document (head {head:?})",
            unresolved.len(),
            source.mandatory_targets.len()
        ));
    }
    findings
}

/// S02/T02: the GREEN reference-integrity contract.
///
/// The contract is: the composer republishes exactly the source's own citation
/// keys, and leaves no mandatory local target dangling in the document it returns
/// as `Accepted`.
///
/// Every assertion is made through the independent `roxmltree` oracle, so the
/// composer is never the judge of its own emission.  The audit configuration
/// (references admitted, supplied anchors `Empty`) is the one that used to leave
/// the pre-splice guard vacuous, so a pass here is a statement about the kept
/// document's own obligations, not about the caller's inputs.
///
/// The manifest ledger is checked against the same independent counts: a
/// vacuous ledger (zero required targets) would be a silent regression of the
/// measurement itself, so `required_targets` must equal the oracle's own count.
#[test]
fn emitted_bibliography_keys_publish_the_source_keys_and_resolve_every_required_target() {
    for (name, sha256, expected_entries, expected_occurrences, expected_unique_targets) in
        PRESERVED_TEI
    {
        let xml = preserved_tei_source(name, sha256);
        let source = oracle_source_reference_profile(&xml);
        let (emitted_document, ledger) = emit_preserved_references(name, &xml, &source);
        let emitted = oracle_emitted_reference_profile(&emitted_document);

        assert_eq!(
            source.published_entries(),
            expected_entries,
            "{name}: {CLASS_DOMAIN_MISMATCH}: source listBibl entries"
        );
        assert_eq!(
            source.mandatory_targets.len(),
            expected_unique_targets,
            "{name}: {CLASS_UNRESOLVED_TARGET}: unique mandatory source targets"
        );

        // The emitted key domain is the source's own published key domain.
        assert_eq!(
            emitted.published_keys,
            source.published_keys,
            "{name}: {CLASS_DOMAIN_MISMATCH}: the composed bibliography must republish the source's own keys (head emitted {:?}, head source {:?})",
            &emitted.published_keys[..emitted.published_keys.len().min(4)],
            &source.published_keys[..source.published_keys.len().min(4)]
        );
        assert_eq!(
            emitted.undeclared_entries, 0,
            "{name}: {CLASS_DOMAIN_MISMATCH}: every emitted bibliography entry must publish a key"
        );

        // Every mandatory local target of the kept document resolves.
        let unresolved: Vec<&str> = source
            .mandatory_targets
            .difference(&emitted.id_space)
            .map(String::as_str)
            .collect();
        assert!(
            unresolved.is_empty(),
            "{name}: {CLASS_UNRESOLVED_TARGET}: {} of {} mandatory source targets are dangling in an Accepted emission (head {:?})",
            unresolved.len(),
            source.mandatory_targets.len(),
            &unresolved[..unresolved.len().min(8)]
        );
        assert_eq!(
            reference_integrity_findings(name, &source, &emitted),
            Vec::<String>::new(),
            "{name}: the independent oracle must find no reference-integrity defect in the emission"
        );

        // The published ledger is not vacuous and agrees with the oracle.
        assert_eq!(
            ledger.source_entries, expected_entries,
            "{name}: ledger source_entries must count the source listBibl children"
        );
        assert_eq!(
            ledger.admitted_references, expected_entries,
            "{name}: ledger admitted_references must count the spliced entries"
        );
        assert_eq!(
            ledger.target_occurrences, expected_occurrences,
            "{name}: ledger target_occurrences must match the source-side measurement"
        );
        assert_eq!(
            ledger.required_targets,
            source.mandatory_targets.len(),
            "{name}: ledger required_targets must match the independent oracle"
        );
        assert_eq!(
            ledger.resolved_targets, expected_unique_targets,
            "{name}: ledger resolved_targets must resolve every mandatory target"
        );
        assert!(
            ledger.id_space >= expected_entries,
            "{name}: the emitted id space must at least contain the published bibliography keys, got {}",
            ledger.id_space
        );
    }
}

/// S02/T02: key-text parity with the published occurrence → entry link contract.
///
/// The composer cannot import `da_ports::citation_link` (its import root set is
/// `{crate, serde, sha2, std}`), so the two must be held equal from the outside:
/// the text `CitationEntryLinkKeys::new(ordinal, declared).published_key()`
/// publishes is what the emitted `xml:id` must carry, for every entry of every
/// frozen fixture.  A private naming scheme in either place fails here.
#[test]
fn emitted_keys_are_the_published_citation_link_key_texts() {
    for (name, sha256, _, _, _) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = oracle_source_reference_profile(&xml);
        let (emitted_document, _) = emit_preserved_references(name, &xml, &source);
        let emitted = oracle_emitted_reference_profile(&emitted_document);

        let expected: Vec<String> = source
            .published_keys
            .iter()
            .enumerate()
            .map(|(ordinal, declared)| {
                CitationEntryLinkKeys::new(ordinal as u32, Some(declared))
                    .published_key()
                    .match_text()
            })
            .collect();
        assert_eq!(
            emitted.published_keys, expected,
            "{name}: the emitted xml:id texts must be the published citation-link key texts"
        );
    }
}

/// S02/T02: the positional fallback is the published `b{ordinal}` key text, and
/// it actually resolves a kept `target="#b0"`.
///
/// The frozen fixtures all declare `xml:id`, so the fallback path is exercised on
/// an inline document: an undeclared `biblStruct` publishes `b{ordinal}` (matching
/// `da_ports::citation_link::positional_entry_key`), and a body link to that text
/// is resolved by the post-splice gate instead of dangling.
#[test]
fn positional_fallback_key_text_resolves_a_kept_body_link() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>body <ref type=\"bibr\" target=\"#b0\">[1]</ref></p></body><listBibl><biblStruct>undeclared entry</biblStruct></listBibl></text></TEI>";
    let profile = oracle_source_reference_profile(xml);
    assert_eq!(
        profile.positional_fallbacks, 1,
        "the inline document must exercise the positional fallback"
    );
    let (emitted_document, ledger) = emit_preserved_references("inline-positional", xml, &profile);
    let emitted = oracle_emitted_reference_profile(&emitted_document);

    let expected = CitationEntryLinkKeys::new(0, None)
        .published_key()
        .match_text();
    assert_eq!(expected, positional_entry_key(0));
    assert_eq!(
        emitted.published_keys,
        vec![expected],
        "an undeclared entry must publish the contract's positional key text"
    );
    assert_eq!(ledger.required_targets, 1);
    assert_eq!(ledger.resolved_targets, 1);
    assert!(
        emitted.id_space.contains("b0"),
        "the positional key must enter the emitted id space"
    );
}

/// Pins the declared-key side of the oracle: every direct `listBibl` child of
/// the preserved TEI declares `xml:id`, and the source publishes
/// `b0..b{entries-1}`.
#[test]
fn oracle_measures_the_source_published_bibliography_key_domain() {
    for (name, sha256, entries, _, _) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = oracle_source_reference_profile(&xml);
        assert_eq!(
            source.published_entries(),
            entries,
            "{name}: direct listBibl biblStruct children"
        );
        assert_eq!(
            source.positional_fallbacks, 0,
            "{name}: every direct listBibl child declares xml:id in the preserved fixtures"
        );
        let unique: BTreeSet<&String> = source.published_keys.iter().collect();
        assert_eq!(
            unique.len(),
            entries,
            "{name}: source published keys are unique"
        );
        assert_eq!(
            source.published_keys.first().map(String::as_str),
            Some("b0"),
            "{name}: the source publishes GROBID's b0-based keys"
        );
        let last = format!("b{}", entries - 1);
        assert_eq!(
            source.published_keys.last().map(String::as_str),
            Some(last.as_str()),
            "{name}: the source publishes GROBID's b0-based keys contiguously"
        );
    }
}

/// Pins the demand side of the oracle and the audit's totals: 207 `listBibl`
/// entries, 217 local `bibr` occurrences, 120 unique mandatory targets
/// (80/49/78, 87/71/59, 55/41/24).
#[test]
fn oracle_measures_required_targets_and_published_entry_totals() {
    let mut entries = 0usize;
    let mut occurrences = 0usize;
    let mut unique_targets = 0usize;
    for (name, sha256, expected_entries, expected_occurrences, expected_unique) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = oracle_source_reference_profile(&xml);
        assert_eq!(
            source.published_entries(),
            expected_entries,
            "{name}: listBibl entries"
        );
        assert_eq!(
            source.target_occurrences, expected_occurrences,
            "{name}: ref type=bibr local target occurrences"
        );
        assert_eq!(
            source.mandatory_targets.len(),
            expected_unique,
            "{name}: unique mandatory targets"
        );
        let declared: BTreeSet<String> = source.published_keys.iter().cloned().collect();
        let unpublished: Vec<&str> = source
            .mandatory_targets
            .difference(&declared)
            .map(String::as_str)
            .collect();
        assert!(
            unpublished.is_empty(),
            "{name}: every source body target is published by the source listBibl, or the oracle is measuring itself: {unpublished:?}"
        );
        entries += source.published_entries();
        occurrences += source.target_occurrences;
        unique_targets += source.mandatory_targets.len();
    }
    assert_eq!(
        entries, 207,
        "total direct listBibl entries across the three fixtures (80/49/78)"
    );
    assert_eq!(
        occurrences, 217,
        "total ref type=bibr local target occurrences across the three fixtures (87/71/59)"
    );
    assert_eq!(
        unique_targets, 120,
        "total unique mandatory targets across the three fixtures (55/41/24)"
    );
}

/// S02/T02 manifest item 5: `reference_links` is a `serde(default)` field, so a
/// manifest written before the ledger existed still decodes — and the absence
/// degrades to the all-zero record rather than to a decode error or to a stale
/// count.  The ledger is a published record of what was measured, so "not
/// measured" must be representable; otherwise adding the field would break every
/// already-persisted manifest.
#[test]
fn manifest_without_the_ledger_still_decodes_to_the_all_zero_record() {
    let (name, sha256, expected_entries, expected_occurrences, _) = PRESERVED_TEI[0];
    let xml = preserved_tei_source(name, sha256);
    let source = oracle_source_reference_profile(&xml);
    let references = source_backed_references(&source);
    let NativeFulltextCompositionOutcome::Accepted(candidate) =
        compose_native_fulltext(composition_input(&xml, references))
    else {
        panic!("{name}: the audit configuration must be Accepted for the manifest measurement");
    };

    // The current manifest carries a non-vacuous ledger: the default below is
    // therefore a real degradation, not the value the composer always emits.
    assert_eq!(
        candidate.manifest.reference_links.source_entries, expected_entries,
        "{name}: the live ledger must count the source entries"
    );
    assert_eq!(
        candidate.manifest.reference_links.target_occurrences, expected_occurrences,
        "{name}: the live ledger must count the `bibr` occurrences"
    );

    let mut legacy = serde_json::to_value(&candidate.manifest)
        .unwrap_or_else(|error| panic!("{name}: manifest must serialize ({error})"));
    let object = legacy
        .as_object_mut()
        .unwrap_or_else(|| panic!("{name}: manifest must serialize to a JSON object"));
    assert!(
        object.remove("reference_links").is_some(),
        "{name}: the ledger is a top-level manifest field, so the pre-S02 wire shape omits it"
    );
    let decoded: NativeFulltextManifest = serde_json::from_value(legacy)
        .unwrap_or_else(|error| panic!("{name}: a pre-S02 manifest must stay decodable ({error})"));
    assert_eq!(
        decoded.reference_links,
        ReferenceLinkLedger::default(),
        "{name}: an absent ledger must degrade to the all-zero record"
    );
    assert_eq!(
        decoded.schema_version, candidate.manifest.schema_version,
        "{name}: dropping the ledger must not disturb the rest of the manifest"
    );
    assert_eq!(
        decoded.components, candidate.manifest.components,
        "{name}: dropping the ledger must not disturb the component records"
    );
}

/// S02/T02 gate liveness: the post-splice resolve gate refuses `Accepted` for a
/// kept document whose own mandatory target is published nowhere — and it hands
/// back the byte-identical Java envelope.
///
/// This is the configuration only the *post-splice* gate can catch: the reference
/// band is `Unsupported`, so nothing is spliced and the pre-splice key-domain
/// check never runs, while the supplied anchors are `Empty`, so the pre-S02 guard
/// had nothing to check either. The body still carries a live `target="#b0"` and
/// the emitted id space publishes only `java-ref`, so `Accepted` would have been a
/// false claim about the bytes returned.
///
/// No other test in the crate reaches that refusal: the pre-splice guard is
/// exercised by the foreign-anchor case in `native_fulltext_composition_contract`,
/// but the emitted-id comparison could have been unreachable code. The full
/// negative matrix (key collision, domain mismatch, inconservable child,
/// whitespace attribute spellings, refs inside comments/CDATA) is the S02/T03
/// deliverable; this case only proves the T02 gate is live.
#[test]
fn dangling_source_target_is_refused_with_the_byte_identical_java_envelope() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>body <ref type=\"bibr\" target=\"#b0\">[1]</ref></p></body><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></text></TEI>";
    let source = oracle_source_reference_profile(xml);
    assert_eq!(
        source.mandatory_targets,
        BTreeSet::from([String::from("b0")]),
        "the inline document must demand exactly one local target"
    );
    assert!(
        !source.published_keys.iter().any(|key| key.as_str() == "b0"),
        "the source listBibl must not publish the demanded target, or the gate has nothing to refuse"
    );

    // The independent oracle judges the bytes as they are: dangling. A future
    // refactor cannot satisfy this test by making the composer lenient, because
    // the demand is measured from the document, not from the composer.
    let emitted = oracle_emitted_reference_profile(xml);
    let findings = reference_integrity_findings("inline-dangling", &source, &emitted);
    assert_eq!(
        findings.len(),
        1,
        "the oracle must find exactly the unresolved-target defect: {findings:?}"
    );
    assert!(
        findings[0].contains(CLASS_UNRESOLVED_TARGET),
        "the oracle finding must name the unresolved-target class: {}",
        findings[0]
    );

    // The refusal is typed, returns the byte-identical Java envelope, and is
    // reproducible: `Accepted` is a claim about the bytes, so it may never be
    // made for a document that does not resolve its own mandatory target.
    let mut refusals = 0usize;
    for attempt in 1..=2 {
        let references = source_backed_references(&source);
        let outcome = compose_native_fulltext(composition_input_with_reference_admission(
            xml,
            references,
            AdmissionDisposition::Unsupported,
        ));
        match outcome {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                assert!(
                    matches!(
                        error,
                        NativeFulltextCompositionError::UnresolvedBibliographyTarget(_)
                    ),
                    "attempt {attempt}: a dangling mandatory target must be a typed \
                     UnresolvedBibliographyTarget, got {error:?}"
                );
                assert_eq!(
                    fallback.raw_tei(),
                    xml.as_bytes(),
                    "attempt {attempt}: a typed rejection must return the byte-identical Java envelope"
                );
                refusals += 1;
            }
            NativeFulltextCompositionOutcome::Accepted(_) => panic!(
                "attempt {attempt}: a document that does not resolve its own mandatory target may never be Accepted"
            ),
        }
    }
    assert_eq!(refusals, 2, "the same dangling input must refuse twice");
}

/// The gate is about ids, not about damaged XML: the emitted document must be
/// well-formed before the reference oracle judges it (A-XML is already fixed by
/// S01). This keeps a future failure attributable to the right defect.
#[test]
fn emitted_documents_are_well_formed_before_the_reference_oracle_judges_them() {
    for (name, sha256, _, _, _) in PRESERVED_TEI {
        let xml = preserved_tei_source(name, sha256);
        let source = oracle_source_reference_profile(&xml);
        let (emitted_document, _) = emit_preserved_references(name, &xml, &source);
        roxmltree::Document::parse(&emitted_document).unwrap_or_else(|error| {
            panic!(
                "{name}: emitted document must be well-formed before its ids are judged ({error})"
            )
        });
    }
}
