//! Integration suite for `da parse-batch` (R116, M464-z84in2 S06/T04).
//!
//! The full CLI flow — manifest → closed composition config → typed
//! admission → `GrobidHttpBackend` over a LOCAL deterministic HTTP stub →
//! T03 atomic payload cache inside the T02 two-stage cache → bounded
//! executor → atomic versioned result report — exercised offline, plus
//! the redb accelerator contract (feature-gated, D295).
//!
//! Objective proofs required by the S06 plan:
//!
//! - invalid blank/unknown/zero/overflow/conflicting config values fail
//!   BEFORE the backend is opened (typed errors, zero HTTP attempts);
//! - an offline loopback run shows a mixed per-document report (typed
//!   busy failure preserved verbatim, exactly one HTTP attempt per
//!   document), the bounded settings echo and resume;
//! - deleting redb never loses raw/projector payload or completed
//!   evidence and the accelerator rebuilds from canonical evidence.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use da_cli::parse_batch::{
    ParseBatchArgs, ParseBatchCliError, ParseBatchManifestV1, ParseBatchReportV1, load_manifest,
    run_with_env,
};
use da_parse::{CacheMode, FailureCode, ParseDocumentFailure, RetryAfter};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ───────────────────────── fixtures ─────────────────────────

const TEI_FIXTURE: &str = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc><titleStmt><title>Parse Batch Parity Paper</title></titleStmt></fileDesc></teiHeader><text><body>
<div><head n="1">INTRODUCTION</head><p>Composed pipeline text.</p></div>
<div><head n="2">METHODS</head><p>Method details.</p></div>
</body><back><div type="references"><listBibl>
<biblStruct><analytic><title level="a" type="main">First Citation</title></analytic><idno type="DOI">10.1000/cite.1</idno></biblStruct>
<biblStruct><analytic><title level="a" type="main">Second Citation</title></analytic><idno type="arXiv">arXiv:2001.00002v1</idno></biblStruct>
</listBibl></div></back></text></TEI>"#;

const MARKER_ALPHA: &str = "MARKER-ALPHA-1f4e";
const MARKER_BETA: &str = "MARKER-BETA-9a20";
const MARKER_GAMMA: &str = "MARKER-GAMMA-77c3";
const MARKER_DELTA: &str = "MARKER-DELTA-b51e";

fn pdf_bytes(marker: &str) -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    bytes.extend_from_slice(marker.as_bytes());
    bytes.extend_from_slice(b"\n%%EOF\n");
    bytes
}

// ───────────────────────── local stub GROBID ─────────────────────────

type RequestLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Per-document response plan (dispatched by a marker byte string inside
/// the multipart body — order-independent under bounded concurrency).
#[derive(Debug, Clone, Copy)]
enum Plan {
    /// 200 OK with the liftable TEI fixture.
    Tei,
    /// 503 with `Retry-After: 7` (typed Busy, never retried).
    Busy,
}

async fn spawn_stub(plans: Vec<(&'static str, Plan)>) -> (SocketAddr, RequestLog) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub GROBID");
    let addr = listener.local_addr().expect("stub address");
    let log: RequestLog = Arc::new(Mutex::new(Vec::new()));
    let plans = Arc::new(plans);
    let log_for_task = Arc::clone(&log);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let log = Arc::clone(&log_for_task);
            let plans = Arc::clone(&plans);
            tokio::spawn(serve_connection(stream, plans, log));
        }
    });
    (addr, log)
}

async fn serve_connection(
    mut stream: tokio::net::TcpStream,
    plans: Arc<Vec<(&'static str, Plan)>>,
    log: RequestLog,
) {
    let Some((path, body)) = read_request(&mut stream).await else {
        return;
    };
    log.lock().expect("request log").push((path, body.clone()));
    let plan = plans
        .iter()
        .find(|(marker, _)| {
            body.windows(marker.len())
                .any(|window| window == marker.as_bytes())
        })
        .map(|(_, plan)| *plan);
    let (status, reason, extra_headers, body): (u16, &str, Vec<(&str, &str)>, Vec<u8>) = match plan
    {
        Some(Plan::Tei) => (200, "OK", Vec::new(), TEI_FIXTURE.as_bytes().to_vec()),
        Some(Plan::Busy) => (
            503,
            "Service Unavailable",
            vec![("Retry-After", "7")],
            Vec::new(),
        ),
        None => (500, "Internal Server Error", Vec::new(), Vec::new()),
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/xml\r\n");
    for (name, value) in extra_headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut response = head.into_bytes();
    response.extend_from_slice(&body);
    let _ = stream.write_all(&response).await;
    let _ = stream.shutdown().await;
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<(String, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(position) = find(&buf, b"\r\n\r\n") {
            break position + 4;
        }
        if buf.len() > (8 << 20) {
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
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "content-length" {
            content_length = value.parse().unwrap_or(0);
        } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
            chunked = true;
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
        body.truncate(content_length);
    }
    Some((path, body))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
        if input.len() < size {
            decoded.extend_from_slice(input);
            break;
        }
        decoded.extend_from_slice(&input[..size]);
        input = &input[(size + 2).min(input.len())..];
    }
    decoded
}

// ───────────────────────── harness ─────────────────────────

const DOCS: [(&str, &str); 3] = [
    ("doc-alpha", MARKER_ALPHA),
    ("doc-beta", MARKER_BETA),
    ("doc-gamma", MARKER_GAMMA),
];

struct Harness {
    _temp: TempDir,
    manifest: PathBuf,
    report: PathBuf,
    cache_root: PathBuf,
    stub: (SocketAddr, RequestLog),
}

async fn harness() -> Harness {
    harness_with_plans(vec![
        (MARKER_ALPHA, Plan::Tei),
        (MARKER_BETA, Plan::Busy),
        (MARKER_GAMMA, Plan::Tei),
    ])
    .await
}

async fn harness_with_plans(plans: Vec<(&'static str, Plan)>) -> Harness {
    let temp = TempDir::new().expect("tempdir");
    let manifest = temp.path().join("manifest.json");
    let report = temp.path().join("report.json");
    let cache_root = temp.path().join("cache");
    write_default_manifest(&manifest, temp.path());
    let stub = spawn_stub(plans).await;
    Harness {
        _temp: temp,
        manifest,
        report,
        cache_root,
        stub,
    }
}

fn write_default_manifest(manifest_path: &Path, dir: &Path) {
    let documents: Vec<_> = DOCS
        .iter()
        .map(|(paper_id, marker)| {
            let pdf_path = dir.join(format!("{paper_id}.pdf"));
            std::fs::write(&pdf_path, pdf_bytes(marker)).expect("write pdf fixture");
            da_cli::parse_batch::ParseBatchManifestDocument {
                paper_id: paper_id.to_string(),
                pdf_path: pdf_path.display().to_string(),
                endpoint: None,
            }
        })
        .collect();
    let manifest = ParseBatchManifestV1 {
        schema_version: 1,
        documents,
    };
    std::fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");
}

fn env_map(h: &Harness, mode: Option<&str>) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("DA_GROBID_URL".to_string(), format!("http://{}", h.stub.0));
    env.insert(
        "DA_PARSE_CACHE_ROOT".to_string(),
        h.cache_root.display().to_string(),
    );
    env.insert("DA_PARSE_WORKERS".to_string(), "2".to_string());
    env.insert("DA_PARSE_QUEUE_CAPACITY".to_string(), "8".to_string());
    if let Some(mode) = mode {
        env.insert("DA_PARSE_CACHE_MODE".to_string(), mode.to_string());
    }
    env
}

async fn run_with(h: &Harness, env: HashMap<String, String>, resume: bool) {
    let args = args_for(h, resume);
    let lookup = |key: &str| env.get(key).cloned();
    run_with_env(&args, &lookup)
        .await
        .expect("parse-batch run must succeed");
}

fn args_for(h: &Harness, resume: bool) -> ParseBatchArgs {
    ParseBatchArgs {
        manifest: h.manifest.clone(),
        report: h.report.clone(),
        mode: None,
        grobid_url: None,
        backend: None,
        cache_root: None,
        batch_size: None,
        workers: None,
        queue_capacity: None,
        max_in_flight: None,
        resume,
        redb_index: None,
        no_redb: false,
        redb_index_path: None,
    }
}

fn read_report(h: &Harness) -> ParseBatchReportV1 {
    let bytes = std::fs::read(&h.report).expect("report exists");
    ParseBatchReportV1::from_json(&bytes).expect("valid report")
}

fn attempts_for(h: &Harness, marker: &str) -> usize {
    h.stub
        .1
        .lock()
        .expect("request log")
        .iter()
        .filter(|(_, body)| {
            body.windows(marker.len())
                .any(|window| window == marker.as_bytes())
        })
        .count()
}

fn total_attempts(h: &Harness) -> usize {
    h.stub.1.lock().expect("request log").len()
}

fn success_identity(report: &ParseBatchReportV1, ordinal: u32) -> String {
    let entry = report
        .documents
        .iter()
        .find(|entry| entry.ordinal == ordinal)
        .expect("ordinal present");
    match &entry.result {
        Ok(success) => success.result_identity.clone(),
        Err(failure) => panic!("expected success at ordinal {ordinal}, got {failure:?}"),
    }
}

fn assert_typed_busy(failure: &ParseDocumentFailure) {
    match failure {
        ParseDocumentFailure::Backend(backend) => {
            assert_eq!(backend.code(), FailureCode::Busy);
            assert_eq!(backend.retry_after(), Some(RetryAfter::Seconds(7)));
        }
        other => panic!("expected typed backend busy failure, got {other:?}"),
    }
}

// ───────────────────────── mixed write-through run ─────────────────────────

#[tokio::test]
async fn mixed_write_through_reports_typed_outcomes_bounded_settings_and_redaction() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let report = read_report(&h);

    assert_eq!(report.schema_version, 1);
    assert_eq!(report.cache_mode, CacheMode::WriteThrough);
    assert_eq!(report.summary.admitted, 3);
    assert_eq!(report.summary.completed, 3);
    assert_eq!(report.summary.succeeded, 2);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.backend_calls, 3);
    assert_eq!(report.summary.raw_writes, 2);
    assert_eq!(report.summary.projector_writes, 2);
    assert_eq!(report.summary.raw_hits, 0);
    assert_eq!(report.summary.sink_errors, 0);
    assert_eq!(report.summary.checkpoint_errors, 0);
    // Bounded settings echo: CLI/env knobs land in the run limits.
    assert_eq!(report.limits.workers().get(), 2);
    assert_eq!(report.limits.queue_capacity().get(), 8);
    assert_eq!(report.limits.max_in_flight().get(), 2);
    assert_eq!(report.limits.max_documents().get(), 1000);
    // Bounded-load gauges never exceed the configured bounds.
    assert!(report.summary.max_in_flight_observed <= 2);
    assert!(report.summary.max_queue_depth_observed <= 8);
    // The full run advanced the cursor past every terminal document.
    assert_eq!(report.resumed_from_ordinal, 0);
    assert_eq!(report.cursor.next_ordinal(), 3);
    assert_eq!(report.cursor.input_fingerprint(), report.input_fingerprint);

    // Exactly one terminal outcome per admitted ordinal, correlated.
    assert_eq!(report.documents.len(), 3);
    for (ordinal, (paper_id, _)) in DOCS.iter().enumerate() {
        let entry = &report.documents[ordinal];
        assert_eq!(entry.ordinal, ordinal as u32);
        assert_eq!(entry.paper_id, *paper_id);
        assert_eq!(entry.cache_mode, CacheMode::WriteThrough);
    }
    // The busy document keeps its typed failure verbatim (Retry-After 7,
    // exactly ONE HTTP attempt — no hidden retry, no legacy collapse).
    assert_typed_busy(report.documents[1].result.as_ref().unwrap_err());
    assert_eq!(attempts_for(&h, MARKER_BETA), 1);
    assert_eq!(total_attempts(&h), 3);

    // Redaction: no base URL, no TEI namespace, no raw PDF/TEI bytes, no
    // physical roots anywhere in the report. (The closed backend identity
    // token "grobid-http" is sanctioned wire material — URL SCHEMES are
    // the leak signal, not the substring.)
    let report_json = std::fs::read_to_string(&h.report).expect("report readable");
    for leak in [
        "http://",
        "https://",
        "127.0.0.1",
        "www.tei-c",
        "%PDF",
        MARKER_ALPHA,
        MARKER_BETA,
        MARKER_GAMMA,
        "<TEI",
    ] {
        if let Some(index) = report_json.find(leak) {
            let start = index.saturating_sub(70);
            let end = (index + 160).min(report_json.len());
            panic!(
                "report leaks {leak} at {index}: ...{}...",
                &report_json[start..end]
            );
        }
    }
    assert!(!report_json.contains(&h.cache_root.display().to_string()));
}

// ───────────────────────── read-only replay (hits) ─────────────────────────

#[tokio::test]
async fn read_only_replay_serves_hits_without_extra_backend_calls() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let first = read_report(&h);
    let http_after_first = total_attempts(&h);

    run_with(&h, env_map(&h, Some("read-only")), false).await;
    let replay = read_report(&h);

    assert_eq!(replay.summary.raw_hits, 2);
    assert_eq!(replay.summary.projector_hits, 2);
    assert_eq!(replay.summary.raw_writes, 0);
    assert_eq!(replay.summary.projector_writes, 0);
    // Only the busy document is re-attempted (it never reached the cache).
    assert_eq!(replay.summary.backend_calls, 1);
    assert_eq!(total_attempts(&h), http_after_first + 1);
    assert_eq!(attempts_for(&h, MARKER_ALPHA), 1);
    // Semantic parity: successful terminal signals are identical across
    // modes (the projector cache served checksum-verified identical bytes).
    assert_eq!(success_identity(&replay, 0), success_identity(&first, 0));
    assert_eq!(success_identity(&replay, 2), success_identity(&first, 2));
    assert_typed_busy(replay.documents[1].result.as_ref().unwrap_err());
}

// ───────────────────────── off mode ─────────────────────────

#[tokio::test]
async fn off_mode_computes_without_any_cache_contact() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("off")), false).await;
    let report = read_report(&h);

    assert_eq!(report.cache_mode, CacheMode::Off);
    assert_eq!(report.summary.backend_calls, 3);
    assert_eq!(report.summary.raw_hits, 0);
    assert_eq!(report.summary.raw_writes, 0);
    assert_eq!(report.summary.projector_hits, 0);
    assert_eq!(report.summary.projector_writes, 0);
    assert_eq!(report.summary.succeeded, 2);
    assert_typed_busy(report.documents[1].result.as_ref().unwrap_err());
    // The cache root was never created — off never reads and never writes.
    assert!(
        !h.cache_root.exists() || std::fs::read_dir(&h.cache_root).unwrap().next().is_none(),
        "off mode must not touch the cache root"
    );
}

// ───────────────────────── refresh ─────────────────────────

#[tokio::test]
async fn refresh_bypasses_warm_entries_and_replaces_them() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let first = read_report(&h);

    run_with(&h, env_map(&h, Some("refresh")), false).await;
    let refreshed = read_report(&h);

    assert_eq!(refreshed.cache_mode, CacheMode::Refresh);
    // Hits are bypassed: every document re-executes on the backend.
    assert_eq!(refreshed.summary.backend_calls, 3);
    assert_eq!(refreshed.summary.raw_hits, 0);
    assert_eq!(refreshed.summary.projector_hits, 0);
    // Both stages are replaced with freshly computed values.
    assert_eq!(refreshed.summary.raw_writes, 2);
    assert_eq!(refreshed.summary.projector_writes, 2);
    assert_eq!(success_identity(&refreshed, 0), success_identity(&first, 0));
    assert_typed_busy(refreshed.documents[1].result.as_ref().unwrap_err());
}

// ───────────────────────── resume ─────────────────────────

#[tokio::test]
async fn resume_after_completed_run_skips_every_document_without_backend() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let first = read_report(&h);
    let http_after_first = total_attempts(&h);

    run_with(&h, env_map(&h, Some("read-only")), true).await;
    let resumed = read_report(&h);

    assert_eq!(resumed.resumed_from_ordinal, 3);
    assert_eq!(resumed.summary.skipped_on_resume, 3);
    assert_eq!(resumed.summary.completed, 0);
    assert_eq!(resumed.summary.backend_calls, 0);
    assert!(resumed.documents.is_empty());
    assert_eq!(resumed.cursor, first.cursor);
    assert_eq!(total_attempts(&h), http_after_first);
}

#[tokio::test]
async fn resume_with_foreign_fingerprint_fails_closed_before_backend() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let http_after_first = total_attempts(&h);

    // A different manifest content = a different ordered input fingerprint.
    let delta_path = h._temp.path().join("doc-delta.pdf");
    std::fs::write(&delta_path, pdf_bytes(MARKER_DELTA)).expect("delta pdf");
    let foreign_manifest = h._temp.path().join("foreign-manifest.json");
    let manifest = ParseBatchManifestV1 {
        schema_version: 1,
        documents: vec![da_cli::parse_batch::ParseBatchManifestDocument {
            paper_id: "doc-delta".to_string(),
            pdf_path: delta_path.display().to_string(),
            endpoint: None,
        }],
    };
    std::fs::write(
        &foreign_manifest,
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");

    let mut args = args_for(&h, true);
    args.manifest = foreign_manifest;
    let env = env_map(&h, Some("read-only"));
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args, &lookup)
        .await
        .expect_err("foreign fingerprint must fail closed");
    assert!(
        matches!(error, ParseBatchCliError::ResumeEvidenceUnavailable(_)),
        "{error}"
    );
    // Failed closed BEFORE the backend: no new HTTP attempts, no report rewrite.
    assert_eq!(total_attempts(&h), http_after_first);
}

// ───────────────────────── fail-closed config / manifest ─────────────────────────

#[tokio::test]
async fn invalid_config_fails_before_any_backend_call() {
    let h = harness().await;

    let mut env = env_map(&h, None);
    env.insert("DA_PARSE_WORKERS".to_string(), "0".to_string());
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args_for(&h, false), &lookup)
        .await
        .expect_err("zero workers rejected");
    assert!(
        matches!(error, ParseBatchCliError::Config(_)),
        "typed config error expected: {error}"
    );
    assert_eq!(total_attempts(&h), 0);

    let mut env = env_map(&h, None);
    env.insert("DA_PARSE_CACHE_MODE".to_string(), "   ".to_string());
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args_for(&h, false), &lookup)
        .await
        .expect_err("blank cache mode rejected");
    assert!(matches!(error, ParseBatchCliError::Config(_)), "{error}");
    assert_eq!(total_attempts(&h), 0);

    let mut env = env_map(&h, None);
    env.insert("DA_PARSE_BATCH_SIZE".to_string(), "999999".to_string());
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args_for(&h, false), &lookup)
        .await
        .expect_err("overflow batch size rejected");
    assert!(matches!(error, ParseBatchCliError::Config(_)), "{error}");
    assert_eq!(total_attempts(&h), 0);
}

#[test]
fn manifest_negatives_are_typed_errors() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("m.json");
    let write = |content: &str| {
        std::fs::write(&path, content).expect("write manifest");
        load_manifest(&path)
    };

    // Unknown field rejected.
    assert!(matches!(
        write(r#"{"schema_version":1,"documents":[{"paper_id":"a","pdf_path":"p","bogus":1}]}"#),
        Err(ParseBatchCliError::ManifestSchema(_))
    ));
    // Foreign schema version rejected.
    assert!(matches!(
        write(r#"{"schema_version":2,"documents":[]}"#),
        Err(ParseBatchCliError::ManifestSchema(_))
    ));
    // Empty batch rejected.
    assert!(matches!(
        write(r#"{"schema_version":1,"documents":[]}"#),
        Err(ParseBatchCliError::ManifestInvalid(_))
    ));
    // Duplicate ids rejected.
    assert!(matches!(
        write(
            r#"{"schema_version":1,"documents":[
                {"paper_id":"a","pdf_path":"p1"},{"paper_id":"a","pdf_path":"p2"}]}"#
        ),
        Err(ParseBatchCliError::ManifestInvalid(_))
    ));
    // Blank id rejected.
    assert!(matches!(
        write(r#"{"schema_version":1,"documents":[{"paper_id":"  ","pdf_path":"p"}]}"#),
        Err(ParseBatchCliError::ManifestInvalid(_))
    ));
    // Unknown endpoint token rejected.
    assert!(matches!(
        write(
            r#"{"schema_version":1,"documents":[{"paper_id":"a","pdf_path":"p","endpoint":"tables"}]}"#
        ),
        Err(ParseBatchCliError::ManifestInvalid(_))
    ));
    // The sanctioned closed endpoint tokens parse.
    std::fs::write(
        &path,
        r#"{"schema_version":1,"documents":[
            {"paper_id":"a","pdf_path":"p1"},
            {"paper_id":"b","pdf_path":"p2","endpoint":"header"},
            {"paper_id":"c","pdf_path":"p3","endpoint":"references"}]}"#,
    )
    .expect("write manifest");
    let entries = load_manifest(&path).expect("valid manifest");
    let endpoints: Vec<_> = entries
        .iter()
        .map(|entry| entry.endpoint.as_str())
        .collect();
    assert_eq!(endpoints, ["fulltext", "header", "references"]);
}

#[tokio::test]
async fn missing_source_pdf_fails_before_backend() {
    let h = harness().await;
    let missing = h._temp.path().join("missing.pdf");
    let manifest = ParseBatchManifestV1 {
        schema_version: 1,
        documents: vec![da_cli::parse_batch::ParseBatchManifestDocument {
            paper_id: "doc-missing".to_string(),
            pdf_path: missing.display().to_string(),
            endpoint: None,
        }],
    };
    let manifest_path = h._temp.path().join("missing-manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");
    let mut args = args_for(&h, false);
    args.manifest = manifest_path;
    let env = env_map(&h, None);
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args, &lookup)
        .await
        .expect_err("missing pdf must fail closed");
    assert!(
        matches!(error, ParseBatchCliError::SourceRead(_)),
        "{error}"
    );
    assert_eq!(total_attempts(&h), 0);
}

#[tokio::test]
async fn conflicting_redb_flags_fail_closed() {
    let h = harness().await;
    let mut args = args_for(&h, false);
    args.redb_index = Some(h._temp.path().join("accel.redb"));
    args.no_redb = true;
    let env = env_map(&h, None);
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args, &lookup)
        .await
        .expect_err("conflicting flags");
    assert_eq!(error, ParseBatchCliError::ConflictingRedbFlags);
}

// ───────────────────────── report wire contract ─────────────────────────

#[test]
fn report_deserialization_rejects_foreign_schema_and_unknown_fields() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("r.json");
    std::fs::write(
        &path,
        r#"{"schema_version":1,"input_fingerprint":"ab","cache_mode":"off",
            "limits":{"schema_version":1,"workers":1,"queue_capacity":1,"max_in_flight":1,"max_documents":1},
            "cursor":{"schema_version":1,"input_fingerprint":"ab","next_ordinal":0},
            "resumed_from_ordinal":0,
            "summary":{"schema_version":1,"admitted":0,"skipped_on_resume":0,"completed":0,
                "succeeded":0,"failed":0,"backend_calls":0,"raw_hits":0,"raw_legacy_hits":0,
                "raw_writes":0,"raw_write_failures":0,"projector_hits":0,"projector_writes":0,
                "projector_write_failures":0,"max_in_flight_observed":0,"max_queue_depth_observed":0,
                "sink_errors":0,"checkpoint_errors":0},
            "documents":[]}"#,
    )
    .expect("write report");
    let bytes = std::fs::read(&path).expect("read");
    assert!(ParseBatchReportV1::from_json(&bytes).is_ok());

    // Foreign schema version rejected.
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    value["schema_version"] = serde_json::json!(2);
    assert!(ParseBatchReportV1::from_json(&serde_json::to_vec(&value).expect("json")).is_err());

    // Unknown field rejected.
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    value["surprise"] = serde_json::json!(1);
    assert!(ParseBatchReportV1::from_json(&serde_json::to_vec(&value).expect("json")).is_err());
}

// ───────────────────────── redb accelerator (D295) ─────────────────────────

#[cfg(feature = "redb")]
#[tokio::test]
async fn redb_delete_preserves_payloads_and_rebuilds_the_accelerator() {
    let h = harness().await;
    let accelerator_path = h._temp.path().join("accel").join("accel.redb");
    let mut env = env_map(&h, Some("write-through"));
    env.insert(
        "DA_PARSE_REDB_INDEX_ENABLED".to_string(),
        "true".to_string(),
    );
    env.insert(
        "DA_PARSE_REDB_INDEX_PATH".to_string(),
        accelerator_path.display().to_string(),
    );

    // Run 1: write-through with the accelerator enabled.
    run_with(&h, env.clone(), false).await;
    assert!(accelerator_path.exists(), "accelerator file must exist");
    {
        let accelerator =
            da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path)
                .expect("open accelerator");
        let report = read_report(&h);
        assert_eq!(
            accelerator
                .load_cursor(&report.input_fingerprint)
                .expect("lookup"),
            Some(3)
        );
        let indexed = accelerator
            .manifest_entries(&report.input_fingerprint)
            .expect("manifest lookup");
        assert_eq!(
            indexed,
            Some(vec![
                (0, "doc-alpha".to_string()),
                (1, "doc-beta".to_string()),
                (2, "doc-gamma".to_string())
            ])
        );
    }

    // DELETE the accelerator: non-authoritative — canonical payloads and
    // completed evidence survive.
    std::fs::remove_file(&accelerator_path).expect("delete accelerator");

    // Run 2: resume works via the canonical report fallback…
    run_with(&h, env.clone(), true).await;
    let resumed = read_report(&h);
    assert_eq!(resumed.summary.skipped_on_resume, 3);
    assert_eq!(resumed.summary.backend_calls, 0);
    // …and the accelerator was REBUILT by the re-enabled run: the file is
    // recreated and the manifest index restored unconditionally. (A fully
    // skipped run performs no terminal work, so its cursor row is written
    // only by a run that actually advances the contiguous prefix.)
    assert!(accelerator_path.exists(), "accelerator must rebuild");
    {
        let accelerator =
            da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path)
                .expect("open rebuilt accelerator");
        assert_eq!(
            accelerator
                .manifest_entries(&resumed.input_fingerprint)
                .expect("manifest lookup"),
            Some(vec![
                (0, "doc-alpha".to_string()),
                (1, "doc-beta".to_string()),
                (2, "doc-gamma".to_string())
            ])
        );
        assert_eq!(
            accelerator
                .load_cursor(&resumed.input_fingerprint)
                .expect("lookup"),
            None,
            "no terminal work happened in a fully skipped run"
        );
    }

    // Run 3: raw/projector payloads are fully intact (read-only hits) and
    // the durable terminal work rebuilds the cursor row.
    run_with(&h, env, false).await;
    let third = read_report(&h);
    assert_eq!(third.summary.raw_hits, 2);
    assert_eq!(third.summary.projector_hits, 2);
    {
        let accelerator =
            da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path)
                .expect("open accelerator");
        assert_eq!(
            accelerator
                .load_cursor(&third.input_fingerprint)
                .expect("lookup"),
            Some(3),
            "durable cursor advancement rebuilds the accelerator row"
        );
    }
}

#[cfg(not(feature = "redb"))]
#[tokio::test]
async fn redb_enabled_without_the_feature_fails_closed() {
    let h = harness().await;
    let mut env = env_map(&h, None);
    env.insert(
        "DA_PARSE_REDB_INDEX_ENABLED".to_string(),
        "true".to_string(),
    );
    env.insert(
        "DA_PARSE_REDB_INDEX_PATH".to_string(),
        h._temp.path().join("accel.redb").display().to_string(),
    );
    let lookup = |key: &str| env.get(key).cloned();
    let error = run_with_env(&args_for(&h, false), &lookup)
        .await
        .expect_err("enabled accelerator without the feature must fail closed");
    assert_eq!(error, ParseBatchCliError::RedbNotCompiled);
    assert_eq!(total_attempts(&h), 0);
}
