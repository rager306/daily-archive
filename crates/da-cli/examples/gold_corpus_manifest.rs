//! Build the deterministic gold-corpus source/layer manifest and batch plan.

use anyhow::{Context, Result};
use clap::Parser;
use da_cli::gold_corpus::{build_gold_corpus_manifest, layer_totals, write_manifest};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "data/gold_standard")]
    fixtures: PathBuf,
    #[arg(long, default_value = "data/article_catalog/article_catalog")]
    catalog: PathBuf,
    #[arg(long, default_value_t = 10)]
    batch_size: usize,
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let manifest = build_gold_corpus_manifest(&args.fixtures, &args.catalog, args.batch_size)
        .map_err(anyhow::Error::msg)
        .context("build gold corpus manifest")?;
    write_manifest(&args.output, &manifest).map_err(anyhow::Error::msg)?;
    println!(
        "gold_corpus fixtures={} ready={} unavailable={} batches={} batch_size={}",
        manifest.fixture_count,
        manifest.source_ready_count,
        manifest.unavailable_count,
        manifest.batch_count,
        manifest.batch_size
    );
    for (layer, count) in layer_totals(&manifest) {
        println!("layer {layer}={count}");
    }
    for record in manifest
        .records
        .iter()
        .filter(|record| record.source.path.is_none())
    {
        println!(
            "unavailable {}: {}",
            record.paper_id,
            record
                .source
                .unavailable_reason
                .as_deref()
                .unwrap_or("unknown")
        );
    }
    println!("manifest={}", args.output.display());
    Ok(())
}
