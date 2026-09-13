//! Provider-neutral parse cache vocabulary (M464-z84in2 S06/T01, R113/R116).
//!
//! This module freezes the typed CACHE CONTRACT AHEAD of the executor and
//! storage adapter:
//!
//! 1. [`CacheMode`] — the closed four-mode vocabulary (`off`, `read-only`,
//!    `write-through`, `refresh`) with the mode truth table as code:
//!    `off` never reads and never writes; `read-only` reads both stages and
//!    computes misses without publishing; `write-through` reads and
//!    publishes, committing raw before the projector; `refresh` bypasses
//!    hits and replaces both stages. Modes change COST, never the semantic
//!    output (pinned in `tests/batch_contract.rs` and, behaviorally, by the
//!    S06 executor suite).
//! 2. [`ParseCacheStage`] — the two independent cache stages with their
//!    stable schema-bound stage identities: raw verbatim TEI
//!    (`parse.raw_tei.v1`) and the `ParsedArticle` projection
//!    (`parse.project_article.v1`). Raw TEI must survive projector fixes
//!    independently; the projector key (T02) derives from the verified raw
//!    payload SHA plus the projector schema fingerprint, never from
//!    locations.
//! 3. [`CacheStageDisposition`] / [`ParseCacheDispositions`] — the closed
//!    per-stage, per-document dispositions every terminal batch outcome
//!    carries, derivable from the mode truth table alone.
//! 4. [`CacheIntegrityCode`] / [`ParseCacheFailure`] — the closed typed
//!    vocabulary for cache integrity failures (checksum/manifest/identity
//!    corruption): corruption is an ERROR, never a silent miss.
//!
//! No filesystem, no env, no URLs, no physical roots, no redb: actual
//! storage, payload envelopes and the rebuildable accelerator live behind
//! injected seams owned by the da-adapters / da-cli composition root (T03+
//! / T04+). This module is the provider-neutral vocabulary the executor and
//! the storage adapter must both speak, and it stays free of any retry
//! surface — availability semantics remain a backend concern.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::num::NonZeroUsize;

use async_trait::async_trait;
use da_ports::parser::ParsedArticle;
use serde::de::{self, Deserializer};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::backend::ParseBackendRequest;
use crate::envelope::{TeiEnvelope, TeiIdentity, TeiResultIdentity};

/// Stable schema version of the per-document cache dispositions.
pub const PARSE_CACHE_DISPOSITION_SCHEMA_VERSION: u32 = 1;

/// Stable schema version of the typed cache-integrity failure vocabulary.
pub const PARSE_CACHE_FAILURE_SCHEMA_VERSION: u32 = 1;

/// Stable stage identity of the raw-TEI cache stage (S06 contract pin).
pub const RAW_TEI_STAGE_ID: &str = "parse.raw_tei.v1";

/// Stable stage identity of the `ParsedArticle` projection cache stage
/// (S06 contract pin).
pub const PROJECT_ARTICLE_STAGE_ID: &str = "parse.project_article.v1";

// ---------------------------------------------------------------------------
// Closed cache mode vocabulary + truth table
// ---------------------------------------------------------------------------

/// Closed cache mode vocabulary of the bounded batch run.
///
/// The four sanctioned modes change only cache COST; the semantic output of
/// a document is mode-independent (byte-equal `ParsedArticle` across all
/// modes is an S06 executor obligation). Unknown wire variants are rejected
/// at deserialization — the closed-vocabulary evolution seam (R113).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheMode {
    /// `off`: stage caches are never read and never written. Every document
    /// is computed fresh from the backend; the stages are not consulted at
    /// all (dispositions record the uncached computation).
    Off,
    /// `read-only`: both stages are consulted; on a miss the value is
    /// computed fresh but never published. Cache stays byte-identical
    /// through the whole run.
    ReadOnly,
    /// `write-through`: both stages are consulted; computed values are
    /// published, with the raw TEI stage committed BEFORE the projector
    /// stage — a projector failure therefore leaves an honest, reusable raw
    /// state behind (T02 executor obligation).
    WriteThrough,
    /// `refresh`: cached hits are bypassed and both stages are REPLACED
    /// with freshly computed values. Only this mode ever overwrites an
    /// existing raw stage value.
    Refresh,
}

impl CacheMode {
    /// Whether a run in this mode consults (reads) the stage caches.
    pub const fn reads_stages(self) -> bool {
        matches!(self, Self::ReadOnly | Self::WriteThrough)
    }

    /// Whether a run in this mode publishes computed results to the stages.
    pub const fn publishes_results(self) -> bool {
        matches!(self, Self::WriteThrough | Self::Refresh)
    }

    /// Whether a run in this mode bypasses hits even when a stage value
    /// exists (refresh semantics: hits are ignored, stages are replaced).
    pub const fn bypasses_hits(self) -> bool {
        matches!(self, Self::Refresh)
    }

    /// Stable kebab-case wire name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ReadOnly => "read-only",
            Self::WriteThrough => "write-through",
            Self::Refresh => "refresh",
        }
    }

    /// The truth-table disposition one stage must record under this mode.
    ///
    /// `cache_hit` states whether the stage served a stored value; it is
    /// consulted only when the mode reads stages without bypassing hits
    /// (read-only, write-through). Under `off` no read happens at all
    /// (`ComputedUncached` regardless of the flag); under `refresh` the hit
    /// would have been bypassed and the stage replaced
    /// (`ComputedPublished`).
    pub const fn stage_disposition_for(self, cache_hit: bool) -> CacheStageDisposition {
        if self.bypasses_hits() {
            return CacheStageDisposition::ComputedPublished;
        }
        if !self.reads_stages() {
            return CacheStageDisposition::ComputedUncached;
        }
        if cache_hit {
            return CacheStageDisposition::Hit;
        }
        if self.publishes_results() {
            CacheStageDisposition::ComputedPublished
        } else {
            CacheStageDisposition::ComputedUnpublished
        }
    }
}

impl fmt::Display for CacheMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Cache stages
// ---------------------------------------------------------------------------

/// Closed two-stage cache vocabulary of the scholarly parse plane: the raw
/// verbatim TEI payload stage and the `ParsedArticle` projection stage.
/// The stages rotate INDEPENDENTLY — a projector schema fix must not
/// invalidate raw TEI (S06/T02 raw-envelope payload identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseCacheStage {
    /// Raw verbatim TEI payload stage (`parse.raw_tei.v1`).
    RawTei,
    /// `ParsedArticle` projection stage (`parse.project_article.v1`).
    ProjectArticle,
}

impl ParseCacheStage {
    /// Stable, versioned stage identity string — the namespace pin the
    /// storage adapter (T03) and the ledger (T05) must both carry.
    pub const fn stage_id(self) -> &'static str {
        match self {
            Self::RawTei => RAW_TEI_STAGE_ID,
            Self::ProjectArticle => PROJECT_ARTICLE_STAGE_ID,
        }
    }
}

impl fmt::Display for ParseCacheStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.stage_id())
    }
}

// ---------------------------------------------------------------------------
// Per-stage, per-document dispositions
// ---------------------------------------------------------------------------

/// Closed per-stage disposition of one document's terminal state.
///
/// Exactly one variant applies per stage per document; the mode truth
/// table ([`CacheMode::stage_disposition_for`]) derives it from the mode
/// and whether the stage served a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStageDisposition {
    /// Served from the stage cache. Only possible in a mode that reads
    /// stages without bypassing hits (read-only, write-through).
    Hit,
    /// Computed fresh and published to the stage: a write-through publish
    /// after a miss, or a refresh replacement of a bypassed hit.
    ComputedPublished,
    /// Computed fresh after a miss; nothing published (read-only mode
    /// never writes).
    ComputedUnpublished,
    /// Computed fresh without consulting the stage at all; nothing
    /// published (cache off).
    ComputedUncached,
}

impl CacheStageDisposition {
    /// Whether this disposition leaves the stage content durably recoverable.
    ///
    /// `Hit` content already lives in the stage and `ComputedPublished` content
    /// was just written there, so either can be read again. The two computed
    /// dispositions produced content that exists only in memory for this run,
    /// and an interrupted run writes no terminal report — so nothing survives
    /// to be resumed from.
    ///
    /// This is the signal a durable resume cursor must consult before claiming
    /// an ordinal complete: advancing over `ComputedUncached` or
    /// `ComputedUnpublished` would let the next run skip a document whose
    /// result exists nowhere.
    pub const fn is_durably_recoverable(self) -> bool {
        matches!(self, Self::Hit | Self::ComputedPublished)
    }
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::ComputedPublished => "computed_published",
            Self::ComputedUnpublished => "computed_unpublished",
            Self::ComputedUncached => "computed_uncached",
        }
    }
}

impl fmt::Display for CacheStageDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed per-document dispositions of both cache stages: raw TEI and
/// the `ParsedArticle` projection. Part of every terminal
/// [`crate::batch::ParseDocumentOutcome`]; schema-versioned with unknown
/// versions and fields rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCacheDispositions {
    schema_version: u32,
    raw: CacheStageDisposition,
    projector: CacheStageDisposition,
}

impl ParseCacheDispositions {
    /// Stamp both stage dispositions with the current schema version.
    pub const fn new(raw: CacheStageDisposition, projector: CacheStageDisposition) -> Self {
        Self {
            schema_version: PARSE_CACHE_DISPOSITION_SCHEMA_VERSION,
            raw,
            projector,
        }
    }

    /// Stable schema version of this instance.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Whether a later run can recover this ordinal's expensive content.
    ///
    /// The criterion is the raw stage: it is what required the Java call, and a
    /// projector can be rebuilt from a stored raw TEI without the backend
    /// (`raw_hit_reprojects_after_projector_schema_rotation_without_backend`
    /// pins that). Requiring the projector too would freeze the cursor on a
    /// perfectly resumable run, which is why this deliberately looks at `raw`
    /// alone rather than both stages.
    pub const fn is_durably_recoverable(&self) -> bool {
        self.raw.is_durably_recoverable()
    }

    /// Raw-TEI stage disposition.
    pub const fn raw(&self) -> CacheStageDisposition {
        self.raw
    }

    /// Projection stage disposition.
    pub const fn projector(&self) -> CacheStageDisposition {
        self.projector
    }
}

impl Serialize for ParseCacheDispositions {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseCacheDispositions", 3)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("raw", &self.raw)?;
        state.serialize_field("projector", &self.projector)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseCacheDispositions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            raw: CacheStageDisposition,
            projector: CacheStageDisposition,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_CACHE_DISPOSITION_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported cache disposition schema version {} (supported: {PARSE_CACHE_DISPOSITION_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            raw: repr.raw,
            projector: repr.projector,
        })
    }
}

// ---------------------------------------------------------------------------
// Closed typed cache-integrity failure vocabulary
// ---------------------------------------------------------------------------

/// Closed classification of cache-integrity failures. Every variant is an
/// ERROR surfaced as a per-document failure — corruption is never a silent
/// miss (S06 slice verification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheIntegrityCode {
    /// Stored payload failed its recorded checksum.
    ChecksumMismatch,
    /// Manifest uninterpretable or schema-foreign (corruption, not a miss:
    /// a MISSING manifest is a normal miss instead).
    ManifestCorrupt,
    /// Stored payload identity does not match the manifest identity.
    IdentityMismatch,
    /// Manifest present and checksummed but the payload bytes cannot be
    /// decoded into the stage's schema.
    PayloadUnreadable,
    /// Stage publication failed after computation (the honest write
    /// failure; the outcome still records the computed terminal state).
    StageWriteFailed,
}

impl CacheIntegrityCode {
    /// Stable snake_case name (identical to the serde wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::ManifestCorrupt => "manifest_corrupt",
            Self::IdentityMismatch => "identity_mismatch",
            Self::PayloadUnreadable => "payload_unreadable",
            Self::StageWriteFailed => "stage_write_failed",
        }
    }
}

impl fmt::Display for CacheIntegrityCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Closed, serializable cache-integrity failure with a stable schema
/// version: one stage, one code, no payload, no paths, no messages.
/// Per-document by design — a cache integrity failure on one document must
/// not fail the whole batch (S06 contract: batch-wide `Err` exists only at
/// admission).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCacheFailure {
    schema_version: u32,
    stage: ParseCacheStage,
    code: CacheIntegrityCode,
}

impl ParseCacheFailure {
    /// Stamp `stage` + `code` with the current schema version.
    pub const fn new(stage: ParseCacheStage, code: CacheIntegrityCode) -> Self {
        Self {
            schema_version: PARSE_CACHE_FAILURE_SCHEMA_VERSION,
            stage,
            code,
        }
    }

    /// Stable schema version of this failure instance.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// The cache stage that failed integrity validation.
    pub const fn stage(&self) -> ParseCacheStage {
        self.stage
    }

    /// Closed integrity classification.
    pub const fn code(&self) -> CacheIntegrityCode {
        self.code
    }
}

impl fmt::Display for ParseCacheFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse cache failure: stage={} code={}",
            self.stage.stage_id(),
            self.code
        )
    }
}

impl Serialize for ParseCacheFailure {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseCacheFailure", 3)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("stage", &self.stage)?;
        state.serialize_field("code", &self.code)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseCacheFailure {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            stage: ParseCacheStage,
            code: CacheIntegrityCode,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_CACHE_FAILURE_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported cache failure schema version {} (supported: {PARSE_CACHE_FAILURE_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            stage: repr.stage,
            code: repr.code,
        })
    }
}

// ---------------------------------------------------------------------------
// S06/T02: two-stage artifact cache seam + versioned stage payload identity
// ---------------------------------------------------------------------------

/// Stable schema version of the versioned raw-TEI stage payload envelope.
pub const RAW_TEI_PAYLOAD_SCHEMA_VERSION: u32 = 1;

/// Stable schema version of the versioned `ParsedArticle` projection
/// payload.
pub const PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION: u32 = 1;

/// Stable schema fingerprint of the projection stage payload: the
/// `ParsedArticle` wire schema the projector emits. A projector schema fix
/// rotates THIS value (invalidating projection-stage entries only) while
/// raw TEI survives independently — the two stages rotate independently.
/// Stage KEYS derive from this fingerprint, so a rotation naturally orphans
/// old projection entries without touching the raw stage.
///
/// Rotation history: `serde-v3-unique-navigation` → `serde-v4-p0-tei-lift`
/// (M464-z84in2 S07/T05) — the additive P0 lift families (declared arXiv
/// identity + typed missingness, header authors, non-table figures with
/// graphic coordinates, figure/table visual callouts) joined the wire, so
/// the old fingerprint would have mislabeled the projection payload. Raw-TEI
/// stage keys, D204 profile fingerprints and endpoint identities are
/// unchanged.
pub const PROJECT_ARTICLE_SCHEMA_FINGERPRINT: &str = "ParsedArticle@serde-v4-p0-tei-lift";

/// Domain separator binding raw-TEI stage keys to this contract generation.
const RAW_TEI_STAGE_KEY_DOMAIN: &[u8] = b"da-parse/raw-tei-stage-key/v1";

/// Domain separator binding projection stage keys to this contract
/// generation.
const PROJECT_ARTICLE_STAGE_KEY_DOMAIN: &[u8] = b"da-parse/project-article-stage-key/v1";

/// SHA-256 of `bytes` as 64 lowercase hex chars (stage payload and key
/// component identity).
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// One length-prefixed, tagged key component (unambiguous framing).
fn write_key_segment(hasher: &mut Sha256, tag: &[u8], bytes: &[u8]) {
    hasher.update(tag);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Content-addressed raw-TEI stage key: the closed endpoint, the complete
/// effective dialect wire form and the exact source bytes. Two requests
/// share a raw entry exactly when their raw TEI can be byte-identical.
/// Logical paper ids are deliberately EXCLUDED — raw TEI depends on the
/// request content, not on the logical id (provenance rides the payload
/// identity instead). Format: `parse.raw_tei.v1:<64 hex>`.
pub fn raw_tei_stage_key(request: &ParseBackendRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(RAW_TEI_STAGE_KEY_DOMAIN);
    write_key_segment(
        &mut hasher,
        b"endpoint",
        request.endpoint().as_str().as_bytes(),
    );
    write_key_segment(
        &mut hasher,
        b"dialect",
        crate::batch::endpoint_dialect_wire(request).as_bytes(),
    );
    write_key_segment(&mut hasher, b"pdf_bytes", request.pdf_bytes());
    format!("{}:{}", RAW_TEI_STAGE_ID, hex::encode(hasher.finalize()))
}

/// Same as [`raw_tei_stage_key`] plus an optional backend version segment.
/// `None` is byte-identical to [`raw_tei_stage_key`] so unstamped parses keep
/// historical keys. A probed Java version is a content-key input (R121).
pub fn raw_tei_stage_key_with_backend_version(
    request: &ParseBackendRequest,
    backend_version: Option<&str>,
) -> String {
    match backend_version.map(str::trim).filter(|v| !v.is_empty()) {
        None => raw_tei_stage_key(request),
        Some(version) => {
            let mut hasher = Sha256::new();
            hasher.update(RAW_TEI_STAGE_KEY_DOMAIN);
            write_key_segment(
                &mut hasher,
                b"endpoint",
                request.endpoint().as_str().as_bytes(),
            );
            write_key_segment(
                &mut hasher,
                b"dialect",
                crate::batch::endpoint_dialect_wire(request).as_bytes(),
            );
            write_key_segment(&mut hasher, b"pdf_bytes", request.pdf_bytes());
            write_key_segment(&mut hasher, b"backend_version", version.as_bytes());
            format!("{}:{}", RAW_TEI_STAGE_ID, hex::encode(hasher.finalize()))
        }
    }
}

/// Projection stage key: the verified raw payload SHA (post-checksum), the
/// source bytes SHA, the logical paper id and the projector schema
/// fingerprint — every input the projection output depends on, and nothing
/// else. A projector schema fix changes the key and orphans old entries
/// while the raw stage survives untouched. Format:
/// `parse.project_article.v1:<64 hex>`.
///
/// This is the explicit historical (versionless) entry point: it is
/// byte-identical to [`project_article_stage_key_with_backend_version`] with
/// `None`, so pre-version projection entries keep their exact key.
pub fn project_article_stage_key(
    paper_id: &str,
    pdf_sha256: &str,
    raw_payload_sha256: &str,
    projector_schema_fingerprint: &str,
) -> String {
    project_article_stage_key_for_version(
        paper_id,
        pdf_sha256,
        raw_payload_sha256,
        projector_schema_fingerprint,
        None,
    )
}

/// Same as [`project_article_stage_key`] plus an optional backend version
/// segment. `None` is byte-identical to [`project_article_stage_key`], so
/// versionless runs keep historical projection keys. A probed service
/// version is a content-key input (R121): two backend versions that emit
/// byte-equal raw TEI still never share a projection entry, so a projection
/// is never presented under a foreign producer identity.
pub fn project_article_stage_key_with_backend_version(
    paper_id: &str,
    pdf_sha256: &str,
    raw_payload_sha256: &str,
    projector_schema_fingerprint: &str,
    backend_version: Option<&str>,
) -> String {
    project_article_stage_key_for_version(
        paper_id,
        pdf_sha256,
        raw_payload_sha256,
        projector_schema_fingerprint,
        backend_version.map(str::trim).filter(|v| !v.is_empty()),
    )
}

fn project_article_stage_key_for_version(
    paper_id: &str,
    pdf_sha256: &str,
    raw_payload_sha256: &str,
    projector_schema_fingerprint: &str,
    backend_version: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_ARTICLE_STAGE_KEY_DOMAIN);
    write_key_segment(&mut hasher, b"paper_id", paper_id.as_bytes());
    write_key_segment(&mut hasher, b"pdf_sha256", pdf_sha256.as_bytes());
    write_key_segment(
        &mut hasher,
        b"raw_payload_sha256",
        raw_payload_sha256.as_bytes(),
    );
    write_key_segment(
        &mut hasher,
        b"schema_fingerprint",
        projector_schema_fingerprint.as_bytes(),
    );
    if let Some(version) = backend_version {
        write_key_segment(&mut hasher, b"backend_version", version.as_bytes());
    }
    format!(
        "{}:{}",
        PROJECT_ARTICLE_STAGE_ID,
        hex::encode(hasher.finalize())
    )
}

/// One stage-cache read: the stored payload bytes plus whether the storage
/// adapter served them from a LEGACY read-only identity (T03+). Legacy
/// hits never create canonical records — the executor only counts them
/// (`ParseBatchRunSummary::raw_legacy_hits`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawStageRead {
    payload: Vec<u8>,
    legacy: bool,
}

impl RawStageRead {
    /// A read served from a canonical (current-generation) entry.
    pub fn canonical(payload: impl Into<Vec<u8>>) -> Self {
        Self {
            payload: payload.into(),
            legacy: false,
        }
    }

    /// A read served from a LEGACY read-only identity (T03+ semantics: the
    /// read itself never promotes the payload into a canonical record).
    pub fn from_legacy(payload: impl Into<Vec<u8>>) -> Self {
        Self {
            payload: payload.into(),
            legacy: true,
        }
    }

    /// The stored payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Whether this read came from a legacy identity.
    pub const fn is_legacy(&self) -> bool {
        self.legacy
    }
}

/// Provider-neutral injected artifact cache seam (S06/T02): two independent
/// stage namespaces over byte-blob payloads, no filesystem in this crate.
/// Contract:
///
/// - a MISSING key is a normal miss (`Ok(None)`);
/// - a PRESENT key whose stored bytes fail their own integrity checks is a
///   typed `ParseCacheFailure` — corruption is an ERROR, never a silent
///   miss;
/// - writes are durable before `Ok(())` returns (the write-through commit
///   order — raw before projector — is built on this).
#[async_trait]
pub trait ParseArtifactCache: Send + Sync {
    /// Read the raw-TEI stage entry for `key`.
    async fn read_raw(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure>;

    /// Publish the raw-TEI stage payload for `key` (durable before `Ok`).
    async fn write_raw(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure>;

    /// Read the projection stage entry for `key`.
    async fn read_projected(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure>;

    /// Publish the projection stage payload for `key` (durable before `Ok`).
    async fn write_projected(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure>;
}

/// Versioned raw-TEI stage payload: the verbatim TEI bytes (hex-encoded for
/// byte-exact round-trips) plus the exact backend/profile provenance and
/// the stamped endpoint result identity. THIS is the payload identity the
/// projector key derives from — `payload_sha256` is the binding pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTeiEnvelopePayload {
    schema_version: u32,
    stage_id: String,
    backend_id: String,
    profile_fingerprint: String,
    result_identity: Option<String>,
    /// Java GROBID version when the envelope carried one (ADR-076 T02).
    /// Omitted on the wire when absent so historical payloads still decode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backend_version: Option<String>,
    payload_sha256: String,
    tei_xml_hex: String,
}

impl RawTeiEnvelopePayload {
    /// Build the versioned raw payload over a fetched envelope, stamping
    /// the exact endpoint result identity of the producing request (the
    /// executor stamps it — not every backend carries a result component).
    pub fn build(envelope: &TeiEnvelope, stamped_result: &TeiResultIdentity) -> Self {
        Self {
            schema_version: RAW_TEI_PAYLOAD_SCHEMA_VERSION,
            stage_id: RAW_TEI_STAGE_ID.to_string(),
            backend_id: envelope.identity().backend_id().to_string(),
            profile_fingerprint: envelope.identity().profile_fingerprint().to_string(),
            result_identity: Some(stamped_result.canonical()),
            backend_version: envelope.identity().backend_version().map(str::to_string),
            payload_sha256: sha256_hex(envelope.raw_tei()),
            tei_xml_hex: hex::encode(envelope.raw_tei()),
        }
    }

    /// Deterministic wire bytes handed to the cache seam.
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("raw payload serialization is total")
    }

    /// Decode and verify one stored raw payload against the exact request
    /// identity it must serve. Returns the envelope rebuilt over the
    /// VERIFIED verbatim TEI bytes (stored backend/profile provenance; the
    /// result component is verified separately and never reconstructed)
    /// plus the verified payload SHA — the projector key's dependency pin.
    ///
    /// Every mismatch is a typed integrity ERROR, never a miss:
    /// uninterpretable wire/schema → `ManifestCorrupt`; hex payload that
    /// cannot decode → `PayloadUnreadable`; byte drift → `ChecksumMismatch`;
    /// a payload produced for a different endpoint result →
    /// `IdentityMismatch`.
    pub fn decode_and_verify(
        bytes: &[u8],
        expected_result: &TeiResultIdentity,
    ) -> Result<(TeiEnvelope, String), ParseCacheFailure> {
        let stage = ParseCacheStage::RawTei;
        let payload: Self = serde_json::from_slice(bytes)
            .map_err(|_| ParseCacheFailure::new(stage, CacheIntegrityCode::ManifestCorrupt))?;
        if payload.schema_version != RAW_TEI_PAYLOAD_SCHEMA_VERSION
            || payload.stage_id != RAW_TEI_STAGE_ID
        {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::ManifestCorrupt,
            ));
        }
        let tei_bytes = hex::decode(&payload.tei_xml_hex)
            .map_err(|_| ParseCacheFailure::new(stage, CacheIntegrityCode::PayloadUnreadable))?;
        if payload.payload_sha256 != sha256_hex(&tei_bytes) {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::ChecksumMismatch,
            ));
        }
        match &payload.result_identity {
            Some(stored) if *stored == expected_result.canonical() => {}
            _ => {
                return Err(ParseCacheFailure::new(
                    stage,
                    CacheIntegrityCode::IdentityMismatch,
                ));
            }
        }
        let mut identity = TeiIdentity::new(payload.backend_id, payload.profile_fingerprint);
        if let Some(version) = payload.backend_version {
            identity = identity.with_backend_version(version);
        }
        Ok((
            TeiEnvelope::new(tei_bytes, identity),
            payload.payload_sha256.clone(),
        ))
    }

    /// The bound raw payload SHA-256 (hex) — the projector key's dependency
    /// pin, exposed for diagnostics and contract tests.
    pub fn payload_sha256(&self) -> &str {
        &self.payload_sha256
    }
}

/// Versioned projection stage payload: the projected `ParsedArticle` bound
/// to its exact dependencies — the verified raw payload SHA, the source
/// bytes SHA, the logical paper id and the projector schema fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectArticlePayload {
    schema_version: u32,
    stage_id: String,
    projector_schema_fingerprint: String,
    dependency_raw_payload_sha256: String,
    dependency_pdf_sha256: String,
    paper_id: String,
    article: ParsedArticle,
}

impl ProjectArticlePayload {
    /// Build the versioned projection payload over the projected article
    /// and its verified dependencies.
    pub fn build(
        article: &ParsedArticle,
        projector_schema_fingerprint: &str,
        raw_payload_sha256: &str,
        paper_id: &str,
        pdf_sha256: &str,
    ) -> Self {
        Self {
            schema_version: PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION,
            stage_id: PROJECT_ARTICLE_STAGE_ID.to_string(),
            projector_schema_fingerprint: projector_schema_fingerprint.to_string(),
            dependency_raw_payload_sha256: raw_payload_sha256.to_string(),
            dependency_pdf_sha256: pdf_sha256.to_string(),
            paper_id: paper_id.to_string(),
            article: article.clone(),
        }
    }

    /// Deterministic wire bytes handed to the cache seam.
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("projection payload serialization is total")
    }

    /// Decode and verify one stored projection payload against the exact
    /// dependencies it must have been produced from. Returns the projected
    /// article (mode-independent semantics — byte-equal across all modes).
    ///
    /// Mapping: uninterpretable wrapper / foreign schema or fingerprint →
    /// `ManifestCorrupt`; an interpretable wrapper whose article cannot
    /// deserialize → `PayloadUnreadable`; a payload bound to different
    /// dependencies (raw payload SHA, source SHA, paper id) →
    /// `IdentityMismatch`.
    pub fn decode_and_verify(
        bytes: &[u8],
        projector_schema_fingerprint: &str,
        raw_payload_sha256: &str,
        paper_id: &str,
        pdf_sha256: &str,
    ) -> Result<ParsedArticle, ParseCacheFailure> {
        let stage = ParseCacheStage::ProjectArticle;
        let payload: Self = serde_json::from_slice(bytes).map_err(|_| {
            let interpretably_wrapped = serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .is_some_and(|value| {
                    value
                        .get("schema_version")
                        .and_then(serde_json::Value::as_u64)
                        == Some(PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION as u64)
                        && value.get("stage_id").and_then(serde_json::Value::as_str)
                            == Some(PROJECT_ARTICLE_STAGE_ID)
                        && value
                            .get("projector_schema_fingerprint")
                            .and_then(serde_json::Value::as_str)
                            == Some(projector_schema_fingerprint)
                });
            if interpretably_wrapped {
                // Interpretable wrapper, undecodable article payload.
                return ParseCacheFailure::new(stage, CacheIntegrityCode::PayloadUnreadable);
            }
            ParseCacheFailure::new(stage, CacheIntegrityCode::ManifestCorrupt)
        })?;
        if payload.schema_version != PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION
            || payload.stage_id != PROJECT_ARTICLE_STAGE_ID
            || payload.projector_schema_fingerprint != projector_schema_fingerprint
        {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::ManifestCorrupt,
            ));
        }
        if payload.dependency_raw_payload_sha256 != raw_payload_sha256
            || payload.dependency_pdf_sha256 != pdf_sha256
            || payload.paper_id != paper_id
        {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::IdentityMismatch,
            ));
        }
        Ok(payload.article)
    }
}

// ---------------------------------------------------------------------------
// S06/T02: the two-tier composite cache — fast memory tier + durable backing
// ---------------------------------------------------------------------------

/// Default combined memory-tier entry capacity of [`TwoStageParseCache`].
/// Deterministic FIFO eviction applies beyond the cap; the backing tier
/// stays the durable authority, so eviction only costs a backing re-read,
/// never a payload.
pub const MEMORY_TIER_DEFAULT_CAPACITY: usize = 4096;

/// The bounded fast memory tier (S06/T02). Entries are exact
/// [`RawStageRead`] values so canonical vs LEGACY provenance survives
/// promotion — the executor's legacy-hit accounting stays honest on later
/// memory hits.
#[derive(Debug)]
struct MemoryTier {
    entries: HashMap<(ParseCacheStage, String), RawStageRead>,
    order: VecDeque<(ParseCacheStage, String)>,
    capacity: usize,
}

impl MemoryTier {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.get(),
        }
    }

    fn get(&self, stage: ParseCacheStage, key: &str) -> Option<RawStageRead> {
        self.entries.get(&(stage, key.to_string())).cloned()
    }

    /// Insert (or refresh) one entry; FIFO-evicts the oldest inserted
    /// entry beyond `capacity`. Refreshing an existing key keeps its FIFO
    /// position stable.
    fn insert(&mut self, stage: ParseCacheStage, key: &str, read: RawStageRead) {
        let tier_key = (stage, key.to_string());
        if let Some(slot) = self.entries.get_mut(&tier_key) {
            // Refresh in place: the FIFO position stays stable.
            *slot = read;
            return;
        }
        // New key: FIFO-evict the oldest inserted entries beyond capacity
        // (deterministic, and always safe because the backing tier still
        // holds the payload).
        while self.entries.len() >= self.capacity {
            match self.order.pop_front() {
                Some(evicted) => {
                    self.entries.remove(&evicted);
                }
                None => break,
            }
        }
        self.entries.insert(tier_key.clone(), read);
        self.order.push_back(tier_key);
    }

    fn invalidate(&mut self, stage: ParseCacheStage, key: &str) -> bool {
        let tier_key = (stage, key.to_string());
        if self.entries.remove(&tier_key).is_some() {
            self.order.retain(|entry| *entry != tier_key);
            true
        } else {
            false
        }
    }

    fn clear(&mut self) -> usize {
        let dropped = self.entries.len();
        self.entries.clear();
        self.order.clear();
        dropped
    }
}

/// The two-tier [`ParseArtifactCache`] composite (S06/T02): a bounded
/// in-process memory tier in front of any durable backing tier (production:
/// the T03 da-adapters artifact cache over `FilePipelineCache`).
///
/// One unified key space and one hit/miss/invalidation semantic across both
/// tiers:
///
/// - a MEMORY hit never touches the backing tier;
/// - a backing hit is PROMOTED into the memory tier verbatim (bytes plus
///   canonical/legacy provenance);
/// - a backing miss stays a plain miss — no negative caching, so a later
///   publication is immediately visible;
/// - a backing READ failure is a typed `ParseCacheFailure` — never a
///   silent miss, never promoted, never cached;
/// - a write commits to the BACKING tier first (durable before `Ok`) and
///   only then populates the memory tier; a backing write failure leaves
///   the memory tier untouched, so the two tiers can never diverge;
/// - invalidation (`invalidate_memory` / `clear_memory`) demotes memory
///   entries; the backing tier remains the durable authority and the next
///   read re-consults it.
///
/// The memory tier is bounded by capacity: beyond the cap the oldest
/// inserted entry is evicted FIFO — deterministic, and always safe because
/// the backing tier still holds the payload. Configurationally incompatible
/// parse profiles cannot collide: stage KEYS are this module's
/// content-addressed identity keys (endpoint/dialect/source bytes for raw;
/// paper id, source SHA, raw payload SHA and schema fingerprint for the
/// projection), so different profiles simply occupy different entries.
/// This composite adds NO key material of its own — one key, one value,
/// both tiers.
pub struct TwoStageParseCache<B: ParseArtifactCache> {
    backing: B,
    memory: RwLock<MemoryTier>,
}

impl<B: ParseArtifactCache> TwoStageParseCache<B> {
    /// Compose the two-tier cache over `backing` with the default memory
    /// capacity ([`MEMORY_TIER_DEFAULT_CAPACITY`]).
    pub fn new(backing: B) -> Self {
        Self::with_memory_capacity(
            backing,
            NonZeroUsize::new(MEMORY_TIER_DEFAULT_CAPACITY)
                .expect("default memory tier capacity is nonzero"),
        )
    }

    /// Compose the two-tier cache over `backing` with an explicit bounded
    /// memory capacity.
    pub fn with_memory_capacity(backing: B, capacity: NonZeroUsize) -> Self {
        Self {
            backing,
            memory: RwLock::new(MemoryTier::new(capacity)),
        }
    }

    /// The durable backing tier (direct seeding and inspection for tests
    /// and diagnostics).
    pub fn backing(&self) -> &B {
        &self.backing
    }

    /// Demote one memory-tier entry; returns whether it was present. The
    /// backing tier keeps the payload — the next read re-consults it.
    pub async fn invalidate_memory(&self, stage: ParseCacheStage, key: &str) -> bool {
        self.memory.write().await.invalidate(stage, key)
    }

    /// Demote every memory-tier entry; returns how many were dropped.
    pub async fn clear_memory(&self) -> usize {
        self.memory.write().await.clear()
    }

    /// Current memory-tier entry count across both stages (diagnostics).
    pub async fn memory_entries(&self) -> usize {
        self.memory.read().await.entries.len()
    }

    async fn read_stage(
        &self,
        stage: ParseCacheStage,
        key: &str,
    ) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        let hit = self.memory.read().await.get(stage, key);
        if let Some(hit) = hit {
            return Ok(Some(hit));
        }
        match self.read_backing(stage, key).await? {
            Some(read) => {
                self.memory.write().await.insert(stage, key, read.clone());
                Ok(Some(read))
            }
            None => Ok(None),
        }
    }

    async fn read_backing(
        &self,
        stage: ParseCacheStage,
        key: &str,
    ) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        match stage {
            ParseCacheStage::RawTei => self.backing.read_raw(key).await,
            ParseCacheStage::ProjectArticle => self.backing.read_projected(key).await,
        }
    }

    async fn write_stage(
        &self,
        stage: ParseCacheStage,
        key: &str,
        payload: &[u8],
    ) -> Result<(), ParseCacheFailure> {
        match stage {
            ParseCacheStage::RawTei => self.backing.write_raw(key, payload).await?,
            ParseCacheStage::ProjectArticle => self.backing.write_projected(key, payload).await?,
        }
        self.memory
            .write()
            .await
            .insert(stage, key, RawStageRead::canonical(payload.to_vec()));
        Ok(())
    }
}

#[async_trait]
impl<B: ParseArtifactCache> ParseArtifactCache for TwoStageParseCache<B> {
    async fn read_raw(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.read_stage(ParseCacheStage::RawTei, key).await
    }

    async fn write_raw(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.write_stage(ParseCacheStage::RawTei, key, payload)
            .await
    }

    async fn read_projected(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.read_stage(ParseCacheStage::ProjectArticle, key).await
    }

    async fn write_projected(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.write_stage(ParseCacheStage::ProjectArticle, key, payload)
            .await
    }
}
