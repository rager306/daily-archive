//! S06 (M511-j7wz2f) — DATA-bound batch budget ceilings and the typed absence
//! carrier for batch budget observations.
//!
//! The only batch budget today (`ParseBatchBudget` / `ParseBatchLimits` in
//! [`crate::batch`]) counts SLOTS — workers, queue capacity, in-flight and
//! document counts — and pins `budget_scope == "process-local"`. It declares no
//! byte, RSS, Java-CPU or latency ceiling, and `H05-N2` forbids presenting a
//! document-count limit as a byte or RSS ceiling. So the ceiling dictionary, the
//! mandatory metric set and the prohibitions live as DATA in
//! `data/parse/comparison_contract.yaml#batch_budget_ceilings` and are bound to
//! the LIVE constants below by
//! `crates/da-parse/tests/batch_budget_data_contract.rs`.
//!
//! [`BatchObservation`] deliberately mirrors
//! `da_cli::native_core_smp::SmpObservation`: `da-parse` must not depend on
//! `da-cli` (layer direction), so the typed absence carrier is re-declared here
//! rather than imported. It is the same semantics, not a second vocabulary — an
//! unmeasured value is never serialized as a fabricated zero.
//!
//! This module designs the ceilings; it measures none of them and it changes no
//! batch behaviour. H05 stays unclaimed (D430) and every lock holds:
//! `import_eligible=false`, `deployed=false`, `production_route=forbidden`,
//! `graph_write=forbidden`, `auto_promote=forbidden`, D127 not lifted.

use serde::{Deserialize, Serialize};

/// Schema version of the `batch_budget_ceilings` DATA block.
pub const BATCH_BUDGET_CEILINGS_SCHEMA_VERSION: &str = "batch-budget-ceilings.v1";

/// ADR that authorises the ceilings block.
pub const BATCH_BUDGET_CEILINGS_ADR_REF: &str = "ADR-076";

/// Requirements this DATA block serves: bounded concurrency/backpressure
/// (R085), bounded derivatives with budget/size evidence (R087) and bounded
/// resources (R139).
pub const BATCH_BUDGET_CEILINGS_REQUIREMENT_IDS: [&str; 3] = ["R085", "R087", "R139"];

/// Closed observation-state vocabulary: a ceiling is either `observed` with a
/// measured value or `not_observed` with a named reason.
pub const BATCH_OBSERVATION_STATES: [&str; 2] = ["observed", "not_observed"];

/// An absent observation must name a reason — never a number (H19-N1).
pub const BATCH_BUDGET_ABSENCE_REQUIRES_REASON: bool = true;

/// Mandatory S06 metric set. Every ceiling observation carries exactly one of
/// these names.
pub const BATCH_BUDGET_METRICS: [&str; 10] = [
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
];

/// Closed verdict vocabulary for a measured ceiling. `memory_pressure` is the
/// honest verdict when an observed peak sits above the declared retention
/// ceiling; `not_observed` is the verdict when no measurement exists.
pub const BATCH_BUDGET_VERDICTS: [&str; 3] = ["within_ceiling", "memory_pressure", "not_observed"];

/// A zero is not a measurement: an unmeasured ceiling is `not_observed`, never
/// a fabricated `0`.
pub const BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT: bool = true;

/// Silently clamping an over-ceiling outcome is forbidden: it must fail closed
/// through the existing typed limit refusal, never be quietly truncated.
pub const BATCH_BUDGET_SILENT_CLAMP: &str = "forbidden";

/// A document-count limit is not a byte ceiling (H05-N2). The slot limits are
/// bounded, but retained serialized bytes must be measured separately.
pub const BATCH_BUDGET_COUNT_LIMIT_IS_NOT_A_BYTE_CEILING: bool = true;

/// The retention ceiling composes with (never replaces) the transport bound:
/// the two bounds are additive, not interchangeable.
pub const BATCH_BUDGET_RETENTION_CEILING_COMPOSES_WITH: &str = "response_bytes_transport_bound";

/// H05 is NOT closed by this design block (D430). H05 stays unclaimed until
/// RSS, Java CPU and latency tails are measured under concurrent batches with a
/// slow ordinal-0 document.
pub const BATCH_BUDGET_H05_CLOSURE: &str = "not_claimed";

/// Why H05 stays unclaimed. Mirrors
/// `data/parse/comparison_contract.yaml#batch_budget_ceilings.h05_reason`
/// (compared after whitespace normalisation by the DATA contract test).
pub const BATCH_BUDGET_H05_REASON: &str = concat!(
    "H05 stays explicitly unclaimed. Slot limits (workers, queue capacity, ",
    "in-flight and document counts) are bounded today, but no byte, RSS, ",
    "Java-CPU or latency-tail ceiling is measured or declared by S06/T01: this ",
    "block only freezes the ceiling dictionary, the mandatory metric set and the ",
    "prohibitions as DATA. H05 stays unclaimed until RSS, Java CPU and latency ",
    "tails are MEASURED under concurrent batches with a slow ordinal-0 document ",
    "(doc/review/grobid-rust-horizon-review.json#H05). An unmeasured value stays ",
    "a typed not_observed with a named reason, never a fabricated number.",
);

/// A measurement or its typed absence.
///
/// A metric that was not measured is never serialized as a fabricated zero:
/// `not_observed` carries a named reason and no numeric field at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BatchObservation<T> {
    /// A measured value.
    Observed { value: T },
    /// A typed absence: the metric was not measured, and this is why.
    NotObserved { reason: String },
}

impl<T> BatchObservation<T> {
    /// A measured observation.
    pub fn observed(value: T) -> Self {
        Self::Observed { value }
    }

    /// An unmeasured observation with its named reason.
    pub fn not_observed(reason: impl Into<String>) -> Self {
        Self::NotObserved {
            reason: reason.into(),
        }
    }

    /// Whether this observation carries a measured value.
    pub fn is_observed(&self) -> bool {
        matches!(self, Self::Observed { .. })
    }

    /// The measured value, if any.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Observed { value } => Some(value),
            Self::NotObserved { .. } => None,
        }
    }

    /// The absence reason, if this observation is unmeasured.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Observed { .. } => None,
            Self::NotObserved { reason } => Some(reason),
        }
    }
}
