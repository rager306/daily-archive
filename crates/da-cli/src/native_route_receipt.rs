//! S06/T03 (M514-1lss2s) — route receipt envelope and independent validator.
//!
//! S04 and S05 proved that a native claim is only honest when it carries a typed
//! *envelope* (`claim_scope`/`product_coverage_effect`/`thresholds`, the five
//! closed `locks`, a declaration binding, guard provenance, typed populations
//! and a redaction invariant) **and** an independent validator that emits
//! receipt-level refusals a builder cannot express (D505). The S06 route ends in
//! a typed document result (T01's `NativeRouteResult`) measured against a limit
//! freeze that was declared *before* any benchmark (T02, D509); it needs the same
//! envelope, so this module **reuses** the envelope types
//! ([`CarrierBinding`], [`GuardProvenanceReceipt`], [`Redaction`],
//! [`REQUIRED_LOCKS`], [`LEAK_MARKERS`], the claim-scope constants and the
//! sha256/path helpers) from [`crate::native_family_receipt`] rather than
//! restating them. It owns only the route-specific vocabulary:
//!
//! * [`RouteLimitsBinding`] — the T02 limit freeze (`freeze_revision`, the frozen
//!   metric dictionary and its `floor`/`ceiling` directions) that the receipt is
//!   bound to;
//! * [`RouteStageStanceCell`] / [`RouteFamilyCell`] — one typed cell per declared
//!   cascade stage P0..P9 and per declared family;
//! * [`RouteBodyAxis`] — the reproduced S05 P1 residual (D499), carried typed and
//!   never resolved;
//! * [`RoutePopulationReceipt`] — the per-document measurement populations that
//!   must not be pooled.
//!
//! ## The freeze is bound, not mentioned
//!
//! The builder refuses a receipt whose limits do not match the declared freeze
//! ([`RouteReceiptError::LimitNotFrozen`]) or whose metric is not in the frozen
//! set, and the validator refuses a *parsed* receipt whose `freeze_revision`
//! drifted from the recomputed one ([`RouteReceiptError::StaleFreezeRevision`]).
//! A ceiling chosen after the numbers is therefore not expressible: the receipt
//! carries the revision, and the revision only recomputes from the pinned
//! carrier bytes.
//!
//! ## A counted zero needs a guard
//!
//! `observed_java_calls == 0` is only evidence under an installed and
//! identity-isolated observer; without either the zero is refused
//! ([`RouteReceiptError::GuardMissingForObservedZero`]). A positive count is
//! refused outright ([`RouteReceiptError::ObservedJavaOnRoute`]).
//!
//! ## Claim scope is not negotiable
//!
//! The receipt is diagnostic only: `claim_scope = diagnostic_only`,
//! `product_coverage_effect = none`, `thresholds = not_accepted`, and the five
//! locks closed. Nothing here promotes, splices, imports or writes a graph
//! (D127 / ADR-076 §14 / D499). This module is pure `std`/`serde`/`sha2`, so it
//! and its contract test compile in the default feature set.

use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, CarrierBinding, DECLARATION_MANIFEST_REL, GuardObserver,
    GuardProvenance, GuardProvenanceReceipt, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE,
    REQUIRED_LOCKS, Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest,
    is_sha256_hex, normalize_document_identity,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted route receipt artifact.
pub const NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION: &str = "native-route-receipt.v1";
/// Default artifact path of the route receipt.
pub const DEFAULT_ROUTE_ARTIFACT_REL: &str = "artifacts/parse_compat/native_route_e2e.json";
/// Repository-relative carrier of the slice declaration and the corpus pin.
pub const ROUTE_DECLARATION_MANIFEST_REL: &str = DECLARATION_MANIFEST_REL;
/// Pointer to the slice declaration block inside the declaration carrier.
pub const ROUTE_DECLARATION_POINTER: &str = "java_free_vertical_slice";
/// Pointer to the pinned corpus documents.
pub const ROUTE_CORPUS_POINTER: &str = "corpus.documents";
/// Repository-relative carrier that names the frozen metric dictionary.
pub const ROUTE_LIMIT_MANIFEST_REL: &str = "data/parse/comparison_contract.yaml";
/// Pointer to the frozen metric dictionary inside the limit carrier.
pub const ROUTE_LIMIT_POINTER: &str = "batch_budget_ceilings.metrics";
/// Repository-relative carrier of the cascade stage ids.
pub const ROUTE_STAGE_SOURCE_MANIFEST_REL: &str = "data/parse/grobid_compatibility.yaml";
/// Pointer to the cascade stage dictionary inside the stage carrier.
pub const ROUTE_STAGE_SOURCE_POINTER: &str = "defaults.cascade_stages";
/// The only freeze state a receipt may carry.
pub const ROUTE_FREEZE_STATE_FROZEN: &str = "frozen";
/// The only freeze scope a receipt may carry.
pub const ROUTE_FREEZE_SCOPE: &str = "prospective_diagnostic_ceiling";
/// The only `tuned_after_scoring` value a receipt may carry.
pub const ROUTE_TUNED_AFTER_SCORING_FORBIDDEN: &str = "forbidden";
/// The only `claim_without_freeze` value a receipt may carry.
pub const ROUTE_CLAIM_WITHOUT_FREEZE_FORBIDDEN: &str = "forbidden";
/// The only `silent_clamp` value a receipt may carry.
pub const ROUTE_SILENT_CLAMP_FORBIDDEN: &str = "forbidden";

/// The closed `floor`/`ceiling` direction vocabulary.
pub const ROUTE_DIRECTION_VOCABULARY: [&str; 2] = ["floor", "ceiling"];

/// The frozen measured vocabulary: the ten batch-ceiling names
/// (`comparison_contract.yaml#batch_budget_ceilings.metrics`) followed by the
/// five route-only counters whose direction the T02 freeze declares. The
/// contract test recomputes this list against the live DATA block, so a metric
/// that drifts out of the freeze fails the gate instead of being silently
/// accepted.
pub const ROUTE_FROZEN_METRIC_VOCABULARY: [&str; 15] = [
    "latency_p50_ms",
    "latency_p95_ms",
    "latency_p99_ms",
    "peak_retained_bytes",
    "peak_rss_bytes",
    "java_cpu_wall_clock_ns",
    "max_reorder_depth",
    "max_publication_outstanding",
    "max_in_flight",
    "budget_waits",
    "native_route_documents",
    "native_route_pages",
    "native_route_java_calls",
    "native_route_unmeasured_stages",
    "native_route_unclassified_families",
];

/// The frozen metrics whose direction is `floor` (a larger number is better).
pub const ROUTE_FLOOR_METRIC_VOCABULARY: [&str; 2] =
    ["native_route_documents", "native_route_pages"];

/// The closed stage-stance vocabulary. It mirrors T01's
/// `NATIVE_ROUTE_STAGE_STANCE_VOCABULARY` token-for-token.
pub const ROUTE_STAGE_STANCE_VOCABULARY: [&str; 4] = [
    "native_measured",
    "not_qualified",
    "out_of_scope",
    "admission_boundary",
];

/// The closed family order. It mirrors T01's `NATIVE_ROUTE_FAMILY_ORDER`
/// token-for-token.
pub const ROUTE_FAMILY_ORDER: [&str; 14] = [
    "header_date",
    "header_personal_name",
    "header_affiliation",
    "header_affiliation_link",
    "bibliography_citation_fields",
    "bibliography_reference_segments",
    "bibliography_citation_anchors",
    "body_accounting",
    "body_sections",
    "body_reading_order",
    "figure",
    "table",
    "formula",
    "callout_marker",
];

/// The closed family-disposition vocabulary. `out_of_scope` is deliberately
/// absent: a required family can never be out of scope, so the token is refused
/// **by name** rather than by a generic serde failure.
pub const ROUTE_FAMILY_DISPOSITION_VOCABULARY: [&str; 4] =
    ["measured", "unmet", "not_observed", "refused"];

/// The closed out-of-slice reason vocabulary, in T01/DATA order.
pub const ROUTE_OUT_OF_SLICE_REASON_VOCABULARY: [&str; 5] = [
    "java_authority",
    "visual_heads_not_measured",
    "java_tei_coordinates",
    "profile_dependent_consolidation",
    "java_tei_lift",
];

/// The closed band-refusal reason vocabulary.
pub const ROUTE_REFUSAL_REASON_VOCABULARY: [&str; 7] = [
    "header_band_refused",
    "bibliography_band_refused",
    "body_text_band_refused",
    "section_band_refused",
    "visual_band_refused",
    "callout_band_refused",
    "oracle_refused",
];

/// The closed unmet-reason vocabulary (the tags of T01's
/// `NativeRouteUnmetReason`). The receipt carries the tag, never the T01 type.
pub const ROUTE_UNMET_REASON_VOCABULARY: [&str; 6] = [
    "region_no_admitted_tokens",
    "header_atom",
    "bibliography_atom",
    "visual",
    "callout",
    "section",
];

/// The closed plane-preservation verdict vocabulary (mirrors the T01
/// `BodyCoverageVerdict`).
pub const ROUTE_BODY_VERDICT_VOCABULARY: [&str; 3] =
    ["granularity_only", "content_loss", "not_assessable"];

/// The closed body-gap reason vocabulary (mirrors the T01 `BodyGapReason`
/// unit tags).
pub const ROUTE_BODY_REASON_VOCABULARY: [&str; 10] = [
    "matched",
    "p1_native_text_absent",
    "p1_localization_uncertain",
    "p2_region_assignment_gap",
    "p2_localization_uncertain",
    "scope_mismatch",
    "scope_join_mismatch",
    "normalization_mismatch",
    "oracle_empty",
    "population_incomplete",
];

/// The closed measurement-population vocabulary. `native_route` is the S06
/// route; `reference` is the paired same-n resource population T06 adds.
pub const ROUTE_POPULATION_VOCABULARY: [&str; 2] = ["native_route", "reference"];

// ───────────────────────────── vocabulary ─────────────────────────────

/// One measurement population. `Serde` rejects any other token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePopulationId {
    /// The S06 native route.
    NativeRoute,
    /// The paired same-n reference population.
    Reference,
}

impl RoutePopulationId {
    /// The stable `snake_case` token this population serializes to.
    pub fn token(&self) -> &'static str {
        match self {
            RoutePopulationId::NativeRoute => "native_route",
            RoutePopulationId::Reference => "reference",
        }
    }
}

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals of the route receipt. Every variant is a refusal, never a
/// silent downgrade.
///
/// The twelve receipt-level refusals a builder cannot express (and the
/// validator therefore owns) are `ThresholdClaimed`, `ProductCoverageClaimed`,
/// `LimitTunedAfterScoring`, `LimitNotFrozen`, `StaleFreezeRevision`,
/// `LockOpened`, `RedactionLeak`, `GuardMissingForObservedZero`,
/// `StageOutsideSliceCounted`, `FamilyOutOfScope`, `BodyGapUntyped`,
/// `PoolingClaimed` and `BindingDrift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteReceiptError {
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// The freeze was tuned after scoring.
    LimitTunedAfterScoring { found: String },
    /// The limits are not frozen, or a metric is outside the frozen set.
    LimitNotFrozen { detail: String },
    /// The parsed receipt's `freeze_revision` did not recompute.
    StaleFreezeRevision { expected: String, found: String },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
    /// A counted zero was claimed without an installed guard.
    GuardMissingForObservedZero { observed_java_calls: u64 },
    /// The route consulted Java.
    ObservedJavaOnRoute { attempts: u64 },
    /// A stage outside the declared slice was counted as natively measured.
    StageOutsideSliceCounted { document: String, stage: String },
    /// The observed seam is not the Java transport seam.
    GuardObserverDrift { found: String },
    /// A required family carried the forbidden `out_of_scope`.
    FamilyOutOfScope {
        document: String,
        family: String,
        token: String,
    },
    /// A body gap was not typed.
    BodyGapUntyped { document: String, detail: String },
    /// Two measurement populations pooled their documents.
    PoolingClaimed { field: String },
    /// A carrier binding drifted from the pointer the contract requires.
    BindingDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A document has no single cell for a declared stage.
    StageMatrixIncomplete { document: String, stage: String },
    /// A stage token is outside the declared cascade.
    UnknownStage { document: String, stage: String },
    /// A stage cell's stance and reason contradict each other.
    StageCellInconsistent {
        document: String,
        stage: String,
        detail: String,
    },
    /// A document has no single cell for a declared family.
    FamilyMatrixIncomplete { document: String, family: String },
    /// A family token is outside the closed vocabulary.
    UnknownFamily { document: String, family: String },
    /// A family cell's disposition and reason contradict each other.
    FamilyCellInconsistent {
        document: String,
        family: String,
        detail: String,
    },
    /// The slice boundary is not total and disjoint over the cascade.
    BoundaryInvalid { detail: String },
    /// The same document appears twice.
    DuplicateDocument { document: String },
    /// The run observed a document the declaration does not pin.
    UnknownDocument { document: String },
    /// The declaration pins a document the receipt never observed.
    MissingDocument { document: String },
    /// The observed sha256 drifted from the pin.
    DocumentShaDrift {
        document: String,
        expected: String,
        found: String,
    },
    /// The observed page count drifted from the pin.
    PageCountDrift {
        document: String,
        expected: u64,
        found: u64,
    },
    /// The corpus pin's document count or page denominator drifted.
    CorpusPinDrift { expected: u64, found: u64 },
    /// A population token is outside the closed vocabulary.
    UnknownPopulation { population: String },
    /// A measured metric is outside the frozen set.
    UnknownMetric { id: String },
    /// A metric direction is outside the `floor`/`ceiling` vocabulary.
    DirectionOutsideVocabulary { id: String, direction: String },
    /// The measurement populations do not partition the declared corpus.
    PopulationStratificationInvalid { detail: String },
    /// A digest leaf is not a `sha256:<hex>` string.
    InvalidDigest { field: String },
    /// A bare-hex leaf is not a 64-character sha256.
    InvalidBareDigest { field: String },
    /// A path leaf is absolute or escapes the repository.
    InvalidPath { field: String },
    /// A body axis carries a token outside its closed vocabulary.
    InvalidBodyVocabulary { field: String, token: String },
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
}

impl fmt::Display for RouteReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteReceiptError::SchemaDrift {
                field,
                expected,
                found,
            } => write!(f, "`{field}` is `{found}`, expected `{expected}`"),
            RouteReceiptError::ClaimScopeDrift { found } => write!(
                f,
                "claim_scope is `{found}`; only `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}` may be emitted"
            ),
            RouteReceiptError::ProductCoverageClaimed { found } => write!(
                f,
                "product_coverage_effect is `{found}`; only `{PRODUCT_COVERAGE_EFFECT_NONE}` may be emitted"
            ),
            RouteReceiptError::ThresholdClaimed { found } => write!(
                f,
                "thresholds are `{found}`; only `{THRESHOLDS_NOT_ACCEPTED}` may be emitted"
            ),
            RouteReceiptError::LimitTunedAfterScoring { found } => write!(
                f,
                "tuned_after_scoring is `{found}`; a limit tuned after scoring is not a limit"
            ),
            RouteReceiptError::LimitNotFrozen { detail } => {
                write!(f, "the route limits are not frozen: {detail}")
            }
            RouteReceiptError::StaleFreezeRevision { expected, found } => {
                write!(f, "freeze_revision is `{found}`, recomputed `{expected}`")
            }
            RouteReceiptError::LockOpened {
                lock,
                expected,
                found,
            } => write!(f, "lock `{lock}` is `{found}`, expected `{expected}`"),
            RouteReceiptError::RedactionLeak { marker } => {
                write!(f, "the artifact leaked marker `{marker}`")
            }
            RouteReceiptError::GuardMissingForObservedZero {
                observed_java_calls,
            } => write!(
                f,
                "a counted zero ({observed_java_calls} Java calls) needs an installed, identity-isolated guard"
            ),
            RouteReceiptError::ObservedJavaOnRoute { attempts } => {
                write!(f, "Java was consulted {attempts} time(s) on the route")
            }
            RouteReceiptError::StageOutsideSliceCounted { document, stage } => write!(
                f,
                "document `{document}` counts stage `{stage}` as natively measured, but it is outside the slice"
            ),
            RouteReceiptError::GuardObserverDrift { found } => write!(
                f,
                "guard observer is `{found}`, expected the Java transport seam"
            ),
            RouteReceiptError::FamilyOutOfScope {
                document,
                family,
                token,
            } => write!(
                f,
                "document `{document}` family `{family}` carries `{token}`: the vocabulary has no out-of-scope arm"
            ),
            RouteReceiptError::BodyGapUntyped { document, detail } => {
                write!(f, "document `{document}` body gap is untyped: {detail}")
            }
            RouteReceiptError::PoolingClaimed { field } => {
                write!(f, "`{field}` pools measurement populations")
            }
            RouteReceiptError::BindingDrift {
                field,
                expected,
                found,
            } => write!(f, "`{field}` is `{found}`, expected `{expected}`"),
            RouteReceiptError::StageMatrixIncomplete { document, stage } => write!(
                f,
                "document `{document}` has no single cell for stage `{stage}`"
            ),
            RouteReceiptError::UnknownStage { document, stage } => write!(
                f,
                "document `{document}` carries stage `{stage}`, outside the declared cascade"
            ),
            RouteReceiptError::StageCellInconsistent {
                document,
                stage,
                detail,
            } => write!(f, "document `{document}` stage `{stage}`: {detail}"),
            RouteReceiptError::FamilyMatrixIncomplete { document, family } => write!(
                f,
                "document `{document}` has no single cell for family `{family}`"
            ),
            RouteReceiptError::UnknownFamily { document, family } => write!(
                f,
                "document `{document}` carries `{family}`, outside the route-family vocabulary"
            ),
            RouteReceiptError::FamilyCellInconsistent {
                document,
                family,
                detail,
            } => write!(f, "document `{document}` family `{family}`: {detail}"),
            RouteReceiptError::BoundaryInvalid { detail } => {
                write!(f, "the route boundary is invalid: {detail}")
            }
            RouteReceiptError::DuplicateDocument { document } => {
                write!(f, "document `{document}` appears more than once")
            }
            RouteReceiptError::UnknownDocument { document } => {
                write!(f, "document `{document}` was observed but is not declared")
            }
            RouteReceiptError::MissingDocument { document } => {
                write!(f, "declared document `{document}` was not observed")
            }
            RouteReceiptError::DocumentShaDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "document `{document}` sha256 is `{found}`, pinned `{expected}`"
            ),
            RouteReceiptError::PageCountDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "document `{document}` page count is {found}, pinned {expected}"
            ),
            RouteReceiptError::CorpusPinDrift { expected, found } => write!(
                f,
                "the corpus pin reports {found}, the documents sum to {expected}"
            ),
            RouteReceiptError::UnknownPopulation { population } => write!(
                f,
                "`{population}` is not a member of the population vocabulary"
            ),
            RouteReceiptError::UnknownMetric { id } => {
                write!(f, "metric `{id}` is outside the frozen metric dictionary")
            }
            RouteReceiptError::DirectionOutsideVocabulary { id, direction } => write!(
                f,
                "metric `{id}` carries direction `{direction}`, outside the vocabulary"
            ),
            RouteReceiptError::PopulationStratificationInvalid { detail } => {
                write!(
                    f,
                    "the measurement populations do not partition the corpus: {detail}"
                )
            }
            RouteReceiptError::InvalidDigest { field } => {
                write!(f, "`{field}` is not a sha256 digest")
            }
            RouteReceiptError::InvalidBareDigest { field } => {
                write!(f, "`{field}` is not a bare sha256 hex")
            }
            RouteReceiptError::InvalidPath { field } => {
                write!(f, "`{field}` is not a repository-relative path")
            }
            RouteReceiptError::InvalidBodyVocabulary { field, token } => write!(
                f,
                "body axis `{field}` carries `{token}`, outside its vocabulary"
            ),
            RouteReceiptError::ArtifactSerialization { reason } => {
                write!(f, "the artifact could not be serialized: {reason}")
            }
        }
    }
}

impl std::error::Error for RouteReceiptError {}

// ───────────────────────── receipt types ─────────────────────────

/// One pinned document identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDocumentIdentity {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
}

/// One frozen metric and its direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteMetricDeclaration {
    /// The metric id.
    pub id: String,
    /// `floor` or `ceiling`.
    pub direction: String,
}

/// One measured metric value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteMetricValue {
    /// The metric id.
    pub id: String,
    /// `floor` or `ceiling`.
    pub direction: String,
    /// The measured value. Zero is a measurement only under the freeze's
    /// `zero_is_not_a_measurement` rule, never a clamp.
    pub value: u64,
}

/// The T02 freeze binding carried by the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteLimitsBinding {
    /// The composite sha256 revision of the pinned freeze carriers.
    pub freeze_revision: String,
    /// Always `frozen`.
    pub freeze_state: String,
    /// The freeze owner.
    pub freeze_owner: String,
    /// Always `prospective_diagnostic_ceiling`.
    pub freeze_scope: String,
    /// Always `forbidden`.
    pub tuned_after_scoring: String,
    /// Always `true`.
    pub zero_is_not_a_measurement: bool,
    /// Always `forbidden`.
    pub silent_clamp: String,
    /// Always `true`.
    pub pass_requires_frozen_threshold: bool,
    /// Always `forbidden`.
    pub claim_without_freeze: String,
    /// The closed `floor`/`ceiling` vocabulary.
    pub direction_vocabulary: Vec<String>,
    /// The frozen metric dictionary.
    pub metrics: Vec<RouteMetricDeclaration>,
}

/// One cascade stage's stance for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteStageStanceCell {
    /// The cascade stage id (P0..P9, resolved from DATA).
    pub stage: String,
    /// `native_measured` | `not_qualified` | `out_of_scope` | `admission_boundary`.
    pub stance: String,
    /// The typed out-of-slice reason, present exactly when `stance` is
    /// `out_of_scope`.
    pub reason: Option<String>,
}

/// One family's typed outcome for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteFamilyCell {
    /// The family this cell measures.
    pub family: String,
    /// `measured` | `unmet` | `not_observed` | `refused`.
    pub disposition: String,
    /// The typed reason token: the unmet tag for `unmet`, the band-refusal tag
    /// for `refused`, absent otherwise.
    pub reason: Option<String>,
    /// Measured native item count. Present exactly when `measured`.
    pub observed_count: Option<u64>,
}

/// The reproduced S05 P1 body axis, carried typed and never resolved (D499).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteBodyAxis {
    /// The P1 plane-preservation verdict token.
    pub plane_preservation: String,
    /// The typed body-gap reason token.
    pub plane_gap_reason: String,
    /// Whether the P1 verdict requires a recorded D498 engine decision.
    pub plane_d498_reopen_required: bool,
    /// Unresolved P1 occurrences existing only as character material.
    pub plane_character_material_units: u64,
    /// Oracle occurrences with no character material in the plane.
    pub plane_absent_units: u64,
    /// Unresolved P1 occurrences sitting inside one native token.
    pub plane_present_in_token_units: u64,
    /// Unresolved P1 occurrences spanning token boundaries.
    pub plane_spanning_token_units: u64,
}

/// One document's full typed route row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDocumentReceipt {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
    /// One stance per cascade stage, in cascade order.
    pub stages: Vec<RouteStageStanceCell>,
    /// One cell per family, in canonical family order.
    pub families: Vec<RouteFamilyCell>,
    /// The reproduced P1 body axis.
    pub body: RouteBodyAxis,
    /// sha256 over the native route result.
    pub route_digest: String,
}

/// One measurement population. Its documents are disjoint from every other
/// population's, so a metric can never be pooled across populations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePopulationReceipt {
    /// The population.
    pub population: RoutePopulationId,
    /// Pinned document count in this population.
    pub document_count: u64,
    /// Pinned page denominator of this population.
    pub page_denominator: u64,
    /// Pinned identities, in carrier order.
    pub documents: Vec<RouteDocumentIdentity>,
    /// Measured metrics for this population.
    pub metrics: Vec<RouteMetricValue>,
}

/// The slice-level residuals: the reproduced body axis plus the two counters
/// the frozen dictionary names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteResiduals {
    /// The S05-reproduced slice body axis.
    pub body: RouteBodyAxis,
    /// Stage cells across the slice that are not `native_measured`.
    pub unmeasured_stages: u64,
    /// Family cells across the slice outside the closed disposition vocabulary.
    /// The builder refuses an unknown disposition, so this is measured as zero.
    pub unclassified_families: u64,
}

/// The emitted route receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Claim scope; only `diagnostic_only` is emitted.
    pub claim_scope: String,
    /// Product-coverage effect; only `none` is emitted.
    pub product_coverage_effect: String,
    /// Thresholds; only `not_accepted` is emitted.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// Binding to the slice declaration carrier.
    pub declaration: CarrierBinding,
    /// Guard provenance.
    pub guard: GuardProvenanceReceipt,
    /// The T02 limit freeze binding.
    pub limits: RouteLimitsBinding,
    /// The measurement populations.
    pub populations: Vec<RoutePopulationReceipt>,
    /// One typed row per declared document, in carrier order.
    pub documents: Vec<RouteDocumentReceipt>,
    /// The slice-level residuals.
    pub residuals: RouteResiduals,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl RouteReceipt {
    /// Serialize the receipt canonically (pretty JSON, deterministic field and
    /// key order) and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, RouteReceiptError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            RouteReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            RouteReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_route_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, RouteReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| RouteReceiptError::ArtifactSerialization {
            reason: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_route_redacted(json: &str) -> Result<(), RouteReceiptError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(RouteReceiptError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────── builder inputs ─────────────────────────

/// One declared document, as the declaration pins it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDeclaredDocument {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
}

/// The validated corpus pin the receipt is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDeclaredCorpus {
    /// Corpus identity.
    pub corpus_id: String,
    /// Pinned document count.
    pub document_count: u64,
    /// Pinned page denominator.
    pub page_denominator: u64,
    /// Pinned documents in carrier order.
    pub documents: Vec<RouteDeclaredDocument>,
}

/// The declared slice boundary: the cascade and how each stage classifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteBoundaryInput {
    /// Every cascade stage, in cascade order (P0..P9 from DATA).
    pub cascade_stages: Vec<String>,
    /// The admission boundary stage (neither in-slice nor out-of-slice).
    pub admission_stage: String,
    /// Stages inside the slice that the route measures.
    pub in_slice_stages: Vec<String>,
    /// Stages outside the slice, with a typed reason on each cell.
    pub out_of_slice_stages: Vec<String>,
}

/// The declared freeze the receipt's limits must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteFreezeDeclaration {
    /// The composite sha256 revision of the pinned freeze carriers.
    pub freeze_revision: String,
    /// The frozen metric dictionary.
    pub metrics: Vec<RouteMetricDeclaration>,
    /// The closed `floor`/`ceiling` vocabulary.
    pub direction_vocabulary: Vec<String>,
}

/// One document's observed route output, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteObservedDocument {
    /// Pinned document id.
    pub id: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
    /// One stance per cascade stage (any order; canonicalized by the builder).
    pub stages: Vec<RouteStageStanceCell>,
    /// One cell per family (any order; canonicalized by the builder).
    pub families: Vec<RouteFamilyCell>,
    /// The reproduced P1 body axis.
    pub body: RouteBodyAxis,
    /// sha256 over the native route result.
    pub route_digest: String,
}

/// One population's observed measurement, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePopulationInput {
    /// The population.
    pub population: RoutePopulationId,
    /// The documents in this population, in carrier order.
    pub documents: Vec<RouteDocumentIdentity>,
    /// The measured metrics for this population.
    pub metrics: Vec<RouteMetricValue>,
}

// ───────────────────────── builder ─────────────────────────

/// Build a canonical route receipt.
///
/// The builder enforces every structural invariant it can express: claim scope,
/// the five closed locks, an installed identity-isolated guard with zero
/// observed Java calls, the declaration binding, the limit freeze, a total
/// boundary over the cascade, exact observed coverage, complete stage and family
/// matrices, typed body axes, disjoint populations over the frozen metric
/// dictionary, and the slice residuals. Receipt-level mutations a builder cannot
/// express on freshly built input are refused by [`validate_route_receipt`].
#[allow(clippy::too_many_arguments)]
pub fn build_route_receipt(
    declaration_revision_sha256: &str,
    corpus: &RouteDeclaredCorpus,
    boundary: &RouteBoundaryInput,
    freeze: &RouteFreezeDeclaration,
    limits: &RouteLimitsBinding,
    claim_scope: &str,
    product_coverage_effect: &str,
    guard: GuardProvenance,
    observed: &[RouteObservedDocument],
    populations: &[RoutePopulationInput],
) -> Result<RouteReceipt, RouteReceiptError> {
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(RouteReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(RouteReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }

    let checked_locks = check_locks(required_locks_map())?;

    if guard.observed_java_calls > 0 {
        return Err(RouteReceiptError::ObservedJavaOnRoute {
            attempts: guard.observed_java_calls,
        });
    }
    if !guard.installed || !guard.identity_isolated {
        return Err(RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: guard.observed_java_calls,
        });
    }

    // Declaration binding.
    require_digest("declaration.revision_sha256", declaration_revision_sha256)?;
    require_repo_path("declaration.manifest", ROUTE_DECLARATION_MANIFEST_REL)?;

    // The limit freeze must match the declared freeze.
    check_limits(limits, freeze)?;

    // Corpus pin.
    if corpus.corpus_id.trim().is_empty() {
        return Err(RouteReceiptError::SchemaDrift {
            field: "corpus.corpus_id".to_owned(),
            expected: "a non-empty corpus id".to_owned(),
            found: "empty".to_owned(),
        });
    }
    if corpus.documents.len() as u64 != corpus.document_count {
        return Err(RouteReceiptError::CorpusPinDrift {
            expected: corpus.documents.len() as u64,
            found: corpus.document_count,
        });
    }
    let declared_page_sum: u64 = corpus.documents.iter().map(|doc| doc.pages).sum();
    if declared_page_sum != corpus.page_denominator {
        return Err(RouteReceiptError::CorpusPinDrift {
            expected: declared_page_sum,
            found: corpus.page_denominator,
        });
    }

    // Boundary totality/disjointness.
    check_boundary(boundary)?;

    // Observed coverage: every pinned document exactly once, pinned identity.
    let mut observed_by_id: BTreeMap<&str, &RouteObservedDocument> = BTreeMap::new();
    for document in observed {
        let declared = corpus
            .documents
            .iter()
            .find(|declared| declared.id == document.id)
            .ok_or_else(|| RouteReceiptError::UnknownDocument {
                document: document.id.clone(),
            })?;
        require_bare_digest("document.sha256", &document.sha256)?;
        if document.sha256 != declared.sha256 {
            return Err(RouteReceiptError::DocumentShaDrift {
                document: document.id.clone(),
                expected: declared.sha256.clone(),
                found: document.sha256.clone(),
            });
        }
        if document.pages != declared.pages {
            return Err(RouteReceiptError::PageCountDrift {
                document: document.id.clone(),
                expected: declared.pages,
                found: document.pages,
            });
        }
        require_repo_path("document.source_pdf", &declared.source_pdf)?;
        require_digest(
            &format!("document[{}].route_digest", document.id),
            &document.route_digest,
        )?;
        validate_body_axis(&document.id, &document.body)?;
        canonical_stages(&document.id, boundary, &document.stages)?;
        canonical_families(&document.id, &document.families)?;
        if observed_by_id.insert(&document.id, document).is_some() {
            return Err(RouteReceiptError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
    }
    for declared in &corpus.documents {
        if !observed_by_id.contains_key(declared.id.as_str()) {
            return Err(RouteReceiptError::MissingDocument {
                document: declared.id.clone(),
            });
        }
    }

    // Canonical per-document rows.
    let mut rows_by_id: BTreeMap<&str, RouteDocumentReceipt> = BTreeMap::new();
    for declared in &corpus.documents {
        let document = observed_by_id
            .get(declared.id.as_str())
            .expect("observed coverage checked above");
        rows_by_id.insert(
            declared.id.as_str(),
            RouteDocumentReceipt {
                id: declared.id.clone(),
                source_pdf: declared.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: declared.pages,
                stages: canonical_stages(&declared.id, boundary, &document.stages)?,
                families: canonical_families(&declared.id, &document.families)?,
                body: document.body.clone(),
                route_digest: document.route_digest.clone(),
            },
        );
    }

    // Measurement populations: disjoint, complete over the declared corpus.
    let frozen: BTreeSet<&str> = freeze.metrics.iter().map(|m| m.id.as_str()).collect();
    let mut population_receipts: Vec<RoutePopulationReceipt> =
        Vec::with_capacity(populations.len());
    let mut population_seen: BTreeSet<&str> = BTreeSet::new();
    let mut population_order: Vec<&str> = Vec::new();
    for population in populations {
        let mut identities = Vec::with_capacity(population.documents.len());
        let mut page_denominator = 0_u64;
        for identity in &population.documents {
            let declared = corpus
                .documents
                .iter()
                .find(|declared| declared.id == identity.id)
                .ok_or_else(|| RouteReceiptError::UnknownDocument {
                    document: identity.id.clone(),
                })?;
            if identity.sha256 != declared.sha256 || identity.pages != declared.pages {
                return Err(RouteReceiptError::PopulationStratificationInvalid {
                    detail: format!("population document `{}` drifted from its pin", identity.id),
                });
            }
            require_repo_path("population.source_pdf", &identity.source_pdf)?;
            require_bare_digest("population.sha256", &identity.sha256)?;
            if !population_seen.insert(identity.id.as_str()) {
                return Err(RouteReceiptError::PoolingClaimed {
                    field: format!("populations[{}]", population.population.token()),
                });
            }
            population_order.push(identity.id.as_str());
            page_denominator += identity.pages;
            identities.push(identity.clone());
        }
        for metric in &population.metrics {
            check_metric(metric, &frozen)?;
        }
        population_receipts.push(RoutePopulationReceipt {
            population: population.population,
            document_count: identities.len() as u64,
            page_denominator,
            documents: identities,
            metrics: population.metrics.clone(),
        });
    }
    if population_order.len() != corpus.documents.len()
        || !corpus
            .documents
            .iter()
            .all(|declared| population_seen.contains(declared.id.as_str()))
    {
        return Err(RouteReceiptError::PopulationStratificationInvalid {
            detail: format!(
                "{} population documents for {} declared documents",
                population_order.len(),
                corpus.documents.len()
            ),
        });
    }

    // Slice residuals derived from the typed cells (never free input).
    let mut unmeasured_stages = 0_u64;
    let mut unclassified_families = 0_u64;
    for document in &corpus.documents {
        let row = rows_by_id
            .get(document.id.as_str())
            .expect("row built above");
        unmeasured_stages += row
            .stages
            .iter()
            .filter(|cell| cell.stance != "native_measured")
            .count() as u64;
        unclassified_families += row
            .families
            .iter()
            .filter(|cell| {
                !ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&cell.disposition.as_str())
            })
            .count() as u64;
    }
    let slice_body = corpus
        .documents
        .first()
        .and_then(|declared| rows_by_id.get(declared.id.as_str()))
        .map(|row| row.body.clone())
        .ok_or_else(|| RouteReceiptError::BoundaryInvalid {
            detail: "the corpus has no documents".to_owned(),
        })?;

    let mut documents = Vec::with_capacity(corpus.documents.len());
    for declared in &corpus.documents {
        documents.push(
            rows_by_id
                .get(declared.id.as_str())
                .expect("row built above")
                .clone(),
        );
    }

    Ok(RouteReceipt {
        schema_version: NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks: checked_locks,
        declaration: CarrierBinding {
            manifest: ROUTE_DECLARATION_MANIFEST_REL.to_owned(),
            pointer: ROUTE_DECLARATION_POINTER.to_owned(),
            revision_sha256: declaration_revision_sha256.to_owned(),
        },
        guard: GuardProvenanceReceipt {
            observer: GuardObserver::JavaTransportFetchTei,
            installed: guard.installed,
            identity_isolated: guard.identity_isolated,
            observed_java_calls: guard.observed_java_calls,
        },
        limits: limits.clone(),
        populations: population_receipts,
        documents,
        residuals: RouteResiduals {
            body: slice_body,
            unmeasured_stages,
            unclassified_families,
        },
        redaction: Redaction::clean(),
    })
}

/// The builder derives the lock map from the required locks; a caller cannot
/// supply an open lock because there is nothing to supply it through.
fn required_locks_map() -> BTreeMap<String, String> {
    REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect()
}

/// Re-check a lock map against the required locks.
fn check_locks(
    locks: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, RouteReceiptError> {
    let mut checked = BTreeMap::new();
    for (lock, expected) in REQUIRED_LOCKS {
        let found = locks
            .get(lock)
            .cloned()
            .unwrap_or_else(|| "<absent>".to_owned());
        if found != expected {
            return Err(RouteReceiptError::LockOpened {
                lock: lock.to_owned(),
                expected: expected.to_owned(),
                found,
            });
        }
        checked.insert(lock.to_owned(), expected.to_owned());
    }
    Ok(checked)
}

/// Re-check the limits against the declared freeze.
fn check_limits(
    limits: &RouteLimitsBinding,
    freeze: &RouteFreezeDeclaration,
) -> Result<(), RouteReceiptError> {
    if limits.tuned_after_scoring != ROUTE_TUNED_AFTER_SCORING_FORBIDDEN {
        return Err(RouteReceiptError::LimitTunedAfterScoring {
            found: limits.tuned_after_scoring.clone(),
        });
    }
    if limits.freeze_state != ROUTE_FREEZE_STATE_FROZEN {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: format!("freeze_state is `{}`", limits.freeze_state),
        });
    }
    if limits.freeze_scope != ROUTE_FREEZE_SCOPE {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: format!("freeze_scope is `{}`", limits.freeze_scope),
        });
    }
    if limits.claim_without_freeze != ROUTE_CLAIM_WITHOUT_FREEZE_FORBIDDEN {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: "claim_without_freeze is not forbidden".to_owned(),
        });
    }
    if limits.silent_clamp != ROUTE_SILENT_CLAMP_FORBIDDEN {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: "silent_clamp is not forbidden".to_owned(),
        });
    }
    if !limits.zero_is_not_a_measurement || !limits.pass_requires_frozen_threshold {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: "the zero/threshold invariants are not declared".to_owned(),
        });
    }
    if limits.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY
        || freeze.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY
    {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: "the direction vocabulary drifted".to_owned(),
        });
    }
    require_digest("limits.freeze_revision", &limits.freeze_revision)?;
    if limits.freeze_revision != freeze.freeze_revision {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: format!(
                "freeze_revision `{}` does not match the declared freeze `{}`",
                limits.freeze_revision, freeze.freeze_revision
            ),
        });
    }
    if limits.metrics != freeze.metrics {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: "the frozen metric dictionary drifted".to_owned(),
        });
    }
    let frozen: BTreeSet<&str> = freeze.metrics.iter().map(|m| m.id.as_str()).collect();
    if frozen.len() != ROUTE_FROZEN_METRIC_VOCABULARY.len() {
        return Err(RouteReceiptError::LimitNotFrozen {
            detail: format!("the freeze declares {} metrics", frozen.len()),
        });
    }
    for metric in &limits.metrics {
        if !ROUTE_FROZEN_METRIC_VOCABULARY.contains(&metric.id.as_str()) {
            return Err(RouteReceiptError::UnknownMetric {
                id: metric.id.clone(),
            });
        }
        if !ROUTE_DIRECTION_VOCABULARY.contains(&metric.direction.as_str()) {
            return Err(RouteReceiptError::DirectionOutsideVocabulary {
                id: metric.id.clone(),
                direction: metric.direction.clone(),
            });
        }
    }
    for id in ROUTE_FROZEN_METRIC_VOCABULARY {
        if !frozen.contains(id) {
            return Err(RouteReceiptError::LimitNotFrozen {
                detail: format!("frozen metric `{id}` is absent from the freeze"),
            });
        }
    }
    Ok(())
}

/// Check one measured metric against the frozen dictionary.
fn check_metric(
    metric: &RouteMetricValue,
    frozen: &BTreeSet<&str>,
) -> Result<(), RouteReceiptError> {
    if !frozen.contains(metric.id.as_str()) {
        return Err(RouteReceiptError::UnknownMetric {
            id: metric.id.clone(),
        });
    }
    if !ROUTE_DIRECTION_VOCABULARY.contains(&metric.direction.as_str()) {
        return Err(RouteReceiptError::DirectionOutsideVocabulary {
            id: metric.id.clone(),
            direction: metric.direction.clone(),
        });
    }
    Ok(())
}

/// Check the boundary is total and disjoint over the cascade.
fn check_boundary(boundary: &RouteBoundaryInput) -> Result<(), RouteReceiptError> {
    if boundary.cascade_stages.is_empty() {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "the cascade is empty".to_owned(),
        });
    }
    let cascade: BTreeSet<&str> = boundary.cascade_stages.iter().map(String::as_str).collect();
    if cascade.len() != boundary.cascade_stages.len() {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "a cascade stage appears twice".to_owned(),
        });
    }
    if !cascade.contains(boundary.admission_stage.as_str()) {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "the admission stage is not in the cascade".to_owned(),
        });
    }
    let in_slice: BTreeSet<&str> = boundary
        .in_slice_stages
        .iter()
        .map(String::as_str)
        .collect();
    if in_slice.len() != boundary.in_slice_stages.len() {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "an in-slice stage appears twice".to_owned(),
        });
    }
    let out_of_slice: BTreeSet<&str> = boundary
        .out_of_slice_stages
        .iter()
        .map(String::as_str)
        .collect();
    if out_of_slice.len() != boundary.out_of_slice_stages.len() {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "an out-of-slice stage appears twice".to_owned(),
        });
    }
    for stage in in_slice.iter().chain(out_of_slice.iter()) {
        if !cascade.contains(stage) {
            return Err(RouteReceiptError::BoundaryInvalid {
                detail: format!("classified stage `{stage}` is not in the cascade"),
            });
        }
    }
    if in_slice.contains(boundary.admission_stage.as_str())
        || out_of_slice.contains(boundary.admission_stage.as_str())
    {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "the admission stage is classified twice".to_owned(),
        });
    }
    if !in_slice.is_disjoint(&out_of_slice) {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: "a stage is both in-slice and out-of-slice".to_owned(),
        });
    }
    let classified = in_slice.len() + out_of_slice.len() + 1;
    if classified != cascade.len() {
        return Err(RouteReceiptError::BoundaryInvalid {
            detail: format!(
                "{} classified stages for a {}-stage cascade",
                classified,
                cascade.len()
            ),
        });
    }
    Ok(())
}

/// Canonicalize a document's stage cells in cascade order, refusing an
/// out-of-slice stage counted as natively measured.
fn canonical_stages(
    document_id: &str,
    boundary: &RouteBoundaryInput,
    cells: &[RouteStageStanceCell],
) -> Result<Vec<RouteStageStanceCell>, RouteReceiptError> {
    let out_of_slice: BTreeSet<&str> = boundary
        .out_of_slice_stages
        .iter()
        .map(String::as_str)
        .collect();
    let in_slice: BTreeSet<&str> = boundary
        .in_slice_stages
        .iter()
        .map(String::as_str)
        .collect();
    let mut ordered = Vec::with_capacity(boundary.cascade_stages.len());
    for stage in &boundary.cascade_stages {
        let mut matching = cells.iter().filter(|cell| &cell.stage == stage);
        let cell = matching
            .next()
            .ok_or_else(|| RouteReceiptError::StageMatrixIncomplete {
                document: document_id.to_owned(),
                stage: stage.clone(),
            })?;
        if matching.next().is_some() {
            return Err(RouteReceiptError::StageMatrixIncomplete {
                document: document_id.to_owned(),
                stage: stage.clone(),
            });
        }
        parse_stage_stance(
            document_id,
            stage,
            cell,
            &boundary.admission_stage,
            &in_slice,
            &out_of_slice,
        )?;
        ordered.push(cell.clone());
    }
    for cell in cells {
        if !boundary.cascade_stages.contains(&cell.stage) {
            return Err(RouteReceiptError::UnknownStage {
                document: document_id.to_owned(),
                stage: cell.stage.clone(),
            });
        }
    }
    Ok(ordered)
}

/// Parse one stage cell's stance/reason pair by name.
fn parse_stage_stance(
    document_id: &str,
    stage: &str,
    cell: &RouteStageStanceCell,
    admission_stage: &str,
    in_slice: &BTreeSet<&str>,
    out_of_slice: &BTreeSet<&str>,
) -> Result<(), RouteReceiptError> {
    let inconsistent = |detail: &str| RouteReceiptError::StageCellInconsistent {
        document: document_id.to_owned(),
        stage: stage.to_owned(),
        detail: detail.to_owned(),
    };
    if !ROUTE_STAGE_STANCE_VOCABULARY.contains(&cell.stance.as_str()) {
        return Err(inconsistent("the stance token is outside the vocabulary"));
    }
    let is_admission = stage == admission_stage;
    if is_admission != (cell.stance == "admission_boundary") {
        return Err(inconsistent(
            "the admission stance does not match the admission stage",
        ));
    }
    match cell.stance.as_str() {
        "native_measured" => {
            if cell.reason.is_some() {
                return Err(inconsistent("a `native_measured` cell carries a reason"));
            }
            if out_of_slice.contains(stage) {
                return Err(RouteReceiptError::StageOutsideSliceCounted {
                    document: document_id.to_owned(),
                    stage: stage.to_owned(),
                });
            }
        }
        "admission_boundary" => {
            if cell.reason.is_some() {
                return Err(inconsistent(
                    "an `admission_boundary` cell carries a reason",
                ));
            }
        }
        "not_qualified" => {
            if cell.reason.is_some() {
                return Err(inconsistent("a `not_qualified` cell carries a reason"));
            }
            if !in_slice.contains(stage) {
                return Err(inconsistent("`not_qualified` on a stage outside the slice"));
            }
        }
        "out_of_scope" => {
            if !out_of_slice.contains(stage) {
                return Err(inconsistent("`out_of_scope` on a stage inside the slice"));
            }
            let reason = cell
                .reason
                .as_deref()
                .ok_or_else(|| inconsistent("an `out_of_scope` cell carries no reason"))?;
            if !ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.contains(&reason) {
                return Err(inconsistent(
                    "the out-of-slice reason is outside the vocabulary",
                ));
            }
        }
        _ => return Err(inconsistent("the stance token was not parsed")),
    }
    Ok(())
}

/// Canonicalize a document's family cells in declared order, refusing an
/// `out_of_scope` required family by name.
fn canonical_families(
    document_id: &str,
    cells: &[RouteFamilyCell],
) -> Result<Vec<RouteFamilyCell>, RouteReceiptError> {
    let mut ordered = Vec::with_capacity(ROUTE_FAMILY_ORDER.len());
    for family in ROUTE_FAMILY_ORDER {
        let mut matching = cells.iter().filter(|cell| cell.family == family);
        let cell = matching
            .next()
            .ok_or_else(|| RouteReceiptError::FamilyMatrixIncomplete {
                document: document_id.to_owned(),
                family: family.to_owned(),
            })?;
        if matching.next().is_some() {
            return Err(RouteReceiptError::FamilyMatrixIncomplete {
                document: document_id.to_owned(),
                family: family.to_owned(),
            });
        }
        parse_family_disposition(document_id, family, cell)?;
        ordered.push(cell.clone());
    }
    for cell in cells {
        if !ROUTE_FAMILY_ORDER.contains(&cell.family.as_str()) {
            return Err(RouteReceiptError::UnknownFamily {
                document: document_id.to_owned(),
                family: cell.family.clone(),
            });
        }
    }
    Ok(ordered)
}

/// Parse a family cell's disposition/reason pair by name.
fn parse_family_disposition(
    document_id: &str,
    family: &str,
    cell: &RouteFamilyCell,
) -> Result<(), RouteReceiptError> {
    let inconsistent = |detail: &str| RouteReceiptError::FamilyCellInconsistent {
        document: document_id.to_owned(),
        family: family.to_owned(),
        detail: detail.to_owned(),
    };
    match cell.disposition.as_str() {
        "out_of_scope" => Err(RouteReceiptError::FamilyOutOfScope {
            document: document_id.to_owned(),
            family: family.to_owned(),
            token: "out_of_scope".to_owned(),
        }),
        "measured" => {
            if cell.reason.is_some() {
                return Err(inconsistent("a `measured` cell carries a reason"));
            }
            if cell.observed_count.is_none() {
                return Err(inconsistent("a `measured` cell carries no observed count"));
            }
            Ok(())
        }
        "unmet" => {
            if cell.observed_count.is_some() {
                return Err(inconsistent("an `unmet` cell carries an observed count"));
            }
            let reason = cell
                .reason
                .as_deref()
                .ok_or_else(|| inconsistent("an `unmet` cell carries no typed reason"))?;
            if !ROUTE_UNMET_REASON_VOCABULARY.contains(&reason) {
                return Err(inconsistent("the unmet reason is outside the vocabulary"));
            }
            Ok(())
        }
        "refused" => {
            if cell.observed_count.is_some() {
                return Err(inconsistent("a `refused` cell carries an observed count"));
            }
            let reason = cell
                .reason
                .as_deref()
                .ok_or_else(|| inconsistent("a `refused` cell carries no typed reason"))?;
            if !ROUTE_REFUSAL_REASON_VOCABULARY.contains(&reason) {
                return Err(inconsistent("the refusal reason is outside the vocabulary"));
            }
            Ok(())
        }
        "not_observed" => {
            if cell.reason.is_some() {
                return Err(inconsistent("a `not_observed` cell carries a reason"));
            }
            if cell.observed_count.is_some() {
                return Err(inconsistent(
                    "a `not_observed` cell carries an observed count",
                ));
            }
            Ok(())
        }
        other => Err(RouteReceiptError::FamilyCellInconsistent {
            document: document_id.to_owned(),
            family: family.to_owned(),
            detail: format!("`{other}` is not a family disposition"),
        }),
    }
}

/// Validate one body axis, refusing an untyped gap by name.
fn validate_body_axis(document_id: &str, axis: &RouteBodyAxis) -> Result<(), RouteReceiptError> {
    for (field, token, vocabulary) in [
        (
            "plane_preservation",
            axis.plane_preservation.as_str(),
            ROUTE_BODY_VERDICT_VOCABULARY.as_slice(),
        ),
        (
            "plane_gap_reason",
            axis.plane_gap_reason.as_str(),
            ROUTE_BODY_REASON_VOCABULARY.as_slice(),
        ),
    ] {
        if !vocabulary.contains(&token) {
            return Err(RouteReceiptError::InvalidBodyVocabulary {
                field: format!("document[{document_id}].body.{field}"),
                token: token.to_owned(),
            });
        }
    }
    let typed_gap = axis.plane_preservation == "content_loss" || axis.plane_absent_units > 0;
    if typed_gap != axis.plane_d498_reopen_required {
        return Err(RouteReceiptError::BodyGapUntyped {
            document: document_id.to_owned(),
            detail: "the P1 D498 flag does not match the P1 verdict".to_owned(),
        });
    }
    if typed_gap && axis.plane_gap_reason == "matched" {
        return Err(RouteReceiptError::BodyGapUntyped {
            document: document_id.to_owned(),
            detail: "a proven gap carries the `matched` reason".to_owned(),
        });
    }
    Ok(())
}

fn require_digest(field: &str, value: &str) -> Result<(), RouteReceiptError> {
    if !is_sha256_digest(value) {
        return Err(RouteReceiptError::InvalidDigest {
            field: field.to_owned(),
        });
    }
    Ok(())
}

fn require_bare_digest(field: &str, value: &str) -> Result<(), RouteReceiptError> {
    if !is_sha256_hex(value) {
        return Err(RouteReceiptError::InvalidBareDigest {
            field: field.to_owned(),
        });
    }
    Ok(())
}

fn require_repo_path(field: &str, value: &str) -> Result<(), RouteReceiptError> {
    if !is_repo_relative_path(value) {
        return Err(RouteReceiptError::InvalidPath {
            field: field.to_owned(),
        });
    }
    Ok(())
}

// ───────────────────────── validation ─────────────────────────

/// The committed sources a receipt is re-read against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteExpectation {
    /// sha256 of the declaration carrier, recomputed from its bytes.
    pub declaration_revision_sha256: String,
    /// The recomputed corpus pin.
    pub corpus: RouteDeclaredCorpus,
    /// The declared slice boundary.
    pub boundary: RouteBoundaryInput,
    /// The recomputed freeze.
    pub freeze: RouteFreezeDeclaration,
    /// The families that must be present in every document's matrix.
    pub required_families: Vec<String>,
}

/// Re-read a route receipt against the committed sources it claims.
///
/// This is the independent half of the route contract: it re-runs the
/// structural checks on the *parsed* bytes and refuses the receipt-level
/// mutations a builder cannot express — a raised threshold, a stale freeze, a
/// drifted binding, an out-of-slice stage counted as measured, an
/// `out_of_scope` family, an untyped body gap, or a pooled population.
pub fn validate_route_receipt(
    receipt: &RouteReceipt,
    expected: &RouteExpectation,
) -> Result<(), RouteReceiptError> {
    if receipt.schema_version != NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION {
        return Err(RouteReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(RouteReceiptError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(RouteReceiptError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(RouteReceiptError::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }

    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(RouteReceiptError::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(RouteReceiptError::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }

    if receipt.guard.observer != GuardObserver::JavaTransportFetchTei {
        return Err(RouteReceiptError::GuardObserverDrift {
            found: receipt.guard.observer.as_str().to_owned(),
        });
    }
    if receipt.guard.observed_java_calls > 0 {
        return Err(RouteReceiptError::ObservedJavaOnRoute {
            attempts: receipt.guard.observed_java_calls,
        });
    }
    if !receipt.guard.installed || !receipt.guard.identity_isolated {
        return Err(RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: receipt.guard.observed_java_calls,
        });
    }

    if receipt.limits.freeze_revision != expected.freeze.freeze_revision {
        return Err(RouteReceiptError::StaleFreezeRevision {
            expected: expected.freeze.freeze_revision.clone(),
            found: receipt.limits.freeze_revision.clone(),
        });
    }
    check_limits(&receipt.limits, &expected.freeze)?;

    if receipt.declaration.manifest != ROUTE_DECLARATION_MANIFEST_REL {
        return Err(RouteReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: ROUTE_DECLARATION_MANIFEST_REL.to_owned(),
            found: receipt.declaration.manifest.clone(),
        });
    }
    if receipt.declaration.pointer != ROUTE_DECLARATION_POINTER {
        return Err(RouteReceiptError::BindingDrift {
            field: "declaration.pointer".to_owned(),
            expected: ROUTE_DECLARATION_POINTER.to_owned(),
            found: receipt.declaration.pointer.clone(),
        });
    }
    if receipt.declaration.revision_sha256 != expected.declaration_revision_sha256 {
        return Err(RouteReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: expected.declaration_revision_sha256.clone(),
            found: receipt.declaration.revision_sha256.clone(),
        });
    }

    if receipt.redaction != Redaction::clean() {
        return Err(RouteReceiptError::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    check_boundary(&expected.boundary)?;

    // Documents: exact coverage, typed matrices.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for document in &receipt.documents {
        if !seen.insert(document.id.as_str()) {
            return Err(RouteReceiptError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
        let declared = expected
            .corpus
            .documents
            .iter()
            .find(|declared| declared.id == document.id)
            .ok_or_else(|| RouteReceiptError::UnknownDocument {
                document: document.id.clone(),
            })?;
        require_bare_digest("document.sha256", &document.sha256)?;
        if document.sha256 != declared.sha256 {
            return Err(RouteReceiptError::DocumentShaDrift {
                document: document.id.clone(),
                expected: declared.sha256.clone(),
                found: document.sha256.clone(),
            });
        }
        if document.pages != declared.pages {
            return Err(RouteReceiptError::PageCountDrift {
                document: document.id.clone(),
                expected: declared.pages,
                found: document.pages,
            });
        }
        require_repo_path("document.source_pdf", &document.source_pdf)?;
        require_digest(
            &format!("document[{}].route_digest", document.id),
            &document.route_digest,
        )?;
        validate_body_axis(&document.id, &document.body)?;
        canonical_stages(&document.id, &expected.boundary, &document.stages)?;
        let families = canonical_families(&document.id, &document.families)?;
        for family in &expected.required_families {
            if !ROUTE_FAMILY_ORDER.contains(&family.as_str()) {
                return Err(RouteReceiptError::UnknownFamily {
                    document: document.id.clone(),
                    family: family.clone(),
                });
            }
        }
        for required in &expected.required_families {
            if !families.iter().any(|cell| &cell.family == required) {
                return Err(RouteReceiptError::FamilyMatrixIncomplete {
                    document: document.id.clone(),
                    family: required.clone(),
                });
            }
        }
    }
    for declared in &expected.corpus.documents {
        if !seen.contains(declared.id.as_str()) {
            return Err(RouteReceiptError::MissingDocument {
                document: declared.id.clone(),
            });
        }
    }

    // Populations: disjoint, complete, frozen metrics.
    let frozen: BTreeSet<&str> = expected
        .freeze
        .metrics
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let mut population_seen: BTreeSet<&str> = BTreeSet::new();
    for population in &receipt.populations {
        if !ROUTE_POPULATION_VOCABULARY.contains(&population.population.token()) {
            return Err(RouteReceiptError::UnknownPopulation {
                population: population.population.token().to_owned(),
            });
        }
        if population.document_count != population.documents.len() as u64 {
            return Err(RouteReceiptError::PopulationStratificationInvalid {
                detail: format!(
                    "population `{}` declares {} documents but carries {}",
                    population.population.token(),
                    population.document_count,
                    population.documents.len()
                ),
            });
        }
        let page_sum: u64 = population.documents.iter().map(|doc| doc.pages).sum();
        if page_sum != population.page_denominator {
            return Err(RouteReceiptError::PopulationStratificationInvalid {
                detail: format!(
                    "population `{}` page denominator drift",
                    population.population.token()
                ),
            });
        }
        for identity in &population.documents {
            let declared = expected
                .corpus
                .documents
                .iter()
                .find(|declared| declared.id == identity.id)
                .ok_or_else(|| RouteReceiptError::UnknownDocument {
                    document: identity.id.clone(),
                })?;
            if identity.sha256 != declared.sha256 || identity.pages != declared.pages {
                return Err(RouteReceiptError::PopulationStratificationInvalid {
                    detail: format!("population document `{}` drifted from its pin", identity.id),
                });
            }
            if !population_seen.insert(identity.id.as_str()) {
                return Err(RouteReceiptError::PoolingClaimed {
                    field: format!("populations[{}]", population.population.token()),
                });
            }
        }
        for metric in &population.metrics {
            check_metric(metric, &frozen)?;
        }
    }
    for declared in &expected.corpus.documents {
        if !population_seen.contains(declared.id.as_str()) {
            return Err(RouteReceiptError::PopulationStratificationInvalid {
                detail: format!("document `{}` is in no population", declared.id),
            });
        }
    }

    // Residuals are derived from the typed cells; re-derive and compare.
    let mut unmeasured_stages = 0_u64;
    let mut unclassified_families = 0_u64;
    for document in &receipt.documents {
        unmeasured_stages += document
            .stages
            .iter()
            .filter(|cell| cell.stance != "native_measured")
            .count() as u64;
        unclassified_families += document
            .families
            .iter()
            .filter(|cell| {
                !ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&cell.disposition.as_str())
            })
            .count() as u64;
    }
    if receipt.residuals.unmeasured_stages != unmeasured_stages
        || receipt.residuals.unclassified_families != unclassified_families
    {
        return Err(RouteReceiptError::BindingDrift {
            field: "residuals".to_owned(),
            expected: format!("{unmeasured_stages}/{unclassified_families}"),
            found: format!(
                "{}/{}",
                receipt.residuals.unmeasured_stages, receipt.residuals.unclassified_families
            ),
        });
    }
    validate_body_axis("slice", &receipt.residuals.body)?;

    // Canonical re-serialization must not leak a payload marker.
    let bytes = receipt.artifact_json_bytes()?;
    let text =
        std::str::from_utf8(&bytes).map_err(|error| RouteReceiptError::ArtifactSerialization {
            reason: error.to_string(),
        })?;
    assert_route_redacted(text)?;

    Ok(())
}

/// Normalize a document identity the same way the sibling receipts do, so a
/// caller cannot invent its own join key.
pub fn route_normalize_document_identity(id: &str) -> String {
    normalize_document_identity(id)
}
