//! MCP server binary for daily-archive knowledge base (ADR-054).
//!
//! Usage:
//!   da-mcp --stdio --snapshot <file.sgsnap>   (load real graph snapshot)
//!   da-mcp --stdio                            (empty graph — tool discovery)
//!
//! Wave 2 (MCP-2): real Samyama graph store + snapshot loading.

use std::sync::Arc;

use clap::Parser;
use da_mcp::DailyArchiveMcpServer;
use kg_storage::{DirectGraphStore, GraphStore, MockGraphStore, SamyamaGraphStore};

mod logging;
use logging::init_logging;
use rmcp::service::serve_server_with_ct;
use rmcp::transport::stdio;

/// MCP server for daily-archive knowledge base.
#[derive(Parser, Debug)]
#[command(name = "da-mcp", version, about)]
struct Cli {
    /// Use stdio transport (default for Claude Desktop / IDE integration).
    #[arg(long)]
    stdio: bool,

    /// Snapshot file path (.sgsnap). Loads graph into in-process engine.
    #[arg(long)]
    snapshot: Option<String>,

    /// HTTP port for Streamable HTTP transport (Wave 3, not yet implemented).
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // stderr-only logging (stdout is reserved for MCP JSON-RPC protocol).
    let _guard = init_logging();

    tracing::info!(
        snapshot = ?cli.snapshot,
        stdio = cli.stdio,
        "da-mcp server starting (ADR-054 Wave 2)"
    );

    // Build the graph store: real Samyama store when a snapshot is provided,
    // otherwise MockGraphStore (for tool discovery / tests).
    let store: Arc<dyn DirectGraphStore> = if let Some(snap_path) = &cli.snapshot {
        let snap_bytes = std::fs::read(snap_path).map_err(|e| {
            tracing::error!(path = %snap_path, error = %e, "failed to read snapshot");
            anyhow::anyhow!("failed to read snapshot {snap_path}: {e}")
        })?;
        let samyama = SamyamaGraphStore::from_env();
        samyama.import_snapshot(&snap_bytes).await.map_err(|e| {
            tracing::error!(error = %e, "snapshot import failed");
            anyhow::anyhow!("snapshot import failed: {e}")
        })?;
        let nodes = samyama.node_count().await;
        let edges = samyama.edge_count().await;
        tracing::info!(nodes, edges, snapshot = %snap_path, "snapshot loaded");
        Arc::new(samyama)
    } else {
        Arc::new(MockGraphStore::new())
    };

    let server = if let Some(snap) = &cli.snapshot {
        DailyArchiveMcpServer::with_snapshot(store, snap.clone())
    } else {
        DailyArchiveMcpServer::new(store)
    };

    if cli.stdio {
        let ct = tokio_util::sync::CancellationToken::new();
        let _running = serve_server_with_ct(server, stdio(), ct.clone())
            .await
            .map_err(|e| anyhow::anyhow!("MCP server init failed: {e}"))?;
        tracing::info!("MCP server ready on stdio — waiting for requests");
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
    } else {
        tracing::warn!("no transport selected (use --stdio)");
    }

    Ok(())
}
