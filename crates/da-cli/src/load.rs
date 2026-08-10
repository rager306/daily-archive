//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Wires acquisition adapters + optional real ingest (GROBID + embed + graph).

use clap::Args;
use da_adapters::{ArxivSourceAcquisition, FdApiEmbedder, GrobidParser};
use da_application::{
    ArticleLoadUseCase, IngestUseCase, LoadContentIngest, LoadIngestOutcome, batch_ingest_pdfs,
};
use da_domain::{AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadStageSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Locator: arXiv id/URL, catalog:… pin, or local path
    pub locator: String,

    /// Provider hint (auto uses arxiv for arXiv locators)
    #[arg(long, default_value = "auto")]
    pub provider: String,

    /// Catalog root for ensure (typically data/article_catalog)
    #[arg(long, default_value = "data/article_catalog")]
    pub catalog_root: PathBuf,

    /// Authorize network fetch when PDF missing (adapter process-boundary)
    #[arg(long, default_value_t = false)]
    pub allow_network: bool,

    /// Include TeX source archive when fetching
    #[arg(long, default_value_t = false)]
    pub include_tex_source: bool,

    /// Stage set: ensure | ingest | ingest+extract | full
    /// Default **ingest** so one command materializes content (not ensure-only theater).
    #[arg(long, default_value = "ingest")]
    pub stages: String,

    /// Dry-run: resolve only, no catalog/network/graph writes
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// D127 unlock (default locked)
    #[arg(long, default_value_t = false)]
    pub enable_import: bool,

    /// Write LoadReport JSON
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Export .sgsnap after ingest (default under artifacts/article_context/)
    #[arg(long)]
    pub output: Option<PathBuf>,
}

struct CliIngest {
    enable_import_default: bool,
}

#[async_trait::async_trait]
impl LoadContentIngest for CliIngest {
    async fn ingest_primary(
        &self,
        paper_id: &str,
        primary_path: &Path,
        enable_import: bool,
        snapshot_output: Option<&Path>,
    ) -> Result<LoadIngestOutcome, String> {
        if !primary_path.is_file() {
            return Err(format!("primary PDF missing: {}", primary_path.display()));
        }
        if std::env::var("FD_API_KEY")
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err("FD_API_KEY is not set (required for ingest embeddings)".into());
        }

        let parser = Box::new(GrobidParser::from_env());
        if !parser.is_alive().await {
            return Err("GROBID is not reachable (start sidecar on :8070)".into());
        }
        let embedder = Box::new(FdApiEmbedder::from_env());
        let graph_store = Box::new(open_graph_store_for_load());
        let mut ingest = IngestUseCase::new(parser, embedder, graph_store);
        if enable_import || self.enable_import_default {
            ingest.eligibility = da_domain::ImportEligibility::Eligible;
        }

        let pdfs = vec![(primary_path.display().to_string(), paper_id.to_string())];
        let result = batch_ingest_pdfs(&ingest, &pdfs, snapshot_output)
            .await
            .map_err(|e| format!("{e:#}"))?;
        if result.ok == 0 {
            let detail = result
                .errors
                .first()
                .map(|(_, e)| e.clone())
                .unwrap_or_else(|| "ingest failed".into());
            return Err(detail);
        }
        Ok(LoadIngestOutcome {
            paper_id: paper_id.into(),
            title: paper_id.into(),
            body_chars: result.total_body_chars,
            section_count: result.total_sections,
            citation_count: result.total_citations,
            snapshot_path: result.snapshot_path,
            import_eligible: result.import_eligible,
        })
    }
}

/// Graph open for load composition (mirrors main open_graph_store).
pub fn open_graph_store_for_load() -> da_adapters::SamyamaGraphStore {
    match da_adapters::SamyamaGraphStore::try_from_env() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("❌ graph store open failed: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_load(args: &LoadArgs) -> Result<(), String> {
    let stages = parse_stages(&args.stages)?;
    let policy = AcquisitionPolicy {
        allow_network: args.allow_network,
        include_html: true,
        include_tex_source: args.include_tex_source,
        dry_run: args.dry_run,
        enable_import: args.enable_import,
        stages,
        run_mode: da_domain::LoadRunMode::Inline,
    };

    let provider = resolve_provider(&args.provider, &args.locator)?;
    if provider != "arxiv" {
        return Err(format!(
            "provider {provider:?} not wired (arxiv only); use auto|arxiv"
        ));
    }

    let acq = ArxivSourceAcquisition::new(&args.catalog_root).with_repo_root(".");
    let uc = ArticleLoadUseCase::new(&acq).with_catalog_root(&args.catalog_root);

    let default_snap = default_snapshot_path(&args.locator);
    let snap = args.output.clone().or(default_snap);
    let snap_ref = snap.as_deref();

    let report = if policy.stages.includes_ingest() && !policy.dry_run {
        let ing = CliIngest {
            enable_import_default: args.enable_import,
        };
        uc.run_with_ingest(&args.locator, &policy, Some(&ing), snap_ref)
            .await
    } else {
        uc.run(&args.locator, &policy)
    };

    println!(
        "load schema={} ok={} import_eligible={} provider={} source_code={}",
        report.schema_version,
        report.ok,
        report.import_eligible,
        report.provider,
        report.source_code
    );
    if let Some(uk) = &report.unit_key {
        println!("  unit_key={uk}");
    }
    if let Some(sp) = &report.snapshot_path {
        println!("  snapshot={sp}");
    }
    for s in &report.stages {
        println!(
            "  stage {:?}: {}{}",
            s.stage,
            s.status,
            s.detail
                .as_ref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default()
        );
    }
    for e in &report.errors {
        eprintln!("  error: {e}");
    }

    if let Some(path) = &args.report {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("serialize LoadReport: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("write report {}: {e}", path.display()))?;
        println!("  report: {}", path.display());
    }

    assert_eq!(report.schema_version, LOAD_REPORT_SCHEMA_VERSION);
    if !report.ok {
        return Err("load failed (see report/errors)".into());
    }
    Ok(())
}

fn default_snapshot_path(locator: &str) -> Option<PathBuf> {
    let id = da_domain::try_normalize_arxiv_id(locator).or_else(|| {
        da_domain::ArticleLocator::parse(locator)
            .ok()
            .map(|l| l.canonical_key().to_string())
    })?;
    Some(PathBuf::from(format!(
        "artifacts/article_context/paper_{id}.sgsnap"
    )))
}

fn parse_stages(s: &str) -> Result<LoadStageSet, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "ensure" => Ok(LoadStageSet::Ensure),
        "ingest" | "ensure+ingest" | "ensure_ingest" => Ok(LoadStageSet::EnsureIngest),
        "ingest+extract" | "ensure+ingest+extract" => Ok(LoadStageSet::EnsureIngestExtract),
        "full" => Ok(LoadStageSet::Full),
        other => Err(format!(
            "unknown --stages {other:?}; use ensure|ingest|ingest+extract|full"
        )),
    }
}

fn resolve_provider(hint: &str, locator: &str) -> Result<String, String> {
    let h = hint.trim().to_ascii_lowercase();
    if h != "auto" && h != "arxiv" {
        return Ok(h);
    }
    let loc = da_domain::ArticleLocator::parse(locator).map_err(|e| e.to_string())?;
    match loc {
        da_domain::ArticleLocator::Arxiv { .. } => Ok("arxiv".into()),
        da_domain::ArticleLocator::Catalog { source_code, .. } if source_code == "arxiv" => {
            Ok("arxiv".into())
        }
        da_domain::ArticleLocator::Catalog { source_code, .. } => Ok(source_code),
        _ if h == "arxiv" => Ok("arxiv".into()),
        _ => Err("auto provider could not select adapter for locator (arxiv only)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stages_ingest_default_shape() {
        assert_eq!(parse_stages("ingest").unwrap(), LoadStageSet::EnsureIngest);
        assert_eq!(parse_stages("ensure").unwrap(), LoadStageSet::Ensure);
    }

    #[test]
    fn resolve_provider_auto_arxiv() {
        assert_eq!(
            resolve_provider("auto", "https://arxiv.org/html/2608.07067v1").unwrap(),
            "arxiv"
        );
    }

    #[test]
    fn default_snapshot_from_html_url() {
        let p = default_snapshot_path("https://arxiv.org/html/2608.07067v1").unwrap();
        assert!(p.ends_with("paper_2608.07067.sgsnap"));
    }
}
