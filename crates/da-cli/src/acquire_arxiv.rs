//! Rust-only batch arXiv acquisition composition root.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use da_adapters::ArxivSourceAcquisition;
use da_application::acquisition_batch::BatchAcquisitionUseCase;
use da_domain::acquisition_batch::{BatchAcquisitionItem, BatchAcquisitionStatus};

#[derive(Debug, Args)]
pub struct AcquireArxivArgs {
    #[arg(long)]
    pub ids: Option<String>,
    #[arg(long)]
    pub ids_file: Option<PathBuf>,
    #[arg(long, default_value = "data/articles")]
    pub catalog_root: PathBuf,
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long, default_value_t = false)]
    pub allow_network: bool,
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u64).range(0..=300))]
    pub sleep_seconds: u64,
}

pub fn run(args: &AcquireArxivArgs) -> Result<i32> {
    let ids = crate::batch_ids::resolve_batch_ids(args.ids.as_deref(), args.ids_file.as_deref())?;
    let total = ids.len();
    let use_case =
        BatchAcquisitionUseCase::new(Arc::new(ArxivSourceAcquisition::new(&args.catalog_root)));
    let report = use_case
        .execute(&ids, args.allow_network, |item| {
            render(item, total);
            if item.ordinal < total && args.sleep_seconds > 0 {
                std::thread::sleep(Duration::from_secs(args.sleep_seconds));
            }
        })
        .map_err(anyhow::Error::msg)?;
    atomic_write_json(&args.report, &report)?;
    eprintln!(
        "summary total={} ok={} skip={} fail={} report={} import_eligible=false",
        report.total,
        report.ok,
        report.skip,
        report.fail,
        args.report.display()
    );
    Ok(if report.fail == 0 { 0 } else { 1 })
}

fn render(item: &BatchAcquisitionItem, total: usize) {
    let status = match item.status {
        BatchAcquisitionStatus::Ok => "ok",
        BatchAcquisitionStatus::Skip => "skip",
        BatchAcquisitionStatus::Fail => "fail",
    };
    if let Some(error) = &item.error {
        eprintln!(
            "[{}/{}] {} {} error={}",
            item.ordinal, total, item.id, status, error
        );
    } else {
        eprintln!(
            "[{}/{}] {} {} assets={}",
            item.ordinal,
            total,
            item.id,
            status,
            item.assets.len()
        );
    }
}

fn atomic_write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        std::fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
