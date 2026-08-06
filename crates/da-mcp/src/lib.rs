//! MCP server for daily-archive knowledge base external access (ADR-054).
//!
//! Read-only MCP server over Samyama graph store. Exposes:
//! - Tools: get_schema, get_graph_stats (Wave 1)
//!   search_papers, get_paper, get_sections, read_cypher (Wave 2, next)
//!
//! D127-safe: no write tools, no import unlock, query_readonly enforced.

use std::sync::Arc;

use kg_storage::DirectGraphStore;
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::Parameters;

pub mod tools;

/// Configuration for the MCP server.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Snapshot file path (`.sgsnap`). When set, the server loads the graph
    /// into an in-process Samyama engine.
    pub snapshot_path: Option<String>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            snapshot_path: None,
        }
    }
}

/// The MCP server handler — read-only tools over the graph store.
pub struct DailyArchiveMcpServer {
    pub config: McpServerConfig,
    pub graph_store: Arc<dyn DirectGraphStore>,
}

impl DailyArchiveMcpServer {
    /// Create a new server with the given graph store and default config.
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self {
            config: McpServerConfig::default(),
            graph_store,
        }
    }

    /// Create a new server with snapshot mode.
    pub fn with_snapshot(
        graph_store: Arc<dyn DirectGraphStore>,
        snapshot_path: impl Into<String>,
    ) -> Self {
        Self {
            config: McpServerConfig {
                snapshot_path: Some(snapshot_path.into()),
            },
            graph_store,
        }
    }
}

/// Wave 1 tools: get_schema + get_graph_stats. Read-only by construction.
#[rmcp::tool_router(server_handler)]
impl DailyArchiveMcpServer {
    /// Get the daily-archive knowledge graph schema.
    #[rmcp::tool(description = "Get the daily-archive knowledge graph schema: node labels + edge types from YAML ontology (ADR-050). Optional: include node/edge counts.")]
    async fn get_schema(
        &self,
        parameters: Parameters<tools::GetSchemaParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_schema(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| format!("get_schema failed: {e}"))?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get daily-archive knowledge graph statistics.
    #[rmcp::tool(description = "Get daily-archive knowledge graph statistics: total node and edge counts, plus per-label node distribution. Snapshot path if snapshot mode.")]
    async fn get_graph_stats(&self) -> Result<String, String> {
        let out = tools::get_graph_stats(
            self.graph_store.as_ref(),
            self.config.snapshot_path.as_deref(),
        )
        .await
        .map_err(|e| format!("get_graph_stats failed: {e}"))?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Run a read-only Cypher query against the knowledge graph.
    #[rmcp::tool(description = "Run a read-only Cypher query against the daily-archive knowledge graph. Rejects destructive keywords (CREATE/SET/DELETE/MERGE). D127-safe read-only. Use Samyama Cypher syntax; LIMIT results for large queries.")]
    async fn read_cypher(
        &self,
        parameters: Parameters<tools::ReadCypherParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::read_cypher(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }
}

// server_handler flag emits `impl ServerHandler for DailyArchiveMcpServer` with
// an auto-generated `get_info()` (tools capability). Import is kept for clarity.

#[cfg(test)]
mod tests {
    use super::*;

    fn test_server() -> DailyArchiveMcpServer {
        DailyArchiveMcpServer::new(Arc::new(kg_storage::MockGraphStore::new()))
    }

    #[test]
    fn test_server_created_with_snapshot_config() {
        let server = DailyArchiveMcpServer::with_snapshot(
            Arc::new(kg_storage::MockGraphStore::new()),
            "test.sgsnap",
        );
        assert_eq!(server.config.snapshot_path.as_deref(), Some("test.sgsnap"));
    }

    #[test]
    fn test_server_default_config_no_snapshot() {
        let server = test_server();
        assert!(server.config.snapshot_path.is_none());
    }

    #[test]
    fn test_server_tools_capability_enabled() {
        let server = test_server();
        let info = server.get_info();
        assert!(
            info.capabilities.tools.is_some(),
            "server must advertise tools capability"
        );
    }
}
