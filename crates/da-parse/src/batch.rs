//! Provider-neutral bounded batch contract (M464-z84in2 S06/T01, R113/R116).
//!
//! This module freezes the TYPED BATCH CONTRACT AHEAD of the executor:
//!
//! 1. [`ParseBatchLimits`] — NonZero worker / queue / document bounds. This
//!    is deliberately the ONLY batch knob set: workers, queue capacity,
//!    in-flight degree and document count are execution tuning and are
//!    excluded from content identity (together with timeouts, physical
//!    roots and redb settings, which have no field here at all).
//! 2. [`ParseBatchItem`] — an ordered item over
//!    [`crate::backend::ParseBackendRequest`] carrying its stable admission
//!    ordinal; the request remains the sole dialect/identity carrier.
//! 3. [`ordered_input_fingerprint`] — one deterministic SHA-256 over the
//!    ordered request content (paper id, closed endpoint, the complete
//!    effective dialect wire form, source bytes). Cache modes, transport
//!    tuning and locations never enter it.
//! 4. [`BatchCursor`] — the schema-bound resume cursor bound to exactly one
//!    ordered input fingerprint; a foreign fingerprint, an out-of-range
//!    ordinal or a foreign schema version fails closed before any document
//!    work (cursor mismatch is an ERROR, not a silent miss).
//! 5. [`ParseBatch`] — the admitted batch. Batch-wide `Err` exists ONLY at
//!    admission (invalid limits/cursor/config); after admission every
//!    backend or cache failure is a per-document
//!    [`ParseDocumentFailure`] that preserves the closed
//!    [`ParseBackendFailure`] metadata verbatim (no legacy `ParserError`
//!    collapse). Lift/projection are total functions in the current
//!    composition, so no projector failure variant exists yet — the
//!    vocabulary grows only when a total seam stops being total.
//! 6. [`ParseDocumentOutcome`] — exactly one typed terminal outcome per
//!    admitted ordinal, carrying the run identity (ordinal, paper id, cache
//!    mode), both stage dispositions and the success/failure result.
//!
//! No retry surface, no graph/import surface, no filesystem, no env, no
//! URLs: this is the provider-neutral contract layer. Composition (backend
//! + artifact cache + rebuildable accelerator) belongs to the da-adapters /
//!
//! da-cli root (R116); `ParserPort` is NOT extended by this contract.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use da_ports::parser::ParsedArticle;
use serde::de::{self, Deserializer};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Semaphore, mpsc};
use tokio::task::JoinHandle;

use crate::backend::{ParseBackend, ParseBackendRequest};
use crate::cache::{
    CacheMode, CacheStageDisposition, PROJECT_ARTICLE_SCHEMA_FINGERPRINT, ParseArtifactCache,
    ParseCacheDispositions, ParseCacheFailure, ProjectArticlePayload, RawTeiEnvelopePayload,
    project_article_stage_key_with_backend_version, raw_tei_stage_key_with_backend_version,
    sha256_hex,
};
use crate::envelope::{TeiEnvelope, TeiResultIdentity};
use crate::lift::TeiLifter;
use crate::parser::lift_and_project_envelope;
use crate::transport::{ParseBackendFailure, ParseEndpoint};

/// Stable schema version of the batch limits / outcome vocabulary.
pub const PARSE_BATCH_SCHEMA_VERSION: u32 = 1;

/// Stable schema version of the resume cursor vocabulary.
pub const PARSE_BATCH_CURSOR_SCHEMA_VERSION: u32 = 1;

/// Domain separator binding the fingerprint to this exact contract
/// generation (rotation forces every persisted cursor to fail closed).
const FINGERPRINT_DOMAIN: &[u8] = b"da-parse/parse-batch-input/v1";

// ---------------------------------------------------------------------------
// Bounded execution limits (tuning, never content identity)
// ---------------------------------------------------------------------------

/// NonZero bounded-execution limits of one batch run.
///
/// Every bound is nonzero at the type level — a zero wire value is a
/// deserialization error, so `admit` never sees degenerate concurrency.
/// These bounds are excluded from content identity: two runs over the same
/// ordered inputs share one input fingerprint (and therefore one cursor
/// lineage) no matter how they were bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseBatchLimits {
    schema_version: u32,
    workers: NonZeroUsize,
    queue_capacity: NonZeroUsize,
    max_in_flight: NonZeroUsize,
    max_documents: NonZeroUsize,
}

impl ParseBatchLimits {
    /// Stamp the four bounds with the current schema version.
    pub const fn new(
        workers: NonZeroUsize,
        queue_capacity: NonZeroUsize,
        max_in_flight: NonZeroUsize,
        max_documents: NonZeroUsize,
    ) -> Self {
        Self {
            schema_version: PARSE_BATCH_SCHEMA_VERSION,
            workers,
            queue_capacity,
            max_in_flight,
            max_documents,
        }
    }

    /// Stable schema version of this instance.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Worker concurrency bound.
    pub const fn workers(&self) -> NonZeroUsize {
        self.workers
    }

    /// Admission queue capacity (independent from the in-flight degree).
    pub const fn queue_capacity(&self) -> NonZeroUsize {
        self.queue_capacity
    }

    /// Simultaneously executing documents (independent from the queue
    /// capacity).
    pub const fn max_in_flight(&self) -> NonZeroUsize {
        self.max_in_flight
    }

    /// Maximum documents one admitted batch may carry.
    pub const fn max_documents(&self) -> NonZeroUsize {
        self.max_documents
    }
}

impl Serialize for ParseBatchLimits {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseBatchLimits", 5)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("workers", &self.workers)?;
        state.serialize_field("queue_capacity", &self.queue_capacity)?;
        state.serialize_field("max_in_flight", &self.max_in_flight)?;
        state.serialize_field("max_documents", &self.max_documents)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseBatchLimits {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            workers: NonZeroUsize,
            queue_capacity: NonZeroUsize,
            max_in_flight: NonZeroUsize,
            max_documents: NonZeroUsize,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_BATCH_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported batch limits schema version {} (supported: {PARSE_BATCH_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            workers: repr.workers,
            queue_capacity: repr.queue_capacity,
            max_in_flight: repr.max_in_flight,
            max_documents: repr.max_documents,
        })
    }
}

// ---------------------------------------------------------------------------
// Ordered items
// ---------------------------------------------------------------------------

/// One ordered batch item over a [`ParseBackendRequest`]: the request
/// (logical id, source bytes, closed endpoint + effective dialect) plus its
/// stable admission ordinal. The ordinal is the document's identity inside
/// the batch — outcomes, dispositions and cursor positions all reference
/// it. Items are in-process values (requests carry source bytes, which are
/// not wire material); the CLI manifest reader (T04+) constructs them from
/// bytes it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBatchItem {
    ordinal: u32,
    request: ParseBackendRequest,
}

impl ParseBatchItem {
    /// Ordered item at `ordinal` over `request`.
    pub const fn new(ordinal: u32, request: ParseBackendRequest) -> Self {
        Self { ordinal, request }
    }

    /// Stable ordinal of this item inside the admitted batch.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// The carried request (endpoint + dialect identity carrier).
    pub const fn request(&self) -> &ParseBackendRequest {
        &self.request
    }

    /// Provider-neutral logical paper id.
    pub fn paper_id(&self) -> &str {
        self.request.paper_id()
    }

    /// Deterministic result identity this request stamps (S05 vocabulary
    /// passthrough — endpoint/result dialect identity is preserved, never
    /// re-derived by the batch layer).
    pub fn result_identity(&self) -> TeiResultIdentity {
        self.request.result_identity()
    }
}

// ---------------------------------------------------------------------------
// Ordered input fingerprint (content identity)
// ---------------------------------------------------------------------------

/// Write one length-prefixed segment into the hasher (unambiguous framing).
fn write_segment(hasher: &mut Sha256, tag: &[u8], bytes: &[u8]) {
    hasher.update(tag);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// The complete effective dialect wire form of one request.
///
/// Rendered over the closed dialect structs (deterministic struct field
/// order, every field serialized) so ANY output-affecting difference —
/// including the rich/structural legacy presets, whose canonical extended
/// identity is deliberately absent — changes the fingerprint. Future
/// dialect fields are covered automatically because serialization walks the
/// struct, not a hand-maintained field list. Shared with the S06 stage-key
/// derivation (`cache::raw_tei_stage_key`).
pub(crate) fn endpoint_dialect_wire(request: &ParseBackendRequest) -> String {
    match request.endpoint() {
        ParseEndpoint::Fulltext => {
            serde_json::to_string(request.dialect()).expect("closed dialect serialization is total")
        }
        ParseEndpoint::Header => serde_json::to_string(
            &request
                .header_dialect()
                .expect("header requests carry a header dialect (ParseBackendRequest invariant)"),
        )
        .expect("closed dialect serialization is total"),
        ParseEndpoint::References => serde_json::to_string(&request.references_dialect().expect(
            "references requests carry a references dialect (ParseBackendRequest invariant)",
        ))
        .expect("closed dialect serialization is total"),
    }
}

/// Deterministic, order-sensitive fingerprint over the ordered request
/// content: domain separator, then per item the ordinal, paper id, closed
/// endpoint, complete dialect wire form and source bytes (each
/// length-prefixed). Returns 64 lowercase hex chars.
///
/// Deliberately EXCLUDED from this identity: cache mode (cost, not
/// semantics), worker/queue/in-flight/document bounds (tuning), timeouts,
/// physical roots and accelerator settings (locations). The fingerprint is
/// pure input CONTENT identity — the exact binding a [`BatchCursor`] may
/// resume against.
///
/// This is the explicit historical (versionless) entry point: it is
/// byte-identical to [`ordered_input_fingerprint_with_backend_version`] with
/// `None`, so pre-version cursors and reports stay resumable.
pub fn ordered_input_fingerprint(items: &[ParseBatchItem]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    for item in items {
        hasher.update(b"item");
        hasher.update(item.ordinal().to_le_bytes());
        write_segment(&mut hasher, b"paper_id", item.paper_id().as_bytes());
        let endpoint = item.request().endpoint();
        write_segment(&mut hasher, b"endpoint", endpoint.as_str().as_bytes());
        let dialect_wire = endpoint_dialect_wire(item.request());
        write_segment(&mut hasher, b"dialect", dialect_wire.as_bytes());
        write_segment(&mut hasher, b"pdf_bytes", item.request().pdf_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Same as [`ordered_input_fingerprint`] plus an optional backend version
/// segment. `None` is byte-identical to [`ordered_input_fingerprint`], so
/// versionless runs keep historical fingerprint/cursor identity. A probed
/// service version is an input-CONTENT identity component (R121): work
/// produced by one backend version must never be resumed or reported under
/// another, so a versioned fingerprint never matches a versionless (or
/// differently-versioned) cursor, report or accelerator row.
pub fn ordered_input_fingerprint_with_backend_version(
    items: &[ParseBatchItem],
    backend_version: Option<&str>,
) -> String {
    match backend_version.map(str::trim).filter(|v| !v.is_empty()) {
        None => ordered_input_fingerprint(items),
        Some(version) => {
            let mut hasher = Sha256::new();
            hasher.update(FINGERPRINT_DOMAIN);
            for item in items {
                hasher.update(b"item");
                hasher.update(item.ordinal().to_le_bytes());
                write_segment(&mut hasher, b"paper_id", item.paper_id().as_bytes());
                let endpoint = item.request().endpoint();
                write_segment(&mut hasher, b"endpoint", endpoint.as_str().as_bytes());
                let dialect_wire = endpoint_dialect_wire(item.request());
                write_segment(&mut hasher, b"dialect", dialect_wire.as_bytes());
                write_segment(&mut hasher, b"pdf_bytes", item.request().pdf_bytes());
            }
            write_segment(&mut hasher, b"backend_version", version.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

// ---------------------------------------------------------------------------
// Schema-bound resume cursor
// ---------------------------------------------------------------------------

/// Fail-closed cursor validation outcome (before any document work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseBatchCursorError {
    /// The cursor was written by a foreign schema generation.
    SchemaVersion {
        /// Version found on the cursor.
        found: u32,
        /// Version this contract supports.
        supported: u32,
    },
    /// The cursor belongs to a different ordered input set.
    FingerprintMismatch {
        /// Fingerprint carried by the cursor.
        cursor_fingerprint: String,
        /// Fingerprint of the batch being resumed.
        batch_fingerprint: String,
    },
    /// The cursor points beyond the end of the input set.
    OrdinalBeyondInput {
        /// Next ordinal requested by the cursor.
        next_ordinal: u32,
        /// Documents actually present in the batch.
        total_documents: u32,
    },
}

impl fmt::Display for ParseBatchCursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaVersion { found, supported } => write!(
                f,
                "cursor schema version {found} unsupported (supported: {supported})"
            ),
            Self::FingerprintMismatch {
                cursor_fingerprint,
                batch_fingerprint,
            } => write!(
                f,
                "cursor input fingerprint {cursor_fingerprint} does not match batch input fingerprint {batch_fingerprint}"
            ),
            Self::OrdinalBeyondInput {
                next_ordinal,
                total_documents,
            } => write!(
                f,
                "cursor next ordinal {next_ordinal} beyond input total {total_documents}"
            ),
        }
    }
}

impl std::error::Error for ParseBatchCursorError {}

/// Schema-bound resume cursor of one bounded batch run.
///
/// The cursor is bound to exactly ONE ordered input fingerprint and points
/// at the next unprocessed ordinal. The executor (T02) moves it only
/// through the durably acknowledged contiguous terminal prefix — a
/// cancellation never skips an unwritten ordinal. Any mismatch against the
/// current batch (fingerprint, ordinal range, schema version) is an error
/// before work starts, never a silent restart-from-zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCursor {
    schema_version: u32,
    input_fingerprint: String,
    next_ordinal: u32,
}

impl BatchCursor {
    /// Cursor at the start of `input_fingerprint`.
    pub fn initial(input_fingerprint: impl Into<String>) -> Self {
        Self::at(input_fingerprint, 0)
    }

    /// Cursor positioned at `next_ordinal` over `input_fingerprint`.
    pub fn at(input_fingerprint: impl Into<String>, next_ordinal: u32) -> Self {
        Self {
            schema_version: PARSE_BATCH_CURSOR_SCHEMA_VERSION,
            input_fingerprint: input_fingerprint.into(),
            next_ordinal,
        }
    }

    /// Stable schema version of this cursor.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// The one ordered input fingerprint this cursor may resume.
    pub fn input_fingerprint(&self) -> &str {
        &self.input_fingerprint
    }

    /// Next unprocessed ordinal.
    pub const fn next_ordinal(&self) -> u32 {
        self.next_ordinal
    }

    /// Fail-closed validation against the batch being resumed.
    pub fn validate_for(
        &self,
        batch_fingerprint: &str,
        total_documents: usize,
    ) -> Result<(), ParseBatchCursorError> {
        if self.schema_version != PARSE_BATCH_CURSOR_SCHEMA_VERSION {
            return Err(ParseBatchCursorError::SchemaVersion {
                found: self.schema_version,
                supported: PARSE_BATCH_CURSOR_SCHEMA_VERSION,
            });
        }
        if self.input_fingerprint != batch_fingerprint {
            return Err(ParseBatchCursorError::FingerprintMismatch {
                cursor_fingerprint: self.input_fingerprint.clone(),
                batch_fingerprint: batch_fingerprint.to_string(),
            });
        }
        let total = u32::try_from(total_documents).unwrap_or(u32::MAX);
        if self.next_ordinal > total {
            return Err(ParseBatchCursorError::OrdinalBeyondInput {
                next_ordinal: self.next_ordinal,
                total_documents: total,
            });
        }
        Ok(())
    }
}

impl Serialize for BatchCursor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("BatchCursor", 3)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("input_fingerprint", &self.input_fingerprint)?;
        state.serialize_field("next_ordinal", &self.next_ordinal)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for BatchCursor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            input_fingerprint: String,
            next_ordinal: u32,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_BATCH_CURSOR_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported batch cursor schema version {} (supported: {PARSE_BATCH_CURSOR_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            input_fingerprint: repr.input_fingerprint,
            next_ordinal: repr.next_ordinal,
        })
    }
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

/// Batch-wide admission failure. This is the ONLY batch-wide error shape in
/// the contract: once admitted, every backend or cache failure is a
/// per-document [`ParseDocumentFailure`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseBatchAdmissionError {
    /// No documents were offered — a degenerate run is a config error, not
    /// an empty success.
    EmptyBatch,
    /// A request carried an empty logical paper id.
    EmptyPaperId {
        /// Zero-based position of the offending request.
        index: usize,
    },
    /// Two requests carried the same logical paper id (duplicate
    /// documents cannot share one batch identity).
    DuplicatePaperId {
        /// The duplicated logical paper id.
        paper_id: String,
    },
    /// More documents than [`ParseBatchLimits::max_documents`] allows.
    DocumentLimitExceeded {
        /// Documents offered to admission.
        requested: usize,
        /// The bound that rejected them.
        max_documents: usize,
    },
    /// The resume cursor does not belong to this ordered input set (or is
    /// schema/out-of-range invalid) — resume fails closed before work.
    InvalidCursor(ParseBatchCursorError),
}

impl fmt::Display for ParseBatchAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => f.write_str("batch admission rejected: no documents offered"),
            Self::EmptyPaperId { index } => {
                write!(
                    f,
                    "batch admission rejected: empty paper id at index {index}"
                )
            }
            Self::DuplicatePaperId { paper_id } => {
                write!(
                    f,
                    "batch admission rejected: duplicate paper id `{paper_id}`"
                )
            }
            Self::DocumentLimitExceeded {
                requested,
                max_documents,
            } => write!(
                f,
                "batch admission rejected: {requested} documents exceed the bound {max_documents}"
            ),
            Self::InvalidCursor(error) => {
                write!(f, "batch admission rejected: invalid cursor: {error}")
            }
        }
    }
}

impl std::error::Error for ParseBatchAdmissionError {}

/// One admitted, bounded batch: the ordered items, their shared content
/// identity, the cache mode and the effective resume cursor.
///
/// Admission is the single fail-fast gate (empty/duplicate/oversized
/// inputs, foreign cursors); after it returns `Ok`, the run owes exactly
/// one terminal [`ParseDocumentOutcome`] per item — continuations after
/// partial failures are the executor's (T02) obligation, not a batch-wide
/// error path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBatch {
    mode: CacheMode,
    limits: ParseBatchLimits,
    input_fingerprint: String,
    cursor: BatchCursor,
    items: Vec<ParseBatchItem>,
    backend_version: Option<String>,
}

impl ParseBatch {
    /// Admit an ordered document set under `mode` and `limits` with the
    /// explicit historical (versionless) identity.
    ///
    /// This is the compatibility entry point: it is byte-identical to
    /// [`Self::admit_with_backend_version`] with `None`, so pre-version
    /// callers keep their exact fingerprint, cursor lineage and stage keys.
    pub fn admit(
        mode: CacheMode,
        limits: ParseBatchLimits,
        requests: Vec<ParseBackendRequest>,
        resume_cursor: Option<BatchCursor>,
    ) -> Result<Self, ParseBatchAdmissionError> {
        Self::admit_with_backend_version(mode, limits, requests, resume_cursor, None)
    }

    /// Admit an ordered document set under `mode` and `limits`, stamping
    /// `backend_version` (a probed service version, when one was observed)
    /// into the ordered input fingerprint.
    ///
    /// Validation order: non-empty → document bound → empty ids →
    /// duplicates → cursor. Admission assigns stable ordinals `0..n` in
    /// input order and stamps the ordered input fingerprint; a provided
    /// resume cursor must match that fingerprint or admission fails before
    /// any document work. `None` (or blank) is the explicit versionless
    /// identity and reproduces the historical fingerprint exactly.
    pub fn admit_with_backend_version(
        mode: CacheMode,
        limits: ParseBatchLimits,
        requests: Vec<ParseBackendRequest>,
        resume_cursor: Option<BatchCursor>,
        backend_version: Option<String>,
    ) -> Result<Self, ParseBatchAdmissionError> {
        if requests.is_empty() {
            return Err(ParseBatchAdmissionError::EmptyBatch);
        }
        if requests.len() > limits.max_documents.get() || requests.len() > u32::MAX as usize {
            return Err(ParseBatchAdmissionError::DocumentLimitExceeded {
                requested: requests.len(),
                max_documents: limits.max_documents.get(),
            });
        }
        let mut seen = BTreeSet::new();
        for (index, request) in requests.iter().enumerate() {
            if request.paper_id().is_empty() {
                return Err(ParseBatchAdmissionError::EmptyPaperId { index });
            }
            if !seen.insert(request.paper_id().to_string()) {
                return Err(ParseBatchAdmissionError::DuplicatePaperId {
                    paper_id: request.paper_id().to_string(),
                });
            }
        }
        // Blank is "absent" (the probe never yields an empty version), so a
        // blank value can never reach a key or fingerprint as a segment.
        let backend_version = backend_version
            .map(|version| version.trim().to_string())
            .filter(|version| !version.is_empty());
        let items: Vec<ParseBatchItem> = requests
            .into_iter()
            .enumerate()
            .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request))
            .collect();
        let input_fingerprint =
            ordered_input_fingerprint_with_backend_version(&items, backend_version.as_deref());
        let cursor = match resume_cursor {
            Some(cursor) => {
                cursor
                    .validate_for(&input_fingerprint, items.len())
                    .map_err(ParseBatchAdmissionError::InvalidCursor)?;
                cursor
            }
            None => BatchCursor::initial(input_fingerprint.clone()),
        };
        Ok(Self {
            mode,
            limits,
            input_fingerprint,
            cursor,
            items,
            backend_version,
        })
    }

    /// Cache mode of this run (cost strategy — never part of the identity).
    pub const fn mode(&self) -> CacheMode {
        self.mode
    }

    /// Probed backend version stamped into this batch's content identity
    /// (`None` = explicit historical/versionless identity).
    pub fn backend_version(&self) -> Option<&str> {
        self.backend_version.as_deref()
    }

    /// Execution bounds of this run (excluded from the identity).
    pub const fn limits(&self) -> &ParseBatchLimits {
        &self.limits
    }

    /// Ordered items with their stable ordinals.
    pub fn items(&self) -> &[ParseBatchItem] {
        &self.items
    }

    /// Number of admitted documents (== `items.len()`).
    pub fn total_documents(&self) -> usize {
        self.items.len()
    }

    /// The ordered input content fingerprint of this batch.
    pub fn input_fingerprint(&self) -> &str {
        &self.input_fingerprint
    }

    /// The effective resume cursor (provided and validated, or initial).
    pub const fn cursor(&self) -> &BatchCursor {
        &self.cursor
    }

    /// Consume the batch, yielding the ordered items (executor handoff —
    /// source bytes move without a copy).
    pub fn into_items(self) -> Vec<ParseBatchItem> {
        self.items
    }
}

// ---------------------------------------------------------------------------
// Per-document terminal outcomes
// ---------------------------------------------------------------------------

/// Successful terminal payload of one admitted document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseDocumentSuccess {
    article: ParsedArticle,
    result_identity: String,
}

impl ParseDocumentSuccess {
    /// Success over the projected article and the canonical result identity
    /// (`TeiResultIdentity::canonical()` — endpoint/result dialect identity
    /// preserved verbatim).
    pub fn new(article: ParsedArticle, result_identity: impl Into<String>) -> Self {
        Self {
            article,
            result_identity: result_identity.into(),
        }
    }

    /// The projected article (mode-independent semantics).
    pub const fn article(&self) -> &ParsedArticle {
        &self.article
    }

    /// Canonical result identity string the request stamped.
    pub fn result_identity(&self) -> &str {
        &self.result_identity
    }
}

/// Closed per-document failure vocabulary: the ONLY failure surface after
/// admission. Both variants stay typed and carry no payload, path or
/// message material; the backend variant preserves the closed S04/S05
/// metadata verbatim (no legacy `ParserError` collapse).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseDocumentFailure {
    /// The closed typed transport failure, verbatim.
    Backend(ParseBackendFailure),
    /// A cache integrity failure on one stage (corruption is an error,
    /// never a silent miss).
    Cache(ParseCacheFailure),
}

impl fmt::Display for ParseDocumentFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(failure) => write!(f, "backend: {failure}"),
            Self::Cache(failure) => write!(f, "cache: {failure}"),
        }
    }
}

impl From<ParseBackendFailure> for ParseDocumentFailure {
    fn from(failure: ParseBackendFailure) -> Self {
        Self::Backend(failure)
    }
}

impl From<ParseCacheFailure> for ParseDocumentFailure {
    fn from(failure: ParseCacheFailure) -> Self {
        Self::Cache(failure)
    }
}

/// Exactly one terminal outcome per admitted ordinal: the run identity
/// (ordinal, paper id, cache mode), both stage dispositions, and the
/// success/failure result. Schema-versioned; unknown versions and fields
/// are rejected at deserialization. (Not `PartialEq`: the carried
/// `ParsedArticle` is not — compare via the serde wire form.)
#[derive(Debug, Clone)]
pub struct ParseDocumentOutcome {
    schema_version: u32,
    ordinal: u32,
    paper_id: String,
    cache_mode: CacheMode,
    dispositions: ParseCacheDispositions,
    result: Result<ParseDocumentSuccess, ParseDocumentFailure>,
}

impl Serialize for ParseDocumentOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseDocumentOutcome", 6)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("ordinal", &self.ordinal)?;
        state.serialize_field("paper_id", &self.paper_id)?;
        state.serialize_field("cache_mode", &self.cache_mode)?;
        state.serialize_field("dispositions", &self.dispositions)?;
        state.serialize_field("result", &self.result)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseDocumentOutcome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            ordinal: u32,
            paper_id: String,
            cache_mode: CacheMode,
            dispositions: ParseCacheDispositions,
            result: Result<ParseDocumentSuccess, ParseDocumentFailure>,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_BATCH_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported batch outcome schema version {} (supported: {PARSE_BATCH_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            ordinal: repr.ordinal,
            paper_id: repr.paper_id,
            cache_mode: repr.cache_mode,
            dispositions: repr.dispositions,
            result: repr.result,
        })
    }
}

impl ParseDocumentOutcome {
    /// Stamp one terminal outcome with the current schema version.
    pub const fn new(
        ordinal: u32,
        paper_id: String,
        cache_mode: CacheMode,
        dispositions: ParseCacheDispositions,
        result: Result<ParseDocumentSuccess, ParseDocumentFailure>,
    ) -> Self {
        Self {
            schema_version: PARSE_BATCH_SCHEMA_VERSION,
            ordinal,
            paper_id,
            cache_mode,
            dispositions,
            result,
        }
    }

    /// Stable schema version of this outcome.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Stable ordinal of the admitted document.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Logical paper id of the admitted document.
    pub fn paper_id(&self) -> &str {
        &self.paper_id
    }

    /// Cache mode of the run that produced this outcome.
    pub const fn cache_mode(&self) -> CacheMode {
        self.cache_mode
    }

    /// Both stage dispositions (raw TEI + projection).
    pub const fn dispositions(&self) -> &ParseCacheDispositions {
        &self.dispositions
    }

    /// The terminal result (exactly one per document, by contract).
    pub const fn result(&self) -> &Result<ParseDocumentSuccess, ParseDocumentFailure> {
        &self.result
    }

    /// Whether the document succeeded.
    pub const fn is_success(&self) -> bool {
        self.result.is_ok()
    }
}
// S06/T01 typed batch contract

// ---------------------------------------------------------------------------
// S06/T02: durable sink/checkpoint seams + bounded two-stage batch executor
// ---------------------------------------------------------------------------

/// Stable schema version of the run summary vocabulary.
pub const PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION: u32 = 2;

/// Failure of the durable result-sink or checkpoint seam. The message is
/// caller-owned text — the executor never attaches payload, path or
/// location material to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBatchSinkError(pub String);

impl fmt::Display for ParseBatchSinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse batch sink failure: {}", self.0)
    }
}

impl std::error::Error for ParseBatchSinkError {}

/// Durable terminal-result sink (S06/T02 seam): results stream out through
/// this seam as documents reach terminal state — no full-corpus payload is
/// retained beyond the bounded queue. `Ok(())` is the DURABLE
/// acknowledgment the cursor advancement is built on; `Err` leaves the
/// document unacknowledged and the cursor never moves past it.
#[async_trait]
pub trait ParseBatchResultSink: Send + Sync {
    /// Record one terminal outcome durably.
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError>;
}

/// Durable batch checkpoint seam (S06/T02): the executor saves the cursor
/// of the durably acknowledged contiguous terminal prefix, in strictly
/// advancing order. A run interrupted anywhere resumes from the last saved
/// cursor — unwritten ordinals are never skipped.
#[async_trait]
pub trait BatchCheckpoint: Send + Sync {
    /// Durably record `cursor` (the contiguous acknowledged terminal
    /// prefix position).
    async fn save(&self, cursor: &BatchCursor) -> Result<(), ParseBatchSinkError>;

    /// Whether a saved cursor survives this process.
    ///
    /// This is a durability obligation, not a hint: a store that answers `true`
    /// promises the next process can resume from it, so the cursor may only be
    /// advanced over content that store can also recover. A `false` store is a
    /// per-run seam (tests, in-memory composition) where the cursor is a
    /// bookkeeping position and no such obligation exists.
    ///
    /// Defaults to `false` so a new implementation must opt in deliberately
    /// rather than inherit an obligation it cannot meet.
    fn is_durable(&self) -> bool {
        false
    }
}

/// In-memory [`ParseBatchResultSink`] (provider-neutral composition/test
/// seam): collects outcomes; reads return them in admission-ordinal order.
#[derive(Debug, Default)]
pub struct ParseBatchCollector {
    outcomes: Mutex<Vec<ParseDocumentOutcome>>,
}

impl ParseBatchCollector {
    /// Empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// All recorded outcomes sorted by admission ordinal (exactly one
    /// entry per terminal document of the run).
    pub async fn outcomes_sorted(&self) -> Vec<ParseDocumentOutcome> {
        let mut outcomes = self.outcomes.lock().await.clone();
        outcomes.sort_by_key(|outcome| outcome.ordinal());
        outcomes
    }
}

#[async_trait]
impl ParseBatchResultSink for ParseBatchCollector {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        self.outcomes.lock().await.push(outcome);
        Ok(())
    }
}

/// In-memory [`BatchCheckpoint`] (provider-neutral composition/test seam):
/// keeps the last durably saved cursor.
#[derive(Debug, Default)]
pub struct ParseBatchMemoryCheckpoint {
    cursor: Mutex<Option<BatchCursor>>,
}

impl ParseBatchMemoryCheckpoint {
    /// Empty checkpoint.
    pub fn new() -> Self {
        Self::default()
    }

    /// The last durably saved cursor, if any.
    pub async fn cursor(&self) -> Option<BatchCursor> {
        self.cursor.lock().await.clone()
    }
}

#[async_trait]
impl BatchCheckpoint for ParseBatchMemoryCheckpoint {
    async fn save(&self, cursor: &BatchCursor) -> Result<(), ParseBatchSinkError> {
        *self.cursor.lock().await = Some(cursor.clone());
        Ok(())
    }
}

/// Closed run summary of one bounded batch execution: admission/skip
/// counts, terminal counts, per-stage hit/write counters (including honest
/// failed-publication counts and legacy read-only hits), bounded-load
/// gauges, sink/checkpoint errors and the durable cursor the run ended
/// with. No payload, no location, no tuning material beyond the declared
/// bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBatchRunSummary {
    schema_version: u32,
    mode: CacheMode,
    limits: ParseBatchLimits,
    input_fingerprint: String,
    resumed_from_ordinal: u32,
    cursor: BatchCursor,
    admitted: u32,
    skipped_on_resume: u32,
    completed: u32,
    succeeded: u32,
    failed: u32,
    backend_calls: u64,
    raw_hits: u64,
    raw_legacy_hits: u64,
    raw_writes: u64,
    raw_write_failures: u64,
    projector_hits: u64,
    projector_writes: u64,
    projector_write_failures: u64,
    configured_publication_capacity: usize,
    configured_execution_capacity: usize,
    max_in_flight_observed: usize,
    max_queue_depth_observed: usize,
    max_publication_outstanding_observed: usize,
    max_reorder_depth_observed: usize,
    budget_waits: u64,
    budget_scope: &'static str,
    sink_errors: u64,
    checkpoint_errors: u64,
}

impl ParseBatchRunSummary {
    /// Stable schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Cache mode of the run.
    pub const fn mode(&self) -> CacheMode {
        self.mode
    }
    /// Effective bounds of the run.
    pub const fn limits(&self) -> &ParseBatchLimits {
        &self.limits
    }
    /// The ordered input content fingerprint.
    pub fn input_fingerprint(&self) -> &str {
        &self.input_fingerprint
    }
    /// Ordinal the run started at (cursor position on admission).
    pub const fn resumed_from_ordinal(&self) -> u32 {
        self.resumed_from_ordinal
    }
    /// The durable cursor at run end (contiguous acknowledged prefix).
    pub const fn cursor(&self) -> &BatchCursor {
        &self.cursor
    }
    /// Documents admitted to the batch.
    pub const fn admitted(&self) -> u32 {
        self.admitted
    }
    /// Documents skipped because a prior run already made them terminal.
    pub const fn skipped_on_resume(&self) -> u32 {
        self.skipped_on_resume
    }
    /// Terminal outcomes produced by THIS run.
    pub const fn completed(&self) -> u32 {
        self.completed
    }
    /// Documents that succeeded.
    pub const fn succeeded(&self) -> u32 {
        self.succeeded
    }
    /// Documents that failed (backend or cache integrity — never silent).
    pub const fn failed(&self) -> u32 {
        self.failed
    }
    /// Backend invocations (exactly once per fetched document).
    pub const fn backend_calls(&self) -> u64 {
        self.backend_calls
    }
    /// Raw-stage cache hits.
    pub const fn raw_hits(&self) -> u64 {
        self.raw_hits
    }
    /// Raw-stage hits served from a LEGACY read-only identity (T03+).
    pub const fn raw_legacy_hits(&self) -> u64 {
        self.raw_legacy_hits
    }
    /// Raw-stage durable publishes.
    pub const fn raw_writes(&self) -> u64 {
        self.raw_writes
    }
    /// Raw-stage publications that failed (honest, never semantic drift).
    pub const fn raw_write_failures(&self) -> u64 {
        self.raw_write_failures
    }
    /// Projection-stage cache hits.
    pub const fn projector_hits(&self) -> u64 {
        self.projector_hits
    }
    /// Projection-stage durable publishes.
    pub const fn projector_writes(&self) -> u64 {
        self.projector_writes
    }
    /// Projection-stage publications that failed.
    pub const fn projector_write_failures(&self) -> u64 {
        self.projector_write_failures
    }
    /// Peak simultaneously executing documents.
    pub const fn max_in_flight_observed(&self) -> usize {
        self.max_in_flight_observed
    }
    /// Configured publication slots for this run (process-local shared budget).
    pub const fn configured_publication_capacity(&self) -> usize {
        self.configured_publication_capacity
    }
    /// Configured active execution slots for this run (process-local shared budget).
    pub const fn configured_execution_capacity(&self) -> usize {
        self.configured_execution_capacity
    }
    /// Peak bounded-queue backlog (admitted, not yet received by a worker).
    pub const fn max_queue_depth_observed(&self) -> usize {
        self.max_queue_depth_observed
    }
    /// Peak number of admitted results retained before ordered publication.
    pub const fn max_publication_outstanding_observed(&self) -> usize {
        self.max_publication_outstanding_observed
    }
    /// Peak number of out-of-order results waiting behind the frontier.
    pub const fn max_reorder_depth_observed(&self) -> usize {
        self.max_reorder_depth_observed
    }
    /// Number of semaphore acquisitions that had to wait.
    pub const fn budget_waits(&self) -> u64 {
        self.budget_waits
    }
    /// Budget scope. This is never shared between CLI processes.
    pub const fn budget_scope(&self) -> &'static str {
        self.budget_scope
    }
    /// Sink acknowledgments that failed (outcomes stay unclaimed).
    pub const fn sink_errors(&self) -> u64 {
        self.sink_errors
    }
    /// Checkpoint saves that failed (durable cursor freezes honestly).
    pub const fn checkpoint_errors(&self) -> u64 {
        self.checkpoint_errors
    }
}

impl Serialize for ParseBatchRunSummary {
    #[rustfmt::skip]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ParseBatchRunSummary", 30)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("mode", &self.mode)?;
        state.serialize_field("limits", &self.limits)?;
        state.serialize_field("input_fingerprint", &self.input_fingerprint)?;
        state.serialize_field("resumed_from_ordinal", &self.resumed_from_ordinal)?;
        state.serialize_field("cursor", &self.cursor)?;
        state.serialize_field("admitted", &self.admitted)?;
        state.serialize_field("skipped_on_resume", &self.skipped_on_resume)?;
        state.serialize_field("completed", &self.completed)?;
        state.serialize_field("succeeded", &self.succeeded)?;
        state.serialize_field("failed", &self.failed)?;
        state.serialize_field("backend_calls", &self.backend_calls)?;
        state.serialize_field("raw_hits", &self.raw_hits)?;
        state.serialize_field("raw_legacy_hits", &self.raw_legacy_hits)?;
        state.serialize_field("raw_writes", &self.raw_writes)?;
        state.serialize_field("raw_write_failures", &self.raw_write_failures)?;
        state.serialize_field("projector_hits", &self.projector_hits)?;
        state.serialize_field("projector_writes", &self.projector_writes)?;
        state.serialize_field("projector_write_failures", &self.projector_write_failures)?;
        let configured_publication = &self.configured_publication_capacity;
        let configured_execution = &self.configured_execution_capacity;
        let publication_outstanding = &self.max_publication_outstanding_observed;
        let reorder_depth = &self.max_reorder_depth_observed;
        state.serialize_field("configured_publication_capacity", configured_publication)?;
        state.serialize_field("configured_execution_capacity", configured_execution)?;
        state.serialize_field("max_in_flight_observed", &self.max_in_flight_observed)?;
        state.serialize_field("max_queue_depth_observed", &self.max_queue_depth_observed)?;
        state.serialize_field("max_publication_outstanding_observed", publication_outstanding)?;
        state.serialize_field("max_reorder_depth_observed", reorder_depth)?;
        state.serialize_field("budget_waits", &self.budget_waits)?;
        state.serialize_field("budget_scope", &self.budget_scope)?;
        state.serialize_field("sink_errors", &self.sink_errors)?;
        state.serialize_field("checkpoint_errors", &self.checkpoint_errors)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ParseBatchRunSummary {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            schema_version: u32,
            mode: CacheMode,
            limits: ParseBatchLimits,
            input_fingerprint: String,
            resumed_from_ordinal: u32,
            cursor: BatchCursor,
            admitted: u32,
            skipped_on_resume: u32,
            completed: u32,
            succeeded: u32,
            failed: u32,
            backend_calls: u64,
            raw_hits: u64,
            raw_legacy_hits: u64,
            raw_writes: u64,
            raw_write_failures: u64,
            projector_hits: u64,
            projector_writes: u64,
            projector_write_failures: u64,
            configured_publication_capacity: usize,
            configured_execution_capacity: usize,
            max_in_flight_observed: usize,
            max_queue_depth_observed: usize,
            max_publication_outstanding_observed: usize,
            max_reorder_depth_observed: usize,
            budget_waits: u64,
            budget_scope: String,
            sink_errors: u64,
            checkpoint_errors: u64,
        }
        let repr = Repr::deserialize(deserializer)?;
        if repr.schema_version != PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported run summary schema version {} (supported: {PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION})",
                repr.schema_version
            )));
        }
        Ok(Self {
            schema_version: repr.schema_version,
            mode: repr.mode,
            limits: repr.limits,
            input_fingerprint: repr.input_fingerprint,
            resumed_from_ordinal: repr.resumed_from_ordinal,
            cursor: repr.cursor,
            admitted: repr.admitted,
            skipped_on_resume: repr.skipped_on_resume,
            completed: repr.completed,
            succeeded: repr.succeeded,
            failed: repr.failed,
            backend_calls: repr.backend_calls,
            raw_hits: repr.raw_hits,
            raw_legacy_hits: repr.raw_legacy_hits,
            raw_writes: repr.raw_writes,
            raw_write_failures: repr.raw_write_failures,
            projector_hits: repr.projector_hits,
            projector_writes: repr.projector_writes,
            projector_write_failures: repr.projector_write_failures,
            configured_publication_capacity: repr.configured_publication_capacity,
            configured_execution_capacity: repr.configured_execution_capacity,
            max_in_flight_observed: repr.max_in_flight_observed,
            max_queue_depth_observed: repr.max_queue_depth_observed,
            max_publication_outstanding_observed: repr.max_publication_outstanding_observed,
            max_reorder_depth_observed: repr.max_reorder_depth_observed,
            budget_waits: repr.budget_waits,
            budget_scope: Box::leak(repr.budget_scope.into_boxed_str()),
            sink_errors: repr.sink_errors,
            checkpoint_errors: repr.checkpoint_errors,
        })
    }
}

// ─────────────────── executor internals ───────────────────

#[derive(Debug, Default, Clone)]
struct DocumentStats {
    backend_calls: u64,
    raw_hit: bool,
    raw_legacy_hit: bool,
    raw_write_published: bool,
    raw_write_failed: bool,
    projector_hit: bool,
    projector_write_published: bool,
    projector_write_failed: bool,
    backend_ms: u64,
    projection_ms: u64,
}

struct DocumentExecution {
    outcome: ParseDocumentOutcome,
    stats: DocumentStats,
}

struct QueuedItem {
    item: ParseBatchItem,
    enqueued_at: Instant,
    /// Publication-window permit held from feeder admission through the
    /// durable sink acknowledgment. This is deliberately distinct from the
    /// execution permit: a worker may finish computation while waiting for
    /// an earlier ordinal without releasing the publication window.
    publication_permit: tokio::sync::OwnedSemaphorePermit,
    shared_publication_permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Debug)]
struct CursorTracker {
    acked: BTreeSet<u32>,
    acked_next: u32,
    saved_next: u32,
    checkpoint_broken: bool,
    /// Lowest ordinal that terminated without durably recoverable content.
    ///
    /// A durable cursor must never advance past this ordinal: the content for
    /// it exists only in memory, and an interrupted run writes no terminal
    /// report, so advancing would let the next resume skip a document whose
    /// result exists nowhere. Frozen once set, like `checkpoint_broken`.
    unrecoverable_at: Option<u32>,
}

#[derive(Debug, Default)]
struct RunCounters {
    succeeded: AtomicU64,
    failed: AtomicU64,
    backend_calls: AtomicU64,
    raw_hits: AtomicU64,
    raw_legacy_hits: AtomicU64,
    raw_writes: AtomicU64,
    raw_write_failures: AtomicU64,
    projector_hits: AtomicU64,
    projector_writes: AtomicU64,
    projector_write_failures: AtomicU64,
    sink_errors: AtomicU64,
    checkpoint_errors: AtomicU64,
    queue_depth: AtomicUsize,
    max_queue_depth: AtomicUsize,
    publication_outstanding: AtomicUsize,
    max_publication_outstanding: AtomicUsize,
    max_reorder_depth: AtomicUsize,
    budget_waits: AtomicU64,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

impl RunCounters {
    fn note_queue_depth(&self, delta: isize) {
        if delta >= 0 {
            let now = self.queue_depth.fetch_add(delta as usize, Ordering::SeqCst) + delta as usize;
            self.max_queue_depth.fetch_max(now, Ordering::SeqCst);
        } else {
            self.queue_depth
                .fetch_sub(delta.unsigned_abs(), Ordering::SeqCst);
        }
    }

    fn note_in_flight(&self, delta: isize) {
        if delta >= 0 {
            let now = self.in_flight.fetch_add(delta as usize, Ordering::SeqCst) + delta as usize;
            self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        } else {
            self.in_flight
                .fetch_sub(delta.unsigned_abs(), Ordering::SeqCst);
        }
    }
}

struct PublicationState {
    next_ordinal: u32,
    pending: BTreeMap<u32, ParseDocumentOutcome>,
}

struct RunShared {
    input_fingerprint: String,
    tracker: Mutex<CursorTracker>,
    publication: Mutex<PublicationState>,
    counters: RunCounters,
}

/// Durable-prefix advancement.
///
/// # Specification (proven by tests — read before changing)
///
/// 1. **Advance under the lock.** The outcome is acked, the contiguous terminal
///    prefix is extended, and the checkpoint is saved while the advancement
///    lock is held, so saved cursor positions are strictly monotonic.
///
/// 2. **A failed save freezes the cursor.** Later outcomes stay recorded but
///    are never claimed as resumable
///    (`sink_failure_freezes_cursor_and_resume_redoes_only_unwritten_ordinals`).
///
/// 3. **`publication.next_ordinal` advancing on a sink error is NOT a defect.**
///    Resume is driven by the durable cursor, never by that counter: a sink
///    failure at ordinal N leaves the cursor at N and the resume pass redoes
///    exactly N. The counter only decides which outcome is published next
///    within this run.
///
/// 4. **Interruption + resume from a durable cursor is a legitimate capability.**
///    It relies on published content already being durable in the artifact
///    cache; the terminal report is deliberately NOT written by an interrupted
///    run. A change that defers the cursor until the report exists (tried:
///    `DeferredRedbCheckpoint`) breaks this and fails
///    `forced_interruption_resumes_from_the_durable_redb_cursor`.
///
/// 5. **Durability binds only a store that claims it.** `is_durable()` gates
///    the obligation, so an in-memory checkpoint keeps bookkeeping semantics.
///
/// 6. **Only SUCCESSFUL, unpublished content freezes the prefix.** A document
///    that failed produced no content by definition and would fail identically
///    again, so it keeps the established "processed" meaning of the cursor
///    (`busy_503_is_typed_exactly_once_and_the_run_continues`). A document that
///    SUCCEEDED while its content was never published — cache off, or a
///    read-only miss (`ComputedUncached` / `ComputedUnpublished`) — has results
///    that exist nowhere, so the cursor must stop before it
///    (`durable_cursor_never_advances_past_unpublished_content`).
///
/// 7. **Recoverability is judged on the raw stage**, because that stage is what
///    required the Java call; a projection is rebuilt from stored raw TEI
///    without the backend
///    (`raw_hit_reprojects_after_projector_schema_rotation_without_backend`).
async fn acknowledge(
    shared: &RunShared,
    checkpoint: &dyn BatchCheckpoint,
    ordinal: u32,
    recoverable: bool,
) {
    let mut tracker = shared.tracker.lock().await;
    tracker.acked.insert(ordinal);
    while tracker.acked.contains(&tracker.acked_next) {
        tracker.acked_next += 1;
    }
    // The durability obligation only binds a store that claims to survive this
    // process. A per-run checkpoint keeps its former bookkeeping semantics, so
    // in-memory composition is unchanged.
    if checkpoint.is_durable() && !recoverable {
        tracker.unrecoverable_at = Some(
            tracker
                .unrecoverable_at
                .map_or(ordinal, |lowest| lowest.min(ordinal)),
        );
    }
    // A durable cursor may only cover the prefix that is both acknowledged AND
    // durably recoverable: content published by this run (or served from a hit)
    // is recoverable, while content computed without publishing exists only in
    // memory, so the cursor stops before it.
    let ceiling = tracker.unrecoverable_at.unwrap_or(u32::MAX);
    let target = tracker.acked_next.min(ceiling);
    if target > tracker.saved_next && !tracker.checkpoint_broken {
        let cursor = BatchCursor::at(shared.input_fingerprint.clone(), target);
        if checkpoint.save(&cursor).await.is_ok() {
            tracker.saved_next = target;
        } else {
            tracker.checkpoint_broken = true;
            shared
                .counters
                .checkpoint_errors
                .fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn elapsed_millis(at: Instant) -> u64 {
    u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn success_outcome(
    item: &ParseBatchItem,
    mode: CacheMode,
    raw: CacheStageDisposition,
    projector: CacheStageDisposition,
    article: ParsedArticle,
    result_identity: String,
    stats: DocumentStats,
) -> DocumentExecution {
    DocumentExecution {
        outcome: ParseDocumentOutcome::new(
            item.ordinal(),
            item.paper_id().to_string(),
            mode,
            ParseCacheDispositions::new(raw, projector),
            Ok(ParseDocumentSuccess::new(article, result_identity)),
        ),
        stats,
    }
}

fn failure_outcome(
    item: &ParseBatchItem,
    mode: CacheMode,
    raw: CacheStageDisposition,
    projector: CacheStageDisposition,
    failure: ParseDocumentFailure,
    stats: DocumentStats,
) -> DocumentExecution {
    DocumentExecution {
        outcome: ParseDocumentOutcome::new(
            item.ordinal(),
            item.paper_id().to_string(),
            mode,
            ParseCacheDispositions::new(raw, projector),
            Err(failure),
        ),
        stats,
    }
}

async fn fetch_timed<B: ParseBackend>(
    backend: &B,
    request: &ParseBackendRequest,
    stats: &mut DocumentStats,
) -> Result<TeiEnvelope, ParseBackendFailure> {
    stats.backend_calls += 1;
    let started = Instant::now();
    let result = backend.fetch_tei(request.clone()).await;
    stats.backend_ms = elapsed_millis(started);
    result
}

fn project_timed(
    paper_id: &str,
    pdf_hash: String,
    envelope: &TeiEnvelope,
    stats: &mut DocumentStats,
) -> ParsedArticle {
    let started = Instant::now();
    let article = lift_and_project_envelope(paper_id, pdf_hash, envelope);
    stats.projection_ms = elapsed_millis(started);
    article
}

/// Execute one document under the mode truth table. Cost strategies only:
/// the semantic output (projected article) is mode-independent.
async fn execute_document<B: ParseBackend, C: ParseArtifactCache>(
    backend: &B,
    cache: &C,
    projector_schema_fingerprint: &str,
    backend_version: Option<&str>,
    mode: CacheMode,
    item: &ParseBatchItem,
) -> DocumentExecution {
    use CacheStageDisposition::{ComputedPublished, ComputedUncached, ComputedUnpublished, Hit};

    let request = item.request();
    let paper_id = item.paper_id().to_string();
    let pdf_sha256 = sha256_hex(request.pdf_bytes());
    let pdf_hash = TeiLifter::pdf_hash(request.pdf_bytes());
    let result_identity = item.result_identity();

    match mode {
        CacheMode::Off => {
            // The mode never consults and never publishes: zero cache calls.
            let mut stats = DocumentStats::default();
            match fetch_timed(backend, request, &mut stats).await {
                Ok(envelope) => {
                    let article = project_timed(&paper_id, pdf_hash, &envelope, &mut stats);
                    success_outcome(
                        item,
                        mode,
                        ComputedUncached,
                        ComputedUncached,
                        article,
                        result_identity.canonical(),
                        stats,
                    )
                }
                Err(failure) => failure_outcome(
                    item,
                    mode,
                    ComputedUncached,
                    ComputedUncached,
                    ParseDocumentFailure::Backend(failure),
                    stats,
                ),
            }
        }
        CacheMode::ReadOnly | CacheMode::WriteThrough => {
            let raw_key = raw_tei_stage_key_with_backend_version(request, backend_version);
            let mut stats = DocumentStats::default();
            match cache.read_raw(&raw_key).await {
                Err(failure) => {
                    // Corruption is a typed ERROR, never a silent miss and
                    // never a backend fallback.
                    failure_outcome(
                        item,
                        mode,
                        ComputedUncached,
                        ComputedUncached,
                        ParseDocumentFailure::Cache(failure),
                        stats,
                    )
                }
                Ok(None) => {
                    // Raw miss: fetch fresh (exactly one backend call).
                    let envelope = match fetch_timed(backend, request, &mut stats).await {
                        Ok(envelope) => envelope,
                        Err(failure) => {
                            return failure_outcome(
                                item,
                                mode,
                                ComputedUncached,
                                ComputedUncached,
                                ParseDocumentFailure::Backend(failure),
                                stats,
                            );
                        }
                    };
                    let raw_sha256 = sha256_hex(envelope.raw_tei());
                    let projector_key = project_article_stage_key_with_backend_version(
                        &paper_id,
                        &pdf_sha256,
                        &raw_sha256,
                        projector_schema_fingerprint,
                        backend_version,
                    );
                    // Write-through commits raw BEFORE any projector-stage
                    // interaction; read-only never writes.
                    let raw_disposition = match mode {
                        CacheMode::WriteThrough => {
                            let raw_payload =
                                RawTeiEnvelopePayload::build(&envelope, &result_identity);
                            match cache.write_raw(&raw_key, &raw_payload.encode()).await {
                                Ok(()) => {
                                    stats.raw_write_published = true;
                                    ComputedPublished
                                }
                                Err(_) => {
                                    stats.raw_write_failed = true;
                                    ComputedUnpublished
                                }
                            }
                        }
                        _ => ComputedUnpublished,
                    };
                    match cache.read_projected(&projector_key).await {
                        Err(failure) => failure_outcome(
                            item,
                            mode,
                            raw_disposition,
                            ComputedUncached,
                            ParseDocumentFailure::Cache(failure),
                            stats,
                        ),
                        Ok(Some(read)) => {
                            match ProjectArticlePayload::decode_and_verify(
                                read.payload(),
                                projector_schema_fingerprint,
                                &raw_sha256,
                                &paper_id,
                                &pdf_sha256,
                            ) {
                                Err(failure) => failure_outcome(
                                    item,
                                    mode,
                                    raw_disposition,
                                    ComputedUncached,
                                    ParseDocumentFailure::Cache(failure),
                                    stats,
                                ),
                                Ok(article) => {
                                    stats.projector_hit = true;
                                    success_outcome(
                                        item,
                                        mode,
                                        raw_disposition,
                                        Hit,
                                        article,
                                        result_identity.canonical(),
                                        stats,
                                    )
                                }
                            }
                        }
                        Ok(None) => {
                            let article = project_timed(&paper_id, pdf_hash, &envelope, &mut stats);
                            match mode {
                                CacheMode::WriteThrough => {
                                    let projector_payload = ProjectArticlePayload::build(
                                        &article,
                                        projector_schema_fingerprint,
                                        &raw_sha256,
                                        &paper_id,
                                        &pdf_sha256,
                                    );
                                    let projector_disposition = match cache
                                        .write_projected(
                                            &projector_key,
                                            &projector_payload.encode(),
                                        )
                                        .await
                                    {
                                        Ok(()) => {
                                            stats.projector_write_published = true;
                                            ComputedPublished
                                        }
                                        Err(_) => {
                                            stats.projector_write_failed = true;
                                            ComputedUnpublished
                                        }
                                    };
                                    success_outcome(
                                        item,
                                        mode,
                                        raw_disposition,
                                        projector_disposition,
                                        article,
                                        result_identity.canonical(),
                                        stats,
                                    )
                                }
                                _ => success_outcome(
                                    item,
                                    mode,
                                    ComputedUnpublished,
                                    ComputedUnpublished,
                                    article,
                                    result_identity.canonical(),
                                    stats,
                                ),
                            }
                        }
                    }
                }
                Ok(Some(read)) => {
                    // Raw hit: verify bytes + identity BEFORE anything else.
                    stats.raw_hit = true;
                    stats.raw_legacy_hit = read.is_legacy();
                    let (envelope, raw_sha256) = match RawTeiEnvelopePayload::decode_and_verify(
                        read.payload(),
                        &result_identity,
                    ) {
                        Ok(verified) => verified,
                        Err(failure) => {
                            return failure_outcome(
                                item,
                                mode,
                                ComputedUncached,
                                ComputedUncached,
                                ParseDocumentFailure::Cache(failure),
                                stats,
                            );
                        }
                    };
                    let raw_disposition = mode.stage_disposition_for(true);
                    let projector_key = project_article_stage_key_with_backend_version(
                        &paper_id,
                        &pdf_sha256,
                        &raw_sha256,
                        projector_schema_fingerprint,
                        backend_version,
                    );
                    match cache.read_projected(&projector_key).await {
                        Err(failure) => failure_outcome(
                            item,
                            mode,
                            raw_disposition,
                            ComputedUncached,
                            ParseDocumentFailure::Cache(failure),
                            stats,
                        ),
                        Ok(Some(read)) => {
                            match ProjectArticlePayload::decode_and_verify(
                                read.payload(),
                                projector_schema_fingerprint,
                                &raw_sha256,
                                &paper_id,
                                &pdf_sha256,
                            ) {
                                Err(failure) => failure_outcome(
                                    item,
                                    mode,
                                    raw_disposition,
                                    ComputedUncached,
                                    ParseDocumentFailure::Cache(failure),
                                    stats,
                                ),
                                Ok(article) => {
                                    stats.projector_hit = true;
                                    success_outcome(
                                        item,
                                        mode,
                                        raw_disposition,
                                        Hit,
                                        article,
                                        result_identity.canonical(),
                                        stats,
                                    )
                                }
                            }
                        }
                        Ok(None) => {
                            // Projector miss: re-project from the verified
                            // raw bytes — never a backend re-fetch.
                            let article = project_timed(&paper_id, pdf_hash, &envelope, &mut stats);
                            match mode {
                                CacheMode::WriteThrough => {
                                    let projector_payload = ProjectArticlePayload::build(
                                        &article,
                                        projector_schema_fingerprint,
                                        &raw_sha256,
                                        &paper_id,
                                        &pdf_sha256,
                                    );
                                    let projector_disposition = match cache
                                        .write_projected(
                                            &projector_key,
                                            &projector_payload.encode(),
                                        )
                                        .await
                                    {
                                        Ok(()) => {
                                            stats.projector_write_published = true;
                                            ComputedPublished
                                        }
                                        Err(_) => {
                                            stats.projector_write_failed = true;
                                            ComputedUnpublished
                                        }
                                    };
                                    success_outcome(
                                        item,
                                        mode,
                                        raw_disposition,
                                        projector_disposition,
                                        article,
                                        result_identity.canonical(),
                                        stats,
                                    )
                                }
                                _ => success_outcome(
                                    item,
                                    mode,
                                    raw_disposition,
                                    ComputedUnpublished,
                                    article,
                                    result_identity.canonical(),
                                    stats,
                                ),
                            }
                        }
                    }
                }
            }
        }
        CacheMode::Refresh => {
            // Bypasses hits and replaces both stages; the backend is
            // always called exactly once.
            let mut stats = DocumentStats::default();
            let envelope = match fetch_timed(backend, request, &mut stats).await {
                Ok(envelope) => envelope,
                Err(failure) => {
                    return failure_outcome(
                        item,
                        mode,
                        ComputedUncached,
                        ComputedUncached,
                        ParseDocumentFailure::Backend(failure),
                        stats,
                    );
                }
            };
            let raw_key = raw_tei_stage_key_with_backend_version(request, backend_version);
            let raw_sha256 = sha256_hex(envelope.raw_tei());
            let raw_disposition = match cache
                .write_raw(
                    &raw_key,
                    &RawTeiEnvelopePayload::build(&envelope, &result_identity).encode(),
                )
                .await
            {
                Ok(()) => {
                    stats.raw_write_published = true;
                    ComputedPublished
                }
                Err(_) => {
                    stats.raw_write_failed = true;
                    ComputedUnpublished
                }
            };
            let projector_key = project_article_stage_key_with_backend_version(
                &paper_id,
                &pdf_sha256,
                &raw_sha256,
                projector_schema_fingerprint,
                backend_version,
            );
            let article = project_timed(&paper_id, pdf_hash, &envelope, &mut stats);
            let projector_payload = ProjectArticlePayload::build(
                &article,
                projector_schema_fingerprint,
                &raw_sha256,
                &paper_id,
                &pdf_sha256,
            );
            let projector_disposition = match cache
                .write_projected(&projector_key, &projector_payload.encode())
                .await
            {
                Ok(()) => {
                    stats.projector_write_published = true;
                    ComputedPublished
                }
                Err(_) => {
                    stats.projector_write_failed = true;
                    ComputedUnpublished
                }
            };
            success_outcome(
                item,
                mode,
                raw_disposition,
                projector_disposition,
                article,
                result_identity.canonical(),
                stats,
            )
        }
    }
}

struct WorkerContext<B: ParseBackend, C: ParseArtifactCache> {
    backend: Arc<B>,
    cache: Arc<C>,
    projector_schema_fingerprint: String,
    backend_version: Option<String>,
    mode: CacheMode,
    shared: Arc<RunShared>,
    rx: Arc<Mutex<mpsc::Receiver<QueuedItem>>>,
    semaphore: Arc<Semaphore>,
    shared_execution: Arc<Semaphore>,
    sink: Arc<dyn ParseBatchResultSink>,
    checkpoint: Arc<dyn BatchCheckpoint>,
}

impl<B: ParseBackend, C: ParseArtifactCache> WorkerContext<B, C> {
    async fn work(self) {
        loop {
            let queued = {
                let mut rx = self.rx.lock().await;
                rx.recv().await
            };
            let Some(queued) = queued else {
                break;
            };
            self.shared.counters.note_queue_depth(-1);
            // The in-flight permit covers the whole durable write path
            // (execution + sink record + acknowledgment), so bounded
            // execution and durable prefix advancement stay in step.
            if self.semaphore.available_permits() == 0 {
                self.shared
                    .counters
                    .budget_waits
                    .fetch_add(1, Ordering::SeqCst);
            }
            let Ok(execution_permit) = self.semaphore.clone().acquire_owned().await else {
                break;
            };
            // All executors sharing a budget take the shared execution permit
            // after their local cap and before entering the backend. Both
            // permits are owned by this worker and therefore release on
            // normal completion or cancellation.
            if self.shared_execution.available_permits() == 0 {
                self.shared
                    .counters
                    .budget_waits
                    .fetch_add(1, Ordering::SeqCst);
            }
            let Ok(shared_execution_permit) = self.shared_execution.clone().acquire_owned().await
            else {
                break;
            };
            self.shared.counters.note_in_flight(1);
            self.execute_one(queued, execution_permit, shared_execution_permit)
                .await;
            self.shared.counters.note_in_flight(-1);
        }
    }

    async fn execute_one(
        &self,
        queued: QueuedItem,
        execution_permit: tokio::sync::OwnedSemaphorePermit,
        shared_execution_permit: tokio::sync::OwnedSemaphorePermit,
    ) {
        let execution = execute_document(
            self.backend.as_ref(),
            self.cache.as_ref(),
            &self.projector_schema_fingerprint,
            self.backend_version.as_deref(),
            self.mode,
            &queued.item,
        )
        .await;
        let outcome = execution.outcome;
        let stats = execution.stats;
        // Execution capacity is independent from the ordinal publication
        // window. Once computation is terminal, release it before waiting for
        // a publication gap; the queued item's permit remains held until the
        // sink and checkpoint path has finished.
        drop(execution_permit);
        drop(shared_execution_permit);
        let succeeded = outcome.is_success();
        tracing::debug!(
            ordinal = outcome.ordinal(),
            paper_id = outcome.paper_id(),
            mode = %self.mode.as_str(),
            queue_wait_ms = elapsed_millis(queued.enqueued_at),
            backend_ms = stats.backend_ms,
            projection_ms = stats.projection_ms,
            succeeded,
            "parse batch document terminal"
        );
        let counters = &self.shared.counters;
        counters
            .backend_calls
            .fetch_add(stats.backend_calls, Ordering::SeqCst);
        if stats.raw_hit {
            counters.raw_hits.fetch_add(1, Ordering::SeqCst);
            if stats.raw_legacy_hit {
                counters.raw_legacy_hits.fetch_add(1, Ordering::SeqCst);
            }
        }
        if stats.raw_write_published {
            counters.raw_writes.fetch_add(1, Ordering::SeqCst);
        }
        if stats.raw_write_failed {
            counters.raw_write_failures.fetch_add(1, Ordering::SeqCst);
        }
        if stats.projector_hit {
            counters.projector_hits.fetch_add(1, Ordering::SeqCst);
        }
        if stats.projector_write_published {
            counters.projector_writes.fetch_add(1, Ordering::SeqCst);
        }
        if stats.projector_write_failed {
            counters
                .projector_write_failures
                .fetch_add(1, Ordering::SeqCst);
        }
        // Computation may finish out of order, but durable publication is
        // strictly ordinal. Holding this lock across the sink acknowledgment
        // intentionally backpressures later results when an earlier ordinal
        // is parked or interrupted; they remain resumable rather than being
        // exposed beyond a gap in the durable prefix.
        let ordinal = queued.item.ordinal();
        let mut publication = self.shared.publication.lock().await;
        publication.pending.insert(ordinal, outcome);
        let pending = publication.pending.len();
        self.shared
            .counters
            .publication_outstanding
            .store(pending, Ordering::SeqCst);
        self.shared
            .counters
            .max_publication_outstanding
            .fetch_max(pending, Ordering::SeqCst);
        self.shared
            .counters
            .max_reorder_depth
            .fetch_max(pending.saturating_sub(1), Ordering::SeqCst);
        loop {
            let next_ordinal = publication.next_ordinal;
            let Some(next_outcome) = publication.pending.remove(&next_ordinal) else {
                break;
            };
            // Durability is decided by the outcome's own stage dispositions,
            // so the cursor can only cover content a later run can read again.
            // A FAILED document is exempt: it produced no content by
            // definition and re-attempting it would reproduce the same
            // deterministic failure, so the established "processed" semantics
            // stand for it. The hazard is a document that SUCCEEDED while its
            // content was never published (cache off, or a read-only miss).
            // Read before the sink takes ownership.
            let recoverable =
                !next_outcome.is_success() || next_outcome.dispositions().is_durably_recoverable();
            match self.sink.record(next_outcome).await {
                Ok(()) => {
                    acknowledge(&self.shared, &*self.checkpoint, next_ordinal, recoverable).await;
                }
                Err(_) => {
                    counters.sink_errors.fetch_add(1, Ordering::SeqCst);
                }
            }
            publication.next_ordinal += 1;
            let pending = publication.pending.len();
            self.shared
                .counters
                .publication_outstanding
                .store(pending, Ordering::SeqCst);
        }
        drop(publication);
        if succeeded {
            counters.succeeded.fetch_add(1, Ordering::SeqCst);
        } else {
            counters.failed.fetch_add(1, Ordering::SeqCst);
        }
        // Release the ordinal publication window explicitly: the permit is
        // held from feeder admission up to (and including) this item's durable
        // sink record and checkpoint acknowledgment. Making the release a read
        // of the field keeps that lifetime enforced and greppable instead of
        // relying on an implicit end-of-scope drop.
        drop(queued.publication_permit);
        drop(queued.shared_publication_permit);
    }
}

/// Process-local shared limits for concurrently running parse batches.
///
/// The two semaphores intentionally represent different resource classes:
/// publication slots bound admitted results retained until ordered durable
/// publication, while execution slots bound active backend/projection work.
/// The value is explicitly injected through [`Arc`] and has no global or
/// cross-process scope.
#[derive(Debug)]
pub struct ParseBatchBudget {
    publication: Arc<Semaphore>,
    execution: Arc<Semaphore>,
    publication_capacity: NonZeroUsize,
    execution_capacity: NonZeroUsize,
}

impl ParseBatchBudget {
    /// Create a shared process-local budget with nonzero publication and
    /// execution capacities.
    pub fn new(publication: NonZeroUsize, execution: NonZeroUsize) -> Self {
        Self {
            publication: Arc::new(Semaphore::new(publication.get())),
            execution: Arc::new(Semaphore::new(execution.get())),
            publication_capacity: publication,
            execution_capacity: execution,
        }
    }

    /// Configured shared admitted-to-publication capacity.
    pub const fn publication_capacity(&self) -> NonZeroUsize {
        self.publication_capacity
    }

    /// Configured shared active-execution capacity.
    pub const fn execution_capacity(&self) -> NonZeroUsize {
        self.execution_capacity
    }
}

/// Bounded batch executor (S06/T02): drives one admitted [`ParseBatch`] over
/// the injected [`ParseBackend`] and the two-stage injected
/// [`ParseArtifactCache`].
///
/// Per document, exactly one typed terminal outcome is produced and
/// streamed through the sink; the four cache modes change only cache COST
/// (the mode truth table), never the semantic output — successful documents
/// carry byte-equal `ParsedArticle` payloads across all modes. Backend and
/// cache failures are per-document; the run continues; a typed backend
/// failure is exactly one backend invocation (no call amplification).
///
/// The cursor moves only through the durably acknowledged contiguous
/// terminal prefix (sink acknowledgments + checkpoint saves under the
/// advancement lock), so an interrupted run never skips an unwritten
/// ordinal.
pub struct ParseBatchExecutor<B: ParseBackend, C: ParseArtifactCache> {
    backend: Arc<B>,
    cache: Arc<C>,
    projector_schema_fingerprint: String,
    /// Maximum number of admitted-but-not-yet-published ordinal results.
    /// This is executor tuning, not batch input identity or wire schema.
    publication_window: Option<NonZeroUsize>,
    shared_budget: Option<Arc<ParseBatchBudget>>,
}

impl<B: ParseBackend + 'static, C: ParseArtifactCache + 'static> ParseBatchExecutor<B, C> {
    /// Compose the executor over a backend and a two-stage artifact cache.
    pub fn new(backend: B, cache: C) -> Self {
        Self {
            backend: Arc::new(backend),
            cache: Arc::new(cache),
            projector_schema_fingerprint: PROJECT_ARTICLE_SCHEMA_FINGERPRINT.to_string(),
            publication_window: None,
            shared_budget: None,
        }
    }

    /// Share process-local publication and execution capacity with other
    /// executors. Local limits remain effective for this run as well.
    pub fn with_shared_budget(mut self, budget: Arc<ParseBatchBudget>) -> Self {
        self.shared_budget = Some(budget);
        self
    }

    /// The explicitly injected process-local shared budget, if any.
    pub fn shared_budget(&self) -> Option<Arc<ParseBatchBudget>> {
        self.shared_budget.clone()
    }

    /// Set the ordinal publication window. The permit is held from feeder
    /// admission through sink acknowledgment and is excluded from identity.
    pub fn with_publication_window(mut self, window: NonZeroUsize) -> Self {
        self.publication_window = Some(window);
        self
    }

    /// Explicit ordinal publication-window override, if configured. When it
    /// is absent, `run` derives the finite default from the batch's
    /// `max_in_flight` limit.
    pub const fn publication_window(&self) -> Option<NonZeroUsize> {
        self.publication_window
    }

    /// Override the projector schema fingerprint (rotation seam: a rotated
    /// fingerprint orphans projection-stage entries while raw TEI survives).
    pub fn with_projector_schema_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.projector_schema_fingerprint = fingerprint.into();
        self
    }

    /// The effective projector schema fingerprint.
    pub fn projector_schema_fingerprint(&self) -> &str {
        &self.projector_schema_fingerprint
    }

    /// Run the admitted batch to completion, streaming terminal outcomes
    /// through `sink` and advancing `checkpoint` through the durably
    /// acknowledged contiguous terminal prefix.
    pub async fn run(
        &self,
        batch: ParseBatch,
        sink: Arc<dyn ParseBatchResultSink>,
        checkpoint: Arc<dyn BatchCheckpoint>,
    ) -> ParseBatchRunSummary {
        let started = Instant::now();
        let limits = *batch.limits();
        let mode = batch.mode();
        let backend_version = batch.backend_version().map(str::to_string);
        let input_fingerprint = batch.input_fingerprint().to_string();
        let resumed_from = batch.cursor().next_ordinal();
        let admitted = batch.total_documents() as u32;
        let skipped_on_resume = resumed_from.min(admitted);
        let queue_capacity = limits.queue_capacity().get();
        let max_in_flight = limits.max_in_flight().get();

        tracing::info!(
            mode = %mode.as_str(),
            input_fingerprint = %input_fingerprint,
            admitted,
            resumed_from_ordinal = resumed_from,
            workers = limits.workers().get(),
            queue_capacity,
            max_in_flight,
            "parse batch run starting"
        );

        let shared = Arc::new(RunShared {
            input_fingerprint: input_fingerprint.clone(),
            tracker: Mutex::new(CursorTracker {
                acked: BTreeSet::new(),
                acked_next: resumed_from,
                saved_next: resumed_from,
                checkpoint_broken: false,
                unrecoverable_at: None,
            }),
            publication: Mutex::new(PublicationState {
                next_ordinal: resumed_from,
                pending: BTreeMap::new(),
            }),
            counters: RunCounters::default(),
        });

        // Independent bounds: the bounded admission queue caps how much
        // work is buffered; the semaphore caps simultaneously executing
        // documents.
        let (tx, rx) = mpsc::channel::<QueuedItem>(queue_capacity);
        let rx = Arc::new(Mutex::new(rx));
        let semaphore = Arc::new(Semaphore::new(max_in_flight));
        // Keep at most this many admitted results alive between feeder and
        // durable publication. The default is deliberately finite and
        // derived from the existing execution bound without changing the
        // ParseBatchLimits v1 wire contract.
        let publication_window = self.publication_window.unwrap_or(limits.max_in_flight());
        let shared_budget = self.shared_budget.clone().unwrap_or_else(|| {
            Arc::new(ParseBatchBudget::new(
                publication_window,
                NonZeroUsize::new(max_in_flight).expect("batch limit is nonzero"),
            ))
        });
        let local_publication_semaphore = Arc::new(Semaphore::new(publication_window.get()));
        let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(limits.workers().get());
        for _ in 0..limits.workers().get() {
            workers.push(tokio::spawn(
                WorkerContext {
                    backend: self.backend.clone(),
                    cache: self.cache.clone(),
                    projector_schema_fingerprint: self.projector_schema_fingerprint.clone(),
                    backend_version: backend_version.clone(),
                    mode,
                    shared: shared.clone(),
                    rx: rx.clone(),
                    semaphore: semaphore.clone(),
                    shared_execution: shared_budget.execution.clone(),
                    sink: sink.clone(),
                    checkpoint: checkpoint.clone(),
                }
                .work(),
            ));
        }

        // Feeder: admits items at/after the resume cursor into the bounded
        // queue. Items before the cursor are already terminal from a prior
        // run — skipped, never re-executed.
        for item in batch.into_items().into_iter().skip(resumed_from as usize) {
            // Reserve before enqueue so a slow ordinal zero bounds both the
            // queue and the completed-but-unpublished outcome set. The
            // permit travels with the item until its sink acknowledgment.
            if local_publication_semaphore.available_permits() == 0 {
                shared.counters.budget_waits.fetch_add(1, Ordering::SeqCst);
            }
            let Ok(publication_permit) = local_publication_semaphore.clone().acquire_owned().await
            else {
                break;
            };
            if shared_budget.publication.available_permits() == 0 {
                shared.counters.budget_waits.fetch_add(1, Ordering::SeqCst);
            }
            let Ok(shared_publication_permit) =
                shared_budget.publication.clone().acquire_owned().await
            else {
                break;
            };
            shared.counters.note_queue_depth(1);
            if tx
                .send(QueuedItem {
                    item,
                    enqueued_at: Instant::now(),
                    publication_permit,
                    shared_publication_permit,
                })
                .await
                .is_err()
            {
                break;
            }
        }
        drop(tx);
        // Await workers during a normal run. If this future is dropped (for
        // example, a process-death simulation after durable work was started),
        // JoinHandle drop detaches the workers instead of aborting them. They
        // finish already-admitted items, observe the closed channel, release
        // their permits and exit without leaving detached capacity holders.
        for worker in workers {
            let _ = worker.await;
        }

        let saved_next = shared.tracker.lock().await.saved_next;
        let completed = shared.counters.succeeded.load(Ordering::SeqCst)
            + shared.counters.failed.load(Ordering::SeqCst);
        let summary = ParseBatchRunSummary {
            schema_version: PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION,
            mode,
            limits,
            input_fingerprint: input_fingerprint.clone(),
            resumed_from_ordinal: resumed_from,
            cursor: BatchCursor::at(input_fingerprint, saved_next),
            admitted,
            skipped_on_resume,
            completed: u32::try_from(completed).unwrap_or(u32::MAX),
            succeeded: u32::try_from(shared.counters.succeeded.load(Ordering::SeqCst))
                .unwrap_or(u32::MAX),
            failed: u32::try_from(shared.counters.failed.load(Ordering::SeqCst))
                .unwrap_or(u32::MAX),
            backend_calls: shared.counters.backend_calls.load(Ordering::SeqCst),
            raw_hits: shared.counters.raw_hits.load(Ordering::SeqCst),
            raw_legacy_hits: shared.counters.raw_legacy_hits.load(Ordering::SeqCst),
            raw_writes: shared.counters.raw_writes.load(Ordering::SeqCst),
            raw_write_failures: shared.counters.raw_write_failures.load(Ordering::SeqCst),
            projector_hits: shared.counters.projector_hits.load(Ordering::SeqCst),
            projector_writes: shared.counters.projector_writes.load(Ordering::SeqCst),
            projector_write_failures: shared
                .counters
                .projector_write_failures
                .load(Ordering::SeqCst),
            configured_publication_capacity: shared_budget.publication_capacity().get(),
            configured_execution_capacity: shared_budget.execution_capacity().get(),
            max_in_flight_observed: shared.counters.max_in_flight.load(Ordering::SeqCst),
            max_queue_depth_observed: shared.counters.max_queue_depth.load(Ordering::SeqCst),
            max_publication_outstanding_observed: shared
                .counters
                .max_publication_outstanding
                .load(Ordering::SeqCst),
            max_reorder_depth_observed: shared.counters.max_reorder_depth.load(Ordering::SeqCst),
            budget_waits: shared.counters.budget_waits.load(Ordering::SeqCst),
            budget_scope: "process-local",
            sink_errors: shared.counters.sink_errors.load(Ordering::SeqCst),
            checkpoint_errors: shared.counters.checkpoint_errors.load(Ordering::SeqCst),
        };
        tracing::info!(
            mode = %summary.mode.as_str(),
            input_fingerprint = %summary.input_fingerprint,
            admitted = summary.admitted,
            skipped_on_resume = summary.skipped_on_resume,
            completed = summary.completed,
            succeeded = summary.succeeded,
            failed = summary.failed,
            backend_calls = summary.backend_calls,
            raw_hits = summary.raw_hits,
            projector_hits = summary.projector_hits,
            cursor_next_ordinal = summary.cursor.next_ordinal(),
            duration_ms = elapsed_millis(started),
            "parse batch run complete"
        );
        summary
    }
}
