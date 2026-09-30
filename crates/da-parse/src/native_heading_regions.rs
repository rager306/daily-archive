//! Additive heading-aware region assignment over the projected word plane.
//!
//! The S03 region walk (`segment_document_full`) promotes a region when a
//! *single token* equals a marker string, and it descends from a lattice that,
//! for the positioned donor, collapses every page into one block and one line.
//! Measured on the declared three-document corpus that walk is both too strict
//! (a heading carried inside a multi-word span is never matched) and too loose
//! (a standalone marker word anywhere, e.g. `Methods` inside a reference, flips
//! the rest of the document) — see S04/T04 blocker WP-02. This module is the
//! additive, heading-aware replacement: it derives visual lines from geometry
//! and enters a region only on a *heading line* with recorded evidence.
//!
//! ## Rules
//!
//! * **Lines come from geometry, not from the lattice.** The lattice carries no
//!   line structure for this donor, so lines are grouped from word boxes by
//!   baseline proximity and vertical overlap. The grouping is a deterministic
//!   function of the projection.
//! * **A region is entered only through evidence.** The preamble is the
//!   [`FullSemanticRegion::Header`] region by construction (`PreambleDefault`);
//!   every other region is entered by a heading line — whole-line or
//!   ordinal-stripped vocabulary equality, a line that *opens* with a span that
//!   is exactly a vocabulary entry (`LeadingSpanLine`), or, only for the first
//!   body section, a short line that is font-prominent against the document's
//!   own measured body font size (`ProminentSectionHeading`). Every transition
//!   records its level, its cues and its line.
//! * **An absent region is typed, never implied.** A region that was never
//!   entered becomes [`NativeHeadingAbsence::RegionNeverEntered`]; it is never
//!   silently folded into the header.
//! * **The vocabulary is a superset of S03's.** The shared entries resolve
//!   through the S03 `full_marker` matcher, so the two vocabularies cannot
//!   drift; `abstract`/`abstracts` are the only additions and they exist
//!   because the abstract is the first reliably marked body section.
//! * **No text reaches a receipt.** Line text is private, reachable only
//!   through [`NativeVisualLine::text`]; `Debug` prints `<redacted>` and the
//!   serde projection carries only its digest.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no TEI/Java type is an input. The S03 wire formats and
//! the frozen carriers are not touched, and nothing here is spliced into the
//! Java-authoritative parse route.

use crate::{
    FullSemanticRegion, LayoutBBox, LayoutDocument, LayoutLattice,
    NATIVE_WORD_PROJECTION_SCHEMA_VERSION, NativeWordProjection,
};
use serde::{Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Schema identity of the additive heading-region surface.
pub const NATIVE_HEADING_REGIONS_SCHEMA_VERSION: &str = "da-parse.native-heading-regions/v1";

/// Upper bound on derived visual lines.
pub const MAX_NATIVE_VISUAL_LINES: usize = 200_000;

/// The nine semantic regions, in the S03 declaration order.
pub const HEADING_REGION_ORDER: [FullSemanticRegion; 9] = [
    FullSemanticRegion::Header,
    FullSemanticRegion::Body,
    FullSemanticRegion::References,
    FullSemanticRegion::Annex,
    FullSemanticRegion::Acknowledgement,
    FullSemanticRegion::Toc,
    FullSemanticRegion::Funding,
    FullSemanticRegion::Availability,
    FullSemanticRegion::PageFurniture,
];

/// Bounds of the heading-aware walk. `baseline_tolerance_pt` is the vertical
/// distance within which two word boxes are read as one line;
/// `heading_max_words` bounds a heading line; `heading_min_font_size_milli_pt`
/// is the absolute floor an S03-configured heading would clear, reported as
/// evidence rather than used as the transition rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeadingRegionConfig {
    pub baseline_tolerance_pt: f64,
    pub heading_max_words: u32,
    pub heading_min_font_size_milli_pt: u32,
    pub vertical_overlap_fraction: f64,
}

impl Default for HeadingRegionConfig {
    fn default() -> Self {
        Self {
            baseline_tolerance_pt: 2.0,
            heading_max_words: 12,
            heading_min_font_size_milli_pt: 11_000,
            vertical_overlap_fraction: 0.5,
        }
    }
}

/// How a line matched the closed heading vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingMatch {
    /// The header preamble, before any heading line. Not a match at all: it is
    /// the honest name of "the document starts here".
    PreambleDefault,
    /// The normalized whole line equals a vocabulary entry.
    NormalizedLine,
    /// The line equals a vocabulary entry once a leading enumeration
    /// (`3.`, `3.1`, `IV)`) and trailing punctuation are stripped.
    OrdinalStrippedLine,
    /// The line *opens* with a span that is exactly a vocabulary entry and the
    /// line looks like a heading. A heading often shares its baseline with the
    /// first entry that follows it, so the whole line never equals the marker;
    /// the leading span still does, and that is the evidence.
    LeadingSpanLine,
    /// A short, font-prominent line that is not a vocabulary entry: the first
    /// body section of a paper whose heading is not in the vocabulary.
    ProminentSectionHeading,
}

/// Why and how a region was entered.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeadingEvidence {
    pub page_ordinal: u32,
    pub line_ordinal: u32,
    pub region: FullSemanticRegion,
    pub matched_by: HeadingMatch,
    pub word_count: u32,
    pub max_font_size_milli_pt: Option<u32>,
    pub font_above_absolute_floor: bool,
    pub bold: bool,
    /// True when the line's font size exceeded the document's measured body
    /// font size.
    pub font_above_body_size: bool,
}

/// One derived visual line.
#[derive(Clone, PartialEq)]
pub struct NativeVisualLine {
    pub page_ordinal: u32,
    pub line_ordinal: u32,
    pub bbox: LayoutBBox,
    pub word_count: u32,
    pub max_font_size_milli_pt: Option<u32>,
    pub bold: bool,
    /// Digest of the normalized line text.
    pub text_digest: String,
    /// The vocabulary region this line's normalized text matched, if any.
    pub marker: Option<FullSemanticRegion>,
    /// The level at which the vocabulary matched, if it did.
    pub matched_by: Option<HeadingMatch>,
    text: String,
}

impl NativeVisualLine {
    /// The normalized line text. Deliberately the only path to it: `Debug` and
    /// the serde projection never carry it.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for NativeVisualLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeVisualLine")
            .field("page_ordinal", &self.page_ordinal)
            .field("line_ordinal", &self.line_ordinal)
            .field("bbox", &self.bbox)
            .field("word_count", &self.word_count)
            .field("max_font_size_milli_pt", &self.max_font_size_milli_pt)
            .field("bold", &self.bold)
            .field("text_digest", &self.text_digest)
            .field("marker", &self.marker)
            .field("matched_by", &self.matched_by)
            .field("text", &"<redacted>")
            .finish()
    }
}

impl Serialize for NativeVisualLine {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NativeVisualLine", 9)?;
        state.serialize_field("page_ordinal", &self.page_ordinal)?;
        state.serialize_field("line_ordinal", &self.line_ordinal)?;
        state.serialize_field("bbox", &self.bbox)?;
        state.serialize_field("word_count", &self.word_count)?;
        state.serialize_field("max_font_size_milli_pt", &self.max_font_size_milli_pt)?;
        state.serialize_field("bold", &self.bold)?;
        state.serialize_field("text_digest", &self.text_digest)?;
        state.serialize_field("marker", &self.marker)?;
        state.serialize_field("matched_by", &self.matched_by)?;
        state.end()
    }
}

/// One projected word's region, addressed by its parent span and word ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeadingAssignment {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub word_ordinal: u32,
    pub line_ordinal: u32,
    pub region: FullSemanticRegion,
}

/// Typed absence in the heading-aware outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum NativeHeadingAbsence {
    /// No heading line and no prominent section line ever entered this region.
    RegionNeverEntered { region: FullSemanticRegion },
    /// A page carried no projected words at all.
    PageWithoutWords { page_ordinal: u32 },
}

/// Provenance identity of the heading-aware outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeadingIdentity {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
    /// The word projection this assignment was derived from.
    pub word_projection_order_digest: String,
}

/// Deterministic heading-aware region assignment for one document.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeadingRegionOutcome {
    pub schema_version: String,
    pub identity: NativeHeadingIdentity,
    /// The document's measured body font size: the modal word font size, ties
    /// resolved towards the larger size. `None` when no word carried one.
    pub body_font_size_milli_pt: Option<u32>,
    pub lines: Vec<NativeVisualLine>,
    pub assignments: Vec<NativeHeadingAssignment>,
    /// One entry per region that was entered, in reading order.
    pub evidences: Vec<HeadingEvidence>,
    /// Regions never entered and pages without words, in declaration order.
    pub absences: Vec<NativeHeadingAbsence>,
    pub order_digest: String,
}

impl NativeHeadingRegionOutcome {
    /// The evidence for a region, when the region was entered.
    pub fn evidence(&self, region: FullSemanticRegion) -> Option<&HeadingEvidence> {
        self.evidences.iter().find(|entry| entry.region == region)
    }

    /// The typed absence for a region, when it was never entered.
    pub fn absence(&self, region: FullSemanticRegion) -> Option<&NativeHeadingAbsence> {
        self.absences.iter().find(|absence| {
            matches!(absence, NativeHeadingAbsence::RegionNeverEntered { region: r } if *r == region)
        })
    }

    /// Canonical redaction-safe digest over the serialized outcome.
    pub fn digest(&self) -> Result<String, NativeHeadingRegionError> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| NativeHeadingRegionError::Serialization)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

/// Fail-closed refusal vocabulary of the heading-aware walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeHeadingRegionError {
    /// The config carries a non-finite or degenerate bound.
    InvalidConfig,
    /// The layout document failed its own contract.
    LayoutInvalid,
    /// The lattice does not validate against the layout document.
    LatticeInvalid,
    /// The projection was built for another document, provider, source or
    /// schema.
    IdentityMismatch(&'static str),
    /// The projection disagrees with its own identity counts.
    ProjectionInconsistent,
    /// A projected word was not covered by exactly one assignment.
    UncoveredWord { page: u32, source: u32, word: u32 },
    /// The walk assigned one word identity twice.
    DuplicateWordAssignment { page: u32, source: u32, word: u32 },
    /// A derived line box was inverted or non-finite.
    LineGeometryInvalid { page: u32, line: u32 },
    /// More derived lines than the surface bounds.
    TooManyLines { count: usize },
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeHeadingRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native heading region assignment rejected input: {self:?}"
        )
    }
}

impl std::error::Error for NativeHeadingRegionError {}

/// A word's derived line input. The text stays private and `Debug` prints
/// `<redacted>`, so a panic on this path cannot carry the article text.
#[derive(Clone, PartialEq)]
struct LineWord {
    page_ordinal: u32,
    source_ordinal: u32,
    word_ordinal: u32,
    bbox: LayoutBBox,
    font_size_milli_pt: Option<u32>,
    bold: bool,
    text: String,
}

impl fmt::Debug for LineWord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LineWord")
            .field("page_ordinal", &self.page_ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("word_ordinal", &self.word_ordinal)
            .field("bbox", &self.bbox)
            .field("font_size_milli_pt", &self.font_size_milli_pt)
            .field("bold", &self.bold)
            .field("text", &"<redacted>")
            .finish()
    }
}

/// Assign heading-aware regions to every projected word.
///
/// The outcome covers the whole projection: every word carries exactly one
/// region, every region carries its entry evidence, and a region that was never
/// entered is a typed absence rather than an implied default.
pub fn assign_heading_regions(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    projection: &NativeWordProjection,
    config: HeadingRegionConfig,
) -> Result<NativeHeadingRegionOutcome, NativeHeadingRegionError> {
    if !config.baseline_tolerance_pt.is_finite()
        || config.baseline_tolerance_pt < 0.0
        || !config.vertical_overlap_fraction.is_finite()
        || !(0.0..=1.0).contains(&config.vertical_overlap_fraction)
        || config.heading_max_words == 0
        || config.heading_min_font_size_milli_pt == 0
    {
        return Err(NativeHeadingRegionError::InvalidConfig);
    }
    document
        .validate()
        .map_err(|_| NativeHeadingRegionError::LayoutInvalid)?;
    lattice
        .validate_against(document)
        .map_err(|_| NativeHeadingRegionError::LatticeInvalid)?;

    if projection.schema_version != NATIVE_WORD_PROJECTION_SCHEMA_VERSION {
        return Err(NativeHeadingRegionError::IdentityMismatch("schema_version"));
    }
    if projection.identity.document_id != document.document_id {
        return Err(NativeHeadingRegionError::IdentityMismatch("document_id"));
    }
    if projection.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeHeadingRegionError::IdentityMismatch(
            "source_pdf_sha256",
        ));
    }
    if lattice.identity.document_id != document.document_id
        || lattice.identity.source_pdf_sha256 != document.source_pdf_sha256
    {
        return Err(NativeHeadingRegionError::IdentityMismatch("layout_lattice"));
    }
    if projection.identity.parent_count as usize != projection.parents.len()
        || projection.identity.word_count as usize
            != projection
                .parents
                .iter()
                .map(|parent| parent.words.len())
                .sum::<usize>()
    {
        return Err(NativeHeadingRegionError::ProjectionInconsistent);
    }

    let config_fingerprint = digest_parts(&[serde_json::to_string(&config)
        .map_err(|_| NativeHeadingRegionError::Serialization)?
        .as_str()]);
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeHeadingRegionError::Serialization)?;

    let mut words: Vec<LineWord> = Vec::new();
    for parent in &projection.parents {
        for word in &parent.words {
            words.push(LineWord {
                page_ordinal: word.page_ordinal,
                source_ordinal: word.source_ordinal,
                word_ordinal: word.word_ordinal,
                bbox: word
                    .geometry
                    .estimated_bbox
                    .unwrap_or(word.geometry.parent_bbox),
                font_size_milli_pt: word.style.font_size_milli_pt,
                bold: word.style.bold,
                text: word.text().to_owned(),
            });
        }
    }

    let body_font_size_milli_pt = modal_font_size(&words);
    let mut parent_norms: BTreeMap<(u32, u32), String> = BTreeMap::new();
    for parent in &projection.parents {
        let joined = parent
            .words
            .iter()
            .map(|word| word.text())
            .collect::<Vec<&str>>()
            .join(" ");
        parent_norms.insert(
            (parent.page_ordinal, parent.source_ordinal),
            normalize(&joined),
        );
    }
    let derived = derive_visual_lines(&words, &parent_norms, body_font_size_milli_pt, config)?;
    if derived.len() > MAX_NATIVE_VISUAL_LINES {
        return Err(NativeHeadingRegionError::TooManyLines {
            count: derived.len(),
        });
    }

    let mut current: Option<FullSemanticRegion> = None;
    let mut evidences: Vec<HeadingEvidence> = Vec::new();
    let mut assignments: Vec<NativeHeadingAssignment> = Vec::new();
    let mut assigned_keys: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
    let mut order_entries: Vec<String> = Vec::new();

    for (global_line, entry) in derived.iter().enumerate() {
        let line = &entry.line;
        let entered = if current.is_none() {
            current = Some(FullSemanticRegion::Header);
            Some(HeadingMatch::PreambleDefault)
        } else if let Some(marker) = line.marker {
            if current != Some(marker) {
                current = Some(marker);
                line.matched_by
            } else {
                None
            }
        } else if current == Some(FullSemanticRegion::Header)
            && global_line > 0
            && line.word_count <= config.heading_max_words
            && line_is_prominent(line, body_font_size_milli_pt)
        {
            current = Some(FullSemanticRegion::Body);
            Some(HeadingMatch::ProminentSectionHeading)
        } else {
            None
        };
        let region = current.unwrap_or(FullSemanticRegion::Header);
        if let Some(matched_by) = entered {
            evidences.push(HeadingEvidence {
                page_ordinal: line.page_ordinal,
                line_ordinal: line.line_ordinal,
                region,
                matched_by,
                word_count: line.word_count,
                max_font_size_milli_pt: line.max_font_size_milli_pt,
                font_above_absolute_floor: line
                    .max_font_size_milli_pt
                    .is_some_and(|size| size >= config.heading_min_font_size_milli_pt),
                bold: line.bold,
                font_above_body_size: line
                    .max_font_size_milli_pt
                    .zip(body_font_size_milli_pt)
                    .is_some_and(|(size, body)| size > body),
            });
        }
        for word in &entry.words {
            let key = (word.page_ordinal, word.source_ordinal, word.word_ordinal);
            if !assigned_keys.insert(key) {
                return Err(NativeHeadingRegionError::DuplicateWordAssignment {
                    page: key.0,
                    source: key.1,
                    word: key.2,
                });
            }
            assignments.push(NativeHeadingAssignment {
                page_ordinal: word.page_ordinal,
                source_ordinal: word.source_ordinal,
                word_ordinal: word.word_ordinal,
                line_ordinal: line.line_ordinal,
                region,
            });
            order_entries.push(format!(
                "{}:{}:{}:{}",
                word.page_ordinal,
                word.source_ordinal,
                word.word_ordinal,
                region_name(region)
            ));
        }
    }

    // Coverage: every projected word carries exactly one region, and every
    // assigned word is one the projection actually carries.
    let projected: BTreeSet<(u32, u32, u32)> = words
        .iter()
        .map(|word| (word.page_ordinal, word.source_ordinal, word.word_ordinal))
        .collect();
    if let Some(missing) = projected.difference(&assigned_keys).next() {
        return Err(NativeHeadingRegionError::UncoveredWord {
            page: missing.0,
            source: missing.1,
            word: missing.2,
        });
    }
    if let Some(extra) = assigned_keys.difference(&projected).next() {
        return Err(NativeHeadingRegionError::UncoveredWord {
            page: extra.0,
            source: extra.1,
            word: extra.2,
        });
    }

    let entered: BTreeSet<FullSemanticRegion> =
        assignments.iter().map(|entry| entry.region).collect();
    let mut absences: Vec<NativeHeadingAbsence> = HEADING_REGION_ORDER
        .iter()
        .filter(|region| !entered.contains(region))
        .map(|region| NativeHeadingAbsence::RegionNeverEntered { region: *region })
        .collect();
    absences.extend(
        document
            .pages
            .iter()
            .filter(|page| {
                !words
                    .iter()
                    .any(|word| word.page_ordinal == page.page_ordinal)
            })
            .map(|page| NativeHeadingAbsence::PageWithoutWords {
                page_ordinal: page.page_ordinal,
            }),
    );

    let order_digest = digest_parts(
        &order_entries
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>(),
    );
    let lines: Vec<NativeVisualLine> = derived.into_iter().map(|entry| entry.line).collect();

    Ok(NativeHeadingRegionOutcome {
        schema_version: NATIVE_HEADING_REGIONS_SCHEMA_VERSION.to_owned(),
        identity: NativeHeadingIdentity {
            schema_version: NATIVE_HEADING_REGIONS_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            provider_id: projection.identity.provider_id.clone(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            lattice_fingerprint,
            config_fingerprint,
            word_projection_order_digest: projection.identity.order_digest.clone(),
        },
        body_font_size_milli_pt,
        lines,
        assignments,
        evidences,
        absences,
        order_digest,
    })
}

/// A derived visual line under construction.
#[derive(Debug, Clone)]
struct PendingLine {
    line_ordinal: u32,
    words: Vec<LineWord>,
    bbox: LayoutBBox,
}

/// A derived visual line together with the words it grouped. The words stay
/// beside the line so the region walk can address them without a second lookup;
/// they are never serialized.
#[derive(Debug, Clone)]
struct DerivedLine {
    line: NativeVisualLine,
    words: Vec<LineWord>,
}

impl PendingLine {
    fn finalize(
        self,
        parent_norms: &BTreeMap<(u32, u32), String>,
        body_font_size_milli_pt: Option<u32>,
    ) -> Result<DerivedLine, NativeHeadingRegionError> {
        let page_ordinal = self.words[0].page_ordinal;
        let word_count = self.words.len() as u32;
        let max_font_size_milli_pt = self
            .words
            .iter()
            .filter_map(|word| word.font_size_milli_pt)
            .max();
        let bold = {
            let bold = self.words.iter().filter(|word| word.bold).count();
            bold * 2 >= self.words.len().max(1)
        };
        let text = normalize_line_text(&self.words);
        let leading = parent_norms
            .get(&(self.words[0].page_ordinal, self.words[0].source_ordinal))
            .map(String::as_str)
            .unwrap_or("");
        let (marker, matched_by) = classify_heading_line(
            &text,
            leading,
            bold,
            max_font_size_milli_pt,
            body_font_size_milli_pt,
        );
        Ok(DerivedLine {
            line: NativeVisualLine {
                page_ordinal,
                line_ordinal: self.line_ordinal,
                bbox: self.bbox,
                word_count,
                max_font_size_milli_pt,
                bold,
                text_digest: digest_text(&text),
                marker,
                matched_by,
                text,
            },
            words: self.words,
        })
    }
}

/// Group words into visual lines by page, baseline proximity and vertical
/// overlap, and classify each line against the heading vocabulary.
fn derive_visual_lines(
    words: &[LineWord],
    parent_norms: &BTreeMap<(u32, u32), String>,
    body_font_size_milli_pt: Option<u32>,
    config: HeadingRegionConfig,
) -> Result<Vec<DerivedLine>, NativeHeadingRegionError> {
    let mut by_page: BTreeMap<u32, Vec<LineWord>> = BTreeMap::new();
    for word in words {
        by_page
            .entry(word.page_ordinal)
            .or_default()
            .push(word.clone());
    }
    let mut out: Vec<DerivedLine> = Vec::new();
    for (page_ordinal, mut page_words) in by_page {
        // Reading order within a page is the baseline, then the left edge; a
        // tie on both falls back to the projection order, which is the
        // document order the donor observed. A heading and the entry below it
        // share a left margin, so the tiebreak must not be geometry: sorting by
        // the right edge would read the short following entry before the
        // heading that names it.
        page_words.sort_by(|a, b| {
            a.bbox
                .y_min
                .partial_cmp(&b.bbox.y_min)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.bbox
                        .x_min
                        .partial_cmp(&b.bbox.x_min)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        let mut pending: Option<PendingLine> = None;
        let mut opened_lines: u32 = 0;
        for word in page_words {
            let joins = pending.as_ref().is_some_and(|line| {
                let anchor = line.words[0].bbox;
                let tolerance =
                    (word.bbox.y_min - anchor.y_min).abs() <= config.baseline_tolerance_pt;
                let overlap = overlap_fraction(word.bbox, line.bbox);
                tolerance || overlap >= config.vertical_overlap_fraction
            });
            if joins {
                let line = pending.as_mut().expect("pending line exists");
                line.bbox = union_bbox(line.bbox, word.bbox).ok_or(
                    NativeHeadingRegionError::LineGeometryInvalid {
                        page: page_ordinal,
                        line: line.line_ordinal,
                    },
                )?;
                line.words.push(word);
            } else {
                if let Some(line) = pending.take() {
                    out.push(line.finalize(parent_norms, body_font_size_milli_pt)?);
                }
                pending = Some(PendingLine {
                    line_ordinal: opened_lines,
                    bbox: word.bbox,
                    words: vec![word],
                });
                opened_lines += 1;
            }
        }
        if let Some(line) = pending.take() {
            out.push(line.finalize(parent_norms, body_font_size_milli_pt)?);
        }
    }
    Ok(out)
}

fn overlap_fraction(candidate: LayoutBBox, line: LayoutBBox) -> f64 {
    let top = candidate.y_min.max(line.y_min);
    let bottom = candidate.y_max.min(line.y_max);
    let overlap = (bottom - top).max(0.0);
    let height = (candidate.y_max - candidate.y_min).min(line.y_max - line.y_min);
    if height <= 0.0 { 0.0 } else { overlap / height }
}

fn union_bbox(left: LayoutBBox, right: LayoutBBox) -> Option<LayoutBBox> {
    LayoutBBox::new(
        left.x_min.min(right.x_min),
        left.y_min.min(right.y_min),
        left.x_max.max(right.x_max),
        left.y_max.max(right.y_max),
    )
    .ok()
}

/// The document's measured body font size: the modal word font size, ties
/// resolved towards the larger size.
fn modal_font_size(words: &[LineWord]) -> Option<u32> {
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for size in words.iter().filter_map(|word| word.font_size_milli_pt) {
        *counts.entry(size).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(size, count)| (*count, *size))
        .map(|(size, _)| size)
}

/// A line is prominent when it is bold or set larger than the document's own
/// body font. The comparison is against a measurement, not a constant.
fn line_is_prominent(line: &NativeVisualLine, body_font_size_milli_pt: Option<u32>) -> bool {
    if line.bold {
        return true;
    }
    line.max_font_size_milli_pt
        .zip(body_font_size_milli_pt)
        .is_some_and(|(size, body)| size > body)
}

/// Normalize a derived line's words into the vocabulary comparison form, and
/// classify it.
///
/// Three levels of evidence, in decreasing strength: the whole normalized line
/// is a vocabulary entry; the line equals one once its leading enumeration and
/// trailing punctuation are stripped; or the line *opens* with a span that is
/// exactly a vocabulary entry while the line itself looks like a heading. The
/// last level exists because a heading commonly shares its baseline with the
/// first entry that follows it, so whole-line equality alone would miss it —
/// and because it demands an exact leading span and a heading-looking line, it
/// does not reopen the inverse defect (a marker word *inside* a line never
/// promotes a region).
fn classify_heading_line(
    text: &str,
    leading_span: &str,
    bold: bool,
    max_font_size_milli_pt: Option<u32>,
    body_font_size_milli_pt: Option<u32>,
) -> (Option<FullSemanticRegion>, Option<HeadingMatch>) {
    let normalized = normalize(text);
    if let Some(region) = heading_marker(&normalized) {
        return (Some(region), Some(HeadingMatch::NormalizedLine));
    }
    let stripped = strip_leading_enumeration(&normalized);
    if stripped != normalized
        && let Some(region) = heading_marker(&stripped)
    {
        return (Some(region), Some(HeadingMatch::OrdinalStrippedLine));
    }
    let prominent = bold
        || max_font_size_milli_pt
            .zip(body_font_size_milli_pt)
            .is_some_and(|(size, body)| size > body);
    if prominent {
        let leading = normalize(leading_span);
        if let Some(region) = heading_marker(&leading) {
            return (Some(region), Some(HeadingMatch::LeadingSpanLine));
        }
        let leading_stripped = strip_leading_enumeration(&leading);
        if leading_stripped != leading
            && let Some(region) = heading_marker(&leading_stripped)
        {
            return (Some(region), Some(HeadingMatch::LeadingSpanLine));
        }
    }
    (None, None)
}

/// Join the words of a line and normalize: lowercase, single spaces, no
/// trailing punctuation.
fn normalize_line_text(words: &[LineWord]) -> String {
    let mut text = String::new();
    for word in words {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&word.text);
    }
    normalize(&text)
}

fn normalize(text: &str) -> String {
    let lowered = text.to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<&str>>().join(" ");
    collapsed
        .trim_end_matches(['.', ':', ';', ','])
        .trim()
        .to_owned()
}

/// Drop a leading section enumeration (`3`, `3.`, `3.1`, `IV)`) when another
/// token remains.
fn strip_leading_enumeration(text: &str) -> String {
    let mut tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() < 2 {
        return text.to_owned();
    }
    if is_enumeration_token(tokens[0]) {
        tokens.remove(0);
        return tokens.join(" ");
    }
    text.to_owned()
}

fn is_enumeration_token(token: &str) -> bool {
    let trimmed = token.trim_end_matches(['.', ')', ':', ',']);
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit())
        || trimmed.contains('.') && trimmed.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return true;
    }
    const ROMAN: [&str; 12] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii",
    ];
    ROMAN.contains(&trimmed)
}

/// The closed heading vocabulary: the S03 marker matcher plus the two abstract
/// spellings. Every shared entry resolves through `full_marker`, so the two
/// vocabularies cannot drift.
fn heading_marker(normalized: &str) -> Option<FullSemanticRegion> {
    match normalized {
        "abstract" | "abstracts" => Some(FullSemanticRegion::Body),
        other => crate::native_segmentation::full_marker(other),
    }
}

fn region_name(region: FullSemanticRegion) -> &'static str {
    match region {
        FullSemanticRegion::Header => "header",
        FullSemanticRegion::Body => "body",
        FullSemanticRegion::References => "references",
        FullSemanticRegion::Annex => "annex",
        FullSemanticRegion::Acknowledgement => "acknowledgement",
        FullSemanticRegion::Toc => "toc",
        FullSemanticRegion::Funding => "funding",
        FullSemanticRegion::Availability => "availability",
        FullSemanticRegion::PageFurniture => "page_furniture",
    }
}

fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn digest_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}
