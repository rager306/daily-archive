//! Accountant-bound, digest-only callout matching for the shadow plane.
//!
//! This module is intentionally downstream of `NativeBodyAccountant` and
//! `NativeFulltextBlocks`. It accepts only their identity-bearing receipts and
//! an injected digest-only inventory; it never reads source payloads or chooses
//! a production parser route.

use crate::{
    FulltextLeafRole, MarkerSubtype, NativeFullRegionAccounting, NativeFulltextBlocksOutcome,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const NATIVE_CALLOUT_MATCHING_SCHEMA_VERSION: &str = "da-parse.native-callout-matching/v1";
pub const MAX_CALLOUT_OBSERVATIONS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutTargetClass {
    Citation,
    Figure,
    Table,
    Equation,
}
impl From<MarkerSubtype> for CalloutTargetClass {
    fn from(value: MarkerSubtype) -> Self {
        match value {
            MarkerSubtype::Citation => Self::Citation,
            MarkerSubtype::Figure => Self::Figure,
            MarkerSubtype::Table => Self::Table,
            MarkerSubtype::Equation => Self::Equation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedCalloutStyle {
    Numeric,
    AuthorYear,
    Roman,
    Label,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjacencyDisposition {
    Adjacent,
    NonAdjacent,
    IllegalBoundaryCrossing,
    SourceNotEstablished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MajorityStyleOutcome {
    Selected,
    Tie,
    Empty,
    SourceNotEstablished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutUnresolvedReason {
    NoTarget,
    TargetNotInInventory,
    Ambiguous,
    IllegalAdjacency,
    UnsupportedEquation,
    SourceNotEstablished,
}

/// Document order is the only ordering accepted by the matcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CalloutPosition {
    pub page_ordinal: u32,
    pub block_ordinal: u32,
    pub line_ordinal: u32,
    pub token_ordinal: u32,
    pub annex_parent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestOnlyCalloutTarget {
    pub class: CalloutTargetClass,
    pub target_digest: String,
    pub position: CalloutPosition,
    pub style: ObservedCalloutStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestOnlyTargetInventory {
    pub document_id: String,
    pub lane_id: String,
    pub source_sha256: String,
    pub inventory_digest: String,
    pub targets: Vec<DigestOnlyCalloutTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajorityStyleDisposition {
    pub class: CalloutTargetClass,
    pub outcome: MajorityStyleOutcome,
    pub style: Option<ObservedCalloutStyle>,
    pub candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutMatch {
    pub class: CalloutTargetClass,
    pub marker_digest: String,
    pub target_digest: String,
    pub marker: CalloutPosition,
    pub target: CalloutPosition,
    pub style: MajorityStyleDisposition,
    pub adjacency: AdjacencyDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedCallout {
    pub class: CalloutTargetClass,
    pub marker_digest: String,
    pub marker: CalloutPosition,
    pub reason: CalloutUnresolvedReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCalloutMatchingOutcome {
    pub schema_version: String,
    pub document_id: String,
    pub source_sha256: String,
    pub lattice_fingerprint: String,
    pub inventory_digest: String,
    pub matched: Vec<CalloutMatch>,
    pub unresolved: Vec<UnresolvedCallout>,
    pub majority: Vec<MajorityStyleDisposition>,
    pub admitted: usize,
    pub conserved: usize,
    pub loss: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutUpstreamStatus {
    Pass,
    Empty,
    Unsupported,
    SourceNotEstablished,
    InvalidEvidence,
    Missing,
    ChecksumMismatch,
    Malformed,
    Timeout,
    Nonzero,
    Oversize,
    Encrypted,
    Truncated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCalloutMatchingError {
    UpstreamTerminal(NativeCalloutUpstreamStatus),
    IdentityMismatch(&'static str),
    InvalidInventory(&'static str),
    DuplicateTarget,
    Oversize { count: usize, max: usize },
    NonMonotonic,
}
impl fmt::Display for NativeCalloutMatchingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native callout matching rejected input: {self:?}")
    }
}
impl std::error::Error for NativeCalloutMatchingError {}

fn digest_inventory(inventory: &DigestOnlyTargetInventory) -> String {
    let mut h = Sha256::new();
    for target in &inventory.targets {
        h.update(format!(
            "{:?}|{}|{:?}|{:?};",
            target.class, target.target_digest, target.position, target.style
        ));
    }
    format!("sha256:{:x}", h.finalize())
}
fn majority(
    class: CalloutTargetClass,
    targets: &[&DigestOnlyCalloutTarget],
) -> MajorityStyleDisposition {
    let mut counts = BTreeMap::new();
    for target in targets {
        *counts.entry(target.style).or_insert(0usize) += 1;
    }
    if counts.is_empty() {
        return MajorityStyleDisposition {
            class,
            outcome: MajorityStyleOutcome::Empty,
            style: None,
            candidate_count: 0,
        };
    }
    let max = *counts.values().max().unwrap();
    let winners: Vec<_> = counts
        .iter()
        .filter(|(_, count)| **count == max)
        .map(|(style, _)| *style)
        .collect();
    MajorityStyleDisposition {
        class,
        outcome: if winners.len() == 1 {
            MajorityStyleOutcome::Selected
        } else {
            MajorityStyleOutcome::Tie
        },
        style: (winners.len() == 1).then_some(winners[0]),
        candidate_count: targets.len(),
    }
}
fn adjacency(marker: CalloutPosition, target: CalloutPosition) -> AdjacencyDisposition {
    if marker.annex_parent != target.annex_parent {
        return AdjacencyDisposition::IllegalBoundaryCrossing;
    }
    if marker.page_ordinal == target.page_ordinal
        && marker.block_ordinal == target.block_ordinal
        && marker.line_ordinal == target.line_ordinal
        && marker.token_ordinal.abs_diff(target.token_ordinal) == 1
    {
        AdjacencyDisposition::Adjacent
    } else {
        AdjacencyDisposition::NonAdjacent
    }
}

/// Match already admitted marker leaves against an exact, digest-only target inventory.
pub fn match_native_callouts(
    accounting: &NativeFullRegionAccounting,
    blocks: &NativeFulltextBlocksOutcome,
    upstream: NativeCalloutUpstreamStatus,
    inventory: &DigestOnlyTargetInventory,
) -> Result<NativeCalloutMatchingOutcome, NativeCalloutMatchingError> {
    if upstream != NativeCalloutUpstreamStatus::Pass {
        return Err(NativeCalloutMatchingError::UpstreamTerminal(upstream));
    }
    if accounting.loss != 0 || accounting.admitted != accounting.conserved {
        return Err(NativeCalloutMatchingError::IdentityMismatch(
            "accounting_conservation",
        ));
    }
    if accounting.document_id != blocks.document_id {
        return Err(NativeCalloutMatchingError::IdentityMismatch("document_id"));
    }
    if accounting.lattice_fingerprint.is_empty()
        || blocks.lattice_fingerprint != accounting.lattice_fingerprint
    {
        return Err(NativeCalloutMatchingError::IdentityMismatch(
            "lattice_fingerprint",
        ));
    }
    if inventory.document_id != accounting.document_id {
        return Err(NativeCalloutMatchingError::IdentityMismatch(
            "inventory_document_id",
        ));
    }
    if inventory.targets.len() > MAX_CALLOUT_OBSERVATIONS {
        return Err(NativeCalloutMatchingError::Oversize {
            count: inventory.targets.len(),
            max: MAX_CALLOUT_OBSERVATIONS,
        });
    }
    if inventory.inventory_digest != digest_inventory(inventory) {
        return Err(NativeCalloutMatchingError::InvalidInventory(
            "inventory_digest",
        ));
    }
    let mut seen: Vec<(CalloutTargetClass, String, CalloutPosition)> = Vec::new();
    for target in &inventory.targets {
        let key = (target.class, target.target_digest.clone(), target.position);
        if target.target_digest.is_empty() || seen.contains(&key) {
            return Err(NativeCalloutMatchingError::DuplicateTarget);
        }
        seen.push(key);
    }
    let mut majority_rows = Vec::new();
    for class in [
        CalloutTargetClass::Citation,
        CalloutTargetClass::Figure,
        CalloutTargetClass::Table,
        CalloutTargetClass::Equation,
    ] {
        majority_rows.push(majority(
            class,
            &inventory
                .targets
                .iter()
                .filter(|t| t.class == class)
                .collect::<Vec<_>>(),
        ));
    }
    let mut markers = Vec::new();
    for leaf in &blocks.leaves {
        if leaf.role != FulltextLeafRole::Marker {
            continue;
        }
        let Some(subtype) = leaf.marker_subtype else {
            continue;
        };
        markers.push((CalloutTargetClass::from(subtype), leaf));
    }
    if markers.len() > MAX_CALLOUT_OBSERVATIONS {
        return Err(NativeCalloutMatchingError::Oversize {
            count: markers.len(),
            max: MAX_CALLOUT_OBSERVATIONS,
        });
    }
    let mut matched = Vec::new();
    let mut unresolved = Vec::new();
    let mut previous = None;
    for (class, marker) in markers {
        let position = CalloutPosition {
            page_ordinal: marker.page_ordinal,
            block_ordinal: marker.block_ordinal,
            line_ordinal: marker.line_ordinal,
            token_ordinal: marker.token_ordinal,
            annex_parent: marker.parent_annex,
        };
        if previous.is_some_and(|p| position < p) {
            return Err(NativeCalloutMatchingError::NonMonotonic);
        }
        previous = Some(position);
        if class == CalloutTargetClass::Equation {
            unresolved.push(UnresolvedCallout {
                class,
                marker_digest: marker.text_digest.clone(),
                marker: position,
                reason: CalloutUnresolvedReason::SourceNotEstablished,
            });
            continue;
        }
        let candidates: Vec<_> = inventory
            .targets
            .iter()
            .filter(|t| t.class == class && t.target_digest == marker.text_digest)
            .collect();
        let Some(target) = candidates.first() else {
            let any = inventory.targets.iter().any(|t| t.class == class);
            unresolved.push(UnresolvedCallout {
                class,
                marker_digest: marker.text_digest.clone(),
                marker: position,
                reason: if any {
                    CalloutUnresolvedReason::TargetNotInInventory
                } else {
                    CalloutUnresolvedReason::NoTarget
                },
            });
            continue;
        };
        let style = majority_rows
            .iter()
            .find(|m| m.class == class)
            .cloned()
            .unwrap();
        let adj = adjacency(position, target.position);
        if adj == AdjacencyDisposition::IllegalBoundaryCrossing {
            unresolved.push(UnresolvedCallout {
                class,
                marker_digest: marker.text_digest.clone(),
                marker: position,
                reason: CalloutUnresolvedReason::IllegalAdjacency,
            });
        } else if style.outcome == MajorityStyleOutcome::Tie {
            unresolved.push(UnresolvedCallout {
                class,
                marker_digest: marker.text_digest.clone(),
                marker: position,
                reason: CalloutUnresolvedReason::Ambiguous,
            });
        } else {
            matched.push(CalloutMatch {
                class,
                marker_digest: marker.text_digest.clone(),
                target_digest: target.target_digest.clone(),
                marker: position,
                target: target.position,
                style,
                adjacency: adj,
            });
        }
    }
    Ok(NativeCalloutMatchingOutcome {
        schema_version: NATIVE_CALLOUT_MATCHING_SCHEMA_VERSION.into(),
        document_id: blocks.document_id.clone(),
        source_sha256: accounting.source_pdf_sha256.clone(),
        lattice_fingerprint: blocks.lattice_fingerprint.clone(),
        inventory_digest: inventory.inventory_digest.clone(),
        admitted: matched.len() + unresolved.len(),
        conserved: matched.len() + unresolved.len(),
        loss: 0,
        matched,
        unresolved,
        majority: majority_rows,
    })
}

pub fn inventory_digest(inventory: &DigestOnlyTargetInventory) -> String {
    digest_inventory(inventory)
}
