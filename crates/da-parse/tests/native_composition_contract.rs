use da_parse::{
    AffiliationSpan, CompositionComponent, CompositionError, CompositionSpan, HeaderAuthorInput,
    HeaderCompositionInput, NativeAffiliationAuthorEvidence, NativeAffiliationLinkOutcome,
    NativeAffiliationParser, NativeCitationAnchorOutcome, NativeCitationAnchorScanner,
    NativeCitationParser, NativeDateOutcome, NativeDateParser, NativePersonalNameParser,
    NativeReference, NativeReferenceListOutcome, ReferenceCompositionInput, compose_header,
    compose_references,
};

fn header() -> HeaderCompositionInput {
    let name =
        NativePersonalNameParser::new().parse(da_parse::PersonalNameContext::Header, "J. Smith");
    let affiliation = NativeAffiliationLinkOutcome::Linked {
        author: NativeAffiliationAuthorEvidence {
            ordinal: 0,
            span: AffiliationSpan { start: 0, end: 4 },
        },
        links: vec![],
    };
    HeaderCompositionInput {
        date: NativeDateParser::new().parse("2024-02-29"),
        authors: vec![HeaderAuthorInput {
            ordinal: 0,
            span: CompositionSpan { start: 10, end: 18 },
            name,
            affiliations: affiliation,
        }],
        affiliation_components: vec![NativeAffiliationParser::new().parse("University of Example")],
    }
}

#[test]
fn header_composes_typed_leaf_values_and_keeps_zero_link_author() {
    let result = compose_header(header());
    let da_parse::HeaderCompositionOutcome::Accepted { composition } = result else {
        panic!("expected accepted")
    };
    assert_eq!(composition.candidate.date.year, 2024);
    assert_eq!(composition.candidate.authors[0].ordinal, 0);
    assert!(composition.candidate.authors[0].affiliations.is_empty());
    assert_eq!(composition.receipts.len(), 3);
}

#[test]
fn header_rejects_incomplete_or_reordered_input_without_invention() {
    let mut input = header();
    input.date = NativeDateOutcome::Malformed {
        reason: da_parse::NativeDateMalformed::GrammarMismatch,
    };
    assert!(matches!(
        compose_header(input),
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::RejectedComponent {
                component: CompositionComponent::Date
            }
        }
    ));

    let mut input = header();
    input.authors[0].ordinal = 2;
    input.authors[0].affiliations = NativeAffiliationLinkOutcome::Linked {
        author: NativeAffiliationAuthorEvidence {
            ordinal: 2,
            span: AffiliationSpan { start: 0, end: 4 },
        },
        links: vec![],
    };
    input.authors.push(HeaderAuthorInput {
        ordinal: 1,
        span: CompositionSpan { start: 20, end: 28 },
        ..input.authors[0].clone()
    });
    assert!(matches!(
        compose_header(input),
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::ReorderedOrdinal { .. }
        }
    ));

    let mut input = header();
    input.authors[0].span = CompositionSpan { start: 8, end: 8 };
    assert!(matches!(
        compose_header(input),
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::InvalidSpan { .. }
        }
    ));
}

fn reference(ordinal: usize) -> NativeReference {
    NativeReference {
        ordinal,
        source: format!("ref-{ordinal}"),
        span: da_parse::CitationSpan {
            start: 0,
            end: format!("ref-{ordinal}").len(),
        },
        provenance: "fixture".into(),
    }
}

#[test]
fn references_are_independent_and_preserve_order_fields_and_anchor_adjacency() {
    let anchors = match NativeCitationAnchorScanner::new()
        .scan("<ref type=\"bibr\">A</ref>, <ref type=\"bibr\" target=\"#b1\">B</ref>")
    {
        NativeCitationAnchorOutcome::Recognized {
            anchors,
            adjacent_pairs,
        } => NativeCitationAnchorOutcome::Recognized {
            anchors,
            adjacent_pairs,
        },
        other => panic!("unexpected anchors: {other:?}"),
    };
    let fields = NativeCitationParser::new().parse("Smith. 2024. A title");
    let input = ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![reference(0), reference(1)],
        },
        fields: vec![fields],
        anchors,
    };
    let result = compose_references(input);
    let da_parse::ReferenceCompositionOutcome::Accepted { composition } = result else {
        panic!("expected accepted")
    };
    assert_eq!(
        composition
            .candidate
            .references
            .iter()
            .map(|r| r.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(composition.candidate.anchors.len(), 2);
    assert_eq!(composition.candidate.adjacent_pairs, vec![(0, 1)]);
    assert!(!composition.candidate.fields.is_empty());
}

#[test]
fn references_reject_duplicate_or_reordered_ordinals_and_bad_adjacency() {
    let base = ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![reference(1), reference(1)],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    };
    assert!(matches!(
        compose_references(base),
        da_parse::ReferenceCompositionOutcome::Rejected {
            error: CompositionError::DuplicateOrdinal { .. }
        }
    ));
    let reordered = ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![reference(2), reference(1)],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    };
    assert!(matches!(
        compose_references(reordered),
        da_parse::ReferenceCompositionOutcome::Rejected {
            error: CompositionError::ReorderedOrdinal { .. }
        }
    ));
}

#[test]
fn serde_replay_is_deterministic_and_does_not_redact_typed_receipts() {
    let result = compose_header(header());
    let bytes = serde_json::to_vec(&result).unwrap();
    let replay: da_parse::HeaderCompositionOutcome = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
    let wire = String::from_utf8(bytes).unwrap();
    assert!(wire.contains("accepted"));
    assert!(wire.contains("\"given_name\":\"J\""));
    assert!(wire.contains("\"surname\":\"Smith\""));
}
