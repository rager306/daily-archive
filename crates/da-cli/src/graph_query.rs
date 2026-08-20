//! Mode-aware scalar graph query composition.
//! Remote is an explicit read plane; embedded remains process-local snapshot mode.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use da_ports::GraphStore;
use kg_storage::{RemoteGraphConfig, RemoteGraphStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GraphQueryMode {
    Embedded,
    Remote,
}

#[derive(Debug, Clone, Args)]
pub struct GraphQueryArgs {
    /// Read mode. When omitted, resolves DA_GRAPH_MODE; default embedded.
    #[arg(long, value_enum)]
    pub mode: Option<GraphQueryMode>,
    /// Read-only Cypher. Mutations/admin commands are rejected in remote mode.
    #[arg(long)]
    pub cypher: String,
    /// Samyama graph name.
    #[arg(long, default_value = "daily_archive")]
    pub graph: String,
    /// Remote host:port. Falls back to DA_REMOTE_GRAPH_ENDPOINT (canonical)
    /// or alias DA_GRAPH_ENDPOINT. Daemon auto-start is forbidden.
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Embedded same-process snapshot preload; invalid in remote mode.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
    /// Emit QueryResult JSON only.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Debug)]
pub enum GraphQueryCliError {
    Configuration(String),
    Runtime(String),
}

impl std::fmt::Display for GraphQueryCliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) => {
                write!(formatter, "invalid graph-query configuration: {message}")
            }
            Self::Runtime(message) => write!(formatter, "graph-query failed: {message}"),
        }
    }
}

impl std::error::Error for GraphQueryCliError {}

impl GraphQueryCliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Configuration(_) => 2,
            Self::Runtime(_) => 3,
        }
    }
}

pub const REMOTE_NODE_COUNT_CYPHER: &str = "MATCH (n) RETURN count(n) AS nodes";
pub const REMOTE_EDGE_COUNT_CYPHER: &str = "MATCH ()-[r]->() RETURN count(r) AS edges";

/// `--snapshot` is embedded-only; remote reads the daemon's current graph.
pub fn reject_remote_snapshot(
    snapshot: Option<&std::path::Path>,
) -> Result<(), GraphQueryCliError> {
    if snapshot.is_some() {
        Err(GraphQueryCliError::Configuration(
            "--snapshot is embedded-only; preload the daemon with serve-graph --snapshot".into(),
        ))
    } else {
        Ok(())
    }
}

/// Open the RESP read plane from `--endpoint` or canonical/alias env keys.
pub fn open_remote_store(
    explicit_endpoint: Option<String>,
) -> Result<RemoteGraphStore, GraphQueryCliError> {
    let config = match explicit_endpoint.filter(|value| !value.trim().is_empty()) {
        Some(endpoint) => RemoteGraphConfig::new(endpoint)
            .map_err(|error| GraphQueryCliError::Configuration(error.to_string()))?,
        None => RemoteGraphConfig::from_env_require_endpoint()
            .map_err(|error| GraphQueryCliError::Configuration(error.to_string()))?,
    };
    RemoteGraphStore::new(config)
        .map_err(|error| GraphQueryCliError::Configuration(error.to_string()))
}

/// Open the RESP read plane from process env (canonical + alias).
pub fn open_remote_store_from_env() -> Result<RemoteGraphStore, GraphQueryCliError> {
    open_remote_store(None)
}

/// Parse `RETURN count(...) AS x` from a remote QueryResult. Fail closed on
/// empty/non-numeric payloads — never guess zero.
pub fn count_from_query_result(
    result: &da_ports::graph_store::QueryResult,
) -> Result<usize, GraphQueryCliError> {
    let value = result
        .records
        .first()
        .and_then(|row| row.first())
        .ok_or_else(|| GraphQueryCliError::Runtime("remote count query returned no rows".into()))?;
    match value {
        serde_json::Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_i64().and_then(|i| u64::try_from(i).ok()))
            .map(|n| n as usize)
            .ok_or_else(|| {
                GraphQueryCliError::Runtime(format!(
                    "remote count is not a non-negative integer: {n}"
                ))
            }),
        serde_json::Value::String(s) => s.parse::<usize>().map_err(|_| {
            GraphQueryCliError::Runtime(format!("remote count string is not an integer: {s:?}"))
        }),
        other => Err(GraphQueryCliError::Runtime(format!(
            "remote count expected a number, got {other}"
        ))),
    }
}

pub async fn run(args: GraphQueryArgs) -> Result<(), GraphQueryCliError> {
    let mode = resolve_mode(args.mode)?;
    let result = match mode {
        GraphQueryMode::Remote => {
            reject_remote_snapshot(args.snapshot.as_deref())?;
            let store = open_remote_store(args.endpoint)?;
            store
                .query_readonly(&args.graph, &args.cypher)
                .await
                .map_err(|error| GraphQueryCliError::Runtime(error.to_string()))?
        }
        GraphQueryMode::Embedded => {
            if args.endpoint.is_some() {
                return Err(GraphQueryCliError::Configuration(
                    "--endpoint is remote-only".into(),
                ));
            }
            let store = da_adapters::SamyamaGraphStore::new();
            if let Some(snapshot) = &args.snapshot {
                let data = std::fs::read(snapshot).map_err(|error| {
                    GraphQueryCliError::Runtime(format!(
                        "read embedded snapshot {}: {error}",
                        snapshot.display()
                    ))
                })?;
                store.import_snapshot(&data).await.map_err(|error| {
                    GraphQueryCliError::Runtime(format!(
                        "import embedded snapshot {}: {error}",
                        snapshot.display()
                    ))
                })?;
            }
            store
                .query_readonly(&args.graph, &args.cypher)
                .await
                .map_err(|error| GraphQueryCliError::Runtime(error.to_string()))?
        }
    };
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&result)
                .map_err(|error| GraphQueryCliError::Runtime(error.to_string()))?
        );
    } else {
        println!("columns: {}", result.columns.join(" | "));
        for record in result.records {
            println!(
                "{}",
                record
                    .iter()
                    .map(serde_json::Value::to_string)
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
    }
    Ok(())
}

fn resolve_mode(explicit: Option<GraphQueryMode>) -> Result<GraphQueryMode, GraphQueryCliError> {
    resolve_mode_value(explicit, std::env::var("DA_GRAPH_MODE"))
}

fn resolve_mode_value(
    explicit: Option<GraphQueryMode>,
    environment: Result<String, std::env::VarError>,
) -> Result<GraphQueryMode, GraphQueryCliError> {
    if let Some(mode) = explicit {
        return Ok(mode);
    }
    match environment {
        Err(_) => Ok(GraphQueryMode::Embedded),
        Ok(value) if value.trim().is_empty() || value.eq_ignore_ascii_case("embedded") => {
            Ok(GraphQueryMode::Embedded)
        }
        Ok(value) if value.eq_ignore_ascii_case("remote") => Ok(GraphQueryMode::Remote),
        Ok(value) => Err(GraphQueryCliError::Configuration(format!(
            "DA_GRAPH_MODE={value:?}; expected embedded or remote"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_mode_wins_and_invalid_environment_fails_closed() {
        assert_eq!(
            resolve_mode_value(Some(GraphQueryMode::Remote), Ok("bogus".into())).unwrap(),
            GraphQueryMode::Remote
        );
        assert!(resolve_mode_value(None, Ok("bogus".into())).is_err());
        assert_eq!(
            resolve_mode_value(None, Err(std::env::VarError::NotPresent)).unwrap(),
            GraphQueryMode::Embedded
        );
    }

    #[test]
    fn configuration_errors_have_stable_exit_code() {
        assert_eq!(GraphQueryCliError::Configuration("x".into()).exit_code(), 2);
        assert_eq!(GraphQueryCliError::Runtime("x".into()).exit_code(), 3);
    }

    #[test]
    fn snapshot_is_rejected_in_remote_mode() {
        let err = reject_remote_snapshot(Some(std::path::Path::new("x.sgsnap"))).unwrap_err();
        assert!(matches!(err, GraphQueryCliError::Configuration(_)));
        assert!(err.to_string().contains("embedded-only"));
        assert!(reject_remote_snapshot(None).is_ok());
    }

    #[test]
    fn open_remote_store_accepts_explicit_endpoint_without_connecting() {
        let store = open_remote_store(Some("127.0.0.1:16379".into())).expect("config");
        assert_eq!(store.config().endpoint, "127.0.0.1:16379");
    }

    #[test]
    fn count_from_query_result_reads_number_and_fails_closed() {
        use da_ports::graph_store::QueryResult;
        let ok = QueryResult {
            columns: vec!["nodes".into()],
            records: vec![vec![serde_json::json!(7)]],
        };
        assert_eq!(count_from_query_result(&ok).unwrap(), 7);
        let empty = QueryResult {
            columns: vec!["nodes".into()],
            records: vec![],
        };
        assert!(count_from_query_result(&empty).is_err());
    }
}
