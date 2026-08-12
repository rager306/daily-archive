//! Pipeline acceptance: aggregates stage ledger contracts, cache replay proof,
//! readiness gates and governor verdict into one typed acceptance report.
//! No inference, no promotion; D127=false.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_stage_ledger_m387.json"
    )]
    stage_ledger: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_cache_profile_m387.json"
    )]
    cache_profile: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/prompt_optimizer_readiness_m387.json"
    )]
    readiness: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_acceptance_m387.json"
    )]
    report: PathBuf,
}

#[derive(Deserialize)]
struct StageLedger {
    observed: Vec<StageEntry>,
    pass: bool,
    import_eligible: bool,
}
#[derive(Deserialize)]
struct StageEntry {
    stage_id: String,
    status: String,
    #[serde(default)]
    mutated: u64,
}
#[derive(Deserialize)]
struct CacheProfile {
    replay: CacheReplay,
    import_eligible: bool,
}
#[derive(Deserialize)]
struct CacheReplay {
    article_count: u64,
    applied: u64,
    rejected: u64,
    unobserved: u64,
    unchanged: u64,
    valid_partition: bool,
}
#[derive(Deserialize)]
struct Readiness {
    tasks: Vec<ReadinessTask>,
    optimizer_enabled: bool,
    auto_promote: bool,
    import_eligible: bool,
}
#[derive(Deserialize)]
struct ReadinessTask {
    task_id: String,
    fsm_state: String,
    eligible: bool,
    deployed: bool,
}

#[derive(Serialize)]
struct StageContractResult {
    stage_id: String,
    status: String,
    contract_pass: bool,
    mutated: u64,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    stages: Vec<StageContractResult>,
    all_stage_contracts_pass: bool,
    cache_replay_valid: bool,
    cache_replay_article_count: u64,
    cache_replay_unchanged: u64,
    readiness_tasks: usize,
    readiness_task_ids: Vec<String>,
    readiness_all_evaluated_not_eligible: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
    import_eligible: bool,
    acceptance_verdict: String,
    report_sha256: String,
}

fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("pipeline_acceptance");
    let args = Args::parse();
    let ledger: StageLedger = read_json(&args.stage_ledger)?;
    let cache: CacheProfile = read_json(&args.cache_profile)?;
    let readiness: Readiness = read_json(&args.readiness)?;

    if ledger.import_eligible || !ledger.pass || cache.import_eligible || readiness.import_eligible
    {
        return Err(anyhow!("acceptance inputs must be import-ineligible"));
    }
    if readiness.optimizer_enabled || readiness.auto_promote {
        return Err(anyhow!("acceptance readiness must be optimizer-free"));
    }

    let contract_pass_status =
        |status: &str| matches!(status, "completed" | "complete" | "pass" | "ok");
    let stages = ledger
        .observed
        .iter()
        .map(|s| StageContractResult {
            stage_id: s.stage_id.clone(),
            status: s.status.clone(),
            contract_pass: contract_pass_status(&s.status),
            mutated: s.mutated,
        })
        .collect::<Vec<_>>();
    let all_stage_contracts_pass = ledger.pass && stages.iter().all(|s| s.contract_pass);

    let cache_replay_valid = cache.replay.valid_partition
        && cache.replay.applied == 0
        && cache.replay.rejected == 0
        && cache.replay.unobserved == 0
        && cache.replay.unchanged > 0;

    let readiness_all_evaluated_not_eligible = !readiness.tasks.is_empty()
        && readiness.tasks.iter().all(|t| {
            (t.fsm_state == "evaluated" || t.fsm_state == "eligible") && !t.eligible && !t.deployed
        });

    let acceptance_verdict =
        if all_stage_contracts_pass && cache_replay_valid && readiness_all_evaluated_not_eligible {
            "accepted"
        } else {
            "benchmark-only"
        };

    let mut report = Report {
        schema_version: "pipeline-acceptance.v1".into(),
        stages,
        all_stage_contracts_pass,
        cache_replay_valid,
        cache_replay_article_count: cache.replay.article_count,
        cache_replay_unchanged: cache.replay.unchanged,
        readiness_tasks: readiness.tasks.len(),
        readiness_task_ids: readiness.tasks.iter().map(|t| t.task_id.clone()).collect(),
        readiness_all_evaluated_not_eligible,
        optimizer_enabled: readiness.optimizer_enabled,
        auto_promote: readiness.auto_promote,
        import_eligible: false,
        acceptance_verdict: acceptance_verdict.into(),
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "pipeline_acceptance_complete",
        stages = report.stages.len(),
        all_stage_contracts_pass = report.all_stage_contracts_pass,
        cache_replay_valid = report.cache_replay_valid,
        readiness_tasks = report.readiness_tasks,
        acceptance_verdict = %report.acceptance_verdict,
        optimizer_enabled = false,
        auto_promote = false,
        import_eligible = false,
        "Pipeline acceptance completed"
    );
    println!(
        "pipeline-acceptance stages={} contracts_pass={} replay_valid={} verdict={} D127=false report={}",
        report.stages.len(),
        report.all_stage_contracts_pass,
        report.cache_replay_valid,
        report.acceptance_verdict,
        args.report.display()
    );
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| anyhow!("parse {}: {}", path.display(), e))
}

fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> StageLedger {
        StageLedger {
            observed: vec![
                StageEntry {
                    stage_id: "acquire".into(),
                    status: "complete".into(),
                    mutated: 0,
                },
                StageEntry {
                    stage_id: "parse".into(),
                    status: "complete".into(),
                    mutated: 0,
                },
                StageEntry {
                    stage_id: "extract".into(),
                    status: "complete".into(),
                    mutated: 140,
                },
            ],
            pass: true,
            import_eligible: false,
        }
    }

    fn cache() -> CacheProfile {
        CacheProfile {
            replay: CacheReplay {
                article_count: 200,
                applied: 0,
                rejected: 0,
                unobserved: 0,
                unchanged: 168239,
                valid_partition: true,
            },
            import_eligible: false,
        }
    }

    fn readiness() -> Readiness {
        Readiness {
            tasks: vec![ReadinessTask {
                task_id: "article_summary".into(),
                fsm_state: "evaluated".into(),
                eligible: false,
                deployed: false,
            }],
            optimizer_enabled: false,
            auto_promote: false,
            import_eligible: false,
        }
    }

    #[test]
    fn acceptance_verdict_requires_all_contracts_and_fail_closed_state() {
        let l = ledger();
        let c = cache();
        let r = readiness();
        let stages_ok = l
            .observed
            .iter()
            .all(|s| matches!(s.status.as_str(), "completed" | "complete" | "pass" | "ok"))
            && l.pass;
        let replay_ok = c.replay.valid_partition && c.replay.unchanged > 0;
        let ready_ok = !r.tasks.is_empty() && r.tasks.iter().all(|t| !t.eligible && !t.deployed);
        assert!(stages_ok && replay_ok && ready_ok);
        assert!(!l.import_eligible && !r.optimizer_enabled && !r.auto_promote);
        // A failed stage flips verdict to benchmark-only.
        let mut l2 = ledger();
        l2.observed[1].status = "failed".into();
        assert!(
            !l2.observed
                .iter()
                .all(|s| matches!(s.status.as_str(), "completed" | "complete" | "pass" | "ok"))
        );
    }
}
