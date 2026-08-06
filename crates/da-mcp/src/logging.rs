//! Structured logging initialization (M359, ADR-052).
//!
//! Hexagonal architecture compliance:
//! - **da-domain**: no tracing dependency (pure types)
//! - **da-ports**: no tracing dependency (pure traits)
//! - **da-application**: `tracing` crate only (`#[instrument]`, `info!`)
//! - **da-adapters**: `tracing` crate only (`info!`, `warn!`)
//! - **da-mcp** (this crate): tracing-subscriber + tracing-appender (infrastructure)
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
pub fn init_logging() -> Option<WorkerGuard> {
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    // Build filter: RUST_LOG > LOG_LEVEL > default info
    let default_filter = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&default_filter));

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
