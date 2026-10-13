//! Endpoint proxy contract (M464-z84in2 S05/T02, R113): the closed header
//! and references operations ride the SAME endpoint-neutral one-shot multipart
//! exchange as the fulltext path.
//!
//! The suite asserts, per proxy endpoint (against a local deterministic HTTP
//! stub — no real GROBID service):
//!
//! - exact static Java routes (`/api/processHeaderDocument`,
//!   `/api/processReferences`) and POST method — route selection is the
//!   closed [`ParseEndpoint`] mapping, never a caller-controlled URL,
//! - exact per-endpoint multipart allowlists (header: `input`,
//!   `consolidateHeader`, optional `includeRawAffiliations`; references:
//!   `input`, `consolidateCitations`, optional `includeRawCitations`) —
//!   never `teiCoordinates` nor a foreign endpoint's knob,
//! - legacy rich/structural profiles projecting onto their single axis,
//! - raw switches emitting only their own wire field and rotating only
//!   their own identity,
//! - verbatim partial TEI envelopes (empty and non-UTF-8 included) stamped
//!   with the S05 endpoint/profile/result identity, pairwise distinct per
//!   operation for one profile, directly comparable as [`TeiEnvelope`]s,
//! - the shared closed typed-outcome ladder: 200 / 204 / 400 / 413 / 503
//!   with bounded numeric/malformed/absent `Retry-After` / timeout vs
//!   connect / response streaming limit / 5xx conversion / unexpected
//!   status,
//! - exactly-once semantics: zero requests on a request-limit breach,
//!   exactly one request on a response-limit breach, and no branch ever
//!   exceeding one request (no hidden retry, no queue, no batch policy),
//! - redaction: failure Display/Debug never carry a base URL or payload.
//!
//! `backend_contract.rs` stays the fulltext regression oracle.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use da_parse::{
    FailureCode, FailurePhase, GROBID_HTTP_BACKEND_ID, GrobidHttpBackend, GrobidProfile,
    HEADER_REQUEST_SCHEMA_VERSION, HeaderDialectV1, LimitKind, ParseBackend, ParseBackendRequest,
    ParseEndpoint, REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1, RetryAfter, TeiEnvelope,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Full TEI fixture (used for failure plans where the body is irrelevant).
const TEI_FIXTURE: &str = "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\">\r\n  <teiHeader>Ünïcödé — plasmid α</teiHeader>\n\t<text>  spaced  </text>\n</TEI>\r\n";

/// Partial TEI the header endpoint returns: only the bibliographic header.
const HEADER_TEI_FIXTURE: &str = "<teiHeader>\n  <fileDesc><titleStmt><title>Proxy header α</title></titleStmt></fileDesc>\n</teiHeader>\n";

/// Partial TEI the references endpoint returns: only the `<listBibl>`.
const LIST_BIBL_TEI_FIXTURE: &str =
    "<listBibl>\n  <bibl><title>Referenced work β</title></bibl>\n</listBibl>\n";

const PDF_FIXTURE: &[u8] = b"%PDF-1.4 endpoint proxy fixture bytes";

/// The two proxy endpoints under contract (fulltext stays in
/// `backend_contract.rs` as the regression oracle).
const PROXY_ENDPOINTS: [ParseEndpoint; 2] = [ParseEndpoint::Header, ParseEndpoint::References];

// ---------------------------------------------------------------------------
// Local deterministic GROBID stub server (tokio TCP, no external deps) —
// the same pattern as backend_contract.rs
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
    let (status, reason, headers, body, delay) = match plan {
        ResponsePlan::Xml(body) => (200u16, "OK", Vec::new(), body, None),
        ResponsePlan::Status(code, reason) => (code, reason, Vec::new(), Vec::new(), None),
        ResponsePlan::StatusWithHeaders(code, reason, headers) => {
            (code, reason, headers, Vec::new(), None)
        }
        ResponsePlan::Delayed(body, delay) => (200u16, "OK", Vec::new(), body, Some(delay)),
    };
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
    let mut head =
        format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/xml; charset=utf-8\r\n");
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
// Fixtures and helpers
// ---------------------------------------------------------------------------

fn http_backend(addr: SocketAddr, profile: GrobidProfile) -> GrobidHttpBackend {
    GrobidHttpBackend::new(Some(&format!("http://{addr}"))).with_profile(profile)
}

/// Build the proxy request for `endpoint` from a legacy profile preset
/// (the profile mapped onto exactly that endpoint's axis).
fn proxy_request(endpoint: ParseEndpoint, profile: &GrobidProfile) -> ParseBackendRequest {
    match endpoint {
        ParseEndpoint::Header => ParseBackendRequest::header_from_legacy_profile(
            profile,
            "proxy-paper",
            PDF_FIXTURE.to_vec(),
        ),
        ParseEndpoint::References => ParseBackendRequest::references_from_legacy_profile(
            profile,
            "proxy-paper",
            PDF_FIXTURE.to_vec(),
        ),
        ParseEndpoint::Fulltext => unreachable!("proxy suite covers header and references"),
    }
}

/// Sorted multipart field names of a recorded request — an exact allowlist.
fn sorted_field_names(request: &RecordedRequest) -> Vec<String> {
    let mut names: Vec<String> = multipart_fields(&request.content_type, &request.body)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names
}

fn field_value<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Assert the shared one-request observable of a failing proxy exchange.
fn assert_exactly_one_request(log: &RequestLog, before: usize) {
    let requests = log.lock().expect("request log lock");
    assert_eq!(
        requests.len(),
        before + 1,
        "a failing exchange must be observable exactly once — no hidden retry"
    );
}

/// Fetch and unwrap, with a contract failure message.
async fn fetch_ok(backend: &GrobidHttpBackend, request: ParseBackendRequest) -> TeiEnvelope {
    backend
        .fetch_tei(request)
        .await
        .expect("proxy fetch must succeed")
}

// ---------------------------------------------------------------------------
// Contract: exact routes and multipart allowlists
// ---------------------------------------------------------------------------

#[tokio::test]
async fn header_endpoint_posts_exact_java_path_and_allowlist() {
    let (addr, log) =
        spawn_grobid_stub(ResponsePlan::Xml(HEADER_TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());
    let request = ParseBackendRequest::header_from_legacy_profile(
        &GrobidProfile::rich(),
        "proxy-paper",
        PDF_FIXTURE.to_vec(),
    );

    let envelope = fetch_ok(&backend, request).await;
    // Partial TEI comes back verbatim as a directly comparable envelope.
    assert_eq!(envelope.raw_tei(), HEADER_TEI_FIXTURE.as_bytes());
    assert_eq!(envelope.identity().backend_id(), GROBID_HTTP_BACKEND_ID);
    assert_eq!(
        envelope.identity().profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint()
    );
    let result = envelope
        .identity()
        .result()
        .expect("header envelope carries the endpoint result identity");
    assert_eq!(result.endpoint(), ParseEndpoint::Header);
    assert_eq!(result.schema_version(), HEADER_REQUEST_SCHEMA_VERSION);
    assert_eq!(
        result.canonical(),
        "endpoint=header;schema=v1;header=v1;consolidateHeader=1;rawAffiliations=0"
    );

    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 1, "exactly one exchange, no hidden retry");
    let recorded = &requests[0];
    assert_eq!(recorded.method, "POST");
    assert_eq!(recorded.path, "/api/processHeaderDocument");
    assert!(
        recorded.content_type.starts_with("multipart/form-data"),
        "unexpected content-type: {}",
        recorded.content_type
    );

    // Exact allowlist: `input` + `consolidateHeader` — nothing else. Never
    // teiCoordinates, never consolidateCitations, never includeRawCitations.
    assert_eq!(
        sorted_field_names(recorded),
        vec!["consolidateHeader".to_string(), "input".to_string()],
        "header allowlist must be exactly input + consolidateHeader"
    );
    let fields = multipart_fields(&recorded.content_type, &recorded.body);
    assert_eq!(field_value(&fields, "consolidateHeader"), Some("1"));
    assert_eq!(
        field_value(&fields, "input"),
        Some("%PDF-1.4 endpoint proxy fixture bytes")
    );
    assert!(
        String::from_utf8_lossy(&recorded.body).contains("filename=\"paper.pdf\""),
        "PDF part must keep the stable upload filename"
    );
}

#[tokio::test]
async fn references_endpoint_posts_exact_java_path_and_allowlist() {
    let (addr, log) =
        spawn_grobid_stub(ResponsePlan::Xml(LIST_BIBL_TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());
    let request = ParseBackendRequest::references_from_legacy_profile(
        &GrobidProfile::rich(),
        "proxy-paper",
        PDF_FIXTURE.to_vec(),
    );

    let envelope = fetch_ok(&backend, request).await;
    assert_eq!(envelope.raw_tei(), LIST_BIBL_TEI_FIXTURE.as_bytes());
    let result = envelope
        .identity()
        .result()
        .expect("references envelope carries the endpoint result identity");
    assert_eq!(result.endpoint(), ParseEndpoint::References);
    assert_eq!(result.schema_version(), REFERENCES_REQUEST_SCHEMA_VERSION);
    assert_eq!(
        result.canonical(),
        "endpoint=references;schema=v1;references=v1;consolidateCitations=1;rawCitations=0"
    );

    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 1, "exactly one exchange, no hidden retry");
    let recorded = &requests[0];
    assert_eq!(recorded.method, "POST");
    assert_eq!(recorded.path, "/api/processReferences");

    // Exact allowlist: `input` + `consolidateCitations` — nothing else.
    assert_eq!(
        sorted_field_names(recorded),
        vec!["consolidateCitations".to_string(), "input".to_string()],
        "references allowlist must be exactly input + consolidateCitations"
    );
    let fields = multipart_fields(&recorded.content_type, &recorded.body);
    assert_eq!(field_value(&fields, "consolidateCitations"), Some("1"));
    assert_eq!(
        field_value(&fields, "input"),
        Some("%PDF-1.4 endpoint proxy fixture bytes")
    );
}

#[tokio::test]
async fn legacy_profiles_project_onto_their_single_axis_only() {
    // Structural profile: consolidation off on BOTH axes; each proxy
    // endpoint must send only its own knob, with level 0, and never carry
    // the other axis or teiCoordinates.
    for endpoint in PROXY_ENDPOINTS {
        let (addr, log) =
            spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
        let backend = http_backend(addr, GrobidProfile::structural());
        let envelope = fetch_ok(
            &backend,
            proxy_request(endpoint, &GrobidProfile::structural()),
        )
        .await;

        let requests = log.lock().expect("request log lock");
        assert_eq!(requests.len(), 1);
        let recorded = &requests[0];
        let (own_knob, expected_names) = match endpoint {
            ParseEndpoint::Header => (
                "consolidateHeader",
                vec!["consolidateHeader".to_string(), "input".to_string()],
            ),
            ParseEndpoint::References => (
                "consolidateCitations",
                vec!["consolidateCitations".to_string(), "input".to_string()],
            ),
            ParseEndpoint::Fulltext => unreachable!("proxy suite covers header and references"),
        };
        assert_eq!(
            sorted_field_names(recorded),
            expected_names,
            "{endpoint}: allowlist stays exactly the endpoint's own fields"
        );
        let fields = multipart_fields(&recorded.content_type, &recorded.body);
        assert_eq!(
            field_value(&fields, own_knob),
            Some("0"),
            "{endpoint}: structural profile disables its consolidation axis"
        );
        assert_eq!(
            envelope.identity().profile_fingerprint(),
            GrobidProfile::structural().configuration_fingerprint()
        );
    }
}

#[tokio::test]
async fn raw_switches_emit_only_their_own_field_and_rotate_identity() {
    // Header: includeRawAffiliations on → exactly its own extra field and a
    // rotated result identity; no includeRawCitations ever appears.
    let (addr, log) =
        spawn_grobid_stub(ResponsePlan::Xml(HEADER_TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());
    let raw_request = ParseBackendRequest::header(
        "raw-header",
        PDF_FIXTURE.to_vec(),
        HeaderDialectV1::legacy_rich().with_include_raw_affiliations(true),
    );
    let raw_envelope = fetch_ok(&backend, raw_request.clone()).await;
    {
        let requests = log.lock().expect("request log lock");
        assert_eq!(requests.len(), 1);
        let recorded = &requests[0];
        assert_eq!(
            sorted_field_names(recorded),
            vec![
                "consolidateHeader".to_string(),
                "includeRawAffiliations".to_string(),
                "input".to_string(),
            ],
            "header raw switch adds exactly includeRawAffiliations"
        );
        let fields = multipart_fields(&recorded.content_type, &recorded.body);
        assert_eq!(field_value(&fields, "includeRawAffiliations"), Some("1"));
        assert!(field_value(&fields, "includeRawCitations").is_none());
    }
    let raw_result = raw_envelope.identity().result().expect("result identity");
    assert!(
        raw_result.canonical().ends_with("rawAffiliations=1"),
        "raw affiliations rotate their own identity: {}",
        raw_result.canonical()
    );
    let plain_request = ParseBackendRequest::header(
        "raw-header",
        PDF_FIXTURE.to_vec(),
        HeaderDialectV1::legacy_rich(),
    );
    assert_ne!(
        raw_request.result_identity(),
        plain_request.result_identity(),
        "raw switch must rotate the result identity"
    );

    // References: includeRawCitations on → exactly its own extra field; no
    // includeRawAffiliations ever appears.
    let (addr, log) =
        spawn_grobid_stub(ResponsePlan::Xml(LIST_BIBL_TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());
    let raw_request = ParseBackendRequest::references(
        "raw-refs",
        PDF_FIXTURE.to_vec(),
        ReferencesDialectV1::legacy_rich().with_include_raw_citations(true),
    );
    let raw_envelope = fetch_ok(&backend, raw_request.clone()).await;
    {
        let requests = log.lock().expect("request log lock");
        assert_eq!(requests.len(), 1);
        let recorded = &requests[0];
        assert_eq!(
            sorted_field_names(recorded),
            vec![
                "consolidateCitations".to_string(),
                "includeRawCitations".to_string(),
                "input".to_string(),
            ],
            "references raw switch adds exactly includeRawCitations"
        );
        let fields = multipart_fields(&recorded.content_type, &recorded.body);
        assert_eq!(field_value(&fields, "includeRawCitations"), Some("1"));
        assert!(field_value(&fields, "includeRawAffiliations").is_none());
    }
    let raw_result = raw_envelope.identity().result().expect("result identity");
    assert!(raw_result.canonical().ends_with("rawCitations=1"));
    let plain_request = ParseBackendRequest::references(
        "raw-refs",
        PDF_FIXTURE.to_vec(),
        ReferencesDialectV1::legacy_rich(),
    );
    assert_ne!(
        raw_request.result_identity(),
        plain_request.result_identity()
    );
}

#[tokio::test]
async fn proxy_result_identities_are_pairwise_distinct_and_directly_comparable() {
    let (addr, log) =
        spawn_grobid_stub(ResponsePlan::Xml(HEADER_TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::rich());

    let fulltext_request = ParseBackendRequest::new("pairwise", PDF_FIXTURE.to_vec());
    let fulltext_envelope = fetch_ok(&backend, fulltext_request).await;
    let header_envelope = fetch_ok(
        &backend,
        proxy_request(ParseEndpoint::Header, &GrobidProfile::rich()),
    )
    .await;
    let references_envelope = fetch_ok(
        &backend,
        proxy_request(ParseEndpoint::References, &GrobidProfile::rich()),
    )
    .await;
    assert_eq!(log.lock().expect("request log lock").len(), 3);

    // Fulltext keeps the pre-S05 byte-identical stamp: no result component.
    assert!(fulltext_envelope.identity().result().is_none());

    let header_result = header_envelope.identity().result().expect("header result");
    let references_result = references_envelope
        .identity()
        .result()
        .expect("references result");

    // Pairwise distinct envelope identities for one profile.
    assert_ne!(fulltext_envelope.identity(), header_envelope.identity());
    assert_ne!(fulltext_envelope.identity(), references_envelope.identity());
    assert_ne!(header_envelope.identity(), references_envelope.identity());
    assert_ne!(header_result, references_result);

    // Collision-free endpoint namespaces (never equal each other or D204).
    assert!(
        header_result
            .canonical()
            .starts_with("endpoint=header;schema=v1;header=v1;"),
        "{}",
        header_result.canonical()
    );
    assert!(
        references_result
            .canonical()
            .starts_with("endpoint=references;schema=v1;references=v1;"),
        "{}",
        references_result.canonical()
    );

    // The stamp is exactly the T01 request result identity, additive over
    // the untouched profile fingerprint.
    let header_request = proxy_request(ParseEndpoint::Header, &GrobidProfile::rich());
    let references_request = proxy_request(ParseEndpoint::References, &GrobidProfile::rich());
    assert_eq!(
        *header_envelope.identity(),
        backend
            .identity()
            .with_result(header_request.result_identity())
    );
    assert_eq!(
        *references_envelope.identity(),
        backend
            .identity()
            .with_result(references_request.result_identity())
    );
    assert_eq!(
        header_envelope.identity().profile_fingerprint(),
        GrobidProfile::rich().configuration_fingerprint()
    );
    assert_eq!(
        header_envelope.identity().backend_id(),
        GROBID_HTTP_BACKEND_ID
    );
}

#[tokio::test]
async fn backend_default_proxy_requests_project_the_configured_profile() {
    let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
    let backend = http_backend(addr, GrobidProfile::structural());

    // Trait-level defaults map THIS backend's profile onto each axis.
    let header_default = backend.default_header_request("dflt", PDF_FIXTURE.to_vec());
    assert_eq!(
        header_default.header_dialect(),
        Some(HeaderDialectV1::legacy_structural())
    );
    let references_default = backend.default_references_request("dflt", PDF_FIXTURE.to_vec());
    assert_eq!(
        references_default.references_dialect(),
        Some(ReferencesDialectV1::legacy_structural())
    );

    // …and the wire projection follows: structural → level 0 on its own
    // axis, allowlist unchanged.
    let _ = fetch_ok(&backend, header_default).await;
    let _ = fetch_ok(&backend, references_default).await;
    let requests = log.lock().expect("request log lock");
    assert_eq!(requests.len(), 2);
    let header_fields = multipart_fields(&requests[0].content_type, &requests[0].body);
    assert_eq!(field_value(&header_fields, "consolidateHeader"), Some("0"));
    let references_fields = multipart_fields(&requests[1].content_type, &requests[1].body);
    assert_eq!(
        field_value(&references_fields, "consolidateCitations"),
        Some("0")
    );
}

// ---------------------------------------------------------------------------
// Contract: the shared closed typed-outcome ladder
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proxy_endpoints_share_typed_204_400_and_413_outcomes() {
    for endpoint in PROXY_ENDPOINTS {
        for (status, reason, code, limit, phase) in [
            (
                204,
                "No Content",
                FailureCode::NoContent,
                None,
                FailurePhase::AwaitResponse,
            ),
            (
                400,
                "Bad Request",
                FailureCode::InvalidRequest,
                None,
                FailurePhase::AwaitResponse,
            ),
            (
                413,
                "Payload Too Large",
                FailureCode::LimitExceeded,
                Some(LimitKind::RequestBytes),
                FailurePhase::Send,
            ),
        ] {
            let (addr, log) = spawn_grobid_stub(ResponsePlan::Status(status, reason)).await;
            let backend = http_backend(addr, GrobidProfile::rich());
            let err = backend
                .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
                .await
                .expect_err("typed status failure");
            assert_eq!(err.code(), code, "{endpoint}: status {status}");
            assert_eq!(err.status(), Some(status), "{endpoint}: status {status}");
            assert_eq!(err.enforced_limit(), limit, "{endpoint}: status {status}");
            assert_eq!(err.phase(), phase, "{endpoint}: status {status}");
            assert_exactly_one_request(&log, 0);
        }
    }
}

#[tokio::test]
async fn proxy_endpoints_share_typed_503_retry_after_without_retrying() {
    for endpoint in PROXY_ENDPOINTS {
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
            let backend = http_backend(addr, GrobidProfile::rich());
            let err = backend
                .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
                .await
                .expect_err("503 is typed busy");
            assert_eq!(err.code(), FailureCode::Busy, "{endpoint}");
            assert_eq!(err.status(), Some(503), "{endpoint}");
            assert_eq!(err.retry_after(), expected, "{endpoint}");
            assert_exactly_one_request(&log, 0);
        }
    }
}

#[tokio::test]
async fn proxy_endpoints_map_5xx_and_unexpected_status_typed() {
    for endpoint in PROXY_ENDPOINTS {
        // Java processing/conversion failure.
        let (addr, log) =
            spawn_grobid_stub(ResponsePlan::Status(500, "Internal Server Error")).await;
        let backend = http_backend(addr, GrobidProfile::rich());
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("5xx is a typed conversion failure");
        assert_eq!(err.code(), FailureCode::ConversionFailed, "{endpoint}");
        assert_eq!(err.status(), Some(500), "{endpoint}");
        assert_eq!(err.phase(), FailurePhase::AwaitResponse, "{endpoint}");
        assert_exactly_one_request(&log, 0);

        // Any non-5xx unexpected status is an invalid response, not a panic
        // and not a silent skip.
        let (addr, log) = spawn_grobid_stub(ResponsePlan::Status(418, "I'm a teapot")).await;
        let backend = http_backend(addr, GrobidProfile::rich());
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("unexpected status is typed invalid response");
        assert_eq!(err.code(), FailureCode::InvalidResponse, "{endpoint}");
        assert_eq!(err.status(), Some(418), "{endpoint}");
        assert_exactly_one_request(&log, 0);
    }
}

#[tokio::test]
async fn proxy_endpoints_carry_partial_tei_verbatim_including_empty_and_non_utf8() {
    for endpoint in PROXY_ENDPOINTS {
        // Non-UTF-8 partial TEI survives verbatim.
        let non_utf8: &[u8] = b"<teiHeader>\xFF\xFE raw</teiHeader>";
        let (addr, log) = spawn_grobid_stub(ResponsePlan::Xml(non_utf8.to_vec())).await;
        let backend = http_backend(addr, GrobidProfile::rich());
        let envelope = fetch_ok(&backend, proxy_request(endpoint, &GrobidProfile::rich())).await;
        assert_eq!(envelope.raw_tei(), non_utf8, "{endpoint}: byte fidelity");
        assert!(envelope.identity().result().is_some(), "{endpoint}");
        assert_eq!(log.lock().expect("request log lock").len(), 1);

        // An empty 200 body is carried verbatim too — observable absence,
        // never normalized into a default payload.
        let (addr, _log) = spawn_grobid_stub(ResponsePlan::Xml(Vec::new())).await;
        let backend = http_backend(addr, GrobidProfile::rich());
        let envelope = fetch_ok(&backend, proxy_request(endpoint, &GrobidProfile::rich())).await;
        assert!(envelope.raw_tei().is_empty(), "{endpoint}");
        assert!(envelope.identity().result().is_some(), "{endpoint}");
    }
}

#[tokio::test]
async fn proxy_endpoints_time_out_without_retrying() {
    for endpoint in PROXY_ENDPOINTS {
        let plan =
            ResponsePlan::Delayed(TEI_FIXTURE.as_bytes().to_vec(), Duration::from_millis(250));
        let (addr, log) = spawn_grobid_stub(plan).await;
        let backend =
            http_backend(addr, GrobidProfile::rich()).with_timeout(Duration::from_millis(30));
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("delayed response must time out");
        assert_eq!(err.code(), FailureCode::Timeout, "{endpoint}");
        assert_eq!(err.phase(), FailurePhase::AwaitResponse, "{endpoint}");
        // The stub logs the request as soon as it reads it, but under load the
        // stub task may not be scheduled before the client's 30ms timeout
        // fires, turning the exactly-once check into a scheduling race rather
        // than a contract check. Wait (bounded) for the single observation;
        // the stub's 250ms delay keeps it from ever answering.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let observed = !log.lock().expect("request log lock").is_empty();
            if observed || tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_exactly_one_request(&log, 0);
    }
}

#[tokio::test]
async fn proxy_endpoints_surface_connect_refusal_as_typed_transport() {
    for endpoint in PROXY_ENDPOINTS {
        // Bind then immediately drop: the port is guaranteed closed.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);

        let backend = GrobidHttpBackend::new(Some(&format!("http://{addr}")));
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("connection refusal must surface as Err");
        assert_eq!(err.code(), FailureCode::Transport, "{endpoint}");
        assert_eq!(err.phase(), FailurePhase::Connect, "{endpoint}");
    }
}

// ---------------------------------------------------------------------------
// Contract: exactly-once under client-enforced bounds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proxy_request_limit_breach_fails_before_any_wire_request() {
    for endpoint in PROXY_ENDPOINTS {
        let (addr, log) =
            spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
        let backend = http_backend(addr, GrobidProfile::rich())
            .with_request_byte_limit(PDF_FIXTURE.len() - 1);
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("limit+1 request must fail fast");
        assert_eq!(err.code(), FailureCode::LimitExceeded, "{endpoint}");
        assert_eq!(
            err.enforced_limit(),
            Some(LimitKind::RequestBytes),
            "{endpoint}"
        );
        assert_eq!(
            err.status(),
            None,
            "{endpoint}: client-enforced, no HTTP status"
        );
        assert_eq!(
            log.lock().expect("request log lock").len(),
            0,
            "{endpoint}: pre-wire rejection — exactly zero HTTP requests"
        );
    }
}

#[tokio::test]
async fn proxy_response_limit_breach_is_exactly_one_request() {
    for endpoint in PROXY_ENDPOINTS {
        let (addr, log) =
            spawn_grobid_stub(ResponsePlan::Xml(TEI_FIXTURE.as_bytes().to_vec())).await;
        let backend = http_backend(addr, GrobidProfile::rich())
            .with_response_byte_limit(TEI_FIXTURE.len() - 1);
        let err = backend
            .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
            .await
            .expect_err("limit+1 response must fail mid-stream");
        assert_eq!(err.code(), FailureCode::LimitExceeded, "{endpoint}");
        assert_eq!(
            err.enforced_limit(),
            Some(LimitKind::ResponseBytes),
            "{endpoint}"
        );
        assert_eq!(err.phase(), FailurePhase::ReadBody, "{endpoint}");
        assert_eq!(err.status(), None, "{endpoint}");
        assert_exactly_one_request(&log, 0);
    }
}

// ---------------------------------------------------------------------------
// Contract: redaction — failures never carry URLs or payloads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proxy_failures_never_leak_base_url_or_payload() {
    for endpoint in PROXY_ENDPOINTS {
        let err = match endpoint {
            ParseEndpoint::Header => {
                let (addr, _log) =
                    spawn_grobid_stub(ResponsePlan::Status(500, "Internal Server Error")).await;
                http_backend(addr, GrobidProfile::rich())
                    .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
                    .await
                    .expect_err("500 failure")
            }
            ParseEndpoint::References => {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
                let addr = listener.local_addr().expect("addr");
                drop(listener);
                GrobidHttpBackend::new(Some(&format!("http://{addr}")))
                    .fetch_tei(proxy_request(endpoint, &GrobidProfile::rich()))
                    .await
                    .expect_err("transport failure")
            }
            ParseEndpoint::Fulltext => unreachable!("proxy suite covers header and references"),
        };
        let display = err.to_string();
        let debug = format!("{err:?}");
        for leak in ["http", "127.0.0.1", "%PDF", "<TEI", "teiHeader"] {
            assert!(
                !display.contains(leak) && !debug.contains(leak),
                "{endpoint}: failure output must not carry base URLs or payloads \
                 (leak={leak}, display={display:?})"
            );
        }
    }
}
