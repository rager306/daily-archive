//! M511-j7wz2f / S04/T03 — RED measurement of the H17 "Byte accounting" scope
//! defect (`doc/review/grobid-absorption-followup.json`, audit 2026-09-21).
//!
//! `fallback_entry` in `native_fulltext_composition.rs` writes, for every
//! component that falls back to Java:
//!
//! ```text
//! evidence_digest: digest(java),   // the *whole* Java document
//! byte_count:      java.len(),     // the *whole* Java document
//! sha256:          digest(java),   // the *whole* Java document
//! ```
//!
//! while `manifest_entry` writes the length and digest of the *native fragment*
//! the composer spliced in.  `FulltextComponentManifest` carries no scope field,
//! so a consumer reading `byte_count` cannot tell "this is how many bytes the
//! native fragment contributed" from "this is how many bytes the fallback
//! evidence document has".  The consequence is arithmetic, not cosmetics: a
//! naive `sum(byte_count)` over the manifest counts the Java document
//! `fallback_components` times and is therefore **not** a partition of the
//! document bytes.  This file measures that profile as data and pins it exactly,
//! failing on deviation in either direction, so T04 must replace these pins
//! (rather than a wishful expectation) when it introduces an explicit scope.
//!
//! ## What is measured here
//!
//! Three scenarios, all through the public composer API (`compose_native_fulltext`)
//! with inline fixtures, and `crates/da-parse/src` untouched:
//!
//! 1. `fallback_components_carry_unscoped_document_byte_counts` — one admitted
//!    native component (header) plus six fallbacks.  Pins: every fallback's
//!    `byte_count`/`sha256`/`evidence_digest` equals the Java document's, the
//!    native component's fragment length differs from `java.len()` (so the
//!    oracle is not vacuous), the serialized manifest field set contains exactly
//!    one byte field and no `scope`/`document`/`emission` companion, and
//!    `sum(byte_count) == java.len() * fallback_components + native_bytes`.
//! 2. `all_fallback_composition_counts_the_document_seven_times` — zero admitted
//!    bands.  The emission is byte-identical to Java and every one of the seven
//!    components reports the whole document, so `sum == 7 * java.len()`.
//! 3. `malformed_java_envelope_is_typed_refusal_with_byte_identical_fallback` —
//!    the failure path: non-UTF-8 input is a typed `InvalidUtf8` refusal whose
//!    fallback bytes are byte-identical to the input (Q5/Q7 negative).
//!
//! Independent (non-manifest) side: the native-partition arithmetic
//! `sum(native byte_count) == emission_len - java_len + sum(replaced source
//! ranges)` is computed from `roxmltree` byte ranges (a dev-dependency only) and
//! the emitted bytes, so the manifest's own numbers are checked against
//! something the manifest did not produce.  This is the GREEN check T04 must
//! keep, with an explicit scope on the fallback side.
//!
//! Run with `-- --nocapture` to read the printed RED profile.

use da_parse::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PROFILE: &str = "profile=fixture";

/// The frozen serialized field set of `FulltextComponentManifest`.  Exactly one
/// byte-accounting field (`byte_count`) and no companion scope/dimension field:
/// a consumer cannot tell a native fragment length from the whole-document
/// fallback reference size.  Pinned as data so T04's scope addition is a
/// deliberate, reviewed change to this list rather than a silent drift.
const MANIFEST_FIELDS: [&str; 8] = [
    "byte_count",
    "component",
    "disposition",
    "evidence_digest",
    "fallback_target",
    "producer",
    "sha256",
    "truncated",
];

const XML: &str = "<TEI><teiHeader><fileDesc><titleStmt><title>Java title</title><affiliation>Java affiliation</affiliation><custom><nested>must survive</nested></custom></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></text></TEI>";

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
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

fn references() -> ReferenceCompositionOutcome {
    compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![NativeReference {
                ordinal: 0,
                source: "Smith 2024".into(),
                span: CitationSpan { start: 0, end: 10 },
                provenance: "fixture".into(),
            }],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    })
}

fn input(
    xml: &str,
    header_admission: AdmissionDisposition,
    reference_admission: AdmissionDisposition,
) -> NativeFulltextCompositionInput {
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, PROFILE),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: SOURCE.into(),
            profile_fingerprint: PROFILE.into(),
            config_fingerprint: "config=fixture".into(),
        },
        header: header(),
        references: references(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: admissions(header_admission, reference_admission),
        s17_lineage: Some(S17LineageReceipt {
            source: "immutable-s17".into(),
            artifact_sha256: ARTIFACT.into(),
            live_ruvllm_receipt: false,
            output_affecting: false,
        }),
    }
}

fn accepted_of(outcome: NativeFulltextCompositionOutcome) -> NativeFulltextCandidate {
    match outcome {
        NativeFulltextCompositionOutcome::Accepted(candidate) => candidate,
        NativeFulltextCompositionOutcome::Rejected { error, .. } => {
            panic!("FATAL fixture must compose as Accepted, got {error:?}")
        }
    }
}

fn component_token(component: FulltextComponent) -> String {
    serde_json::to_value(component)
        .expect("FATAL component must serialize")
        .as_str()
        .expect("FATAL component token must be a string")
        .to_owned()
}

fn disposition_token(disposition: AdmissionDisposition) -> String {
    serde_json::to_value(disposition)
        .expect("FATAL disposition must serialize")
        .as_str()
        .expect("FATAL disposition token must be a string")
        .to_owned()
}

/// The byte length of the source element the composer replaced when it spliced a
/// native component in, read independently through `roxmltree` (never through
/// the composer's own scanner).  Only the two independently admitted bands can
/// splice, so any other native component means the fixture no longer matches the
/// measured profile: FATAL.
fn replaced_source_bytes(xml: &str, component: FulltextComponent) -> usize {
    let name = match component {
        FulltextComponent::Header => "teiHeader",
        FulltextComponent::References => "listBibl",
        other => panic!("FATAL unexpected native component {other:?}"),
    };
    let document = roxmltree::Document::parse(xml)
        .unwrap_or_else(|error| panic!("FATAL fixture is not well-formed XML ({error})"));
    let node = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == name)
        .unwrap_or_else(|| panic!("FATAL fixture declares no <{name}>"));
    node.range().end - node.range().start
}

fn is_native(manifest: &FulltextComponentManifest) -> bool {
    manifest.disposition == AdmissionDisposition::Passed
}

#[test]
fn fallback_components_carry_unscoped_document_byte_counts() {
    let candidate = accepted_of(compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )));
    let manifest = &candidate.manifest;
    let java = XML.as_bytes();

    // The manifest exposes the whole Java document as the fallback reference.
    assert_eq!(
        manifest.java_fallback_bytes,
        java.len(),
        "FATAL java_fallback_bytes drifted"
    );
    assert_eq!(
        manifest.java_fallback_sha256,
        digest(java),
        "FATAL java_fallback_sha256 drifted"
    );

    for component in &manifest.components {
        println!(
            "[H17-T03 RED] component={} disposition={} byte_count={} java_fallback_bytes={}",
            component_token(component.component),
            disposition_token(component.disposition),
            component.byte_count,
            manifest.java_fallback_bytes
        );
    }

    let natives: Vec<&FulltextComponentManifest> = manifest
        .components
        .iter()
        .filter(|m| is_native(m))
        .collect();
    let fallbacks: Vec<&FulltextComponentManifest> = manifest
        .components
        .iter()
        .filter(|m| !is_native(m))
        .collect();
    assert_eq!(
        natives.len(),
        1,
        "FATAL expected exactly one native component"
    );
    assert_eq!(
        fallbacks.len(),
        6,
        "FATAL expected exactly six fallback components"
    );
    assert_eq!(
        manifest.native_components,
        natives.len(),
        "FATAL native_components drifted"
    );
    assert_eq!(
        manifest.fallback_components,
        fallbacks.len(),
        "FATAL fallback_components drifted"
    );

    // Defect, measured as data: every fallback component carries the *whole*
    // Java document as its byte count, evidence digest and sha256 — with no
    // field saying so.
    for component in &fallbacks {
        assert_eq!(
            component.byte_count, manifest.java_fallback_bytes,
            "FATAL fallback byte_count no longer equals the document size for {:?}",
            component.component
        );
        assert_eq!(
            component.sha256, manifest.java_fallback_sha256,
            "FATAL fallback sha256 no longer equals the document digest for {:?}",
            component.component
        );
        assert_eq!(
            component.evidence_digest, manifest.java_fallback_sha256,
            "FATAL fallback evidence_digest no longer equals the document digest for {:?}",
            component.component
        );
    }

    // Oracle non-vacuity: a native component carries a *fragment* length and
    // digest, distinct from the whole document, so "fallback == document size" is
    // a real statement about attribution and not an artefact of a trivial fixture.
    for component in &natives {
        assert_ne!(
            component.byte_count, manifest.java_fallback_bytes,
            "FATAL native {:?} reports the whole document length; the fixture cannot distinguish scope",
            component.component
        );
        assert_ne!(
            component.sha256, manifest.java_fallback_sha256,
            "FATAL native {:?} reports the whole document digest",
            component.component
        );
    }

    // No scope field: the manifest's serialized shape carries exactly one byte
    // accounting field and no companion naming what `byte_count` measures.
    let shape = serde_json::to_value(&manifest.components[0])
        .expect("FATAL manifest component must serialize");
    let keys: BTreeSet<String> = shape
        .as_object()
        .unwrap_or_else(|| panic!("FATAL manifest component is not a JSON object"))
        .keys()
        .cloned()
        .collect();
    let expected: BTreeSet<String> = MANIFEST_FIELDS
        .iter()
        .map(|field| (*field).to_owned())
        .collect();
    assert_eq!(
        keys, expected,
        "FATAL manifest field set drifted; replace this pin deliberately when adding a byte-count scope"
    );
    assert!(
        !keys.iter().any(|key| {
            key.contains("scope") || key.contains("document") || key.contains("emission")
        }),
        "FATAL manifest grew a byte-count scope field without updating this RED pin"
    );

    // The naive sum is not a partition of the document: the Java document is
    // counted once per fallback component, on top of the native fragment bytes.
    let sum_all: usize = manifest.components.iter().map(|m| m.byte_count).sum();
    let native_sum: usize = natives.iter().map(|m| m.byte_count).sum();
    let fallback_sum: usize = fallbacks.iter().map(|m| m.byte_count).sum();
    println!(
        "[H17-T03 RED] native_sum={native_sum} fallback_sum={fallback_sum} sum_all={sum_all} java_fallback_bytes={} fallback_components={}",
        manifest.java_fallback_bytes, manifest.fallback_components
    );
    assert_eq!(
        sum_all,
        manifest.java_fallback_bytes * manifest.fallback_components + native_sum,
        "FATAL sum(byte_count) is no longer document*fallbacks + native_fragment"
    );
    assert!(
        sum_all > manifest.java_fallback_bytes,
        "FATAL sum(byte_count) is expected to exceed the document length (document counted per fallback)"
    );

    // Native partition invariant — the future GREEN check.  The native bytes must
    // be attributable to the splice: total emitted growth equals the native
    // fragment bytes minus the source bytes they replaced.
    let emission_len = candidate.envelope.raw_tei().len();
    let replaced: usize = natives
        .iter()
        .map(|m| replaced_source_bytes(XML, m.component))
        .sum();
    assert!(
        native_sum <= emission_len,
        "FATAL native fragment bytes exceed the emission length"
    );
    assert_eq!(
        native_sum,
        emission_len - manifest.java_fallback_bytes + replaced,
        "FATAL native byte_count is not equal to the sum of the spliced fragment lengths"
    );
}

#[test]
fn all_fallback_composition_counts_the_document_seven_times() {
    let candidate = accepted_of(compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Unsupported,
        AdmissionDisposition::Unsupported,
    )));
    let manifest = &candidate.manifest;
    let java = XML.as_bytes();

    assert_eq!(
        candidate.envelope.raw_tei(),
        java,
        "FATAL an all-fallback composition must return the byte-identical Java envelope"
    );
    assert_eq!(
        manifest.native_components, 0,
        "FATAL expected no native components"
    );
    assert_eq!(
        manifest.fallback_components, 7,
        "FATAL expected all seven components to fall back"
    );
    for component in &manifest.components {
        assert!(
            !is_native(component),
            "FATAL {:?} must fall back in this fixture",
            component.component
        );
        assert_eq!(
            component.byte_count, manifest.java_fallback_bytes,
            "FATAL {:?} must report the whole document",
            component.component
        );
        assert_eq!(
            component.sha256, manifest.java_fallback_sha256,
            "FATAL {:?} must report the whole document digest",
            component.component
        );
    }
    let sum_all: usize = manifest.components.iter().map(|m| m.byte_count).sum();
    println!(
        "[H17-T03 RED] all-fallback sum_all={sum_all} java_fallback_bytes={} components={}",
        manifest.java_fallback_bytes, manifest.fallback_components
    );
    assert_eq!(
        sum_all,
        manifest.java_fallback_bytes * 7,
        "FATAL all-fallback sum(byte_count) must count the document once per component"
    );
}

#[test]
fn malformed_java_envelope_is_typed_refusal_with_byte_identical_fallback() {
    // Invalid UTF-8 after a `<` opener: the composer must refuse the bytes with a
    // typed error rather than attribute a byte count to a document it could not
    // read, and must hand back the verbatim input.
    let malformed: &[u8] = &[0x3c, 0x54, 0x45, 0xff, 0xfe, 0x3e];
    let mut composition_input = input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    );
    composition_input.java_fulltext =
        TeiEnvelope::new(malformed, TeiIdentity::new(GROBID_HTTP_BACKEND_ID, PROFILE));

    match compose_native_fulltext(composition_input) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert!(
                matches!(error, NativeFulltextCompositionError::InvalidUtf8),
                "FATAL malformed UTF-8 must be a typed InvalidUtf8 refusal, got {error:?}"
            );
            assert_eq!(
                fallback.raw_tei(),
                malformed,
                "FATAL the typed refusal must return the byte-identical fallback envelope"
            );
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("FATAL invalid UTF-8 must never be accepted")
        }
    }
}
