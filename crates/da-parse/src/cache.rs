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

use std::fmt;

use serde::de::{self, Deserializer};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};

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
