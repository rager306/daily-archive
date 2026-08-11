//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Single locator or `--batch-toc` for catalog HTML ensure of entire TOC.

use clap::Args;
use da_adapters::{
    ArxivSourceAcquisition, CatalogHtmlAcquisition, FdApiEmbedder, GrobidParser, HtmlParser,
    RuleBasedExtractor, toc_unit_keys,
};
use da_application::{
    ArticleLoadUseCase, ExtractionUseCase, IngestUseCase, LoadContentOutcome, LoadContentPipeline,
    LoadContentRequest, LoadExtractOutcome, LoadIngestOutcome,
};
use da_domain::{AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadStageSet, PrimaryContentKind};
use da_ports::SourceAcquisitionPort;
use da_ports::embedder::Embedder;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{ParsedArticle, ParserPort};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Locator: arXiv id/URL, catalog:… pin, gnn-ch-* unit, or local path.
    /// Optional when `--batch-toc` is set.
    #[arg(required_unless_present = "batch_toc")]
    pub locator: Option<String>,

    /// Provider: auto | arxiv | gnn_textbook | textbook | catalog
    #[arg(long, default_value = "auto")]
    pub provider: String,

    /// Catalog root for ensure (typically data/article_catalog)
    #[arg(long, default_value = "data/article_catalog")]
    pub catalog_root: PathBuf,

    /// Authorize network fetch when assets missing
    #[arg(long, default_value_t = false)]
    pub allow_network: bool,

    /// Include TeX source archive when fetching arXiv
    #[arg(long, default_value_t = false)]
    pub include_tex_source: bool,

    /// Stage set: ensure | ingest | ingest+extract | full
    #[arg(long, default_value = "ingest")]
    pub stages: String,

    /// Dry-run: resolve only
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// D127 unlock (default locked)
    #[arg(long, default_value_t = false)]
    pub enable_import: bool,

    /// Write LoadReport JSON (single locator)
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Export .sgsnap after content stages (single locator)
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Batch ensure all units from a source-catalog TOC YAML
    #[arg(long)]
    pub batch_toc: Option<PathBuf>,

    /// Directory for per-unit batch load reports
    #[arg(long, default_value = "artifacts/loads/batch")]
    pub batch_report_dir: PathBuf,
}

struct CliContentPipeline;

#[async_trait::async_trait]
impl LoadContentPipeline for CliContentPipeline {
    async fn run_content(&self, req: LoadContentRequest<'_>) -> Result<LoadContentOutcome, String> {
        if !req.primary_path.is_file() {
            return Err(format!(
                "primary asset missing: {}",
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

        let primary_path = req.primary_path.display().to_string();
        let is_html = req
            .primary_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));

        let parsed: ParsedArticle = if is_html {
            let parser = HtmlParser::new();
            parser
                .parse_html(&primary_path, req.paper_id)
                .await
                .map_err(|e| format!("html parse: {e:#}"))?
        } else {
            let parser = GrobidParser::from_env();
            if !parser.is_alive().await {
                return Err("GROBID is not reachable (start sidecar on :8070)".into());
            }
            parser
                .parse_pdf(&primary_path, req.paper_id)
                .await
                .map_err(|e| format!("pdf parse: {e:#}"))?
        };

        let store = Arc::new(open_graph_store_for_load());
        let embedder = FdApiEmbedder::from_env();
        let embed_text = if parsed.abstract_text.is_empty() {
            parsed.title.as_str()
        } else {
            parsed.abstract_text.as_str()
        };
        let vector = embedder
            .embed(embed_text)
            .await
            .map_err(|e| format!("embed: {e:#}"))?;

        let materializer =
            IngestUseCase::materializer_only(store.clone() as Arc<dyn DirectGraphStore>);
        let materializer = if req.enable_import {
            materializer.with_import_eligibility(da_domain::ImportEligibility::Eligible)
        } else {
            materializer
        };

        let source_code = infer_source_code(&primary_path);
        let result = materializer
            .materialize_parsed(&parsed, vector, None, source_code, None)
            .await
            .map_err(|e| format!("materialize: {e:#}"))?;

        let mut extract_out = None;
        if req.include_extract {
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

fn infer_source_code(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    for code in [
        "gnn_textbook",
        "textbook",
        "stanford",
        "openalex",
        "company_blog",
        "nature",
        "arxiv",
    ] {
        if lower.contains(code) {
            return Some(code.to_string());
        }
    }
    None
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
    if let Some(toc) = &args.batch_toc {
        return run_batch_toc(args, toc).await;
    }
    let locator = args
        .locator
        .as_deref()
        .ok_or_else(|| "locator required unless --batch-toc".to_string())?;
    run_single(args, locator).await
}

async fn run_batch_toc(args: &LoadArgs, toc: &Path) -> Result<(), String> {
    let keys = toc_unit_keys(toc).map_err(|e| format!("TOC: {e}"))?;
    if keys.is_empty() {
        return Err(format!("TOC {} has no units", toc.display()));
    }
    // Batch defaults to ensure unless user overrode stages explicitly via non-default?
    // Prefer ensure for catalog completeness (product debt close).
    let stages_str = if args.stages == "ingest" {
        "ensure".to_string()
    } else {
        args.stages.clone()
    };
    let stages = parse_stages(&stages_str)?;
    let policy = AcquisitionPolicy {
        allow_network: args.allow_network,
        include_html: true,
        include_tex_source: false,
        dry_run: args.dry_run,
        enable_import: false,
        stages,
        run_mode: da_domain::LoadRunMode::Inline,
    };

    let provider = if args.provider == "auto" || args.provider == "catalog" {
        // Infer from TOC filename / first unit
        "gnn_textbook".to_string()
    } else {
        args.provider.clone()
    };

    let acq = CatalogHtmlAcquisition::new(&args.catalog_root, &provider)
        .with_repo_root(".")
        .with_toc_path(toc);
    let pipe = CliContentPipeline;
    let _ = std::fs::create_dir_all(&args.batch_report_dir);

    let mut ok_n = 0usize;
    let mut fail_n = 0usize;
    let mut hit_n = 0usize;
    let mut fetched_n = 0usize;

    println!(
        "batch-toc units={} provider={} stages={:?} allow_network={}",
        keys.len(),
        provider,
        policy.stages,
        policy.allow_network
    );

    for (i, unit_key) in keys.iter().enumerate() {
        let locator = format!("catalog:{provider}/html/{unit_key}");
        let report = run_with_acq(&acq, &args.catalog_root, &locator, &policy, &pipe, None).await;
        let status = if report.ok { "ok" } else { "FAIL" };
        if report.ok {
            ok_n += 1;
        } else {
            fail_n += 1;
        }
        if report.stages.iter().any(|s| s.status == "catalog_hit") {
            hit_n += 1;
        }
        if report.stages.iter().any(|s| s.status == "fetched") {
            fetched_n += 1;
        }
        println!(
            "[{}/{}] {} {} primary={:?}",
            i + 1,
            keys.len(),
            status,
            unit_key,
            report.primary_kind
        );
        let rep_path = args.batch_report_dir.join(format!("{unit_key}.json"));
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(&rep_path, json);
        }
        if !report.ok {
            for e in &report.errors {
                eprintln!("    error: {e}");
            }
        }
    }

    println!(
        "batch done ok={ok_n} fail={fail_n} catalog_hit≈{hit_n} fetched≈{fetched_n} reports={}",
        args.batch_report_dir.display()
    );
    if fail_n > 0 {
        return Err(format!("batch-toc: {fail_n} unit(s) failed"));
    }
    Ok(())
}

async fn run_single(args: &LoadArgs, locator: &str) -> Result<(), String> {
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

    let provider = resolve_provider(&args.provider, locator)?;
    let snap = args
        .output
        .clone()
        .or_else(|| default_snapshot_path(locator));
    let snap_ref = snap.as_deref();
    let pipe = CliContentPipeline;

    let report = match provider.as_str() {
        "arxiv" => {
            let acq = ArxivSourceAcquisition::new(&args.catalog_root).with_repo_root(".");
            run_with_acq(&acq, &args.catalog_root, locator, &policy, &pipe, snap_ref).await
        }
        "gnn_textbook" | "textbook" | "stanford" | "company_blog" | "nature" => {
            let mut acq =
                CatalogHtmlAcquisition::new(&args.catalog_root, &provider).with_repo_root(".");
            if let Some(toc) = &args.batch_toc {
                acq = acq.with_toc_path(toc);
            }
            run_with_acq(&acq, &args.catalog_root, locator, &policy, &pipe, snap_ref).await
        }
        other => {
            return Err(format!(
                "provider {other:?} not wired; use auto|arxiv|gnn_textbook|textbook|stanford"
            ));
        }
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
    if let Some(pk) = &report.primary_kind {
        println!("  primary_kind={pk:?}");
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
    let _ = PrimaryContentKind::Pdf;
    if !report.ok {
        return Err("load failed (see report/errors)".into());
    }
    Ok(())
}

async fn run_with_acq<A: SourceAcquisitionPort + ?Sized>(
    acq: &A,
    catalog_root: &Path,
    locator: &str,
    policy: &AcquisitionPolicy,
    pipe: &CliContentPipeline,
    snap: Option<&Path>,
) -> da_domain::LoadReport {
    let uc = ArticleLoadUseCase::new(acq).with_catalog_root(catalog_root);
    if policy.stages.includes_ingest() && !policy.dry_run {
        uc.run_with_content(locator, policy, Some(pipe), snap).await
    } else {
        uc.run(locator, policy)
    }
}

fn default_snapshot_path(locator: &str) -> Option<PathBuf> {
    let id = da_domain::try_normalize_arxiv_id(locator).or_else(|| {
        da_domain::ArticleLocator::parse(locator)
            .ok()
            .map(|l| l.canonical_key().to_string())
    })?;
    let safe = id.replace('/', "_");
    Some(PathBuf::from(format!(
        "artifacts/article_context/paper_{safe}.sgsnap"
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
    if h != "auto" && h != "catalog" {
        return Ok(h);
    }
    let loc = da_domain::ArticleLocator::parse(locator).map_err(|e| e.to_string())?;
    match loc {
        da_domain::ArticleLocator::Arxiv { .. } => Ok("arxiv".into()),
        da_domain::ArticleLocator::Catalog { source_code, .. } => Ok(source_code),
        da_domain::ArticleLocator::Local { path } => {
            if path.ends_with(".html") || path.ends_with(".htm") {
                Ok("gnn_textbook".into())
            } else {
                Ok("arxiv".into())
            }
        }
        da_domain::ArticleLocator::Raw { value } => {
            if value.starts_with("gnn-ch-") || value.contains("gnn_textbook") {
                Ok("gnn_textbook".into())
            } else if h == "arxiv" {
                Ok("arxiv".into())
            } else {
                Err(format!(
                    "auto provider could not select adapter for locator {value:?}"
                ))
            }
        }
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
    }

    #[test]
    fn resolve_provider_arxiv_and_textbook() {
        assert_eq!(
            resolve_provider("auto", "https://arxiv.org/html/2608.07067v1").unwrap(),
            "arxiv"
        );
        assert_eq!(
            resolve_provider(
                "auto",
                "catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs"
            )
            .unwrap(),
            "gnn_textbook"
        );
        assert_eq!(
            resolve_provider("auto", "gnn-ch-about").unwrap(),
            "gnn_textbook"
        );
    }

    #[test]
    fn default_snapshot_from_html_url() {
        let p = default_snapshot_path("https://arxiv.org/html/2608.07067v1").unwrap();
        assert!(p.ends_with("paper_2608.07067.sgsnap"));
    }
}
