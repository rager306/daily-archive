//! Effective-input GROBID profile — the legacy compatibility surface over
//! the typed S04 transport vocabulary (M464-z84in2 S04/T01).
//!
//! Since S02 this type owned two public raw bools (`consolidate_citations`,
//! `consolidate_header`) plus the frozen D204 fingerprint strings. Since S04
//! the raw bools are replaced by typed [`Consolidation`] levels inside their
//! sanctioned ranges; the staged compatibility surface keeps:
//!
//! - [`GrobidProfile::rich`], [`GrobidProfile::structural`],
//!   [`GrobidProfile::name`],
//! - byte-identical [`GrobidProfile::configuration_fingerprint`] strings
//!   (D204; rotation is a coordinated identity decision, never a refactor
//!   side effect),
//! - [`GrobidProfile::TEI_COORDINATES`] / [`GrobidProfile::tei_coordinates_json`],
//! - boolean accessors for the historical fields, plus
//!   [`GrobidProfile::custom`] / [`GrobidProfile::with_consolidation`] for
//!   the custom profiles previously built through struct-literal mutation.
//!
//! The typed request dialect lives in [`crate::transport`];
//! [`GrobidProfile::dialect`] projects this profile into it without loss.

use crate::transport::{Consolidation, CoordinateSet, FulltextDialectV1};

/// Effective-input tuning of the GROBID REST dialect.
///
/// `Copy` by design: it is handed to every envelope identity and cached
/// backend unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrobidProfile {
    consolidation: Consolidation,
    coordinates: CoordinateSet,
}

impl GrobidProfile {
    /// Coordinate families requested for TEI output (legacy wire string,
    /// D204) — the exact historical associated constant.
    pub const TEI_COORDINATES: &str = "figure,table,note,formula";

    /// Rich profile: citation + header consolidation on, full coordinates.
    pub fn rich() -> Self {
        Self {
            consolidation: Consolidation::legacy(true, true),
            coordinates: CoordinateSet::default(),
        }
    }

    /// Structural profile: remote consolidation off, full coordinates.
    pub fn structural() -> Self {
        Self {
            consolidation: Consolidation::legacy(false, false),
            coordinates: CoordinateSet::default(),
        }
    }

    /// Custom profile from typed consolidation levels (full coordinates —
    /// the compatibility domain of the previous struct-literal mutation).
    pub fn custom(consolidation: Consolidation) -> Self {
        Self {
            consolidation,
            coordinates: CoordinateSet::default(),
        }
    }

    /// Staged compatibility builder: replace the consolidation levels
    /// (previously public raw bools).
    pub fn with_consolidation(mut self, consolidation: Consolidation) -> Self {
        self.consolidation = consolidation;
        self
    }

    /// Typed consolidation levels of this profile.
    pub const fn consolidation(&self) -> Consolidation {
        self.consolidation
    }

    /// Requested coordinate families.
    pub const fn coordinates(&self) -> CoordinateSet {
        self.coordinates
    }

    /// Historical raw-bool accessor: header consolidation enabled.
    pub fn consolidate_header(&self) -> bool {
        self.consolidation.header().value() != 0
    }

    /// Historical raw-bool accessor: citation consolidation enabled.
    pub fn consolidate_citations(&self) -> bool {
        self.consolidation.citations().value() != 0
    }

    /// Stable preset name (`rich`, `structural`, `custom`) — the exact
    /// historical derivation over the two consolidation levels.
    pub fn name(&self) -> &'static str {
        match (
            self.consolidation.header().value(),
            self.consolidation.citations().value(),
        ) {
            (1, 1) => "rich",
            (0, 0) => "structural",
            _ => "custom",
        }
    }

    /// D204 configuration fingerprint — byte-stable since S02.
    pub fn configuration_fingerprint(&self) -> String {
        format!(
            "profile={};consolidateHeader={};consolidateCitations={};teiCoordinates={}",
            self.name(),
            self.consolidation.header(),
            self.consolidation.citations(),
            self.coordinates.wire_value()
        )
    }

    /// TEI coordinate JSON for the REST body (legacy constant rendering).
    pub fn tei_coordinates_json() -> String {
        CoordinateSet::default().json_value()
    }

    /// Typed V1 dialect reproducing this profile's fulltext wire request
    /// (extended fields off, no page window).
    pub fn dialect(&self) -> FulltextDialectV1 {
        FulltextDialectV1::from_parts(self.consolidation, self.coordinates)
    }
}
