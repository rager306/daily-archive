//! `da serve-graph` — opt-in in-memory Samyama RespServer (M308).
//!
//! Anti-goals:
//! - No RocksDB / PersistenceManager (`RespServer::new`, `data_path: None`)
//! - Does not call `open_graph_store` (CRITICAL blast radius — left untouched)
//! - Does not auto-start from other commands
//! - Does not open D127 / import_eligible
//! - Does not wire GraphMode::Remote client

use std::sync::Arc;

use clap::Args;
use samyama::{GraphStore, RespServer, ServerConfig};
use tokio::sync::RwLock;

/// Default host: loopback only.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Default port: off Redis default (6379) to avoid accidental clash.
pub const DEFAULT_PORT: u16 = 16379;

#[derive(Debug, Clone, Args)]
pub struct ServeGraphArgs {
    /// Bind address (default 127.0.0.1)
    #[arg(long, default_value = DEFAULT_HOST)]
    pub host: String,

    /// TCP port (default 16379; use 0 only via tests that pre-pick ephemeral)
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
}

/// Operator banners printed before listen (honest mode).
pub fn operator_banners(host: &str, port: u16) -> Vec<String> {
    vec![
        format!("daily-archive serve-graph — in-memory RespServer"),
        format!("  bind:            {host}:{port}"),
        format!("  persistence:     NONE (RespServer::new, data_path=None)"),
        format!("  rocksdb:         not used"),
        format!("  import_eligible: false (D127 locked)"),
        format!("  remote client:   not wired (GraphMode::Remote still NotConfigured)"),
        format!("  health:          RESP PING → PONG"),
        format!("  stop:            Ctrl-C"),
    ]
}

/// Build in-memory server config (no disk path).
pub fn in_memory_config(host: &str, port: u16) -> ServerConfig {
    ServerConfig {
        address: host.to_string(),
        port,
        max_connections: 100,
        data_path: None,
    }
}

/// Run blocking accept loop until process killed. Caller owns the Tokio runtime.
pub async fn run_in_memory_server(args: &ServeGraphArgs) -> Result<(), Box<dyn std::error::Error>> {
    for line in operator_banners(&args.host, args.port) {
        println!("{line}");
    }
    let config = in_memory_config(&args.host, args.port);
    debug_assert!(
        config.data_path.is_none(),
        "serve-graph must never enable RocksDB data_path"
    );
    let store = Arc::new(RwLock::new(GraphStore::new()));
    let server = RespServer::new(config, store);
    server.start().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_loopback_and_non_redis_port() {
        assert_eq!(DEFAULT_HOST, "127.0.0.1");
        assert_eq!(DEFAULT_PORT, 16379);
        assert_ne!(DEFAULT_PORT, 6379);
    }

    #[test]
    fn in_memory_config_has_no_data_path() {
        let c = in_memory_config("127.0.0.1", 16379);
        assert!(c.data_path.is_none());
        assert_eq!(c.address, "127.0.0.1");
        assert_eq!(c.port, 16379);
    }

    #[test]
    fn banners_state_non_persistent_and_d127() {
        let lines = operator_banners("127.0.0.1", 16379).join("\n");
        assert!(lines.contains("NONE"), "must state no persistence");
        assert!(lines.to_lowercase().contains("rocksdb"));
        assert!(lines.contains("import_eligible: false") || lines.contains("D127"));
        assert!(lines.contains("NotConfigured") || lines.contains("remote client"));
        assert!(lines.contains("PING"));
    }

    #[tokio::test]
    async fn spawn_server_answers_ping() {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        // Ephemeral free port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let args = ServeGraphArgs {
            host: "127.0.0.1".into(),
            port,
        };
        let task = tokio::spawn(async move {
            let _ = run_in_memory_server(&args).await;
        });

        let addr = format!("127.0.0.1:{port}");
        let mut stream = None;
        for _ in 0..50 {
            match TcpStream::connect(&addr).await {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
        let mut stream = stream.expect("server should accept");
        stream.write_all(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        let mut buf = vec![0u8; 256];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("PONG"), "expected PONG, got {response:?}");
        drop(stream);
        task.abort();
        let _ = task.await;
    }
}
