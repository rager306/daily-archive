//! Independent `roxmltree` judge for the S06 one-lane promotion rehearsal.
//!
//! Rule of evidence #6 (CONTEXT, S06): numbers produced by one instrument on
//! both sides are not independence. The rehearsal example claims that switching
//! the declared `da-parse.native_header_reference_composition` candidate's
//! header lane to the native route changes exactly one band and that returning
//! to Java restores the baseline bytes. A claim about *emitted bytes* must be
//! judged by an instrument that is neither the example nor the example's own
//! report field.
//!
//! This file is that instrument. It never imports
//! `crates/da-cli/examples/native_family_promotion_rehearsal.rs` and never reads
//! that example's artifact: it rebuilds the same three emissions from the frozen
//! fixture `fixtures/native_fulltext_composition_oracle.yaml` through da-parse's
//! public API and judges them with `roxmltree`, an XML parser that is not the
//! composer's own tag scanner and not the example's boolean summary.
//!
//! Why the judge lives here and cannot live in `da-cli`: `roxmltree` is a
//! dev-dependency of `da-parse` only. The composer's import-root set stays
//! `{crate, serde, sha2, std}` (checked by
//! `architecture_contract::native_fulltext_composition_is_pure_and_stays_outside_every_runtime_route`),
//! so an independent XML parser can exist in this crate's tests and nowhere
//! else.

use da_parse::{
    AdmissionDisposition, AffiliationSpan, CompositionSpan, FulltextAdmissionReceipt,
    FulltextComponent, FulltextCompositionIdentity, GROBID_HTTP_BACKEND_ID, HeaderAuthorInput,
    HeaderCompositionInput, HeaderCompositionOutcome, NATIVE_FULLTEXT_FALLBACK_TARGET,
    NativeAffiliationAuthorEvidence, NativeAffiliationLinkOutcome, NativeDateParser,
    NativeFulltextCompositionInput, NativeFulltextCompositionOutcome, NativeHeaderFields,
    NativePersonalNameParser, PersonalNameContext, ReferenceCompositionOutcome, TeiEnvelope,
    TeiIdentity, compose_header_with_fields, compose_native_fulltext,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/native_fulltext_composition_oracle.yaml");

#[derive(Debug, Deserialize)]
struct Fixture {
    source_sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    source_xml: String,
}

fn fixture() -> Fixture {
    serde_yaml::from_str(FIXTURE).expect("oracle fixture parses")
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn identity_for(f: &Fixture, java: &[u8]) -> FulltextCompositionIdentity {
    FulltextCompositionIdentity {
        source_sha256: sha(java),
        profile_fingerprint: f.profile_fingerprint.clone(),
        config_fingerprint: f.config_fingerprint.clone(),
    }
}

/// The native header band the oracle admits, built HERE from da-parse's public
/// typed parsers. The example's own `fixture_header_outcome` is never imported
/// and its literal values are never copied; only the typed construction shape
/// (one dated author, no affiliation links) is shared, because that is the
/// public API's contract, not the example's output.
fn native_header_band() -> HeaderCompositionOutcome {
    compose_header_with_fields(
        HeaderCompositionInput {
            date: NativeDateParser::new().parse("2024-02-29"),
            authors: vec![HeaderAuthorInput {
                ordinal: 0,
                span: CompositionSpan { start: 1, end: 9 },
                name: NativePersonalNameParser::new()
                    .parse(PersonalNameContext::Header, "O. Judge"),
                affiliations: NativeAffiliationLinkOutcome::Linked {
                    author: NativeAffiliationAuthorEvidence {
                        ordinal: 0,
                        span: AffiliationSpan { start: 0, end: 1 },
                    },
                    links: vec![],
                },
            }],
            affiliation_components: vec![],
        },
        NativeHeaderFields {
            title: vec!["Oracle judge title".into()],
            email: vec!["oracle@example.invalid".into()],
            ids: vec!["ORACLE-REHEARSAL-1".into()],
        },
    )
}

/// The seven admission receipts the composer requires, with the rehearsed lane's
/// disposition chosen by the caller and every other band pinned to the declared
/// `admission.current_matrix` fallback dispositions.
fn receipts(
    identity: &FulltextCompositionIdentity,
    header_disposition: AdmissionDisposition,
) -> Vec<FulltextAdmissionReceipt> {
    let source = identity.source_sha256.as_str();
    let receipt = |component: FulltextComponent,
                   disposition: AdmissionDisposition,
                   evidence_state: &str| FulltextAdmissionReceipt {
        component,
        producer: "oracle-rehearsal".into(),
        disposition,
        evidence_state: evidence_state.into(),
        schema_version: "native-composition.outcome/v1".into(),
        source_sha256: source.into(),
        artifact_sha256: String::new(),
        fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
    };
    vec![
        receipt(FulltextComponent::Header, header_disposition, "passing"),
        receipt(
            FulltextComponent::References,
            AdmissionDisposition::Fallback,
            "passing",
        ),
        receipt(
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::Fallback,
            "invalid_evidence",
        ),
        receipt(
            FulltextComponent::Figure,
            AdmissionDisposition::Fallback,
            "unsupported",
        ),
        receipt(
            FulltextComponent::Table,
            AdmissionDisposition::Fallback,
            "not_observed",
        ),
        receipt(
            FulltextComponent::Formula,
            AdmissionDisposition::Fallback,
            "unsupported",
        ),
        receipt(
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
            "not_observed",
        ),
    ]
}

/// One emission of the triad. `accepted` is the composer's own outcome and
/// `bytes` is the document it returned, so a rollback is judged on bytes and
/// never on a boolean field.
struct Emission {
    accepted: bool,
    bytes: Vec<u8>,
    detail: Option<String>,
}

fn emission(
    java: &[u8],
    f: &Fixture,
    header: &HeaderCompositionOutcome,
    header_disposition: AdmissionDisposition,
) -> Emission {
    let identity = identity_for(f, java);
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            java,
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, &f.profile_fingerprint),
        ),
        identity: identity.clone(),
        header: header.clone(),
        // The references lane is not rehearsed here: no native bibliography band
        // is produced, so the lane stays Java in every emission. The composer
        // never inspects this value unless the lane is admitted.
        references: ReferenceCompositionOutcome::Rejected {
            error: da_parse::CompositionError::RejectedComponent {
                component: da_parse::CompositionComponent::References,
            },
        },
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: receipts(&identity, header_disposition),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => Emission {
            accepted: true,
            bytes: candidate.envelope.raw_tei().to_vec(),
            detail: None,
        },
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => Emission {
            accepted: false,
            bytes: fallback.raw_tei().to_vec(),
            detail: Some(format!("{error:?}")),
        },
    }
}

// ───────────────────────── the independent judge ─────────────────────────

/// A canonical, structure-only rendering of one roxmltree node: element names,
/// sorted attributes and leaf text. Built from the parser's node tree, so the
/// equality it supports is structural and never a substring match.
fn canonical(node: roxmltree::Node<'_, '_>, out: &mut String) {
    use roxmltree::NodeType;
    match node.node_type() {
        NodeType::Root => {
            for child in node.children() {
                canonical(child, out);
            }
        }
        NodeType::Element => {
            out.push('<');
            out.push_str(node.tag_name().name());
            let mut attributes: Vec<(String, String)> = node
                .attributes()
                .map(|attribute| {
                    let name = match attribute.namespace() {
                        Some(namespace) => format!("{{{namespace}}}{}", attribute.name()),
                        None => attribute.name().to_owned(),
                    };
                    (name, attribute.value().to_owned())
                })
                .collect();
            attributes.sort();
            for (name, value) in attributes {
                out.push(' ');
                out.push_str(&name);
                out.push('=');
                out.push_str(&format!("{value:?}"));
            }
            out.push('>');
            for child in node.children() {
                canonical(child, out);
            }
            out.push_str("</");
            out.push_str(node.tag_name().name());
            out.push('>');
        }
        NodeType::Text => {
            out.push_str("text:");
            out.push_str(&format!("{:?}", node.text().unwrap_or_default()));
        }
        NodeType::Comment => {
            out.push_str("comment:");
            out.push_str(&format!("{:?}", node.text().unwrap_or_default()));
        }
        NodeType::PI => {
            out.push_str("pi:");
            out.push_str(&format!("{:?}", node.text().unwrap_or_default()));
        }
    }
}

fn parse(bytes: &[u8]) -> roxmltree::Document<'_> {
    let text = std::str::from_utf8(bytes).expect("emission is valid UTF-8");
    roxmltree::Document::parse(text).expect("emission is well-formed XML")
}

fn canonical_children(doc: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    doc.root_element()
        .children()
        .filter(|node| node.is_element())
        .map(|node| {
            let mut rendered = String::new();
            canonical(node, &mut rendered);
            (node.tag_name().name().to_owned(), rendered)
        })
        .collect()
}

fn element_bytes(bytes: &[u8], name: &str) -> Vec<u8> {
    let doc = parse(bytes);
    let node = doc
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == name)
        .unwrap_or_else(|| panic!("emission has no <{name}> element"));
    let range = node.range();
    bytes[range.start..range.end].to_vec()
}

fn element_text(bytes: &[u8], name: &str) -> Option<String> {
    let doc = parse(bytes);
    doc.descendants()
        .find(|node| node.is_element() && node.tag_name().name() == name)
        .and_then(|node| node.text())
        .map(str::to_owned)
}

// ───────────────────────────── the tests ─────────────────────────────────

#[test]
fn rehearsal_switched_emission_moves_exactly_the_header_band() {
    let f = fixture();
    let java = f.source_xml.as_bytes().to_vec();
    assert_eq!(
        sha(&java),
        f.source_sha256,
        "the frozen fixture checksum must bind the Java bytes, or nothing below is evidence"
    );
    let band = native_header_band();

    let baseline = emission(&java, &f, &band, AdmissionDisposition::Fallback);
    let switched = emission(&java, &f, &band, AdmissionDisposition::Passed);
    let returned = emission(&java, &f, &band, AdmissionDisposition::Fallback);

    // (a) The accepted-without-splice branch: with the lane on Java, the
    // composer does not touch a single byte. This is the ground the return
    // invariant stands on, and it is asserted on bytes, not on a flag.
    assert!(
        baseline.accepted,
        "a fallback-only emission must be accepted: {:?}",
        baseline.detail
    );
    assert_eq!(
        baseline.bytes, java,
        "a fallback-only emission must be the Java document byte-for-byte"
    );

    // (b) The switch must actually splice the native header band.
    assert!(
        switched.accepted,
        "the native header band must be admitted so the switch is non-vacuous: {:?}",
        switched.detail
    );

    // (c) The executed rollback, asserted on the bytes of the returned document.
    assert_eq!(
        returned.bytes, baseline.bytes,
        "the executed rollback must return the baseline document byte-for-byte"
    );
    assert_eq!(returned.accepted, baseline.accepted);

    // (d) Independent judge: parse BOTH sides with roxmltree. A well-formedness
    // failure here is an emission the composer should not have produced.
    let base_doc = parse(&baseline.bytes);
    let switched_doc = parse(&switched.bytes);
    let base_children = canonical_children(&base_doc);
    let switched_children = canonical_children(&switched_doc);

    // The number of root element children is unchanged...
    assert_eq!(
        switched_children.len(),
        base_children.len(),
        "the splice added or removed a root child: {switched_children:?} vs {base_children:?}"
    );
    assert_eq!(
        base_children.len(),
        2,
        "frozen fixture must have two root children"
    );

    // ...every child except the header is canonically identical...
    let non_header = |children: &[(String, String)]| -> Vec<(String, String)> {
        children
            .iter()
            .filter(|(name, _)| name != "teiHeader")
            .cloned()
            .collect()
    };
    assert_eq!(
        non_header(&switched_children),
        non_header(&base_children),
        "the switch moved a band other than teiHeader"
    );

    // ...and the header itself differs.
    let header = |children: &[(String, String)]| -> String {
        children
            .iter()
            .find(|(name, _)| name == "teiHeader")
            .map(|(_, rendered)| rendered.clone())
            .expect("a teiHeader child")
    };
    assert_ne!(
        header(&switched_children),
        header(&base_children),
        "the switch changed nothing the judge can see"
    );

    // The admitted band reached the emission: the judge reads the oracle's own
    // title out of the switched document, so the byte difference above is the
    // band and not some unrelated rewrite.
    assert_eq!(
        element_text(&switched.bytes, "title").as_deref(),
        Some("Oracle judge title"),
        "the admitted native title did not reach the emission"
    );

    // (e) Independent one-band check: the emitted listBibl subtree (the
    // references band, which the switch never moved) is byte-identical, and so
    // is the body. Ranges come from roxmltree, not from the composer's scanner.
    assert_eq!(
        element_bytes(&switched.bytes, "listBibl"),
        element_bytes(&baseline.bytes, "listBibl"),
        "the switch moved the references band"
    );
    assert_eq!(
        element_bytes(&switched.bytes, "body"),
        element_bytes(&baseline.bytes, "body"),
        "the switch moved the body"
    );
}

#[test]
fn rehearsal_fallback_is_the_java_clone_on_the_rejected_path() {
    let f = fixture();
    // A document whose own kept body demands a `#fragment` the emitted id space
    // cannot satisfy: the composer must refuse rather than republish a dangling
    // link, and the refusal must return the byte-identical Java envelope.
    let java = f
        .source_xml
        .replace(
            "</p>",
            " and <ref type=\"bibr\" target=\"#ghost\">[2]</ref></p>",
        )
        .into_bytes();
    assert_ne!(
        sha(&java),
        f.source_sha256,
        "the mutation must escape the frozen checksum"
    );
    let band = native_header_band();
    let rejected = emission(&java, &f, &band, AdmissionDisposition::Fallback);

    assert!(
        !rejected.accepted,
        "a dangling mandatory target must be a typed refusal, not a silent accept"
    );
    assert_eq!(
        rejected.bytes, java,
        "the typed refusal must return the byte-identical Java envelope"
    );
    let doc = parse(&rejected.bytes);
    assert_eq!(doc.root_element().tag_name().name(), "TEI");
}
