// Archived measurement probe for A-XML/A-REF (audit 2026-09-21).
//
// STATUS: archived audit evidence, NOT a cargo target and NOT production
// reachability evidence. The same measurement is now executed as a tracked
// regression in the library test surface:
//   crates/da-parse/tests/native_fulltext_composition_contract.rs
//     * a_xml_red_preserved_tei_emissions_are_rejected_by_an_independent_parser
//       (runs by default; pins the measured RED)
//     * preserved_tei_composition_is_independently_well_formed
//       (#[ignore]d flip gate; fails while the defect is RED)
// Both use the dev-only `roxmltree` parser (never importable from src/) and
// verify the sha256 of the three preserved fixtures before measuring.
//
// Measurement re-confirmed on the tracked fixtures: `Accepted` on all three
// (hub_2507.19457, reca_2605.26525v1, rlm_2512.24601) with an independent
// parser rejecting every emission at the duplicated `</title>`.
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

fn main() {
    for name in [
        "hub_2507.19457.tei.xml",
        "reca_2605.26525v1.tei.xml",
        "rlm_2512.24601.tei.xml",
    ] {
        let xml =
            std::fs::read_to_string(format!("crates/da-parse/tests/fixtures/preserved/{name}"))
                .unwrap();
        match compose_native_fulltext(input(
            &xml,
            AdmissionDisposition::Passed,
            AdmissionDisposition::Unsupported,
        )) {
            NativeFulltextCompositionOutcome::Accepted(c) => {
                std::fs::write(format!("/tmp/audit-{name}"), c.envelope.raw_tei()).unwrap();
                println!("{name}: Accepted");
            }
            other => println!("{name}: {other:?}"),
        }
    }
    let xml="<TEI><teiHeader><fileDesc><titleStmt><title>T</title></titleStmt></fileDesc></teiHeader><text><body><p><ref type=\"bibr\" target=\"#b0\">[1]</ref></p></body><listBibl><biblStruct xml:id=\"b0\">Original</biblStruct></listBibl></text></TEI>";
    if let NativeFulltextCompositionOutcome::Accepted(c) = compose_native_fulltext(input(
        xml,
        AdmissionDisposition::Unsupported,
        AdmissionDisposition::Passed,
    )) {
        std::fs::write("/tmp/audit-dangling.xml", c.envelope.raw_tei()).unwrap();
        println!("empty supplied anchors: Accepted");
    }
}
