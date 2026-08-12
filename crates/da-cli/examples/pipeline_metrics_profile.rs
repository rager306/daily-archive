//! Join current durable evidence into the complete 14-stage metrics envelope.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_application::pipeline_metrics::no_claim_pipeline_metrics;
use da_domain::{
    assurance::ContentDigest, context_conservation::StageContextSnapshot,
    pipeline_metrics::StageExecutionState,
};
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/ontology/pipeline_stage_contracts.yaml")]
    ledger: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/context_conservation_m387.json"
    )]
    context_report: PathBuf,
    #[arg(long, default_value = "artifacts/runtime/pipeline_metrics_m387.json")]
    report: PathBuf,
    #[arg(long, default_value = "m387-baseline-30")]
    run_id: String,
}
#[derive(Deserialize)]
struct ContextReport {
    stages: Vec<ContextRow>,
    import_eligible: bool,
}
#[derive(Deserialize)]
struct ContextRow {
    stage_id: String,
    aggregate: Option<StageContextSnapshot>,
}
fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("pipeline_metrics_profile");
    let a = Args::parse();
    let ledger_bytes = std::fs::read(&a.ledger)?;
    let ledger = load_pipeline_stage_ledger(&a.ledger).map_err(|e| anyhow!(e))?;
    let context_bytes = std::fs::read(&a.context_report)?;
    let context: ContextReport = serde_json::from_slice(&context_bytes)?;
    if context.import_eligible {
        return Err(anyhow!("context report cannot authorize import"));
    }
    let mut report = no_claim_pipeline_metrics(
        &ledger,
        &ledger_bytes,
        &a.run_id,
        "corpus:representative-30",
    )
    .map_err(|e| anyhow!(e))?;
    for row in context.stages {
        if let Some(snapshot) = row.aggregate {
            let metric = report
                .stages
                .iter_mut()
                .find(|x| x.stage_id == row.stage_id)
                .ok_or_else(|| anyhow!("unknown context stage {}", row.stage_id))?;
            metric.state = StageExecutionState::Complete;
            metric.input_sha256 = Some(ContentDigest::of_bytes(&context_bytes));
            metric.output_sha256 = Some(snapshot.artifact_sha256.clone());
            metric.context = Some(snapshot);
        }
    }
    let ids = ledger
        .stages
        .iter()
        .map(|x| x.id.clone())
        .collect::<Vec<_>>();
    report.validate(&ids).map_err(|e| anyhow!(e))?;
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = a.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&a.report, serde_json::to_vec_pretty(&report)?)?;
    let observed = report
        .stages
        .iter()
        .filter(|x| x.state != StageExecutionState::NoClaim)
        .count();
    tracing::info!(event="pipeline_metrics_profile_complete",run_id=%a.run_id,stage_rows=report.stages.len(),observed_stages=observed,report=%a.report.display(),import_eligible=false,"Whole-pipeline metrics profile completed");
    println!(
        "pipeline-metrics stages={} observed={} no_claim={} optimizer_admission=none D127=false report={}",
        report.stages.len(),
        observed,
        report.stages.len() - observed,
        a.report.display()
    );
    Ok(())
}
fn digest<T: serde::Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}
