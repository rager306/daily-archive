//! Inspect one canonical TeX archive into candidate-only bibliography evidence.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilesystemTexAnalysisReportStore, FilesystemTexArchive};
use da_application::TexInspectUseCase;
use da_domain::{ContentDigest, ProviderDenominator};
use da_ports::TexAnalysisRequest;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    archive: PathBuf,
    #[arg(long)]
    inventory: PathBuf,
    /// Optional article-source-evidence.v1 provider evidence.
    #[arg(long)]
    provider_evidence: Option<PathBuf>,
    #[arg(long)]
    report: PathBuf,
}

pub fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("tex_source_inspect");
    let args = Args::parse();
    let archive = FilesystemTexArchive::new(&args.archive, &args.inventory);
    let providers = args
        .provider_evidence
        .as_deref()
        .map(load_providers)
        .transpose()?
        .unwrap_or_default();
    let store = FilesystemTexAnalysisReportStore::new(&args.report);
    let report = TexInspectUseCase::with_store(&archive, &store)
        .inspect(TexAnalysisRequest { providers })?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn load_providers(path: &std::path::Path) -> Result<Vec<ProviderDenominator>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let value: Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    if value["schema_version"].as_str() != Some("article-source-evidence.v1")
        || value["import_eligible"].as_bool() != Some(false)
    {
        anyhow::bail!("provider evidence schema or D127 authority is invalid");
    }
    let candidates = value["candidate_metadata"]
        .as_array()
        .context("candidate_metadata array missing")?;
    let mut providers = Vec::new();
    for candidate in candidates {
        let Some(provider) = candidate["provider"].as_str() else {
            continue;
        };
        let count = candidate["referenced_works_count"]
            .as_u64()
            .or_else(|| {
                candidate["observed_via_public_api_before_rate_limit"]["reference_count"].as_u64()
            })
            .map(|value| value as usize);
        let payload_sha256 = candidate["sha256"]
            .as_str()
            .map(str::to_string)
            .map(ContentDigest::try_from)
            .transpose()
            .with_context(|| format!("invalid {provider} payload SHA-256"))?;
        providers.push(ProviderDenominator {
            provider: provider.into(),
            count,
            payload_sha256,
            status: candidate["status"].as_str().unwrap_or("unknown").into(),
        });
    }
    providers.sort_by(|a, b| a.provider.cmp(&b.provider));
    Ok(providers)
}
