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
//! compatibility facade for existing `Arc<dyn ParserPort>` callers.

mod backend;
mod conservation;
mod envelope;
mod http;
mod ir;
mod lift;
mod parser;
mod profile;
mod projector;

pub use backend::{
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, GROBID_HTTP_BACKEND_ID, ParseBackend,
    ParseBackendRequest,
};
pub use conservation::{
    ByteRange, ConservationDelta, ConservationEngine, ConservationEntry, ConservationFamily,
    ConservationIdentity, ConservationInventory, ConservationObservation, ConservationReasonCode,
    ConservationVerdict, FamilyDeltaCounts,
};
pub use envelope::{TeiEnvelope, TeiIdentity};
pub use http::{GrobidHttpBackend, GrobidParser};
pub use ir::TeiParseIr;
pub use lift::TeiLifter;
pub use parser::ScholarlyParser;
pub use profile::GrobidProfile;

#[cfg(test)]
mod real_tei_tests;
#[cfg(test)]
mod tests;
