//! `da graph-ping` — thin health presentation over `RemoteGraphStore`.
//!
//! Transport, bounded RESP parsing, timeout and protocol validation belong to
//! `kg-storage`; this module owns only CLI arguments and stable `PONG` output.

use std::time::Duration;

use clap::Args;
use da_ports::GraphStore;
use kg_storage::{RemoteGraphConfig, RemoteGraphStore};

pub use crate::serve_graph::{DEFAULT_HOST, DEFAULT_PORT};
pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

#[derive(Debug, Clone, Args)]
pub struct GraphPingArgs {
    /// Host of running serve-graph (default 127.0.0.1)
    #[arg(long, default_value = DEFAULT_HOST)]
    pub host: String,

    /// Port of running serve-graph (default 16379)
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Timeout in milliseconds for one bounded probe (default 2000)
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub struct GraphPingError(String);

impl std::fmt::Display for GraphPingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "graph PING failed: {}", self.0)
    }
}

impl std::error::Error for GraphPingError {}

fn remote_store(args: &GraphPingArgs) -> Result<RemoteGraphStore, GraphPingError> {
    let endpoint = format!("{}:{}", args.host, args.port);
    let mut config =
        RemoteGraphConfig::new(endpoint).map_err(|error| GraphPingError(error.to_string()))?;
    config.timeout = Duration::from_millis(args.timeout_ms.max(1));
    RemoteGraphStore::new(config).map_err(|error| GraphPingError(error.to_string()))
}

pub async fn ping(args: &GraphPingArgs) -> Result<(), GraphPingError> {
    let healthy = remote_store(args)?
        .health()
        .await
        .map_err(|error| GraphPingError(error.to_string()))?;
    if !healthy {
        return Err(GraphPingError("remote graph reported unhealthy".into()));
    }
    Ok(())
}

/// CLI entry: print exactly one stable success line or return a non-zero error.
pub async fn run_graph_ping(args: &GraphPingArgs) -> Result<(), GraphPingError> {
    ping(args).await?;
    println!("PONG");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_serve_graph_port() {
        assert_eq!(DEFAULT_HOST, crate::serve_graph::DEFAULT_HOST);
        assert_eq!(DEFAULT_PORT, crate::serve_graph::DEFAULT_PORT);
        assert_eq!(DEFAULT_PORT, 16379);
        assert_ne!(DEFAULT_PORT, 6379);
    }

    #[tokio::test]
    async fn ping_unreachable_uses_shared_remote_diagnostic() {
        let args = GraphPingArgs {
            host: "127.0.0.1".into(),
            port: 1,
            timeout_ms: 100,
        };
        let error = ping(&args).await.unwrap_err().to_string();
        assert!(error.contains("remote graph connect failed"), "{error}");
        assert!(error.contains("127.0.0.1:1"), "{error}");
    }

    #[tokio::test]
    async fn ping_live_serve_graph_through_remote_store() {
        use crate::serve_graph::{ServeGraphArgs, run_in_memory_server};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let serve_args = ServeGraphArgs {
            host: "127.0.0.1".into(),
            port,
            snapshot: None,
        };
        let task = tokio::spawn(async move {
            let _ = run_in_memory_server(&serve_args).await;
        });
        let args = GraphPingArgs {
            host: "127.0.0.1".into(),
            port,
            timeout_ms: 500,
        };
        let mut ready = false;
        for _ in 0..50 {
            if ping(&args).await.is_ok() {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(ready, "serve-graph did not become healthy");
        ping(&args)
            .await
            .expect("shared remote PING should succeed");
        task.abort();
        let _ = task.await;
    }
}
