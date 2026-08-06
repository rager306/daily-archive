//! MCP server binary for daily-archive knowledge base (ADR-054).
//!
//! Usage:
//!   da-mcp --stdio --snapshot <file.sgsnap>
//!   da-mcp --stdio  (empty graph — tool discovery only)
//!
//! Wave 1.5 (MCP-1.5): rmcp ServerHandler wired over stdio transport.

use std::sync::Arc;

use clap::Parser;
use da_mcp::DailyArchiveMcpServer;
use kg_storage::{DirectGraphStore, MockGraphStore};
use rmcp::transport::stdio;
use rmcp::{RoleServer, service::serve_server_with_ct};

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
    let _guard = da_cli::logging::init_logging();

    tracing::info!(
        snapshot = ?cli.snapshot,
        stdio = cli.stdio,
        "da-mcp server starting (ADR-054 Wave 1.5)"
    );

    let store: Arc<dyn DirectGraphStore> = Arc::new(MockGraphStore::new());
    let server = if let Some(snap) = &cli.snapshot {
        DailyArchiveMcpServer::with_snapshot(store, snap.clone())
    } else {
        DailyArchiveMcpServer::new(store)
    };

    if cli.stdio {
        // rmcp v3: serve over stdio. The stdio transport is a (Stdin, Stdout)
        // pair which implements IntoTransport for RoleServer.
        let ct = tokio_util::sync::CancellationToken::new();
        let running = serve_server_with_ct(server, stdio(), ct.clone())
            .await
            .map_err(|e| anyhow::anyhow!("MCP server init failed: {e}"))?;
        tracing::info!("MCP server ready on stdio — waiting for requests");
        // Block until cancellation (e.g. client closes).
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
        let _ = running;
    } else {
        tracing::warn!("no transport selected (use --stdio)");
    }

    Ok(())
}
