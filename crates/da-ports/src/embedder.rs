//! Embedder port — re-exported from kg_embeddings::embedder (M290-j98lkq/S01).
//!
//! Universal Embedder trait + EmbedderError live in kg-embeddings
//! (no da-* deps). This module re-exports for backward compatibility.

pub use kg_embeddings::embedder::{Embedder, EmbedderError, EmbedResult};
