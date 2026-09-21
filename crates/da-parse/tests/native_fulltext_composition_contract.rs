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
        // S01/T03: a stray closing tag, an unterminated comment and an
        // unterminated CDATA section are malformed/unbalanced inputs, not
        // composer crashes, and each must keep the byte-identical Java envelope.
        "<TEI><teiHeader/></TEI></TEI>",
        "<TEI><!-- unterminated comment",
        "<TEI><![CDATA[unterminated",
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

// ───── M511-j7wz2f / S01 / T02: A-XML GREEN — real preserved TEI, ─────
// ───── independent XML parser, structural header placement ─────
//
// T01 measured the RED (`doc/review/grobid-absorption-audit-2026-09-21.md`,
// gsd_exec 6c9cbd4d-e9eb-4281-8da8-1fdc7d724f1f) from the tracked fixtures:
// `compose_native_fulltext` returned `Accepted` for all three sha256-pinned
// preserved TEI while an independent XML parser rejected the emitted document
// with `mismatched tag` at a duplicated `</title>`.
//
// Root cause, fixed in T02: `preserve_header_children` walked *every* parsed tag,
// closing tags included, so an element range could be entered at a `</title>` and
// then run to the next `<title>` in the document, pushing a range that started
// with a stray closing tag and swallowed the elements between. The synthesized
// `<teiHeader>` additionally dropped the source header attributes (`xml:lang`,
// `xmlns:*`).
//
// T02 fixed it structurally rather than cosmetically:
//   * `XmlDocument` pairs open and close tags in one stack, so a span is always
//     `(open tag start, close tag end)` and can never start at a closing tag;
//   * header content is re-placed by *parent* (children of `titleStmt` into the
//     composed title statement, children of `fileDesc` minus `titleStmt` into the
//     composed file description, the rest after it), which is provably disjoint
//     and complete for every descendant of the source `teiHeader`;
//   * the source `<teiHeader>` attributes are reproduced verbatim;
//   * the emitted bytes are re-parsed before `Accepted`
//     (`ComposedOutputNotWellFormed`), so a damaged splice is a typed rejection
//     and the byte-identical Java envelope is used instead.
//
// This file is still the A-XML judge, and the judge is still independent:
// `roxmltree` shares no code with the composer's tag scanner, and it resolves
// namespace prefixes, so an unbound prefix or a lost declaration fails here.
//
// Evidence: the T01 regression gate
// (`preserved_tei_composition_is_independently_well_formed`) was RED in T01 and
// is green here with its `#[ignore]` removed (gsd_exec
// d38840db-e9b4-44cc-9cd4-1fb3b85096d6). T01's RED pin was replaced by
// `preserved_tei_emissions_conserve_every_source_header_subtree`, which asserts
// post-fix conservation instead of the defect; the structural requirements
// (repeat, nesting, attributes, namespaces, unknown subtrees, output validation)
// are covered by the tests below it.
//
// `compose_native_fulltext` has no runtime caller (library re-export, shadow
// example and tests only — GitNexus reports no incoming edge, confirmed by a
// text search over `crates/*/src`). Nothing below proves production
// reachability, and nothing below changes a lock: D127 / D321 stay closed and
// the composer import-root set stays `{crate, serde, sha2, std}` — `roxmltree`
// is a dev-dependency and is never reachable from `src/`.

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

/// Independent children of the first element whose local name is `parent`, as
/// (local name, exact source bytes) pairs.
///
/// Extraction goes through `roxmltree`, not through the composer's own parser, so
/// a range-pairing regression cannot hide behind the code under test.
fn independent_children(xml: &str, parent: &str) -> Vec<(String, String)> {
    let document = roxmltree::Document::parse(xml).expect("independent parser accepts");
    let node = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == parent)
        .unwrap_or_else(|| panic!("{parent} present"));
    node.children()
        .filter(|child| child.is_element())
        .map(|child| {
            (
                child.tag_name().name().to_owned(),
                xml[child.range()].to_owned(),
            )
        })
        .collect()
}

/// Independent element count by local name.
fn independent_count(xml: &str, tag: &str) -> usize {
    roxmltree::Document::parse(xml)
        .expect("independent parser accepts")
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == tag)
        .count()
}

/// A-XML regression gate: an `Accepted` emission of a real preserved TEI must be
/// well-formed for an independent parser.
///
/// Green since S01/T02; the `#[ignore]` T01 carried for the measured RED is gone.
#[test]
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

/// Post-fix conservation, replacing T01's RED pin.
///
/// The emission must parse, keep the source header attributes, keep one title
/// element per source title element, and re-place every source header subtree
/// under the wrapper it came from — all judged by the independent parser.
#[test]
fn preserved_tei_emissions_conserve_every_source_header_subtree() {
    for (name, sha256) in PRESERVED_TEI {
        let source = preserved_tei_source(name, sha256);
        assert_eq!(
            independent_xml_verdict(&source),
            Ok(()),
            "{name}: the fixture must itself be well-formed"
        );
        assert!(
            source.contains("<teiHeader xml:lang=\"en\">"),
            "{name}: the fixture must carry the header attribute the emitter used to drop"
        );
        let emitted = emit_preserved_tei(name, sha256);
        assert_eq!(
            independent_xml_verdict(&emitted),
            Ok(()),
            "A-XML: an Accepted emission must be well-formed XML ({name})"
        );
        assert!(
            emitted.contains("<teiHeader xml:lang=\"en\">"),
            "{name}: the source <teiHeader> attributes must survive the splice"
        );
        assert!(
            !emitted.contains("</title></title>"),
            "{name}: no range may start at a closing </title> tag"
        );
        assert_eq!(
            independent_count(&emitted, "title"),
            independent_count(&source, "title"),
            "{name}: every source <title> must survive exactly once"
        );
        for (parent, excluded) in [
            ("teiHeader", &["fileDesc", "titleStmt"][..]),
            ("fileDesc", &["titleStmt"][..]),
            ("titleStmt", &[] as &[&str]),
        ] {
            let emitted_children = independent_children(&emitted, parent);
            let mut conserved = 0usize;
            for (child_name, bytes) in independent_children(&source, parent) {
                if excluded.contains(&child_name.as_str()) {
                    continue;
                }
                assert!(
                    emitted_children.contains(&(child_name.clone(), bytes.clone())),
                    "{name}: {parent}/{child_name} was lost or rewritten by the splice"
                );
                conserved += 1;
            }
            assert!(
                conserved > 0,
                "{name}: {parent} exercised no preserved child, so the assertion proved nothing"
            );
        }
    }
}

/// Structural requirements: repeated and nested same-name tags, an attribute
/// whose value contains `>`, and a namespace declaration carried by the header
/// itself.
///
/// The independent parser is also the namespace judge: it resolves prefixes, so a
/// preserved `ext:` subtree whose declaration was dropped could not parse.
#[test]
fn nested_repeated_namespaced_and_unknown_header_subtrees_survive_structurally() {
    let xml = "<TEI xmlns:ext=\"urn:example:ext\"><teiHeader xml:lang=\"en\" xmlns:ext=\"urn:example:ext\"><fileDesc><titleStmt><title type=\"main\" note=\"a>b\">First title</title><title type=\"sub\">Second title</title><custom><custom>inner</custom><custom>third</custom></custom></titleStmt><publicationStmt><publisher>Example</publisher></publicationStmt><sourceDesc><ext:blob><ext:inner/></ext:blob></sourceDesc></fileDesc><profileDesc><abstract><p>abstract</p></abstract></profileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let outcome = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    ));
    let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
        panic!("header composition should be admitted, got {outcome:?}")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    assert_eq!(
        output
            .matches("<teiHeader xml:lang=\"en\" xmlns:ext=\"urn:example:ext\">")
            .count(),
        1,
        "the source header attributes must be reproduced verbatim: {output}"
    );
    for fragment in [
        "<title type=\"main\" note=\"a>b\">First title</title>",
        "<title type=\"sub\">Second title</title>",
        "<custom><custom>inner</custom><custom>third</custom></custom>",
        "<publicationStmt><publisher>Example</publisher></publicationStmt>",
        "<ext:blob><ext:inner/></ext:blob>",
        "<abstract><p>abstract</p></abstract>",
    ] {
        assert!(
            output.contains(fragment),
            "header lost {fragment}: {output}"
        );
    }
    assert_eq!(independent_count(&output, "title"), 2);
    assert_eq!(
        independent_count(&output, "custom"),
        3,
        "one outer and two nested custom elements"
    );
    // Placement, not just survival: profileDesc belongs to teiHeader, not to the
    // composed fileDesc.
    let header_children = independent_children(&output, "teiHeader");
    assert!(
        header_children
            .iter()
            .any(|(name, bytes)| name == "profileDesc" && bytes.contains("<p>abstract</p>")),
        "profileDesc must stay a child of teiHeader: {header_children:?}"
    );
}

/// Namespace declarations travel with the subtrees they cover — including when
/// the declaration sits on a wrapper the composer synthesizes.
///
/// S01/T03 measured the residual defect this closes (gsd_exec
/// c9cbe8d7-b4dc-492a-aa7d-ab7134a5335b): a source `<fileDesc xmlns:ext="…">` or
/// `<titleStmt xmlns:ext="…">` returned `Accepted` while `roxmltree` rejected the
/// emission with `an unknown namespace prefix 'ext' at 1:202` / `1:153` — the
/// declaration was dropped with the wrapper whose children were moved. The
/// composer now reproduces `xmlns`/`xmlns:*` from both wrappers. Non-namespace
/// wrapper attributes (a wrapper-level `xml:lang`, for instance) are still not
/// copied: that is a semantic-loss residual for the field lanes, not a
/// well-formedness one.
#[test]
fn wrapper_namespace_declarations_are_carried_so_no_prefix_is_unbound() {
    for (xml, fragment, prefixed_local) in [
        (
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc xmlns:ext=\"urn:example:ext\"><titleStmt><title>Java</title></titleStmt><sourceDesc><ext:blob><ext:inner/></ext:blob></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            "<fileDesc xmlns:ext=\"urn:example:ext\">",
            "blob",
        ),
        (
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt xmlns:ext=\"urn:example:ext\"><title>Java</title><ext:note/></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            "<titleStmt xmlns:ext=\"urn:example:ext\">",
            "note",
        ),
    ] {
        let outcome = compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        ));
        let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
            panic!("a wrapper declaration must not reject the splice: {outcome:?}")
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
        assert!(
            output.contains(fragment),
            "the wrapper declaration was dropped: {output}"
        );
        // The independent parser is the namespace judge: the preserved prefix must
        // resolve to the URI the source declared, not merely parse.
        let document = roxmltree::Document::parse(&output).expect("independent parser accepts");
        assert!(
            document.descendants().any(|node| {
                node.is_element()
                    && node.tag_name().name() == prefixed_local
                    && node.tag_name().namespace() == Some("urn:example:ext")
            }),
            "{prefixed_local} did not resolve into the declared namespace: {output}"
        );
        assert_eq!(
            tei_element_bytes(xml.as_bytes(), "body").unwrap(),
            tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
        );
    }
}

/// Markup declarations are not elements: a comment carrying `<title>` and a CDATA
/// section carrying `<p>` must not be read as tags.
#[test]
fn commented_markup_and_cdata_are_not_read_as_elements() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title><!-- <title>comment, not a tag</title> --></titleStmt></fileDesc></teiHeader><text><body><p><![CDATA[ <p>not a tag</p> ]]></p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let outcome = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    ));
    let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
        panic!("commented and CDATA markup must not break the splice, got {outcome:?}")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    assert!(output.contains("<title>Java</title>"));
    assert_eq!(
        independent_count(&output, "title"),
        1,
        "the comment's <title> must not be counted as an element"
    );
    assert_eq!(
        tei_element_bytes(xml.as_bytes(), "body").unwrap(),
        tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap(),
        "CDATA body bytes must stay Java's"
    );
}

/// The emitter validates its own output before `Accepted`: a preserved source id
/// that collides with a synthesized `author-N` is a typed rejection, not a
/// published duplicate.
#[test]
fn emitted_duplicate_id_is_typed_and_falls_back_to_java() {
    let xml = "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title><note xml:id=\"author-0\">collides with the synthesized author id</note></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    match compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert!(
                matches!(
                    error,
                    NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                ),
                "a duplicate xml:id in the emission must be typed, got {error:?}"
            );
            assert_eq!(fallback.raw_tei(), xml.as_bytes());
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("an emission with a duplicate xml:id must not be published")
        }
    }
}

/// A missing `fileDesc`/`titleStmt` is supplied by the synthesized skeleton, so it
/// stays composable and lossless; a duplicated one cannot be placed without
/// guessing and is a typed rejection.
#[test]
fn header_skeleton_absence_is_supplied_and_ambiguity_is_typed() {
    for xml in [
        // no titleStmt: the composed skeleton supplies it.
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><publicationStmt><publisher>Example</publisher></publicationStmt><sourceDesc><p>source</p></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
        // no fileDesc: the composed skeleton supplies it, profileDesc stays a header child.
        "<TEI><teiHeader xml:lang=\"en\"><titleStmt><title>Java title</title></titleStmt><profileDesc><abstract><p>abstract</p></abstract></profileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
    ] {
        let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) else {
            panic!("a missing skeleton wrapper must be supplied, not rejected: {xml}")
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
        for fragment in ["<surname>Smith</surname>", "<teiHeader xml:lang=\"en\">"] {
            assert!(output.contains(fragment), "{fragment} missing: {output}");
        }
        for fragment in [
            "<publisher>Example</publisher>",
            "<p>source</p>",
            "<title>Java title</title>",
            "<p>abstract</p>",
        ] {
            if xml.contains(fragment) {
                assert!(output.contains(fragment), "{fragment} lost: {output}");
            }
        }
    }

    for xml in [
        // two title statements: the composer cannot choose a parent for the source titles.
        "<TEI><teiHeader><fileDesc><titleStmt><title>a</title></titleStmt><titleStmt><title>b</title></titleStmt></fileDesc></teiHeader><text><body><p>body</p></body><listBibl><biblStruct xml:id=\"b0\">r</biblStruct></listBibl></text></TEI>",
        // two file descriptions, unique titleStmt: the fileDesc set is ambiguous.
        "<TEI><teiHeader><fileDesc><titleStmt><title>a</title></titleStmt></fileDesc><fileDesc><publicationStmt/></fileDesc></teiHeader><text><body><p>body</p></body><listBibl><biblStruct xml:id=\"b0\">r</biblStruct></listBibl></text></TEI>",
    ] {
        match compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                assert!(
                    matches!(error, NativeFulltextCompositionError::AmbiguousSplice(_)),
                    "an ambiguous skeleton must be typed, got {error:?}"
                );
                assert_eq!(fallback.raw_tei(), xml.as_bytes());
            }
            NativeFulltextCompositionOutcome::Accepted(_) => {
                panic!("an ambiguous header skeleton must not be spliced")
            }
        }
    }
}
