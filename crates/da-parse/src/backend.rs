//! Provider-neutral parse transport seam: the typed request, the backend
//! trait and the deterministic cached-TEI backend (R105, M464-z84in2
//! S02/T01; S04/T01 adds the versioned request dialect and the typed failure
//! vocabulary).
//!
//! Since S04/T01 [`ParseBackendRequest`] carries the serde-versioned V1
//! fulltext dialect ([`crate::transport::FulltextDialectV1`]) next to the
//! provider-neutral logical id + source bytes, and [`ParseBackend::fetch_tei`]
//! returns a closed typed failure ([`crate::transport::ParseBackendFailure`])
//! instead of the free-form `ParserError` string. [`CachedTeiBackend`] stays
//! a deterministic success.

use crate::envelope::{TeiEnvelope, TeiIdentity};
use crate::profile::GrobidProfile;
use crate::transport::{FulltextDialectV1, ParseBackendFailure};
use async_trait::async_trait;

/// Stable backend id for the Java GROBID HTTP transport.
pub const GROBID_HTTP_BACKEND_ID: &str = "grobid-http";
/// Stable backend id for cached-TEI replay.
pub const CACHED_TEI_BACKEND_ID: &str = "cached-tei";

/// Provider-neutral fulltext parse request (R105/R107).
///
/// Logical identity (`paper_id`), source bytes and the output-affecting V1
/// dialect — nothing else. No `PathBuf`, no env values, no cache roots, no
/// timeout or size limits: transport tuning is deliberately not part of the
/// request vocabulary and therefore never enters content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBackendRequest {
    paper_id: String,
    pdf_bytes: Vec<u8>,
    dialect: FulltextDialectV1,
}

impl ParseBackendRequest {
    /// Legacy/default constructor: provider-neutral logical id + source
    /// bytes, with the dialect reproducing the current default
    /// (`GrobidProfile::rich()`) fulltext wire request exactly.
    pub fn new(paper_id: impl Into<String>, pdf_bytes: impl Into<Vec<u8>>) -> Self {
        Self::with_dialect(paper_id, pdf_bytes, FulltextDialectV1::legacy_rich())
    }

    /// Build from a legacy profile preset: the dialect reproduces that
    /// profile's fulltext wire request (`rich`, `structural` or `custom`).
    pub fn from_legacy_profile(
        profile: &GrobidProfile,
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self::with_dialect(paper_id, pdf_bytes, profile.dialect())
    }

    /// Full constructor over an explicit V1 dialect.
    pub fn with_dialect(
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
        dialect: FulltextDialectV1,
    ) -> Self {
        Self {
            paper_id: paper_id.into(),
            pdf_bytes: pdf_bytes.into(),
            dialect,
        }
    }

    /// Provider-neutral logical paper id (never a physical path obligation).
    pub fn paper_id(&self) -> &str {
        &self.paper_id
    }

    /// Source bytes (PDF for the fulltext path; irrelevant payload for
    /// cached-TEI replay).
    pub fn pdf_bytes(&self) -> &[u8] {
        &self.pdf_bytes
    }

    /// The versioned V1 dialect carried by this request.
    pub const fn dialect(&self) -> &FulltextDialectV1 {
        &self.dialect
    }
}

/// Provider-neutral transport seam (R105/R107).
///
/// Exactly one transport outcome per call: either a verbatim-byte
/// [`TeiEnvelope`] or one closed [`ParseBackendFailure`]. Implementations
/// never retry internally — failures surface verbatim with their structured
/// metadata intact.
#[async_trait]
pub trait ParseBackend: Send + Sync {
    /// Fetch TEI for `request` — at most one invocation, typed outcome.
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure>;
}

/// Deterministic cached-TEI backend: serves already-available bytes with no
/// transport at all. Infallible by construction (S04: still a deterministic
/// success under the typed failure trait).
#[derive(Debug, Clone)]
pub struct CachedTeiBackend {
    tei_bytes: Vec<u8>,
    identity: TeiIdentity,
}

impl CachedTeiBackend {
    /// Cached backend over `tei_bytes`, identity stamped from `profile`.
    pub fn new(tei_bytes: impl Into<Vec<u8>>, profile: GrobidProfile) -> Self {
        Self::with_identity(tei_bytes, TeiIdentity::cached_tei(profile))
    }

    /// Cached backend over `tei_bytes` with an explicit identity.
    pub fn with_identity(tei_bytes: impl Into<Vec<u8>>, identity: TeiIdentity) -> Self {
        Self {
            tei_bytes: tei_bytes.into(),
            identity,
        }
    }

    /// Exact provenance identity stamped on every served envelope.
    pub fn identity(&self) -> &TeiIdentity {
        &self.identity
    }
}

#[async_trait]
impl ParseBackend for CachedTeiBackend {
    async fn fetch_tei(
        &self,
        _request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        Ok(TeiEnvelope::new(
            self.tei_bytes.clone(),
            self.identity.clone(),
        ))
    }
}
