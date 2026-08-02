//! Deterministic mock Embedder for tests.
//!
//! Produces stable vectors from text using a simple hash-based LCG
//! (borrowed from ADR-051 §3 deterministic vector pattern). Same text
//! always yields the same vector; different texts yield different
//! vectors. Zero external dependencies.

use async_trait::async_trait;

use crate::embedder::{EmbedResult, Embedder, EmbedderError};

/// Deterministic mock embedder — hash-based stable vectors.
///
/// No network calls. Same text → same vector. The vector dimension
/// is configurable at construction time.
pub struct MockEmbedder {
    dims: usize,
}

impl MockEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new(8)
    }
}

/// FNV-1a hash — stable across runs and platforms.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// LCG (borrowed from ADR-051 §3) seeded from a hash.
fn lcg_vector(seed: u64, dims: usize) -> Vec<f32> {
    let mut state = seed;
    let mut vec = Vec::with_capacity(dims);
    for _ in 0..dims {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Map high 24 bits to [0.0, 1.0) — avoids NaN/inf from f32 division.
        let bits24 = ((state >> 40) as u32) & 0x00FF_FFFF;
        vec.push(bits24 as f32 / 0x0100_0000u32 as f32);
    }
    vec
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> EmbedResult<Vec<f32>> {
        if self.dims == 0 {
            return Err(EmbedderError::EmbedFailed(
                "dimensions must be > 0".to_string(),
            ));
        }
        let seed = fnv1a(text.as_bytes());
        Ok(lcg_vector(seed, self.dims))
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.embed(text).await?);
        }
        Ok(out)
    }

    fn dimensions(&self) -> usize {
        self.dims
    }

    fn model_id(&self) -> &str {
        "mock-embedder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deterministic_same_text() {
        let e = MockEmbedder::new(8);
        let a = e.embed("hello").await.unwrap();
        let b = e.embed("hello").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn test_different_text_different_vector() {
        let e = MockEmbedder::new(8);
        let a = e.embed("hello").await.unwrap();
        let b = e.embed("world").await.unwrap();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn test_dimensions_match() {
        let e = MockEmbedder::new(16);
        let v = e.embed("test").await.unwrap();
        assert_eq!(v.len(), 16);
    }
}
