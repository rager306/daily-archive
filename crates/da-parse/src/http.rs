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
//!
//! Since S05/T02 all three closed endpoints (fulltext, header, references)
//! share ONE endpoint-neutral one-shot multipart exchange: exactly one `POST`
//! to the endpoint's static Java route and exactly one streamed response
//! read, classified by the one shared typed-outcome ladder. Each endpoint
//! projects its own closed dialect — the header/references allowlists never
//! emit `teiCoordinates` or a foreign knob — route selection is the static
//! closed [`ParseEndpoint`] mapping (never a caller-controlled URL), and
//! proxy successes return the partial header/listBibl TEI directly as a
//! verbatim [`TeiEnvelope`] stamped with the S05 endpoint result identity
//! (no lifter, no projection, no `ParserPort`). Logs stay redacted: logical
//! endpoints, sizes and typed codes only — never a base URL, payload or
//! response body (reqwest error text embeds URLs, so only typed code/phase
//! metadata is ever logged).

use crate::backend::{ParseBackend, ParseBackendRequest};
use crate::envelope::{TeiEnvelope, TeiIdentity};
use crate::profile::GrobidProfile;
use crate::transport::{FailurePhase, LimitKind, ParseBackendFailure, ParseEndpoint, RetryAfter};
use async_trait::async_trait;

pub(crate) const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";

/// Fail-closed outcome of a GROBID liveness/identity probe (not ParseBackend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFailure {
    /// Transport error or HTTP 5xx. Same availability class as `is_alive == false`.
    Unavailable,
    /// 2xx/4xx with a body that is not the closed health/version shape.
    InvalidResponse,
}

/// Successful `GET /api/health` (JSON object, status recorded, body not identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthProbe {
    pub status: u16,
}

/// Successful `GET /api/version` (JSON object or plain string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionProbe {
    pub version: String,
    pub name: Option<String>,
    pub revision: Option<String>,
}

fn parse_version_payload(bytes: &[u8]) -> Result<VersionProbe, ProbeFailure> {
    let trimmed = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &bytes[i..])
        .unwrap_or(&[]);
    if trimmed.is_empty() {
        return Err(ProbeFailure::InvalidResponse);
    }
    if trimmed[0] == b'{' {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| ProbeFailure::InvalidResponse)?;
        let obj = value.as_object().ok_or(ProbeFailure::InvalidResponse)?;
        let version = obj
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or(ProbeFailure::InvalidResponse)?;
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let revision = obj
            .get("revision")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(VersionProbe {
            version: version.to_string(),
            name,
            revision,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ProbeFailure::InvalidResponse)?
        .trim();
    if text.is_empty() || text.contains('<') {
        return Err(ProbeFailure::InvalidResponse);
    }
    Ok(VersionProbe {
        version: text.to_string(),
        name: None,
        revision: None,
    })
}

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
    backend_version: Option<String>,
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
            backend_version: None,
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

    /// Stamp a previously probed `/api/version` string onto envelopes.
    /// Empty input is ignored. Does not call the network; `fetch_tei` never
    /// probes version on its own (T02 additive identity only).
    pub fn with_backend_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        let trimmed = version.trim();
        if !trimmed.is_empty() {
            self.backend_version = Some(trimmed.to_string());
        }
        self
    }

    /// Exact backend/profile identity stamped on every envelope. Deliberately
    /// independent of transport tuning (timeout, pools, byte bounds).
    /// Backend version is included only when [`Self::with_backend_version`]
    /// was used — default identities stay historically comparable.
    pub fn identity(&self) -> TeiIdentity {
        let identity = TeiIdentity::grobid_http(self.profile);
        match &self.backend_version {
            Some(version) => identity.with_backend_version(version.clone()),
            None => identity,
        }
    }

    /// Check if GROBID is alive.
    pub async fn is_alive(&self) -> bool {
        let url = format!("{}/api/isalive", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// `GET /api/health` (ADR-076 W0 / R121). Not part of [`ParseBackend`].
    ///
    /// 2xx with a JSON object is success. Transport/5xx → [`ProbeFailure::Unavailable`].
    /// Other statuses or non-object bodies → [`ProbeFailure::InvalidResponse`].
    /// The payload never enters [`TeiIdentity`].
    pub async fn probe_health(&self) -> Result<HealthProbe, ProbeFailure> {
        let url = format!("{}/api/health", self.url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|_| ProbeFailure::Unavailable)?;
        let status = response.status().as_u16();
        if status >= 500 || status == 0 {
            return Err(ProbeFailure::Unavailable);
        }
        if !(200..300).contains(&status) {
            return Err(ProbeFailure::InvalidResponse);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ProbeFailure::Unavailable)?;
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(serde_json::Value::Object(_)) => Ok(HealthProbe { status }),
            _ => Err(ProbeFailure::InvalidResponse),
        }
    }

    /// `GET /api/version` (ADR-076 W0 / R121). Not part of [`ParseBackend`].
    ///
    /// Accepts JSON `{name, version, revision}` (version required) or a non-empty
    /// plain version string. Transport/5xx → Unavailable. Empty/unparseable →
    /// InvalidResponse. The string is diagnostic until T02 stamps identity.
    pub async fn probe_version(&self) -> Result<VersionProbe, ProbeFailure> {
        let url = format!("{}/api/version", self.url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|_| ProbeFailure::Unavailable)?;
        let status = response.status().as_u16();
        if status >= 500 || status == 0 {
            return Err(ProbeFailure::Unavailable);
        }
        if !(200..300).contains(&status) {
            return Err(ProbeFailure::InvalidResponse);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ProbeFailure::Unavailable)?;
        parse_version_payload(&bytes)
    }

    /// The shared multipart `input` part (S05/T02): the source PDF bytes
    /// under the stable upload filename, identical for every endpoint.
    fn input_part(pdf_bytes: &[u8]) -> Result<reqwest::multipart::Part, ParseBackendFailure> {
        reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|_| ParseBackendFailure::conversion(FailurePhase::MultipartEncode, None))
    }

    /// Closed fulltext projector (S02/S04 wire form, D204 byte-compatible):
    /// the **sole projector** of the request's V1 fulltext dialect.
    ///
    /// Identical field names and enum values to the S02 request; the typed
    /// consolidation levels render the same `"0"`/`"1"` wire strings for the
    /// legacy presets. Field semantics:
    /// - `consolidateCitations`: enrich references via Crossref/biblio-glutton
    ///   (ADR-053 Layer F cross-paper edges).
    /// - `consolidateHeader`: enrich paper header (title, authors, DOI,
    ///   abstract) — author disambiguation (Layer G), DOI coverage.
    /// - `teiCoordinates`: PDF coordinates for figures, tables, footnotes
    ///   and formulas (Layer I cropping + provenance).
    fn fulltext_form(
        request: &ParseBackendRequest,
    ) -> Result<reqwest::multipart::Form, ParseBackendFailure> {
        let dialect = request.dialect();
        let consolidation = dialect.consolidation();
        let mut form = reqwest::multipart::Form::new()
            .part("input", Self::input_part(request.pdf_bytes())?)
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
        Ok(form)
    }

    /// Closed header projector (S05/T02): the multipart allowlist is exactly
    /// `input`, `consolidateHeader` and the optional `includeRawAffiliations`
    /// switch — never `teiCoordinates` and never a fulltext or references
    /// knob (R113).
    fn header_form(
        request: &ParseBackendRequest,
    ) -> Result<reqwest::multipart::Form, ParseBackendFailure> {
        let dialect = request
            .header_dialect()
            .expect("header requests carry a header dialect");
        let mut form = reqwest::multipart::Form::new()
            .part("input", Self::input_part(request.pdf_bytes())?)
            .text(
                "consolidateHeader",
                dialect.consolidate_header().to_string(),
            );
        if dialect.include_raw_affiliations() {
            form = form.text("includeRawAffiliations", "1");
        }
        Ok(form)
    }

    /// Closed references projector (S05/T02): the multipart allowlist is
    /// exactly `input`, `consolidateCitations` and the optional
    /// `includeRawCitations` switch — never `teiCoordinates` and never a
    /// fulltext or header knob (R113).
    fn references_form(
        request: &ParseBackendRequest,
    ) -> Result<reqwest::multipart::Form, ParseBackendFailure> {
        let dialect = request
            .references_dialect()
            .expect("references requests carry a references dialect");
        let mut form = reqwest::multipart::Form::new()
            .part("input", Self::input_part(request.pdf_bytes())?)
            .text(
                "consolidateCitations",
                dialect.consolidate_citations().to_string(),
            );
        if dialect.include_raw_citations() {
            form = form.text("includeRawCitations", "1");
        }
        Ok(form)
    }

    /// One endpoint-neutral one-shot multipart exchange (S05/T02): exactly
    /// one `POST` to `endpoint`'s static Java GROBID route and exactly one
    /// streamed response read, classified by the one shared typed-outcome
    /// ladder. No retry, no queue, no concurrency policy — one operation is
    /// one PDF upload and one response stream (S06 owns any load policy).
    ///
    /// Outcomes are closed and typed: `200` → bytes, `204` → no content,
    /// `503` → busy with bounded `Retry-After` metadata, `400` → invalid
    /// request, `413` / opt-in byte bounds → limit, other `5xx` → conversion
    /// (Java processing/conversion failure), anything else → invalid
    /// response. There is no internal retry and the error body is never
    /// read. Logs stay redacted: the logical endpoint, sizes and typed
    /// code/phase metadata only — never a URL, payload or response body
    /// (reqwest error text embeds URLs, so it is never logged).
    async fn exchange_once(
        &self,
        endpoint: ParseEndpoint,
        pdf_len: usize,
        form: reqwest::multipart::Form,
    ) -> Result<Vec<u8>, ParseBackendFailure> {
        let url = format!("{}{}", self.url, endpoint.static_path());

        // Opt-in client-enforced request bound: checked before any wire
        // bytes exist, so a breach produces exactly zero HTTP requests.
        if let Some(max_bytes) = self.limits.request
            && pdf_len > max_bytes
        {
            tracing::warn!(
                api = "grobid",
                endpoint = %endpoint,
                enforced_limit = "request_bytes",
                "GROBID request exceeded the opt-in byte bound before any wire request"
            );
            return Err(ParseBackendFailure::limit(LimitKind::RequestBytes, None));
        }

        let mut resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                // Honest typed classification (timeout is distinct from
                // connect/DNS/other transport I/O). No internal retry.
                let failure = if e.is_timeout() {
                    ParseBackendFailure::timeout(FailurePhase::AwaitResponse)
                } else if e.is_connect() {
                    ParseBackendFailure::transport(FailurePhase::Connect)
                } else {
                    ParseBackendFailure::transport(FailurePhase::Send)
                };
                tracing::warn!(
                    api = "grobid",
                    endpoint = %endpoint,
                    code = %failure.code(),
                    phase = %failure.phase(),
                    "GROBID exchange network error (single attempt, no retry)"
                );
                failure
            })?;

        let status = resp.status();
        if !status.is_success() {
            // Closed typed classification; the raw error body is never read.
            let failure = match status.as_u16() {
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
            };
            tracing::warn!(
                api = "grobid",
                endpoint = %endpoint,
                status = status.as_u16(),
                code = %failure.code(),
                "GROBID non-success HTTP status (single attempt, no retry)"
            );
            return Err(failure);
        }

        // Success is exactly 200: 204 is a typed absence (no TEI was
        // produced), and no other 2xx belongs to the Java GROBID contract.
        match status.as_u16() {
            200 => {}
            204 => {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %endpoint,
                    status = 204,
                    "GROBID returned no content"
                );
                return Err(ParseBackendFailure::no_content());
            }
            s => {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %endpoint,
                    status = s,
                    "GROBID returned an uninterpretable success status"
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
                let failure = if e.is_timeout() {
                    ParseBackendFailure::timeout(FailurePhase::ReadBody)
                } else {
                    ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(200))
                };
                tracing::warn!(
                    api = "grobid",
                    endpoint = %endpoint,
                    code = %failure.code(),
                    phase = %failure.phase(),
                    "GROBID response body read failed (single attempt, no retry)"
                );
                failure
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
                    endpoint = %endpoint,
                    enforced_limit = "response_bytes",
                    "GROBID response exceeded the opt-in byte bound"
                );
                return Err(ParseBackendFailure::limit(LimitKind::ResponseBytes, None));
            }
        }

        // Endpoint-aware success signal: logical endpoint + size only.
        tracing::debug!(
            api = "grobid",
            endpoint = %endpoint,
            tei_size = body.len(),
            "GROBID TEI received"
        );

        Ok(body)
    }

    /// Run the closed fulltext operation (R105/R107): exactly one
    /// `POST /api/processFulltextDocument` with the fulltext allowlist.
    pub(crate) async fn process_fulltext(
        &self,
        request: &ParseBackendRequest,
    ) -> Result<Vec<u8>, ParseBackendFailure> {
        let form = Self::fulltext_form(request)?;
        self.exchange_once(ParseEndpoint::Fulltext, request.pdf_bytes().len(), form)
            .await
    }

    /// Run the closed header operation (S05/T02): exactly one
    /// `POST /api/processHeaderDocument` with the header allowlist.
    pub(crate) async fn process_header(
        &self,
        request: &ParseBackendRequest,
    ) -> Result<Vec<u8>, ParseBackendFailure> {
        let form = Self::header_form(request)?;
        self.exchange_once(ParseEndpoint::Header, request.pdf_bytes().len(), form)
            .await
    }

    /// Run the closed references operation (S05/T02): exactly one
    /// `POST /api/processReferences` with the references allowlist.
    pub(crate) async fn process_references(
        &self,
        request: &ParseBackendRequest,
    ) -> Result<Vec<u8>, ParseBackendFailure> {
        let form = Self::references_form(request)?;
        self.exchange_once(ParseEndpoint::References, request.pdf_bytes().len(), form)
            .await
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

    fn default_header_request(&self, paper_id: &str, pdf_bytes: Vec<u8>) -> ParseBackendRequest {
        // Same rule as `default_request`, mapped onto the header axis only:
        // this backend's consolidateHeader level, raw switches off.
        ParseBackendRequest::header_from_legacy_profile(&self.profile, paper_id, pdf_bytes)
    }

    fn default_references_request(
        &self,
        paper_id: &str,
        pdf_bytes: Vec<u8>,
    ) -> ParseBackendRequest {
        // Same rule as `default_request`, mapped onto the citations axis
        // only: this backend's consolidateCitations level, raw switches off.
        ParseBackendRequest::references_from_legacy_profile(&self.profile, paper_id, pdf_bytes)
    }

    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let paper_id = request.paper_id().to_string();
        // One closed operation per fetch_tei, routed ONLY through the static
        // closed endpoint mapping (no caller-controlled route or URL).
        // Proxy successes return the partial header/listBibl TEI directly as
        // a TeiEnvelope: no TeiLifter, no ParsedArticle projection, no
        // ParserPort involvement on this seam.
        let (tei_bytes, result) = match request.endpoint() {
            ParseEndpoint::Fulltext => (self.process_fulltext(&request).await?, None),
            ParseEndpoint::Header => (
                self.process_header(&request).await?,
                Some(request.result_identity()),
            ),
            ParseEndpoint::References => (
                self.process_references(&request).await?,
                Some(request.result_identity()),
            ),
        };
        // S05/T02: header/references envelopes carry the endpoint/profile/
        // result identity; the fulltext stamp keeps its pre-S05 shape
        // (legacy-equivalent fulltext rides the D204 profile fingerprint),
        // so every pre-S05 identity pin stays byte-identical.
        let identity = match result {
            Some(result) => self.identity().with_result(result),
            None => self.identity(),
        };
        // Sizes and identity only — raw TEI/PDF never reaches logs.
        tracing::debug!(
            api = "grobid",
            backend = %identity.backend_id(),
            profile = %identity.profile_fingerprint(),
            endpoint = %request.endpoint(),
            result_kind = %request.endpoint().result_kind(),
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

    /// Facade for [`GrobidHttpBackend::probe_health`].
    pub async fn probe_health(&self) -> Result<HealthProbe, ProbeFailure> {
        self.transport.probe_health().await
    }

    /// Facade for [`GrobidHttpBackend::probe_version`].
    pub async fn probe_version(&self) -> Result<VersionProbe, ProbeFailure> {
        self.transport.probe_version().await
    }

    /// Provider-neutral transport seam behind this facade (R105).
    ///
    /// The `impl ParserPort` (crate::parser) drives the composition over this
    /// backend; only the facade shell keeps filesystem/env concerns.
    pub(crate) fn backend(&self) -> &GrobidHttpBackend {
        &self.transport
    }
}
