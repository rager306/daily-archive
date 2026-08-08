//! Verified cache-to-graph materialization for bounded gold-corpus waves.

use anyhow::{Context, Result};
use da_application::ingest::IngestResult;
use da_application::{ExtractionResult, ExtractionUseCase, IngestUseCase};
use da_application::{PipelineRunReport, StageDisposition};
use da_ports::extractor::ExtractedEntity;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, StageOutcome};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldWaveReport {
    pub manifest_sha256: String,
    pub batch: usize,
    pub wave_passed: bool,
    pub papers: Vec<GoldWavePaper>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldWavePaper {
    pub paper_id: String,
    pub source_sha256: String,
    pub replay: PipelineRunReport,
}

#[derive(Debug, Clone)]
pub struct VerifiedPaperArtifacts {
    pub parsed: ParsedArticle,
    pub entities: Vec<ExtractedEntity>,
    pub embedding: Vec<f32>,
    pub hashes: ArtifactHashes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactHashes {
    pub parse: String,
    pub extract: String,
    pub embed: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetWrites {
    pub sections: usize,
    pub references: usize,
    pub citations: usize,
    pub entities: usize,
    pub mentions: usize,
    pub found_in: usize,
    pub evidence_bundles: usize,
    pub participates_in: usize,
    pub claims: usize,
    pub problems: usize,
    pub observations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperGraphMaterialization {
    pub paper_id: String,
    pub source_sha256: String,
    pub artifacts: ArtifactHashes,
    pub first: NetWrites,
    pub replay: NetWrites,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchGraphMaterialization {
    pub manifest_sha256: String,
    pub batch: usize,
    pub graph_before: (usize, usize),
    pub graph_after_first: (usize, usize),
    pub graph_after_replay: (usize, usize),
    pub replay_zero_writes: bool,
    pub import_eligible: bool,
    pub papers: Vec<PaperGraphMaterialization>,
}

impl BatchGraphMaterialization {
    pub fn passed(&self) -> bool {
        self.replay_zero_writes
            && self.graph_after_first == self.graph_after_replay
            && !self.import_eligible
    }
}

pub fn load_wave_report(path: &Path) -> Result<GoldWaveReport> {
    let report: GoldWaveReport = serde_json::from_slice(
        &std::fs::read(path).with_context(|| format!("read {}", path.display()))?,
    )
    .with_context(|| format!("parse {}", path.display()))?;
    anyhow::ensure!(
        report.wave_passed,
        "source wave did not pass: {}",
        path.display()
    );
    anyhow::ensure!(!report.papers.is_empty(), "source wave has no papers");
    Ok(report)
}

pub fn load_verified_artifacts(
    cache: &dyn PipelineCache,
    paper: &GoldWavePaper,
) -> Result<VerifiedPaperArtifacts> {
    let (parse, parse_key) = stage_identity(&paper.replay, "parse")?;
    let (extract, extract_key) = stage_identity(&paper.replay, "extract")?;
    let (embed, embed_key) = stage_identity(&paper.replay, "embed")?;
    let parsed: ParsedArticle =
        load_typed_payload(cache, &paper.paper_id, "parse", &parse, &parse_key)?;
    anyhow::ensure!(
        parsed.paper_id == paper.paper_id,
        "parsed paper identity mismatch"
    );
    anyhow::ensure!(
        parsed.pdf_hash == paper.source_sha256,
        "parsed source hash mismatch"
    );
    let entities = load_typed_payload(cache, &paper.paper_id, "extract", &extract, &extract_key)?;
    let embedding: Vec<f32> =
        load_typed_payload(cache, &paper.paper_id, "embed", &embed, &embed_key)?;
    anyhow::ensure!(
        !embedding.is_empty(),
        "empty embedding for {}",
        paper.paper_id
    );
    anyhow::ensure!(
        embedding.iter().all(|value| value.is_finite()),
        "non-finite embedding"
    );
    Ok(VerifiedPaperArtifacts {
        parsed,
        entities,
        embedding,
        hashes: ArtifactHashes {
            parse,
            extract,
            embed,
        },
    })
}

pub async fn materialize_wave(
    store: Arc<dyn DirectGraphStore>,
    cache: &dyn PipelineCache,
    report: &GoldWaveReport,
    metadata: impl Fn(&str) -> Result<(Option<String>, Option<String>)>,
) -> Result<BatchGraphMaterialization> {
    let ingest = IngestUseCase::materializer_only(Arc::clone(&store));
    let extraction = ExtractionUseCase::materializer_only(Arc::clone(&store));
    let graph_before = ingest.graph_stats().await;
    let mut prepared = Vec::with_capacity(report.papers.len());
    for paper in &report.papers {
        prepared.push((
            paper.clone(),
            load_verified_artifacts(cache, paper)?,
            metadata(&paper.paper_id)?,
        ));
    }

    let mut papers = Vec::with_capacity(prepared.len());
    for (paper, artifacts, (primary_domain, source_code)) in &prepared {
        let first_ingest = ingest
            .materialize_parsed(
                &artifacts.parsed,
                artifacts.embedding.clone(),
                primary_domain.clone(),
                source_code.clone(),
                None,
            )
            .await?;
        let first_extract = extraction
            .materialize_extracted(&artifacts.parsed, artifacts.entities.clone())
            .await?;
        papers.push(PaperGraphMaterialization {
            paper_id: paper.paper_id.clone(),
            source_sha256: paper.source_sha256.clone(),
            artifacts: artifacts.hashes.clone(),
            first: net_writes(&first_ingest, &first_extract),
            replay: NetWrites::default(),
        });
    }
    let graph_after_first = ingest.graph_stats().await;

    for ((_, artifacts, (primary_domain, source_code)), paper_report) in
        prepared.into_iter().zip(&mut papers)
    {
        let replay_ingest = ingest
            .materialize_parsed(
                &artifacts.parsed,
                artifacts.embedding,
                primary_domain,
                source_code,
                None,
            )
            .await?;
        let replay_extract = extraction
            .materialize_extracted(&artifacts.parsed, artifacts.entities)
            .await?;
        paper_report.replay = net_writes(&replay_ingest, &replay_extract);
    }
    let graph_after_replay = ingest.graph_stats().await;
    let replay_zero_writes = papers
        .iter()
        .all(|paper| paper.replay == NetWrites::default());
    Ok(BatchGraphMaterialization {
        manifest_sha256: report.manifest_sha256.clone(),
        batch: report.batch,
        graph_before,
        graph_after_first,
        graph_after_replay,
        replay_zero_writes,
        import_eligible: false,
        papers,
    })
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stage_identity(report: &PipelineRunReport, step: &str) -> Result<(String, String)> {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.step_name == step)
        .with_context(|| format!("missing successful stage '{step}'"))?;
    let key = stage
        .cache_key_sha256
        .clone()
        .with_context(|| format!("stage '{step}' report lacks exact cache key identity"))?;
    match &stage.disposition {
        StageDisposition::Executed { output_sha256 }
        | StageDisposition::CacheHit { output_sha256 } => Ok((output_sha256.clone(), key)),
        other => anyhow::bail!("stage '{step}' has no successful output: {other:?}"),
    }
}

fn load_typed_payload<T: serde::de::DeserializeOwned>(
    cache: &dyn PipelineCache,
    article_id: &str,
    stage_id: &str,
    output_sha256: &str,
    key_sha256: &str,
) -> Result<T> {
    let matches = cache
        .manifest(article_id)?
        .into_iter()
        .filter(|entry| {
            entry.key.stage_id == stage_id
                && entry.key_sha256 == key_sha256
                && matches!(
                    &entry.outcome,
                    StageOutcome::Success { payload_sha256, .. }
                        if payload_sha256 == output_sha256
                )
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        matches.len() == 1,
        "expected one {stage_id} cache entry for {article_id}/{key_sha256}/{output_sha256}, found {}",
        matches.len()
    );
    let CacheLookup::Hit(hit) = cache.lookup(&matches[0].key, "9999-12-31T23:59:59Z")? else {
        anyhow::bail!("verified cache entry was not a hit for {article_id}/{stage_id}");
    };
    serde_json::from_slice(&hit.payload)
        .with_context(|| format!("deserialize {stage_id} payload for {article_id}"))
}

fn net_writes(ingest: &IngestResult, extraction: &ExtractionResult) -> NetWrites {
    NetWrites {
        sections: ingest.sections_written,
        references: ingest.references_written,
        citations: ingest.cites_resolved,
        entities: extraction.entities_created,
        mentions: extraction.mentions_edges,
        found_in: extraction.found_in_edges,
        evidence_bundles: extraction.evidence_bundles_created,
        participates_in: extraction.participates_in_edges,
        claims: extraction.claims_created,
        problems: extraction.problems_created,
        observations: extraction.observations_created,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_application::StageRunRecord;

    fn stage(step: &str) -> StageRunRecord {
        StageRunRecord {
            step_name: step.into(),
            stage_id: step.into(),
            cache_key_sha256: Some(format!("key-{step}")),
            disposition: StageDisposition::CacheHit {
                output_sha256: step.repeat(64),
            },
        }
    }

    #[test]
    fn failed_wave_report_is_rejected_before_cache_or_graph_access() {
        let root =
            std::env::temp_dir().join(format!("da-gold-graph-report-{}", std::process::id()));
        let report = GoldWaveReport {
            manifest_sha256: "m".repeat(64),
            batch: 1,
            wave_passed: false,
            papers: vec![],
        };
        std::fs::write(&root, serde_json::to_vec(&report).unwrap()).unwrap();
        let error = load_wave_report(&root).unwrap_err();
        assert!(error.to_string().contains("source wave did not pass"));
        std::fs::remove_file(root).unwrap();
    }

    #[test]
    fn embedding_stage_is_mandatory_for_graph_materialization() {
        let paper = GoldWavePaper {
            paper_id: "2401.00001".into(),
            source_sha256: "a".repeat(64),
            replay: PipelineRunReport {
                pipeline_id: "gold".into(),
                article_id: "2401.00001".into(),
                completed: true,
                stages: vec![stage("parse"), stage("extract")],
            },
        };
        let error = stage_identity(&paper.replay, "embed").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing successful stage 'embed'")
        );
    }
}
