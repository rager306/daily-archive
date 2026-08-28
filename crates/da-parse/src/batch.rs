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

use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroUsize;

use da_ports::parser::ParsedArticle;
use serde::de::{self, Deserializer};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::backend::ParseBackendRequest;
use crate::cache::{CacheMode, ParseCacheDispositions, ParseCacheFailure};
use crate::envelope::TeiResultIdentity;
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
/// struct, not a hand-maintained field list.
fn endpoint_dialect_wire(request: &ParseBackendRequest) -> String {
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
}

impl ParseBatch {
    /// Admit an ordered document set under `mode` and `limits`.
    ///
    /// Validation order: non-empty → document bound → empty ids →
    /// duplicates → cursor. Admission assigns stable ordinals `0..n` in
    /// input order and stamps the ordered input fingerprint; a provided
    /// resume cursor must match that fingerprint or admission fails before
    /// any document work.
    pub fn admit(
        mode: CacheMode,
        limits: ParseBatchLimits,
        requests: Vec<ParseBackendRequest>,
        resume_cursor: Option<BatchCursor>,
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
        let items: Vec<ParseBatchItem> = requests
            .into_iter()
            .enumerate()
            .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request))
            .collect();
        let input_fingerprint = ordered_input_fingerprint(&items);
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
        })
    }

    /// Cache mode of this run (cost strategy — never part of the identity).
    pub const fn mode(&self) -> CacheMode {
        self.mode
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
