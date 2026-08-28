//! Provider-neutral parse backend seam (R105): transport ends here.
//!
//! A [`ParseBackend`] turns a logical request (paper id + source bytes) into a
//! [`TeiEnvelope`]. It owns *how* TEI is obtained (Java GROBID HTTP, cached
//! bytes) and nothing else: lifting and projection live in the lifter and the
//! [`crate::http::GrobidParser`] compatibility facade.

use crate::envelope::{TeiEnvelope, TeiIdentity};
use crate::profile::GrobidProfile;
use async_trait::async_trait;
use da_ports::parser::ParseResult;

/// Stable backend id for the Java GROBID HTTP transport.
pub const GROBID_HTTP_BACKEND_ID: &str = "grobid-http";

/// Stable backend id for replay of already-available cached TEI bytes.
pub const CACHED_TEI_BACKEND_ID: &str = "cached-tei";

/// Logical, provider-neutral backend request.
///
/// Carries the paper's logical identity and its source bytes. Deliberately
/// free of `PathBuf`s, env/cache roots and physical source paths: filesystem
/// layout is a compatibility-shell concern (`GrobidParser::parse_pdf` reads
/// the file, then hands bytes across this seam).
#[derive(Debug, Clone)]
pub struct ParseBackendRequest {
    paper_id: String,
    pdf_bytes: Vec<u8>,
}

impl ParseBackendRequest {
    /// Build a request from a logical paper id and raw source bytes.
    pub fn new(paper_id: impl Into<String>, pdf_bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            paper_id: paper_id.into(),
            pdf_bytes: pdf_bytes.into(),
        }
    }

    /// Logical paper identity (never a filesystem path).
    pub fn paper_id(&self) -> &str {
        &self.paper_id
    }

    /// Raw source document bytes.
    pub fn pdf_bytes(&self) -> &[u8] {
        &self.pdf_bytes
    }
}

/// Transport seam between source acquisition and TEI lifting (R105).
///
/// Contract (enforced by `tests/backend_contract.rs` for every backend):
/// - the returned [`TeiEnvelope`] carries the TEI bytes verbatim (no
///   normalization) plus exact backend/profile identity;
/// - identity is independent of transport tuning (timeouts);
/// - failures surface as `Err(ParserError)` — never panic, never silently
///   skip. Typed 204/503/timeout semantics belong to slice S04; this seam
///   must not widen the transport dialect.
#[async_trait]
pub trait ParseBackend: Send + Sync {
    /// Acquire the raw TEI payload for `request`.
    async fn fetch_tei(&self, request: ParseBackendRequest) -> ParseResult<TeiEnvelope>;
}

/// Backend serving already-available TEI bytes without any transport.
///
/// This is the offline/cached side of R105: preserved TEI (e.g. S01
/// archived sidecars) is lifted without HTTP. Bytes are wrapped, never
/// validated or normalized here — empty/malformed payloads stay verbatim so
/// nothing is silently skipped (S04 types the absence later).
#[derive(Debug, Clone)]
pub struct CachedTeiBackend {
    raw_tei: Vec<u8>,
    identity: TeiIdentity,
}

impl CachedTeiBackend {
    /// Wrap already-available raw TEI bytes with cached-tei identity for
    /// `profile` (the profile that produced the bytes).
    pub fn new(raw_tei: impl Into<Vec<u8>>, profile: GrobidProfile) -> Self {
        Self::with_identity(raw_tei, TeiIdentity::cached_tei(profile))
    }

    /// Wrap bytes with an explicit [`TeiIdentity`], preserving the
    /// producing-backend provenance of archived payloads.
    pub fn with_identity(raw_tei: impl Into<Vec<u8>>, identity: TeiIdentity) -> Self {
        Self {
            raw_tei: raw_tei.into(),
            identity,
        }
    }

    /// Exact backend/profile identity stamped on every envelope.
    pub fn identity(&self) -> &TeiIdentity {
        &self.identity
    }
}

#[async_trait]
impl ParseBackend for CachedTeiBackend {
    async fn fetch_tei(&self, _request: ParseBackendRequest) -> ParseResult<TeiEnvelope> {
        // Bytes are already available: no transport, no failure modes, no
        // normalization — identical envelope on every call.
        Ok(TeiEnvelope::new(
            self.raw_tei.clone(),
            self.identity.clone(),
        ))
    }
}
