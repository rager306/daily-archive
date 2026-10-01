//! Additive native callout-marker band (S05/T04): the callout atoms run on the
//! S03 native carrier instead of on TEI-derived copies.
//!
//! S04 published the callout *atoms* ([`crate::native_callout_classification`]).
//! They read typed callout tokens (text + superscript flag + document position)
//! and ledger them into two checksum-bound denominators, but nothing in
//! production connected them to the S03 layout plane, so the only feeds were
//! TEI-derived copies. This module is the missing input band.
//!
//! ## What it builds
//!
//! [`build_native_callout_input`] takes the S03 plane
//! ([`LayoutDocument`], [`LayoutLattice`], [`LexemeRow`],
//! [`FullSegmentationOutcome`]) and:
//!
//! 1. builds the S04 region carrier for the two callout-bearing regions
//!    ([`NATIVE_CALLOUT_REGIONS`]: `Body` and `Annex` — the regions the block
//!    sequence plane admits), so marker inventory comes from native regions and
//!    the carrier's own reconstructed lines;
//! 2. extracts callout-shaped marker runs with the documented, conservative
//!    lexical rule of [`extract_native_callout_markers`] — no TEI and no Java
//!    type is an input anywhere in this file;
//! 3. runs [`classify_callout_marker`], [`majority_callout_style`] and
//!    [`build_callout_inventory`] over the extracted markers, and projects the
//!    result onto the digest plane's matching vocabulary
//!    ([`CalloutUnresolvedReason`], [`AdjacencyDisposition`]).
//!
//! ## The target inventory is `source_not_established`, and that is structural
//!
//! `data/parse/callout_matching_semantics.yaml` fixes
//! `target_inventory.status = source_not_established`, `fuzzy_guess = forbidden`
//! and `equation.positive_match = forbidden_until_inventory_proven`. This band
//! therefore passes an **empty** [`CalloutLabelOrdinalMap`] to the inventory
//! builder: the native plane alone cannot establish a target inventory, so every
//! citation marker is a typed `unresolved` and no marker can be reported as a
//! match. [`NativeCalloutMatchingDisposition::try_from_resolution`] refuses a
//! positive resolution while the inventory is not established, with a failure
//! that names the rule:
//!
//! * an equation positive resolution is
//!   [`NativeCalloutInputError::EquationPositiveMatch`];
//! * any other positive resolution is
//!   [`NativeCalloutInputError::FuzzyTargetClaimed`].
//!
//! A marker whose own tokens cross a page/block/line/annex boundary is refused
//! as [`NativeCalloutInputError::CrossBoundaryMarker`]: it is not one marker.
//!
//! ## Redaction
//!
//! [`CalloutInventory`] carries the marker text (it is the atom's ABI surface).
//! The outcome published here does **not**: it carries the closed marker type,
//! the majority tally, the two sealed denominators and per-marker digests, so it
//! can be serialized without carrying article text. The raw-text ABI stays
//! reachable only through [`CalloutMarkerInput`], exactly as it is today.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging or
//! process surface. Nothing here is spliced into the Java-authoritative parse
//! route; `import_eligible` stays false, no threshold is accepted and no
//! promotion happens.

use crate::native_callout_classification::{
    CalloutInventory, CalloutLabelOrdinalMap, CalloutMarkerInput, CalloutMarkerType,
    CalloutStyleMajority, CalloutToken, build_callout_inventory, majority_callout_style,
    marker_adjacency,
};
use crate::native_callout_matching::{
    AdjacencyDisposition, CalloutPosition, CalloutTargetClass, CalloutUnresolvedReason,
    ObservedCalloutStyle,
};
use crate::native_region_input::{
    NativeRegionCarrier, NativeRegionError, NativeRegionRun, NativeRegionToken,
    build_native_region_carrier,
};
use crate::{
    FULL_SEGMENTATION_SCHEMA_VERSION, FullSegmentationOutcome, FullSemanticRegion, LayoutDocument,
    LayoutLattice, LayoutTokenKind, LexemeRow, NativeCalloutResolution,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the additive native callout-input surface.
pub const NATIVE_CALLOUT_NATIVE_INPUT_SCHEMA_VERSION: &str =
    "da-parse.native-callout-native-input/v1";

/// Generator identity of this band. An independent oracle must not declare it
/// (see [`require_independent_callout_oracle`]).
pub const NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID: &str = "da-parse.native-callout-native-input";

/// The regions this band reads markers from — the two regions the block
/// sequence plane admits (`Body`, `Annex`). Header/References/PageFurniture/Toc
/// are deliberately excluded: their numbers are affiliation markers, reference
/// labels and page furniture, not in-text callouts.
pub const NATIVE_CALLOUT_REGIONS: [FullSemanticRegion; 2] =
    [FullSemanticRegion::Body, FullSemanticRegion::Annex];

/// The closed marker-family vocabulary, in declaration order.
pub const NATIVE_CALLOUT_FAMILY_VOCABULARY: [&str; 4] = ["citation", "figure", "table", "equation"];

/// The four marker families in declaration order.
pub const NATIVE_CALLOUT_FAMILIES: [CalloutTargetClass; 4] = [
    CalloutTargetClass::Citation,
    CalloutTargetClass::Figure,
    CalloutTargetClass::Table,
    CalloutTargetClass::Equation,
];

/// The closed disposition vocabulary of one document × family. `out_of_scope` is
/// deliberately not a member: a family the oracle requires and the native lane
/// cannot observe is a typed `unmet`, never a scope reduction.
pub const NATIVE_CALLOUT_FAMILY_DISPOSITION_VOCABULARY: [&str; 3] =
    ["measured", "unmet", "not_observed"];

/// The declared state of the digest-plane target inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutTargetInventoryStatus {
    /// `callout_matching_semantics.yaml#matching_semantics.target_inventory.status`.
    SourceNotEstablished,
    /// A proven inventory; not reachable in this cut.
    Established,
}

/// Typed absence of the whole callout lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutInputAbsence {
    /// The document admitted no tokens at all.
    LaneAbsent,
    /// The document carried tokens but no callout-shaped run was admitted.
    NoMarkerShapedRun,
}

/// Provenance identity of the native plane the outcome was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutInputIdentity {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
    /// The S03 `FullSegmentationOutcome::order_digest`.
    pub order_digest: String,
    /// Digest of the redaction-safe region carrier the markers came from.
    pub carrier_digest: String,
}

/// One admitted marker, redaction-safe: no marker text is carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutMarkerRow {
    /// 0-based position in document order.
    pub index: usize,
    pub family: CalloutTargetClass,
    pub position: CalloutPosition,
    /// The closed Java `MarkerType` the classifier produced.
    pub marker_type: CalloutMarkerType,
    pub style: ObservedCalloutStyle,
    pub adjacency: AdjacencyDisposition,
    pub token_count: usize,
    /// Whether every non-blank token was superscript. On the native plane this
    /// is a **derived** signal (a font size strictly smaller than the line's
    /// dominant size); the plane cannot carry Java's own flag.
    pub superscript: bool,
    pub resolution: NativeCalloutResolution,
    /// Checksum of the occurrence, identical to the inventory's own digest.
    pub digest: String,
}

/// The digest-plane matching disposition of one marker.
///
/// While the target inventory is `source_not_established`, `Matched` is
/// structurally unreachable: [`Self::try_from_resolution`] refuses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutMatchingDisposition {
    /// A positive match. Not reachable while the inventory is not established.
    Matched { ordinal: usize },
    /// A typed unresolved occurrence.
    Unresolved { reason: CalloutUnresolvedReason },
}

impl NativeCalloutMatchingDisposition {
    /// Project one resolution onto the matching vocabulary, enforcing the
    /// declared inventory state.
    ///
    /// A positive resolution while the inventory is
    /// [`NativeCalloutTargetInventoryStatus::SourceNotEstablished`] is refused
    /// by name: `equation` produces
    /// [`NativeCalloutInputError::EquationPositiveMatch`], every other family
    /// [`NativeCalloutInputError::FuzzyTargetClaimed`].
    pub fn try_from_resolution(
        index: usize,
        family: CalloutTargetClass,
        resolution: &NativeCalloutResolution,
        status: NativeCalloutTargetInventoryStatus,
    ) -> Result<Self, NativeCalloutInputError> {
        match resolution {
            NativeCalloutResolution::Unresolved { reason } => {
                Ok(Self::Unresolved { reason: *reason })
            }
            NativeCalloutResolution::Targeted { ordinal, .. } => match status {
                NativeCalloutTargetInventoryStatus::SourceNotEstablished => {
                    if family == CalloutTargetClass::Equation {
                        Err(NativeCalloutInputError::EquationPositiveMatch { index })
                    } else {
                        Err(NativeCalloutInputError::FuzzyTargetClaimed { index })
                    }
                }
                NativeCalloutTargetInventoryStatus::Established => {
                    Ok(Self::Matched { ordinal: *ordinal })
                }
            },
        }
    }
}

/// One marker's matching row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutMatchingRow {
    pub index: usize,
    pub family: CalloutTargetClass,
    pub disposition: NativeCalloutMatchingDisposition,
}

/// The redaction-safe inventory projection: counts, closed tokens, checksums.
///
/// `Present` exactly when at least one marker was admitted. Every field is
/// derived from the [`CalloutInventory`] the atoms produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutInventoryProjection {
    pub lane: String,
    pub markers: Vec<NativeCalloutMarkerRow>,
    pub matching: Vec<NativeCalloutMatchingRow>,
    pub majority: CalloutStyleMajority,
    pub targeted: crate::native_callout_classification::CalloutDenominator,
    pub targetless: crate::native_callout_classification::CalloutDenominator,
    pub verdict: crate::native_callout_classification::CalloutLinkageVerdict,
}

/// Redaction-safe result of running the callout atoms on the native carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutFromInputOutcome {
    pub schema_version: String,
    pub identity: NativeCalloutInputIdentity,
    /// The declared target-inventory state, carried so the outcome is
    /// self-describing.
    pub target_inventory_status: NativeCalloutTargetInventoryStatus,
    /// The regions the carrier was built for.
    pub regions: Vec<FullSemanticRegion>,
    /// `None` exactly when no callout-shaped marker was admitted: a typed
    /// absence, never a zero-length inventory.
    pub inventory: Option<NativeCalloutInventoryProjection>,
    /// Typed absence of the whole lane when `inventory` is `None`.
    pub absence: Option<NativeCalloutInputAbsence>,
}

impl NativeCalloutFromInputOutcome {
    /// Admitted marker count of one family; `0` for an absent lane.
    pub fn admitted_markers(&self, family: CalloutTargetClass) -> usize {
        self.inventory
            .as_ref()
            .map(|inventory| {
                inventory
                    .markers
                    .iter()
                    .filter(|marker| marker.family == family)
                    .count()
            })
            .unwrap_or(0)
    }

    /// Every admitted marker.
    pub fn total_markers(&self) -> usize {
        self.inventory
            .as_ref()
            .map(|inventory| inventory.markers.len())
            .unwrap_or(0)
    }
}

/// Fail-closed refusal vocabulary of the native callout-input build.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeCalloutInputError {
    /// The layout document failed its own contract.
    LayoutInvalid,
    /// The lattice does not validate against the layout document.
    LatticeInvalid,
    /// A segmentation identity field drifted (`schema_version`,
    /// `document_id`, `source_pdf_sha256`, `lattice_fingerprint`).
    IdentityMismatch(&'static str),
    /// The region carrier refused the plane.
    RegionCarrier(NativeRegionError),
    /// A marker carried no token, so it is not an occurrence.
    EmptyMarker { index: usize },
    /// A marker's own tokens cross a page/block/line/annex boundary: it is not
    /// one marker, so it is refused rather than merged.
    CrossBoundaryMarker { index: usize },
    /// The band's extracted tokens and the classifier's recorded adjacency
    /// disagree, so the projection would publish a marker the atoms did not see.
    AdjacencyMismatch { index: usize },
    /// A positive (non-equation) resolution was claimed while the target
    /// inventory is `source_not_established`. Fuzzy guessing is forbidden.
    FuzzyTargetClaimed { index: usize },
    /// An equation positive resolution was claimed. `equation.positive_match` is
    /// forbidden until the inventory is proven.
    EquationPositiveMatch { index: usize },
    /// The callout atoms refused the extracted markers.
    Classification(crate::native_callout_classification::NativeCalloutClassificationError),
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeCalloutInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayoutInvalid => write!(formatter, "layout document is invalid"),
            Self::LatticeInvalid => write!(formatter, "layout lattice does not match the document"),
            Self::IdentityMismatch(field) => {
                write!(formatter, "segmentation identity drifted on {field}")
            }
            Self::RegionCarrier(error) => write!(formatter, "region carrier refused: {error}"),
            Self::EmptyMarker { index } => {
                write!(formatter, "native callout marker {index} carried no token")
            }
            Self::CrossBoundaryMarker { index } => write!(
                formatter,
                "native callout marker {index} crosses a page/block/line/annex boundary"
            ),
            Self::AdjacencyMismatch { index } => write!(
                formatter,
                "native callout marker {index} adjacency disagrees with its extracted tokens"
            ),
            Self::FuzzyTargetClaimed { index } => write!(
                formatter,
                "native callout marker {index} claims a target while the inventory is source_not_established"
            ),
            Self::EquationPositiveMatch { index } => write!(
                formatter,
                "native callout equation marker {index} claims a positive match, which is forbidden"
            ),
            Self::Classification(error) => write!(formatter, "callout atoms refused: {error}"),
            Self::Serialization => write!(formatter, "native callout outcome serialization failed"),
        }
    }
}

impl std::error::Error for NativeCalloutInputError {}

/// Validate one extracted marker before it is admitted.
///
/// A marker is one occurrence only when its tokens share page, block, line and
/// annex parent. Crossing any of those is
/// [`NativeCalloutInputError::CrossBoundaryMarker`] — the fail-closed reading of
/// `matching_semantics.adjacency` (`sibling_crossing`/`annex_crossing`
/// forbidden).
pub fn validate_native_callout_marker(
    index: usize,
    marker: &CalloutMarkerInput,
) -> Result<(), NativeCalloutInputError> {
    let Some(first) = marker.tokens.first() else {
        return Err(NativeCalloutInputError::EmptyMarker { index });
    };
    if marker.tokens.len() > crate::native_callout_classification::MAX_NATIVE_CALLOUT_MARKER_TOKENS
    {
        return Err(NativeCalloutInputError::Classification(
            crate::native_callout_classification::NativeCalloutClassificationError::LimitsExceeded {
                resource: "marker_token",
                actual: marker.tokens.len(),
                maximum: crate::native_callout_classification::MAX_NATIVE_CALLOUT_MARKER_TOKENS,
            },
        ));
    }
    for token in &marker.tokens {
        if token.position.page_ordinal != first.position.page_ordinal
            || token.position.block_ordinal != first.position.block_ordinal
            || token.position.line_ordinal != first.position.line_ordinal
            || token.position.annex_parent != first.position.annex_parent
        {
            return Err(NativeCalloutInputError::CrossBoundaryMarker { index });
        }
    }
    Ok(())
}

/// A callout line: the run-local token indices sharing one
/// `(page, block, line)` key, in carrier order.
struct CalloutLine {
    indices: Vec<usize>,
}

fn line_of(run: &NativeRegionRun, index: usize) -> (u32, u32, u32) {
    let location = &run.tokens[index].location;
    (
        location.page_ordinal,
        location.block_ordinal,
        location.line_ordinal,
    )
}

fn group_lines(run: &NativeRegionRun) -> Vec<CalloutLine> {
    let mut lines: Vec<CalloutLine> = Vec::new();
    let mut current: Option<(u32, u32, u32)> = None;
    for index in 0..run.tokens.len() {
        let key = line_of(run, index);
        if current != Some(key) {
            lines.push(CalloutLine {
                indices: Vec::new(),
            });
            current = Some(key);
        }
        lines
            .last_mut()
            .expect("a line was just pushed")
            .indices
            .push(index);
    }
    lines
}

fn is_blank(token: &NativeRegionToken) -> bool {
    matches!(token.kind, LayoutTokenKind::Whitespace) || token.text().trim().is_empty()
}

fn position_of(run: &NativeRegionRun, index: usize, region: FullSemanticRegion) -> CalloutPosition {
    let location = &run.tokens[index].location;
    CalloutPosition {
        page_ordinal: location.page_ordinal,
        block_ordinal: location.block_ordinal,
        line_ordinal: location.line_ordinal,
        token_ordinal: location.token_ordinal,
        annex_parent: (region == FullSemanticRegion::Annex).then_some(0),
    }
}

/// The line's dominant font size: the most frequent `font_size_milli_pt` among
/// its non-blank tokens, ties broken by the larger size so the choice is
/// deterministic.
fn dominant_font_milli_pt(run: &NativeRegionRun, line: &CalloutLine) -> Option<u32> {
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for &index in &line.indices {
        if is_blank(&run.tokens[index]) {
            continue;
        }
        if let Some(size) = run.tokens[index].style.font_size_milli_pt {
            *counts.entry(size).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)))
        .map(|(size, _)| size)
}

/// Whether a token is superscript on the native plane: its font size is
/// strictly smaller than its line's dominant size.
///
/// This is a documented *derivation*, not Java's flag (the layout plane does not
/// carry one). It is named as a derivation in
/// [`NativeCalloutMarkerRow::superscript`].
fn is_superscript_font(run: &NativeRegionRun, index: usize, dominant: Option<u32>) -> bool {
    match (run.tokens[index].style.font_size_milli_pt, dominant) {
        (Some(size), Some(dominant)) => size < dominant,
        _ => false,
    }
}

/// `[0-9]{1,4}[a-f]?` — the bare numeric shape a callout marker can print.
fn is_bare_number(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 5 {
        return false;
    }
    let mut chars: Vec<char> = trimmed.chars().collect();
    if let Some(last) = chars.last().copied()
        && last.is_ascii_lowercase()
        && ('a'..='f').contains(&last)
    {
        chars.pop();
    }
    !chars.is_empty() && chars.len() <= 4 && chars.iter().all(char::is_ascii_digit)
}

/// Normalize a printed lead-in word into its lookup key.
fn normalize_lead_in(text: &str) -> String {
    text.trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase()
}

/// The family a printed lead-in word selects, when it selects one.
///
/// This is a documented, conservative lexical rule: the native plane cannot tell
/// a figure callout from a citation callout by the marker glyphs alone, so the
/// family comes from the printed lead-in (`Fig. 3`, `Table 2`, `Eq. (1)`). A
/// candidate with no recognized lead-in is a citation — the default the
/// vocabulary already prescribes, and never a `fuzzy` target guess: the target
/// inventory stays `source_not_established` either way.
fn family_of_lead_in(text: &str) -> Option<CalloutTargetClass> {
    match normalize_lead_in(text).as_str() {
        "fig" | "figs" | "figure" | "figures" => Some(CalloutTargetClass::Figure),
        "tab" | "tabs" | "table" | "tables" => Some(CalloutTargetClass::Table),
        "eq" | "eqn" | "eqns" | "equ" | "equation" | "equations" | "formula" | "formulas"
        | "formulae" => Some(CalloutTargetClass::Equation),
        _ => None,
    }
}

/// The lead-in family of the candidate whose first token sits at `position`
/// within `line.indices`, or `None` when no recognized lead-in precedes it.
fn lead_in_family(
    run: &NativeRegionRun,
    line: &CalloutLine,
    position: usize,
) -> Option<CalloutTargetClass> {
    for &index in line.indices[..position].iter().rev() {
        if is_blank(&run.tokens[index]) {
            continue;
        }
        return family_of_lead_in(run.tokens[index].text());
    }
    None
}

/// Extract the callout-shaped marker runs of one line, in reading order.
///
/// The rule is conservative and closed:
///
/// * a **closed bracket/parenthesis group** (`[…]` or `(…)`) is a candidate; its
///   family comes from its lead-in, defaulting to `citation`;
/// * a **bare number** is a candidate only when it is superscript-derived or has
///   a recognized lead-in, so an ordinary sentence number (`1`) is never a
///   callout;
/// * a candidate never spans a line — the grouping is line-local, which is why
///   no crossing marker can be extracted.
fn markers_in_line(
    run: &NativeRegionRun,
    region: FullSemanticRegion,
    line: &CalloutLine,
    dominant: Option<u32>,
    out: &mut Vec<CalloutMarkerInput>,
) {
    let mut covered = vec![false; run.tokens.len()];

    for (position, &start) in line.indices.iter().enumerate() {
        if covered[start] {
            continue;
        }
        let text = run.tokens[start].text();
        let open = match text.trim_start().chars().next() {
            Some('[') => '[',
            Some('(') => '(',
            _ => continue,
        };
        let close = if open == '[' { ']' } else { ')' };
        let mut end_position = None;
        let opening = run.tokens[start].text().trim();
        if opening.len() >= 2 && opening.ends_with(close) {
            // `[12]` / `(4)` as one token: the group opens and closes here.
            end_position = Some(position);
        } else {
            for (offset, &candidate) in line.indices[position + 1..].iter().enumerate() {
                if run.tokens[candidate].text().trim_end().ends_with(close) {
                    end_position = Some(position + 1 + offset);
                    break;
                }
            }
        }
        let Some(end_position) = end_position else {
            continue;
        };
        for &index in &line.indices[position..=end_position] {
            covered[index] = true;
        }
        let family = lead_in_family(run, line, position).unwrap_or(CalloutTargetClass::Citation);
        out.push(build_marker(
            run,
            region,
            &line.indices[position..=end_position],
            dominant,
            family,
        ));
    }

    for (position, &index) in line.indices.iter().enumerate() {
        if covered[index] || is_blank(&run.tokens[index]) {
            continue;
        }
        if !is_bare_number(run.tokens[index].text()) {
            continue;
        }
        let superscript = is_superscript_font(run, index, dominant);
        let lead_in = lead_in_family(run, line, position);
        if !superscript && lead_in.is_none() {
            continue;
        }
        covered[index] = true;
        out.push(build_marker(
            run,
            region,
            &[index],
            dominant,
            lead_in.unwrap_or(CalloutTargetClass::Citation),
        ));
    }
}

fn build_marker(
    run: &NativeRegionRun,
    region: FullSemanticRegion,
    indices: &[usize],
    dominant: Option<u32>,
    family: CalloutTargetClass,
) -> CalloutMarkerInput {
    let tokens = indices
        .iter()
        .map(|&index| CalloutToken {
            text: run.tokens[index].text().to_owned(),
            superscript: is_superscript_font(run, index, dominant),
            position: position_of(run, index, region),
        })
        .collect();
    CalloutMarkerInput {
        target_class: family,
        tokens,
    }
}

/// Extract callout-shaped markers from the region carrier, in document order.
///
/// Only the [`NATIVE_CALLOUT_REGIONS`] runs are read; the carrier's own token
/// text and geometry are the only inputs.
///
/// Emission is line-local and runs in two passes — closed groups first (so the
/// tokens inside one shield a bare number), then bare numbers. That precedence
/// does **not** preserve document order when a bare number precedes a group on
/// the same line, so the atoms are ordered by the position of each marker's
/// first token before they are admitted. The *set* of markers is unchanged; the
/// inventory refuses a non-monotonic stream by name, so this ordering is what
/// keeps a real corpus admissible.
pub fn extract_native_callout_markers(
    carrier: &NativeRegionCarrier,
) -> Result<Vec<CalloutMarkerInput>, NativeCalloutInputError> {
    let mut markers = Vec::new();
    for run in &carrier.runs {
        if !NATIVE_CALLOUT_REGIONS.contains(&run.region) {
            continue;
        }
        for line in group_lines(run) {
            let dominant = dominant_font_milli_pt(run, &line);
            markers_in_line(run, run.region, &line, dominant, &mut markers);
        }
    }
    markers.sort_by(|left, right| left.tokens[0].position.cmp(&right.tokens[0].position));
    for (index, marker) in markers.iter().enumerate() {
        validate_native_callout_marker(index, marker)?;
    }
    Ok(markers)
}

/// Validate the S03 plane and derive its provider plus lattice fingerprint.
fn validate_plane(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    segmentation: &FullSegmentationOutcome,
) -> Result<String, NativeCalloutInputError> {
    document
        .validate()
        .map_err(|_| NativeCalloutInputError::LayoutInvalid)?;
    lattice
        .validate_against(document)
        .map_err(|_| NativeCalloutInputError::LatticeInvalid)?;
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeCalloutInputError::Serialization)?;
    if segmentation.schema_version != FULL_SEGMENTATION_SCHEMA_VERSION {
        return Err(NativeCalloutInputError::IdentityMismatch("schema_version"));
    }
    if segmentation.identity.document_id != document.document_id {
        return Err(NativeCalloutInputError::IdentityMismatch("document_id"));
    }
    if segmentation.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeCalloutInputError::IdentityMismatch(
            "source_pdf_sha256",
        ));
    }
    if segmentation.identity.lattice_fingerprint != lattice_fingerprint {
        return Err(NativeCalloutInputError::IdentityMismatch(
            "lattice_fingerprint",
        ));
    }
    Ok(lattice_fingerprint)
}

/// Build the native callout outcome from the S03 plane.
///
/// The target inventory is always [`NativeCalloutTargetInventoryStatus::SourceNotEstablished`]:
/// the band passes an empty label map, so no marker can resolve to a target.
pub fn build_native_callout_input(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    segmentation: &FullSegmentationOutcome,
    regions: &[FullSemanticRegion],
) -> Result<NativeCalloutFromInputOutcome, NativeCalloutInputError> {
    let lattice_fingerprint = validate_plane(document, lattice, segmentation)?;
    let carrier = build_native_region_carrier(document, lattice, lexemes, segmentation, regions)
        .map_err(NativeCalloutInputError::RegionCarrier)?;
    let carrier_digest = carrier
        .digest()
        .map_err(|_| NativeCalloutInputError::Serialization)?;
    let identity = NativeCalloutInputIdentity {
        schema_version: NATIVE_CALLOUT_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
        document_id: document.document_id.clone(),
        provider_id: segmentation.identity.provider_id.clone(),
        source_pdf_sha256: document.source_pdf_sha256.clone(),
        lattice_fingerprint,
        config_fingerprint: segmentation.identity.config_fingerprint.clone(),
        order_digest: segmentation.order_digest.clone(),
        carrier_digest,
    };
    let markers = extract_native_callout_markers(&carrier)?;
    if markers.is_empty() {
        let lane_absent = carrier.runs.is_empty();
        return Ok(NativeCalloutFromInputOutcome {
            schema_version: NATIVE_CALLOUT_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
            identity,
            target_inventory_status: NativeCalloutTargetInventoryStatus::SourceNotEstablished,
            regions: regions.to_vec(),
            inventory: None,
            absence: Some(if lane_absent {
                NativeCalloutInputAbsence::LaneAbsent
            } else {
                NativeCalloutInputAbsence::NoMarkerShapedRun
            }),
        });
    }

    let labels = CalloutLabelOrdinalMap {
        entries: Vec::new(),
        unlabeled_lines: Vec::new(),
        conflicts: Vec::new(),
    };
    let inventory = build_callout_inventory(
        &document.document_id,
        &document.source_pdf_sha256,
        &markers,
        &labels,
    )
    .map_err(NativeCalloutInputError::Classification)?;
    let projection = project_inventory(&inventory, &markers)?;
    Ok(NativeCalloutFromInputOutcome {
        schema_version: NATIVE_CALLOUT_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
        identity,
        target_inventory_status: NativeCalloutTargetInventoryStatus::SourceNotEstablished,
        regions: regions.to_vec(),
        inventory: Some(projection),
        absence: None,
    })
}

/// Project the text-bearing [`CalloutInventory`] onto the redaction-safe
/// outcome, enforcing every closed rule on the way.
///
/// `markers` is the band's own extracted marker list, in the same order the
/// inventory admitted it; the adjacency each token run produces is re-checked
/// against the inventory's recorded adjacency so the projection cannot publish
/// a marker shape the atoms did not see.
fn project_inventory(
    inventory: &CalloutInventory,
    markers: &[CalloutMarkerInput],
) -> Result<NativeCalloutInventoryProjection, NativeCalloutInputError> {
    if markers.len() != inventory.observations.len() {
        return Err(NativeCalloutInputError::Classification(
            crate::native_callout_classification::NativeCalloutClassificationError::DenominatorCountMismatch {
                targeted: inventory.targeted.count,
                targetless: inventory.targetless.count,
                total: inventory.observations.len(),
            },
        ));
    }
    let classifications: Vec<_> = inventory
        .observations
        .iter()
        .map(|observation| observation.marker.clone())
        .collect();
    let majority = majority_callout_style(&classifications);
    if majority != inventory.majority {
        return Err(NativeCalloutInputError::Classification(
            crate::native_callout_classification::NativeCalloutClassificationError::MajorityMismatch,
        ));
    }
    let mut rows = Vec::with_capacity(inventory.observations.len());
    let mut matching = Vec::with_capacity(inventory.observations.len());
    for (observation, marker) in inventory.observations.iter().zip(markers.iter()) {
        if marker.target_class != observation.target_class {
            return Err(NativeCalloutInputError::Classification(
                crate::native_callout_classification::NativeCalloutClassificationError::StyleMismatch {
                    index: observation.index,
                },
            ));
        }
        if marker_adjacency(&marker.tokens) != observation.marker.adjacency {
            return Err(NativeCalloutInputError::AdjacencyMismatch {
                index: observation.index,
            });
        }
        let disposition = NativeCalloutMatchingDisposition::try_from_resolution(
            observation.index,
            observation.target_class,
            &observation.resolution,
            NativeCalloutTargetInventoryStatus::SourceNotEstablished,
        )?;
        rows.push(NativeCalloutMarkerRow {
            index: observation.index,
            family: observation.target_class,
            position: observation.position,
            marker_type: observation.marker.marker_type,
            style: observation.marker.style,
            adjacency: observation.marker.adjacency,
            token_count: observation.marker.token_count,
            superscript: observation.marker.is_superscript,
            resolution: observation.resolution.clone(),
            digest: observation.digest.clone(),
        });
        matching.push(NativeCalloutMatchingRow {
            index: observation.index,
            family: observation.target_class,
            disposition,
        });
    }
    Ok(NativeCalloutInventoryProjection {
        lane: inventory.lane.clone(),
        markers: rows,
        matching,
        majority,
        targeted: inventory.targeted.clone(),
        targetless: inventory.targetless.clone(),
        verdict: inventory.verdict(),
    })
}

// ───────────────────────── independent-oracle comparison ─────────────────────────

/// Provenance identity of an independent callout oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutOracleProvenance {
    /// Identity of the code that produced the oracle. Must differ from
    /// [`NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID`].
    pub generator_id: String,
    /// The producing backend, e.g. `java-grobid-crf`.
    pub backend: String,
    /// sha256 pin of the oracle source bytes.
    pub sha256: String,
}

/// Refusal vocabulary of the oracle independence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCalloutOracleIndependenceError {
    MissingGenerator,
    MissingBackend,
    MissingPin,
    /// The oracle was produced by the same code as the SUT.
    NotIndependent {
        generator_id: String,
    },
}

impl fmt::Display for NativeCalloutOracleIndependenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGenerator => write!(formatter, "oracle provenance has no generator id"),
            Self::MissingBackend => write!(formatter, "oracle provenance has no backend"),
            Self::MissingPin => write!(formatter, "oracle provenance has no sha256 pin"),
            Self::NotIndependent { generator_id } => write!(
                formatter,
                "oracle generator `{generator_id}` is the SUT generator, not independent"
            ),
        }
    }
}

impl std::error::Error for NativeCalloutOracleIndependenceError {}

/// Require an oracle to have a distinct generator, a backend, and a sha256 pin.
pub fn require_independent_callout_oracle(
    provenance: &NativeCalloutOracleProvenance,
) -> Result<(), NativeCalloutOracleIndependenceError> {
    if provenance.generator_id.trim().is_empty() {
        return Err(NativeCalloutOracleIndependenceError::MissingGenerator);
    }
    if provenance.generator_id == NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID {
        return Err(NativeCalloutOracleIndependenceError::NotIndependent {
            generator_id: provenance.generator_id.clone(),
        });
    }
    if provenance.backend.trim().is_empty() {
        return Err(NativeCalloutOracleIndependenceError::MissingBackend);
    }
    if !is_sha256(&provenance.sha256) {
        return Err(NativeCalloutOracleIndependenceError::MissingPin);
    }
    Ok(())
}

/// One positive per-family row read from preserved bytes by an independent
/// scanner. Digest-only: no marker text is carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutOracleRow {
    pub document_id: String,
    pub family: CalloutTargetClass,
    /// Zero-based ordinal of the row within its document and family.
    pub ordinal: u32,
    /// sha256 of the raw element bytes.
    pub content_digest: String,
}

/// A document pin of the oracle source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutOraclePin {
    pub document_id: String,
    pub sha256: String,
}

/// The independent positive oracle for the callout marker families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutOracle {
    pub schema_version: String,
    pub generator_id: String,
    pub backend: String,
    pub pins: Vec<NativeCalloutOraclePin>,
    pub rows: Vec<NativeCalloutOracleRow>,
}

/// Schema identity of the independent callout oracle.
pub const NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION: &str = "da-parse.native-callout-oracle/v1";

/// Refusal vocabulary of the oracle guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCalloutOracleError {
    InvalidSchema,
    /// A document pin is missing or not a sha256.
    InvalidPin {
        document_id: String,
    },
    /// The oracle carries no positive row anywhere, so agreement with absence
    /// would be vacuous. It cannot give a pass.
    OracleVacuous,
    /// The oracle is not independent of the SUT.
    NotIndependent(NativeCalloutOracleIndependenceError),
}

impl fmt::Display for NativeCalloutOracleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema => write!(formatter, "callout oracle schema is not recognised"),
            Self::InvalidPin { document_id } => write!(
                formatter,
                "callout oracle pin `{document_id}` is not a sha256"
            ),
            Self::OracleVacuous => write!(
                formatter,
                "callout oracle carries no positive row: vacuous, cannot pass"
            ),
            Self::NotIndependent(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NativeCalloutOracleError {}

/// Positive rows of one family, across the whole oracle or one document.
pub fn callout_oracle_positive_rows(
    oracle: &NativeCalloutOracle,
    document_id: Option<&str>,
    family: CalloutTargetClass,
) -> usize {
    oracle
        .rows
        .iter()
        .filter(|row| row.family == family)
        .filter(|row| document_id.is_none_or(|id| row.document_id == id))
        .count()
}

/// Require the oracle to be independent and to carry at least one positive row.
///
/// The equation family is deliberately **not** required to be positive:
/// `callout_matching_semantics.yaml` fixes
/// `equation.inventory_source = source_not_established` and
/// `positive_match = forbidden_until_inventory_proven`, so an oracle with no
/// equation marker row is the declared state, not a defect. An oracle with no
/// positive row at all is still refused as vacuous.
pub fn require_non_vacuous_callout_oracle(
    oracle: &NativeCalloutOracle,
) -> Result<(), NativeCalloutOracleError> {
    if oracle.schema_version != NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION {
        return Err(NativeCalloutOracleError::InvalidSchema);
    }
    require_independent_callout_oracle(&NativeCalloutOracleProvenance {
        generator_id: oracle.generator_id.clone(),
        backend: oracle.backend.clone(),
        sha256: oracle
            .pins
            .first()
            .map(|pin| pin.sha256.clone())
            .unwrap_or_default(),
    })
    .map_err(NativeCalloutOracleError::NotIndependent)?;
    if oracle.pins.is_empty() {
        return Err(NativeCalloutOracleError::InvalidPin {
            document_id: String::new(),
        });
    }
    for pin in &oracle.pins {
        if !is_sha256(&pin.sha256) {
            return Err(NativeCalloutOracleError::InvalidPin {
                document_id: pin.document_id.clone(),
            });
        }
    }
    if oracle.rows.is_empty() {
        return Err(NativeCalloutOracleError::OracleVacuous);
    }
    Ok(())
}

/// Why a required family was not met by the native lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutUnmetReason {
    /// The document admitted no tokens at all.
    LaneAbsent,
    /// The native plane admitted tokens but no marker-shaped run of the family.
    MarkerShapeUnreachable,
}

/// The closed disposition of one document × family. `out_of_scope` is
/// structurally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutFamilyDisposition {
    /// The native lane admitted at least one marker of the family.
    Measured { native_markers: usize },
    /// The oracle requires the family but the native lane admitted nothing.
    Unmet { reason: NativeCalloutUnmetReason },
    /// The oracle carries no positive row for this document × family, so there
    /// is nothing to meet.
    NotObserved,
}

/// One document × family disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCalloutFamilyEvaluation {
    pub document_id: String,
    pub family: CalloutTargetClass,
    pub disposition: NativeCalloutFamilyDisposition,
}

/// Compare one document's native callout band against the independent oracle.
///
/// Every family gets exactly one disposition. A family the native lane cannot
/// observe is `unmet` with a typed reason when the oracle requires it, and
/// `not_observed` when the oracle does not — never `out_of_scope`.
pub fn evaluate_callout_family_rows(
    outcome: &NativeCalloutFromInputOutcome,
    oracle: &NativeCalloutOracle,
) -> Result<Vec<NativeCalloutFamilyEvaluation>, NativeCalloutOracleError> {
    if oracle.schema_version != NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION {
        return Err(NativeCalloutOracleError::InvalidSchema);
    }
    let document_id = outcome.identity.document_id.as_str();
    let mut evaluations = Vec::with_capacity(NATIVE_CALLOUT_FAMILIES.len());
    for family in NATIVE_CALLOUT_FAMILIES {
        let native_markers = outcome.admitted_markers(family);
        let required = callout_oracle_positive_rows(oracle, Some(document_id), family);
        let disposition = if native_markers > 0 {
            NativeCalloutFamilyDisposition::Measured { native_markers }
        } else if required == 0 {
            NativeCalloutFamilyDisposition::NotObserved
        } else {
            NativeCalloutFamilyDisposition::Unmet {
                reason: unmet_reason(outcome),
            }
        };
        evaluations.push(NativeCalloutFamilyEvaluation {
            document_id: document_id.to_owned(),
            family,
            disposition,
        });
    }
    Ok(evaluations)
}

fn unmet_reason(outcome: &NativeCalloutFromInputOutcome) -> NativeCalloutUnmetReason {
    match outcome.absence {
        Some(NativeCalloutInputAbsence::LaneAbsent) => NativeCalloutUnmetReason::LaneAbsent,
        _ => NativeCalloutUnmetReason::MarkerShapeUnreachable,
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Redaction-safe digest of one oracle element's bytes.
pub fn oracle_content_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
