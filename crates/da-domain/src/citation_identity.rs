//! Revision-aware scholarly citation identities and partial reading order.
//!
//! A bibliography entry, an in-text occurrence, and the cited scholarly work are
//! different identities.  Resolution is deliberately typed: an unresolved or
//! ambiguous target is evidence about the observation, not a guessed work link.
//! See ADR-058 and the citation/segmentation data contracts.

use serde::{Deserialize, Serialize};

use crate::vid::{Vid, sha256_vid, sha256_vid2};

pub const CITATION_IDENTITY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CitationTarget {
    ResolvedWork {
        work_vid: Vid,
    },
    Unresolved {
        reason: UnresolvedCitationReason,
    },
    Ambiguous {
        candidates: Vec<Vid>,
        reason: AmbiguousCitationReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedCitationReason {
    MissingTarget,
    TargetNotInInventory,
    MalformedTarget,
    RevisionInvalidated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbiguousCitationReason {
    MultipleWorkCandidates,
    ConflictingRevisionEvidence,
}

impl CitationTarget {
    pub fn resolved(work_vid: impl Into<String>) -> Self {
        Self::ResolvedWork {
            work_vid: work_vid.into(),
        }
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::ResolvedWork { .. })
    }

    pub fn work_vid(&self) -> Option<&str> {
        match self {
            Self::ResolvedWork { work_vid } => Some(work_vid),
            Self::Unresolved { .. } | Self::Ambiguous { .. } => None,
        }
    }
}

/// A bibliography/list-of-references record in one source revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationEntry {
    pub schema_version: u16,
    pub vid: Vid,
    pub source_revision_vid: Vid,
    pub ordinal: u32,
    pub target: CitationTarget,
}

impl CitationEntry {
    pub fn new(source_revision_vid: impl AsRef<str>, ordinal: u32, target: CitationTarget) -> Self {
        let revision = source_revision_vid.as_ref();
        Self {
            schema_version: CITATION_IDENTITY_SCHEMA_VERSION,
            vid: citation_entry_vid(revision, ordinal),
            source_revision_vid: revision.to_owned(),
            ordinal,
            target,
        }
    }
}

/// One in-text callout/anchor, which may point to an entry but is not the entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationOccurrence {
    pub schema_version: u16,
    pub vid: Vid,
    pub source_revision_vid: Vid,
    pub anchor_id: String,
    pub entry_vid: Option<Vid>,
    pub target: CitationTarget,
}

impl CitationOccurrence {
    pub fn new(
        source_revision_vid: impl AsRef<str>,
        anchor_id: impl Into<String>,
        entry_vid: Option<Vid>,
        target: CitationTarget,
    ) -> Self {
        let revision = source_revision_vid.as_ref();
        let anchor_id = anchor_id.into();
        Self {
            schema_version: CITATION_IDENTITY_SCHEMA_VERSION,
            vid: citation_occurrence_vid(revision, &anchor_id),
            source_revision_vid: revision.to_owned(),
            anchor_id,
            entry_vid,
            target,
        }
    }
}

/// Work-level target link.  A work is stable across expressions/revisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationWorkLink {
    pub source_revision_vid: Vid,
    pub work: CitationTarget,
}

/// A partial-order relation between reading anchors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadingOrderConstraint {
    Definite {
        predecessor: String,
        successor: String,
    },
    Alternative {
        predecessors: Vec<String>,
        successor: String,
    },
    Unresolved {
        anchor: String,
        reason: UnresolvedOrderReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedOrderReason {
    MissingAnchor,
    ConflictingEvidence,
    InsufficientCoordinates,
    RevisionInvalidated,
}

/// Constraints remain valid only for the revision that produced their anchors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingOrder {
    pub schema_version: u16,
    pub source_revision_vid: Vid,
    pub constraints: Vec<ReadingOrderConstraint>,
    pub invalidated_by_revision: Option<Vid>,
}

impl ReadingOrder {
    pub fn new(
        source_revision_vid: impl Into<String>,
        constraints: Vec<ReadingOrderConstraint>,
    ) -> Self {
        Self {
            schema_version: CITATION_IDENTITY_SCHEMA_VERSION,
            source_revision_vid: source_revision_vid.into(),
            constraints,
            invalidated_by_revision: None,
        }
    }

    pub fn invalidate_for_revision(&mut self, revision_vid: impl Into<String>) {
        self.invalidated_by_revision = Some(revision_vid.into());
    }

    pub fn is_valid_for(&self, revision_vid: &str) -> bool {
        self.invalidated_by_revision.is_none() && self.source_revision_vid == revision_vid
    }
}

pub fn citation_entry_vid(source_revision_vid: &str, ordinal: u32) -> Vid {
    sha256_vid2(
        "citation_entry",
        source_revision_vid.trim(),
        &ordinal.to_string(),
    )
}

pub fn citation_occurrence_vid(source_revision_vid: &str, anchor_id: &str) -> Vid {
    sha256_vid(
        "citation_occurrence",
        &format!("{}\n{}", source_revision_vid.trim(), anchor_id.trim()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roundtrip_json;

    #[test]
    fn citation_identity_separates_entry_occurrence_and_work() {
        let revision = "r1";
        let work = CitationTarget::resolved("work-1");
        let entry = CitationEntry::new(revision, 1, work.clone());
        let occurrence =
            CitationOccurrence::new(revision, "anchor-1", Some(entry.vid.clone()), work);
        assert_ne!(entry.vid, occurrence.vid);
        assert_eq!(occurrence.entry_vid.as_deref(), Some(entry.vid.as_str()));
        assert_eq!(occurrence.target.work_vid(), Some("work-1"));
    }

    #[test]
    fn unresolved_and_ambiguous_are_not_work_links() {
        let unresolved = CitationTarget::Unresolved {
            reason: UnresolvedCitationReason::MissingTarget,
        };
        let ambiguous = CitationTarget::Ambiguous {
            candidates: vec!["w1".into(), "w2".into()],
            reason: AmbiguousCitationReason::MultipleWorkCandidates,
        };
        assert!(!unresolved.is_resolved());
        assert!(!ambiguous.is_resolved());
        assert!(unresolved.work_vid().is_none());
        roundtrip_json(&unresolved);
        roundtrip_json(&ambiguous);
    }

    #[test]
    fn reading_order_is_partial_and_revision_scoped() {
        let mut order = ReadingOrder::new(
            "r1",
            vec![
                ReadingOrderConstraint::Definite {
                    predecessor: "a".into(),
                    successor: "b".into(),
                },
                ReadingOrderConstraint::Alternative {
                    predecessors: vec!["b".into(), "c".into()],
                    successor: "d".into(),
                },
                ReadingOrderConstraint::Unresolved {
                    anchor: "e".into(),
                    reason: UnresolvedOrderReason::MissingAnchor,
                },
            ],
        );
        assert!(order.is_valid_for("r1"));
        assert!(!order.is_valid_for("r2"));
        order.invalidate_for_revision("r2");
        assert!(!order.is_valid_for("r1"));
        roundtrip_json(&order);
    }
}
