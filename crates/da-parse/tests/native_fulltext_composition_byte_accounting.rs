//! M511-j7wz2f / S04 — H17 "Byte accounting": every component byte count
//! carries an explicit scope, and only native fragments partition the emission.
//!
//! T03 measured the defect as data (`doc/review/grobid-absorption-followup.json`,
//! audit 2026-09-21): `fallback_entry` in `native_fulltext_composition.rs` wrote,
//! for every component that falls back to Java,
//!
//! ```text
//! evidence_digest: digest(java),   // the *whole* Java document
//! byte_count:      java.len(),     // the *whole* Java document
//! sha256:          digest(java),   // the *whole* Java document
//! ```
//!
//! while `manifest_entry` wrote the length and digest of the *native fragment*
//! the composer spliced in. `FulltextComponentManifest` carried no scope field,
//! so a consumer reading `byte_count` could not tell "how many bytes the native
//! fragment contributed" from "how many bytes the fallback evidence document
//! has". The consequence was arithmetic: a naive `sum(byte_count)` over the
//! manifest counted the Java document `fallback_components` times and was
//! therefore **not** a partition of the document bytes.
//!
//! T04 repaired it by making the scope a field, not a convention:
//!
//! * `FulltextComponentManifest::byte_scope` is `NativeFragment` for a spliced
//!   native fragment and `WholeDocumentReference` for a component that defers to
//!   the preserved Java document;
//! * `NativeFulltextManifest::byte_accounting` records the partition once
//!   (`native_fragment_bytes` / `java_document_bytes` /
//!   `fallback_is_reference_not_partition`), and
//!   `NativeFulltextManifest::native_byte_partition()` re-derives it from the
//!   components — the two must agree;
//! * `NativeFulltextManifest::byte_accounting_violations()` is the detector: a
//!   substituted scope cannot satisfy it.
//!
//! ## What this GREEN contract asserts
//!
//! All scenarios go through the public composer API (`compose_native_fulltext`)
//! with inline fixtures:
//!
//! 1. `component_byte_counts_carry_their_scope_and_only_native_fragments_partition`
//!    — one admitted native band (header) and six fallbacks. Every fallback is a
//!    `WholeDocumentReference` repeating `java.len()`/`digest(java)`; the native
//!    band is a `NativeFragment` whose length differs from `java.len()` (so the
//!    oracle is not vacuous); `native_byte_partition()` equals the independently
//!    derived splice arithmetic and never exceeds the emission; the naive
//!    `sum(byte_count)` is the document counted once per fallback plus the native
//!    fragment, which is exactly why it is not a partition; the manifest is
//!    violation-free; and the whole-document reference is never confused with the
//!    native partition.
//! 2. `all_fallback_composition_counts_the_document_seven_times` — zero admitted
//!    bands. The emission is byte-identical to Java, all seven components are
//!    `WholeDocumentReference`, and `native_byte_partition() == 0`.
//! 3. `substituted_scope_is_refused_as_non_additive` — the negative: flipping one
//!    fallback component's `byte_scope` to `NativeFragment` is caught, both as a
//!    non-admitted splice and as a partition that no longer matches the recorded
//!    ledger (the whole document is then counted inside the "native" side).
//! 4. `detector_names_each_byte_accounting_violation_class` — every remaining
//!    violation class (a reference that does not repeat the document, a ledger
//!    with a different Java length, a ledger that claims the fallback partitions
//!    the emission, a partition that disagrees with the ledger) is reached from a
//!    real manifest.
//! 5. `serde_roundtrip_preserves_scope_and_legacy_manifests_decode_as_native_fragment`
//!    — the scope survives serialization; a manifest without `byte_scope` still
//!    decodes (`#[serde(default)]`) and is reported as inconsistent rather than
//!    silently trusted.
//! 6. `malformed_java_envelope_is_typed_refusal_with_byte_identical_fallback` —
//!    the failure path: non-UTF-8 input is a typed `InvalidUtf8` refusal whose
//!    fallback bytes are byte-identical to the input (Q5/Q7 negative).
//!
//! Independent (non-manifest) side: the native-partition arithmetic
//! `sum(native byte_count) == emission_len - java_len + sum(replaced source
//! ranges)` is computed from `roxmltree` byte ranges (a dev-dependency only) and
//! the emitted bytes, so the manifest's own numbers are checked against
//! something the manifest did not produce.
//!
//! Run with `-- --nocapture` to read the printed GREEN profile.

use da_parse::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PROFILE: &str = "profile=fixture";

/// The frozen serialized field set of `FulltextComponentManifest`.  Since S04/T04
/// there are exactly two byte-accounting fields: `byte_count` and the
/// `byte_scope` that qualifies it.  Pinned as data so any further accounting
/// addition is a deliberate, reviewed change to this list rather than a silent
/// drift.
const MANIFEST_FIELDS: [&str; 9] = [
    "byte_count",
    "byte_scope",
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

fn scope_token(scope: ByteCountScope) -> String {
    serde_json::to_value(scope)
        .expect("FATAL byte scope must serialize")
        .as_str()
        .expect("FATAL byte scope token must be a string")
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
fn component_byte_counts_carry_their_scope_and_only_native_fragments_partition() {
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
            "[H17-T04 GREEN] component={} disposition={} byte_scope={} byte_count={} java_fallback_bytes={}",
            component_token(component.component),
            disposition_token(component.disposition),
            scope_token(component.byte_scope),
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

    // Every fallback component is an explicit whole-document *reference*: its
    // `byte_count`/`sha256`/`evidence_digest` repeat the preserved Java document,
    // and `byte_scope` now says so.
    for component in &fallbacks {
        assert_eq!(
            component.byte_scope,
            ByteCountScope::WholeDocumentReference,
            "FATAL fallback {:?} must declare the whole-document reference scope",
            component.component
        );
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

    // Every native component is an explicit fragment: a length and digest that
    // differ from the whole document, so "fallback == document size" is a real
    // statement about attribution rather than an artefact of a trivial fixture.
    for component in &natives {
        assert_eq!(
            component.byte_scope,
            ByteCountScope::NativeFragment,
            "FATAL native {:?} must declare the native-fragment scope",
            component.component
        );
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

    // The scope is part of the serialized shape, and the field set is exactly the
    // pinned one — a further byte-accounting field is a deliberate change.
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
        "FATAL manifest field set drifted; replace this pin deliberately when adding a byte-accounting field"
    );
    assert!(
        keys.contains("byte_scope"),
        "FATAL the manifest must carry the byte-count scope"
    );

    // The naive sum is not a partition: the Java document is counted once per
    // fallback component, on top of the native fragment bytes.
    let sum_all: usize = manifest.components.iter().map(|m| m.byte_count).sum();
    let native_sum: usize = natives.iter().map(|m| m.byte_count).sum();
    let fallback_sum: usize = fallbacks.iter().map(|m| m.byte_count).sum();
    println!(
        "[H17-T04 GREEN] native_sum={native_sum} fallback_sum={fallback_sum} sum_all={sum_all} java_fallback_bytes={} fallback_components={}",
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

    // The partition helper excludes the whole-document references by scope, so it
    // counts the document zero times as a fallback and exactly once as a native
    // fragment.
    let partition = manifest.native_byte_partition();
    assert_eq!(
        partition, native_sum,
        "FATAL native_byte_partition must equal the native-fragment bytes"
    );
    assert_eq!(
        sum_all - partition,
        manifest.java_fallback_bytes * manifest.fallback_components,
        "FATAL every non-native byte must be the whole-document reference"
    );

    // Native partition invariant.  The native bytes are attributable to the
    // splice: emitted growth equals the native fragment bytes minus the source
    // bytes they replaced, computed independently through `roxmltree`.
    let emission_len = candidate.envelope.raw_tei().len();
    let replaced: usize = natives
        .iter()
        .map(|m| replaced_source_bytes(XML, m.component))
        .sum();
    assert!(
        partition <= emission_len,
        "FATAL native fragment bytes exceed the emission length"
    );
    assert_eq!(
        partition,
        emission_len - manifest.java_fallback_bytes + replaced,
        "FATAL native byte_count is not equal to the sum of the spliced fragment lengths"
    );

    // The whole-document reference is not the native partition.
    assert_ne!(
        manifest.java_fallback_bytes, partition,
        "FATAL the whole-document reference must not be confused with the native partition"
    );

    // The manifest-level ledger records the partition once, and agrees with the
    // re-derived helper.
    let ledger = manifest.byte_accounting;
    println!(
        "[H17-T04 GREEN] byte_accounting native_fragment_bytes={} java_document_bytes={} fallback_is_reference_not_partition={}",
        ledger.native_fragment_bytes,
        ledger.java_document_bytes,
        ledger.fallback_is_reference_not_partition
    );
    assert_eq!(
        ledger.native_fragment_bytes, partition,
        "FATAL the ledger must record the native partition"
    );
    assert_eq!(
        ledger.java_document_bytes, manifest.java_fallback_bytes,
        "FATAL the ledger must record the preserved Java document length"
    );
    assert!(
        ledger.fallback_is_reference_not_partition,
        "FATAL the ledger must record that a whole-document fallback is not a partition member"
    );

    // The production detector is live on the composer's own output.
    assert_eq!(
        manifest.byte_accounting_violations(),
        Vec::new(),
        "FATAL the composer's manifest must carry no byte-accounting violation"
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
            component.byte_scope,
            ByteCountScope::WholeDocumentReference,
            "FATAL {:?} must declare the whole-document reference scope",
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
        "[H17-T04 GREEN] all-fallback sum_all={sum_all} java_fallback_bytes={} components={} native_byte_partition={}",
        manifest.java_fallback_bytes,
        manifest.fallback_components,
        manifest.native_byte_partition()
    );
    assert_eq!(
        sum_all,
        manifest.java_fallback_bytes * 7,
        "FATAL all-fallback sum(byte_count) must count the document once per component"
    );
    // Nothing partitions the emission: with no native band the partition is empty
    // even though the document is named seven times.
    assert_eq!(
        manifest.native_byte_partition(),
        0,
        "FATAL an all-fallback manifest has no native partition"
    );
    assert_eq!(
        manifest.byte_accounting.native_fragment_bytes, 0,
        "FATAL the ledger must record an empty native partition"
    );
    assert!(manifest.byte_accounting.fallback_is_reference_not_partition);
    assert_eq!(
        manifest.byte_accounting_violations(),
        Vec::new(),
        "FATAL an all-fallback manifest must carry no byte-accounting violation"
    );
}

/// The negative: a substituted scope cannot satisfy the invariant.
///
/// Flipping one fallback component's `byte_scope` to `NativeFragment` makes the
/// record self-contradictory in the two ways the detector names — a
/// non-admitted splice inside the native partition, and a re-derived partition
/// that no longer matches the ledger the composer recorded.  The whole-document
/// reference is then counted inside the "native" side, which is exactly the
/// non-additivity H17 measured.
#[test]
fn substituted_scope_is_refused_as_non_additive() {
    let candidate = accepted_of(compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )));
    let honest = &candidate.manifest;
    assert_eq!(
        honest.byte_accounting_violations(),
        Vec::new(),
        "the composer's own manifest must be consistent before the mutation"
    );

    let mut mutated = honest.clone();
    let victim = mutated
        .components
        .iter_mut()
        .find(|component| component.byte_scope == ByteCountScope::WholeDocumentReference)
        .expect("FATAL the fixture must have a fallback component");
    let victim_component = victim.component;
    victim.byte_scope = ByteCountScope::NativeFragment;

    let violations = mutated.byte_accounting_violations();
    assert!(
        violations.contains(
            &ByteAccountingViolation::NativeFragmentWithoutAdmittedSplice {
                component: victim_component,
            }
        ),
        "the detector must name the mis-declared component: {violations:?}"
    );
    assert!(
        violations.iter().any(|violation| matches!(
            violation,
            ByteAccountingViolation::NativePartitionDisagreesWithLedger { .. }
        )),
        "the re-derived partition must disagree with the recorded ledger: {violations:?}"
    );

    // Non-additivity, measured: the document reference is now on the native side.
    let honest_partition = honest.native_byte_partition();
    assert_eq!(
        mutated.native_byte_partition(),
        honest_partition + honest.java_fallback_bytes,
        "FATAL the substituted scope must add one whole-document reference to the native side"
    );
    let emission_len = candidate.envelope.raw_tei().len();
    assert!(
        mutated.native_byte_partition() > emission_len,
        "FATAL the non-additive partition is expected to exceed the emission"
    );
}

/// The detector names each violation class, so the scope and the recorded
/// partition cannot be mis-declared in any of the ways the contract rules out.
/// Together with the non-additivity case above, every
/// [`ByteAccountingViolation`] class is reached from a real manifest.
#[test]
fn detector_names_each_byte_accounting_violation_class() {
    let candidate = accepted_of(compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )));
    let honest = &candidate.manifest;
    let honest_partition = honest.native_byte_partition();

    // A whole-document reference whose digest does not repeat the document.
    let mut reference_drift = honest.clone();
    let victim = reference_drift
        .components
        .iter_mut()
        .find(|component| component.byte_scope == ByteCountScope::WholeDocumentReference)
        .expect("FATAL the fixture must have a fallback component");
    let victim_component = victim.component;
    victim.sha256 = digest(b"not the preserved document");
    assert!(
        reference_drift.byte_accounting_violations().contains(
            &ByteAccountingViolation::ReferenceDoesNotRepeatDocument {
                component: victim_component,
            }
        ),
        "a reference that does not repeat the preserved document must be named"
    );

    // A ledger that repeats a different Java document length.
    let mut length_drift = honest.clone();
    length_drift.byte_accounting.java_document_bytes = honest.java_fallback_bytes + 1;
    assert!(
        length_drift.byte_accounting_violations().contains(
            &ByteAccountingViolation::JavaDocumentLengthDisagreesWithManifest {
                derived: honest.java_fallback_bytes,
                recorded: honest.java_fallback_bytes + 1,
            }
        ),
        "a ledger whose java_document_bytes disagrees with the manifest must be named"
    );

    // A ledger that claims the whole-document fallback is a partition member.
    let mut partition_claim = honest.clone();
    partition_claim
        .byte_accounting
        .fallback_is_reference_not_partition = false;
    assert!(
        partition_claim
            .byte_accounting_violations()
            .contains(&ByteAccountingViolation::FallbackDeclaredAsPartition),
        "a ledger that claims the fallback partitions the emission must be named"
    );

    // A re-derived partition that no longer matches the recorded ledger.
    let mut partition_drift = honest.clone();
    partition_drift.byte_accounting.native_fragment_bytes = honest_partition + 1;
    assert!(
        partition_drift.byte_accounting_violations().contains(
            &ByteAccountingViolation::NativePartitionDisagreesWithLedger {
                derived: honest_partition,
                recorded: honest_partition + 1,
            }
        ),
        "a partition that disagrees with the recorded ledger must be named"
    );
}

/// Serde contract: the scope survives a round trip, and a manifest serialized
/// before S04/T04 (`#[serde(default)]`, no `byte_scope`, no `byte_accounting`)
/// still decodes — but is reported as inconsistent instead of silently trusted.
#[test]
fn serde_roundtrip_preserves_scope_and_legacy_manifests_decode_as_native_fragment() {
    let candidate = accepted_of(compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )));
    let manifest = &candidate.manifest;

    let value = serde_json::to_value(manifest).expect("FATAL manifest must serialize");
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("FATAL manifest is not a JSON object"));
    assert!(
        object.contains_key("byte_accounting"),
        "FATAL the manifest must serialize the byte-accounting ledger"
    );
    let components = object["components"]
        .as_array()
        .unwrap_or_else(|| panic!("FATAL components must be a JSON array"));
    assert!(
        components
            .iter()
            .all(|component| component.get("byte_scope").is_some()),
        "FATAL every component must serialize its byte scope"
    );

    let decoded: NativeFulltextManifest =
        serde_json::from_value(value.clone()).expect("FATAL the manifest must round trip");
    assert_eq!(
        decoded.components, manifest.components,
        "FATAL the component records must survive the round trip"
    );
    assert_eq!(
        decoded.byte_accounting, manifest.byte_accounting,
        "FATAL the byte-accounting ledger must survive the round trip"
    );
    assert_eq!(
        &decoded, manifest,
        "FATAL the whole manifest must round trip"
    );

    // Backward compatibility: drop the S04 fields and decode the pre-S04 shape.
    let mut legacy = value;
    let legacy_object = legacy
        .as_object_mut()
        .expect("FATAL manifest is a JSON object");
    assert!(
        legacy_object.remove("byte_accounting").is_some(),
        "the ledger is a top-level manifest field, so the pre-S04 wire shape omits it"
    );
    for component in legacy_object["components"]
        .as_array_mut()
        .expect("FATAL components are a JSON array")
    {
        let component = component
            .as_object_mut()
            .expect("FATAL a component is a JSON object");
        assert!(
            component.remove("byte_scope").is_some(),
            "the scope is a component field, so the pre-S04 wire shape omits it"
        );
    }
    let legacy_decoded: NativeFulltextManifest =
        serde_json::from_value(legacy).expect("FATAL a pre-S04 manifest must stay decodable");
    assert!(
        legacy_decoded
            .components
            .iter()
            .all(|component| component.byte_scope == ByteCountScope::NativeFragment),
        "FATAL an absent scope must degrade to the documented default"
    );
    assert_eq!(
        legacy_decoded.byte_accounting,
        ByteAccountingLedger::default(),
        "FATAL an absent ledger must degrade to the all-zero record"
    );
    assert_eq!(
        legacy_decoded.schema_version, manifest.schema_version,
        "FATAL dropping the S04 fields must not disturb the rest of the manifest"
    );

    // The legacy default is not silently trusted: with the whole-document
    // references now claiming to be native fragments, the detector reports it.
    assert!(
        !legacy_decoded.byte_accounting_violations().is_empty(),
        "FATAL a scope-less manifest must be reported as inconsistent, not accepted"
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
