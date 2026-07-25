//! Samyama Graph adapter — implements GraphStore port.
//!
//! ADR-040 §1 Tier 1: Samyama Graph as sole graph+vector+persist engine.
//! Uses HTTP API (not embedded) for now — embedded mode is Phase 3+.
//!
//! Samyama runs at:
//!   RESP: 127.0.0.1:6380 (Redis-compatible)
//!   HTTP: http://127.0.0.1:8080 (REST API + Cypher)

use async_trait::async_trait;
use da_ports::graph_store::{GraphStore, GraphStoreError, GraphResult, QueryResult, VectorMetric, VectorSearchResult};
use serde::{Deserialize, Serialize};

const DEFAULT_HTTP_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_GRAPH: &str = "default";

/// Samyama Graph HTTP adapter.
#[derive(Clone)]
pub struct SamyamaGraphStore {
    http_url: String,
    graph: String,
    client: reqwest::Client,
}

impl SamyamaGraphStore {
    pub fn new(http_url: Option<&str>, graph: Option<&str>) -> Self {
        Self {
            http_url: http_url.unwrap_or(DEFAULT_HTTP_URL).to_string(),
            graph: graph.unwrap_or(DEFAULT_GRAPH).to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("SAMYAMA_HTTP_URL").unwrap_or_else(|_| DEFAULT_HTTP_URL.to_string());
        let graph = std::env::var("SAMYAMA_DEFAULT_TENANT").unwrap_or_else(|_| DEFAULT_GRAPH.to_string());
        Self::new(Some(&url), Some(&graph))
    }

    async fn post_query(&self, cypher: &str) -> GraphResult<QueryResult> {
        let url = format!("{}/api/query", self.http_url);
        let body = serde_json::json!({
            "graph": self.graph,
            "query": cypher,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| GraphStoreError::Query(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(GraphStoreError::Query(format!("HTTP {}: {}", "error", text)));
        }

        let raw: SamyamaQueryResponse = resp
            .json()
            .await
            .map_err(|e| GraphStoreError::Query(format!("parse: {e}")))?;

        Ok(QueryResult {
            columns: raw.columns,
            records: raw.records,
        })
    }
}

/// Raw Samyama API response shape.
#[derive(Debug, Deserialize)]
struct SamyamaQueryResponse {
    columns: Vec<String>,
    #[serde(default)]
    records: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    nodes: Vec<serde_json::Value>,
    #[serde(default)]
    edges: Vec<serde_json::Value>,
}

#[async_trait]
impl GraphStore for SamyamaGraphStore {
    async fn query(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        tracing::debug!(cypher = %cypher, "Samyama write query");
        self.post_query(cypher).await
    }

    async fn query_readonly(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        tracing::debug!(cypher = %cypher, "Samyama read query");
        self.post_query(cypher).await
    }

    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()> {
        // Samyama vector indexes are created via Cypher procedure or API
        let metric_str = match metric {
            VectorMetric::Cosine => "cosine",
            VectorMetric::L2 => "l2",
            VectorMetric::Dot => "dot",
        };
        tracing::info!(
            label, property, dimensions, metric = metric_str,
            "Vector index creation (deferred to Samyama SDK direct call)"
        );
        // TODO: call Samyama VectorIndexManager::create_index via embedded SDK
        Ok(())
    }

    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        let url = format!("{}/api/vector/search", self.http_url);
        let body = serde_json::json!({
            "graph": self.graph,
            "label": label,
            "property": property,
            "query_vector": query_vector,
            "k": k,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| GraphStoreError::Vector(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(GraphStoreError::Vector(text));
        }

        // Parse results — Samyama returns array of {vid, score, properties}
        let results: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| GraphStoreError::Vector(format!("parse: {e}")))?;

        Ok(results.iter().map(|v| VectorSearchResult {
            vid: v.get("vid").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            score: v.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
            properties: v.clone(),
        }).collect())
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        // ADR-040 §11.6: Samyama .sgsnap export
        // TODO: call snapshot export API
        tracing::warn!("Snapshot export not yet implemented via HTTP API");
        Ok(vec![])
    }

    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        tracing::warn!("Snapshot import not yet implemented via HTTP API");
        Ok(())
    }

    async fn health(&self) -> GraphResult<bool> {
        let url = format!("{}/api/status", self.http_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_env_defaults() {
        // Should not panic even if env vars are missing
        let store = SamyamaGraphStore::from_env();
        assert!(store.http_url.starts_with("http"));
    }
}
