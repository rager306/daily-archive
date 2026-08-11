//! `da serve-graph` — opt-in in-memory Samyama RespServer (M308/M309).
//!
//! Anti-goals:
//! - No RocksDB / PersistenceManager (`RespServer::new`, `data_path: None`)
//! - Does not call `open_graph_store` (CRITICAL blast radius — left untouched)
//! - Does not auto-start from other commands
//! - Does not open D127 / import_eligible
//! - Serves M378 scalar `graph-query --mode remote`; HOT GraphMode writes stay unsupported
//!
//! M309: optional `--snapshot` preloads `.sgsnap` into the same process memory
//! before listen. Store still dies when the process exits (honest banners).

use std::path::{Path, PathBuf};
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

    /// Optional `.sgsnap` to import into this process before listen (M309).
    /// Still non-persistent: memory dies when the process exits.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
}

/// Result of optional snapshot preload (for banners + tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPreloadInfo {
    pub path: PathBuf,
    pub bytes: usize,
    pub nodes: usize,
}

/// Operator banners printed before listen (honest mode).
pub fn operator_banners(
    host: &str,
    port: u16,
    preload: Option<&SnapshotPreloadInfo>,
) -> Vec<String> {
    let mut lines = vec![
        format!("daily-archive serve-graph — in-memory RespServer"),
        format!("  bind:            {host}:{port}"),
        format!("  persistence:     NONE (RespServer::new, data_path=None)"),
        format!("  rocksdb:         not used"),
        format!("  import_eligible: false (D127 locked)"),
        format!("  remote client:   scalar read plane available; HOT writes unsupported"),
    ];
    match preload {
        Some(p) => {
            lines.push(format!(
                "  snapshot:        {} ({} bytes, {} nodes in this process)",
                p.path.display(),
                p.bytes,
                p.nodes
            ));
            lines.push(
                "  snapshot note:   NON-PERSISTENT — dies when this process exits".to_string(),
            );
        }
        None => lines.push("  snapshot:        none (empty in-memory store)".to_string()),
    }
    lines.push("  health:          RESP PING → PONG".to_string());
    lines.push("  stop:            Ctrl-C or SIGTERM (graceful exit 0)".to_string());
    lines
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

/// Load `.sgsnap` bytes into a new `GraphStore` via `import_tenant`.
pub fn load_sgsnap_into_store(
    path: &Path,
) -> Result<(GraphStore, SnapshotPreloadInfo), Box<dyn std::error::Error>> {
    let data =
        std::fs::read(path).map_err(|e| format!("cannot read snapshot {}: {e}", path.display()))?;
    let bytes = data.len();
    let mut store = GraphStore::new();
    samyama::snapshot::import_tenant(&mut store, &mut std::io::Cursor::new(&data))
        .map_err(|e| format!("import_tenant failed for {}: {e}", path.display()))?;
    let nodes = store.all_nodes().len();
    let info = SnapshotPreloadInfo {
        path: path.to_path_buf(),
        bytes,
        nodes,
    };
    Ok((store, info))
}

/// Wait for an operator shutdown signal. Signal termination is a normal server
/// lifecycle transition, not a crash.
#[cfg(unix)]
async fn shutdown_signal() -> Result<&'static str, std::io::Error> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = terminate.recv() => Ok("SIGTERM"),
        _ = interrupt.recv() => Ok("SIGINT"),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<&'static str, std::io::Error> {
    tokio::signal::ctrl_c().await?;
    Ok("Ctrl-C")
}

/// Run the accept loop until a normal operator shutdown signal or server error.
pub async fn run_in_memory_server(args: &ServeGraphArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (store, preload) = match &args.snapshot {
        Some(path) => {
            let (s, info) = load_sgsnap_into_store(path)?;
            (s, Some(info))
        }
        None => (GraphStore::new(), None),
    };

    for line in operator_banners(&args.host, args.port, preload.as_ref()) {
        println!("{line}");
    }

    let config = in_memory_config(&args.host, args.port);
    debug_assert!(
        config.data_path.is_none(),
        "serve-graph must never enable RocksDB data_path"
    );
    let store = Arc::new(RwLock::new(store));
    let server = RespServer::new(config, store);
    tokio::select! {
        result = server.start() => result,
        signal = shutdown_signal() => {
            let signal = signal?;
            println!("serve-graph shutdown: {signal} (expected operator stop)");
            Ok(())
        }
    }
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
        let lines = operator_banners("127.0.0.1", 16379, None).join("\n");
        assert!(lines.contains("NONE"), "must state no persistence");
        assert!(lines.to_lowercase().contains("rocksdb"));
        assert!(lines.contains("import_eligible: false") || lines.contains("D127"));
        assert!(lines.contains("NotConfigured") || lines.contains("remote client"));
        assert!(lines.contains("PING"));
        assert!(lines.contains("snapshot:        none"));
    }

    #[test]
    fn banners_include_snapshot_preload_info() {
        let info = SnapshotPreloadInfo {
            path: PathBuf::from("/tmp/demo.sgsnap"),
            bytes: 42,
            nodes: 3,
        };
        let lines = operator_banners("127.0.0.1", 16379, Some(&info)).join("\n");
        assert!(lines.contains("demo.sgsnap"));
        assert!(lines.contains("42 bytes"));
        assert!(lines.contains("3 nodes"));
        assert!(lines.contains("NON-PERSISTENT"));
    }

    #[test]
    fn load_missing_snapshot_errors() {
        let err = load_sgsnap_into_store(Path::new("/tmp/definitely-missing-m309.sgsnap"));
        assert!(err.is_err());
        let msg = err.err().unwrap().to_string();
        assert!(msg.contains("cannot read") || msg.contains("No such"));
    }

    /// Build a tiny real sgsnap via export_tenant for round-trip preload tests.
    fn write_tiny_sgsnap(path: &Path) -> usize {
        let mut store = GraphStore::new();
        let _n = store.create_node(samyama::graph::types::Label::new("Paper"));
        let mut buf = Vec::new();
        samyama::snapshot::export_tenant(&store, &mut buf).expect("export");
        std::fs::write(path, &buf).expect("write");
        buf.len()
    }

    #[test]
    fn load_synthetic_sgsnap_restores_nodes() {
        let dir = std::env::temp_dir().join(format!("m309-snap-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.sgsnap");
        let bytes = write_tiny_sgsnap(&path);
        let (store, info) = load_sgsnap_into_store(&path).expect("load");
        assert_eq!(info.bytes, bytes);
        assert_eq!(info.nodes, 1);
        assert_eq!(store.all_nodes().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn spawn_server_answers_ping() {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let args = ServeGraphArgs {
            host: "127.0.0.1".into(),
            port,
            snapshot: None,
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

    #[tokio::test]
    async fn spawn_with_snapshot_still_answers_ping() {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let dir = std::env::temp_dir().join(format!("m309-serve-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.sgsnap");
        write_tiny_sgsnap(&path);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let args = ServeGraphArgs {
            host: "127.0.0.1".into(),
            port,
            snapshot: Some(path.clone()),
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
        let mut stream = stream.expect("server should accept with snapshot");
        stream.write_all(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        let mut buf = vec![0u8; 256];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("PONG"), "expected PONG, got {response:?}");
        drop(stream);
        task.abort();
        let _ = task.await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
