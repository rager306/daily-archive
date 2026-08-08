//! da-adapters — port implementations.
//!
//! ADR-037 §2: Adapters implement Ports using infrastructure.
//! ADR-040 §1: Samyama Graph (Tier 1) + external services.

pub mod anthropic_llm;
pub mod fd_embedder;
pub mod grobid_parser;
pub mod html_parser;
pub mod openalex_adapter;
pub mod pipeline_cache;
pub mod rule_extractor;
pub mod samyama_graph;

#[cfg(feature = "pdf-classifier")]
pub mod pdf_classifier;

pub use fd_embedder::FdApiEmbedder;
pub use grobid_parser::GrobidParser;
pub use html_parser::HtmlParser;

pub use openalex_adapter::OpenAlexHttpAdapter;
#[cfg(feature = "pdf-classifier")]
pub use pdf_classifier::PdfInspectorClassifier;
pub use pipeline_cache::FilePipelineCache;
pub use rule_extractor::RuleBasedExtractor;
pub use samyama_graph::SamyamaGraphStore;
