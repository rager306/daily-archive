//! S06/T05 (M511-j7wz2f) — MEASURED batch-budget load shadow harness (H05/H19).
//!
//! The test surface is not runtime proof. This example drives the REAL
//! [`da_parse::ParseBatchExecutor`] (no mock backend, no mock executor) under
//! two CONCURRENT batches with a slow ordinal-0 in each, sharing ONE
//! process-local [`ParseBatchBudget`], and emits
//! `artifacts/parse_compat/parse_batch_budget_shadow.json`.
//!
//! Substrate is DECLARED, never assumed (M513-r7dg4z/S04/T02):
//!
//! * `--substrate stub` (the default) drives a deterministic local HTTP stub
//!   over `tokio::net::TcpListener` (the pattern of
//!   `crates/da-cli/tests/parse_batch_test.rs`). No Java process participates,
//!   so the Java-CPU ceiling is a typed `not_observed` with a named reason —
//!   never a fabricated `0` (D430, H19-N1).
//! * `--substrate live-grobid` measures the REAL Java GROBID sidecar over HTTP
//!   with the `structural` profile (D204: `consolidateHeader=0`,
//!   `consolidateCitations=0`, TEI coordinates kept ON) so no Crossref wait can
//!   enter the measurement window and the Java-CPU share stays attributable.
//!   The load is the pinned `data/parse/layout_bakeoff.yaml#corpus` set, every
//!   PDF sha256-verified BEFORE it is sent, and the slow ordinal-0 is the real
//!   longest document (96 pages) declared honestly as
//!   `declared_page_count_disparity` — there is no synthetic delay.
//!
//! The `--report` default is PER SUBSTRATE (`..._shadow.json` for the stub,
//! `..._live.json` for the live run), so a live run can never silently
//! overwrite the byte-pinned stub artifact.
//!
//! Both substrates emit the same ADDITIVE top-level `java_cpu_remainder` block
//! in a unified typed form: `state` ∈ {observed, not_observed}, `counter_scope`,
//! `basis`, and — when observed — `java_cpu_ns`, `wall_clock_ns` and
//! `java_cpu_to_wall_ratio_micros`. The numerator is the container-wide sidecar
//! CPU total over the window (all threads, NOT per-document), so the ratio is a
//! COUNT OF CORES and may exceed 1_000_000; it is never a share ≤ 1.
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
//! * `java_cpu_remainder` observed without `java_cpu_ns` / `wall_clock_ns` / a
//!   positive `java_cpu_to_wall_ratio_micros`, or a ratio that disagrees with
//!   `java_cpu_ns * 1_000_000 / wall_clock_ns`, is rejected;
//! * `java_cpu_remainder` not_observed that carries a numeric field is rejected;
//! * `live_grobid_participated` claimed while the Java-CPU ceiling is not
//!   observed (live substrate), or claimed at all by the stub, is rejected;
//! * a physical path (`/sys/`, a container-scope id, the sidecar name) in the
//!   serialized report is rejected;
//! * non-additive retained-byte accounting is rejected against a
//!   SUBSTRATE-AWARE reference: the stub's deterministic gate keeps every
//!   success of a batch in the retention window at once, so its peak must
//!   cover their sum, while the live substrate releases each reservation as
//!   the sink consumes it, so its peak must cover at least the largest single
//!   success;
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
use std::time::{Duration, Instant};

use async_trait::async_trait;
use da_parse::{
    BATCH_BUDGET_H05_CLOSURE, BATCH_BUDGET_METRICS, BATCH_BUDGET_VERDICTS,
    BATCH_OBSERVATION_STATES, BatchBudgetVerdict, BatchObservation, CacheMode, GrobidHttpBackend,
    GrobidProfile, ParseArtifactCache, ParseBackend, ParseBatch, ParseBatchBudget,
    ParseBatchBudgetObservation, ParseBatchExecutor, ParseBatchLimits, ParseBatchMemoryCheckpoint,
    ParseBatchResultSink, ParseBatchRunSummary, ParseBatchSinkError, ParseCacheFailure,
    ParseDocumentFailure, ParseDocumentOutcome, RawStageRead,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify};

/// Canonical artifact path for the deterministic stub substrate (the default).
const DEFAULT_REPORT_STUB: &str = "artifacts/parse_compat/parse_batch_budget_shadow.json";

/// Canonical artifact path for the live Java GROBID substrate. Distinct from the
/// stub path on purpose: a live run must never overwrite the byte-pinned stub
/// artifact, which the governor pins by sha256.
const DEFAULT_REPORT_LIVE: &str = "artifacts/parse_compat/parse_batch_budget_live.json";

/// Artifact schema version, per substrate.
const STUB_REPORT_SCHEMA_VERSION: &str = "parse-batch-budget-shadow.v1";
const LIVE_REPORT_SCHEMA_VERSION: &str = "parse-batch-budget-live.v1";

/// Substrate identity literals written into `substrate.kind`.
const STUB_SUBSTRATE_KIND: &str = "local_deterministic_loopback_stub";
const LIVE_SUBSTRATE_KIND: &str = "live_grobid_http";

/// Declared live inputs.
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8070";
const DEFAULT_CORPUS: &str = "data/parse/layout_bakeoff.yaml";
const DEFAULT_MANIFEST: &str = "data/parse/acceptance_manifest.yaml";

/// Live Java GROBID request timeout. A 96-page structural-profile document costs
/// roughly 61.9s, and each of the two concurrent batches carries it, so the
/// bounded interactive default is far too tight.
const LIVE_REQUEST_TIMEOUT: Duration = Duration::from_secs(900);

/// The live run must use the structural profile (D204), never the rich one: the
/// rich profile waits on Crossref consolidation, which would land in the
/// measurement window and make the Java-CPU share unattributable.
const LIVE_PROFILE_NAME: &str = "structural";

/// Logical (never physical) name of the Java-CPU counter scope.
const JAVA_CPU_COUNTER_SCOPE: &str = "grobid_sidecar_cgroup_cpuacct";

/// Honest gate name for the live substrate: ordinal-0 is slow because its
/// DECLARED page count is the largest in the pinned corpus, not because a
/// synthetic delay was injected.
const LIVE_SLOW_ORDINAL_ZERO_GATE: &str = "declared_page_count_disparity";

/// The stub gates ordinal-0 on its deterministic sibling-answered barrier.
const STUB_SLOW_ORDINAL_ZERO_GATE: &str = "sibling_responses_answered";

/// Named reason for `java_cpu_remainder` when the Java-CPU ceiling is not observed.
const REMAINDER_NOT_OBSERVED_REASON: &str =
    "the Java-CPU ceiling was not observed in this run, so no Java share can be computed";

/// Mandatory `basis` for `java_cpu_remainder`. It must state (a) the numerator
/// semantics — container-wide sidecar CPU total across all threads, NOT
/// per-document, so the ratio can exceed 1_000_000 and means a COUNT OF CORES —
/// (b) that the wall-clock window is the same window as the counter delta
/// (measured between the before/after reads), and (c) that no synthetic delay
/// sits inside that window.
const JAVA_CPU_REMAINDER_BASIS: &str = "numerator is the GROBID sidecar's container-wide CPU total over the window (cgroup v2 cpu.stat usage_usec delta scaled to ns, all threads, NOT per-document), so the ratio is a count of cores and may exceed 1_000_000; the wall-clock window is measured between the same before/after counter reads that produce the delta, and that window contains no synthetic delay";

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

/// Declared retained-byte ceiling for the synthetic stub load (generous: the
/// measurement must OBSERVE a peak under the ceiling, not trigger refusals).
const RETAINED_BYTE_CEILING: usize = 262_144;

/// Declared retained-byte ceiling for the live corpus load. The pinned corpus
/// carries a 96-page, a 63-page and a 43-page document per batch, so their real
/// GROBID TEI is orders of magnitude larger than the synthetic stub bodies. The
/// ceiling stays generous for the same reason as above: this harness measures a
/// peak under a declared bound, it does not manufacture a memory-pressure
/// refusal by declaring a bound the real payload cannot meet.
const LIVE_RETAINED_BYTE_CEILING: usize = 64 * 1024 * 1024;

/// A zero is not a measurement for these process metrics (H19-N1).
const PROCESS_METRICS: [&str; 2] = ["peak_rss_bytes", "java_cpu_wall_clock_ns"];

/// The three measurements H05 closure requires (D430).
const H05_REQUIRED_MEASUREMENTS: [&str; 3] =
    ["peak_rss_bytes", "java_cpu_wall_clock_ns", "latency_tails"];

/// The three latency-tail metric names.
const LATENCY_TAIL_METRICS: [&str; 3] = ["latency_p50_ms", "latency_p95_ms", "latency_p99_ms"];

/// Redaction denylist for the SERIALIZED report (defence in depth; the primary
/// control is that no producer ever writes these). `/sys/`, the container-scope
/// prefix and the sidecar name are here because the live counter is a physical
/// cgroup path: only its logical scope name may ever be published.
const FORBIDDEN_REPORT_TOKENS: [&str; 15] = [
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
    "/sys/",
    "docker-",
    ".scope",
    "daily-archive-grobid",
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
    marker: String,
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

/// Production Java-CPU reader: the GROBID sidecar's cgroup CPU counter,
/// declared through `PARSE_BATCH_JAVA_CPUACCT` (a cgroup directory, a cgroup
/// `cpu.stat` file, or a `docker stats`-derived bare counter file).
/// Absent/unreadable/non-numeric is a typed absence. The host/cgroup convention
/// lives in [`da_cli::java_cpu_counter`] so probing a cgroup file does not leak
/// a host dependency into `da-parse`.
fn read_java_cpu_counter() -> Option<u64> {
    let path = std::env::var("PARSE_BATCH_JAVA_CPUACCT").ok()?;
    da_cli::java_cpu_counter::read_java_cpu_ns(Path::new(&path))
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

/// One document of the declared load, reduced to what the executor consumes:
/// the identity it is sent under and the exact bytes it is sent as.
#[derive(Clone)]
struct LoadDocument {
    document_id: String,
    pdf: Vec<u8>,
    /// Declared page count for a pinned corpus document; `None` for the
    /// synthetic stub bodies, which have no declared page count.
    pages: Option<u64>,
}

/// One batch of the declared concurrent load.
struct BatchSpec {
    id: &'static str,
    payload_label: &'static str,
    documents: Vec<LoadDocument>,
}

struct DocumentIdentity {
    ordinal: u32,
    document_id: String,
    input_sha256: String,
    pages: Option<u64>,
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
    /// additivity reference the STUB window peak can never fall below, because
    /// its deterministic gate keeps all of them in the window at once.
    fn known_window_success_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| record.succeeded)
            .map(|record| record.outcome_bytes as u64)
            .sum()
    }

    /// The largest single successful outcome of this batch. A success is
    /// reserved by the retention gauge before it is parked and released only
    /// after the sink consumed it, so the window peak must cover at least this
    /// much whatever the publish order — the sound reference for the live
    /// substrate, whose successes need not coexist.
    fn largest_successful_outcome_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| record.succeeded)
            .map(|record| record.outcome_bytes as u64)
            .max()
            .unwrap_or(0)
    }
}

/// The declared STUB load: two batches with a deterministic slow ordinal-0 each
/// and payloads of clearly different sizes. The executor-facing specs and the
/// stub-server documents are built together so they can never diverge.
fn stub_load() -> (Vec<BatchSpec>, Vec<StubDocument>) {
    let document = |group: &'static str,
                    marker: &'static str,
                    ordinal: usize,
                    filler_words: usize|
     -> (StubDocument, LoadDocument) {
        let stub = StubDocument {
            marker: marker.to_string(),
            group,
            slow: ordinal == 0,
            tei: tei_fixture(marker, filler_words),
        };
        let load = LoadDocument {
            document_id: marker.to_string(),
            pdf: pdf_bytes(marker),
            pages: None,
        };
        (stub, load)
    };
    let (stub_a0, load_a0) = document("A", "SYNTH-A-0-ZX", 0, 40);
    let (stub_a1, load_a1) = document("A", "SYNTH-A-1-ZX", 1, 40);
    let (stub_a2, load_a2) = document("A", "SYNTH-A-2-ZX", 2, 40);
    let (stub_b0, load_b0) = document("B", "SYNTH-B-0-ZX", 0, 900);
    let (stub_b1, load_b1) = document("B", "SYNTH-B-1-ZX", 1, 900);
    let (stub_b2, load_b2) = document("B", "SYNTH-B-2-ZX", 2, 900);
    let specs = vec![
        BatchSpec {
            id: "A",
            payload_label: "small_synthetic_tei_body",
            documents: vec![load_a0, load_a1, load_a2],
        },
        BatchSpec {
            id: "B",
            payload_label: "large_synthetic_tei_body",
            documents: vec![load_b0, load_b1, load_b2],
        },
    ];
    let stub_documents = vec![stub_a0, stub_a1, stub_a2, stub_b0, stub_b1, stub_b2];
    (specs, stub_documents)
}

/// The declared LIVE load: the pinned `data/parse/layout_bakeoff.yaml#corpus`
/// set, sha256-verified BEFORE any byte is sent, carried by two concurrent
/// batches. Ordinal-0 is the DECLARED longest document, so the slow ordinal is
/// a real page-count disparity rather than an injected delay.
fn live_load(corpus_path: &Path) -> Result<Vec<BatchSpec>, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(corpus_path).map_err(|error| {
        format!(
            "FATAL: the declared corpus {} is unreadable: {error}",
            corpus_path.display()
        )
    })?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|error| format!("FATAL: the declared corpus is not YAML: {error}"))?;
    let documents = parsed
        .get("corpus")
        .and_then(|corpus| corpus.get("documents"))
        .and_then(|documents| documents.as_sequence())
        .ok_or("FATAL: corpus.documents is absent from the declared corpus")?;
    let mut loaded: Vec<LoadDocument> = Vec::with_capacity(documents.len());
    let mut declared_pages: Vec<u64> = Vec::with_capacity(documents.len());
    for document in documents {
        let id = document
            .get("id")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no id")?;
        let source_pdf = document
            .get("source_pdf")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no source_pdf")?;
        let declared_sha = document
            .get("sha256")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no sha256")?;
        let pages = document
            .get("pages")
            .and_then(|value| value.as_u64())
            .ok_or("FATAL: a corpus document has no declared page count")?;
        let bytes = std::fs::read(source_pdf)
            .map_err(|error| format!("FATAL: the corpus PDF for `{id}` is unreadable: {error}"))?;
        let observed = hex::encode(Sha256::digest(&bytes));
        if !observed.eq_ignore_ascii_case(declared_sha.trim_start_matches("sha256:")) {
            return Err(format!(
                "FATAL: corpus PDF `{id}` sha256 mismatch: declared {declared_sha}, observed sha256:{observed}"
            )
            .into());
        }
        loaded.push(LoadDocument {
            document_id: id.to_string(),
            pdf: bytes,
            pages: Some(pages),
        });
        declared_pages.push(pages);
    }
    if loaded.len() != DOCUMENTS_PER_BATCH {
        return Err(format!(
            "FATAL: the declared corpus carries {} documents but the load declares {} per batch",
            loaded.len(),
            DOCUMENTS_PER_BATCH
        )
        .into());
    }
    // Ordinal-0 must be the DECLARED longest document, so the page-count
    // disparity the report names is a fact of the corpus, not an ordering guess.
    let mut order: Vec<usize> = (0..loaded.len()).collect();
    order.sort_by_key(|index| std::cmp::Reverse(declared_pages[*index]));
    let ordered: Vec<LoadDocument> = order.iter().map(|index| loaded[*index].clone()).collect();
    let batch = |id: &'static str| BatchSpec {
        id,
        payload_label: "layout_bakeoff_corpus_three_pdf",
        documents: ordered.clone(),
    };
    Ok(vec![batch("A"), batch("B")])
}

/// Stub substrate backend: the default rich profile is irrelevant to the stub
/// (it ignores the profile) and the 30s bound comfortably covers the 80ms gate.
fn stub_backend(endpoint: &str) -> GrobidHttpBackend {
    GrobidHttpBackend::new(Some(endpoint)).with_timeout(Duration::from_secs(30))
}

/// Live substrate backend: the structural profile is MANDATORY (D204) and the
/// timeout must exceed the real cost of the 96-page document.
fn live_backend(endpoint: &str) -> GrobidHttpBackend {
    GrobidHttpBackend::new(Some(endpoint))
        .with_profile(GrobidProfile::structural())
        .with_timeout(LIVE_REQUEST_TIMEOUT)
}

async fn run_batch(
    spec: &BatchSpec,
    backend: GrobidHttpBackend,
    retained_byte_ceiling: usize,
    budget: Arc<ParseBatchBudget>,
) -> Result<BatchRun, Box<dyn std::error::Error>> {
    let requests = spec
        .documents
        .iter()
        .map(|document| backend.default_request(&document.document_id, document.pdf.clone()))
        .collect::<Vec<_>>();
    let identities = spec
        .documents
        .iter()
        .enumerate()
        .map(|(ordinal, document)| DocumentIdentity {
            ordinal: ordinal as u32,
            document_id: document.document_id.clone(),
            input_sha256: sha256_hex(&document.pdf),
            pages: document.pages,
        })
        .collect();
    let limits = ParseBatchLimits::new(
        nz(spec.documents.len()),
        nz(spec.documents.len()),
        nz(spec.documents.len()),
        nz(spec.documents.len()),
    )
    .with_max_retained_bytes(nz(retained_byte_ceiling));
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

/// Declared substrate. The default is the hermetic stub; the live substrate is
/// opt-in, uses its own report default, and is never touched by `--self-check`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Substrate {
    Stub,
    LiveGrobid,
}

impl Substrate {
    fn schema_version(self) -> &'static str {
        match self {
            Substrate::Stub => STUB_REPORT_SCHEMA_VERSION,
            Substrate::LiveGrobid => LIVE_REPORT_SCHEMA_VERSION,
        }
    }

    fn default_report(self) -> &'static str {
        match self {
            Substrate::Stub => DEFAULT_REPORT_STUB,
            Substrate::LiveGrobid => DEFAULT_REPORT_LIVE,
        }
    }
}

/// The declared production release the live service must be attributable to.
/// Read as DATA so a measured run can never be pinned to a re-pointed tag.
struct DeclaredRelease {
    backend: String,
    version: String,
    image_digest: String,
}

/// Read `external_holdout_contract.release_probe.declared_release` from the
/// acceptance manifest (the same path `grobid_release_identity_test` pins).
fn load_declared_release(
    manifest_path: &Path,
) -> Result<DeclaredRelease, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|error| format!("FATAL: the declared-release manifest is unreadable: {error}"))?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|error| format!("FATAL: the declared-release manifest is not YAML: {error}"))?;
    let declared = parsed
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .and_then(|probe| probe.get("declared_release"))
        .ok_or("FATAL: external_holdout_contract.release_probe.declared_release is absent")?;
    let field = |name: &str| -> Result<String, Box<dyn std::error::Error>> {
        declared
            .get(name)
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("FATAL: declared_release.{name} is absent").into())
    };
    Ok(DeclaredRelease {
        backend: field("backend")?,
        version: field("version")?,
        image_digest: field("image_digest")?,
    })
}

/// Run the declared load and assemble the artifact. Fails closed on any
/// additivity or observation violation.
async fn measure(
    substrate: Substrate,
    endpoint: &str,
    corpus_path: &Path,
    manifest_path: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    // ── declared load, per substrate ────────────────────────────────────
    let (specs, stub_documents) = match substrate {
        Substrate::Stub => {
            let (specs, documents) = stub_load();
            (specs, Some(documents))
        }
        Substrate::LiveGrobid => (live_load(corpus_path)?, None),
    };
    let retained_byte_ceiling = match substrate {
        Substrate::Stub => RETAINED_BYTE_CEILING,
        Substrate::LiveGrobid => LIVE_RETAINED_BYTE_CEILING,
    };
    let slow_ordinal_zero_gate = match substrate {
        Substrate::Stub => STUB_SLOW_ORDINAL_ZERO_GATE,
        Substrate::LiveGrobid => LIVE_SLOW_ORDINAL_ZERO_GATE,
    };
    let slow_ordinal_zero_settle_ms = match substrate {
        Substrate::Stub => SLOW_ORDINAL_ZERO_SETTLE.as_millis() as u64,
        Substrate::LiveGrobid => 0u64,
    };

    // ── declared release + liveness/version gate (live only) ────────────
    let declared_release = match substrate {
        Substrate::Stub => None,
        Substrate::LiveGrobid => {
            let declared = load_declared_release(manifest_path)?;
            let probe = live_backend(endpoint);
            if !probe.is_alive().await {
                return Err(
                    "FATAL: the declared live GROBID sidecar did not answer /api/isalive; refusing to measure"
                        .into(),
                );
            }
            let version = probe
                .probe_version()
                .await
                .map_err(|error| format!("FATAL: /api/version probe failed: {error:?}"))?;
            if version.version.trim() != declared.version.trim() {
                return Err(format!(
                    "FATAL: live GROBID version `{}` does not match the declared release `{}`",
                    version.version.trim(),
                    declared.version.trim()
                )
                .into());
            }
            Some(declared)
        }
    };

    // ── the stub substrate needs its deterministic loopback server ──────
    let address = match stub_documents {
        Some(documents) => Some(spawn_stub(documents, SLOW_ORDINAL_ZERO_SETTLE).await),
        None => None,
    };

    let budget = Arc::new(ParseBatchBudget::new(
        nz(SHARED_PUBLICATION_CAPACITY),
        nz(SHARED_EXECUTION_CAPACITY),
    ));

    // Injected readers (`FnMut() -> Option<u64>`), each counting its own
    // invocations so the artifact reports how often the seam was exercised
    // rather than asserting it. Peak RSS is a high-water mark, so one read
    // after the load suffices; the Java-CPU counter is a monotonic total, so it
    // needs a before/after delta. The wall-clock window is opened right after
    // the before read and closed right before the after read, so the share is
    // computed over exactly the counter-delta window.
    let mut rss_reader = CountingReader::new(read_process_peak_rss);
    let mut java_cpu_reader = CountingReader::new(read_java_cpu_counter);
    let java_cpu_before = java_cpu_reader.read();
    let window_start = Instant::now();

    let (left, right) = match substrate {
        Substrate::Stub => {
            let base = format!("http://{}", address.expect("stub address"));
            tokio::join!(
                run_batch(
                    &specs[0],
                    stub_backend(&base),
                    retained_byte_ceiling,
                    Arc::clone(&budget)
                ),
                run_batch(
                    &specs[1],
                    stub_backend(&base),
                    retained_byte_ceiling,
                    Arc::clone(&budget)
                ),
            )
        }
        Substrate::LiveGrobid => tokio::join!(
            run_batch(
                &specs[0],
                live_backend(endpoint),
                retained_byte_ceiling,
                Arc::clone(&budget)
            ),
            run_batch(
                &specs[1],
                live_backend(endpoint),
                retained_byte_ceiling,
                Arc::clone(&budget)
            ),
        ),
    };
    let left = left?;
    let right = right?;
    let runs = [&left, &right];
    let wall_clock_ns = window_start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

    let rss_after = rss_reader.read();
    let java_cpu_after = java_cpu_reader.read();

    // ── fail-closed: retained-byte accounting must be additive ──────────
    //
    // The reference is SUBSTRATE-AWARE. The stub's deterministic gate keeps
    // every successful outcome of a batch in the retention window at once, so
    // its peak must cover their whole sum. The live substrate publishes
    // strictly by ordinal and releases a reservation as soon as the sink
    // consumes it, so successes need not coexist and the sound reference is
    // the largest single successful outcome (every success is reserved before
    // it is parked and released only after the sink took it). The ceiling
    // itself is enforced at reservation time in `try_reserve_retained_bytes`,
    // independent of this reference.
    for run in runs {
        let peak = run.peak_retained_bytes().ok_or_else(|| {
            format!(
                "FATAL: batch {} did not observe peak_retained_bytes, so no additive accounting can be claimed",
                run.id
            )
        })?;
        let (reference, reference_name) = match substrate {
            Substrate::Stub => (
                run.known_window_success_bytes(),
                "known window success bytes",
            ),
            Substrate::LiveGrobid => (
                run.largest_successful_outcome_bytes(),
                "largest successful outcome bytes",
            ),
        };
        if peak < reference {
            return Err(format!(
                "FATAL: non-additive retained-byte accounting in batch {}: window peak {peak} < {reference_name} {reference}",
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

    // ── unified `java_cpu_remainder`: what share of wall-clock stayed on Java ─
    //
    // Same typed form for BOTH substrates. The numerator is the sidecar's
    // container-wide CPU total over the window, so the ratio is a count of
    // cores (`* 1_000_000` micro-cores), NOT a share ≤ 1 — there is no upper
    // bound on the ratio. When the ceiling was not observed there is a named
    // reason and NO numeric field at all.
    let java_cpu_remainder = match java_cpu_observation.value() {
        Some(java_cpu_ns) if *java_cpu_ns > 0 && wall_clock_ns > 0 => {
            let java_cpu_ns = *java_cpu_ns;
            let ratio_micros =
                (u128::from(java_cpu_ns) * 1_000_000 / u128::from(wall_clock_ns)) as u64;
            json!({
                "state": "observed",
                "counter_scope": JAVA_CPU_COUNTER_SCOPE,
                "basis": JAVA_CPU_REMAINDER_BASIS,
                "java_cpu_ns": java_cpu_ns,
                "wall_clock_ns": wall_clock_ns,
                "java_cpu_to_wall_ratio_micros": ratio_micros,
            })
        }
        _ => json!({
            "state": "not_observed",
            "counter_scope": JAVA_CPU_COUNTER_SCOPE,
            "basis": JAVA_CPU_REMAINDER_BASIS,
            "reason": java_cpu_observation
                .reason()
                .unwrap_or(REMAINDER_NOT_OBSERVED_REASON),
        }),
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
                "declared_pages": identity.pages,
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

    // ── substrate-aware identity and prose: DATA, not branching in the body ─
    let (reference_identity, execution_evidence, corpus_execution, evaluated_reason) =
        match substrate {
            Substrate::Stub => (
                "synthetic_two_batch_slow_ordinal_zero_load",
                "offline_loopback_stub_existing_typed_transport:two_batch_slow_ordinal_zero",
                false,
                "local deterministic stub substrate; no corpus execution and therefore no evaluation claim",
            ),
            Substrate::LiveGrobid => (
                "layout_bakeoff_corpus_two_batch_load",
                "live_grobid_http_structural:two_batch_corpus_load",
                true,
                "live GROBID corpus execution with no frozen gold comparison in this harness; no evaluation claim",
            ),
        };

    let substrate_block = match substrate {
        Substrate::Stub => json!({
            "kind": STUB_SUBSTRATE_KIND,
            "profile": "deterministic_loopback_stub",
            "live_grobid_participated": false,
            "java_process_participation_observed": java_cpu_observed,
            "deterministic_substrate": true,
            "determinism_note": "deterministic stub, deterministic sibling-answered gate on ordinal-0, deterministic slow-ordinal-0 settle; measured durations are wall-clock and therefore not byte-stable",
            "rss_scope": "process_peak_vmhwm_including_this_harness",
            "java_cpu_scope": JAVA_CPU_COUNTER_SCOPE,
            "measurement_note": "Both batches ran concurrently against one shared process-local budget over a loopback stub. Peak RSS is a process high-water mark and includes this harness; the Java CPU counter is not observed because no Java process participated.",
            "reader_invocations": {
                "peak_rss_bytes": rss_reader.calls(),
                "java_cpu_wall_clock_ns": java_cpu_reader.calls(),
            },
            "load": {
                "concurrent_batches": runs.len(),
                "documents_per_batch": DOCUMENTS_PER_BATCH,
                "slow_ordinal_zero_settle_ms": slow_ordinal_zero_settle_ms,
                "slow_ordinal_zero_gate": slow_ordinal_zero_gate,
                "shared_budget": {
                    "publication_capacity": SHARED_PUBLICATION_CAPACITY,
                    "execution_capacity": SHARED_EXECUTION_CAPACITY,
                },
                "payload_profile": {
                    "batch_a": specs[0].payload_label,
                    "batch_b": specs[1].payload_label,
                },
            },
        }),
        Substrate::LiveGrobid => {
            let release = declared_release
                .as_ref()
                .expect("the live substrate reads its declared release before measuring");
            json!({
                "kind": LIVE_SUBSTRATE_KIND,
                "profile": LIVE_PROFILE_NAME,
                "live_grobid_participated": java_cpu_observed,
                "java_process_participation_observed": java_cpu_observed,
                "deterministic_substrate": false,
                "determinism_note": "live Java GROBID over HTTP with the structural profile; the sidecar is the real Java process and the corpus is the pinned layout_bakeoff set; measured durations are wall-clock and therefore not byte-stable",
                "declared_release": {
                    "backend": release.backend.clone(),
                    "version": release.version.clone(),
                    "image_digest": release.image_digest.clone(),
                },
                "rss_scope": "process_peak_vmhwm_including_this_harness",
                "java_cpu_scope": JAVA_CPU_COUNTER_SCOPE,
                "measurement_note": "Both batches ran concurrently against one shared process-local budget over the live Java GROBID HTTP sidecar with the structural profile. The pinned corpus PDFs were sha256-verified before sending; the slow ordinal-0 is the declared longest document and no synthetic delay sits in the measurement window. Peak RSS is a process high-water mark and includes this harness.",
                "reader_invocations": {
                    "peak_rss_bytes": rss_reader.calls(),
                    "java_cpu_wall_clock_ns": java_cpu_reader.calls(),
                },
                "load": {
                    "concurrent_batches": runs.len(),
                    "documents_per_batch": DOCUMENTS_PER_BATCH,
                    "slow_ordinal_zero_settle_ms": slow_ordinal_zero_settle_ms,
                    "slow_ordinal_zero_gate": slow_ordinal_zero_gate,
                    "shared_budget": {
                        "publication_capacity": SHARED_PUBLICATION_CAPACITY,
                        "execution_capacity": SHARED_EXECUTION_CAPACITY,
                    },
                    "payload_profile": {
                        "batch_a": specs[0].payload_label,
                        "batch_b": specs[1].payload_label,
                    },
                },
            })
        }
    };

    Ok(json!({
        "schema_version": substrate.schema_version(),
        "requirement_ids": ["R085", "R087", "R139"],
        "adr_ref": "ADR-076",
        "substrate": substrate_block,
        "reference_population": {
            "role": REFERENCE_POPULATION_ROLE,
            "identity": reference_identity,
            "documents": documents,
            "batches": runs.len(),
        },
        "executed_population": {
            "documents": documents,
            "corpus_execution": corpus_execution,
            "execution_evidence": execution_evidence,
            "batches": runs.len(),
            "terminal_outcomes": terminal,
            "succeeded": succeeded,
            "failed": failed,
        },
        "evaluated_population": {
            "documents": 0,
            "status": "not_evaluated",
            "reason": evaluated_reason,
            "aggregation": "max_across_batches",
            "measurements_observed": observations.values().filter(|observation| observation.get("state").and_then(Value::as_str) == Some("observed")).count(),
            "measurements_not_observed": observations.values().filter(|observation| observation.get("state").and_then(Value::as_str) == Some("not_observed")).count(),
        },
        "batches": batches,
        "outcomes": outcomes,
        "observations": Value::Object(observations),
        "java_cpu_remainder": java_cpu_remainder,
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

    // ── substrate identity decides the schema; checked FIRST ────────────
    let substrate = require_object(report, "substrate")?;
    let kind = require_str(substrate, "kind")?;
    if kind != STUB_SUBSTRATE_KIND && kind != LIVE_SUBSTRATE_KIND {
        return Err(format!("substrate-kind-unknown:{kind}"));
    }
    let schema = require_str(report.as_object().expect("checked"), "schema_version")?;
    let expected_schema = if kind == STUB_SUBSTRATE_KIND {
        STUB_REPORT_SCHEMA_VERSION
    } else {
        LIVE_REPORT_SCHEMA_VERSION
    };
    if schema != expected_schema {
        return Err(format!("schema-version-drift:{schema}"));
    }
    let profile = require_str(substrate, "profile")?;
    if kind == LIVE_SUBSTRATE_KIND && profile != LIVE_PROFILE_NAME {
        return Err(format!("live-substrate-profile-not-structural:{profile}"));
    }

    // ── the unified `java_cpu_remainder` block ──────────────────────────
    let remainder = require_object(report, "java_cpu_remainder")?;
    let remainder_state = require_str(remainder, "state")?;
    require_str(remainder, "counter_scope")?;
    require_str(remainder, "basis")?;
    match remainder_state {
        "observed" => {
            let field = |name: &str| -> Result<u64, String> {
                remainder
                    .get(name)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("java-cpu-remainder-observed-missing:{name}"))
            };
            let java_cpu_ns = field("java_cpu_ns")?;
            let wall_clock_ns = field("wall_clock_ns")?;
            let ratio = field("java_cpu_to_wall_ratio_micros")?;
            if java_cpu_ns == 0 {
                return Err("java-cpu-remainder-numerator-zero".to_string());
            }
            if wall_clock_ns == 0 {
                return Err("java-cpu-remainder-wall-clock-zero".to_string());
            }
            if ratio == 0 {
                return Err("java-cpu-remainder-ratio-not-positive".to_string());
            }
            // There is deliberately NO 1_000_000 upper bound: the numerator is
            // the container-wide CPU total, so the ratio counts cores and may
            // legitimately exceed one.
            let expected = (u128::from(java_cpu_ns) * 1_000_000 / u128::from(wall_clock_ns)) as u64;
            if ratio != expected {
                return Err(format!(
                    "java-cpu-remainder-ratio-inconsistent: declared={ratio} recomputed={expected}"
                ));
            }
            if remainder.contains_key("reason") {
                return Err("java-cpu-remainder-observed-carries-reason".to_string());
            }
        }
        "not_observed" => {
            for name in [
                "java_cpu_ns",
                "wall_clock_ns",
                "java_cpu_to_wall_ratio_micros",
            ] {
                if remainder.contains_key(name) {
                    return Err(format!(
                        "java-cpu-remainder-not-observed-carries-numeric-field:{name}"
                    ));
                }
            }
            require_str(remainder, "reason")?;
        }
        other => return Err(format!("java-cpu-remainder-state-unknown:{other}")),
    }

    // ── the observation and the remainder must tell one story ──────────
    let observations = require_object(report, "observations")?;
    let java_cpu_observed = observation_state(
        observations
            .get("java_cpu_wall_clock_ns")
            .ok_or_else(|| "java-cpu-observation-absent".to_string())?,
        "java_cpu_wall_clock_ns",
    )? == "observed";
    if (remainder_state == "observed") != java_cpu_observed {
        return Err(format!(
            "java-cpu-remainder-disagrees-with-observation: remainder={remainder_state} observation_observed={java_cpu_observed}"
        ));
    }

    // ── participation claims are earned, never asserted ────────────────
    let participation = substrate
        .get("java_process_participation_observed")
        .and_then(Value::as_bool)
        .ok_or_else(|| "substrate-participation-not-boolean".to_string())?;
    if participation != java_cpu_observed {
        return Err("java-process-participation-disagrees-with-observation".to_string());
    }
    let live_claimed = substrate
        .get("live_grobid_participated")
        .and_then(Value::as_bool)
        .ok_or_else(|| "substrate-live-grobid-participated-not-boolean".to_string())?;
    if kind == STUB_SUBSTRATE_KIND && live_claimed {
        return Err("stub-substrate-claims-live-grobid".to_string());
    }
    if kind == LIVE_SUBSTRATE_KIND && live_claimed != java_cpu_observed {
        return Err("live-grobid-participation-without-observed-java-cpu".to_string());
    }
    if kind == LIVE_SUBSTRATE_KIND
        && substrate
            .get("declared_release")
            .and_then(Value::as_object)
            .is_none()
    {
        return Err("live-substrate-declared-release-absent".to_string());
    }
    if kind == STUB_SUBSTRATE_KIND
        && substrate
            .get("deterministic_substrate")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err("stub-substrate-not-declared-deterministic".to_string());
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
    match executed.get("corpus_execution").and_then(Value::as_bool) {
        Some(value) if value == (kind == LIVE_SUBSTRATE_KIND) => {}
        Some(_) => return Err("gar006-corpus-execution-does-not-match-substrate".to_string()),
        None => return Err("gar006-executed-corpus-execution-not-boolean".to_string()),
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
        let mut largest_single = 0u64;
        for outcome in outcomes {
            let outcome = outcome
                .as_object()
                .ok_or_else(|| "outcome-not-object".to_string())?;
            if outcome.get("batch").and_then(Value::as_str) != Some(id) {
                continue;
            }
            if outcome.get("status").and_then(Value::as_str) == Some("succeeded") {
                let bytes = outcome
                    .get("outcome_bytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("outcome-bytes-missing:{id}"))?;
                recomputed += bytes;
                largest_single = largest_single.max(bytes);
            }
        }
        if recomputed != known {
            return Err(format!(
                "batch-known-bytes-not-additive:{id}: recorded={known} recomputed={recomputed}"
            ));
        }
        // Substrate-aware additivity reference, the same reasoning as the
        // `measure` gate: the stub's deterministic gate keeps every success of
        // a batch in the retention window at once, so its peak must cover the
        // sum; the live substrate releases each reservation as the sink
        // consumes it, so its peak must cover at least the largest single
        // success. A `non-additive-retained-bytes` mutation of the stub report
        // still fails closed here.
        let reference = if kind == STUB_SUBSTRATE_KIND {
            recomputed
        } else {
            largest_single
        };
        if peak < reference {
            return Err(format!(
                "non-additive-retained-bytes:{id}: peak={peak} reference={reference}"
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
        (
            "physical-path-in-report",
            |report: &mut Value| {
                report["substrate"]["measurement_note"] =
                    json!("/sys/fs/cgroup/system.slice/docker-deadbeef.scope");
            },
            "redaction-forbidden-token:/sys/",
        ),
        (
            "live-grobid-participation-without-observed-java-cpu",
            |report: &mut Value| {
                report["schema_version"] = json!(LIVE_REPORT_SCHEMA_VERSION);
                report["substrate"]["kind"] = json!(LIVE_SUBSTRATE_KIND);
                report["substrate"]["profile"] = json!(LIVE_PROFILE_NAME);
                report["substrate"]["live_grobid_participated"] = json!(true);
                report["substrate"]["declared_release"] = json!({
                    "backend": "java-grobid-http",
                    "version": "injected",
                    "image_digest": "sha256:injected",
                });
            },
            "live-grobid-participation-without-observed-java-cpu",
        ),
        (
            "java-cpu-remainder-observed-without-numbers",
            |report: &mut Value| {
                report["java_cpu_remainder"] = json!({
                    "state": "observed",
                    "counter_scope": "grobid_sidecar_cgroup_cpuacct",
                    "basis": "injected",
                });
            },
            "java-cpu-remainder-observed-missing",
        ),
        (
            "java-cpu-remainder-ratio-not-positive",
            |report: &mut Value| {
                report["java_cpu_remainder"] = json!({
                    "state": "observed",
                    "counter_scope": "grobid_sidecar_cgroup_cpuacct",
                    "basis": "injected",
                    "java_cpu_ns": 1,
                    "wall_clock_ns": 2_000_000_000u64,
                    "java_cpu_to_wall_ratio_micros": 0,
                });
            },
            "java-cpu-remainder-ratio-not-positive",
        ),
        (
            "java-cpu-remainder-not-observed-with-numeric-field",
            |report: &mut Value| {
                report["java_cpu_remainder"] = json!({
                    "state": "not_observed",
                    "counter_scope": "grobid_sidecar_cgroup_cpuacct",
                    "basis": "injected",
                    "reason": "injected",
                    "java_cpu_ns": 1,
                });
            },
            "java-cpu-remainder-not-observed-carries-numeric-field",
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
    substrate: Substrate,
    report: PathBuf,
    endpoint: String,
    corpus: PathBuf,
    manifest: PathBuf,
    self_check: bool,
}

fn parse_substrate(value: &str) -> Result<Substrate, Box<dyn std::error::Error>> {
    match value {
        "stub" => Ok(Substrate::Stub),
        "live-grobid" => Ok(Substrate::LiveGrobid),
        other => {
            Err(format!("unknown --substrate `{other}` (expected `stub` or `live-grobid`)").into())
        }
    }
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut substrate = Substrate::Stub;
    let mut report: Option<PathBuf> = None;
    let mut endpoint = DEFAULT_ENDPOINT.to_string();
    let mut corpus = PathBuf::from(DEFAULT_CORPUS);
    let mut manifest = PathBuf::from(DEFAULT_MANIFEST);
    let mut self_check = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--substrate" => {
                substrate =
                    parse_substrate(&arguments.next().ok_or("--substrate requires a value")?)?;
            }
            "--report" => {
                report = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or("--report requires a path")?,
                );
            }
            "--endpoint" => {
                endpoint = arguments.next().ok_or("--endpoint requires a URL")?;
            }
            "--corpus" => {
                corpus = arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--corpus requires a path")?;
            }
            "--manifest" => {
                manifest = arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--manifest requires a path")?;
            }
            "--self-check" => self_check = true,
            "--help" => {
                println!(
                    "parse_batch_budget_shadow [--substrate stub|live-grobid] [--report FILE]\n\
                     \x20                        [--endpoint URL] [--corpus FILE] [--manifest FILE] [--self-check]\n\
                     \n\
                     Measures the batch budget under two concurrent batches with a slow\n\
                     ordinal-0 (shared budget, declared max_retained_bytes) and writes the\n\
                     GAR-006 artifact whose --report DEFAULT is per substrate\n\
                     (stub -> parse_batch_budget_shadow.json, live -> parse_batch_budget_live.json).\n\
                     The live substrate requires a real GROBID sidecar and verifies it against\n\
                     the declared release; --self-check always runs the hermetic stub, proves\n\
                     the fail-closed guards fire, and writes nothing."
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }
    let report = report.unwrap_or_else(|| PathBuf::from(substrate.default_report()));
    Ok(Args {
        substrate,
        report,
        endpoint,
        corpus,
        manifest,
        self_check,
    })
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
        "parse_batch_budget_shadow: substrate={} schema={} batches={} documents={} peak_retained_bytes={} p50/p95/p99={}/{}/{} max_reorder_depth={} max_publication_outstanding={} max_in_flight={} budget_waits={} peak_rss_bytes={} java_cpu_wall_clock_ns={} java_cpu_to_wall_ratio_micros={} verdict={} h05_closure={}",
        report["substrate"]["kind"],
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
        report["java_cpu_remainder"]
            .get("java_cpu_to_wall_ratio_micros")
            .map_or_else(|| "not_observed".to_string(), Value::to_string),
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
    // `--self-check` is hermetic by construction: it must never touch the live
    // sidecar, so it always proves the guards against the stub report.
    let substrate = if args.self_check {
        Substrate::Stub
    } else {
        args.substrate
    };
    let report = measure(substrate, &args.endpoint, &args.corpus, &args.manifest).await?;
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
