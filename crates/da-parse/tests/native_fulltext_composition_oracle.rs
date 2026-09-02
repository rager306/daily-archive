//! Independent oracle for the S18 fulltext shadow composition.
//!
//! Expected values are derived from the frozen XML fixture by a deliberately
//! small test-local inventory.  This file never calls the composer to build
//! its expected dispositions or digests.

use da_parse::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/native_fulltext_composition_oracle.yaml");
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Debug, Deserialize)]
struct Fixture {
    source_sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    source_xml: String,
    expected: Expected,
}
#[derive(Debug, Deserialize)]
struct Expected {
    source_families: Vec<String>,
    section_order: Vec<String>,
    reference_ids: Vec<String>,
    figure_ids: Vec<String>,
    coordinate_values: Vec<String>,
    native_components: Vec<String>,
    fallback_components: Vec<String>,
    denominators: Denominators,
    s17_output_affecting: bool,
}
#[derive(Debug, Deserialize)]
struct Denominators {
    s12_named: usize,
    s15_terminal: usize,
    s16_visual_heads: usize,
}

fn fixture() -> Fixture {
    serde_yaml::from_str(FIXTURE).expect("oracle fixture parses")
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = xml[at..].find(&open) {
        let start = at + i;
        let open_end = start + xml[start..].find('>').expect("fixture tag") + 1;
        let end = open_end + xml[open_end..].find(&close).expect("fixture close") + close.len();
        out.push(&xml[start..end]);
        at = end;
    }
    out
}
fn attr(block: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = block.find(&needle)? + needle.len();
    Some(block[start..].split('"').next()?.to_owned())
}
fn text(block: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    block
        .split_once(&open)
        .and_then(|(_, x)| x.split_once(&close))
        .map(|(x, _)| x.to_owned())
        .unwrap_or_default()
}
fn opening_tags<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let needle = format!("<{tag}");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = xml[at..].find(&needle) {
        let start = at + i;
        let end = start + xml[start..].find('>').expect("fixture tag") + 1;
        out.push(&xml[start..end]);
        at = end;
    }
    out
}
struct IndependentInventory {
    families: Vec<String>,
    sections: Vec<String>,
    references: Vec<String>,
    figures: Vec<String>,
    coordinates: Vec<String>,
}

fn independent_inventory(f: &Fixture) -> IndependentInventory {
    let mut families = vec!["title".into()];
    let mut sections = Vec::new();
    let mut at = 0;
    while let Some(i) = f.source_xml[at..].find("<head>") {
        let start = at + i;
        let end = start
            + f.source_xml[start..]
                .find("</head>")
                .expect("fixture close")
            + 7;
        if !f.source_xml[..start]
            .rfind("<figure")
            .is_some_and(|figure| figure > f.source_xml[..start].rfind("</figure>").unwrap_or(0))
        {
            sections.push(text(&f.source_xml[start..end], "head"));
        }
        at = end;
    }
    families.push("section".into());
    let figures = opening_tags(&f.source_xml, "figure")
        .into_iter()
        .filter_map(|x| attr(x, "xml:id"))
        .collect::<Vec<_>>();
    families.push("figure".into());
    let coords = opening_tags(&f.source_xml, "graphic")
        .into_iter()
        .filter_map(|x| attr(x, "coords"))
        .collect::<Vec<_>>();
    families.push("coordinate".into());
    let refs = blocks(&f.source_xml, "biblStruct")
        .into_iter()
        .filter_map(|x| attr(x, "xml:id"))
        .collect::<Vec<_>>();
    families.push("citation_entry".into());
    IndependentInventory {
        families,
        sections,
        references: refs,
        figures,
        coordinates: coords,
    }
}
fn admissions() -> Vec<FulltextAdmissionReceipt> {
    [
        (
            FulltextComponent::Header,
            AdmissionDisposition::Passed,
            "passing",
        ),
        (
            FulltextComponent::References,
            AdmissionDisposition::Passed,
            "passing",
        ),
        (
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::InvalidEvidence,
            "unsupported",
        ),
        (
            FulltextComponent::Figure,
            AdmissionDisposition::Unsupported,
            "unsupported",
        ),
        (
            FulltextComponent::Table,
            AdmissionDisposition::NotObserved,
            "unsupported",
        ),
        (
            FulltextComponent::Formula,
            AdmissionDisposition::Unsupported,
            "unsupported",
        ),
        (
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
            "unsupported",
        ),
    ]
    .into_iter()
    .map(
        |(component, disposition, evidence_state)| FulltextAdmissionReceipt {
            component,
            producer: "oracle-fixture".into(),
            disposition,
            evidence_state: evidence_state.into(),
            schema_version: "native-composition.outcome/v1".into(),
            source_sha256: fixture().source_sha256,
            artifact_sha256: ARTIFACT.into(),
            fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
        },
    )
    .collect()
}
fn input(f: &Fixture) -> NativeFulltextCompositionInput {
    let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, "J. Smith");
    let header = compose_header(HeaderCompositionInput {
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
    });
    let references = compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![NativeReference {
                ordinal: 0,
                source: "Reference one".into(),
                span: CitationSpan { start: 0, end: 13 },
                provenance: "oracle-fixture".into(),
            }],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    });
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            f.source_xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, &f.profile_fingerprint),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: f.source_sha256.clone(),
            profile_fingerprint: f.profile_fingerprint.clone(),
            config_fingerprint: f.config_fingerprint.clone(),
        },
        header,
        references,
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: admissions(),
        s17_lineage: Some(S17LineageReceipt {
            source: "immutable-s17".into(),
            artifact_sha256: ARTIFACT.into(),
            live_ruvllm_receipt: false,
            output_affecting: false,
        }),
    }
}

#[test]
fn independent_oracle_matches_hybrid_candidate_and_all_fallback_families() {
    let f = fixture();
    assert_eq!(sha(f.source_xml.as_bytes()), f.source_sha256);
    let inventory = independent_inventory(&f);
    assert_eq!(inventory.families, f.expected.source_families);
    assert_eq!(inventory.sections, f.expected.section_order);
    assert_eq!(inventory.references, f.expected.reference_ids);
    assert_eq!(inventory.figures, f.expected.figure_ids);
    assert_eq!(
        inventory.coordinates.concat(),
        f.expected.coordinate_values.concat()
    );
    assert_eq!(f.expected.native_components, ["header", "references"]);
    assert_eq!(f.expected.fallback_components.len(), 5);
    assert_eq!(
        (
            f.expected.denominators.s12_named,
            f.expected.denominators.s15_terminal,
            f.expected.denominators.s16_visual_heads
        ),
        (3, 9, 27)
    );
    assert!(!f.expected.s17_output_affecting);

    let candidate = match compose_native_fulltext(input(&f)) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => candidate,
        NativeFulltextCompositionOutcome::Rejected { error, .. } => {
            panic!("frozen input rejected: {error:?}")
        }
    };
    let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
    assert!(output.contains("<surname>Smith</surname>"));
    assert!(output.contains("<biblStruct xml:id=\"bibl-0\">Reference one</biblStruct>"));
    assert!(output.contains(
        "<figure xml:id=\"fig-1\"><figDesc>Caption</figDesc><graphic coords=\"1,2,3,4\"/>"
    ));
    assert_eq!(candidate.manifest.native_components, 2);
    assert_eq!(candidate.manifest.fallback_components, 5);
    assert_eq!(candidate.manifest.java_fallback_bytes, f.source_xml.len());
    assert_eq!(
        candidate.manifest.java_fallback_sha256,
        format!("sha256:{0}", sha(f.source_xml.as_bytes()))
    );
    assert!(!candidate.manifest.s17_output_affecting);
    for component in [
        FulltextComponent::DocumentSegmentation,
        FulltextComponent::Figure,
        FulltextComponent::Table,
        FulltextComponent::Formula,
        FulltextComponent::LearningPolicy,
    ] {
        let entry = candidate
            .manifest
            .components
            .iter()
            .find(|x| x.component == component)
            .unwrap();
        assert!(matches!(
            entry.disposition,
            AdmissionDisposition::InvalidEvidence
                | AdmissionDisposition::Unsupported
                | AdmissionDisposition::NotObserved
        ));
        assert_eq!(entry.byte_count, f.source_xml.len());
        assert_eq!(entry.fallback_target, NATIVE_FULLTEXT_FALLBACK_TARGET);
    }
}

#[test]
fn forged_oracle_checksum_and_source_mutations_are_not_accepted() {
    let mut f = fixture();
    f.source_xml = f.source_xml.replace("Second section", "Reordered section");
    assert_ne!(
        sha(f.source_xml.as_bytes()),
        f.source_sha256,
        "mutation must escape frozen checksum"
    );
    // The independent oracle must detect source drift before a candidate is
    // admitted; the pure composer intentionally consumes an already-bound
    // identity and does not guess a checksum from raw bytes.
    assert_ne!(sha(f.source_xml.as_bytes()), f.source_sha256);
    assert!(!fixture().expected.s17_output_affecting);
}
