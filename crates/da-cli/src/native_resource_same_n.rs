//! S06/T06 (M514-1lss2s) — same-n resource comparison receipt.
//!
//! R141 requires a resource pair that names its measured peer row, and the wave
//! success criterion says same-n resource measurements confirm operability. No
//! committed artifact compared cost on *the same documents*: `native_core_smp`
//! is a synthetic `cpu_only` load (`reproducible:false` / `not_a_receipt:true`)
//! and `parse_batch_budget_live` is Java-only and container-wide. This module
//! owns the honest S06 deliverable: a corpus-bound receipt that pairs every
//! native metric with a measured reference row **only where a pinned same-n
//! source exists**, and records a typed absence otherwise (D510).
//!
//! ## The pairing rule is the contract
//!
//! `data/parse/comparison_contract.yaml#same_n` says `mismatch: hard_failure`,
//! so the builder refuses a receipt whose native document set or sha256 differs
//! from the reference set ([`SameNReceiptError::Mismatch`]) — by name, with the
//! offending document id. `pairing.pooled = forbidden` and
//! `pairing.saving_claim = forbidden` mean a number mixing the two populations,
//! or a claimed saving, is not expressible.
//!
//! ## A zero is not a measurement
//!
//! Every metric is either `observed` (a value **and** a source) or
//! `not_observed` (a closed reason and **no** value). A `not_observed` row that
//! carries a number is refused ([`SameNReceiptError::ZeroMasquerade`]), and a
//! reference row is only `observed` when its source is a repository-relative
//! measured carrier with a pinned sha256
//! ([`SameNReceiptError::ReferenceSourceUnpinned`]).
//!
//! ## Envelope reuse
//!
//! The claim envelope (locks, claim scope, redaction, carrier binding, guard
//! provenance) and the T02 limit freeze binding are reused from
//! [`crate::native_family_receipt`] and [`crate::native_route_receipt`] rather
//! than restated. The module is pure `std`/`serde`/`sha2`, so it and its
//! contract test compile in the default feature set.

use crate::native_family_receipt::{
    CarrierBinding, GuardObserver, GuardProvenance, GuardProvenanceReceipt, LEAK_MARKERS,
    REQUIRED_LOCKS, Redaction, is_repo_relative_path, is_sha256_digest, is_sha256_hex, sha256_hex,
    sha256_prefixed,
};
use crate::native_route_receipt::{
    ROUTE_DIRECTION_VOCABULARY, ROUTE_FREEZE_SCOPE, ROUTE_FREEZE_STATE_FROZEN,
    ROUTE_SILENT_CLAMP_FORBIDDEN, ROUTE_TUNED_AFTER_SCORING_FORBIDDEN, RouteLimitsBinding,
    RouteMetricDeclaration, RouteReceiptError,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted same-n resource comparison artifact.
pub const NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION: &str = "native-resource-same-n.v1";
/// Default artifact path of the same-n resource comparison.
pub const DEFAULT_SAME_N_ARTIFACT_REL: &str = "artifacts/parse_compat/native_resource_same_n.json";
/// Repository-relative carrier of the pinned reference work rows.
pub const SAME_N_REFERENCE_WORK_REL: &str = "artifacts/parse_compat/native_java_work.json";
/// Repository-relative carrier of the Java batch-budget live observation.
pub const SAME_N_REFERENCE_BUDGET_REL: &str = "artifacts/parse_compat/parse_batch_budget_live.json";
/// The only pairing rule a receipt may carry.
pub const SAME_N_PAIRING_RULE: &str = "same_documents_only";
/// The only `pooled` value a receipt may carry.
pub const SAME_N_POOLED_FORBIDDEN: &str = "forbidden";
/// The only `saving_claim` value a receipt may carry.
pub const SAME_N_SAVING_CLAIM_FORBIDDEN: &str = "forbidden";
/// The only absence policy a receipt may carry.
pub const SAME_N_ABSENCE_POLICY: &str = "typed_not_observed";
/// `same_n.mismatch` from `data/parse/comparison_contract.yaml`.
pub const SAME_N_MISMATCH_HARD_FAILURE: &str = "hard_failure";

/// The closed population vocabulary. `native` is the S06 Java-free route;
/// `reference` is the pinned Java work row measured on the same documents. There
/// is deliberately no `pooled` token, so a mixed number cannot be expressed.
pub const SAME_N_POPULATION_VOCABULARY: [&str; 2] = ["native", "reference"];

/// The closed observation-state vocabulary.
pub const SAME_N_OBSERVATION_STATE_VOCABULARY: [&str; 2] = ["observed", "not_observed"];

/// The closed absence-reason vocabulary. An unmeasured value names *why* it is
/// unmeasured; a number never stands in for an absence.
pub const SAME_N_ABSENCE_REASON_VOCABULARY: [&str; 5] = [
    "native_metric_not_measured",
    "reference_row_absent",
    "reference_row_not_same_n",
    "reference_row_not_measured",
    "not_applicable_to_population",
];

/// The native metrics this receipt compares, in canonical order. Every id is a
/// member of the T02 frozen metric set except `throughput`, which the freeze
/// carries without a ceiling direction and which is therefore observation-only.
pub const SAME_N_NATIVE_METRIC_ORDER: [&str; 6] = [
    "latency_p50_ms",
    "latency_p95_ms",
    "latency_p99_ms",
    "peak_rss_bytes",
    "peak_retained_bytes",
    "throughput",
];

/// The reference metrics this receipt carries, in canonical order.
pub const SAME_N_REFERENCE_METRIC_ORDER: [&str; 2] =
    ["java_model_invocations", "java_cpu_wall_clock_ns"];

/// The metric ids that must resolve to a frozen ceiling direction.
pub const SAME_N_FROZEN_CAP_METRICS: [&str; 5] = [
    "latency_p50_ms",
    "latency_p95_ms",
    "latency_p99_ms",
    "peak_rss_bytes",
    "peak_retained_bytes",
];

/// The declared measurement scope of the native resource rows. The native
/// number is the S03 plane extraction cost (the dominant native route cost),
/// never a restated Java latency.
pub const SAME_N_NATIVE_MEASUREMENT_SCOPE: &str = "native_plane_extraction";
/// The declared measurement scope of the reference rows.
pub const SAME_N_REFERENCE_MEASUREMENT_SCOPE: &str = "java_crf_fulltext";

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals of the same-n resource receipt. Every variant is a refusal,
/// never a silent downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SameNReceiptError {
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt claimed more than `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// A required lock is open or missing.
    LockOpened { lock: String, found: String },
    /// The serialized receipt leaked a path/host/TEI marker.
    RedactionLeak { marker: String },
    /// The native and reference document sets disagree (hard failure).
    Mismatch { detail: String },
    /// The receipt pooled populations.
    PooledClaimed { found: String },
    /// The receipt claimed a saving.
    SavingClaimed { found: String },
    /// The pairing rule drifted.
    PairingRuleDrift { found: String },
    /// A `not_observed` row carried a number, or an `observed` row did not.
    ZeroMasquerade { metric: String, detail: String },
    /// An absence reason outside the closed vocabulary.
    AbsenceReasonUnknown { metric: String, reason: String },
    /// An observed reference row has no pinned measured source.
    ReferenceSourceUnpinned { metric: String, detail: String },
    /// A metric id outside the declared vocabulary.
    MetricUnknown { metric: String },
    /// A metric is declared more than once.
    MetricDuplicate { metric: String },
    /// The cap direction of a frozen metric drifted from the freeze.
    DirectionDrift {
        metric: String,
        expected: String,
        found: String,
    },
    /// The limits are not frozen, or a metric is outside the frozen set.
    LimitNotFrozen { detail: String },
    /// The freeze was tuned after scoring.
    LimitTunedAfterScoring { found: String },
    /// The parsed receipt's freeze revision drifted from the recomputed one.
    StaleFreezeRevision { expected: String, found: String },
    /// An observed zero lacks an installed, identity-isolated guard.
    GuardMissingForObservedZero { detail: String },
    /// The route observed Java calls.
    ObservedJavaOnRoute { found: u64 },
    /// A population outside the closed vocabulary.
    PopulationUnknown { population: String },
    /// The corpus binding is empty or disagrees with the document rows.
    CorpusDrift { detail: String },
    /// A redaction flag is not the required `false`.
    RedactionFlag { field: String, found: bool },
    /// Malformed JSON was handed to the parser.
    Malformed { detail: String },
    /// A required field was absent.
    MissingField { field: String },
}

impl fmt::Display for SameNReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SameNReceiptError::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "schema drift on `{field}`: expected `{expected}`, found `{found}`"
            ),
            SameNReceiptError::ClaimScopeDrift { found } => {
                write!(f, "claim scope drifted: found `{found}`")
            }
            SameNReceiptError::ProductCoverageClaimed { found } => {
                write!(f, "product coverage claimed: found `{found}`")
            }
            SameNReceiptError::ThresholdClaimed { found } => {
                write!(f, "thresholds claimed: found `{found}`")
            }
            SameNReceiptError::LockOpened { lock, found } => {
                write!(f, "lock `{lock}` is not closed: found `{found}`")
            }
            SameNReceiptError::RedactionLeak { marker } => {
                write!(f, "redaction leak: marker `{marker}`")
            }
            SameNReceiptError::Mismatch { detail } => {
                write!(f, "same-n document mismatch (hard failure): {detail}")
            }
            SameNReceiptError::PooledClaimed { found } => {
                write!(f, "pooled value claimed: found `{found}`")
            }
            SameNReceiptError::SavingClaimed { found } => {
                write!(f, "saving claimed: found `{found}`")
            }
            SameNReceiptError::PairingRuleDrift { found } => {
                write!(f, "pairing rule drifted: found `{found}`")
            }
            SameNReceiptError::ZeroMasquerade { metric, detail } => {
                write!(f, "metric `{metric}` is not observable: {detail}")
            }
            SameNReceiptError::AbsenceReasonUnknown { metric, reason } => {
                write!(
                    f,
                    "metric `{metric}` names unknown absence reason `{reason}`"
                )
            }
            SameNReceiptError::ReferenceSourceUnpinned { metric, detail } => {
                write!(
                    f,
                    "reference metric `{metric}` has no pinned measured source: {detail}"
                )
            }
            SameNReceiptError::MetricUnknown { metric } => {
                write!(f, "metric `{metric}` is outside the declared vocabulary")
            }
            SameNReceiptError::MetricDuplicate { metric } => {
                write!(f, "metric `{metric}` is declared more than once")
            }
            SameNReceiptError::DirectionDrift {
                metric,
                expected,
                found,
            } => write!(
                f,
                "metric `{metric}` direction drifted: expected `{expected}`, found `{found}`"
            ),
            SameNReceiptError::LimitNotFrozen { detail } => {
                write!(f, "limits are not frozen: {detail}")
            }
            SameNReceiptError::LimitTunedAfterScoring { found } => {
                write!(f, "limits tuned after scoring: found `{found}`")
            }
            SameNReceiptError::StaleFreezeRevision { expected, found } => {
                write!(
                    f,
                    "stale freeze revision: recomputed `{expected}`, receipt `{found}`"
                )
            }
            SameNReceiptError::GuardMissingForObservedZero { detail } => {
                write!(f, "observed zero without a guard: {detail}")
            }
            SameNReceiptError::ObservedJavaOnRoute { found } => {
                write!(f, "the route observed {found} Java calls")
            }
            SameNReceiptError::PopulationUnknown { population } => {
                write!(
                    f,
                    "population `{population}` is outside the closed vocabulary"
                )
            }
            SameNReceiptError::CorpusDrift { detail } => write!(f, "corpus drift: {detail}"),
            SameNReceiptError::RedactionFlag { field, found } => {
                write!(f, "redaction flag `{field}` must be false, found {found}")
            }
            SameNReceiptError::Malformed { detail } => write!(f, "malformed receipt: {detail}"),
            SameNReceiptError::MissingField { field } => write!(f, "missing field `{field}`"),
        }
    }
}

impl std::error::Error for SameNReceiptError {}

impl From<RouteReceiptError> for SameNReceiptError {
    fn from(error: RouteReceiptError) -> Self {
        SameNReceiptError::Malformed {
            detail: format!("route receipt binding refused: {error}"),
        }
    }
}

// ───────────────────────────── types ─────────────────────────────

/// The pinned identity of one document in the compared corpus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNDocumentIdentity {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
}

/// Where a measured value came from. A reference source must name a
/// repository-relative carrier and its pinned sha256 or the row is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNSource {
    /// `native_resource_run` | `measured_reference`.
    pub kind: String,
    /// Repository-relative carrier of the measurement, when external.
    pub manifest: Option<String>,
    /// Pointer inside the carrier.
    pub pointer: Option<String>,
    /// Bare sha256 of the carrier bytes, when external.
    pub sha256: Option<String>,
    /// The carrier's own measurement state, when external.
    pub measurement_state: Option<String>,
    /// The metric id inside the carrier, when it differs from the receipt's.
    pub metric_id: Option<String>,
}

/// One metric observation: `observed` carries a value and a source;
/// `not_observed` carries a closed reason and no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNMetricObservation {
    /// The metric id.
    pub id: String,
    /// `ceiling` for a frozen cap metric, absent otherwise.
    pub direction: Option<String>,
    /// `observed` | `not_observed`.
    pub state: String,
    /// The measured value; present exactly when `observed`.
    pub value: Option<u64>,
    /// The closed absence reason; present exactly when `not_observed`.
    pub reason: Option<String>,
    /// The measured source; present exactly when `observed`.
    pub source: Option<SameNSource>,
}

impl SameNMetricObservation {
    /// Whether this row carries a measurement.
    pub fn is_observed(&self) -> bool {
        self.state == "observed"
    }
}

/// One population's measured rows, with its own pinned members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNPopulationReceipt {
    /// `native` | `reference`.
    pub population: String,
    /// The declared measurement scope of this population's numbers.
    pub measurement_scope: String,
    /// Pinned document count in this population.
    pub document_count: u64,
    /// Pinned page denominator of this population.
    pub page_denominator: u64,
    /// Pinned identities, in carrier order.
    pub documents: Vec<SameNDocumentIdentity>,
    /// Measured rows for this population.
    pub metrics: Vec<SameNMetricObservation>,
}

/// One document's native latency sample and its paired reference rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNDocumentRow {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
    /// The measured native plane extraction latency for this document.
    pub native_latency_ms: Option<u64>,
    /// The paired reference rows measured on this same document.
    pub reference: Vec<SameNMetricObservation>,
}

/// The declared pairing frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNPairing {
    /// Always `same_documents_only`.
    pub rule: String,
    /// Always `forbidden`.
    pub pooled: String,
    /// Always `forbidden`.
    pub saving_claim: String,
    /// Always `typed_not_observed`.
    pub absence_policy: String,
    /// The `same_n.mismatch` disposition from the comparison contract.
    pub mismatch_disposition: String,
}

/// The pinned corpus the comparison binds to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNCorpusBinding {
    /// Pinned document count.
    pub document_count: u64,
    /// Pinned page denominator.
    pub page_denominator: u64,
    /// Pinned identities, in carrier order.
    pub documents: Vec<SameNDocumentIdentity>,
}

/// The comparison residuals: the closure counters the contract test recomputes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNResiduals {
    /// Observed metric rows across both populations and all documents.
    pub observed_metrics: u64,
    /// Typed-absence metric rows.
    pub absent_metrics: u64,
    /// Rows whose state is outside the closed vocabulary (must be zero).
    pub untyped_metrics: u64,
    /// Reference rows bound to a pinned measured source.
    pub reference_rows_bound: u64,
}

/// The emitted same-n resource comparison receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameNReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// `diagnostic_only`.
    pub claim_scope: String,
    /// `none`.
    pub product_coverage_effect: String,
    /// `not_accepted`.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// Binding to the slice declaration carrier.
    pub declaration: CarrierBinding,
    /// Guard provenance for the observed Java zero.
    pub guard: GuardProvenanceReceipt,
    /// The T02 limit freeze binding.
    pub limits: RouteLimitsBinding,
    /// The declared pairing frame.
    pub pairing: SameNPairing,
    /// The pinned corpus.
    pub corpus: SameNCorpusBinding,
    /// One row per population.
    pub populations: Vec<SameNPopulationReceipt>,
    /// One row per document.
    pub documents: Vec<SameNDocumentRow>,
    /// The closure counters.
    pub residuals: SameNResiduals,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl SameNReceipt {
    /// Parse a receipt from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, SameNReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, SameNReceiptError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| SameNReceiptError::Malformed {
                detail: error.to_string(),
            })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
        assert_same_n_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized receipt.
pub fn assert_same_n_redacted(json: &str) -> Result<(), SameNReceiptError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(SameNReceiptError::RedactionLeak {
                marker: (*marker).to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── inputs ─────────────────────────────

/// One measured native document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameNMeasuredDocument {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 observed for the bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
    /// The measured native plane extraction latency, when measured.
    pub native_latency_ms: Option<u64>,
    /// The reference rows measured on this same document.
    pub reference: Vec<SameNMetricObservation>,
}

/// The full builder input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameNInput {
    /// Binding to the declaration carrier.
    pub declaration: CarrierBinding,
    /// Guard provenance.
    pub guard: GuardProvenance,
    /// The T02 freeze declaration the limits were resolved from.
    pub freeze: crate::native_route_receipt::RouteFreezeDeclaration,
    /// The frozen limits binding.
    pub limits: RouteLimitsBinding,
    /// The pinned corpus.
    pub corpus: SameNCorpusBinding,
    /// One measured row per document.
    pub documents: Vec<SameNMeasuredDocument>,
    /// The population-level native metrics.
    pub native_metrics: Vec<SameNMetricObservation>,
    /// The population-level reference metrics.
    pub reference_metrics: Vec<SameNMetricObservation>,
    /// The declared claim scope.
    pub claim_scope: String,
    /// The declared product-coverage effect.
    pub product_coverage_effect: String,
    /// The declared thresholds state.
    pub thresholds: String,
}

/// What the validator recomputes and compares against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameNExpectation {
    /// The recomputed composite freeze revision.
    pub freeze_revision: String,
    /// The pinned corpus.
    pub corpus: SameNCorpusBinding,
    /// The reference document set the receipt must match exactly.
    pub reference_documents: Vec<SameNDocumentIdentity>,
}

// ───────────────────────────── builder ─────────────────────────────

/// Build the receipt, refusing every contract violation by name.
pub fn build_same_n_receipt(input: &SameNInput) -> Result<SameNReceipt, SameNReceiptError> {
    let locks = required_locks()?;
    check_locks(&locks)?;
    check_claim(
        &input.claim_scope,
        &input.product_coverage_effect,
        &input.thresholds,
    )?;
    check_limits(&input.limits)?;
    // The limits must be the ones the declared freeze produced, not a tuned set.
    if input.freeze.freeze_revision != input.limits.freeze_revision
        || input.freeze.metrics != input.limits.metrics
    {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!(
                "limits {} do not match the declared freeze {}",
                input.limits.freeze_revision, input.freeze.freeze_revision
            ),
        });
    }

    // The observed Java zero is a measurement only under a live guard.
    if input.guard.observed_java_calls != 0 {
        return Err(SameNReceiptError::ObservedJavaOnRoute {
            found: input.guard.observed_java_calls,
        });
    }
    if !input.guard.installed || !input.guard.identity_isolated {
        return Err(SameNReceiptError::GuardMissingForObservedZero {
            detail: format!(
                "installed={}, identity_isolated={}",
                input.guard.installed, input.guard.identity_isolated
            ),
        });
    }

    // Corpus ↔ document rows must agree; the paired reference set must too.
    if input.corpus.document_count != input.corpus.documents.len() as u64 {
        return Err(SameNReceiptError::CorpusDrift {
            detail: format!(
                "declared count {} but {} identities",
                input.corpus.document_count,
                input.corpus.documents.len()
            ),
        });
    }
    if input.documents.len() != input.corpus.documents.len() {
        return Err(SameNReceiptError::CorpusDrift {
            detail: format!(
                "{} measured documents for {} pinned identities",
                input.documents.len(),
                input.corpus.documents.len()
            ),
        });
    }
    let pinned: BTreeMap<&str, &SameNDocumentIdentity> = input
        .corpus
        .documents
        .iter()
        .map(|document| (document.id.as_str(), document))
        .collect();
    for document in &input.documents {
        let identity =
            pinned
                .get(document.id.as_str())
                .ok_or_else(|| SameNReceiptError::Mismatch {
                    detail: format!(
                        "measured document `{}` is not in the pinned corpus",
                        document.id
                    ),
                })?;
        if identity.sha256 != document.sha256 {
            return Err(SameNReceiptError::Mismatch {
                detail: format!(
                    "document `{}` sha256 drifted: measured {}, pinned {}",
                    document.id, document.sha256, identity.sha256
                ),
            });
        }
        if identity.pages != document.pages {
            return Err(SameNReceiptError::Mismatch {
                detail: format!(
                    "document `{}` page count drifted: measured {}, pinned {}",
                    document.id, document.pages, identity.pages
                ),
            });
        }
    }

    // Every reference row must name a pinned, measured, same-n carrier.
    for document in &input.documents {
        for row in &document.reference {
            check_reference(row)?;
        }
    }

    // Native rows must be in the declared vocabulary and freeze-bound.
    for row in &input.native_metrics {
        require_known_native_metric(row, &input.limits)?;
    }
    for row in &input.reference_metrics {
        require_known_reference_metric(row)?;
    }
    check_no_duplicates(&input.native_metrics)?;
    check_no_duplicates(&input.reference_metrics)?;

    let corpus = input.corpus.clone();
    let native_population = SameNPopulationReceipt {
        population: "native".to_owned(),
        measurement_scope: SAME_N_NATIVE_MEASUREMENT_SCOPE.to_owned(),
        document_count: corpus.document_count,
        page_denominator: corpus.page_denominator,
        documents: corpus.documents.clone(),
        metrics: input.native_metrics.clone(),
    };
    let reference_population = SameNPopulationReceipt {
        population: "reference".to_owned(),
        measurement_scope: SAME_N_REFERENCE_MEASUREMENT_SCOPE.to_owned(),
        document_count: corpus.document_count,
        page_denominator: corpus.page_denominator,
        documents: corpus.documents.clone(),
        metrics: input.reference_metrics.clone(),
    };

    let document_reference: Vec<&SameNMetricObservation> = input
        .documents
        .iter()
        .flat_map(|document| document.reference.iter())
        .collect();
    let residuals = residuals_of(
        &input.native_metrics,
        &input.reference_metrics,
        &document_reference,
    );

    Ok(SameNReceipt {
        schema_version: NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION.to_owned(),
        claim_scope: input.claim_scope.clone(),
        product_coverage_effect: input.product_coverage_effect.clone(),
        thresholds: input.thresholds.clone(),
        locks,
        declaration: input.declaration.clone(),
        guard: GuardProvenanceReceipt {
            observer: GuardObserver::JavaTransportFetchTei,
            installed: input.guard.installed,
            identity_isolated: input.guard.identity_isolated,
            observed_java_calls: input.guard.observed_java_calls,
        },
        limits: input.limits.clone(),
        pairing: SameNPairing {
            rule: SAME_N_PAIRING_RULE.to_owned(),
            pooled: SAME_N_POOLED_FORBIDDEN.to_owned(),
            saving_claim: SAME_N_SAVING_CLAIM_FORBIDDEN.to_owned(),
            absence_policy: SAME_N_ABSENCE_POLICY.to_owned(),
            mismatch_disposition: SAME_N_MISMATCH_HARD_FAILURE.to_owned(),
        },
        corpus,
        populations: vec![native_population, reference_population],
        documents: input
            .documents
            .iter()
            .map(|document| SameNDocumentRow {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
                native_latency_ms: document.native_latency_ms,
                reference: document.reference.clone(),
            })
            .collect(),
        residuals,
        redaction: Redaction::clean(),
    })
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed receipt and refuse what a builder cannot
/// express: a stale freeze revision, a reopened lock, a drifted population, a
/// zero masquerade, or a pooled/saving claim.
pub fn validate_same_n_receipt(
    receipt: &SameNReceipt,
    expectation: &SameNExpectation,
) -> Result<(), SameNReceiptError> {
    if receipt.schema_version != NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION {
        return Err(SameNReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    // A drifted freeze revision is named before the generic freeze check.
    if receipt.limits.freeze_revision != expectation.freeze_revision {
        return Err(SameNReceiptError::StaleFreezeRevision {
            expected: expectation.freeze_revision.clone(),
            found: receipt.limits.freeze_revision.clone(),
        });
    }
    check_claim(
        &receipt.claim_scope,
        &receipt.product_coverage_effect,
        &receipt.thresholds,
    )?;
    check_locks(&receipt.locks)?;
    check_limits(&receipt.limits)?;

    if receipt.pairing.rule != SAME_N_PAIRING_RULE {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.rule.clone(),
        });
    }
    if receipt.pairing.pooled != SAME_N_POOLED_FORBIDDEN {
        return Err(SameNReceiptError::PooledClaimed {
            found: receipt.pairing.pooled.clone(),
        });
    }
    if receipt.pairing.saving_claim != SAME_N_SAVING_CLAIM_FORBIDDEN {
        return Err(SameNReceiptError::SavingClaimed {
            found: receipt.pairing.saving_claim.clone(),
        });
    }
    if receipt.pairing.absence_policy != SAME_N_ABSENCE_POLICY {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.absence_policy.clone(),
        });
    }
    if receipt.pairing.mismatch_disposition != SAME_N_MISMATCH_HARD_FAILURE {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.mismatch_disposition.clone(),
        });
    }

    // Corpus must equal the expectation, and the paired reference set must be
    // the same documents: `same_n.mismatch = hard_failure`.
    if receipt.corpus != expectation.corpus {
        return Err(SameNReceiptError::Mismatch {
            detail: "receipt corpus differs from the pinned corpus".to_owned(),
        });
    }
    let reference: BTreeSet<(&str, &str)> = expectation
        .reference_documents
        .iter()
        .map(|document| (document.id.as_str(), document.sha256.as_str()))
        .collect();
    let native: BTreeSet<(&str, &str)> = receipt
        .corpus
        .documents
        .iter()
        .map(|document| (document.id.as_str(), document.sha256.as_str()))
        .collect();
    if reference != native {
        let only_native: Vec<&str> = native.difference(&reference).map(|(id, _)| *id).collect();
        let only_reference: Vec<&str> = reference.difference(&native).map(|(id, _)| *id).collect();
        return Err(SameNReceiptError::Mismatch {
            detail: format!(
                "native-only {:?}, reference-only {:?}",
                only_native, only_reference
            ),
        });
    }

    if receipt.guard.observed_java_calls != 0 {
        return Err(SameNReceiptError::ObservedJavaOnRoute {
            found: receipt.guard.observed_java_calls,
        });
    }
    if !receipt.guard.installed || !receipt.guard.identity_isolated {
        return Err(SameNReceiptError::GuardMissingForObservedZero {
            detail: format!(
                "installed={}, identity_isolated={}",
                receipt.guard.installed, receipt.guard.identity_isolated
            ),
        });
    }

    let mut seen_native = BTreeSet::new();
    let mut seen_reference = BTreeSet::new();
    for population in &receipt.populations {
        if !SAME_N_POPULATION_VOCABULARY.contains(&population.population.as_str()) {
            return Err(SameNReceiptError::PopulationUnknown {
                population: population.population.clone(),
            });
        }
        if population.document_count != receipt.corpus.document_count
            || population.page_denominator != receipt.corpus.page_denominator
        {
            return Err(SameNReceiptError::Mismatch {
                detail: format!(
                    "population `{}` holds {}/{} but the corpus pins {}/{}",
                    population.population,
                    population.document_count,
                    population.page_denominator,
                    receipt.corpus.document_count,
                    receipt.corpus.page_denominator
                ),
            });
        }
        if population.documents != receipt.corpus.documents {
            return Err(SameNReceiptError::Mismatch {
                detail: format!(
                    "population `{}` documents differ from the corpus",
                    population.population
                ),
            });
        }
        for row in &population.metrics {
            if population.population == "native" {
                require_known_native_metric(row, &receipt.limits)?;
                if !seen_native.insert(row.id.clone()) {
                    return Err(SameNReceiptError::MetricDuplicate {
                        metric: row.id.clone(),
                    });
                }
            } else {
                require_known_reference_metric(row)?;
                if !seen_reference.insert(row.id.clone()) {
                    return Err(SameNReceiptError::MetricDuplicate {
                        metric: row.id.clone(),
                    });
                }
            }
        }
    }

    if receipt.documents.len() != receipt.corpus.documents.len() {
        return Err(SameNReceiptError::CorpusDrift {
            detail: format!(
                "{} document rows for {} pinned identities",
                receipt.documents.len(),
                receipt.corpus.documents.len()
            ),
        });
    }
    for document in &receipt.documents {
        let identity = receipt
            .corpus
            .documents
            .iter()
            .find(|candidate| candidate.id == document.id)
            .ok_or_else(|| SameNReceiptError::Mismatch {
                detail: format!("document row `{}` is not in the pinned corpus", document.id),
            })?;
        if identity.sha256 != document.sha256 || identity.pages != document.pages {
            return Err(SameNReceiptError::Mismatch {
                detail: format!("document row `{}` drifted from the pin", document.id),
            });
        }
        for row in &document.reference {
            require_known_reference_metric(row)?;
            if row.is_observed() {
                check_reference(row)?;
            }
        }
    }

    assert_redaction(&receipt.redaction)?;
    let re_serialized =
        serde_json::to_vec(receipt).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
    let re_serialized =
        std::str::from_utf8(&re_serialized).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
    assert_same_n_redacted(re_serialized)?;

    // Recompute the closure counters; a mismatch is an untyped row.
    let document_reference: Vec<&SameNMetricObservation> = receipt
        .documents
        .iter()
        .flat_map(|document| document.reference.iter())
        .collect();
    let recomputed = residuals_of(
        &receipt
            .populations
            .iter()
            .find(|population| population.population == "native")
            .map(|population| population.metrics.clone())
            .unwrap_or_default(),
        &receipt
            .populations
            .iter()
            .find(|population| population.population == "reference")
            .map(|population| population.metrics.clone())
            .unwrap_or_default(),
        &document_reference,
    );
    if recomputed != receipt.residuals {
        return Err(SameNReceiptError::ZeroMasquerade {
            metric: "residuals".to_owned(),
            detail: format!(
                "declared {:?}, recomputed {:?}",
                receipt.residuals, recomputed
            ),
        });
    }

    Ok(())
}

// ───────────────────────────── helpers ─────────────────────────────

fn required_locks() -> Result<BTreeMap<String, String>, SameNReceiptError> {
    let mut locks = BTreeMap::new();
    for (lock, value) in REQUIRED_LOCKS {
        locks.insert((*lock).to_owned(), (*value).to_owned());
    }
    Ok(locks)
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), SameNReceiptError> {
    for (lock, expected) in REQUIRED_LOCKS {
        match locks.get(lock) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(SameNReceiptError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(SameNReceiptError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_claim(
    claim_scope: &str,
    product_coverage_effect: &str,
    thresholds: &str,
) -> Result<(), SameNReceiptError> {
    if claim_scope != crate::native_family_receipt::CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(SameNReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != crate::native_family_receipt::PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(SameNReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }
    if thresholds != crate::native_family_receipt::THRESHOLDS_NOT_ACCEPTED {
        return Err(SameNReceiptError::ThresholdClaimed {
            found: thresholds.to_owned(),
        });
    }
    Ok(())
}

fn check_limits(limits: &RouteLimitsBinding) -> Result<(), SameNReceiptError> {
    if limits.freeze_state != ROUTE_FREEZE_STATE_FROZEN {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_state `{}`", limits.freeze_state),
        });
    }
    if limits.freeze_scope != ROUTE_FREEZE_SCOPE {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_scope `{}`", limits.freeze_scope),
        });
    }
    if limits.tuned_after_scoring != ROUTE_TUNED_AFTER_SCORING_FORBIDDEN {
        return Err(SameNReceiptError::LimitTunedAfterScoring {
            found: limits.tuned_after_scoring.clone(),
        });
    }
    if limits.silent_clamp != ROUTE_SILENT_CLAMP_FORBIDDEN {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("silent_clamp `{}`", limits.silent_clamp),
        });
    }
    if limits.claim_without_freeze != "forbidden" {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("claim_without_freeze `{}`", limits.claim_without_freeze),
        });
    }
    if !limits.zero_is_not_a_measurement || !limits.pass_requires_frozen_threshold {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: "zero_is_not_a_measurement / pass_requires_frozen_threshold must be true"
                .to_owned(),
        });
    }
    if limits.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: "direction vocabulary drifted".to_owned(),
        });
    }
    if limits.freeze_revision.is_empty() || !is_sha256_digest(&limits.freeze_revision) {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_revision `{}`", limits.freeze_revision),
        });
    }
    Ok(())
}

/// The frozen direction for a metric id, or `None` when the freeze carries no
/// direction for it (an observation-only metric).
fn frozen_direction(limits: &RouteLimitsBinding, id: &str) -> Option<String> {
    limits
        .metrics
        .iter()
        .find(|declaration| declaration.id == id)
        .map(|declaration| declaration.direction.clone())
}

fn require_known_native_metric(
    row: &SameNMetricObservation,
    limits: &RouteLimitsBinding,
) -> Result<(), SameNReceiptError> {
    if !SAME_N_NATIVE_METRIC_ORDER.contains(&row.id.as_str()) {
        return Err(SameNReceiptError::MetricUnknown {
            metric: row.id.clone(),
        });
    }
    let frozen = frozen_direction(limits, &row.id);
    if SAME_N_FROZEN_CAP_METRICS.contains(&row.id.as_str()) {
        match frozen.as_deref() {
            Some("ceiling") if row.direction.as_deref() == Some("ceiling") => {}
            Some(other) => {
                return Err(SameNReceiptError::DirectionDrift {
                    metric: row.id.clone(),
                    expected: "ceiling".to_owned(),
                    found: other.to_owned(),
                });
            }
            None => {
                return Err(SameNReceiptError::LimitNotFrozen {
                    detail: format!("metric `{}` is not in the frozen set", row.id),
                });
            }
        }
    } else if frozen.is_some() || row.direction.is_some() {
        return Err(SameNReceiptError::DirectionDrift {
            metric: row.id.clone(),
            expected: "<absent>".to_owned(),
            found: row
                .direction
                .clone()
                .unwrap_or_else(|| frozen.unwrap_or_default()),
        });
    }
    check_observation_shape(row)
}

fn require_known_reference_metric(row: &SameNMetricObservation) -> Result<(), SameNReceiptError> {
    if !SAME_N_REFERENCE_METRIC_ORDER.contains(&row.id.as_str()) {
        return Err(SameNReceiptError::MetricUnknown {
            metric: row.id.clone(),
        });
    }
    check_observation_shape(row)
}

/// `not_observed` carries no value; `observed` carries a value and a source.
fn check_observation_shape(row: &SameNMetricObservation) -> Result<(), SameNReceiptError> {
    match row.state.as_str() {
        "observed" => {
            if row.value.is_none() {
                return Err(SameNReceiptError::ZeroMasquerade {
                    metric: row.id.clone(),
                    detail: "an observed row carries no value".to_owned(),
                });
            }
            if row.reason.is_some() {
                return Err(SameNReceiptError::ZeroMasquerade {
                    metric: row.id.clone(),
                    detail: "an observed row carries an absence reason".to_owned(),
                });
            }
            if row.source.is_none() {
                return Err(SameNReceiptError::ZeroMasquerade {
                    metric: row.id.clone(),
                    detail: "an observed row carries no source".to_owned(),
                });
            }
        }
        "not_observed" => {
            if row.value.is_some() {
                return Err(SameNReceiptError::ZeroMasquerade {
                    metric: row.id.clone(),
                    detail: "a not_observed row carries a number".to_owned(),
                });
            }
            let reason =
                row.reason
                    .as_deref()
                    .ok_or_else(|| SameNReceiptError::AbsenceReasonUnknown {
                        metric: row.id.clone(),
                        reason: "<absent>".to_owned(),
                    })?;
            if !SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason) {
                return Err(SameNReceiptError::AbsenceReasonUnknown {
                    metric: row.id.clone(),
                    reason: reason.to_owned(),
                });
            }
        }
        other => {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.id.clone(),
                detail: format!("unknown observation state `{other}`"),
            });
        }
    }
    Ok(())
}

/// An observed reference row must name a repository-relative measured carrier
/// with a pinned sha256.
fn check_reference(row: &SameNMetricObservation) -> Result<(), SameNReceiptError> {
    // Absent rows need no source, but still need a closed reason.
    if !row.is_observed() {
        return check_observation_shape(row);
    }
    let source = row
        .source
        .as_ref()
        .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: "no source".to_owned(),
        })?;
    if source.kind != "measured_reference" {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("source kind `{}`", source.kind),
        });
    }
    let manifest =
        source
            .manifest
            .as_deref()
            .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
                metric: row.id.clone(),
                detail: "no manifest".to_owned(),
            })?;
    if !is_repo_relative_path(manifest) {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("manifest `{manifest}` is not repository-relative"),
        });
    }
    let sha256 =
        source
            .sha256
            .as_deref()
            .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
                metric: row.id.clone(),
                detail: "no sha256".to_owned(),
            })?;
    if !is_sha256_hex(sha256) {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("sha256 `{sha256}` is not 64 hex chars"),
        });
    }
    if source.pointer.as_deref().unwrap_or_default().is_empty() {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: "no pointer".to_owned(),
        });
    }
    if source.measurement_state.as_deref() != Some("measured") {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!(
                "measurement_state `{}`",
                source.measurement_state.as_deref().unwrap_or("<absent>")
            ),
        });
    }
    Ok(())
}

fn check_no_duplicates(rows: &[SameNMetricObservation]) -> Result<(), SameNReceiptError> {
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(row.id.as_str()) {
            return Err(SameNReceiptError::MetricDuplicate {
                metric: row.id.clone(),
            });
        }
    }
    Ok(())
}

fn residuals_of(
    native_metrics: &[SameNMetricObservation],
    reference_metrics: &[SameNMetricObservation],
    document_reference: &[&SameNMetricObservation],
) -> SameNResiduals {
    let mut observed = 0u64;
    let mut absent = 0u64;
    let mut untyped = 0u64;
    let mut bound = 0u64;
    let mut visit = |row: &SameNMetricObservation| match row.state.as_str() {
        "observed" => {
            observed += 1;
            if row.source.as_ref().map(|source| source.kind.as_str()) == Some("measured_reference")
            {
                bound += 1;
            }
        }
        "not_observed" => absent += 1,
        _ => untyped += 1,
    };
    for row in native_metrics {
        visit(row);
    }
    for row in reference_metrics {
        visit(row);
    }
    for document in document_reference {
        visit(document);
    }
    SameNResiduals {
        observed_metrics: observed,
        absent_metrics: absent,
        untyped_metrics: untyped,
        reference_rows_bound: bound,
    }
}

fn assert_redaction(redaction: &Redaction) -> Result<(), SameNReceiptError> {
    let flags = [
        ("raw_text", redaction.raw_text),
        ("raw_pdf", redaction.raw_pdf),
        ("tei_xml", redaction.tei_xml),
        ("images", redaction.images),
        ("physical_paths", redaction.physical_paths),
        ("hostname", redaction.hostname),
        ("executable", redaction.executable),
    ];
    for (field, value) in flags {
        if value {
            return Err(SameNReceiptError::RedactionFlag {
                field: field.to_owned(),
                found: value,
            });
        }
    }
    Ok(())
}

/// The deterministic digest of a pinned carrier, used to bind a reference row.
pub fn carrier_digest(bytes: &[u8]) -> String {
    sha256_prefixed(bytes)
}

/// The bare hex digest of a pinned carrier.
pub fn carrier_bare_digest(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// The frozen metric declaration set, as carried by the freeze.
pub fn frozen_metric_ids(limits: &RouteLimitsBinding) -> Vec<String> {
    limits
        .metrics
        .iter()
        .map(|declaration: &RouteMetricDeclaration| declaration.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabularies_are_closed_and_disjoint_from_display() {
        assert_eq!(SAME_N_NATIVE_METRIC_ORDER.len(), 6);
        assert_eq!(SAME_N_REFERENCE_METRIC_ORDER.len(), 2);
        for id in SAME_N_FROZEN_CAP_METRICS {
            assert!(SAME_N_NATIVE_METRIC_ORDER.contains(&id));
        }
        assert!(!SAME_N_POPULATION_VOCABULARY.contains(&"pooled"));
    }
}
