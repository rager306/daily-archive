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

/// Concatenated bytes of every *direct* child of the first element whose local
/// name is `parent`, minus the children whose local name is excluded, plus the
/// number of non-element children seen.
///
/// Unlike `independent_children`, this is deliberately **not** filtered by
/// `is_element()`: a text node, comment or processing instruction between
/// children is conservation content too, and an element-only view is blind to it
/// disappearing (measured S01/T05).  The count is returned so a caller can
/// assert the check was not vacuous for that class.
fn independent_child_stream(xml: &str, parent: &str, excluded: &[&str]) -> (String, usize) {
    let document = roxmltree::Document::parse(xml).expect("independent parser accepts");
    let node = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == parent)
        .unwrap_or_else(|| panic!("{parent} present"));
    let mut stream = String::new();
    let mut non_elements = 0usize;
    for child in node.children() {
        if child.is_element() {
            if excluded.contains(&child.tag_name().name()) {
                continue;
            }
        } else {
            non_elements += 1;
        }
        stream.push_str(&xml[child.range()]);
    }
    (stream, non_elements)
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
            // Element-only conservation cannot see a dropped comment,
            // processing instruction or text node, so the same direct children
            // are also compared as a byte stream that includes those nodes.  The
            // preserved TEI roots carry `xml:space="preserve"` and whitespace
            // between children, so the class is exercised by construction.
            let (source_stream, source_non_elements) =
                independent_child_stream(&source, parent, excluded);
            let (emitted_stream, _) = independent_child_stream(&emitted, parent, excluded);
            assert!(
                emitted_stream.contains(&source_stream),
                "{name}: {parent} lost or rewrote its non-element children"
            );
            assert!(
                source_non_elements > 0,
                "{name}: {parent} has no non-element child, so the conservation check was blind to that class"
            );
        }
    }
}

/// S01/T05: a `titleStmt` that is *not* a direct child of the file description is
/// not the wrapper the composer replaces.
///
/// `element_index_opt` matched by name anywhere in the document, so the only
/// `<titleStmt>` — one nested under `fileDesc/seriesStmt` — was taken as the
/// wrapper: its `<title>` was moved into the composed title statement *and* the
/// enclosing `<seriesStmt>` travelled over as a file-description child, which
/// measured two opening `<title>` tags in an `Accepted` emission.
#[test]
// The name is prescribed by the S01/T05 contract (and the audit it closes), so the
// camel-cased `titleStmt` is kept rather than renamed to satisfy the lint.
#[allow(non_snake_case)]
fn nested_unique_titleStmt_under_seriesStmt_is_not_duplicated() {
    let xml = "<TEI><teiHeader xml:lang=\"en\"><fileDesc><seriesStmt><titleStmt><title>Series</title></titleStmt></seriesStmt><publicationStmt><publisher>Example</publisher></publicationStmt><sourceDesc><p>source</p></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!("a nested titleStmt must neither reject nor damage the splice")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    assert_eq!(
        independent_count(&output, "title"),
        independent_count(xml, "title"),
        "the nested <title> must survive exactly once: {output}"
    );
    assert_eq!(
        output.matches("<title>Series</title>").count(),
        1,
        "the nested title must not be duplicated: {output}"
    );
    assert_eq!(
        independent_count(&output, "titleStmt"),
        2,
        "the composed skeleton adds one title statement and the nested one stays in its parent: {output}"
    );
    // The nested subtree stays where it was: under seriesStmt, inside fileDesc.
    let file_desc_children = independent_children(&output, "fileDesc");
    assert!(
        file_desc_children
            .iter()
            .any(|(name, bytes)| name == "seriesStmt"
                && bytes.contains("<titleStmt><title>Series</title></titleStmt>")),
        "seriesStmt must stay a fileDesc child with its nested titleStmt: {file_desc_children:?}"
    );
    assert!(
        output.contains("<publisher>Example</publisher>"),
        "{output}"
    );
    assert!(output.contains("<p>source</p>"), "{output}");
}

/// S01/T05: the wrappers the composer synthesizes must conserve the source
/// wrapper's *non-element* children — a direct comment, a processing instruction
/// and the text between children — not only its element subtrees.
#[test]
fn wrapper_direct_comments_pis_and_text_are_conserved() {
    let xml = "<TEI><teiHeader xml:lang=\"en\">\n<!-- header direct comment --><?header-pi keep?>\n<fileDesc>\n<!-- fileDesc direct comment -->\n<titleStmt><title>Java title</title><!-- titleStmt direct comment --></titleStmt>\n<publicationStmt><publisher>Example</publisher></publicationStmt>\n</fileDesc>\nheader level text\n<profileDesc><abstract><p>abstract</p></abstract></profileDesc>\n</teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!("direct comments, processing instructions and text must not reject the splice")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    for fragment in [
        "<!-- header direct comment -->",
        "<?header-pi keep?>",
        "<!-- fileDesc direct comment -->",
        "<!-- titleStmt direct comment -->",
        "\nheader level text\n",
    ] {
        assert!(output.contains(fragment), "{fragment} lost: {output}");
    }
    // Structural, judged by the independent parser: each conserved node is still
    // a direct child of the wrapper it came from, not merely present somewhere.
    let document = roxmltree::Document::parse(&output).expect("independent parser accepts");
    let children_stream = |parent: &str| -> String {
        document
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == parent)
            .unwrap_or_else(|| panic!("{parent} present"))
            .children()
            .map(|child| &output[child.range()])
            .collect()
    };
    let header = children_stream("teiHeader");
    assert!(
        header.contains("<!-- header direct comment -->"),
        "{header}"
    );
    assert!(header.contains("<?header-pi keep?>"), "{header}");
    assert!(header.contains("\nheader level text\n"), "{header}");
    let file_desc = children_stream("fileDesc");
    assert!(
        file_desc.contains("<!-- fileDesc direct comment -->"),
        "{file_desc}"
    );
    let title_stmt = children_stream("titleStmt");
    assert!(
        title_stmt.contains("<!-- titleStmt direct comment -->"),
        "{title_stmt}"
    );
    assert!(output.contains("<p>abstract</p>"), "{output}");
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

// ───── M511-j7wz2f / S01 / T04: `Accepted` is provably well-formed — ─────
// ───── declarations at any legal spacing, prefix bindings, independent ─────
// ───── assent on every adversarial emission ─────
//
// T04 measured a P0 class (gsd_exec 02527830-1994-4a28-96de-9630fc69d19a): the
// composer returned `Accepted` for emissions an independent parser rejected.
//
//   * `xmlns:ext = "urn:example:ext"` and `xmlns:ext ="urn:example:ext"` on
//     either synthesized wrapper were dropped — `attribute_tokens` split the tag
//     body on unquoted whitespace, so a declaration written with spaces around
//     `=` became three tokens and neither its name nor its value survived —
//     leaving `ext:` unbound in the moved subtree;
//   * a raw `&` in preserved source bytes was republished unchanged.
//
// `validate_emitted` could not see either: it ran the same `XmlDocument::parse`
// the splice ranges were built on, and that scanner deliberately does not
// interpret namespaces or references. The emitter now also runs a separate
// `validate_well_formed` pass over its own bytes (an enumerated class list: one
// document element, `Name`/`QName` legality, `Name S? Eq S? AttValue` attribute
// form, duplicate attribute, reference syntax, prefix bindings, `]]>`, `--` in a
// comment, CDATA placement, depth, legal characters, DOCTYPE/ENTITY forbidden).
// That list is not a validating parser, so these tests keep the independent
// parser (`roxmltree`) as the judge of the stronger claim.

/// Whitespace is legal on both sides of `=` (`Name S? Eq S? AttValue`), and a
/// declaration written that way must still travel with the subtree it covers —
/// including from a wrapper the composer synthesizes, and with several
/// declarations in one tag.
#[test]
fn space_around_equals_namespace_declaration_is_carried_on_every_wrapper() {
    for (xml, fragments, resolved) in [
        (
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc xmlns:ext = \"urn:example:ext\"><titleStmt><title>Java</title></titleStmt><sourceDesc><ext:blob><ext:inner/></ext:blob></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            &["<fileDesc xmlns:ext=\"urn:example:ext\">"][..],
            &[("blob", "urn:example:ext"), ("inner", "urn:example:ext")][..],
        ),
        (
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt xmlns:ext =\"urn:example:ext\"><title>Java</title><ext:note/></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            &["<titleStmt xmlns:ext=\"urn:example:ext\">"][..],
            &[("note", "urn:example:ext")][..],
        ),
        (
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc\n  xmlns:a = \"urn:a\"\n\txmlns:b\t=\t\"urn:b\"\n><titleStmt><title>Java</title></titleStmt><sourceDesc><a:one/><b:two/></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            &["<fileDesc xmlns:a=\"urn:a\" xmlns:b=\"urn:b\">"][..],
            &[("one", "urn:a"), ("two", "urn:b")][..],
        ),
    ] {
        let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) else {
            panic!("a legally spelled wrapper declaration must not reject the splice: {xml}")
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
        for fragment in fragments {
            assert!(
                output.contains(fragment),
                "the wrapper declaration {fragment} was dropped: {output}"
            );
        }
        // The independent parser is the namespace judge: each preserved prefix
        // must resolve to the URI the source declared, not merely parse.
        let document = roxmltree::Document::parse(&output).expect("independent parser accepts");
        for (local, uri) in resolved {
            assert!(
                document.descendants().any(|node| {
                    node.is_element()
                        && node.tag_name().name() == *local
                        && node.tag_name().namespace() == Some(*uri)
                }),
                "{local} did not resolve into {uri}: {output}"
            );
        }
        assert_eq!(
            tei_element_bytes(xml.as_bytes(), "body").unwrap(),
            tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
        );
    }
}

/// A prefix that resolves to no declaration in the emission is a typed refusal,
/// not `Accepted` — and the fallback is the byte-identical Java envelope.
///
/// The positive control below is the same shape with the declaration on an
/// ancestor that survives the splice: scope, not the wrapper, is what makes a
/// prefix bound, so that case stays admitted.
#[test]
fn unbound_prefix_in_a_moved_subtree_is_typed_and_falls_back_to_java() {
    let xml = "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt><sourceDesc><ext:blob/></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    match compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            let message = format!("{error:?}");
            assert!(
                matches!(
                    error,
                    NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                ),
                "an unbound prefix must be a typed refusal, got {error:?}"
            );
            assert!(
                message.contains("unbound namespace prefix") && message.contains("ext"),
                "the refusal must name the unbound prefix: {message}"
            );
            assert_eq!(fallback.raw_tei(), xml.as_bytes());
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("an emission with an unbound prefix must not be published")
        }
    }

    let declared = "<TEI xmlns:ext=\"urn:example:ext\"><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt><sourceDesc><ext:blob/></sourceDesc></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        declared,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!("a declaration on a surviving ancestor keeps the prefix bound")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    assert!(
        roxmltree::Document::parse(&output)
            .expect("independent parser accepts")
            .descendants()
            .any(|node| node.is_element()
                && node.tag_name().name() == "blob"
                && node.tag_name().namespace() == Some("urn:example:ext")),
        "the root declaration must still bind ext:blob: {output}"
    );
}

/// A raw `&` in the source is the other measured class: the composer copies
/// preserved bytes verbatim, so it cannot claim such an emission is well-formed.
/// It must refuse and hand back the byte-identical Java envelope, whether the `&`
/// sits in the admitted header subtree or only in the body it copies through.
#[test]
fn raw_ampersand_in_source_is_rejected_with_the_byte_identical_java_envelope() {
    let sources = [
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Smith & Sons</title></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>Smith & Sons</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body><p>Smith &foo; Sons</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
    ];
    for xml in sources {
        match compose_native_fulltext(input(
            xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                let message = format!("{error:?}");
                assert!(
                    matches!(
                        error,
                        NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                    ),
                    "a raw `&` must be a typed refusal, got {error:?}"
                );
                assert!(
                    message.contains("reference"),
                    "the refusal must name the reference class: {message}"
                );
                assert_eq!(fallback.raw_tei(), xml.as_bytes());
            }
            NativeFulltextCompositionOutcome::Accepted(_) => {
                panic!("an ill-formed emission must not be published: {xml}")
            }
        }
    }

    // The module's own range scanner still reads this source: it never
    // interpreted references, which is exactly why `Accepted` may not rest on it.
    assert!(
        tei_element_bytes(sources[1].as_bytes(), "body").is_ok(),
        "the structural scanner is intentionally reference-blind"
    );
    assert!(
        independent_xml_verdict(sources[1]).is_err(),
        "an independent parser rejects the raw `&` the composer must not republish"
    );
}

/// Every adversarial source is either refused with the byte-identical Java
/// envelope or emitted as XML an independent parser accepts — never an
/// `Accepted` candidate built on ill-formed bytes.
#[test]
fn adversarial_sources_are_never_accepted_without_independent_assent() {
    const HEADER: &str = "<teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader>";
    let document = |payload: &str| {
        format!(
            "<TEI>{HEADER}<text><body><p>{payload}</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>"
        )
    };
    let sources = [
        ("raw ampersand", document("a & b")),
        ("literal ]]> in character data", document("a ]]> b")),
        ("duplicate attribute", document("<x id=\"a\" id=\"b\"/>")),
        ("unquoted attribute value", document("<x id=a/>")),
        ("illegal XML character", document("a\u{1}b")),
        ("`--` inside a comment", document("<!-- a -- b -->")),
        ("unbound namespace prefix", document("<ext:blob/>")),
        ("unsupported `<!` declaration", document("<!doctype TEI>")),
        (
            "reserved `xml` processing instruction",
            document("<?xml version=\"1.0\"?>"),
        ),
        ("illegal element name", document("<1bad/>")),
        (
            "two document elements",
            format!(
                "<TEI>{HEADER}<text><body><p>body</p></body><listBibl><biblStruct xml:id=\"b0\">r</biblStruct></listBibl></text></TEI><extra/>"
            ),
        ),
        (
            "CDATA outside the document element",
            format!(
                "<![CDATA[x]]><TEI>{HEADER}<text><body><p>body</p></body><listBibl><biblStruct xml:id=\"b0\">r</biblStruct></listBibl></text></TEI>"
            ),
        ),
    ];
    let mut rejected = 0usize;
    for (class, source) in &sources {
        assert!(
            independent_xml_verdict(source).is_err(),
            "{class}: the adversarial source must itself be ill-formed, else the case proves nothing"
        );
        match compose_native_fulltext(input(
            source,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Passed,
        )) {
            NativeFulltextCompositionOutcome::Accepted(candidate) => {
                let output = String::from_utf8_lossy(candidate.envelope.raw_tei()).to_string();
                assert_eq!(
                    independent_xml_verdict(&output),
                    Ok(()),
                    "{class}: an Accepted emission must parse independently: {output}"
                );
                assert_eq!(
                    tei_element_bytes(source.as_bytes(), "body").unwrap(),
                    tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap(),
                    "{class}: an Accepted emission may not rewrite the Java body"
                );
            }
            NativeFulltextCompositionOutcome::Rejected { fallback, .. } => {
                assert_eq!(
                    fallback.raw_tei(),
                    source.as_bytes(),
                    "{class}: a refusal must hand back the byte-identical Java envelope"
                );
                rejected += 1;
            }
        }
    }
    assert_eq!(
        rejected,
        sources.len(),
        "every adversarial source is ill-formed, so none may be spliced"
    );
}

/// Positive control for the well-formedness pass: legal markup must not be
/// rejected, including references, CDATA carrying markup, a processing
/// instruction, a comment with single dashes, and attribute values holding `/`
/// and `>`.
#[test]
fn legal_markup_is_not_rejected_by_the_well_formedness_pass() {
    let xml = "<TEI><teiHeader xml:lang=\"en\" note=\"a/b&gt;c\"><fileDesc><titleStmt><title>Smith &amp; Sons &#x2013; apos &apos;</title><ext:note xmlns:ext=\"urn:example:ext\"/></titleStmt></fileDesc><profileDesc><!-- a - b - c --><?target data?><p><![CDATA[ <p> ]] & ]] not a tag ]]></p></profileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let outcome = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    ));
    let NativeFulltextCompositionOutcome::Accepted(candidate) = &outcome else {
        panic!("legal XML must not be refused by the well-formedness pass: {outcome:?}")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    for fragment in [
        "Smith &amp; Sons &#x2013; apos &apos;",
        "<!-- a - b - c -->",
        "<?target data?>",
        "<![CDATA[ <p> ]] & ]] not a tag ]]>",
        "<ext:note xmlns:ext=\"urn:example:ext\"/>",
    ] {
        assert!(
            output.contains(fragment),
            "legal markup lost {fragment}: {output}"
        );
    }
    assert!(
        output.contains("<teiHeader xml:lang=\"en\" note=\"a/b&gt;c\">"),
        "an attribute value holding `/` and `>` must survive verbatim: {output}"
    );
    assert_eq!(
        tei_element_bytes(xml.as_bytes(), "body").unwrap(),
        tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
    );
}

/// S01/T06: the `id`/`xml:id` uniqueness gate must not depend on spelling.
///
/// `xml:id = 'author-0'` is the same attribute as `xml:id="author-0"`, but the
/// uniqueness scan tokenized the open tag on whitespace and only recognised the
/// compact double-quoted form, so a preserved subtree carrying the spaced or
/// single-quoted spelling slipped past the gate while the composer had already
/// synthesized `author-0` — an `Accepted` emission carrying two elements with the
/// same `xml:id` (measured S01/T06).  Every legal spelling has to reach the same
/// typed refusal, and a distinct id written the same way must still compose.
#[test]
fn xml_id_spelling_never_escapes_the_duplicate_id_gate() {
    let java_tail = "<text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let colliding = [
        ("compact double quotes", "xml:id=\"author-0\""),
        ("single quotes", "xml:id='author-0'"),
        ("spaced equals", "xml:id = \"author-0\""),
        ("spaced equals with single quotes", "xml:id = 'author-0'"),
        ("space before the value", "xml:id =\"author-0\""),
        ("space before the equals", "xml:id= 'author-0'"),
    ];
    for (label, attribute) in colliding {
        let xml = format!(
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title><note {attribute}>collides with the synthesized author id</note></titleStmt></fileDesc></teiHeader>{java_tail}"
        );
        match compose_native_fulltext(input(
            &xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                assert!(
                    matches!(
                        error,
                        NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                    ),
                    "{label}: a duplicate xml:id must be a typed refusal, got {error:?}"
                );
                assert_eq!(fallback.raw_tei(), xml.as_bytes(), "{label}");
            }
            NativeFulltextCompositionOutcome::Accepted(candidate) => {
                let output = String::from_utf8_lossy(candidate.envelope.raw_tei()).to_string();
                panic!(
                    "{label}: an emission with a duplicate xml:id must not be published: {output}"
                )
            }
        }
    }
    for (label, attribute) in [
        ("compact double quotes", "xml:id=\"author-9\""),
        ("single quotes", "xml:id='author-9'"),
        ("spaced equals", "xml:id = \"author-9\""),
        ("spaced equals with single quotes", "xml:id = 'author-9'"),
    ] {
        let xml = format!(
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title><note {attribute}>distinct id</note></titleStmt></fileDesc></teiHeader>{java_tail}"
        );
        let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
            &xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) else {
            panic!("{label}: a distinct xml:id must not be refused")
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        assert_eq!(
            independent_xml_verdict(&output),
            Ok(()),
            "{label}: {output}"
        );
        assert!(
            output.contains("author-9"),
            "{label}: the preserved id was lost: {output}"
        );
    }
}

/// S01/T06: `<!DOCTYPE`/`<!ENTITY` is a declaration only where XML markup
/// starts.  Comments and CDATA sections are opaque character data, so a
/// preserved TEI that merely mentions the string stays composable — and the
/// emission must still parse for an independent parser.
#[test]
fn doctype_string_inside_comment_or_cdata_is_not_a_declaration() {
    let comment = "<!-- archived from <!DOCTYPE TEI SYSTEM \"tei.dtd\"> and <!ENTITY ext SYSTEM \"ext.ent\"> -->";
    let cdata = "<![CDATA[<!DOCTYPE TEI SYSTEM \"tei.dtd\"> & <!ENTITY x \"y\">]]>";
    let xml = format!(
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc>{comment}{cdata}</teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>"
    );
    assert_eq!(
        independent_xml_verdict(&xml),
        Ok(()),
        "the source itself must be well-formed, else the case proves nothing"
    );
    let outcome = compose_native_fulltext(input(
        &xml,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    ));
    let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
        panic!("a `<!DOCTYPE` string inside a comment or CDATA is not a declaration: {outcome:?}")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");
    assert!(output.contains(comment), "the comment was lost: {output}");
    assert!(
        output.contains(cdata),
        "the CDATA section was lost: {output}"
    );
}

/// S01/T06: the markup-aware reading must not weaken the gate it replaces.
///
/// XML is case-sensitive, so `<!doctype` is not a declaration — it is illegal
/// markup and a typed well-formedness refusal, never `Accepted`.  A real
/// `<!DOCTYPE` stays the forbidden-external-entity refusal, and a `<!DOCTYPE`
/// string inside an attribute value is still rejected (a raw `<` in `AttValue`),
/// which is what keeps the removal of the raw substring scan from opening a hole.
#[test]
fn markup_declaration_detection_is_markup_aware_not_a_substring_scan() {
    const HEAD: &str = "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title></titleStmt></fileDesc></teiHeader><text><body>";
    const TAIL: &str =
        "</body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>";
    let cases = [
        (
            "uppercase declaration",
            "<!DOCTYPE TEI SYSTEM \"tei.dtd\">",
            true,
        ),
        ("lowercase declaration", "<!doctype TEI>", false),
        (
            "declaration string inside an attribute value",
            "<p note=\"<!DOCTYPE TEI>\">x</p>",
            false,
        ),
    ];
    for (label, payload, forbidden) in cases {
        let xml = format!("{HEAD}{payload}{TAIL}");
        assert!(
            independent_xml_verdict(&xml).is_err(),
            "{label}: the source must itself be ill-formed"
        );
        match compose_native_fulltext(input(
            &xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Passed,
        )) {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                if forbidden {
                    assert!(
                        matches!(
                            error,
                            NativeFulltextCompositionError::ForbiddenExternalEntity
                        ),
                        "{label}: a real declaration stays the forbidden-entity refusal, got {error:?}"
                    );
                } else {
                    let message = format!("{error:?}");
                    assert!(
                        matches!(
                            error,
                            NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                        ),
                        "{label}: illegal markup must be a typed refusal, got {error:?}"
                    );
                    assert!(
                        message.contains("unsupported") || message.contains("attribute value"),
                        "{label}: the refusal must name the class: {message}"
                    );
                }
                assert_eq!(fallback.raw_tei(), xml.as_bytes(), "{label}");
            }
            NativeFulltextCompositionOutcome::Accepted(candidate) => {
                let output = String::from_utf8_lossy(candidate.envelope.raw_tei()).to_string();
                panic!("{label}: ill-formed markup must not be published: {output}")
            }
        }
    }
}

/// S01/T06: the native fragment byte ceiling and the XML depth ceiling refuse
/// with the byte-identical Java envelope — a composed prefix must never escape.
///
/// The byte ceiling is exercised with a header fragment one byte over
/// `NATIVE_FULLTEXT_MAX_COMPONENT_BYTES`, and the depth ceiling with a chain whose
/// element depth is exactly at the limit (accepted) and one over it (refused).
/// The observed depths come from an independent parser, not from the composer's
/// own constants, so the boundary is measured rather than asserted by fiat.
#[test]
fn native_fragment_and_depth_ceilings_refuse_without_partial_output() {
    let padding = "x".repeat(NATIVE_FULLTEXT_MAX_COMPONENT_BYTES + 1);
    let over_ceiling_fragment = format!(
        "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title><note>{padding}</note></titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>"
    );
    match compose_native_fulltext(input(
        &over_ceiling_fragment,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert!(
                matches!(
                    error,
                    NativeFulltextCompositionError::Oversized {
                        component: "native",
                        ..
                    }
                ),
                "a fragment over the component byte ceiling must be an Oversized refusal, got {error:?}"
            );
            assert_eq!(
                fallback.raw_tei(),
                over_ceiling_fragment.as_bytes(),
                "no partial output may escape an over-ceiling fragment"
            );
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("a fragment over the component byte ceiling must not be published")
        }
    }

    let chain = |depth: usize| {
        let mut nested = String::new();
        for index in 0..depth {
            nested.push_str(&format!("<n{index}>"));
        }
        for index in (0..depth).rev() {
            nested.push_str(&format!("</n{index}>"));
        }
        nested
    };
    let element_depth = |xml: &str| {
        roxmltree::Document::parse(xml)
            .expect("independent parser accepts")
            .descendants()
            .filter(|node| node.is_element())
            .map(|node| {
                node.ancestors()
                    .filter(|ancestor| ancestor.is_element())
                    .count()
            })
            .max()
            .expect("at least one element")
    };
    let document = |depth: usize| {
        format!(
            "<TEI><teiHeader xml:lang=\"en\"><fileDesc><titleStmt><title>Java</title>{}</titleStmt></fileDesc></teiHeader><text><body><p>body bytes</p></body><listBibl><biblStruct xml:id=\"b0\">Java ref</biblStruct></listBibl></text></TEI>",
            chain(depth)
        )
    };
    // The chain sits inside TEI > teiHeader > fileDesc > titleStmt, so `depth`
    // nested elements put the deepest one at element level `depth + 4`.
    let at_ceiling = document(NATIVE_FULLTEXT_MAX_XML_DEPTH - 4);
    assert_eq!(
        element_depth(&at_ceiling),
        NATIVE_FULLTEXT_MAX_XML_DEPTH,
        "the at-ceiling document must measure exactly at the limit"
    );
    let NativeFulltextCompositionOutcome::Accepted(candidate) = compose_native_fulltext(input(
        &at_ceiling,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) else {
        panic!("a document exactly at the depth ceiling must still compose")
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert_eq!(element_depth(&output), NATIVE_FULLTEXT_MAX_XML_DEPTH);
    assert_eq!(independent_xml_verdict(&output), Ok(()), "{output}");

    let over_ceiling_depth = document(NATIVE_FULLTEXT_MAX_XML_DEPTH - 3);
    assert_eq!(
        element_depth(&over_ceiling_depth),
        NATIVE_FULLTEXT_MAX_XML_DEPTH + 1
    );
    match compose_native_fulltext(input(
        &over_ceiling_depth,
        AdmissionDisposition::Passed,
        AdmissionDisposition::Unsupported,
    )) {
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            assert!(
                matches!(error, NativeFulltextCompositionError::MalformedXml),
                "a document over the depth ceiling must be a typed refusal, got {error:?}"
            );
            assert_eq!(
                fallback.raw_tei(),
                over_ceiling_depth.as_bytes(),
                "no partial output may escape a document over the depth ceiling"
            );
        }
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("a document over the depth ceiling must not be published")
        }
    }
}
