//! Occurrence → bibliography entry link keys and published tie-break
//! precedence (M509-yzblqx / S05 / GAR-013).
//!
//! A citation involves three different identities: the in-text *occurrence*
//! ([`CitationAnchor`]), the bibliography *entry* ([`CitationEntry`]) and the
//! cited *work*. Resolution between them is deliberately typed: a callout that
//! cannot be linked is evidence about the observation, never a licence to guess
//! a neighbour. This module publishes the first link, occurrence → entry.
//!
//! ## Published key forms
//!
//! | Key form | Precedence | Where it comes from |
//! |---|---|---|
//! | [`CitationLinkKey::DeclaredEntryId`] | rank 0 — strongest | The occurrence's `ref_id` and the entry's own declared identifier are the same text (TEI `<ref type="bibr" target="#b12">` ↔ `<biblStruct xml:id="b12">`). |
//! | [`CitationLinkKey::SyntheticEntryOrdinal`] | rank 1 — fallback | Neither side declares an identifier: the occurrence points at the positional key [`positional_entry_key`] (`b{ordinal}`) and the entry publishes the position it was lifted at. |
//!
//! [`CITATION_LINK_PRECEDENCE`] is the published order (strongest first). A
//! lower rank always wins, so a link can never depend on construction or
//! iteration order: when a positional key collides with a declared identifier,
//! the declared identifier takes the link and the positional candidate is
//! reported as shadowed ([`CitationLinkOutcome::shadowed_entry_indexes`]).
//!
//! An entry publishes exactly one key. An entry with a declared identifier does
//! **not** also offer its position: a positional coincidence is not an
//! identity, and offering it would be a guess in disguise.
//!
//! ## No guessed identity
//!
//! [`resolve_occurrence_to_entry`] returns exactly one of three outcomes:
//!
//! * [`CitationLinkOutcome::Linked`] — one entry, reached by the strongest
//!   matching key, with any weaker matching candidates listed as shadowed;
//! * [`CitationLinkOutcome::Ambiguous`] — *every* competing entry index, never
//!   one of them, so the caller cannot read a single guessed identity;
//! * [`CitationLinkOutcome::Unresolved`] — a named [`UnlinkedCitationReason`],
//!   either `missing_occurrence_key` (absent identifier) or
//!   `no_entry_with_key` (partial/unknown identifier).
//!
//! [`CitationLinkCounts`] aggregates a whole occurrence stream, so unresolved
//! links stay countable instead of vanishing.
//!
//! `CitationEntry` carries no declared identifier on the wire today (the TEI
//! `xml:id` is consumed while lifting), so the producer supplies the declared
//! id through [`CitationEntryLinkKeys::new`]; an entry viewed through the wire
//! type alone publishes its positional key ([`CitationEntryLinkKeys::from_entry`]).

use serde::{Deserialize, Serialize};

use crate::parser::{CitationAnchor, CitationEntry};

/// Version of the published occurrence → entry link contract.
pub const CITATION_LINK_SCHEMA_VERSION: u16 = 1;

/// Closed set of published occurrence → entry key forms.
///
/// The declaration order is the published precedence order, but callers must
/// read [`CITATION_LINK_PRECEDENCE`] rather than rely on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationLinkKeyKind {
    /// The bibliography entry declares the identifier the occurrence points
    /// at (TEI `<biblStruct xml:id="b12">` ↔ `<ref type="bibr" target="#b12">`).
    DeclaredEntryId,
    /// Neither side declares an identifier: the occurrence points at the
    /// positional key of the entry (`b{ordinal}`).
    SyntheticEntryOrdinal,
}

/// Published tie-break precedence, strongest first.
///
/// The index of a kind in this array is its precedence rank (see
/// [`precedence_rank`]); a lower rank wins.
pub const CITATION_LINK_PRECEDENCE: [CitationLinkKeyKind; 2] = [
    CitationLinkKeyKind::DeclaredEntryId,
    CitationLinkKeyKind::SyntheticEntryOrdinal,
];

/// Published precedence rank of a key kind (0 = strongest).
pub fn precedence_rank(kind: CitationLinkKeyKind) -> usize {
    match kind {
        CitationLinkKeyKind::DeclaredEntryId => 0,
        CitationLinkKeyKind::SyntheticEntryOrdinal => 1,
    }
}

/// Positional key text published for an entry lifted without a declared
/// identifier. Mirrors GROBID's back-matter `b{index}` naming, so synthetic or
/// hand-written TEI that targets `#b0` still resolves.
pub fn positional_entry_key(ordinal: u32) -> String {
    format!("b{ordinal}")
}

/// Exact, allocation-free equality check for a positional key text.
///
/// Equivalent to `positional_entry_key(ordinal) == text` without building the
/// canonical string, so the entry scan allocates nothing for the entries that
/// do not match. Non-canonical spellings (`b00`, `b+0`, `B0`, trailing text)
/// are rejected — matching is exact text equality, never a parse that could
/// approximate an identity.
fn positional_key_matches(ordinal: u32, text: &str) -> bool {
    let Some(digits) = text.strip_prefix('b') else {
        return false;
    };
    let canonical_len = ordinal.checked_ilog10().map_or(1, |log| log as usize + 1);
    digits.len() == canonical_len
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits.parse::<u32>() == Ok(ordinal)
}

/// Normalize a key text: trim, strip one leading `#` (the TEI `target="#b12"`
/// form), trim again.
///
/// Returns `None` when nothing usable remains — a blank identifier is absence
/// ([`UnlinkedCitationReason::MissingOccurrenceKey`]), never a key.
pub fn normalize_link_key_text(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    let stripped = raw.strip_prefix('#').unwrap_or(raw).trim();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

/// One published link key: what an occurrence asks for and what an entry offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CitationLinkKey {
    /// Declared identifier shared by both sides.
    DeclaredEntryId {
        /// The declared identifier (leading `#` already stripped).
        entry_id: String,
    },
    /// Positional fallback key of an entry that declares no identifier.
    SyntheticEntryOrdinal {
        /// 0-based position of the entry in the lifted bibliography.
        ordinal: u32,
    },
}

impl CitationLinkKey {
    /// Key form of this key.
    pub fn kind(&self) -> CitationLinkKeyKind {
        match self {
            Self::DeclaredEntryId { .. } => CitationLinkKeyKind::DeclaredEntryId,
            Self::SyntheticEntryOrdinal { .. } => CitationLinkKeyKind::SyntheticEntryOrdinal,
        }
    }

    /// Published precedence rank of this key (0 = strongest).
    pub fn rank(&self) -> usize {
        precedence_rank(self.kind())
    }

    /// Text an occurrence's `ref_id` must carry to match this key.
    pub fn match_text(&self) -> String {
        match self {
            Self::DeclaredEntryId { entry_id } => entry_id.clone(),
            Self::SyntheticEntryOrdinal { ordinal } => positional_entry_key(*ordinal),
        }
    }

    /// True when an occurrence key text (already normalized) matches this key.
    ///
    /// Matching is exact text equality — never a prefix, case-folded or
    /// fuzzy comparison, because an approximated identity is a guessed one.
    pub fn matches_text(&self, text: &str) -> bool {
        match self {
            Self::DeclaredEntryId { entry_id } => entry_id == text,
            Self::SyntheticEntryOrdinal { ordinal } => positional_entry_key(*ordinal) == text,
        }
    }
}

/// The key one bibliography entry publishes.
///
/// Built by the producer that lifts the bibliography: it knows both the
/// entry's declared identifier (TEI `xml:id`) and the position it was lifted
/// at. [`CitationEntryLinkKeys::from_entry`] is the wire-type view, which
/// publishes only the positional key because `CitationEntry` carries no
/// declared identifier today.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationEntryLinkKeys {
    ordinal: u32,
    declared_id: Option<String>,
}

impl CitationEntryLinkKeys {
    /// Build the published keys of an entry at `ordinal`.
    ///
    /// `declared_id` is normalized (trimmed, one leading `#` stripped, see
    /// [`normalize_link_key_text`]); a blank id is absence, not a key.
    pub fn new(ordinal: u32, declared_id: Option<&str>) -> Self {
        Self {
            ordinal,
            declared_id: normalize_link_key_text(declared_id),
        }
    }

    /// The wire-type view of an entry: `CitationEntry` carries no declared
    /// identifier, so only the positional key is published.
    pub fn from_entry(ordinal: u32, _entry: &CitationEntry) -> Self {
        Self::new(ordinal, None)
    }

    /// 0-based position of the entry in the lifted bibliography.
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Declared identifier, when the entry carries one.
    pub fn declared_id(&self) -> Option<&str> {
        self.declared_id.as_deref()
    }

    /// The single published key of this entry: the declared identifier when it
    /// has one, otherwise the positional fallback.
    pub fn published_key(&self) -> CitationLinkKey {
        match self.declared_id.as_deref() {
            Some(entry_id) => CitationLinkKey::DeclaredEntryId {
                entry_id: entry_id.to_string(),
            },
            None => CitationLinkKey::SyntheticEntryOrdinal {
                ordinal: self.ordinal,
            },
        }
    }
}

/// Why an occurrence cannot be linked to a single entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlinkedCitationReason {
    /// The occurrence carries no usable identifier (absent, blank, or bare `#`).
    MissingOccurrenceKey,
    /// The occurrence's identifier matches no published entry key in this
    /// bibliography — the identity is partial or unknown, and stays so.
    NoEntryWithKey,
}

/// Why an occurrence matches more than one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbiguousLinkReason {
    /// More than one entry publishes the same declared identifier.
    DuplicateDeclaredId,
    /// More than one entry publishes the same key at the same precedence rank
    /// (a producer reused an ordinal). Defensive: still never a single guess.
    CompetingKeysAtSamePrecedence,
}

/// The published outcome of resolving one occurrence against a bibliography.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CitationLinkOutcome {
    /// Exactly one entry was reached, by the strongest matching key.
    Linked {
        /// Index of the linked entry in the resolved bibliography.
        entry_index: usize,
        /// The key that won.
        key: CitationLinkKey,
        /// Entry indexes that also matched but lost the published precedence
        /// (weaker ranks only). Empty when the link was uncontested; never
        /// silently dropped.
        #[serde(default)]
        shadowed_entry_indexes: Vec<usize>,
    },
    /// More than one entry matched at the winning precedence rank. Every
    /// candidate is reported; none is chosen.
    Ambiguous {
        /// Normalized identifier the occurrence carried, when it had one.
        #[serde(default)]
        ref_id: Option<String>,
        /// Every competing entry index, ascending. Never a single guess.
        candidate_entry_indexes: Vec<usize>,
        /// Why the match was ambiguous.
        reason: AmbiguousLinkReason,
    },
    /// No link was produced, for a named reason.
    Unresolved {
        /// Normalized identifier the occurrence carried, when it had one.
        #[serde(default)]
        ref_id: Option<String>,
        /// Why the link stayed unresolved.
        reason: UnlinkedCitationReason,
    },
}

impl CitationLinkOutcome {
    /// True only when exactly one entry was linked.
    pub fn is_linked(&self) -> bool {
        matches!(self, Self::Linked { .. })
    }

    /// True when the link was contested and decided by published precedence.
    pub fn is_contested(&self) -> bool {
        matches!(self, Self::Linked { shadowed_entry_indexes, .. } if !shadowed_entry_indexes.is_empty())
    }

    /// The linked entry index, or `None` for ambiguous/unresolved outcomes.
    pub fn entry_index(&self) -> Option<usize> {
        match self {
            Self::Linked { entry_index, .. } => Some(*entry_index),
            Self::Ambiguous { .. } | Self::Unresolved { .. } => None,
        }
    }

    /// The winning key, or `None` when no link was produced.
    pub fn linked_key(&self) -> Option<&CitationLinkKey> {
        match self {
            Self::Linked { key, .. } => Some(key),
            Self::Ambiguous { .. } | Self::Unresolved { .. } => None,
        }
    }

    /// Entry indexes that lost the published precedence, ascending.
    pub fn shadowed_entry_indexes(&self) -> &[usize] {
        match self {
            Self::Linked {
                shadowed_entry_indexes,
                ..
            } => shadowed_entry_indexes,
            Self::Ambiguous { .. } | Self::Unresolved { .. } => &[],
        }
    }

    /// Competing entry indexes for an ambiguous outcome, else empty.
    pub fn candidate_entry_indexes(&self) -> &[usize] {
        match self {
            Self::Ambiguous {
                candidate_entry_indexes,
                ..
            } => candidate_entry_indexes,
            Self::Linked { .. } | Self::Unresolved { .. } => &[],
        }
    }

    /// The unresolved reason, or `None` when the occurrence was not left
    /// unresolved.
    pub fn unresolved_reason(&self) -> Option<UnlinkedCitationReason> {
        match self {
            Self::Unresolved { reason, .. } => Some(*reason),
            Self::Linked { .. } | Self::Ambiguous { .. } => None,
        }
    }

    /// Normalized identifier the occurrence carried, when it had one.
    pub fn ref_id(&self) -> Option<&str> {
        match self {
            Self::Ambiguous { ref_id, .. } | Self::Unresolved { ref_id, .. } => ref_id.as_deref(),
            Self::Linked { .. } => None,
        }
    }
}

/// Countable summary of a whole occurrence stream.
///
/// `contested` is a subset of `linked`; `unlinked()` = `ambiguous + unresolved`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationLinkCounts {
    /// Outcomes that linked exactly one entry.
    pub linked: usize,
    /// Linked outcomes whose link was contested and decided by precedence.
    pub contested: usize,
    /// Outcomes with more than one candidate.
    pub ambiguous: usize,
    /// Outcomes with no candidate at all.
    pub unresolved: usize,
}

impl CitationLinkCounts {
    /// Count a batch of outcomes.
    pub fn from_outcomes(outcomes: &[CitationLinkOutcome]) -> Self {
        let mut counts = Self::default();
        for outcome in outcomes {
            match outcome {
                CitationLinkOutcome::Linked { .. } => {
                    counts.linked += 1;
                    if outcome.is_contested() {
                        counts.contested += 1;
                    }
                }
                CitationLinkOutcome::Ambiguous { .. } => counts.ambiguous += 1,
                CitationLinkOutcome::Unresolved { .. } => counts.unresolved += 1,
            }
        }
        counts
    }

    /// Number of outcomes counted.
    pub fn total(&self) -> usize {
        self.linked + self.ambiguous + self.unresolved
    }

    /// Outcomes that produced no link (ambiguous + unresolved).
    pub fn unlinked(&self) -> usize {
        self.ambiguous + self.unresolved
    }
}

/// Resolve one in-text occurrence against the bibliography entries of the same
/// revision, using the published precedence.
///
/// Pure and deterministic: the outcome depends only on the inputs, never on
/// construction or iteration order.
pub fn resolve_occurrence_to_entry(
    ref_id: Option<&str>,
    entries: &[CitationEntryLinkKeys],
) -> CitationLinkOutcome {
    let Some(key_text) = normalize_link_key_text(ref_id) else {
        return CitationLinkOutcome::Unresolved {
            ref_id: None,
            reason: UnlinkedCitationReason::MissingOccurrenceKey,
        };
    };

    // Gather every match grouped by published precedence rank (ascending).
    // Exact text equality only: a prefix, case-folded or fuzzy comparison
    // would turn an approximated identity into a guessed one. The scan is
    // allocation-free for entries that do not match.
    let mut matches_by_rank: Vec<(usize, Vec<(usize, CitationLinkKey)>)> = Vec::new();
    for (entry_index, entry) in entries.iter().enumerate() {
        let matched = match entry.declared_id() {
            Some(entry_id) => entry_id == key_text,
            None => positional_key_matches(entry.ordinal(), &key_text),
        };
        if !matched {
            continue;
        }
        let key = entry.published_key();
        let rank = key.rank();
        match matches_by_rank
            .iter_mut()
            .find(|(bucket_rank, _)| *bucket_rank == rank)
        {
            Some((_, bucket)) => bucket.push((entry_index, key)),
            None => matches_by_rank.push((rank, vec![(entry_index, key)])),
        }
    }
    matches_by_rank.sort_by_key(|(rank, _)| *rank);

    let Some((winner_rank, winners)) = matches_by_rank.first() else {
        return CitationLinkOutcome::Unresolved {
            ref_id: Some(key_text),
            reason: UnlinkedCitationReason::NoEntryWithKey,
        };
    };

    if winners.len() > 1 {
        return CitationLinkOutcome::Ambiguous {
            ref_id: Some(key_text),
            candidate_entry_indexes: winners.iter().map(|(index, _)| *index).collect(),
            reason: match *winner_rank {
                rank if rank == precedence_rank(CitationLinkKeyKind::DeclaredEntryId) => {
                    AmbiguousLinkReason::DuplicateDeclaredId
                }
                _ => AmbiguousLinkReason::CompetingKeysAtSamePrecedence,
            },
        };
    }

    let (entry_index, key) = winners[0].clone();
    let mut shadowed_entry_indexes: Vec<usize> = matches_by_rank
        .iter()
        .skip(1)
        .flat_map(|(_, bucket)| bucket.iter().map(|(index, _)| *index))
        .collect();
    shadowed_entry_indexes.sort_unstable();
    shadowed_entry_indexes.dedup();

    CitationLinkOutcome::Linked {
        entry_index,
        key,
        shadowed_entry_indexes,
    }
}

/// Resolve one lifted anchor against the entries of the same document.
pub fn resolve_anchor_to_entry(
    anchor: &CitationAnchor,
    entries: &[CitationEntryLinkKeys],
) -> CitationLinkOutcome {
    resolve_occurrence_to_entry(Some(anchor.ref_id.as_str()), entries)
}

/// Resolve an occurrence stream in input order (e.g. anchors in document order).
pub fn resolve_occurrences<'a, I>(
    ref_ids: I,
    entries: &[CitationEntryLinkKeys],
) -> Vec<CitationLinkOutcome>
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    ref_ids
        .into_iter()
        .map(|ref_id| resolve_occurrence_to_entry(ref_id, entries))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_normalization_treats_blank_as_absence() {
        assert_eq!(
            normalize_link_key_text(Some("  #b12 ")).as_deref(),
            Some("b12")
        );
        assert_eq!(normalize_link_key_text(Some("b12")).as_deref(), Some("b12"));
        assert_eq!(normalize_link_key_text(None), None);
        assert_eq!(normalize_link_key_text(Some("")), None);
        assert_eq!(normalize_link_key_text(Some("   ")), None);
        assert_eq!(normalize_link_key_text(Some("#")), None);
    }

    #[test]
    fn precedence_table_matches_ranks() {
        for (index, kind) in CITATION_LINK_PRECEDENCE.iter().enumerate() {
            assert_eq!(precedence_rank(*kind), index);
        }
        assert!(
            precedence_rank(CitationLinkKeyKind::DeclaredEntryId)
                < precedence_rank(CitationLinkKeyKind::SyntheticEntryOrdinal)
        );
    }

    #[test]
    fn positional_key_text_matches_canonically_and_rejects_near_misses() {
        for ordinal in [0, 1, 9, 10, 99, 100, 1_000, 4_294_967_295] {
            let canonical = positional_entry_key(ordinal);
            assert!(
                positional_key_matches(ordinal, &canonical),
                "canonical key {canonical} must match"
            );
            assert!(
                CitationLinkKey::SyntheticEntryOrdinal { ordinal }.matches_text(&canonical),
                "the public predicate agrees with the scan predicate"
            );
        }
        for text in [
            "", "b", "b00", "b01", "b+0", "B0", " b0", "b0 ", "b0v2", "0",
        ] {
            assert!(
                !positional_key_matches(0, text),
                "near-miss {text:?} must not match ordinal 0"
            );
        }
        assert!(!positional_key_matches(1, "b0"));
        assert!(!positional_key_matches(10, "b1"));
    }

    #[test]
    fn entry_publishes_one_key_only() {
        let declared = CitationEntryLinkKeys::new(0, Some("b0"));
        assert_eq!(
            declared.published_key(),
            CitationLinkKey::DeclaredEntryId {
                entry_id: "b0".into()
            },
            "a declared identifier replaces the position, it does not add to it"
        );
        let positional = CitationEntryLinkKeys::new(7, None);
        assert_eq!(
            positional.published_key(),
            CitationLinkKey::SyntheticEntryOrdinal { ordinal: 7 }
        );
        assert_eq!(positional_entry_key(7), "b7");
    }
}
