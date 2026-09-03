//! Derive deterministic zero-mention drift statistics from a materialization report.
//!
//! Usage:
//!   cargo run -p da-cli --example catalog_drift_stats -- \
//!     --run-report archive/artifacts-legacy/catalog_sweep/materialization_run_report.json \
//!     --output artifacts/catalog_sweep/catalog_drift_stats.json

use anyhow::{Context, Result};
use clap::Parser;
use da_cli::catalog_drift::derive_stats;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    /// Materialization run report to inspect.
    #[arg(long)]
    run_report: PathBuf,
    /// Destination for the deterministic drift report.
    #[arg(long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("catalog_drift_stats");
    let args = Args::parse();
    let input = std::fs::read(&args.run_report)
        .with_context(|| format!("read run report {}", args.run_report.display()))?;
    let stats = derive_stats(&input).with_context(|| {
        format!(
            "validate materialization run report {}",
            args.run_report.display()
        )
    })?;
    let output = serde_json::to_vec_pretty(&stats)?;
    da_cli::gold_graph::atomic_write(&args.output, &output)
        .with_context(|| format!("write drift report {}", args.output.display()))?;
    println!(
        "drift stats: {} completed papers, {} mentions, {} zero-mention alarms",
        stats.completed_papers, stats.total_mentions, stats.zero_mention_papers
    );
    println!("report: {}", args.output.display());
    Ok(())
}
