//! Generic read-only RESP server over `Arc<dyn GraphStore>`.
//!
//! The server is the trust boundary of the read-only graph plane (slice S01,
//! ADR-061 lands in T04): it accepts only `PING`, `ECHO`, `INFO`,
//! `DA.IDENTITY` and `GRAPH.RO_QUERY` (after server-side read-only Cypher
//! validation) and rejects every mutating, index, vector, import, snapshot
//! or administrative command — as well as malformed, oversized and
//! trailing-byte frames — **before** the store is ever touched. Rejected
//! writes therefore cannot change graph state.
//!
//! Bounds: `max_frame_bytes` caps inbound command frames,
//! `max_response_bytes` caps outbound responses, `operation_timeout` bounds
//! each dispatched command, and `max_connections` bounds concurrent clients
//! with a fail-closed busy response when saturated.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::error::{
    EnvParseError, GraphResult, GraphStoreError, env_optional_string, env_u16, env_u64, env_usize,
};
use crate::readonly::validate_readonly_cypher;
use crate::resp::{RespValue, encode_value, parse_resp};
use crate::snapshot_identity::{SnapshotIdentity, fingerprint_snapshot};
use crate::traits::{GraphStore, QueryResult};

/// Structured connection/command events when kg-storage is built with
/// `tracing` (the da-cli path); a no-op in default builds so kg-storage
/// keeps zero unconditional tracing dependency (ADR-052).
#[cfg(feature = "tracing")]
macro_rules! server_trace {
    ($($tokens:tt)*) => {
        tracing::debug!($($tokens)*)
    };
}

#[cfg(not(feature = "tracing"))]
macro_rules! server_trace {
    ($($tokens:tt)*) => {};
}

/// Default bind host: loopback only (a hostile `0.0.0.0` never ships).
pub const DEFAULT_SERVER_HOST: &str = "127.0.0.1";
/// Default port: off Redis default (6379) to avoid accidental clash.
pub const DEFAULT_SERVER_PORT: u16 = 16379;
/// Default inbound command-frame cap.
pub const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;
/// Default outbound response cap (mirrors the remote client's limit).
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Default per-command operation timeout.
pub const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
/// Default concurrent-connection cap.
pub const DEFAULT_MAX_CONNECTIONS: usize = 100;

/// Bounded server configuration. The typed environment contract
/// ([`ReadOnlyServerConfig::from_env`]) resolves documented `DA_SERVE_GRAPH_*`
/// keys with defaults when unset and rejects blank, non-numeric, zero and
/// overflow values fail-closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyServerConfig {
    pub host: String,
    pub port: u16,
    pub max_frame_bytes: usize,
    pub max_response_bytes: usize,
    pub operation_timeout: Duration,
    pub max_connections: usize,
}

impl ReadOnlyServerConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            operation_timeout: DEFAULT_OPERATION_TIMEOUT,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }

    /// Env key: bind host (default [`DEFAULT_SERVER_HOST`]).
    pub const ENV_HOST: &'static str = "DA_SERVE_GRAPH_HOST";
    /// Env key: bind port (default [`DEFAULT_SERVER_PORT`]; zero rejected).
    pub const ENV_PORT: &'static str = "DA_SERVE_GRAPH_PORT";
    /// Env key: inbound frame cap in bytes (default [`DEFAULT_MAX_FRAME_BYTES`]; zero rejected).
    pub const ENV_MAX_FRAME_BYTES: &'static str = "DA_SERVE_GRAPH_MAX_FRAME_BYTES";
    /// Env key: outbound response cap in bytes (default [`DEFAULT_MAX_RESPONSE_BYTES`]; zero rejected).
    pub const ENV_MAX_RESPONSE_BYTES: &'static str = "DA_SERVE_GRAPH_MAX_RESPONSE_BYTES";
    /// Env key: per-command operation timeout in whole seconds (default [`DEFAULT_OPERATION_TIMEOUT`]; zero rejected).
    pub const ENV_OPERATION_TIMEOUT_SECONDS: &'static str =
        "DA_SERVE_GRAPH_OPERATION_TIMEOUT_SECONDS";
    /// Env key: concurrent-connection cap (default [`DEFAULT_MAX_CONNECTIONS`]; zero rejected).
    pub const ENV_MAX_CONNECTIONS: &'static str = "DA_SERVE_GRAPH_MAX_CONNECTIONS";

    /// Read the documented `DA_SERVE_GRAPH_*` keys from the process
    /// environment. Unset keys fall back to defaults; present values are
    /// parsed strictly — blank, non-numeric, zero and overflow are rejected
    /// with a typed [`EnvParseError`] instead of a silent default.
    pub fn from_env() -> Result<Self, EnvParseError> {
        Self::from_env_map(&std::env::vars().collect::<BTreeMap<_, _>>())
    }

    /// Testable map-backed variant of [`Self::from_env`].
    pub fn from_env_map(map: &BTreeMap<String, String>) -> Result<Self, EnvParseError> {
        let host = env_optional_string(map, Self::ENV_HOST)?
            .unwrap_or_else(|| DEFAULT_SERVER_HOST.to_string());
        let port = env_u16(map, Self::ENV_PORT)?.unwrap_or(DEFAULT_SERVER_PORT);
        let max_frame_bytes =
            env_usize(map, Self::ENV_MAX_FRAME_BYTES)?.unwrap_or(DEFAULT_MAX_FRAME_BYTES);
        let max_response_bytes =
            env_usize(map, Self::ENV_MAX_RESPONSE_BYTES)?.unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
        let operation_timeout_secs = env_u64(map, Self::ENV_OPERATION_TIMEOUT_SECONDS)?
            .unwrap_or(DEFAULT_OPERATION_TIMEOUT.as_secs());
        let max_connections =
            env_usize(map, Self::ENV_MAX_CONNECTIONS)?.unwrap_or(DEFAULT_MAX_CONNECTIONS);
        if port == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_PORT,
                value: "0".into(),
            });
        }
        if max_frame_bytes == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_MAX_FRAME_BYTES,
                value: "0".into(),
            });
        }
        if max_response_bytes == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_MAX_RESPONSE_BYTES,
                value: "0".into(),
            });
        }
        if operation_timeout_secs == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_OPERATION_TIMEOUT_SECONDS,
                value: "0".into(),
            });
        }
        if max_connections == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_MAX_CONNECTIONS,
                value: "0".into(),
            });
        }
        Ok(Self {
            host,
            port,
            max_frame_bytes,
            max_response_bytes,
            operation_timeout: Duration::from_secs(operation_timeout_secs),
            max_connections,
        })
    }

    pub fn validate(&self) -> GraphResult<()> {
        if self.host.trim().is_empty() {
            return Err(GraphStoreError::Storage(
                "read-only server host must not be blank".into(),
            ));
        }
        if self.max_frame_bytes == 0 {
            return Err(GraphStoreError::Storage(
                "read-only server max_frame_bytes must be greater than zero".into(),
            ));
        }
        if self.max_response_bytes == 0 {
            return Err(GraphStoreError::Storage(
                "read-only server max_response_bytes must be greater than zero".into(),
            ));
        }
        if self.operation_timeout.is_zero() {
            return Err(GraphStoreError::Storage(
                "read-only server operation_timeout must be greater than zero".into(),
            ));
        }
        if self.max_connections == 0 {
            return Err(GraphStoreError::Storage(
                "read-only server max_connections must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

/// A generic bounded read-only RESP server over one shared graph store.
#[derive(Clone)]
pub struct ReadOnlyServer {
    inner: Arc<ReadOnlyServerInner>,
}

struct ReadOnlyServerInner {
    config: ReadOnlyServerConfig,
    store: Arc<dyn GraphStore>,
    /// DA.IDENTITY JSON payload; None until the operator preloads/computes it
    /// (T02 computes it from exported graph bytes, T03 wires the preload).
    /// std Mutex: access is a brief clone, never held across an await.
    identity: Mutex<Option<String>>,
    permits: Arc<Semaphore>,
}

impl ReadOnlyServer {
    pub fn new(config: ReadOnlyServerConfig, store: Arc<dyn GraphStore>) -> GraphResult<Self> {
        config.validate()?;
        let max_connections = config.max_connections;
        Ok(Self {
            inner: Arc::new(ReadOnlyServerInner {
                config,
                store,
                identity: Mutex::new(None),
                permits: Arc::new(Semaphore::new(max_connections)),
            }),
        })
    }

    pub fn config(&self) -> &ReadOnlyServerConfig {
        &self.inner.config
    }

    /// Builder: pre-set the DA.IDENTITY JSON payload before serving.
    pub fn with_identity(self, json: String) -> Self {
        self.inner.identity.lock().unwrap().replace(json);
        self
    }

    /// Replace the DA.IDENTITY payload at runtime (e.g. after a preload).
    pub fn set_identity(&self, json: Option<String>) {
        *self.inner.identity.lock().unwrap() = json;
    }

    /// Compute the deterministic [`SnapshotIdentity`] from the store's
    /// exported graph bytes (T02). The identity is therefore bound to the
    /// exact graph state this server serves — a second process can verify
    /// semantic identity against it via the remote client.
    pub async fn compute_identity(&self) -> GraphResult<SnapshotIdentity> {
        let bytes = self.inner.store.export_snapshot().await?;
        fingerprint_snapshot(&bytes).map_err(|error| {
            GraphStoreError::Storage(format!(
                "failed to compute snapshot identity from exported graph bytes: {error}"
            ))
        })
    }

    /// Builder: compute DA.IDENTITY from the store's exported graph bytes
    /// now and pre-set the JSON payload before serving (T02).
    pub async fn with_identity_from_store(self) -> GraphResult<Self> {
        let identity = self.compute_identity().await?;
        let json = identity.to_json().map_err(|error| {
            GraphStoreError::Storage(format!("failed to serialize snapshot identity: {error}"))
        })?;
        Ok(self.with_identity(json))
    }

    /// Bind the listener. Port 0 picks an ephemeral port (tests, T03 uses
    /// the returned listener to learn the real bound address).
    pub async fn bind(&self) -> Result<TcpListener, std::io::Error> {
        TcpListener::bind((self.inner.config.host.as_str(), self.inner.config.port)).await
    }

    /// Accept loop over an already-bound listener. Each connection gets its
    /// own task bounded by `max_connections`; a saturated server refuses new
    /// clients with a visible error instead of queueing them.
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<(), std::io::Error> {
        loop {
            let (socket, peer) = listener.accept().await?;
            let server = self.clone();
            let permit = match self.inner.permits.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // Fail closed: refuse with a visible error instead of
                    // queueing. Drain any unread client bytes first so the
                    // close is a clean FIN (EOF), not a TCP RST.
                    let mut socket = socket;
                    let _ = socket
                        .write_all(b"-ERR server is at max connections\r\n")
                        .await;
                    let mut discard = [0_u8; 512];
                    let _ =
                        tokio::time::timeout(Duration::from_millis(200), socket.read(&mut discard))
                            .await;
                    let _ = socket.shutdown().await;
                    continue;
                }
            };
            tokio::spawn(async move {
                let _permit = permit;
                server.handle_connection(socket, peer).await;
            });
        }
    }

    /// Bind and serve until a server error (or the task is aborted).
    pub async fn serve(&self) -> Result<(), std::io::Error> {
        let listener = self.bind().await?;
        self.serve_listener(listener).await
    }

    /// Per-connection loop: multiple frames per connection, bounded input,
    /// bounded responses, bounded operation time. Protocol violations
    /// (malformed, oversized, trailing-byte frames) are answered with an
    /// error and the connection is closed; ordinary command rejections are
    /// answered and the connection stays usable.
    async fn handle_connection(&self, socket: TcpStream, _peer: std::net::SocketAddr) {
        server_trace!(peer = %_peer, "read-only server: connection accepted");
        let mut socket = socket;
        let mut pending = Vec::with_capacity(4096);
        let mut chunk = [0_u8; 8192];
        loop {
            if pending.is_empty() {
                match socket.read(&mut chunk).await {
                    Ok(0) => return, // clean EOF between frames
                    Ok(n) => pending.extend_from_slice(&chunk[..n]),
                    Err(error) => {
                        self.write_error_and_close(
                            &mut socket,
                            &format!("ERR read failed: {error}"),
                        )
                        .await;
                        return;
                    }
                }
            }
            if pending.len() > self.inner.config.max_frame_bytes {
                self.write_error_and_close(
                    &mut socket,
                    &format!(
                        "ERR frame exceeds {} byte limit",
                        self.inner.config.max_frame_bytes
                    ),
                )
                .await;
                return;
            }
            match parse_resp(&pending) {
                Ok(Some((value, consumed))) => {
                    pending = pending[consumed..].to_vec();
                    let response =
                        match timeout(self.inner.config.operation_timeout, self.dispatch(value))
                            .await
                        {
                            Ok(response) => response,
                            Err(_) => {
                                self.write_error_and_close(
                                    &mut socket,
                                    &format!(
                                        "ERR operation timed out after {} ms",
                                        self.inner.config.operation_timeout.as_millis()
                                    ),
                                )
                                .await;
                                return;
                            }
                        };
                    let encoded = encode_value(&response);
                    if encoded.len() > self.inner.config.max_response_bytes {
                        self.write_error_and_close(
                            &mut socket,
                            &format!(
                                "ERR response exceeds {} byte limit",
                                self.inner.config.max_response_bytes
                            ),
                        )
                        .await;
                        return;
                    }
                    if socket.write_all(&encoded).await.is_err() {
                        return;
                    }
                }
                Ok(None) => match socket.read(&mut chunk).await {
                    Ok(0) => return, // EOF mid-frame
                    Ok(n) => pending.extend_from_slice(&chunk[..n]),
                    Err(error) => {
                        self.write_error_and_close(
                            &mut socket,
                            &format!("ERR read failed: {error}"),
                        )
                        .await;
                        return;
                    }
                },
                Err(error) => {
                    self.write_error_and_close(
                        &mut socket,
                        &format!("ERR protocol error: {error}"),
                    )
                    .await;
                    return;
                }
            }
        }
    }

    async fn write_error_and_close(&self, socket: &mut TcpStream, message: &str) {
        let _ = socket
            .write_all(encode_value(&RespValue::Error(message.into())).as_slice())
            .await;
        let _ = socket.shutdown().await;
    }

    /// Server-side command FSM: only PING, ECHO, INFO, DA.IDENTITY and
    /// GRAPH.RO_QUERY (after read-only Cypher validation) are accepted.
    /// Every other command class is rejected before the store is touched.
    async fn dispatch(&self, value: RespValue) -> RespValue {
        let RespValue::Array(args) = value else {
            return RespValue::Error("ERR command must be a RESP array".into());
        };
        if args.is_empty() {
            return RespValue::Error("ERR empty command".into());
        }
        let Some(name) = args.first().and_then(RespValue::as_str) else {
            return RespValue::Error("ERR command name must be a string".into());
        };
        let upper = name.to_uppercase();
        match command_disposition(&upper) {
            CommandDisposition::Allowed => match upper.as_str() {
                "PING" => ping_response(&args),
                "ECHO" => echo_response(&args),
                "INFO" => RespValue::Bulk(self.info_string()),
                "DA.IDENTITY" => self.identity_response().await,
                "GRAPH.RO_QUERY" => self.ro_query_response(&args).await,
                _ => RespValue::Error(format!("ERR unknown command '{upper}'")),
            },
            CommandDisposition::Rejected(reason) => {
                server_trace!(command = %upper, "read-only server rejected command");
                RespValue::Error(format!("ERR {reason}"))
            }
            CommandDisposition::Unknown => {
                server_trace!(command = %upper, "read-only server rejected unknown command");
                RespValue::Error(format!("ERR unknown command '{upper}'"))
            }
        }
    }

    async fn ro_query_response(&self, args: &[RespValue]) -> RespValue {
        if args.len() != 3 {
            return RespValue::Error(
                "ERR wrong number of arguments for 'GRAPH.RO_QUERY': expected GRAPH.RO_QUERY <graph> <cypher>"
                    .into(),
            );
        }
        let graph = args[1].as_str().unwrap_or_default();
        let Some(cypher) = args[2].as_str() else {
            return RespValue::Error("ERR GRAPH.RO_QUERY cypher must be a string".into());
        };
        // Server-side read-only validation happens BEFORE the store is
        // touched, so mutating Cypher is rejected even against a hostile
        // client that never runs the client-side validator.
        if let Err(error) = validate_readonly_cypher(cypher) {
            return RespValue::Error(format!("ERR {error}"));
        }
        match self.inner.store.query_readonly(graph, cypher).await {
            Ok(result) => query_result_to_resp(&result),
            Err(error) => RespValue::Error(format!("ERR {error}")),
        }
    }

    async fn identity_response(&self) -> RespValue {
        let identity = self.inner.identity.lock().unwrap().clone();
        match identity {
            Some(json) => RespValue::Bulk(json),
            None => RespValue::Error("ERR DA.IDENTITY is not configured for this server".into()),
        }
    }

    /// Deterministic, secret-free server info.
    fn info_string(&self) -> String {
        format!(
            "# Server\r\nversion: {}\r\nmode: readonly\r\nprotocol: resp2\r\naccepted_commands: PING ECHO INFO DA.IDENTITY GRAPH.RO_QUERY\r\n# Limits\r\nmax_frame_bytes: {}\r\nmax_response_bytes: {}\r\noperation_timeout_ms: {}\r\nmax_connections: {}\r\n",
            env!("CARGO_PKG_VERSION"),
            self.inner.config.max_frame_bytes,
            self.inner.config.max_response_bytes,
            self.inner.config.operation_timeout.as_millis(),
            self.inner.config.max_connections,
        )
    }
}

/// Fail-closed command classification. The allowlist is exactly the
/// read-plane contract; every mutating, index, vector, import, snapshot or
/// administrative command is rejected before dispatch reaches the store.
enum CommandDisposition {
    Allowed,
    Rejected(&'static str),
    Unknown,
}

fn command_disposition(name: &str) -> CommandDisposition {
    match name {
        "PING" | "ECHO" | "INFO" | "DA.IDENTITY" | "GRAPH.RO_QUERY" => CommandDisposition::Allowed,
        "GRAPH.QUERY" => CommandDisposition::Rejected(
            "mutating command GRAPH.QUERY is not allowed on the read-only plane; use GRAPH.RO_QUERY",
        ),
        "GRAPH.DELETE" => CommandDisposition::Rejected(
            "mutating command GRAPH.DELETE is not allowed on the read-only plane",
        ),
        "GRAPH.LIST" => CommandDisposition::Rejected(
            "administrative command GRAPH.LIST is not allowed on the read-only plane",
        ),
        n if n.starts_with("GRAPH.") => CommandDisposition::Rejected(
            "unsupported GRAPH command is not allowed on the read-only plane",
        ),
        n if n.starts_with("INDEX") || n.starts_with("VECTOR") => CommandDisposition::Rejected(
            "index or vector command is not allowed on the read-only plane",
        ),
        n if n.starts_with("IMPORT") || n.starts_with("SNAPSHOT") => CommandDisposition::Rejected(
            "import or snapshot command is not allowed on the read-only plane",
        ),
        n if n.starts_with("ADMIN")
            || matches!(
                n,
                "CONFIG"
                    | "SHUTDOWN"
                    | "FLUSHALL"
                    | "FLUSHDB"
                    | "SAVE"
                    | "BGSAVE"
                    | "DEBUG"
                    | "MONITOR"
                    | "CLIENT"
                    | "AUTH"
                    | "ACL"
                    | "COMMAND"
                    | "KEYS"
                    | "SCAN"
                    | "DBSIZE"
            ) =>
        {
            CommandDisposition::Rejected(
                "administrative command is not allowed on the read-only plane",
            )
        }
        _ => CommandDisposition::Unknown,
    }
}

fn ping_response(args: &[RespValue]) -> RespValue {
    match args.len() {
        1 => RespValue::Simple("PONG".into()),
        2 => match args[1].as_str() {
            Some(message) => RespValue::Bulk(message.into()),
            None => RespValue::Error("ERR PING message must be a string".into()),
        },
        _ => RespValue::Error("ERR wrong number of arguments for 'PING' command".into()),
    }
}

fn echo_response(args: &[RespValue]) -> RespValue {
    match args.len() {
        2 => match args[1].as_str() {
            Some(message) => RespValue::Bulk(message.into()),
            None => RespValue::Error("ERR ECHO message must be a string".into()),
        },
        _ => RespValue::Error("ERR wrong number of arguments for 'ECHO' command".into()),
    }
}

/// Encode a [`QueryResult`] as a RESP array: header row of bulk column names
/// followed by one array per record. Integers stay integers, nulls stay
/// nulls, everything else becomes a JSON-text bulk string — the exact shape
/// the remote client's `query_result_from_resp` decodes.
fn query_result_to_resp(result: &QueryResult) -> RespValue {
    let mut rows = Vec::with_capacity(result.records.len() + 1);
    let header = result
        .columns
        .iter()
        .map(|column| RespValue::Bulk(column.clone()))
        .collect();
    rows.push(RespValue::Array(header));
    for record in &result.records {
        rows.push(RespValue::Array(
            record.iter().map(json_to_resp_scalar).collect(),
        ));
    }
    RespValue::Array(rows)
}

fn json_to_resp_scalar(value: &serde_json::Value) -> RespValue {
    match value {
        serde_json::Value::Null => RespValue::Null,
        serde_json::Value::Number(number) if number.is_i64() => {
            RespValue::Integer(number.as_i64().unwrap_or_default())
        }
        other => RespValue::Bulk(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockGraphStore;
    use crate::resp::encode_command;
    use crate::traits::{DirectGraphStore, VectorMetric, VectorSearchResult};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinHandle;

    /// Records read-query calls and refuses the mutable query path so tests
    /// prove rejected commands never touch the store.
    #[derive(Clone)]
    struct RecordingStore {
        inner: MockGraphStore,
        read_calls: Arc<AtomicUsize>,
        last_query: Arc<Mutex<Option<(String, String)>>>,
        read_delay: Duration,
    }

    impl RecordingStore {
        fn new() -> Self {
            Self {
                inner: MockGraphStore::new(),
                read_calls: Arc::new(AtomicUsize::new(0)),
                last_query: Arc::new(Mutex::new(None)),
                read_delay: Duration::ZERO,
            }
        }

        fn with_read_delay(delay: Duration) -> Self {
            let mut store = Self::new();
            store.read_delay = delay;
            store
        }

        fn read_calls(&self) -> usize {
            self.read_calls.load(Ordering::SeqCst)
        }

        fn last_query(&self) -> Option<(String, String)> {
            self.last_query.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl GraphStore for RecordingStore {
        async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
            Err(GraphStoreError::NotImplemented(
                "recording store has no mutable query path; the server must reject before this is called"
                    .into(),
            ))
        }

        async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            *self.last_query.lock().unwrap() = Some((graph.to_string(), cypher.to_string()));
            if !self.read_delay.is_zero() {
                tokio::time::sleep(self.read_delay).await;
            }
            Ok(QueryResult {
                columns: vec!["vid".into(), "label".into()],
                records: vec![vec![
                    serde_json::json!("paper-1"),
                    serde_json::json!("Paper"),
                ]],
            })
        }

        async fn create_vector_index(
            &self,
            _label: &str,
            _property: &str,
            _dimensions: usize,
            _metric: VectorMetric,
        ) -> GraphResult<()> {
            Err(GraphStoreError::NotImplemented("no vector index".into()))
        }

        async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
            Err(GraphStoreError::NotImplemented("no property index".into()))
        }

        async fn vector_search(
            &self,
            _label: &str,
            _property: &str,
            _query_vector: &[f32],
            _k: usize,
        ) -> GraphResult<Vec<VectorSearchResult>> {
            Err(GraphStoreError::NotImplemented("no vector search".into()))
        }

        async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
            self.inner.export_snapshot().await
        }

        async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
            Err(GraphStoreError::NotImplemented(
                "read-only plane never imports".into(),
            ))
        }

        async fn health(&self) -> GraphResult<bool> {
            Ok(true)
        }
    }

    fn test_config() -> ReadOnlyServerConfig {
        ReadOnlyServerConfig::new("127.0.0.1", 0)
    }

    async fn start(
        store: Arc<dyn GraphStore>,
        config: ReadOnlyServerConfig,
    ) -> (String, JoinHandle<()>) {
        let server = ReadOnlyServer::new(config, store).expect("server config valid");
        let listener = server.bind().await.expect("bind");
        let endpoint = listener.local_addr().expect("local addr").to_string();
        let task = tokio::spawn(async move {
            let _ = server.serve_listener(listener).await;
        });
        (endpoint, task)
    }

    async fn connect(endpoint: &str) -> TcpStream {
        TcpStream::connect(endpoint).await.expect("connect")
    }

    /// Read exactly one complete RESP frame (pipelined frames stay queued).
    async fn read_one_frame(stream: &mut TcpStream) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            match timeout(Duration::from_secs(2), stream.read(&mut chunk)).await {
                Ok(Ok(0)) => panic!("EOF before a complete frame"),
                Ok(Ok(n)) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    if let Ok(Some((_, consumed))) = parse_resp(&buffer) {
                        return buffer[..consumed].to_vec();
                    }
                }
                Ok(Err(error)) => panic!("read error: {error}"),
                Err(_) => panic!("timed out waiting for a frame"),
            }
        }
    }

    fn frame_text(frame: &[u8]) -> String {
        String::from_utf8_lossy(frame).to_string()
    }

    /// Test TCP client with a persistent parse buffer: pipelined responses
    /// that arrive in one read are preserved across frame reads.
    struct TestClient {
        stream: TcpStream,
        buffer: Vec<u8>,
    }

    impl TestClient {
        async fn connect(endpoint: &str) -> Self {
            Self {
                stream: TcpStream::connect(endpoint).await.expect("connect"),
                buffer: Vec::new(),
            }
        }

        async fn write(&mut self, bytes: &[u8]) {
            self.stream.write_all(bytes).await.unwrap();
        }

        /// Read exactly one complete RESP frame; extra bytes stay buffered.
        async fn read_frame(&mut self) -> Vec<u8> {
            loop {
                if let Ok(Some((_, consumed))) = parse_resp(&self.buffer) {
                    let frame = self.buffer[..consumed].to_vec();
                    self.buffer.drain(..consumed);
                    return frame;
                }
                let mut chunk = [0_u8; 8192];
                match timeout(Duration::from_secs(2), self.stream.read(&mut chunk)).await {
                    Ok(Ok(0)) => panic!("EOF before a complete frame"),
                    Ok(Ok(n)) => self.buffer.extend_from_slice(&chunk[..n]),
                    Ok(Err(error)) => panic!("read error: {error}"),
                    Err(_) => panic!("timed out waiting for a frame"),
                }
            }
        }

        /// Read raw bytes (for EOF / close assertions).
        async fn read_raw(&mut self) -> std::io::Result<usize> {
            let mut chunk = [0_u8; 64];
            self.stream.read(&mut chunk).await
        }
    }

    #[tokio::test]
    async fn read_only_server_accepts_ping_echo_info() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;

        stream.write_all(&encode_command(&["PING"])).await.unwrap();
        assert_eq!(frame_text(&read_one_frame(&mut stream).await), "+PONG\r\n");

        stream
            .write_all(&encode_command(&["PING", "hi"]))
            .await
            .unwrap();
        assert_eq!(
            frame_text(&read_one_frame(&mut stream).await),
            "$2\r\nhi\r\n"
        );

        stream
            .write_all(&encode_command(&["ECHO", "hello"]))
            .await
            .unwrap();
        assert_eq!(
            frame_text(&read_one_frame(&mut stream).await),
            "$5\r\nhello\r\n"
        );

        stream.write_all(&encode_command(&["INFO"])).await.unwrap();
        let info = frame_text(&read_one_frame(&mut stream).await);
        assert!(info.contains("mode: readonly"), "{info}");
        assert!(info.contains("GRAPH.RO_QUERY"), "{info}");
        assert!(!info.contains("api_key"), "no secrets in INFO: {info}");

        // DA.IDENTITY without a configured identity is a visible typed error.
        stream
            .write_all(&encode_command(&["DA.IDENTITY"]))
            .await
            .unwrap();
        let identity = frame_text(&read_one_frame(&mut stream).await);
        assert!(identity.starts_with("-ERR"), "{identity}");
        assert!(identity.contains("DA.IDENTITY"), "{identity}");

        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_identity_returns_stored_json() {
        let store = Arc::new(RecordingStore::new());
        let server = ReadOnlyServer::new(test_config(), store).expect("config");
        let identity = r#"{"semantic_sha256":"abc123","node_count":2,"edge_count":1}"#;
        let server = server.with_identity(identity.to_string());
        let listener = server.bind().await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move {
            let _ = server.serve_listener(listener).await;
        });
        let mut stream = connect(&endpoint).await;
        stream
            .write_all(&encode_command(&["DA.IDENTITY"]))
            .await
            .unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert_eq!(frame, format!("${}\r\n{identity}\r\n", identity.len()));
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_accepts_validated_ro_query_and_forwards_to_store() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;
        stream
            .write_all(&encode_command(&[
                "GRAPH.RO_QUERY",
                "g",
                "MATCH (n) RETURN n.vid",
            ]))
            .await
            .unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.starts_with("*2\r\n"), "{frame}"); // header + 1 record
        assert!(frame.contains("vid"), "{frame}");
        assert!(frame.contains("paper-1"), "{frame}");
        assert_eq!(store.read_calls(), 1);
        assert_eq!(
            store.last_query(),
            Some(("g".to_string(), "MATCH (n) RETURN n.vid".to_string()))
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_mutating_cypher_in_ro_query_before_store() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;
        for cypher in [
            "CREATE (n)",
            "MATCH (n) SET n.x = 1 RETURN n",
            "MATCH (n) DETACH DELETE n",
            "MERGE (n:Paper {id: 'x'}) RETURN n",
        ] {
            stream
                .write_all(&encode_command(&["GRAPH.RO_QUERY", "g", cypher]))
                .await
                .unwrap();
            let frame = frame_text(&read_one_frame(&mut stream).await);
            assert!(frame.starts_with("-ERR"), "{cypher} -> {frame}");
            assert!(frame.contains("read-only"), "{cypher} -> {frame}");
        }
        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_every_mutation_and_admin_class_before_store() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;

        let cases: &[(&[&str], &str)] = &[
            (&["GRAPH.QUERY", "g", "CREATE (n) RETURN n"], "GRAPH.QUERY"),
            (&["GRAPH.QUERY", "g", "MATCH (n) DELETE n"], "GRAPH.QUERY"),
            (&["GRAPH.DELETE", "g"], "GRAPH.DELETE"),
            (&["GRAPH.LIST"], "GRAPH.LIST"),
            (&["GRAPH.CREATE_INDEX", "g"], "not allowed"),
            (&["INDEX.CREATE", "Paper", "title"], "index"),
            (&["VECTOR.ADD", "Paper", "embedding"], "vector"),
            (&["VECTOR.SEARCH", "Paper", "embedding"], "vector"),
            (&["IMPORT.SNAPSHOT", "g"], "import"),
            (&["SNAPSHOT.IMPORT", "g"], "import"),
            (&["ADMIN.SHUTDOWN"], "administrative"),
            (&["CONFIG", "SET", "save", "900 1"], "administrative"),
            (&["FLUSHALL"], "administrative"),
            (&["AUTH", "secret"], "administrative"),
            (&["KEYS", "*"], "administrative"),
        ];

        for (args, expected) in cases {
            stream.write_all(&encode_command(args)).await.unwrap();
            let frame = frame_text(&read_one_frame(&mut stream).await);
            assert!(frame.starts_with("-ERR"), "{args:?} -> {frame}");
            assert!(
                frame.contains(expected),
                "{args:?} -> {frame} (expected {expected})"
            );
        }

        // The store was never touched: no read calls and no state change.
        assert_eq!(store.read_calls(), 0);
        assert_eq!(store.inner.node_count_total(), 0);
        assert_eq!(store.inner.edge_count_total(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_unknown_commands() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;
        for cmd in ["FOO", "GET", "SET", "DEL"] {
            stream.write_all(&encode_command(&[cmd])).await.unwrap();
            let frame = frame_text(&read_one_frame(&mut stream).await);
            assert!(frame.starts_with("-ERR"), "{cmd} -> {frame}");
            assert!(frame.contains("unknown command"), "{cmd} -> {frame}");
        }
        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_non_array_and_empty_commands() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;

        // Plain string frame (not an array) -> typed error, connection usable.
        stream.write_all(b"+PING\r\n").await.unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.contains("must be a RESP array"), "{frame}");

        // Empty array -> error.
        stream.write_all(b"*0\r\n").await.unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.contains("empty command"), "{frame}");

        // Integer as command name -> error.
        stream.write_all(b"*1\r\n:5\r\n").await.unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.contains("command name must be a string"), "{frame}");

        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_malformed_frames_and_closes() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;

        stream.write_all(b"!boom\r\n").await.unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.starts_with("-ERR"), "{frame}");
        assert!(frame.contains("protocol error"), "{frame}");

        // Connection is closed after a protocol violation: next read is EOF.
        let mut chunk = [0_u8; 64];
        let read = timeout(Duration::from_secs(2), stream.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read, 0, "connection must close after malformed frame");

        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_oversized_frames_before_parse() {
        let store = Arc::new(RecordingStore::new());
        let mut config = test_config();
        config.max_frame_bytes = 32;
        let (endpoint, task) = start(store.clone(), config).await;
        let mut stream = connect(&endpoint).await;

        let big = "x".repeat(128);
        stream
            .write_all(&encode_command(&["ECHO", &big]))
            .await
            .unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.starts_with("-ERR"), "{frame}");
        assert!(frame.contains("frame exceeds 32 byte limit"), "{frame}");

        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_bounds_oversized_responses() {
        let store = Arc::new(RecordingStore::new());
        let mut config = test_config();
        config.max_response_bytes = 32;
        let (endpoint, task) = start(store.clone(), config).await;
        let mut stream = connect(&endpoint).await;

        stream
            .write_all(&encode_command(&[
                "GRAPH.RO_QUERY",
                "g",
                "MATCH (n) RETURN n",
            ]))
            .await
            .unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.starts_with("-ERR"), "{frame}");
        assert!(frame.contains("response exceeds 32 byte limit"), "{frame}");

        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_bounds_slow_operations_with_timeout() {
        let store = Arc::new(RecordingStore::with_read_delay(Duration::from_secs(10)));
        let mut config = test_config();
        config.operation_timeout = Duration::from_millis(50);
        let (endpoint, task) = start(store.clone(), config).await;
        let mut stream = connect(&endpoint).await;

        stream
            .write_all(&encode_command(&[
                "GRAPH.RO_QUERY",
                "g",
                "MATCH (n) RETURN n",
            ]))
            .await
            .unwrap();
        let frame = frame_text(&read_one_frame(&mut stream).await);
        assert!(frame.starts_with("-ERR"), "{frame}");
        assert!(frame.contains("timed out after 50 ms"), "{frame}");

        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_handles_multiple_frames_per_connection() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut client = TestClient::connect(&endpoint).await;

        // Pipeline three commands in a single write.
        let mut batch = Vec::new();
        batch.extend_from_slice(&encode_command(&["PING"]));
        batch.extend_from_slice(&encode_command(&["ECHO", "a"]));
        batch.extend_from_slice(&encode_command(&["ECHO", "b"]));
        client.write(&batch).await;

        assert_eq!(frame_text(&client.read_frame().await), "+PONG\r\n");
        assert_eq!(frame_text(&client.read_frame().await), "$1\r\na\r\n");
        assert_eq!(frame_text(&client.read_frame().await), "$1\r\nb\r\n");

        // Connection still usable afterwards.
        client.write(&encode_command(&["PING"])).await;
        assert_eq!(frame_text(&client.read_frame().await), "+PONG\r\n");

        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejects_trailing_bytes_after_frame() {
        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut client = TestClient::connect(&endpoint).await;

        let mut poisoned = encode_command(&["PING"]);
        poisoned.extend_from_slice(b"GARBAGE");
        client.write(&poisoned).await;

        // The valid frame is still answered...
        assert_eq!(frame_text(&client.read_frame().await), "+PONG\r\n");
        // ...then the trailing bytes are a protocol error and the connection closes.
        let frame = frame_text(&client.read_frame().await);
        assert!(frame.starts_with("-ERR"), "{frame}");
        assert!(frame.contains("protocol error"), "{frame}");
        let read = timeout(Duration::from_secs(2), client.read_raw())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read, 0);

        assert_eq!(store.read_calls(), 0);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_rejected_writes_leave_seeded_mock_state_unchanged() {
        let store = Arc::new(RecordingStore::new());
        // Seed two nodes through the HOT-path direct surface.
        store.inner.create_node("Paper").await.unwrap();
        store.inner.create_node("Entity").await.unwrap();
        assert_eq!(store.inner.node_count_total(), 2);

        let (endpoint, task) = start(store.clone(), test_config()).await;
        let mut stream = connect(&endpoint).await;
        for args in [
            &["GRAPH.QUERY", "g", "CREATE (n) RETURN n"][..],
            &["GRAPH.DELETE", "g"][..],
            &["FLUSHALL"][..],
            &["VECTOR.ADD", "Paper", "v"][..],
            &["INDEX.CREATE", "Paper", "title"][..],
        ] {
            stream.write_all(&encode_command(args)).await.unwrap();
            let frame = frame_text(&read_one_frame(&mut stream).await);
            assert!(frame.starts_with("-ERR"), "{args:?} -> {frame}");
        }
        // Graph counts are unchanged after every rejected write.
        assert_eq!(store.inner.node_count_total(), 2);
        assert_eq!(store.inner.edge_count_total(), 0);
        task.abort();
        let _ = task.await;
    }

    #[test]
    fn config_rejects_zero_limits_and_blank_host() {
        let mut config = test_config();
        config.max_frame_bytes = 0;
        assert!(config.validate().is_err());

        let mut config = test_config();
        config.max_response_bytes = 0;
        assert!(config.validate().is_err());

        let mut config = test_config();
        config.operation_timeout = Duration::ZERO;
        assert!(config.validate().is_err());

        let mut config = test_config();
        config.max_connections = 0;
        assert!(config.validate().is_err());

        let mut config = test_config();
        config.host = "  ".into();
        assert!(config.validate().is_err());

        // Defaults are valid and match the da-cli operator contract.
        let defaults = ReadOnlyServerConfig::new(DEFAULT_SERVER_HOST, DEFAULT_SERVER_PORT);
        assert!(defaults.validate().is_ok());
        assert_eq!(defaults.port, 16379);
    }

    /// A deterministic, valid gzip sgsnap fixture: 2 nodes, 1 edge.
    /// Served via MockGraphStore::set_snapshot_data so the server's
    /// export_snapshot() returns genuine sgsnap bytes for fingerprinting.
    fn canned_sgsnap() -> Vec<u8> {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let header = serde_json::json!({
            "format": "sgsnap", "version": 2, "tenant": "daily_archive",
            "node_count": 2, "edge_count": 1,
            "labels": [], "edge_types": [],
            "created_at": "2026-08-18T00:00:00Z", "samyama_version": "x"
        });
        let records = vec![
            serde_json::json!({"t":"n","id":1,"labels":["Paper"],"props":{"vid":"paper-a","title":"A"}}),
            serde_json::json!({"t":"n","id":2,"labels":["Entity"],"props":{"vid":"entity-b","name":"B"}}),
            serde_json::json!({"t":"e","id":3,"src":1,"tgt":2,"type":"MENTIONS","props":{"confidence":0.9}}),
        ];
        let text = std::iter::once(header)
            .chain(records)
            .map(|value| serde_json::to_string(&value).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn server_config_from_env_parses_all_keys_and_defaults() {
        let mut map = BTreeMap::new();
        map.insert(
            ReadOnlyServerConfig::ENV_HOST.to_string(),
            " 0.0.0.0 ".to_string(),
        );
        map.insert(
            ReadOnlyServerConfig::ENV_PORT.to_string(),
            "17000".to_string(),
        );
        map.insert(
            ReadOnlyServerConfig::ENV_MAX_FRAME_BYTES.to_string(),
            "2048".to_string(),
        );
        map.insert(
            ReadOnlyServerConfig::ENV_MAX_RESPONSE_BYTES.to_string(),
            "4096".to_string(),
        );
        map.insert(
            ReadOnlyServerConfig::ENV_OPERATION_TIMEOUT_SECONDS.to_string(),
            "7".to_string(),
        );
        map.insert(
            ReadOnlyServerConfig::ENV_MAX_CONNECTIONS.to_string(),
            "12".to_string(),
        );
        let config = ReadOnlyServerConfig::from_env_map(&map).expect("parse");
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 17000);
        assert_eq!(config.max_frame_bytes, 2048);
        assert_eq!(config.max_response_bytes, 4096);
        assert_eq!(config.operation_timeout, Duration::from_secs(7));
        assert_eq!(config.max_connections, 12);
        assert!(config.validate().is_ok());

        // Empty map: every documented key falls back to its default.
        let defaults = ReadOnlyServerConfig::from_env_map(&BTreeMap::new()).expect("defaults");
        assert_eq!(
            defaults,
            ReadOnlyServerConfig::new(DEFAULT_SERVER_HOST, DEFAULT_SERVER_PORT)
        );
    }

    #[test]
    fn server_config_from_env_rejects_blank_non_numeric_zero_and_overflow() {
        use crate::error::EnvParseError;

        let blank = [(
            ReadOnlyServerConfig::ENV_HOST.to_string(),
            "   ".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert_eq!(
            ReadOnlyServerConfig::from_env_map(&blank).unwrap_err(),
            EnvParseError::Blank {
                key: ReadOnlyServerConfig::ENV_HOST
            }
        );

        for (key, raw) in [
            (ReadOnlyServerConfig::ENV_PORT, "not-a-port"),
            (ReadOnlyServerConfig::ENV_MAX_FRAME_BYTES, "1.5"),
            (ReadOnlyServerConfig::ENV_MAX_RESPONSE_BYTES, "-8"),
            (ReadOnlyServerConfig::ENV_OPERATION_TIMEOUT_SECONDS, "10s"),
            (ReadOnlyServerConfig::ENV_MAX_CONNECTIONS, "many"),
        ] {
            let map = [(key.to_string(), raw.to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            assert!(
                matches!(
                    ReadOnlyServerConfig::from_env_map(&map).unwrap_err(),
                    EnvParseError::NonNumeric { key: k, .. } if k == key
                ),
                "{key} with {raw:?}"
            );
        }

        for (key, raw) in [
            (ReadOnlyServerConfig::ENV_PORT, "0"),
            (ReadOnlyServerConfig::ENV_MAX_FRAME_BYTES, "0"),
            (ReadOnlyServerConfig::ENV_MAX_RESPONSE_BYTES, "0"),
            (ReadOnlyServerConfig::ENV_OPERATION_TIMEOUT_SECONDS, "0"),
            (ReadOnlyServerConfig::ENV_MAX_CONNECTIONS, "0"),
        ] {
            let map = [(key.to_string(), raw.to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                ReadOnlyServerConfig::from_env_map(&map).unwrap_err(),
                EnvParseError::Zero {
                    key,
                    value: "0".into()
                },
                "{key}"
            );
        }

        // u16 overflow for port; usize overflow for byte caps.
        let overflow_port = [(
            ReadOnlyServerConfig::ENV_PORT.to_string(),
            "65536".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert!(matches!(
            ReadOnlyServerConfig::from_env_map(&overflow_port).unwrap_err(),
            EnvParseError::Overflow {
                key: ReadOnlyServerConfig::ENV_PORT,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn compute_identity_from_exported_graph_bytes_is_deterministic() {
        let store = Arc::new(RecordingStore::new());
        store.inner.set_snapshot_data(canned_sgsnap());
        let server = ReadOnlyServer::new(test_config(), store).expect("config");
        let identity = server.compute_identity().await.expect("compute identity");
        assert_eq!(identity.tenant, "daily_archive");
        assert_eq!(identity.node_count, 2);
        assert_eq!(identity.edge_count, 1);
        assert_eq!(identity.snapshot_format_version, 2);
        assert_eq!(
            identity.semantic_algorithm,
            crate::snapshot_identity::SEMANTIC_SNAPSHOT_ALGORITHM
        );
        assert!(!identity.semantic_sha256.is_empty());
        assert!(!identity.physical_sha256.is_empty());
        // Deterministic: recomputation over identical exported bytes matches.
        let again = server.compute_identity().await.expect("recompute");
        assert_eq!(identity, again);
    }

    #[tokio::test]
    async fn identity_command_serves_identity_computed_from_store_bytes() {
        let store = Arc::new(RecordingStore::new());
        store.inner.set_snapshot_data(canned_sgsnap());
        let server = ReadOnlyServer::new(test_config(), store).expect("config");
        let server = server.with_identity_from_store().await.expect("compute");
        let listener = server.bind().await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move {
            let _ = server.serve_listener(listener).await;
        });
        let mut stream = connect(&endpoint).await;
        stream
            .write_all(&encode_command(&["DA.IDENTITY"]))
            .await
            .unwrap();
        let frame = read_one_frame(&mut stream).await;
        let (value, consumed) = parse_resp(&frame).expect("parse").expect("frame");
        assert_eq!(consumed, frame.len());
        let RespValue::Bulk(json) = value else {
            panic!("expected bulk identity, got {value:?}");
        };
        let identity: SnapshotIdentity = serde_json::from_str(&json).expect("identity JSON");
        assert_eq!(identity.node_count, 2);
        assert_eq!(identity.edge_count, 1);
        assert_eq!(identity.tenant, "daily_archive");
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn read_only_server_refuses_when_saturated() {
        let store = Arc::new(RecordingStore::new());
        let mut config = test_config();
        config.max_connections = 1;
        let (endpoint, task) = start(store.clone(), config).await;

        // First client holds the only permit.
        let mut first = connect(&endpoint).await;
        first.write_all(&encode_command(&["PING"])).await.unwrap();
        assert_eq!(frame_text(&read_one_frame(&mut first).await), "+PONG\r\n");

        // Second client is refused with a visible busy error and closed.
        let mut second = connect(&endpoint).await;
        second.write_all(&encode_command(&["PING"])).await.unwrap();
        let frame = frame_text(&read_one_frame(&mut second).await);
        assert!(frame.contains("max connections"), "{frame}");
        let mut chunk = [0_u8; 64];
        let read = timeout(Duration::from_secs(2), second.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read, 0);

        task.abort();
        let _ = task.await;
    }

    /// The shared codec contract: the real `RemoteGraphStore` client talks to
    /// the in-process `ReadOnlyServer` end to end (server encode + client
    /// decode symmetry). This is the in-crate seed of the T04 cross-process
    /// contract test.
    #[tokio::test]
    async fn remote_client_reads_through_read_only_server() {
        use crate::remote::{RemoteGraphConfig, RemoteGraphStore};

        let store = Arc::new(RecordingStore::new());
        let (endpoint, task) = start(store.clone(), test_config()).await;

        let client = RemoteGraphStore::new(RemoteGraphConfig::new(endpoint).unwrap()).unwrap();

        // PING health through the shared codec.
        assert!(client.health().await.unwrap());

        // GRAPH.RO_QUERY round-trips through the server.
        let result = client
            .query_readonly("g", "MATCH (n) RETURN n.vid")
            .await
            .unwrap();
        assert_eq!(result.columns, ["vid", "label"]);
        assert_eq!(result.records.len(), 1);
        assert_eq!(store.read_calls(), 1);

        // Client-side fail-closed validation still blocks mutable queries.
        let error = client
            .query("g", "CREATE (n)")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("CREATE"), "{error}");
        assert_eq!(store.read_calls(), 1, "server store must stay untouched");

        task.abort();
        let _ = task.await;
    }

    /// T02 read-plane identity contract end to end: the real remote client
    /// fetches DA.IDENTITY, verifies a matching expected identity, and gets a
    /// typed, visible mismatch for a wrong expected identity.
    #[tokio::test]
    async fn remote_identity_verification_is_typed_and_visible() {
        use crate::remote::{RemoteGraphConfig, RemoteGraphStore};

        let store = Arc::new(RecordingStore::new());
        store.inner.set_snapshot_data(canned_sgsnap());
        let server = ReadOnlyServer::new(test_config(), store).expect("config");
        let expected = server.compute_identity().await.expect("compute");
        let server = server.with_identity(expected.to_json().unwrap());
        let listener = server.bind().await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move {
            let _ = server.serve_listener(listener).await;
        });

        let client = RemoteGraphStore::new(RemoteGraphConfig::new(endpoint).unwrap()).unwrap();

        // identity() decodes the server's JSON into a typed SnapshotIdentity.
        let actual = client.identity().await.expect("identity");
        assert_eq!(actual, expected);

        // Matching expected identity verifies cleanly.
        client.verify_identity(&expected).await.expect("verify ok");

        // A wrong expected identity is a typed, visible mismatch — never an
        // empty result. The error names both semantic SHAs and counts.
        let wrong = SnapshotIdentity {
            semantic_sha256: "deadbeef".into(),
            ..expected.clone()
        };
        let error = client.verify_identity(&wrong).await.expect_err("mismatch");
        match &error {
            GraphStoreError::IdentityMismatch(detail) => {
                assert_eq!(detail.expected_semantic_sha256, "deadbeef");
                assert_eq!(detail.actual_semantic_sha256, expected.semantic_sha256);
                assert_eq!(detail.expected_nodes, 2);
                assert_eq!(detail.actual_nodes, 2);
            }
            other => panic!("expected IdentityMismatch, got {other:?}"),
        }
        let message = error.to_string();
        assert!(message.contains("deadbeef"), "{message}");
        assert!(message.contains(&expected.semantic_sha256), "{message}");

        task.abort();
        let _ = task.await;
    }
}
