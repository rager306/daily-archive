//! fd_api embedder adapter — implements Embedder port.
//!
//! ADR-037 §5: TEI embedder (fd_api) at http://127.0.0.1:8000.
//! Model: deepvk/USER-bge-m3 (1024 dimensions).
//!
//! GOTCHA (from Python experience): stale/wrong FD_API_KEY causes
//! silent zero-vector degradation. Always verify with live embed smoke test.

use async_trait::async_trait;
use da_ports::embedder::{Embedder, EmbedderError, EmbedResult};
use serde::{Deserialize, Serialize};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8000/v1/embeddings";
const DEFAULT_MODEL: &str = "deepvk/USER-bge-m3";
const DEFAULT_DIMENSIONS: usize = 1024;

/// fd_api TEI embedder adapter.
#[derive(Clone)]
pub struct FdApiEmbedder {
    endpoint: String,
    api_key: Option<String>,
    model: String,
    dimensions: usize,
    client: reqwest::Client,
}

impl FdApiEmbedder {
    pub fn new(endpoint: Option<&str>, api_key: Option<&str>) -> Self {
        Self {
            endpoint: endpoint.unwrap_or(DEFAULT_ENDPOINT).to_string(),
            api_key: api_key.map(|s| s.to_string()),
            model: DEFAULT_MODEL.to_string(),
            dimensions: DEFAULT_DIMENSIONS,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = std::env::var("FD_EMBEDDINGS_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
        let api_key = std::env::var("FD_API_KEY").ok().filter(|s| !s.is_empty());
        Self::new(Some(&endpoint), api_key.as_deref())
    }
}

/// fd_api request shape.
#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: EmbedInput,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum EmbedInput {
    Single(String),
    Batch(Vec<String>),
}

/// fd_api response shape.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Debug, Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for FdApiEmbedder {
    async fn embed(&self, text: &str) -> EmbedResult<Vec<f32>> {
        let req = EmbedRequest {
            model: self.model.clone(),
            input: EmbedInput::Single(text.to_string()),
        };

        let mut builder = self.client.post(&self.endpoint).json(&req);
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| EmbedderError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::EmbedFailed(format!("HTTP {status}: {text}")));
        }

        let embed_resp: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| EmbedderError::EmbedFailed(format!("parse: {e}")))?;

        let vec = embed_resp.data.into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| EmbedderError::EmbedFailed("empty response".into()))?;

        // GOTCHA check: zero-vector detection
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 0.001 {
            return Err(EmbedderError::EmbedFailed(
                "zero-vector detected (check FD_API_KEY — stale key causes silent degradation)"
                    .into()
            ));
        }

        Ok(vec)
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        let req = EmbedRequest {
            model: self.model.clone(),
            input: EmbedInput::Batch(texts.iter().map(|s| s.to_string()).collect()),
        };

        let mut builder = self.client.post(&self.endpoint).json(&req);
        if let Some(ref key) = self.api_key {
            builder = builder.bearer_auth(key);
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| EmbedderError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::EmbedFailed(format!("HTTP {status}: {text}")));
        }

        let embed_resp: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| EmbedderError::EmbedFailed(format!("parse: {e}")))?;

        Ok(embed_resp.data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_env_defaults() {
        let emb = FdApiEmbedder::from_env();
        assert_eq!(emb.dimensions(), 1024);
        assert!(emb.model_id().contains("bge-m3"));
    }
}
