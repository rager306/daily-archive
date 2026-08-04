//! Pure report lines for `da health` graph section (M312).
//!
//! Honesty goals:
//! - Always print `DA_GRAPH_MODE` (resolved or invalid)
//! - Never process-exit inside the reporter (caller owns exit policy)
//! - On remote / open failure: clear error text, Wave 3 tip (serve-graph / graph-ping)
//! - On success: healthy + node count (same as pre-M312)
//!
//! Does **not** change `open_graph_store` (other commands still exit 1 on remote).

use kg_storage::{GraphMode, GraphModeError};

/// Operator tip for multi-process health path (M307–M310).
pub fn wave3_tip() -> &'static str {
    "  tip: multi-process RESP health uses `da serve-graph` + `da graph-ping` (port 16379); full GraphMode::remote store is not configured"
}

/// Label for a successfully resolved mode.
pub fn mode_label(mode: GraphMode) -> &'static str {
    match mode {
        GraphMode::Embedded => "embedded",
        GraphMode::Remote => "remote",
    }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_success_lines() {
        let lines = format_graph_health_lines("embedded", true, true, "12");
        let joined = lines.join("\n");
        assert!(joined.contains("DA_GRAPH_MODE:  embedded"));
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
        assert!(joined.contains("serve-graph"));
        assert!(joined.contains("graph-ping"));
        assert!(joined.contains("16379"));
    }

    #[test]
    fn embedded_failure_no_wave3_tip_unless_remote_token() {
        let lines = format_graph_health_lines("embedded", false, false, "disk full");
        let joined = lines.join("\n");
        assert!(joined.contains("❌ not open (disk full)"));
        assert!(!joined.contains("serve-graph"));
    }

    #[test]
    fn mode_label_stable() {
        assert_eq!(mode_label(GraphMode::Embedded), "embedded");
        assert_eq!(mode_label(GraphMode::Remote), "remote");
    }
}
