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
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, GROBID_HTTP_BACKEND_ID, GrobidHttpBackend,
    GrobidProfile, ParseBackend, ParseBackendRequest, TeiEnvelope,
};
use da_ports::parser::ParserError;
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
    /// Non-success status with a reason phrase.
    Status(u16, &'static str),
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
    let (status, reason, body) = match plan {
        ResponsePlan::Xml(body) => (200u16, "OK", body),
        ResponsePlan::Status(code, reason) => (code, reason, Vec::new()),
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
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

    let envelope = backend.fetch_tei(request_fixture()).await.expect("fetch");
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
    assert!(
        matches!(err, ParserError::ParseFailed(ref msg) if msg.contains("GROBID HTTP 500")),
        "unexpected error shape: {err:?}"
    );
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
    assert!(
        matches!(err, ParserError::Unavailable(_)),
        "transport outage must keep the Unavailable mapping: {err:?}"
    );
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
