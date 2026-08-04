//! Graph store open mode (Wave 3 entry — M305).
//!
//! Resolves `DA_GRAPH_MODE` into a typed mode. Default is **embedded**
//! (process-local Samyama). **remote** is declared but not configured —
//! opening remote returns [`GraphModeError::RemoteNotConfigured`] with no
//! fake success and no server auto-start.
//!
//! Env: `DA_GRAPH_MODE=embedded|remote` (case-insensitive). Unset → embedded.

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
}

/// Errors from mode resolution or mode-aware open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphModeError {
    /// `DA_GRAPH_MODE` set to an unsupported value.
    InvalidMode { value: String },
    /// Mode is remote but no remote client/URL is configured yet.
    RemoteNotConfigured,
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
        }
    }
}

impl std::error::Error for GraphModeError {}

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
}
