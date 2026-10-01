//! Additive native section and reading-order surface (S05/T02).
//!
//! S03 (`segment_document_full`) publishes *which region* each admitted token
//! belongs to and the region runs it forms; the heading-aware walk
//! ([`crate::assign_heading_regions`]) publishes *why* a region was entered and
//! the visual lines it derived; the word-level region carrier
//! ([`crate::build_native_word_region_carrier`]) joins the two into ordered,
//! redaction-safe region token runs. What none of them publishes is the
//! document's *section structure* and reading-order identity that the reviewed
//! segmentation oracle measures. This module is that missing band.
//!
//! ## What it derives
//!
//! * [`NativeSectionOutcome`] — sections inside the two section-bearing regions
//!   ([`SECTION_PARENT_REGIONS`], the `body`/`annex` pair the segmentation
//!   contract names), each with a depth, a parent section, and a boundary
//!   digest; plus typed absences for the regions that were never entered or
//!   carried no heading evidence.
//! * [`NativeReadingOrderOutcome`] — the admitted order digest, the ordered
//!   region rows (region + start/end + token count), per-page spans, and the
//!   column shifts, reusing [`ColumnShift`] from
//!   [`crate::native_fulltext_reading_order`] so the shift fact has one shape.
//!
//! ## Vocabulary discipline
//!
//! There is **no new regional vocabulary**: every region is a
//! [`FullSemanticRegion`] and the derivation never invents a label. There is
//! **no TEI/Java type** anywhere in the input or the output. Section depths come
//! from the heading line's own leading ordinal (`3.1` → depth 2), never from a
//! classifier.
//!
//! ## No silent skip
//!
//! A lane the native segmenter never measured is a *typed refusal*, not a zero:
//! [`native_lane_disposition`] classifies every lane id and
//! [`evaluate_native_section_rows`] turns a non-measured lane into
//! [`NativeSectionRowVerdict::NotAssessable`] rather than an agreement. An
//! empty section run is a typed [`NativeSectionAbsence`], and a region the
//! heading walk never entered keeps its `RegionNeverEntered` absence.
//!
//! ## Oracle separation (why this is not the T01 body verdict)
//!
//! The reviewed segmentation oracle is **boundary/count based**: its rows are
//! `(region, start, end)` tuples. The character-coverage oracle of S05/T01 is a
//! *different* surface with a *different* verdict vocabulary
//! (`GranularityOnly`/`ContentLoss`/`NotAssessable`). The two must not be
//! conflated: [`NATIVE_SECTION_ROW_VERDICT_VOCABULARY`] is closed and contains
//! no character-level term, [`NativeSectionRegionRow`] carries no character or
//! lexical-denominator field, and the contract test proves both structurally.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface. Nothing here is spliced into the Java-authoritative parse
//! route; `import_eligible` stays false and no threshold is accepted.

use crate::native_fulltext_reading_order::ColumnShift;
use crate::native_heading_regions::{
    HeadingEvidence, HeadingMatch, NATIVE_HEADING_REGIONS_SCHEMA_VERSION,
    NativeHeadingRegionOutcome, NativeVisualLine,
};
use crate::native_region_input::{NATIVE_REGION_INPUT_SCHEMA_VERSION, NativeRegionCarrier};
use crate::{FullLocation, FullSemanticRegion, LayoutLattice};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the additive section / reading-order surface.
pub const NATIVE_SECTION_READING_ORDER_SCHEMA_VERSION: &str =
    "da-parse.native-section-reading-order/v1";

/// The closed nine-region vocabulary, in the S03 declaration order.
pub const NATIVE_SECTION_REGION_COUNT: usize = 9;

/// The two section-bearing regions (`document_segmentation.yaml`
/// `annotations.section_parent_regions`). A section can only live here.
pub const SECTION_PARENT_REGIONS: [FullSemanticRegion; 2] =
    [FullSemanticRegion::Body, FullSemanticRegion::Annex];

/// Upper bound on emitted section runs.
pub const MAX_NATIVE_SECTION_RUNS: usize = 16_384;

/// Upper bound on a section's nesting depth.
pub const MAX_NATIVE_SECTION_DEPTH: u32 = 16;

/// The native plane's generator identity. A reviewed oracle carrying this
/// identity would agree by construction, so it is refused as `NotIndependent`.
pub const NATIVE_SECTION_SUT_GENERATOR_ID: &str = "native-layout-plane";

/// The lane the native segmenter actually measured on the declared corpus. The
/// other declared lanes are *not* silently equal to it.
pub const NATIVE_MEASURED_LANE_ID: &str = "pdf_inspector_positioned";

/// The declared lane whose lattice the native segmenter refused (`typed
/// segmenter_rejected_lattice`): it is never counted, never silently skipped.
pub const SEGMENTER_REJECTED_LANE_IDS: [&str; 1] = ["pdf_inspector_positioned_xy_cut"];

/// The Java-authoritative lane. It is not a native plane and cannot be measured
/// by this surface.
pub const JAVA_AUTHORITY_LANE_ID: &str = "pdfalto";

/// The closed section verdict vocabulary. It is deliberately *not* the T01
/// body-gap vocabulary: a character-coverage verdict cannot be expressed here.
pub const NATIVE_SECTION_ROW_VERDICT_VOCABULARY: [&str; 4] = [
    "agreement",
    "boundary_displaced",
    "count_mismatch",
    "not_assessable",
];

/// The closed nine-region order, mirroring the S03 declaration.
pub fn native_section_regions() -> [FullSemanticRegion; NATIVE_SECTION_REGION_COUNT] {
    [
        FullSemanticRegion::Header,
        FullSemanticRegion::Body,
        FullSemanticRegion::References,
        FullSemanticRegion::Annex,
        FullSemanticRegion::Acknowledgement,
        FullSemanticRegion::Toc,
        FullSemanticRegion::Funding,
        FullSemanticRegion::Availability,
        FullSemanticRegion::PageFurniture,
    ]
}

/// The section-entry vocabulary, mirrored from S03's [`HeadingMatch`] so the
/// closed set is *deserializable* here (the S03 type is serialize-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSectionHeadingMatch {
    PreambleDefault,
    NormalizedLine,
    OrdinalStrippedLine,
    LeadingSpanLine,
    ProminentSectionHeading,
}

impl From<HeadingMatch> for NativeSectionHeadingMatch {
    fn from(value: HeadingMatch) -> Self {
        match value {
            HeadingMatch::PreambleDefault => Self::PreambleDefault,
            HeadingMatch::NormalizedLine => Self::NormalizedLine,
            HeadingMatch::OrdinalStrippedLine => Self::OrdinalStrippedLine,
            HeadingMatch::LeadingSpanLine => Self::LeadingSpanLine,
            HeadingMatch::ProminentSectionHeading => Self::ProminentSectionHeading,
        }
    }
}

/// How a lane id maps onto the native plane. Every declared lane has an answer;
/// an unlisted lane is `Unknown` rather than silently assumed measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLaneDisposition {
    /// The lane the native segmenter measured.
    Measured,
    /// The lattice the native segmenter refused (`segmenter_rejected_lattice`).
    SegmenterRejectedLattice,
    /// The Java-authoritative lane; not a native plane.
    JavaAuthority,
    /// A lane id outside the declaration.
    UnknownLane,
}

/// Classify a lane id. Never panics, never guesses: an unknown id is `Unknown`.
pub fn native_lane_disposition(lane_id: &str) -> NativeLaneDisposition {
    if lane_id == NATIVE_MEASURED_LANE_ID {
        NativeLaneDisposition::Measured
    } else if SEGMENTER_REJECTED_LANE_IDS.contains(&lane_id) {
        NativeLaneDisposition::SegmenterRejectedLattice
    } else if lane_id == JAVA_AUTHORITY_LANE_ID {
        NativeLaneDisposition::JavaAuthority
    } else {
        NativeLaneDisposition::UnknownLane
    }
}

/// Bounds of the section derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionReadingOrderConfig {
    pub max_sections: usize,
    pub max_depth: u32,
}

impl Default for NativeSectionReadingOrderConfig {
    fn default() -> Self {
        Self {
            max_sections: MAX_NATIVE_SECTION_RUNS,
            max_depth: MAX_NATIVE_SECTION_DEPTH,
        }
    }
}

/// The heading line that started a section. Redaction-safe: the line text is
/// never carried, only its digest-backed location and match level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionHeadingEvidence {
    pub page_ordinal: u32,
    pub line_ordinal: u32,
    pub matched_by: NativeSectionHeadingMatch,
    pub word_count: u32,
}

/// One section run inside a section-bearing region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionRun {
    pub section_id: String,
    pub parent_region: FullSemanticRegion,
    pub depth: u32,
    pub order_ordinal: usize,
    pub parent_section_id: Option<String>,
    pub start: FullLocation,
    pub end: FullLocation,
    pub admitted_tokens: u32,
    pub boundary_digest: String,
    /// Present only when a heading line entered the section; never invented.
    pub heading_evidence: Option<NativeSectionHeadingEvidence>,
}

/// One admitted region run as the reading order publishes it. This is the row
/// shape the reviewed segmentation oracle is compared against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionRegionRow {
    pub region: FullSemanticRegion,
    pub start: FullLocation,
    pub end: FullLocation,
    pub admitted_tokens: u32,
    pub boundary_digest: String,
}

/// One page's admitted span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionPageSpan {
    pub page_ordinal: u32,
    pub first: FullLocation,
    pub last: FullLocation,
    pub admitted_tokens: u32,
}

/// Typed absence on the section surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum NativeSectionAbsence {
    /// A section-bearing region was admitted but no heading line entered it.
    NoHeadingEvidence { region: FullSemanticRegion },
    /// The heading-aware walk never entered this region.
    RegionNeverEntered { region: FullSemanticRegion },
    /// A heading anchor was found but no admitted token followed it.
    EmptySectionRun {
        region: FullSemanticRegion,
        page_ordinal: u32,
    },
}

/// The reading-order half of the derivation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeReadingOrderOutcome {
    pub schema_version: String,
    /// The S03 admitted order identity the plane was built with.
    pub admitted_order_digest: String,
    pub admitted_token_count: u32,
    pub region_rows: Vec<NativeSectionRegionRow>,
    /// The region sequence in reading order (presence and order, no labels
    /// invented).
    pub region_sequence: Vec<FullSemanticRegion>,
    pub page_spans: Vec<NativeSectionPageSpan>,
    /// Reused from [`crate::native_fulltext_reading_order`]: the same column
    /// shift fact, one shape.
    pub column_shifts: Vec<ColumnShift>,
}

/// The section half of the derivation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionOutcome {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub section_parent_regions: Vec<FullSemanticRegion>,
    pub sections: Vec<NativeSectionRun>,
    pub absences: Vec<NativeSectionAbsence>,
    pub reading_order: NativeReadingOrderOutcome,
}

/// Fail-closed refusal vocabulary of the section / reading-order derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeSectionReadingOrderError {
    /// The carrier was built by another schema.
    CarrierSchemaMismatch,
    /// The heading outcome was built by another schema.
    HeadingSchemaMismatch,
    /// A carrier/heading/lattice identity field drifted.
    IdentityMismatch(&'static str),
    /// A section would live outside the two declared section-bearing regions.
    SectionParentRegionNotAdmitted { region: FullSemanticRegion },
    /// A heading ordinal exceeded [`NativeSectionReadingOrderConfig::max_depth`].
    SectionDepthExceeded { depth: u32 },
    /// More sections than [`NativeSectionReadingOrderConfig::max_sections`].
    TooManySections { count: usize },
    /// Section runs are not strictly ordered by their start location.
    NonMonotonicSection { region: FullSemanticRegion },
    /// The reviewed oracle shares the SUT's generator identity.
    NotIndependent,
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeSectionReadingOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native section reading order rejected input: {self:?}"
        )
    }
}

impl std::error::Error for NativeSectionReadingOrderError {}

/// Refuse an oracle that would agree by construction.
pub fn require_independent_section_oracle(
    generator_id: &str,
) -> Result<(), NativeSectionReadingOrderError> {
    if generator_id == NATIVE_SECTION_SUT_GENERATOR_ID {
        Err(NativeSectionReadingOrderError::NotIndependent)
    } else {
        Ok(())
    }
}

/// Derive the section tree and reading-order identity from the S03 plane.
///
/// The carrier supplies the ordered region runs (region, start/end, tokens);
/// the heading outcome supplies the entry evidence and the visual lines that
/// subdivide a section-bearing run. Neither input can reorder the other: runs
/// are ordered by their first admitted location, sections by their start.
pub fn derive_native_section_reading_order(
    lattice: &LayoutLattice,
    carrier: &NativeRegionCarrier,
    heading: &NativeHeadingRegionOutcome,
    config: NativeSectionReadingOrderConfig,
) -> Result<NativeSectionOutcome, NativeSectionReadingOrderError> {
    if carrier.schema_version != NATIVE_REGION_INPUT_SCHEMA_VERSION {
        return Err(NativeSectionReadingOrderError::CarrierSchemaMismatch);
    }
    if heading.schema_version != NATIVE_HEADING_REGIONS_SCHEMA_VERSION {
        return Err(NativeSectionReadingOrderError::HeadingSchemaMismatch);
    }
    if carrier.identity.document_id != heading.identity.document_id {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "document_id",
        ));
    }
    if carrier.identity.source_pdf_sha256 != heading.identity.source_pdf_sha256 {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "source_pdf_sha256",
        ));
    }
    if carrier.identity.provider_id != heading.identity.provider_id {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "provider_id",
        ));
    }
    if carrier.identity.lattice_fingerprint != heading.identity.lattice_fingerprint {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "lattice_fingerprint",
        ));
    }
    if lattice.identity.document_id != carrier.identity.document_id {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "lattice_document_id",
        ));
    }
    if lattice.identity.source_pdf_sha256 != carrier.identity.source_pdf_sha256 {
        return Err(NativeSectionReadingOrderError::IdentityMismatch(
            "lattice_source_pdf_sha256",
        ));
    }

    let mut region_rows: Vec<NativeSectionRegionRow> = carrier
        .runs
        .iter()
        .map(|run| NativeSectionRegionRow {
            region: run.region,
            start: run.start.clone(),
            end: run.end.clone(),
            admitted_tokens: run.tokens.len() as u32,
            boundary_digest: run.boundary_digest.clone(),
        })
        .collect();
    region_rows.sort_by_key(|left| location_key(&left.start));
    for pair in region_rows.windows(2) {
        if location_key(&pair[0].start) > location_key(&pair[1].start) {
            return Err(NativeSectionReadingOrderError::NonMonotonicSection {
                region: pair[1].region,
            });
        }
    }
    let region_sequence: Vec<FullSemanticRegion> =
        region_rows.iter().map(|row| row.region).collect();
    let admitted_token_count = region_rows
        .iter()
        .map(|row| row.admitted_tokens)
        .sum::<u32>();

    let mut page_spans: Vec<NativeSectionPageSpan> = Vec::new();
    for run in &carrier.runs {
        for token in &run.tokens {
            let same_page = page_spans
                .last()
                .map(|span| span.page_ordinal == token.location.page_ordinal)
                .unwrap_or(false);
            if same_page {
                let span = page_spans.last_mut().expect("checked above");
                span.last = token.location.clone();
                span.admitted_tokens += 1;
            } else {
                page_spans.push(NativeSectionPageSpan {
                    page_ordinal: token.location.page_ordinal,
                    first: token.location.clone(),
                    last: token.location.clone(),
                    admitted_tokens: 1,
                });
            }
        }
    }
    page_spans.sort_by_key(|span| span.page_ordinal);

    let column_shifts = column_shifts(lattice);

    let mut sections: Vec<NativeSectionRun> = Vec::new();
    let mut absences: Vec<NativeSectionAbsence> = Vec::new();
    for region in SECTION_PARENT_REGIONS {
        let run = carrier.run(region);
        let evidence = heading.evidence(region);
        match run {
            None => {
                absences.push(NativeSectionAbsence::RegionNeverEntered { region });
            }
            Some(run) => {
                let anchors = heading_anchors(run, region, evidence, heading);
                if anchors.is_empty() {
                    absences.push(NativeSectionAbsence::NoHeadingEvidence { region });
                    push_section(
                        &mut sections,
                        &mut absences,
                        region,
                        1,
                        None,
                        run.start.clone(),
                        run.end.clone(),
                        run.tokens.len() as u32,
                        None,
                        config,
                    )?;
                } else {
                    for (index, anchor) in anchors.iter().enumerate() {
                        let start = match anchor_start(run, anchor) {
                            Some(location) => location,
                            None => {
                                absences.push(NativeSectionAbsence::EmptySectionRun {
                                    region,
                                    page_ordinal: anchor.page_ordinal,
                                });
                                continue;
                            }
                        };
                        let end = anchors
                            .get(index + 1)
                            .and_then(|next| anchor_start(run, next))
                            .and_then(|next_start| previous_location(run, &next_start))
                            .unwrap_or_else(|| run.end.clone());
                        let admitted = run
                            .tokens
                            .iter()
                            .filter(|token| {
                                location_key(&token.location) >= location_key(&start)
                                    && location_key(&token.location) <= location_key(&end)
                            })
                            .count() as u32;
                        let parent_id = sections
                            .iter()
                            .rev()
                            .find(|section| {
                                section.parent_region == region && section.depth < anchor.depth
                            })
                            .map(|section| section.section_id.clone());
                        push_section(
                            &mut sections,
                            &mut absences,
                            region,
                            anchor.depth,
                            parent_id,
                            start,
                            end,
                            admitted,
                            Some(anchor.evidence.clone()),
                            config,
                        )?;
                    }
                }
            }
        }
    }

    let outcome = NativeSectionOutcome {
        schema_version: NATIVE_SECTION_READING_ORDER_SCHEMA_VERSION.to_owned(),
        document_id: carrier.identity.document_id.clone(),
        provider_id: carrier.identity.provider_id.clone(),
        source_pdf_sha256: carrier.identity.source_pdf_sha256.clone(),
        lattice_fingerprint: carrier.identity.lattice_fingerprint.clone(),
        section_parent_regions: SECTION_PARENT_REGIONS.to_vec(),
        sections,
        absences,
        reading_order: NativeReadingOrderOutcome {
            schema_version: NATIVE_SECTION_READING_ORDER_SCHEMA_VERSION.to_owned(),
            admitted_order_digest: carrier.identity.order_digest.clone(),
            admitted_token_count,
            region_rows,
            region_sequence,
            page_spans,
            column_shifts,
        },
    };
    Ok(outcome)
}

/// A heading anchor: a visual line that starts a section, or the region entry
/// evidence when the entry was not a vocabulary line.
#[derive(Debug, Clone)]
struct SectionAnchor {
    page_ordinal: u32,
    y_min: f64,
    line_ordinal: u32,
    depth: u32,
    evidence: NativeSectionHeadingEvidence,
}

fn heading_anchors(
    run: &crate::native_region_input::NativeRegionRun,
    region: FullSemanticRegion,
    evidence: Option<&HeadingEvidence>,
    heading: &NativeHeadingRegionOutcome,
) -> Vec<SectionAnchor> {
    let page_lo = run.start.page_ordinal;
    let page_hi = run.end.page_ordinal;
    let mut anchors: Vec<SectionAnchor> = Vec::new();
    for line in &heading.lines {
        if line.marker != Some(region) || line.page_ordinal < page_lo || line.page_ordinal > page_hi
        {
            continue;
        }
        let matched_by = line
            .matched_by
            .map(NativeSectionHeadingMatch::from)
            .unwrap_or(NativeSectionHeadingMatch::NormalizedLine);
        anchors.push(SectionAnchor {
            page_ordinal: line.page_ordinal,
            y_min: line.bbox.y_min,
            line_ordinal: line.line_ordinal,
            depth: section_depth(line.text()),
            evidence: NativeSectionHeadingEvidence {
                page_ordinal: line.page_ordinal,
                line_ordinal: line.line_ordinal,
                matched_by,
                word_count: line.word_count,
            },
        });
    }
    if let Some(evidence) = evidence
        && !anchors.iter().any(|anchor| {
            anchor.page_ordinal == evidence.page_ordinal
                && anchor.line_ordinal == evidence.line_ordinal
        })
    {
        let y_min = heading
            .lines
            .iter()
            .find(|line| {
                line.page_ordinal == evidence.page_ordinal
                    && line.line_ordinal == evidence.line_ordinal
            })
            .map(|line: &NativeVisualLine| line.bbox.y_min)
            .unwrap_or(0.0);
        anchors.push(SectionAnchor {
            page_ordinal: evidence.page_ordinal,
            y_min,
            line_ordinal: evidence.line_ordinal,
            depth: 1,
            evidence: NativeSectionHeadingEvidence {
                page_ordinal: evidence.page_ordinal,
                line_ordinal: evidence.line_ordinal,
                matched_by: NativeSectionHeadingMatch::from(evidence.matched_by),
                word_count: evidence.word_count,
            },
        });
    }
    anchors.sort_by(|left, right| {
        (left.page_ordinal, left.y_min, left.line_ordinal)
            .partial_cmp(&(right.page_ordinal, right.y_min, right.line_ordinal))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    anchors
}

/// The first admitted token at or after an anchor's page/ordinate.
fn anchor_start(
    run: &crate::native_region_input::NativeRegionRun,
    anchor: &SectionAnchor,
) -> Option<FullLocation> {
    run.tokens
        .iter()
        .find(|token| {
            token.location.page_ordinal > anchor.page_ordinal
                || (token.location.page_ordinal == anchor.page_ordinal
                    && token.bbox.y_min >= anchor.y_min)
        })
        .map(|token| token.location.clone())
}

/// The last admitted token strictly before `next_start`.
fn previous_location(
    run: &crate::native_region_input::NativeRegionRun,
    next_start: &FullLocation,
) -> Option<FullLocation> {
    run.tokens
        .iter()
        .rev()
        .find(|token| location_key(&token.location) < location_key(next_start))
        .map(|token| token.location.clone())
}

#[allow(clippy::too_many_arguments)]
fn push_section(
    sections: &mut Vec<NativeSectionRun>,
    absences: &mut Vec<NativeSectionAbsence>,
    region: FullSemanticRegion,
    depth: u32,
    parent_section_id: Option<String>,
    start: FullLocation,
    end: FullLocation,
    admitted_tokens: u32,
    heading_evidence: Option<NativeSectionHeadingEvidence>,
    config: NativeSectionReadingOrderConfig,
) -> Result<(), NativeSectionReadingOrderError> {
    if depth > config.max_depth {
        return Err(NativeSectionReadingOrderError::SectionDepthExceeded { depth });
    }
    if sections.len() >= config.max_sections {
        return Err(NativeSectionReadingOrderError::TooManySections {
            count: sections.len() + 1,
        });
    }
    if admitted_tokens == 0 {
        absences.push(NativeSectionAbsence::EmptySectionRun {
            region,
            page_ordinal: start.page_ordinal,
        });
        return Ok(());
    }
    let order_ordinal = sections.len();
    let section_id = format!(
        "{}-{}-{}",
        region_name(region),
        order_ordinal,
        start.page_ordinal
    );
    let boundary_digest = digest_parts(&[
        &section_id,
        region_name(region),
        &location_text(&start),
        &location_text(&end),
    ]);
    sections.push(NativeSectionRun {
        section_id,
        parent_region: region,
        depth,
        order_ordinal,
        parent_section_id,
        start,
        end,
        admitted_tokens,
        boundary_digest,
        heading_evidence,
    });
    Ok(())
}

/// The section depth from the heading line's own leading ordinal. `3.1 X` is
/// depth 2, a plain vocabulary heading is depth 1. No classifier is consulted.
pub fn section_depth(text: &str) -> u32 {
    let first = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    let numeric = first.chars().any(|c| c.is_ascii_digit())
        && first.chars().all(|c| c.is_ascii_digit() || c == '.');
    if numeric {
        (first.split('.').filter(|part| !part.is_empty()).count() as u32).max(1)
    } else {
        1
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

fn location_key(location: &FullLocation) -> (u32, u32, u32, u32) {
    (
        location.page_ordinal,
        location.block_ordinal,
        location.line_ordinal,
        location.token_ordinal,
    )
}

fn location_text(location: &FullLocation) -> String {
    format!(
        "{},{},{},{}",
        location.page_ordinal,
        location.block_ordinal,
        location.line_ordinal,
        location.token_ordinal
    )
}

fn digest_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// Column shifts derived from the lattice blocks, mirroring the Java rule
/// (`lowestPos > block.getY()`): a block that starts above the lowest position
/// already reached on its page is a typed shift, not a printed spacing weight.
fn column_shifts(lattice: &LayoutLattice) -> Vec<ColumnShift> {
    let mut shifts: Vec<ColumnShift> = Vec::new();
    for page in &lattice.pages {
        let mut lowest = 0.0f64;
        for block in &page.blocks {
            let column_change = lowest > block.bbox.y_min;
            let vertical_delta = block.bbox.y_min - lowest;
            shifts.push(ColumnShift {
                page: page.page_ordinal,
                block: block.source_ordinal,
                column_change,
                vertical_delta,
                spacing: if column_change {
                    None
                } else {
                    Some(vertical_delta)
                },
            });
            lowest = lowest.max(block.bbox.y_max);
        }
    }
    shifts
}

// ───────────────────────── oracle evaluation surface ─────────────────────────

/// One reviewed oracle boundary for a `(document, lane, region)` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionOracleBoundary {
    pub start: FullLocation,
    pub end: FullLocation,
}

/// One expected row re-derived from the reviewed segmentation oracle. `boundary`
/// is `None` exactly when the reviewed oracle recorded the region absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionOracleRow {
    pub document_id: String,
    pub lane_id: String,
    pub region: FullSemanticRegion,
    pub boundary: Option<NativeSectionOracleBoundary>,
}

/// Why a row could not be assessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum NativeSectionNotAssessable {
    /// The lane was refused by the segmenter (`segmenter_rejected_lattice`).
    LaneNotMeasuredBySegmenter,
    /// The lane is the Java authority, not a native plane.
    LaneIsJavaAuthority,
    /// The lane id is outside the declaration.
    UnknownLane,
    /// No oracle row exists for this document/lane/region.
    OracleRowMissing,
    /// The oracle shares the SUT's generator, so agreement would be vacuous.
    OracleNotIndependent,
}

/// The typed per-region verdict. Deliberately *not* the T01 body vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum NativeSectionRowVerdict {
    Agreement,
    BoundaryDisplaced {
        oracle_start: FullLocation,
        oracle_end: FullLocation,
        native_start: FullLocation,
        native_end: FullLocation,
    },
    CountMismatch {
        oracle_runs: u32,
        native_runs: u32,
    },
    NotAssessable {
        reason: NativeSectionNotAssessable,
    },
}

/// The verdict for one region, in the closed nine-region order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSectionRegionVerdict {
    pub region: FullSemanticRegion,
    pub verdict: NativeSectionRowVerdict,
}

/// Evaluate a native outcome's region rows against the reviewed oracle.
///
/// The oracle is *count/boundary based*: a region with exactly one reviewed
/// run that the native outcome also has is compared by its start/end; a
/// presence disagreement is a `CountMismatch`; a lane the native segmenter did
/// not measure is a typed `NotAssessable`, never an agreement.
pub fn evaluate_native_section_rows(
    outcome: &NativeSectionOutcome,
    lane_id: &str,
    oracle_rows: &[NativeSectionOracleRow],
    oracle_independent: bool,
) -> Result<Vec<NativeSectionRegionVerdict>, NativeSectionReadingOrderError> {
    let lane = native_lane_disposition(lane_id);
    let mut verdicts = Vec::with_capacity(NATIVE_SECTION_REGION_COUNT);
    for region in native_section_regions() {
        let verdict = if !oracle_independent {
            NativeSectionRowVerdict::NotAssessable {
                reason: NativeSectionNotAssessable::OracleNotIndependent,
            }
        } else if lane != NativeLaneDisposition::Measured {
            NativeSectionRowVerdict::NotAssessable {
                reason: match lane {
                    NativeLaneDisposition::SegmenterRejectedLattice => {
                        NativeSectionNotAssessable::LaneNotMeasuredBySegmenter
                    }
                    NativeLaneDisposition::JavaAuthority => {
                        NativeSectionNotAssessable::LaneIsJavaAuthority
                    }
                    NativeLaneDisposition::UnknownLane => NativeSectionNotAssessable::UnknownLane,
                    NativeLaneDisposition::Measured => unreachable!("lane checked above"),
                },
            }
        } else {
            let oracle = oracle_rows.iter().find(|row| {
                row.document_id == outcome.document_id
                    && row.lane_id == lane_id
                    && row.region == region
            });
            let native_rows: Vec<&NativeSectionRegionRow> = outcome
                .reading_order
                .region_rows
                .iter()
                .filter(|row| row.region == region)
                .collect();
            match oracle {
                None => NativeSectionRowVerdict::NotAssessable {
                    reason: NativeSectionNotAssessable::OracleRowMissing,
                },
                Some(oracle) => match (&oracle.boundary, native_rows.as_slice()) {
                    (None, []) => NativeSectionRowVerdict::Agreement,
                    (None, rows) => NativeSectionRowVerdict::CountMismatch {
                        oracle_runs: 0,
                        native_runs: rows.len() as u32,
                    },
                    (Some(_), []) => NativeSectionRowVerdict::CountMismatch {
                        oracle_runs: 1,
                        native_runs: 0,
                    },
                    (Some(boundary), [row]) => {
                        if boundary.start == row.start && boundary.end == row.end {
                            NativeSectionRowVerdict::Agreement
                        } else {
                            NativeSectionRowVerdict::BoundaryDisplaced {
                                oracle_start: boundary.start.clone(),
                                oracle_end: boundary.end.clone(),
                                native_start: row.start.clone(),
                                native_end: row.end.clone(),
                            }
                        }
                    }
                    (Some(_), rows) => NativeSectionRowVerdict::CountMismatch {
                        oracle_runs: 1,
                        native_runs: rows.len() as u32,
                    },
                },
            }
        };
        verdicts.push(NativeSectionRegionVerdict { region, verdict });
    }
    Ok(verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdict_vocabulary_is_closed_and_not_the_body_gap_vocabulary() {
        let render = |verdict: &NativeSectionRowVerdict| {
            serde_json::to_value(verdict)
                .ok()
                .and_then(|value| value["verdict"].as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let samples = [
            NativeSectionRowVerdict::Agreement,
            NativeSectionRowVerdict::BoundaryDisplaced {
                oracle_start: FullLocation {
                    page_ordinal: 1,
                    block_ordinal: 0,
                    line_ordinal: 0,
                    token_ordinal: 0,
                },
                oracle_end: FullLocation {
                    page_ordinal: 1,
                    block_ordinal: 0,
                    line_ordinal: 1,
                    token_ordinal: 0,
                },
                native_start: FullLocation {
                    page_ordinal: 1,
                    block_ordinal: 0,
                    line_ordinal: 0,
                    token_ordinal: 0,
                },
                native_end: FullLocation {
                    page_ordinal: 1,
                    block_ordinal: 0,
                    line_ordinal: 2,
                    token_ordinal: 0,
                },
            },
            NativeSectionRowVerdict::CountMismatch {
                oracle_runs: 1,
                native_runs: 0,
            },
            NativeSectionRowVerdict::NotAssessable {
                reason: NativeSectionNotAssessable::LaneNotMeasuredBySegmenter,
            },
        ];
        let mut seen = Vec::new();
        for sample in &samples {
            let tag = render(sample);
            assert!(!tag.is_empty(), "every verdict serializes a tag");
            seen.push(tag);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen, NATIVE_SECTION_ROW_VERDICT_VOCABULARY.to_vec());
        for forbidden in ["content_loss", "granularity_only", "body_gap", "character"] {
            assert!(
                !seen.iter().any(|tag| tag == forbidden),
                "the section vocabulary must not carry {forbidden}"
            );
        }
    }

    #[test]
    fn lane_disposition_is_total_and_never_guesses() {
        assert_eq!(
            native_lane_disposition(NATIVE_MEASURED_LANE_ID),
            NativeLaneDisposition::Measured
        );
        assert_eq!(
            native_lane_disposition(SEGMENTER_REJECTED_LANE_IDS[0]),
            NativeLaneDisposition::SegmenterRejectedLattice
        );
        assert_eq!(
            native_lane_disposition(JAVA_AUTHORITY_LANE_ID),
            NativeLaneDisposition::JavaAuthority
        );
        assert_eq!(
            native_lane_disposition("some-other-lane"),
            NativeLaneDisposition::UnknownLane
        );
    }

    #[test]
    fn a_same_generator_oracle_is_refused_by_name() {
        assert_eq!(
            require_independent_section_oracle(NATIVE_SECTION_SUT_GENERATOR_ID),
            Err(NativeSectionReadingOrderError::NotIndependent)
        );
        assert_eq!(
            require_independent_section_oracle("preserved-java-tei-scanner"),
            Ok(())
        );
    }

    #[test]
    fn section_depth_reads_the_leading_ordinal_only() {
        assert_eq!(section_depth("Introduction"), 1);
        assert_eq!(section_depth("3 Methods"), 1);
        assert_eq!(section_depth("3.1 Evaluation"), 2);
        assert_eq!(section_depth("3.1.4.2 Details"), 4);
    }
}
