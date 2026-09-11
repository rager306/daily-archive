//! Real PDF-to-candidate vertical receipt (M502/S08/T01).
//!
//! The critical path is deliberately small and honest: two tracked PDFs are
//! parsed by the production GROBID parser, then their structure, visual
//! callouts, bibliography entries, citation anchors, and revision identity are
//! recorded as candidate-only receipts. This example never imports a graph or
//! promotes evidence; D127 remains false.
//!
//! Usage:
//!   cargo run -p da-cli --example vertical_scientific_path -- --help
//!   cargo run -p da-cli --example vertical_scientific_path

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use da_ports::parser::ParserPort;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "data/sources/arxiv/article_catalog/arxiv/cs-ai/2608.20845/source/2608.20845.pdf"
    )]
    positive_pdf: PathBuf,
    #[arg(
        long,
        default_value = "data/sources/arxiv/article_catalog/arxiv/math-ho/1512.06808/source/1512.06808.pdf"
    )]
    negative_pdf: PathBuf,
    #[arg(long, default_value = "2608.20845")]
    positive_id: String,
    #[arg(long, default_value = "1512.06808")]
    negative_id: String,
    #[arg(long, default_value = "b999999")]
    negative_ref_id: String,
    #[arg(
        long,
        default_value = "artifacts/runtime/vertical_scientific_path_m502.json"
    )]
    report: PathBuf,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: &'static str,
    stages: Vec<&'static str>,
    positive: CaseReceipt,
    negative: CaseReceipt,
    policy: PolicyReceipt,
    report_sha256: String,
}

#[derive(Debug, Serialize)]
struct CaseReceipt {
    case: &'static str,
    paper_id: String,
    source_pdf: String,
    pdf_sha256: String,
    revision_vid: String,
    sections: usize,
    section_anchors: usize,
    bibliography_entries: usize,
    citation_anchors: usize,
    visual_callouts: usize,
    candidate_receipts: Vec<CandidateReceipt>,
    negative_ref_id: Option<String>,
    negative_ref_absent: Option<bool>,
    verdict: &'static str,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    candidate_id: String,
    revision_vid: String,
    section_path_id: Option<String>,
    bibliography_ref_id: String,
    anchor_count: usize,
    source_digest: String,
    candidate_only: bool,
}

#[derive(Debug, Serialize)]
struct PolicyReceipt {
    import_eligible: bool,
    graph_imports: usize,
    graph_writes: usize,
    synthetic_inputs: usize,
    production_parser: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("vertical_scientific_path");
    let args = Args::parse();
    let positive = run_case("positive", &args.positive_pdf, &args.positive_id, None).await?;
    ensure!(
        positive.sections > 0
            && positive.bibliography_entries > 0
            && positive.citation_anchors > 0
            && !positive.candidate_receipts.is_empty(),
        "positive case lacks a complete structure → callout → bibliography → anchored candidate path"
    );

    let negative = run_case(
        "negative",
        &args.negative_pdf,
        &args.negative_id,
        Some(&args.negative_ref_id),
    )
    .await?;
    ensure!(
        negative.negative_ref_absent == Some(true) && negative.candidate_receipts.is_empty(),
        "negative case unexpectedly resolved the forbidden reference target"
    );

    let mut report = Report {
        schema_version: "vertical-scientific-path.v1",
        stages: vec![
            "pdf",
            "grobid_structure",
            "section_anchors",
            "visual_callouts",
            "bibliography",
            "revision_bound_candidate",
        ],
        positive,
        negative,
        policy: PolicyReceipt {
            import_eligible: false,
            graph_imports: 0,
            graph_writes: 0,
            synthetic_inputs: 0,
            production_parser: "da_cli::parse_profile::corpus_grobid_parser",
        },
        report_sha256: String::new(),
    };
    report.report_sha256 = digest_report(&report)?;
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "vertical-scientific-path positive_candidates={} negative_candidates=0 import_eligible=false report={}",
        report.positive.candidate_receipts.len(),
        args.report.display()
    );
    Ok(())
}

async fn run_case(
    case: &'static str,
    pdf: &Path,
    paper_id: &str,
    negative_ref_id: Option<&str>,
) -> Result<CaseReceipt> {
    ensure!(
        pdf.is_file(),
        "{} PDF does not exist: {}",
        case,
        pdf.display()
    );
    let bytes = std::fs::read(pdf).with_context(|| format!("read {} PDF", case))?;
    ensure!(bytes.starts_with(b"%PDF-"), "{} input is not a PDF", case);
    let pdf_sha256 = hex::encode(Sha256::digest(&bytes));
    let parser = da_cli::parse_profile::corpus_grobid_parser();
    ensure!(
        parser.is_alive().await,
        "GROBID is unavailable for {} case",
        case
    );
    let pdf_path = pdf.to_str().context("PDF path is not UTF-8")?;
    let parsed = parser
        .parse_pdf(pdf_path, paper_id)
        .await
        .with_context(|| format!("GROBID parse {} case", case))?;
    ensure!(
        parsed.pdf_hash.trim().eq_ignore_ascii_case(&pdf_sha256),
        "{} parser digest does not bind the input PDF",
        case
    );

    let revision_vid = da_domain::paper_revision_vid("arxiv", paper_id, 1)
        .map_err(|error| anyhow::anyhow!(error))?;
    let section_anchors = parsed
        .citations
        .iter()
        .flat_map(|entry| entry.anchors.iter())
        .filter(|anchor| anchor.section_path_id.is_some() && anchor.offset.is_some())
        .count();
    let citation_anchors = parsed
        .citations
        .iter()
        .map(|entry| entry.anchors.len())
        .sum();
    let candidate_receipts = if negative_ref_id.is_none() {
        parsed
            .citations
            .iter()
            .filter(|entry| !entry.anchors.is_empty())
            .map(|entry| candidate_receipt(entry, &revision_vid, &pdf_sha256))
            .collect()
    } else {
        Vec::new()
    };
    let negative_ref_absent = negative_ref_id.map(|wanted| {
        !parsed
            .citations
            .iter()
            .flat_map(|entry| entry.anchors.iter())
            .any(|anchor| anchor.ref_id == wanted)
    });
    if case == "negative" && negative_ref_absent != Some(true) {
        bail!("negative reference target was present in parsed bibliography anchors");
    }

    Ok(CaseReceipt {
        case,
        paper_id: paper_id.to_string(),
        source_pdf: pdf.display().to_string(),
        pdf_sha256,
        revision_vid,
        sections: parsed.sections.len(),
        section_anchors,
        bibliography_entries: parsed.citations.len(),
        citation_anchors,
        visual_callouts: parsed.visual_callouts.len(),
        candidate_receipts,
        negative_ref_id: negative_ref_id.map(str::to_string),
        negative_ref_absent,
        verdict: if case == "positive" {
            "candidate"
        } else {
            "rejected"
        },
    })
}

fn candidate_receipt(
    entry: &da_ports::parser::CitationEntry,
    revision_vid: &str,
    pdf_sha256: &str,
) -> CandidateReceipt {
    let ref_id = entry
        .anchors
        .first()
        .map(|anchor| anchor.ref_id.clone())
        .unwrap_or_else(|| "unresolved".into());
    let source = format!("{revision_vid}|{ref_id}|{pdf_sha256}");
    CandidateReceipt {
        candidate_id: format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
        revision_vid: revision_vid.to_string(),
        section_path_id: entry
            .anchors
            .first()
            .and_then(|anchor| anchor.section_path_id.clone()),
        bibliography_ref_id: ref_id,
        anchor_count: entry.anchors.len(),
        source_digest: format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
        candidate_only: true,
    }
}

fn digest_report(report: &Report) -> Result<String> {
    let mut value = serde_json::to_value(report)?;
    value["report_sha256"] = serde_json::Value::String(String::new());
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&value)?)
    ))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
