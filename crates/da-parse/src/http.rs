//! Java GROBID HTTP transport backend + `GrobidParser` compatibility facade.
//!
//! Since M464-z84in2 S02/T01 the transport is exposed through the
//! provider-neutral [`ParseBackend`] seam ([`GrobidHttpBackend`]); the
//! historical [`GrobidParser`] stays as a compatibility facade delegating to
//! it, so existing `Arc<dyn ParserPort>` callers, the `grobid_parse` lifecycle
//! span, external `ParserError` mapping and cache identities are unchanged
//! (R105). Since S04/T01 the backend seam carries the closed typed failure
//! vocabulary ([`ParseBackendFailure`]). Since S04/T02 the multipart form is
//! the **sole projector** of the typed V1 request dialect, successful bodies
//! are carried as verbatim bytes (any encoding), every wire outcome (200 /
//! 204 / 503 + `Retry-After` / 400 / 413 / other 5xx / timeout / opt-in byte
//! bounds) is a typed failure branch proven by the stub contract suite, and
//! opt-in client-enforced byte bounds exist as transport tuning that never
//! enters identity.
//!
//! Since S02/T03 the facade's `impl ParserPort` lives in [`crate::parser`] and
//! drives the full backend→envelope→lift→project composition through
//! [`crate::parser::ScholarlyParser`]; filesystem/env/HTTP stay in this shell
//! only.

use crate::backend::{ParseBackend, ParseBackendRequest};
use crate::envelope::{TeiEnvelope, TeiIdentity};
use crate::profile::GrobidProfile;
use crate::transport::{FailurePhase, LimitKind, ParseBackendFailure, RetryAfter};
use async_trait::async_trait;

pub(crate) const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";

const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Opt-in client-enforced byte bounds for one fulltext exchange.
///
/// Pure transport tuning: never part of [`GrobidHttpBackend::identity`] and
/// never a request field. The default is unbounded in both directions —
/// exactly the S02 compatibility behavior; a bound only acts once it is
/// explicitly configured, and then maps onto the typed
/// [`FailureCode::LimitExceeded`] with [`LimitKind::RequestBytes`] /
/// [`LimitKind::ResponseBytes`] and no provider status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ByteLimits {
    request: Option<usize>,
    response: Option<usize>,
}

/// Provider-neutral view of the Java GROBID HTTP transport (R105).
///
/// Owns exactly the transport concern: endpoint URL, HTTP client, the
/// effective-input [`GrobidProfile`] that pairs with its requests, and the
/// opt-in byte bounds. Lifting and projection do not live here.
#[derive(Clone, Debug)]
pub struct GrobidHttpBackend {
    url: String,
    client: reqwest::Client,
    profile: GrobidProfile,
    limits: ByteLimits,
}

impl GrobidHttpBackend {
    /// Build the backend against `url` (default `DEFAULT_GROBID_URL`) with
    /// the rich profile, the bounded interactive timeout and no byte bounds.
    pub fn new(url: Option<&str>) -> Self {
        Self {
            url: url.unwrap_or(DEFAULT_GROBID_URL).to_string(),
            client: build_client(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
            profile: GrobidProfile::rich(),
            limits: ByteLimits::default(),
        }
    }

    pub fn with_profile(mut self, profile: GrobidProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Override the bounded HTTP request timeout for batch profiles.
    ///
    /// Full citation/header consolidation can legitimately exceed the 120s
    /// interactive default while Crossref applies rate-limit backoff. The
    /// caller should record this operational policy in its run report. It does
    /// not alter successful TEI semantics and therefore is not a content-key
    /// input (and not part of [`Self::identity`]).
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.client = build_client(timeout);
        self
    }

    /// Opt-in bound on the outbound request payload (the PDF part of the
    /// multipart form).
    ///
    /// A breach fails fast with a typed, client-enforced
    /// [`FailureCode::LimitExceeded`] ([`LimitKind::RequestBytes`], no HTTP
    /// status) **before any wire bytes exist** — exactly zero HTTP requests.
    /// Transport tuning only: never part of [`Self::identity`], unset by
    /// default (the S02 compatibility behavior).
    pub fn with_request_byte_limit(mut self, max_bytes: usize) -> Self {
        self.limits.request = Some(max_bytes);
        self
    }

    /// Opt-in bound on the inbound TEI response body.
    ///
    /// The body is streamed; as soon as more than `max_bytes` arrive, the
    /// fetch fails with a typed, client-enforced
    /// [`FailureCode::LimitExceeded`] ([`LimitKind::ResponseBytes`], no HTTP
    /// status) instead of buffering an unbounded payload. Transport tuning
    /// only: never part of [`Self::identity`], unset by default.
    pub fn with_response_byte_limit(mut self, max_bytes: usize) -> Self {
        self.limits.response = Some(max_bytes);
        self
    }

    pub fn profile(&self) -> GrobidProfile {
        self.profile
    }

    /// Endpoint base URL (diagnostics only; never part of identity).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Exact backend/profile identity stamped on every envelope. Deliberately
    /// independent of transport tuning (timeout, pools, byte bounds).
    pub fn identity(&self) -> TeiIdentity {
        TeiIdentity::grobid_http(self.profile)
    }

    /// Check if GROBID is alive.
    pub async fn is_alive(&self) -> bool {
        let url = format!("{}/api/isalive", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Project a typed [`ParseBackendRequest`] onto the Java GROBID fulltext
    /// wire and run exactly one `POST /api/processFulltextDocument` exchange
    /// (R105/R107).
    ///
    /// The multipart form is the **sole projector** of the request's V1
    /// dialect: every sanctioned field keeps its exact Java GROBID name and
    /// enum value, and extended V1 fields are appended only when the dialect
    /// actually enables them (omitted equals the GROBID default), so
    /// legacy-equivalent dialects render exactly the S02 wire form — `input`,
    /// `consolidateCitations`, `consolidateHeader`, `teiCoordinates` and
    /// nothing else (D204 byte-compatible).
    ///
    /// Successful bodies are carried as verbatim bytes: no lossy text
    /// decoding, non-UTF-8 bytes survive. Outcomes are closed and typed:
    /// `200` → bytes, `204` → no content, `503` → busy with bounded
    /// `Retry-After` metadata, `400` → invalid request, `413` / opt-in byte
    /// bounds → limit, other `5xx` → conversion (Java processing/conversion
    /// failure), anything else → invalid response. There is no internal
    /// retry, the error body is never read, and no payload ever enters an
    /// error or a log.
    pub(crate) async fn process_fulltext(
        &self,
        request: &ParseBackendRequest,
    ) -> Result<Vec<u8>, ParseBackendFailure> {
        let url = format!("{}/api/processFulltextDocument", self.url);

        // Opt-in client-enforced request bound: checked before the multipart
        // form even exists, so a breach produces exactly zero wire requests.
        if let Some(max_bytes) = self.limits.request
            && request.pdf_bytes().len() > max_bytes
        {
            tracing::warn!(
                api = "grobid",
                endpoint = %url,
                enforced_limit = "request_bytes",
                "GROBID processFulltextDocument: request exceeded the opt-in byte bound"
            );
            return Err(ParseBackendFailure::limit(LimitKind::RequestBytes, None));
        }

        let part = reqwest::multipart::Part::bytes(request.pdf_bytes().to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|_| ParseBackendFailure::conversion(FailurePhase::MultipartEncode, None))?;

        // Sole dialect projection: identical field names and enum values to
        // the S02 request (D204 byte-compatible); the typed consolidation
        // levels render the same `"0"`/`"1"` wire strings for the legacy
        // presets. Field semantics:
        // - `consolidateCitations`: enrich references via Crossref/biblio-glutton
        //   (ADR-053 Layer F cross-paper edges).
        // - `consolidateHeader`: enrich paper header (title, authors, DOI,
        //   abstract) — author disambiguation (Layer G), DOI coverage.
        // - `teiCoordinates`: PDF coordinates for figures, tables, footnotes
        //   and formulas (Layer I cropping + provenance).
        let dialect = request.dialect();
        let consolidation = dialect.consolidation();
        let mut form = reqwest::multipart::Form::new()
            .part("input", part)
            .text(
                "consolidateCitations",
                consolidation.citations().to_string(),
            )
            .text("consolidateHeader", consolidation.header().to_string())
            .text("teiCoordinates", dialect.coordinates().json_value());

        // Extended V1 fields are appended only when the dialect actually
        // enables them — omitted fields equal the GROBID defaults, which
        // keeps legacy-equivalent requests byte-identical to S02.
        if consolidation.funders().value() != 0 {
            form = form.text("consolidateFunders", consolidation.funders().to_string());
        }
        if dialect.raw_citations() {
            form = form.text("includeRawCitations", "1");
        }
        if dialect.raw_affiliations() {
            form = form.text("includeRawAffiliations", "1");
        }
        if dialect.segment_sentences() {
            form = form.text("segmentSentences", "1");
        }
        if dialect.emit_ids() {
            form = form.text("generateIDs", "1");
        }
        if let Some(window) = dialect.page_window() {
            form = form
                .text("start", window.start().to_string())
                .text("end", window.end().to_string());
        }

        let mut resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %url,
                    error = %e,
                    "GROBID processFulltextDocument network error"
                );
                // Honest typed classification (timeout is distinct from
                // connect/DNS/other transport I/O). No internal retry.
                if e.is_timeout() {
                    ParseBackendFailure::timeout(FailurePhase::AwaitResponse)
                } else if e.is_connect() {
                    ParseBackendFailure::transport(FailurePhase::Connect)
                } else {
                    ParseBackendFailure::transport(FailurePhase::Send)
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            tracing::warn!(
                api = "grobid",
                endpoint = %url,
                status = %status,
                "GROBID processFulltextDocument non-success HTTP status"
            );
            // Closed typed classification; the raw error body is never read.
            return Err(match status.as_u16() {
                // Honest backpressure: typed busy, no internal retry, bounded
                // parsed Retry-After metadata (valid seconds / honest
                // Unparsable / absent).
                503 => ParseBackendFailure::busy(
                    503,
                    resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(RetryAfter::parse_header),
                ),
                400 => ParseBackendFailure::invalid_request(400),
                413 => ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
                // Java 5xx processing/conversion failures.
                s if (500..600).contains(&s) => {
                    ParseBackendFailure::conversion(FailurePhase::AwaitResponse, Some(s))
                }
                s => ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(s)),
            });
        }

        // Success is exactly 200: 204 is a typed absence (no TEI was
        // produced), and no other 2xx belongs to the Java GROBID fulltext
        // contract.
        match status.as_u16() {
            200 => {}
            204 => {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %url,
                    status = 204,
                    "GROBID processFulltextDocument returned no content"
                );
                return Err(ParseBackendFailure::no_content());
            }
            s => {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %url,
                    status = s,
                    "GROBID processFulltextDocument returned an uninterpretable success status"
                );
                return Err(ParseBackendFailure::invalid_response(
                    FailurePhase::AwaitResponse,
                    Some(s),
                ));
            }
        }

        // Success bodies are read as raw streamed bytes: the opt-in response
        // bound is enforced mid-stream, and any encoding — including
        // non-UTF-8 — survives verbatim (no lossy text decoding).
        let mut body: Vec<u8> = Vec::new();
        loop {
            let Some(chunk) = resp.chunk().await.map_err(|e| {
                tracing::warn!(
                    api = "grobid",
                    error = %e,
                    "GROBID processFulltextDocument: failed to read response body"
                );
                if e.is_timeout() {
                    ParseBackendFailure::timeout(FailurePhase::ReadBody)
                } else {
                    ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(200))
                }
            })?
            else {
                break;
            };
            body.extend_from_slice(&chunk);
            if let Some(max_bytes) = self.limits.response
                && body.len() > max_bytes
            {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %url,
                    enforced_limit = "response_bytes",
                    "GROBID processFulltextDocument: response exceeded the opt-in byte bound"
                );
                return Err(ParseBackendFailure::limit(LimitKind::ResponseBytes, None));
            }
        }

        tracing::debug!(
            api = "grobid",
            endpoint = %url,
            tei_size = body.len(),
            "GROBID processFulltextDocument: TEI XML received"
        );

        Ok(body)
    }
}

fn build_client(timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest client")
}

#[async_trait]
impl ParseBackend for GrobidHttpBackend {
    fn default_request(&self, paper_id: &str, pdf_bytes: Vec<u8>) -> ParseBackendRequest {
        // The request dialect is the sole wire projector, so the HTTP backend
        // builds requests reproducing its own profile: a structural-profile
        // facade must never silently send a rich wire request.
        ParseBackendRequest::from_legacy_profile(&self.profile, paper_id, pdf_bytes)
    }

    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let paper_id = request.paper_id().to_string();
        let tei_bytes = self.process_fulltext(&request).await?;
        let identity = self.identity();
        // Sizes and identity only — raw TEI/PDF never reaches logs.
        tracing::debug!(
            api = "grobid",
            backend = %identity.backend_id(),
            profile = %identity.profile_fingerprint(),
            paper_id = %paper_id,
            tei_size = tei_bytes.len(),
            "parse backend produced TEI envelope"
        );
        Ok(TeiEnvelope::new(tei_bytes, identity))
    }
}

/// GROBID HTTP adapter for PDF → scholarly structure (TEI XML).
///
/// Compatibility facade: same constructors, builder methods, `is_alive`
/// semantics as before the seam extraction; all of them
/// delegate to [`GrobidHttpBackend`]. The `impl ParserPort` in
/// [`crate::parser`] delegates through
/// [`crate::parser::ScholarlyParser::fetch_envelope_once`] +
/// [`crate::parser::ScholarlyParser::lift_and_project`], preserving the
/// `grobid_parse` span and aggregate identity/size/timing-only logs.
#[derive(Clone, Debug)]
pub struct GrobidParser {
    transport: GrobidHttpBackend,
}

impl GrobidParser {
    pub fn new(url: Option<&str>) -> Self {
        Self {
            transport: GrobidHttpBackend::new(url),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("GROBID_URL").unwrap_or_else(|_| DEFAULT_GROBID_URL.to_string());
        Self::new(Some(&url))
    }

    pub fn with_profile(mut self, profile: GrobidProfile) -> Self {
        self.transport = self.transport.with_profile(profile);
        self
    }

    pub fn profile(&self) -> GrobidProfile {
        self.transport.profile()
    }

    /// Override the bounded HTTP request timeout for batch profiles.
    ///
    /// Full citation/header consolidation can legitimately exceed the 120s
    /// interactive default while Crossref applies rate-limit backoff. The
    /// caller should record this operational policy in its run report. It does
    /// not alter successful TEI semantics and therefore is not a content-key input.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.transport = self.transport.with_timeout(timeout);
        self
    }

    /// Check if GROBID is alive.
    pub async fn is_alive(&self) -> bool {
        self.transport.is_alive().await
    }

    /// Provider-neutral transport seam behind this facade (R105).
    ///
    /// The `impl ParserPort` (crate::parser) drives the composition over this
    /// backend; only the facade shell keeps filesystem/env concerns.
    pub(crate) fn backend(&self) -> &GrobidHttpBackend {
        &self.transport
    }
}
