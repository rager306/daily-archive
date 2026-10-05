//! Audit article metadata schema_version and safety coverage across the catalog.
//! D127=false, import_eligible=false throughout.

use std::collections::BTreeMap;

use anyhow::Result;
use clap::Parser;
use da_domain::assurance::ContentDigest;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/sources/arxiv/article_catalog/arxiv")]
    arxiv_root: PathBuf,
    #[arg(
        long,
        default_value = "data/sources/arxiv/article_catalog/gnn_textbook"
    )]
    gnn_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/article_metadata_v00_02_m386.json"
    )]
    report: PathBuf,
}

#[derive(Serialize)]
struct SchemaVersionCount {
    schema_version: String,
    article_count: u64,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    total_articles: u64,
    by_schema_version: Vec<SchemaVersionCount>,
    all_safety_import_locked: bool,
    checksum_coverage: f64,
    report_sha256: String,
}

fn scan_catalog(root: &Path) -> Vec<(String, serde_json::Value)> {
    let mut articles = Vec::new();
    if !root.is_dir() {
        return articles;
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name().to_string_lossy() == "article.json"
            && let Ok(bytes) = std::fs::read(entry.path())
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            let version = value
                .get("schema_version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            articles.push((version, value));
        }
    }
    articles
}

fn check_safety_locked(value: &serde_json::Value) -> bool {
    let sf = value.get("safety_flags");
    let sd = value.get("safety_defaults");
    let acq = value.get("acquisition");
    // Accept either graph_import_allowed or trusted_kg_import_allowed (v00.01 compat).
    let sf_import = sf
        .and_then(|s| {
            s.get("graph_import_allowed")
                .or_else(|| s.get("trusted_kg_import_allowed"))
        })
        .and_then(|v| v.as_bool())
        == Some(false);
    let sf_prod = !sf
        .and_then(|s| s.get("production_import_attempted"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // safety_defaults is optional in v00.01; when present, production_import_authorized must be false.
    let sd_ok = sd.is_none()
        || sd
            .and_then(|s| s.get("production_import_authorized"))
            .and_then(|v| v.as_bool())
            == Some(false);
    // acquisition.import_eligible is optional in v00.01; when present, must be false.
    let acq_ok = acq.is_none()
        || acq
            .and_then(|a| a.get("import_eligible"))
            .and_then(|v| v.as_bool())
            == Some(false);
    sf_import && sf_prod && sd_ok && acq_ok
}

fn has_checksum(value: &serde_json::Value) -> bool {
    value
        .get("source_variants")
        .and_then(|v| v.as_array())
        .map(|variants| {
            variants.iter().any(|v| {
                v.get("sha256")
                    .and_then(|s| s.as_str())
                    .is_some_and(|s| !s.is_empty())
            })
        })
        .unwrap_or(false)
}

pub fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("article_metadata_audit");
    let args = Args::parse();

    let arxiv = scan_catalog(&args.arxiv_root);
    let gnn = scan_catalog(&args.gnn_root);
    let all: Vec<&(String, serde_json::Value)> = arxiv.iter().chain(gnn.iter()).collect();

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut checksum_count = 0u64;
    let mut all_locked = true;
    for (version, value) in &all {
        *counts.entry(version.clone()).or_default() += 1;
        if has_checksum(value) {
            checksum_count += 1;
        }
        if !check_safety_locked(value) {
            all_locked = false;
        }
    }

    let total = all.len() as u64;
    let by_schema_version = counts
        .into_iter()
        .map(|(schema_version, article_count)| SchemaVersionCount {
            schema_version,
            article_count,
        })
        .collect::<Vec<_>>();

    let checksum_coverage = if total > 0 {
        checksum_count as f64 / total as f64
    } else {
        0.0
    };

    let mut report = Report {
        schema_version: "article-metadata-audit.v1".into(),
        total_articles: total,
        by_schema_version,
        all_safety_import_locked: all_locked,
        checksum_coverage,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "metadata_audit_complete",
        total_articles = report.total_articles,
        checksum_coverage = report.checksum_coverage,
        all_safety_import_locked = report.all_safety_import_locked,
        "Article metadata audit completed"
    );
    println!(
        "metadata-audit articles={} checksum_coverage={:.3} safety_locked={} D127=false report={}",
        report.total_articles,
        report.checksum_coverage,
        report.all_safety_import_locked,
        args.report.display()
    );
    Ok(())
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
    use serde_json::json;

    #[test]
    fn safety_check_catches_unlocked_article() {
        let locked = json!({
            "safety_flags": {"graph_import_allowed": false, "production_import_attempted": false},
            "safety_defaults": {"production_import_authorized": false},
            "acquisition": {"import_eligible": false}
        });
        assert!(check_safety_locked(&locked));
        let mut unlocked = locked.clone();
        unlocked["safety_flags"]["graph_import_allowed"] = json!(true);
        assert!(!check_safety_locked(&unlocked));
    }

    #[test]
    fn checksum_detection_works() {
        let with_sha = json!({"source_variants": [{"sha256": "abc123"}]});
        let without = json!({"source_variants": [{"sha256": ""}]});
        assert!(has_checksum(&with_sha));
        assert!(!has_checksum(&without));
    }
}
