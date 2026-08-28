//! Provider-neutral TEI conservation engine (M464-z84in2 S03, R106).
//!
//! Pure comparator over [`crate::envelope::TeiEnvelope`] (raw TEI source
//! bytes) and [`crate::ir::TeiParseIr`] (this-cut lift result): it inventories
//! every observed TEI family in the source, compares the source against the
//! candidate IR, and produces an ordered deterministic
//! [`ConservationDelta`] with closed reason codes.
//!
//! Guarantees:
//! - **provider-neutral**: no filesystem, env, or cache-root access; no
//!   backend or HTTP calls; never mutates lift/project output. The engine is
//!   a pure function of `(&TeiEnvelope, &TeiParseIr)`.
//! - **redacting**: serializable output carries only identity strings, counts,
//!   ordinals, byte ranges, SHA-256 digests, and reason codes — never raw TEI,
//!   titles, quotes, or body text.
//! - **deterministic**: identical input produces byte-identical serialization
//!   (`BTreeMap` families, explicitly sorted entries, stable SHA-256 digests).
//! - **total**: malformed or adversarial bytes never panic; structural damage
//!   is surfaced as typed [`ConservationReasonCode::MalformedSource`] entries
//!   with bounded byte ranges.
//!
//! Family observation rules mirror the M463 lift matrix and the this-cut
//! lifter semantics:
//! - header region (before `<body`): first `<title>` → [`ConservationFamily::Title`],
//!   first `<abstract>` → [`ConservationFamily::Abstract`], `<persName>` →
//!   [`ConservationFamily::Author`], `<idno>` → [`ConservationFamily::ArticleIdno`];
//! - body region: `<head>` outside `<figure>` → [`ConservationFamily::Section`],
//!   `<biblStruct>` → [`ConservationFamily::CitationEntry`], `<ref type="bibr">`
//!   → [`ConservationFamily::CitationAnchor`], other `<ref>`/`<xref>` →
//!   [`ConservationFamily::CrossReference`], `<ptr>` → [`ConservationFamily::Link`],
//!   `<note>` → [`ConservationFamily::Note`], non-table `<figure>` →
//!   [`ConservationFamily::Figure`], `<table>` → [`ConservationFamily::Table`],
//!   `<cell>` → [`ConservationFamily::TableCell`], `<formula>` →
//!   [`ConservationFamily::Formula`];
//! - any element carrying a `coords` attribute → [`ConservationFamily::Coordinate`]
//!   with the owning family recorded.
//!
//! Deliberately-not-lifted families (authors, article idno, non-table figures,
//! links, cross references, notes — the S07 cut) surface as
//! [`ConservationReasonCode::UnsupportedThisCut`] entries, never silently.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};

use crate::envelope::TeiEnvelope;
use crate::ir::TeiParseIr;

// ─── Bounded byte range ─────────────────────────────────────────────────────

/// Half-open byte range `[start, end)` into the envelope's raw TEI bytes.
///
/// Always satisfies `start <= end <= raw_tei.len()`; the constructor clamps,
/// so adversarial scanner inputs can never produce an out-of-bounds span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    /// Clamping constructor: safe bounds are structurally guaranteed.
    pub fn new(start: usize, end: usize, raw_len: usize) -> Self {
        let s = start.min(end).min(raw_len) as u32;
        let e = (end.min(raw_len) as u32).max(s);
        Self { start: s, end: e }
    }

    /// Validity check against a raw byte length.
    pub fn is_valid(&self, raw_len: usize) -> bool {
        (self.start as usize) <= (self.end as usize) && (self.end as usize) <= raw_len
    }
}

// ─── Closed vocabularies ────────────────────────────────────────────────────

/// Every observable TEI conservation family (closed set, M463 lift matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConservationFamily {
    Title,
    Abstract,
    Author,
    ArticleIdno,
    Section,
    CitationEntry,
    CitationAnchor,
    Link,
    CrossReference,
    Note,
    Figure,
    Table,
    TableCell,
    Formula,
    Coordinate,
    Malformed,
}

impl ConservationFamily {
    /// Canonical snake_case encoding (matches serde).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Abstract => "abstract",
            Self::Author => "author",
            Self::ArticleIdno => "article_idno",
            Self::Section => "section",
            Self::CitationEntry => "citation_entry",
            Self::CitationAnchor => "citation_anchor",
            Self::Link => "link",
            Self::CrossReference => "cross_reference",
            Self::Note => "note",
            Self::Figure => "figure",
            Self::Table => "table",
            Self::TableCell => "table_cell",
            Self::Formula => "formula",
            Self::Coordinate => "coordinate",
            Self::Malformed => "malformed",
        }
    }

    /// Families the this-cut lifter intentionally does not lift (S07 cut).
    /// Observed occurrences surface as `UnsupportedThisCut`, not loss.
    pub fn is_unsupported_this_cut(self) -> bool {
        matches!(
            self,
            Self::Author
                | Self::ArticleIdno
                | Self::Link
                | Self::CrossReference
                | Self::Note
                | Self::Figure
        )
    }
}

/// Closed conservation reason vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConservationReasonCode {
    /// Item observed in source and present, identical, in the candidate IR.
    Retained,
    /// Observed in source, deliberately not lifted in this cut (S07 gaps).
    UnsupportedThisCut,
    /// Observed in source but absent from the candidate IR.
    ObservedButMissing,
    /// Family-level count differs between source and candidate.
    CountMismatch,
    /// Same items present but in a different order (structure observation).
    Reordered,
    /// Citation anchor observed in source whose linkage is lost in the IR.
    LinkageLoss,
    /// Observation has no attributable source span.
    SourceSpanMissing,
    /// Attributed span fails the safe-bounds invariant (defensive; the
    /// clamping constructor makes this unreachable from the engine itself).
    SourceSpanInvalid,
    /// Coordinate observed in source on a lifted surface but absent in IR.
    CoordinateDropped,
    /// Source bytes are structurally malformed at this location.
    MalformedSource,
}

impl ConservationReasonCode {
    /// Canonical snake_case encoding (matches serde).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retained => "retained",
            Self::UnsupportedThisCut => "unsupported_this_cut",
            Self::ObservedButMissing => "observed_but_missing",
            Self::CountMismatch => "count_mismatch",
            Self::Reordered => "reordered",
            Self::LinkageLoss => "linkage_loss",
            Self::SourceSpanMissing => "source_span_missing",
            Self::SourceSpanInvalid => "source_span_invalid",
            Self::CoordinateDropped => "coordinate_dropped",
            Self::MalformedSource => "malformed_source",
        }
    }

    /// Does this reason constitute introduced loss (not intentional cut)?
    pub fn is_loss(self) -> bool {
        matches!(
            self,
            Self::ObservedButMissing
                | Self::CountMismatch
                | Self::Reordered
                | Self::LinkageLoss
                | Self::CoordinateDropped
        )
    }
}

/// Aggregate verdict over the whole comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConservationVerdict {
    /// No introduced loss and no malformed source.
    Conserved,
    /// At least one loss reason code is present.
    LossDetected,
    /// Malformed source bytes were observed (takes precedence).
    MalformedSource,
}

impl ConservationVerdict {
    /// Canonical snake_case encoding (matches serde).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conserved => "conserved",
            Self::LossDetected => "loss_detected",
            Self::MalformedSource => "malformed_source",
        }
    }
}

// ─── Inventory ──────────────────────────────────────────────────────────────

/// One observed occurrence, on the source side (with byte range) or the
/// candidate side (byte range `None`; candidate observations come from IR,
/// which carries no source spans).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConservationObservation {
    pub family: ConservationFamily,
    /// 0-based ordinal within the family, in document order (source) or IR
    /// order (candidate).
    pub ordinal: u32,
    /// Bounded span into the raw TEI bytes (source side only).
    pub byte_range: Option<ByteRange>,
    /// SHA-256 hex of the observation's stable identity (stripped text for
    /// heads/titles, reference id for anchors, coords value for coordinates,
    /// element id for bibliography entries). Never raw content.
    pub digest: String,
    /// Owning family for coordinate observations.
    pub owner_family: Option<ConservationFamily>,
    /// True when the source bytes were structurally malformed here.
    pub malformed: bool,
}

/// Ordered per-family inventory. Source order is preserved inside each
/// family; families iterate in `BTreeMap` (canonical) order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ConservationInventory {
    pub families: BTreeMap<ConservationFamily, Vec<ConservationObservation>>,
}

impl ConservationInventory {
    /// Number of observations for one family (0 when absent).
    pub fn count(&self, family: ConservationFamily) -> usize {
        self.families.get(&family).map_or(0, Vec::len)
    }

    /// Total observations across all families.
    pub fn total(&self) -> usize {
        self.families.values().map(Vec::len).sum()
    }

    fn push(&mut self, observation: ConservationObservation) {
        let ordinal = self.count(observation.family) as u32;
        let mut observation = observation;
        observation.ordinal = ordinal;
        self.families
            .entry(observation.family)
            .or_default()
            .push(observation);
    }
}

// ─── Delta ──────────────────────────────────────────────────────────────────

/// Per-family aggregate counts in the delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FamilyDeltaCounts {
    pub source: u32,
    pub candidate: u32,
    pub retained: u32,
    pub unsupported: u32,
    pub lost: u32,
    pub malformed: u32,
}

/// One typed conservation finding. Serializable content is identity-only:
/// family, reason, ordinals, byte range, counts, a structure key (stable
/// ASCII label), and a SHA-256 observation digest. Never raw document text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConservationEntry {
    pub family: ConservationFamily,
    pub reason: ConservationReasonCode,
    pub source_ordinal: Option<u32>,
    pub candidate_ordinal: Option<u32>,
    pub byte_range: Option<ByteRange>,
    pub expected: u32,
    pub observed: u32,
    /// Stable structure label for structural findings
    /// (e.g. `"section_document_order"`, `"citation_anchor_linkage"`).
    pub structure_key: Option<String>,
    /// SHA-256 hex of the observed source identity ("" for family-level rows).
    pub digest: String,
}

/// Exact identity binding of a delta: backend, profile, paper, raw bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConservationIdentity {
    pub backend_id: String,
    pub profile_fingerprint: String,
    pub paper_id: String,
    pub raw_tei_len: u32,
    pub raw_tei_sha256: String,
}

/// Deterministic conservation delta: identity, verdict, per-family counts,
/// ordered entries, and a SHA-256 digest over the canonical delta content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConservationDelta {
    pub identity: ConservationIdentity,
    pub verdict: ConservationVerdict,
    pub family_counts: BTreeMap<ConservationFamily, FamilyDeltaCounts>,
    /// Sorted by (family, reason, source_ordinal, candidate_ordinal,
    /// byte_range, structure_key, digest) — deterministic total order.
    pub entries: Vec<ConservationEntry>,
    pub delta_digest: String,
}

// ─── Engine ─────────────────────────────────────────────────────────────────

/// Stateless conservation comparator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConservationEngine;

impl ConservationEngine {
    /// Build the stateless engine.
    pub fn new() -> Self {
        Self
    }

    /// Inventory every observed TEI family in the raw source bytes, each with
    /// a bounded byte span, in original document order.
    pub fn source_inventory(&self, envelope: &TeiEnvelope) -> ConservationInventory {
        scan_source(envelope.raw_tei())
    }

    /// Inventory the candidate side from the current `TeiParseIr` only.
    /// Authors, article idno, non-table figures and not-yet-lifted structures
    /// are intentionally absent here (typed gaps, never invented IR).
    pub fn candidate_inventory(&self, ir: &TeiParseIr) -> ConservationInventory {
        let mut inv = ConservationInventory::default();
        if !ir.title.is_empty() {
            inv.push(observation(
                ConservationFamily::Title,
                None,
                sha256_hex(ir.title.as_bytes()),
                None,
                false,
            ));
        }
        if !ir.abstract_text.is_empty() {
            inv.push(observation(
                ConservationFamily::Abstract,
                None,
                sha256_hex(ir.abstract_text.as_bytes()),
                None,
                false,
            ));
        }
        for section in &ir.sections {
            inv.push(observation(
                ConservationFamily::Section,
                None,
                sha256_hex(section.title.as_bytes()),
                None,
                false,
            ));
        }
        for citation in &ir.citations {
            inv.push(observation(
                ConservationFamily::CitationEntry,
                None,
                sha256_hex(citation.raw_text.as_bytes()),
                None,
                false,
            ));
            for anchor in &citation.anchors {
                inv.push(observation(
                    ConservationFamily::CitationAnchor,
                    None,
                    sha256_hex(anchor.ref_id.as_bytes()),
                    None,
                    false,
                ));
            }
        }
        for table in &ir.tables {
            inv.push(observation(
                ConservationFamily::Table,
                None,
                sha256_hex(table.table_id.as_bytes()),
                None,
                false,
            ));
            if let Some(coords) = table.coordinates.as_deref() {
                inv.push(observation(
                    ConservationFamily::Coordinate,
                    None,
                    sha256_hex(coords.as_bytes()),
                    Some(ConservationFamily::Table),
                    false,
                ));
            }
            for cell in &table.cells {
                inv.push(observation(
                    ConservationFamily::TableCell,
                    None,
                    sha256_hex(cell.text.as_bytes()),
                    None,
                    false,
                ));
                if let Some(coords) = cell.coordinates.as_deref() {
                    inv.push(observation(
                        ConservationFamily::Coordinate,
                        None,
                        sha256_hex(coords.as_bytes()),
                        Some(ConservationFamily::TableCell),
                        false,
                    ));
                }
            }
        }
        for formula in &ir.formulas {
            inv.push(observation(
                ConservationFamily::Formula,
                None,
                sha256_hex(formula.formula_id.as_bytes()),
                None,
                false,
            ));
            if let Some(coords) = formula.coordinates.as_deref() {
                inv.push(observation(
                    ConservationFamily::Coordinate,
                    None,
                    sha256_hex(coords.as_bytes()),
                    Some(ConservationFamily::Formula),
                    false,
                ));
            }
        }
        inv
    }

    /// Compare a source envelope against its candidate IR and produce the
    /// deterministic conservation delta.
    pub fn compare(&self, envelope: &TeiEnvelope, ir: &TeiParseIr) -> ConservationDelta {
        let raw_len = envelope.raw_tei().len();
        let source = self.source_inventory(envelope);
        let candidate = self.candidate_inventory(ir);

        let mut entries: Vec<ConservationEntry> = Vec::new();
        let mut families: Vec<ConservationFamily> = source
            .families
            .keys()
            .chain(candidate.families.keys())
            .copied()
            .collect();
        families.sort_unstable();
        families.dedup();

        for family in &families {
            let family = *family;
            let src = source
                .families
                .get(&family)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let cand = candidate
                .families
                .get(&family)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let (clean, malformed): (Vec<&ConservationObservation>, Vec<&ConservationObservation>) =
                src.iter().partition(|o| !o.malformed);
            for obs in malformed {
                entries.push(ConservationEntry {
                    family: obs.family,
                    reason: ConservationReasonCode::MalformedSource,
                    source_ordinal: Some(obs.ordinal),
                    candidate_ordinal: None,
                    byte_range: obs.byte_range,
                    expected: 0,
                    observed: 1,
                    structure_key: None,
                    digest: obs.digest.clone(),
                });
            }
            match family {
                ConservationFamily::Malformed => {}
                ConservationFamily::Title | ConservationFamily::Abstract => {
                    compare_presence(family, &clean, cand, &mut entries);
                }
                ConservationFamily::CitationEntry
                | ConservationFamily::Table
                | ConservationFamily::Formula
                | ConservationFamily::TableCell => {
                    compare_ordinal_aligned(family, &clean, cand, &mut entries);
                }
                ConservationFamily::Section => {
                    compare_digest_aligned(
                        family,
                        &clean,
                        cand,
                        "section_document_order",
                        ConservationReasonCode::ObservedButMissing,
                        &mut entries,
                    );
                }
                ConservationFamily::CitationAnchor => {
                    compare_digest_aligned(
                        family,
                        &clean,
                        cand,
                        "citation_anchor_linkage",
                        ConservationReasonCode::LinkageLoss,
                        &mut entries,
                    );
                }
                ConservationFamily::Coordinate => {
                    compare_coordinates(&clean, cand, &mut entries);
                }
                ConservationFamily::Author
                | ConservationFamily::ArticleIdno
                | ConservationFamily::Link
                | ConservationFamily::CrossReference
                | ConservationFamily::Note
                | ConservationFamily::Figure => {
                    compare_unsupported(family, &clean, cand, &mut entries);
                }
            }
        }

        sort_entries(&mut entries);

        let mut family_counts: BTreeMap<ConservationFamily, FamilyDeltaCounts> = BTreeMap::new();
        for family in &families {
            let src = source.count(*family) as u32;
            let cand = candidate.count(*family) as u32;
            let mut counts = FamilyDeltaCounts {
                source: src,
                candidate: cand,
                ..Default::default()
            };
            for entry in entries.iter().filter(|e| e.family == *family) {
                match entry.reason {
                    ConservationReasonCode::Retained => counts.retained += 1,
                    ConservationReasonCode::UnsupportedThisCut => counts.unsupported += 1,
                    ConservationReasonCode::MalformedSource => counts.malformed += 1,
                    reason if reason.is_loss() => counts.lost += 1,
                    _ => {}
                }
            }
            family_counts.insert(*family, counts);
        }

        let verdict = if entries
            .iter()
            .any(|e| e.reason == ConservationReasonCode::MalformedSource)
        {
            ConservationVerdict::MalformedSource
        } else if entries.iter().any(|e| e.reason.is_loss()) {
            ConservationVerdict::LossDetected
        } else {
            ConservationVerdict::Conserved
        };

        let identity = ConservationIdentity {
            backend_id: envelope.identity().backend_id().to_string(),
            profile_fingerprint: envelope.identity().profile_fingerprint().to_string(),
            paper_id: ir.paper_id.clone(),
            raw_tei_len: raw_len as u32,
            raw_tei_sha256: sha256_hex(envelope.raw_tei()),
        };

        let delta_digest = digest_delta(&identity, &verdict, &family_counts, &entries);

        ConservationDelta {
            identity,
            verdict,
            family_counts,
            entries,
            delta_digest,
        }
    }
}

// ─── Comparison strategies ──────────────────────────────────────────────────

fn compare_presence(
    family: ConservationFamily,
    src: &[&ConservationObservation],
    cand: &[ConservationObservation],
    entries: &mut Vec<ConservationEntry>,
) {
    if src.len() == 1 && cand.len() == 1 {
        entries.push(retained_entry(family, src[0], Some(0)));
    } else if src.len() == 1 && cand.is_empty() {
        entries.push(ConservationEntry {
            family,
            reason: ConservationReasonCode::ObservedButMissing,
            source_ordinal: Some(0),
            candidate_ordinal: None,
            byte_range: src[0].byte_range,
            expected: 1,
            observed: 0,
            structure_key: None,
            digest: src[0].digest.clone(),
        });
    } else if src.len() != cand.len() {
        entries.push(count_mismatch_entry(
            family,
            src.len() as u32,
            cand.len() as u32,
        ));
    }
}

/// Ordinal alignment for order-preserving families: the lifter emits items in
/// document order, so source ordinal i corresponds to candidate ordinal i.
fn compare_ordinal_aligned(
    family: ConservationFamily,
    src: &[&ConservationObservation],
    cand: &[ConservationObservation],
    entries: &mut Vec<ConservationEntry>,
) {
    for (i, obs) in src.iter().enumerate() {
        if i < cand.len() {
            entries.push(retained_entry(family, obs, Some(i as u32)));
        } else {
            entries.push(ConservationEntry {
                family,
                reason: ConservationReasonCode::ObservedButMissing,
                source_ordinal: Some(obs.ordinal),
                candidate_ordinal: None,
                byte_range: obs.byte_range,
                expected: src.len() as u32,
                observed: cand.len() as u32,
                structure_key: None,
                digest: obs.digest.clone(),
            });
        }
    }
    if src.len() != cand.len() {
        entries.push(count_mismatch_entry(
            family,
            src.len() as u32,
            cand.len() as u32,
        ));
    }
}

/// Digest-multiset alignment with positional preference: exact positional
/// matches stay clean, digest matches at a different position additionally
/// raise a `Reordered` structure observation, and unmatched source items get
/// `missing_reason` (per-family: missing vs linkage loss).
fn compare_digest_aligned(
    family: ConservationFamily,
    src: &[&ConservationObservation],
    cand: &[ConservationObservation],
    structure_key: &str,
    missing_reason: ConservationReasonCode,
    entries: &mut Vec<ConservationEntry>,
) {
    let mut used = vec![false; cand.len()];
    let mut reordered = false;
    for (i, obs) in src.iter().enumerate() {
        let exact = i < cand.len() && !used[i] && cand[i].digest == obs.digest;
        let matched = if exact {
            used[i] = true;
            Some(i)
        } else {
            cand.iter()
                .enumerate()
                .find(|(j, c)| !used[*j] && c.digest == obs.digest)
                .map(|(j, _)| {
                    used[j] = true;
                    reordered = true;
                    j
                })
        };
        match matched {
            Some(j) => entries.push(retained_entry_at(family, obs, i as u32, j as u32)),
            None => entries.push(ConservationEntry {
                family,
                reason: missing_reason,
                source_ordinal: Some(obs.ordinal),
                candidate_ordinal: None,
                byte_range: obs.byte_range,
                expected: src.len() as u32,
                observed: cand.len() as u32,
                structure_key: Some(structure_key.to_string()),
                digest: obs.digest.clone(),
            }),
        }
    }
    if reordered {
        entries.push(ConservationEntry {
            family,
            reason: ConservationReasonCode::Reordered,
            source_ordinal: None,
            candidate_ordinal: None,
            byte_range: None,
            expected: src.len() as u32,
            observed: cand.len() as u32,
            structure_key: Some(structure_key.to_string()),
            digest: String::new(),
        });
    }
    if src.len() != cand.len() {
        entries.push(count_mismatch_entry(
            family,
            src.len() as u32,
            cand.len() as u32,
        ));
    }
}

/// Coordinate alignment restricted to lifted surfaces: coords owned by
/// Table/TableCell/Formula must appear in the candidate coords digest pool
/// (matched → retained, unmatched → `CoordinateDropped`); coords owned by
/// unsupported surfaces (non-table figures, unknown owners) are
/// `UnsupportedThisCut`.
fn compare_coordinates(
    src: &[&ConservationObservation],
    cand: &[ConservationObservation],
    entries: &mut Vec<ConservationEntry>,
) {
    let lifted = |owner: Option<ConservationFamily>| {
        matches!(
            owner,
            Some(ConservationFamily::Table)
                | Some(ConservationFamily::TableCell)
                | Some(ConservationFamily::Formula)
        )
    };
    let mut used = vec![false; cand.len()];
    for obs in src {
        if !lifted(obs.owner_family) {
            entries.push(ConservationEntry {
                family: ConservationFamily::Coordinate,
                reason: ConservationReasonCode::UnsupportedThisCut,
                source_ordinal: Some(obs.ordinal),
                candidate_ordinal: None,
                byte_range: obs.byte_range,
                expected: 0,
                observed: 1,
                structure_key: None,
                digest: obs.digest.clone(),
            });
            continue;
        }
        let matched = cand
            .iter()
            .enumerate()
            .find(|(j, c)| !used[*j] && c.digest == obs.digest)
            .map(|(j, _)| {
                used[j] = true;
                j
            });
        entries.push(match matched {
            Some(j) => ConservationEntry {
                family: ConservationFamily::Coordinate,
                reason: ConservationReasonCode::Retained,
                source_ordinal: Some(obs.ordinal),
                candidate_ordinal: Some(j as u32),
                byte_range: obs.byte_range,
                expected: 1,
                observed: 1,
                structure_key: None,
                digest: obs.digest.clone(),
            },
            None => ConservationEntry {
                family: ConservationFamily::Coordinate,
                reason: ConservationReasonCode::CoordinateDropped,
                source_ordinal: Some(obs.ordinal),
                candidate_ordinal: None,
                byte_range: obs.byte_range,
                expected: 1,
                observed: 0,
                structure_key: None,
                digest: obs.digest.clone(),
            },
        });
    }
}

/// Unsupported-this-cut families: every source observation is typed as
/// intentional; a non-empty candidate side for these families is unexpected
/// and surfaces as `CountMismatch`.
fn compare_unsupported(
    family: ConservationFamily,
    src: &[&ConservationObservation],
    cand: &[ConservationObservation],
    entries: &mut Vec<ConservationEntry>,
) {
    for obs in src {
        entries.push(ConservationEntry {
            family,
            reason: ConservationReasonCode::UnsupportedThisCut,
            source_ordinal: Some(obs.ordinal),
            candidate_ordinal: None,
            byte_range: obs.byte_range,
            expected: 0,
            observed: 1,
            structure_key: None,
            digest: obs.digest.clone(),
        });
    }
    if !cand.is_empty() {
        entries.push(count_mismatch_entry(family, 0, cand.len() as u32));
    }
}

fn retained_entry(
    family: ConservationFamily,
    obs: &ConservationObservation,
    candidate_ordinal: Option<u32>,
) -> ConservationEntry {
    retained_entry_at(
        family,
        obs,
        obs.ordinal,
        candidate_ordinal.unwrap_or(obs.ordinal),
    )
}

fn retained_entry_at(
    family: ConservationFamily,
    obs: &ConservationObservation,
    source_ordinal: u32,
    candidate_ordinal: u32,
) -> ConservationEntry {
    ConservationEntry {
        family,
        reason: ConservationReasonCode::Retained,
        source_ordinal: Some(source_ordinal),
        candidate_ordinal: Some(candidate_ordinal),
        byte_range: obs.byte_range,
        expected: 1,
        observed: 1,
        structure_key: None,
        digest: obs.digest.clone(),
    }
}

fn count_mismatch_entry(
    family: ConservationFamily,
    expected: u32,
    observed: u32,
) -> ConservationEntry {
    ConservationEntry {
        family,
        reason: ConservationReasonCode::CountMismatch,
        source_ordinal: None,
        candidate_ordinal: None,
        byte_range: None,
        expected,
        observed,
        structure_key: None,
        digest: String::new(),
    }
}

fn observation(
    family: ConservationFamily,
    byte_range: Option<(usize, usize)>,
    digest: String,
    owner: Option<ConservationFamily>,
    malformed: bool,
) -> ConservationObservation {
    // raw_len is threaded through the scanner; this helper is used by the
    // candidate side (no range). Source-side observations construct directly.
    let byte_range = byte_range.map(|(s, e)| ByteRange::new(s, e, usize::MAX));
    ConservationObservation {
        family,
        ordinal: 0,
        byte_range,
        digest,
        owner_family: owner,
        malformed,
    }
}

fn sort_entries(entries: &mut [ConservationEntry]) {
    entries.sort_by(|a, b| {
        (
            a.family,
            a.reason,
            a.source_ordinal,
            a.candidate_ordinal,
            a.byte_range,
            a.structure_key.as_ref(),
            a.digest.as_str(),
        )
            .cmp(&(
                b.family,
                b.reason,
                b.source_ordinal,
                b.candidate_ordinal,
                b.byte_range,
                b.structure_key.as_ref(),
                b.digest.as_str(),
            ))
    });
}

/// Canonical digest over the delta content (identity, verdict, family
/// counts, sorted entries). Byte-stable for identical inputs.
fn digest_delta(
    identity: &ConservationIdentity,
    verdict: &ConservationVerdict,
    family_counts: &BTreeMap<ConservationFamily, FamilyDeltaCounts>,
    entries: &[ConservationEntry],
) -> String {
    let mut buf = Vec::new();
    buf.extend_from_slice(identity.backend_id.as_bytes());
    buf.push(0);
    buf.extend_from_slice(identity.profile_fingerprint.as_bytes());
    buf.push(0);
    buf.extend_from_slice(identity.paper_id.as_bytes());
    buf.push(0);
    buf.extend_from_slice(&identity.raw_tei_len.to_le_bytes());
    buf.push(0);
    buf.extend_from_slice(identity.raw_tei_sha256.as_bytes());
    buf.push(0);
    buf.extend_from_slice(verdict.as_str().as_bytes());
    buf.push(0);
    for (family, counts) in family_counts {
        buf.extend_from_slice(family.as_str().as_bytes());
        for v in [
            counts.source,
            counts.candidate,
            counts.retained,
            counts.unsupported,
            counts.lost,
            counts.malformed,
        ] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.push(1);
    }
    buf.push(2);
    for entry in entries {
        buf.extend_from_slice(entry.family.as_str().as_bytes());
        buf.extend_from_slice(entry.reason.as_str().as_bytes());
        push_opt_u32(&mut buf, entry.source_ordinal);
        push_opt_u32(&mut buf, entry.candidate_ordinal);
        match entry.byte_range {
            Some(r) => {
                buf.push(1);
                buf.extend_from_slice(&r.start.to_le_bytes());
                buf.extend_from_slice(&r.end.to_le_bytes());
            }
            None => buf.push(0),
        }
        buf.extend_from_slice(&entry.expected.to_le_bytes());
        buf.extend_from_slice(&entry.observed.to_le_bytes());
        match &entry.structure_key {
            Some(k) => buf.extend_from_slice(k.as_bytes()),
            None => buf.push(0xFF),
        }
        buf.push(0);
        buf.extend_from_slice(entry.digest.as_bytes());
        buf.push(3);
    }
    sha256_hex(&buf)
}

fn push_opt_u32(buf: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(v) => {
            buf.push(1);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        None => buf.push(0),
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

// ─── Tolerant byte scanner ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Open,
    Close,
    SelfClose,
}

#[derive(Debug, Clone, Copy)]
struct TagTok {
    start: usize,
    end: usize,
    /// Name byte range.
    name: (usize, usize),
    kind: TagKind,
    /// Attribute byte range (between name and closing `>`).
    attrs: (usize, usize),
    /// False when the tag was truncated (no closing `>` found).
    complete: bool,
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':' | b'.')
}

/// Scan raw bytes into tag tokens. Handles comments, PIs and declarations;
/// stops at a truncated tag (which the element matcher records as malformed).
fn scan_tag_tokens(bytes: &[u8]) -> Vec<TagTok> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if bytes[i..].starts_with(b"!--") {
            match find_sub(bytes, i + 3, b"-->") {
                Some(p) => i = p + 3,
                None => break,
            }
            continue;
        }
        if bytes.get(i) == Some(&b'!') || bytes.get(i) == Some(&b'?') {
            match find_byte(bytes, i, b'>') {
                Some(p) => i = p + 1,
                None => break,
            }
            continue;
        }
        let mut kind = if bytes.get(i) == Some(&b'/') {
            i += 1;
            TagKind::Close
        } else {
            TagKind::Open
        };
        let name_start = i;
        while i < bytes.len() && is_name_byte(bytes[i]) {
            i += 1;
        }
        let name_end = i;
        if name_end == name_start {
            // Bare `<` with no name (e.g. `a < b` in text): malformed text
            // fragment; skip one byte and continue scanning.
            i = start + 1;
            continue;
        }
        let mut j = i;
        let mut quote = 0u8;
        let mut closed = false;
        while j < bytes.len() {
            let b = bytes[j];
            if quote != 0 {
                if b == quote {
                    quote = 0;
                }
            } else if b == b'"' || b == b'\'' {
                quote = b;
            } else if b == b'>' {
                closed = true;
                break;
            }
            j += 1;
        }
        let end_excl = if closed { j + 1 } else { j };
        if closed && kind == TagKind::Open && j > name_start && bytes[j - 1] == b'/' {
            kind = TagKind::SelfClose;
        }
        let attrs_end = j;
        out.push(TagTok {
            start,
            end: end_excl,
            name: (name_start, name_end),
            kind,
            attrs: (i, attrs_end),
            complete: closed,
        });
        i = end_excl;
        if !closed {
            break;
        }
    }
    out
}

fn find_byte(bytes: &[u8], from: usize, needle: u8) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|&b| b == needle)
        .map(|p| from + p)
}

fn find_sub(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

#[derive(Debug, Clone)]
struct Elem {
    name: String,
    /// Full element span (open-tag start to close-tag end; self-close tags
    /// span the single tag).
    span: (usize, usize),
    /// End of the opening tag (exclusive) — precise span anchor for
    /// attribute-level observations such as `coords`.
    open_tag_end: usize,
    attrs: String,
    malformed: bool,
    inside_bibl: bool,
}

/// Match open/close tokens into elements with a recovery stack. Unclosed
/// inner elements become malformed observations bounded by the enclosing
/// close tag (or the raw length at EOF); stray close tags become malformed
/// observations of the `Malformed` family.
fn match_elements(tokens: &[TagTok], raw: &[u8]) -> Vec<Elem> {
    let raw_len = raw.len();
    fn tok_name(raw: &[u8], t: &TagTok) -> String {
        String::from_utf8_lossy(&raw[t.name.0..t.name.1.min(raw.len())]).into_owned()
    }
    fn tok_attrs(raw: &[u8], t: &TagTok) -> String {
        String::from_utf8_lossy(&raw[t.attrs.0..t.attrs.1.min(raw.len())]).into_owned()
    }
    let mut elements: Vec<Elem> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for (ti, tok) in tokens.iter().enumerate() {
        let inside_bibl = stack
            .iter()
            .any(|&si| tok_name(raw, &tokens[si]) == "biblStruct");
        match tok.kind {
            TagKind::Open => stack.push(ti),
            TagKind::SelfClose => elements.push(Elem {
                name: tok_name(raw, tok),
                span: (tok.start, tok.end.min(raw_len)),
                open_tag_end: tok.end.min(raw_len),
                attrs: tok_attrs(raw, tok),
                malformed: !tok.complete,
                inside_bibl,
            }),
            TagKind::Close => {
                let close_name = tok_name(raw, tok);
                let match_pos = stack
                    .iter()
                    .rev()
                    .find(|&&si| tok_name(raw, &tokens[si]) == close_name)
                    .copied();
                match match_pos {
                    Some(open_ti) => {
                        while let Some(&ui) = stack.last() {
                            if ui == open_ti {
                                break;
                            }
                            let ut = &tokens[ui];
                            elements.push(Elem {
                                name: tok_name(raw, ut),
                                span: (ut.start, tok.end.min(raw_len)),
                                open_tag_end: ut.end.min(raw_len),
                                attrs: tok_attrs(raw, ut),
                                malformed: true,
                                inside_bibl,
                            });
                            stack.pop();
                        }
                        let ot = &tokens[open_ti];
                        elements.push(Elem {
                            name: tok_name(raw, ot),
                            span: (ot.start, tok.end.min(raw_len)),
                            open_tag_end: ot.end.min(raw_len),
                            attrs: tok_attrs(raw, ot),
                            malformed: false,
                            inside_bibl,
                        });
                        stack.pop();
                    }
                    None => {
                        // Stray close tag: structurally malformed.
                        elements.push(Elem {
                            name: close_name,
                            span: (tok.start, tok.end.min(raw_len)),
                            open_tag_end: tok.end.min(raw_len),
                            attrs: tok_attrs(raw, tok),
                            malformed: true,
                            inside_bibl,
                        });
                    }
                }
            }
        }
    }
    while let Some(ui) = stack.pop() {
        let ut = &tokens[ui];
        elements.push(Elem {
            name: tok_name(raw, ut),
            span: (ut.start, raw_len),
            open_tag_end: ut.end.min(raw_len),
            attrs: tok_attrs(raw, ut),
            malformed: true,
            inside_bibl: false,
        });
    }
    elements
}

/// Tolerant attribute extractor: finds `name`, requires an attribute-name
/// boundary before it and `=` + quoted value after it.
fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let bytes = attrs.as_bytes();
    let nb = name.as_bytes();
    let mut i = 0usize;
    while i + nb.len() <= bytes.len() {
        if &bytes[i..i + nb.len()] == nb
            && (i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b'/')
        {
            let mut j = i + nb.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if bytes.get(j) == Some(&b'=') {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if let Some(&q) = bytes.get(j)
                    && (q == b'"' || q == b'\'')
                    && let Some(end) = find_byte(bytes, j + 1, q)
                {
                    return Some(String::from_utf8_lossy(&bytes[j + 1..end]).into_owned());
                }
            }
        }
        i += 1;
    }
    None
}

/// Strip XML tags from raw bytes (lossy UTF-8), mirroring the lifter's
/// visible-text semantics for identity digests.
fn strip_tags_bytes(bytes: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut in_tag = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_tag {
            if b == b'>' {
                in_tag = false;
            }
        } else if b == b'<' {
            in_tag = true;
        } else {
            out.push(b);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Full source scan: tokens → elements → family observations.
fn scan_source(raw: &[u8]) -> ConservationInventory {
    let raw_len = raw.len();
    let tokens = scan_tag_tokens(raw);
    let body_start = tokens
        .iter()
        .find(|t| t.kind == TagKind::Open && raw[t.name.0..t.name.1] == *b"body")
        .map(|t| t.start);
    let elements = match_elements(&tokens, raw);

    let mut inv = ConservationInventory::default();
    let mut title_seen = false;
    let mut abstract_seen = false;

    for elem in &elements {
        let region_header = body_start.is_none_or(|bs| elem.span.0 < bs);
        let family = classify_element(elem, region_header, &mut title_seen, &mut abstract_seen);

        // Structural damage on elements outside every conservation family
        // (stray/unclosed structural containers, bare closes) must stay
        // typed: emit a Malformed-family observation instead of vanishing
        // silently (R106: corruption can never disappear without a reason).
        // Well-formed sources produce no malformed elements, so this is a
        // no-op on every preserved fixture and lift baseline.
        if family.is_none() && elem.malformed {
            inv.push(ConservationObservation {
                family: ConservationFamily::Malformed,
                ordinal: 0,
                byte_range: Some(ByteRange::new(
                    elem.span.0,
                    elem.span.1.min(raw_len),
                    raw_len,
                )),
                digest: sha256_hex(&raw[elem.span.0.min(raw.len())..elem.span.1.min(raw.len())]),
                owner_family: None,
                malformed: true,
            });
        }

        // Coordinate observations: any element carrying a coords attribute.
        if let Some(coords) = attr_value(&elem.attrs, "coords") {
            let owner = coordinate_owner(elem);
            let (s, e) = (elem.span.0, elem.open_tag_end.min(raw_len));
            inv.push(ConservationObservation {
                family: ConservationFamily::Coordinate,
                ordinal: 0,
                byte_range: Some(ByteRange::new(s, e, raw_len)),
                digest: sha256_hex(coords.as_bytes()),
                owner_family: owner,
                malformed: elem.malformed,
            });
        }

        let Some(family) = family else { continue };
        let digest = element_identity(family, elem, raw);
        inv.push(ConservationObservation {
            family,
            ordinal: 0,
            byte_range: Some(ByteRange::new(
                elem.span.0,
                elem.span.1.min(raw_len),
                raw_len,
            )),
            digest,
            owner_family: None,
            malformed: elem.malformed,
        });
    }

    // Truncated trailing tag: the scanner stopped inside an incomplete token.
    if let Some(last) = tokens.last()
        && !last.complete
    {
        let name = String::from_utf8_lossy(&raw[last.name.0..last.name.1]).into_owned();
        let family = family_for_partial_name(&name);
        inv.push(ConservationObservation {
            family,
            ordinal: 0,
            byte_range: Some(ByteRange::new(last.start, raw_len, raw_len)),
            digest: sha256_hex(&raw[last.start..raw_len]),
            owner_family: None,
            malformed: true,
        });
    }

    inv
}

fn classify_element(
    elem: &Elem,
    region_header: bool,
    title_seen: &mut bool,
    abstract_seen: &mut bool,
) -> Option<ConservationFamily> {
    let body = !region_header;
    match elem.name.as_str() {
        "title" if region_header && !elem.inside_bibl && !*title_seen => {
            *title_seen = true;
            Some(ConservationFamily::Title)
        }
        "abstract" if region_header && !*abstract_seen => {
            *abstract_seen = true;
            Some(ConservationFamily::Abstract)
        }
        // GROBID header authors/idnos live inside <sourceDesc><biblStruct>:
        // the header-region guard alone decides Author/ArticleIdno (the
        // manifest pins header authors by TEI bytes); bibliography persName
        // in the body stays unclassified because it is not in the header.
        "persName" if region_header => Some(ConservationFamily::Author),
        "idno" if region_header => Some(ConservationFamily::ArticleIdno),
        // Mirrors the flat-scan lifter semantics: every `<head>` in the body
        // is a section-window boundary (caption heads included).
        "head" if body => Some(ConservationFamily::Section),
        "biblStruct" => Some(ConservationFamily::CitationEntry),
        "ref" if body && !elem.inside_bibl => {
            if attr_value(&elem.attrs, "type").as_deref() == Some("bibr") {
                Some(ConservationFamily::CitationAnchor)
            } else {
                Some(ConservationFamily::CrossReference)
            }
        }
        "xref" if body && !elem.inside_bibl => Some(ConservationFamily::CrossReference),
        // GROBID emits link <ptr> elements inside <biblStruct> (relatedItem
        // URLs) — they are the document's observed link family; the header
        // region stays out of scope via the body guard.
        "ptr" if body => Some(ConservationFamily::Link),
        "note" if body && !elem.inside_bibl => Some(ConservationFamily::Note),
        "figure" if body => {
            if attr_value(&elem.attrs, "type").as_deref() == Some("table") {
                // Table wrapper: the `<table>` element itself is the observed
                // Table family member (mirrors the lifter's table iteration).
                None
            } else {
                Some(ConservationFamily::Figure)
            }
        }
        "table" if body => Some(ConservationFamily::Table),
        "cell" => Some(ConservationFamily::TableCell),
        "formula" => Some(ConservationFamily::Formula),
        _ => None,
    }
}

fn coordinate_owner(elem: &Elem) -> Option<ConservationFamily> {
    match elem.name.as_str() {
        "figure" => {
            if attr_value(&elem.attrs, "type").as_deref() == Some("table") {
                Some(ConservationFamily::Table)
            } else {
                Some(ConservationFamily::Figure)
            }
        }
        "table" => Some(ConservationFamily::Table),
        "cell" => Some(ConservationFamily::TableCell),
        "formula" => Some(ConservationFamily::Formula),
        _ => None,
    }
}

fn family_for_partial_name(name: &str) -> ConservationFamily {
    match name {
        "title" => ConservationFamily::Title,
        "abstract" => ConservationFamily::Abstract,
        "persName" => ConservationFamily::Author,
        "idno" => ConservationFamily::ArticleIdno,
        "head" => ConservationFamily::Section,
        "biblStruct" => ConservationFamily::CitationEntry,
        "ref" | "xref" => ConservationFamily::CrossReference,
        "ptr" => ConservationFamily::Link,
        "note" => ConservationFamily::Note,
        "figure" => ConservationFamily::Figure,
        "table" => ConservationFamily::Table,
        "cell" => ConservationFamily::TableCell,
        "formula" => ConservationFamily::Formula,
        _ => ConservationFamily::Malformed,
    }
}

/// Stable identity digest for one observed element (never raw content in
/// output — only its SHA-256).
fn element_identity(family: ConservationFamily, elem: &Elem, raw: &[u8]) -> String {
    match family {
        ConservationFamily::Section => {
            // Content between open tag end and `</head>` start. The element
            // span ends at the close tag; the close tag is 7 bytes
            // (`</head>`), bounded defensively.
            let end = elem
                .span
                .1
                .saturating_sub("</head>".len())
                .max(elem.open_tag_end);
            let content =
                strip_tags_bytes(&raw[elem.open_tag_end.min(raw.len())..end.min(raw.len())]);
            sha256_hex(content.as_bytes())
        }
        ConservationFamily::CitationAnchor => {
            let target = attr_value(&elem.attrs, "target")
                .map(|t| t.trim_start_matches('#').to_string())
                .unwrap_or_default();
            sha256_hex(target.as_bytes())
        }
        ConservationFamily::CitationEntry
        | ConservationFamily::Table
        | ConservationFamily::Formula => {
            // Identity: xml:id when present, else the deterministic ordinal
            // key is filled in by the ordinal assignment (digest stays over
            // the element id bytes; fallback rows are distinguished by
            // ordinal alignment, not digest).
            let id = attr_value(&elem.attrs, "xml:id").unwrap_or_default();
            sha256_hex(id.as_bytes())
        }
        ConservationFamily::TableCell => {
            let end = elem
                .span
                .1
                .saturating_sub("</cell>".len())
                .max(elem.open_tag_end);
            let content =
                strip_tags_bytes(&raw[elem.open_tag_end.min(raw.len())..end.min(raw.len())]);
            sha256_hex(content.trim().as_bytes())
        }
        ConservationFamily::Title
        | ConservationFamily::Abstract
        | ConservationFamily::Author
        | ConservationFamily::ArticleIdno
        | ConservationFamily::Link
        | ConservationFamily::CrossReference
        | ConservationFamily::Note
        | ConservationFamily::Figure
        | ConservationFamily::Coordinate
        | ConservationFamily::Malformed => {
            sha256_hex(&raw[elem.span.0.min(raw.len())..elem.span.1.min(raw.len())])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TeiIdentity, TeiLifter};

    const TEI: &str = r#"<TEI><teiHeader><fileDesc><titleStmt><title>Paper title</title><author><persName>Ada</persName></author></titleStmt></fileDesc><profileDesc><abstract>Summary</abstract></profileDesc></teiHeader><text><body><div><head>Introduction</head><p>Body</p></div><div><head>Methods</head><p>More</p></div></body></text></TEI>"#;

    fn fixture() -> (TeiEnvelope, TeiParseIr) {
        let envelope = TeiEnvelope::new(
            TEI.as_bytes().to_vec(),
            TeiIdentity::new("cached-tei", "profile=test"),
        );
        let ir = TeiLifter::new().lift(&envelope, "paper-1", "pdf-hash");
        (envelope, ir)
    }

    #[test]
    fn byte_range_clamps_to_valid_half_open_bounds() {
        assert_eq!(ByteRange::new(9, 2, 5), ByteRange { start: 2, end: 2 });
        assert!(ByteRange::new(9, 2, 5).is_valid(5));
    }

    #[test]
    fn matching_lift_is_conserved_and_keeps_intentional_gaps_typed() {
        let (envelope, ir) = fixture();
        let delta = ConservationEngine::new().compare(&envelope, &ir);

        assert_eq!(delta.verdict, ConservationVerdict::Conserved);
        assert!(delta.entries.iter().any(|entry| {
            entry.family == ConservationFamily::Author
                && entry.reason == ConservationReasonCode::UnsupportedThisCut
        }));
        assert!(delta.entries.iter().all(|entry| {
            entry
                .byte_range
                .is_none_or(|range| range.is_valid(envelope.raw_tei().len()))
        }));
    }

    #[test]
    fn missing_section_is_reported_as_typed_loss() {
        let (envelope, mut ir) = fixture();
        ir.sections.pop();
        let delta = ConservationEngine::new().compare(&envelope, &ir);

        assert_eq!(delta.verdict, ConservationVerdict::LossDetected);
        assert!(delta.entries.iter().any(|entry| {
            entry.family == ConservationFamily::Section
                && entry.reason == ConservationReasonCode::ObservedButMissing
                && entry.source_ordinal == Some(1)
        }));
    }

    #[test]
    fn replay_has_stable_order_and_byte_identical_serialization() {
        let (envelope, ir) = fixture();
        let engine = ConservationEngine::new();
        let first = engine.compare(&envelope, &ir);
        let second = engine.compare(&envelope, &ir);

        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_vec(&first).expect("delta serializes"),
            serde_json::to_vec(&second).expect("replayed delta serializes")
        );
        assert!(first.entries.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
