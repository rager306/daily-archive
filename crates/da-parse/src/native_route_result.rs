//! Typed end-to-end native route result aggregate (S06/T01).
//!
//! This module is the missing carrier for the S06 promise: one typed
//! document-level result that joins the S03 layout plane
//! (`LayoutDocument` + `LayoutLattice` + `LexemeRow` + the full segmentation
//! outcome and heading-aware regions) with the S04 header/bibliography bands
//! and the S05 body/section/visual/callout bands. The only fulltext composer
//! that existed before took a frozen foreign envelope; nothing folded the
//! native plane together with its bands into a single typed document result, so
//! a receipt, a governor signal and a hermetic audit had nothing to attach to.
//!
//! The module is a pure function of typed plane inputs and independent typed
//! oracles. It imports nothing but `crate`, `serde`, `sha2` and `std`; it never
//! names a foreign envelope or a transport type, never reads a path, an
//! environment variable, a clock or a process, and introduces no retry. Band
//! refusals are carried as a closed disposition rather than an error, so a
//! refusal can never be mistaken for a measurement and can never silently
//! become a zero.
//!
//! Slice-boundary facts (which cascade stages are in the slice, which are
//! outside it and why, and which lanes the slice declares) are *resolved from
//! DATA by pointer* and handed in as [`NativeRouteBoundary`]. This module does
//! not restate stage ids or corpus pins as literals: it validates the declared
//! boundary against the cascade stage set it is given and refuses any stage it
//! cannot classify.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BibliographyAtom, BibliographyAtomDisposition, BodyCoverageVerdict, BodyGapReason,
    BodyJoinPolicy, BodyUnitInventory, BodyUnitRung, FullSegmentationOutcome, FullSemanticRegion,
    HeaderAtom, HeaderAtomDisposition, HeadingRegionConfig, LayoutDocument, LayoutLattice,
    LexemeRow, LexiconBundle, NATIVE_CALLOUT_FAMILIES, NATIVE_CALLOUT_REGIONS,
    NativeBibliographyFromInputOutcome, NativeBodyText, NativeCalloutFamilyDisposition,
    NativeCalloutFromInputOutcome, NativeCalloutOracle, NativeCalloutUnmetReason,
    NativeHeaderFromInputOutcome, NativeHeadingRegionOutcome, NativeRegionCarrier,
    NativeSectionNotAssessable, NativeSectionOracleRow, NativeSectionReadingOrderConfig,
    NativeSectionRegionVerdict, NativeSectionRowVerdict, NativeVisualFamily,
    NativeVisualFamilyDisposition, NativeVisualFromInputConfig, NativeVisualFromInputOutcome,
    NativeVisualOracle, NativeVisualUnmetReason, SegmentationConfig, assign_heading_regions,
    build_native_bibliography_input, build_native_callout_input, build_native_header_input,
    build_native_visual_input, build_native_word_region_carrier, classify_body_plane,
    derive_native_section_reading_order, evaluate_callout_family_rows,
    evaluate_native_section_rows, evaluate_visual_family_rows, localize_body_plane_checked,
    native_section_regions, project_native_words, segment_document_full,
};

/// Schema tag of the aggregate.
pub const NATIVE_ROUTE_RESULT_SCHEMA_VERSION: &str = "da-parse.native-route-result/v1";

/// The declared family order. Every declared family carries exactly one
/// disposition; the set is closed and never abbreviated.
pub const NATIVE_ROUTE_FAMILY_ORDER: [NativeRouteFamily; 14] = [
    NativeRouteFamily::HeaderDate,
    NativeRouteFamily::HeaderPersonalName,
    NativeRouteFamily::HeaderAffiliation,
    NativeRouteFamily::HeaderAffiliationLink,
    NativeRouteFamily::BibliographyCitationFields,
    NativeRouteFamily::BibliographyReferenceSegments,
    NativeRouteFamily::BibliographyCitationAnchors,
    NativeRouteFamily::BodyAccounting,
    NativeRouteFamily::BodySections,
    NativeRouteFamily::BodyReadingOrder,
    NativeRouteFamily::Figure,
    NativeRouteFamily::Table,
    NativeRouteFamily::Formula,
    NativeRouteFamily::CalloutMarker,
];

/// Closed family-disposition vocabulary tokens. `out_of_scope` is deliberately
/// absent: a required family can never be out of scope.
pub const NATIVE_ROUTE_FAMILY_DISPOSITION_VOCABULARY: [&str; 4] =
    ["measured", "unmet", "not_observed", "refused"];

/// Closed stage-stance vocabulary tokens.
pub const NATIVE_ROUTE_STAGE_STANCE_VOCABULARY: [&str; 4] = [
    "native_measured",
    "not_qualified",
    "out_of_scope",
    "admission_boundary",
];

/// Closed out-of-slice reason tokens, in DATA order.
pub const NATIVE_ROUTE_OUT_OF_SLICE_REASON_VOCABULARY: [&str; 5] = [
    "java_authority",
    "visual_heads_not_measured",
    "java_tei_coordinates",
    "profile_dependent_consolidation",
    "java_tei_lift",
];

/// Closed band-refusal reason tokens.
pub const NATIVE_ROUTE_REFUSAL_REASON_VOCABULARY: [&str; 7] = [
    "header_band_refused",
    "bibliography_band_refused",
    "body_text_band_refused",
    "section_band_refused",
    "visual_band_refused",
    "callout_band_refused",
    "oracle_refused",
];

/// One declared family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteFamily {
    HeaderDate,
    HeaderPersonalName,
    HeaderAffiliation,
    HeaderAffiliationLink,
    BibliographyCitationFields,
    BibliographyReferenceSegments,
    BibliographyCitationAnchors,
    BodyAccounting,
    BodySections,
    BodyReadingOrder,
    Figure,
    Table,
    Formula,
    CalloutMarker,
}

impl NativeRouteFamily {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteFamily::HeaderDate => "header_date",
            NativeRouteFamily::HeaderPersonalName => "header_personal_name",
            NativeRouteFamily::HeaderAffiliation => "header_affiliation",
            NativeRouteFamily::HeaderAffiliationLink => "header_affiliation_link",
            NativeRouteFamily::BibliographyCitationFields => "bibliography_citation_fields",
            NativeRouteFamily::BibliographyReferenceSegments => "bibliography_reference_segments",
            NativeRouteFamily::BibliographyCitationAnchors => "bibliography_citation_anchors",
            NativeRouteFamily::BodyAccounting => "body_accounting",
            NativeRouteFamily::BodySections => "body_sections",
            NativeRouteFamily::BodyReadingOrder => "body_reading_order",
            NativeRouteFamily::Figure => "figure",
            NativeRouteFamily::Table => "table",
            NativeRouteFamily::Formula => "formula",
            NativeRouteFamily::CalloutMarker => "callout_marker",
        }
    }
}

/// Why a required family was not met. Every member reuses an existing typed
/// S04/S05 reason; there is no free string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteUnmetReason {
    /// The declared region carries no admitted token in the carrier, so the
    /// family has no native population at all.
    RegionNoAdmittedTokens { region: FullSemanticRegion },
    /// The header band ran and reported a non-recognized atom disposition.
    HeaderAtom { disposition: HeaderAtomDisposition },
    /// The bibliography band ran and reported a non-recognized atom
    /// disposition.
    BibliographyAtom {
        disposition: BibliographyAtomDisposition,
    },
    /// The native visual lane admitted nothing while the oracle requires the
    /// family.
    Visual { reason: NativeVisualUnmetReason },
    /// The native callout lane admitted nothing while the oracle requires the
    /// family.
    Callout { reason: NativeCalloutUnmetReason },
    /// The section oracle could not assess the region.
    Section { reason: NativeSectionNotAssessable },
}

/// Why a band could not produce a measured disposition. Closed; the fine
/// per-band failure class is the responsibility of the typed failure recovery
/// layer, not of this aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteRefusalReason {
    HeaderBandRefused,
    BibliographyBandRefused,
    BodyTextBandRefused,
    SectionBandRefused,
    VisualBandRefused,
    CalloutBandRefused,
    OracleRefused,
}

impl NativeRouteRefusalReason {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteRefusalReason::HeaderBandRefused => "header_band_refused",
            NativeRouteRefusalReason::BibliographyBandRefused => "bibliography_band_refused",
            NativeRouteRefusalReason::BodyTextBandRefused => "body_text_band_refused",
            NativeRouteRefusalReason::SectionBandRefused => "section_band_refused",
            NativeRouteRefusalReason::VisualBandRefused => "visual_band_refused",
            NativeRouteRefusalReason::CalloutBandRefused => "callout_band_refused",
            NativeRouteRefusalReason::OracleRefused => "oracle_refused",
        }
    }
}

/// The closed disposition of one document × family. `out_of_scope` is
/// structurally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteFamilyDisposition {
    Measured { observed: u64 },
    Unmet { reason: NativeRouteUnmetReason },
    NotObserved,
    Refused { reason: NativeRouteRefusalReason },
}

impl NativeRouteFamilyDisposition {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteFamilyDisposition::Measured { .. } => "measured",
            NativeRouteFamilyDisposition::Unmet { .. } => "unmet",
            NativeRouteFamilyDisposition::NotObserved => "not_observed",
            NativeRouteFamilyDisposition::Refused { .. } => "refused",
        }
    }
}

/// One family's disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteFamilyCell {
    pub family: NativeRouteFamily,
    pub disposition: NativeRouteFamilyDisposition,
}

/// Why a cascade stage is outside the declared slice. Mirrors the DATA
/// vocabulary token-for-token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteOutOfSliceReason {
    JavaAuthority,
    VisualHeadsNotMeasured,
    JavaTeiCoordinates,
    ProfileDependentConsolidation,
    JavaTeiLift,
}

impl NativeRouteOutOfSliceReason {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteOutOfSliceReason::JavaAuthority => "java_authority",
            NativeRouteOutOfSliceReason::VisualHeadsNotMeasured => "visual_heads_not_measured",
            NativeRouteOutOfSliceReason::JavaTeiCoordinates => "java_tei_coordinates",
            NativeRouteOutOfSliceReason::ProfileDependentConsolidation => {
                "profile_dependent_consolidation"
            }
            NativeRouteOutOfSliceReason::JavaTeiLift => "java_tei_lift",
        }
    }
}

/// The closed stance of one cascade stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteStageStance {
    /// The stage is inside the slice and its native surface was measured.
    NativeMeasured,
    /// The stage is inside the slice but was not qualified by this run.
    NotQualified,
    /// The stage is outside the slice for a typed reason.
    OutOfScope { reason: NativeRouteOutOfSliceReason },
    /// The stage is the identity/profile admission boundary; it is neither an
    /// in-slice lane nor a counted out-of-slice stage.
    AdmissionBoundary,
}

impl NativeRouteStageStance {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteStageStance::NativeMeasured => "native_measured",
            NativeRouteStageStance::NotQualified => "not_qualified",
            NativeRouteStageStance::OutOfScope { .. } => "out_of_scope",
            NativeRouteStageStance::AdmissionBoundary => "admission_boundary",
        }
    }
}

/// One stage's stance, with the stage id resolved from DATA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteStage {
    pub stage_id: String,
    pub stance: NativeRouteStageStance,
}

/// One out-of-slice stage, with its typed reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteOutOfSliceStage {
    pub stage_id: String,
    pub reason: NativeRouteOutOfSliceReason,
}

/// The resolved slice boundary. Built by pointer from the declaration DATA
/// (membership/reasons and the cascade stage set), then handed to
/// [`build_native_route_result`]. This module never restates a stage id as a
/// literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteBoundary {
    pub admission_stage_id: String,
    pub in_slice_stage_ids: Vec<String>,
    pub out_of_slice: Vec<NativeRouteOutOfSliceStage>,
    pub lane_ids: Vec<String>,
}

/// A boundary that cannot classify the whole cascade is refused by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRouteBoundaryError {
    EmptyCascade,
    EmptyInSlice,
    EmptyLanes,
    DuplicateCascadeStage { stage_id: String },
    DuplicateClassification { stage_id: String },
    AdmissionStageNotInCascade { stage_id: String },
    UnknownClassifiedStage { stage_id: String },
    UnclassifiedStage { stage_id: String },
}

impl fmt::Display for NativeRouteBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native route boundary rejected: {self:?}")
    }
}

impl std::error::Error for NativeRouteBoundaryError {}

impl NativeRouteBoundary {
    /// The stance of one cascade stage, or `None` when the boundary does not
    /// classify it.
    fn stance_of(&self, stage_id: &str) -> Option<NativeRouteStageStance> {
        if stage_id == self.admission_stage_id {
            return Some(NativeRouteStageStance::AdmissionBoundary);
        }
        if self.in_slice_stage_ids.iter().any(|id| id == stage_id) {
            return Some(NativeRouteStageStance::NativeMeasured);
        }
        self.out_of_slice
            .iter()
            .find(|entry| entry.stage_id == stage_id)
            .map(|entry| NativeRouteStageStance::OutOfScope {
                reason: entry.reason,
            })
    }

    /// Validate that the boundary classifies every cascade stage exactly once.
    ///
    /// A missing, duplicated or unknown stage is refused by name: the union of
    /// admission + in-slice + out-of-slice must equal the cascade set and the
    /// three parts must be disjoint.
    pub fn validate(&self, cascade_stage_ids: &[String]) -> Result<(), NativeRouteBoundaryError> {
        if cascade_stage_ids.is_empty() {
            return Err(NativeRouteBoundaryError::EmptyCascade);
        }
        if self.in_slice_stage_ids.is_empty() {
            return Err(NativeRouteBoundaryError::EmptyInSlice);
        }
        if self.lane_ids.is_empty() {
            return Err(NativeRouteBoundaryError::EmptyLanes);
        }
        let mut cascade: BTreeSet<&str> = BTreeSet::new();
        for id in cascade_stage_ids {
            if !cascade.insert(id.as_str()) {
                return Err(NativeRouteBoundaryError::DuplicateCascadeStage {
                    stage_id: id.clone(),
                });
            }
        }
        if !cascade.contains(self.admission_stage_id.as_str()) {
            return Err(NativeRouteBoundaryError::AdmissionStageNotInCascade {
                stage_id: self.admission_stage_id.clone(),
            });
        }
        let mut classified: BTreeSet<&str> = BTreeSet::new();
        for id in self.in_slice_stage_ids.iter().map(String::as_str).chain(
            self.out_of_slice
                .iter()
                .map(|entry| entry.stage_id.as_str()),
        ) {
            if !classified.insert(id) {
                return Err(NativeRouteBoundaryError::DuplicateClassification {
                    stage_id: id.to_string(),
                });
            }
        }
        for id in &classified {
            if !cascade.contains(id) {
                return Err(NativeRouteBoundaryError::UnknownClassifiedStage {
                    stage_id: id.to_string(),
                });
            }
        }
        for id in &cascade {
            if *id != self.admission_stage_id.as_str() && !classified.contains(id) {
                return Err(NativeRouteBoundaryError::UnclassifiedStage {
                    stage_id: id.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Resolve one stance per cascade stage, in the cascade's own order.
    pub fn stances(
        &self,
        cascade_stage_ids: &[String],
    ) -> Result<Vec<NativeRouteStage>, NativeRouteBoundaryError> {
        self.validate(cascade_stage_ids)?;
        let mut stages = Vec::with_capacity(cascade_stage_ids.len());
        for stage_id in cascade_stage_ids {
            let stance = self.stance_of(stage_id).ok_or_else(|| {
                NativeRouteBoundaryError::UnclassifiedStage {
                    stage_id: stage_id.clone(),
                }
            })?;
            stages.push(NativeRouteStage {
                stage_id: stage_id.clone(),
                stance,
            });
        }
        Ok(stages)
    }

    /// Declared lane ids, in declaration order.
    pub fn lane_ids(&self) -> &[String] {
        &self.lane_ids
    }
}

/// The closed P1 body verdict. Mirrors [`BodyCoverageVerdict`]; the S03 enum
/// carries only `Serialize`, so a round-trippable mirror is required. The
/// mapping is exhaustive, so a new S03 variant breaks the build rather than
/// silently drifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteBodyVerdict {
    GranularityOnly,
    ContentLoss,
    NotAssessable,
}

impl NativeRouteBodyVerdict {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteBodyVerdict::GranularityOnly => "granularity_only",
            NativeRouteBodyVerdict::ContentLoss => "content_loss",
            NativeRouteBodyVerdict::NotAssessable => "not_assessable",
        }
    }
}

impl From<BodyCoverageVerdict> for NativeRouteBodyVerdict {
    fn from(verdict: BodyCoverageVerdict) -> Self {
        match verdict {
            BodyCoverageVerdict::GranularityOnly => NativeRouteBodyVerdict::GranularityOnly,
            BodyCoverageVerdict::ContentLoss => NativeRouteBodyVerdict::ContentLoss,
            BodyCoverageVerdict::NotAssessable => NativeRouteBodyVerdict::NotAssessable,
        }
    }
}

/// The closed P1 body gap reason. Mirrors [`BodyGapReason`] with `u64` counts;
/// see [`NativeRouteBodyVerdict`] for why the mirror exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRouteBodyGapReason {
    Matched,
    P1NativeTextAbsent {
        missing_units: u64,
    },
    P1LocalizationUncertain {
        present_in_token_units: u64,
        spanning_token_units: u64,
        character_material_units: u64,
    },
    P2RegionAssignmentGap {
        units_in_plane_not_in_body: u64,
    },
    P2LocalizationUncertain {
        present_in_token_units: u64,
        spanning_token_units: u64,
        character_material_units: u64,
    },
    ScopeMismatch,
    ScopeJoinMismatch,
    NormalizationMismatch,
    OracleEmpty,
    PopulationIncomplete {
        documents_absent: u64,
    },
}

impl NativeRouteBodyGapReason {
    /// Stable token, closed by the exhaustive match. The P1 uncertainty token
    /// is the S05 residual name.
    pub fn token(self) -> &'static str {
        match self {
            NativeRouteBodyGapReason::Matched => "matched",
            NativeRouteBodyGapReason::P1NativeTextAbsent { .. } => "p1_native_text_absent",
            NativeRouteBodyGapReason::P1LocalizationUncertain { .. } => "p1_localization_uncertain",
            NativeRouteBodyGapReason::P2RegionAssignmentGap { .. } => "p2_region_assignment_gap",
            NativeRouteBodyGapReason::P2LocalizationUncertain { .. } => "p2_localization_uncertain",
            NativeRouteBodyGapReason::ScopeMismatch => "scope_mismatch",
            NativeRouteBodyGapReason::ScopeJoinMismatch => "scope_join_mismatch",
            NativeRouteBodyGapReason::NormalizationMismatch => "normalization_mismatch",
            NativeRouteBodyGapReason::OracleEmpty => "oracle_empty",
            NativeRouteBodyGapReason::PopulationIncomplete { .. } => "population_incomplete",
        }
    }
}

impl From<&BodyGapReason> for NativeRouteBodyGapReason {
    fn from(reason: &BodyGapReason) -> Self {
        match reason {
            BodyGapReason::Matched => NativeRouteBodyGapReason::Matched,
            BodyGapReason::P1NativeTextAbsent { missing_units } => {
                NativeRouteBodyGapReason::P1NativeTextAbsent {
                    missing_units: *missing_units as u64,
                }
            }
            BodyGapReason::P1LocalizationUncertain {
                present_in_token_units,
                spanning_token_units,
                character_material_units,
            } => NativeRouteBodyGapReason::P1LocalizationUncertain {
                present_in_token_units: *present_in_token_units as u64,
                spanning_token_units: *spanning_token_units as u64,
                character_material_units: *character_material_units as u64,
            },
            BodyGapReason::P2RegionAssignmentGap {
                units_in_plane_not_in_body,
            } => NativeRouteBodyGapReason::P2RegionAssignmentGap {
                units_in_plane_not_in_body: *units_in_plane_not_in_body as u64,
            },
            BodyGapReason::P2LocalizationUncertain {
                present_in_token_units,
                spanning_token_units,
                character_material_units,
            } => NativeRouteBodyGapReason::P2LocalizationUncertain {
                present_in_token_units: *present_in_token_units as u64,
                spanning_token_units: *spanning_token_units as u64,
                character_material_units: *character_material_units as u64,
            },
            BodyGapReason::ScopeMismatch => NativeRouteBodyGapReason::ScopeMismatch,
            BodyGapReason::ScopeJoinMismatch => NativeRouteBodyGapReason::ScopeJoinMismatch,
            BodyGapReason::NormalizationMismatch => NativeRouteBodyGapReason::NormalizationMismatch,
            BodyGapReason::OracleEmpty => NativeRouteBodyGapReason::OracleEmpty,
            BodyGapReason::PopulationIncomplete { documents_absent } => {
                NativeRouteBodyGapReason::PopulationIncomplete {
                    documents_absent: *documents_absent as u64,
                }
            }
        }
    }
}

/// The body axis: either a measured P1 verdict or a typed refusal.
///
/// The measured form reproduces the S05 residual verbatim
/// (`not_assessable` / `p1_localization_uncertain` with `plane_absent_units`),
/// carries it typed, and never tries to resolve it — the frozen P1 semantics
/// stay frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum NativeRouteBodyAxis {
    Measured {
        verdict: NativeRouteBodyVerdict,
        reason: NativeRouteBodyGapReason,
        d498_reopen_required: bool,
        plane_absent_units: u64,
        plane_present_in_token_units: u64,
        plane_spanning_token_units: u64,
        plane_character_material_units: u64,
        oracle_units: u64,
        native_plane_units: u64,
    },
    Refused {
        reason: NativeRouteRefusalReason,
    },
}

impl NativeRouteBodyAxis {
    /// The S05 residual, named without resolving it: `not_assessable` /
    /// `p1_localization_uncertain`, no proven absence, no D498 reopen.
    pub fn is_frozen_s05_residual(&self) -> bool {
        matches!(
            self,
            NativeRouteBodyAxis::Measured {
                verdict: NativeRouteBodyVerdict::NotAssessable,
                reason: NativeRouteBodyGapReason::P1LocalizationUncertain { .. },
                d498_reopen_required: false,
                plane_absent_units: 0,
                ..
            }
        )
    }
}

/// Document identity of the aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteDocumentIdentity {
    pub document_id: String,
    pub source_sha256: String,
}

/// Identity of the S03 plane the aggregate was built over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRoutePlaneIdentity {
    pub lane_ids: Vec<String>,
    pub segmentation_digest: String,
    pub word_projection_digest: String,
    pub heading_digest: String,
    pub carrier_digest: String,
}

/// One document's end-to-end typed native route result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteResult {
    pub schema_version: String,
    pub identity: NativeRouteDocumentIdentity,
    pub plane: NativeRoutePlaneIdentity,
    pub stages: Vec<NativeRouteStage>,
    pub families: Vec<NativeRouteFamilyCell>,
    pub body_axis: NativeRouteBodyAxis,
    pub digest: String,
}

/// A plane-level stage the aggregate cannot proceed without.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRoutePlaneFailure {
    WordProjection,
    Segmentation,
    HeadingRegions,
    RegionCarrier,
}

impl NativeRoutePlaneFailure {
    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            NativeRoutePlaneFailure::WordProjection => "word_projection",
            NativeRoutePlaneFailure::Segmentation => "segmentation",
            NativeRoutePlaneFailure::HeadingRegions => "heading_regions",
            NativeRoutePlaneFailure::RegionCarrier => "region_carrier",
        }
    }
}

/// Closed structural invariants of a route result. Every variant names the
/// offending token so a mutation is rejected by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRouteValidationError {
    SchemaMismatch { found: String },
    StageMissing { stage_id: String },
    StageDuplicated { stage_id: String },
    StageOutsideCascade { stage_id: String },
    StageStanceMismatch { stage_id: String },
    FamilyMissing { family: &'static str },
    FamilyDuplicated { family: &'static str },
    BoundaryInvalid(NativeRouteBoundaryError),
    BodyGapUntyped { verdict: &'static str },
}

impl fmt::Display for NativeRouteValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native route result invariant violated: {self:?}"
        )
    }
}

impl std::error::Error for NativeRouteValidationError {}

/// Build/validation failure of the aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRouteResultError {
    /// A plane stage refused; the aggregate cannot exist.
    Plane { stage: NativeRoutePlaneFailure },
    /// The declared boundary does not classify the whole cascade.
    Boundary(NativeRouteBoundaryError),
    /// The aggregate itself violates a closed invariant.
    Invalid(NativeRouteValidationError),
}

impl fmt::Display for NativeRouteResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native route result rejected: {self:?}")
    }
}

impl std::error::Error for NativeRouteResultError {}

/// The independent body oracle: the recorded native-plane inventory and the
/// recorded oracle inventory, already tokenized at one declared rung.
#[derive(Clone, PartialEq, Eq)]
pub struct NativeRouteBodyOracle {
    pub native_plane_units: BodyUnitInventory,
    pub oracle_units: BodyUnitInventory,
}

impl fmt::Debug for NativeRouteBodyOracle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeRouteBodyOracle")
            .field("rung", &self.native_plane_units.rung())
            .field("native_plane_units", &self.native_plane_units.total())
            .field("oracle_units", &self.oracle_units.total())
            .finish()
    }
}

/// Everything the aggregate needs. Plane and band inputs are typed data; no
/// path, transport or envelope is nameable here.
pub struct NativeRouteInput<'a> {
    pub document: &'a LayoutDocument,
    pub lattice: &'a LayoutLattice,
    pub lexemes: &'a [LexemeRow],
    pub lexicon: &'a LexiconBundle,
    pub boundary: &'a NativeRouteBoundary,
    pub cascade_stage_ids: &'a [String],
    pub body_oracle: &'a NativeRouteBodyOracle,
    pub section_lane_id: &'a str,
    pub section_oracle_independent: bool,
    pub section_oracle: &'a [NativeSectionOracleRow],
    pub visual_oracle: &'a NativeVisualOracle,
    pub callout_oracle: &'a NativeCalloutOracle,
}

/// Build the end-to-end typed result for one document.
///
/// The plane is folded in the declared order (`project_native_words` →
/// `segment_document_full` → `assign_heading_regions` →
/// `build_native_word_region_carrier`), then each band runs and its outcome is
/// mapped into exactly one typed disposition per declared family. A band
/// refusal becomes a `refused` cell, never an error and never a zero.
pub fn build_native_route_result(
    input: &NativeRouteInput<'_>,
) -> Result<NativeRouteResult, NativeRouteResultError> {
    let stages = input
        .boundary
        .stances(input.cascade_stage_ids)
        .map_err(NativeRouteResultError::Boundary)?;

    let projection = project_native_words(input.document, input.lexemes).map_err(|_| {
        NativeRouteResultError::Plane {
            stage: NativeRoutePlaneFailure::WordProjection,
        }
    })?;
    let segmentation = segment_document_full(
        input.document,
        input.lattice,
        input.lexemes,
        SegmentationConfig::default(),
    )
    .map_err(|_| NativeRouteResultError::Plane {
        stage: NativeRoutePlaneFailure::Segmentation,
    })?;
    let heading = assign_heading_regions(
        input.document,
        input.lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .map_err(|_| NativeRouteResultError::Plane {
        stage: NativeRoutePlaneFailure::HeadingRegions,
    })?;
    let carrier = build_native_word_region_carrier(
        input.document,
        input.lattice,
        &projection,
        &heading,
        &native_section_regions(),
    )
    .map_err(|_| NativeRouteResultError::Plane {
        stage: NativeRoutePlaneFailure::RegionCarrier,
    })?;

    let plane = NativeRoutePlaneIdentity {
        lane_ids: input.boundary.lane_ids().to_vec(),
        segmentation_digest: segmentation.order_digest.clone(),
        word_projection_digest: projection.identity.order_digest.clone(),
        heading_digest: heading.order_digest.clone(),
        carrier_digest: carrier.identity.order_digest.clone(),
    };

    let rung = input.body_oracle.native_plane_units.rung();
    let families = collect_families(input, &carrier, &segmentation, &heading, rung);
    let body_axis = collect_body_axis(input);

    let mut result = NativeRouteResult {
        schema_version: NATIVE_ROUTE_RESULT_SCHEMA_VERSION.to_owned(),
        identity: NativeRouteDocumentIdentity {
            document_id: input.document.document_id.clone(),
            source_sha256: input.document.source_pdf_sha256.clone(),
        },
        plane,
        stages,
        families,
        body_axis,
        digest: String::new(),
    };
    result.digest = result.compute_digest();
    result
        .validate(input.boundary, input.cascade_stage_ids)
        .map_err(NativeRouteResultError::Invalid)?;
    Ok(result)
}

fn collect_body_axis(input: &NativeRouteInput<'_>) -> NativeRouteBodyAxis {
    match localize_body_plane_checked(
        &input.body_oracle.native_plane_units,
        &input.body_oracle.oracle_units,
    ) {
        Ok(localization) => {
            let verdict = classify_body_plane(&localization);
            NativeRouteBodyAxis::Measured {
                verdict: verdict.verdict.into(),
                reason: NativeRouteBodyGapReason::from(&verdict.reason),
                d498_reopen_required: verdict.d498_reopen_required,
                plane_absent_units: localization.absent_from_plane as u64,
                plane_present_in_token_units: localization.uncertain_present_in_token_from_plane
                    as u64,
                plane_spanning_token_units: localization.uncertain_spanning_tokens_from_plane
                    as u64,
                plane_character_material_units: localization.uncertain_character_material_from_plane
                    as u64,
                oracle_units: localization.oracle_units as u64,
                native_plane_units: input.body_oracle.native_plane_units.total() as u64,
            }
        }
        Err(_) => NativeRouteBodyAxis::Refused {
            reason: NativeRouteRefusalReason::OracleRefused,
        },
    }
}

fn collect_families(
    input: &NativeRouteInput<'_>,
    carrier: &NativeRegionCarrier,
    segmentation: &FullSegmentationOutcome,
    heading: &NativeHeadingRegionOutcome,
    rung: BodyUnitRung,
) -> Vec<NativeRouteFamilyCell> {
    let mut cells = Vec::with_capacity(NATIVE_ROUTE_FAMILY_ORDER.len());

    // Header band (S04): four declared atoms.
    let header = build_native_header_input(carrier, input.lexicon);
    for (atom, family) in [
        (HeaderAtom::Date, NativeRouteFamily::HeaderDate),
        (
            HeaderAtom::PersonalName,
            NativeRouteFamily::HeaderPersonalName,
        ),
        (
            HeaderAtom::Affiliation,
            NativeRouteFamily::HeaderAffiliation,
        ),
        (
            HeaderAtom::AffiliationLink,
            NativeRouteFamily::HeaderAffiliationLink,
        ),
    ] {
        let disposition = match &header {
            Ok(outcome) => header_atom_disposition(outcome, atom),
            Err(_) => NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::HeaderBandRefused,
            },
        };
        cells.push(NativeRouteFamilyCell {
            family,
            disposition,
        });
    }

    // Bibliography band (S04): three declared atoms.
    let bibliography = build_native_bibliography_input(carrier, input.lexicon);
    for (atom, family) in [
        (
            BibliographyAtom::CitationFields,
            NativeRouteFamily::BibliographyCitationFields,
        ),
        (
            BibliographyAtom::ReferenceSegments,
            NativeRouteFamily::BibliographyReferenceSegments,
        ),
        (
            BibliographyAtom::CitationAnchors,
            NativeRouteFamily::BibliographyCitationAnchors,
        ),
    ] {
        let disposition = match &bibliography {
            Ok(outcome) => bibliography_atom_disposition(outcome, atom),
            Err(_) => NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::BibliographyBandRefused,
            },
        };
        cells.push(NativeRouteFamilyCell {
            family,
            disposition,
        });
    }

    // Body accounting (S05): the assembled body text over the Body run.
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::BodyAccounting,
        disposition: body_accounting_disposition(carrier, input.document, rung),
    });

    // Body sections and reading order (S05).
    let sections = derive_native_section_reading_order(
        input.lattice,
        carrier,
        heading,
        NativeSectionReadingOrderConfig::default(),
    );
    let (section_cell, reading_cell) = match &sections {
        Ok(outcome) => {
            let rows = evaluate_native_section_rows(
                outcome,
                input.section_lane_id,
                input.section_oracle,
                input.section_oracle_independent,
            );
            match rows {
                Ok(rows) => (
                    section_row_disposition(&rows),
                    section_row_disposition(&rows),
                ),
                Err(_) => (
                    NativeRouteFamilyDisposition::Refused {
                        reason: NativeRouteRefusalReason::SectionBandRefused,
                    },
                    NativeRouteFamilyDisposition::Refused {
                        reason: NativeRouteRefusalReason::SectionBandRefused,
                    },
                ),
            }
        }
        Err(_) => (
            NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::SectionBandRefused,
            },
            NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::SectionBandRefused,
            },
        ),
    };
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::BodySections,
        disposition: section_cell,
    });
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::BodyReadingOrder,
        disposition: reading_cell,
    });

    // Figure/table/formula (S05).
    let (figure, table, formula) = visual_dispositions(input, segmentation);
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::Figure,
        disposition: figure,
    });
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::Table,
        disposition: table,
    });
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::Formula,
        disposition: formula,
    });

    // Callout marker (S05).
    cells.push(NativeRouteFamilyCell {
        family: NativeRouteFamily::CalloutMarker,
        disposition: callout_disposition(input, segmentation),
    });

    cells.sort_by_key(|cell| {
        NATIVE_ROUTE_FAMILY_ORDER
            .iter()
            .position(|family| *family == cell.family)
            .unwrap_or(usize::MAX)
    });
    cells
}

fn header_atom_disposition(
    outcome: &NativeHeaderFromInputOutcome,
    atom: HeaderAtom,
) -> NativeRouteFamilyDisposition {
    match outcome.atoms.iter().find(|entry| entry.atom == atom) {
        Some(entry) if entry.recognized_count > 0 => NativeRouteFamilyDisposition::Measured {
            observed: entry.recognized_count as u64,
        },
        Some(entry) => NativeRouteFamilyDisposition::Unmet {
            reason: NativeRouteUnmetReason::HeaderAtom {
                disposition: entry.disposition,
            },
        },
        None => NativeRouteFamilyDisposition::Refused {
            reason: NativeRouteRefusalReason::HeaderBandRefused,
        },
    }
}

fn bibliography_atom_disposition(
    outcome: &NativeBibliographyFromInputOutcome,
    atom: BibliographyAtom,
) -> NativeRouteFamilyDisposition {
    match outcome.atoms.iter().find(|entry| entry.atom == atom) {
        Some(entry) if entry.recognized_count > 0 => NativeRouteFamilyDisposition::Measured {
            observed: entry.recognized_count as u64,
        },
        Some(entry) => NativeRouteFamilyDisposition::Unmet {
            reason: NativeRouteUnmetReason::BibliographyAtom {
                disposition: entry.disposition,
            },
        },
        None => NativeRouteFamilyDisposition::Refused {
            reason: NativeRouteRefusalReason::BibliographyBandRefused,
        },
    }
}

fn body_accounting_disposition(
    carrier: &NativeRegionCarrier,
    document: &LayoutDocument,
    rung: BodyUnitRung,
) -> NativeRouteFamilyDisposition {
    let run = match carrier.run(FullSemanticRegion::Body) {
        Some(run) if !run.tokens.is_empty() => run,
        _ => {
            return NativeRouteFamilyDisposition::Unmet {
                reason: NativeRouteUnmetReason::RegionNoAdmittedTokens {
                    region: FullSemanticRegion::Body,
                },
            };
        }
    };
    let fragments: Vec<&str> = run.tokens.iter().map(|token| token.text()).collect();
    match NativeBodyText::from_fragments(
        document.document_id.clone(),
        document.source_pdf_sha256.clone(),
        fragments.len(),
        Some((run.start.page_ordinal, run.end.page_ordinal)),
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        fragments.iter().copied(),
        rung,
    ) {
        Ok(body) if body.token_count > 0 => NativeRouteFamilyDisposition::Measured {
            observed: body.token_count as u64,
        },
        Ok(_) => NativeRouteFamilyDisposition::Unmet {
            reason: NativeRouteUnmetReason::RegionNoAdmittedTokens {
                region: FullSemanticRegion::Body,
            },
        },
        Err(_) => NativeRouteFamilyDisposition::Refused {
            reason: NativeRouteRefusalReason::BodyTextBandRefused,
        },
    }
}

fn section_row_disposition(rows: &[NativeSectionRegionVerdict]) -> NativeRouteFamilyDisposition {
    let mut agreements = 0usize;
    let mut first_reason: Option<NativeSectionNotAssessable> = None;
    for row in rows {
        match &row.verdict {
            NativeSectionRowVerdict::Agreement => agreements += 1,
            NativeSectionRowVerdict::NotAssessable { reason } => {
                if first_reason.is_none() {
                    first_reason = Some(*reason);
                }
            }
            NativeSectionRowVerdict::BoundaryDisplaced { .. }
            | NativeSectionRowVerdict::CountMismatch { .. } => {
                if first_reason.is_none() {
                    first_reason = Some(NativeSectionNotAssessable::OracleRowMissing);
                }
            }
        }
    }
    if agreements > 0 {
        return NativeRouteFamilyDisposition::Measured {
            observed: agreements as u64,
        };
    }
    match first_reason {
        Some(reason) => NativeRouteFamilyDisposition::Unmet {
            reason: NativeRouteUnmetReason::Section { reason },
        },
        None => NativeRouteFamilyDisposition::NotObserved,
    }
}

fn visual_dispositions(
    input: &NativeRouteInput<'_>,
    segmentation: &FullSegmentationOutcome,
) -> (
    NativeRouteFamilyDisposition,
    NativeRouteFamilyDisposition,
    NativeRouteFamilyDisposition,
) {
    let outcome: NativeVisualFromInputOutcome = match build_native_visual_input(
        input.document,
        input.lattice,
        segmentation,
        NativeVisualFromInputConfig::default(),
    ) {
        Ok(outcome) => outcome,
        Err(_) => {
            let refused = NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::VisualBandRefused,
            };
            return (refused, refused, refused);
        }
    };
    let evaluations = match evaluate_visual_family_rows(&outcome, input.visual_oracle) {
        Ok(evaluations) => evaluations,
        Err(_) => {
            let refused = NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::OracleRefused,
            };
            return (refused, refused, refused);
        }
    };
    let pick = |family: NativeVisualFamily| {
        evaluations
            .iter()
            .find(|evaluation| evaluation.family == family)
            .map(|evaluation| match evaluation.disposition {
                NativeVisualFamilyDisposition::Measured { native_heads } => {
                    NativeRouteFamilyDisposition::Measured {
                        observed: native_heads as u64,
                    }
                }
                NativeVisualFamilyDisposition::Unmet { reason } => {
                    NativeRouteFamilyDisposition::Unmet {
                        reason: NativeRouteUnmetReason::Visual { reason },
                    }
                }
                NativeVisualFamilyDisposition::NotObserved => {
                    NativeRouteFamilyDisposition::NotObserved
                }
            })
            .unwrap_or(NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::VisualBandRefused,
            })
    };
    (
        pick(NativeVisualFamily::Figure),
        pick(NativeVisualFamily::Table),
        pick(NativeVisualFamily::Formula),
    )
}

fn callout_disposition(
    input: &NativeRouteInput<'_>,
    segmentation: &FullSegmentationOutcome,
) -> NativeRouteFamilyDisposition {
    let outcome: NativeCalloutFromInputOutcome = match build_native_callout_input(
        input.document,
        input.lattice,
        input.lexemes,
        segmentation,
        &NATIVE_CALLOUT_REGIONS,
    ) {
        Ok(outcome) => outcome,
        Err(_) => {
            return NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::CalloutBandRefused,
            };
        }
    };
    let evaluations = match evaluate_callout_family_rows(&outcome, input.callout_oracle) {
        Ok(evaluations) => evaluations,
        Err(_) => {
            return NativeRouteFamilyDisposition::Refused {
                reason: NativeRouteRefusalReason::OracleRefused,
            };
        }
    };
    let mut measured = 0u64;
    let mut unmet: Option<NativeCalloutUnmetReason> = None;
    let mut observed_any = false;
    for family in NATIVE_CALLOUT_FAMILIES {
        match evaluations
            .iter()
            .find(|evaluation| evaluation.family == family)
            .map(|evaluation| evaluation.disposition)
        {
            Some(NativeCalloutFamilyDisposition::Measured { native_markers }) => {
                measured += native_markers as u64;
                observed_any = true;
            }
            Some(NativeCalloutFamilyDisposition::Unmet { reason }) => {
                if unmet.is_none() {
                    unmet = Some(reason);
                }
            }
            Some(NativeCalloutFamilyDisposition::NotObserved) => observed_any = true,
            None => {
                return NativeRouteFamilyDisposition::Refused {
                    reason: NativeRouteRefusalReason::CalloutBandRefused,
                };
            }
        }
    }
    if measured > 0 {
        NativeRouteFamilyDisposition::Measured { observed: measured }
    } else if let Some(reason) = unmet {
        NativeRouteFamilyDisposition::Unmet {
            reason: NativeRouteUnmetReason::Callout { reason },
        }
    } else if observed_any {
        NativeRouteFamilyDisposition::NotObserved
    } else {
        NativeRouteFamilyDisposition::Refused {
            reason: NativeRouteRefusalReason::CalloutBandRefused,
        }
    }
}

impl NativeRouteResult {
    /// Deterministic digest over the typed fields. Built from explicit field
    /// formatting so the module needs no serialization dependency.
    pub fn compute_digest(&self) -> String {
        let mut canonical = String::new();
        let _ = write!(
            canonical,
            "{}|{}|{}|{}|{}|{}|{}",
            self.schema_version,
            self.identity.document_id,
            self.identity.source_sha256,
            self.plane.lane_ids.join(","),
            self.plane.segmentation_digest,
            self.plane.word_projection_digest,
            self.plane.heading_digest,
        );
        let _ = write!(canonical, "|{}", self.plane.carrier_digest);
        for stage in &self.stages {
            let _ = write!(canonical, "|s:{}:{}", stage.stage_id, stage.stance.token());
            if let NativeRouteStageStance::OutOfScope { reason } = stage.stance {
                let _ = write!(canonical, ":{}", reason.token());
            }
        }
        for cell in &self.families {
            let _ = write!(
                canonical,
                "|f:{}:{}",
                cell.family.token(),
                cell.disposition.token()
            );
            match cell.disposition {
                NativeRouteFamilyDisposition::Measured { observed } => {
                    let _ = write!(canonical, ":{observed}");
                }
                NativeRouteFamilyDisposition::Unmet { reason } => {
                    let _ = write!(canonical, "|u:{reason:?}");
                }
                NativeRouteFamilyDisposition::NotObserved => {}
                NativeRouteFamilyDisposition::Refused { reason } => {
                    let _ = write!(canonical, ":{}", reason.token());
                }
            }
        }
        match self.body_axis {
            NativeRouteBodyAxis::Measured {
                verdict,
                reason,
                d498_reopen_required,
                plane_absent_units,
                plane_present_in_token_units,
                plane_spanning_token_units,
                plane_character_material_units,
                oracle_units,
                native_plane_units,
            } => {
                let _ = write!(
                    canonical,
                    "|b:{}:{}:{d498_reopen_required}:{plane_absent_units}:{plane_present_in_token_units}:{plane_spanning_token_units}:{plane_character_material_units}:{oracle_units}:{native_plane_units}",
                    verdict.token(),
                    reason.token(),
                );
            }
            NativeRouteBodyAxis::Refused { reason } => {
                let _ = write!(canonical, "|b:refused:{}", reason.token());
            }
        }
        format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
    }

    /// Validate the closed structural invariants against the declared boundary.
    ///
    /// Totality and disjointness of the stage set, one disposition per declared
    /// family, and a typed body gap are enforced here; every rejection names the
    /// offending token.
    pub fn validate(
        &self,
        boundary: &NativeRouteBoundary,
        cascade_stage_ids: &[String],
    ) -> Result<(), NativeRouteValidationError> {
        if self.schema_version != NATIVE_ROUTE_RESULT_SCHEMA_VERSION {
            return Err(NativeRouteValidationError::SchemaMismatch {
                found: self.schema_version.clone(),
            });
        }
        let expected = boundary
            .stances(cascade_stage_ids)
            .map_err(NativeRouteValidationError::BoundaryInvalid)?;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for stage in &self.stages {
            if !seen.insert(stage.stage_id.as_str()) {
                return Err(NativeRouteValidationError::StageDuplicated {
                    stage_id: stage.stage_id.clone(),
                });
            }
            if !expected
                .iter()
                .any(|entry| entry.stage_id == stage.stage_id)
            {
                return Err(NativeRouteValidationError::StageOutsideCascade {
                    stage_id: stage.stage_id.clone(),
                });
            }
        }
        for entry in &expected {
            let Some(found) = self
                .stages
                .iter()
                .find(|stage| stage.stage_id == entry.stage_id)
            else {
                return Err(NativeRouteValidationError::StageMissing {
                    stage_id: entry.stage_id.clone(),
                });
            };
            if found.stance != entry.stance {
                return Err(NativeRouteValidationError::StageStanceMismatch {
                    stage_id: entry.stage_id.clone(),
                });
            }
        }

        let mut seen_families: BTreeSet<NativeRouteFamily> = BTreeSet::new();
        for cell in &self.families {
            if !seen_families.insert(cell.family) {
                return Err(NativeRouteValidationError::FamilyDuplicated {
                    family: cell.family.token(),
                });
            }
        }
        for family in NATIVE_ROUTE_FAMILY_ORDER {
            if !seen_families.contains(&family) {
                return Err(NativeRouteValidationError::FamilyMissing {
                    family: family.token(),
                });
            }
        }

        if let NativeRouteBodyAxis::Measured {
            verdict,
            d498_reopen_required,
            plane_absent_units,
            ..
        } = self.body_axis
            && (verdict == NativeRouteBodyVerdict::ContentLoss || plane_absent_units > 0)
            && !d498_reopen_required
        {
            return Err(NativeRouteValidationError::BodyGapUntyped {
                verdict: verdict.token(),
            });
        }
        Ok(())
    }
}
