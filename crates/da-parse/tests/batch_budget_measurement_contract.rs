//! S06/T02 (M511-j7wz2f) — MEASUREMENT contract for the current batch budget
//! shell. It measures; it does not fix.
//!
//! H05 requires memory/byte/CPU/latency ceilings and a BOUNDED reorder to be
//! measured before any ceiling is introduced, so that a later change can be
//! told apart from curve-fitting. This target fixes the MEASURED shell of the
//! executor that exists today, using only the public `da_parse` surface
//! (`crates/da-parse/src` is not touched).
//!
//! The probes below answer five questions with data:
//!
//! A. Do the SLOT bounds hold under a blocked frontier, and how far does the
//!    ordinal publication window actually reach?
//! B. Does the retained BYTE volume scale with document size at a constant
//!    slot count? (H05-N2: a document-count limit is not a byte ceiling.)
//! C. Is the only byte-ish observable (`PayloadAccountingSink`-style
//!    cumulative sink sum) a window PEAK, or a cumulative total? (It has no
//!    release visibility, so the window peak is not observable at all.)
//! D. Are latency tails, a retention ceiling or a per-document deadline
//!    published by `ParseBatchRunSummary`? (No.)
//! E. Negative paths: typed backend failure, sink failure.
//!
//! Each probe prints a report line under `-- --nocapture` and the summary wire
//! shape is pinned FATAL-style, so input/observability drift cannot pass
//! silently.
//!
//! ## MEASURED PRE-FIX SNAPSHOT (2026-09-24, default run)
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
//! ## What T03 must turn GREEN
//!
//! * The ordinal publication window does NOT bound retained outcomes. The plan
//!   expected `backend_calls == publication_window` and
//!   `max_publication_outstanding <= window` / `max_reorder_depth <= window-1`;
//!   the MEASURED shell reaches the whole batch instead (6 and 5 for a window of
//!   3). The permit is released when a worker finishes COMPUTATION, not when its
//!   outcome is published, so parked outcomes accumulate. T03 must hold the
//!   permit until publication (or otherwise bound the parked set) and turn these
//!   probes GREEN.
//! * There is no byte ceiling on retention: retained bytes are the sum of full
//!   `ParseDocumentOutcome`s (with `ParsedArticle`) and grow linearly with
//!   document size at a constant slot count. T03 must add a measured byte
//!   ceiling (or prove the slot bound implies one).
//! * The sink sees no release, so a `peak_retained_bytes`-style sink sum is a
//!   CUMULATIVE total, never a window peak. T03 must expose a real
//!   window-retention observation.
//! * `ParseBatchRunSummary` publishes no latency tail, no peak retained bytes,
//!   no RSS, no Java CPU and no per-document deadline. T03 must publish at
//!   least the latency tails and the retention observation.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use da_parse::{
    CacheMode, ParseArtifactCache, ParseBackend, ParseBackendFailure, ParseBackendRequest,
    ParseBatch, ParseBatchExecutor, ParseBatchLimits, ParseBatchMemoryCheckpoint,
    ParseBatchResultSink, ParseBatchRunSummary, ParseBatchSinkError, ParseCacheFailure,
    RawStageRead, TeiEnvelope, TeiIdentity,
};
use tokio::sync::{Mutex, Notify};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bounds are nonzero")
}

/// The 29 keys `ParseBatchRunSummary::serialize` writes today (in wire order).
/// `cancelled` is deliberately absent: the serializer never writes it.
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

/// Probe D: the summary must not publish a latency tail, a byte ceiling, an
/// RSS/CPU observation or a per-document deadline. Returns every wire key it
/// does publish, for the report line.
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
    ParseBatch::admit(
        CacheMode::Off,
        ParseBatchLimits::new(
            nz(workers),
            nz(queue_capacity),
            nz(max_in_flight),
            nz(count),
        ),
        requests(prefix, count),
        None,
    )
    .expect("fixture admits")
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

    fn peak_cumulative_bytes(&self) -> usize {
        self.peak_cumulative_bytes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ParseBatchResultSink for MeasuringSink {
    async fn record(
        &self,
        outcome: da_parse::ParseDocumentOutcome,
    ) -> Result<(), ParseBatchSinkError> {
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
    async fn record(
        &self,
        outcome: da_parse::ParseDocumentOutcome,
    ) -> Result<(), ParseBatchSinkError> {
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

async fn run_blocked(
    body_chars: usize,
    count: usize,
    window: usize,
    workers: usize,
    queue_capacity: usize,
    max_in_flight: usize,
) -> (ParseBatchRunSummary, Arc<MeasuringSink>, usize, usize) {
    let backend = MeasuredBackend::new(body_chars);
    let executor =
        ParseBatchExecutor::new(backend.clone(), MissCache).with_publication_window(nz(window));
    let batch = admitted_batch("measure", count, workers, queue_capacity, max_in_flight);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let run = tokio::spawn({
        let sink = sink.clone();
        async move { executor.run(batch, sink, checkpoint).await }
    });
    backend.state.frontier_started.notified().await;
    settle_quiet(&backend.state.calls, 8, "backend calls before release").await;
    let calls_before_release = backend.calls();
    let sink_len_before_release = sink.ordinals().await.len();
    let summary = {
        backend.release_frontier();
        run.await.expect("executor task completes")
    };
    (summary, sink, calls_before_release, sink_len_before_release)
}

// ---------------------------------------------------------------------------
// Probe A — slot shell + how far the publication window actually reaches
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_publication_window_shell_under_a_blocked_frontier() {
    const WINDOW: usize = 3;
    const COUNT: usize = 6;
    let (summary, sink, calls_before_release, sink_len_before_release) =
        run_blocked(64, COUNT, WINDOW, 3, 3, 3).await;
    assert_summary_wire_shape(&summary);

    let ordinals = sink.ordinals().await;
    println!(
        "[S06/T02 probe A] window={WINDOW} admitted={COUNT} backend_calls_before_release={calls_before_release} \
sink_len_before_release={sink_len_before_release} max_publication_outstanding={} max_reorder_depth={} \
max_in_flight={} max_queue_depth={} configured_publication={} configured_execution={} budget_waits={} \
cursor_next={}",
        summary.max_publication_outstanding_observed(),
        summary.max_reorder_depth_observed(),
        summary.max_in_flight_observed(),
        summary.max_queue_depth_observed(),
        summary.configured_publication_capacity(),
        summary.configured_execution_capacity(),
        summary.budget_waits(),
        summary.cursor().next_ordinal(),
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

    // Nothing is published while the frontier ordinal is blocked, so the sink
    // has observed no release at all.
    assert_eq!(
        sink_len_before_release, 0,
        "no outcome may be published while ordinal 0 holds the frontier"
    );

    // MEASURED truth, and the plan's expected bound named explicitly:
    // the publication window does NOT bound retained outcomes under a blocked
    // frontier. Its permit is released when a worker finishes COMPUTATION, so
    // parked outcomes accumulate up to the whole admitted batch. Plan item 3
    // expected `backend_calls == publication_window` ({WINDOW}); measured
    // admission reaches the entire batch ({COUNT}).
    assert_eq!(
        calls_before_release, COUNT,
        "MEASURED PRE-FIX SNAPSHOT: admission reaches the whole batch while the finite publication \
         window is held by the blocked frontier; the plan expected `backend_calls == window` ({WINDOW})"
    );
    assert!(
        summary.max_publication_outstanding_observed() <= COUNT,
        "retained outcomes are bounded only by the admitted batch, not by the window"
    );
    assert!(
        summary.max_publication_outstanding_observed() > WINDOW,
        "FATAL: expected the measured retained-outcome count to exceed the publication window \
         (plan item 3 expected `<= window`); measured {} vs window {WINDOW}",
        summary.max_publication_outstanding_observed()
    );
    assert!(
        summary.max_reorder_depth_observed() >= summary.max_publication_outstanding_observed() - 1,
        "reorder depth tracks the parked outcome count"
    );
    assert!(
        summary.max_reorder_depth_observed() > WINDOW - 1,
        "FATAL: expected the measured reorder depth to exceed `window - 1`; measured {}",
        summary.max_reorder_depth_observed()
    );
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

    let (small_summary, small_sink, _, _) = run_blocked(SMALL_BODY, COUNT, WINDOW, 3, 3, 3).await;
    let (large_summary, large_sink, _, _) = run_blocked(LARGE_BODY, COUNT, WINDOW, 3, 3, 3).await;
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
        "[S06/T02 probe B] window={WINDOW} admitted={COUNT} small_body={SMALL_BODY} large_body={LARGE_BODY} \
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

    // H05-N2: bytes grow with document size at a CONSTANT slot count.
    assert!(
        large_bytes > small_bytes,
        "retained bytes must grow with document size"
    );
    // Every outcome is retained in full: no truncation, no ceiling. The delta
    // is exactly the added body characters per document (bodies are plain
    // `x` runs, so JSON encoding adds no escapes).
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
// Probe C — the cumulative sink sum is not a window peak
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_sink_cumulative_bytes_is_not_a_window_peak() {
    // Fast frontier: outcomes publish as they complete, so the live retained
    // set stays far below the total. The sink still sums everything.
    let executor = ParseBatchExecutor::new(MeasuredBackend::fast(64), MissCache)
        .with_publication_window(nz(1));
    let batch = admitted_batch("measure", 4, 4, 4, 4);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let summary = executor.run(batch, sink.clone(), checkpoint).await;
    assert_summary_wire_shape(&summary);

    let encoded = sink.encoded().await;
    let total = encoded.iter().map(|(_, len)| *len).sum::<usize>();
    println!(
        "[S06/T02 probe C] admitted={} cumulative_sink_bytes={} peak_cumulative_sink_bytes={} \
sum_of_all_outcomes={} max_publication_outstanding={} max_reorder_depth={}",
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
    // The cumulative total covers every outcome, while the retained window is
    // smaller: the sink never observes release, so it cannot report a peak.
    assert!(
        summary.max_publication_outstanding_observed() < summary.admitted() as usize,
        "the live retained set is smaller than the cumulative total"
    );
    assert!(
        sink.cumulative_bytes() > summary.max_publication_outstanding_observed() * encoded[0].1,
        "the cumulative sink sum overstates any per-window retained-bytes peak"
    );
}

// ---------------------------------------------------------------------------
// Probe D — latency tails, retention ceiling and deadline are not published
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn measured_latency_tails_and_retention_ceiling_are_not_observable() {
    const COUNT: usize = 4;
    let backend = SleepingBackend {
        step: Duration::from_millis(25),
    };
    let executor = ParseBatchExecutor::new(backend, MissCache).with_publication_window(nz(COUNT));
    let batch = admitted_batch("measure", COUNT, COUNT, COUNT, COUNT);
    let sink = Arc::new(MeasuringSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let started = tokio::time::Instant::now();
    let summary = executor.run(batch, sink.clone(), checkpoint).await;
    let elapsed = started.elapsed();
    assert_summary_wire_shape(&summary);

    // Per-document durations demonstrably differ (ordinal 0 is instant, the
    // rest sleep 25ms * ordinal on the paused logical clock).
    let published_keys = assert_no_latency_ceiling_or_deadline_surface(&summary);
    println!(
        "[S06/T02 probe D] admitted={COUNT} logical_elapsed_ms={} summary_wire_keys={} \
latency_or_ceiling_fields=0",
        elapsed.as_millis(),
        published_keys.len(),
    );

    assert!(
        elapsed >= Duration::from_millis(25 * (COUNT as u64 - 1)),
        "the run duration is measurable and differs across documents"
    );
    // No latency tail, no peak retained bytes, no RSS/CPU, no deadline: the
    // recursive forbidden-key scan above found nothing, and the wire carries
    // no more keys than the known shape (top-level + nested limits/cursor).
    assert!(published_keys.len() >= RUN_SUMMARY_WIRE_KEYS.len());
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
        "[S06/T02 probe E1] admitted={} succeeded={} failed={} backend_calls={} cursor_next={} ordinals={:?}",
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
        "[S06/T02 probe E2] admitted={} succeeded={} sink_errors={} cursor_next={} recorded={:?}",
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
