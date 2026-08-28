//! Provider-neutral parse transport seam: the typed request, the backend
//! trait and the deterministic cached-TEI backend (R105, M464-z84in2
//! S02/T01; S04/T01 adds the versioned request dialect and the typed failure
//! vocabulary; S04/T02 adds backend-profile-aware default requests;
//! S05/T01 adds the closed header/references endpoint operations with
//! per-endpoint result identities, through the same single `fetch_tei`
//! method).
//!
//! Since S04/T01 [`ParseBackendRequest`] carries the serde-versioned V1
//! fulltext dialect ([`crate::transport::FulltextDialectV1`]) next to the
//! provider-neutral logical id + source bytes, and [`ParseBackend::fetch_tei`]
//! returns a closed typed failure ([`crate::transport::ParseBackendFailure`])
//! instead of the free-form `ParserError` string. [`CachedTeiBackend`] stays
//! a deterministic success.

use std::fmt;

use crate::envelope::{TeiEnvelope, TeiIdentity, TeiResultIdentity};
use crate::profile::GrobidProfile;
use crate::transport::{
    FulltextDialectV1, HeaderDialectV1, ParseBackendFailure, ParseEndpoint, ReferencesDialectV1,
};
use async_trait::async_trait;

/// Stable backend id for the Java GROBID HTTP transport.
pub const GROBID_HTTP_BACKEND_ID: &str = "grobid-http";
/// Stable backend id for cached-TEI replay.
pub const CACHED_TEI_BACKEND_ID: &str = "cached-tei";

/// Provider-neutral parse request over one closed scholarly operation
/// (R105/R107; S05/T01 adds the header and references operations).
///
/// Logical identity (`paper_id`), source bytes and the output-affecting
/// dialect of exactly one operation — nothing else. No `PathBuf`, no env
/// values, no cache roots, no timeout or size limits: transport tuning is
/// deliberately not part of the request vocabulary and therefore never
/// enters content identity. The endpoint discriminant is closed
/// ([`ParseEndpoint`]); endpoint-specific knobs cannot cross endpoints
/// because each operation owns its own dialect type.
///
/// Debug rendering is a deterministic redaction: source bytes are never
/// rendered (length only), keeping PDF payloads out of logs (pinned by
/// `tests/transport_contract.rs`).
#[derive(Clone, PartialEq, Eq)]
pub struct ParseBackendRequest {
    paper_id: String,
    pdf_bytes: Vec<u8>,
    dialect: FulltextDialectV1,
    endpoint: ParseEndpoint,
    header_dialect: Option<HeaderDialectV1>,
    references_dialect: Option<ReferencesDialectV1>,
}

impl fmt::Debug for ParseBackendRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParseBackendRequest")
            .field("paper_id", &self.paper_id)
            .field("pdf_bytes_len", &self.pdf_bytes.len())
            .field("endpoint", &self.endpoint)
            .field("fulltext_dialect", &self.dialect)
            .field("header_dialect", &self.header_dialect)
            .field("references_dialect", &self.references_dialect)
            .finish()
    }
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

    /// Full constructor over an explicit V1 dialect (fulltext endpoint).
    pub fn with_dialect(
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
        dialect: FulltextDialectV1,
    ) -> Self {
        Self {
            paper_id: paper_id.into(),
            pdf_bytes: pdf_bytes.into(),
            dialect,
            endpoint: ParseEndpoint::Fulltext,
            header_dialect: None,
            references_dialect: None,
        }
    }

    /// Header operation request (S05/T01): the closed header dialect is the
    /// sole control surface. The legacy fulltext dialect field carries the
    /// neutral rich default and is never projected for endpoint-specific
    /// requests — the endpoint discriminant routes the transport.
    pub fn header(
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
        dialect: HeaderDialectV1,
    ) -> Self {
        Self {
            paper_id: paper_id.into(),
            pdf_bytes: pdf_bytes.into(),
            dialect: FulltextDialectV1::legacy_rich(),
            endpoint: ParseEndpoint::Header,
            header_dialect: Some(dialect),
            references_dialect: None,
        }
    }

    /// References operation request (S05/T01): the closed references
    /// dialect is the sole control surface.
    pub fn references(
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
        dialect: ReferencesDialectV1,
    ) -> Self {
        Self {
            paper_id: paper_id.into(),
            pdf_bytes: pdf_bytes.into(),
            dialect: FulltextDialectV1::legacy_rich(),
            endpoint: ParseEndpoint::References,
            header_dialect: None,
            references_dialect: Some(dialect),
        }
    }

    /// Legacy profile mapped onto the header axis only (S05/T01): the
    /// profile's `consolidateHeader` level, raw affiliations off.
    pub fn header_from_legacy_profile(
        profile: &GrobidProfile,
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self::header(
            paper_id,
            pdf_bytes,
            HeaderDialectV1::from_legacy_profile(profile),
        )
    }

    /// Legacy profile mapped onto the citations axis only (S05/T01): the
    /// profile's `consolidateCitations` level, raw citations off.
    pub fn references_from_legacy_profile(
        profile: &GrobidProfile,
        paper_id: impl Into<String>,
        pdf_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self::references(
            paper_id,
            pdf_bytes,
            ReferencesDialectV1::from_legacy_profile(profile),
        )
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

    /// The versioned V1 dialect carried by this request (fulltext
    /// endpoint; the neutral rich default for endpoint-specific requests).
    pub const fn dialect(&self) -> &FulltextDialectV1 {
        &self.dialect
    }

    /// Closed logical endpoint of this request (S05/T01).
    pub const fn endpoint(&self) -> ParseEndpoint {
        self.endpoint
    }

    /// The header dialect of a header request (`None` otherwise).
    pub const fn header_dialect(&self) -> Option<HeaderDialectV1> {
        self.header_dialect
    }

    /// The references dialect of a references request (`None` otherwise).
    pub const fn references_dialect(&self) -> Option<ReferencesDialectV1> {
        self.references_dialect
    }

    /// Deterministic result identity this request will stamp (S05/T01):
    /// derived from the closed endpoint dialect only — never from transport
    /// tuning, locations or credentials.
    pub fn result_identity(&self) -> TeiResultIdentity {
        match self.endpoint {
            ParseEndpoint::Fulltext => TeiResultIdentity::fulltext(self.dialect),
            ParseEndpoint::Header => TeiResultIdentity::header(
                self.header_dialect
                    .expect("header requests carry a header dialect"),
            ),
            ParseEndpoint::References => TeiResultIdentity::references(
                self.references_dialect
                    .expect("references requests carry a references dialect"),
            ),
        }
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

    /// Build the request paired with this backend's default effective-input
    /// profile. The request dialect remains the sole wire projector.
    fn default_request(&self, paper_id: &str, pdf_bytes: Vec<u8>) -> ParseBackendRequest {
        ParseBackendRequest::new(paper_id, pdf_bytes)
    }

    /// Build a header-endpoint request with this backend's default effective
    /// header profile (S05/T01). The neutral default maps the rich preset
    /// onto the header axis; profile-configured backends override it.
    fn default_header_request(&self, paper_id: &str, pdf_bytes: Vec<u8>) -> ParseBackendRequest {
        ParseBackendRequest::header(paper_id, pdf_bytes, HeaderDialectV1::legacy_rich())
    }

    /// Build a references-endpoint request with this backend's default
    /// effective citations profile (S05/T01).
    fn default_references_request(
        &self,
        paper_id: &str,
        pdf_bytes: Vec<u8>,
    ) -> ParseBackendRequest {
        ParseBackendRequest::references(paper_id, pdf_bytes, ReferencesDialectV1::legacy_rich())
    }
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
