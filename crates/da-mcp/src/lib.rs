//! MCP server for daily-archive knowledge base external access (ADR-054).
//!
//! Read-only MCP server over Samyama graph store. Exposes:
//! - Tools: search_papers, get_paper, get_sections, get_references, read_cypher,
//!   get_schema, get_graph_stats, validate_node (Wave 1 subset)
//! - Resources: ontology YAMLs, schema (Wave 2)
//! - Prompts: reading-path workflows (Wave 2)
//!
//! D127-safe: no write tools, no import unlock, query_readonly enforced.
//!
//! Transports: stdio (default), Streamable HTTP (Wave 3).
//!
//! Snapshot mode: `da-mcp --snapshot <file.sgsnap>` loads graph into process.

use kg_storage::DirectGraphStore;

pub mod tools;

/// Configuration for the MCP server.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Snapshot file path (`.sgsnap`). When set, the server loads the graph
    /// into an in-process Samyama engine. When None, a live connection is
    /// used (future, when daemon exists — currently requires snapshot).
    pub snapshot_path: Option<String>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            snapshot_path: None,
        }
    }
}

/// The MCP server handler. Holds a reference to the graph store.
pub struct DailyArchiveMcpServer {
    pub config: McpServerConfig,
    pub graph_store: Box<dyn DirectGraphStore>,
}

impl DailyArchiveMcpServer {
    /// Create a new server with the given graph store and default config.
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            config: McpServerConfig::default(),
            graph_store,
        }
    }

    /// Create a new server with snapshot mode.
    pub fn with_snapshot(
        graph_store: Box<dyn DirectGraphStore>,
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
