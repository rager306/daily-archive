//! M444: corpus regression check over materialization run reports.
//!
//! Compares two run reports (before/after a rebuild) and fails loudly on
//! regressions: removed papers, revision drift, per-paper structural drops
//! beyond tolerance, lost replay honesty, failed papers. Works over ALL
//! articles — the gold corpus is not involved.
//!
//! Usage:
//!   cargo run -p da-cli --example corpus_regression_check -- \
//!     --before artifacts/catalog_sweep/materialization_run_report.json \
//!     --after  /tmp/new_run_report.json \
//!     --report /tmp/regression_report.json

use anyhow::{Context, Result};
use clap::Parser;
use da_cli::catalog_regression::diff_reports;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    /// Baseline run report (e.g. the committed full-run report).
    #[arg(long)]
    before: PathBuf,
    /// Candidate run report (e.g. after a rebuild).
    #[arg(long)]
    after: PathBuf,
    /// Output regression report JSON.
    #[arg(long, default_value = "artifacts/catalog_sweep/regression_report.json")]
    report: PathBuf,
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("corpus_regression_check");
    let args = Args::parse();
    let before =
        std::fs::read(&args.before).with_context(|| format!("read {}", args.before.display()))?;
    let after =
        std::fs::read(&args.after).with_context(|| format!("read {}", args.after.display()))?;
    let result = diff_reports(&before, &after).with_context(|| "parse run reports")?;
    let json = serde_json::to_vec_pretty(&result)?;
    da_cli::gold_graph::atomic_write(&args.report, &json)
        .with_context(|| format!("write {}", args.report.display()))?;
    println!(
        "regression check: before {} papers, after {} papers, {} finding(s)",
        result.before_papers,
        result.after_papers,
        result.regressions.len()
    );
    for finding in &result.regressions {
        println!("  - {finding:?}");
    }
    println!("report: {}", args.report.display());
    if !result.is_clean() {
        std::process::exit(1);
    }
    Ok(())
}
