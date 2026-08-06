//! MCP server for daily-archive knowledge base external access (ADR-054).
//!
//! Read-only MCP server over Samyama graph store. Exposes:
//! - Tools: get_schema, get_graph_stats (Wave 1)
//!   search_papers, get_paper, get_sections, read_cypher (Wave 2, next)
//!
//! D127-safe: no write tools, no import unlock, query_readonly enforced.

use std::sync::Arc;

use kg_storage::DirectGraphStore;
use rmcp::handler::server::wrapper::Parameters;

pub mod tools;

/// Configuration for the MCP server.
#[derive(Debug, Clone, Default)]
pub struct McpServerConfig {
    /// Snapshot file path (`.sgsnap`). When set, the server loads the graph
    /// into an in-process Samyama engine.
    pub snapshot_path: Option<String>,
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
    #[rmcp::tool(
        description = "Get the daily-archive knowledge graph schema: node labels + edge types from YAML ontology (ADR-050). Optional: include node/edge counts."
    )]
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
    #[rmcp::tool(
        description = "Get daily-archive knowledge graph statistics: total node and edge counts, plus per-label node distribution. Snapshot path if snapshot mode."
    )]
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
    #[rmcp::tool(
        description = "Run a read-only Cypher query against the daily-archive knowledge graph. Rejects destructive keywords (CREATE/SET/DELETE/MERGE). D127-safe read-only. Use Samyama Cypher syntax; LIMIT results for large queries."
    )]
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

    /// Get a paper's basic metadata by arXiv id.
    #[rmcp::tool(
        description = "Get a paper node and its basic metadata (title, publication date, cited-by count, OpenAlex id) by arXiv id."
    )]
    async fn get_paper(
        &self,
        parameters: Parameters<tools::GetPaperParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_paper(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get a paper's section tree (title, path_id, level) by arXiv id.
    #[rmcp::tool(
        description = "Get the section navigation tree for a paper by arXiv id: section titles, path_id, and level in document order."
    )]
    async fn get_sections(
        &self,
        parameters: Parameters<tools::GetSectionsParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_sections(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get CITES references for a paper (backward = papers it cites, forward =
    /// papers that cite it, both = default).
    #[rmcp::tool(
        description = "Get CITES references for a paper by arXiv id. direction: 'backward' (papers this paper cites), 'forward' (papers citing this paper), or 'both'."
    )]
    async fn get_references(
        &self,
        parameters: Parameters<tools::GetReferencesParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_references(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Search papers by title substring (case-insensitive, read-only).
    #[rmcp::tool(
        description = "Search papers by title substring (case-insensitive). Returns arxiv_id and title for matching Paper nodes. D127-safe read-only."
    )]
    async fn search_papers(
        &self,
        parameters: Parameters<tools::SearchPapersParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::search_papers(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get entities linked to a paper (read-only, MENTIONS edges).
    #[rmcp::tool(
        description = "Get entities linked to a paper via MENTIONS edges. Optional entity_type substring filter. D127-safe read-only."
    )]
    async fn get_entities(
        &self,
        parameters: Parameters<tools::GetEntitiesParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_entities(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Validate a node label against ontology schemas.
    #[rmcp::tool(
        description = "Validate a node label against registered schemas (YAML ontology + Rust). Returns is_known and violations."
    )]
    async fn validate_node(
        &self,
        parameters: Parameters<tools::ValidateNodeParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::validate_node(self.graph_store.as_ref(), params)
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
    use rmcp::ServerHandler;

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
