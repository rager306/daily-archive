//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Wires `ArticleLoadUseCase` + provider acquisition adapters. Does **not** own
//! acquisition logic (that lives in ports/adapters). Draft `load_arxiv.rs` is
//! not the product architecture.

use clap::Args;
use da_adapters::ArxivSourceAcquisition;
use da_application::ArticleLoadUseCase;
use da_domain::{AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadStageSet};
use da_ports::SourceAcquisitionPort;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Locator: arXiv id/URL, catalog:… pin, or local path
    pub locator: String,

    /// Provider hint (auto uses arxiv for arXiv locators in this wave)
    #[arg(long, default_value = "auto")]
    pub provider: String,

    /// Catalog root for offline ensure (article_catalog parent or tree root)
    #[arg(long, default_value = "data/article_catalog")]
    pub catalog_root: PathBuf,

    /// Authorize network (fail-closed default). Fetch not implemented in W4/W5.
    #[arg(long, default_value_t = false)]
    pub allow_network: bool,

    /// Stage set: ensure | ingest | ingest+extract | full
    #[arg(long, default_value = "ensure")]
    pub stages: String,

    /// Dry-run: resolve only, no ensure side effects
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// D127 unlock (default locked)
    #[arg(long, default_value_t = false)]
    pub enable_import: bool,

    /// Write LoadReport JSON
    #[arg(long)]
    pub report: Option<PathBuf>,
}

pub fn run_load(args: &LoadArgs) -> Result<(), String> {
    let stages = parse_stages(&args.stages)?;
    let mut policy = AcquisitionPolicy {
        allow_network: args.allow_network,
        dry_run: args.dry_run,
        enable_import: args.enable_import,
        stages,
        ..AcquisitionPolicy::default()
    };
    if matches!(stages, LoadStageSet::Ensure) {
        policy = AcquisitionPolicy {
            stages: LoadStageSet::Ensure,
            allow_network: args.allow_network,
            dry_run: args.dry_run,
            enable_import: args.enable_import,
            ..AcquisitionPolicy::default()
        };
    }

    let provider = resolve_provider(&args.provider, &args.locator)?;
    let report = match provider.as_str() {
        "arxiv" => {
            let acq = ArxivSourceAcquisition::new(&args.catalog_root);
            if !acq.can_resolve(
                &da_domain::ArticleLocator::parse(&args.locator).map_err(|e| e.to_string())?,
            ) {
                // Still run use case for honest report
            }
            let uc = ArticleLoadUseCase::new(&acq);
            uc.run(&args.locator, &policy)
        }
        other => {
            return Err(format!(
                "provider {other:?} not wired in W5 (arxiv only); use auto|arxiv"
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
        _ => Err("auto provider could not select adapter for locator (W5: arxiv only)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_stages_ensure() {
        assert_eq!(parse_stages("ensure").unwrap(), LoadStageSet::Ensure);
    }

    #[test]
    fn run_load_catalog_hit_temp_tree() {
        let root = std::env::temp_dir().join(format!("da-cli-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root
            .join("article_catalog")
            .join("arxiv")
            .join("cs-ai")
            .join("2602.04837")
            .join("source");
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("2602.04837.pdf");
        let mut f = std::fs::File::create(&pdf).unwrap();
        f.write_all(b"%PDF-1.4 cli-load").unwrap();

        let report_path = root.join("load-report.json");
        let args = LoadArgs {
            locator: "2602.04837".into(),
            provider: "arxiv".into(),
            catalog_root: root.clone(),
            allow_network: false,
            stages: "ensure".into(),
            dry_run: false,
            enable_import: false,
            report: Some(report_path.clone()),
        };
        run_load(&args).expect("catalog_hit load");
        let text = std::fs::read_to_string(&report_path).unwrap();
        assert!(text.contains(LOAD_REPORT_SCHEMA_VERSION));
        assert!(text.contains("\"import_eligible\": false"));
        assert!(text.contains("catalog_hit"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_provider_auto_arxiv() {
        assert_eq!(
            resolve_provider("auto", "https://arxiv.org/abs/2602.04837").unwrap(),
            "arxiv"
        );
    }
}
