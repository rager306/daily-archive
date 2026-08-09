//! da-adapters — port implementations.
//!
//! ADR-037 §2: Adapters implement Ports using infrastructure.
//! ADR-040 §1: Samyama Graph (Tier 1) + external services.

pub mod anthropic_llm;
pub mod anthropic_multimodal;
pub mod assurance_store;
pub mod catalog_inventory;
pub mod catalog_mutation;
pub mod fd_embedder;
pub mod grobid_parser;
pub mod html_parser;
pub mod openalex_adapter;
pub mod pdffigures2;
pub mod pipeline_cache;
pub mod pipeline_stages;
pub mod rule_extractor;
pub mod samyama_graph;

#[cfg(feature = "pdf-classifier")]
pub mod pdf_classifier;

pub use anthropic_llm::AnthropicLlmClient;
pub use anthropic_multimodal::{
    AnthropicMultimodalClient, AnthropicMultimodalRoute, AnthropicRouteAuth,
};
pub use assurance_store::FilesystemAssuranceReportStore;
pub use catalog_inventory::FilesystemCatalogInventory;
pub use catalog_mutation::FilesystemCatalogMutation;
pub use fd_embedder::FdApiEmbedder;
pub use grobid_parser::{GrobidParser, GrobidProfile};
pub use html_parser::HtmlParser;

pub use openalex_adapter::OpenAlexHttpAdapter;
#[cfg(feature = "pdf-classifier")]
pub use pdf_classifier::PdfInspectorClassifier;
pub use pdffigures2::PdfFigures2Adapter;
pub use pipeline_cache::FilePipelineCache;
pub use pipeline_stages::{
    FigureDetectionStage, GrobidParseStage, HtmlParseStage, ParsedArticleEmbeddingStage,
    RuleExtractionStage,
};
pub use rule_extractor::RuleBasedExtractor;
pub use samyama_graph::SamyamaGraphStore;
