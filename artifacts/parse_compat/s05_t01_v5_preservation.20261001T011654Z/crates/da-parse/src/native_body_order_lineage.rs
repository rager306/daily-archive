//! Additive, read-only order-lineage diagnostic for the S05 native body plane.
//!
//! The S05/T01 character comparison measures one native token stream against an
//! independent preserved-oracle token stream. That measurement is only
//! interpretable when the *order* of the native stream is stated explicitly,
//! because one multiset of native tokens can be delivered in very different
//! orders:
//!
//! * the positioned donor delivers lexical spans in extraction order
//!   (`source_ordinal`);
//! * the same document also carries an admitted token order (`token_ordinal`)
//!   that a reading-order reorder can change while `source_ordinal` is preserved
//!   as the stable join identity.
//!
//! This module makes that order a first-class, auditable value. It holds no text
//! (only token identities and digests), it never compares content, and it
//! **cannot** derive an order from the comparison oracle: the lane vocabulary is
//! closed over native order sources only.
//!
//! Three facts are enforced, fail-closed:
//!
//! 1. **Explicit document token identity.** Every entry names a
//!    `(page_ordinal, source_ordinal)` identity and the `token_ordinal` it sits
//!    at, so an order map is a list of concrete document tokens.
//! 2. **No silent drop or duplication.** An order refuses a duplicate identity
//!    or a duplicate `token_ordinal` within a page.
//! 3. **No lexeme mismatch across orders.** When two or more orders are audited
//!    they must carry exactly the same identities and the same digest for every
//!    identity; a differing digest is a typed refusal, never a reordering. The
//!    same rule is re-checked when fragments are resolved for measurement.
//!
//! The module is pure: no clock, no IO, no environment, no logging, and no
//! dependence on any runtime route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema identity of the additive order-lineage surface.
pub const NATIVE_BODY_ORDER_LINEAGE_SCHEMA_VERSION: &str = "da-parse.native-body-order-lineage/v1";

/// Upper bound on entries in one order map. Mirrors the native word ceiling so a
/// lineage can never admit more tokens than its document already bounds.
pub const MAX_ORDER_LINEAGE_ENTRIES: usize = 250_000;

/// The typed label the slice uses for a lane the segmenter never measured. The
/// XY-cut order is retained under this label and is never promoted.
pub const SEGMENTER_REJECTED_LATTICE: &str = "segmenter_rejected_lattice";

/// Closed vocabulary of native order sources.
///
/// There is deliberately no oracle or reader variant: an order may only be
/// derived from the native delivery order or from a native admitted token order,
/// never from the independent comparison oracle. Deserializing any other token
/// is a typed refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOrderLane {
    /// The order the positioned donor delivered lexical spans in: ascending
    /// `(page_ordinal, source_ordinal)`.
    RawSourceOrder,
    /// The admitted token order of the positioned lane: ascending
    /// `(page_ordinal, token_ordinal)` of the document as delivered.
    PositionedAdmittedOrder,
    /// The admitted token order after a reading-order reorder that reassigns
    /// `token_ordinal` while preserving `source_ordinal`.
    XyCutAdmittedOrder,
}

impl BodyOrderLane {
    /// Every lane, in declaration order.
    pub const ALL: [BodyOrderLane; 3] = [
        BodyOrderLane::RawSourceOrder,
        BodyOrderLane::PositionedAdmittedOrder,
        BodyOrderLane::XyCutAdmittedOrder,
    ];

    /// How the slice disposes of this lane. The rejected lattice keeps its typed
    /// label; no lane is promoted by asking this question.
    pub const fn disposition(self) -> BodyOrderLaneDisposition {
        match self {
            BodyOrderLane::RawSourceOrder => BodyOrderLaneDisposition::PrimaryLaneDeliveryOrder,
            BodyOrderLane::PositionedAdmittedOrder => {
                BodyOrderLaneDisposition::PrimaryLaneAdmittedOrder
            }
            BodyOrderLane::XyCutAdmittedOrder => BodyOrderLaneDisposition::SegmenterRejectedLattice,
        }
    }
}

/// Typed disposition of a lane inside the slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOrderLaneDisposition {
    /// The primary lane's delivered lexical order.
    PrimaryLaneDeliveryOrder,
    /// The primary lane's admitted token order.
    PrimaryLaneAdmittedOrder,
    /// A lattice the segmenter never measured; retained as a typed rejection.
    SegmenterRejectedLattice,
}

impl BodyOrderLaneDisposition {
    /// The wire token. Kept explicit so the retained rejection label is a
    /// literal, not a serde-internal rendering.
    pub const fn wire(self) -> &'static str {
        match self {
            BodyOrderLaneDisposition::PrimaryLaneDeliveryOrder => "primary_lane_delivery_order",
            BodyOrderLaneDisposition::PrimaryLaneAdmittedOrder => "primary_lane_admitted_order",
            BodyOrderLaneDisposition::SegmenterRejectedLattice => SEGMENTER_REJECTED_LATTICE,
        }
    }
}

/// One document token's stable identity: the page ordinal plus the extraction
/// ordinal the donor assigned. This identity survives any reading-order reorder,
/// so it is the join key across orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentTokenId {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
}

/// One position in an order map: an explicit document token identity, the
/// `token_ordinal` it occupies there, and the token's text digest.
///
/// The digest is what makes "no lexeme mismatch" checkable: two orders that
/// claim the same identity must agree on the digest, or the audited pair is
/// refused rather than silently reordered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrderEntry {
    pub id: DocumentTokenId,
    pub token_ordinal: u32,
    pub text_digest: String,
}

/// One explicit order map over a document's tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrderLineage {
    pub schema_version: String,
    pub document_id: String,
    pub lane: BodyOrderLane,
    pub disposition: BodyOrderLaneDisposition,
    pub entries: Vec<BodyOrderEntry>,
    /// Digest over the ordered entries. Two permutations of the same tokens have
    /// different digests; a reordering can therefore never look like identity.
    pub order_digest: String,
}

impl BodyOrderLineage {
    /// Build and validate one order map.
    ///
    /// Refuses an empty order, more entries than the surface bounds, a
    /// duplicate document identity, a duplicate `token_ordinal` within a page,
    /// or a malformed digest. The lane's typed disposition is attached here so
    /// a caller cannot silently relabel the rejected lattice.
    pub fn build(
        document_id: impl Into<String>,
        lane: BodyOrderLane,
        entries: Vec<BodyOrderEntry>,
    ) -> Result<Self, BodyOrderLineageError> {
        if entries.is_empty() {
            return Err(BodyOrderLineageError::EmptyOrder { lane });
        }
        if entries.len() > MAX_ORDER_LINEAGE_ENTRIES {
            return Err(BodyOrderLineageError::TooManyEntries {
                count: entries.len(),
            });
        }
        let mut identities = BTreeSet::new();
        let mut ordinals: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for entry in &entries {
            if !is_hex_sha256(&entry.text_digest) {
                return Err(BodyOrderLineageError::InvalidDigest { id: entry.id });
            }
            if !identities.insert(entry.id) {
                return Err(BodyOrderLineageError::DuplicateTokenId { id: entry.id });
            }
            let page_ordinals = ordinals.entry(entry.id.page_ordinal).or_default();
            if !page_ordinals.insert(entry.token_ordinal) {
                return Err(BodyOrderLineageError::DuplicateTokenOrdinal {
                    page_ordinal: entry.id.page_ordinal,
                    token_ordinal: entry.token_ordinal,
                });
            }
        }
        let order_digest = order_digest(&entries);
        Ok(Self {
            schema_version: NATIVE_BODY_ORDER_LINEAGE_SCHEMA_VERSION.to_owned(),
            document_id: document_id.into(),
            lane,
            disposition: lane.disposition(),
            entries,
            order_digest,
        })
    }

    /// The order map's length in document tokens.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the order map holds no document token.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The identities in the order they occupy.
    pub fn ids(&self) -> impl Iterator<Item = DocumentTokenId> + '_ {
        self.entries.iter().map(|entry| entry.id)
    }
}

/// One lane's audited identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrderLaneAudit {
    pub lane: BodyOrderLane,
    pub disposition: BodyOrderLaneDisposition,
    pub token_count: usize,
    /// Order-sensitive digest of this lane.
    pub order_digest: String,
    /// Order-independent digest of this lane's `(identity, digest)` set, so two
    /// permutations of the same tokens share this value while their
    /// `order_digest` differs.
    pub identity_digest: String,
}

/// How two lanes' orders relate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrderPairAudit {
    pub left_lane: BodyOrderLane,
    pub right_lane: BodyOrderLane,
    /// Positions at which the two order maps name different tokens.
    pub positions_differing: usize,
    /// True only when every position names the same token in both lanes.
    pub identical_order: bool,
}

/// The verified relation between several order maps over one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrderLineageAudit {
    pub document_id: String,
    pub token_count: usize,
    pub lanes: Vec<BodyOrderLaneAudit>,
    pub pairs: Vec<BodyOrderPairAudit>,
}

/// Verify that several order maps describe the same document tokens in
/// (possibly) different orders.
///
/// Fail-closed: fewer than two orders, a repeated lane, a different document,
/// a different token count, a different identity set, or a differing digest for
/// any shared identity is a typed refusal. Only after all of that holds are the
/// pairwise order relations reported.
pub fn audit_body_order_lineage(
    orders: &[BodyOrderLineage],
) -> Result<BodyOrderLineageAudit, BodyOrderLineageError> {
    if orders.len() < 2 {
        return Err(BodyOrderLineageError::TooFewOrders {
            orders: orders.len(),
        });
    }
    let mut lanes = BTreeSet::new();
    for order in orders {
        if !lanes.insert(order.lane) {
            return Err(BodyOrderLineageError::RepeatedLane { lane: order.lane });
        }
    }
    let reference = &orders[0];
    let reference_ids: BTreeSet<DocumentTokenId> = reference.ids().collect();
    let mut reference_digests: BTreeMap<DocumentTokenId, &str> = BTreeMap::new();
    for entry in &reference.entries {
        reference_digests.insert(entry.id, entry.text_digest.as_str());
    }

    for order in &orders[1..] {
        if order.document_id != reference.document_id {
            return Err(BodyOrderLineageError::DocumentMismatch {
                reference_lane: reference.lane,
                lane: order.lane,
            });
        }
        if order.entries.len() != reference.entries.len() {
            return Err(BodyOrderLineageError::TokenCountMismatch {
                reference_lane: reference.lane,
                reference: reference.entries.len(),
                lane: order.lane,
                tokens: order.entries.len(),
            });
        }
        let ids: BTreeSet<DocumentTokenId> = order.ids().collect();
        if ids != reference_ids {
            let absent = reference_ids.difference(&ids).count();
            let unexpected = ids.difference(&reference_ids).count();
            return Err(BodyOrderLineageError::TokenSetMismatch {
                reference_lane: reference.lane,
                lane: order.lane,
                absent,
                unexpected,
            });
        }
        for entry in &order.entries {
            match reference_digests.get(&entry.id) {
                Some(digest) if *digest == entry.text_digest.as_str() => {}
                _ => {
                    return Err(BodyOrderLineageError::LexemeDigestMismatch {
                        id: entry.id,
                        reference_lane: reference.lane,
                        lane: order.lane,
                    });
                }
            }
        }
    }

    let lane_audits = orders
        .iter()
        .map(|order| BodyOrderLaneAudit {
            lane: order.lane,
            disposition: order.disposition,
            token_count: order.entries.len(),
            order_digest: order.order_digest.clone(),
            identity_digest: identity_digest(&order.entries),
        })
        .collect::<Vec<_>>();

    let mut pairs = Vec::new();
    for (left_index, left) in orders.iter().enumerate() {
        for right in orders.iter().skip(left_index + 1) {
            let positions_differing = left
                .entries
                .iter()
                .zip(right.entries.iter())
                .filter(|(left_entry, right_entry)| left_entry.id != right_entry.id)
                .count();
            pairs.push(BodyOrderPairAudit {
                left_lane: left.lane,
                right_lane: right.lane,
                positions_differing,
                identical_order: positions_differing == 0,
            });
        }
    }

    Ok(BodyOrderLineageAudit {
        document_id: reference.document_id.clone(),
        token_count: reference.entries.len(),
        lanes: lane_audits,
        pairs,
    })
}

/// Resolve one order map into measurement fragments, re-checking identity.
///
/// Every entry must resolve to text and that text's digest must equal the
/// entry's declared digest; a missing identity is
/// [`BodyOrderLineageError::MissingLexeme`] and a digest disagreement is
/// [`BodyOrderLineageError::LexemeDigestMismatch`]. The order of the returned
/// fragments is exactly the order map's order, so a caller cannot reorder the
/// stream while believing it measured the map.
pub fn ordered_fragments<'a>(
    lineage: &BodyOrderLineage,
    mut text_of: impl FnMut(DocumentTokenId) -> Option<&'a str>,
) -> Result<Vec<&'a str>, BodyOrderLineageError> {
    let mut fragments = Vec::with_capacity(lineage.entries.len());
    for entry in &lineage.entries {
        let text =
            text_of(entry.id).ok_or(BodyOrderLineageError::MissingLexeme { id: entry.id })?;
        if digest_text(text) != entry.text_digest {
            return Err(BodyOrderLineageError::LexemeDigestMismatch {
                id: entry.id,
                reference_lane: lineage.lane,
                lane: lineage.lane,
            });
        }
        fragments.push(text);
    }
    Ok(fragments)
}

/// Fail-closed refusal vocabulary of the order-lineage surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyOrderLineageError {
    /// An order map with no document token is a typed absence, never a pass.
    EmptyOrder { lane: BodyOrderLane },
    /// More entries than the surface bounds.
    TooManyEntries { count: usize },
    /// The same document identity appears twice in one order map.
    DuplicateTokenId { id: DocumentTokenId },
    /// The same `token_ordinal` appears twice on one page of one order map.
    DuplicateTokenOrdinal {
        page_ordinal: u32,
        token_ordinal: u32,
    },
    /// A text digest is not a lowercase 64-character hex sha256.
    InvalidDigest { id: DocumentTokenId },
    /// Fewer than two orders cannot be related.
    TooFewOrders { orders: usize },
    /// The same lane was supplied twice.
    RepeatedLane { lane: BodyOrderLane },
    /// Two orders describe different documents.
    DocumentMismatch {
        reference_lane: BodyOrderLane,
        lane: BodyOrderLane,
    },
    /// Two orders hold a different number of tokens.
    TokenCountMismatch {
        reference_lane: BodyOrderLane,
        reference: usize,
        lane: BodyOrderLane,
        tokens: usize,
    },
    /// Two orders do not carry the same set of document identities.
    TokenSetMismatch {
        reference_lane: BodyOrderLane,
        lane: BodyOrderLane,
        absent: usize,
        unexpected: usize,
    },
    /// Two orders disagree about one identity's text digest.
    LexemeDigestMismatch {
        id: DocumentTokenId,
        reference_lane: BodyOrderLane,
        lane: BodyOrderLane,
    },
    /// An order map names an identity that has no text.
    MissingLexeme { id: DocumentTokenId },
}

impl fmt::Display for BodyOrderLineageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native body order lineage rejected input: {self:?}"
        )
    }
}

impl std::error::Error for BodyOrderLineageError {}

/// Digest over the ordered entries: the order-sensitive identity.
fn order_digest(entries: &[BodyOrderEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.id.page_ordinal.to_le_bytes());
        hasher.update(entry.id.source_ordinal.to_le_bytes());
        hasher.update(entry.token_ordinal.to_le_bytes());
        hasher.update(entry.text_digest.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// Digest over the `(identity, digest)` set in identity order: the
/// order-independent identity. Two permutations share this value.
fn identity_digest(entries: &[BodyOrderEntry]) -> String {
    let mut sorted: Vec<&BodyOrderEntry> = entries.iter().collect();
    sorted.sort_by_key(|entry| entry.id);
    let mut hasher = Sha256::new();
    for entry in sorted {
        hasher.update(entry.id.page_ordinal.to_le_bytes());
        hasher.update(entry.id.source_ordinal.to_le_bytes());
        hasher.update(entry.text_digest.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// Sha256 of one token's text, matching the donor's own digest rule.
fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// True when `digest` is a lowercase 64-character hex sha256.
fn is_hex_sha256(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
