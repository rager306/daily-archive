//! Bind cache cold/warm ablation, materialization scaling and optimizer readiness
//! into one stage ablation report with per-scale deltas. No inference, no promotion.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_cache_profile_m387.json"
    )]
    cache_profile: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/performance/materialization_scaling_m384.json"
    )]
    materialization_profile: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/prompt_optimizer_readiness_m387.json"
    )]
    readiness: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/stage_ablation_profile_m387.json"
    )]
    report: PathBuf,
}

#[derive(Deserialize)]
struct CacheProfile {
    scales: Vec<CacheScale>,
    replay: CacheReplay,
    import_eligible: bool,
}
#[derive(Deserialize)]
struct CacheScale {
    articles: usize,
    manifests: usize,
    cold_us: u64,
    warm_us: u64,
    warm_hits: usize,
    hit_us: u64,
    drift_misses: usize,
    key_sha256: Vec<String>,
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
struct MaterializationProfile {
    scaling_exponent: f64,
    projected_200_seconds: f64,
    cases: Vec<MaterializationCase>,
}
#[derive(Clone, Deserialize, Serialize)]
struct MaterializationCase {
    name: String,
    papers: u64,
    nodes: u64,
    edges: u64,
    wall_seconds: f64,
    cpu_percent: f64,
    max_rss_kb: u64,
}

#[derive(Deserialize)]
struct ReadinessReport {
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
struct CacheAblationDelta {
    articles: usize,
    manifests: usize,
    cold_us: u64,
    warm_us: u64,
    warm_over_cold_us: i64,
    file_hit_us: u64,
    warm_hits: usize,
    drift_misses: usize,
    key_sha256_count: usize,
}

#[derive(Serialize)]
struct ReadinessDelta {
    task_id: String,
    fsm_state: String,
    eligible: bool,
    deployed: bool,
}

#[derive(Serialize)]
struct StageAblationReport {
    schema_version: String,
    cache_ablation: Vec<CacheAblationDelta>,
    cache_replay_valid: bool,
    cache_replay_article_count: u64,
    cache_replay_unchanged: u64,
    materialization_scaling_exponent: f64,
    materialization_projected_200_seconds: f64,
    materialization_cases: Vec<MaterializationCase>,
    readiness_deltas: Vec<ReadinessDelta>,
    optimizer_enabled: bool,
    auto_promote: bool,
    import_eligible: bool,
    report_sha256: String,
}

fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("stage_ablation_report");
    let args = Args::parse();
    let cache: CacheProfile = read_json(&args.cache_profile)?;
    let materialization: MaterializationProfile = read_json(&args.materialization_profile)?;
    let readiness: ReadinessReport = read_json(&args.readiness)?;

    if cache.import_eligible || materialization.cases.is_empty() {
        return Err(anyhow!(
            "stage ablation inputs must be import-ineligible and non-empty"
        ));
    }
    if !cache.replay.valid_partition
        || cache.replay.applied != 0
        || cache.replay.rejected != 0
        || cache.replay.unobserved != 0
        || cache.replay.unchanged == 0
    {
        return Err(anyhow!("stage ablation cache replay partition invalid"));
    }
    if readiness.import_eligible || readiness.optimizer_enabled || readiness.auto_promote {
        return Err(anyhow!(
            "stage ablation readiness must stay import-ineligible and optimizer-free"
        ));
    }

    let cache_ablation = cache
        .scales
        .iter()
        .map(|s| CacheAblationDelta {
            articles: s.articles,
            manifests: s.manifests,
            cold_us: s.cold_us,
            warm_us: s.warm_us,
            warm_over_cold_us: s.warm_us as i64 - s.cold_us as i64,
            file_hit_us: s.hit_us,
            warm_hits: s.warm_hits,
            drift_misses: s.drift_misses,
            key_sha256_count: s.key_sha256.len(),
        })
        .collect::<Vec<_>>();

    let readiness_deltas = readiness
        .tasks
        .iter()
        .map(|t| ReadinessDelta {
            task_id: t.task_id.clone(),
            fsm_state: t.fsm_state.clone(),
            eligible: t.eligible,
            deployed: t.deployed,
        })
        .collect::<Vec<_>>();

    let mut report = StageAblationReport {
        schema_version: "stage-ablation-profile.v1".into(),
        cache_ablation,
        cache_replay_valid: cache.replay.valid_partition,
        cache_replay_article_count: cache.replay.article_count,
        cache_replay_unchanged: cache.replay.unchanged,
        materialization_scaling_exponent: materialization.scaling_exponent,
        materialization_projected_200_seconds: materialization.projected_200_seconds,
        materialization_cases: materialization.cases,
        readiness_deltas,
        optimizer_enabled: readiness.optimizer_enabled,
        auto_promote: readiness.auto_promote,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "stage_ablation_report_complete",
        cache_scales = report.cache_ablation.len(),
        cache_replay_valid = report.cache_replay_valid,
        materialization_exponent = report.materialization_scaling_exponent,
        readiness_tasks = report.readiness_deltas.len(),
        optimizer_enabled = false,
        auto_promote = false,
        import_eligible = false,
        report = %args.report.display(),
        "Stage ablation report completed"
    );
    println!(
        "stage-ablation cache_scales={} replay_valid={} mat_exponent={:.3} readiness_tasks={} optimizer=false D127=false report={}",
        report.cache_ablation.len(),
        report.cache_replay_valid,
        report.materialization_scaling_exponent,
        report.readiness_deltas.len(),
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
    use serde_json::json;

    fn cache_profile() -> CacheProfile {
        CacheProfile {
            scales: vec![CacheScale {
                articles: 1,
                manifests: 3,
                cold_us: 159,
                warm_us: 129,
                warm_hits: 3,
                hit_us: 5958,
                drift_misses: 3,
                key_sha256: vec!["k1".into()],
            }],
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

    fn materialization_profile() -> MaterializationProfile {
        MaterializationProfile {
            scaling_exponent: 0.968,
            projected_200_seconds: 18.59,
            cases: vec![MaterializationCase {
                name: "one".into(),
                papers: 1,
                nodes: 51,
                edges: 68,
                wall_seconds: 0.11,
                cpu_percent: 93.0,
                max_rss_kb: 22728,
            }],
        }
    }

    fn readiness_report() -> ReadinessReport {
        ReadinessReport {
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
    fn stage_ablation_binds_deltas_and_stays_import_ineligible() {
        let cache = cache_profile();
        let mat = materialization_profile();
        let readiness = readiness_report();
        let ablation = cache
            .scales
            .iter()
            .map(|s| CacheAblationDelta {
                articles: s.articles,
                manifests: s.manifests,
                cold_us: s.cold_us,
                warm_us: s.warm_us,
                warm_over_cold_us: s.warm_us as i64 - s.cold_us as i64,
                file_hit_us: s.hit_us,
                warm_hits: s.warm_hits,
                drift_misses: s.drift_misses,
                key_sha256_count: s.key_sha256.len(),
            })
            .collect::<Vec<_>>();
        assert_eq!(ablation[0].warm_over_cold_us, -30);
        assert!(cache.replay.valid_partition);
        assert_eq!(readiness.tasks[0].fsm_state, "evaluated");
        assert!(!readiness.optimizer_enabled && !readiness.import_eligible);
        let report = json!({
            "schema_version": "stage-ablation-profile.v1",
            "import_eligible": false,
            "optimizer_enabled": readiness.optimizer_enabled,
            "auto_promote": readiness.auto_promote,
        });
        assert_eq!(report["import_eligible"], json!(false));
        assert_eq!(report["optimizer_enabled"], json!(false));
        let _ = mat;
    }
}
