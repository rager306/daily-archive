//! Profile real cache cold/hit/drift/corruption paths and bind graph replay proof.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::{memory_pipeline_cache::MemoryPipelineCache, pipeline_cache::FilePipelineCache};
use da_domain::assurance::ContentDigest;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, StageManifestEntry, StageOutcome};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};
#[derive(Parser)]
struct Args {
    #[arg(
        long,
        env = "DA_PIPELINE_CACHE_ROOT",
        default_value = "artifacts/gold_corpus/pipeline_cache"
    )]
    cache_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.json"
    )]
    materialization_report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/pipeline_cache_profile_m387.json"
    )]
    report: PathBuf,
}
#[derive(Serialize)]
struct ScaleProfile {
    articles: usize,
    manifests: usize,
    cold_misses: usize,
    cold_us: u64,
    hits: usize,
    hit_us: u64,
    drift_misses: usize,
    drift_us: u64,
    key_sha256: Vec<String>,
    payload_sha256: Vec<String>,
}
#[derive(Serialize)]
struct ReplayProof {
    source_report_sha256: ContentDigest,
    article_count: u64,
    applied: u64,
    rejected: u64,
    unobserved: u64,
    unchanged: u64,
    valid_partition: bool,
}
#[derive(Serialize)]
struct Report {
    schema_version: String,
    cache_root: String,
    scales: Vec<ScaleProfile>,
    corruption_visible: bool,
    replay: ReplayProof,
    optimizer_admission_verdict: Option<String>,
    import_eligible: bool,
    report_sha256: String,
}
fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("pipeline_cache_profile");
    let a = Args::parse();
    let file = FilePipelineCache::new(&a.cache_root);
    let ids = article_ids(&a.cache_root)?;
    let mut scales = vec![];
    for n in [1, 10, 30] {
        let selected = ids.iter().take(n).collect::<Vec<_>>();
        let manifests = selected
            .iter()
            .map(|id| file.manifest(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let memory = MemoryPipelineCache::new();
        let t = Instant::now();
        let mut cold = 0;
        for m in &manifests {
            cold += usize::from(matches!(
                memory.lookup(&m.key, "2026-08-12T00:00:00Z")?,
                CacheLookup::Miss
            ));
        }
        let cold_us = t.elapsed().as_micros() as u64;
        let t = Instant::now();
        let mut hits = 0;
        for m in &manifests {
            hits += usize::from(matches!(
                file.lookup(&m.key, "2026-08-12T00:00:00Z")?,
                CacheLookup::Hit(_)
            ));
        }
        let hit_us = t.elapsed().as_micros() as u64;
        let t = Instant::now();
        let mut drift = 0;
        for m in &manifests {
            let mut key = m.key.clone();
            key.configuration_fingerprint.push_str(":profile-drift");
            drift += usize::from(matches!(
                file.lookup(&key, "2026-08-12T00:00:00Z")?,
                CacheLookup::Miss
            ));
        }
        let drift_us = t.elapsed().as_micros() as u64;
        scales.push(ScaleProfile {
            articles: selected.len(),
            manifests: manifests.len(),
            cold_misses: cold,
            cold_us,
            hits,
            hit_us,
            drift_misses: drift,
            drift_us,
            key_sha256: manifests.iter().map(|x| x.key_sha256.clone()).collect(),
            payload_sha256: manifests.iter().filter_map(payload_sha).collect(),
        });
    }
    let corruption_visible = corruption_probe(&file, &ids)?;
    let replay = replay_proof(&a.materialization_report)?;
    if !replay.valid_partition {
        return Err(anyhow!("materialization replay partition invalid"));
    }
    let mut r = Report {
        schema_version: "pipeline-cache-profile.v1".into(),
        cache_root: a.cache_root.display().to_string(),
        scales,
        corruption_visible,
        replay,
        optimizer_admission_verdict: None,
        import_eligible: false,
        report_sha256: String::new(),
    };
    r.report_sha256 = digest(&r)?;
    if let Some(parent) = a.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&a.report, serde_json::to_vec_pretty(&r)?)?;
    tracing::info!(event="pipeline_cache_profile_complete",scales=3,corruption_visible=r.corruption_visible,replay_unchanged=r.replay.unchanged,report=%a.report.display(),import_eligible=false,"Pipeline cache profile completed");
    println!(
        "cache-profile scales=1/10/30 corruption_visible={} replay_unchanged={} D127=false report={}",
        r.corruption_visible,
        r.replay.unchanged,
        a.report.display()
    );
    Ok(())
}
fn article_ids(root: &Path) -> Result<Vec<String>> {
    let mut ids = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect::<Vec<_>>();
    ids.sort();
    Ok(ids)
}
fn payload_sha(m: &StageManifestEntry) -> Option<String> {
    match &m.outcome {
        StageOutcome::Success { payload_sha256, .. } => Some(payload_sha256.clone()),
        _ => None,
    }
}
fn corruption_probe(source: &FilePipelineCache, ids: &[String]) -> Result<bool> {
    let id = ids.first().ok_or_else(|| anyhow!("cache empty"))?;
    let m = source
        .manifest(id)?
        .into_iter()
        .find(|x| matches!(x.outcome, StageOutcome::Success { .. }))
        .ok_or_else(|| anyhow!("no success manifest"))?;
    let root =
        std::env::temp_dir().join(format!("da-cache-profile-corrupt-{}", std::process::id()));
    let cache = FilePipelineCache::new(&root);
    cache.store_success(&m.key, b"original", BTreeMap::new(), "2026-08-12T00:00:00Z")?;
    let payload = root
        .join(&m.key.source.article_id)
        .join(&m.key.stage_id)
        .join(m.key.key_sha256()?)
        .join("payload.bin");
    std::fs::write(payload, b"corrupt")?;
    let visible = cache.lookup(&m.key, "2026-08-12T00:00:01Z").is_err();
    std::fs::remove_dir_all(root)?;
    Ok(visible)
}
fn replay_proof(path: &Path) -> Result<ReplayProof> {
    let bytes = std::fs::read(path)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let r = &v["materialization"]["aggregate"]["replay"];
    let sum = |field: &str| {
        ["node", "property", "edge", "vector"]
            .iter()
            .map(|k| r[*k][field].as_u64().unwrap_or(0))
            .sum::<u64>()
    };
    let applied = sum("applied");
    let rejected = sum("rejected");
    let unchanged = sum("unchanged");
    let unobserved = r["unobserved_operations"].as_u64().unwrap_or(u64::MAX);
    let article_count = v["materialization"]["article_count"].as_u64().unwrap_or(0);
    Ok(ReplayProof {
        source_report_sha256: ContentDigest::of_bytes(&bytes),
        article_count,
        applied,
        rejected,
        unobserved,
        unchanged,
        valid_partition: article_count > 0
            && applied == 0
            && rejected == 0
            && unobserved == 0
            && unchanged > 0,
    })
}
fn digest<T: Serialize>(v: &T) -> Result<String> {
    let mut x = serde_json::to_value(v)?;
    x["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&x)?)
        .as_str()
        .into())
}
