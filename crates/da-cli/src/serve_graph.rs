//! `da serve-graph` — opt-in read-only RESP graph daemon (M401/S01, ADR-061).
//!
//! Replaces the M308/M309 vendor `samyama::RespServer` composition with the
//! kg-storage bounded read-only server (`ReadOnlyServer`) over an in-memory
//! `SamyamaGraphStore`. The wire contract is exactly the read-plane allowlist
//! `PING | ECHO | INFO | DA.IDENTITY | GRAPH.RO_QUERY`; every mutating,
//! index, vector, import, snapshot or administrative command is rejected
//! server-side before the store is touched.
//!
//! Anti-goals (unchanged from M308/M309, now enforced by composition):
//! - No RocksDB / PersistenceManager: `SamyamaGraphStore::new()` is pure
//!   memory; a non-empty `DA_GRAPH_DATA_PATH` is refused via
//!   `refuse_persistence_path` before serving.
//! - No DirectGraphStore exposure: the server path holds only
//!   `Arc<dyn GraphStore>` (kg-storage read-only contract) — the HOT mutable
//!   client never crosses the wire.
//! - No vendor RespServer: kg-storage owns the bounded RESP plane (T01).
//! - Does not call `open_graph_store`; does not auto-start from other commands.
//! - Does not open D127 / import_eligible.
//!
//! M309 behaviour preserved: optional `--snapshot` preloads `.sgsnap` into
//! the same process memory (through the storage port, `import_snapshot`)
//! before listen; `DA.IDENTITY` is computed from the exported graph bytes, so
//! a second process can verify semantic identity against the preload artifact.
//! Store still dies when the process exits (honest banners).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use clap::Args;
use kg_storage::{
    DEFAULT_MAX_CONNECTIONS, DEFAULT_MAX_FRAME_BYTES, DEFAULT_MAX_RESPONSE_BYTES,
    DEFAULT_OPERATION_TIMEOUT, DEFAULT_SERVER_HOST, DEFAULT_SERVER_PORT, GraphStore,
    ReadOnlyServer, ReadOnlyServerConfig, SamyamaGraphStore,
};

/// Default host: loopback only.
pub const DEFAULT_HOST: &str = DEFAULT_SERVER_HOST;
/// Default port: off Redis default (6379) to avoid accidental clash.
pub const DEFAULT_PORT: u16 = DEFAULT_SERVER_PORT;

#[derive(Debug, Clone, Args)]
pub struct ServeGraphArgs {
    /// Bind address (default 127.0.0.1; env DA_SERVE_GRAPH_HOST).
    #[arg(long, env = "DA_SERVE_GRAPH_HOST", default_value = DEFAULT_HOST)]
    pub host: String,

    /// TCP port (default 16379; env DA_SERVE_GRAPH_PORT; zero rejected).
    #[arg(long, env = "DA_SERVE_GRAPH_PORT", default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Inbound command-frame cap in bytes (env DA_SERVE_GRAPH_MAX_FRAME_BYTES).
    #[arg(
        long,
        env = "DA_SERVE_GRAPH_MAX_FRAME_BYTES",
        default_value_t = DEFAULT_MAX_FRAME_BYTES
    )]
    pub max_frame_bytes: usize,

    /// Outbound response cap in bytes (env DA_SERVE_GRAPH_MAX_RESPONSE_BYTES).
    #[arg(
        long,
        env = "DA_SERVE_GRAPH_MAX_RESPONSE_BYTES",
        default_value_t = DEFAULT_MAX_RESPONSE_BYTES
    )]
    pub max_response_bytes: usize,

    /// Per-command operation timeout in seconds (env
    /// DA_SERVE_GRAPH_OPERATION_TIMEOUT_SECONDS; zero rejected).
    #[arg(
        long,
        env = "DA_SERVE_GRAPH_OPERATION_TIMEOUT_SECONDS",
        default_value_t = DEFAULT_OPERATION_TIMEOUT.as_secs()
    )]
    pub operation_timeout_seconds: u64,

    /// Concurrent-connection cap (env DA_SERVE_GRAPH_MAX_CONNECTIONS).
    #[arg(
        long,
        env = "DA_SERVE_GRAPH_MAX_CONNECTIONS",
        default_value_t = DEFAULT_MAX_CONNECTIONS
    )]
    pub max_connections: usize,

    /// Optional `.sgsnap` to import into this process before listen (M309).
    /// Still non-persistent: memory dies when the process exits.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
}

impl ServeGraphArgs {
    /// Resolve the bounded server config from the (already env-aware) CLI
    /// arguments. Fail-closed: blank host and zero port/limits are rejected
    /// with a typed message — never a silent default. The config has no
    /// `data_path` field at all, so the daemon cannot enable RocksDB through
    /// the server configuration (compile-time guarantee).
    pub fn to_server_config(&self) -> Result<ReadOnlyServerConfig, String> {
        if self.port == 0 {
            return Err("serve-graph port must be greater than zero (ephemeral port 0 is not supported by the env contract)".into());
        }
        let mut config = ReadOnlyServerConfig::new(&self.host, self.port);
        config.max_frame_bytes = self.max_frame_bytes;
        config.max_response_bytes = self.max_response_bytes;
        config.operation_timeout = Duration::from_secs(self.operation_timeout_seconds);
        config.max_connections = self.max_connections;
        config.validate().map_err(|error| error.to_string())?;
        Ok(config)
    }

    /// Minimal constructor for tests (serve_graph + graph_ping modules):
    /// protocol limits at the kg-storage defaults, environment untouched.
    #[cfg(test)]
    pub fn test_defaults(host: impl Into<String>, port: u16, snapshot: Option<PathBuf>) -> Self {
        Self {
            host: host.into(),
            port,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            operation_timeout_seconds: DEFAULT_OPERATION_TIMEOUT.as_secs(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            snapshot,
        }
    }
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
        "daily-archive serve-graph — read-only RESP plane (kg-storage ReadOnlyServer)".to_string(),
        format!("  bind:            {host}:{port}"),
        "  persistence:     NONE (in-memory SamyamaGraphStore, no data path)".to_string(),
        "  rocksdb:         not used".to_string(),
        "  import_eligible: false (D127 locked)".to_string(),
        "  data plane:      read-only — GRAPH.QUERY/DELETE/LIST, INDEX.*, VECTOR.*,".to_string(),
        "                   IMPORT/SNAPSHOT.*, ADMIN.* rejected before touching store".to_string(),
        "  identity:        DA.IDENTITY = deterministic SnapshotIdentity JSON".to_string(),
        "                   (SHA-256 of exported graph bytes, tenant, counts)".to_string(),
        "  remote client:   scalar read plane available; HOT writes unsupported".to_string(),
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

/// Preload `.sgsnap` bytes through the storage port (`GraphStore::import_snapshot`)
/// into the shared in-memory store. The store itself is created by the caller so
/// the same instance is handed to the read-only server afterwards.
pub async fn load_sgsnap_into_store(
    store: &SamyamaGraphStore,
    path: &Path,
) -> Result<SnapshotPreloadInfo, Box<dyn std::error::Error>> {
    let data =
        std::fs::read(path).map_err(|e| format!("cannot read snapshot {}: {e}", path.display()))?;
    let bytes = data.len();
    store
        .import_snapshot(&data)
        .await
        .map_err(|e| format!("import_snapshot failed for {}: {e}", path.display()))?;
    let nodes = store.node_count().await;
    let info = SnapshotPreloadInfo {
        path: path.to_path_buf(),
        bytes,
        nodes,
    };
    Ok(info)
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
/// Reads the process environment for the honesty gates (data-path refusal).
pub async fn run_in_memory_server(args: &ServeGraphArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_in_memory_server_with_env(args, &std::env::vars().collect()).await
}

/// Testable core: the process environment is injected so the honesty gate
/// (`DA_GRAPH_DATA_PATH` refusal) can be exercised without mutating global env.
pub async fn run_in_memory_server_with_env(
    args: &ServeGraphArgs,
    env: &BTreeMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Honesty gate (M332): a RocksDB data-path env is refused — the daemon
    // must never silently fall back to persistence.
    kg_storage::refuse_persistence_path(
        env.get(kg_storage::GraphMode::ENV_DATA_PATH)
            .map(String::as_str),
    )?;

    let config = args.to_server_config()?;

    // Pure in-memory Samyama store. Only `Arc<dyn GraphStore>` is handed to
    // the read-only server — the DirectGraphStore (HOT mutable) client never
    // crosses the server path.
    let store = SamyamaGraphStore::new();
    let preload = match &args.snapshot {
        Some(path) => Some(load_sgsnap_into_store(&store, path).await?),
        None => None,
    };

    for line in operator_banners(&config.host, config.port, preload.as_ref()) {
        println!("{line}");
    }

    // DA.IDENTITY is computed from the exported graph bytes now and fixed for
    // the lifetime of the process (T02) — a second process can verify semantic
    // identity against the preload artifact.
    let server = ReadOnlyServer::new(config, Arc::new(store))?
        .with_identity_from_store()
        .await?;
    let listener = server.bind().await?;

    tokio::select! {
        result = server.serve_listener(listener) => {
            result.map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        }
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
    use clap::{Command, FromArgMatches};
    use kg_storage::{
        RespValue, SnapshotIdentity, encode_command, fingerprint_snapshot, parse_resp,
    };
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    /// Serializes tests that mutate the process environment for the
    /// `DA_SERVE_GRAPH_*` keys (clap `env` reads the real process env at parse
    /// time; env is process-global so parallel tests would race).
    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Restores a previously-set env var on drop (edition 2024 marks
    /// `std::env::set_var` unsafe; callers hold `TEST_ENV_LOCK`).
    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            // SAFETY: single-threaded test scope; TEST_ENV_LOCK serializes all
            // env-mutating tests in this module.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: see EnvGuard::set; restoration happens under TEST_ENV_LOCK.
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    /// Parse serve-graph args the way clap would (env-aware), with a fresh
    /// `Command` so tests do not depend on the full `da` CLI definition.
    fn parse_serve_args(argv: &[&str]) -> Result<ServeGraphArgs, clap::Error> {
        let command = ServeGraphArgs::augment_args(Command::new("da"));
        let matches = command.try_get_matches_from(argv)?;
        ServeGraphArgs::from_arg_matches(&matches)
    }

    #[test]
    fn defaults_are_loopback_and_non_redis_port() {
        assert_eq!(DEFAULT_HOST, "127.0.0.1");
        assert_eq!(DEFAULT_PORT, 16379);
        assert_ne!(DEFAULT_PORT, 6379);
    }

    #[test]
    fn server_config_defaults_and_no_persistence() {
        let args = ServeGraphArgs::test_defaults("127.0.0.1", 16379, None);
        let config = args.to_server_config().expect("config");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 16379);
        assert_eq!(config.max_frame_bytes, DEFAULT_MAX_FRAME_BYTES);
        assert_eq!(config.max_response_bytes, DEFAULT_MAX_RESPONSE_BYTES);
        assert_eq!(config.operation_timeout, DEFAULT_OPERATION_TIMEOUT);
        assert_eq!(config.max_connections, DEFAULT_MAX_CONNECTIONS);
        // ReadOnlyServerConfig has no data_path/persistence field at all — the
        // daemon cannot enable RocksDB through the server config (compile-time).
        config.validate().expect("valid config");
    }

    #[test]
    fn server_config_rejects_zero_port_and_limits() {
        let mut args = ServeGraphArgs::test_defaults("127.0.0.1", 0, None);
        let err = args.to_server_config().unwrap_err().to_string();
        assert!(err.contains("port"), "{err}");

        args.port = 16379;
        args.max_frame_bytes = 0;
        assert!(args.to_server_config().is_err());

        let mut args = ServeGraphArgs::test_defaults("127.0.0.1", 16379, None);
        args.max_response_bytes = 0;
        assert!(args.to_server_config().is_err());

        let mut args = ServeGraphArgs::test_defaults("127.0.0.1", 16379, None);
        args.operation_timeout_seconds = 0;
        assert!(args.to_server_config().is_err());

        let mut args = ServeGraphArgs::test_defaults("127.0.0.1", 16379, None);
        args.max_connections = 0;
        assert!(args.to_server_config().is_err());

        let args = ServeGraphArgs::test_defaults("   ", 16379, None);
        assert!(args.to_server_config().is_err());
    }

    #[test]
    fn banners_state_non_persistent_readonly_and_d127() {
        let lines = operator_banners("127.0.0.1", 16379, None).join("\n");
        assert!(lines.contains("NONE"), "must state no persistence");
        assert!(lines.to_lowercase().contains("rocksdb"));
        assert!(lines.contains("import_eligible: false") || lines.contains("D127"));
        assert!(lines.contains("D127"));
        assert!(lines.contains("remote client"));
        assert!(lines.contains("read-only"));
        assert!(lines.contains("DA.IDENTITY"));
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
    fn env_host_port_flow_into_args_and_config() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let _port_env = EnvGuard::set(ReadOnlyServerConfig::ENV_PORT, "23456");
        let _frame_env = EnvGuard::set(ReadOnlyServerConfig::ENV_MAX_FRAME_BYTES, "4096");
        let args = parse_serve_args(&["da"]).expect("parse");
        assert_eq!(args.port, 23456, "env must beat the clap default");
        assert_eq!(args.max_frame_bytes, 4096);
        let config = args.to_server_config().expect("config");
        assert_eq!(config.port, 23456);
        assert_eq!(config.max_frame_bytes, 4096);
    }

    #[test]
    fn explicit_cli_flag_beats_env_value() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(ReadOnlyServerConfig::ENV_PORT, "23456");
        let args = parse_serve_args(&["da", "--port", "23457"]).expect("parse");
        assert_eq!(args.port, 23457, "explicit CLI flag must beat env");
    }

    #[test]
    fn bad_env_values_fail_closed() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(ReadOnlyServerConfig::ENV_PORT, "not-a-number");
        let error = parse_serve_args(&["da"]).unwrap_err();
        assert!(error.to_string().contains("invalid value"), "{error}");
    }

    #[test]
    fn zero_env_port_is_rejected_by_config() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set(ReadOnlyServerConfig::ENV_PORT, "0");
        let args = parse_serve_args(&["da"]).expect("parse");
        let err = args.to_server_config().unwrap_err();
        assert!(err.contains("port"), "{err}");
    }

    #[tokio::test]
    async fn load_missing_snapshot_errors() {
        let store = SamyamaGraphStore::new();
        let err = load_sgsnap_into_store(&store, Path::new("/tmp/definitely-missing-m401.sgsnap"))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("cannot read") || err.contains("No such"),
            "{err}"
        );
    }

    /// Build a tiny real sgsnap via export_tenant for round-trip preload tests.
    /// Returns the exact bytes so tests can fingerprint the artifact. The node
    /// carries a `vid` property — the semantic fingerprint (sgsnap-vid-canonical-json.v1)
    /// requires a unique string vid per node.
    fn write_tiny_sgsnap(path: &Path) -> Vec<u8> {
        let mut store = samyama::GraphStore::new();
        let node = store.create_node(samyama::graph::types::Label::new("Paper"));
        store
            .set_node_property(
                "default",
                node,
                "vid",
                samyama::graph::property::PropertyValue::String("vid:paper:tiny-1".into()),
            )
            .expect("set vid");
        let mut buf = Vec::new();
        samyama::snapshot::export_tenant(&store, &mut buf).expect("export");
        std::fs::write(path, &buf).expect("write");
        buf
    }

    #[tokio::test]
    async fn load_synthetic_sgsnap_restores_nodes_with_identity() {
        let dir = std::env::temp_dir().join(format!("m401-snap-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.sgsnap");
        let bytes = write_tiny_sgsnap(&path);

        let store = SamyamaGraphStore::new();
        let info = load_sgsnap_into_store(&store, &path).await.expect("load");
        assert_eq!(info.bytes, bytes.len());
        assert_eq!(info.nodes, 1);
        assert_eq!(store.node_count().await, 1);

        // Preload identity: the semantic fingerprint of the exported graph
        // bytes must match the preload artifact byte-for-byte.
        let exported = store.export_snapshot().await.expect("export");
        let artifact = fingerprint_snapshot(&bytes).expect("artifact fingerprint");
        let served = fingerprint_snapshot(&exported).expect("served fingerprint");
        assert_eq!(artifact.semantic_sha256, served.semantic_sha256);
        assert_eq!(artifact.node_count, served.node_count);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Connect with retry until the server accepts (bind-to-connect lag).
    async fn connect_with_retry(addr: &str) -> TcpStream {
        use std::time::Duration;
        for _ in 0..50 {
            match TcpStream::connect(addr).await {
                Ok(stream) => return stream,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
        panic!("server never accepted connections on {addr}");
    }

    /// Write a command and read exactly one complete RESP value back.
    async fn roundtrip_value(stream: &mut TcpStream, cmd: &[&str]) -> RespValue {
        stream
            .write_all(&encode_command(cmd))
            .await
            .expect("write command");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            if let Ok(Some((value, _))) = parse_resp(&buf) {
                return value;
            }
            let n = stream.read(&mut chunk).await.expect("read response");
            assert!(n > 0, "connection closed mid-response");
            buf.extend_from_slice(&chunk[..n]);
        }
    }

    #[tokio::test]
    async fn spawn_server_answers_ping() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let args = ServeGraphArgs::test_defaults("127.0.0.1", port, None);
        let task = tokio::spawn(async move {
            let _ = run_in_memory_server(&args).await;
        });

        let mut stream = connect_with_retry(&format!("127.0.0.1:{port}")).await;
        match roundtrip_value(&mut stream, &["PING"]).await {
            RespValue::Simple(text) => assert_eq!(text, "PONG"),
            other => panic!("expected PONG, got {other:?}"),
        }
        drop(stream);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn spawn_with_snapshot_serves_identity_and_rejects_writes() {
        let dir = std::env::temp_dir().join(format!("m401-serve-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.sgsnap");
        let bytes = write_tiny_sgsnap(&path);
        let artifact_identity = fingerprint_snapshot(&bytes).expect("artifact fingerprint");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let args = ServeGraphArgs::test_defaults("127.0.0.1", port, Some(path.clone()));
        let task = tokio::spawn(async move {
            let _ = run_in_memory_server(&args).await;
        });

        let addr = format!("127.0.0.1:{port}");
        let mut stream = connect_with_retry(&addr).await;

        // Health: PING → PONG.
        match roundtrip_value(&mut stream, &["PING"]).await {
            RespValue::Simple(text) => assert_eq!(text, "PONG"),
            other => panic!("expected PONG, got {other:?}"),
        }

        // Semantic identity: DA.IDENTITY reports the preload artifact.
        let identity_json = match roundtrip_value(&mut stream, &["DA.IDENTITY"]).await {
            RespValue::Bulk(json) => json,
            other => panic!("expected identity bulk string, got {other:?}"),
        };
        let identity: SnapshotIdentity =
            SnapshotIdentity::from_json(&identity_json).expect("identity JSON");
        assert_eq!(identity.node_count, 1, "{identity_json}");
        assert_eq!(
            identity.semantic_sha256, artifact_identity.semantic_sha256,
            "preload identity must match the artifact semantic fingerprint"
        );

        // Mutation rejection: raw GRAPH.QUERY CREATE and GRAPH.DELETE are
        // refused server-side (no DirectGraphStore exposure on the wire).
        for cmd in [
            &["GRAPH.QUERY", "0", "CREATE (n:Paper) RETURN n"][..],
            &["GRAPH.QUERY", "0", "CREATE (a)-[:CITES]->(b) RETURN a"][..],
            &["GRAPH.DELETE", "0", "1"][..],
        ] {
            match roundtrip_value(&mut stream, cmd).await {
                RespValue::Error(message) => {
                    assert!(message.contains("not allowed"), "unexpected: {message}")
                }
                other => panic!("expected rejection error, got {other:?}"),
            }
        }

        // Read counts unchanged after the rejected writes.
        let identity_json = match roundtrip_value(&mut stream, &["DA.IDENTITY"]).await {
            RespValue::Bulk(json) => json,
            other => panic!("expected identity bulk string, got {other:?}"),
        };
        let identity: SnapshotIdentity =
            SnapshotIdentity::from_json(&identity_json).expect("identity JSON");
        assert_eq!(
            identity.node_count, 1,
            "rejected writes must not change graph counts, got {identity_json}"
        );

        drop(stream);
        task.abort();
        let _ = task.await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn serve_graph_refuses_data_path_env() {
        let mut env = std::env::vars().collect::<BTreeMap<_, _>>();
        env.insert(
            kg_storage::GraphMode::ENV_DATA_PATH.to_string(),
            "/tmp/da-refuse-test".to_string(),
        );
        let args = ServeGraphArgs::test_defaults("127.0.0.1", 16379, None);
        let err = run_in_memory_server_with_env(&args, &env)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("DA_GRAPH_DATA_PATH"),
            "expected persistence refusal, got {err}"
        );
    }
}
