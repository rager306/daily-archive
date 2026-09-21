use da_parse::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, "profile=fixture"),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: SOURCE.into(),
            profile_fingerprint: "profile=fixture".into(),
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

const XML: &str = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></text></TEI>";

#[test]
fn header_splice_conserves_title_affiliation_and_unknown_subtree() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java title</title><affiliation>Java affiliation</affiliation><custom><nested>must survive</nested></custom></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></text></TEI>";
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!("header composition should be admitted")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    for fragment in [
        "<title>Java title</title>",
        "<affiliation>Java affiliation</affiliation>",
        "<custom><nested>must survive</nested></custom>",
    ] {
        assert!(output.contains(fragment), "header lost {fragment}");
    }
}

#[test]
fn bibliography_anchor_must_resolve_to_composed_reference() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>body <ref type=\"bibr\" target=\"#bibl-9\">[1]</ref></p></body><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></text></TEI>";
    let mut composed = input(
        xml,
        AdmissionDisposition::Unsupported,
        AdmissionDisposition::Passed,
    );
    composed.references = compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![NativeReference {
                ordinal: 0,
                source: "Smith 2024".into(),
                span: CitationSpan { start: 0, end: 10 },
                provenance: "fixture".into(),
            }],
        },
        fields: vec![],
        anchors: NativeCitationAnchorScanner::new()
            .scan("<ref type=\"bibr\" target=\"#bibl-9\">[1]</ref>"),
    });
    let NativeFulltextCompositionOutcome::Rejected { fallback, error } =
        compose_native_fulltext(composed)
    else {
        panic!("foreign bibliography target must reject composition")
    };
    assert_eq!(fallback.raw_tei(), xml.as_bytes());
    assert!(matches!(
        error,
        NativeFulltextCompositionError::AmbiguousSplice(_)
    ));
}

#[test]
fn accepted_header_and_references_are_independent_and_other_families_fallback() {
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    )) else {
        panic!("expected candidate")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert!(output.contains("<surname>Smith</surname>"));
    assert!(output.contains("<biblStruct xml:id=\"bibl-0\">Smith 2024</biblStruct>"));
    assert!(output.contains("<body><p>body bytes</p></body>"));
    assert_eq!(candidate.manifest.native_components, 2);
    assert_eq!(candidate.manifest.fallback_components, 5);
    assert!(
        candidate
            .manifest
            .components
            .iter()
            .filter(|m| m.disposition == AdmissionDisposition::Fallback
                || m.disposition == AdmissionDisposition::Unsupported
                || m.disposition == AdmissionDisposition::InvalidEvidence
                || m.disposition == AdmissionDisposition::NotObserved)
            .count()
            == 5
    );
    assert_eq!(candidate.manifest.java_fallback_bytes, XML.len());
    let java_body = tei_element_bytes(XML.as_bytes(), "body").expect("java body");
    let out_body = tei_element_bytes(candidate.envelope.raw_tei(), "body").expect("out body");
    assert_eq!(
        java_body, out_body,
        "header/reference splice must not rewrite Java body bytes"
    );
}

#[test]
fn body_bytes_stay_java_when_only_one_lane_splices() {
    for (header, references) in [
        (
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        ),
        (
            AdmissionDisposition::Unsupported,
            AdmissionDisposition::Passed,
        ),
        (
            AdmissionDisposition::Unsupported,
            AdmissionDisposition::Unsupported,
        ),
    ] {
        let NativeFulltextCompositionOutcome::Accepted(candidate) =
            compose_native_fulltext(input(XML, header, references))
        else {
            panic!("expected candidate")
        };
        assert_eq!(
            tei_element_bytes(XML.as_bytes(), "body").unwrap(),
            tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
        );
    }
}

#[test]
fn listbibl_inside_body_is_changed_reading_order_and_keeps_java_envelope() {
    let nested = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p><listBibl><biblStruct xml:id=\"java-ref\">Java ref</biblStruct></listBibl></body></text></TEI>";
    let result = compose_native_fulltext(input(
        nested,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    ));
    match result {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert!(matches!(
                error,
                NativeFulltextCompositionError::ChangedReadingOrder
            ));
            assert_eq!(fallback.raw_tei(), nested.as_bytes());
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("body-overlapping listBibl must not splice")
        }
    }
}

#[test]
fn each_lane_can_fallback_without_pooling_or_changing_other_lane() {
    let NativeFulltextCompositionOutcome::Accepted(header_only) = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!()
    };
    let NativeFulltextCompositionOutcome::Accepted(ref_only) = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Unsupported,
        AdmissionDisposition::Passed,
    )) else {
        panic!()
    };
    let h = String::from_utf8(header_only.envelope.raw_tei().to_vec()).unwrap();
    let r = String::from_utf8(ref_only.envelope.raw_tei().to_vec()).unwrap();
    assert!(h.contains("<surname>Smith</surname>"));
    assert!(h.contains("<biblStruct xml:id=\"java-ref\">Java ref</biblStruct>"));
    assert!(r.contains("<title>Java</title>"));
    assert!(r.contains("<biblStruct xml:id=\"bibl-0\">Smith 2024</biblStruct>"));
}

#[test]
fn forged_pass_and_stale_identity_fail_closed_to_unchanged_java() {
    let mut forged = input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    );
    forged.admissions[0].evidence_state = "terminal".into();
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(forged)
    else {
        panic!("terminal is fallback, not whole rejection")
    };
    assert!(String::from_utf8_lossy(candidate.envelope.raw_tei()).contains("<title>Java</title>"));
    assert!(
        !String::from_utf8_lossy(candidate.envelope.raw_tei()).contains("<surname>Smith</surname>")
    );

    let mut stale = input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    );
    stale.admissions[0].source_sha256 =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into();
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(stale)
    else {
        panic!()
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert!(output.contains("<title>Java</title>"));
    assert!(!output.contains("<surname>Smith</surname>"));
    assert!(output.contains("<biblStruct xml:id=\"bibl-0\">Smith 2024</biblStruct>"));
}

#[test]
fn malformed_external_entity_duplicate_id_and_replay_are_safe() {
    for xml in [
        "<TEI><teiHeader>",
        "<!DOCTYPE TEI [<!ENTITY x SYSTEM \"secret\">]><TEI/>",
        "<TEI id=\"x\"><teiHeader id=\"x\"/><text><listBibl/></text></TEI>",
    ] {
        let result = compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Passed,
        ));
        assert!(matches!(
            result,
            NativeFulltextCompositionOutcome::Rejected { .. }
        ));
        if let NativeFulltextCompositionOutcome::Rejected { fallback, .. } = result {
            assert_eq!(fallback.raw_tei(), xml.as_bytes());
        }
    }
    let a = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    ));
    let b = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    ));
    let NativeFulltextCompositionOutcome::Accepted(a) = a else {
        panic!()
    };
    let NativeFulltextCompositionOutcome::Accepted(b) = b else {
        panic!()
    };
    assert_eq!(a.envelope.raw_tei(), b.envelope.raw_tei());
    assert_eq!(
        serde_json::to_vec(&a.manifest).unwrap(),
        serde_json::to_vec(&b.manifest).unwrap()
    );
}

/// Admission digest binding: empty claims are produced from the spliced bytes.
///
/// An explicitly supplied digest is checked against the fragment, while an
/// empty claim is replaced by the digest produced by composition.
#[test]
fn evidence_digest_describes_the_spliced_fragment() {
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Passed,
    )) else {
        panic!("expected candidate")
    };

    let admitted: Vec<_> = candidate
        .manifest
        .components
        .iter()
        .filter(|m| m.component == FulltextComponent::Header)
        .collect();
    assert_eq!(admitted.len(), 1, "the header lane is recorded once");
    let entry = admitted[0];

    assert_eq!(entry.disposition, AdmissionDisposition::Passed);
    assert_eq!(entry.evidence_digest, entry.sha256);
    assert_ne!(entry.evidence_digest, ARTIFACT);
}

#[test]
fn foreign_valid_artifact_digest_rejects_and_preserves_java_envelope() {
    let mut rejected = input(
        XML,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    );
    rejected.admissions[0].artifact_sha256 = ARTIFACT.into();
    let NativeFulltextCompositionOutcome::Rejected { fallback, error } =
        compose_native_fulltext(rejected)
    else {
        panic!("foreign digest must reject the candidate")
    };
    assert_eq!(fallback.raw_tei(), XML.as_bytes());
    assert!(matches!(
        error,
        NativeFulltextCompositionError::ArtifactDigestMismatch {
            component: "header",
            ..
        }
    ));
}

// ───── M511-j7wz2f / S01 / T01: A-XML RED — real preserved TEI, ─────
// ───── independent XML parser, recorded as an expected diagnostic ─────
//
// Measured 2026-09-21 (`doc/review/grobid-absorption-audit-2026-09-21.md`,
// gsd_exec 6c9cbd4d-e9eb-4281-8da8-1fdc7d724f1f) and re-measured from the
// tracked fixtures here: `compose_native_fulltext` returns `Accepted` for all
// three sha256-pinned preserved TEI while an independent XML parser rejects the
// emitted document with `mismatched tag` at a duplicated `</title>`.
//
// Root-cause surface: `preserve_header_children` walks *every* parsed tag,
// closing tags included, so `element_range` can be entered at a `</title>` and
// then run to the next `<title>` in the document, pushing a range that starts
// with a stray closing tag and swallows the elements between. The synthesized
// `<teiHeader>` additionally drops the source header attributes (`xml:lang`).
//
// `compose_native_fulltext` has no runtime caller (library re-export, shadow
// example and tests only — GitNexus reports no incoming edge, confirmed by a
// text search over `crates/*/src`). Nothing below proves production
// reachability, and nothing below changes a lock: D127 / D321 stay closed and
// the composer import-root set stays `{crate, serde, sha2, std}` — `roxmltree`
// is a dev-dependency and is never reachable from `src/`.
//
// Two artifacts, deliberately different:
//   * `preserved_tei_composition_is_independently_well_formed` is the
//     regression gate. It is RED today, so it carries `#[ignore]` with the
//     expected diagnostic; S01/T02 removes the attribute once range pairing and
//     header attributes are repaired.
//   * `a_xml_red_preserved_tei_emissions_are_rejected_by_an_independent_parser`
//     runs by default and pins the measured RED (Accepted + independent
//     rejection + the duplicate closing tag + the dropped header attribute) so
//     the defect can neither be silently declared fixed nor silently lost.
//     S01/T02 replaces it with the post-fix preservation assertions.

/// The three sha256-pinned preserved TEI, in `provenance.yaml` order.
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

fn preserved_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a preserved TEI fixture and verify its frozen checksum. A missing or
/// drifted fixture is FATAL: the regression must never silently shrink.
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

/// Independent well-formedness verdict. `roxmltree` is a strict,
/// non-validating XML parser that shares no code with the composer's own tag
/// scanner, so `Accepted` and "parses as XML" stay separate facts.
fn independent_xml_verdict(xml: &str) -> Result<(), String> {
    roxmltree::Document::parse(xml)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Compose the audit's measurement configuration (header admitted, references
/// left to the Java fallback) against a real preserved TEI document.
fn emit_preserved_tei(name: &str, declared_sha256: &str) -> String {
    let source = preserved_tei_source(name, declared_sha256);
    match compose_native_fulltext(input(
        &source,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            String::from_utf8(candidate.envelope.raw_tei().to_vec())
                .unwrap_or_else(|error| panic!("{name}: emitted document is not UTF-8 ({error})"))
        }
        other => panic!("{name}: expected Accepted for the preserved TEI, got {other:?}"),
    }
}

/// A-XML regression gate. RED until S01/T02 repairs range pairing and header
/// attributes: the independent parser rejects all three emissions today.
#[test]
#[ignore = "A-XML RED (H14 / M511 S01 T01): emitter still produces a stray closing tag + drops header attributes on all three preserved TEI; remove this #[ignore] in S01/T02 once the composition is structurally sound"]
fn preserved_tei_composition_is_independently_well_formed() {
    for (name, sha256) in PRESERVED_TEI {
        let source = preserved_tei_source(name, sha256);
        assert_eq!(
            independent_xml_verdict(&source),
            Ok(()),
            "{name}: the preserved fixture must itself be well-formed"
        );
        let emitted = emit_preserved_tei(name, sha256);
        assert_eq!(
            independent_xml_verdict(&emitted),
            Ok(()),
            "A-XML: an Accepted emission must be well-formed XML ({name})"
        );
    }
}

/// Expected diagnostic for the measured RED, pinned over all three fixtures.
///
/// S01/T02 replaces this with post-fix preservation assertions: the emission
/// must parse, keep one `<title>` per source title, and keep the header
/// attributes.
#[test]
fn a_xml_red_preserved_tei_emissions_are_rejected_by_an_independent_parser() {
    for (name, sha256) in PRESERVED_TEI {
        let source = preserved_tei_source(name, sha256);
        assert_eq!(
            independent_xml_verdict(&source),
            Ok(()),
            "{name}: the preserved fixture must itself be well-formed, otherwise the \
             independent parser proves nothing about the emission"
        );
        assert!(
            !source.contains("</title></title>"),
            "{name}: the fixture must not already carry the duplicated closing tag"
        );
        assert!(
            source.contains("<teiHeader xml:lang"),
            "{name}: the fixture must carry the header attribute the emitter drops"
        );

        let emitted = emit_preserved_tei(name, sha256);
        let verdict = independent_xml_verdict(&emitted);
        let diagnostic = match verdict {
            Ok(()) => panic!(
                "A-XML RED no longer reproduces for {name}: an Accepted emission parsed as \
                 well-formed. Update S01/T01/T02 instead of relaxing this pin."
            ),
            Err(diagnostic) => diagnostic,
        };
        println!(
            "[a-xml-red] {name}: independent parser rejects the Accepted emission: {diagnostic}"
        );
        assert!(
            diagnostic.contains("expected 'titleStmt' tag, not 'title'")
                && diagnostic.contains("at 6:"),
            "{name}: the independent diagnostic drifted from the measured RED (mismatched tag at 
             the duplicated </title>): {diagnostic}"
        );

        assert_eq!(
            emitted.matches("</title></title>").count(),
            1,
            "{name}: exactly one range entry starts at a closing </title> tag"
        );
        assert!(
            emitted.matches("</title>").count() > source.matches("</title>").count(),
            "{name}: the emission carries more closing title tags than its source"
        );
        assert!(
            !emitted.contains("<teiHeader xml:lang"),
            "{name}: the synthesized <teiHeader> must reproduce the dropped header attribute \
             once S01/T02 lands"
        );
    }
}
