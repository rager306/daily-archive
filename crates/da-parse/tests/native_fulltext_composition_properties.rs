use da_parse::*;
use proptest::prelude::*;

const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const XML_PREFIX: &str = "<TEI><teiHeader><fileDesc><titleStmt><title>";
const XML_SUFFIX: &str = "</title></titleStmt></fileDesc></teiHeader><text><body><p>fallback</p></body><listBibl><biblStruct>ref</biblStruct></listBibl></text></TEI>";
/// Body and bibliography only: lets a property build its own header statement.
const XML_BODY: &str = "<text><body><p>fallback</p></body><listBibl><biblStruct>ref</biblStruct></listBibl></text></TEI>";

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
        artifact_sha256: String::new(),
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

    /// M511-j7wz2f/S01/T02: an accepted header splice stays well-formed and loses
    /// no source title, whatever the repetition and nesting inside the source
    /// title statement.  The T01 defect was a range that could start at a closing
    /// `</title>` and swallow the elements up to the next `<title>`, so titles are
    /// generated as repeats and the nested subtree is generated two levels deep.
    #[test]
    fn accepted_header_splice_stays_well_formed_and_loses_no_title(
        titles in prop::collection::vec("[A-Za-z0-9][A-Za-z0-9 ]{0,30}", 1..4),
        nested in "[A-Za-z0-9 ]{0,20}",
    ) {
        let mut statement = String::new();
        for (index, title) in titles.iter().enumerate() {
            statement.push_str(&format!("<title type=\"t{index}\">{title}</title>"));
        }
        statement.push_str(&format!("<custom><custom>{nested}</custom></custom>"));
        let xml = format!(
            "<TEI><teiHeader><fileDesc><titleStmt>{statement}</titleStmt></fileDesc></teiHeader>{XML_BODY}"
        );
        let outcome = compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed));
        let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
            prop_assert!(false, "header splice rejected for {xml}: {outcome:?}");
            unreachable!()
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        let parsed = roxmltree::Document::parse(&output);
        prop_assert!(parsed.is_ok(), "emission is not well-formed: {output}");
        prop_assert_eq!(
            roxmltree::Document::parse(&output)
                .unwrap()
                .descendants()
                .filter(|node| node.is_element() && node.tag_name().name() == "title")
                .count(),
            titles.len()
        );
        prop_assert!(
            output.contains(&format!("<custom><custom>{nested}</custom></custom>")),
            "nested subtree lost: {output}"
        );
        prop_assert_eq!(
            tei_element_bytes(xml.as_bytes(), "body").unwrap(),
            tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
        );
    }

    /// M511-j7wz2f/S01/T04: one measured A-XML class was a namespace declaration
    /// written with whitespace around `=` (`Name S? Eq S? AttValue`).  However
    /// legal XML spells it, the declaration must travel with the subtree it
    /// covers, the emission must parse for an independent parser, and the
    /// preserved prefix must resolve to the declared URI.
    #[test]
    fn wrapper_declaration_spacing_never_breaks_the_emission(
        left in prop::collection::vec(prop::sample::select(vec![' ', '\t', '\n', '\r']), 0..3)
            .prop_map(|chars| chars.into_iter().collect::<String>()),
        right in prop::collection::vec(prop::sample::select(vec![' ', '\t', '\n', '\r']), 0..3)
            .prop_map(|chars| chars.into_iter().collect::<String>()),
    ) {
        let declarations = format!("xmlns:ext{left}={right}\"urn:example:ext\"");
        for xml in [
            format!(
                "<TEI><teiHeader><fileDesc {declarations}><titleStmt><title>Java</title></titleStmt><sourceDesc><ext:blob/></sourceDesc></fileDesc></teiHeader>{XML_BODY}"
            ),
            format!(
                "<TEI><teiHeader><fileDesc><titleStmt {declarations}><title>Java</title><ext:note/></titleStmt></fileDesc></teiHeader>{XML_BODY}"
            ),
        ] {
            let outcome =
                compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed));
            let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
                prop_assert!(false, "legal declaration spacing rejected {xml}");
                unreachable!()
            };
            let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
            prop_assert!(
                output.contains("xmlns:ext=\"urn:example:ext\""),
                "the declaration was not carried: {output}"
            );
            let parsed = roxmltree::Document::parse(&output);
            prop_assert!(parsed.is_ok(), "emission is not well-formed: {output}");
            prop_assert!(
                parsed.unwrap().descendants().any(|node| {
                    node.is_element() && node.tag_name().namespace() == Some("urn:example:ext")
                }),
                "the preserved prefix did not resolve: {output}"
            );
            prop_assert_eq!(
                tei_element_bytes(xml.as_bytes(), "body").unwrap(),
                tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
            );
        }
    }

    /// M511-j7wz2f/S01/T04: no adversarial source text may produce an `Accepted`
    /// emission an independent parser rejects.  Either the emission parses (and
    /// the Java body bytes are untouched), or the composition refuses and hands
    /// back the byte-identical Java envelope — never a republished ill-formed
    /// document, which is what the composer used to return for a raw `&`.
    #[test]
    fn adversarial_title_text_never_yields_an_unparseable_accepted_emission(
        text in prop::collection::vec(prop::char::any(), 0..24)
            .prop_map(|chars| chars.into_iter().collect::<String>()),
    ) {
        let xml = format!(
            "<TEI><teiHeader><fileDesc><titleStmt><title>{text}</title></titleStmt></fileDesc></teiHeader>{XML_BODY}"
        );
        match compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed)) {
            NativeFulltextCompositionOutcome::Accepted(candidate) => {
                let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
                let parsed = roxmltree::Document::parse(&output);
                prop_assert!(
                    parsed.is_ok(),
                    "an Accepted emission must parse independently: {output}"
                );
                prop_assert_eq!(
                    tei_element_bytes(xml.as_bytes(), "body").unwrap(),
                    tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
                );
            }
            NativeFulltextCompositionOutcome::Rejected { fallback, .. } => {
                prop_assert_eq!(fallback.raw_tei(), xml.as_bytes());
            }
        }
    }

    /// M511-j7wz2f/S01/T05: the wrappers the composer synthesizes conserve their
    /// source *non-element* children too.  Element-only copying silently dropped a
    /// direct comment, processing instruction or text node while still returning
    /// `Accepted`, so each generated marker must still be a direct child of the
    /// wrapper it came from.
    #[test]
    fn wrapper_non_element_children_survive_the_splice(marker in "[A-Za-z0-9 ]{1,24}") {
        let xml = format!(
            "<TEI><teiHeader xml:lang=\"en\"><!-- c {marker} --><?p {marker}?><fileDesc><titleStmt><title>Java</title><!-- t {marker} --></titleStmt><publicationStmt><publisher>Example</publisher></publicationStmt></fileDesc>h {marker} <profileDesc><abstract><p>abstract</p></abstract></profileDesc></teiHeader>{XML_BODY}"
        );
        let outcome = compose_native_fulltext(input(xml.clone(), AdmissionDisposition::Passed));
        let NativeFulltextCompositionOutcome::Accepted(candidate) = outcome else {
            prop_assert!(
                false,
                "well-formed source with non-element children was not accepted: {xml}"
            );
            unreachable!()
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        let parsed = roxmltree::Document::parse(&output);
        prop_assert!(parsed.is_ok(), "emission is not well-formed: {output}");
        let document = parsed.unwrap();
        let direct_children_stream = |parent: &str| -> String {
            document
                .descendants()
                .find(|node| node.is_element() && node.tag_name().name() == parent)
                .unwrap_or_else(|| panic!("{parent} present"))
                .children()
                .map(|child| &output[child.range()])
                .collect()
        };
        let header_stream = direct_children_stream("teiHeader");
        for fragment in [
            format!("<!-- c {marker} -->"),
            format!("<?p {marker}?>"),
            format!("h {marker} "),
        ] {
            prop_assert!(
                header_stream.contains(&fragment),
                "direct teiHeader child {fragment} was lost: {header_stream}"
            );
        }
        let title_stmt_stream = direct_children_stream("titleStmt");
        prop_assert!(
            title_stmt_stream.contains(&format!("<!-- t {marker} -->")),
            "direct titleStmt child was lost: {title_stmt_stream}"
        );
        prop_assert_eq!(
            tei_element_bytes(xml.as_bytes(), "body").unwrap(),
            tei_element_bytes(candidate.envelope.raw_tei(), "body").unwrap()
        );
    }

    /// M511-j7wz2f/S01/T06: the `xml:id` uniqueness gate reads the XML attribute
    /// grammar (`Name S? Eq S? AttValue`), so no legal spelling — either quote
    /// character, any whitespace around `=` — may slip a colliding id past it,
    /// and a distinct id written the same way must still compose.
    #[test]
    fn any_legal_xml_id_spelling_hits_the_same_duplicate_id_gate(
        quote in prop::sample::select(vec!['"', '\'']),
        left in "[ \t]{0,2}",
        right in "[ \t]{0,2}",
    ) {
        let attribute = |id: &str| format!("xml:id{left}={right}{quote}{id}{quote}");
        let document = |id: &str| {
            format!(
                "<TEI><teiHeader><fileDesc><titleStmt><title>Java</title><note {}>an id</note></titleStmt></fileDesc></teiHeader>{XML_BODY}",
                attribute(id)
            )
        };
        let colliding = document("author-0");
        match compose_native_fulltext(input(colliding.clone(), AdmissionDisposition::Passed)) {
            NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                prop_assert!(
                    matches!(
                        error,
                        NativeFulltextCompositionError::ComposedOutputNotWellFormed(_)
                    ),
                    "a colliding id written as {} must be a typed refusal: {error:?}",
                    attribute("author-0")
                );
                prop_assert_eq!(fallback.raw_tei(), colliding.as_bytes());
            }
            NativeFulltextCompositionOutcome::Accepted(candidate) => {
                let output = String::from_utf8_lossy(candidate.envelope.raw_tei()).to_string();
                prop_assert!(
                    false,
                    "a colliding id written as {} was published: {output}",
                    attribute("author-0")
                );
            }
        }
        let distinct = document("author-7");
        let NativeFulltextCompositionOutcome::Accepted(candidate) =
            compose_native_fulltext(input(distinct.clone(), AdmissionDisposition::Passed))
        else {
            prop_assert!(false, "a distinct id was refused: {distinct}");
            unreachable!()
        };
        let output = String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap();
        prop_assert!(
            roxmltree::Document::parse(&output).is_ok(),
            "emission is not well-formed: {output}"
        );
        prop_assert!(
            output.contains("author-7"),
            "the preserved distinct id was lost: {output}"
        );
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
