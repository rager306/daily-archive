//! S08/T01 (M514-1lss2s) — corpus-bound native acceptance measurement runner
//! with a typed gap ledger.
//!
//! This module is the *typed envelope* half of the S08 acceptance measurement.
//! It is deliberately pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2`: no
//! `da-parse`, no `pdf-layout`, no `pdf-classifier`. That is what lets the
//! module and its contract test (`native_corpus_acceptance_contract_test`)
//! compile in the default feature set, while the gated example
//! (`native_corpus_acceptance`) drives it against the real 610-document frozen
//! holdout.
//!
//! ## What this runner is, and what it refuses to be
//!
//! The frozen three thresholds live in
//! `artifacts/parse_compat/absorption_progress_reference_pins.json` and resolve
//! there **by pointer**, with the SAME definitions as the frozen reference: no
//! numeric threshold literal is restated here, and the candidate is never its
//! own reference. The full denominator is the frozen partition
//! `610 assigned = 531 measurable + 79 typed exclusions`; no document is dropped
//! and no share is narrowed.
//!
//! Where a frozen metric is natively observable *through the runner's own
//! producing path*, it is measured with a non-empty denominator. Where the
//! producing mechanism is absent (the class-parity axis) or the oracle is absent
//! (the S01 composition scanner is not on the native route path), the runner
//! emits a TYPED gap cell — `not_observable` with a machine-readable reason — and
//! that gate is carried into the acceptance package's blocking set. A family the
//! frozen schema still carries as `threshold_pending` is emitted as a
//! `threshold_pending` gap. Nothing is silently zeroed and nothing is renamed.
//!
//! The type order below is the canonical receipt order used by
//! `native_route_receipt` (envelope → reference → guard → populations → per
//! document rows → residuals), so canonical byte re-serialization is stable.
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, all five locks closed, no graph/import/
//! promotion (D127 / ADR-040 / ADR-076 §14).

use crate::native_family_receipt::{LEAK_MARKERS, Redaction};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Receipt schema version.
pub const NATIVE_ACCEPTANCE_SCHEMA_VERSION: &str = "native-corpus-acceptance.v1";
/// The canonical artifact the full run writes.
pub const DEFAULT_ACCEPTANCE_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_corpus_acceptance.json";

/// The frozen 610-document holdout identity carrier (read by pointer).
pub const HOLDOUT_MANIFEST_REL: &str = "data/parse/holdout_manifests.yaml";
/// Pointer inside the holdout manifest.
pub const HOLDOUT_MANIFEST_POINTER: &str = "articles";
/// The frozen prospective regression floor (read by pointer).
pub const REFERENCE_PINS_REL: &str =
    "artifacts/parse_compat/absorption_progress_reference_pins.json";
/// The executed reference run the pins derive from (read by pointer).
pub const REFERENCE_PROBE_REL: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
/// The closed absorption-progress schema: required scope + pending families.
pub const ABSORPTION_SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";

/// Only `diagnostic_only` is a legal claim scope.
pub const ACCEPTANCE_CLAIM_SCOPE_DIAGNOSTIC_ONLY: &str = "diagnostic_only";
/// Only `none` is a legal product-coverage effect.
pub const ACCEPTANCE_PRODUCT_COVERAGE_EFFECT_NONE: &str = "none";
/// Thresholds are recorded, never accepted, by this runner.
pub const ACCEPTANCE_THRESHOLDS_NOT_ACCEPTED: &str = "not_accepted";

/// The two legal gap kinds. A third kind is rejected by name.
pub const GAP_KIND_VOCABULARY: [&str; 2] = ["not_observable", "threshold_pending"];
/// The closed per-document measurement state vocabulary.
pub const DOCUMENT_STATE_VOCABULARY: [&str; 2] = ["measurable", "excluded"];
/// The closed exclusion-reason vocabulary (mirrors the frozen schema).
pub const EXCLUSION_REASON_VOCABULARY: [&str; 3] = [
    "header_date_absent",
    "header_title_absent",
    "header_author_absent",
];
/// The closed required-family state vocabulary (mirrors the frozen schema).
pub const FAMILY_STATE_VOCABULARY: [&str; 2] = ["required", "threshold_pending"];

/// The five locks and their required closed values.
pub const ACCEPTANCE_LOCK_KEYS: [&str; 5] = [
    "deployed",
    "import_eligible",
    "auto_promote",
    "production_route",
    "graph_write",
];
/// The required closed value for each lock, in [`ACCEPTANCE_LOCK_KEYS`] order.
pub const ACCEPTANCE_LOCK_VALUES: [&str; 5] =
    ["false", "false", "forbidden", "forbidden", "forbidden"];

// ───────────────────────── receipt types ─────────────────────────

/// One carrier binding: a repository-relative path, the pointer inside it and
/// the sha256 of the exact bytes the receipt was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCarrierBinding {
    /// Repository-relative carrier path.
    pub path: String,
    /// Pointer inside the carrier.
    pub pointer: String,
    /// Prefixed sha256 of the exact carrier bytes.
    pub sha256: String,
}

/// The frozen-reference identity the receipt is bound to. Every field is a
/// pointer to committed bytes; no frozen value is restated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceReferenceIdentity {
    /// The prospective regression floor.
    pub pins: AcceptanceCarrierBinding,
    /// The executed reference run the pins derive from.
    pub probe: AcceptanceCarrierBinding,
    /// The closed absorption-progress schema.
    pub schema: AcceptanceCarrierBinding,
    /// The frozen threshold ids, in pins order.
    pub threshold_ids: Vec<String>,
    /// Always `true`: the candidate is never its own reference.
    pub candidate_is_not_an_input: bool,
}

/// Guard provenance on the real Java transport seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceGuard {
    /// The observed seam.
    pub observer: String,
    /// An observer was installed on the real backend.
    pub installed: bool,
    /// Installing it did not rotate the identity.
    pub identity_isolated: bool,
    /// Measured Java consult attempts. Must be zero for a native run.
    pub observed_java_calls: u64,
    /// A positive control on a separate seam raised the counter.
    pub positive_control: bool,
}

/// The frozen denominator partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptancePopulations {
    /// Full assigned population.
    pub assigned: u64,
    /// Documents carrying an admitted non-zero evaluated population.
    pub measurable: u64,
    /// Documents refused by the route (zero here: refusal is a finding).
    pub refused: u64,
    /// Documents excluded by a typed reason.
    pub excluded: u64,
}

/// One assigned document's typed acceptance row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceDocumentRow {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Prefixed sha256 of the pinned PDF bytes.
    pub sha256: String,
    /// `measurable` or `excluded`.
    pub state: String,
    /// The typed exclusion reason, present exactly when `state = excluded`.
    pub exclusion_reason: Option<String>,
    /// Observed page count, when the native plane ran.
    pub pages: Option<u64>,
    /// Observed native lexical units, when the native plane ran.
    pub layout_units: Option<u64>,
    /// sha256 over the native plane observation, when it ran.
    pub layout_digest: Option<String>,
    /// `observed` (plane ran), `refused` (measurable but the plane refused),
    /// or `not_run` (excluded document). Never a silent zero.
    pub plane_state: String,
    /// The typed plane-refusal reason, present exactly when
    /// `plane_state = refused`.
    pub plane_reason: Option<String>,
}

/// One measured metric row. Its denominator must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceMetricRow {
    /// The metric id.
    pub metric_id: String,
    /// The measured numerator.
    pub numerator: u64,
    /// The measured denominator. Zero is rejected as a silent zero.
    pub denominator: u64,
    /// The frozen stratum this row resolves, when the metric id is shared by
    /// more than one threshold (the two extraction thresholds differ only by
    /// stratum). `None` while the producing mechanism does not yet emit one;
    /// `skip_serializing_if` keeps committed receipt bytes unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stratum: Option<String>,
}

/// One typed gap cell. `kind` is `not_observable` or `threshold_pending`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceGapCell {
    /// The frozen threshold id this gap is about (`""` for a non-threshold
    /// required family).
    pub threshold_id: String,
    /// The frozen metric id this gap is about.
    pub metric_id: String,
    /// The frozen family id whose mechanism is missing.
    pub family_id: String,
    /// `not_observable` or `threshold_pending`.
    pub kind: String,
    /// A machine-readable reason naming what is missing.
    pub reason: String,
}

/// The emitted corpus-bound native acceptance receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAcceptanceReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Claim scope; only `diagnostic_only` is emitted.
    pub claim_scope: String,
    /// Product-coverage effect; only `none` is emitted.
    pub product_coverage_effect: String,
    /// Thresholds; only `not_accepted` is emitted.
    pub thresholds: String,
    /// The five locks, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// The frozen-reference identity.
    pub reference: AcceptanceReferenceIdentity,
    /// Guard provenance.
    pub guard: AcceptanceGuard,
    /// The denominator partition.
    pub populations: AcceptancePopulations,
    /// One typed row per assigned document, in carrier order.
    pub documents: Vec<AcceptanceDocumentRow>,
    /// Measured (non-empty-denominator) metrics.
    pub metrics: Vec<AcceptanceMetricRow>,
    /// The typed gap ledger.
    pub gaps: Vec<AcceptanceGapCell>,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl NativeAcceptanceReceipt {
    /// Serialize canonically (pretty JSON, deterministic field order) and refuse
    /// if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, AcceptanceError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            AcceptanceError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            AcceptanceError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_acceptance_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, AcceptanceError> {
        serde_json::from_slice(bytes).map_err(|error| AcceptanceError::ArtifactSerialization {
            reason: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_acceptance_redacted(json: &str) -> Result<(), AcceptanceError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(AcceptanceError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────── expectation ─────────────────────────

/// One frozen threshold, resolved by pointer from the pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceThresholdExpectation {
    /// The frozen threshold id.
    pub threshold_id: String,
    /// The frozen metric id.
    pub metric_id: String,
    /// The frozen family id.
    pub family_id: String,
    /// The frozen stratum this threshold is compared at, read by pointer from
    /// the pins (never restated). `None` when the pin carries no stratum axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stratum: Option<String>,
}

/// One required family resolved from the closed schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceFamilyExpectation {
    /// The family id.
    pub family_id: String,
    /// `required` or `threshold_pending`.
    pub state: String,
    /// The schema's typed pending reason; present exactly when
    /// `state = threshold_pending`.
    pub reason: Option<String>,
}

/// One expected document row, re-derived from the committed carriers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceDocumentExpectation {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Prefixed sha256 of the pinned PDF bytes.
    pub sha256: String,
    /// `measurable` or `excluded`.
    pub state: String,
    /// The typed exclusion reason, present exactly when `state = excluded`.
    pub exclusion_reason: Option<String>,
}

/// The re-derived expectation the receipt must satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceExpectation {
    /// The pins carrier binding.
    pub pins: AcceptanceCarrierBinding,
    /// The probe carrier binding.
    pub probe: AcceptanceCarrierBinding,
    /// The schema carrier binding.
    pub schema: AcceptanceCarrierBinding,
    /// The frozen thresholds.
    pub thresholds: Vec<AcceptanceThresholdExpectation>,
    /// The required families (measured_by_corpus_parity and pending alike).
    pub families: Vec<AcceptanceFamilyExpectation>,
    /// The frozen denominator partition.
    pub populations: AcceptancePopulations,
    /// The expected per-document rows, in carrier order.
    pub documents: Vec<AcceptanceDocumentExpectation>,
}

// ───────────────────────── errors ─────────────────────────

/// Every refusal is a named variant so a mutation is rejected *by name*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceError {
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
    /// A payload/host marker leaked into the artifact.
    RedactionLeak { marker: String },
    /// Schema version mismatch.
    SchemaMismatch { found: String },
    /// A claim scope other than `diagnostic_only`.
    ClaimScope { found: String },
    /// A product-coverage effect other than `none`.
    ProductCoverageEffect { found: String },
    /// A thresholds marker other than `not_accepted`.
    ThresholdsMarker { found: String },
    /// An opened or missing lock.
    OpenLock { lock: String, found: String },
    /// Reference carrier drift.
    ReferenceDrift { carrier: String },
    /// Denominator identity broken (`assigned != measurable + refused +
    /// excluded`).
    DenominatorIdentity { assigned: u64, parts: u64 },
    /// Denominator does not match the frozen partition.
    DenominatorDrift { expected: u64, found: u64 },
    /// A document row count mismatch.
    DocumentCount { expected: usize, found: usize },
    /// A per-document row does not match the pinned expectation.
    DocumentMismatch { id: String, field: &'static str },
    /// An unknown document id.
    UnknownDocument { id: String },
    /// A duplicate document id.
    DuplicateDocument { id: String },
    /// An unknown per-document state token.
    UnknownDocumentState { state: String },
    /// An exclusion reason outside the closed vocabulary.
    UnknownExclusionReason { reason: String },
    /// A measurable row carries an exclusion reason, or vice versa.
    ExclusionReasonShape { id: String },
    /// A gap cell kind outside the closed vocabulary.
    UnknownGapKind { kind: String },
    /// A gap cell with an empty reason.
    EmptyGapReason { family_id: String },
    /// A gap cell for an unknown threshold/family.
    UnknownGapTarget { family_id: String },
    /// A frozen threshold is carried BOTH measured and as a gap.
    ThresholdBothMeasuredAndGapped { threshold_id: String },
    /// A frozen threshold is carried NEITHER measured nor as a gap (silent
    /// zero).
    ThresholdSilentZero { threshold_id: String },
    /// A measured metric row with a zero (silent) denominator.
    MetricZeroDenominator { metric_id: String },
    /// A required family is missing from the ledger entirely.
    RequiredFamilyMissing { family_id: String },
    /// A required family state drifted from the closed schema.
    RequiredFamilyStateDrift { family_id: String, found: String },
    /// A plane state outside the closed vocabulary.
    UnknownPlaneState { state: String },
    /// A plane row's shape contradicts its state.
    PlaneShape { id: String },
    /// A refused plane row carries an empty reason.
    EmptyPlaneReason { id: String },
    /// A guard-less zero: a zero Java-call reading without an installed,
    /// identity-isolated observer and a raised positive control.
    GuardlessZero,
    /// Java was consulted during a native run.
    JavaConsulted { observed: u64 },
    /// A threshold was measured but is not a frozen threshold (tuned/unknown).
    UnknownThreshold { threshold_id: String },
}

impl fmt::Display for AcceptanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AcceptanceError::ArtifactSerialization { reason } => {
                write!(formatter, "the artifact could not be serialized: {reason}")
            }
            AcceptanceError::RedactionLeak { marker } => {
                write!(formatter, "the artifact leaked a payload marker: {marker}")
            }
            AcceptanceError::SchemaMismatch { found } => {
                write!(formatter, "schema version `{found}` is not expected")
            }
            AcceptanceError::ClaimScope { found } => {
                write!(formatter, "claim scope `{found}` is not diagnostic_only")
            }
            AcceptanceError::ProductCoverageEffect { found } => {
                write!(formatter, "product coverage effect `{found}` is not none")
            }
            AcceptanceError::ThresholdsMarker { found } => {
                write!(formatter, "thresholds marker `{found}` is not not_accepted")
            }
            AcceptanceError::OpenLock { lock, found } => {
                write!(formatter, "lock `{lock}` is `{found}` and must be closed")
            }
            AcceptanceError::ReferenceDrift { carrier } => {
                write!(formatter, "reference carrier `{carrier}` drifted")
            }
            AcceptanceError::DenominatorIdentity { assigned, parts } => {
                write!(
                    formatter,
                    "denominator identity broken: assigned {assigned} != parts {parts}"
                )
            }
            AcceptanceError::DenominatorDrift { expected, found } => {
                write!(
                    formatter,
                    "denominator drifted: expected {expected}, found {found}"
                )
            }
            AcceptanceError::DocumentCount { expected, found } => {
                write!(
                    formatter,
                    "document count drifted: expected {expected}, found {found}"
                )
            }
            AcceptanceError::DocumentMismatch { id, field } => {
                write!(formatter, "document `{id}` drifted on `{field}`")
            }
            AcceptanceError::UnknownDocument { id } => {
                write!(formatter, "document `{id}` is not in the pinned corpus")
            }
            AcceptanceError::DuplicateDocument { id } => {
                write!(formatter, "document `{id}` appears more than once")
            }
            AcceptanceError::UnknownDocumentState { state } => {
                write!(
                    formatter,
                    "document state `{state}` is not in the vocabulary"
                )
            }
            AcceptanceError::UnknownExclusionReason { reason } => {
                write!(
                    formatter,
                    "exclusion reason `{reason}` is not a typed exclusion"
                )
            }
            AcceptanceError::ExclusionReasonShape { id } => {
                write!(
                    formatter,
                    "document `{id}` has an inconsistent exclusion shape"
                )
            }
            AcceptanceError::UnknownGapKind { kind } => {
                write!(
                    formatter,
                    "gap kind `{kind}` is not not_observable/threshold_pending"
                )
            }
            AcceptanceError::EmptyGapReason { family_id } => {
                write!(
                    formatter,
                    "gap cell for `{family_id}` carries an empty reason"
                )
            }
            AcceptanceError::UnknownGapTarget { family_id } => {
                write!(formatter, "gap cell targets unknown family `{family_id}`")
            }
            AcceptanceError::ThresholdBothMeasuredAndGapped { threshold_id } => {
                write!(
                    formatter,
                    "threshold `{threshold_id}` is both measured and gapped"
                )
            }
            AcceptanceError::ThresholdSilentZero { threshold_id } => {
                write!(
                    formatter,
                    "threshold `{threshold_id}` is measured nor gapped (silent zero)"
                )
            }
            AcceptanceError::MetricZeroDenominator { metric_id } => {
                write!(
                    formatter,
                    "metric `{metric_id}` carries a zero (silent) denominator"
                )
            }
            AcceptanceError::RequiredFamilyMissing { family_id } => {
                write!(formatter, "required family `{family_id}` is missing")
            }
            AcceptanceError::RequiredFamilyStateDrift { family_id, found } => {
                write!(
                    formatter,
                    "required family `{family_id}` state `{found}` drifted"
                )
            }
            AcceptanceError::UnknownPlaneState { state } => {
                write!(formatter, "plane state `{state}` is not in the vocabulary")
            }
            AcceptanceError::PlaneShape { id } => {
                write!(formatter, "document `{id}` has an inconsistent plane shape")
            }
            AcceptanceError::EmptyPlaneReason { id } => {
                write!(
                    formatter,
                    "refused plane row for `{id}` carries an empty reason"
                )
            }
            AcceptanceError::GuardlessZero => {
                write!(
                    formatter,
                    "a zero Java-call reading without an installed, isolated guard and a raised positive control"
                )
            }
            AcceptanceError::JavaConsulted { observed } => {
                write!(formatter, "the native run consulted Java {observed} times")
            }
            AcceptanceError::UnknownThreshold { threshold_id } => {
                write!(
                    formatter,
                    "threshold `{threshold_id}` is not a frozen threshold"
                )
            }
        }
    }
}

impl std::error::Error for AcceptanceError {}

// ───────────────────────── builder ─────────────────────────

/// Build a canonical receipt, validate it against the expectation and return
/// the canonical bytes. Refuses (and writes nothing) on any named violation.
pub fn build_native_acceptance(
    expectation: &AcceptanceExpectation,
    guard: AcceptanceGuard,
    documents: Vec<AcceptanceDocumentRow>,
    metrics: Vec<AcceptanceMetricRow>,
    gaps: Vec<AcceptanceGapCell>,
) -> Result<NativeAcceptanceReceipt, AcceptanceError> {
    let locks: BTreeMap<String, String> = ACCEPTANCE_LOCK_KEYS
        .iter()
        .zip(ACCEPTANCE_LOCK_VALUES.iter())
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let receipt = NativeAcceptanceReceipt {
        schema_version: NATIVE_ACCEPTANCE_SCHEMA_VERSION.to_owned(),
        claim_scope: ACCEPTANCE_CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: ACCEPTANCE_PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: ACCEPTANCE_THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks,
        reference: AcceptanceReferenceIdentity {
            pins: expectation.pins.clone(),
            probe: expectation.probe.clone(),
            schema: expectation.schema.clone(),
            threshold_ids: expectation
                .thresholds
                .iter()
                .map(|threshold| threshold.threshold_id.clone())
                .collect(),
            candidate_is_not_an_input: true,
        },
        guard,
        populations: expectation.populations.clone(),
        documents,
        metrics,
        gaps,
        redaction: Redaction::clean(),
    };
    validate_native_acceptance(&receipt, expectation)?;
    Ok(receipt)
}

/// Validate a receipt against the re-derived expectation. Every failure is a
/// named variant.
pub fn validate_native_acceptance(
    receipt: &NativeAcceptanceReceipt,
    expectation: &AcceptanceExpectation,
) -> Result<(), AcceptanceError> {
    // Envelope.
    if receipt.schema_version != NATIVE_ACCEPTANCE_SCHEMA_VERSION {
        return Err(AcceptanceError::SchemaMismatch {
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != ACCEPTANCE_CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(AcceptanceError::ClaimScope {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != ACCEPTANCE_PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(AcceptanceError::ProductCoverageEffect {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != ACCEPTANCE_THRESHOLDS_NOT_ACCEPTED {
        return Err(AcceptanceError::ThresholdsMarker {
            found: receipt.thresholds.clone(),
        });
    }

    // The five closed locks.
    for (key, expected) in ACCEPTANCE_LOCK_KEYS
        .iter()
        .zip(ACCEPTANCE_LOCK_VALUES.iter())
    {
        match receipt.locks.get(*key) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(AcceptanceError::OpenLock {
                    lock: (*key).to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(AcceptanceError::OpenLock {
                    lock: (*key).to_owned(),
                    found: "<missing>".to_owned(),
                });
            }
        }
    }

    // Reference identity: the receipt must bind the exact committed bytes.
    if receipt.reference.pins.sha256 != expectation.pins.sha256
        || receipt.reference.pins.path != expectation.pins.path
    {
        return Err(AcceptanceError::ReferenceDrift {
            carrier: "pins".to_owned(),
        });
    }
    if receipt.reference.probe.sha256 != expectation.probe.sha256
        || receipt.reference.probe.path != expectation.probe.path
    {
        return Err(AcceptanceError::ReferenceDrift {
            carrier: "probe".to_owned(),
        });
    }
    if receipt.reference.schema.sha256 != expectation.schema.sha256
        || receipt.reference.schema.path != expectation.schema.path
    {
        return Err(AcceptanceError::ReferenceDrift {
            carrier: "schema".to_owned(),
        });
    }
    if !receipt.reference.candidate_is_not_an_input {
        return Err(AcceptanceError::ReferenceDrift {
            carrier: "candidate_is_not_an_input".to_owned(),
        });
    }

    // Denominator identity + frozen partition.
    let parts = receipt
        .populations
        .measurable
        .saturating_add(receipt.populations.refused)
        .saturating_add(receipt.populations.excluded);
    if receipt.populations.assigned != parts {
        return Err(AcceptanceError::DenominatorIdentity {
            assigned: receipt.populations.assigned,
            parts,
        });
    }
    if receipt.populations.assigned != expectation.populations.assigned
        || receipt.populations.measurable != expectation.populations.measurable
        || receipt.populations.refused != expectation.populations.refused
        || receipt.populations.excluded != expectation.populations.excluded
    {
        return Err(AcceptanceError::DenominatorDrift {
            expected: expectation.populations.assigned,
            found: receipt.populations.assigned,
        });
    }

    // Per-document rows, in carrier order.
    if receipt.documents.len() != expectation.documents.len() {
        return Err(AcceptanceError::DocumentCount {
            expected: expectation.documents.len(),
            found: receipt.documents.len(),
        });
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut measurable_rows = 0u64;
    let mut excluded_rows = 0u64;
    for (row, expected) in receipt.documents.iter().zip(expectation.documents.iter()) {
        if !seen.insert(row.id.as_str()) {
            return Err(AcceptanceError::DuplicateDocument { id: row.id.clone() });
        }
        if row.id != expected.id {
            return Err(AcceptanceError::DocumentMismatch {
                id: expected.id.clone(),
                field: "id",
            });
        }
        if row.source_pdf != expected.source_pdf {
            return Err(AcceptanceError::DocumentMismatch {
                id: row.id.clone(),
                field: "source_pdf",
            });
        }
        if row.sha256 != expected.sha256 {
            return Err(AcceptanceError::DocumentMismatch {
                id: row.id.clone(),
                field: "sha256",
            });
        }
        match row.state.as_str() {
            "measurable" => measurable_rows += 1,
            "excluded" => excluded_rows += 1,
            other => {
                return Err(AcceptanceError::UnknownDocumentState {
                    state: other.to_owned(),
                });
            }
        }
        match (&row.exclusion_reason, &expected.exclusion_reason) {
            (None, None) => {}
            (Some(found), Some(want)) if found == want => {
                if !EXCLUSION_REASON_VOCABULARY.contains(&found.as_str()) {
                    return Err(AcceptanceError::UnknownExclusionReason {
                        reason: found.clone(),
                    });
                }
            }
            _ => return Err(AcceptanceError::ExclusionReasonShape { id: row.id.clone() }),
        }
        if row.state != expected.state {
            return Err(AcceptanceError::DocumentMismatch {
                id: row.id.clone(),
                field: "state",
            });
        }
        // Plane state is typed and never a silent zero.
        match row.plane_state.as_str() {
            "observed" => {
                let observed_shape = row.pages.is_some()
                    && row.layout_units.is_some()
                    && row.layout_digest.is_some();
                if !observed_shape || row.plane_reason.is_some() {
                    return Err(AcceptanceError::PlaneShape { id: row.id.clone() });
                }
            }
            "refused" => {
                let refused_shape = row.pages.is_none()
                    && row.layout_units.is_none()
                    && row.layout_digest.is_none();
                let reason_ok = row
                    .plane_reason
                    .as_ref()
                    .map(|reason| !reason.trim().is_empty())
                    .unwrap_or(false);
                if !refused_shape {
                    return Err(AcceptanceError::PlaneShape { id: row.id.clone() });
                }
                if !reason_ok {
                    return Err(AcceptanceError::EmptyPlaneReason { id: row.id.clone() });
                }
            }
            "not_run" => {
                let not_run_shape = row.pages.is_none()
                    && row.layout_units.is_none()
                    && row.layout_digest.is_none();
                if !not_run_shape || row.plane_reason.is_some() {
                    return Err(AcceptanceError::PlaneShape { id: row.id.clone() });
                }
                if row.state != "excluded" {
                    return Err(AcceptanceError::PlaneShape { id: row.id.clone() });
                }
            }
            other => {
                return Err(AcceptanceError::UnknownPlaneState {
                    state: other.to_owned(),
                });
            }
        }
    }
    if measurable_rows != receipt.populations.measurable
        || excluded_rows != receipt.populations.excluded
    {
        return Err(AcceptanceError::DenominatorDrift {
            expected: receipt.populations.measurable,
            found: measurable_rows,
        });
    }

    // Measured metrics: a zero denominator is a silent zero, never a value.
    let mut measured_metric_ids: BTreeSet<&str> = BTreeSet::new();
    for metric in &receipt.metrics {
        if metric.denominator == 0 {
            return Err(AcceptanceError::MetricZeroDenominator {
                metric_id: metric.metric_id.clone(),
            });
        }
        measured_metric_ids.insert(metric.metric_id.as_str());
    }

    // Gap ledger.
    let threshold_ids: BTreeSet<&str> = expectation
        .thresholds
        .iter()
        .map(|threshold| threshold.threshold_id.as_str())
        .collect();
    let family_ids: BTreeSet<&str> = expectation
        .families
        .iter()
        .map(|family| family.family_id.as_str())
        .collect();
    let mut gapped_thresholds: BTreeSet<&str> = BTreeSet::new();
    let mut gapped_families: BTreeSet<&str> = BTreeSet::new();
    for gap in &receipt.gaps {
        if !GAP_KIND_VOCABULARY.contains(&gap.kind.as_str()) {
            return Err(AcceptanceError::UnknownGapKind {
                kind: gap.kind.clone(),
            });
        }
        if gap.reason.trim().is_empty() {
            return Err(AcceptanceError::EmptyGapReason {
                family_id: gap.family_id.clone(),
            });
        }
        if !family_ids.contains(gap.family_id.as_str()) {
            return Err(AcceptanceError::UnknownGapTarget {
                family_id: gap.family_id.clone(),
            });
        }
        gapped_families.insert(gap.family_id.as_str());
        if !gap.threshold_id.is_empty() {
            if !threshold_ids.contains(gap.threshold_id.as_str()) {
                return Err(AcceptanceError::UnknownGapTarget {
                    family_id: gap.family_id.clone(),
                });
            }
            gapped_thresholds.insert(gap.threshold_id.as_str());
        }
    }

    // Every frozen threshold is either measured with a non-empty denominator or
    // explicitly carried in the ledger with a reason. Never both, never neither.
    for threshold in &expectation.thresholds {
        let measured = measured_metric_ids.contains(threshold.metric_id.as_str());
        let gapped = gapped_thresholds.contains(threshold.threshold_id.as_str());
        if measured && gapped {
            return Err(AcceptanceError::ThresholdBothMeasuredAndGapped {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if !measured && !gapped {
            return Err(AcceptanceError::ThresholdSilentZero {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if measured && !gapped_families.contains(threshold.family_id.as_str()) {
            // A measured threshold must still name its family in the ledger only
            // when the family is required; here the measurement itself is the
            // evidence, so absence from the gap ledger is legal.
        }
    }

    // No measured metric may claim an unknown threshold id via its name; and no
    // measured metric may silently vanish: every metric id must be either a
    // frozen threshold's metric or an explicitly named observable population
    // metric (denominator non-empty above).
    for metric_id in &measured_metric_ids {
        let frozen = expectation
            .thresholds
            .iter()
            .any(|threshold| threshold.metric_id == *metric_id);
        let observable = metric_id.starts_with("native_acceptance.");
        if !frozen && !observable {
            return Err(AcceptanceError::UnknownThreshold {
                threshold_id: (*metric_id).to_owned(),
            });
        }
    }

    // Every required family must be present in the ledger (measured or gapped)
    // with its frozen state. A required family silently dropped is a shrink.
    for family in &expectation.families {
        if !FAMILY_STATE_VOCABULARY.contains(&family.state.as_str()) {
            return Err(AcceptanceError::RequiredFamilyStateDrift {
                family_id: family.family_id.clone(),
                found: family.state.clone(),
            });
        }
        let gapped = gapped_families.contains(family.family_id.as_str());
        let measured_family = expectation.thresholds.iter().any(|threshold| {
            threshold.family_id == family.family_id
                && measured_metric_ids.contains(threshold.metric_id.as_str())
        });
        if !gapped && !measured_family {
            return Err(AcceptanceError::RequiredFamilyMissing {
                family_id: family.family_id.clone(),
            });
        }
    }

    // Guard: a zero reading is a measurement only under an installed,
    // identity-isolated observer with a raised positive control.
    if receipt.guard.observed_java_calls > 0 {
        return Err(AcceptanceError::JavaConsulted {
            observed: receipt.guard.observed_java_calls,
        });
    }
    if !receipt.guard.installed
        || !receipt.guard.identity_isolated
        || !receipt.guard.positive_control
    {
        return Err(AcceptanceError::GuardlessZero);
    }

    Ok(())
}

// ───────────────────────── carrier loaders ─────────────────────────
//
// These loaders are the single source of truth for the runner and the contract
// test: both re-derive the same expectation from the same committed bytes, so a
// restated value in either has nowhere to enter.

/// One document as the holdout manifest publishes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldoutDocument {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub pdf_path: String,
    /// Prefixed sha256 of the pinned PDF bytes.
    pub sha256: String,
    /// Manifest status.
    pub status: String,
}

/// One typed probe exclusion: a document plus why it is not measurable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeExclusion {
    /// The excluded document id.
    pub document_id: String,
    /// The typed exclusion reason.
    pub reason: String,
}

/// Load the 610 assigned documents from the holdout manifest (pointer
/// `articles`). No id, count or sha is restated.
pub fn load_holdout_documents(text: &str) -> Result<Vec<HoldoutDocument>, AcceptanceError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|error| AcceptanceError::ArtifactSerialization {
            reason: error.to_string(),
        })?;
    let articles = value
        .get(HOLDOUT_MANIFEST_POINTER)
        .and_then(|node| node.as_sequence())
        .ok_or_else(|| AcceptanceError::ArtifactSerialization {
            reason: format!("holdout manifest has no `{HOLDOUT_MANIFEST_POINTER}` sequence"),
        })?;
    let mut documents = Vec::with_capacity(articles.len());
    for article in articles {
        let id = article
            .get("article_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: "holdout article has no article_id".to_owned(),
            })?
            .to_owned();
        let pdf_path = article
            .get("pdf_path")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("holdout article {id} has no pdf_path"),
            })?
            .to_owned();
        let sha256 = article
            .get("sha256")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("holdout article {id} has no sha256"),
            })?
            .to_owned();
        let status = article
            .get("status")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_owned();
        documents.push(HoldoutDocument {
            id,
            pdf_path,
            sha256,
            status,
        });
    }
    Ok(documents)
}

/// Load the three frozen thresholds from the pins (pointer `values`). Only the
/// ids are read; no numeric threshold is restated.
pub fn load_reference_thresholds(
    text: &str,
) -> Result<Vec<AcceptanceThresholdExpectation>, AcceptanceError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| AcceptanceError::ArtifactSerialization {
            reason: error.to_string(),
        })?;
    let values = value
        .get("values")
        .and_then(|node| node.as_array())
        .ok_or_else(|| AcceptanceError::ArtifactSerialization {
            reason: "reference pins have no `values` array".to_owned(),
        })?;
    let mut thresholds = Vec::with_capacity(values.len());
    for entry in values {
        let threshold_id = entry
            .get("threshold_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: "reference pin has no threshold_id".to_owned(),
            })?
            .to_owned();
        let metric_id = entry
            .get("metric_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("reference pin {threshold_id} has no metric_id"),
            })?
            .to_owned();
        let family_id = entry
            .get("family_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("reference pin {threshold_id} has no family_id"),
            })?
            .to_owned();
        let stratum = entry
            .get("stratum")
            .and_then(|node| node.as_str())
            .map(|value| value.to_owned());
        thresholds.push(AcceptanceThresholdExpectation {
            threshold_id,
            metric_id,
            family_id,
            stratum,
        });
    }
    Ok(thresholds)
}

/// Load the frozen denominator partition and the typed exclusions from the
/// reference probe (pointer `reference_population`).
pub fn load_probe_partition(
    text: &str,
) -> Result<(AcceptancePopulations, Vec<ProbeExclusion>), AcceptanceError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| AcceptanceError::ArtifactSerialization {
            reason: error.to_string(),
        })?;
    let population = value.get("reference_population").ok_or_else(|| {
        AcceptanceError::ArtifactSerialization {
            reason: "reference probe has no reference_population".to_owned(),
        }
    })?;
    let number = |key: &str| -> Result<u64, AcceptanceError> {
        population
            .get(key)
            .and_then(|node| node.as_u64())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("reference_population has no numeric {key}"),
            })
    };
    let populations = AcceptancePopulations {
        assigned: number("documents")?,
        measurable: number("measurable_documents")?,
        refused: 0,
        excluded: number("excluded_documents")?,
    };
    let mut exclusions = Vec::new();
    if let Some(exceptions) = population
        .get("exceptions")
        .and_then(|node| node.as_array())
    {
        for entry in exceptions {
            let document_id = entry
                .get("document_id")
                .and_then(|node| node.as_str())
                .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                    reason: "probe exception has no document_id".to_owned(),
                })?
                .to_owned();
            let reason = entry
                .get("reason")
                .and_then(|node| node.as_str())
                .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                    reason: format!("probe exception {document_id} has no reason"),
                })?
                .to_owned();
            exclusions.push(ProbeExclusion {
                document_id,
                reason,
            });
        }
    }
    Ok((populations, exclusions))
}

/// Load the required family scope from the closed schema (pointer
/// `required_scope.families`).
pub fn load_required_families(
    text: &str,
) -> Result<Vec<AcceptanceFamilyExpectation>, AcceptanceError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|error| AcceptanceError::ArtifactSerialization {
            reason: error.to_string(),
        })?;
    let families = value
        .get("required_scope")
        .and_then(|scope| scope.get("families"))
        .and_then(|node| node.as_sequence())
        .ok_or_else(|| AcceptanceError::ArtifactSerialization {
            reason: "absorption schema has no required_scope.families".to_owned(),
        })?;
    let mut required = Vec::with_capacity(families.len());
    for family in families {
        let family_id = family
            .get("family_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: "required family has no family_id".to_owned(),
            })?
            .to_owned();
        let state = family
            .get("state")
            .and_then(|node| node.as_str())
            .ok_or_else(|| AcceptanceError::ArtifactSerialization {
                reason: format!("required family {family_id} has no state"),
            })?
            .to_owned();
        let reason = family
            .get("reason")
            .and_then(|node| node.as_str())
            .map(|value| value.to_owned());
        if state == "threshold_pending" && reason.is_none() {
            return Err(AcceptanceError::ArtifactSerialization {
                reason: format!("pending family {family_id} carries no reason"),
            });
        }
        required.push(AcceptanceFamilyExpectation {
            family_id,
            state,
            reason,
        });
    }
    Ok(required)
}

/// Re-derive the full expectation from the committed carrier bytes. This is the
/// only place the partition is assembled; the runner and the contract test share
/// it so a restated value cannot enter either side.
pub fn derive_expectation(
    pins_text: &str,
    pins_sha256: &str,
    probe_text: &str,
    probe_sha256: &str,
    schema_text: &str,
    schema_sha256: &str,
    manifest_text: &str,
) -> Result<AcceptanceExpectation, AcceptanceError> {
    let thresholds = load_reference_thresholds(pins_text)?;
    let (populations, exclusions) = load_probe_partition(probe_text)?;
    let families = load_required_families(schema_text)?;
    let declared = load_holdout_documents(manifest_text)?;

    let exclusions_by_id: BTreeMap<&str, &str> = exclusions
        .iter()
        .map(|exclusion| (exclusion.document_id.as_str(), exclusion.reason.as_str()))
        .collect();

    let mut documents = Vec::with_capacity(declared.len());
    for document in &declared {
        let reason = exclusions_by_id.get(document.id.as_str()).copied();
        let state = if reason.is_some() {
            "excluded"
        } else {
            "measurable"
        };
        documents.push(AcceptanceDocumentExpectation {
            id: document.id.clone(),
            source_pdf: document.pdf_path.clone(),
            sha256: document.sha256.clone(),
            state: state.to_owned(),
            exclusion_reason: reason.map(|value| value.to_owned()),
        });
    }

    Ok(AcceptanceExpectation {
        pins: AcceptanceCarrierBinding {
            path: REFERENCE_PINS_REL.to_owned(),
            pointer: "values".to_owned(),
            sha256: pins_sha256.to_owned(),
        },
        probe: AcceptanceCarrierBinding {
            path: REFERENCE_PROBE_REL.to_owned(),
            pointer: "reference_population".to_owned(),
            sha256: probe_sha256.to_owned(),
        },
        schema: AcceptanceCarrierBinding {
            path: ABSORPTION_SCHEMA_REL.to_owned(),
            pointer: "required_scope.families".to_owned(),
            sha256: schema_sha256.to_owned(),
        },
        thresholds,
        families,
        populations,
        documents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(values: serde_json::Value) -> String {
        serde_json::json!({ "values": values }).to_string()
    }

    #[test]
    fn load_reference_thresholds_reads_stratum_by_pointer_and_tolerates_absence() {
        let text = pins(serde_json::json!([
            {
                "threshold_id": "extraction_micro_occurrence",
                "metric_id": "extraction.native_capability_recall",
                "family_id": "stage.reference_segmentation",
                "stratum": "micro_occurrence"
            },
            {
                "threshold_id": "extraction_weakest_family",
                "metric_id": "extraction.native_capability_recall",
                "family_id": "stage.header_composition",
                "stratum": "weakest_stratum"
            },
            {
                "threshold_id": "preservation_micro_occurrence",
                "metric_id": "preservation.occurrence_conservation",
                "family_id": "parse.component.native_fulltext_composition_shadow"
            }
        ]));
        let thresholds = load_reference_thresholds(&text).expect("the pins must load");
        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds[0].stratum.as_deref(), Some("micro_occurrence"));
        assert_eq!(thresholds[1].stratum.as_deref(), Some("weakest_stratum"));
        assert!(
            thresholds[2].stratum.is_none(),
            "a pin with no stratum axis must resolve None, never a fabricated token"
        );
        // The two extraction thresholds share one metric id and differ only by
        // stratum; collapsing them would lose which one a row resolves.
        assert_eq!(thresholds[0].metric_id, thresholds[1].metric_id);
        assert_ne!(thresholds[0].stratum, thresholds[1].stratum);
    }

    #[test]
    fn malformed_pins_are_refused_by_name() {
        let error =
            load_reference_thresholds("{}").expect_err("a pins file without values is refused");
        assert!(matches!(
            error,
            AcceptanceError::ArtifactSerialization { .. }
        ));
        let text = pins(serde_json::json!([
            { "threshold_id": "t", "family_id": "f" }
        ]));
        let error =
            load_reference_thresholds(&text).expect_err("an entry without metric_id is refused");
        assert!(matches!(
            error,
            AcceptanceError::ArtifactSerialization { .. }
        ));
    }

    #[test]
    fn metric_row_stratum_is_optional_and_omitted_when_absent() {
        let absent = AcceptanceMetricRow {
            metric_id: "m".to_owned(),
            numerator: 1,
            denominator: 2,
            stratum: None,
        };
        let serialized = serde_json::to_string(&absent).expect("the row must serialize");
        assert!(
            !serialized.contains("stratum"),
            "an absent stratum must not enter the committed bytes: {serialized}"
        );
        let parsed: AcceptanceMetricRow =
            serde_json::from_str(r#"{"metric_id":"m","numerator":1,"denominator":2}"#)
                .expect("a legacy row without stratum must still parse");
        assert!(parsed.stratum.is_none());

        let present = AcceptanceMetricRow {
            metric_id: "m".to_owned(),
            numerator: 1,
            denominator: 2,
            stratum: Some("micro_occurrence".to_owned()),
        };
        let serialized = serde_json::to_string(&present).expect("the row must serialize");
        assert!(serialized.contains(r#""stratum":"micro_occurrence""#));
    }

    #[test]
    fn threshold_expectation_stratum_is_optional() {
        let parsed: AcceptanceThresholdExpectation =
            serde_json::from_str(r#"{"threshold_id":"t","metric_id":"m","family_id":"f"}"#)
                .expect("a legacy expectation without stratum must still parse");
        assert!(parsed.stratum.is_none());
        let serialized = serde_json::to_string(&parsed).expect("the expectation must serialize");
        assert!(!serialized.contains("stratum"));
    }
}
