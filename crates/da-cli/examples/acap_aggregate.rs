//! Project canonical corpus ACAP aggregate from an existing multi-wave
//! materialization artifact (M371/S07/T03).
//!
//! No graph writes, no D127 unlock. Reads policy YAML + materialization JSON,
//! upgrades legacy per-article layers from paper-level `replay_mutations`, and
//! emits `artifacts/article_context/assurance_report.json`.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::corpus_assurance::{
    CorpusArticleContribution, CorpusAssuranceInput, CorpusAssuranceUseCase, CorpusPolicyLayer,
};
use da_domain::{
    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ArticleAssuranceReport, ContentDigest,
    MutationFamilyObservation, ReplayObservation,
};
use serde::Deserialize;

const MAX_POLICY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "scripts/governor/checks/article_context_assurance.yaml"
    )]
    policy: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.json"
    )]
    materialization: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/assurance_report.json"
    )]
    report: PathBuf,
}

#[derive(Deserialize)]
struct PolicyYaml {
    #[serde(default)]
    layers: Vec<LayerYaml>,
}

#[derive(Deserialize, Default)]
struct LayerYaml {
    id: String,
    #[serde(default)]
    enforcement: String,
}

#[derive(Deserialize)]
struct MaterializationRoot {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    manifest_sha256: String,
    materialization: MaterializationBody,
}

#[derive(Deserialize)]
struct MaterializationBody {
    #[serde(default)]
    waves: Vec<WaveBody>,
}

#[derive(Deserialize)]
struct WaveBody {
    #[serde(default)]
    papers: Vec<PaperBody>,
}

#[derive(Deserialize)]
struct PaperBody {
    #[allow(dead_code)]
    paper_id: String,
    assurance: ArticleAssuranceReport,
    #[serde(default)]
    replay_mutations: Option<MutationCountersJson>,
}

#[derive(Deserialize, Clone)]
struct MutationCountersJson {
    #[serde(default)]
    node: FamilyJson,
    #[serde(default)]
    property: FamilyJson,
    #[serde(default)]
    edge: FamilyJson,
    #[serde(default)]
    vector: FamilyJson,
    #[serde(default)]
    unobserved_operations: u64,
}

#[derive(Deserialize, Clone, Default)]
struct FamilyJson {
    #[serde(default)]
    attempted: u64,
    #[serde(default)]
    applied: u64,
    #[serde(default)]
    unchanged: u64,
    #[serde(default)]
    rejected: u64,
}

impl MutationCountersJson {
    fn to_replay_observation(&self) -> ReplayObservation {
        ReplayObservation {
            node: family(&self.node),
            property: family(&self.property),
            edge: family(&self.edge),
            vector: family(&self.vector),
            unobserved_operations: self.unobserved_operations,
        }
    }
}

fn family(f: &FamilyJson) -> MutationFamilyObservation {
    MutationFamilyObservation {
        attempted: f.attempted,
        applied: f.applied,
        unchanged: f.unchanged,
        rejected: f.rejected,
    }
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("acap_aggregate");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let policy_path = resolve(&repo_root, &args.policy);
    let mat_path = resolve(&repo_root, &args.materialization);
    let report_path = resolve(&repo_root, &args.report);

    let policy_bytes = read_capped(&policy_path, MAX_POLICY_BYTES)?;
    let policy_sha = ContentDigest::of_bytes(&policy_bytes);
    let policy: PolicyYaml = serde_yaml::from_slice(&policy_bytes)
        .with_context(|| format!("parse {}", policy_path.display()))?;
    let policy_layers = policy
        .layers
        .into_iter()
        .map(|layer| CorpusPolicyLayer {
            id: layer.id,
            enforcement: layer.enforcement,
        })
        .collect::<Vec<_>>();
    if policy_layers.is_empty() {
        return Err(anyhow!("policy declares no layers"));
    }

    let mat_bytes = read_capped(&mat_path, MAX_ARTIFACT_BYTES)?;
    let source_sha = ContentDigest::of_bytes(&mat_bytes);
    let root: MaterializationRoot = serde_json::from_slice(&mat_bytes)
        .with_context(|| format!("parse {}", mat_path.display()))?;

    let mut articles = Vec::new();
    let mut upgraded = 0_u64;
    for wave in &root.materialization.waves {
        for paper in &wave.papers {
            let needs_upgrade = paper
                .assurance
                .layer_results
                .iter()
                .any(|layer| layer.replay_observation.is_none());
            let replay_upgrade = if needs_upgrade {
                paper.replay_mutations.as_ref().map(|m| {
                    upgraded += 1;
                    m.to_replay_observation()
                })
            } else {
                None
            };
            articles.push(CorpusArticleContribution {
                report: paper.assurance.clone(),
                replay_upgrade,
            });
        }
    }

    let pipeline_revision = if !root.manifest_sha256.is_empty() {
        format!("manifest:{}", root.manifest_sha256)
    } else if !root.schema_version.is_empty() {
        format!("schema:{}", root.schema_version)
    } else {
        "materialization_artifact".into()
    };

    let input = CorpusAssuranceInput {
        policy_sha256: policy_sha,
        pipeline_revision,
        source_artifact_sha256: source_sha,
        policy_layers,
        articles,
        import_eligible: false,
    };
    let report = CorpusAssuranceUseCase::aggregate(input)
        .map_err(|error| anyhow!("aggregate failed: {error}"))?;

    let json = serde_json::to_vec_pretty(&report)?;
    atomic_write(&report_path, &json)?;

    println!(
        "acap_aggregate schema={} articles={} verdict={:?} hard_failures={} required_absent={} upgraded_replay={} import_eligible=false report={}",
        ACAP_AGGREGATE_REPORT_SCHEMA_VERSION,
        report.article_count,
        report.verdict,
        report.aggregate.hard_failures.len(),
        report
            .aggregate
            .hard_failures
            .iter()
            .filter(|code| code.starts_with("required_layer_absent:"))
            .count(),
        upgraded,
        display_path(&repo_root, &report_path),
    );
    for code in report.aggregate.hard_failures.iter().take(20) {
        eprintln!("  hard_failure: {code}");
    }
    Ok(())
}

fn resolve(repo_root: &Path, relative: &Path) -> PathBuf {
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        repo_root.join(relative)
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn read_capped(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let metadata = file.metadata()?;
    anyhow::ensure!(metadata.is_file(), "not a regular file: {}", path.display());
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        anyhow::bail!("file exceeds byte budget: {}", path.display());
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::File::open(parent).and_then(|dir| dir.sync_all());
    }
    Ok(())
}
