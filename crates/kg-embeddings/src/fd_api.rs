//! FdApi TEI embedder adapter — implements Embedder port.
//!
//! Backed by fd_api TEI service (bge-m3, 1024 dimensions).
//! Endpoint configurable via FD_EMBEDDINGS_ENDPOINT env var.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::embedder::{EmbedResult, Embedder, EmbedderError};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8000/v1/embeddings";
const DEFAULT_MODEL: &str = "deepvk/USER-bge-m3";
const DEFAULT_DIMENSIONS: usize = 1024;

/// FdApi TEI embedder adapter.
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

        let resp = self
            .client
            .post(&self.endpoint)
            .header(
                "Authorization",
                self.api_key
                    .as_ref()
                    .map(|k| format!("Bearer {k}"))
                    .unwrap_or_default(),
            )
            .json(&req)
            .send()
            .await
            .map_err(|e| EmbedderError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::EmbedFailed(format!(
                "fd_api returned {status}: {body}"
            )));
        }

        let result: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| EmbedderError::EmbedFailed(e.to_string()))?;

        result
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| EmbedderError::EmbedFailed("no embedding in response".into()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        let req = EmbedRequest {
            model: self.model.clone(),
            input: EmbedInput::Batch(texts.iter().map(|s| s.to_string()).collect()),
        };

        let resp = self
            .client
            .post(&self.endpoint)
            .header(
                "Authorization",
                self.api_key
                    .as_ref()
                    .map(|k| format!("Bearer {k}"))
                    .unwrap_or_default(),
            )
            .json(&req)
            .send()
            .await
            .map_err(|e| EmbedderError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(EmbedderError::EmbedFailed(format!(
                "fd_api returned {status}: {body}"
            )));
        }

        let result: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| EmbedderError::EmbedFailed(e.to_string()))?;

        Ok(result.data.into_iter().map(|d| d.embedding).collect())
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
    fn test_from_env_uses_defaults() {
        // Without env vars set, should use defaults.
        let embedder = FdApiEmbedder::from_env();
        assert_eq!(embedder.dimensions(), 1024);
        assert_eq!(embedder.model_id(), "deepvk/USER-bge-m3");
    }

    #[test]
    fn test_new_with_custom_endpoint() {
        let embedder = FdApiEmbedder::new(Some("http://localhost:9999/v1/embeddings"), None);
        assert_eq!(embedder.dimensions(), 1024);
    }
}
