//! Integration suite for `da parse-batch` (R116, M464-z84in2 S06/T04).
//!
//! The full CLI flow — manifest → closed composition config → typed
//! admission → `GrobidHttpBackend` over a LOCAL deterministic HTTP stub →
//! T03 atomic payload cache inside the T02 two-stage cache → bounded
//! executor → atomic versioned result report — exercised offline, plus
//! the redb accelerator contract (feature-gated, D295).
//!
//! Objective proofs required by the S06 plan (T06 semantic parity +
//! architecture closure):
//!
//! - invalid blank/unknown/zero/overflow/conflicting config values fail
//!   BEFORE the backend is opened (typed errors, zero HTTP attempts);
//! - an offline loopback run shows a MIXED per-document report (typed busy
//!   AND malformed-response failures preserved verbatim, exactly one HTTP
//!   attempt per document), the bounded settings echo and resume;
//! - successful `ParsedArticle` payloads are byte-identical across cache
//!   modes (write-through compute vs refresh recompute via the real
//!   payload cache; off via ordered terminal identities);
//! - a raw hit re-projects after the projector stage is lost/rotated, with
//!   zero backend calls, and re-publishes byte-identical bytes;
//! - a pre-cut legacy fixture hits read-only with no HTTP and no
//!   canonical write inside the batch-composed cache root;
//! - a forced mid-run interruption resumes from the durable redb cursor
//!   against the exact input fingerprint (feature-gated, D295);
//! - deleting redb never loses raw/projector payload or completed
//!   evidence and the accelerator rebuilds from canonical evidence.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use da_adapters::parse_cache::{FileParseArtifactCache, lookup_with_legacy_parse_identity};
use da_adapters::pipeline_cache::FilePipelineCache;
use da_cli::parse_batch::{
    ParseBatchArgs, ParseBatchCliError, ParseBatchManifestV1, ParseBatchReportV1, load_manifest,
    run_with_env,
};
use da_parse::{
    CacheMode, FailureCode, GrobidProfile, PROJECT_ARTICLE_SCHEMA_FINGERPRINT, ParseArtifactCache,
    ParseBackendRequest, ParseDocumentFailure, ProjectArticlePayload, RetryAfter,
    project_article_stage_key_with_backend_version, raw_tei_stage_key_with_backend_version,
};
// Fingerprint recomputation is only exercised by the redb-gated forced
// interruption test (the interrupted run leaves NO report to read it from).
#[cfg(feature = "redb")]
use da_parse::{ParseBatchItem, ordered_input_fingerprint_with_backend_version};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageCacheKey,
};
use sha2::{Digest, Sha256};
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

/// Fixed service version the deterministic stub reports from `GET /api/version`.
const STUB_VERSION: &str = "0.9.0-crf";

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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Pre-cut legacy `ParsedArticle` fixture (pre-S06 collapsed pdf-grobid
/// identity). The legacy read seam serves payload bytes verbatim without
/// interpreting them — byte identity is the contract (same as the adapter
/// contract tests).
const LEGACY_PARSED_ARTICLE_FIXTURE: &str = r#"{"paper_id":"doc-legacy","language":"en","title":"Legacy Parsed Fixture","sections":[],"references":[],"full_text":"legacy body","figures":[],"tables":[],"annotations":[]}"#;

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
    /// 418 — outside the closed interpreted status set: typed
    /// `invalid_response`, never retried.
    Malformed,
    /// 200 OK with the liftable TEI fixture after `delay_ms` (parks one
    /// document mid-flight for the forced-interruption test).
    #[cfg_attr(not(feature = "redb"), allow(dead_code))]
    SlowTei(u64),
}

async fn spawn_stub(plans: Vec<(&'static str, Plan)>) -> (SocketAddr, RequestLog) {
    spawn_stub_with_version(plans, STUB_VERSION).await
}

/// Spawn the stub reporting a specific service version from
/// `GET /api/version` (version-isolation tests).
async fn spawn_stub_with_version(
    plans: Vec<(&'static str, Plan)>,
    version: &'static str,
) -> (SocketAddr, RequestLog) {
    spawn_stub_with_version_response(plans, Some(version)).await
}

/// Spawn a stub whose `GET /api/version` fails with 503 (probe fail-closed
/// tests: a real service outage must never yield a silent legacy identity).
async fn spawn_stub_without_version(plans: Vec<(&'static str, Plan)>) -> (SocketAddr, RequestLog) {
    spawn_stub_with_version_response(plans, None).await
}

async fn spawn_stub_with_version_response(
    plans: Vec<(&'static str, Plan)>,
    version: Option<&'static str>,
) -> (SocketAddr, RequestLog) {
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
            tokio::spawn(serve_connection(stream, plans, log, version));
        }
    });
    (addr, log)
}

async fn serve_connection(
    mut stream: tokio::net::TcpStream,
    plans: Arc<Vec<(&'static str, Plan)>>,
    log: RequestLog,
    version: Option<&'static str>,
) {
    let Some((path, body)) = read_request(&mut stream).await else {
        return;
    };
    log.lock()
        .expect("request log")
        .push((path.clone(), body.clone()));

    // `GET /api/version` is the mandatory identity probe the production CLI
    // performs before admission (ADR-076 / R121).
    if path == "/api/version" {
        let (head, version_body) = match version {
            Some(version) => {
                let body = format!("{{\"name\":\"GROBID\",\"version\":\"{version}\"}}");
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                (head, body)
            }
            None => (
                "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_string(),
                String::new(),
            ),
        };
        let mut response = head.into_bytes();
        response.extend_from_slice(version_body.as_bytes());
        let _ = stream.write_all(&response).await;
        let _ = stream.shutdown().await;
        return;
    }

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
        Some(Plan::Malformed) => (418, "I'm a Teapot", Vec::new(), Vec::new()),
        Some(Plan::SlowTei(delay_ms)) => {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            (200, "OK", Vec::new(), TEI_FIXTURE.as_bytes().to_vec())
        }
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

const DOCS: [(&str, &str); 4] = [
    ("doc-alpha", MARKER_ALPHA),
    ("doc-beta", MARKER_BETA),
    ("doc-gamma", MARKER_GAMMA),
    ("doc-delta", MARKER_DELTA),
];

struct Harness {
    _temp: TempDir,
    manifest: PathBuf,
    report: PathBuf,
    cache_root: PathBuf,
    stub: (SocketAddr, RequestLog),
}

async fn harness() -> Harness {
    // Mixed run: success / busy / success / malformed — one typed terminal
    // class per document across the whole suite.
    harness_with_plans(vec![
        (MARKER_ALPHA, Plan::Tei),
        (MARKER_BETA, Plan::Busy),
        (MARKER_GAMMA, Plan::Tei),
        (MARKER_DELTA, Plan::Malformed),
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

/// Build a harness around an already-spawned stub (probe-failure and
/// explicit-legacy tests need a stub with a broken `/api/version`).
async fn harness_with_stub(stub: (SocketAddr, RequestLog)) -> Harness {
    let temp = TempDir::new().expect("tempdir");
    let manifest = temp.path().join("manifest.json");
    let report = temp.path().join("report.json");
    let cache_root = temp.path().join("cache");
    write_default_manifest(&manifest, temp.path());
    Harness {
        _temp: temp,
        manifest,
        report,
        cache_root,
        stub,
    }
}

/// The default mixed plan: success / busy / success / malformed.
fn default_plans() -> Vec<(&'static str, Plan)> {
    vec![
        (MARKER_ALPHA, Plan::Tei),
        (MARKER_BETA, Plan::Busy),
        (MARKER_GAMMA, Plan::Tei),
        (MARKER_DELTA, Plan::Malformed),
    ]
}

/// Run one manifest against an explicit stub/cache/report combination (the
/// version-isolation test shares a cache root across two service versions).
#[allow(clippy::too_many_arguments)]
async fn run_manifest(
    manifest: &Path,
    report: &Path,
    cache_root: &Path,
    stub: &(SocketAddr, RequestLog),
    mode: &str,
    resume: bool,
) -> Result<(), ParseBatchCliError> {
    let args = ParseBatchArgs {
        manifest: manifest.to_path_buf(),
        report: report.to_path_buf(),
        mode: None,
        grobid_url: None,
        backend: None,
        cache_root: None,
        batch_size: None,
        timeout_secs: None,
        workers: None,
        queue_capacity: None,
        max_in_flight: None,
        resume,
        allow_versionless_identity: false,
        redb_index: None,
        no_redb: false,
        redb_index_path: None,
        learning_rvf_path: None,
        learning_redb_index_path: None,
    };
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("DA_GROBID_URL".to_string(), format!("http://{}", stub.0));
    env.insert(
        "DA_PARSE_CACHE_ROOT".to_string(),
        cache_root.display().to_string(),
    );
    env.insert("DA_PARSE_CACHE_MODE".to_string(), mode.to_string());
    env.insert("DA_PARSE_WORKERS".to_string(), "2".to_string());
    env.insert("DA_PARSE_QUEUE_CAPACITY".to_string(), "8".to_string());
    let lookup = |key: &str| env.get(key).cloned();
    run_with_env(&args, &lookup).await
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
        timeout_secs: None,
        workers: None,
        queue_capacity: None,
        max_in_flight: None,
        resume,
        allow_versionless_identity: false,
        redb_index: None,
        no_redb: false,
        redb_index_path: None,
        learning_rvf_path: None,
        learning_redb_index_path: None,
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

/// Count of `/api/version` identity probes (one per non-legacy run).
fn identity_probes(h: &Harness) -> usize {
    identity_probes_of(&h.stub.1)
}

fn identity_probes_of(log: &RequestLog) -> usize {
    log.lock()
        .expect("request log")
        .iter()
        .filter(|(path, _)| path == "/api/version")
        .count()
}

/// Count of parse-route attempts (excludes the identity probe).
fn parse_attempts(h: &Harness) -> usize {
    parse_attempts_of(&h.stub.1)
}

fn parse_attempts_of(log: &RequestLog) -> usize {
    log.lock()
        .expect("request log")
        .iter()
        .filter(|(path, _)| path.starts_with("/api/process"))
        .count()
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

fn assert_typed_malformed(failure: &ParseDocumentFailure) {
    match failure {
        ParseDocumentFailure::Backend(backend) => {
            assert_eq!(backend.code(), FailureCode::InvalidResponse);
            assert_eq!(backend.retry_after(), None);
        }
        other => panic!("expected typed malformed-response failure, got {other:?}"),
    }
}

// ───────────────── payload-level parity helpers (T06) ─────────────────

/// The exact fulltext request the CLI builds for one manifest document —
/// same constructor, so the same stage keys.
fn fulltext_request(paper_id: &str, marker: &str) -> ParseBackendRequest {
    ParseBackendRequest::new(paper_id, pdf_bytes(marker))
}

/// The ordered input fingerprint the CLI would compute for the default
/// manifest under `version` (`None` = explicit versionless identity).
fn fingerprint_for_default_manifest(manifest: &Path, version: Option<&str>) -> String {
    let entries = load_manifest(manifest).expect("manifest");
    let items: Vec<da_parse::ParseBatchItem> = entries
        .iter()
        .enumerate()
        .map(|(ordinal, entry)| {
            let bytes = std::fs::read(&entry.pdf_path).expect("fixture pdf");
            da_parse::ParseBatchItem::new(
                ordinal as u32,
                ParseBackendRequest::new(entry.paper_id.clone(), bytes),
            )
        })
        .collect();
    da_parse::ordered_input_fingerprint_with_backend_version(&items, version)
}

/// The (raw, projected) stage keys the batch run published for one
/// successful document (TEI fixture ⇒ known raw payload sha).
fn stage_keys_for(paper_id: &str, marker: &str) -> (String, String) {
    let request = fulltext_request(paper_id, marker);
    let pdf_sha256 = sha256_hex(&pdf_bytes(marker));
    let raw_sha256 = sha256_hex(TEI_FIXTURE.as_bytes());
    let raw_key = raw_tei_stage_key_with_backend_version(&request, Some(STUB_VERSION));
    let projected_key = project_article_stage_key_with_backend_version(
        paper_id,
        &pdf_sha256,
        &raw_sha256,
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        Some(STUB_VERSION),
    );
    (raw_key, projected_key)
}

/// Read the durable (backing-tier) raw-stage payload for `key`.
async fn read_backing_raw(h: &Harness, key: &str) -> Option<Vec<u8>> {
    FileParseArtifactCache::new(&h.cache_root)
        .read_raw(key)
        .await
        .expect("backing raw stage read")
        .map(|read| read.payload().to_vec())
}

/// Read the durable (backing-tier) projected-stage payload for `key`.
async fn read_backing_projected(h: &Harness, key: &str) -> Option<Vec<u8>> {
    FileParseArtifactCache::new(&h.cache_root)
        .read_projected(key)
        .await
        .expect("backing projected stage read")
        .map(|read| read.payload().to_vec())
}

/// Simulate projector schema rotation at the composition root: every
/// projected-stage record for the current fingerprint disappears while the
/// raw TEI stage survives (a rotated fingerprint orphans its entries).
fn remove_projected_stage_entries(root: &Path) {
    fn walk(dir: &Path, targets: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .map(|name| name.to_string_lossy() == da_parse::PROJECT_ARTICLE_STAGE_ID)
                    .unwrap_or(false)
                {
                    targets.push(path);
                } else {
                    walk(&path, targets);
                }
            }
        }
    }
    let mut targets = Vec::new();
    walk(root, &mut targets);
    assert!(
        !targets.is_empty(),
        "expected published projected entries to exist"
    );
    for path in targets {
        std::fs::remove_dir_all(path).expect("remove projected stage subtree");
    }
}

/// The current production rich-PDF parse key (`da load` vocabulary) for a
/// single-manifestation PDF source — the canonical input of the legacy
/// collapsed-identity lookup.
fn legacy_canonical_parse_key(paper_id: &str, pdf_sha: &str) -> StageCacheKey {
    StageCacheKey {
        source: SourceIdentity {
            article_id: paper_id.to_string(),
            selected_format: SourceFormat::Pdf,
            pdf_sha256: Some(pdf_sha.to_string()),
            html_sha256: None,
            acquisition_fingerprint: format!("acq-v2;pdf={pdf_sha};html=none"),
            normalization_fingerprint: pdf_sha.to_string(),
        },
        stage_id: "parse".to_string(),
        dependency_output_sha256: BTreeMap::new(),
        implementation_fingerprint: "da_adapters::GrobidParser@load-v1".to_string(),
        configuration_fingerprint: GrobidProfile::rich()
            .configuration_fingerprint()
            .to_string(),
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: "ParsedArticle.v1".to_string(),
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
    assert_eq!(report.summary.admitted, 4);
    assert_eq!(report.summary.completed, 4);
    assert_eq!(report.summary.succeeded, 2);
    assert_eq!(report.summary.failed, 2);
    assert_eq!(report.summary.backend_calls, 4);
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
    assert_eq!(report.cursor.next_ordinal(), 4);
    assert_eq!(report.cursor.input_fingerprint(), report.input_fingerprint);

    // Exactly one terminal outcome per admitted ordinal, correlated.
    assert_eq!(report.documents.len(), 4);
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
    // The malformed document keeps its typed invalid_response failure —
    // also exactly one HTTP attempt — and later documents still ran
    // (later documents survive earlier failures).
    assert_typed_malformed(report.documents[3].result.as_ref().unwrap_err());
    assert_eq!(attempts_for(&h, MARKER_DELTA), 1);
    assert_eq!(parse_attempts(&h), 4);
    assert_eq!(identity_probes(&h), 1, "exactly one version probe per run");

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
        MARKER_DELTA,
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
    let http_after_first = parse_attempts(&h);

    run_with(&h, env_map(&h, Some("read-only")), false).await;
    let replay = read_report(&h);

    assert_eq!(replay.summary.raw_hits, 2);
    assert_eq!(replay.summary.projector_hits, 2);
    assert_eq!(replay.summary.raw_writes, 0);
    assert_eq!(replay.summary.projector_writes, 0);
    // Only the busy/malformed documents are re-attempted (neither ever
    // reached the cache).
    assert_eq!(replay.summary.backend_calls, 2);
    assert_eq!(parse_attempts(&h), http_after_first + 2);
    assert_eq!(identity_probes(&h), 2, "each run probes once");
    assert_eq!(attempts_for(&h, MARKER_ALPHA), 1);
    // Semantic parity: successful terminal signals are identical across
    // modes (the projector cache served checksum-verified identical bytes).
    assert_eq!(success_identity(&replay, 0), success_identity(&first, 0));
    assert_eq!(success_identity(&replay, 2), success_identity(&first, 2));
    assert_typed_busy(replay.documents[1].result.as_ref().unwrap_err());
    assert_typed_malformed(replay.documents[3].result.as_ref().unwrap_err());
}

// ───────────────────────── off mode ─────────────────────────

#[tokio::test]
async fn off_mode_computes_without_any_cache_contact() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("off")), false).await;
    let report = read_report(&h);

    assert_eq!(report.cache_mode, CacheMode::Off);
    assert_eq!(report.summary.backend_calls, 4);
    assert_eq!(report.summary.raw_hits, 0);
    assert_eq!(report.summary.raw_writes, 0);
    assert_eq!(report.summary.projector_hits, 0);
    assert_eq!(report.summary.projector_writes, 0);
    assert_eq!(report.summary.succeeded, 2);
    assert_typed_busy(report.documents[1].result.as_ref().unwrap_err());
    assert_typed_malformed(report.documents[3].result.as_ref().unwrap_err());
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
    assert_eq!(refreshed.summary.backend_calls, 4);
    assert_eq!(refreshed.summary.raw_hits, 0);
    assert_eq!(refreshed.summary.projector_hits, 0);
    // Both stages are replaced with freshly computed values.
    assert_eq!(refreshed.summary.raw_writes, 2);
    assert_eq!(refreshed.summary.projector_writes, 2);
    assert_eq!(success_identity(&refreshed, 0), success_identity(&first, 0));
    assert_eq!(success_identity(&refreshed, 2), success_identity(&first, 2));
    assert_typed_busy(refreshed.documents[1].result.as_ref().unwrap_err());
    assert_typed_malformed(refreshed.documents[3].result.as_ref().unwrap_err());
}

// ───────────────────────── resume ─────────────────────────

#[tokio::test]
async fn resume_after_completed_run_skips_every_document_without_backend() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let first = read_report(&h);
    let http_after_first = parse_attempts(&h);

    run_with(&h, env_map(&h, Some("read-only")), true).await;
    let resumed = read_report(&h);

    assert_eq!(resumed.resumed_from_ordinal, 4);
    assert_eq!(resumed.summary.skipped_on_resume, 4);
    assert_eq!(resumed.summary.completed, 0);
    assert_eq!(resumed.summary.backend_calls, 0);
    assert!(resumed.documents.is_empty());
    assert_eq!(resumed.cursor, first.cursor);
    assert_eq!(parse_attempts(&h), http_after_first);
    assert_eq!(identity_probes(&h), 2);
}

#[tokio::test]
async fn resume_with_foreign_fingerprint_fails_closed_before_backend() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let http_after_first = parse_attempts(&h);

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
    // Failed closed BEFORE any parse work (only the identity probe ran):
    // no new parse attempts, no report rewrite.
    assert_eq!(parse_attempts(&h), http_after_first);
}

// ───────────────────── backend version identity (R121) ─────────────────────

#[tokio::test]
async fn probed_backend_version_is_stamped_and_separates_from_the_legacy_identity() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let report = read_report(&h);

    // The probed service version is stamped into the canonical report and
    // enters the ordered input fingerprint.
    assert_eq!(report.backend_version.as_deref(), Some(STUB_VERSION));
    assert_eq!(identity_probes(&h), 1, "one identity probe per run");
    let versioned = fingerprint_for_default_manifest(&h.manifest, Some(STUB_VERSION));
    let versionless = fingerprint_for_default_manifest(&h.manifest, None);
    assert_eq!(report.input_fingerprint, versioned);
    assert_ne!(
        versioned, versionless,
        "the probed version must change the input fingerprint"
    );
}

#[tokio::test]
async fn version_probe_failure_fails_closed_without_a_legacy_fallback() {
    let h = harness_with_stub(spawn_stub_without_version(default_plans()).await).await;
    let env = env_map(&h, Some("write-through"));
    let lookup = |key: &str| env.get(key).cloned();

    let error = run_with_env(&args_for(&h, false), &lookup)
        .await
        .expect_err("a failed /api/version probe must fail the run");
    assert_eq!(
        error,
        ParseBatchCliError::BackendVersionUnavailable(da_parse::ProbeFailure::Unavailable)
    );
    // No parse work and no report before the producer identity is known.
    assert_eq!(parse_attempts(&h), 0);
    assert_eq!(identity_probes(&h), 1);
    assert!(!h.report.exists(), "no report on a fail-closed probe");
}

#[tokio::test]
async fn allow_versionless_identity_is_the_only_explicit_historical_path() {
    let h = harness_with_stub(spawn_stub_without_version(default_plans()).await).await;
    let mut args = args_for(&h, false);
    args.allow_versionless_identity = true;
    let env = env_map(&h, Some("write-through"));
    let lookup = |key: &str| env.get(key).cloned();

    run_with_env(&args, &lookup)
        .await
        .expect("the explicit versionless identity must run without a probe");
    let report = read_report(&h);
    assert_eq!(
        report.backend_version, None,
        "the explicit historical identity stays unversioned"
    );
    assert_eq!(identity_probes(&h), 0, "explicit legacy never probes");
    assert_eq!(parse_attempts(&h), 4);
    assert_eq!(
        report.input_fingerprint,
        fingerprint_for_default_manifest(&h.manifest, None),
        "historical identity reproduces the pre-version fingerprint exactly"
    );
}

#[tokio::test]
async fn different_backend_versions_do_not_share_cache_or_resume_evidence() {
    let temp = TempDir::new().expect("tempdir");
    let manifest = temp.path().join("manifest.json");
    write_default_manifest(&manifest, temp.path());
    let cache_root = temp.path().join("cache");
    let report_v1 = temp.path().join("report-v1.json");
    let report_v2 = temp.path().join("report-v2.json");
    let stub_v1 = spawn_stub_with_version(default_plans(), "0.9.0-crf").await;
    let stub_v2 = spawn_stub_with_version(default_plans(), "0.9.1").await;

    // Version A populates the shared cache root.
    run_manifest(
        &manifest,
        &report_v1,
        &cache_root,
        &stub_v1,
        "write-through",
        false,
    )
    .await
    .expect("version A run");
    let first = ParseBatchReportV1::from_json(&std::fs::read(&report_v1).unwrap()).unwrap();
    assert_eq!(first.backend_version.as_deref(), Some("0.9.0-crf"));
    assert_eq!(parse_attempts_of(&stub_v1.1), 4);

    // Version B over the SAME inputs and cache root must not serve A's
    // raw or projection entries.
    run_manifest(
        &manifest,
        &report_v2,
        &cache_root,
        &stub_v2,
        "read-only",
        false,
    )
    .await
    .expect("version B run");
    let second = ParseBatchReportV1::from_json(&std::fs::read(&report_v2).unwrap()).unwrap();
    assert_eq!(second.backend_version.as_deref(), Some("0.9.1"));
    assert_eq!(second.summary.raw_hits, 0, "version B shares no raw entry");
    assert_eq!(
        second.summary.projector_hits, 0,
        "version B shares no projection entry"
    );
    assert_eq!(
        second.summary.backend_calls, 4,
        "version B recomputes every document"
    );
    assert_eq!(parse_attempts_of(&stub_v2.1), 4);
    assert_ne!(first.input_fingerprint, second.input_fingerprint);

    // Version B cannot resume from version A evidence (foreign fingerprint).
    let error = run_manifest(
        &manifest,
        &report_v1,
        &cache_root,
        &stub_v2,
        "read-only",
        true,
    )
    .await
    .expect_err("cross-version resume must fail closed");
    assert!(
        matches!(error, ParseBatchCliError::ResumeEvidenceUnavailable(_)),
        "{error}"
    );

    // Same-version resume reuses the exact evidence (no backend calls).
    run_manifest(
        &manifest,
        &report_v2,
        &cache_root,
        &stub_v2,
        "read-only",
        true,
    )
    .await
    .expect("same-version resume");
    let resumed = ParseBatchReportV1::from_json(&std::fs::read(&report_v2).unwrap()).unwrap();
    assert_eq!(resumed.summary.skipped_on_resume, 4);
    assert_eq!(resumed.summary.backend_calls, 0);
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
            Some(4)
        );
        let indexed = accelerator
            .manifest_entries(&report.input_fingerprint)
            .expect("manifest lookup");
        assert_eq!(
            indexed,
            Some(vec![
                (0, "doc-alpha".to_string()),
                (1, "doc-beta".to_string()),
                (2, "doc-gamma".to_string()),
                (3, "doc-delta".to_string())
            ])
        );
    }

    // DELETE the accelerator: non-authoritative — canonical payloads and
    // completed evidence survive.
    std::fs::remove_file(&accelerator_path).expect("delete accelerator");

    // Run 2: resume works via the canonical report fallback…
    run_with(&h, env.clone(), true).await;
    let resumed = read_report(&h);
    assert_eq!(resumed.summary.skipped_on_resume, 4);
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
                (2, "doc-gamma".to_string()),
                (3, "doc-delta".to_string())
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
            Some(4),
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

// ───────────────── T06: semantic parity across cache modes ─────────────────

#[tokio::test]
async fn parsed_article_payloads_are_byte_identical_across_cache_modes() {
    // Write-through: fresh end-to-end compute, both stages published.
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let write_through = read_report(&h);
    let (alpha_raw, alpha_projected) = stage_keys_for("doc-alpha", MARKER_ALPHA);
    let (_, gamma_projected) = stage_keys_for("doc-gamma", MARKER_GAMMA);
    let wt_alpha_raw = read_backing_raw(&h, &alpha_raw)
        .await
        .expect("raw published");
    let wt_alpha_projected = read_backing_projected(&h, &alpha_projected)
        .await
        .expect("projected published");
    let wt_gamma_projected = read_backing_projected(&h, &gamma_projected)
        .await
        .expect("projected published");

    // The published projection binds the current schema fingerprint, the
    // exact raw TEI bytes and the document identity, and decodes.
    let alpha_pdf_sha = sha256_hex(&pdf_bytes(MARKER_ALPHA));
    let alpha_raw_sha = sha256_hex(TEI_FIXTURE.as_bytes());
    ProjectArticlePayload::decode_and_verify(
        &wt_alpha_projected,
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &alpha_raw_sha,
        "doc-alpha",
        &alpha_pdf_sha,
    )
    .expect("published projected payload verifies");

    // Refresh: bypasses every hit and REPLACES both stages with freshly
    // computed values — the recomputation must be byte-identical.
    run_with(&h, env_map(&h, Some("refresh")), false).await;
    let refreshed = read_report(&h);
    assert_eq!(
        read_backing_raw(&h, &alpha_raw).await,
        Some(wt_alpha_raw.clone()),
        "raw TEI bytes are verbatim across computing modes"
    );
    assert_eq!(
        read_backing_projected(&h, &alpha_projected).await,
        Some(wt_alpha_projected.clone()),
        "ParsedArticle payload bytes are mode-independent"
    );
    assert_eq!(
        read_backing_projected(&h, &gamma_projected).await,
        Some(wt_gamma_projected.clone()),
        "ParsedArticle payload bytes are mode-independent"
    );
    assert_eq!(
        success_identity(&refreshed, 0),
        success_identity(&write_through, 0)
    );
    assert_eq!(
        success_identity(&refreshed, 2),
        success_identity(&write_through, 2)
    );
    assert_typed_busy(refreshed.documents[1].result.as_ref().unwrap_err());
    assert_typed_malformed(refreshed.documents[3].result.as_ref().unwrap_err());

    // Off: zero cache contact; the same inputs show identical ordered
    // terminal identities (semantic parity at the report surface).
    let off_h = harness().await;
    run_with(&off_h, env_map(&off_h, Some("off")), false).await;
    let off = read_report(&off_h);
    assert_eq!(off.documents.len(), write_through.documents.len());
    for (ordinal, _) in DOCS.iter().enumerate() {
        match (
            &off.documents[ordinal].result,
            &write_through.documents[ordinal].result,
        ) {
            (Ok(left), Ok(right)) => assert_eq!(
                left.result_identity, right.result_identity,
                "semantic drift at ordinal {ordinal}"
            ),
            (Err(left), Err(right)) => {
                assert_eq!(left, right, "typed failure drift at ordinal {ordinal}")
            }
            _ => panic!("terminal class drift at ordinal {ordinal}"),
        }
    }
    assert!(
        !off_h.cache_root.exists()
            || std::fs::read_dir(&off_h.cache_root)
                .unwrap()
                .next()
                .is_none(),
        "off mode must not touch the cache root"
    );
}

// ───────────────── T06: raw-hit re-projection after projector rotation ─────────────────

#[tokio::test]
async fn raw_hits_reproject_after_projector_stage_loss_without_backend() {
    let h = harness().await;
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let first = read_report(&h);
    let (alpha_raw, alpha_projected) = stage_keys_for("doc-alpha", MARKER_ALPHA);
    let (_, gamma_projected) = stage_keys_for("doc-gamma", MARKER_GAMMA);
    let projected_alpha_before = read_backing_projected(&h, &alpha_projected)
        .await
        .expect("projected entry");
    let projected_gamma_before = read_backing_projected(&h, &gamma_projected)
        .await
        .expect("projected entry");
    let raw_alpha_before = read_backing_raw(&h, &alpha_raw).await.expect("raw entry");

    // Projector schema rotation in effect: projected records are gone for
    // the current fingerprint while the raw TEI stage survives.
    remove_projected_stage_entries(&h.cache_root);
    assert!(read_backing_projected(&h, &alpha_projected).await.is_none());

    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let reprojected = read_report(&h);
    // Raw hits carried the re-projection: zero backend calls for the two
    // successful documents (only busy/malformed documents re-fetch).
    assert_eq!(reprojected.summary.raw_hits, 2);
    assert_eq!(reprojected.summary.projector_hits, 0);
    assert_eq!(
        reprojected.summary.raw_writes, 0,
        "raw hits are never rewritten"
    );
    assert_eq!(
        reprojected.summary.projector_writes, 2,
        "re-projection re-publishes the projector stage"
    );
    assert_eq!(reprojected.summary.backend_calls, 2);
    assert_eq!(
        attempts_for(&h, MARKER_ALPHA),
        1,
        "raw hit never re-fetches"
    );
    assert_eq!(
        attempts_for(&h, MARKER_GAMMA),
        1,
        "raw hit never re-fetches"
    );
    // Deterministic projector: re-projected bytes equal the originals and
    // the raw stage is untouched.
    assert_eq!(
        read_backing_projected(&h, &alpha_projected).await,
        Some(projected_alpha_before)
    );
    assert_eq!(
        read_backing_projected(&h, &gamma_projected).await,
        Some(projected_gamma_before)
    );
    assert_eq!(
        read_backing_raw(&h, &alpha_raw).await,
        Some(raw_alpha_before)
    );
    // Terminal identities unchanged; failures stay typed.
    assert_eq!(
        success_identity(&reprojected, 0),
        success_identity(&first, 0)
    );
    assert_eq!(
        success_identity(&reprojected, 2),
        success_identity(&first, 2)
    );
    assert_typed_busy(reprojected.documents[1].result.as_ref().unwrap_err());
    assert_typed_malformed(reprojected.documents[3].result.as_ref().unwrap_err());
}

// ───────────────── T06: pre-cut legacy fixture inside the batch root ─────────────────

#[tokio::test]
async fn precut_legacy_fixture_hits_without_http_or_canonical_write() {
    let h = harness().await;
    // A pre-cut legacy record (pre-S06 collapsed pdf-grobid identity) is
    // seeded into the batch cache root before any run happens.
    let pipeline = FilePipelineCache::new(&h.cache_root);
    let pdf_sha = sha256_hex(&pdf_bytes(MARKER_ALPHA));
    let canonical = legacy_canonical_parse_key("doc-legacy", &pdf_sha);
    let mut legacy_key = canonical.clone();
    legacy_key.source.acquisition_fingerprint = pdf_sha.clone();
    legacy_key.configuration_fingerprint = "pdf-grobid".to_string();
    pipeline
        .store_success(
            &legacy_key,
            LEGACY_PARSED_ARTICLE_FIXTURE.as_bytes(),
            BTreeMap::new(),
            "2026-08-29T00:00:00Z",
        )
        .expect("seed legacy fixture");

    // Real batch composition over the same root: the write-through run
    // must not disturb the legacy record.
    run_with(&h, env_map(&h, Some("write-through")), false).await;
    let attempts_after_batch = total_attempts(&h);

    // The legacy hit: read-only, resolved from the collapsed pdf-grobid
    // identity, bytes verbatim, no canonical record, no HTTP.
    let outcome = lookup_with_legacy_parse_identity(
        &pipeline,
        &canonical,
        false,
        GrobidProfile::rich(),
        "2026-08-29T00:01:00Z",
    )
    .expect("legacy lookup");
    let CacheLookup::Hit(hit) = outcome.lookup else {
        panic!("pre-cut legacy fixture must hit under the collapsed identity")
    };
    assert_eq!(
        hit.payload,
        LEGACY_PARSED_ARTICLE_FIXTURE.as_bytes(),
        "legacy fixture bytes round-trip verbatim"
    );
    assert_eq!(outcome.resolved_from.as_deref(), Some("pdf-grobid"));
    assert_eq!(
        pipeline.manifest("doc-legacy").expect("manifest").len(),
        1,
        "a legacy hit never creates a canonical record"
    );
    assert_eq!(
        total_attempts(&h),
        attempts_after_batch,
        "no HTTP for the legacy document"
    );
}

// ───────────────── T06: forced interruption + durable redb resume ─────────────────

#[cfg(feature = "redb")]
#[tokio::test]
async fn forced_interruption_resumes_from_the_durable_redb_cursor() {
    let h = harness_with_plans(vec![
        (MARKER_ALPHA, Plan::Tei),
        (MARKER_BETA, Plan::SlowTei(3000)),
        (MARKER_GAMMA, Plan::Tei),
        (MARKER_DELTA, Plan::Malformed),
    ])
    .await;
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

    // Run 1: interrupted mid-flight. doc-alpha (ordinal 0) reaches durable
    // terminal state; doc-beta (ordinal 1) stays parked on the stub, so
    // the contiguous durable prefix freezes at 1. The watcher races the
    // run and wins as soon as alpha is durably published (+ one beat for
    // the sink ack + checkpoint save); the losing run future is DROPPED —
    // the forced cancellation.
    // Box::pin so the forced `drop` below REALLY cancels the run: a
    // stack-pinned future (tokio::pin!) survives `drop(Pin<&mut>)` and
    // would keep the run frame — and its accelerator Arc — alive until the
    // end of the test scope, blocking the resume's accelerator open.
    let mut run_fut = Box::pin(run_with(&h, env.clone(), false));
    let (_, alpha_projected) = stage_keys_for("doc-alpha", MARKER_ALPHA);
    let watcher = async {
        loop {
            if read_backing_projected(&h, &alpha_projected).await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    tokio::select! {
        result = &mut run_fut => panic!(
            "run finished before the forced interruption: {result:?}"
        ),
        () = watcher => {}
    }
    drop(run_fut);
    // An interrupted run never writes the terminal report.
    assert!(
        !h.report.exists(),
        "interrupted run must not write a report"
    );

    // Dropping the run future detaches the executor workers (the true
    // process-death analog is process exit): they drain the parked
    // documents durably while the interrupted run itself never returns —
    // no report, no run summary.
    let (_, beta_projected) = stage_keys_for("doc-beta", MARKER_BETA);
    let (_, gamma_projected) = stage_keys_for("doc-gamma", MARKER_GAMMA);
    let mut drained = false;
    for _ in 0..400 {
        if read_backing_projected(&h, &beta_projected).await.is_some()
            && read_backing_projected(&h, &gamma_projected).await.is_some()
        {
            drained = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(drained, "interrupted run never drained its durable work");
    // True process death releases the accelerator file lock via the OS at
    // process exit; the in-process analog is the detached workers exiting on
    // the closed admission channel and dropping the last checkpoint handles.
    // Poll for the lock instead of guessing a fixed beat.
    let mut lock_free = false;
    for attempt in 0..150 {
        match da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path) {
            Ok(probe) => {
                drop(probe);
                lock_free = true;
                println!("accelerator lock free after {attempt} polls");
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    assert!(
        lock_free,
        "detached workers never released the accelerator file lock"
    );

    // The input fingerprint the interrupted run worked on — computed
    // independently here, since the interrupted run left NO report.
    let fingerprint = {
        let items: Vec<ParseBatchItem> = DOCS
            .iter()
            .map(|(paper_id, marker)| fulltext_request(paper_id, marker))
            .enumerate()
            .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request))
            .collect();
        ordered_input_fingerprint_with_backend_version(&items, Some(STUB_VERSION))
    };

    // Run 2: resume against the exact input fingerprint. NO report exists,
    // so the resume evidence can only be the redb accelerator's durable
    // cursor — the accelerator-first contract after an interrupted run.
    run_with(&h, env.clone(), true).await;
    let resumed = read_report(&h);
    assert_eq!(resumed.resumed_from_ordinal, 4);
    assert_eq!(resumed.summary.skipped_on_resume, 4);
    assert!(resumed.documents.is_empty());
    assert_eq!(resumed.summary.backend_calls, 0);
    assert_eq!(resumed.cursor.next_ordinal(), 4);
    assert_eq!(resumed.input_fingerprint, fingerprint);
    // Exactly one HTTP attempt per document across the interruption.
    assert_eq!(attempts_for(&h, MARKER_ALPHA), 1);
    assert_eq!(attempts_for(&h, MARKER_BETA), 1);
    assert_eq!(attempts_for(&h, MARKER_GAMMA), 1);
    assert_eq!(attempts_for(&h, MARKER_DELTA), 1);
    // The accelerator still carries the exact manifest index.
    {
        let accelerator =
            da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path)
                .expect("open accelerator");
        assert_eq!(
            accelerator
                .load_cursor(&resumed.input_fingerprint)
                .expect("lookup"),
            Some(4),
            "the fully skipped resume leaves the durable cursor intact"
        );
        let indexed = accelerator
            .manifest_entries(&resumed.input_fingerprint)
            .expect("manifest lookup");
        assert_eq!(
            indexed,
            Some(vec![
                (0, "doc-alpha".to_string()),
                (1, "doc-beta".to_string()),
                (2, "doc-gamma".to_string()),
                (3, "doc-delta".to_string())
            ])
        );
    }
}

/// End-to-end crash scenario for unpublished content (M506/S02/T04).
///
/// The sibling test above pins the legitimate case: an interrupted
/// write-through run leaves a durable cursor that resume may trust, because
/// the payloads are already durable in the artifact cache. This test covers
/// the complementary case the slice demo names — an interruption where the
/// run published nothing at all (read-only mode), so no document's result
/// exists in the cache and no terminal report was written either.
///
/// The invariant: resume must never report those documents as already done.
/// Skipping them would lose the work silently, which is exactly the failure
/// the durable-cursor rule exists to prevent. Either safe outcome is accepted
/// here — failing closed for lack of evidence, or redoing the documents — and
/// only silent skipping fails the test.
#[cfg(feature = "redb")]
#[tokio::test]
async fn interrupted_unpublished_run_never_lets_resume_skip_the_work() {
    let h = harness_with_plans(vec![
        (MARKER_ALPHA, Plan::Tei),
        (MARKER_BETA, Plan::SlowTei(3000)),
        (MARKER_GAMMA, Plan::Tei),
        (MARKER_DELTA, Plan::Tei),
    ])
    .await;
    let accelerator_path = h._temp.path().join("accel-unpublished").join("accel.redb");
    // read-only: content is computed and cached nowhere.
    let mut env = env_map(&h, Some("read-only"));
    env.insert(
        "DA_PARSE_REDB_INDEX_ENABLED".to_string(),
        "true".to_string(),
    );
    env.insert(
        "DA_PARSE_REDB_INDEX_PATH".to_string(),
        accelerator_path.display().to_string(),
    );

    // Run 1: interrupted once the slow document is actually in flight.
    //
    // The trigger cannot be "alpha's projection exists": read-only publishes
    // nothing, so that condition never becomes true and the run would simply
    // finish. Waiting on the backend call for the slow document pins a moment
    // where the run is genuinely mid-flight.
    let mut run_fut = Box::pin(run_with(&h, env.clone(), false));
    let watcher = async {
        loop {
            if attempts_for(&h, MARKER_BETA) >= 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        // One beat for the published prefix to settle before cancelling.
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    tokio::select! {
        result = &mut run_fut => panic!("run finished before the forced interruption: {result:?}"),
        () = watcher => {}
    }
    drop(run_fut);

    // The premise: read-only published nothing, so no content is recoverable.
    let (_, alpha_projected) = stage_keys_for("doc-alpha", MARKER_ALPHA);
    assert!(
        read_backing_projected(&h, &alpha_projected).await.is_none(),
        "read-only must not publish any content"
    );
    assert!(
        !h.report.exists(),
        "an interrupted run writes no terminal report"
    );

    // Release any detached workers' accelerator handles, as at process exit.
    for _ in 0..150 {
        match da_adapters::parse_cursor::ParseCursorAccelerator::open(&accelerator_path) {
            Ok(probe) => {
                drop(probe);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    // Run 2: resume. Either it fails closed (no evidence) or it redoes work;
    // it must never claim the documents were already done.
    let outcome = run_with_env(&args_for(&h, true), &|key: &str| env.get(key).cloned()).await;
    match outcome {
        Err(error) => {
            let text = error.to_string();
            assert!(
                text.contains("resume"),
                "a refusal must be a typed resume-evidence failure, got: {text}"
            );
        }
        Ok(()) => {
            let resumed = read_report(&h);
            assert_eq!(
                resumed.summary.skipped_on_resume, 0,
                "nothing was published, so resume must not skip any document"
            );
            assert!(
                !resumed.documents.is_empty(),
                "resume must redo the work it cannot recover"
            );
        }
    }
}
