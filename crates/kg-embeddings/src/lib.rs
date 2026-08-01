//! kg-embeddings — Universal vector embeddings.
//!
//! Text → vector embedding trait, an FdApi (TEI/bge-m3) adapter, and
//! a RuVector bridge stub for graph-aware embeddings (PPR, GNN).
//! Zero project-specific dependencies — works for any domain.

pub mod embedder;
pub mod fd_api;
pub mod ruvector;

pub use embedder::{Embedder, EmbedderError, EmbedResult};
pub use fd_api::FdApiEmbedder;
pub use ruvector::RuVectorBridge;
