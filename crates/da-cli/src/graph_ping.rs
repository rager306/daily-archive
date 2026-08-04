//! `da graph-ping` — RESP health probe against `da serve-graph` (M310).
//!
//! Thin multi-process path: PING → PONG only.
//! Anti-goals:
//! - Not a full remote GraphStore / GraphMode::Remote implementation
//! - Does not call or change `open_graph_store` / `open_for_mode`
//! - Does not open D127 / import_eligible
//! - Does not start a server (use `da serve-graph` separately)

use std::time::Duration;

use clap::Args;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

// Single source of truth — keep aligned with `da serve-graph` (M311).
pub use crate::serve_graph::{DEFAULT_HOST, DEFAULT_PORT};
/// Connect + read timeout for a single probe.
pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

/// RESP array: *1\r\n$4\r\nPING\r\n
pub const RESP_PING: &[u8] = b"*1\r\n$4\r\nPING\r\n";

#[derive(Debug, Clone, Args)]
pub struct GraphPingArgs {
    /// Host of running serve-graph (default 127.0.0.1)
    #[arg(long, default_value = DEFAULT_HOST)]
    pub host: String,

    /// Port of running serve-graph (default 16379)
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Timeout in milliseconds for connect+read (default 2000)
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MS)]
    pub timeout_ms: u64,
}

/// Errors from a health probe (honest, non-silent).
#[derive(Debug)]
pub enum GraphPingError {
    Connect(String),
    Timeout,
    Io(String),
    BadResponse(String),
}

impl std::fmt::Display for GraphPingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "cannot connect: {e}"),
            Self::Timeout => write!(f, "timeout waiting for PONG"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::BadResponse(r) => write!(f, "unexpected response (expected PONG): {r:?}"),
        }
    }
}

impl std::error::Error for GraphPingError {}

/// Parse: response must contain the token PONG (SimpleString `+PONG` or bulk).
pub fn response_is_pong(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(bytes);
    s.contains("PONG")
}

/// One-shot RESP PING against host:port.
pub async fn ping_resp(args: &GraphPingArgs) -> Result<String, GraphPingError> {
    let addr = format!("{}:{}", args.host, args.port);
    let dur = Duration::from_millis(args.timeout_ms.max(1));

    let mut stream = timeout(dur, TcpStream::connect(&addr))
        .await
        .map_err(|_| GraphPingError::Timeout)?
        .map_err(|e| GraphPingError::Connect(e.to_string()))?;

    timeout(dur, stream.write_all(RESP_PING))
        .await
        .map_err(|_| GraphPingError::Timeout)?
        .map_err(|e| GraphPingError::Io(e.to_string()))?;

    let mut buf = vec![0u8; 256];
    let n = timeout(dur, stream.read(&mut buf))
        .await
        .map_err(|_| GraphPingError::Timeout)?
        .map_err(|e| GraphPingError::Io(e.to_string()))?;

    if n == 0 {
        return Err(GraphPingError::BadResponse("empty".into()));
    }
    let raw = String::from_utf8_lossy(&buf[..n]).into_owned();
    if !response_is_pong(&buf[..n]) {
        return Err(GraphPingError::BadResponse(raw));
    }
    Ok(raw)
}

/// CLI entry: print PONG line or return error for non-zero exit.
pub async fn run_graph_ping(args: &GraphPingArgs) -> Result<(), GraphPingError> {
    let raw = ping_resp(args).await?;
    // Operator-facing success line (stable for scripts).
    println!("PONG");
    // Debug detail on stderr only if response is not the plain simple string.
    if raw.trim() != "+PONG" && !raw.contains("+PONG") {
        eprintln!("resp: {}", raw.trim());
    }
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

    #[test]
    fn response_is_pong_accepts_simple_string() {
        assert!(response_is_pong(b"+PONG\r\n"));
        assert!(response_is_pong(b"+PONG"));
        assert!(!response_is_pong(b"+OK\r\n"));
        assert!(!response_is_pong(b""));
    }

    #[tokio::test]
    async fn ping_unreachable_errors() {
        // Port unlikely to be listening.
        let args = GraphPingArgs {
            host: "127.0.0.1".into(),
            port: 1,
            timeout_ms: 300,
        };
        let err = ping_resp(&args).await.unwrap_err();
        match err {
            GraphPingError::Connect(_) | GraphPingError::Timeout => {}
            other => panic!("expected connect/timeout, got {other}"),
        }
    }

    #[tokio::test]
    async fn ping_live_serve_graph() {
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

        // Wait until accepting
        let mut ready = false;
        for _ in 0..50 {
            if TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(ready, "serve-graph did not start");

        let args = GraphPingArgs {
            host: "127.0.0.1".into(),
            port,
            timeout_ms: 2000,
        };
        let raw = ping_resp(&args).await.expect("ping should succeed");
        assert!(response_is_pong(raw.as_bytes()), "got {raw:?}");

        task.abort();
        let _ = task.await;
    }
}
