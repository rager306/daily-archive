//! da-adapters — port implementations.
//!
//! ADR-037 §2: Adapters implement Ports using infrastructure.
//! ADR-040 §1: Samyama Graph (Tier 1) + external services.

pub mod agent_memory;
pub mod agent_read_plane;
pub mod anthropic_llm;
pub mod anthropic_multimodal;
pub mod arxiv_acquisition;
pub mod assurance_store;
pub mod catalog_html_acquisition;
pub mod catalog_inventory;
pub mod catalog_mutation;
pub mod context_conservation_policy;
pub mod fd_embedder;
pub mod figure_quote_policy;
pub mod fs_schema_objects;
pub mod grobid_parser;
pub mod html_parser;
pub mod http_fetch;
#[cfg(feature = "pdf-layout")]
pub mod layout_donors;
#[cfg(feature = "pdf-layout")]
pub mod layout_visual;
pub mod memory_pipeline_cache;
pub mod negative_memory;
pub use figure_quote_policy::load_figure_quote_policy;
pub mod model_registry;
pub mod openai_compatible_llm;
pub mod openai_multimodal_prompt;
pub mod openalex_adapter;
pub mod parse_cache;
#[cfg(feature = "redb")]
pub mod parse_cursor;
pub mod parse_experience_rvf;
pub mod pdffigures2;
pub mod pipeline_cache;
pub mod pipeline_ledger;
pub mod pipeline_stages;
pub mod prompt_execution;
pub mod prompt_multimodal_bridge;
pub mod prompt_task_registry;
pub mod revision_readiness_policy;
pub mod rule_extractor;
pub mod samyama_graph;
pub mod section_summary_policy;
pub mod tex_archive;
pub mod wasm_policy;

#[cfg(feature = "pdf-classifier")]
pub use pdf_inspector;
#[cfg(feature = "pdf-classifier")]
pub mod pdf_classifier;
#[cfg(feature = "pdf-classifier")]
pub mod region_text;
#[cfg(feature = "pdf-classifier")]
pub use region_text::PdfInspectorRegionText;

pub use agent_read_plane::{RemoteAgentReadPlane, result_ref};
pub use anthropic_llm::AnthropicLlmClient;
pub use anthropic_multimodal::{
    AnthropicMultimodalClient, AnthropicMultimodalRoute, AnthropicRouteAuth,
};
pub use arxiv_acquisition::ArxivSourceAcquisition;
pub use assurance_store::FilesystemAssuranceReportStore;
pub use catalog_html_acquisition::{CatalogHtmlAcquisition, load_toc, toc_unit_keys};
pub use catalog_inventory::FilesystemCatalogInventory;
pub use catalog_mutation::FilesystemCatalogMutation;
pub use fd_embedder::FdApiEmbedder;
pub use grobid_parser::{GrobidParser, GrobidProfile};
pub use html_parser::HtmlParser;
#[cfg(feature = "pdf-layout")]
pub use layout_donors::{
    LayoutDonor, LayoutDonorError, LayoutDonorLimits, LayoutShadowObservation,
    PdfInspectorLayoutDonor, PdfaltoLayoutDonor, XyCutLayoutDonor, parse_pdfalto_xml,
    parse_pdfalto_xml_shadow, reorder_shadow_with_xy_cut, reorder_with_xy_cut,
};
#[cfg(feature = "pdf-layout")]
pub use layout_visual::{
    MAX_VISUAL_PRIMITIVES, VISUAL_OBSERVATION_SCHEMA_VERSION, VisualAbsenceReason, VisualError,
    VisualObservation, VisualPrimitive, VisualPrimitiveKind, VisualPrimitiveState, VisualProvider,
};

pub use openalex_adapter::OpenAlexHttpAdapter;
pub use parse_experience_rvf::RvfFileStore;
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
pub use tex_archive::{FilesystemTexAnalysisReportStore, FilesystemTexArchive};
