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
    #[arg(long, default_value = "artifacts/article_catalog/reconcile_plan.json")]
    plan: PathBuf,
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
    let plan = CatalogReconcileUseCase::<FilesystemCatalogInventory>::plan(&report);
    atomic_write(&args.plan, &serde_json::to_vec_pretty(&plan)?)?;
    tracing::info!(event="catalog_inventory_completed", metadata=report.article_metadata_count, pdf=report.pdf_count, html=report.html_count, canonical_identities=report.canonical_identity_count, findings=report.findings.len(), planned_actions=plan.actions.len(), auto_applicable_actions=plan.auto_applicable_count(), report=%args.report.display(), plan=%args.plan.display(), "Catalog reconciliation inventory and plan completed");
    println!(
        "metadata={} pdf={} html={} identities={} findings={} actions={} auto={} report={} plan={}",
        report.article_metadata_count,
        report.pdf_count,
        report.html_count,
        report.canonical_identity_count,
        report.findings.len(),
        plan.actions.len(),
        plan.auto_applicable_count(),
        args.report.display(),
        args.plan.display()
    );
    if args.fail_on_findings && !report.is_clean() {
        anyhow::bail!(
            "catalog has {} reconciliation finding(s)",
            report.findings.len()
        );
    }
    Ok(())
}
