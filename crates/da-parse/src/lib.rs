//! Scholarly PDF fulltext parse plane.
//!
//! This crate owns Java GROBID HTTP, this-cut TEI IR, and the `ParserPort`
//! projector onto `da_ports::parser::ParsedArticle`. HTML parse, PDF
//! classification, and XY-Cut stay in `da-adapters`.

mod http;
mod ir;
mod lift;
mod profile;
mod projector;

pub use http::GrobidParser;
pub use ir::TeiParseIr;
pub use profile::GrobidProfile;

#[cfg(test)]
mod real_tei_tests;
#[cfg(test)]
mod tests;
