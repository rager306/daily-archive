//! Scholarly PDF fulltext parse plane.
//!
//! This crate owns Java GROBID HTTP, this-cut TEI IR, and the `ParserPort`
//! projector onto `da_ports::parser::ParsedArticle`. HTML parse, PDF
//! classification, and XY-Cut stay in `da-adapters`.
//!
//! Since M464-z84in2 S02 the transport is behind the provider-neutral
//! `ParseBackend` seam (`TeiEnvelope` in, exact backend/profile identity out),
//! so cached-TEI replay and the GROBID HTTP path share one contract (R105).
//! [`ScholarlyParser`] closes the seams into the runtime
//! backend→envelope→lift→project composition; [`GrobidParser`] stays the
//! compatibility facade for existing `Arc<dyn ParserPort>` callers. Since
//! S04/T01 the request carries the serde-versioned V1 fulltext dialect and
//! the transport returns the closed typed [`ParseBackendFailure`] vocabulary
//! (R107); the compatibility `ParserError` mapping stays exactly once at the
//! facade boundary. Since S05/T01 the same seam covers the closed header
//! and references operations: versioned per-endpoint request dialects and
//! endpoint result identities distinguish every operation and effective
//! request without touching the fulltext facade or the D204 pins (R113).
//! Since S06/T01 the frozen bounded-batch and cache contract lives in
//! [`batch`] / [`cache`]: the closed four-mode cache vocabulary with its
//! truth table, NonZero bounded limits, ordered items over
//! [`ParseBackendRequest`], the ordered input fingerprint, the schema-bound
//! resume cursor, per-stage dispositions and one typed terminal outcome per
//! admitted document — still without touching `ParserPort` or introducing
//! any path/env/retry surface.

mod backend;
mod batch;
mod cache;
mod conservation;
mod envelope;
mod http;
mod ir;
mod lift;
mod native_affiliation;
mod native_date;
mod native_personal_name;
mod parser;
mod profile;
mod projector;
mod transport;

pub use backend::{
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, GROBID_HTTP_BACKEND_ID, ParseBackend,
    ParseBackendRequest,
};
pub use batch::{
    BatchCheckpoint, BatchCursor, PARSE_BATCH_CURSOR_SCHEMA_VERSION,
    PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION, PARSE_BATCH_SCHEMA_VERSION, ParseBatch,
    ParseBatchAdmissionError, ParseBatchCollector, ParseBatchCursorError, ParseBatchExecutor,
    ParseBatchItem, ParseBatchLimits, ParseBatchMemoryCheckpoint, ParseBatchResultSink,
    ParseBatchRunSummary, ParseBatchSinkError, ParseDocumentFailure, ParseDocumentOutcome,
    ParseDocumentSuccess, ordered_input_fingerprint,
};
pub use cache::{
    CacheIntegrityCode, CacheMode, CacheStageDisposition, MEMORY_TIER_DEFAULT_CAPACITY,
    PARSE_CACHE_DISPOSITION_SCHEMA_VERSION, PARSE_CACHE_FAILURE_SCHEMA_VERSION,
    PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION, PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    PROJECT_ARTICLE_STAGE_ID, ParseArtifactCache, ParseCacheDispositions, ParseCacheFailure,
    ParseCacheStage, ProjectArticlePayload, RAW_TEI_PAYLOAD_SCHEMA_VERSION, RAW_TEI_STAGE_ID,
    RawStageRead, RawTeiEnvelopePayload, TwoStageParseCache, project_article_stage_key,
    raw_tei_stage_key,
};
pub use conservation::{
    ByteRange, ConservationDelta, ConservationEngine, ConservationEntry, ConservationFamily,
    ConservationIdentity, ConservationInventory, ConservationObservation, ConservationReasonCode,
    ConservationSubtype, ConservationVerdict, FamilyDeltaCounts,
};
pub use envelope::{TeiEnvelope, TeiIdentity, TeiResultIdentity};
pub use http::{GrobidHttpBackend, GrobidParser};
pub use ir::TeiParseIr;
pub use lift::TeiLifter;
pub use native_affiliation::{
    AffiliationComponentKind, AffiliationSpan, NATIVE_AFFILIATION_INPUT_BYTE_CAP,
    NATIVE_AFFILIATION_LINK_SCHEMA_VERSION, NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
    NATIVE_AFFILIATION_PARSER_ID, NATIVE_AFFILIATION_PARSER_VERSION, NativeAffiliationAmbiguous,
    NativeAffiliationAuthorEvidence, NativeAffiliationLink, NativeAffiliationLinkError,
    NativeAffiliationLinkOutcome, NativeAffiliationLinker, NativeAffiliationMalformed,
    NativeAffiliationNesting, NativeAffiliationOccurrence, NativeAffiliationOutcome,
    NativeAffiliationParser, NativeAffiliationUnsupported, RecognizedAffiliationComponent,
};
pub use native_date::{
    DatePrecision, NATIVE_DATE_INPUT_BYTE_CAP, NATIVE_DATE_OUTCOME_SCHEMA_VERSION,
    NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION, NativeDateAmbiguous, NativeDateMalformed,
    NativeDateOutcome, NativeDateParser, NativeDateUnsupported, RecognizedDate,
};
pub use native_personal_name::{
    NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP, NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
    NATIVE_PERSONAL_NAME_PARSER_ID, NATIVE_PERSONAL_NAME_PARSER_VERSION,
    NativePersonalNameAmbiguous, NativePersonalNameMalformed, NativePersonalNameOutcome,
    NativePersonalNameParser, NativePersonalNameUnsupported, PersonalNameContext,
    RecognizedPersonalName,
};
pub use parser::{ScholarlyParser, lift_and_project_envelope};
pub use profile::GrobidProfile;
pub use transport::{
    CitationsConsolidation, Consolidation, CoordinateFamily, CoordinateSet,
    FULLTEXT_REQUEST_SCHEMA_VERSION, FailureCode, FailurePhase, FulltextDialectV1,
    FulltextRequestVersioned, FundersConsolidation, HEADER_REQUEST_SCHEMA_VERSION,
    HeaderConsolidation, HeaderDialectV1, HeaderRequestVersioned, InvalidConsolidation,
    InvalidPageWindow, LimitKind, PARSE_FAILURE_SCHEMA_VERSION, PageWindow, ParseBackendFailure,
    ParseEndpoint, ParseResultKind, REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1,
    ReferencesRequestVersioned, RetryAfter,
};

#[cfg(test)]
mod real_tei_tests;
#[cfg(test)]
mod tests;
