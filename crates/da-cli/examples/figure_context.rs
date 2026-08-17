//! Compose real GROBID parsing with deterministic figure context assembly.

use anyhow::{Context, Result, bail};
use clap::Parser;
use da_adapters::GrobidParser;
use da_application::{FigureContextAssembler, FigureSectionLocator};
use da_domain::{ArticleSummaryContext, DetectedVisualArtifact};
use da_ports::parser::{ParsedArticle, ParserPort};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    pdf: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    article_json: PathBuf,
    #[arg(long)]
    article_id: String,
    #[arg(long)]
    artifact_id: String,
    #[arg(long)]
    section_path: Option<String>,
    #[arg(long)]
    list_references: bool,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct FigureManifest {
    artifacts: Vec<DetectedVisualArtifact>,
}

#[derive(Debug, Deserialize)]
struct ArticleMetadata {
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("figure_context");
    let args = Args::parse();
    let manifest: FigureManifest = serde_json::from_slice(
        &std::fs::read(&args.manifest)
            .with_context(|| format!("read figure manifest {}", args.manifest.display()))?,
    )?;
    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == args.artifact_id)
        .with_context(|| format!("artifact {} not found", args.artifact_id))?;
    let parser = GrobidParser::from_env();
    if !parser.is_alive().await {
        bail!("GROBID is unavailable; cannot assemble real section context");
    }
    let pdf_path = args
        .pdf
        .to_str()
        .with_context(|| format!("PDF path is not UTF-8: {}", args.pdf.display()))?;
    let article = parser
        .parse_pdf(pdf_path, &args.article_id)
        .await
        .context("parse canonical PDF with GROBID")?;
    validate_navigation_identity(&article)?;

    if args.list_references {
        print_reference_candidates(&article, &artifact.artifact_id);
        return Ok(());
    }
    let section_path = args.section_path.as_deref().context(
        "--section-path is required; run with --list-references and choose an explicit path",
    )?;
    let article_metadata: ArticleMetadata = serde_json::from_slice(
        &std::fs::read(&args.article_json)
            .with_context(|| format!("read article metadata {}", args.article_json.display()))?,
    )?;
    let article_summary = article_metadata
        .abstract_text
        .filter(|text| !text.trim().is_empty())
        .map(|text| ArticleSummaryContext {
            source_id: format!("{}#abstract", args.article_json.display()),
            source_text_sha256: sha256(text.as_bytes()),
            text,
            truncated: false,
        })
        .or_else(|| {
            (!article.abstract_text.trim().is_empty()).then(|| ArticleSummaryContext {
                source_id: format!("grobid:{}#abstract", args.article_id),
                source_text_sha256: sha256(article.abstract_text.as_bytes()),
                text: article.abstract_text.clone(),
                truncated: false,
            })
        });
    let context = FigureContextAssembler::with_default_budget().assemble(
        artifact,
        &article,
        Some(&FigureSectionLocator::PathId(section_path.into())),
        article_summary.as_ref(),
        None,
    )?;
    let output = args
        .output
        .context("--output is required when assembling context")?;
    let bytes = serde_json::to_vec_pretty(&context)?;
    write_atomic(&output, &bytes)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "artifact_id": context.artifact_id,
            "section_path": context.containing_section.path_id,
            "reference_count": context.reference_excerpts.len(),
            "context_chars": context.total_context_chars,
            "truncated": context.truncated,
            "output": output,
            "output_sha256": sha256(&bytes),
            "import_eligible": false
        }))?
    );
    Ok(())
}

fn validate_navigation_identity(article: &ParsedArticle) -> Result<()> {
    let mut paths = HashSet::new();
    let mut section_ids = HashSet::new();
    for section in &article.sections {
        let path = section
            .path_id
            .as_deref()
            .context("parsed section lacks path_id")?;
        let section_id = section
            .section_id
            .as_deref()
            .context("parsed section lacks section_id")?;
        if !paths.insert(path) {
            bail!("duplicate parsed section path_id: {path}");
        }
        if !section_ids.insert(section_id) {
            bail!("duplicate parsed section_id: {section_id}");
        }
    }
    Ok(())
}

fn print_reference_candidates(article: &ParsedArticle, artifact_id: &str) {
    let number = artifact_id.trim_start_matches("Figure");
    let forms = [format!("figure {number}"), format!("fig. {number}")];
    for section in &article.sections {
        let lower = section.text.to_ascii_lowercase();
        if forms.iter().any(|form| lower.contains(form)) {
            println!(
                "path={} section_id={} title={}",
                section.path_id.as_deref().unwrap_or("<missing>"),
                section.section_id.as_deref().unwrap_or("<missing>"),
                section.title
            );
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("output has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
