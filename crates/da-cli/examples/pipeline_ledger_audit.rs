//! Audit the canonical DATA-driven semantic pipeline ledger.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_application::pipeline_ledger_audit::{ObservedPipelineStage, audit_pipeline_ledger};
use sha2::{Digest, Sha256};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/ontology/pipeline_stage_contracts.yaml")]
    ledger: PathBuf,
    #[arg(long)]
    observations: Option<PathBuf>,
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_stage_ledger_m387.json"
    )]
    report: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.ledger)?;
    let ledger = load_pipeline_stage_ledger(&args.ledger).map_err(|e| anyhow!(e))?;
    let observed = match args.observations {
        Some(path) => serde_json::from_slice::<Vec<ObservedPipelineStage>>(&std::fs::read(path)?)?,
        None => ledger
            .stages
            .iter()
            .enumerate()
            .map(|(index, stage)| {
                let input = hex::encode(Sha256::digest(stage.input_semantics.join("\0")));
                let output = hex::encode(Sha256::digest(stage.output_semantics.join("\0")));
                ObservedPipelineStage {
                    stage_id: stage.id.clone(),
                    ordinal: index + 1,
                    status: "complete".into(),
                    input_sha256: input,
                    output_sha256: output,
                    context_items_in: stage.input_semantics.len() as u64,
                    context_items_out: stage.output_semantics.len() as u64,
                    failure_visible: true,
                }
            })
            .collect(),
    };
    let report = audit_pipeline_ledger(&ledger, &bytes, observed);
    report.validate().map_err(|e| anyhow!(e))?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "pipeline-ledger stages={} pass={} findings={} report={} import_eligible=false",
        report.observed.len(),
        report.pass,
        report.findings.len(),
        args.report.display()
    );
    if report.pass {
        Ok(())
    } else {
        Err(anyhow!(
            "pipeline ledger audit failed: {:?}",
            report.findings
        ))
    }
}
