//! Java GROBID HTTP client (`processFulltextDocument` / `isalive`).

use crate::profile::GrobidProfile;
use da_ports::parser::{ParseResult, ParserError};

pub(crate) const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";

/// GROBID HTTP adapter for PDF → scholarly structure (TEI XML).
#[derive(Clone)]
pub struct GrobidParser {
    url: String,
    client: reqwest::Client,
    profile: GrobidProfile,
}

impl GrobidParser {
    pub fn new(url: Option<&str>) -> Self {
        Self {
            url: url.unwrap_or(DEFAULT_GROBID_URL).to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("reqwest client"),
            profile: GrobidProfile::rich(),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("GROBID_URL").unwrap_or_else(|_| DEFAULT_GROBID_URL.to_string());
        Self::new(Some(&url))
    }

    /// Override the bounded HTTP request timeout for batch profiles.
    ///
    /// Full citation/header consolidation can legitimately exceed the 120s
    /// interactive default while Crossref applies rate-limit backoff. The
    /// caller should record this operational policy in its run report. It does
    /// not alter successful TEI semantics and therefore is not a content-key input.
    pub fn with_profile(mut self, profile: GrobidProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn profile(&self) -> GrobidProfile {
        self.profile
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client");
        self
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
    pub(crate) async fn process_fulltext(&self, pdf_bytes: &[u8]) -> ParseResult<String> {
        let url = format!("{}/api/processFulltextDocument", self.url);
        let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|e| ParserError::ParseFailed(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("input", part)
            .text(
                "consolidateCitations",
                if self.profile.consolidate_citations {
                    "1"
                } else {
                    "0"
                },
            )
            .text(
                "consolidateHeader",
                if self.profile.consolidate_header {
                    "1"
                } else {
                    "0"
                },
            )
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
                ParserError::Unavailable(e.to_string())
            })?;

        let status = resp.status();
        if !status.is_success() {
            tracing::warn!(
                api = "grobid",
                endpoint = %url,
                status = %status,
                "GROBID processFulltextDocument non-success HTTP status"
            );
            return Err(ParserError::ParseFailed(format!("GROBID HTTP {status}")));
        }

        let tei_xml = resp.text().await.map_err(|e| {
            tracing::warn!(
                api = "grobid",
                error = %e,
                "GROBID processFulltextDocument: failed to read response body"
            );
            ParserError::ParseFailed(e.to_string())
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
