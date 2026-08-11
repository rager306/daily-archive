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
    /// Remote host:port. Falls back to DA_GRAPH_ENDPOINT only in remote mode.
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

pub async fn run(args: GraphQueryArgs) -> Result<(), GraphQueryCliError> {
    let mode = resolve_mode(args.mode)?;
    let result = match mode {
        GraphQueryMode::Remote => {
            if args.snapshot.is_some() {
                return Err(GraphQueryCliError::Configuration(
                    "--snapshot is embedded-only; preload the daemon with serve-graph --snapshot"
                        .into(),
                ));
            }
            let endpoint = args
                .endpoint
                .or_else(|| std::env::var("DA_GRAPH_ENDPOINT").ok())
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    GraphQueryCliError::Configuration(
                        "remote mode requires --endpoint or DA_GRAPH_ENDPOINT; daemon auto-start is forbidden"
                            .into(),
                    )
                })?;
            let config = RemoteGraphConfig::new(endpoint)
                .map_err(|error| GraphQueryCliError::Configuration(error.to_string()))?;
            let store = RemoteGraphStore::new(config)
                .map_err(|error| GraphQueryCliError::Configuration(error.to_string()))?;
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
}
