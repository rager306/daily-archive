//! Embedder port — text → vector embedding.

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
/// Backed by fd_api TEI (bge-m3) or OnnxEmbedder.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> EmbedResult<Vec<f32>>;

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>>;

    fn dimensions(&self) -> usize;

    fn model_id(&self) -> &str;
}
