//! Deterministic read-only article-catalog reconciliation inventory.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilesystemCatalogInventory, FilesystemCatalogMutation};
use da_application::CatalogReconcileUseCase;
use da_cli::gold_graph::atomic_write;
use da_ports::CatalogMutationPort;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/sources/arxiv")]
    catalog_root: PathBuf,
    #[arg(long, default_value = "artifacts/article_catalog/reconciliation.json")]
    report: PathBuf,
    #[arg(long, default_value = "artifacts/article_catalog/reconcile_plan.json")]
    plan: PathBuf,
    #[arg(long)]
    fail_on_findings: bool,
    #[arg(long)]
    apply_low: bool,
    #[arg(long, default_value = "artifacts/article_catalog/apply_report.json")]
    apply_report: PathBuf,
    #[arg(long)]
    rebuild_index: bool,
}

pub fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("catalog_reconcile");
    let args = Args::parse();
    let report = CatalogReconcileUseCase::new(FilesystemCatalogInventory::new(&args.catalog_root))
        .inspect()
        .context("catalog reconciliation inventory failed")?;
    atomic_write(&args.report, &(serde_json::to_vec_pretty(&report)?))?;
    let plan = CatalogReconcileUseCase::<FilesystemCatalogInventory>::plan(&report);
    atomic_write(&args.plan, &serde_json::to_vec_pretty(&plan)?)?;
    let apply = if args.apply_low {
        let receipt = CatalogReconcileUseCase::<FilesystemCatalogInventory>::apply_low(
            &plan,
            &report,
            &FilesystemCatalogMutation::new(&args.catalog_root),
        )?;
        atomic_write(&args.apply_report, &serde_json::to_vec_pretty(&receipt)?)?;
        tracing::info!(event="catalog_low_actions_applied", attempted=receipt.attempted, applied=receipt.applied, skipped=receipt.skipped, rejected=receipt.rejected, write_ops=receipt.write_ops, apply_report=%args.apply_report.display(), "Low-risk catalog actions applied");
        Some(receipt)
    } else {
        None
    };
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
    if args.rebuild_index {
        let bytes = build_canonical_index(&args.catalog_root)?;
        let mutation = FilesystemCatalogMutation::new(&args.catalog_root);
        let root = mutation.write_file_atomic("index.json", &bytes)?;
        let nested = mutation.write_file_atomic("article_catalog/index.json", &bytes)?;
        tracing::info!(event="catalog_indexes_rebuilt", entries=index_entry_count(&bytes)?, root_disposition=?root, nested_disposition=?nested, "Canonical catalog indexes rebuilt atomically");
        println!(
            "index root={root:?} nested={nested:?} entries={}",
            index_entry_count(&bytes)?
        );
    }
    if let Some(receipt) = apply {
        println!(
            "apply attempted={} applied={} skipped={} rejected={} write_ops={} report={}",
            receipt.attempted,
            receipt.applied,
            receipt.skipped,
            receipt.rejected,
            receipt.write_ops,
            args.apply_report.display()
        );
    }
    if args.fail_on_findings && !report.is_clean() {
        anyhow::bail!(
            "catalog has {} reconciliation finding(s)",
            report.findings.len()
        );
    }
    Ok(())
}

fn build_canonical_index(root: &Path) -> Result<Vec<u8>> {
    let mut files = Vec::new();
    collect_article_json(root, &mut files)?;
    files.sort();
    let mut chosen: BTreeMap<String, (bool, serde_json::Value, String)> = BTreeMap::new();
    for path in files {
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let raw = value
            .get("article_key")
            .and_then(|v| v.as_str())
            .or_else(|| value.pointer("/identity/arxiv_id").and_then(|v| v.as_str()))
            .context("article identity missing")?;
        let id = raw
            .rsplit_once('v')
            .filter(|(_, s)| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .map_or(raw, |(b, _)| b)
            .to_string();
        let bare = raw == id;
        let rel = path.strip_prefix(root)?.display().to_string();
        if !chosen.contains_key(&id) || bare {
            chosen.insert(id, (bare, value, rel));
        }
    }
    let articles=chosen.into_iter().map(|(id,(_,v,path))|serde_json::json!({"article_key":id,"article_ref":format!("arxiv:{id}"),"source_code":v.get("source_code").cloned().unwrap_or_default(),"coarse_topic_code":v.get("coarse_topic_code").cloned().unwrap_or_default(),"canonical_url":v.pointer("/identity/canonical_url").cloned().unwrap_or_default(),"primary_source_role":v.pointer("/source_strategy/primary_source_variant_id").cloned().unwrap_or_default(),"content_fallback_roles":[],"metadata_roles":[],"article_path":path,"title":v.pointer("/identity/title").cloned().unwrap_or_default()})).collect::<Vec<_>>();
    let payload = serde_json::json!({"catalog_schema_version":"article-catalog.v00.02","index_id":"canonical-article-catalog","article_count":articles.len(),"articles":articles,"safety_defaults":{"graph_import_allowed":false,"production_ladybugdb_write_allowed":false,"trusted_kg_import_allowed":false}});
    let mut bytes = serde_json::to_vec_pretty(&payload)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn collect_article_json(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in std::fs::read_dir(root)? {
        let p = e?.path();
        if p.is_dir() {
            collect_article_json(&p, out)?
        } else if p.file_name().is_some_and(|n| n == "article.json") {
            out.push(p)
        }
    }
    Ok(())
}
fn index_entry_count(bytes: &[u8]) -> Result<usize> {
    Ok(
        serde_json::from_slice::<serde_json::Value>(bytes)?["article_count"]
            .as_u64()
            .unwrap_or(0) as usize,
    )
}
