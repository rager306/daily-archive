//! Versioned provider-neutral request vocabulary and the closed typed
//! failure vocabulary for the parse transport seam (M464-z84in2 S04/T01 and
//! S05/T01, R107/R113).
//!
//! Three contracts live here:
//!
//! 1. [`FulltextRequestVersioned`] — a serde-versioned request envelope whose
//!    V1 dialect ([`FulltextDialectV1`]) expresses, without loss, the Java
//!    GROBID fulltext wire dialect: typed consolidation levels inside their
//!    sanctioned ranges, the typed coordinate family set, raw
//!    citation/affiliation switches, sentence/ID switches and a validated
//!    inclusive page window. The legacy/default projection reproduces the
//!    current [`crate::profile::GrobidProfile`] wire request byte-for-byte
//!    (D204 fingerprints untouched).
//! 2. [`ParseBackendFailure`] — a closed, serializable failure with a stable
//!    schema version and structured fields carrying only safe status / phase
//!    / limit / retry metadata. Raw PDF/TEI bytes, response bodies, URL
//!    queries, env values and physical paths have no field to live in, and
//!    `Display`/`Debug` are deterministic redactions of that metadata only.
//! 3. [`ParseEndpoint`] / [`ParseResultKind`] — the closed provider-neutral
//!    endpoint and result-kind vocabulary (S05/T01) plus the versioned V1
//!    request dialects of the header ([`HeaderDialectV1`]) and references
//!    ([`ReferencesDialectV1`]) operations: only output-affecting controls,
//!    `deny_unknown_fields` payloads, and deterministic per-endpoint
//!    canonical identities. Endpoint knobs cannot cross endpoints because
//!    each operation owns its own dialect type.
//!
//! Transport tuning (timeout, opt-in request/response limits) is deliberately
//! absent from the request vocabulary: it never belongs to content identity
//! (identity exclusion is pinned by `tests/transport_contract.rs` and
//! `tests/cache_identity.rs`). Limits surface only as typed [`LimitKind`]
//! metadata on a failure after they have actually been hit.
//!
//! No HTTP, no filesystem, no env: this module is the provider-neutral
//! protocol layer; the Java GROBID projector stays in [`crate::http`].

use std::fmt;

use crate::profile::GrobidProfile;
use da_ports::parser::ParserError;
use serde::de::{self, Deserializer};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// Stable schema version of the fulltext request vocabulary.
pub const FULLTEXT_REQUEST_SCHEMA_VERSION: u8 = 1;

/// Stable schema version of the typed failure vocabulary.
pub const PARSE_FAILURE_SCHEMA_VERSION: u32 = 1;

/// Stable schema version of the header request dialect (S05/T01).
pub const HEADER_REQUEST_SCHEMA_VERSION: u8 = 1;

/// Stable schema version of the references request dialect (S05/T01).
pub const REFERENCES_REQUEST_SCHEMA_VERSION: u8 = 1;

// ---------------------------------------------------------------------------
// Typed consolidation levels (sanctioned GROBID consolidation domains)
// ---------------------------------------------------------------------------

/// A validated GROBID consolidation level inside its sanctioned range.
macro_rules! sanctioned_consolidation_level {
    ($(#[$doc:meta])* $name:ident, $lo:literal, $hi:literal, $field:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u8);

        impl $name {
            /// Sanctioned value range (half-open; matches the consolidation
            /// enum domain of the Java GROBID REST dialect).
            pub const ALLOWED: core::ops::Range<u8> = $lo..$hi;

            /// Build from a raw level, rejecting values outside
            /// [`Self::ALLOWED`].
            pub fn new(value: u8) -> Result<Self, InvalidConsolidation> {
                if !Self::ALLOWED.contains(&value) {
                    return Err(InvalidConsolidation { field: $field, value });
                }
                Ok(Self(value))
            }

            /// Raw wire level (`0`, `1`, ... as sent/consolidated).
            pub const fn value(self) -> u8 {
                self.0
            }
        }

        impl TryFrom<u8> for $name {
            type Error = InvalidConsolidation;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_u8(self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = u8::deserialize(deserializer)?;
                Self::new(value).map_err(|e| de::Error::custom(e.to_string()))
            }
        }
    };
}

sanctioned_consolidation_level!(
    /// `consolidateHeader` level. Sanctioned range `0..3` (`0`, `1` or `2`).
    HeaderConsolidation,
    0,
    3,
    "header"
);
sanctioned_consolidation_level!(
    /// `consolidateCitations` level. Sanctioned range `0..2` (`0` or `1`).
    CitationsConsolidation,
    0,
    2,
    "citations"
);
sanctioned_consolidation_level!(
    /// `consolidateFunders` level. Sanctioned range `0..2` (`0` or `1`).
    FundersConsolidation,
    0,
    2,
    "funders"
);

/// Rejection of a consolidation level outside its sanctioned range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidConsolidation {
    /// Which level (`"header"`, `"citations"` or `"funders"`).
    pub field: &'static str,
    /// The rejected raw value.
    pub value: u8,
}

impl fmt::Display for InvalidConsolidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let range = match self.field {
            "header" => HeaderConsolidation::ALLOWED,
            "citations" => CitationsConsolidation::ALLOWED,
            _ => FundersConsolidation::ALLOWED,
        };
        write!(
            f,
            "consolidation level `{}`={} outside sanctioned range {:?}",
            self.field, self.value, range
        )
    }
}

impl std::error::Error for InvalidConsolidation {}

/// Typed consolidation dialect for the fulltext request (header/citations/
/// funders levels, each inside its sanctioned range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Consolidation {
    header: HeaderConsolidation,
    citations: CitationsConsolidation,
    funders: FundersConsolidation,
}

impl Consolidation {
    /// Build from raw levels, validating every sanctioned range.
    pub fn try_new(header: u8, citations: u8, funders: u8) -> Result<Self, InvalidConsolidation> {
        Ok(Self {
            header: HeaderConsolidation::new(header)?,
            citations: CitationsConsolidation::new(citations)?,
            funders: FundersConsolidation::new(funders)?,
        })
    }

    /// Legacy preset domain: historical raw bools (`0`/`1`), funders off.
    pub fn legacy(header: bool, citations: bool) -> Self {
        Self::try_new(u8::from(header), u8::from(citations), 0)
            .expect("legacy consolidation bools are inside the sanctioned ranges")
    }

    /// `consolidateHeader` level.
    pub const fn header(self) -> HeaderConsolidation {
        self.header
    }

    /// `consolidateCitations` level.
    pub const fn citations(self) -> CitationsConsolidation {
        self.citations
    }

    /// `consolidateFunders` level.
    pub const fn funders(self) -> FundersConsolidation {
        self.funders
    }
}

impl Serialize for Consolidation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Consolidation", 3)?;
        state.serialize_field("header", &self.header)?;
        state.serialize_field("citations", &self.citations)?;
        state.serialize_field("funders", &self.funders)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Consolidation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            header: u8,
            citations: u8,
            funders: u8,
        }
        let repr = Repr::deserialize(deserializer)?;
        Self::try_new(repr.header, repr.citations, repr.funders)
            .map_err(|e| de::Error::custom(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Typed coordinate families / set
// ---------------------------------------------------------------------------

/// A sanctioned `teiCoordinates` family of the Java GROBID fulltext dialect.
///
/// Declaration order is the canonical wire order; the derived ordering is
/// the identity ordering (deterministic, deduped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordinateFamily {
    /// `<figure>` coordinates (images).
    Figure,
    /// `<table>` coordinates.
    Table,
    /// `<note>` coordinates (footnotes).
    Note,
    /// `<formula>` coordinates (M400 S03 T01).
    Formula,
}

impl CoordinateFamily {
    /// Exact wire token sent to Java GROBID.
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Figure => "figure",
            Self::Table => "table",
            Self::Note => "note",
            Self::Formula => "formula",
        }
    }
}

/// Deduplicated, canonically ordered set of requested coordinate families
/// (`Copy`, closed over the four sanctioned families).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoordinateSet {
    bits: u8,
}

impl CoordinateSet {
    const FIGURE: u8 = 1 << 0;
    const TABLE: u8 = 1 << 1;
    const NOTE: u8 = 1 << 2;
    const FORMULA: u8 = 1 << 3;
    const LEGACY_FULL: u8 = Self::FIGURE | Self::TABLE | Self::NOTE | Self::FORMULA;

    /// Every sanctioned family (the legacy/full coordinate request).
    pub fn all() -> Self {
        Self {
            bits: Self::LEGACY_FULL,
        }
    }

    /// The empty set (coordinate output disabled).
    pub fn none() -> Self {
        Self { bits: 0 }
    }

    /// Build from families; deduplicates into canonical order.
    pub fn from_families(families: impl IntoIterator<Item = CoordinateFamily>) -> Self {
        let mut bits = 0u8;
        for family in families {
            bits |= family.bit();
        }
        Self { bits }
    }

    /// Families in canonical (wire) order.
    pub fn families(self) -> impl Iterator<Item = CoordinateFamily> {
        const ALL: [CoordinateFamily; 4] = [
            CoordinateFamily::Figure,
            CoordinateFamily::Table,
            CoordinateFamily::Note,
            CoordinateFamily::Formula,
        ];
        ALL.into_iter().filter(move |f| self.bits & f.bit() != 0)
    }

    /// Whether `family` is requested.
    pub fn contains(self, family: CoordinateFamily) -> bool {
        self.bits & family.bit() != 0
    }

    /// Comma-joined wire value (e.g. `figure,table,note,formula`).
    pub fn wire_value(self) -> String {
        self.families()
            .map(|f| f.wire_name())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// JSON array rendering for the REST body
    /// (`["figure","table","note","formula"]`) — the byte-exact historical
    /// form of `GrobidProfile::tei_coordinates_json`.
    pub fn json_value(self) -> String {
        let items: Vec<String> = self
            .families()
            .map(|f| format!("\"{}\"", f.wire_name()))
            .collect();
        format!("[{}]", items.join(","))
    }
}

impl CoordinateFamily {
    const fn bit(self) -> u8 {
        match self {
            Self::Figure => CoordinateSet::FIGURE,
            Self::Table => CoordinateSet::TABLE,
            Self::Note => CoordinateSet::NOTE,
            Self::Formula => CoordinateSet::FORMULA,
        }
    }
}

impl Default for CoordinateSet {
    /// The legacy default: every sanctioned family (D204 constant).
    fn default() -> Self {
        Self::all()
    }
}

impl Serialize for CoordinateSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.families())
    }
}

impl<'de> Deserialize<'de> for CoordinateSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let families = Vec::<CoordinateFamily>::deserialize(deserializer)?;
        Ok(Self::from_families(families))
    }
}

// ---------------------------------------------------------------------------
// Validated inclusive page window
// ---------------------------------------------------------------------------

/// Validated inclusive page window (`1 <= start <= end`, both ends sent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageWindow {
    start: u32,
    end: u32,
}

impl PageWindow {
    /// Build a window, rejecting invalid ranges.
    pub fn try_new(start: u32, end: u32) -> Result<Self, InvalidPageWindow> {
        if start < 1 || start > end {
            return Err(InvalidPageWindow { start, end });
        }
        Ok(Self { start, end })
    }

    /// First page (inclusive, 1-based).
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Last page (inclusive).
    pub const fn end(self) -> u32 {
        self.end
    }
}

/// Rejection of an invalid inclusive page window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPageWindow {
    /// Rejected start.
    pub start: u32,
    /// Rejected end.
    pub end: u32,
}

impl fmt::Display for InvalidPageWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "page window {}..{} invalid: must satisfy 1 <= start <= end",
            self.start, self.end
        )
    }
}

impl std::error::Error for InvalidPageWindow {}

impl Serialize for PageWindow {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("PageWindow", 2)?;
        state.serialize_field("start", &self.start)?;
        state.serialize_field("end", &self.end)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PageWindow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            start: u32,
            end: u32,
        }
        let repr = Repr::deserialize(deserializer)?;
        Self::try_new(repr.start, repr.end).map_err(|e| de::Error::custom(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// V1 fulltext request dialect + versioned envelope
// ---------------------------------------------------------------------------

/// V1 fulltext request dialect: the closed, lossless expression of the Java
/// GROBID fulltext output-affecting parameters.
///
/// The legacy presets ([`Self::legacy_rich`], [`Self::legacy_structural`],
/// anything [`Self::is_legacy_equivalent`]) reproduce the current
/// `GrobidProfile` wire request exactly; extended fields define a versioned
/// canonical identity that cannot collide with the legacy D204 fingerprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FulltextDialectV1 {
    consolidation: Consolidation,
    coordinates: CoordinateSet,
    raw_citations: bool,
    raw_affiliations: bool,
    segment_sentences: bool,
    emit_ids: bool,
    page_window: Option<PageWindow>,
}

impl FulltextDialectV1 {
    /// Legacy/default dialect: reproduces the current default
    /// (`GrobidProfile::rich()`) fulltext wire request — citations and header
    /// consolidation on, funders off, full coordinates, no extended fields.
    pub fn legacy_rich() -> Self {
        Self::from_parts(Consolidation::legacy(true, true), CoordinateSet::default())
    }

    /// Legacy structural dialect (`GrobidProfile::structural()` wire request).
    pub fn legacy_structural() -> Self {
        Self::from_parts(
            Consolidation::legacy(false, false),
            CoordinateSet::default(),
        )
    }

    /// Dialect from typed consolidation levels + coordinate set, with all
    /// extended fields off (the legacy profile domain).
    pub fn from_parts(consolidation: Consolidation, coordinates: CoordinateSet) -> Self {
        Self {
            consolidation,
            coordinates,
            raw_citations: false,
            raw_affiliations: false,
            segment_sentences: false,
            emit_ids: false,
            page_window: None,
        }
    }

    pub fn consolidation(self) -> Consolidation {
        self.consolidation
    }

    pub fn coordinates(self) -> CoordinateSet {
        self.coordinates
    }

    /// `includeRawCitations` switch.
    pub fn raw_citations(self) -> bool {
        self.raw_citations
    }

    /// `includeRawAffiliations` switch.
    pub fn raw_affiliations(self) -> bool {
        self.raw_affiliations
    }

    /// Sentence segmentation switch.
    pub fn segment_sentences(self) -> bool {
        self.segment_sentences
    }

    /// Element ID emission switch.
    pub fn emit_ids(self) -> bool {
        self.emit_ids
    }

    /// Validated inclusive page window (absent = all pages).
    pub fn page_window(self) -> Option<PageWindow> {
        self.page_window
    }

    pub fn with_consolidation(mut self, consolidation: Consolidation) -> Self {
        self.consolidation = consolidation;
        self
    }

    pub fn with_coordinates(mut self, coordinates: CoordinateSet) -> Self {
        self.coordinates = coordinates;
        self
    }

    pub fn with_raw_citations(mut self, on: bool) -> Self {
        self.raw_citations = on;
        self
    }

    pub fn with_raw_affiliations(mut self, on: bool) -> Self {
        self.raw_affiliations = on;
        self
    }

    pub fn with_segment_sentences(mut self, on: bool) -> Self {
        self.segment_sentences = on;
        self
    }

    pub fn with_emit_ids(mut self, on: bool) -> Self {
        self.emit_ids = on;
        self
    }

    pub fn with_page_window(mut self, window: Option<PageWindow>) -> Self {
        self.page_window = window;
        self
    }

    /// Whether this dialect stays inside the legacy preset domain (only
    /// fields the historical `GrobidProfile` presets could express). Legacy
    /// equivalents own no extended identity — the D204 fingerprint remains
    /// their only identity, so the two namespaces can never collide.
    pub fn is_legacy_equivalent(self) -> bool {
        self.consolidation.header().value() <= 1
            && self.consolidation.citations().value() <= 1
            && self.consolidation.funders().value() == 0
            && self.coordinates == CoordinateSet::default()
            && !self.raw_citations
            && !self.raw_affiliations
            && !self.segment_sentences
            && !self.emit_ids
            && self.page_window.is_none()
    }

    /// Versioned canonical identity for output-affecting extended dialects.
    ///
    /// `None` for legacy-equivalent dialects (identity stays the D204
    /// fingerprint); a deterministic, collision-free `fulltext=v1;...`
    /// string otherwise. Transport tuning (timeout, request/response limits)
    /// has no representation in this vocabulary and therefore cannot enter
    /// the identity.
    pub fn canonical_identity(self) -> Option<String> {
        if self.is_legacy_equivalent() {
            return None;
        }
        let pages = match self.page_window {
            Some(window) => format!("{}..{}", window.start(), window.end()),
            None => "all".to_string(),
        };
        Some(format!(
            "fulltext=v1;consolidateHeader={};consolidateCitations={};consolidateFunders={};\
             teiCoordinates={};rawCitations={};rawAffiliations={};segmentSentences={};emitIds={};pages={}",
            self.consolidation.header(),
            self.consolidation.citations(),
            self.consolidation.funders(),
            self.coordinates.wire_value(),
            u8::from(self.raw_citations),
            u8::from(self.raw_affiliations),
            u8::from(self.segment_sentences),
            u8::from(self.emit_ids),
            pages,
        ))
    }
}

/// Versioned fulltext request envelope: the `V1` discriminant is explicit on
/// the wire (`{"version":"v1",...}`) and unknown versions are rejected at
/// deserialization — the closed-vocabulary evolution seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "lowercase")]
pub enum FulltextRequestVersioned {
    /// V1: the Java GROBID fulltext dialect, expressed losslessly.
    V1(FulltextDialectV1),
}

impl FulltextRequestVersioned {
    /// Wrap a V1 dialect.
    pub fn v1(dialect: FulltextDialectV1) -> Self {
        Self::V1(dialect)
    }

    /// Explicit version label of this envelope.
    pub fn version_label(&self) -> &'static str {
        match self {
            Self::V1(_) => "v1",
        }
    }

    /// The carried dialect.
    pub fn dialect(&self) -> &FulltextDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }

    /// Consume the envelope, returning the dialect.
    pub fn into_dialect(self) -> FulltextDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }
}

// ---------------------------------------------------------------------------
// Closed endpoint / result-kind vocabulary and header/references request
// dialects (S05/T01)
// ---------------------------------------------------------------------------

/// Closed provider-neutral logical endpoint vocabulary of the parse
/// transport. One variant per sanctioned scholarly Java GROBID operation;
/// the set is exhaustive by design — callers can neither invent endpoints
/// nor inject routes (R113).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseEndpoint {
    /// Full-document fulltext extraction.
    Fulltext,
    /// Bibliographic header extraction.
    Header,
    /// Bibliography (`listBibl`) extraction.
    References,
}

impl ParseEndpoint {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fulltext => "fulltext",
            Self::Header => "header",
            Self::References => "references",
        }
    }

    /// Exact static Java GROBID route of this endpoint. The mapping is
    /// closed and static: a caller-controlled route or URL has no input
    /// channel (R113).
    pub const fn static_path(self) -> &'static str {
        match self {
            Self::Fulltext => "/api/processFulltextDocument",
            Self::Header => "/api/processHeaderDocument",
            Self::References => "/api/processReferences",
        }
    }

    /// The closed TEI result kind this endpoint produces.
    pub const fn result_kind(self) -> ParseResultKind {
        match self {
            Self::Fulltext => ParseResultKind::FullDocument,
            Self::Header => ParseResultKind::Header,
            Self::References => ParseResultKind::ListBibl,
        }
    }
}

impl fmt::Display for ParseEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Closed TEI result-kind vocabulary: the TEI shape a sanctioned endpoint
/// returns. The endpoint→result-kind mapping is total and static — partial
/// results are never free-form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseResultKind {
    /// Full `<TEI>` document (fulltext endpoint).
    FullDocument,
    /// Partial TEI: the bibliographic header (header endpoint).
    Header,
    /// Partial TEI: the bibliography `<listBibl>` (references endpoint).
    ListBibl,
}

impl ParseResultKind {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullDocument => "full_document",
            Self::Header => "header",
            Self::ListBibl => "list_bibl",
        }
    }
}

impl fmt::Display for ParseResultKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// V1 header request dialect: the closed, output-affecting control set of
/// the header endpoint — `consolidateHeader` inside its sanctioned range
/// plus the `includeRawAffiliations` switch. Fulltext-only controls
/// (coordinates, pages, sentences, IDs), references-only controls and
/// transport tuning have no field and cannot cross endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderDialectV1 {
    consolidate_header: HeaderConsolidation,
    include_raw_affiliations: bool,
}

impl HeaderDialectV1 {
    /// Build from a validated consolidation level and the raw-affiliations
    /// switch.
    pub const fn new(
        consolidate_header: HeaderConsolidation,
        include_raw_affiliations: bool,
    ) -> Self {
        Self {
            consolidate_header,
            include_raw_affiliations,
        }
    }

    /// Legacy profile mapped onto the header axis ONLY: the profile's
    /// `consolidateHeader` level; raw affiliations off (legacy profiles own
    /// no raw switches).
    pub fn from_legacy_profile(profile: &GrobidProfile) -> Self {
        Self::new(profile.consolidation().header(), false)
    }

    /// Rich-equivalent header dialect (`consolidateHeader=1`).
    pub fn legacy_rich() -> Self {
        Self::new(
            HeaderConsolidation::new(1).expect("1 is inside the sanctioned header range"),
            false,
        )
    }

    /// Structural-equivalent header dialect (`consolidateHeader=0`).
    pub fn legacy_structural() -> Self {
        Self::new(
            HeaderConsolidation::new(0).expect("0 is inside the sanctioned header range"),
            false,
        )
    }

    /// The `consolidateHeader` level.
    pub const fn consolidate_header(self) -> HeaderConsolidation {
        self.consolidate_header
    }

    /// The `includeRawAffiliations` switch.
    pub const fn include_raw_affiliations(self) -> bool {
        self.include_raw_affiliations
    }

    /// Flip the `includeRawAffiliations` switch.
    pub fn with_include_raw_affiliations(mut self, on: bool) -> Self {
        self.include_raw_affiliations = on;
        self
    }

    /// Versioned canonical identity of the effective header request —
    /// deterministic and collision-free against every other endpoint
    /// namespace (`fulltext=v1;…`, `references=v1;…`) and every legacy D204
    /// fingerprint. Transport tuning and locations have no representation.
    pub fn canonical_identity(self) -> String {
        format!(
            "header=v1;consolidateHeader={};rawAffiliations={}",
            self.consolidate_header.value(),
            u8::from(self.include_raw_affiliations),
        )
    }
}

/// V1 references request dialect: the closed, output-affecting control set
/// of the references endpoint — `consolidateCitations` inside its
/// sanctioned range plus the `includeRawCitations` switch. Fulltext- and
/// header-only controls and transport tuning have no field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencesDialectV1 {
    consolidate_citations: CitationsConsolidation,
    include_raw_citations: bool,
}

impl ReferencesDialectV1 {
    /// Build from a validated consolidation level and the raw-citations
    /// switch.
    pub const fn new(
        consolidate_citations: CitationsConsolidation,
        include_raw_citations: bool,
    ) -> Self {
        Self {
            consolidate_citations,
            include_raw_citations,
        }
    }

    /// Legacy profile mapped onto the citations axis ONLY: the profile's
    /// `consolidateCitations` level; raw citations off (legacy profiles own
    /// no raw switches).
    pub fn from_legacy_profile(profile: &GrobidProfile) -> Self {
        Self::new(profile.consolidation().citations(), false)
    }

    /// Rich-equivalent references dialect (`consolidateCitations=1`).
    pub fn legacy_rich() -> Self {
        Self::new(
            CitationsConsolidation::new(1).expect("1 is inside the sanctioned citations range"),
            false,
        )
    }

    /// Structural-equivalent references dialect (`consolidateCitations=0`).
    pub fn legacy_structural() -> Self {
        Self::new(
            CitationsConsolidation::new(0).expect("0 is inside the sanctioned citations range"),
            false,
        )
    }

    /// The `consolidateCitations` level.
    pub const fn consolidate_citations(self) -> CitationsConsolidation {
        self.consolidate_citations
    }

    /// The `includeRawCitations` switch.
    pub const fn include_raw_citations(self) -> bool {
        self.include_raw_citations
    }

    /// Flip the `includeRawCitations` switch.
    pub fn with_include_raw_citations(mut self, on: bool) -> Self {
        self.include_raw_citations = on;
        self
    }

    /// Versioned canonical identity of the effective references request
    /// (same namespace rules as [`HeaderDialectV1::canonical_identity`]).
    pub fn canonical_identity(self) -> String {
        format!(
            "references=v1;consolidateCitations={};rawCitations={}",
            self.consolidate_citations.value(),
            u8::from(self.include_raw_citations),
        )
    }
}

/// Versioned header request envelope: the `V1` discriminant is explicit on
/// the wire (`{"version":"v1",...}`); unknown versions, unknown fields and
/// foreign knobs are rejected at deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "lowercase")]
pub enum HeaderRequestVersioned {
    /// V1: the Java GROBID header dialect, expressed losslessly.
    V1(HeaderDialectV1),
}

impl HeaderRequestVersioned {
    /// Wrap a V1 header dialect.
    pub fn v1(dialect: HeaderDialectV1) -> Self {
        Self::V1(dialect)
    }

    /// Explicit version label of this envelope.
    pub fn version_label(&self) -> &'static str {
        match self {
            Self::V1(_) => "v1",
        }
    }

    /// The carried header dialect.
    pub fn dialect(&self) -> &HeaderDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }

    /// Consume the envelope, returning the header dialect.
    pub fn into_dialect(self) -> HeaderDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }
}

/// Versioned references request envelope (same evolution seam as the
/// fulltext and header envelopes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "lowercase")]
pub enum ReferencesRequestVersioned {
    /// V1: the Java GROBID references dialect, expressed losslessly.
    V1(ReferencesDialectV1),
}

impl ReferencesRequestVersioned {
    /// Wrap a V1 references dialect.
    pub fn v1(dialect: ReferencesDialectV1) -> Self {
        Self::V1(dialect)
    }

    /// Explicit version label of this envelope.
    pub fn version_label(&self) -> &'static str {
        match self {
            Self::V1(_) => "v1",
        }
    }

    /// The carried references dialect.
    pub fn dialect(&self) -> &ReferencesDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }

    /// Consume the envelope, returning the references dialect.
    pub fn into_dialect(self) -> ReferencesDialectV1 {
        match self {
            Self::V1(dialect) => dialect,
        }
    }
}

// ---------------------------------------------------------------------------
// Closed typed failure vocabulary
// ---------------------------------------------------------------------------

/// Closed failure classification for the fulltext transport. Wire form is
/// stable snake_case; the set is exhaustive by design (R107: machine-
/// classifiable by code/phase/status/retry/limit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// Provider busy/backpressure (HTTP 503) — surfaced honestly, never
    /// retried internally.
    Busy,
    /// Client-side request timeout elapsed.
    Timeout,
    /// Connect/DNS or other transport I/O failure.
    Transport,
    /// Successful response carrying no content (HTTP 204).
    NoContent,
    /// A request/response size bound was exceeded (HTTP 413 or an opt-in
    /// client-enforced bound).
    LimitExceeded,
    /// Provider rejected the request as invalid (HTTP 400).
    InvalidRequest,
    /// Provider-side processing/conversion failed (5xx), or the response
    /// body could not be read/framed.
    ConversionFailed,
    /// Uninterpretable HTTP-layer outcome (unexpected status/framing).
    InvalidResponse,
}

impl FailureCode {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Timeout => "timeout",
            Self::Transport => "transport",
            Self::NoContent => "no_content",
            Self::LimitExceeded => "limit_exceeded",
            Self::InvalidRequest => "invalid_request",
            Self::ConversionFailed => "conversion_failed",
            Self::InvalidResponse => "invalid_response",
        }
    }
}

impl fmt::Display for FailureCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Transport phase in which a failure occurred (safe structural metadata;
/// never a URL, path or payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePhase {
    /// Building the multipart request payload.
    MultipartEncode,
    /// Establishing the connection (connect/DNS).
    Connect,
    /// Sending the request bytes.
    Send,
    /// Waiting for / reading the response head.
    AwaitResponse,
    /// Reading the response body.
    ReadBody,
}

impl FailurePhase {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MultipartEncode => "multipart_encode",
            Self::Connect => "connect",
            Self::Send => "send",
            Self::AwaitResponse => "await_response",
            Self::ReadBody => "read_body",
        }
    }
}

impl fmt::Display for FailurePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bounded parsed `Retry-After` metadata (whole seconds, or an honest
/// `unparsable` marker for HTTP-date forms). Never carries the raw header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryAfter {
    /// Provider-advertised delay in whole seconds.
    Seconds(u64),
    /// Present but not a plain seconds value (e.g. HTTP-date form).
    Unparsable,
}

impl RetryAfter {
    /// Bounded parse of a `Retry-After` header value: whitespace-tolerant
    /// decimal seconds (max 10 digits) → [`RetryAfter::Seconds`]; any other
    /// non-empty value → [`RetryAfter::Unparsable`]; empty → `None`.
    pub fn parse_header(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.len() > 10 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
            return Some(Self::Unparsable);
        }
        Some(Self::Seconds(trimmed.parse::<u64>().ok()?))
    }
}

impl fmt::Display for RetryAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seconds(seconds) => write!(f, "seconds={seconds}"),
            Self::Unparsable => f.write_str("unparsable"),
        }
    }
}

/// Which enforced bound produced a [`FailureCode::LimitExceeded`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    /// Outbound request payload exceeded a bound.
    RequestBytes,
    /// Inbound response body exceeded an opt-in bound.
    ResponseBytes,
}

impl LimitKind {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestBytes => "request_bytes",
            Self::ResponseBytes => "response_bytes",
        }
    }
}

impl fmt::Display for LimitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Closed, serializable transport failure with a stable schema version.
///
/// Structured fields carry only safe status / phase / limit / retry metadata.
/// There is deliberately no free-form message, no cause chain and no payload
/// field: raw PDF/TEI bytes, response bodies, URL queries, env values and
/// physical paths cannot leak because they have nowhere to live. `Display`
/// and `Debug` are deterministic redactions of the safe metadata (pinned by
/// `tests/transport_contract.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParseBackendFailure {
    schema_version: u32,
    code: FailureCode,
    phase: FailurePhase,
    status: Option<u16>,
    retry_after: Option<RetryAfter>,
    limit: Option<LimitKind>,
}

impl ParseBackendFailure {
    /// Provider busy/backpressure (HTTP 503) with bounded Retry-After metadata.
    pub fn busy(status: u16, retry_after: Option<RetryAfter>) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::Busy,
            phase: FailurePhase::AwaitResponse,
            status: Some(status),
            retry_after,
            limit: None,
        }
    }

    /// Client-side timeout in `phase`.
    pub fn timeout(phase: FailurePhase) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::Timeout,
            phase,
            status: None,
            retry_after: None,
            limit: None,
        }
    }

    /// Transport I/O failure (connect/DNS/other) in `phase`.
    pub fn transport(phase: FailurePhase) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::Transport,
            phase,
            status: None,
            retry_after: None,
            limit: None,
        }
    }

    /// Successful response with no content (HTTP 204).
    pub fn no_content() -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::NoContent,
            phase: FailurePhase::AwaitResponse,
            status: Some(204),
            retry_after: None,
            limit: None,
        }
    }

    /// Enforced bound: `kind` derives the phase (request → send, response →
    /// read body); `status` is `Some(413)` for provider-enforced bounds and
    /// `None` for opt-in client-enforced bounds.
    pub fn limit(kind: LimitKind, status: Option<u16>) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::LimitExceeded,
            phase: match kind {
                LimitKind::RequestBytes => FailurePhase::Send,
                LimitKind::ResponseBytes => FailurePhase::ReadBody,
            },
            status,
            retry_after: None,
            limit: Some(kind),
        }
    }

    /// Provider rejected the request as invalid (HTTP 400).
    pub fn invalid_request(status: u16) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::InvalidRequest,
            phase: FailurePhase::AwaitResponse,
            status: Some(status),
            retry_after: None,
            limit: None,
        }
    }

    /// Provider-side processing/conversion failure (Java 5xx) or a response
    /// body that could not be read/framed.
    pub fn conversion(phase: FailurePhase, status: Option<u16>) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::ConversionFailed,
            phase,
            status,
            retry_after: None,
            limit: None,
        }
    }

    /// Uninterpretable HTTP-layer outcome.
    pub fn invalid_response(phase: FailurePhase, status: Option<u16>) -> Self {
        Self {
            schema_version: PARSE_FAILURE_SCHEMA_VERSION,
            code: FailureCode::InvalidResponse,
            phase,
            status,
            retry_after: None,
            limit: None,
        }
    }

    /// Stable schema version of this failure instance.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Closed failure classification.
    pub const fn code(&self) -> FailureCode {
        self.code
    }

    /// Phase in which the failure occurred.
    pub const fn phase(&self) -> FailurePhase {
        self.phase
    }

    /// HTTP status when the provider answered with one.
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// Bounded Retry-After metadata when advertised.
    pub const fn retry_after(&self) -> Option<RetryAfter> {
        self.retry_after
    }

    /// Enforced bound when the failure is limit-shaped.
    pub const fn enforced_limit(&self) -> Option<LimitKind> {
        self.limit
    }

    /// The single explicit compatibility conversion boundary: typed failure
    /// → historical `ParserError`, fail-closed. Availability-class failures
    /// (busy, timeout, transport) map to [`ParserError::Unavailable`];
    /// everything else (no content, limits, invalid request/response,
    /// conversion) maps to [`ParserError::ParseFailed`]. The rendered string
    /// carries only the stable code/safe metadata — never a cause, body,
    /// source or payload.
    pub fn to_parser_error(&self) -> ParserError {
        let detail = self.to_string();
        match self.code {
            FailureCode::Busy | FailureCode::Timeout | FailureCode::Transport => {
                ParserError::Unavailable(detail)
            }
            _ => ParserError::ParseFailed(detail),
        }
    }
}

impl fmt::Display for ParseBackendFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let or_dash = |value: Option<String>| value.unwrap_or_else(|| "-".to_string());
        write!(
            f,
            "parse backend failure: code={} phase={} status={} limit={} retry_after={}",
            self.code,
            self.phase,
            or_dash(self.status.map(|s| s.to_string())),
            or_dash(self.limit.map(|l| l.to_string())),
            or_dash(self.retry_after.map(|r| r.to_string())),
        )
    }
}

impl Serialize for ParseBackendFailure {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseBackendFailure", 6)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("phase", &self.phase)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("retry_after", &self.retry_after)?;
        state.serialize_field("limit", &self.limit)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseBackendFailure {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "snake_case")]
        struct Repr {
            schema_version: u32,
            code: FailureCode,
            phase: FailurePhase,
            status: Option<u16>,
            retry_after: Option<RetryAfter>,
            limit: Option<LimitKind>,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_FAILURE_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported parse failure schema version {} (supported: {PARSE_FAILURE_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            code: repr.code,
            phase: repr.phase,
            status: repr.status,
            retry_after: repr.retry_after,
            limit: repr.limit,
        })
    }
}
