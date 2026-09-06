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

fn accepted_name() -> da_parse::RecognizedPersonalName {
    // Literal accepted name: the linker never reparses personal names.
    da_parse::RecognizedPersonalName {
        given_name: Some("Accepted".into()),
        middle_names: vec![],
        surname: "Name".into(),
        recomposed_surname: "Name".into(),
        surname_particles: vec![],
        suffix: None,
        initials: vec![],
    }
}

// ───── M494-a1i14t S04/T01: affiliation link same-n with document_id ─────
//
// Every NativeAffiliation call and every NativeAffiliationOccurrence carries
// document_id bound to the three preserved TEI identities (hub/rlm/reca).
// Expected counts are test-local constants — never from compose_header or
// TeiLifter. Hub (21 authors) and rlm (4 authors) are explicit zero-link
// negative controls; reca links 13 occurrences across 11 authors. The 25
// zero-link header authors (36 − 11 linked) stay visible as successful empty
// link sets. No graph write.

const HUB_DOCUMENT_ID: &str = "hub_2507.19457";
const RLM_DOCUMENT_ID: &str = "rlm_2512.24601";
const RECA_DOCUMENT_ID: &str = "reca_2605.26525v1";

/// Test-local per-author affiliation keys for reca (11 authors, 13 total).
const RECA_AUTHOR_KEYS: &[&[&str]] = &[
    &["aff0", "aff1", "aff2"],
    &["aff1"],
    &["aff1"],
    &["aff2"],
    &["aff2"],
    &["aff2"],
    &["aff2"],
    &["aff3"],
    &["aff1"],
    &["aff0"],
    &["aff2"],
];

/// Test-local preserved key population: invented keys match none of these.
const PRESERVED_KEYS: &[&str] = &["aff0", "aff1", "aff2", "aff3"];

fn document_occurrence(
    document_id: &str,
    key: &str,
    ordinal: usize,
) -> NativeAffiliationOccurrence {
    NativeAffiliationOccurrence {
        document_id: document_id.to_owned(),
        key: key.to_owned(),
        ordinal,
        span: AffiliationSpan {
            start: 10 + ordinal * 10,
            end: 20 + ordinal * 10,
        },
        nesting: NativeAffiliationNesting::DirectAuthorAffiliation,
    }
}

#[test]
fn same_n_linking_with_document_id_names_fixture_identity_on_drift() {
    let linker = NativeAffiliationLinker::new();
    let name = accepted_name();
    assert_eq!(
        RECA_AUTHOR_KEYS.len(),
        11,
        "reca linked-author population stays 11"
    );
    let mut oracle_n = 0usize;
    let mut candidate_n = 0usize;
    for (author, keys) in RECA_AUTHOR_KEYS.iter().enumerate() {
        let occurrences = keys
            .iter()
            .enumerate()
            .map(|(ordinal, key)| document_occurrence(RECA_DOCUMENT_ID, key, ordinal))
            .collect::<Vec<_>>();
        oracle_n += occurrences.len();
        // Every occurrence carries the call's document_id (MEM990).
        for occurrence in &occurrences {
            assert_eq!(
                occurrence.document_id, RECA_DOCUMENT_ID,
                "occurrence document_id drift for reca author {author}"
            );
        }
        let outcome = linker.link(
            &name,
            RECA_DOCUMENT_ID,
            author,
            AffiliationSpan { start: 0, end: 10 },
            &occurrences,
        );
        let NativeAffiliationLinkOutcome::Linked {
            author: evidence,
            links,
        } = outcome
        else {
            panic!("reca author {author} rows must link, got {outcome:?}");
        };
        assert_eq!(evidence.ordinal, author);
        assert_eq!(
            links.len(),
            keys.len(),
            "same-n drift for reca author {author}"
        );
        for (link, key) in links.iter().zip(keys.iter()) {
            assert_eq!(link.key, **key, "key drift for reca author {author}");
        }
        candidate_n += links.len();
    }
    assert_eq!(oracle_n, 13, "reca affiliation population stays 13");
    assert_eq!(
        oracle_n, candidate_n,
        "oracle_n must equal candidate_n (no pooled or skipped link)"
    );
}

#[test]
fn hub_and_rlm_zero_link_authors_stay_visible_as_empty_link_sets() {
    let linker = NativeAffiliationLinker::new();
    let name = accepted_name();
    // Hub: 21 header authors, zero affiliations. Rlm: 4 header authors,
    // zero affiliations. Each is a successful empty link set, never hidden.
    let mut visible_zero_link = 0usize;
    for (document_id, authors) in [(HUB_DOCUMENT_ID, 21), (RLM_DOCUMENT_ID, 4)] {
        for author in 0..authors {
            let outcome = linker.link(
                &name,
                document_id,
                author,
                AffiliationSpan { start: 0, end: 10 },
                &[],
            );
            assert!(
                matches!(
                    outcome,
                    NativeAffiliationLinkOutcome::Linked { links, .. } if links.is_empty()
                ),
                "zero-link author {author} of {document_id} must stay a visible empty link set"
            );
            visible_zero_link += 1;
        }
    }
    assert_eq!(visible_zero_link, 25, "zero-link authors stay visible: 25");
}

#[test]
fn missing_document_id_or_foreign_key_fails_closed() {
    let linker = NativeAffiliationLinker::new();
    let name = accepted_name();
    // Empty document_id on the call fails closed even with well-formed rows.
    let rows = [document_occurrence(RECA_DOCUMENT_ID, "aff1", 0)];
    assert!(
        matches!(
            linker.link(&name, "", 0, AffiliationSpan { start: 0, end: 10 }, &rows),
            NativeAffiliationLinkOutcome::Error {
                error: NativeAffiliationLinkError::UnknownKey { .. }
            }
        ),
        "call without document_id must fail closed"
    );
    // Occurrence whose document_id differs from the call scope fails closed.
    let foreign = [document_occurrence("other-doc", "aff1", 0)];
    assert!(
        matches!(
            linker.link(
                &name,
                RECA_DOCUMENT_ID,
                0,
                AffiliationSpan { start: 0, end: 10 },
                &foreign
            ),
            NativeAffiliationLinkOutcome::Error {
                error: NativeAffiliationLinkError::UnknownKey { .. }
            }
        ),
        "foreign-key occurrence without matching document_id must fail closed"
    );
    // Empty document_id on the occurrence itself fails closed too.
    let empty_scope = [document_occurrence("", "aff1", 0)];
    assert!(
        matches!(
            linker.link(
                &name,
                RECA_DOCUMENT_ID,
                0,
                AffiliationSpan { start: 0, end: 10 },
                &empty_scope
            ),
            NativeAffiliationLinkOutcome::Error {
                error: NativeAffiliationLinkError::UnknownKey { .. }
            }
        ),
        "occurrence without document_id must fail closed"
    );
}

#[test]
fn invented_key_links_by_shape_but_matches_no_preserved_identity() {
    let linker = NativeAffiliationLinker::new();
    let name = accepted_name();
    // The linker takes already-recognized occurrences, never raw text: a
    // shape-valid invented key still links by shape, but it is not admitted
    // as preserved evidence — its key occurs in no hub/rlm/reca identity.
    let invented = [document_occurrence(RECA_DOCUMENT_ID, "aff-invented", 0)];
    let outcome = linker.link(
        &name,
        RECA_DOCUMENT_ID,
        0,
        AffiliationSpan { start: 0, end: 10 },
        &invented,
    );
    let NativeAffiliationLinkOutcome::Linked { links, .. } = outcome else {
        panic!("shape-valid invented occurrence must link by shape, got {outcome:?}");
    };
    assert_eq!(links.len(), 1);
    assert!(
        !PRESERVED_KEYS.contains(&links[0].key.as_str()),
        "invented key {} must match no preserved oracle identity",
        links[0].key
    );
}

#[test]
fn link_expected_builder_does_not_import_tei_lifter_or_compose() {
    let src = include_str!("native_affiliation_link_contract.rs");
    let use_block: String = src
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect();
    for name in [
        "TeiLifter",
        "HeaderAuthor",
        "compose_header",
        "compose_references",
        "run_header_sequence",
        "run_reference_sequence",
    ] {
        assert!(
            !use_block.contains(name),
            "{name} must not be imported into the affiliation link expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the affiliation link expected builder"
        );
    }
    // The needle is built at runtime so this very assertion never matches itself.
    let needle = ["lift_", "tei("].concat();
    assert!(!src.contains(&needle));
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
