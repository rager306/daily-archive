//! S06/T05 (M511-j7wz2f) — MEASURED batch-budget load shadow harness (H05/H19).
//!
//! The test surface is not runtime proof. This example drives the REAL
//! [`da_parse::ParseBatchExecutor`] (no mock backend, no mock executor) under
//! two CONCURRENT batches with a slow ordinal-0 in each, sharing ONE
//! process-local [`ParseBatchBudget`], and emits
//! `artifacts/parse_compat/parse_batch_budget_shadow.json`.
//!
//! Substrate: a deterministic local HTTP stub over `tokio::net::TcpListener`
//! (the pattern of `crates/da-cli/tests/parse_batch_test.rs`). A live Java
//! GROBID is deliberately NOT required, and the artifact says so: no Java
//! process participated, so the Java-CPU ceiling is a typed `not_observed`
//! with a named reason — never a fabricated `0` (D430, H19-N1).
//!
//! Readers are INJECTED, exactly as in `crates/da-cli/examples/native_core_smp.rs`:
//!
//! * peak RSS — `FnMut() -> Option<u64>` reading `/proc/self/status` `VmHWM`;
//! * Java CPU — `FnMut() -> Option<u64>` reading the GROBID sidecar's cgroup
//!   `cpuacct` counter (`PARSE_BATCH_JAVA_CPUACCT`); absent/unreadable/zero is
//!   a typed absence with a named reason, never `0`.
//!
//! The artifact carries the GAR-006 reference/executed/evaluated split (it is a
//! shadow report, so the split is mandatory, not optional), the closed
//! `batch_budget_ceilings` metric set bound to the live `da_parse` DATA
//! constants, per-batch typed observations, a closed verdict, an honest
//! `h05_closure`/`h05_reason`, and every policy lock.
//!
//! Fail-closed in the harness itself (not only in the test):
//!
//! * a zero/unavailable RSS or Java-CPU value is never published as a measurement;
//! * a `not_observed` without a reason is rejected;
//! * a `not_observed` that carries a numeric field is rejected;
//! * non-additive retained-byte accounting (window peak below the sum of the
//!   known outcome sizes of that batch) is rejected;
//! * an empty or drifted metric set is rejected;
//! * a missing GAR-006 split is rejected;
//! * a lifted policy lock is rejected;
//! * `h05_closure == complete` without observed RSS + Java CPU + tails is rejected;
//! * a forbidden token (raw payload, physical path, host identity, executable)
//!   in the serialized report is rejected.
//!
//! `--self-check` runs the real measurement, validates it, and then proves every
//! one of those guards fires on a deliberately corrupted copy. It writes
//! nothing: the only filesystem write this example ever performs is the
//! operator-declared `--report` path.
//!
//! Locks hold throughout: `import_eligible=false`, `deployed=false`,
//! `production_route=forbidden`, `graph_write=forbidden`,
//! `auto_promote=forbidden`, D127 not lifted. No production reachability is
//! claimed.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use da_parse::{
    BATCH_BUDGET_H05_CLOSURE, BATCH_BUDGET_METRICS, BATCH_BUDGET_VERDICTS,
    BATCH_OBSERVATION_STATES, BatchBudgetVerdict, BatchObservation, CacheMode, GrobidHttpBackend,
    ParseArtifactCache, ParseBackend, ParseBatch, ParseBatchBudget, ParseBatchBudgetObservation,
    ParseBatchExecutor, ParseBatchLimits, ParseBatchMemoryCheckpoint, ParseBatchResultSink,
    ParseBatchRunSummary, ParseBatchSinkError, ParseCacheFailure, ParseDocumentFailure,
    ParseDocumentOutcome, RawStageRead,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify};

/// Canonical artifact path (the `--report` default).
const DEFAULT_REPORT: &str = "artifacts/parse_compat/parse_batch_budget_shadow.json";

/// Artifact schema version.
const REPORT_SCHEMA_VERSION: &str = "parse-batch-budget-shadow.v1";

/// GAR-006 reference-population role literal.
const REFERENCE_POPULATION_ROLE: &str = "identity_only";

/// Declared shared budget: six admitted results and four simultaneously
/// executing documents across BOTH batches. Four is chosen deliberately: with
/// two slow ordinal-0 documents holding one execution permit each, at least two
/// permits stay available to the fast documents, so the batch gate below can
/// never form a wait cycle. Six documents contend for four slots, so the shared
/// bound is real and the two batches are provably in flight together.
const SHARED_PUBLICATION_CAPACITY: usize = 6;
const SHARED_EXECUTION_CAPACITY: usize = 4;

/// Documents per batch.
const DOCUMENTS_PER_BATCH: usize = 3;

/// The deliberate slow ordinal-0 is DETERMINISTIC, not clock-raced: the stub
/// holds ordinal 0's response until every non-slow sibling of its batch has
/// been answered, and only then settles for this long so the client has time to
/// materialize those siblings' outcomes. Ordinal 0 therefore holds the
/// strict-ordinal frontier while the rest of its batch is already retained —
/// exactly the H05 scenario, by construction rather than by a race.
const SLOW_ORDINAL_ZERO_SETTLE: Duration = Duration::from_millis(80);

/// Declared retained-byte ceiling (generous: the measurement must OBSERVE a peak
/// under the ceiling, not trigger refusals).
const RETAINED_BYTE_CEILING: usize = 262_144;

/// A zero is not a measurement for these process metrics (H19-N1).
const PROCESS_METRICS: [&str; 2] = ["peak_rss_bytes", "java_cpu_wall_clock_ns"];

/// The three measurements H05 closure requires (D430).
const H05_REQUIRED_MEASUREMENTS: [&str; 3] =
    ["peak_rss_bytes", "java_cpu_wall_clock_ns", "latency_tails"];

/// The three latency-tail metric names.
const LATENCY_TAIL_METRICS: [&str; 3] = ["latency_p50_ms", "latency_p95_ms", "latency_p99_ms"];

/// Redaction denylist for the SERIALIZED report (defence in depth; the primary
/// control is that no producer ever writes these).
const FORBIDDEN_REPORT_TOKENS: [&str; 11] = [
    "<TEI",
    "<tei",
    "%PDF-",
    "data/sources",
    "file://",
    "/root/",
    "/home/",
    "target/debug",
    "target/release",
    "executable",
    "hostname",
];

/// Named reason for an absent process-RSS measurement.
const RSS_ABSENCE_REASON: &str = "process high-water RSS was unreadable or zero; /proc/self/status VmHWM yielded no positive byte count";

/// Named reason for an absent Java-CPU measurement when no Java process ran.
const JAVA_CPU_NO_PROCESS_REASON: &str =
    "measured substrate is the local deterministic stub; no Java process participated";

/// Named reason for an absent Java-CPU measurement when a counter was declared
/// but did not advance.
const JAVA_CPU_NO_DELTA_REASON: &str =
    "the declared Java cgroup cpuacct counter did not advance across the measured run";

/// Named reason for an absent latency tail when nothing reached a terminal outcome.
const NO_LATENCY_REASON: &str = "no document reached a terminal outcome in this run";

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("declared load bounds are nonzero")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Synthetic source bytes. The marker travels inside the multipart body, which
/// is how the stub dispatches a response to a document (order-independent under
/// concurrency). Only the marker is ever published — never these bytes.
fn pdf_bytes(marker: &str) -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    bytes.extend_from_slice(marker.as_bytes());
    bytes.extend_from_slice(b"\n%%EOF\n");
    bytes
}

/// A liftable TEI body whose size is driven by `filler_words`, so the two
/// batches carry payloads of clearly different sizes.
fn tei_fixture(title: &str, filler_words: usize) -> String {
    let filler = "filler ".repeat(filler_words);
    format!(
        r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc><titleStmt><title>Synthetic {title}</title></titleStmt></fileDesc></teiHeader><text><body><div><head n="1">INTRODUCTION</head><p>{filler}</p></div></body><back><div type="references"><listBibl><biblStruct><analytic><title level="a" type="main">Citation {title}</title></analytic><idno type="DOI">10.1000/{title}</idno></biblStruct></listBibl></div></back></text></TEI>"#
    )
}

// ───────────────────────── deterministic local stub ─────────────────────────

/// One synthetic document the stub can answer for.
#[derive(Clone)]
struct StubDocument {
    marker: &'static str,
    /// Batch gate group: slow documents wait for their group's fast siblings.
    group: &'static str,
    slow: bool,
    tei: String,
}

/// Deterministic batch gate for the slow ordinal-0 response.
struct StubGate {
    answered: Mutex<HashMap<&'static str, usize>>,
    notify: Notify,
    fast_targets: HashMap<&'static str, usize>,
    settle: Duration,
}

async fn spawn_stub(documents: Vec<StubDocument>, settle: Duration) -> SocketAddr {
    let mut fast_targets: HashMap<&'static str, usize> = HashMap::new();
    for document in &documents {
        if !document.slow {
            *fast_targets.entry(document.group).or_insert(0) += 1;
        }
    }
    let gate = Arc::new(StubGate {
        answered: Mutex::new(HashMap::new()),
        notify: Notify::new(),
        fast_targets,
        settle,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind deterministic loopback stub");
    let address = listener.local_addr().expect("stub address");
    let documents = Arc::new(documents);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let documents = Arc::clone(&documents);
            let gate = Arc::clone(&gate);
            tokio::spawn(async move { serve_stub(stream, documents, gate).await });
        }
    });
    address
}

/// Wait until every non-slow sibling of `group` has been answered.
///
/// The `notified()` future is created BEFORE the count is read, so a wakeup
/// between the read and the await can never be missed.
async fn wait_for_group(gate: &StubGate, group: &'static str) {
    let target = gate.fast_targets.get(group).copied().unwrap_or(0);
    loop {
        let notified = gate.notify.notified();
        {
            let answered = gate.answered.lock().await;
            if answered.get(group).copied().unwrap_or(0) >= target {
                return;
            }
        }
        notified.await;
    }
}

async fn mark_answered(gate: &StubGate, group: &'static str) {
    {
        let mut answered = gate.answered.lock().await;
        *answered.entry(group).or_insert(0) += 1;
    }
    gate.notify.notify_waiters();
}

async fn serve_stub(
    mut stream: tokio::net::TcpStream,
    documents: Arc<Vec<StubDocument>>,
    gate: Arc<StubGate>,
) {
    let Some((path, body)) = read_http_request(&mut stream).await else {
        return;
    };
    // The production identity probe is answered for realism; the executor never
    // calls it (the batch is admitted with an explicit backend version).
    if path.starts_with("/api/version") {
        write_http(
            &mut stream,
            200,
            "OK",
            "application/json",
            br#"{"name":"GROBID","version":"0.9.0-crf"}"#,
        )
        .await;
        return;
    }
    let document = documents
        .iter()
        .find(|document| contains(&body, document.marker.as_bytes()))
        .cloned();
    let Some(document) = document else {
        write_http(&mut stream, 500, "Internal Server Error", "text/plain", b"").await;
        return;
    };
    if document.slow {
        // Deterministic hold: the frontier document is answered only after its
        // batch's other documents have been answered, plus a settle margin for
        // the client to materialize their outcomes.
        wait_for_group(&gate, document.group).await;
        tokio::time::sleep(gate.settle).await;
    }
    write_http(
        &mut stream,
        200,
        "OK",
        "application/xml",
        document.tei.as_bytes(),
    )
    .await;
    if !document.slow {
        mark_answered(&gate, document.group).await;
    }
}

async fn write_http(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body).await;
    let _ = stream.shutdown().await;
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> Option<(String, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(position) = find(&buf, b"\r\n\r\n") {
            break position + 4;
        }
        if buf.len() > (16 << 20) {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?.to_string();
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let mut content_length = 0usize;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => content_length = value.trim().parse().unwrap_or(0),
            "transfer-encoding" if value.trim().eq_ignore_ascii_case("chunked") => chunked = true,
            _ => {}
        }
    }
    let mut body = buf.split_off(head_end);
    if chunked {
        while !body.ends_with(b"0\r\n\r\n") {
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
            if body.len() > (16 << 20) {
                break;
            }
        }
        body = decode_chunked(&body);
    } else {
        while body.len() < content_length {
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(content_length.min(body.len()));
    }
    Some((path, body))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find(haystack, needle).is_some()
}

fn decode_chunked(mut input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    while let Some(line_end) = find(input, b"\r\n") {
        let size_text = String::from_utf8_lossy(&input[..line_end]).to_string();
        let size = usize::from_str_radix(size_text.trim(), 16).unwrap_or(0);
        input = &input[line_end + 2..];
        if size == 0 {
            break;
        }
        let take = size.min(input.len());
        decoded.extend_from_slice(&input[..take]);
        input = &input[take..];
        if let Some(skip) = find(input, b"\r\n") {
            input = &input[skip + 2..];
        } else {
            break;
        }
    }
    decoded
}

// ───────────────────────── injected readers ─────────────────────────

/// Process high-water RSS in bytes, or `None` when it cannot be read.
///
/// `VmHWM` is a high-water mark, so reading it once after the load yields the
/// peak of the whole run. A zero is treated as unreadable by the caller.
fn read_process_peak_rss() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kilobytes: u64 = line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())?;
    Some(kilobytes.saturating_mul(1024))
}

/// Production Java-CPU reader: the GROBID sidecar's cgroup `cpuacct` counter,
/// declared through `PARSE_BATCH_JAVA_CPUACCT` (a cgroup file path or a
/// `docker stats`-derived counter file). Absent/unreadable is a typed absence.
fn read_java_cpu_counter() -> Option<u64> {
    let path = std::env::var("PARSE_BATCH_JAVA_CPUACCT").ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

/// An injected `FnMut() -> Option<u64>` reader that counts its own invocations,
/// so the artifact can report how often the seam was exercised instead of
/// asserting it. The closure seam is exactly the one
/// `crates/da-cli/examples/native_core_smp.rs` uses.
struct CountingReader<F: FnMut() -> Option<u64>> {
    read: F,
    calls: u32,
}

impl<F: FnMut() -> Option<u64>> CountingReader<F> {
    fn new(read: F) -> Self {
        Self { read, calls: 0 }
    }

    fn read(&mut self) -> Option<u64> {
        self.calls += 1;
        (self.read)()
    }

    fn calls(&self) -> u32 {
        self.calls
    }
}

// ───────────────────────── sink and cache seams ─────────────────────────

/// One terminal outcome, reduced to identity, type and size — never payload.
#[derive(Debug, Clone)]
struct OutcomeRecord {
    ordinal: u32,
    succeeded: bool,
    failure_code: Option<String>,
    outcome_bytes: usize,
}

#[derive(Default)]
struct RecordingSink {
    records: Mutex<Vec<OutcomeRecord>>,
}

#[async_trait]
impl ParseBatchResultSink for RecordingSink {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        // The same honest serde length the executor's retention gauge reserves.
        let encoded = serde_json::to_vec(&outcome)
            .map_err(|error| ParseBatchSinkError(format!("outcome encoding failed: {error}")))?;
        let record = OutcomeRecord {
            ordinal: outcome.ordinal(),
            succeeded: outcome.is_success(),
            failure_code: match outcome.result() {
                Ok(_) => None,
                Err(ParseDocumentFailure::Backend(failure)) => {
                    Some(failure.code().as_str().to_string())
                }
                Err(ParseDocumentFailure::Cache(_)) => Some("cache".to_string()),
            },
            outcome_bytes: encoded.len(),
        };
        self.records.lock().await.push(record);
        Ok(())
    }
}

/// Always-miss, never-write cache: the measurement is about the executor's
/// retention, not about cache behaviour.
#[derive(Debug, Default)]
struct NoCache;

#[async_trait]
impl ParseArtifactCache for NoCache {
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

// ───────────────────────── the measured load ─────────────────────────

/// One batch of the declared concurrent load.
struct BatchSpec {
    id: &'static str,
    payload_label: &'static str,
    documents: Vec<StubDocument>,
}

struct DocumentIdentity {
    ordinal: u32,
    document_id: String,
    input_sha256: String,
}

struct BatchRun {
    id: &'static str,
    payload_label: &'static str,
    limits: ParseBatchLimits,
    summary: ParseBatchRunSummary,
    observation: ParseBatchBudgetObservation,
    identities: Vec<DocumentIdentity>,
    records: Vec<OutcomeRecord>,
}

impl BatchRun {
    fn peak_retained_bytes(&self) -> Option<u64> {
        self.observation.peak_retained_bytes().value().copied()
    }

    /// Sum of the KNOWN sizes of this batch's successful outcomes — the
    /// additivity reference the window peak can never fall below.
    fn known_window_success_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| record.succeeded)
            .map(|record| record.outcome_bytes as u64)
            .sum()
    }
}

/// The declared concurrent load: two batches with a deterministic slow
/// ordinal-0 each and payloads of clearly different sizes.
fn load_specs() -> Vec<BatchSpec> {
    let document = |group: &'static str,
                    marker: &'static str,
                    ordinal: usize,
                    filler_words: usize| StubDocument {
        marker,
        group,
        slow: ordinal == 0,
        tei: tei_fixture(marker, filler_words),
    };
    vec![
        BatchSpec {
            id: "A",
            payload_label: "small_synthetic_tei_body",
            documents: vec![
                document("A", "SYNTH-A-0-ZX", 0, 40),
                document("A", "SYNTH-A-1-ZX", 1, 40),
                document("A", "SYNTH-A-2-ZX", 2, 40),
            ],
        },
        BatchSpec {
            id: "B",
            payload_label: "large_synthetic_tei_body",
            documents: vec![
                document("B", "SYNTH-B-0-ZX", 0, 900),
                document("B", "SYNTH-B-1-ZX", 1, 900),
                document("B", "SYNTH-B-2-ZX", 2, 900),
            ],
        },
    ]
}

async fn run_batch(
    spec: &BatchSpec,
    address: SocketAddr,
    budget: Arc<ParseBatchBudget>,
) -> Result<BatchRun, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{address}");
    let backend = GrobidHttpBackend::new(Some(&endpoint)).with_timeout(Duration::from_secs(30));
    let requests = spec
        .documents
        .iter()
        .map(|document| backend.default_request(document.marker, pdf_bytes(document.marker)))
        .collect::<Vec<_>>();
    let identities = spec
        .documents
        .iter()
        .enumerate()
        .map(|(ordinal, document)| DocumentIdentity {
            ordinal: ordinal as u32,
            document_id: document.marker.to_string(),
            input_sha256: sha256_hex(&pdf_bytes(document.marker)),
        })
        .collect();
    let limits = ParseBatchLimits::new(
        nz(spec.documents.len()),
        nz(spec.documents.len()),
        nz(spec.documents.len()),
        nz(spec.documents.len()),
    )
    .with_max_retained_bytes(nz(RETAINED_BYTE_CEILING));
    let batch = ParseBatch::admit(CacheMode::Off, limits, requests, None)
        .map_err(|error| format!("batch {} admission failed: {error:?}", spec.id))?;
    let sink = Arc::new(RecordingSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let executor = ParseBatchExecutor::new(backend, NoCache)
        .with_shared_budget(budget)
        .with_publication_window(nz(spec.documents.len()));
    let (summary, observation) = executor.run_observed(batch, sink.clone(), checkpoint).await;
    let records = sink.records.lock().await.clone();
    Ok(BatchRun {
        id: spec.id,
        payload_label: spec.payload_label,
        limits,
        summary,
        observation,
        identities,
        records,
    })
}

/// Aggregated observation across the concurrent batches: the maximum observed
/// value (`max_across_batches`). A metric no batch observed stays a typed
/// absence with the supplied reason.
fn max_observed(
    left: &BatchObservation<u64>,
    right: &BatchObservation<u64>,
    absence_reason: &str,
) -> BatchObservation<u64> {
    match (left.value(), right.value()) {
        (Some(left), Some(right)) => BatchObservation::observed(*left.max(right)),
        (Some(value), None) | (None, Some(value)) => BatchObservation::observed(*value),
        (None, None) => BatchObservation::not_observed(absence_reason),
    }
}

fn observation_json<T: Serialize>(
    observation: &BatchObservation<T>,
) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::to_value(observation)?)
}

fn measured_batch(run: &BatchRun) -> Result<Value, Box<dyn std::error::Error>> {
    let execution = json!({
        "admitted": run.summary.admitted(),
        "completed": run.summary.completed(),
        "succeeded": run.summary.succeeded(),
        "failed": run.summary.failed(),
        "backend_calls": run.summary.backend_calls(),
        "cursor_next_ordinal": run.summary.cursor().next_ordinal(),
        "max_in_flight_observed": run.summary.max_in_flight_observed(),
        "max_queue_depth_observed": run.summary.max_queue_depth_observed(),
        "max_publication_outstanding_observed": run.summary.max_publication_outstanding_observed(),
        "max_reorder_depth_observed": run.summary.max_reorder_depth_observed(),
        "budget_waits": run.summary.budget_waits(),
        "sink_errors": run.summary.sink_errors(),
        "checkpoint_errors": run.summary.checkpoint_errors(),
        "configured_publication_capacity": run.summary.configured_publication_capacity(),
        "configured_execution_capacity": run.summary.configured_execution_capacity(),
        "budget_scope": run.summary.budget_scope(),
    });
    Ok(json!({
        "id": run.id,
        "payload_profile": run.payload_label,
        "documents": run.identities.len(),
        "limits": {
            "workers": run.limits.workers().get(),
            "queue_capacity": run.limits.queue_capacity().get(),
            "max_in_flight": run.limits.max_in_flight().get(),
            "max_documents": run.limits.max_documents().get(),
            "max_retained_bytes": run.limits.max_retained_bytes().map(NonZeroUsize::get),
        },
        "execution": execution,
        "budget_observation": serde_json::to_value(&run.observation)?,
        "retained_window_success_bytes": run.known_window_success_bytes(),
    }))
}

/// Run the declared load and assemble the artifact. Fails closed on any
/// additivity or observation violation.
async fn measure() -> Result<Value, Box<dyn std::error::Error>> {
    let specs = load_specs();
    let stub_documents = specs
        .iter()
        .flat_map(|spec| spec.documents.iter().cloned())
        .collect();
    let address = spawn_stub(stub_documents, SLOW_ORDINAL_ZERO_SETTLE).await;
    let budget = Arc::new(ParseBatchBudget::new(
        nz(SHARED_PUBLICATION_CAPACITY),
        nz(SHARED_EXECUTION_CAPACITY),
    ));

    // Injected readers (`FnMut() -> Option<u64>`), each counting its own
    // invocations so the artifact reports how often the seam was exercised
    // rather than asserting it. Peak RSS is a high-water mark, so one read
    // after the load suffices; the Java-CPU counter is a monotonic total, so it
    // needs a before/after delta.
    let mut rss_reader = CountingReader::new(read_process_peak_rss);
    let mut java_cpu_reader = CountingReader::new(read_java_cpu_counter);
    let java_cpu_before = java_cpu_reader.read();

    let (left, right) = tokio::join!(
        run_batch(&specs[0], address, Arc::clone(&budget)),
        run_batch(&specs[1], address, Arc::clone(&budget)),
    );
    let left = left?;
    let right = right?;
    let runs = [&left, &right];

    let rss_after = rss_reader.read();
    let java_cpu_after = java_cpu_reader.read();

    // ── fail-closed: retained-byte accounting must be additive ──────────
    for run in runs {
        let known = run.known_window_success_bytes();
        let peak = run.peak_retained_bytes().ok_or_else(|| {
            format!(
                "FATAL: batch {} did not observe peak_retained_bytes, so no additive accounting can be claimed",
                run.id
            )
        })?;
        if peak < known {
            return Err(format!(
                "FATAL: non-additive retained-byte accounting in batch {}: window peak {peak} < known window success bytes {known}",
                run.id
            )
            .into());
        }
    }

    // ── typed process observations: zero/unavailable is NEVER a value ───
    let rss_observation = match rss_after {
        Some(bytes) if bytes > 0 => BatchObservation::observed(bytes),
        _ => BatchObservation::not_observed(RSS_ABSENCE_REASON),
    };
    let java_cpu_observation = match (java_cpu_before, java_cpu_after) {
        (Some(before), Some(after)) if after > before => BatchObservation::observed(after - before),
        _ if std::env::var("PARSE_BATCH_JAVA_CPUACCT").is_ok() => {
            BatchObservation::not_observed(JAVA_CPU_NO_DELTA_REASON)
        }
        _ => BatchObservation::not_observed(JAVA_CPU_NO_PROCESS_REASON),
    };

    // ── aggregated observations over the closed DATA metric set ─────────
    let mut observations = Map::new();
    let tails = [
        (
            "latency_p50_ms",
            max_observed(
                left.observation.latency_p50_ms(),
                right.observation.latency_p50_ms(),
                NO_LATENCY_REASON,
            ),
        ),
        (
            "latency_p95_ms",
            max_observed(
                left.observation.latency_p95_ms(),
                right.observation.latency_p95_ms(),
                NO_LATENCY_REASON,
            ),
        ),
        (
            "latency_p99_ms",
            max_observed(
                left.observation.latency_p99_ms(),
                right.observation.latency_p99_ms(),
                NO_LATENCY_REASON,
            ),
        ),
        (
            "peak_retained_bytes",
            max_observed(
                left.observation.peak_retained_bytes(),
                right.observation.peak_retained_bytes(),
                "no batch observed a retained-byte window peak",
            ),
        ),
        (
            "max_reorder_depth",
            max_observed(
                left.observation.max_reorder_depth(),
                right.observation.max_reorder_depth(),
                "no batch observed a reorder depth",
            ),
        ),
        (
            "max_publication_outstanding",
            max_observed(
                left.observation.max_publication_outstanding(),
                right.observation.max_publication_outstanding(),
                "no batch observed a publication outstanding count",
            ),
        ),
        (
            "max_in_flight",
            max_observed(
                left.observation.max_in_flight(),
                right.observation.max_in_flight(),
                "no batch observed an in-flight count",
            ),
        ),
        (
            "budget_waits",
            max_observed(
                left.observation.budget_waits(),
                right.observation.budget_waits(),
                "no batch observed a budget wait",
            ),
        ),
    ];
    for (name, observation) in tails {
        observations.insert(name.to_string(), observation_json(&observation)?);
    }
    observations.insert(
        "peak_rss_bytes".to_string(),
        observation_json(&rss_observation)?,
    );
    observations.insert(
        "java_cpu_wall_clock_ns".to_string(),
        observation_json(&java_cpu_observation)?,
    );

    // ── closed verdict, derived from the declared ceiling and the peak ──
    let verdict = if runs
        .iter()
        .any(|run| run.observation.verdict() == BatchBudgetVerdict::MemoryPressure)
    {
        BatchBudgetVerdict::MemoryPressure
    } else if runs
        .iter()
        .all(|run| run.observation.verdict() == BatchBudgetVerdict::WithinCeiling)
    {
        BatchBudgetVerdict::WithinCeiling
    } else {
        BatchBudgetVerdict::NotObserved
    };

    // ── H05 is never pre-claimed (D430) ─────────────────────────────────
    let rss_observed = rss_observation.is_observed();
    let java_cpu_observed = java_cpu_observation.is_observed();
    let tails_observed = observations
        .iter()
        .filter(|(name, _)| LATENCY_TAIL_METRICS.contains(&name.as_str()))
        .all(|(_, observation)| {
            observation.get("state").and_then(Value::as_str) == Some("observed")
        });
    let mut h05_required = Map::new();
    for (name, observed) in [
        ("peak_rss_bytes", rss_observed),
        ("java_cpu_wall_clock_ns", java_cpu_observed),
        ("latency_tails", tails_observed),
    ] {
        h05_required.insert(
            name.to_string(),
            json!(if observed { "observed" } else { "not_observed" }),
        );
    }
    let h05_closure = if rss_observed && java_cpu_observed && tails_observed {
        "complete"
    } else {
        "in_progress"
    };
    let h05_reason = if h05_closure == "complete" {
        "H05 closure condition met: process peak RSS, Java CPU and latency tails were all measured on concurrent batches with a slow ordinal-0".to_string()
    } else {
        let mut missing = Vec::new();
        if !rss_observed {
            missing.push(format!("peak_rss_bytes ({RSS_ABSENCE_REASON})"));
        }
        if !java_cpu_observed {
            missing.push(format!(
                "java_cpu_wall_clock_ns ({})",
                java_cpu_observation
                    .reason()
                    .unwrap_or(JAVA_CPU_NO_PROCESS_REASON)
            ));
        }
        if !tails_observed {
            missing.push(format!("latency_tails ({NO_LATENCY_REASON})"));
        }
        format!(
            "H05 stays in_progress: unmeasured closure requirement(s): {}",
            missing.join("; ")
        )
    };

    // ── flat terminal outcomes (identity, type and size only) ───────────
    let mut outcomes = Vec::new();
    for run in runs {
        for identity in &run.identities {
            let record = run
                .records
                .iter()
                .find(|record| record.ordinal == identity.ordinal);
            outcomes.push(json!({
                "batch": run.id,
                "ordinal": identity.ordinal,
                "document_id": identity.document_id,
                "input_sha256": identity.input_sha256,
                "status": match record {
                    Some(record) if record.succeeded => "succeeded",
                    Some(_) => "failed",
                    None => "not_terminal",
                },
                "failure_code": record.and_then(|record| record.failure_code.clone()),
                "outcome_bytes": record.map(|record| record.outcome_bytes),
            }));
        }
    }

    let succeeded: u64 = runs.iter().map(|run| run.summary.succeeded() as u64).sum();
    let failed: u64 = runs.iter().map(|run| run.summary.failed() as u64).sum();
    let terminal: u64 = runs.iter().map(|run| run.summary.completed() as u64).sum();
    let documents = (DOCUMENTS_PER_BATCH * runs.len()) as u64;

    let mut batches = Vec::new();
    for run in runs {
        batches.push(measured_batch(run)?);
    }

    Ok(json!({
        "schema_version": REPORT_SCHEMA_VERSION,
        "requirement_ids": ["R085", "R087", "R139"],
        "adr_ref": "ADR-076",
        "substrate": {
            "kind": "local_deterministic_loopback_stub",
            "live_grobid_participated": false,
            "java_process_participation_observed": java_cpu_observed,
            "deterministic_substrate": true,
            "determinism_note": "deterministic stub, deterministic sibling-answered gate on ordinal-0, deterministic slow-ordinal-0 settle; measured durations are wall-clock and therefore not byte-stable",
            "rss_scope": "process_peak_vmhwm_including_this_harness",
            "java_cpu_scope": "grobid_sidecar_cgroup_cpuacct",
            "measurement_note": "Both batches ran concurrently against one shared process-local budget over a loopback stub. Peak RSS is a process high-water mark and includes this harness; the Java CPU counter is not observed because no Java process participated.",
            "reader_invocations": {
                "peak_rss_bytes": rss_reader.calls(),
                "java_cpu_wall_clock_ns": java_cpu_reader.calls(),
            },
            "load": {
                "concurrent_batches": runs.len(),
                "documents_per_batch": DOCUMENTS_PER_BATCH,
                "slow_ordinal_zero_settle_ms": SLOW_ORDINAL_ZERO_SETTLE.as_millis(),
                "slow_ordinal_zero_gate": "sibling_responses_answered",
                "shared_budget": {
                    "publication_capacity": SHARED_PUBLICATION_CAPACITY,
                    "execution_capacity": SHARED_EXECUTION_CAPACITY,
                },
                "payload_profile": {
                    "batch_a": specs[0].payload_label,
                    "batch_b": specs[1].payload_label,
                },
            },
        },
        "reference_population": {
            "role": REFERENCE_POPULATION_ROLE,
            "identity": "synthetic_two_batch_slow_ordinal_zero_load",
            "documents": documents,
            "batches": runs.len(),
        },
        "executed_population": {
            "documents": documents,
            "corpus_execution": false,
            "execution_evidence": "offline_loopback_stub_existing_typed_transport:two_batch_slow_ordinal_zero",
            "batches": runs.len(),
            "terminal_outcomes": terminal,
            "succeeded": succeeded,
            "failed": failed,
        },
        "evaluated_population": {
            "documents": 0,
            "status": "not_evaluated",
            "reason": "local deterministic stub substrate; no corpus execution and therefore no evaluation claim",
            "aggregation": "max_across_batches",
            "measurements_observed": observations.values().filter(|observation| observation.get("state").and_then(Value::as_str) == Some("observed")).count(),
            "measurements_not_observed": observations.values().filter(|observation| observation.get("state").and_then(Value::as_str) == Some("not_observed")).count(),
        },
        "batches": batches,
        "outcomes": outcomes,
        "observations": Value::Object(observations),
        "verdict": verdict.as_str(),
        "h05_closure": h05_closure,
        "h05_reason": h05_reason,
        "h05_required_measurements": Value::Object(h05_required),
        "h05_design_closure": BATCH_BUDGET_H05_CLOSURE,
        "policy": {
            "observation_mode": "shadow",
            "promotion_state": "candidate_only",
            "import_eligible": false,
            "deployed": false,
            "production_route": "forbidden",
            "graph_write": "forbidden",
            "auto_promote": "forbidden",
            "d127_import_lock": "intact",
        },
        "redaction": {
            "raw_payload_published": false,
            "document_text_published": false,
            "physical_paths_published": false,
        },
    }))
}

// ───────────────────────── fail-closed validation ─────────────────────────

fn require_object<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing-object:{key}"))
}

fn require_str<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("missing-string:{key}"))
}

fn require_u64(object: &Map<String, Value>, key: &str) -> Result<u64, String> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing-count:{key}"))
}

fn observation_state<'a>(observation: &'a Value, name: &str) -> Result<&'a str, String> {
    observation
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("observation-state-missing:{name}"))
}

/// The single fail-closed gate. Every check names itself, so a report that
/// loses a guarantee fails visibly rather than silently.
fn validate_report(report: &Value) -> Result<(), String> {
    if report.as_object().is_none() {
        return Err("report-not-object".to_string());
    }
    let schema = require_str(report.as_object().expect("checked"), "schema_version")?;
    if schema != REPORT_SCHEMA_VERSION {
        return Err(format!("schema-version-drift:{schema}"));
    }

    // ── GAR-006: the reference/executed/evaluated split is mandatory ────
    let reference = require_object(report, "reference_population")?;
    let executed = require_object(report, "executed_population")?;
    let evaluated = require_object(report, "evaluated_population")?;
    if !report
        .get("outcomes")
        .is_some_and(|value| value.is_array() || value.is_object())
    {
        return Err("gar006-outcomes-missing".to_string());
    }
    if reference == executed || reference == evaluated || executed == evaluated {
        return Err("gar006-populations-not-distinguishable".to_string());
    }
    if require_str(reference, "role")? != REFERENCE_POPULATION_ROLE {
        return Err("gar006-reference-role-drift".to_string());
    }
    require_str(reference, "identity")?;
    let reference_documents = require_u64(reference, "documents")?;
    if executed
        .get("corpus_execution")
        .and_then(Value::as_bool)
        .is_none()
    {
        return Err("gar006-executed-corpus-execution-not-boolean".to_string());
    }
    require_str(executed, "execution_evidence")?;
    let executed_documents = require_u64(executed, "documents")?;
    require_str(evaluated, "status")?;
    let evaluated_documents = require_u64(evaluated, "documents")?;
    if report.get("denominators").is_some() || report.get("corpus").is_some() {
        return Err("gar006-legacy-block-present".to_string());
    }
    if executed_documents > reference_documents {
        return Err("gar006-executed-exceeds-reference".to_string());
    }
    if evaluated_documents > executed_documents {
        return Err("gar006-evaluated-exceeds-executed".to_string());
    }

    // ── the closed DATA metric set, typed ──────────────────────────────
    let observations = require_object(report, "observations")?;
    if observations.is_empty() {
        return Err("metric-set-empty".to_string());
    }
    let actual: Vec<&str> = observations.keys().map(String::as_str).collect();
    let missing: Vec<&str> = BATCH_BUDGET_METRICS
        .iter()
        .copied()
        .filter(|name| !actual.contains(name))
        .collect();
    let unexpected: Vec<&str> = actual
        .iter()
        .copied()
        .filter(|name| !BATCH_BUDGET_METRICS.contains(name))
        .collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(format!(
            "metric-set-drift: missing={missing:?} unexpected={unexpected:?}"
        ));
    }
    for (name, observation) in observations {
        let object = observation
            .as_object()
            .ok_or_else(|| format!("observation-not-object:{name}"))?;
        let state = observation_state(observation, name)?;
        if !BATCH_OBSERVATION_STATES.contains(&state) {
            return Err(format!("observation-state-unknown:{name}:{state}"));
        }
        match state {
            "observed" => {
                let value = object
                    .get("value")
                    .ok_or_else(|| format!("observed-without-value:{name}"))?;
                if value.as_u64().is_none() {
                    return Err(format!("observed-value-not-a-byte-count:{name}"));
                }
                if object.contains_key("reason") {
                    return Err(format!("observed-carries-reason:{name}"));
                }
                if PROCESS_METRICS.contains(&name.as_str()) && value.as_u64() == Some(0) {
                    return Err(format!("process-metric-zero-is-not-a-measurement:{name}"));
                }
            }
            "not_observed" => {
                if object.contains_key("value") {
                    return Err(format!("not-observed-carries-numeric-field:{name}"));
                }
                let reason = object
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if reason.is_empty() {
                    return Err(format!("not-observed-without-reason:{name}"));
                }
            }
            _ => return Err(format!("observation-state-unknown:{name}:{state}")),
        }
    }

    // ── additivity: the window peak can never fall below the known sizes ─
    let batches = report
        .get("batches")
        .and_then(Value::as_array)
        .ok_or_else(|| "batches-missing".to_string())?;
    if batches.is_empty() {
        return Err("batches-empty".to_string());
    }
    let outcomes = report
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or_else(|| "outcomes-not-array".to_string())?;
    let mut aggregate_peak: Option<u64> = None;
    for batch in batches {
        let object = batch
            .as_object()
            .ok_or_else(|| "batch-not-object".to_string())?;
        let id = require_str(object, "id")?;
        let peak = object
            .get("budget_observation")
            .and_then(|observation| observation.get("peak_retained_bytes"))
            .filter(|observation| {
                observation.get("state").and_then(Value::as_str) == Some("observed")
            })
            .and_then(|observation| observation.get("value"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("batch-peak-not-observed:{id}"))?;
        let known = object
            .get("retained_window_success_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("batch-known-bytes-missing:{id}"))?;
        let mut recomputed = 0u64;
        for outcome in outcomes {
            let outcome = outcome
                .as_object()
                .ok_or_else(|| "outcome-not-object".to_string())?;
            if outcome.get("batch").and_then(Value::as_str) != Some(id) {
                continue;
            }
            if outcome.get("status").and_then(Value::as_str) == Some("succeeded") {
                recomputed += outcome
                    .get("outcome_bytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("outcome-bytes-missing:{id}"))?;
            }
        }
        if recomputed != known {
            return Err(format!(
                "batch-known-bytes-not-additive:{id}: recorded={known} recomputed={recomputed}"
            ));
        }
        if peak < recomputed {
            return Err(format!(
                "non-additive-retained-bytes:{id}: peak={peak} known={recomputed}"
            ));
        }
        aggregate_peak = Some(aggregate_peak.map_or(peak, |current| current.max(peak)));
    }
    let declared_peak = observations
        .get("peak_retained_bytes")
        .and_then(|observation| observation.get("value"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "aggregate-peak-not-observed".to_string())?;
    if Some(declared_peak) != aggregate_peak {
        return Err(format!(
            "aggregate-peak-not-max-across-batches: declared={declared_peak} computed={aggregate_peak:?}"
        ));
    }

    // ── H05 is honest, and never pre-claimed ───────────────────────────
    let h05_closure = require_str(report.as_object().expect("checked"), "h05_closure")?;
    let required = require_object(report, "h05_required_measurements")?;
    for name in H05_REQUIRED_MEASUREMENTS {
        if !required.contains_key(name) {
            return Err(format!("h05-required-measurement-absent:{name}"));
        }
    }
    if h05_closure == "complete" {
        for name in H05_REQUIRED_MEASUREMENTS {
            if required.get(name).and_then(Value::as_str) != Some("observed") {
                return Err(format!("h05-complete-without-measurement:{name}"));
            }
        }
        for name in ["peak_rss_bytes", "java_cpu_wall_clock_ns"] {
            if observation_state(
                observations
                    .get(name)
                    .ok_or_else(|| format!("h05-complete-without-observation:{name}"))?,
                name,
            )? != "observed"
            {
                return Err(format!("h05-complete-without-measurement:{name}"));
            }
        }
        for name in LATENCY_TAIL_METRICS {
            if observation_state(
                observations
                    .get(name)
                    .ok_or_else(|| format!("h05-complete-without-observation:{name}"))?,
                name,
            )? != "observed"
            {
                return Err(format!("h05-complete-without-measurement:{name}"));
            }
        }
    } else if require_str(report.as_object().expect("checked"), "h05_reason").is_err() {
        return Err("h05-non-complete-without-reason".to_string());
    }
    if require_str(report.as_object().expect("checked"), "h05_design_closure")?
        != BATCH_BUDGET_H05_CLOSURE
    {
        return Err("h05-design-closure-not-bound-to-data".to_string());
    }

    // ── closed verdict ─────────────────────────────────────────────────
    let verdict = require_str(report.as_object().expect("checked"), "verdict")?;
    if !BATCH_BUDGET_VERDICTS.contains(&verdict) {
        return Err(format!("verdict-unknown:{verdict}"));
    }

    // ── every policy lock ──────────────────────────────────────────────
    let policy = require_object(report, "policy")?;
    for (key, expected) in [
        ("import_eligible", json!(false)),
        ("deployed", json!(false)),
        ("production_route", json!("forbidden")),
        ("graph_write", json!("forbidden")),
        ("auto_promote", json!("forbidden")),
        ("observation_mode", json!("shadow")),
        ("promotion_state", json!("candidate_only")),
    ] {
        if policy.get(key) != Some(&expected) {
            return Err(format!("policy-lock-drift:{key}"));
        }
    }

    // ── redaction markers and the serialized denylist scan ─────────────
    let redaction = require_object(report, "redaction")?;
    if redaction.is_empty() {
        return Err("redaction-empty".to_string());
    }
    for (key, value) in redaction {
        if value != &json!(false) {
            return Err(format!("redaction-marker-not-false:{key}"));
        }
    }
    let text = serde_json::to_string(report)
        .map_err(|error| format!("report-serialize-failed:{error}"))?;
    for token in FORBIDDEN_REPORT_TOKENS {
        if text.contains(token) {
            return Err(format!("redaction-forbidden-token:{token}"));
        }
    }

    Ok(())
}

// ───────────────────────── fail-closed self-check ─────────────────────────

type Mutation = (&'static str, fn(&mut Value), &'static str);

fn self_check_mutations() -> Vec<Mutation> {
    vec![
        (
            "zero-rss-published-as-measurement",
            |report: &mut Value| {
                report["observations"]["peak_rss_bytes"] = json!({"state": "observed", "value": 0});
            },
            "process-metric-zero-is-not-a-measurement",
        ),
        (
            "not-observed-without-reason",
            |report: &mut Value| {
                report["observations"]["java_cpu_wall_clock_ns"] = json!({"state": "not_observed"});
            },
            "not-observed-without-reason",
        ),
        (
            "not-observed-with-numeric-field",
            |report: &mut Value| {
                report["observations"]["java_cpu_wall_clock_ns"] =
                    json!({"state": "not_observed", "reason": "injected", "value": 0});
            },
            "not-observed-carries-numeric-field",
        ),
        (
            "non-additive-retained-bytes",
            |report: &mut Value| {
                report["batches"][0]["budget_observation"]["peak_retained_bytes"] =
                    json!({"state": "observed", "value": 1});
                report["observations"]["peak_retained_bytes"] =
                    json!({"state": "observed", "value": 1});
            },
            "non-additive-retained-bytes",
        ),
        (
            "incomplete-metric-set",
            |report: &mut Value| {
                if let Some(object) = report["observations"].as_object_mut() {
                    object.remove("budget_waits");
                }
            },
            "metric-set-drift",
        ),
        (
            "missing-gar006-split",
            |report: &mut Value| {
                if let Some(object) = report.as_object_mut() {
                    object.remove("executed_population");
                }
            },
            "missing-object:executed_population",
        ),
        (
            "forbidden-token-in-report",
            |report: &mut Value| {
                report["substrate"]["measurement_note"] =
                    json!("leaked file:///root/private/source.pdf");
            },
            "redaction-forbidden-token",
        ),
        (
            "h05-complete-without-java-cpu",
            |report: &mut Value| {
                report["h05_closure"] = json!("complete");
            },
            "h05-complete-without-measurement",
        ),
        (
            "policy-lock-lifted",
            |report: &mut Value| {
                report["policy"]["import_eligible"] = json!(true);
            },
            "policy-lock-drift:import_eligible",
        ),
    ]
}

/// Prove every fail-closed guard fires on a deliberately corrupted copy of the
/// REAL measured report. The uncorrupted report must pass first.
fn self_check_guards(report: &Value) -> Result<(), Box<dyn std::error::Error>> {
    validate_report(report).map_err(|finding| {
        format!("FATAL: the measured report was rejected by its own validator: {finding}")
    })?;
    let mut failures = Vec::new();
    let mutations = self_check_mutations();
    for (name, mutate, expected) in &mutations {
        let mut corrupted = report.clone();
        mutate(&mut corrupted);
        match validate_report(&corrupted) {
            Ok(()) => failures.push(format!("guard did not fire for mutation `{name}`")),
            Err(finding) if !finding.contains(expected) => failures.push(format!(
                "mutation `{name}` produced `{finding}`, expected `{expected}`"
            )),
            Err(finding) => println!("SELF-CHECK guard fired: {name} -> {finding}"),
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "FATAL: fail-closed self-check failed: {}",
            failures.join("; ")
        )
        .into());
    }
    println!(
        "SELF-CHECK OK: {} fail-closed guards fired on corrupted copies",
        mutations.len()
    );
    Ok(())
}

// ───────────────────────── CLI ─────────────────────────

struct Args {
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut report = PathBuf::from(DEFAULT_REPORT);
    let mut self_check = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--report" => {
                report = arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--report requires a path")?;
            }
            "--self-check" => self_check = true,
            "--help" => {
                println!(
                    "parse_batch_budget_shadow [--report FILE] [--self-check]\n\
                     \n\
                     Measures the batch budget under two concurrent batches with a slow\n\
                     ordinal-0 (shared budget, declared max_retained_bytes) and writes the\n\
                     GAR-006 shadow artifact. --self-check proves the fail-closed guards fire\n\
                     and writes nothing."
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }
    Ok(Args { report, self_check })
}

fn write_report(path: &Path, report: &Value) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(report)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, format!("{text}\n"))?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn print_summary(path: &Path, report: &Value) {
    let observations = &report["observations"];
    let value = |name: &str| {
        observations
            .get(name)
            .and_then(|observation| observation.get("value"))
            .map_or_else(|| "not_observed".to_string(), Value::to_string)
    };
    println!(
        "parse_batch_budget_shadow: schema={} batches={} documents={} peak_retained_bytes={} p50/p95/p99={}/{}/{} max_reorder_depth={} max_publication_outstanding={} max_in_flight={} budget_waits={} peak_rss_bytes={} java_cpu_wall_clock_ns={} verdict={} h05_closure={}",
        report["schema_version"],
        report["executed_population"]["batches"],
        report["executed_population"]["documents"],
        value("peak_retained_bytes"),
        value("latency_p50_ms"),
        value("latency_p95_ms"),
        value("latency_p99_ms"),
        value("max_reorder_depth"),
        value("max_publication_outstanding"),
        value("max_in_flight"),
        value("budget_waits"),
        value("peak_rss_bytes"),
        value("java_cpu_wall_clock_ns"),
        report["verdict"],
        report["h05_closure"],
    );
    println!("wrote {}", path.display());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Every da-cli example is a composition root: it owns the .env-backed
    // rolling JSON subscriber (ADR-052) and must hold the guard for its
    // lifetime, otherwise file logging silently stops.
    let _log_guard = da_cli::logging::init_pipeline_logging("parse_batch_budget_shadow");
    let args = parse_args()?;
    let report = measure().await?;
    validate_report(&report).map_err(|finding| {
        format!("FATAL: measured report failed its own fail-closed validation: {finding}")
    })?;
    if args.self_check {
        return self_check_guards(&report);
    }
    write_report(&args.report, &report)?;
    print_summary(&args.report, &report);
    Ok(())
}
