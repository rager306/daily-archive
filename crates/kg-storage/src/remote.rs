//! Bounded Samyama RESP read-plane adapter.
//! All mutation, index, vector and snapshot capabilities fail closed.

use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

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

fn encode_command(args: &[&str]) -> Vec<u8> {
    let mut output = format!("*{}\r\n", args.len()).into_bytes();
    for argument in args {
        output.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        output.extend_from_slice(argument.as_bytes());
        output.extend_from_slice(b"\r\n");
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RespValue {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(String),
    Null,
    Array(Vec<RespValue>),
}

fn parse_resp(input: &[u8]) -> Result<Option<(RespValue, usize)>, String> {
    parse_at(input, 0)
}

fn parse_at(input: &[u8], position: usize) -> Result<Option<(RespValue, usize)>, String> {
    let Some(prefix) = input.get(position).copied() else {
        return Ok(None);
    };
    match prefix {
        b'+' | b'-' | b':' => {
            let Some((line, next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let text = std::str::from_utf8(line)
                .map_err(|error| format!("remote graph RESP is not UTF-8: {error}"))?;
            let value = match prefix {
                b'+' => RespValue::Simple(text.into()),
                b'-' => RespValue::Error(text.into()),
                b':' => RespValue::Integer(text.parse().map_err(|error| {
                    format!("remote graph RESP invalid integer {text:?}: {error}")
                })?),
                _ => unreachable!(),
            };
            Ok(Some((value, next)))
        }
        b'$' => {
            let Some((line, mut next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let length: i64 = std::str::from_utf8(line)
                .map_err(|error| format!("remote graph RESP bulk length UTF-8: {error}"))?
                .parse()
                .map_err(|error| format!("remote graph RESP invalid bulk length: {error}"))?;
            if length == -1 {
                return Ok(Some((RespValue::Null, next)));
            }
            if length < -1 {
                return Err(format!("remote graph RESP invalid bulk length {length}"));
            }
            let length = length as usize;
            let Some(end) = next.checked_add(length) else {
                return Err("remote graph RESP bulk length overflow".into());
            };
            if input.len() < end + 2 {
                return Ok(None);
            }
            if &input[end..end + 2] != b"\r\n" {
                return Err("remote graph RESP bulk missing CRLF".into());
            }
            let value = std::str::from_utf8(&input[next..end])
                .map_err(|error| format!("remote graph RESP bulk is not UTF-8: {error}"))?;
            next = end + 2;
            Ok(Some((RespValue::Bulk(value.into()), next)))
        }
        b'*' => {
            let Some((line, mut next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let length: usize = std::str::from_utf8(line)
                .map_err(|error| format!("remote graph RESP array length UTF-8: {error}"))?
                .parse()
                .map_err(|error| format!("remote graph RESP invalid array length: {error}"))?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                let Some((value, after)) = parse_at(input, next)? else {
                    return Ok(None);
                };
                values.push(value);
                next = after;
            }
            Ok(Some((RespValue::Array(values), next)))
        }
        b'_' => {
            if input.get(position..position + 3) == Some(b"_\r\n") {
                Ok(Some((RespValue::Null, position + 3)))
            } else if input.len() < position + 3 {
                Ok(None)
            } else {
                Err("remote graph RESP invalid null frame".into())
            }
        }
        other => Err(format!(
            "remote graph RESP unsupported prefix 0x{other:02x}"
        )),
    }
}

fn read_line(input: &[u8], start: usize) -> Option<(&[u8], usize)> {
    input
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| (&input[start..start + offset], start + offset + 2))
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
}
