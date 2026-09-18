//! Occurrence → bibliography entry link contract (M509-yzblqx / S05 / T01 /
//! GAR-013).
//!
//! This target is the executable form of the published contract in
//! [`da_ports::citation_link`]: which key forms join an in-text occurrence to a
//! bibliography entry, which form wins when several entries compete, and what
//! happens when the join cannot be made. A competing, partial or absent
//! identity must resolve by the stated precedence or stay explicitly
//! unresolved — never by construction order, and never as a guessed identity.

use da_ports::citation_link::{
    AmbiguousLinkReason, CITATION_LINK_PRECEDENCE, CITATION_LINK_SCHEMA_VERSION,
    CitationEntryLinkKeys, CitationLinkCounts, CitationLinkKey, CitationLinkKeyKind,
    CitationLinkOutcome, UnlinkedCitationReason, positional_entry_key, precedence_rank,
    resolve_anchor_to_entry, resolve_occurrence_to_entry, resolve_occurrences,
};
use da_ports::parser::{CitationAnchor, CitationEntry};

/// `(ordinal, declared_id)` per entry, in lifted document order.
fn link_keys(spec: &[(u32, Option<&str>)]) -> Vec<CitationEntryLinkKeys> {
    spec.iter()
        .map(|(ordinal, declared)| CitationEntryLinkKeys::new(*ordinal, *declared))
        .collect()
}

fn wire_entry(raw_text: &str) -> CitationEntry {
    CitationEntry {
        raw_text: raw_text.into(),
        doi: None,
        arxiv_id: None,
        title: None,
        anchors: vec![],
    }
}

/// A declared identifier on both sides links by `declared_entry_id`, and the
/// key carries which entry was reached.
#[test]
fn declared_id_is_the_winning_key() {
    let entries = link_keys(&[(0, Some("b12")), (1, Some("b13"))]);
    let outcome = resolve_occurrence_to_entry(Some("b12"), &entries);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Linked {
            entry_index: 0,
            key: CitationLinkKey::DeclaredEntryId {
                entry_id: "b12".into()
            },
            shadowed_entry_indexes: vec![],
        }
    );
    assert!(outcome.is_linked() && !outcome.is_contested());
    assert_eq!(outcome.entry_index(), Some(0));
    assert_eq!(
        outcome.linked_key().map(CitationLinkKey::rank),
        Some(precedence_rank(CitationLinkKeyKind::DeclaredEntryId))
    );
}

/// Neither side declares an identifier: the occurrence points at the
/// positional key and the entry publishes the position it was lifted at.
#[test]
fn positional_fallback_links_an_entry_without_declared_id() {
    let entries = link_keys(&[(0, None), (1, None)]);
    let outcome = resolve_occurrence_to_entry(Some(&positional_entry_key(1)), &entries);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Linked {
            entry_index: 1,
            key: CitationLinkKey::SyntheticEntryOrdinal { ordinal: 1 },
            shadowed_entry_indexes: vec![],
        }
    );
}

/// A positional key that collides with another entry's declared identifier is
/// decided by published precedence, not by construction order: the declared
/// identifier wins and the positional candidate is reported as shadowed.
#[test]
fn declared_id_outranks_positional_fallback_regardless_of_entry_order() {
    // entry 0 has no declared id → positional key "b0"; entry 3 declares "b0".
    let declared_last = link_keys(&[(0, None), (1, None), (2, None), (3, Some("b0"))]);
    let outcome = resolve_occurrence_to_entry(Some("b0"), &declared_last);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Linked {
            entry_index: 3,
            key: CitationLinkKey::DeclaredEntryId {
                entry_id: "b0".into()
            },
            shadowed_entry_indexes: vec![0],
        },
        "the declared identity takes the link; the positional coincidence is shadowed, not silently dropped"
    );
    assert!(outcome.is_contested());
    assert_eq!(outcome.shadowed_entry_indexes(), [0]);

    // Same identities, reversed construction order → same winner.
    let declared_first = link_keys(&[(3, Some("b0")), (0, None), (1, None), (2, None)]);
    let mut mirrored = declared_last.clone();
    mirrored.reverse();
    let mirrored_outcome = resolve_occurrence_to_entry(Some("b0"), &mirrored);
    assert_eq!(mirrored_outcome.entry_index(), Some(0));
    assert_eq!(
        mirrored_outcome.shadowed_entry_indexes(),
        [3],
        "the shadowed candidate is whichever entry publishes the weaker positional key (ordinal 0 now sits at list index 3)"
    );
    assert_eq!(
        mirrored[3].published_key(),
        CitationLinkKey::SyntheticEntryOrdinal { ordinal: 0 },
        "the shadowed candidate really is the positional key that lost"
    );
    assert_eq!(
        mirrored_outcome.linked_key(),
        outcome.linked_key(),
        "the winning key form is order-independent"
    );
    assert!(mirrored_outcome.is_contested());
    assert_eq!(
        resolve_occurrence_to_entry(Some("b0"), &declared_first).entry_index(),
        Some(0)
    );
}

/// Competing declared identifiers are ambiguous: every candidate is reported
/// and no single entry is chosen.
#[test]
fn duplicate_declared_id_is_ambiguous_with_every_candidate() {
    let entries = link_keys(&[(0, Some("b7")), (1, Some("b8")), (2, Some("b7"))]);
    let outcome = resolve_occurrence_to_entry(Some("b7"), &entries);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Ambiguous {
            ref_id: Some("b7".into()),
            candidate_entry_indexes: vec![0, 2],
            reason: AmbiguousLinkReason::DuplicateDeclaredId,
        }
    );
    assert!(!outcome.is_linked());
    assert_eq!(
        outcome.entry_index(),
        None,
        "an ambiguous match never yields one guessed identity"
    );
    assert_eq!(outcome.candidate_entry_indexes(), [0, 2]);
    assert_eq!(outcome.ref_id(), Some("b7"));
}

/// A producer that reuses an ordinal is ambiguous too — still no guess.
#[test]
fn competing_keys_at_the_same_precedence_are_ambiguous() {
    let entries = link_keys(&[(0, None), (0, None)]);
    let outcome = resolve_occurrence_to_entry(Some("b0"), &entries);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Ambiguous {
            ref_id: Some("b0".into()),
            candidate_entry_indexes: vec![0, 1],
            reason: AmbiguousLinkReason::CompetingKeysAtSamePrecedence,
        }
    );
}

/// An occurrence with no usable identifier is explicitly unresolved, and the
/// absence is named rather than converted into a link.
#[test]
fn absent_occurrence_key_is_unresolved_without_a_guess() {
    let entries = link_keys(&[(0, Some("b0")), (1, None)]);
    for absent in [None, Some(""), Some("   "), Some("#"), Some(" # ")] {
        let outcome = resolve_occurrence_to_entry(absent, &entries);
        assert_eq!(
            outcome,
            CitationLinkOutcome::Unresolved {
                ref_id: None,
                reason: UnlinkedCitationReason::MissingOccurrenceKey,
            },
            "absent key {absent:?} must stay unresolved"
        );
        assert_eq!(outcome.entry_index(), None);
        assert_eq!(outcome.ref_id(), None);
        assert_eq!(
            outcome.unresolved_reason(),
            Some(UnlinkedCitationReason::MissingOccurrenceKey)
        );
    }
}

/// A partial or unknown identity is unresolved, not approximated against a
/// neighbouring entry.
#[test]
fn partial_or_unknown_identity_is_unresolved() {
    let entries = link_keys(&[(0, Some("b0")), (1, Some("b1")), (2, None)]);
    for unknown in ["b99", "b0v2", "b1~2", "B0"] {
        let outcome = resolve_occurrence_to_entry(Some(unknown), &entries);
        assert_eq!(
            outcome,
            CitationLinkOutcome::Unresolved {
                ref_id: Some(unknown.to_string()),
                reason: UnlinkedCitationReason::NoEntryWithKey,
            },
            "{unknown} matches no published key and must stay unresolved"
        );
        assert!(!outcome.is_linked());
    }
    // entry 2 publishes "b2" only; "b12" is not a prefix match.
    let outcome = resolve_occurrence_to_entry(Some("b12"), &entries);
    assert_eq!(outcome.entry_index(), None);
    assert_eq!(outcome.ref_id(), Some("b12"));

    // Empty bibliography: nothing to link to, still named.
    let outcome = resolve_occurrence_to_entry(Some("b0"), &[]);
    assert_eq!(
        outcome,
        CitationLinkOutcome::Unresolved {
            ref_id: Some("b0".into()),
            reason: UnlinkedCitationReason::NoEntryWithKey,
        }
    );
}

/// An entry with a declared identifier does not also offer its position: a
/// positional coincidence is not an identity.
#[test]
fn declared_entry_does_not_offer_its_position_as_identity() {
    let entries = link_keys(&[(0, Some("bX"))]);
    assert_eq!(
        entries[0].published_key(),
        CitationLinkKey::DeclaredEntryId {
            entry_id: "bX".into()
        }
    );
    assert_eq!(entries[0].declared_id(), Some("bX"));
    assert_eq!(entries[0].ordinal(), 0);
    let outcome = resolve_occurrence_to_entry(Some("b0"), &entries);
    assert_eq!(
        outcome.unresolved_reason(),
        Some(UnlinkedCitationReason::NoEntryWithKey),
        "the positional key of a declared entry is not published"
    );
}

/// The wire-type view degrades honestly: `CitationEntry` carries no declared
/// identifier, so it publishes only its positional key.
#[test]
fn wire_entry_publishes_only_the_positional_key() {
    let view = CitationEntryLinkKeys::from_entry(4, &wire_entry("Smith et al. 2024"));
    assert_eq!(view.declared_id(), None);
    assert_eq!(
        view.published_key(),
        CitationLinkKey::SyntheticEntryOrdinal { ordinal: 4 }
    );
    assert_eq!(positional_entry_key(4), "b4");
}

/// Real anchors resolve through the contract; a document with a declared
/// identifier and a positional target both link, in anchor order.
#[test]
fn anchors_resolve_through_the_published_contract() {
    let entries = link_keys(&[(0, Some("b12")), (1, None)]);
    let anchors = [
        CitationAnchor {
            ref_id: "b12".into(),
            section_path_id: Some("3.2".into()),
            quote: "[12]".into(),
            offset: Some(41),
        },
        CitationAnchor {
            ref_id: "b1".into(),
            section_path_id: None,
            quote: "[1]".into(),
            offset: None,
        },
        CitationAnchor {
            ref_id: "b404".into(),
            section_path_id: Some("5".into()),
            quote: "[?]".into(),
            offset: Some(7),
        },
    ];

    let declared = resolve_anchor_to_entry(&anchors[0], &entries);
    assert_eq!(declared.entry_index(), Some(0));
    assert!(declared.is_linked() && !declared.is_contested());

    let positional = resolve_anchor_to_entry(&anchors[1], &entries);
    assert_eq!(positional.entry_index(), Some(1));
    assert_eq!(
        positional.linked_key(),
        Some(&CitationLinkKey::SyntheticEntryOrdinal { ordinal: 1 })
    );

    let unresolved = resolve_anchor_to_entry(&anchors[2], &entries);
    assert_eq!(unresolved.entry_index(), None);
    assert_eq!(
        unresolved.unresolved_reason(),
        Some(UnlinkedCitationReason::NoEntryWithKey)
    );

    // Stream form preserves input order and agrees with per-anchor resolution.
    let stream = resolve_occurrences(
        anchors.iter().map(|anchor| Some(anchor.ref_id.as_str())),
        &entries,
    );
    assert_eq!(stream.len(), anchors.len());
    assert_eq!(stream[0], declared);
    assert_eq!(stream[1], positional);
    assert_eq!(stream[2], unresolved);
}

/// The precedence table is published data: ranks are consistent, the declared
/// form is strictly stronger, and every key kind is ranked.
#[test]
fn precedence_table_is_published_and_consistent() {
    assert_eq!(CITATION_LINK_SCHEMA_VERSION, 1);
    assert_eq!(CITATION_LINK_PRECEDENCE.len(), 2);
    assert_eq!(
        CITATION_LINK_PRECEDENCE[0],
        CitationLinkKeyKind::DeclaredEntryId
    );
    for (index, kind) in CITATION_LINK_PRECEDENCE.iter().enumerate() {
        assert_eq!(precedence_rank(*kind), index);
        let key = match kind {
            CitationLinkKeyKind::DeclaredEntryId => CitationLinkKey::DeclaredEntryId {
                entry_id: "b1".into(),
            },
            CitationLinkKeyKind::SyntheticEntryOrdinal => {
                CitationLinkKey::SyntheticEntryOrdinal { ordinal: 1 }
            }
        };
        assert_eq!(key.kind(), *kind);
        assert_eq!(key.rank(), index);
        assert_eq!(key.match_text(), "b1");
    }
}

/// Outcomes and key forms are machine-readable: a consumer can tag-match the
/// wire form without re-deriving the semantics.
#[test]
fn outcomes_are_machine_readable() {
    let entries = link_keys(&[(0, Some("b0"))]);

    let linked = serde_json::to_value(resolve_occurrence_to_entry(Some("b0"), &entries)).unwrap();
    assert_eq!(linked["outcome"], "linked");
    assert_eq!(linked["entry_index"], 0);
    assert_eq!(linked["key"]["kind"], "declared_entry_id");
    assert_eq!(linked["key"]["entry_id"], "b0");
    assert_eq!(linked["shadowed_entry_indexes"], serde_json::json!([]));

    let ambiguous = serde_json::to_value(resolve_occurrence_to_entry(
        Some("b0"),
        &link_keys(&[(0, Some("b0")), (1, Some("b0"))]),
    ))
    .unwrap();
    assert_eq!(ambiguous["outcome"], "ambiguous");
    assert_eq!(ambiguous["reason"], "duplicate_declared_id");
    assert_eq!(
        ambiguous["candidate_entry_indexes"],
        serde_json::json!([0, 1])
    );
    assert!(ambiguous.get("entry_index").is_none());

    let unresolved = serde_json::to_value(resolve_occurrence_to_entry(None, &entries)).unwrap();
    assert_eq!(unresolved["outcome"], "unresolved");
    assert_eq!(unresolved["reason"], "missing_occurrence_key");

    let positional =
        serde_json::to_value(CitationLinkKey::SyntheticEntryOrdinal { ordinal: 3 }).unwrap();
    assert_eq!(
        positional,
        serde_json::json!({ "kind": "synthetic_entry_ordinal", "ordinal": 3 })
    );
}

/// Unresolved and ambiguous links stay countable for a whole stream: a reader
/// counts them instead of trusting a guess.
#[test]
fn unresolved_links_stay_countable() {
    let entries = link_keys(&[(0, Some("b0")), (1, None)]);
    let anchors = [
        "b0", // declared link
        "b1", // positional link (entry 1 has no declared id)
        "b9", // unknown → unresolved
        "#",  // absent → unresolved
    ];
    let outcomes = resolve_occurrences(anchors.iter().map(|ref_id| Some(*ref_id)), &entries);
    let counts = CitationLinkCounts::from_outcomes(&outcomes);
    assert_eq!(counts.linked, 2);
    assert_eq!(counts.contested, 0);
    assert_eq!(counts.unresolved, 2);
    assert_eq!(counts.ambiguous, 0);
    assert_eq!(counts.total(), anchors.len());
    assert_eq!(counts.unlinked(), 2);

    // Countable per reason, not just in aggregate.
    let unresolved_missing = outcomes
        .iter()
        .filter(|outcome| {
            outcome.unresolved_reason() == Some(UnlinkedCitationReason::MissingOccurrenceKey)
        })
        .count();
    let unresolved_unknown = outcomes
        .iter()
        .filter(|outcome| {
            outcome.unresolved_reason() == Some(UnlinkedCitationReason::NoEntryWithKey)
        })
        .count();
    assert_eq!((unresolved_missing, unresolved_unknown), (1, 1));

    // Ambiguous links are counted separately, never folded into linked.
    let mixed = CitationLinkCounts::from_outcomes(&resolve_occurrences(
        ["b0", "b0"].into_iter().map(Some),
        &link_keys(&[(0, Some("b0")), (1, Some("b0"))]),
    ));
    assert_eq!((mixed.linked, mixed.ambiguous, mixed.unresolved), (0, 2, 0));
    assert_eq!(mixed.unlinked(), 2);
}

/// Resolution is pure: repeated calls agree, and reordering the occurrence
/// stream permutes the outcomes without changing any of them.
#[test]
fn resolution_is_repeatable_and_occurrence_order_independent() {
    let entries = link_keys(&[(0, Some("b10")), (1, Some("b11"))]);
    let refs = ["b10", "b11", "b99"];
    let forward = resolve_occurrences(refs.iter().map(|ref_id| Some(*ref_id)), &entries);
    let again = resolve_occurrences(refs.iter().map(|ref_id| Some(*ref_id)), &entries);
    assert_eq!(forward, again, "same inputs, same outcomes");

    let reversed = resolve_occurrences(refs.iter().rev().map(|ref_id| Some(*ref_id)), &entries);
    let mut expected = forward.clone();
    expected.reverse();
    assert_eq!(
        reversed, expected,
        "per-occurrence outcomes do not depend on stream order"
    );
}

/// No outcome shape can be read as a guessed identity: a link names exactly
/// one entry, ambiguity names two or more and no entry, and an unresolved
/// occurrence names none.
#[test]
fn no_outcome_shape_produces_a_guessed_identity() {
    let entries = link_keys(&[(0, Some("b0")), (1, Some("b0")), (2, None), (3, None)]);
    let outcomes = resolve_occurrences(
        [
            Some("b0"),   // ambiguous (two declared)
            Some("b2"),   // positional link
            Some("b3"),   // positional link
            Some("b404"), // unresolved
            None,         // unresolved
        ],
        &entries,
    );

    for outcome in &outcomes {
        match outcome {
            CitationLinkOutcome::Linked {
                entry_index,
                shadowed_entry_indexes,
                ..
            } => {
                assert!(*entry_index < entries.len());
                assert!(
                    !shadowed_entry_indexes.contains(entry_index),
                    "the winner is never also shadowed"
                );
                assert!(outcome.candidate_entry_indexes().is_empty());
                assert!(outcome.unresolved_reason().is_none());
            }
            CitationLinkOutcome::Ambiguous {
                candidate_entry_indexes,
                ..
            } => {
                assert!(
                    candidate_entry_indexes.len() >= 2,
                    "ambiguity requires more than one candidate"
                );
                assert!(
                    candidate_entry_indexes
                        .windows(2)
                        .all(|pair| pair[0] < pair[1]),
                    "candidates are ascending and deduplicated"
                );
                assert!(outcome.entry_index().is_none());
                assert!(outcome.linked_key().is_none());
                assert!(outcome.unresolved_reason().is_none());
            }
            CitationLinkOutcome::Unresolved { ref_id, reason: _ } => {
                assert!(outcome.entry_index().is_none());
                assert!(outcome.linked_key().is_none());
                assert!(outcome.candidate_entry_indexes().is_empty());
                assert!(
                    ref_id.is_none() || ref_id.as_deref().is_some_and(|r| !r.is_empty()),
                    "an unresolved outcome never carries a blank identifier as evidence"
                );
            }
        }
    }
}

/// 10x scale check: a 1000-entry bibliography (10x a large real one) resolved
/// for 2000 occurrences stays linear and bounded. The key scan is the only cost
/// on this path — there is no I/O, pool, cache or retry to protect.
#[test]
fn ten_x_bibliography_stays_bounded() {
    let declared: Vec<String> = (0..1_000).map(|index| format!("b{index}")).collect();
    let entries: Vec<CitationEntryLinkKeys> = declared
        .iter()
        .enumerate()
        .map(|(index, id)| CitationEntryLinkKeys::new(index as u32, Some(id.as_str())))
        .collect();
    let refs: Vec<Option<&str>> = (0..2_000)
        .map(|index| Some(declared[index % declared.len()].as_str()))
        .collect();

    let started = std::time::Instant::now();
    let outcomes = resolve_occurrences(refs, &entries);
    let elapsed = started.elapsed();

    let counts = CitationLinkCounts::from_outcomes(&outcomes);
    assert_eq!(
        counts.linked, 2_000,
        "every occurrence links by declared id"
    );
    assert_eq!(counts.unlinked(), 0);
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "2_000_000 key comparisons must stay well under 5s, took {elapsed:?}"
    );
}
