use da_parse::{
    AffiliationSpan, NativeAffiliationLinkError, NativeAffiliationLinkOutcome,
    NativeAffiliationLinker, NativeAffiliationNesting, NativeAffiliationOccurrence,
    NativePersonalNameOutcome, NativePersonalNameParser, PersonalNameContext,
};

fn recognized_name() -> da_parse::RecognizedPersonalName {
    match NativePersonalNameParser::new().parse(PersonalNameContext::Header, "Jane Q. Public") {
        NativePersonalNameOutcome::Recognized { name } => name,
        other => panic!("expected accepted name, got {other:?}"),
    }
}

fn occurrence(key: &str, ordinal: usize, start: usize, end: usize) -> NativeAffiliationOccurrence {
    NativeAffiliationOccurrence {
        document_id: "doc-a".to_owned(),
        key: key.to_owned(),
        ordinal,
        span: AffiliationSpan { start, end },
        nesting: NativeAffiliationNesting::DirectAuthorAffiliation,
    }
}

#[test]
fn links_three_declared_affiliations_in_source_order_and_preserves_evidence() {
    let name = recognized_name();
    let affiliations = [
        occurrence("aff-1", 0, 30, 48),
        occurrence("aff-2", 1, 50, 67),
        occurrence("aff-3", 2, 69, 88),
    ];

    let outcome = NativeAffiliationLinker::new().link(
        &name,
        "doc-a",
        4,
        AffiliationSpan { start: 10, end: 28 },
        &affiliations,
    );

    let NativeAffiliationLinkOutcome::Linked { author, links } = outcome else {
        panic!("expected linked outcome");
    };
    assert_eq!(author.ordinal, 4);
    assert_eq!(author.span, AffiliationSpan { start: 10, end: 28 });
    assert_eq!(links.len(), 3);
    assert_eq!(
        links
            .iter()
            .map(|link| link.affiliation_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        links
            .iter()
            .map(|link| link.key.as_str())
            .collect::<Vec<_>>(),
        vec!["aff-1", "aff-2", "aff-3"]
    );
    assert_eq!(links[2].affiliation_span, affiliations[2].span);
}

#[test]
fn an_accepted_name_with_no_affiliations_is_a_successful_empty_link_set() {
    let outcome = NativeAffiliationLinker::new().link(
        &recognized_name(),
        "doc-a",
        0,
        AffiliationSpan { start: 0, end: 10 },
        &[],
    );

    assert!(matches!(
        outcome,
        NativeAffiliationLinkOutcome::Linked { links, .. } if links.is_empty()
    ));
}

#[test]
fn rejects_duplicate_keys_and_foreign_or_missing_keys_without_inventing_links() {
    let linker = NativeAffiliationLinker::new();
    let name = recognized_name();
    let duplicate = [
        occurrence("aff-1", 0, 20, 30),
        occurrence("aff-1", 1, 32, 42),
    ];
    let duplicate_outcome = linker.link(
        &name,
        "doc-a",
        1,
        AffiliationSpan { start: 0, end: 10 },
        &duplicate,
    );
    assert!(matches!(
        duplicate_outcome,
        NativeAffiliationLinkOutcome::Error {
            error: NativeAffiliationLinkError::DuplicateKey { .. }
        }
    ));

    let missing = [occurrence("", 0, 20, 30)];
    let missing_outcome = linker.link(
        &name,
        "doc-a",
        1,
        AffiliationSpan { start: 0, end: 10 },
        &missing,
    );
    assert!(matches!(
        missing_outcome,
        NativeAffiliationLinkOutcome::Error {
            error: NativeAffiliationLinkError::MissingKey { .. }
        }
    ));

    let mut foreign = [occurrence("aff-9", 0, 20, 30)];
    foreign[0].document_id = "other-doc".to_owned();
    let foreign_outcome = linker.link(
        &name,
        "doc-a",
        1,
        AffiliationSpan { start: 0, end: 10 },
        &foreign,
    );
    assert!(matches!(
        foreign_outcome,
        NativeAffiliationLinkOutcome::Error {
            error: NativeAffiliationLinkError::UnknownKey { .. }
        }
    ));
}

#[test]
fn rejects_invalid_spans_and_reordered_affiliation_ordinals() {
    let linker = NativeAffiliationLinker::new();
    let name = recognized_name();
    let invalid = [occurrence("aff-1", 0, 20, 20)];
    assert!(matches!(
        linker.link(
            &name,
            "doc-a",
            1,
            AffiliationSpan { start: 0, end: 10 },
            &invalid
        ),
        NativeAffiliationLinkOutcome::Error {
            error: NativeAffiliationLinkError::InvalidSpan { .. }
        }
    ));

    let reordered = [
        occurrence("aff-2", 1, 40, 50),
        occurrence("aff-1", 0, 20, 30),
    ];
    assert!(matches!(
        linker.link(
            &name,
            "doc-a",
            1,
            AffiliationSpan { start: 0, end: 10 },
            &reordered,
        ),
        NativeAffiliationLinkOutcome::Error {
            error: NativeAffiliationLinkError::Reordered { .. }
        }
    ));
}

#[test]
fn serde_replay_is_deterministic_and_redacted() {
    let outcome = NativeAffiliationLinker::new().link(
        &recognized_name(),
        "doc-a",
        2,
        AffiliationSpan { start: 4, end: 14 },
        &[occurrence("aff-1", 0, 20, 30)],
    );
    let bytes = serde_json::to_vec(&outcome).unwrap();
    let replay: NativeAffiliationLinkOutcome = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(outcome, replay);
    let json = String::from_utf8(bytes).unwrap();
    assert!(!json.contains("Jane") && !json.contains("Public"));
}
