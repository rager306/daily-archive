//! MCP server binary for daily-archive knowledge base (ADR-054).
//!
//! Usage:
//!   da-mcp --stdio --snapshot <file.sgsnap>
//!   da-mcp --stdio  (uses empty graph — for tool discovery only)
//!
//! Wave 1 (MCP-1): skeleton + get_schema + get_graph_stats tools.

use std::sync::Arc;

use clap::Parser;
use da_mcp::{tools as mcp_tools, DailyArchiveMcpServer};
use kg_storage::{DirectGraphStore, MockGraphStore};

/// MCP server for daily-archive knowledge base.
#[derive(Parser, Debug)]
#[command(name = "da-mcp", version, about)]
struct Cli {
    /// Use stdio transport (default, for Claude Desktop and IDE integration).
    #[arg(long)]
    stdio: bool,

    /// Snapshot file path (.sgsnap). Loads graph into in-process Samyama engine.
    #[arg(long)]
    snapshot: Option<String>,

    /// HTTP port for Streamable HTTP transport (Wave 3, not yet implemented).
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging (stderr only — stdout is for MCP protocol).
    let _guard = da_cli::logging::init_logging();

    tracing::info!(
        snapshot = ?cli.snapshot,
        stdio = cli.stdio,
        "da-mcp server starting (ADR-054 Wave 1)"
    );

    // For Wave 1 skeleton: use MockGraphStore (snapshot loading comes in MCP-1.5).
    // Real snapshot loading requires kg-storage SamyamaGraphStore::import_snapshot.
    let store: Box<dyn DirectGraphStore> = Box::new(MockGraphStore::new());
    let server = DailyArchiveMcpServer::new(store);

    // Wave 1: demonstrate tools work by calling them directly.
    // Full MCP protocol wiring (rmcp ServerHandler) comes next.
    let schema = mcp_tools::get_schema(server.graph_store.as_ref(), Default::default())
        .await
        .map_err(|e| anyhow::anyhow!("get_schema failed: {e}"))?;
    tracing::info!(
        node_labels = schema.node_labels.len(),
        edge_types = schema.edge_types.len(),
        "Schema loaded"
    );

    let stats = mcp_tools::get_graph_stats(server.graph_store.as_ref(), cli.snapshot.as_deref())
        .await
        .map_err(|e| anyhow::anyhow!("get_graph_stats failed: {e}"))?;
    tracing::info!(
        total_nodes = stats.total_nodes,
        total_edges = stats.total_edges,
        "Graph stats loaded"
    );

    if cli.stdio {
        tracing::info!("stdio transport requested — full MCP protocol wiring is the next slice");
        // TODO: rmcp serve_server via stdio transport (MCP-1 next step)
        println!(
            "{}",
            serde_json::json!({
                "status": "da-mcp skeleton ready",
                "node_labels": schema.node_labels.len(),
                "edge_types": schema.edge_types.len(),
                "total_nodes": stats.total_nodes,
                "next": "MCP-1.5: rmcp ServerHandler wiring for stdio transport"
            })
        );
    }

    Ok(())
}
