//! Verified cache-to-graph materialization for bounded gold-corpus waves.

use crate::gold_corpus::GoldCorpusManifest;
use anyhow::{Context, Result};
use da_application::ingest::IngestResult;
use da_application::{
    ExtractionResult, ExtractionUseCase, IngestUseCase, MaterializationAssuranceUseCase,
    MaterializationReplayObservation,
};
use da_application::{PipelineRunReport, StageDisposition};
use da_domain::{ArticleAssuranceReport, ContentDigest};
use da_ports::Clock;
use da_ports::extractor::ExtractedEntity;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, StageOutcome};
use kg_storage::{InstrumentedGraphStore, MutationCounters};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
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

type PreparedPaper = (GoldWavePaper, VerifiedPaperArtifacts, CatalogMetadata);

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
    /// Business object creation counters retained for operator diagnostics.
    pub first: NetWrites,
    pub replay: NetWrites,
    /// Physical storage mutation outcomes; authoritative for replay proof.
    pub first_mutations: MutationCounters,
    pub replay_mutations: MutationCounters,
    pub assurance: ArticleAssuranceReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchGraphMaterialization {
    pub manifest_sha256: String,
    pub batch: usize,
    /// Explicit transaction time shared by every materializer in this wave.
    pub valid_at: String,
    pub prepare_workers: usize,
    pub graph_before: (usize, usize),
    pub graph_after_first: (usize, usize),
    pub graph_after_replay: (usize, usize),
    pub replay_zero_writes: bool,
    pub import_eligible: bool,
    pub papers: Vec<PaperGraphMaterialization>,
}

impl BatchGraphMaterialization {
    pub fn passed(&self) -> bool {
        self.papers.iter().all(|paper| {
            paper.first_mutations.partition_is_valid()
                && paper.first_mutations.unobserved_operations == 0
                && paper.first_mutations.rejected() == 0
                && paper.first_mutations.applied() > 0
        }) && self.replay_zero_writes
            && self
                .papers
                .iter()
                .all(|paper| paper.replay_mutations.replay_passes())
            && self
                .papers
                .iter()
                .all(|paper| paper.assurance.verdict == da_domain::AssuranceVerdict::Pass)
            && self.graph_after_first == self.graph_after_replay
            && !self.import_eligible
    }
}

pub type CatalogMetadata = (Option<String>, Option<String>);
pub type MetadataLookup<'a> = dyn Fn(&str) -> Result<CatalogMetadata> + Sync + 'a;

pub struct MaterializationRuntime<'a> {
    pub policy_sha256: ContentDigest,
    pub clock: Arc<dyn Clock>,
    pub prepare_workers: usize,
    pub metadata: &'a MetadataLookup<'a>,
}

const MAX_INDEX_POLICY_BYTES: u64 = 4 * 1024 * 1024;

fn read_project_policy(path: &Path) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("resolve {}", path.display()))?;
    anyhow::ensure!(
        resolved.starts_with(&root),
        "policy path escapes repository: {}",
        path.display()
    );
    let mut file =
        std::fs::File::open(&resolved).with_context(|| format!("open {}", resolved.display()))?;
    anyhow::ensure!(file.metadata()?.is_file(), "policy is not a regular file");
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_INDEX_POLICY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_INDEX_POLICY_BYTES,
        "policy exceeds byte budget"
    );
    Ok(bytes)
}

pub fn load_identity_indexes(
    policy_path: &Path,
    node_schemas_path: &Path,
) -> Result<kg_ontology::IdentityIndexPolicy> {
    let policy_bytes = read_project_policy(policy_path)?;
    let schema_bytes = read_project_policy(node_schemas_path)?;
    let schema_text = std::str::from_utf8(&schema_bytes)
        .with_context(|| format!("{} is not UTF-8", node_schemas_path.display()))?;
    let registry = kg_ontology::OntologyRegistry::load_from_str(
        schema_text,
        &node_schemas_path.display().to_string(),
    )?;
    kg_ontology::IdentityIndexPolicy::parse_and_validate(&policy_bytes, &registry)
        .map_err(anyhow::Error::msg)
}

pub fn load_property_indexes(
    policy_path: &Path,
    node_schemas_path: &Path,
) -> Result<kg_ontology::PropertyIndexPolicy> {
    let policy_bytes = read_project_policy(policy_path)?;
    let schema_bytes = read_project_policy(node_schemas_path)?;
    let schema_text = std::str::from_utf8(&schema_bytes)
        .with_context(|| format!("{} is not UTF-8", node_schemas_path.display()))?;
    let registry = kg_ontology::OntologyRegistry::load_from_str(
        schema_text,
        &node_schemas_path.display().to_string(),
    )?;
    kg_ontology::PropertyIndexPolicy::parse_and_validate(&policy_bytes, &registry)
        .map_err(anyhow::Error::msg)
}

pub async fn validate_identity_values(
    store: &dyn DirectGraphStore,
    policy: &kg_ontology::IdentityIndexPolicy,
) -> Result<()> {
    for index in policy.indexes() {
        let mut seen = BTreeMap::<String, u64>::new();
        for node_id in store.get_nodes_by_label(&index.label).await {
            let Some(value) = store
                .get_node_property_string(node_id, &index.property)
                .await
                .filter(|value| !value.trim().is_empty())
            else {
                if index.required {
                    anyhow::bail!(
                        "missing identity {}.{} on node {node_id}",
                        index.label,
                        index.property
                    );
                }
                continue;
            };
            if let Some(previous) = seen.insert(value.clone(), node_id) {
                anyhow::bail!(
                    "duplicate identity {}.{}={value:?} on nodes {previous} and {node_id}",
                    index.label,
                    index.property
                );
            }
        }
    }
    Ok(())
}

pub async fn ensure_identity_indexes(
    store: &dyn DirectGraphStore,
    policy: &kg_ontology::IdentityIndexPolicy,
) -> Result<()> {
    validate_identity_values(store, policy).await?;
    for index in policy.indexes() {
        store
            .create_property_index(&index.label, &index.property)
            .await
            .with_context(|| format!("create identity index {}.{}", index.label, index.property))?;
    }
    Ok(())
}

pub async fn ensure_property_indexes(
    store: &dyn DirectGraphStore,
    policy: &kg_ontology::PropertyIndexPolicy,
) -> Result<()> {
    for index in policy.indexes() {
        store
            .create_property_index(&index.label, &index.property)
            .await
            .with_context(|| format!("create property index {}.{}", index.label, index.property))?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ArticleMetadata {
    source_code: Option<String>,
    coarse_topic_code: Option<String>,
}

pub fn catalog_metadata_index(
    manifest: &GoldCorpusManifest,
) -> Result<BTreeMap<String, CatalogMetadata>> {
    let mut index = BTreeMap::new();
    for record in &manifest.records {
        let Some(source) = record.source.path.as_deref() else {
            continue;
        };
        let metadata_path = article_root(Path::new(source)).map(|root| root.join("article.json"));
        let value = metadata_path
            .filter(|path| path.is_file())
            .map(|path| {
                serde_json::from_slice::<ArticleMetadata>(
                    &std::fs::read(&path).with_context(|| format!("read {}", path.display()))?,
                )
                .with_context(|| format!("parse {}", path.display()))
            })
            .transpose()?;
        let fallback = infer_catalog_metadata(Path::new(source));
        index.insert(
            record.paper_id.clone(),
            (
                value
                    .as_ref()
                    .and_then(|item| item.coarse_topic_code.clone())
                    .or(fallback.0),
                value
                    .as_ref()
                    .and_then(|item| item.source_code.clone())
                    .or(fallback.1),
            ),
        );
    }
    Ok(index)
}

fn article_root(source: &Path) -> Option<PathBuf> {
    let parent = source.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some("source") {
        parent.parent().map(Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn infer_catalog_metadata(source: &Path) -> CatalogMetadata {
    let parts = source
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let Some(index) = parts.iter().position(|part| part == "arxiv") else {
        return (None, None);
    };
    (parts.get(index + 1).cloned(), Some("arxiv".into()))
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
    let cached_source_sha256 = cache_source_sha256(cache, &paper.paper_id, &parse_key)?;
    anyhow::ensure!(
        cached_source_sha256 == paper.source_sha256,
        "parse cache source hash mismatch for {}",
        paper.paper_id
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusMutationAggregate {
    pub first: MutationCounters,
    pub replay: MutationCounters,
    pub assurance_pass: usize,
    pub assurance_fail: usize,
    pub assurance_no_claim: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiWaveMaterialization {
    pub manifest_sha256: String,
    pub valid_at: String,
    pub expected_batches: usize,
    pub article_count: usize,
    pub prepare_workers: usize,
    pub graph_before: (usize, usize),
    pub graph_after: (usize, usize),
    pub same_process: bool,
    pub import_eligible: bool,
    pub aggregate: CorpusMutationAggregate,
    pub waves: Vec<BatchGraphMaterialization>,
}

impl MultiWaveMaterialization {
    pub fn passed(&self) -> bool {
        self.same_process
            && !self.import_eligible
            && self.waves.len() == self.expected_batches
            && self.article_count
                == self
                    .waves
                    .iter()
                    .map(|wave| wave.papers.len())
                    .sum::<usize>()
            && self.aggregate.assurance_pass == self.article_count
            && self.aggregate.assurance_fail == 0
            && self.aggregate.assurance_no_claim == 0
            && self.aggregate.first.partition_is_valid()
            && self.aggregate.first.rejected() == 0
            && self.aggregate.replay.replay_passes()
            && self.waves.iter().all(BatchGraphMaterialization::passed)
            && self.waves.windows(2).all(|pair| {
                pair[0].batch + 1 == pair[1].batch
                    && pair[0].graph_after_replay == pair[1].graph_before
            })
            && self
                .waves
                .first()
                .is_some_and(|wave| wave.graph_before == self.graph_before)
            && self
                .waves
                .last()
                .is_some_and(|wave| wave.graph_after_replay == self.graph_after)
    }
}

pub fn validate_wave_sequence(reports: &[GoldWaveReport], expected_batches: usize) -> Result<()> {
    anyhow::ensure!(
        expected_batches > 0,
        "expected batch count must be positive"
    );
    anyhow::ensure!(
        reports.len() == expected_batches,
        "wave denominator mismatch: expected {expected_batches}, got {}",
        reports.len()
    );
    let manifest = reports
        .first()
        .context("multi-wave materialization requires reports")?
        .manifest_sha256
        .as_str();
    let mut paper_ids = std::collections::BTreeSet::new();
    for (index, report) in reports.iter().enumerate() {
        let expected_batch = index + 1;
        anyhow::ensure!(report.wave_passed, "wave {} did not pass", report.batch);
        anyhow::ensure!(
            report.batch == expected_batch,
            "wave sequence gap: expected batch {expected_batch}, got {}",
            report.batch
        );
        anyhow::ensure!(
            report.manifest_sha256 == manifest,
            "wave {} manifest mismatch",
            report.batch
        );
        anyhow::ensure!(
            !report.papers.is_empty(),
            "wave {} has no papers",
            report.batch
        );
        for paper in &report.papers {
            anyhow::ensure!(
                paper_ids.insert(paper.paper_id.as_str()),
                "duplicate paper across waves: {}",
                paper.paper_id
            );
        }
    }
    Ok(())
}

pub async fn materialize_waves(
    store: Arc<dyn DirectGraphStore>,
    cache: &dyn PipelineCache,
    reports: &[GoldWaveReport],
    expected_batches: usize,
    runtime: &MaterializationRuntime<'_>,
) -> Result<MultiWaveMaterialization> {
    validate_wave_sequence(reports, expected_batches)?;
    let graph_before = (store.node_count().await, store.edge_count().await);
    let valid_at = runtime.clock.now().to_rfc3339();
    let mut waves: Vec<BatchGraphMaterialization> = Vec::with_capacity(reports.len());
    for report in reports {
        let wave = materialize_wave(Arc::clone(&store), cache, report, runtime).await?;
        anyhow::ensure!(wave.passed(), "batch {} materialization failed", wave.batch);
        if let Some(previous) = waves.last() {
            anyhow::ensure!(
                previous.graph_after_replay == wave.graph_before,
                "graph chain discontinuity before batch {}",
                wave.batch
            );
        }
        waves.push(wave);
    }
    let graph_after = (store.node_count().await, store.edge_count().await);
    let article_count = waves.iter().map(|wave| wave.papers.len()).sum();
    let import_eligible = waves.iter().any(|wave| wave.import_eligible);
    let mut first = MutationCounters::default();
    let mut replay = MutationCounters::default();
    let mut assurance_pass = 0;
    let mut assurance_fail = 0;
    let mut assurance_no_claim = 0;
    for wave in &waves {
        for paper in &wave.papers {
            first.merge(paper.first_mutations);
            replay.merge(paper.replay_mutations);
            match paper.assurance.verdict {
                da_domain::AssuranceVerdict::Pass => assurance_pass += 1,
                da_domain::AssuranceVerdict::Fail => assurance_fail += 1,
                da_domain::AssuranceVerdict::NoClaim => assurance_no_claim += 1,
            }
        }
    }
    let aggregate = CorpusMutationAggregate {
        first,
        replay,
        assurance_pass,
        assurance_fail,
        assurance_no_claim,
    };
    let result = MultiWaveMaterialization {
        manifest_sha256: reports[0].manifest_sha256.clone(),
        valid_at,
        expected_batches,
        article_count,
        prepare_workers: runtime.prepare_workers,
        graph_before,
        graph_after,
        same_process: true,
        import_eligible,
        aggregate,
        waves,
    };
    anyhow::ensure!(result.passed(), "multi-wave materialization gate failed");
    Ok(result)
}

fn prepare_wave(
    cache: &dyn PipelineCache,
    report: &GoldWaveReport,
    metadata: &MetadataLookup<'_>,
    workers: usize,
) -> Result<Vec<PreparedPaper>> {
    anyhow::ensure!(workers > 0, "prepare worker count must be positive");
    let worker_count = workers.min(report.papers.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(paper) = report.papers.get(index) else {
                        break;
                    };
                    let result = (|| {
                        Ok((
                            paper.clone(),
                            load_verified_artifacts(cache, paper)?,
                            metadata(&paper.paper_id)?,
                        ))
                    })();
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut outputs = receiver.into_iter().collect::<Vec<_>>();
        outputs.sort_by_key(|(index, _)| *index);
        outputs
            .into_iter()
            .map(|(_, result)| result)
            .collect::<Result<Vec<_>>>()
    })
}

pub async fn materialize_wave(
    store: Arc<dyn DirectGraphStore>,
    cache: &dyn PipelineCache,
    report: &GoldWaveReport,
    runtime: &MaterializationRuntime<'_>,
) -> Result<BatchGraphMaterialization> {
    let instrumented = InstrumentedGraphStore::new(store);
    let observed_store: Arc<dyn DirectGraphStore> = Arc::new(instrumented.clone());
    let valid_at = runtime.clock.now().to_rfc3339();
    let ingest = IngestUseCase::materializer_only_with_clock(
        Arc::clone(&observed_store),
        Arc::clone(&runtime.clock),
    );
    let extraction = ExtractionUseCase::materializer_only_with_clock(
        Arc::clone(&observed_store),
        Arc::clone(&runtime.clock),
    );
    let assurance = MaterializationAssuranceUseCase::new();
    let graph_before = ingest.graph_stats().await;
    let prepared = prepare_wave(cache, report, runtime.metadata, runtime.prepare_workers)?;

    let mut papers = Vec::with_capacity(prepared.len());
    for (paper, artifacts, (primary_domain, source_code)) in &prepared {
        instrumented.reset().await;
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
            first_mutations: instrumented.snapshot().await,
            replay_mutations: MutationCounters::default(),
            assurance: ArticleAssuranceReport::new(
                paper.paper_id.clone(),
                runtime.policy_sha256.clone(),
                da_domain::ArticleContextState::Materialized,
                vec![],
            )?,
        });
    }
    let graph_after_first = ingest.graph_stats().await;

    let mut replay_contexts = Vec::with_capacity(prepared.len());
    for ((_, artifacts, (primary_domain, source_code)), paper_report) in
        prepared.into_iter().zip(&mut papers)
    {
        instrumented.reset().await;
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
        paper_report.replay_mutations = instrumented.snapshot().await;
        replay_contexts.push((artifacts.parsed, replay_ingest));
    }
    let d127_unlock_count =
        MaterializationAssuranceUseCase::count_d127_unlocks(&instrumented).await;
    for ((parsed, replay_ingest), paper_report) in replay_contexts.into_iter().zip(&mut papers) {
        paper_report.assurance = assurance
            .evaluate_graph_with_d127(
                &instrumented,
                &parsed,
                &replay_ingest,
                MaterializationReplayObservation::from_mutations(paper_report.replay_mutations),
                runtime.policy_sha256.clone(),
                d127_unlock_count,
            )
            .await?;
    }
    let graph_after_replay = ingest.graph_stats().await;
    let replay_zero_writes = papers
        .iter()
        .all(|paper| paper.replay_mutations.replay_passes());
    let import_eligible = d127_unlock_count != 0;
    Ok(BatchGraphMaterialization {
        manifest_sha256: report.manifest_sha256.clone(),
        batch: report.batch,
        valid_at,
        prepare_workers: runtime.prepare_workers.min(report.papers.len()),
        graph_before,
        graph_after_first,
        graph_after_replay,
        replay_zero_writes,
        import_eligible,
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

fn cache_source_sha256(
    cache: &dyn PipelineCache,
    article_id: &str,
    key_sha256: &str,
) -> Result<String> {
    let entries = cache
        .manifest(article_id)?
        .into_iter()
        .filter(|entry| entry.key.stage_id == "parse" && entry.key_sha256 == key_sha256)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        entries.len() == 1,
        "expected one parse cache source identity"
    );
    let source = &entries[0].key.source;
    match source.selected_format {
        da_ports::pipeline_cache::SourceFormat::Pdf => source.pdf_sha256.clone(),
        da_ports::pipeline_cache::SourceFormat::Html => source.html_sha256.clone(),
        da_ports::pipeline_cache::SourceFormat::MetadataOnly => None,
    }
    .context("selected parse source fingerprint missing")
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

    fn wave(batch: usize, manifest: &str, paper_id: &str) -> GoldWaveReport {
        GoldWaveReport {
            manifest_sha256: manifest.into(),
            batch,
            wave_passed: true,
            papers: vec![GoldWavePaper {
                paper_id: paper_id.into(),
                source_sha256: "a".repeat(64),
                replay: PipelineRunReport {
                    pipeline_id: "gold".into(),
                    article_id: paper_id.into(),
                    completed: true,
                    stages: vec![],
                },
            }],
        }
    }

    #[tokio::test]
    async fn identity_index_setup_rejects_duplicate_values() {
        let store = kg_storage::MockGraphStore::new();
        for ordinal in 0..2 {
            let node = store.create_node("Paper").await.unwrap();
            store
                .set_node_property_string(node, "vid", "vid:duplicate".into())
                .await
                .unwrap();
            store
                .set_node_property_string(node, "arxiv_id", format!("2401.{ordinal:05}"))
                .await
                .unwrap();
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let policy = load_identity_indexes(
            &root.join("data/ontology/identity_indexes.yaml"),
            &root.join("data/ontology/node_schemas.yaml"),
        )
        .unwrap();
        let error = ensure_identity_indexes(&store, &policy).await.unwrap_err();
        assert!(error.to_string().contains("duplicate identity"));
    }

    #[test]
    fn identity_indexes_are_ontology_validated_data() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let indexes = load_identity_indexes(
            &root.join("data/ontology/identity_indexes.yaml"),
            &root.join("data/ontology/node_schemas.yaml"),
        )
        .unwrap();
        assert!(indexes.indexes().len() >= 36);
        assert!(indexes.indexes().iter().all(|index| index.identity));
        let unique = indexes
            .indexes()
            .iter()
            .map(|index| (&index.label, &index.property))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), indexes.indexes().len());
    }

    #[test]
    fn multi_wave_sequence_requires_complete_contiguous_unique_denominator() {
        let valid = vec![wave(1, "manifest", "p1"), wave(2, "manifest", "p2")];
        validate_wave_sequence(&valid, 2).unwrap();

        assert!(
            validate_wave_sequence(&valid, 3)
                .unwrap_err()
                .to_string()
                .contains("denominator mismatch")
        );
        assert!(
            validate_wave_sequence(&[wave(2, "manifest", "p1")], 1)
                .unwrap_err()
                .to_string()
                .contains("sequence gap")
        );
        assert!(
            validate_wave_sequence(
                &[wave(1, "manifest-a", "p1"), wave(2, "manifest-b", "p2")],
                2,
            )
            .unwrap_err()
            .to_string()
            .contains("manifest mismatch")
        );
        assert!(
            validate_wave_sequence(&[wave(1, "manifest", "p1"), wave(2, "manifest", "p1")], 2,)
                .unwrap_err()
                .to_string()
                .contains("duplicate paper")
        );
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
