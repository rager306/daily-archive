//! Graph store open mode (Wave 3 entry — M305 / M332).
//!
//! Resolves `DA_GRAPH_MODE` into a typed mode. Default is **embedded**
//! (process-local Samyama). **remote** is declared but not configured —
//! opening remote returns [`GraphModeError::RemoteNotConfigured`] with no
//! fake success and no server auto-start.
//!
//! Env: `DA_GRAPH_MODE=embedded|remote` (case-insensitive). Unset → embedded.
//!
//! Persistence (M332): process-local store is always in-memory. If
//! `DA_GRAPH_DATA_PATH` is set to a non-empty path, open fails with
//! [`GraphModeError::PersistenceNotConfigured`] — RocksDB is not wired
//! for daily-archive CLI (rocksdb rebuild trap; D127). Unset the var.

use std::fmt;

/// How the CLI / composition root should open a graph store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    /// Process-local in-memory Samyama (today's default).
    Embedded,
    /// Future remote Samyama daemon client. Not configured yet.
    Remote,
}

impl GraphMode {
    /// Parse mode from an optional env value (None = embedded).
    pub fn parse(raw: Option<&str>) -> Result<Self, GraphModeError> {
        match raw.map(str::trim).filter(|s| !s.is_empty()) {
            None => Ok(Self::Embedded),
            Some(s) if s.eq_ignore_ascii_case("embedded") => Ok(Self::Embedded),
            Some(s) if s.eq_ignore_ascii_case("remote") => Ok(Self::Remote),
            Some(other) => Err(GraphModeError::InvalidMode {
                value: other.to_string(),
            }),
        }
    }

    /// Read `DA_GRAPH_MODE` from the process environment.
    pub fn from_env() -> Result<Self, GraphModeError> {
        Self::parse(std::env::var("DA_GRAPH_MODE").ok().as_deref())
    }

    pub const ENV_KEY: &'static str = "DA_GRAPH_MODE";

    /// Env key for a RocksDB / PersistenceManager data directory.
    /// **Not supported** in daily-archive CLI open path — non-empty value is refused.
    pub const ENV_DATA_PATH: &'static str = "DA_GRAPH_DATA_PATH";
}

/// Errors from mode resolution or mode-aware open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphModeError {
    /// `DA_GRAPH_MODE` set to an unsupported value.
    InvalidMode { value: String },
    /// Mode is remote but no remote client/URL is configured yet.
    RemoteNotConfigured,
    /// `DA_GRAPH_DATA_PATH` set but RocksDB persistence is not wired for CLI open.
    PersistenceNotConfigured { path: String },
}

impl fmt::Display for GraphModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMode { value } => write!(
                f,
                "invalid {}: {value:?} (expected embedded or remote)",
                GraphMode::ENV_KEY
            ),
            Self::RemoteNotConfigured => write!(
                f,
                "{}=remote is not configured yet (Wave 3 remote client not shipped). \
                 Use embedded (default) or unset {}. \
                 Do not expect cross-process graph state until a Samyama daemon is wired.",
                GraphMode::ENV_KEY,
                GraphMode::ENV_KEY
            ),
            Self::PersistenceNotConfigured { path } => write!(
                f,
                "{}={path:?} is set but RocksDB/PersistenceManager is not wired for \
                 daily-archive CLI open (process-local store is in-memory only). \
                 Unset {} or use `da batch-ingest --output file.sgsnap` for durability. \
                 Enabling disk persistence is Phase 3+ (server mode).",
                GraphMode::ENV_DATA_PATH,
                GraphMode::ENV_DATA_PATH
            ),
        }
    }
}

impl std::error::Error for GraphModeError {}

/// Pure check: empty/unset path → Ok(()); non-empty → PersistenceNotConfigured.
pub fn refuse_persistence_path(raw: Option<&str>) -> Result<(), GraphModeError> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(()),
        Some(path) => Err(GraphModeError::PersistenceNotConfigured {
            path: path.to_string(),
        }),
    }
}

/// Read `DA_GRAPH_DATA_PATH` from process env and refuse if set (M332).
pub fn refuse_persistence_path_from_env() -> Result<(), GraphModeError> {
    refuse_persistence_path(std::env::var(GraphMode::ENV_DATA_PATH).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_none_is_embedded() {
        assert_eq!(GraphMode::parse(None).unwrap(), GraphMode::Embedded);
    }

    #[test]
    fn empty_string_is_embedded() {
        assert_eq!(GraphMode::parse(Some("")).unwrap(), GraphMode::Embedded);
        assert_eq!(GraphMode::parse(Some("  ")).unwrap(), GraphMode::Embedded);
    }

    #[test]
    fn parses_embedded_case_insensitive() {
        assert_eq!(
            GraphMode::parse(Some("embedded")).unwrap(),
            GraphMode::Embedded
        );
        assert_eq!(
            GraphMode::parse(Some("EMBEDDED")).unwrap(),
            GraphMode::Embedded
        );
    }

    #[test]
    fn parses_remote() {
        assert_eq!(GraphMode::parse(Some("remote")).unwrap(), GraphMode::Remote);
        assert_eq!(GraphMode::parse(Some("Remote")).unwrap(), GraphMode::Remote);
    }

    #[test]
    fn invalid_mode_errors() {
        let err = GraphMode::parse(Some("server")).unwrap_err();
        assert_eq!(
            err,
            GraphModeError::InvalidMode {
                value: "server".into()
            }
        );
        let msg = err.to_string();
        assert!(msg.contains("DA_GRAPH_MODE"), "{msg}");
        assert!(msg.contains("embedded"), "{msg}");
    }

    #[test]
    fn remote_not_configured_message_is_actionable() {
        let msg = GraphModeError::RemoteNotConfigured.to_string();
        assert!(msg.contains("remote"), "{msg}");
        assert!(msg.contains("not configured"), "{msg}");
        assert!(msg.contains("embedded"), "{msg}");
    }

    #[test]
    fn persistence_not_configured_message_is_actionable() {
        let msg = GraphModeError::PersistenceNotConfigured {
            path: "/tmp/samyama_data".into(),
        }
        .to_string();
        assert!(msg.contains("DA_GRAPH_DATA_PATH"), "{msg}");
        assert!(
            msg.contains("RocksDB") || msg.contains("in-memory"),
            "{msg}"
        );
        assert!(
            msg.contains("unset") || msg.contains("Unset") || msg.contains("batch-ingest"),
            "{msg}"
        );
    }

    #[test]
    fn refuse_persistence_path_empty_ok() {
        assert!(refuse_persistence_path(None).is_ok());
        assert!(refuse_persistence_path(Some("")).is_ok());
        assert!(refuse_persistence_path(Some("   ")).is_ok());
    }

    #[test]
    fn refuse_persistence_path_nonempty_errors() {
        let err = refuse_persistence_path(Some("/tmp/samyama_data")).unwrap_err();
        match err {
            GraphModeError::PersistenceNotConfigured { path } => {
                assert_eq!(path, "/tmp/samyama_data");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
