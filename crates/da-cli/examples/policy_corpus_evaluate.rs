//! SHA-bound, candidate-only paired-policy evaluation. No parsing, journals or graph writes.

#[path = "policy_corpus/evaluation.rs"]
mod evaluation;
#[path = "policy_corpus/shared.rs"]
mod shared;

use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Evaluate frozen paired captures against native DATA on fixed held-out PDFs")]
struct Args {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    registry: PathBuf,
    /// Repository root. Manifest capture_dir is resolved relative to this directory.
    #[arg(long)]
    capture_root: PathBuf,
    #[arg(long)]
    train_limit: usize,
    #[arg(long)]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let _logging_guard = da_cli::logging::init_pipeline_logging("policy_corpus_evaluate");
    evaluation::run(
        &args.manifest,
        &args.config,
        &args.registry,
        &args.capture_root,
        args.train_limit,
        &args.out,
    )
}
