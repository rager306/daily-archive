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
//! S06/T01 designed the ceilings and measured none of them. S06/T03 keeps the
//! DATA block frozen and adds the runtime CARRIER
//! ([`ParseBatchBudgetObservation`]): the executor measures the retained-byte
//! window peak and the per-document latency tails and publishes them through
//! the additive `ParseBatchExecutor::run_observed` seam. `ParseBatchRunSummary`
//! keeps its v2 wire unchanged, so a caller that does not ask for observations
//! sees byte-identical output. H05 stays unclaimed (D430): RSS, Java CPU and the
//! slow-ordinal-0 concurrency scenario are still unmeasured, so
//! `h05_closure == not_claimed` is carried verbatim. Every lock holds:
//! `import_eligible=false`, `deployed=false`, `production_route=forbidden`,
//! `graph_write=forbidden`, `auto_promote=forbidden`, D127 not lifted.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::transport::{FailureCode, FailurePhase, ParseBackendFailure};

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

/// Reason carried by `retention_ceiling` when the admitted batch declared no
/// byte ceiling. A document-count limit is NOT a byte ceiling (H05-N2), so an
/// undeclared ceiling is a typed absence — never a fabricated `0`.
pub const BATCH_BUDGET_NO_RETENTION_CEILING_REASON: &str =
    "no retention ceiling declared; document-count limits do not bound bytes";

/// Reason carried by every latency tail when no document reached a terminal
/// outcome in this run. Nothing was measured, so the tail is absent, not `0`.
pub const BATCH_BUDGET_NO_LATENCY_OBSERVATION_REASON: &str =
    "no document reached a terminal outcome in this run";

/// Closed verdict vocabulary of a measured retention ceiling. The wire names
/// are exactly [`BATCH_BUDGET_VERDICTS`]; the Rust enum is the typed form of
/// that DATA vocabulary, not a second one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchBudgetVerdict {
    /// An observed peak sits at or below the declared retention ceiling.
    WithinCeiling,
    /// An observed peak sits above the declared retention ceiling.
    MemoryPressure,
    /// No retention ceiling was declared, so no verdict can be claimed.
    NotObserved,
}

impl BatchBudgetVerdict {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WithinCeiling => "within_ceiling",
            Self::MemoryPressure => "memory_pressure",
            Self::NotObserved => "not_observed",
        }
    }

    /// Every verdict, in DATA declaration order.
    pub const ALL: [Self; 3] = [Self::WithinCeiling, Self::MemoryPressure, Self::NotObserved];
}

impl fmt::Display for BatchBudgetVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Runtime budget observation of one bounded batch execution (S06/T03).
///
/// Every ceiling is a [`BatchObservation`]: a measured value or a typed
/// absence with a named reason. The carrier is additive — it is returned only
/// by `ParseBatchExecutor::run_observed`, so the frozen
/// `ParseBatchRunSummary` v2 wire and `run`/`run_managed` stay byte-identical.
///
/// `peak_retained_bytes` is a genuine WINDOW peak: bytes are added when a
/// successful outcome is retained and subtracted when its durable sink record
/// completes. A cumulative sink sum is not a peak (T02 probe C), so the two
/// cannot be confused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseBatchBudgetObservation {
    peak_retained_bytes: BatchObservation<u64>,
    latency_p50_ms: BatchObservation<u64>,
    latency_p95_ms: BatchObservation<u64>,
    latency_p99_ms: BatchObservation<u64>,
    retention_ceiling: BatchObservation<u64>,
    max_reorder_depth: BatchObservation<u64>,
    max_publication_outstanding: BatchObservation<u64>,
    max_in_flight: BatchObservation<u64>,
    budget_waits: BatchObservation<u64>,
    verdict: BatchBudgetVerdict,
    h05_closure: String,
    h05_reason: String,
}

impl ParseBatchBudgetObservation {
    /// Compose one observation. The verdict is DERIVED from the declared
    /// ceiling and the observed peak, so the two can never disagree.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        peak_retained_bytes: BatchObservation<u64>,
        latency_p50_ms: BatchObservation<u64>,
        latency_p95_ms: BatchObservation<u64>,
        latency_p99_ms: BatchObservation<u64>,
        retention_ceiling: BatchObservation<u64>,
        max_reorder_depth: BatchObservation<u64>,
        max_publication_outstanding: BatchObservation<u64>,
        max_in_flight: BatchObservation<u64>,
        budget_waits: BatchObservation<u64>,
    ) -> Self {
        let verdict = match (retention_ceiling.value(), peak_retained_bytes.value()) {
            (Some(ceiling), Some(peak)) if peak <= ceiling => BatchBudgetVerdict::WithinCeiling,
            (Some(_), Some(_)) => BatchBudgetVerdict::MemoryPressure,
            _ => BatchBudgetVerdict::NotObserved,
        };
        Self {
            peak_retained_bytes,
            latency_p50_ms,
            latency_p95_ms,
            latency_p99_ms,
            retention_ceiling,
            max_reorder_depth,
            max_publication_outstanding,
            max_in_flight,
            budget_waits,
            verdict,
            h05_closure: BATCH_BUDGET_H05_CLOSURE.to_string(),
            h05_reason: BATCH_BUDGET_H05_REASON.to_string(),
        }
    }

    /// Window peak of serialized bytes retained between materialization and
    /// the durable sink record.
    pub const fn peak_retained_bytes(&self) -> &BatchObservation<u64> {
        &self.peak_retained_bytes
    }

    /// Median per-document latency (document start → terminal outcome).
    pub const fn latency_p50_ms(&self) -> &BatchObservation<u64> {
        &self.latency_p50_ms
    }

    /// p95 per-document latency tail.
    pub const fn latency_p95_ms(&self) -> &BatchObservation<u64> {
        &self.latency_p95_ms
    }

    /// p99 per-document latency tail.
    pub const fn latency_p99_ms(&self) -> &BatchObservation<u64> {
        &self.latency_p99_ms
    }

    /// The retention ceiling the admitted batch declared, or its typed
    /// absence ([`BATCH_BUDGET_NO_RETENTION_CEILING_REASON`]).
    pub const fn retention_ceiling(&self) -> &BatchObservation<u64> {
        &self.retention_ceiling
    }

    /// Peak out-of-order results waiting behind the frontier.
    pub const fn max_reorder_depth(&self) -> &BatchObservation<u64> {
        &self.max_reorder_depth
    }

    /// Peak admitted results retained before ordered publication.
    pub const fn max_publication_outstanding(&self) -> &BatchObservation<u64> {
        &self.max_publication_outstanding
    }

    /// Peak simultaneously executing documents.
    pub const fn max_in_flight(&self) -> &BatchObservation<u64> {
        &self.max_in_flight
    }

    /// Semaphore acquisitions that had to wait.
    pub const fn budget_waits(&self) -> &BatchObservation<u64> {
        &self.budget_waits
    }

    /// Closed retention verdict derived from the ceiling and the peak.
    pub const fn verdict(&self) -> BatchBudgetVerdict {
        self.verdict
    }

    /// H05 closure state carried verbatim from the DATA block (`not_claimed`).
    pub fn h05_closure(&self) -> &str {
        &self.h05_closure
    }

    /// Why H05 stays unclaimed.
    pub fn h05_reason(&self) -> &str {
        &self.h05_reason
    }
}

// ---------------------------------------------------------------------------
// Typed backend-termination classification (S06/T04, H19-N2)
// ---------------------------------------------------------------------------

/// Closed classification of WHY a parse backend stopped serving a request.
///
/// H19-N2 requires the refusal to be classified by TYPE, never by assumption,
/// and forbids recording a client-side transport timeout as if it were a
/// terminated Java process. The classes below are derived ONLY from the
/// already-closed [`FailureCode`] / [`FailurePhase`] of a
/// [`ParseBackendFailure`], so a timeout stays a timeout, a peer-closed
/// connection stays a connection loss, and anything the client cannot
/// attribute stays honestly [`BackendTerminationClass::Unattributable`].
///
/// **No variant claims, and none may ever be added to claim, that an external
/// Java/GROBID process was killed.** The death of a remote process is not
/// observable from this side of the socket; the client only ever sees the
/// transport consequence (a timeout, a closed connection or a refused
/// connect). A Java 5xx conversion failure therefore classifies as
/// [`BackendTerminationClass::Unattributable`] — it says nothing about the
/// process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendTerminationClass {
    /// The client-side request timeout elapsed (response head or body read).
    TransportTimeout,
    /// A non-timeout transport I/O failure where the peer ended the
    /// connection (request send / await response / response body framing).
    PeerClosedConnection,
    /// The connect phase failed before any request bytes were sent (a closed
    /// local port surfaces here as a refused connection; DNS failures share
    /// this phase).
    ConnectionRefused,
    /// The failure is a closed typed outcome the client-side transport cannot
    /// attribute to a termination class (busy, limits, invalid request /
    /// response, Java-side conversion, client-side encode).
    Unattributable,
}

impl BackendTerminationClass {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransportTimeout => "transport_timeout",
            Self::PeerClosedConnection => "peer_closed_connection",
            Self::ConnectionRefused => "connection_refused",
            Self::Unattributable => "unattributable",
        }
    }

    /// Every class, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::TransportTimeout,
        Self::PeerClosedConnection,
        Self::ConnectionRefused,
        Self::Unattributable,
    ];
}

impl fmt::Display for BackendTerminationClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify a closed failure *shape* by termination class (S06/T04, H19-N2).
///
/// The pure mapping behind [`classify_backend_termination`], exposed so a
/// caller that holds only a code/phase pair — for example a native route
/// refusal, which is not a [`ParseBackendFailure`] — reuses this classification
/// instead of restating it.
///
/// Mapping (the closed vocabulary is documented in [`crate::transport`]):
///
/// - `timeout` → [`BackendTerminationClass::TransportTimeout`] (any phase);
/// - `transport` at `connect` → [`BackendTerminationClass::ConnectionRefused`];
/// - `transport` at `send`/`await_response`/`read_body` →
///   [`BackendTerminationClass::PeerClosedConnection`];
/// - `conversion_failed` at `read_body` (a response body that could not be
///   read/framed, i.e. the peer ended mid-body) →
///   [`BackendTerminationClass::PeerClosedConnection`];
/// - everything else → [`BackendTerminationClass::Unattributable`].
pub const fn classify_termination(
    code: FailureCode,
    phase: FailurePhase,
) -> BackendTerminationClass {
    match (code, phase) {
        (FailureCode::Timeout, _) => BackendTerminationClass::TransportTimeout,
        (FailureCode::Transport, FailurePhase::Connect) => {
            BackendTerminationClass::ConnectionRefused
        }
        (FailureCode::Transport, FailurePhase::Send)
        | (FailureCode::Transport, FailurePhase::AwaitResponse)
        | (FailureCode::Transport, FailurePhase::ReadBody) => {
            BackendTerminationClass::PeerClosedConnection
        }
        (FailureCode::ConversionFailed, FailurePhase::ReadBody) => {
            BackendTerminationClass::PeerClosedConnection
        }
        _ => BackendTerminationClass::Unattributable,
    }
}

/// Classify a closed transport failure by termination class (S06/T04, H19-N2).
///
/// Pure and deterministic: the result is a function of [`FailureCode`] and
/// [`FailurePhase`] alone — never of a status, a payload, an order of calls or
/// any external state. It cannot and does not claim that a Java process was
/// killed (see [`classify_termination`] for the mapping).
///
/// The timeout/peer-close split is exactly the H19-N2 distinction: a transport
/// timeout must never be recorded as a killed process, and a closed connection
/// must never be recorded as a timeout.
pub const fn classify_backend_termination(
    failure: &ParseBackendFailure,
) -> BackendTerminationClass {
    classify_termination(failure.code(), failure.phase())
}
