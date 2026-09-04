use da_parse::*;

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
        artifact_sha256: ARTIFACT.into(),
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
