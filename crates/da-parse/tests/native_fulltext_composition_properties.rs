use da_parse::*;
use proptest::prelude::*;

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const XML_PREFIX: &str = "<TEI><teiHeader><fileDesc><titleStmt><title>";
const XML_SUFFIX: &str = "</title></titleStmt></fileDesc></teiHeader><text><body><p>fallback</p></body><listBibl><biblStruct>ref</biblStruct></listBibl></text></TEI>";

fn receipt(
    component: FulltextComponent,
    disposition: AdmissionDisposition,
) -> FulltextAdmissionReceipt {
    FulltextAdmissionReceipt {
        component,
        producer: "property-fixture".into(),
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
fn input(xml: String, disposition: AdmissionDisposition) -> NativeFulltextCompositionInput {
    let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, "J. Smith");
    let header = compose_header(HeaderCompositionInput {
        date: NativeDateParser::new().parse("2024-02-29"),
        authors: vec![HeaderAuthorInput {
            ordinal: 0,
            span: CompositionSpan { start: 1, end: 2 },
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
    });
    let references = compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![NativeReference {
                ordinal: 0,
                source: "ref".into(),
                span: CitationSpan { start: 0, end: 3 },
                provenance: "fixture".into(),
            }],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    });
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml,
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, "profile=property"),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: SOURCE.into(),
            profile_fingerprint: "profile=property".into(),
            config_fingerprint: "config=property".into(),
        },
        header,
        references,
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: vec![
            receipt(FulltextComponent::Header, disposition),
            receipt(
                FulltextComponent::References,
                AdmissionDisposition::Unsupported,
            ),
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
        ],
        s17_lineage: None,
    }
}

proptest! {
    #[test]
    fn java_payload_is_preserved_exactly_when_header_is_not_admitted(
        title in "[A-Za-z0-9 ]{0,40}"
    ) {
        let xml = format!("{XML_PREFIX}{title}{XML_SUFFIX}");
        let expected = xml.as_bytes().to_vec();
        let outcome = compose_native_fulltext(input(xml, AdmissionDisposition::Stale));
        let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else { panic!("valid Java XML should yield a hybrid candidate") };
        prop_assert_eq!(candidate.envelope.raw_tei(), expected.as_slice());
        prop_assert_eq!(candidate.manifest.native_components, 0);
        prop_assert_eq!(candidate.manifest.fallback_components, 7);
        prop_assert_eq!(candidate.manifest.java_fallback_bytes, expected.len());
    }

    #[test]
    fn accepted_header_is_replay_stable(title in "[A-Za-z0-9 ]{0,40}") {
        let xml = format!("{XML_PREFIX}{title}{XML_SUFFIX}");
        let a = compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed));
        let b = compose_native_fulltext(input(xml, AdmissionDisposition::Passed));
        let NativeFulltextCompositionOutcome::Accepted(a) = a else { panic!() };
        let NativeFulltextCompositionOutcome::Accepted(b) = b else { panic!() };
        prop_assert_eq!(a.envelope.raw_tei(), b.envelope.raw_tei());
        prop_assert_eq!(a.manifest, b.manifest);
    }
}

#[test]
fn oversized_java_payload_is_rejected_without_partial_output() {
    let xml = format!(
        "{XML_PREFIX}{}{}",
        "x".repeat(NATIVE_FULLTEXT_MAX_JAVA_BYTES),
        XML_SUFFIX
    );
    let outcome = compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed));
    let NativeFulltextCompositionOutcome::Rejected { fallback, error } = outcome else {
        panic!()
    };
    assert!(matches!(
        error,
        NativeFulltextCompositionError::Oversized { .. }
    ));
    assert_eq!(fallback.raw_tei(), xml.as_bytes());
}
