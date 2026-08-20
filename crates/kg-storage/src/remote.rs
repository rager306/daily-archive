//! Bounded Samyama RESP read-plane adapter.
//! All mutation, index, vector and snapshot capabilities fail closed.

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::error::{EnvParseError, env_optional_string, env_u64, env_usize};
use crate::resp::{RespValue, encode_command, parse_resp};
use crate::snapshot_identity::SnapshotIdentity;
use crate::{
    GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric, VectorSearchResult,
    validate_readonly_cypher,
};

pub const DEFAULT_REMOTE_ENDPOINT: &str = "127.0.0.1:16379";
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteGraphConfig {
    pub endpoint: String,
    pub timeout: Duration,
    pub max_response_bytes: usize,
}

impl RemoteGraphConfig {
    /// Canonical env key: remote endpoint `host:port` (default [`DEFAULT_REMOTE_ENDPOINT`]).
    pub const ENV_ENDPOINT: &'static str = "DA_REMOTE_GRAPH_ENDPOINT";
    /// Explicit alias of [`Self::ENV_ENDPOINT`]. Used by `da health` / `da graph-query`.
    /// If both keys are set to different values, resolution fails closed.
    pub const ENV_ENDPOINT_ALIAS: &'static str = "DA_GRAPH_ENDPOINT";
    /// Env key: per-operation timeout in whole seconds (default [`DEFAULT_TIMEOUT`]; zero rejected).
    pub const ENV_TIMEOUT_SECONDS: &'static str = "DA_REMOTE_GRAPH_TIMEOUT_SECONDS";
    /// Env key: max response bytes (default [`DEFAULT_MAX_RESPONSE_BYTES`]; zero rejected).
    pub const ENV_MAX_RESPONSE_BYTES: &'static str = "DA_REMOTE_GRAPH_MAX_RESPONSE_BYTES";

    /// Read the documented `DA_REMOTE_GRAPH_*` keys from the process
    /// environment. Unset keys fall back to defaults; present values are
    /// parsed strictly — blank, non-numeric, zero and overflow are rejected
    /// with a typed [`EnvParseError`] instead of a silent default.
    pub fn from_env() -> Result<Self, EnvParseError> {
        Self::from_env_map(&std::env::vars().collect::<BTreeMap<_, _>>())
    }

    /// Like [`Self::from_env`], but an unset endpoint is a typed error instead
    /// of the loopback default. CLI remote reads use this so a missing key
    /// cannot silently talk to `127.0.0.1:16379`.
    pub fn from_env_require_endpoint() -> Result<Self, EnvParseError> {
        Self::from_env_map_require_endpoint(&std::env::vars().collect::<BTreeMap<_, _>>())
    }

    /// Testable map-backed variant of [`Self::from_env_require_endpoint`].
    pub fn from_env_map_require_endpoint(
        map: &BTreeMap<String, String>,
    ) -> Result<Self, EnvParseError> {
        match Self::resolve_endpoint_from_map(map)? {
            None => Err(EnvParseError::Invalid {
                key: Self::ENV_ENDPOINT,
                value: String::new(),
                reason: format!(
                    "unset; set {} (canonical) or alias {}",
                    Self::ENV_ENDPOINT,
                    Self::ENV_ENDPOINT_ALIAS
                ),
            }),
            Some(_) => Self::from_env_map(map),
        }
    }

    /// Resolve the remote endpoint from a map: canonical key first, then alias.
    /// Both unset → `Ok(None)` (caller may default). Both set and unequal →
    /// typed [`EnvParseError::Invalid`] (no silent pick).
    pub fn resolve_endpoint_from_map(
        map: &BTreeMap<String, String>,
    ) -> Result<Option<String>, EnvParseError> {
        let canonical = env_optional_string(map, Self::ENV_ENDPOINT)?;
        let alias = env_optional_string(map, Self::ENV_ENDPOINT_ALIAS)?;
        match (canonical, alias) {
            (Some(canonical), Some(alias)) if canonical != alias => Err(EnvParseError::Invalid {
                key: Self::ENV_ENDPOINT,
                value: canonical,
                reason: format!(
                    "conflicts with {}={alias:?}; set only {} (canonical) or the alias {}",
                    Self::ENV_ENDPOINT_ALIAS,
                    Self::ENV_ENDPOINT,
                    Self::ENV_ENDPOINT_ALIAS
                ),
            }),
            (Some(endpoint), Some(_)) | (Some(endpoint), None) | (None, Some(endpoint)) => {
                Ok(Some(endpoint))
            }
            (None, None) => Ok(None),
        }
    }

    /// Testable map-backed variant of [`Self::from_env`].
    pub fn from_env_map(map: &BTreeMap<String, String>) -> Result<Self, EnvParseError> {
        let endpoint = Self::resolve_endpoint_from_map(map)?
            .unwrap_or_else(|| DEFAULT_REMOTE_ENDPOINT.to_string());
        let timeout_secs =
            env_u64(map, Self::ENV_TIMEOUT_SECONDS)?.unwrap_or(DEFAULT_TIMEOUT.as_secs());
        let max_response_bytes =
            env_usize(map, Self::ENV_MAX_RESPONSE_BYTES)?.unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
        if timeout_secs == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_TIMEOUT_SECONDS,
                value: "0".into(),
            });
        }
        if max_response_bytes == 0 {
            return Err(EnvParseError::Zero {
                key: Self::ENV_MAX_RESPONSE_BYTES,
                value: "0".into(),
            });
        }
        let config = Self {
            endpoint,
            timeout: Duration::from_secs(timeout_secs),
            max_response_bytes,
        };
        config.validate().map_err(|error| EnvParseError::Invalid {
            key: Self::ENV_ENDPOINT,
            value: config.endpoint.clone(),
            reason: error.to_string(),
        })?;
        Ok(config)
    }

    pub fn new(endpoint: impl Into<String>) -> Result<Self, GraphStoreError> {
        let endpoint = endpoint.into();
        let endpoint = endpoint.trim();
        if endpoint.is_empty() || !endpoint.contains(':') {
            return Err(GraphStoreError::Storage(format!(
                "invalid remote graph endpoint {endpoint:?}; expected host:port"
            )));
        }
        Ok(Self {
            endpoint: endpoint.to_string(),
            timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        })
    }

    pub fn validate(&self) -> GraphResult<()> {
        if self.timeout.is_zero() {
            return Err(GraphStoreError::Storage(
                "remote graph timeout must be greater than zero".into(),
            ));
        }
        if self.max_response_bytes == 0 {
            return Err(GraphStoreError::Storage(
                "remote graph max_response_bytes must be greater than zero".into(),
            ));
        }
        if self.endpoint.trim().is_empty() || !self.endpoint.contains(':') {
            return Err(GraphStoreError::Storage(format!(
                "invalid remote graph endpoint {:?}; expected host:port",
                self.endpoint
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RemoteGraphStore {
    config: RemoteGraphConfig,
}

impl RemoteGraphStore {
    pub fn new(config: RemoteGraphConfig) -> GraphResult<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &RemoteGraphConfig {
        &self.config
    }

    async fn command(&self, args: &[&str]) -> GraphResult<RespValue> {
        let request = encode_command(args);
        let operation = async {
            let mut stream = TcpStream::connect(&self.config.endpoint)
                .await
                .map_err(|error| remote_io_error(&self.config.endpoint, "connect", error))?;
            stream
                .write_all(&request)
                .await
                .map_err(|error| remote_io_error(&self.config.endpoint, "write", error))?;
            stream
                .flush()
                .await
                .map_err(|error| remote_io_error(&self.config.endpoint, "flush", error))?;
            let mut response = Vec::new();
            let mut chunk = [0_u8; 8192];
            loop {
                match parse_resp(&response) {
                    Ok(Some((value, consumed))) => {
                        if consumed != response.len() {
                            return Err(GraphStoreError::Storage(format!(
                                "remote graph protocol returned {} trailing bytes",
                                response.len() - consumed
                            )));
                        }
                        return Ok(value);
                    }
                    Ok(None) => {}
                    Err(error) => return Err(GraphStoreError::Storage(error)),
                }
                let read = stream
                    .read(&mut chunk)
                    .await
                    .map_err(|error| remote_io_error(&self.config.endpoint, "read", error))?;
                if read == 0 {
                    return Err(GraphStoreError::Storage(
                        "remote graph protocol closed before a complete RESP frame".into(),
                    ));
                }
                if response.len().saturating_add(read) > self.config.max_response_bytes {
                    return Err(GraphStoreError::Storage(format!(
                        "remote graph response exceeds {} byte limit",
                        self.config.max_response_bytes
                    )));
                }
                response.extend_from_slice(&chunk[..read]);
            }
        };
        timeout(self.config.timeout, operation).await.map_err(|_| {
            GraphStoreError::Storage(format!(
                "remote graph operation timed out after {} ms at {}",
                self.config.timeout.as_millis(),
                self.config.endpoint
            ))
        })?
    }

    async fn query_inner(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        validate_readonly_cypher(cypher)?;
        let response = self.command(&["GRAPH.RO_QUERY", graph, cypher]).await?;
        query_result_from_resp(response)
    }

    /// Fetch the server's `DA.IDENTITY` as a typed [`SnapshotIdentity`] (T02).
    pub async fn identity(&self) -> GraphResult<SnapshotIdentity> {
        let response = self.command(&["DA.IDENTITY"]).await?;
        match response {
            RespValue::Bulk(json) | RespValue::Simple(json) => SnapshotIdentity::from_json(&json)
                .map_err(|error| {
                    GraphStoreError::Storage(format!(
                        "remote graph DA.IDENTITY returned invalid SnapshotIdentity JSON: {error}"
                    ))
                }),
            RespValue::Error(message) => Err(GraphStoreError::Storage(message)),
            other => Err(GraphStoreError::Storage(format!(
                "remote graph DA.IDENTITY expected a bulk JSON string, got {other:?}"
            ))),
        }
    }

    /// Verify the remote graph matches an expected snapshot identity (T02).
    /// A mismatch is a typed [`GraphStoreError::IdentityMismatch`] carrying
    /// both sides (semantic SHA, tenant, node/edge counts) so the caller sees
    /// the actual divergence — never an empty result.
    pub async fn verify_identity(&self, expected: &SnapshotIdentity) -> GraphResult<()> {
        let actual = self.identity().await?;
        if &actual == expected {
            Ok(())
        } else {
            Err(GraphStoreError::identity_mismatch(expected, &actual))
        }
    }

    fn unsupported(operation: &str) -> GraphStoreError {
        GraphStoreError::NotImplemented(format!(
            "RemoteGraphStore read plane does not support {operation}"
        ))
    }
}

#[async_trait]
impl GraphStore for RemoteGraphStore {
    async fn query(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.query_inner(graph, cypher).await
    }

    async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.query_inner(graph, cypher).await
    }

    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Err(Self::unsupported("create_vector_index"))
    }

    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
        Err(Self::unsupported("create_property_index"))
    }

    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Err(Self::unsupported("vector_search"))
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Err(Self::unsupported("export_snapshot"))
    }

    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        Err(Self::unsupported("import_snapshot"))
    }

    async fn health(&self) -> GraphResult<bool> {
        match self.command(&["PING"]).await? {
            RespValue::Simple(value) | RespValue::Bulk(value) if value == "PONG" => Ok(true),
            other => Err(GraphStoreError::Storage(format!(
                "remote graph PING returned unexpected response {other:?}"
            ))),
        }
    }
}

fn remote_io_error(endpoint: &str, phase: &str, error: std::io::Error) -> GraphStoreError {
    GraphStoreError::Storage(format!(
        "remote graph {phase} failed at {endpoint}: {error}"
    ))
}

fn query_result_from_resp(response: RespValue) -> GraphResult<QueryResult> {
    let RespValue::Array(mut rows) = response else {
        return match response {
            RespValue::Error(message) => Err(GraphStoreError::Query(message)),
            other => Err(GraphStoreError::Storage(format!(
                "remote graph query expected RESP array, got {other:?}"
            ))),
        };
    };
    if rows.is_empty() {
        return Err(GraphStoreError::Storage(
            "remote graph query response has no header row".into(),
        ));
    }
    let header = rows.remove(0);
    let RespValue::Array(header) = header else {
        return Err(GraphStoreError::Storage(
            "remote graph query header is not an array".into(),
        ));
    };
    let columns = header
        .into_iter()
        .map(resp_header_string)
        .collect::<GraphResult<Vec<_>>>()?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let RespValue::Array(row) = row else {
            return Err(GraphStoreError::Storage(
                "remote graph query data row is not an array".into(),
            ));
        };
        if row.len() != columns.len() {
            return Err(GraphStoreError::Storage(format!(
                "remote graph query row width {} does not match header width {}",
                row.len(),
                columns.len()
            )));
        }
        records.push(
            row.into_iter()
                .map(resp_scalar_json)
                .collect::<GraphResult<_>>()?,
        );
    }
    Ok(QueryResult { columns, records })
}

fn resp_header_string(value: RespValue) -> GraphResult<String> {
    match value {
        RespValue::Bulk(value) | RespValue::Simple(value) => Ok(value),
        other => Err(GraphStoreError::Storage(format!(
            "remote graph query header value is not a string: {other:?}"
        ))),
    }
}

fn resp_scalar_json(value: RespValue) -> GraphResult<serde_json::Value> {
    match value {
        RespValue::Bulk(value) | RespValue::Simple(value) => Ok(value.into()),
        RespValue::Integer(value) => Ok(value.into()),
        RespValue::Null => Ok(serde_json::Value::Null),
        RespValue::Error(message) => Err(GraphStoreError::Query(message)),
        RespValue::Array(_) => Err(GraphStoreError::Storage(
            "remote graph query nested scalar array is unsupported".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resp_parser_is_incremental_and_maps_wire_types_without_guessing() {
        let frame = b"*3\r\n*3\r\n$1\r\ns\r\n$1\r\ni\r\n$1\r\nn\r\n*3\r\n$4\r\ntrue\r\n:7\r\n_\r\n*3\r\n$3\r\n1.5\r\n:-2\r\n$-1\r\n";
        assert!(parse_resp(&frame[..10]).unwrap().is_none());
        let (value, consumed) = parse_resp(frame).unwrap().unwrap();
        assert_eq!(consumed, frame.len());
        let result = query_result_from_resp(value).unwrap();
        assert_eq!(result.columns, ["s", "i", "n"]);
        assert_eq!(
            result.records[0],
            [
                serde_json::json!("true"),
                serde_json::json!(7),
                serde_json::Value::Null
            ]
        );
        assert_eq!(
            result.records[1],
            [
                serde_json::json!("1.5"),
                serde_json::json!(-2),
                serde_json::Value::Null
            ]
        );
    }

    #[test]
    fn malformed_and_wrong_width_responses_fail_closed() {
        assert!(parse_resp(b"$3\r\nabcXX").is_err());
        let value = parse_resp(b"*2\r\n*2\r\n$1\r\na\r\n$1\r\nb\r\n*1\r\n:1\r\n")
            .unwrap()
            .unwrap()
            .0;
        assert!(query_result_from_resp(value).is_err());
    }

    async fn one_shot_server(response: Vec<u8>, hold_open: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            if !response.is_empty() {
                socket.write_all(&response).await.unwrap();
            }
            tokio::time::sleep(hold_open).await;
        });
        endpoint
    }

    #[tokio::test]
    async fn server_error_response_is_typed_query_failure() {
        let endpoint = one_shot_server(b"-ERR query failed\r\n".to_vec(), Duration::ZERO).await;
        let store = RemoteGraphStore::new(RemoteGraphConfig::new(endpoint).unwrap()).unwrap();
        assert!(matches!(
            store.query_readonly("g", "RETURN 1").await,
            Err(GraphStoreError::Query(message)) if message == "ERR query failed"
        ));
    }

    #[tokio::test]
    async fn response_limit_and_timeout_are_bounded() {
        let endpoint =
            one_shot_server(b"$100\r\n0123456789".to_vec(), Duration::from_secs(1)).await;
        let mut config = RemoteGraphConfig::new(endpoint).unwrap();
        config.max_response_bytes = 8;
        let error = RemoteGraphStore::new(config)
            .unwrap()
            .health()
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds 8 byte limit"), "{error}");

        let endpoint = one_shot_server(Vec::new(), Duration::from_secs(1)).await;
        let mut config = RemoteGraphConfig::new(endpoint).unwrap();
        config.timeout = Duration::from_millis(20);
        let error = RemoteGraphStore::new(config)
            .unwrap()
            .health()
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("timed out after 20 ms"), "{error}");
    }

    #[tokio::test]
    async fn write_is_rejected_before_any_connection_attempt() {
        let store = RemoteGraphStore::new(RemoteGraphConfig::new("127.0.0.1:9").unwrap()).unwrap();
        let error = store
            .query("g", "MATCH (n) DELETE n")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("DELETE"), "{error}");
        assert!(!error.contains("connect"), "{error}");
    }

    #[tokio::test]
    async fn unsupported_capabilities_are_explicit() {
        let store = RemoteGraphStore::new(RemoteGraphConfig::new("127.0.0.1:9").unwrap()).unwrap();
        assert!(matches!(
            store.export_snapshot().await,
            Err(GraphStoreError::NotImplemented(_))
        ));
        assert!(matches!(
            store.import_snapshot(&[]).await,
            Err(GraphStoreError::NotImplemented(_))
        ));
        assert!(matches!(
            store.create_property_index("Paper", "vid").await,
            Err(GraphStoreError::NotImplemented(_))
        ));
    }

    // ---- T02: RemoteGraphConfig::from_env typed environment contract ----

    #[test]
    fn remote_config_from_env_parses_all_keys_and_defaults() {
        let mut map = BTreeMap::new();
        map.insert(
            RemoteGraphConfig::ENV_ENDPOINT.to_string(),
            "10.0.0.5:19999".to_string(),
        );
        map.insert(
            RemoteGraphConfig::ENV_TIMEOUT_SECONDS.to_string(),
            "42".to_string(),
        );
        map.insert(
            RemoteGraphConfig::ENV_MAX_RESPONSE_BYTES.to_string(),
            "1048576".to_string(),
        );
        let config = RemoteGraphConfig::from_env_map(&map).expect("parse");
        assert_eq!(config.endpoint, "10.0.0.5:19999");
        assert_eq!(config.timeout, Duration::from_secs(42));
        assert_eq!(config.max_response_bytes, 1_048_576);

        // Unset keys fall back to documented defaults.
        let defaults = RemoteGraphConfig::from_env_map(&BTreeMap::new()).expect("defaults");
        assert_eq!(defaults.endpoint, DEFAULT_REMOTE_ENDPOINT);
        assert_eq!(defaults.timeout, DEFAULT_TIMEOUT);
        assert_eq!(defaults.max_response_bytes, DEFAULT_MAX_RESPONSE_BYTES);
    }

    #[test]
    fn remote_config_from_env_rejects_blank_non_numeric_zero_overflow_and_invalid_endpoint() {
        // Blank (whitespace-only) endpoint.
        let blank = [(
            RemoteGraphConfig::ENV_ENDPOINT.to_string(),
            "   ".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert_eq!(
            RemoteGraphConfig::from_env_map(&blank).unwrap_err(),
            EnvParseError::Blank {
                key: RemoteGraphConfig::ENV_ENDPOINT
            }
        );

        // Non-numeric, zero, overflow on every numeric key.
        for (key, raw) in [
            (RemoteGraphConfig::ENV_TIMEOUT_SECONDS, "1.5"),
            (RemoteGraphConfig::ENV_MAX_RESPONSE_BYTES, "-8"),
        ] {
            let map = [(key.to_string(), raw.to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                RemoteGraphConfig::from_env_map(&map).unwrap_err(),
                EnvParseError::NonNumeric {
                    key,
                    value: raw.to_string()
                },
                "{key}"
            );
        }

        for (key, raw) in [
            (RemoteGraphConfig::ENV_TIMEOUT_SECONDS, "0"),
            (RemoteGraphConfig::ENV_MAX_RESPONSE_BYTES, "0"),
        ] {
            let map = [(key.to_string(), raw.to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                RemoteGraphConfig::from_env_map(&map).unwrap_err(),
                EnvParseError::Zero {
                    key,
                    value: raw.to_string()
                },
                "{key}"
            );
        }

        // Overflow: digits that do not fit the target type.
        let overflow = [(
            RemoteGraphConfig::ENV_MAX_RESPONSE_BYTES.to_string(),
            "99999999999999999999999999".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert!(matches!(
            RemoteGraphConfig::from_env_map(&overflow).unwrap_err(),
            EnvParseError::Overflow {
                key: RemoteGraphConfig::ENV_MAX_RESPONSE_BYTES,
                ..
            }
        ));

        // Invalid endpoint (no port) surfaces as a typed Invalid error
        // naming the endpoint key — never a silent default.
        let invalid = [(
            RemoteGraphConfig::ENV_ENDPOINT.to_string(),
            "noport".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert!(matches!(
            RemoteGraphConfig::from_env_map(&invalid).unwrap_err(),
            EnvParseError::Invalid {
                key: RemoteGraphConfig::ENV_ENDPOINT,
                value,
                ..
            } if value == "noport"
        ));
    }

    #[test]
    fn remote_endpoint_alias_is_explicit_and_conflicts_fail_closed() {
        let alias_only = [(
            RemoteGraphConfig::ENV_ENDPOINT_ALIAS.to_string(),
            "10.1.2.3:16379".to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let config = RemoteGraphConfig::from_env_map(&alias_only).expect("alias");
        assert_eq!(config.endpoint, "10.1.2.3:16379");

        let equal = [
            (
                RemoteGraphConfig::ENV_ENDPOINT.to_string(),
                "10.1.2.3:16379".to_string(),
            ),
            (
                RemoteGraphConfig::ENV_ENDPOINT_ALIAS.to_string(),
                "10.1.2.3:16379".to_string(),
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let config = RemoteGraphConfig::from_env_map(&equal).expect("equal keys");
        assert_eq!(config.endpoint, "10.1.2.3:16379");

        let conflict = [
            (
                RemoteGraphConfig::ENV_ENDPOINT.to_string(),
                "10.1.2.3:16379".to_string(),
            ),
            (
                RemoteGraphConfig::ENV_ENDPOINT_ALIAS.to_string(),
                "10.9.9.9:1".to_string(),
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let err = RemoteGraphConfig::from_env_map(&conflict).unwrap_err();
        match err {
            EnvParseError::Invalid { key, reason, .. } => {
                assert_eq!(key, RemoteGraphConfig::ENV_ENDPOINT);
                assert!(
                    reason.contains(RemoteGraphConfig::ENV_ENDPOINT_ALIAS),
                    "{reason}"
                );
                assert!(reason.contains("conflicts"), "{reason}");
            }
            other => panic!("expected Invalid conflict, got {other:?}"),
        }

        assert_eq!(
            RemoteGraphConfig::resolve_endpoint_from_map(&BTreeMap::new()).unwrap(),
            None
        );

        let missing =
            RemoteGraphConfig::from_env_map_require_endpoint(&BTreeMap::new()).unwrap_err();
        match missing {
            EnvParseError::Invalid { key, reason, .. } => {
                assert_eq!(key, RemoteGraphConfig::ENV_ENDPOINT);
                assert!(
                    reason.contains(RemoteGraphConfig::ENV_ENDPOINT_ALIAS),
                    "{reason}"
                );
            }
            other => panic!("expected Invalid missing endpoint, got {other:?}"),
        }
    }
}
