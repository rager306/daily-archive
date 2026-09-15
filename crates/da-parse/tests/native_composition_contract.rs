use da_parse::{
    AffiliationSpan, CompositionComponent, CompositionError, CompositionSpan, HeaderAuthorInput,
    HeaderCompositionInput, HeaderFeatureRow, HeaderFieldSpan, HeaderSequenceInput,
    NativeAffiliationAuthorEvidence, NativeAffiliationLinkOutcome, NativeAffiliationParser,
    NativeCitationAnchorOutcome, NativeCitationAnchorScanner, NativeCitationParser,
    NativeDateOutcome, NativeDateParser, NativeHeaderFields, NativePersonalNameParser,
    NativeReference, NativeReferenceListOutcome, ReferenceCompositionInput, ReferenceSequenceInput,
    compose_header, compose_header_with_fields, compose_references, project_native_header_fields,
    run_header_sequence, run_header_sequence_with_fields, run_reference_sequence,
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
fn header_sequence_is_date_then_name_then_affiliation() {
    let result = run_header_sequence(HeaderSequenceInput {
        date_text: "2024-02-29",
        name_texts: vec!["J. Smith"],
        affiliation_texts: vec!["University of Example"],
    });
    let da_parse::HeaderCompositionOutcome::Accepted { composition } = result else {
        panic!("expected accepted sequence")
    };
    let order: Vec<_> = composition.receipts.iter().map(|r| r.component).collect();
    assert_eq!(
        order,
        vec![
            CompositionComponent::Date,
            CompositionComponent::Names,
            CompositionComponent::Affiliations,
        ]
    );
    assert_eq!(composition.candidate.date.year, 2024);
    assert_eq!(composition.candidate.authors.len(), 1);
}

#[test]
fn header_sequence_stops_on_empty_date_and_does_not_invent_authors() {
    let result = run_header_sequence(HeaderSequenceInput {
        date_text: "   ",
        name_texts: vec!["J. Smith"],
        affiliation_texts: vec!["University of Example"],
    });
    assert!(matches!(
        result,
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Date
            }
        }
    ));
}

#[test]
fn invented_or_empty_author_is_rejected() {
    let empty_name = run_header_sequence(HeaderSequenceInput {
        date_text: "2024-02-29",
        name_texts: vec![""],
        affiliation_texts: vec!["University of Example"],
    });
    assert!(matches!(
        empty_name,
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Names
            }
        }
    ));

    let mut input = header();
    input.authors[0].name = da_parse::NativePersonalNameOutcome::Empty;
    assert!(matches!(
        compose_header(input),
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Names
            }
        }
    ));

    let mut foreign = header();
    foreign.authors[0].affiliations = NativeAffiliationLinkOutcome::Linked {
        author: NativeAffiliationAuthorEvidence {
            ordinal: 9,
            span: AffiliationSpan { start: 0, end: 4 },
        },
        links: vec![],
    };
    assert!(matches!(
        compose_header(foreign),
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::ForeignAffiliationLink { author: 0, .. }
        }
    ));
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
fn reference_sequence_is_segment_then_fields_then_compose() {
    let result = run_reference_sequence(ReferenceSequenceInput {
        entries: vec!["Smith, J. Example Title. 2024."],
        body_text: Some("<ref type=\"bibr\">A</ref>"),
    });
    let da_parse::ReferenceCompositionOutcome::Accepted { composition } = result else {
        panic!("expected accepted bibliography sequence")
    };
    let order: Vec<_> = composition.receipts.iter().map(|r| r.component).collect();
    assert_eq!(
        order,
        vec![
            CompositionComponent::References,
            CompositionComponent::CitationFields,
            CompositionComponent::Anchors,
        ]
    );
    assert_eq!(composition.candidate.references.len(), 1);
}

#[test]
fn lost_or_empty_reference_is_hard_fail_and_does_not_invent_candidate() {
    let empty = run_reference_sequence(ReferenceSequenceInput {
        entries: vec![],
        body_text: None,
    });
    assert!(matches!(
        empty,
        da_parse::ReferenceCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::References
            }
        }
    ));

    let hole = run_reference_sequence(ReferenceSequenceInput {
        entries: vec!["Smith, J. Example Title. 2024.", "   "],
        body_text: None,
    });
    assert!(matches!(
        hole,
        da_parse::ReferenceCompositionOutcome::Rejected {
            error: CompositionError::RejectedComponent {
                component: CompositionComponent::References
            }
        }
    ));
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

// ───── S03/T04: decoded header fields ride the composition without invention ─────

/// Independent fixture: rows and spans are hand-written here, never produced
/// by the header renderer or decoder under test.
fn projected_fields() -> NativeHeaderFields {
    let rows = vec![
        HeaderFeatureRow {
            columns: vec!["Attention".into()],
        },
        HeaderFeatureRow {
            columns: vec!["Is".into()],
        },
        HeaderFeatureRow {
            columns: vec!["ashish@example.test".into()],
        },
        HeaderFeatureRow {
            columns: vec!["arXiv:1706.03762".into()],
        },
    ];
    let spans = [
        HeaderFieldSpan {
            label: "<title>".into(),
            start: 0,
            end: 2,
        },
        HeaderFieldSpan {
            label: "<email>".into(),
            start: 2,
            end: 3,
        },
        HeaderFieldSpan {
            label: "<pubnum>".into(),
            start: 3,
            end: 4,
        },
    ];
    project_native_header_fields(&rows, &spans).unwrap()
}

#[test]
fn legacy_header_composition_keeps_fields_empty() {
    let da_parse::HeaderCompositionOutcome::Accepted { composition } = compose_header(header())
    else {
        panic!("expected accepted")
    };
    assert_eq!(composition.candidate.fields, NativeHeaderFields::default());
    assert!(composition.candidate.fields.title.is_empty());
}

#[test]
fn projected_fields_attach_to_accepted_header_on_both_entry_points() {
    let fields = projected_fields();
    assert_eq!(fields.title, vec!["Attention Is"]);
    assert_eq!(fields.email, vec!["ashish@example.test"]);
    assert_eq!(fields.ids, vec!["arXiv:1706.03762"]);

    let da_parse::HeaderCompositionOutcome::Accepted { composition } =
        compose_header_with_fields(header(), fields.clone())
    else {
        panic!("expected accepted")
    };
    assert_eq!(composition.candidate.fields, fields);
    assert_eq!(composition.receipts.len(), 3);

    let da_parse::HeaderCompositionOutcome::Accepted { composition } =
        run_header_sequence_with_fields(
            HeaderSequenceInput {
                date_text: "2024-02-29",
                name_texts: vec!["J. Smith"],
                affiliation_texts: vec!["University of Example"],
            },
            fields.clone(),
        )
    else {
        panic!("expected accepted sequence")
    };
    assert_eq!(composition.candidate.fields, fields);
    assert_eq!(
        composition
            .receipts
            .iter()
            .map(|r| r.component)
            .collect::<Vec<_>>(),
        vec![
            CompositionComponent::Date,
            CompositionComponent::Names,
            CompositionComponent::Affiliations,
        ]
    );
    let wire = serde_json::to_string(&composition.candidate.fields).unwrap();
    assert!(wire.contains("Attention Is"));
    assert!(wire.contains("arXiv:1706.03762"));
}

#[test]
fn rejected_header_never_carries_projected_fields() {
    let fields = projected_fields();

    let mut broken = header();
    broken.authors[0].name = da_parse::NativePersonalNameOutcome::Empty;
    let rejected = compose_header_with_fields(broken, fields.clone());
    assert!(matches!(
        rejected,
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Names
            }
        }
    ));

    let rejected = run_header_sequence_with_fields(
        HeaderSequenceInput {
            date_text: "   ",
            name_texts: vec!["J. Smith"],
            affiliation_texts: vec![],
        },
        fields,
    );
    assert!(matches!(
        rejected,
        da_parse::HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Date
            }
        }
    ));
    let wire = serde_json::to_string(&rejected).unwrap();
    for leaked in ["Attention Is", "ashish@example.test", "arXiv:1706.03762"] {
        assert!(
            !wire.contains(leaked),
            "a rejected header must not carry the projected field {leaked:?}"
        );
    }
}
