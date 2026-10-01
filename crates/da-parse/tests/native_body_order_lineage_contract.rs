//! Contract and negative tests for the additive order-lineage surface
//! (`da-parse.native-body-order-lineage/v1`).
//!
//! These tests pin the three properties the S05/T01 diagnostic depends on:
//! explicit document token identity, no silent drop or duplication, and no
//! lexeme mismatch across orders. They are deliberately independent of any PDF
//! or preserved-oracle bytes — the probe owns the bytes, this suite owns the
//! refusal vocabulary.

use da_parse::{
    BodyOrderEntry, BodyOrderLane, BodyOrderLaneDisposition, BodyOrderLineage,
    BodyOrderLineageError, DocumentTokenId, NATIVE_BODY_ORDER_LINEAGE_SCHEMA_VERSION,
    SEGMENTER_REJECTED_LATTICE, audit_body_order_lineage, ordered_fragments,
};
use sha2::{Digest, Sha256};

fn hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn id(page_ordinal: u32, source_ordinal: u32) -> DocumentTokenId {
    DocumentTokenId {
        page_ordinal,
        source_ordinal,
    }
}

fn entry(text: &str, page_ordinal: u32, source_ordinal: u32, token_ordinal: u32) -> BodyOrderEntry {
    BodyOrderEntry {
        id: id(page_ordinal, source_ordinal),
        token_ordinal,
        text_digest: hex(text),
    }
}

/// A three-token order map whose tokens are `applied`, `learner`, `model`.
fn sample(lane: BodyOrderLane) -> BodyOrderLineage {
    BodyOrderLineage::build(
        "doc-a",
        lane,
        vec![
            entry("applied", 0, 0, 0),
            entry("learner", 0, 1, 1),
            entry("model", 0, 2, 2),
        ],
    )
    .expect("sample lineage builds")
}

#[test]
fn the_lane_vocabulary_is_closed_over_native_order_sources() {
    assert_eq!(BodyOrderLane::ALL.len(), 3);
    assert_eq!(
        BodyOrderLane::RawSourceOrder.disposition(),
        BodyOrderLaneDisposition::PrimaryLaneDeliveryOrder
    );
    assert_eq!(
        BodyOrderLane::PositionedAdmittedOrder.disposition(),
        BodyOrderLaneDisposition::PrimaryLaneAdmittedOrder
    );
    assert_eq!(
        BodyOrderLane::XyCutAdmittedOrder.disposition(),
        BodyOrderLaneDisposition::SegmenterRejectedLattice
    );
    assert_eq!(
        BodyOrderLaneDisposition::SegmenterRejectedLattice.wire(),
        SEGMENTER_REJECTED_LATTICE
    );
}

#[test]
fn a_lane_may_not_be_deserialized_from_an_oracle_source() {
    // The order vocabulary is closed over native order sources. An oracle-derived
    // order is not a lane, so it cannot even be written down: reordering from the
    // comparison oracle is structurally unreachable, not merely discouraged.
    assert!(serde_json::from_str::<BodyOrderLane>("\"raw_source_order\"").is_ok());
    assert!(serde_json::from_str::<BodyOrderLane>("\"xy_cut_admitted_order\"").is_ok());
    assert!(serde_json::from_str::<BodyOrderLane>("\"oracle_reading_order\"").is_err());
    assert!(serde_json::from_str::<BodyOrderLane>("\"oracle\"").is_err());
    assert!(serde_json::from_str::<BodyOrderLane>("\"preserved_source_order\"").is_err());
}

#[test]
fn an_order_map_is_an_explicit_list_of_document_tokens() {
    let lineage = sample(BodyOrderLane::RawSourceOrder);
    assert_eq!(
        lineage.schema_version,
        NATIVE_BODY_ORDER_LINEAGE_SCHEMA_VERSION
    );
    assert_eq!(lineage.document_id, "doc-a");
    assert_eq!(lineage.len(), 3);
    assert!(!lineage.is_empty());
    assert_eq!(
        lineage.ids().collect::<Vec<_>>(),
        vec![id(0, 0), id(0, 1), id(0, 2)]
    );
    assert!(lineage.order_digest.starts_with("sha256:"));
}

#[test]
fn an_empty_order_is_a_typed_absence_not_a_pass() {
    let error = BodyOrderLineage::build("doc-a", BodyOrderLane::RawSourceOrder, vec![])
        .expect_err("empty order must be refused");
    assert_eq!(
        error,
        BodyOrderLineageError::EmptyOrder {
            lane: BodyOrderLane::RawSourceOrder
        }
    );
}

#[test]
fn a_duplicate_document_identity_is_refused() {
    let error = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::RawSourceOrder,
        vec![entry("applied", 0, 0, 0), entry("learner", 0, 0, 1)],
    )
    .expect_err("duplicate identity must be refused");
    assert_eq!(
        error,
        BodyOrderLineageError::DuplicateTokenId { id: id(0, 0) }
    );
}

#[test]
fn a_duplicate_token_ordinal_on_one_page_is_refused() {
    let error = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::RawSourceOrder,
        vec![entry("applied", 0, 0, 1), entry("learner", 0, 1, 1)],
    )
    .expect_err("duplicate token ordinal must be refused");
    assert_eq!(
        error,
        BodyOrderLineageError::DuplicateTokenOrdinal {
            page_ordinal: 0,
            token_ordinal: 1
        }
    );
}

#[test]
fn an_uppercase_digest_is_not_the_declared_digest_form() {
    let mut bad = entry("applied", 0, 0, 0);
    bad.text_digest = bad.text_digest.to_uppercase();
    let error = BodyOrderLineage::build("doc-a", BodyOrderLane::RawSourceOrder, vec![bad])
        .expect_err("uppercase digest must be refused");
    assert_eq!(error, BodyOrderLineageError::InvalidDigest { id: id(0, 0) });
}

#[test]
fn orders_over_different_documents_cannot_be_related() {
    let left = sample(BodyOrderLane::RawSourceOrder);
    let right = BodyOrderLineage::build(
        "doc-b",
        BodyOrderLane::PositionedAdmittedOrder,
        left.entries.clone(),
    )
    .expect("second document order builds");
    let error = audit_body_order_lineage(&[left, right]).expect_err("distinct documents must fail");
    assert_eq!(
        error,
        BodyOrderLineageError::DocumentMismatch {
            reference_lane: BodyOrderLane::RawSourceOrder,
            lane: BodyOrderLane::PositionedAdmittedOrder,
        }
    );
}

#[test]
fn orders_with_different_token_counts_cannot_be_related() {
    let left = sample(BodyOrderLane::RawSourceOrder);
    let right = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::PositionedAdmittedOrder,
        vec![entry("applied", 0, 0, 0), entry("learner", 0, 1, 1)],
    )
    .expect("shorter order builds");
    let error = audit_body_order_lineage(&[left, right]).expect_err("count mismatch must fail");
    assert_eq!(
        error,
        BodyOrderLineageError::TokenCountMismatch {
            reference_lane: BodyOrderLane::RawSourceOrder,
            reference: 3,
            lane: BodyOrderLane::PositionedAdmittedOrder,
            tokens: 2,
        }
    );
}

#[test]
fn orders_with_different_identity_sets_cannot_be_related() {
    let left = sample(BodyOrderLane::RawSourceOrder);
    let right = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::PositionedAdmittedOrder,
        vec![
            entry("applied", 0, 0, 0),
            entry("learner", 0, 1, 1),
            entry("model", 0, 3, 2),
        ],
    )
    .expect("mutated identity set builds");
    let error = audit_body_order_lineage(&[left, right]).expect_err("identity drift must fail");
    assert_eq!(
        error,
        BodyOrderLineageError::TokenSetMismatch {
            reference_lane: BodyOrderLane::RawSourceOrder,
            lane: BodyOrderLane::PositionedAdmittedOrder,
            absent: 1,
            unexpected: 1,
        }
    );
}

#[test]
fn orders_disagreeing_on_one_lexeme_digest_cannot_be_related() {
    let left = sample(BodyOrderLane::RawSourceOrder);
    let mut entries = left.entries.clone();
    entries[1].text_digest = hex("different");
    let right = BodyOrderLineage::build("doc-a", BodyOrderLane::PositionedAdmittedOrder, entries)
        .expect("digest-disagreeing order builds");
    let error = audit_body_order_lineage(&[left, right]).expect_err("lexeme mismatch must fail");
    assert_eq!(
        error,
        BodyOrderLineageError::LexemeDigestMismatch {
            id: id(0, 1),
            reference_lane: BodyOrderLane::RawSourceOrder,
            lane: BodyOrderLane::PositionedAdmittedOrder,
        }
    );
}

#[test]
fn a_single_order_cannot_be_related_to_anything() {
    let error = audit_body_order_lineage(&[sample(BodyOrderLane::RawSourceOrder)])
        .expect_err("one order is not a comparison");
    assert_eq!(error, BodyOrderLineageError::TooFewOrders { orders: 1 });
}

#[test]
fn the_same_lane_twice_is_refused() {
    let error = audit_body_order_lineage(&[
        sample(BodyOrderLane::RawSourceOrder),
        sample(BodyOrderLane::RawSourceOrder),
    ])
    .expect_err("repeated lane must fail");
    assert_eq!(
        error,
        BodyOrderLineageError::RepeatedLane {
            lane: BodyOrderLane::RawSourceOrder
        }
    );
}

#[test]
fn a_reordered_pair_is_reported_as_differing_never_as_identity() {
    let forward = sample(BodyOrderLane::RawSourceOrder);
    let mut entries = forward.entries.clone();
    entries.reverse();
    for (position, entry) in entries.iter_mut().enumerate() {
        entry.token_ordinal = position as u32;
    }
    let reversed =
        BodyOrderLineage::build("doc-a", BodyOrderLane::PositionedAdmittedOrder, entries)
            .expect("reversed order builds");

    assert_ne!(forward.order_digest, reversed.order_digest);
    let audit = audit_body_order_lineage(&[forward, reversed]).expect("same token set audits");
    assert_eq!(audit.token_count, 3);
    assert_eq!(audit.lanes.len(), 2);
    assert_eq!(
        audit.lanes[0].identity_digest,
        audit.lanes[1].identity_digest
    );
    assert_eq!(audit.pairs.len(), 1);
    // A three-token reversal fixes the middle position, so exactly two
    // positions differ. A reorder is still never summarised as identity.
    assert_eq!(audit.pairs[0].positions_differing, 2);
    assert!(!audit.pairs[0].identical_order);
}

#[test]
fn an_identical_pair_is_reported_as_identical_order() {
    let left = sample(BodyOrderLane::RawSourceOrder);
    let right = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::PositionedAdmittedOrder,
        left.entries.clone(),
    )
    .expect("identical order builds");
    let audit = audit_body_order_lineage(&[left, right]).expect("identical pair audits");
    assert_eq!(audit.pairs[0].positions_differing, 0);
    assert!(audit.pairs[0].identical_order);
}

#[test]
fn the_audit_depends_only_on_the_native_orders() {
    // A permuted oracle-like sequence is not an input to the audit at all, so the
    // same native orders audit identically before and after any oracle shuffle.
    let left = sample(BodyOrderLane::RawSourceOrder);
    let right = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::PositionedAdmittedOrder,
        left.entries.clone(),
    )
    .expect("second order builds");
    let before = audit_body_order_lineage(&[left.clone(), right.clone()]).expect("audit");
    let mut oracle_like = vec!["model", "applied", "learner"];
    oracle_like.reverse();
    assert_eq!(
        before,
        audit_body_order_lineage(&[left, right]).expect("audit is repeatable")
    );
    assert_eq!(oracle_like, vec!["learner", "applied", "model"]);
}

#[test]
fn ordered_fragments_refuse_a_missing_lexeme() {
    let lineage = sample(BodyOrderLane::RawSourceOrder);
    let texts = ["applied", "learner"];
    let error = ordered_fragments(&lineage, |token| {
        texts.get(token.source_ordinal as usize).copied()
    })
    .expect_err("missing lexeme must be refused");
    assert_eq!(error, BodyOrderLineageError::MissingLexeme { id: id(0, 2) });
}

#[test]
fn ordered_fragments_refuse_a_lexeme_digest_mismatch() {
    let lineage = sample(BodyOrderLane::RawSourceOrder);
    let texts = ["applied", "learned", "model"];
    let error = ordered_fragments(&lineage, |token| {
        texts.get(token.source_ordinal as usize).copied()
    })
    .expect_err("digest mismatch must be refused");
    assert_eq!(
        error,
        BodyOrderLineageError::LexemeDigestMismatch {
            id: id(0, 1),
            reference_lane: BodyOrderLane::RawSourceOrder,
            lane: BodyOrderLane::RawSourceOrder,
        }
    );
}

#[test]
fn ordered_fragments_preserve_the_order_map_order() {
    let mut entries = sample(BodyOrderLane::RawSourceOrder).entries;
    entries.swap(0, 2);
    let lineage =
        BodyOrderLineage::build("doc-a", BodyOrderLane::RawSourceOrder, entries).expect("builds");
    let texts = ["applied", "learner", "model"];
    let fragments = ordered_fragments(&lineage, |token| {
        texts.get(token.source_ordinal as usize).copied()
    })
    .expect("fragments resolve");
    assert_eq!(fragments, vec!["model", "learner", "applied"]);
}

#[test]
fn an_order_map_is_deterministic() {
    let first = sample(BodyOrderLane::RawSourceOrder);
    let second = sample(BodyOrderLane::RawSourceOrder);
    assert_eq!(first, second);
    assert_eq!(first.order_digest, second.order_digest);
}

#[test]
fn the_serialized_lineage_carries_digests_and_no_text() {
    let secret = "confidential token text";
    let lineage = BodyOrderLineage::build(
        "doc-a",
        BodyOrderLane::RawSourceOrder,
        vec![entry(secret, 0, 0, 0)],
    )
    .expect("builds");
    let json = serde_json::to_string(&lineage).expect("serializes");
    assert!(json.contains("text_digest"));
    assert!(json.contains(&hex(secret)));
    assert!(
        !json.contains(secret),
        "the serialized order map must not carry the token text"
    );
}

#[test]
fn the_entry_bound_is_enforced() {
    let entries = (0..=da_parse::MAX_ORDER_LINEAGE_ENTRIES as u32)
        .map(|ordinal| entry("word", 0, ordinal, ordinal))
        .collect::<Vec<_>>();
    let error = BodyOrderLineage::build("doc-a", BodyOrderLane::RawSourceOrder, entries)
        .expect_err("oversize order must be refused");
    assert_eq!(
        error,
        BodyOrderLineageError::TooManyEntries {
            count: da_parse::MAX_ORDER_LINEAGE_ENTRIES + 1
        }
    );
}
