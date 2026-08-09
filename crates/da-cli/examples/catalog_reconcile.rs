//! Deterministic read-only article-catalog reconciliation inventory.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::FilesystemCatalogInventory;
use da_application::CatalogReconcileUseCase;
use da_cli::gold_graph::atomic_write;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/article_catalog")]
    catalog_root: PathBuf,
    #[arg(long, default_value = "artifacts/article_catalog/reconciliation.json")]
    report: PathBuf,
    #[arg(long)]
    fail_on_findings: bool,
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("catalog_reconcile");
    let args = Args::parse();
    let report = CatalogReconcileUseCase::new(FilesystemCatalogInventory::new(&args.catalog_root))
        .inspect()
        .context("catalog reconciliation inventory failed")?;
    atomic_write(&args.report, &(serde_json::to_vec_pretty(&report)?))?;
    tracing::info!(event="catalog_inventory_completed", metadata=report.article_metadata_count, pdf=report.pdf_count, html=report.html_count, canonical_identities=report.canonical_identity_count, findings=report.findings.len(), report=%args.report.display(), "Catalog reconciliation inventory completed");
    println!(
        "metadata={} pdf={} html={} identities={} findings={} report={}",
        report.article_metadata_count,
        report.pdf_count,
        report.html_count,
        report.canonical_identity_count,
        report.findings.len(),
        args.report.display()
    );
    if args.fail_on_findings && !report.is_clean() {
        anyhow::bail!(
            "catalog has {} reconciliation finding(s)",
            report.findings.len()
        );
    }
    Ok(())
}
