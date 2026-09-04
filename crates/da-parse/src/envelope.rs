//! Provider-neutral TEI envelope and exact backend/profile/result identity
//! (R105; S05/T01 adds the endpoint result identity component).
//!
//! The envelope is the transport result handed from a [`crate::backend::ParseBackend`]
//! to the TEI lifter: verbatim raw TEI bytes plus the exact backend/profile
//! identity that produced them. It deliberately carries no `PathBuf`, no
//! env/cache roots and no physical source paths — filesystem layout stays in
//! compatibility/infrastructure shells (`crate::http::GrobidParser::parse_pdf`).

use std::fmt;

use crate::profile::GrobidProfile;
use crate::transport::{
    FULLTEXT_REQUEST_SCHEMA_VERSION, FulltextDialectV1, HEADER_REQUEST_SCHEMA_VERSION,
    HeaderDialectV1, ParseEndpoint, REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1,
};

/// Deterministic result identity of a TEI payload (S05/T01): the logical
/// endpoint, the request schema version and the *effective* endpoint
/// dialect — exactly the information that distinguishes which closed
/// operation produced the payload and what was asked for, and deliberately
/// nothing about how it was transported.
///
/// Base URLs, timeouts, request/response limits, physical paths and cache
/// locations have no field here and can never enter the identity (same
/// exclusion rule as [`TeiIdentity`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TeiResultIdentity {
    endpoint: ParseEndpoint,
    schema_version: u8,
    dialect_identity: String,
}

impl TeiResultIdentity {
    /// Result identity for the fulltext endpoint. Legacy-equivalent
    /// dialects own no extended fulltext identity — they ride the D204
    /// profile fingerprint already carried by [`TeiIdentity`] — and are
    /// marked `legacy-equivalent` here.
    pub fn fulltext(dialect: FulltextDialectV1) -> Self {
        Self {
            endpoint: ParseEndpoint::Fulltext,
            schema_version: FULLTEXT_REQUEST_SCHEMA_VERSION,
            dialect_identity: dialect
                .canonical_identity()
                .unwrap_or_else(|| "legacy-equivalent".to_string()),
        }
    }

    /// Result identity for the header endpoint.
    pub fn header(dialect: HeaderDialectV1) -> Self {
        Self {
            endpoint: ParseEndpoint::Header,
            schema_version: HEADER_REQUEST_SCHEMA_VERSION,
            dialect_identity: dialect.canonical_identity(),
        }
    }

    /// Result identity for the references endpoint.
    pub fn references(dialect: ReferencesDialectV1) -> Self {
        Self {
            endpoint: ParseEndpoint::References,
            schema_version: REFERENCES_REQUEST_SCHEMA_VERSION,
            dialect_identity: dialect.canonical_identity(),
        }
    }

    /// Logical endpoint of the operation that produced the payload.
    pub const fn endpoint(&self) -> ParseEndpoint {
        self.endpoint
    }

    /// Request schema version of the effective endpoint dialect.
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }

    /// Versioned canonical identity of the effective endpoint dialect.
    pub fn dialect_identity(&self) -> &str {
        &self.dialect_identity
    }

    /// Deterministic canonical rendering, e.g.
    /// `endpoint=header;schema=v1;header=v1;consolidateHeader=1;rawAffiliations=0`.
    pub fn canonical(&self) -> String {
        format!(
            "endpoint={};schema=v{};{}",
            self.endpoint.as_str(),
            self.schema_version,
            self.dialect_identity
        )
    }
}

impl fmt::Display for TeiResultIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

/// Exact provenance identity of a TEI payload: which backend produced it,
/// under which effective-input profile fingerprint, and — since S05/T01 —
/// which closed operation produced it ([`TeiResultIdentity`]).
///
/// The fingerprint is the same stable string the production parse cache uses
/// ([`GrobidProfile::configuration_fingerprint`], e.g.
/// `profile=rich;consolidateHeader=1;...`), so envelope identity and cache
/// identity can never drift apart. Transport tuning (timeouts, pools) and
/// locations are deliberately absent: they must not influence identity —
/// the result component carries only logical endpoint, schema version and
/// effective endpoint dialect.
///
/// Pre-S05 constructors keep `result == None` so all pre-S05 identity pins
/// stay byte-identical; result stamping is purely additive
/// ([`TeiIdentity::with_result`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeiIdentity {
    backend_id: String,
    profile_fingerprint: String,
    result: Option<TeiResultIdentity>,
    /// Java GROBID `/api/version` string when probed (ADR-076 W0 / R121).
    /// Absent keeps historical identities byte-identical.
    backend_version: Option<String>,
}

impl TeiIdentity {
    /// Build an identity from a stable backend id and a profile fingerprint
    /// (pre-S05 shape: no endpoint result component, no backend version).
    pub fn new(backend_id: impl Into<String>, profile_fingerprint: impl Into<String>) -> Self {
        Self {
            backend_id: backend_id.into(),
            profile_fingerprint: profile_fingerprint.into(),
            result: None,
            backend_version: None,
        }
    }

    /// Identity for TEI produced by the Java GROBID HTTP backend.
    pub fn grobid_http(profile: GrobidProfile) -> Self {
        Self::new(
            crate::backend::GROBID_HTTP_BACKEND_ID,
            profile.configuration_fingerprint(),
        )
    }

    /// Identity for TEI served from already-available bytes (cache replay).
    pub fn cached_tei(profile: GrobidProfile) -> Self {
        Self::new(
            crate::backend::CACHED_TEI_BACKEND_ID,
            profile.configuration_fingerprint(),
        )
    }

    /// Stable provider-neutral backend identifier (e.g. `grobid-http`).
    pub fn backend_id(&self) -> &str {
        &self.backend_id
    }

    /// Exact effective-input profile fingerprint of the producing backend.
    pub fn profile_fingerprint(&self) -> &str {
        &self.profile_fingerprint
    }

    /// Attach an endpoint result identity (S05/T01): the same
    /// backend/profile provenance plus the logical endpoint, schema version
    /// and effective endpoint dialect of the result. Purely additive —
    /// existing backend/profile fields stay untouched.
    pub fn with_result(mut self, result: TeiResultIdentity) -> Self {
        self.result = Some(result);
        self
    }

    /// The endpoint result identity when one is stamped (`None` for
    /// pre-S05 identities).
    pub fn result(&self) -> Option<&TeiResultIdentity> {
        self.result.as_ref()
    }

    /// Stamp the Java GROBID version string (T02). Empty/whitespace is ignored
    /// so a failed probe cannot invent an identity component. Profile
    /// fingerprint is unchanged (D204 pins).
    pub fn with_backend_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        let trimmed = version.trim();
        if !trimmed.is_empty() {
            self.backend_version = Some(trimmed.to_string());
        }
        self
    }

    /// Probed backend version when stamped.
    pub fn backend_version(&self) -> Option<&str> {
        self.backend_version.as_deref()
    }
}

/// Provider-neutral transport result: verbatim raw TEI bytes plus exact
/// backend/profile identity.
///
/// Byte fidelity contract: whatever bytes the backend received come out
/// unchanged — no trimming, no re-encoding, no XML normalization. Empty and
/// malformed payloads are carried verbatim so downstream stages observe the
/// real transport outcome (typed 204/503/timeout semantics are slice S04
/// scope; this seam must not widen the transport dialect).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeiEnvelope {
    raw_tei: Vec<u8>,
    identity: TeiIdentity,
}

impl TeiEnvelope {
    /// Wrap raw TEI bytes with their exact backend/profile identity.
    pub fn new(raw_tei: impl Into<Vec<u8>>, identity: TeiIdentity) -> Self {
        Self {
            raw_tei: raw_tei.into(),
            identity,
        }
    }

    /// Verbatim raw TEI bytes as received from the backend.
    pub fn raw_tei(&self) -> &[u8] {
        &self.raw_tei
    }

    /// Consume the envelope, returning the verbatim raw TEI bytes.
    pub fn into_raw_tei(self) -> Vec<u8> {
        self.raw_tei
    }

    /// Exact backend/profile identity of this payload.
    pub fn identity(&self) -> &TeiIdentity {
        &self.identity
    }

    /// Consume the envelope into its raw bytes and identity parts.
    pub fn into_parts(self) -> (Vec<u8>, TeiIdentity) {
        (self.raw_tei, self.identity)
    }
}
