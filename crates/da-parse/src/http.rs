//! Java GROBID HTTP transport backend + `GrobidParser` compatibility facade.
//!
//! Since M464-z84in2 S02/T01 the transport is exposed through the
//! provider-neutral [`ParseBackend`] seam ([`GrobidHttpBackend`]); the
//! historical [`GrobidParser`] stays as a compatibility facade delegating to
//! it, so existing `Arc<dyn ParserPort>` callers, the `grobid_parse` lifecycle
//! span, external `ParserError` mapping and cache identities are unchanged
//! (R105). Since S04/T01 the backend seam carries the closed typed failure
//! vocabulary ([`ParseBackendFailure`]); the wire-level per-branch 204/503/
//! timeout/limit stub proofs land in slice S04/T02.
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

/// Provider-neutral view of the Java GROBID HTTP transport (R105).
///
/// Owns exactly the transport concern: endpoint URL, HTTP client and the
/// effective-input [`GrobidProfile`] that tunes the REST dialect. Lifting and
/// projection do not live here.
#[derive(Clone, Debug)]
pub struct GrobidHttpBackend {
    url: String,
    client: reqwest::Client,
    profile: GrobidProfile,
}

impl GrobidHttpBackend {
    /// Build the backend against `url` (default `DEFAULT_GROBID_URL`) with
    /// the rich profile and the bounded interactive timeout.
    pub fn new(url: Option<&str>) -> Self {
        Self {
            url: url.unwrap_or(DEFAULT_GROBID_URL).to_string(),
            client: build_client(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
            profile: GrobidProfile::rich(),
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

    pub fn profile(&self) -> GrobidProfile {
        self.profile
    }

    /// Endpoint base URL (diagnostics only; never part of identity).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Exact backend/profile identity stamped on every envelope. Deliberately
    /// independent of transport tuning (timeout, pools).
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

    /// Process PDF via GROBID full-text endpoint → TEI XML.
    ///
    /// Sends consolidation + coordinate parameters for richer extraction:
    /// - `consolidateCitations=1`: enrich references via Crossref/biblio-glutton
    ///   (adds DOI, venue, publisher, normalized author names). Critical for
    ///   ADR-053 Layer F cross-paper edges (predecessor_of, superseded_by).
    /// - `consolidateHeader=1`: enrich paper header (title, authors, DOI, abstract)
    ///   via Crossref. Improves author disambiguation (Layer G) and DOI coverage.
    /// - `teiCoordinates=["figure","table","note","formula"]`: add PDF
    ///   coordinates for figures, tables, footnotes, and formulas (M400 S03
    ///   T01 adds `formula` so GROBID emits `coords` on `<formula>` elements).
    ///   Enables Layer I figure image cropping and provenance tracking.
    ///
    /// These parameters are GROBID 0.9.0 REST API options (see GROBID docs).
    /// They add ~2-5s per paper for Crossref lookups; acceptable for batch ingest.
    pub(crate) async fn process_fulltext(
        &self,
        pdf_bytes: &[u8],
    ) -> Result<String, ParseBackendFailure> {
        let url = format!("{}/api/processFulltextDocument", self.url);
        let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|_| ParseBackendFailure::conversion(FailurePhase::MultipartEncode, None))?;

        // Wire dialect: identical field names and enum values to the S02
        // request (D204 byte-compatible); the typed consolidation levels
        // render the same `"0"`/`"1"` wire strings for the legacy presets.
        // S04/T02 makes this multipart the sole projector of the typed
        // request dialect.
        let consolidation = self.profile.consolidation();
        let form = reqwest::multipart::Form::new()
            .part("input", part)
            .text(
                "consolidateCitations",
                consolidation.citations().to_string(),
            )
            .text("consolidateHeader", consolidation.header().to_string())
            .text("teiCoordinates", GrobidProfile::tei_coordinates_json());

        let resp = self
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
                503 => ParseBackendFailure::busy(
                    503,
                    resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(RetryAfter::parse_header),
                ),
                400 => ParseBackendFailure::invalid_request(400),
                413 => ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
                s if (500..600).contains(&s) => {
                    ParseBackendFailure::conversion(FailurePhase::AwaitResponse, Some(s))
                }
                s => ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(s)),
            });
        }

        let tei_xml = resp.text().await.map_err(|e| {
            tracing::warn!(
                api = "grobid",
                error = %e,
                "GROBID processFulltextDocument: failed to read response body"
            );
            ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(status.as_u16()))
        })?;

        tracing::debug!(
            api = "grobid",
            endpoint = %url,
            tei_size = tei_xml.len(),
            "GROBID processFulltextDocument: TEI XML received"
        );

        Ok(tei_xml)
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
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let paper_id = request.paper_id().to_string();
        let tei_xml = self.process_fulltext(request.pdf_bytes()).await?;
        let identity = self.identity();
        // Sizes and identity only — raw TEI/PDF never reaches logs.
        tracing::debug!(
            api = "grobid",
            backend = %identity.backend_id(),
            profile = %identity.profile_fingerprint(),
            paper_id = %paper_id,
            tei_size = tei_xml.len(),
            "parse backend produced TEI envelope"
        );
        Ok(TeiEnvelope::new(tei_xml.into_bytes(), identity))
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
