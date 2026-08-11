//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Wires acquisition + real content pipeline (GROBID ingest, optional extract, snapshot).

use clap::Args;
use da_adapters::{ArxivSourceAcquisition, FdApiEmbedder, GrobidParser, RuleBasedExtractor};
use da_application::{
    ArticleLoadUseCase, ExtractionUseCase, IngestUseCase, LoadContentOutcome, LoadContentPipeline,
    LoadContentRequest, LoadExtractOutcome, LoadIngestOutcome,
};
use da_domain::{AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadStageSet};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::ParserPort;
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Default **ingest** materializes content. Use ingest+extract for entities.
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

    /// Export .sgsnap after content stages
    #[arg(long)]
    pub output: Option<PathBuf>,
}

struct CliContentPipeline;

#[async_trait::async_trait]
impl LoadContentPipeline for CliContentPipeline {
    async fn run_content(&self, req: LoadContentRequest<'_>) -> Result<LoadContentOutcome, String> {
        if !req.primary_path.is_file() {
            return Err(format!(
                "primary PDF missing: {}",
                req.primary_path.display()
            ));
        }
        if std::env::var("FD_API_KEY")
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err("FD_API_KEY is not set (required for ingest embeddings)".into());
        }

        let parser = GrobidParser::from_env();
        if !parser.is_alive().await {
            return Err("GROBID is not reachable (start sidecar on :8070)".into());
        }

        let store = Arc::new(open_graph_store_for_load());
        let embedder = Box::new(FdApiEmbedder::from_env());
        let mut ingest = IngestUseCase::from_shared(
            Box::new(parser),
            embedder,
            store.clone() as Arc<dyn DirectGraphStore>,
        );
        if req.enable_import {
            ingest = ingest.with_import_eligibility(da_domain::ImportEligibility::Eligible);
        }

        let pdf_path = req.primary_path.display().to_string();
        let result = ingest
            .ingest_pdf(&pdf_path, req.paper_id)
            .await
            .map_err(|e| format!("ingest: {e:#}"))?;

        let mut extract_out = None;
        if req.include_extract {
            // Same process-local store; re-parse for section text (GROBID may cache locally).
            let parser2 = GrobidParser::from_env();
            let parsed = parser2
                .parse_pdf(&pdf_path, req.paper_id)
                .await
                .map_err(|e| format!("parse for extract: {e:#}"))?;
            let extractor = Box::new(RuleBasedExtractor::new());
            let extract_uc = ExtractionUseCase::from_shared(
                extractor,
                store.clone() as Arc<dyn DirectGraphStore>,
            );
            let er = extract_uc
                .extract_from_parsed(&parsed)
                .await
                .map_err(|e| format!("extract: {e:#}"))?;
            extract_out = Some(LoadExtractOutcome {
                entities_extracted: er.entities_extracted,
                entities_created: er.entities_created,
                mentions_edges: er.mentions_edges,
                found_in_edges: er.found_in_edges,
            });
        }

        let snapshot_path = if let Some(out) = req.snapshot_output {
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let bytes = GraphStore::export_snapshot(store.as_ref())
                .await
                .map_err(|e| format!("export snapshot: {e}"))?;
            std::fs::write(out, &bytes)
                .map_err(|e| format!("write snapshot {}: {e}", out.display()))?;
            Some(out.display().to_string())
        } else {
            None
        };

        Ok(LoadContentOutcome {
            ingest: LoadIngestOutcome {
                paper_id: result.paper_id,
                title: result.title,
                body_chars: result.body_chars,
                section_count: result.section_count,
                citation_count: result.citation_count,
                snapshot_path,
                import_eligible: result.import_eligible,
            },
            extract: extract_out,
        })
    }
}

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
        let pipe = CliContentPipeline;
        uc.run_with_content(&args.locator, &policy, Some(&pipe), snap_ref)
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
        "ingest+extract" | "ensure+ingest+extract" | "full" => {
            Ok(LoadStageSet::EnsureIngestExtract)
        }
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
    fn parse_stages_shapes() {
        assert_eq!(parse_stages("ingest").unwrap(), LoadStageSet::EnsureIngest);
        assert_eq!(
            parse_stages("ingest+extract").unwrap(),
            LoadStageSet::EnsureIngestExtract
        );
        assert_eq!(
            parse_stages("full").unwrap(),
            LoadStageSet::EnsureIngestExtract
        );
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
