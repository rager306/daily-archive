//! Pure report lines for `da health` graph section (M312) + embedder env (M321).
//!
//! Honesty goals:
//! - Always print `DA_GRAPH_MODE` (resolved or invalid)
//! - Never process-exit inside the reporter (caller owns exit policy)
//! - On remote / open failure: clear error text, Wave 3 tip (serve-graph / graph-ping)
//! - On success: healthy + node count (same as pre-M312)
//! - FD_API_KEY presence without printing the secret (M321)
//!
//! Does **not** change `open_graph_store` (other commands still exit 1 on remote).

use kg_storage::GraphModeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteReadHealth {
    MissingEndpoint,
    Reachable,
    Unreachable { error_class: String },
}

pub fn remote_error_class(message: &str) -> String {
    for (needle, class) in [
        ("timed out", "timeout"),
        ("connect failed", "connect failed"),
        ("protocol", "protocol error"),
        ("PING returned", "unexpected PING response"),
    ] {
        if message.contains(needle) {
            return class.into();
        }
    }
    "remote health failure".into()
}

pub fn format_remote_graph_health_lines(status: &RemoteReadHealth) -> Vec<String> {
    let read_line = match status {
        RemoteReadHealth::MissingEndpoint => {
            "❌ not configured (set DA_GRAPH_ENDPOINT=host:port)".into()
        }
        RemoteReadHealth::Reachable => "✅ reachable (bounded scalar reads)".into(),
        RemoteReadHealth::Unreachable { error_class } => {
            format!("❌ unreachable ({error_class})")
        }
    };
    vec![
        "  DA_GRAPH_MODE:  remote".into(),
        format!("  Persistence:    {}", persistence_status_line()),
        format!("  Import:         {}", d127_import_status_line()),
        format!("  Remote reads:   {read_line}"),
        "  Remote writes:  ⛔ unsupported (embedded HOT path only)".into(),
    ]
}

/// Whether `FD_API_KEY` is usable after dotenv / process env load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdApiKeyStatus {
    /// Non-empty key present (length only is safe to display).
    Present { len: usize },
    /// Missing or empty — production embed will 401 with empty Bearer.
    Missing,
}

impl FdApiKeyStatus {
    /// Classify raw env value (do not log the value itself).
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => Self::Present { len: s.len() },
            None => Self::Missing,
        }
    }

    pub fn is_present(self) -> bool {
        matches!(self, Self::Present { .. })
    }

    /// Single health line fragment — never includes key material.
    pub fn display_line(self) -> String {
        match self {
            Self::Present { len } => format!("✅ present (len={len})"),
            Self::Missing => "❌ missing — set FD_API_KEY in .env (loaded by da) or export".into(),
        }
    }
}

/// Format embed probe result for health (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedProbeStatus {
    SkippedNoKey,
    Ok { dimensions: usize },
    Failed { message: String },
}

impl EmbedProbeStatus {
    pub fn display_line(&self) -> String {
        match self {
            Self::SkippedNoKey => "⏭ skipped (no FD_API_KEY)".into(),
            Self::Ok { dimensions } => format!("✅ ok (dim {dimensions})"),
            Self::Failed { message } => format!("❌ {message}"),
        }
    }
}

/// Operator tip when key is missing (composition root loads `.env`).
pub fn fd_api_key_tip() -> &'static str {
    "  tip: da loads project .env at startup (M321); export FD_API_KEY if running without .env"
}

/// Always-honest persistence status for process-local Samyama (M332).
pub fn persistence_status_line() -> String {
    match std::env::var("DA_GRAPH_DATA_PATH")
        .ok()
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => "✅ none (in-memory; durable path is batch-ingest --output .sgsnap)".into(),
        Some(path) => {
            format!("❌ DA_GRAPH_DATA_PATH={path:?} set but RocksDB not wired — unset for CLI open")
        }
    }
}

/// Always-honest D127 import gate status (M333). Never unlocks import.
pub fn d127_import_status_line() -> String {
    "🔒 import_eligible=false (D127 locked — needs explicit user yes)".into()
}

/// Operator tip for multi-process health path (M307–M310).
pub fn wave3_tip() -> &'static str {
    "  tip: scalar remote reads use `da graph-query --mode remote --endpoint HOST:PORT`; HOT GraphMode::remote writes remain intentionally not configured"
}

/// Format graph section lines for health output.
///
/// - `mode_text`: resolved mode label or invalid raw value
/// - `opened`: `try_from_env` succeeded (store exists in-process)
/// - `healthy`: `GraphStore::health` when opened; ignored when not opened
/// - `detail`: node count when opened, or error message when not
pub fn format_graph_health_lines(
    mode_text: &str,
    opened: bool,
    healthy: bool,
    detail: &str,
) -> Vec<String> {
    let mut lines = vec![format!("  DA_GRAPH_MODE:  {mode_text}")];
    lines.push(format!("  Persistence:    {}", persistence_status_line()));
    lines.push(format!("  Import:         {}", d127_import_status_line()));
    if opened {
        let status = if healthy { "✅ healthy" } else { "❌ down" };
        lines.push(format!("  Samyama Graph:  {status} ({detail} nodes)"));
    } else {
        lines.push(format!("  Samyama Graph:  ❌ not open ({detail})"));
        let show_tip = mode_text.eq_ignore_ascii_case("remote")
            || detail.contains("RemoteNotConfigured")
            || detail.contains("remote is not configured");
        if show_tip {
            lines.push(wave3_tip().to_string());
        }
    }
    lines
}

/// Human detail for open / mode failures (stable tokens for tests/scripts).
pub fn open_error_detail(err: &GraphModeError) -> String {
    match err {
        GraphModeError::RemoteNotConfigured => {
            "RemoteNotConfigured — full remote GraphStore not wired".into()
        }
        GraphModeError::InvalidMode { value } => format!("InvalidMode({value})"),
        GraphModeError::PersistenceNotConfigured { path } => format!(
            "PersistenceNotConfigured — DA_GRAPH_DATA_PATH={path:?} (RocksDB not wired; unset)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_health_states_separate_reads_from_hot_writes() {
        let missing =
            format_remote_graph_health_lines(&RemoteReadHealth::MissingEndpoint).join("\n");
        assert!(missing.contains("not configured"));
        assert!(missing.contains("DA_GRAPH_ENDPOINT"));
        assert!(missing.contains("Remote writes:  ⛔ unsupported"));
        let reachable = format_remote_graph_health_lines(&RemoteReadHealth::Reachable).join("\n");
        assert!(reachable.contains("✅ reachable"));
        assert!(reachable.contains("bounded scalar reads"));
        let unreachable = format_remote_graph_health_lines(&RemoteReadHealth::Unreachable {
            error_class: remote_error_class("remote graph connect failed at 127.0.0.1:9: refused"),
        })
        .join("\n");
        assert!(unreachable.contains("❌ unreachable"));
        assert!(unreachable.contains("connect failed"));
        assert!(!unreachable.contains("127.0.0.1"));
    }

    #[test]
    fn embedded_success_lines() {
        let lines = format_graph_health_lines("embedded", true, true, "12");
        let joined = lines.join("\n");
        assert!(joined.contains("DA_GRAPH_MODE:  embedded"));
        assert!(joined.contains("Persistence:"));
        assert!(joined.contains("import_eligible=false") || joined.contains("D127"));
        assert!(joined.contains("✅ healthy (12 nodes)"));
        assert!(!joined.contains("serve-graph"));
    }

    #[test]
    fn opened_but_unhealthy() {
        let lines = format_graph_health_lines("embedded", true, false, "3");
        let joined = lines.join("\n");
        assert!(joined.contains("❌ down (3 nodes)"));
        assert!(!joined.contains("not open"));
    }

    #[test]
    fn remote_failure_includes_tip_and_not_configured() {
        let detail = open_error_detail(&GraphModeError::RemoteNotConfigured);
        let lines = format_graph_health_lines("remote", false, false, &detail);
        let joined = lines.join("\n");
        assert!(joined.contains("DA_GRAPH_MODE:  remote"));
        assert!(joined.contains("❌ not open"));
        assert!(joined.contains("RemoteNotConfigured"));
        assert!(joined.contains("graph-query"));
        assert!(joined.contains("scalar remote reads"));
        assert!(joined.contains("HOT"));
    }

    #[test]
    fn embedded_failure_no_wave3_tip_unless_remote_token() {
        let lines = format_graph_health_lines("embedded", false, false, "disk full");
        let joined = lines.join("\n");
        assert!(joined.contains("❌ not open (disk full)"));
        assert!(!joined.contains("serve-graph"));
    }

    #[test]
    fn fd_key_status_present_and_missing() {
        assert_eq!(
            FdApiKeyStatus::from_env_value(None),
            FdApiKeyStatus::Missing
        );
        assert_eq!(
            FdApiKeyStatus::from_env_value(Some("")),
            FdApiKeyStatus::Missing
        );
        assert_eq!(
            FdApiKeyStatus::from_env_value(Some("   ")),
            FdApiKeyStatus::Missing
        );
        assert_eq!(
            FdApiKeyStatus::from_env_value(Some("abc123")),
            FdApiKeyStatus::Present { len: 6 }
        );
        let line = FdApiKeyStatus::Present { len: 43 }.display_line();
        assert!(line.contains("present"));
        assert!(line.contains("43"));
        assert!(!line.contains("abc"));
        assert!(!FdApiKeyStatus::Missing.display_line().contains("Bearer"));
    }

    #[test]
    fn embed_probe_status_lines_have_no_secret_shape() {
        assert!(
            EmbedProbeStatus::SkippedNoKey
                .display_line()
                .contains("skipped")
        );
        assert!(
            EmbedProbeStatus::Ok { dimensions: 1024 }
                .display_line()
                .contains("1024")
        );
        let fail = EmbedProbeStatus::Failed {
            message: "fd_api returned 401 Unauthorized".into(),
        };
        let line = fail.display_line();
        assert!(line.contains("401"));
        assert!(!line.contains("Bearer sk"));
    }
    #[test]
    fn persistence_not_configured_detail() {
        let detail = open_error_detail(&GraphModeError::PersistenceNotConfigured {
            path: "/tmp/x".into(),
        });
        assert!(detail.contains("PersistenceNotConfigured"), "{detail}");
        assert!(
            detail.contains("/tmp/x") || detail.contains("DA_GRAPH_DATA_PATH"),
            "{detail}"
        );
        let lines = format_graph_health_lines("embedded", false, false, &detail);
        let joined = lines.join("\n");
        assert!(joined.contains("Persistence:"));
        assert!(joined.contains("not open"));
        assert!(!joined.contains("serve-graph"));
    }

    #[test]
    fn persistence_status_line_is_honest() {
        let line = persistence_status_line();
        assert!(
            line.contains("none") || line.contains("DA_GRAPH_DATA_PATH"),
            "{line}"
        );
    }
    #[test]
    fn d127_import_status_line_locked() {
        let line = d127_import_status_line();
        assert!(line.contains("import_eligible=false"), "{line}");
        assert!(line.contains("D127"), "{line}");
        assert!(!line.contains("true"));
    }
}
