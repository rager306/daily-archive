//! Multi-provider scholarly evidence reconcile canary (M371/S11).
//!
//! Reads durable OpenAlex / Semantic Scholar / TeX analysis / sources artifacts
//! for one article, runs pure domain reconcile, emits a candidate-only report
//! with visible conflicts. No graph writes. D127 stays false.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::scholarly_evidence::ScholarlyEvidenceUseCase;
use da_domain::ContentDigest;
use da_domain::scholarly_evidence::{
    LocalBibObservation, LocalTexObservation, ProviderObservation, ScholarlyEvidenceInput,
    ScholarlyReconcileVerdict,
};
use serde_json::Value;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "2606.06036")]
    article_id: String,
    #[arg(
        long,
        default_value = "artifacts/article_context/article_2606.06036_sources.json"
    )]
    sources: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/article_2606.06036_tex_analysis.json"
    )]
    tex: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/article_2606.06036_openalex.json"
    )]
    openalex: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/scholarly_evidence_2606.06036.json"
    )]
    report: PathBuf,
}

pub fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("scholarly_evidence_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let sources_path = resolve(&repo_root, &args.sources);
    let tex_path = resolve(&repo_root, &args.tex);
    let openalex_path = resolve(&repo_root, &args.openalex);
    let report_path = resolve(&repo_root, &args.report);

    let sources: Value = read_json(&sources_path)?;
    if sources["schema_version"].as_str() != Some("article-source-evidence.v1") {
        anyhow::bail!("sources schema must be article-source-evidence.v1");
    }
    if sources["import_eligible"].as_bool() != Some(false) {
        anyhow::bail!("sources import_eligible must be false");
    }

    let mut providers = Vec::new();
    if let Some(candidates) = sources["candidate_metadata"].as_array() {
        for candidate in candidates {
            let Some(provider) = candidate["provider"].as_str() else {
                continue;
            };
            let referenced = candidate["referenced_works_count"].as_u64().or_else(|| {
                candidate["observed_via_public_api_before_rate_limit"]["reference_count"].as_u64()
            });
            let cited = candidate["cited_by_count"].as_u64().or_else(|| {
                candidate["observed_via_public_api_before_rate_limit"]["citation_count"].as_u64()
            });
            let authors = candidate["authors"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let topics = candidate["topics"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            providers.push(ProviderObservation {
                provider: provider.into(),
                status: candidate["status"].as_str().unwrap_or("unknown").into(),
                payload_sha256: candidate["sha256"]
                    .as_str()
                    .map(|s| ContentDigest::try_from(s.to_string()))
                    .transpose()
                    .with_context(|| format!("invalid {provider} sha256"))?,
                work_id: candidate["work_id"]
                    .as_str()
                    .or_else(|| candidate["paper_id"].as_str())
                    .map(str::to_string),
                doi: candidate["doi"].as_str().map(str::to_string),
                title: candidate["title"].as_str().map(str::to_string),
                authors,
                cited_by_count: cited,
                referenced_works_count: referenced,
                topics,
            });
        }
    }

    // Prefer full OpenAlex payload when present for DOI/title completeness.
    if openalex_path.is_file() {
        let oa: Value = read_json(&openalex_path)?;
        let oa_sha = ContentDigest::of_bytes(&std::fs::read(&openalex_path)?);
        if let Some(provider) = providers.iter_mut().find(|p| p.provider == "openalex") {
            provider.payload_sha256 = Some(oa_sha);
            provider.status = "fetched".into();
            provider.work_id = oa["id"].as_str().map(str::to_string);
            provider.doi = oa["doi"].as_str().map(str::to_string);
            provider.title = oa["title"]
                .as_str()
                .or_else(|| oa["display_name"].as_str())
                .map(str::to_string);
            provider.cited_by_count = oa["cited_by_count"].as_u64();
            provider.referenced_works_count = oa["referenced_works"]
                .as_array()
                .map(|a| a.len() as u64)
                .or(provider.referenced_works_count);
            if let Some(authorships) = oa["authorships"].as_array() {
                provider.authors = authorships
                    .iter()
                    .filter_map(|a| a["author"]["display_name"].as_str().map(str::to_string))
                    .collect();
            }
        }
    }

    let tex = if tex_path.is_file() {
        let tex_val: Value = read_json(&tex_path)?;
        if tex_val["schema_version"].as_str() != Some("tex-source-analysis.v1") {
            anyhow::bail!("tex analysis schema must be tex-source-analysis.v1");
        }
        if tex_val["import_eligible"].as_bool() != Some(false) {
            anyhow::bail!("tex analysis import_eligible must be false");
        }
        let bibliography = tex_val["bibliography_candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let candidate_id = entry["candidate_id"].as_str()?;
                let entry_key = entry["entry_key"].as_str()?;
                Some(LocalBibObservation {
                    candidate_id: ContentDigest::try_from(candidate_id.to_string()).ok()?,
                    entry_key: entry_key.into(),
                    entry_type: entry["entry_type"].as_str().unwrap_or("unknown").into(),
                    title: entry["title"].as_str().map(str::to_string),
                    doi: entry["doi"].as_str().map(str::to_string),
                    eprint: entry["eprint"].as_str().map(str::to_string),
                    source_member: entry["source_member"].as_str().unwrap_or("").into(),
                })
            })
            .collect::<Vec<_>>();
        Some(LocalTexObservation {
            analysis_sha256: ContentDigest::try_from(
                tex_val["analysis_sha256"]
                    .as_str()
                    .ok_or_else(|| anyhow!("tex analysis_sha256 missing"))?
                    .to_string(),
            )?,
            archive_sha256: ContentDigest::try_from(
                tex_val["archive_sha256"]
                    .as_str()
                    .ok_or_else(|| anyhow!("tex archive_sha256 missing"))?
                    .to_string(),
            )?,
            root_document: tex_val["root_document"].as_str().map(str::to_string),
            status: tex_val["status"].as_str().unwrap_or("unknown").into(),
            local_reference_denominator: tex_val["local_reference_denominator"]
                .as_u64()
                .map(|n| n as usize)
                .or(Some(bibliography.len())),
            bibliography,
            diagnostics: tex_val["diagnostics"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
    } else {
        None
    };

    let input = ScholarlyEvidenceInput {
        article_id: args.article_id.clone(),
        title: sources["title"].as_str().map(str::to_string),
        providers,
        tex,
        import_eligible: false,
    };
    let report = ScholarlyEvidenceUseCase::reconcile(input)
        .map_err(|e| anyhow!("scholarly reconcile failed: {e}"))?;

    let json = serde_json::to_vec_pretty(&report)?;
    atomic_write(&report_path, &json)?;

    println!(
        "scholarly_evidence_canary article={} verdict={:?} findings={} local_bib={} unmatched_local={} import_eligible=false report={}",
        report.article_id,
        report.verdict,
        report.findings.len(),
        report
            .tex
            .as_ref()
            .map(|t| t.bibliography.len())
            .unwrap_or(0),
        report.unmatched_local_bibliography,
        display_path(&repo_root, &report_path),
    );
    for finding in report.findings.iter().take(12) {
        eprintln!(
            "  {:?}:{}:{}",
            finding.severity, finding.class, finding.subject
        );
    }

    // Product gate: report sealed, D127 false, conflicts visible (not silent PASS).
    if report.import_eligible {
        std::process::exit(1);
    }
    if matches!(report.verdict, ScholarlyReconcileVerdict::Reject) {
        // Reject is valid product outcome for hard conflicts; still durable.
        eprintln!("verdict=Reject (hard conflicts recorded)");
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn resolve(repo_root: &Path, relative: &Path) -> PathBuf {
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        repo_root.join(relative)
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
    }
    Ok(())
}
