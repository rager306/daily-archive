//! Seal controlled materialization run outputs into comparable scaling evidence.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_domain::materialization_profile::{
    MaterializationProfileCase, MaterializationScalingProfile,
};
use serde::Deserialize;

#[derive(Parser)]
struct Args {
    /// NAME:PAPERS:MATERIALIZATION_REPORT:RESOURCE_JSON:IDENTITY_JSON (repeat in ascending size).
    #[arg(long = "case")]
    cases: Vec<String>,
    #[arg(long)]
    source_revision: String,
    #[arg(long)]
    binary_sha256: String,
    #[arg(long)]
    cpu_count: u32,
    #[arg(long)]
    cpu_model: String,
    #[arg(long)]
    memory_total_kb: u64,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Deserialize)]
struct Resource {
    wall_seconds: f64,
    user_seconds: f64,
    system_seconds: f64,
    cpu_percent: String,
    max_rss_kb: u64,
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("materialization_profile");
    let args = Args::parse();
    let cases = args
        .cases
        .iter()
        .map(|spec| parse_case(spec))
        .collect::<Result<Vec<_>>>()?;
    let report = MaterializationScalingProfile::seal(
        args.source_revision,
        args.binary_sha256,
        args.cpu_count,
        args.cpu_model,
        args.memory_total_kb,
        cases,
    )
    .map_err(|error| anyhow!(error))?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, bytes)
        .with_context(|| format!("write {}", args.output.display()))?;
    println!(
        "materialization-profile cases={} exponent={:.3} projected_200_seconds={:.1} report={} import_eligible=false",
        report.cases.len(),
        report.scaling_exponent,
        report.projected_200_seconds,
        args.output.display()
    );
    Ok(())
}

fn parse_case(spec: &str) -> Result<MaterializationProfileCase> {
    let fields = spec.splitn(5, ':').collect::<Vec<_>>();
    anyhow::ensure!(fields.len() == 5, "invalid --case {spec:?}");
    let name = fields[0].to_string();
    let papers: u64 = fields[1].parse()?;
    let run: serde_json::Value = serde_json::from_slice(&std::fs::read(fields[2])?)?;
    let resource: Resource = serde_json::from_slice(&std::fs::read(fields[3])?)?;
    let identity: serde_json::Value = serde_json::from_slice(&std::fs::read(fields[4])?)?;
    let graph = run
        .pointer("/materialization/graph_after")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("{name}: missing graph_after"))?;
    let aggregate = run
        .pointer("/materialization/aggregate")
        .ok_or_else(|| anyhow!("{name}: missing aggregate"))?;
    let mutations_attempted = ["first", "replay"]
        .iter()
        .flat_map(|phase| {
            ["node", "property", "edge", "vector"]
                .iter()
                .map(move |family| (phase, family))
        })
        .map(|(phase, family)| {
            aggregate
                .pointer(&format!("/{phase}/{family}/attempted"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        })
        .sum::<u64>();
    let semantic_sha256 = identity
        .pointer("/snapshot/semantic_sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("{name}: missing semantic identity"))?
        .to_string();
    let cpu_percent = resource.cpu_percent.trim_end_matches('%').parse()?;
    Ok(MaterializationProfileCase {
        name,
        papers,
        nodes: graph[0].as_u64().ok_or_else(|| anyhow!("invalid nodes"))?,
        edges: graph[1].as_u64().ok_or_else(|| anyhow!("invalid edges"))?,
        mutations_attempted,
        wall_seconds: resource.wall_seconds,
        user_seconds: resource.user_seconds,
        system_seconds: resource.system_seconds,
        cpu_percent,
        max_rss_kb: resource.max_rss_kb,
        seconds_per_1000_mutations: resource.wall_seconds / mutations_attempted as f64 * 1000.0,
        semantic_sha256,
    })
}
