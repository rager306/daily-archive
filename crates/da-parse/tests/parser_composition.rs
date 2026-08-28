//! Integration composition suite for the ScholarlyParser runtime pipeline
//! (R105, M464-z84in2 S02/T03).
//!
//! Asserted here (over the public seams only):
//!
//! - the composed parser invokes its [`ParseBackend`] **exactly once** per
//!   parse (stateful fake, no hidden retries),
//! - a backend failure short-circuits the pipeline: the backend's own typed
//!   failure ([`da_parse::ParseBackendFailure`], S04/T01) surfaces verbatim
//!   with its structured metadata and neither lift nor projection runs,
//! - the cached/offline composition needs no HTTP at all and replays
//!   byte-equal articles,
//! - the `GrobidParser` compatibility facade really drives
//!   backend→envelope→lift→project through the composition: facade (as
//!   `Arc<dyn ParserPort>`, reading a real PDF file) and the composed parser
//!   produce **equal `ParsedArticle`s** (identical serde fields) with
//!   `tei_xml = Some(raw TEI)`, `layout_json = None`,
//!   `page_text_missingness = None`,
//! - external `ParserError` mapping stays fail-closed through the delegation
//!   (missing source file → `ParseFailed`, transport outage → `Unavailable`,
//!   HTTP 500 → `ParseFailed` carrying the stable typed metadata
//!   `code=conversion_failed … status=500`),
//! - **every** [`da_parse::FailureCode`] of the closed S04 vocabulary is proven to
//!   short-circuit exactly once, preserve all structured metadata on the
//!   typed surface, keep the historical two-variant compatibility mapping
//!   (availability class → `Unavailable`, the rest → `ParseFailed`) and leak
//!   no TEI/PDF payload through the error string (S04/T03),
//! - `ParserPort::parse_html` default stays `Unavailable`.
//!
//! The GROBID transport is a local deterministic stub server — no real GROBID
//! service, no network beyond loopback.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use da_parse::{
    CachedTeiBackend, FailurePhase, GrobidHttpBackend, GrobidParser, GrobidProfile, LimitKind,
    ParseBackend, ParseBackendFailure, ParseBackendRequest, RetryAfter, ScholarlyParser,
    TeiEnvelope, TeiIdentity,
};
use da_ports::parser::{ParserError, ParserPort};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// TEI fixture exercising title, sections and bibliography through the full
/// composition (real lift coverage is pinned by the preserved-fixture suite;
/// this suite pins the wiring, so a small non-trivial document suffices).
const TEI_FIXTURE: &str = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc><titleStmt><title>Composition Parity Paper</title></titleStmt></fileDesc></teiHeader><text><body>
<div><head n="1">INTRODUCTION</head><p>Composed pipeline text.</p></div>
<div><head n="2">METHODS</head><p>Method details.</p></div>
</body><back><div type="references"><listBibl>
<biblStruct><analytic><title level="a" type="main">First Citation</title></analytic><idno type="DOI">10.1000/cite.1</idno></biblStruct>
<biblStruct><analytic><title level="a" type="main">Second Citation</title></analytic><idno type="arXiv">arXiv:2001.00002v1</idno></biblStruct>
</listBibl></div></back></text></TEI>"#;

const PDF_FIXTURE: &[u8] = b"%PDF-1.4 composition fixture bytes";

// ---------------------------------------------------------------------------
// Stateful fake backend
// ---------------------------------------------------------------------------

/// Response the fake backend hands out per invocation.
enum FakeResponse {
    Tei(Vec<u8>),
    /// Typed backend failure surfaced verbatim (S04/T01).
    Failure(ParseBackendFailure),
}

/// Stateful fake backend: records every invocation and the exact logical
/// request it received.
type RecordedRequest = (String, Vec<u8>);

#[derive(Clone)]
struct StatefulFakeBackend {
    calls: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    response: Arc<FakeResponse>,
}

impl StatefulFakeBackend {
    fn serving(tei: Vec<u8>) -> Self {
        Self::new(FakeResponse::Tei(tei))
    }

    fn failing() -> Self {
        Self::new(FakeResponse::Failure(ParseBackendFailure::transport(
            FailurePhase::Connect,
        )))
    }

    fn new(response: FakeResponse) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(response),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_request(&self) -> (String, Vec<u8>) {
        self.requests
            .lock()
            .expect("request log")
            .last()
            .cloned()
            .expect("at least one recorded request")
    }
}

#[async_trait::async_trait]
impl ParseBackend for StatefulFakeBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests
            .lock()
            .expect("request log")
            .push((request.paper_id().to_string(), request.pdf_bytes().to_vec()));
        match self.response.as_ref() {
            FakeResponse::Tei(bytes) => Ok(TeiEnvelope::new(
                bytes.clone(),
                TeiIdentity::new("fake-composition", "profile=fake"),
            )),
            FakeResponse::Failure(failure) => Err(failure.clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// Local deterministic GROBID stub server (tokio TCP, no external deps)
// ---------------------------------------------------------------------------

/// Per-connection response plan of the stub server.
#[derive(Clone, Copy)]
enum StubPlan {
    /// `200 OK` with an `application/xml` body.
    Xml(&'static [u8]),
    /// Non-success status with a reason phrase.
    Status(u16, &'static str),
}

async fn spawn_grobid_stub(plan: StubPlan) -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub GROBID server");
    let addr = listener.local_addr().expect("stub server address");
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_task = Arc::clone(&hits);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let hits = Arc::clone(&hits_for_task);
            let plan = plan;
            tokio::spawn(serve_connection(stream, plan, hits));
        }
    });
    (addr, hits)
}

async fn serve_connection(
    mut stream: tokio::net::TcpStream,
    plan: StubPlan,
    hits: Arc<AtomicUsize>,
) {
    if drain_request(&mut stream).await.is_none() {
        return;
    }
    hits.fetch_add(1, Ordering::SeqCst);
    let (status_line, body) = match plan {
        StubPlan::Xml(body) => ("200 OK".to_string(), body),
        StubPlan::Status(code, reason) => (format!("{code} {reason}"), &[][..]),
    };
    let mut response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: application/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    let _ = stream.write_all(&response).await;
    let _ = stream.shutdown().await;
}

/// Read the request head and its Content-Length body so the socket can be
/// answered without a mid-upload reset. Returns `None` on malformed input.
async fn drain_request(stream: &mut tokio::net::TcpStream) -> Option<()> {
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
    let mut content_length = 0usize;
    for line in head.split("\r\n").skip(1) {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    while buf.len() - head_end < content_length {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Some(())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Write the PDF fixture to a unique temp file for facade `parse_pdf` calls
/// (the facade owns the filesystem concern and must read a real path).
fn write_temp_pdf(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "da-parse-composition-{label}-{}-{nanos}.pdf",
        std::process::id()
    ));
    std::fs::write(&path, PDF_FIXTURE).expect("write temp PDF fixture");
    path.to_string_lossy().into_owned()
}

fn expected_pdf_hash() -> String {
    hex::encode(Sha256::digest(PDF_FIXTURE))
}

// ---------------------------------------------------------------------------
// Composition over the stateful fake backend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn composed_parser_calls_backend_exactly_once_and_projects_full_article() {
    let backend = StatefulFakeBackend::serving(TEI_FIXTURE.as_bytes().to_vec());
    let parser = ScholarlyParser::new(backend.clone());

    let article = parser
        .parse("comp-paper", PDF_FIXTURE.to_vec())
        .await
        .expect("composition over a healthy backend must succeed");

    assert_eq!(
        backend.call_count(),
        1,
        "exactly one backend invocation per parse (no hidden retries)"
    );
    let (paper_id, source_bytes) = backend.last_request();
    assert_eq!(paper_id, "comp-paper", "logical paper id flows verbatim");
    assert_eq!(source_bytes, PDF_FIXTURE, "source bytes flow verbatim");

    assert_eq!(article.paper_id, "comp-paper");
    assert_eq!(article.title, "Composition Parity Paper");
    assert_eq!(article.pdf_hash, expected_pdf_hash());
    assert_eq!(
        article.tei_xml.as_deref(),
        Some(TEI_FIXTURE),
        "tei_xml must be Some(raw TEI), byte-preserved through the composition"
    );
    assert_eq!(article.layout_json, None);
    assert_eq!(article.page_text_missingness, None);
    assert_eq!(
        article.sections.len(),
        2,
        "lift must run inside the composition"
    );
    assert_eq!(article.sections[0].title, "INTRODUCTION");
    assert_eq!(
        article.citations.len(),
        2,
        "projection must run inside the composition"
    );
    assert_eq!(article.citations[0].doi.as_deref(), Some("10.1000/cite.1"));
    assert_eq!(article.citations[1].arxiv_id.as_deref(), Some("2001.00002"));
}

#[tokio::test]
async fn backend_error_short_circuits_lift_and_projection() {
    let backend = StatefulFakeBackend::failing();
    let parser = ScholarlyParser::new(backend.clone());

    // Typed consumers get the backend's own failure verbatim — structured
    // metadata intact, unmapped (fetch_envelope_once is the typed surface).
    let typed = parser
        .fetch_envelope_once("comp-paper", PDF_FIXTURE.to_vec())
        .await
        .expect_err("backend failure must surface as Err, never a silent skip");
    assert_eq!(
        typed,
        ParseBackendFailure::transport(FailurePhase::Connect),
        "the backend's typed failure must surface verbatim and unmapped"
    );
    assert_eq!(
        backend.call_count(),
        1,
        "no retry after failure: exactly one backend invocation"
    );

    // The composition maps to the historical ParserError exactly once
    // (fail-closed Unavailable for availability-class failures) and never
    // lifts or projects behind the error (no fallback envelope exists).
    let err = parser
        .parse("comp-paper", PDF_FIXTURE.to_vec())
        .await
        .expect_err("backend failure must surface as Err through parse, too");
    assert!(
        matches!(err, ParserError::Unavailable(ref message) if message.contains("code=transport")),
        "availability-class failures must map to Unavailable with stable metadata: {err:?}"
    );
    assert_eq!(
        backend.call_count(),
        2,
        "each parse above drove exactly one backend invocation"
    );
}

/// Representative fake failure per closed [`da_parse::FailureCode`] plus the expected
/// historical [`ParserError`] variant at the compatibility boundary (S04/T03).
/// Availability class (busy/timeout/transport) stays `Unavailable`; everything
/// else fails closed as `ParseFailed`.
fn every_typed_failure() -> Vec<(&'static str, ParseBackendFailure, ExpectedCompatVariant)> {
    vec![
        (
            "busy",
            ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(7))),
            ExpectedCompatVariant::Unavailable,
        ),
        (
            "timeout",
            ParseBackendFailure::timeout(FailurePhase::AwaitResponse),
            ExpectedCompatVariant::Unavailable,
        ),
        (
            "transport",
            ParseBackendFailure::transport(FailurePhase::Connect),
            ExpectedCompatVariant::Unavailable,
        ),
        (
            "no_content",
            ParseBackendFailure::no_content(),
            ExpectedCompatVariant::ParseFailed,
        ),
        (
            "limit_exceeded",
            ParseBackendFailure::limit(LimitKind::ResponseBytes, None),
            ExpectedCompatVariant::ParseFailed,
        ),
        (
            "invalid_request",
            ParseBackendFailure::invalid_request(400),
            ExpectedCompatVariant::ParseFailed,
        ),
        (
            "conversion_failed",
            ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(500)),
            ExpectedCompatVariant::ParseFailed,
        ),
        (
            "invalid_response",
            ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(418)),
            ExpectedCompatVariant::ParseFailed,
        ),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpectedCompatVariant {
    Unavailable,
    ParseFailed,
}

#[tokio::test]
async fn every_typed_failure_code_short_circuits_and_maps_fail_closed() {
    // Exhaustive closed-vocabulary coverage: every FailureCode the backend can
    // emit (S04/T01) short-circuits the composition exactly once and keeps its
    // historical two-variant compatibility mapping without payload leakage.
    assert_eq!(
        every_typed_failure().len(),
        8,
        "the closed FailureCode vocabulary has exactly 8 variants — extend this table when it grows"
    );

    for (name, failure, expected_variant) in every_typed_failure() {
        let backend = StatefulFakeBackend::new(FakeResponse::Failure(failure.clone()));
        let parser = ScholarlyParser::new(backend.clone());

        // Typed surface: the structured failure surfaces verbatim — every
        // safe metadata field preserved (S04/T01 public typed backend).
        let typed = parser
            .fetch_envelope_once("comp-paper", PDF_FIXTURE.to_vec())
            .await
            .expect_err("typed consumers must observe the backend's own failure verbatim");
        assert_eq!(
            typed, failure,
            "code {name}: typed failure must surface verbatim, unmapped"
        );
        assert_eq!(typed.code(), failure.code(), "code {name}");
        assert_eq!(typed.phase(), failure.phase(), "code {name}");
        assert_eq!(typed.status(), failure.status(), "code {name}");
        assert_eq!(typed.retry_after(), failure.retry_after(), "code {name}");
        assert_eq!(
            typed.enforced_limit(),
            failure.enforced_limit(),
            "code {name}"
        );
        assert_eq!(
            backend.call_count(),
            1,
            "code {name}: no hidden retry behind the typed failure"
        );

        // Compatibility surface: the historical two-variant ParserError
        // mapping, exactly once per parse, never a third variant, and never
        // a lift/projection fallback behind the failure.
        let err = parser
            .parse("comp-paper", PDF_FIXTURE.to_vec())
            .await
            .expect_err("compatibility facade must keep failing closed");
        assert_eq!(
            backend.call_count(),
            2,
            "code {name}: each parse above drove exactly one backend invocation"
        );
        // Exhaustive match pins the two historical variants as the ONLY shapes
        // the facade can produce.
        let message = match &err {
            ParserError::ParseFailed(message) | ParserError::Unavailable(message) => message,
        };
        match expected_variant {
            ExpectedCompatVariant::Unavailable => assert!(
                matches!(&err, ParserError::Unavailable(_)),
                "code {name}: availability-class failure must stay Unavailable, got {err:?}"
            ),
            ExpectedCompatVariant::ParseFailed => assert!(
                matches!(&err, ParserError::ParseFailed(_)),
                "code {name}: fail-closed failure must stay ParseFailed, got {err:?}"
            ),
        }
        assert!(
            message.contains(&format!("code={}", failure.code())),
            "code {name}: stable typed metadata must survive the mapping: {message}"
        );
        assert!(
            !message.contains("Composition Parity Paper"),
            "code {name}: TEI payload must never leak through the error string"
        );
        assert!(
            !message.contains("%PDF-1.4"),
            "code {name}: PDF payload must never leak through the error string"
        );
    }
}

#[tokio::test]
async fn cached_backend_composition_requires_no_http() {
    // No socket is bound anywhere in this test: offline cached-TEI replay
    // must complete without any transport at all.
    let parser = ScholarlyParser::new(CachedTeiBackend::new(
        TEI_FIXTURE,
        GrobidProfile::structural(),
    ));

    let first = parser
        .parse("offline-paper", PDF_FIXTURE.to_vec())
        .await
        .expect("offline composition must succeed without HTTP");
    let second = parser
        .parse("offline-paper", PDF_FIXTURE.to_vec())
        .await
        .expect("offline replay must succeed without HTTP");

    assert_eq!(
        serde_json::to_value(&first).expect("serialize first"),
        serde_json::to_value(&second).expect("serialize second"),
        "offline replay over the same composition must be serde-equal"
    );
    assert_eq!(first.tei_xml.as_deref(), Some(TEI_FIXTURE));
    assert_eq!(first.layout_json, None);
    assert_eq!(first.page_text_missingness, None);
    assert_eq!(first.pdf_hash, expected_pdf_hash());
}

// ---------------------------------------------------------------------------
// Facade compatibility: the facade drives the same composition
// ---------------------------------------------------------------------------

#[tokio::test]
async fn facade_and_composed_parser_produce_equal_articles() {
    let (facade_addr, facade_hits) = spawn_grobid_stub(StubPlan::Xml(TEI_FIXTURE.as_bytes())).await;
    let (composed_addr, composed_hits) =
        spawn_grobid_stub(StubPlan::Xml(TEI_FIXTURE.as_bytes())).await;

    // Facade path: held as Arc<dyn ParserPort> — exactly the shape existing
    // callers (CLI/adapters) compile against.
    let pdf_path = write_temp_pdf("parity");
    let facade: Arc<dyn ParserPort> = Arc::new(
        GrobidParser::new(Some(&format!("http://{facade_addr}")))
            .with_profile(GrobidProfile::rich()),
    );
    let facade_article = facade
        .parse_pdf(&pdf_path, "parity-paper")
        .await
        .expect("facade parse must succeed over the stub transport");
    let _ = std::fs::remove_file(&pdf_path);

    // Composed path: same backend dialect, same source bytes, no filesystem.
    let composed = ScholarlyParser::new(
        GrobidHttpBackend::new(Some(&format!("http://{composed_addr}")))
            .with_profile(GrobidProfile::rich()),
    );
    let composed_article = composed
        .parse("parity-paper", PDF_FIXTURE.to_vec())
        .await
        .expect("composed parse must succeed over the stub transport");

    assert_eq!(
        facade_hits.load(Ordering::SeqCst),
        1,
        "facade drives exactly one backend invocation per parse"
    );
    assert_eq!(
        composed_hits.load(Ordering::SeqCst),
        1,
        "composition drives exactly one backend invocation per parse"
    );

    // Equal ParsedArticle through the same serde fields.
    assert_eq!(
        serde_json::to_value(&facade_article).expect("facade serialize"),
        serde_json::to_value(&composed_article).expect("composed serialize"),
        "facade and composed parser must produce equal ParsedArticles"
    );

    assert_eq!(facade_article.tei_xml.as_deref(), Some(TEI_FIXTURE));
    assert_eq!(composed_article.tei_xml.as_deref(), Some(TEI_FIXTURE));
    assert_eq!(facade_article.layout_json, None);
    assert_eq!(facade_article.page_text_missingness, None);
    assert_eq!(facade_article.pdf_hash, expected_pdf_hash());
    assert_eq!(facade_article.sections.len(), 2);
    assert_eq!(facade_article.citations.len(), 2);
}

#[tokio::test]
async fn facade_preserves_external_parser_error_mapping() {
    // Missing source file: the facade's filesystem concern keeps its
    // historical ParseFailed mapping (the composition is never reached).
    let missing: Arc<dyn ParserPort> = Arc::new(GrobidParser::new(Some("http://127.0.0.1:1")));
    let err = missing
        .parse_pdf("/nonexistent/da-parse-composition/missing.pdf", "p")
        .await
        .expect_err("missing source must fail");
    assert!(
        matches!(err, ParserError::ParseFailed(ref message) if message.contains("read PDF")),
        "unexpected mapping for read failure: {err:?}"
    );

    // Existing source + dead transport: outage keeps the Unavailable mapping
    // through the full facade → composition → backend delegation.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let dead_addr = listener.local_addr().expect("addr");
    drop(listener);

    let pdf_path = write_temp_pdf("outage");
    let port: Arc<dyn ParserPort> =
        Arc::new(GrobidParser::new(Some(&format!("http://{dead_addr}"))));
    let err = port
        .parse_pdf(&pdf_path, "p")
        .await
        .expect_err("dead transport must fail");
    let _ = std::fs::remove_file(&pdf_path);
    assert!(
        matches!(err, ParserError::Unavailable(_)),
        "transport outage must keep the Unavailable mapping: {err:?}"
    );
}

#[tokio::test]
async fn facade_maps_http_error_status_through_composition() {
    let (addr, hits) = spawn_grobid_stub(StubPlan::Status(500, "Internal Server Error")).await;

    let pdf_path = write_temp_pdf("status500");
    let port: Arc<dyn ParserPort> = Arc::new(
        GrobidParser::new(Some(&format!("http://{addr}")))
            .with_profile(GrobidProfile::structural()),
    );
    let err = port
        .parse_pdf(&pdf_path, "p")
        .await
        .expect_err("HTTP 500 must fail visibly");
    let _ = std::fs::remove_file(&pdf_path);

    assert!(
        matches!(err, ParserError::ParseFailed(ref message)
            if message.contains("code=conversion_failed") && message.contains("status=500")),
        "non-success status must keep its historical ParseFailed mapping with stable typed metadata: {err:?}"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the failing invocation is observable, not retried"
    );
}

#[tokio::test]
async fn parse_html_default_stays_unavailable() {
    let facade = GrobidParser::new(Some("http://127.0.0.1:1"));
    let err = facade
        .parse_html("/unused.html", "p")
        .await
        .expect_err("parse_html must stay Unavailable on the GROBID facade");
    assert!(
        matches!(err, ParserError::Unavailable(_)),
        "parse_html default must stay Unavailable: {err:?}"
    );
}
