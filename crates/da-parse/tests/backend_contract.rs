//! Integration contract suite for the provider-neutral parse backend seam
//! (R105, M464-z84in2 S02/T01).
//!
//! One suite, two backends: [`GrobidHttpBackend`] against a local
//! deterministic HTTP stub server (no real GROBID service) and
//! [`CachedTeiBackend`] over already-available bytes. The suite asserts:
//!
//! - verbatim byte preservation (no TEI normalization),
//! - exact backend/profile identity on every envelope,
//! - rich/structural fingerprint separation,
//! - identity independence from HTTP timeout,
//! - exactly one backend invocation per fetch (no hidden retries),
//! - fail-visible empty/malformed/backend failures (no panic, no silent skip),
//! - no physical paths / env roots in the provider-neutral request/envelope.
//!
//! Raw TEI/PDF bytes are never logged — the seam logs sizes and identity only.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use da_parse::{
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, Consolidation, CoordinateFamily, CoordinateSet,
    FailureCode, FailurePhase, FulltextDialectV1, GROBID_HTTP_BACKEND_ID, GrobidHttpBackend,
    GrobidParser, GrobidProfile, LimitKind, PageWindow, ParseBackend, ParseBackendRequest,
    ParseEndpoint, ProbeFailure, RetryAfter, TeiEnvelope,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// TEI fixture with multi-byte text and a deliberate CRLF/LF/tab/whitespace
/// mix: byte fidelity means exactly these bytes come back out.
const TEI_FIXTURE: &str = "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\">\r\n  <teiHeader>Ünïcödé — plasmid α (β-lactamase)</teiHeader>\n\t<text>  spaced  </text>\n</TEI>\r\n";

/// Deliberately malformed TEI: unclosed elements, no header, stray bytes.
const TEI_MALFORMED: &str = "<TEI><text>unclosed element <p>still open \u{7f}junk";

const PDF_FIXTURE: &[u8] = b"%PDF-1.4 contract fixture bytes";

// ---------------------------------------------------------------------------
// Local deterministic GROBID stub server (tokio TCP, no external deps)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct RecordedRequest {
    method: String,
    path: String,
    content_type: String,
    body: Vec<u8>,
}

/// Per-connection response plan of the stub server.
#[derive(Clone)]
enum ResponsePlan {
    /// `200 OK` with an `application/xml` body.
    Xml(Vec<u8>),
    /// `200 OK` with an `application/json` body (health/version probes).
    Json(Vec<u8>),
    /// `200 OK` with a `text/plain` body (plain GROBID version string).
    Plain(Vec<u8>),
    /// Status with a reason phrase and no extra headers.
    Status(u16, &'static str),
    /// Status plus explicit response headers (Retry-After contract).
    StatusWithHeaders(u16, &'static str, Vec<(&'static str, &'static str)>),
    /// Delay the successful response head to trigger the client timeout.
    Delayed(Vec<u8>, Duration),
}

type RequestLog = Arc<Mutex<Vec<RecordedRequest>>>;

/// Spawn the stub GROBID server on an ephemeral localhost port.
async fn spawn_grobid_stub(plan: ResponsePlan) -> (SocketAddr, RequestLog) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub GROBID server");
    let addr = listener.local_addr().expect("stub server address");
    let log: RequestLog = Arc::new(Mutex::new(Vec::new()));
    let log_for_task = Arc::clone(&log);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let log = Arc::clone(&log_for_task);
            let plan = plan.clone();
            tokio::spawn(serve_connection(stream, plan, log));
        }
    });
    (addr, log)
}

async fn serve_connection(mut stream: tokio::net::TcpStream, plan: ResponsePlan, log: RequestLog) {
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    log.lock().expect("request log lock").push(request);
    let (status, reason, headers, body, delay, content_type) = match plan {
        ResponsePlan::Xml(body) => (
            200u16,
            "OK",
            Vec::new(),
            body,
            None,
            "application/xml; charset=utf-8",
        ),
        ResponsePlan::Json(body) => (200u16, "OK", Vec::new(), body, None, "application/json"),
        ResponsePlan::Plain(body) => (200u16, "OK", Vec::new(), body, None, "text/plain"),
        ResponsePlan::Status(code, reason) => (
            code,
            reason,
            Vec::new(),
            Vec::new(),
            None,
            "application/xml",
        ),
        ResponsePlan::StatusWithHeaders(code, reason, headers) => {
            (code, reason, headers, Vec::new(), None, "application/xml")
        }
        ResponsePlan::Delayed(body, delay) => (
            200u16,
            "OK",
            Vec::new(),
            body,
            Some(delay),
            "application/xml; charset=utf-8",
        ),
    };
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\n");
    for (name, value) in headers {
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

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<RecordedRequest> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > (1 << 20) {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?.to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    let mut chunked = false;
    let mut content_type = String::new();
    for line in lines {
        if let Some(v) = header_value(line, "content-length") {
            content_length = v.parse().unwrap_or(0);
        } else if let Some(v) = header_value(line, "transfer-encoding") {
            chunked = v.eq_ignore_ascii_case("chunked");
        } else if let Some(v) = header_value(line, "content-type") {
            content_type = v.to_string();
        }
    }

    let mut body = buf.split_off(head_end);
    if chunked {
        while !body.ends_with(b"0\r\n\r\n") {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
            if body.len() > (32 << 20) {
                return None;
            }
        }
        body = decode_chunked(&body);
    } else {
        while body.len() < content_length {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(content_length);
    }
    Some(RecordedRequest {
        method,
        path,
        content_type,
        body,
    })
}

fn header_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}:");
    if line.len() >= prefix.len() && line[..prefix.len()].eq_ignore_ascii_case(&prefix) {
        Some(line[prefix.len()..].trim())
    } else {
        None
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = input;
    loop {
        let Some(pos) = find_subsequence(rest, b"\r\n") else {
            return out;
        };
        let size_line = String::from_utf8_lossy(&rest[..pos]);
        let Ok(size) = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16)
        else {
            return out;
        };
        rest = &rest[pos + 2..];
        if size == 0 || rest.len() < size + 2 {
            return out;
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

/// Extract `(name, value)` pairs from a recorded multipart/form-data body.
fn multipart_fields(content_type: &str, body: &[u8]) -> Vec<(String, String)> {
    let Some(boundary) = content_type
        .split("boundary=")
        .nth(1)
        .map(|b| b.trim().trim_matches('"').to_string())
    else {
        return Vec::new();
    };
    let delimiter = format!("--{boundary}");
    let text = String::from_utf8_lossy(body);
    let mut fields = Vec::new();
    for part in text.split(delimiter.as_str()) {
        let part = part.strip_prefix("\r\n").unwrap_or(part);
        if part.starts_with("--") {
            continue; // closing delimiter
        }
        let Some(split_at) = part.find("\r\n\r\n") else {
            continue;
        };
        let headers = &part[..split_at];
        let value = part[split_at + 4..].trim_end_matches("\r\n");
        let Some(name) = headers
            .split("name=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        fields.push((name.to_string(), value.to_string()));
    }
    fields
}

// ---------------------------------------------------------------------------
// Fixtures and backend builders
// ---------------------------------------------------------------------------

fn request_fixture() -> ParseBackendRequest {
    ParseBackendRequest::new("contract-paper", PDF_FIXTURE.to_vec())
}

fn http_backend(addr: SocketAddr, profile: GrobidProfile) -> GrobidHttpBackend {
    GrobidHttpBackend::new(Some(&format!("http://{addr}"))).with_profile(profile)
}

/// Shared contract helper: the backend must carry `payload` through verbatim.
async fn assert_carries_bytes_verbatim(backend: &dyn ParseBackend, payload: &[u8]) -> TeiEnvelope {
    let envelope = backend
        .fetch_tei(request_fixture())
        .await
        .expect("backend fetch must succeed");
    assert_eq!(
        envelope.raw_tei(),
        payload,
        "TEI bytes must pass through the backend verbatim (no normalization)"
    );
    envelope
}

// ---------------------------------------------------------------------------
// Contract: GROBID HTTP backend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_backend_sends_exactly_one_profile_tuned_multipart_invocation() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());

    let envelope = backend.fetch_tei(request_fixture()).await.expect("fetch");
    assert_eq!(envelope.raw_tei(), TEI_FIXTURE.as_bytes());
    assert_eq!(envelope.identity().backend_id(), GROBID_HTTP_BACKEND_ID);

    let requests = log.lock().expect("request log lock");
    assert_eq!(
        requests.len(),
        1,
        "exactly one backend invocation per fetch_tei (no hidden retries)"
    );
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/processFulltextDocument");
    assert!(
        request.content_type.starts_with("multipart/form-data"),
        "unexpected content-type: {}",
        request.content_type
    );

    let fields = multipart_fields(&request.content_type, &request.body);
    let field = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(field("consolidateCitations"), Some("1"), "rich dialect");
    assert_eq!(field("consolidateHeader"), Some("1"), "rich dialect");
    assert_eq!(
        field("teiCoordinates"),
        Some(GrobidProfile::tei_coordinates_json().as_str()),
        "coordinates must match the configuration fingerprint (D204)"
    );
    assert_eq!(field("input"), Some("%PDF-1.4 contract fixture bytes"));
    assert!(
        String::from_utf8_lossy(&request.body).contains("filename=\"paper.pdf\""),
        "PDF part must keep the stable upload filename"
    );
}

#[tokio::test]
async fn http_backend_structural_profile_sends_bounded_dialect() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::structural());

    let request = ParseBackendRequest::from_legacy_profile(
        &GrobidProfile::structural(),
        "contract-paper",
        PDF_FIXTURE.to_vec(),
    );
    let envelope = backend.fetch_tei(request).await.expect("fetch");
    assert_eq!(envelope.raw_tei(), TEI_FIXTURE.as_bytes());

    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 1);
    let fields = multipart_fields(&requests[0].content_type, &requests[0].body);
    let field = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        field("consolidateCitations"),
        Some("0"),
        "structural profile must disable remote consolidation"
    );
    assert_eq!(field("consolidateHeader"), Some("0"));
    assert_eq!(
        field("teiCoordinates"),
        Some(GrobidProfile::tei_coordinates_json().as_str()),
        "structural keeps the same coordinates wire string as its fingerprint"
    );
}

#[tokio::test]
async fn http_backend_identity_separates_profiles_and_ignores_timeout() {
    let (addr, _log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;

    let rich = http_backend(addr, GrobidProfile::rich());
    let structural = http_backend(addr, GrobidProfile::structural());
    let rich_envelope = rich.fetch_tei(request_fixture()).await.expect("fetch");
    let structural_envelope = structural
        .fetch_tei(request_fixture())
        .await
        .expect("fetch");

    assert_ne!(
        rich_envelope.identity(),
        structural_envelope.identity(),
        "rich and structural TEI must be distinguishable by identity"
    );
    assert_eq!(
        rich_envelope.identity().backend_id(),
        GROBID_HTTP_BACKEND_ID
    );
    assert_eq!(
        rich_envelope.identity().profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint()
    );
    assert_eq!(
        structural_envelope.identity().profile_fingerprint(),
        GrobidProfile::structural().configuration_fingerprint()
    );

    // Transport tuning must never leak into identity: same profile + URL,
    // different timeouts → identical identity.
    let slow = http_backend(addr, GrobidProfile::rich()).with_timeout(Duration::from_secs(300));
    let fast = http_backend(addr, GrobidProfile::rich()).with_timeout(Duration::from_secs(5));
    assert_eq!(
        slow.identity(),
        fast.identity(),
        "identity must be independent of timeout"
    );
}

#[tokio::test]
async fn http_backend_surfaces_http_error_status_fail_visibly() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Status(500, "Internal Server Error")).await;
    let backend = http_backend(addr, GrobidProfile::structural());

    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("backend failure must surface as Err, never a silent skip");
    assert_eq!(
        err.code(),
        FailureCode::ConversionFailed,
        "unexpected failure shape: {err}"
    );
    assert_eq!(err.status(), Some(500));
    assert_eq!(err.phase(), FailurePhase::AwaitResponse);
    assert_eq!(err.schema_version(), da_parse::PARSE_FAILURE_SCHEMA_VERSION);
    assert_eq!(
        log.lock().expect("request log lock").len(),
        1,
        "the failing invocation is observable, not swallowed"
    );
}

#[tokio::test]
async fn http_backend_surfaces_transport_outage_as_unavailable() {
    // Bind then immediately drop: the port is guaranteed closed.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);

    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("connection refusal must surface as Err");
    assert_eq!(
        err.code(),
        FailureCode::Transport,
        "transport outage must stay a typed Transport failure: {err}"
    );
    assert_eq!(err.phase(), FailurePhase::Connect);
}

#[tokio::test]
async fn http_backend_carries_empty_tei_verbatim_until_typed() {
    let (addr, _log) = spawn_grobid_stub(ResponsePlan::Xml(Vec::new())).await;
    let backend = http_backend(addr, GrobidProfile::rich());

    // Until slice S04 types transport absence (204/503/timeout semantics), an
    // empty 200 body is carried verbatim — observable, never normalized away
    // into a default payload and never silently skipped.
    let envelope = assert_carries_bytes_verbatim(&backend, b"").await;
    assert!(envelope.raw_tei().is_empty());
    assert_eq!(envelope.identity().backend_id(), GROBID_HTTP_BACKEND_ID);
    assert_eq!(
        envelope.identity().profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint()
    );
}

#[tokio::test]
async fn http_backend_projects_the_full_v1_java_wire_dialect() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());
    let dialect = FulltextDialectV1::from_parts(
        Consolidation::try_new(2, 1, 1).expect("sanctioned modes"),
        CoordinateSet::from_families([CoordinateFamily::Figure, CoordinateFamily::Note]),
    )
    .with_raw_citations(true)
    .with_raw_affiliations(true)
    .with_segment_sentences(true)
    .with_emit_ids(true)
    .with_page_window(Some(PageWindow::try_new(2, 5).expect("valid pages")));
    let request = ParseBackendRequest::with_dialect("extended", PDF_FIXTURE.to_vec(), dialect);

    let envelope = backend.fetch_tei(request).await.expect("extended fetch");
    assert_eq!(envelope.raw_tei(), TEI_FIXTURE.as_bytes());
    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 1, "no hidden retry");
    let fields = multipart_fields(&requests[0].content_type, &requests[0].body);
    let field = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    for (name, expected) in [
        ("consolidateHeader", "2"),
        ("consolidateCitations", "1"),
        ("consolidateFunders", "1"),
        ("includeRawCitations", "1"),
        ("includeRawAffiliations", "1"),
        ("segmentSentences", "1"),
        ("generateIDs", "1"),
        ("start", "2"),
        ("end", "5"),
        ("teiCoordinates", "[\"figure\",\"note\"]"),
    ] {
        assert_eq!(field(name), Some(expected), "Java GROBID field {name}");
    }
}

#[tokio::test]
async fn http_backend_carries_non_utf8_success_bytes_verbatim() {
    let bytes = b"<TEI>\xff\xfe raw</TEI>".to_vec();
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(bytes.clone())).await;
    let envelope = http_backend(addr, GrobidProfile::rich())
        .fetch_tei(request_fixture())
        .await
        .expect("non-UTF-8 success");
    assert_eq!(envelope.raw_tei(), bytes);
    assert_eq!(log.lock().expect("request log lock").len(), 1);
}

#[tokio::test]
async fn http_backend_maps_204_400_and_413_to_typed_failures() {
    for (status, reason, code, limit) in [
        (204, "No Content", FailureCode::NoContent, None),
        (400, "Bad Request", FailureCode::InvalidRequest, None),
        (
            413,
            "Payload Too Large",
            FailureCode::LimitExceeded,
            Some(LimitKind::RequestBytes),
        ),
    ] {
        let (addr, log) = spawn_grobid_stub(ResponsePlan::Status(status, reason)).await;
        let err = http_backend(addr, GrobidProfile::rich())
            .fetch_tei(request_fixture())
            .await
            .expect_err("typed status failure");
        assert_eq!(err.code(), code, "status {status}");
        assert_eq!(err.status(), Some(status), "status {status}");
        assert_eq!(err.enforced_limit(), limit, "status {status}");
        assert_eq!(log.lock().expect("request log lock").len(), 1, "no retry");
    }
}

#[tokio::test]
async fn http_backend_maps_503_retry_after_without_retrying() {
    for (headers, expected) in [
        (vec![("Retry-After", "42")], Some(RetryAfter::Seconds(42))),
        (
            vec![("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT")],
            Some(RetryAfter::Unparsable),
        ),
        (Vec::new(), None),
    ] {
        let plan = ResponsePlan::StatusWithHeaders(503, "Service Unavailable", headers);
        let (addr, log) = spawn_grobid_stub(plan).await;
        let err = http_backend(addr, GrobidProfile::rich())
            .fetch_tei(request_fixture())
            .await
            .expect_err("503 is typed busy");
        assert_eq!(err.code(), FailureCode::Busy);
        assert_eq!(err.status(), Some(503));
        assert_eq!(err.retry_after(), expected);
        assert_eq!(
            log.lock().expect("request log lock").len(),
            1,
            "honest no-retry 503"
        );
    }
}

#[tokio::test]
async fn http_backend_maps_delayed_response_to_distinct_timeout() {
    let plan = ResponsePlan::Delayed(TEI_FIXTURE.as_bytes().to_vec(), Duration::from_millis(250));
    let (addr, log) = spawn_grobid_stub(plan).await;
    let backend = http_backend(addr, GrobidProfile::rich()).with_timeout(Duration::from_millis(30));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("delayed response must time out");
    assert_eq!(err.code(), FailureCode::Timeout);
    assert_eq!(err.phase(), FailurePhase::AwaitResponse);
    assert_eq!(
        log.lock().expect("request log lock").len(),
        1,
        "no timeout retry"
    );
}

#[tokio::test]
async fn opt_in_request_and_response_limits_are_inclusive_and_identity_free() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
    let base = http_backend(addr, GrobidProfile::rich());
    let bounded = http_backend(addr, GrobidProfile::rich())
        .with_request_byte_limit(PDF_FIXTURE.len())
        .with_response_byte_limit(TEI_FIXTURE.len());
    assert_eq!(
        base.identity(),
        bounded.identity(),
        "limits never enter identity"
    );
    let envelope = bounded
        .fetch_tei(request_fixture())
        .await
        .expect("inclusive limits");
    assert_eq!(envelope.raw_tei(), TEI_FIXTURE.as_bytes());
    assert_eq!(log.lock().expect("request log lock").len(), 1);

    let before = log.lock().expect("request log lock").len();
    let request_err = http_backend(addr, GrobidProfile::rich())
        .with_request_byte_limit(PDF_FIXTURE.len() - 1)
        .fetch_tei(request_fixture())
        .await
        .expect_err("limit+1 request");
    assert_eq!(request_err.code(), FailureCode::LimitExceeded);
    assert_eq!(request_err.enforced_limit(), Some(LimitKind::RequestBytes));
    assert_eq!(request_err.status(), None);
    assert_eq!(
        log.lock().expect("request log lock").len(),
        before,
        "pre-wire rejection"
    );

    let response_err = http_backend(addr, GrobidProfile::rich())
        .with_response_byte_limit(TEI_FIXTURE.len() - 1)
        .fetch_tei(request_fixture())
        .await
        .expect_err("limit+1 response");
    assert_eq!(response_err.code(), FailureCode::LimitExceeded);
    assert_eq!(
        response_err.enforced_limit(),
        Some(LimitKind::ResponseBytes)
    );
    assert_eq!(response_err.phase(), FailurePhase::ReadBody);
    assert_eq!(response_err.status(), None);
    assert_eq!(log.lock().expect("request log lock").len(), before + 1);
}

// ---------------------------------------------------------------------------
// Contract: CachedTeiBackend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cached_backend_carries_bytes_and_identity_verbatim() {
    let backend = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::structural());

    let envelope = assert_carries_bytes_verbatim(&backend, TEI_FIXTURE.as_bytes()).await;
    assert_eq!(envelope.identity().backend_id(), CACHED_TEI_BACKEND_ID);
    assert_eq!(
        envelope.identity().profile_fingerprint(),
        GrobidProfile::structural().configuration_fingerprint()
    );

    // Non-UTF-8 bytes must survive too: the envelope carries raw bytes, so no
    // encoding normalization can hide payload damage.
    let non_utf8: &[u8] = b"<TEI>\xFF\xFE raw preserved</TEI>";
    let backend_raw = CachedTeiBackend::with_identity(
        non_utf8.to_vec(),
        da_parse::TeiIdentity::new(CACHED_TEI_BACKEND_ID, "archived-provenance"),
    );
    let raw_envelope = backend_raw
        .fetch_tei(request_fixture())
        .await
        .expect("fetch");
    assert_eq!(raw_envelope.raw_tei(), non_utf8);
    assert_eq!(raw_envelope.identity().backend_id(), CACHED_TEI_BACKEND_ID);

    // Stateless by construction: identical envelope on every fetch, no
    // transport involved at all.
    let again = backend.fetch_tei(request_fixture()).await.expect("fetch");
    assert_eq!(envelope, again);
}

#[tokio::test]
async fn cached_backend_identity_separates_profiles() {
    let rich = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::rich());
    let structural = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::structural());

    assert_ne!(
        rich.identity(),
        structural.identity(),
        "cached TEI must stay distinguishable by producing profile"
    );
    let rich_envelope = rich.fetch_tei(request_fixture()).await.expect("fetch");
    let structural_envelope = structural
        .fetch_tei(request_fixture())
        .await
        .expect("fetch");
    assert_ne!(rich_envelope.identity(), structural_envelope.identity());
    assert_eq!(rich_envelope.identity().backend_id(), CACHED_TEI_BACKEND_ID);
}

// ---------------------------------------------------------------------------
// Shared contract: both backends
// ---------------------------------------------------------------------------

#[tokio::test]
async fn both_backends_carries_malformed_tei_verbatim_without_panic() {
    let malformed = TEI_MALFORMED.as_bytes();
    let (addr, _log) = spawn_grobid_stub(ResponsePlan::Xml(malformed.to_vec())).await;

    let http_envelope =
        assert_carries_bytes_verbatim(&http_backend(addr, GrobidProfile::structural()), malformed)
            .await;
    let cached_envelope = assert_carries_bytes_verbatim(
        &CachedTeiBackend::new(malformed, GrobidProfile::structural()),
        malformed,
    )
    .await;

    // Same bytes in, same bytes out — the seam never repairs, trims or
    // normalizes malformed payloads (lifting owns that, total and panicking
    // never, in T02).
    assert_eq!(http_envelope.raw_tei(), cached_envelope.raw_tei());
}

#[tokio::test]
async fn request_and_envelope_stay_free_of_physical_paths() {
    // A path-looking logical id must be carried as a logical string only; the
    // envelope exposes exactly (raw bytes, identity) — no PathBuf, no env or
    // cache roots, no source paths on the provider-neutral seam.
    let path_looking = "../../data/cache/root/paper.pdf";
    let request = ParseBackendRequest::new(path_looking, PDF_FIXTURE.to_vec());
    assert_eq!(request.paper_id(), path_looking);
    assert_eq!(request.pdf_bytes(), PDF_FIXTURE);

    let backend = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::rich());
    let envelope = backend.fetch_tei(request).await.expect("fetch");
    let (raw, identity) = envelope.into_parts();
    assert_eq!(raw, TEI_FIXTURE.as_bytes());
    assert_eq!(identity.backend_id(), CACHED_TEI_BACKEND_ID);
    assert_eq!(
        identity.profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint()
    );
    let debug_shape = format!("{identity:?}");
    assert!(
        !debug_shape.contains('/') && !debug_shape.contains(".."),
        "identity debug output must not embed path-like material: {debug_shape}"
    );
}

#[tokio::test]
async fn http_backend_health_probe_accepts_json_object() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Json(br#"{"status":"UP"}"#.to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    let probe = backend.probe_health().await.expect("health json object");
    assert_eq!(probe.status, 200);
    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/health");
    assert_eq!(
        backend.identity().profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint(),
        "health probe must not alter cache identity"
    );
}

#[tokio::test]
async fn http_backend_health_probe_rejects_non_object_and_outage() {
    let (addr, _) = spawn_grobid_stub(ResponsePlan::Xml(b"<html>nope</html>".to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    assert_eq!(
        backend.probe_health().await.expect_err("xml is not health"),
        ProbeFailure::InvalidResponse
    );

    let (addr, _) = spawn_grobid_stub(ResponsePlan::Status(503, "Service Unavailable")).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    assert_eq!(
        backend.probe_health().await.expect_err("503"),
        ProbeFailure::Unavailable
    );

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    assert_eq!(
        backend.probe_health().await.expect_err("connect"),
        ProbeFailure::Unavailable
    );
}

#[tokio::test]
async fn http_backend_version_probe_accepts_json_and_plain() {
    let json = br#"{"name":"grobid","version":"0.9.0-crf","revision":"abc"}"#;
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Json(json.to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    let probe = backend.probe_version().await.expect("version json");
    assert_eq!(probe.version, "0.9.0-crf");
    assert_eq!(probe.name.as_deref(), Some("grobid"));
    assert_eq!(probe.revision.as_deref(), Some("abc"));
    assert_eq!(
        log.lock().expect("log").last().unwrap().path,
        "/api/version"
    );

    let (addr, _) = spawn_grobid_stub(ResponsePlan::Plain(b"0.9.0-crf\n".to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    let probe = backend.probe_version().await.expect("plain version");
    assert_eq!(probe.version, "0.9.0-crf");
    assert!(probe.name.is_none());

    let facade = GrobidParser::new(Some(&format!("http://{addr}")));
    let _ = facade.probe_version().await;
}

#[tokio::test]
async fn http_backend_version_probe_rejects_empty_and_html() {
    let (addr, _) = spawn_grobid_stub(ResponsePlan::Plain(b"   ".to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    assert_eq!(
        backend.probe_version().await.expect_err("blank"),
        ProbeFailure::InvalidResponse
    );

    let (addr, _) = spawn_grobid_stub(ResponsePlan::Xml(b"<TEI/>".to_vec())).await;
    let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
    assert_eq!(
        backend.probe_version().await.expect_err("xml"),
        ProbeFailure::InvalidResponse
    );
}

/// Wire-level debug contract (M506/S01/T04).
///
/// The in-module test proves `debugMode` is emitted from exactly one place in
/// the source; this proves it actually reaches the wire on the debug route and
/// only there. A source-level check cannot catch a form built elsewhere, and a
/// debug field leaking into a production parse would silently change what the
/// service computes — so the assertion is made against the recorded request
/// body, not against the builder.
#[tokio::test]
async fn debug_capture_carries_debug_mode_only_on_the_debug_route() {
    let dump = b"=== model: segmentation ===\n1\tA\t<header>\n".to_vec();
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Plain(dump.clone())).await;
    let facade = GrobidParser::new(Some(&format!("http://{addr}")));

    // Unfiltered debug request.
    let text = facade
        .debug_capture(ParseEndpoint::Fulltext, &request_fixture(), None)
        .await
        .expect("debug capture succeeds against the stub");
    assert_eq!(text.as_bytes(), dump.as_slice());

    // Filtered debug request.
    let models = vec![" segmentation ".to_string(), "date".to_string()];
    let _ = facade
        .debug_capture(ParseEndpoint::Fulltext, &request_fixture(), Some(&models))
        .await
        .expect("filtered debug capture succeeds");

    let recorded = log.lock().expect("request log lock").clone();
    assert_eq!(recorded.len(), 2, "one request per debug capture");

    let unfiltered = String::from_utf8_lossy(&recorded[0].body).to_string();
    assert!(
        unfiltered.contains("name=\"debugMode\""),
        "debug request must carry debugMode on the wire"
    );
    assert!(
        !unfiltered.contains("name=\"models\""),
        "an unfiltered debug request must not send a models field"
    );
    assert_eq!(recorded[0].path, "/api/processFulltextDocument");

    let filtered = String::from_utf8_lossy(&recorded[1].body).to_string();
    assert!(filtered.contains("name=\"debugMode\""));
    assert!(
        filtered.contains("name=\"models\""),
        "a filtered debug request must carry the models field"
    );
    // The filter is trimmed and comma-joined, so the service sees a clean
    // allowlist rather than padded names that would match nothing.
    assert!(
        filtered.contains("segmentation,date"),
        "models filter must be trimmed and comma-joined on the wire"
    );
}

/// A rejected debug filter must surface as a typed failure, never as an empty
/// dump: an empty tape that looks successful is worse than a visible error.
#[tokio::test]
async fn debug_capture_rejects_an_unknown_model_as_a_typed_invalid_request() {
    let (addr, _) = spawn_grobid_stub(ResponsePlan::Status(400, "Bad Request")).await;
    let facade = GrobidParser::new(Some(&format!("http://{addr}")));

    let models = vec!["definitely-not-a-model".to_string()];
    let failure = facade
        .debug_capture(ParseEndpoint::Fulltext, &request_fixture(), Some(&models))
        .await
        .expect_err("unknown model must fail");
    assert_eq!(failure.code(), FailureCode::InvalidRequest);
    assert_eq!(failure.phase(), FailurePhase::AwaitResponse);
}
