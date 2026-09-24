//! S06/T02→T03 (M511-j7wz2f) — MEASUREMENT contract for the batch budget.
//!
//! T02 measured the PRE-FIX shell with data. T03 replaced the RED pins with
//! GREEN ones: the executor now measures a real window peak of retained
//! serialized bytes, publishes deterministic integer latency tails, and
//! enforces an OPTIONAL byte ceiling that fails closed through the existing
//! typed limit refusal (`ResponseBytes`) — all without rotating the summary
//! wire, the limits schema, the failure dictionary or the durable cursor.
//!
//! ## MEASURED PRE-FIX SNAPSHOT (T02, 2026-09-24, default run)
//!
//! ```text
//! probe A  window=3 admitted=6 backend_calls_before_release=6 sink_len_before_release=0
//!           max_publication_outstanding=6 max_reorder_depth=5
//!           max_in_flight=2 max_queue_depth=3 budget_waits=2 cursor_next=6
//! probe B  window=3 admitted=6 body=512→retained=7566  body=2048→retained=16782
//!           delta=9216 == 6*(2048-512); max_outstanding=6 in BOTH runs
//! probe C  window=1 admitted=4 cumulative_sink_bytes=3252 == sum(all outcomes)
//!           while max_publication_outstanding=1
//! probe D  admitted=4 logical_elapsed=75ms; 34 wire keys; 0 latency/ceiling/deadline
//! probe E1 typed backend failure at ordinal 2: succeeded=3 failed=1 backend_calls=4 cursor=4
//! probe E2 sink failure at ordinal 1: sink_errors=1 cursor_next=1 recorded=[0,2]
//! ```
//!
//! ## MEASURED T03 SNAPSHOT (2026-09-24, GREEN run)
//!
//! ```text
//! probe A  window=3 admitted=6 backend_calls_before_release=6
//!           max_publication_outstanding=6 max_reorder_depth=5 max_in_flight=2
//!           peak_retained_bytes=4878 retention_ceiling=not_observed(reason)
//! probe B  small_body=512→7566  large_body=2048→16782  delta=9216 == 6*(2048-512)
//!           outstanding=6 in BOTH runs (sizes 1261 vs 2797)
//! probe C  window=1 admitted=4 cumulative=3252 == peak_cumulative=3252
//!           observed_window_peak=813 max_outstanding=1
//! probe D  logical_elapsed=75ms p50=25 p95=75 p99=75; 34 summary wire keys;
//!           0 latency/ceiling/deadline fields in the summary
//! probe E3 ceiling=64 succeeded=0 failed=4 typed_response_bytes_refusals=4
//!           delivered_success_bytes=0 cursor=4 observed_peak=0 within_ceiling
//! probe E5 ceiling=2809 succeeded=1 failed=3 observed_peak=2809 cursor=4
//!           configured slots 4/4 unchanged, within_ceiling
//! probe E4 no ceiling → retention_ceiling=not_observed with the named reason,
//!           peak_retained_bytes=observed(813), verdict=not_observed
//! ```
//!
//! ## What T03 turned GREEN (and what it deliberately did NOT change)
//!
//! * The ordinal publication window bounds ADMITTED-in-flight work, not
//!   retained outcomes; T03 does not rewrite that admission bound. It adds the
//!   missing bound the milestone actually needs — a byte ceiling on retention —
//!   and pins the measured truth positively instead of failing on it.
//! * `peak_retained_bytes` is a genuine WINDOW peak (add when a successful
//!   outcome is retained, subtract when its durable sink record completes),
//!   so the cumulative sink sum can never be mistaken for a peak.
//! * Latency tails (p50/p95/p99) are published deterministically through the
//!   ADDITIVE `run_observed` seam; `ParseBatchRunSummary` keeps its v2 wire.
//! * A declared byte ceiling bounds the observed peak (H05-N2: a count limit
//!   is not a byte ceiling). An outcome larger than the WHOLE ceiling fails
//!   closed with the EXISTING typed refusal, never a hang and never a
//!   truncated payload.
//! * An undeclared ceiling is a typed `not_observed` WITH a named reason,
//!   never a fabricated zero. H05 stays unclaimed (RSS/Java CPU/slow-ordinal-0
//!   concurrency are still unmeasured).
//!
//! Each probe prints a report line under `-- --nocapture` and the summary wire
//! shape is pinned FATAL-style, so input/observability drift cannot pass
//! silently.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use da_parse::{
    BATCH_BUDGET_NO_RETENTION_CEILING_REASON, BATCH_BUDGET_VERDICTS, BatchBudgetVerdict, CacheMode,
    FailureCode, LimitKind, ParseArtifactCache, ParseBackend, ParseBackendFailure,
    ParseBackendRequest, ParseBatch, ParseBatchBudgetObservation, ParseBatchExecutor,
    ParseBatchLimits, ParseBatchMemoryCheckpoint, ParseBatchResultSink, ParseBatchRunSummary,
    ParseBatchSinkError, ParseCacheFailure, ParseDocumentFailure, ParseDocumentOutcome,
    RawStageRead, TeiEnvelope, TeiIdentity,
};
use tokio::sync::{Barrier, Mutex, Notify};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bounds are nonzero")
}

/// The 29 keys `ParseBatchRunSummary::serialize` writes today (in wire order).
/// `cancelled` is deliberately absent: the serializer never writes it. T03
/// must not grow, lose or rename any of them.
const RUN_SUMMARY_WIRE_KEYS: [&str; 29] = [
    "schema_version",
    "mode",
    "limits",
    "input_fingerprint",
    "resumed_from_ordinal",
    "cursor",
    "admitted",
    "skipped_on_resume",
    "completed",
    "succeeded",
    "failed",
    "backend_calls",
    "raw_hits",
    "raw_legacy_hits",
    "raw_writes",
    "raw_write_failures",
    "projector_hits",
    "projector_writes",
    "projector_write_failures",
    "configured_publication_capacity",
    "configured_execution_capacity",
    "max_in_flight_observed",
    "max_queue_depth_observed",
    "max_publication_outstanding_observed",
    "max_reorder_depth_observed",
    "budget_waits",
    "budget_scope",
    "sink_errors",
    "checkpoint_errors",
];

/// Fail FATAL on observability drift: if the summary wire grows, loses or
/// renames a field, every measurement below would silently change meaning.
fn assert_summary_wire_shape(summary: &ParseBatchRunSummary) {
    let value = serde_json::to_value(summary).expect("run summary serializes");
    let object = value.as_object().expect("run summary wire is an object");
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = RUN_SUMMARY_WIRE_KEYS.into_iter().collect();
    if actual != expected {
        panic!(
            "FATAL: ParseBatchRunSummary wire shape drifted; missing={:?} unexpected={:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&expected).collect::<Vec<_>>()
        );
    }
}

fn collect_wire_keys(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                out.insert(key.clone());
                collect_wire_keys(nested, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_wire_keys(item, out);
            }
        }
        _ => {}
    }
}

/// The summary must NOT publish a latency tail, a byte ceiling, an RSS/CPU
/// observation or a per-document deadline. `run_observed` carries those
/// instead, so the frozen summary stays a frozen summary. Returns every wire
/// key it does publish, for the report line.
fn assert_no_latency_ceiling_or_deadline_surface(summary: &ParseBatchRunSummary) -> Vec<String> {
    let value = serde_json::to_value(summary).expect("run summary serializes");
    let mut keys = BTreeSet::new();
    collect_wire_keys(&value, &mut keys);
    let forbidden = [
        "latency",
        "p50",
        "p95",
        "p99",
        "duration",
        "retained_bytes",
        "rss",
        "cpu",
        "ceiling",
        "deadline",
        "timeout_ms",
    ];
    let hits: Vec<&String> = keys
        .iter()
        .filter(|key| {
            let lower = key.to_lowercase();
            forbidden.iter().any(|token| lower.contains(token))
        })
        .collect();
    assert!(
        hits.is_empty(),
        "FATAL: the run summary unexpectedly publishes a latency/ceiling/deadline field: {hits:?}"
    );
    keys.into_iter().collect()
}

/// Assert the closed verdict vocabulary is bound to the DATA block.
fn assert_closed_verdict_vocabulary() {
    assert_eq!(
        BatchBudgetVerdict::ALL.len(),
        BATCH_BUDGET_VERDICTS.len(),
        "the Rust verdict vocabulary must match the DATA metric vocabulary"
    );
    for (verdict, wire) in [
        (BatchBudgetVerdict::WithinCeiling, "within_ceiling"),
        (BatchBudgetVerdict::MemoryPressure, "memory_pressure"),
        (BatchBudgetVerdict::NotObserved, "not_observed"),
    ] {
        assert_eq!(verdict.as_str(), wire);
        assert_eq!(
            serde_json::to_value(verdict).unwrap(),
            serde_json::json!(wire)
        );
        assert!(
            BATCH_BUDGET_VERDICTS.contains(&verdict.as_str()),
            "`{wire}` must stay in the DATA verdict vocabulary"
        );
    }
}

/// Poll an atomic until it stops growing for `quiet_rounds` consecutive yields.
/// Under a paused clock a yield is cheap, so this settles the run without
/// assuming any particular (possibly wrong) expected call count.
async fn settle_quiet(counter: &AtomicUsize, quiet_rounds: usize, label: &str) {
    let mut last = usize::MAX;
    let mut quiet = 0usize;
    for _ in 0..200_000 {
        let now = counter.load(Ordering::SeqCst);
        if now == last {
            quiet += 1;
            if quiet >= quiet_rounds {
                return;
            }
        } else {
            quiet = 0;
            last = now;
        }
        tokio::task::yield_now().await;
    }
    panic!("FATAL: `{label}` never settled");
}

fn requests(prefix: &str, count: usize) -> Vec<ParseBackendRequest> {
    (0..count)
        .map(|ordinal| {
            ParseBackendRequest::new(
                format!("{prefix}-{ordinal}"),
                format!("%PDF-{prefix}-{ordinal}").into_bytes(),
            )
        })
        .collect()
}

fn admitted_batch(
    prefix: &str,
    count: usize,
    workers: usize,
    queue_capacity: usize,
    max_in_flight: usize,
) -> ParseBatch {
    admitted_batch_with(
        prefix,
        count,
        CacheMode::Off,
        workers,
        queue_capacity,
        max_in_flight,
        None,
    )
}

/// Admit a batch with an explicit cache mode and an optional declared byte
/// ceiling (S06/T03: `max_retained_bytes` is additive on `ParseBatchLimits`).
#[allow(clippy::too_many_arguments)]
fn admitted_batch_with(
    prefix: &str,
    count: usize,
    mode: CacheMode,
    workers: usize,
    queue_capacity: usize,
    max_in_flight: usize,
    ceiling: Option<usize>,
) -> ParseBatch {
    let mut limits = ParseBatchLimits::new(
        nz(workers),
        nz(queue_capacity),
        nz(max_in_flight),
        nz(count),
    );
    if let Some(bytes) = ceiling {
        limits = limits.with_max_retained_bytes(nz(bytes));
    }
    ParseBatch::admit(mode, limits, requests(prefix, count), None).expect("fixture admits")
}

// ---------------------------------------------------------------------------
// Backend: a blocked ordinal-0 frontier with a known TEI body size
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct MeasuredBackendState {
    calls: AtomicUsize,
    frontier_started: Notify,
    released: AtomicBool,
    release_signal: Notify,
    body_chars: usize,
    block_frontier: bool,
}

#[derive(Debug, Clone)]
struct MeasuredBackend {
    state: Arc<MeasuredBackendState>,
}

impl MeasuredBackend {
    /// Blocks ordinal 0 until [`Self::release_frontier`] — used by the probes
    /// that measure the parked-outcome set.
    fn new(body_chars: usize) -> Self {
        Self::with_frontier(body_chars, true)
    }

    /// Never blocks: outcomes publish as they complete.
    fn fast(body_chars: usize) -> Self {
        Self::with_frontier(body_chars, false)
    }

    fn with_frontier(body_chars: usize, block_frontier: bool) -> Self {
        Self {
            state: Arc::new(MeasuredBackendState {
                calls: AtomicUsize::new(0),
                frontier_started: Notify::new(),
                released: AtomicBool::new(false),
                release_signal: Notify::new(),
                body_chars,
                block_frontier,
            }),
        }
    }

    fn calls(&self) -> usize {
        self.state.calls.load(Ordering::SeqCst)
    }

    fn release_frontier(&self) {
        self.state.released.store(true, Ordering::SeqCst);
        self.state.release_signal.notify_waiters();
    }
}

#[async_trait]
impl ParseBackend for MeasuredBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        self.state.calls.fetch_add(1, Ordering::SeqCst);
        if self.state.block_frontier && request.paper_id() == "measure-0" {
            self.state.frontier_started.notify_waiters();
            while !self.state.released.load(Ordering::SeqCst) {
                self.state.release_signal.notified().await;
            }
        }
        let body = "x".repeat(self.state.body_chars);
        let payload = format!("<TEI><text>{body}</text></TEI>");
        Ok(TeiEnvelope::new(
            payload.into_bytes(),
            TeiIdentity::new("budget-measurement", "budget-measurement-v1"),
        ))
    }
}

// ---------------------------------------------------------------------------
// Backend: per-ordinal logical sleep (latency-tail probe)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct SleepingBackend {
    step: Duration,
}

#[async_trait]
impl ParseBackend for SleepingBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let ordinal = request
            .paper_id()
            .rsplit('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_default();
        if !self.step.is_zero() {
            tokio::time::sleep(self.step * ordinal).await;
        }
        Ok(TeiEnvelope::new(
            b"<TEI><text>latency fixture</text></TEI>".to_vec(),
            TeiIdentity::new("budget-measurement", "budget-measurement-v1"),
        ))
    }
}

// ---------------------------------------------------------------------------
// Backend: typed failure for a chosen ordinal (negative path)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct FailingBackend {
    fail_ordinal: u32,
}

#[async_trait]
impl ParseBackend for FailingBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let ordinal = request
            .paper_id()
            .rsplit('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_default();
        if ordinal == self.fail_ordinal {
            return Err(ParseBackendFailure::no_content());
        }
        Ok(TeiEnvelope::new(
            b"<TEI><text>failing fixture</text></TEI>".to_vec(),
            TeiIdentity::new("budget-measurement", "budget-measurement-v1"),
        ))
    }
}

// ---------------------------------------------------------------------------
// Backend: a barrier so every document materializes at the same moment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BarrierBackend {
    barrier: Arc<Barrier>,
    body_chars: usize,
}

#[async_trait]
impl ParseBackend for BarrierBackend {
    async fn fetch_tei(
        &self,
        _request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        self.barrier.wait().await;
        let body = "x".repeat(self.body_chars);
        let payload = format!("<TEI><text>{body}</text></TEI>");
        Ok(TeiEnvelope::new(
            payload.into_bytes(),
            TeiIdentity::new("budget-measurement", "budget-measurement-v1"),
        ))
    }
}

#[derive(Debug, Default)]
struct MissCache;

#[async_trait]
impl ParseArtifactCache for MissCache {
    async fn read_raw(&self, _key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        Ok(None)
    }

    async fn write_raw(&self, _key: &str, _payload: &[u8]) -> Result<(), ParseCacheFailure> {
        Ok(())
    }

    async fn read_projected(&self, _key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        Ok(None)
    }

    async fn write_projected(&self, _key: &str, _payload: &[u8]) -> Result<(), ParseCacheFailure> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sink: records ordinals, per-outcome encoded sizes and a cumulative total
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct SinkState {
    ordinals: Vec<u32>,
    encoded: Vec<(u32, usize)>,
}

#[derive(Debug, Default)]
struct MeasuringSink {
    state: Mutex<SinkState>,
    cumulative_bytes: AtomicUsize,
    peak_cumulative_bytes: AtomicUsize,
}

impl MeasuringSink {
    async fn ordinals(&self) -> Vec<u32> {
        self.state.lock().await.ordinals.clone()
    }

    async fn encoded(&self) -> Vec<(u32, usize)> {
        self.state.lock().await.encoded.clone()
    }

    fn cumulative_bytes(&self) -> usize {
        self.cumulative_bytes.load(Ordering::SeqCst)
    }

    /// The sink's own running maximum. It has no release visibility, so this
    /// equals the cumulative total — never a window peak (T02 probe C).
    fn peak_cumulative_bytes(&self) -> usize {
        self.peak_cumulative_bytes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ParseBatchResultSink for MeasuringSink {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        let ordinal = outcome.ordinal();
        let encoded = serde_json::to_vec(&outcome).expect("outcome wire encoding");
        let len = encoded.len();
        {
            let mut state = self.state.lock().await;
            state.ordinals.push(ordinal);
            state.encoded.push((ordinal, len));
        }
        let total = self.cumulative_bytes.fetch_add(len, Ordering::SeqCst) + len;
        self.peak_cumulative_bytes
            .fetch_max(total, Ordering::SeqCst);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sink: fails on a chosen ordinal (negative path)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct FailingSink {
    fail_ordinal: u32,
    recorded: Arc<Mutex<Vec<u32>>>,
}

#[async_trait]
impl ParseBatchResultSink for FailingSink {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        if outcome.ordinal() == self.fail_ordinal {
            return Err(ParseBatchSinkError(format!(
                "injected sink failure at ordinal {}",
                self.fail_ordinal
            )));
        }
        self.recorded.lock().await.push(outcome.ordinal());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sink: classifies every terminal outcome (retention-refusal probe)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct OutcomeKindSink {
    ordinals: Mutex<Vec<u32>>,
    successes: AtomicUsize,
    backend_limit_refusals: AtomicUsize,
    other_failures: AtomicUsize,
    delivered_success_bytes: AtomicUsize,
}

#[async_trait]
impl ParseBatchResultSink for OutcomeKindSink {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        self.ordinals.lock().await.push(outcome.ordinal());
        match outcome.result() {
            Ok(_) => {
                self.successes.fetch_add(1, Ordering::SeqCst);
                let len = serde_json::to_vec(&outcome).map_or(0, |v| v.len());
                self.delivered_success_bytes
                    .fetch_add(len, Ordering::SeqCst);
            }
            Err(ParseDocumentFailure::Backend(failure))
                if failure.enforced_limit() == Some(LimitKind::ResponseBytes)
                    && failure.code() == FailureCode::LimitExceeded =>
            {
                self.backend_limit_refusals.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                self.other_failures.fetch_add(1, Ordering::SeqCst);
            }
        }
        Ok(())
    }
}

impl OutcomeKindSink {
    async fn ordinals(&self) -> Vec<u32> {
        self.ordinals.lock().await.clone()
    }
}

// ---------------------------------------------------------------------------
// Run helpers
// ---------------------------------------------------------------------------

/// Run a blocked-frontier batch through the OBSERVED seam and return the
/// summary, sink, pre-release measurements and the budget observation.
#[allow(clippy::too_many_arguments)]
async fn run_blocked_observed(
    body_chars: usize,
    count: usize,
    mode: CacheMode,
    window: usize,
    workers: usize,
    queue_capacity: usize,
    max_in_flight: usize,
    ceiling: Option<usize>,
) -> (
    ParseBatchRunSummary,
    Arc<MeasuringSink>,
    usize,
    usize,
    ParseBatchBudgetObservation,
) {
    let backend = MeasuredBackend::new(body_chars);
    let executor =
        ParseBatchExecutor::new(backend.clone(), MissCache).with_publication_window(nz(window));
    let batch = admitted_batch_with(
        "measure",
        count,
        mode,
        workers,
        queue_capacity,
        max_in_flight,
        ceiling,
    );
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let run = tokio::spawn({
        let sink = sink.clone();
        async move { executor.run_observed(batch, sink, checkpoint).await }
    });
    backend.state.frontier_started.notified().await;
    settle_quiet(&backend.state.calls, 8, "backend calls before release").await;
    let calls_before_release = backend.calls();
    let sink_len_before_release = sink.ordinals().await.len();
    backend.release_frontier();
    let (summary, observation) = run.await.expect("executor task completes");
    (
        summary,
        sink,
        calls_before_release,
        sink_len_before_release,
        observation,
    )
}

/// Measure the exact serialized size of ONE successful outcome for `body_chars`
/// (no ceiling). Used to declare a ceiling that fits exactly one outcome.
async fn observed_single_outcome_bytes(body_chars: usize, mode: CacheMode) -> u64 {
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(body_chars), MissCache);
    let batch = admitted_batch_with("measure", 1, mode, 1, 1, 1, None);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let (summary, observation) = executor.run_observed(batch, sink, checkpoint).await;
    assert_summary_wire_shape(&summary);
    assert_eq!(summary.succeeded(), 1);
    observation
        .peak_retained_bytes()
        .value()
        .copied()
        .expect("a single-outcome run observes its retained bytes")
}

// ---------------------------------------------------------------------------
// Probe A — the count window bounds admission; the BYTE ceiling bounds retention
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_count_window_does_not_bound_retention_the_byte_ceiling_does() {
    const WINDOW: usize = 3;
    const COUNT: usize = 6;
    assert_closed_verdict_vocabulary();
    let (summary, sink, calls_before_release, sink_len_before_release, observation) =
        run_blocked_observed(64, COUNT, CacheMode::Off, WINDOW, 3, 3, 3, None).await;
    assert_summary_wire_shape(&summary);

    let ordinals = sink.ordinals().await;
    let peak = observation
        .peak_retained_bytes()
        .value()
        .copied()
        .expect("the retained-byte window peak is measured");
    println!(
        "[S06/T03 probe A] window={WINDOW} admitted={COUNT} backend_calls_before_release={calls_before_release} \
sink_len_before_release={sink_len_before_release} max_publication_outstanding={} max_reorder_depth={} \
max_in_flight={} peak_retained_bytes={peak} retention_ceiling={:?} verdict={}",
        summary.max_publication_outstanding_observed(),
        summary.max_reorder_depth_observed(),
        summary.max_in_flight_observed(),
        observation.retention_ceiling(),
        observation.verdict(),
    );

    // Ordered, exactly-once publication and durable-prefix advancement.
    assert_eq!(summary.succeeded(), COUNT as u32);
    assert_eq!(summary.failed(), 0);
    assert_eq!(ordinals, (0..COUNT as u32).collect::<Vec<u32>>());
    assert_eq!(summary.cursor().next_ordinal(), COUNT as u32);

    // The two INDEPENDENT slot bounds that do hold: queue depth and in-flight
    // execution degree.
    assert!(summary.max_queue_depth_observed() <= 3, "queue bound");
    assert!(summary.max_in_flight_observed() <= 3, "in-flight bound");
    assert_eq!(summary.configured_publication_capacity(), WINDOW);
    assert_eq!(summary.configured_execution_capacity(), 3);

    // Nothing is published while the frontier ordinal is blocked.
    assert_eq!(
        sink_len_before_release, 0,
        "no outcome may be published while ordinal 0 holds the frontier"
    );

    // GREEN pin for the T02 RED measurement. The ordinal publication window is
    // an ADMISSION bound, not a retention bound: its permit is released when a
    // worker finishes COMPUTATION, so parked outcomes accumulate up to the
    // admitted batch. T03 pins that truth positively and adds the retention
    // bound the milestone needs — the byte ceiling (probes below).
    assert_eq!(
        calls_before_release, COUNT,
        "admission reaches the whole batch while the finite publication window is held"
    );
    assert!(
        summary.max_publication_outstanding_observed() <= COUNT,
        "retained outcomes are bounded by the admitted batch count"
    );
    assert!(
        peak > 0,
        "the window peak of retained bytes must be a real measurement, not zero"
    );

    // No ceiling declared → a typed absence WITH a named reason, never a zero,
    // and the verdict cannot be claimed.
    assert!(!observation.retention_ceiling().is_observed());
    assert_eq!(
        observation.retention_ceiling().reason(),
        Some(BATCH_BUDGET_NO_RETENTION_CEILING_REASON)
    );
    assert_eq!(
        observation.retention_ceiling().value(),
        None,
        "an undeclared ceiling must carry no numeric value"
    );
    assert_eq!(observation.verdict(), BatchBudgetVerdict::NotObserved);
    assert_eq!(observation.h05_closure(), "not_claimed");
}

// ---------------------------------------------------------------------------
// Probe B — retained bytes scale with document size at a constant slot count
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_retained_bytes_scale_with_document_size_at_constant_slots() {
    const WINDOW: usize = 3;
    const COUNT: usize = 6;
    const SMALL_BODY: usize = 512;
    const LARGE_BODY: usize = 2048;

    let (small_summary, small_sink, _, _, _) =
        run_blocked_observed(SMALL_BODY, COUNT, CacheMode::Off, WINDOW, 3, 3, 3, None).await;
    let (large_summary, large_sink, _, _, _) =
        run_blocked_observed(LARGE_BODY, COUNT, CacheMode::Off, WINDOW, 3, 3, 3, None).await;
    assert_summary_wire_shape(&small_summary);
    assert_summary_wire_shape(&large_summary);

    let small_bytes = small_sink.cumulative_bytes();
    let large_bytes = large_sink.cumulative_bytes();
    let small_sizes: Vec<usize> = small_sink
        .encoded()
        .await
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let large_sizes: Vec<usize> = large_sink
        .encoded()
        .await
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let delta = large_bytes.saturating_sub(small_bytes);
    let expected_delta = COUNT * (LARGE_BODY - SMALL_BODY);

    println!(
        "[S06/T03 probe B] window={WINDOW} admitted={COUNT} small_body={SMALL_BODY} large_body={LARGE_BODY} \
small_retained_bytes={small_bytes} large_retained_bytes={large_bytes} delta={delta} expected_delta={expected_delta} \
small_payload_sizes={:?} large_payload_sizes={:?} small_max_outstanding={} large_max_outstanding={}",
        &small_sizes[..small_sizes.len().min(3)],
        &large_sizes[..large_sizes.len().min(3)],
        small_summary.max_publication_outstanding_observed(),
        large_summary.max_publication_outstanding_observed(),
    );

    // Constant slot count across both runs: the count limit is unchanged.
    assert_eq!(
        small_summary.max_publication_outstanding_observed(),
        large_summary.max_publication_outstanding_observed(),
        "the slot count must not move when only the document size changes"
    );
    assert_eq!(
        small_summary.max_reorder_depth_observed(),
        large_summary.max_reorder_depth_observed()
    );
    assert_eq!(
        small_summary.configured_publication_capacity(),
        large_summary.configured_publication_capacity()
    );

    // H05-N2: bytes grow with document size at a CONSTANT slot count, and no
    // byte ceiling is implied by those slots.
    assert!(
        large_bytes > small_bytes,
        "retained bytes must grow with document size"
    );
    assert_eq!(
        delta, expected_delta,
        "retained bytes must grow by exactly COUNT * (large_body - small_body): \
         full outcomes, no truncation, no byte ceiling"
    );
    assert!(
        large_bytes >= COUNT * LARGE_BODY,
        "retained bytes must at least carry the full generated body text"
    );
}

// ---------------------------------------------------------------------------
// Probe C — the window peak is not the cumulative sink sum
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_window_peak_is_not_the_cumulative_sink_sum() {
    // Fast frontier: outcomes publish as they complete, so the live retained
    // set stays far below the total. The sink still sums everything.
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(64), MissCache)
        .with_publication_window(nz(1));
    let batch = admitted_batch("measure", 4, 4, 4, 4);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let (summary, observation) = executor.run_observed(batch, sink.clone(), checkpoint).await;
    assert_summary_wire_shape(&summary);

    let encoded = sink.encoded().await;
    let total = encoded.iter().map(|(_, len)| *len).sum::<usize>();
    let peak = observation
        .peak_retained_bytes()
        .value()
        .copied()
        .expect("the retained-byte window peak is measured") as usize;
    println!(
        "[S06/T03 probe C] admitted={} cumulative_sink_bytes={} peak_cumulative_sink_bytes={} \
sum_of_all_outcomes={} observed_window_peak_retained_bytes={peak} max_publication_outstanding={} \
max_reorder_depth={}",
        summary.admitted(),
        sink.cumulative_bytes(),
        sink.peak_cumulative_bytes(),
        total,
        summary.max_publication_outstanding_observed(),
        summary.max_reorder_depth_observed(),
    );

    assert_eq!(sink.cumulative_bytes(), total);
    assert_eq!(sink.peak_cumulative_bytes(), total);
    assert_eq!(encoded.len(), 4);
    // The sink never observes release, so its running sum is a CUMULATIVE
    // total, not a window peak. The executor-side observation is the real
    // peak: with a one-slot publication window at most one outcome is
    // retained at a time, so the peak is one outcome and strictly below the
    // cumulative total.
    assert!(peak > 0, "the window peak must be measured, not zero");
    assert!(
        peak < total,
        "FATAL: the window peak ({peak}) must be strictly below the cumulative sink total ({total})"
    );
    assert!(
        summary.max_publication_outstanding_observed() < summary.admitted() as usize,
        "the live retained set is smaller than the cumulative total"
    );
}

// ---------------------------------------------------------------------------
// Probe D — latency tails are published; the summary wire is unchanged
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_latency_tails_are_published_and_the_summary_wire_is_unchanged() {
    const COUNT: usize = 4;
    let backend = SleepingBackend {
        step: Duration::from_millis(25),
    };
    let executor = ParseBatchExecutor::new(backend, MissCache).with_publication_window(nz(COUNT));
    let batch = admitted_batch("measure", COUNT, COUNT, COUNT, COUNT);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let started = tokio::time::Instant::now();
    let (summary, observation) = executor.run_observed(batch, sink.clone(), checkpoint).await;
    let elapsed = started.elapsed();
    assert_summary_wire_shape(&summary);

    let published_keys = assert_no_latency_ceiling_or_deadline_surface(&summary);
    let p50 = observation
        .latency_p50_ms()
        .value()
        .copied()
        .expect("p50 latency tail is observed");
    let p95 = observation
        .latency_p95_ms()
        .value()
        .copied()
        .expect("p95 latency tail is observed");
    let p99 = observation
        .latency_p99_ms()
        .value()
        .copied()
        .expect("p99 latency tail is observed");
    println!(
        "[S06/T03 probe D] admitted={COUNT} logical_elapsed_ms={} summary_wire_keys={} \
latency_or_ceiling_fields=0 latency_p50_ms={p50} latency_p95_ms={p95} latency_p99_ms={p99} \
retention_ceiling={:?}",
        elapsed.as_millis(),
        published_keys.len(),
        observation.retention_ceiling(),
    );

    assert!(
        elapsed >= Duration::from_millis(25 * (COUNT as u64 - 1)),
        "the run duration is measurable and differs across documents"
    );
    // Deterministic integer nearest-rank tails, ordered by construction.
    assert!(
        p50 <= p95 && p95 <= p99,
        "latency tails must be ordered: p50={p50} p95={p95} p99={p99}"
    );
    assert!(
        p95 >= 25,
        "at least one document slept 25ms; p95 must reflect it, got {p95}"
    );
    // The summary must not have grown the latency/ceiling surface.
    assert!(published_keys.len() >= RUN_SUMMARY_WIRE_KEYS.len());
    assert_eq!(observation.verdict(), BatchBudgetVerdict::NotObserved);
}

// ---------------------------------------------------------------------------
// Probe E — negative paths
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_typed_backend_failure_is_terminal_and_not_amplified() {
    let executor = ParseBatchExecutor::new(FailingBackend { fail_ordinal: 2 }, MissCache);
    let batch = admitted_batch("measure", 4, 4, 4, 4);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let summary = executor.run(batch, sink.clone(), checkpoint).await;
    assert_summary_wire_shape(&summary);

    println!(
        "[S06/T03 probe E1] admitted={} succeeded={} failed={} backend_calls={} cursor_next={} ordinals={:?}",
        summary.admitted(),
        summary.succeeded(),
        summary.failed(),
        summary.backend_calls(),
        summary.cursor().next_ordinal(),
        sink.ordinals().await,
    );

    assert_eq!(summary.succeeded(), 3);
    assert_eq!(summary.failed(), 1);
    // Exactly one backend invocation per document: a typed failure is not
    // retried or amplified.
    assert_eq!(summary.backend_calls(), 4);
    // The failed document still produces a terminal outcome and the durable
    // prefix advances over it (it has no content to lose).
    assert_eq!(sink.ordinals().await, vec![0, 1, 2, 3]);
    assert_eq!(summary.cursor().next_ordinal(), 4);
}

#[tokio::test(start_paused = true)]
async fn measured_sink_failure_freezes_the_durable_cursor_before_the_unacked_ordinal() {
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(64), MissCache);
    let batch = admitted_batch("measure", 3, 3, 3, 3);
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(FailingSink {
        fail_ordinal: 1,
        recorded: recorded.clone(),
    });
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let summary = executor.run(batch, sink, checkpoint).await;
    assert_summary_wire_shape(&summary);

    println!(
        "[S06/T03 probe E2] admitted={} succeeded={} sink_errors={} cursor_next={} recorded={:?}",
        summary.admitted(),
        summary.succeeded(),
        summary.sink_errors(),
        summary.cursor().next_ordinal(),
        recorded.lock().await,
    );

    assert_eq!(summary.sink_errors(), 1);
    assert_eq!(
        summary.cursor().next_ordinal(),
        1,
        "the durable cursor must not move past the unacknowledged ordinal"
    );
    assert_eq!(recorded.lock().await.clone(), vec![0, 2]);
}

/// Over-ceiling refusal is typed, terminal, non-blocking and non-truncating.
///
/// Runs on the real clock with an explicit timeout so a hang would be a test
/// failure, not a CI stall.
#[tokio::test]
async fn retention_ceiling_refuses_an_over_ceiling_outcome_without_hanging_or_truncating() {
    const COUNT: usize = 4;
    const CEILING: usize = 64;
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(4096), MissCache);
    // WriteThrough + the always-publishing MissCache keeps every successful
    // outcome durably recoverable, so cursor semantics stay the established
    // ones.
    let batch = admitted_batch_with(
        "measure",
        COUNT,
        CacheMode::WriteThrough,
        4,
        4,
        4,
        Some(CEILING),
    );
    let sink = Arc::new(OutcomeKindSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let (summary, observation) = tokio::time::timeout(
        Duration::from_secs(30),
        executor.run_observed(batch, sink.clone(), checkpoint),
    )
    .await
    .expect("a declared byte ceiling must fail closed, never hang");
    assert_summary_wire_shape(&summary);

    println!(
        "[S06/T03 probe E3] declared_ceiling={CEILING} admitted={COUNT} succeeded={} failed={} \
typed_response_limit_refusals={} delivered_success_bytes={} cursor_next={} ordinals={:?} peak={:?} verdict={}",
        summary.succeeded(),
        summary.failed(),
        sink.backend_limit_refusals.load(Ordering::SeqCst),
        sink.delivered_success_bytes.load(Ordering::SeqCst),
        summary.cursor().next_ordinal(),
        sink.ordinals().await,
        observation.peak_retained_bytes(),
        observation.verdict(),
    );

    // Every document whose own size exceeds the WHOLE ceiling fails closed
    // through the EXISTING typed limit vocabulary — no new failure code.
    assert_eq!(summary.succeeded(), 0);
    assert_eq!(summary.failed(), COUNT as u32);
    assert_eq!(
        sink.backend_limit_refusals.load(Ordering::SeqCst),
        COUNT,
        "every over-ceiling outcome must be a typed ResponseBytes limit refusal"
    );
    assert_eq!(sink.other_failures.load(Ordering::SeqCst), 0);
    // No truncation: no success payload is delivered at all.
    assert_eq!(sink.successes.load(Ordering::SeqCst), 0);
    assert_eq!(sink.delivered_success_bytes.load(Ordering::SeqCst), 0);
    // Publication order and cursor semantics are unchanged: every ordinal is
    // published exactly once in order and a terminal failure still advances
    // the durable prefix.
    assert_eq!(
        sink.ordinals().await,
        (0..COUNT as u32).collect::<Vec<u32>>()
    );
    assert_eq!(summary.cursor().next_ordinal(), COUNT as u32);
    // The observation is honest: the declared ceiling is observed, the peak is
    // a real measurement of zero (nothing was ever retained), and the verdict
    // follows from the two.
    assert_eq!(
        observation.retention_ceiling().value().copied(),
        Some(CEILING as u64)
    );
    assert_eq!(
        observation.peak_retained_bytes().value().copied(),
        Some(0),
        "nothing was retained because every outcome exceeded the ceiling"
    );
    assert_eq!(observation.verdict(), BatchBudgetVerdict::WithinCeiling);
}

/// The byte ceiling bounds the observed peak independently of the slot count
/// (H05-N2), under real simultaneous pressure, without hanging.
#[tokio::test]
async fn declared_byte_ceiling_bounds_the_peak_independently_of_the_slot_count() {
    const COUNT: usize = 4;
    const BODY: usize = 2048;
    let single = observed_single_outcome_bytes(BODY, CacheMode::WriteThrough).await;
    let ceiling = usize::try_from(single).expect("outcome size fits usize");
    assert!(ceiling > 0);

    // A barrier makes every document materialize at the same moment, so the
    // byte ceiling is exercised under genuine pressure while the SLOT count is
    // unchanged.
    let barrier = Arc::new(Barrier::new(COUNT + 1));
    let executor = ParseBatchExecutor::new(
        BarrierBackend {
            barrier: barrier.clone(),
            body_chars: BODY,
        },
        MissCache,
    )
    .with_publication_window(nz(COUNT));
    let batch = admitted_batch_with(
        "measure",
        COUNT,
        CacheMode::WriteThrough,
        COUNT,
        COUNT,
        COUNT,
        Some(ceiling),
    );
    let sink = Arc::new(OutcomeKindSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let run = tokio::spawn({
        let sink = sink.clone();
        async move { executor.run_observed(batch, sink, checkpoint).await }
    });
    barrier.wait().await;
    let (summary, observation) = tokio::time::timeout(Duration::from_secs(30), run)
        .await
        .expect("a byte ceiling under pressure must never hang")
        .expect("executor task completes");
    assert_summary_wire_shape(&summary);

    let peak = observation
        .peak_retained_bytes()
        .value()
        .copied()
        .expect("the retained-byte window peak is measured");
    println!(
        "[S06/T03 probe E5] declared_ceiling={ceiling} admitted={COUNT} succeeded={} failed={} \
observed_peak_retained_bytes={peak} configured_publication={} configured_execution={} cursor_next={} \
ordinals={:?} verdict={}",
        summary.succeeded(),
        summary.failed(),
        summary.configured_publication_capacity(),
        summary.configured_execution_capacity(),
        summary.cursor().next_ordinal(),
        sink.ordinals().await,
        observation.verdict(),
    );

    // The declared ceiling bounds the observed peak — the property a
    // document-count limit alone cannot provide (H05-N2).
    assert!(
        peak <= single,
        "FATAL: the observed peak {peak} exceeded the declared ceiling {single}"
    );
    assert!(peak > 0, "at least one outcome fits and is retained");
    assert_eq!(
        observation.retention_ceiling().value().copied(),
        Some(single)
    );
    assert_eq!(observation.verdict(), BatchBudgetVerdict::WithinCeiling);
    // Slots are unchanged and every ordinal still publishes in order.
    assert_eq!(summary.configured_publication_capacity(), COUNT);
    assert_eq!(summary.configured_execution_capacity(), COUNT);
    assert_eq!(
        sink.ordinals().await,
        (0..COUNT as u32).collect::<Vec<u32>>()
    );
    assert_eq!(summary.completed(), COUNT as u32);
    assert_eq!(summary.succeeded() + summary.failed(), COUNT as u32);
    assert_eq!(summary.cursor().next_ordinal(), COUNT as u32);
}

/// An undeclared ceiling is a typed absence with a named reason, never a zero.
#[tokio::test(start_paused = true)]
async fn absent_retention_ceiling_is_a_named_absence_not_a_zero() {
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(64), MissCache);
    let batch = admitted_batch("measure", 3, 3, 3, 3);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let (summary, observation) = executor.run_observed(batch, sink, checkpoint).await;
    assert_summary_wire_shape(&summary);

    let wire = serde_json::to_value(&observation).expect("observation serializes");
    println!(
        "[S06/T03 probe E4] retention_ceiling={:?} peak_retained_bytes={:?} latency_p50_ms={:?} verdict={} h05_closure={}",
        wire["retention_ceiling"],
        wire["peak_retained_bytes"],
        wire["latency_p50_ms"],
        observation.verdict(),
        observation.h05_closure(),
    );

    assert!(!observation.retention_ceiling().is_observed());
    assert_eq!(
        observation.retention_ceiling().reason(),
        Some(BATCH_BUDGET_NO_RETENTION_CEILING_REASON)
    );
    assert_eq!(
        wire["retention_ceiling"]["state"],
        serde_json::json!("not_observed")
    );
    assert_eq!(
        wire["retention_ceiling"]["reason"],
        serde_json::json!(BATCH_BUDGET_NO_RETENTION_CEILING_REASON)
    );
    assert!(
        wire["retention_ceiling"].get("value").is_none(),
        "a typed absence must carry no numeric field — a zero is not a measurement"
    );
    assert_eq!(wire["verdict"], serde_json::json!("not_observed"));
    // The peak IS measured even without a ceiling: it is a real observation.
    assert!(observation.peak_retained_bytes().is_observed());
    assert_eq!(
        wire["peak_retained_bytes"]["state"],
        serde_json::json!("observed")
    );
    // H05 stays unclaimed: RSS/Java CPU and the slow-ordinal-0 concurrency
    // scenario are still unmeasured.
    assert_eq!(observation.h05_closure(), "not_claimed");
    assert!(!observation.h05_reason().is_empty());
    assert_eq!(observation.verdict(), BatchBudgetVerdict::NotObserved);
}
