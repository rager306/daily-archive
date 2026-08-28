//! Provider-neutral TEI envelope and exact backend/profile identity (R105).
//!
//! The envelope is the transport result handed from a [`crate::backend::ParseBackend`]
//! to the TEI lifter: verbatim raw TEI bytes plus the exact backend/profile
//! identity that produced them. It deliberately carries no `PathBuf`, no
//! env/cache roots and no physical source paths — filesystem layout stays in
//! compatibility/infrastructure shells (`crate::http::GrobidParser::parse_pdf`).

use crate::profile::GrobidProfile;

/// Exact provenance identity of a TEI payload: which backend produced it and
/// under which effective-input profile fingerprint.
///
/// The fingerprint is the same stable string the production parse cache uses
/// ([`GrobidProfile::configuration_fingerprint`], e.g.
/// `profile=rich;consolidateHeader=1;...`), so envelope identity and cache
/// identity can never drift apart. Transport tuning (timeouts, pools) is
/// deliberately absent: it must not influence identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeiIdentity {
    backend_id: String,
    profile_fingerprint: String,
}

impl TeiIdentity {
    /// Build an identity from a stable backend id and a profile fingerprint.
    pub fn new(backend_id: impl Into<String>, profile_fingerprint: impl Into<String>) -> Self {
        Self {
            backend_id: backend_id.into(),
            profile_fingerprint: profile_fingerprint.into(),
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
