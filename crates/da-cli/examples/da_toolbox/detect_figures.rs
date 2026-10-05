//! Production composition root for pdffigures2 figure detection (M371 S02).
//!
//! Retains content-addressed assets and a canonical manifest under the article
//! catalog. Detector output remains candidate evidence; this command never
//! writes graph truth or changes D127 import eligibility.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::PdfFigures2Adapter;
use da_domain::VisualArtifactKind;
use da_ports::{FigureDetectionPort, FigureDetectionRequest};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    pdf: PathBuf,
    #[arg(long)]
    article_dir: PathBuf,
    #[arg(long)]
    article_id: String,
    #[arg(long, env = "PDFFIGURES2_JAR")]
    jar: PathBuf,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("detect_figures");
    let args = Args::parse();
    let pdf = std::fs::read(&args.pdf)
        .with_context(|| format!("read canonical PDF {}", args.pdf.display()))?;
    let source_pdf_sha256 = hex::encode(Sha256::digest(&pdf));
    let adapter = PdfFigures2Adapter::new(&args.jar);
    let result = adapter
        .detect(FigureDetectionRequest {
            article_id: args.article_id.clone(),
            source_pdf_path: args.pdf,
            source_pdf_sha256,
            canonical_article_dir: args.article_dir,
            revision_vid: None,
        })
        .await
        .with_context(|| format!("detect figures for {}", args.article_id))?;
    let figure_count = result
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == VisualArtifactKind::Figure)
        .count();
    let table_count = result
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == VisualArtifactKind::Table)
        .count();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "article_id": args.article_id,
            "detector_fingerprint": result.detector_fingerprint,
            "artifact_count": result.artifacts.len(),
            "figure_count": figure_count,
            "table_count": table_count,
            "metadata_path": result.metadata_path,
            "detector_metadata_sha256": result.detector_metadata_sha256,
            "manifest_sha256": result.manifest_sha256,
            "duration_ms": result.duration_ms,
            "import_eligible": false
        }))?
    );
    Ok(())
}
