//! da-adapters — port implementations.
//!
//! ADR-037 §2: Adapters implement Ports using infrastructure.
//! ADR-040 §1: Samyama Graph (Tier 1) + external services.

pub mod fd_embedder;
pub mod grobid_parser;
pub mod samyama_graph;

pub use fd_embedder::FdApiEmbedder;
pub use grobid_parser::GrobidParser;
pub use samyama_graph::SamyamaGraphStore;
