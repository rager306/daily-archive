//! Structured logging initialization (M359, ADR-052).
//!
//! Hexagonal architecture compliance:
//! - **da-domain**: no tracing dependency (pure types)
//! - **da-ports**: no tracing dependency (pure traits)
//! - **da-application**: `tracing` crate only (`#[instrument]`, `info!`)
//! - **da-adapters**: `tracing` crate only (`info!`, `warn!`)
//! - **da-cli** (this crate): tracing-subscriber + tracing-appender (infrastructure)
//!
//! Best practices from tracing community (2025-2026):
//!
//! 1. **Two layers**: stderr (compact, ANSI) + file (JSON, no ANSI)
//! 2. **Daily rolling files** under LOG_DIR (default: logs/)
//! 3. **with_ansi(false)** on file layer — ANSI codes corrupt JSON
//! 4. **flatten_event(true)** — event fields at top level for jq queries
//! 5. **with_span_events(FmtSpan::CLOSE)** — log span close with duration
//! 6. **WorkerGuard** returned from init, held in main() — #1 footgun
//! 7. **try_init()** — don't panic if subscriber already set
//! 8. **#[instrument(skip(large_field))]** on pipeline functions
//!
//! Future agents parse JSON logs:
//! ```sh
//! jq 'select(.paper_id == "2507.19457")' logs/da.log.YYYY-MM-DD
//! jq 'select(.level == "ERROR")' logs/da.log.YYYY-MM-DD
//! ```

use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Third-party diagnostics suppressed by default.
///
/// `lopdf` emits one `WARN` per undecodable glyph / font-encoding difference in
/// real-world PDFs (observed: ~2.5k lines/day across 19 distinct messages),
/// which drowns our own signal in `logs/da.log.*` and defeats the documented
/// "query the log by level" workflow. The directive names an explicit
/// third-party target, so no `da-*`/`kg-*` target is affected, and `error` (not
/// `off`) keeps genuine PDF parse failures visible. Override with
/// `LOG_SUPPRESS` (comma-separated `target=level` directives; empty string
/// disables suppression).
const DEFAULT_LOG_SUPPRESS: &str = "lopdf::object=error";

/// Append suppression directives to the base filter.
///
/// Kept as a pure function so the composition rule is unit-testable without
/// initialising a subscriber.
fn compose_filter(base: &str, suppress: &str) -> String {
    let base = base.trim();
    let suppress = suppress.trim();
    if suppress.is_empty() {
        base.to_string()
    } else if base.is_empty() {
        suppress.to_string()
    } else {
        format!("{base},{suppress}")
    }
}

/// Initialize logging. Returns WorkerGuard that **must be kept alive**
/// in the caller's scope (typically `main`) for the duration of the program.
///
/// Dropping the guard flushes the non-blocking writer — if it drops too
/// early (e.g. inside a helper function), file logs silently stop writing.
/// This is the #1 tracing footgun documented in tracing-appender docs.
///
/// # Configuration (env vars)
///
/// | Var | Default | Purpose |
/// |-----|---------|---------|
/// | `LOG_DIR` | `logs/` | Directory for rolling log files |
/// | `LOG_LEVEL` | `info` | Filter level (RUST_LOG overrides if set) |
/// | `LOG_SUPPRESS` | `lopdf::object=error` | Extra `target=level` directives appended after the base filter |
pub fn init_logging() -> Option<WorkerGuard> {
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    // Build filter: RUST_LOG > LOG_LEVEL > default info, then append the
    // suppression directives so third-party noise cannot drown our own signal.
    let base_filter = std::env::var("RUST_LOG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()));
    let suppress =
        std::env::var("LOG_SUPPRESS").unwrap_or_else(|_| DEFAULT_LOG_SUPPRESS.to_string());
    let composed = compose_filter(&base_filter, &suppress);
    let filter = EnvFilter::try_new(&composed).unwrap_or_else(|e| {
        eprintln!("⚠  Invalid log filter `{composed}`: {e}; falling back to `info`");
        EnvFilter::new("info")
    });

    // Create log directory
    let log_path = PathBuf::from(&log_dir);
    if let Err(e) = std::fs::create_dir_all(&log_path) {
        eprintln!("⚠  Cannot create log dir {log_dir}: {e}; falling back to stderr-only");
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .compact()
            .try_init();
        return None;
    }

    // Rolling daily file appender: logs/da.log.YYYY-MM-DD
    let file_appender = tracing_appender::rolling::daily(&log_dir, "da.log");
    let (file_writer, guard) = tracing_appender::non_blocking::NonBlocking::new(file_appender);

    // Build layered subscriber — direct composition without type erasure.
    // Per-layer filtering requires nightly features; use global filter instead.
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .with_ansi(true)
        .compact();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_file(false)
        .with_line_number(true)
        .with_ansi(false);

    let result = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    if let Err(e) = result {
        eprintln!("⚠  Logging init returned non-fatal: {e}");
    }

    let abs_path = log_path.canonicalize().unwrap_or_else(|_| log_path.clone());
    tracing::info!(
        log_dir = %abs_path.display(),
        "Logging initialized: stderr (compact) + rolling file (JSON)"
    );
    tracing::debug!(
        log_dir = %abs_path.display(),
        "WorkerGuard active — file logs will flush on program exit"
    );

    Some(guard)
}

/// Initialize a pipeline composition root from project `.env` and retainable guard.
///
/// The composition root owns dotenv and subscriber setup; domain/application code
/// only emits structured events and remains infrastructure-agnostic.
pub fn init_pipeline_logging(composition_root: &'static str) -> Option<WorkerGuard> {
    let _ = dotenvy::dotenv();
    let guard = init_logging();
    tracing::info!(
        composition_root,
        event = "pipeline_composition_started",
        "Pipeline composition root started"
    );
    guard
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tracing_subscriber::EnvFilter;

    use super::{DEFAULT_LOG_SUPPRESS, compose_filter};

    #[test]
    fn suppression_directives_append_after_the_base_filter() {
        assert_eq!(
            compose_filter("info", DEFAULT_LOG_SUPPRESS),
            "info,lopdf::object=error"
        );
        assert_eq!(compose_filter("  debug  ", " a=warn "), "debug,a=warn");
    }

    #[test]
    fn empty_suppression_is_a_no_op_and_never_injects_a_dangling_comma() {
        assert_eq!(compose_filter("info", ""), "info");
        assert_eq!(compose_filter("info", "   "), "info");
        assert_eq!(compose_filter("", "a=warn"), "a=warn");
        assert_eq!(compose_filter("", ""), "");
    }

    #[test]
    fn default_suppression_is_valid_and_targets_only_a_third_party_crate() {
        let composed = compose_filter("info", DEFAULT_LOG_SUPPRESS);
        assert!(
            EnvFilter::try_new(&composed).is_ok(),
            "default filter must parse: {composed}"
        );
        let target = DEFAULT_LOG_SUPPRESS
            .split('=')
            .next()
            .expect("target directive");
        assert!(
            !target.starts_with("da_") && !target.starts_with("da-") && !target.starts_with("kg_"),
            "suppression must never silence our own crates: {target}"
        );
    }

    #[test]
    fn active_pipeline_examples_initialize_structured_file_logging() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(
                source.contains("init_pipeline_logging"),
                "{} bypasses .env-backed rolling JSON logging",
                path.display()
            );
        }
    }
}
