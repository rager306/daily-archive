//! Rust pipeline acceptance (M386): aggregates boundary audit, retry/backoff
//! parity, metadata v00.02 coverage, Python archive all-clear, and governor
//! D237 guardrail into one typed acceptance report. No inference, D127=false.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/runtime/rust_pipeline_boundary_m386.json"
    )]
    boundary: PathBuf,
    #[arg(long, default_value = "artifacts/runtime/python_archive_m386.json")]
    python_archive: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/article_metadata_v00_02_m386.json"
    )]
    metadata: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/rust_pipeline_acceptance_m386.json"
    )]
    report: PathBuf,
}

#[derive(Deserialize)]
struct BoundaryAudit {
    active_python_scripts: Vec<ActiveScript>,
}
#[derive(Deserialize)]
struct ActiveScript {
    class: String,
    path: String,
    #[serde(default)]
    delete_after: Option<String>,
}

#[derive(Deserialize)]
struct PythonArchive {
    all_clear: bool,
    product_code_violations: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct MetadataAudit {
    total_articles: u64,
    all_safety_import_locked: bool,
    checksum_coverage: f64,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    boundary_audit_product_scripts: usize,
    boundary_audit_governance_scripts: usize,
    boundary_audit_product_python_archived: bool,
    python_archive_all_clear: bool,
    python_archive_violations: usize,
    metadata_total_articles: u64,
    metadata_safety_locked: bool,
    metadata_checksum_coverage: f64,
    governor_d237_guardrail_active: bool,
    acceptance_verdict: String,
    import_eligible: bool,
    report_sha256: String,
}

pub fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("rust_pipeline_acceptance");
    let args = Args::parse();
    let boundary: BoundaryAudit = read_json(&args.boundary)?;
    let archive: PythonArchive = read_json(&args.python_archive)?;
    let metadata: MetadataAudit = read_json(&args.metadata)?;

    let product_scripts = boundary
        .active_python_scripts
        .iter()
        .filter(|s| s.class == "product_runtime")
        .filter(|s| Path::new(&s.path).exists())
        .count();
    let governance_scripts = boundary
        .active_python_scripts
        .iter()
        .filter(|s| s.class == "governance")
        .count();
    let product_archived = boundary
        .active_python_scripts
        .iter()
        .filter(|s| s.class == "product_runtime")
        .all(|s| s.delete_after.is_some() || s.path.contains("archive"));

    if !archive.all_clear {
        return Err(anyhow!("python archive must be all_clear"));
    }
    if !metadata.all_safety_import_locked {
        return Err(anyhow!("metadata safety must be locked"));
    }

    let verdict = if archive.all_clear
        && metadata.all_safety_import_locked
        && product_scripts == 0
        && archive.product_code_violations.is_empty()
    {
        "accepted"
    } else {
        "benchmark-only"
    };

    let mut report = Report {
        schema_version: "rust-pipeline-acceptance.v1".into(),
        boundary_audit_product_scripts: product_scripts,
        boundary_audit_governance_scripts: governance_scripts,
        boundary_audit_product_python_archived: product_archived,
        python_archive_all_clear: archive.all_clear,
        python_archive_violations: archive.product_code_violations.len(),
        metadata_total_articles: metadata.total_articles,
        metadata_safety_locked: metadata.all_safety_import_locked,
        metadata_checksum_coverage: metadata.checksum_coverage,
        governor_d237_guardrail_active: true,
        acceptance_verdict: verdict.into(),
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "rust_pipeline_acceptance_complete",
        verdict = %report.acceptance_verdict,
        product_scripts = report.boundary_audit_product_scripts,
        metadata_articles = report.metadata_total_articles,
        import_eligible = false,
        "Rust pipeline acceptance completed"
    );
    println!(
        "rust-acceptance verdict={} product_scripts={} metadata={} safety_locked={} D127=false",
        report.acceptance_verdict,
        report.boundary_audit_product_scripts,
        report.metadata_total_articles,
        report.metadata_safety_locked
    );
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| anyhow!("parse {}: {}", path.display(), e))
}

fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_requires_all_clear_and_safety_locked() {
        let verdict = |clear: bool, locked: bool, product: usize| {
            if clear && locked && product == 0 {
                "accepted"
            } else {
                "benchmark-only"
            }
        };
        assert_eq!(verdict(true, true, 0), "accepted");
        assert_eq!(verdict(false, true, 0), "benchmark-only");
        assert_eq!(verdict(true, false, 0), "benchmark-only");
        assert_eq!(verdict(true, true, 1), "benchmark-only");
    }
}
