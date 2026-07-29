//! da-adapters — port implementations.
//!
//! ADR-037 §2: Adapters implement Ports using infrastructure.
//! ADR-040 §1: Samyama Graph (Tier 1) + external services.

pub mod fd_embedder;
pub mod grobid_parser;
pub mod html_parser;
pub mod openalex_adapter;
pub mod rule_extractor;
pub mod samyama_graph;

pub use fd_embedder::FdApiEmbedder;
pub use grobid_parser::GrobidParser;
pub use html_parser::HtmlParser;
pub use openalex_adapter::OpenAlexHttpAdapter;
pub use rule_extractor::RuleBasedExtractor;
pub use samyama_graph::SamyamaGraphStore;
