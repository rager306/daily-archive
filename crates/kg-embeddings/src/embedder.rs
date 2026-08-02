//! Embedder trait — text → vector embedding.
//!
//! Universal trait for text embedding. Backed by FdApi (TEI/bge-m3)
//! or any compatible service. The RuVector bridge (feature-gated)
//! extends this with graph-aware embeddings.

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("Embedding failed: {0}")]
    EmbedFailed(String),
    #[error("Service unavailable: {0}")]
    Unavailable(String),
}

pub type EmbedResult<T> = Result<T, EmbedderError>;

/// Text → vector embedding port.
/// Backed by fd_api TEI (bge-m3) or any compatible embedding service.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> EmbedResult<Vec<f32>>;

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>>;

    fn dimensions(&self) -> usize;

    fn model_id(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EmbedderError Display formats correctly.
    #[test]
    fn test_embedder_error_display() {
        let e = EmbedderError::EmbedFailed("timeout".into());
        assert_eq!(format!("{e}"), "Embedding failed: timeout");

        let e2 = EmbedderError::Unavailable("service down".into());
        assert_eq!(format!("{e2}"), "Service unavailable: service down");
    }

    /// EmbedderError variants are Debug-printable.
    #[test]
    fn test_embedder_error_debug() {
        let e = EmbedderError::EmbedFailed("err".into());
        let s = format!("{e:?}");
        assert!(s.contains("EmbedFailed"));
    }
}
