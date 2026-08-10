//! Project required pre-materialization ACAP layers from an existing multi-wave
//! materialization artifact, then emit the corpus aggregate (post-S07 product wave).
//!
//! No graph writes, no D127 unlock. Reuses PipelineStageAssuranceUseCase +
//! CorpusAssuranceUseCase over durable evidence only.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::corpus_assurance::{
    CorpusArticleContribution, CorpusAssuranceInput, CorpusAssuranceUseCase, CorpusPolicyLayer,
};
use da_application::pipeline_stage_assurance::{
    PipelineStageAssuranceInput, PipelineStageAssuranceUseCase, StageArtifactLineage,
    StageGraphCounts, StageMaterializationReplay, digest_hex, replay_from_family_counts,
};
use da_domain::{ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ContentDigest};
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
    paper_id: String,
    source_sha256: String,
    artifacts: ArtifactsBody,
    first: FirstBody,
    #[serde(default)]
    replay_mutations: Option<MutationCountersJson>,
    #[serde(default)]
    import_eligible: Option<bool>,
}

#[derive(Deserialize)]
struct ArtifactsBody {
    parse: String,
    extract: String,
    embed: String,
}

#[derive(Deserialize, Default)]
struct FirstBody {
    #[serde(default)]
    sections: u64,
    #[serde(default)]
    references: u64,
    #[serde(default)]
    citations: u64,
    #[serde(default)]
    entities: u64,
    #[serde(default)]
    mentions: u64,
    #[serde(default)]
    found_in: u64,
    #[serde(default)]
    evidence_bundles: u64,
    #[serde(default)]
    claims: u64,
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
    fn to_replay(&self) -> da_domain::ReplayObservation {
        replay_from_family_counts(
            (
                self.node.attempted,
                self.node.applied,
                self.node.unchanged,
                self.node.rejected,
            ),
            (
                self.property.attempted,
                self.property.applied,
                self.property.unchanged,
                self.property.rejected,
            ),
            (
                self.edge.attempted,
                self.edge.applied,
                self.edge.unchanged,
                self.edge.rejected,
            ),
            (
                self.vector.attempted,
                self.vector.applied,
                self.vector.unchanged,
                self.vector.rejected,
            ),
            self.unobserved_operations,
        )
    }
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("acap_project_pipeline_layers");
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
    let mut pass_articles = 0_u64;
    let mut fail_articles = 0_u64;
    for wave in &root.materialization.waves {
        for paper in &wave.papers {
            let lineage = StageArtifactLineage {
                source_sha256: digest_hex(&paper.source_sha256)
                    .map_err(|e| anyhow!("{} source_sha256: {e}", paper.paper_id))?,
                parse_sha256: digest_hex(&paper.artifacts.parse)
                    .map_err(|e| anyhow!("{} parse: {e}", paper.paper_id))?,
                extract_sha256: digest_hex(&paper.artifacts.extract)
                    .map_err(|e| anyhow!("{} extract: {e}", paper.paper_id))?,
                embed_sha256: digest_hex(&paper.artifacts.embed)
                    .map_err(|e| anyhow!("{} embed: {e}", paper.paper_id))?,
            };
            let first = StageGraphCounts {
                sections: paper.first.sections,
                references: paper.first.references,
                citations: paper.first.citations,
                entities: paper.first.entities,
                mentions: paper.first.mentions,
                found_in: paper.first.found_in,
                evidence_bundles: paper.first.evidence_bundles,
                claims: paper.first.claims,
            };
            let materialization_replay =
                paper
                    .replay_mutations
                    .as_ref()
                    .map(|m| StageMaterializationReplay {
                        observation: m.to_replay(),
                    });
            let input = PipelineStageAssuranceInput {
                article_id: paper.paper_id.clone(),
                policy_sha256: policy_sha.clone(),
                lineage,
                first,
                materialization_replay,
                d127_unlock_count: 0,
                import_eligible: paper.import_eligible.unwrap_or(false),
            };
            let report = PipelineStageAssuranceUseCase::evaluate(input)
                .map_err(|e| anyhow!("{} stage assurance: {e}", paper.paper_id))?;
            match report.verdict {
                da_domain::AssuranceVerdict::Pass => pass_articles += 1,
                da_domain::AssuranceVerdict::Fail => fail_articles += 1,
                da_domain::AssuranceVerdict::NoClaim => {}
            }
            articles.push(CorpusArticleContribution {
                report,
                replay_upgrade: None, // already embedded in materialization layer
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

    let corpus = CorpusAssuranceUseCase::aggregate(CorpusAssuranceInput {
        policy_sha256: policy_sha,
        pipeline_revision,
        source_artifact_sha256: source_sha,
        policy_layers,
        articles,
        import_eligible: false,
    })
    .map_err(|e| anyhow!("corpus aggregate: {e}"))?;

    let json = serde_json::to_vec_pretty(&corpus)?;
    atomic_write(&report_path, &json)?;

    println!(
        "acap_project_pipeline_layers schema={} articles={} pass_articles={} fail_articles={} corpus_verdict={:?} hard_failures={} required_absent={} import_eligible=false report={}",
        ACAP_AGGREGATE_REPORT_SCHEMA_VERSION,
        corpus.article_count,
        pass_articles,
        fail_articles,
        corpus.verdict,
        corpus.aggregate.hard_failures.len(),
        corpus
            .aggregate
            .hard_failures
            .iter()
            .filter(|code| code.starts_with("required_layer_absent:"))
            .count(),
        display_path(&repo_root, &report_path),
    );
    for layer in &corpus.layer_results {
        eprintln!(
            "  layer {} status={:?} pass={} fail={} hard={}",
            layer.layer_id,
            layer.status,
            layer.pass_articles,
            layer.fail_articles,
            layer.hard_failures.len()
        );
    }
    for code in corpus.aggregate.hard_failures.iter().take(20) {
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
