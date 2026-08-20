//! Verified cache-to-graph materialization for bounded gold-corpus waves.

use crate::gold_corpus::GoldCorpusManifest;
use anyhow::{Context, Result};
use da_application::ingest::IngestResult;
use da_application::{
    ExtractionResult, ExtractionUseCase, IngestUseCase, MaterializationAssuranceUseCase,
    MaterializationReadback, MaterializationReplayObservation, MaterializedSectionObservation,
    RevisionClassifyUseCase, RevisionContentContext, RevisionManifestationInput,
};
use da_application::{PipelineRunReport, StageDisposition, StageRunRecord};
use da_domain::{ArticleAssuranceReport, ArticleLocator, ContentDigest};
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

type PreparedPaper = (
    GoldWavePaper,
    VerifiedPaperArtifacts,
    CatalogMetadata,
    CatalogRevisionBinding,
);

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
    #[serde(default)]
    pub revision: u32,
    #[serde(default)]
    pub binding_method: String,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogMetadata {
    pub primary_domain: Option<String>,
    pub source_code: Option<String>,
    pub article_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRevisionBinding {
    pub context: RevisionContentContext,
    pub method: &'static str,
}

pub type MetadataLookup<'a> = dyn Fn(&str) -> Result<CatalogMetadata> + Sync + 'a;

pub struct MaterializationRuntime<'a> {
    pub policy_sha256: ContentDigest,
    pub clock: Arc<dyn Clock>,
    pub prepare_workers: usize,
    pub metadata: &'a MetadataLookup<'a>,
    pub progress: Option<crate::materialization_progress::MaterializationProgress>,
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
        let root = article_root(Path::new(source));
        let metadata_path = root.as_ref().map(|root| root.join("article.json"));
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
            CatalogMetadata {
                primary_domain: value
                    .as_ref()
                    .and_then(|item| item.coarse_topic_code.clone())
                    .or(fallback.primary_domain),
                source_code: value
                    .as_ref()
                    .and_then(|item| item.source_code.clone())
                    .or(fallback.source_code),
                article_root: root,
            },
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
        return CatalogMetadata::default();
    };
    CatalogMetadata {
        primary_domain: parts.get(index + 1).cloned(),
        source_code: Some("arxiv".into()),
        article_root: article_root(source),
    }
}

#[derive(Debug, Deserialize)]
struct VersionedArticleMetadata {
    article_key: String,
    source_code: Option<String>,
    requested_source: RequestedRevisionSource,
    source_variants: Vec<VersionedSourceVariant>,
}

#[derive(Debug, Deserialize)]
struct RequestedRevisionSource {
    revision: u32,
    source_key: String,
    versioned: bool,
}

#[derive(Debug, Deserialize)]
struct VersionedSourceVariant {
    byte_size: u64,
    capture_status: String,
    fetched_from_url: Option<String>,
    path: String,
    requested_revision: Option<u32>,
    sha256: String,
    source_format: String,
}

fn is_hex_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn revision_from_metadata_path(path: &Path) -> Result<u32> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("versioned metadata path is not UTF-8")?;
    let revision = name
        .strip_prefix("article.v")
        .and_then(|value| value.strip_suffix(".json"))
        .with_context(|| format!("invalid versioned metadata filename {name}"))?;
    revision
        .parse::<u32>()
        .with_context(|| format!("invalid revision in versioned metadata filename {name}"))
}

fn resolved_catalog_path(root: &Path, value: &str) -> Result<PathBuf> {
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalize catalog root {}", root.display()))?;
    let candidate = if Path::new(value).is_absolute() {
        PathBuf::from(value)
    } else {
        root.join(value)
    };
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("canonicalize versioned source {value}"))?;
    anyhow::ensure!(
        resolved.starts_with(&root),
        "versioned source escapes article root: {}",
        resolved.display()
    );
    Ok(resolved)
}

fn evidence_path(path: &Path) -> String {
    let Ok(current) = std::env::current_dir().and_then(|root| root.canonicalize()) else {
        return path.display().to_string();
    };
    match path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&current) => resolved
            .strip_prefix(&current)
            .map(|relative| relative.display().to_string())
            .unwrap_or_else(|_| resolved.display().to_string()),
        Ok(resolved) => resolved.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn manifestation_from_variant(
    root: &Path,
    revision: u32,
    variant: &VersionedSourceVariant,
) -> Result<RevisionManifestationInput> {
    let format = variant.source_format.to_ascii_lowercase();
    anyhow::ensure!(
        format == "pdf" || format == "html",
        "unsupported versioned source format {:?}",
        variant.source_format
    );
    anyhow::ensure!(
        is_hex_sha256(&variant.sha256),
        "invalid versioned source digest {:?}",
        variant.sha256
    );
    anyhow::ensure!(
        variant
            .requested_revision
            .is_some_and(|value| value == revision),
        "versioned source revision drift for {}: expected v{revision}, found {:?}",
        variant.path,
        variant.requested_revision
    );
    let path = resolved_catalog_path(root, &variant.path)?;
    let bytes = std::fs::metadata(&path)
        .with_context(|| format!("stat versioned source {}", path.display()))?
        .len();
    anyhow::ensure!(
        bytes == variant.byte_size,
        "versioned source byte mismatch for {}: metadata {}, disk {bytes}",
        path.display(),
        variant.byte_size
    );
    let actual = sha256(
        &std::fs::read(&path)
            .with_context(|| format!("read versioned source {}", path.display()))?,
    );
    anyhow::ensure!(
        actual == variant.sha256,
        "versioned source digest mismatch for {}: metadata {}, disk {actual}",
        path.display(),
        variant.sha256
    );
    Ok(RevisionManifestationInput {
        format,
        sha256: variant.sha256.clone(),
        bytes,
        uri: variant
            .fetched_from_url
            .clone()
            .unwrap_or_else(|| format!("catalog:{}", path.display())),
        path: variant.path.clone(),
        capture_status: variant.capture_status.clone(),
    })
}

fn exact_context(
    paper_id: &str,
    revision: u32,
    source_code: &str,
    manifestations: Vec<RevisionManifestationInput>,
) -> Result<RevisionContentContext> {
    let locator = ArticleLocator::Arxiv {
        id: paper_id.to_string(),
        revision: Some(revision),
    };
    RevisionClassifyUseCase::bundled()?
        .revision_context_for_classified_load(&locator, source_code, paper_id, manifestations)
        .map_err(anyhow::Error::msg)?
        .with_context(|| {
            format!("exact revision {paper_id}v{revision} classified without a context")
        })
}

pub fn revision_binding_from_catalog(
    paper_id: &str,
    source_sha256: &str,
    root: &Path,
) -> Result<CatalogRevisionBinding> {
    anyhow::ensure!(
        root.is_dir(),
        "catalog article root missing: {}",
        root.display()
    );
    let mut by_digest: BTreeMap<String, CatalogRevisionBinding> = BTreeMap::new();
    let mut metadata_entries = root
        .read_dir()
        .with_context(|| format!("read catalog root {}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("article.v") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    metadata_entries.sort();

    for metadata_path in metadata_entries {
        let revision = revision_from_metadata_path(&metadata_path)?;
        let metadata: VersionedArticleMetadata = serde_json::from_slice(
            &std::fs::read(&metadata_path)
                .with_context(|| format!("read {}", metadata_path.display()))?,
        )
        .with_context(|| format!("parse {}", metadata_path.display()))?;
        anyhow::ensure!(
            metadata.article_key == paper_id,
            "versioned metadata identity drift: expected {paper_id}, found {}",
            metadata.article_key
        );
        anyhow::ensure!(
            metadata.requested_source.versioned && metadata.requested_source.revision == revision,
            "versioned metadata revision drift in {}",
            metadata_path.display()
        );
        let expected_source_key = format!("{paper_id}v{revision}");
        anyhow::ensure!(
            metadata.requested_source.source_key == expected_source_key,
            "versioned source key drift: expected {expected_source_key}, found {}",
            metadata.requested_source.source_key
        );
        anyhow::ensure!(
            !metadata.source_variants.is_empty(),
            "versioned metadata has no source variants: {}",
            metadata_path.display()
        );
        let manifestations = metadata
            .source_variants
            .iter()
            .map(|variant| manifestation_from_variant(root, revision, variant))
            .collect::<Result<Vec<_>>>()?;
        let source_code = metadata
            .source_code
            .clone()
            .unwrap_or_else(|| "arxiv".to_string());
        let context = exact_context(paper_id, revision, &source_code, manifestations)?;
        let binding = CatalogRevisionBinding {
            context,
            method: "authoritative_version_metadata",
        };
        for manifestation in &binding.context.manifestations {
            if let Some(previous) = by_digest.insert(manifestation.sha256.clone(), binding.clone())
                && previous.context != binding.context
            {
                anyhow::bail!(
                    "ambiguous revision binding for {paper_id} digest {}: v{} and v{}",
                    manifestation.sha256,
                    previous.context.revision,
                    binding.context.revision
                );
            }
        }
    }

    if let Some(binding) = by_digest.get(source_sha256) {
        return Ok(binding.clone());
    }

    // Recovery path: no version metadata digest matched, but exactly one local
    // versioned asset may have the selected bytes. This never invents a revision.
    let mut recovered = Vec::new();
    let mut scan_paths = Vec::new();
    if let Ok(entries) = root.join("source").read_dir() {
        scan_paths.extend(entries.filter_map(Result::ok).map(|entry| entry.path()));
    }
    if let Ok(entries) = root.read_dir() {
        scan_paths.extend(entries.filter_map(Result::ok).map(|entry| entry.path()));
    }
    for path in scan_paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let (prefix, extension) = if let Some(value) = name.strip_suffix(".pdf") {
            (value, "pdf")
        } else if let Some(value) = name.strip_suffix(".html") {
            (value, "html")
        } else {
            continue;
        };
        let Some(revision) = prefix
            .strip_prefix(&format!("{paper_id}v"))
            .or_else(|| prefix.strip_prefix("article.v"))
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        anyhow::ensure!(revision > 0, "revision zero is invalid");
        let bytes = std::fs::metadata(&path)
            .with_context(|| format!("stat {}", path.display()))?
            .len();
        let digest =
            sha256(&std::fs::read(&path).with_context(|| format!("read {}", path.display()))?);
        if digest == source_sha256 {
            recovered.push((
                revision,
                RevisionManifestationInput {
                    format: extension.to_string(),
                    sha256: digest,
                    bytes,
                    uri: format!("catalog:{}", evidence_path(&path)),
                    path: evidence_path(&path),
                    capture_status: "captured_local".into(),
                },
            ));
        }
    }
    if recovered.len() > 1 {
        let revisions = recovered
            .iter()
            .map(|(revision, _)| format!("v{revision}"))
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("ambiguous checksum revision binding for {paper_id}: {revisions}");
    }
    if let Some((revision, manifestation)) = recovered.pop() {
        let context = exact_context(paper_id, revision, "arxiv", vec![manifestation])?;
        return Ok(CatalogRevisionBinding {
            context,
            method: "unique_checksum_recovery",
        });
    }

    anyhow::bail!(
        "no exact revision binding for {paper_id} source {source_sha256}; versionless corpus materialization is blocked"
    )
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
    let extract_stage_id = stage_cache_id(&paper.replay, "extract")?;
    let embed_stage_id = stage_cache_id(&paper.replay, "embed")?;
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
    let entities = load_typed_payload(
        cache,
        &paper.paper_id,
        &extract_stage_id,
        &extract,
        &extract_key,
    )?;
    let embedding: Vec<f32> =
        load_typed_payload(cache, &paper.paper_id, &embed_stage_id, &embed, &embed_key)?;
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
        if let Some(progress) = &runtime.progress {
            progress.phase(
                crate::materialization_progress::MaterializationPhase::WaveStart,
                report.batch,
            );
        }
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
                        let metadata = metadata(&paper.paper_id)?;
                        let root = metadata.article_root.as_ref().with_context(|| {
                            format!("catalog article root missing for {}", paper.paper_id)
                        })?;
                        let binding = revision_binding_from_catalog(
                            &paper.paper_id,
                            &paper.source_sha256,
                            root,
                        )?;
                        Ok((
                            paper.clone(),
                            load_verified_artifacts(cache, paper)?,
                            metadata,
                            binding,
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

async fn revision_readback(
    graph: &dyn DirectGraphStore,
    parsed: &ParsedArticle,
    context: &RevisionContentContext,
    d127_unlock_count: u64,
) -> Result<MaterializationReadback> {
    let revision_vid = context
        .revision_vid()
        .map_err(|error| anyhow::anyhow!("revision VID: {error}"))?;
    let revision_node = graph
        .find_node_by_string_property("PaperRevision", "vid", &revision_vid)
        .await
        .with_context(|| format!("revision node missing for {revision_vid}"))?;
    let abstract_text = graph
        .get_node_property_string(revision_node, "abstract_text")
        .await;
    let mut sections = Vec::with_capacity(parsed.sections.len());
    for ordinal in 0..parsed.sections.len() {
        let section_vid = context
            .section_vid(ordinal)
            .map_err(|error| anyhow::anyhow!("revision section VID: {error}"))?;
        let Some(node) = graph
            .find_node_by_string_property("Section", "vid", &section_vid)
            .await
        else {
            continue;
        };
        if let Some(text) = graph.get_node_property_string(node, "text").await {
            sections.push(MaterializedSectionObservation { ordinal, text });
        }
    }
    let observed_sections = sections.len() as u64;
    let paper_citation_count = graph
        .get_node_property_int(revision_node, "citation_count")
        .await
        .and_then(|value| u64::try_from(value).ok());
    Ok(MaterializationReadback {
        abstract_text,
        sections,
        paper_section_count: Some(observed_sections),
        // Bibliography is counted on the revision that owns the parsed bytes;
        // cross-revision citation ownership remains PIR-011.
        paper_citation_count,
        import_eligible_true_nodes: d127_unlock_count,
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
    if let Some(progress) = &runtime.progress {
        progress.phase(
            crate::materialization_progress::MaterializationPhase::Prepare,
            report.batch,
        );
    }
    let prepared = prepare_wave(cache, report, runtime.metadata, runtime.prepare_workers)?;

    let mut papers = Vec::with_capacity(prepared.len());
    for (paper, artifacts, metadata, binding) in &prepared {
        if let Some(progress) = &runtime.progress {
            progress.paper_started(
                crate::materialization_progress::MaterializationPhase::FirstWrite,
                report.batch,
                &paper.paper_id,
            );
        }
        instrumented.reset().await;
        let first_ingest = ingest
            .materialize_parsed_with_context(
                &artifacts.parsed,
                artifacts.embedding.clone(),
                metadata.primary_domain.clone(),
                metadata.source_code.clone(),
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                Some(&binding.context),
            )
            .await?;
        let first_extract = extraction
            .materialize_extracted_with_context(
                &artifacts.parsed,
                artifacts.entities.clone(),
                Some(&binding.context),
            )
            .await?;
        let first_mutations = instrumented.snapshot().await;
        if let Some(progress) = &runtime.progress {
            progress.work_unit_completed(first_mutations);
        }
        papers.push(PaperGraphMaterialization {
            paper_id: paper.paper_id.clone(),
            source_sha256: paper.source_sha256.clone(),
            revision: binding.context.revision,
            binding_method: binding.method.to_string(),
            artifacts: artifacts.hashes.clone(),
            first: net_writes(&first_ingest, &first_extract),
            replay: NetWrites::default(),
            first_mutations,
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
    for ((paper, artifacts, metadata, binding), paper_report) in
        prepared.into_iter().zip(&mut papers)
    {
        if let Some(progress) = &runtime.progress {
            progress.paper_started(
                crate::materialization_progress::MaterializationPhase::Replay,
                report.batch,
                &paper.paper_id,
            );
        }
        instrumented.reset().await;
        let replay_ingest = ingest
            .materialize_parsed_with_context(
                &artifacts.parsed,
                artifacts.embedding,
                metadata.primary_domain,
                metadata.source_code,
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                Some(&binding.context),
            )
            .await?;
        let replay_extract = extraction
            .materialize_extracted_with_context(
                &artifacts.parsed,
                artifacts.entities,
                Some(&binding.context),
            )
            .await?;
        paper_report.replay = net_writes(&replay_ingest, &replay_extract);
        paper_report.replay_mutations = instrumented.snapshot().await;
        if let Some(progress) = &runtime.progress {
            progress.work_unit_completed(paper_report.replay_mutations);
        }
        replay_contexts.push((artifacts.parsed, replay_ingest, binding.context));
    }
    let d127_unlock_count =
        MaterializationAssuranceUseCase::count_d127_unlocks(&instrumented).await;
    for ((parsed, replay_ingest, revision_context), paper_report) in
        replay_contexts.into_iter().zip(&mut papers)
    {
        if let Some(progress) = &runtime.progress {
            progress.paper_started(
                crate::materialization_progress::MaterializationPhase::Assurance,
                report.batch,
                &paper_report.paper_id,
            );
        }
        let readback =
            revision_readback(&instrumented, &parsed, &revision_context, d127_unlock_count).await?;
        paper_report.assurance = assurance.evaluate(
            &parsed,
            &replay_ingest,
            &readback,
            MaterializationReplayObservation::from_mutations(paper_report.replay_mutations),
            runtime.policy_sha256.clone(),
        )?;
        if let Some(progress) = &runtime.progress {
            progress.paper_completed();
        }
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
    let stage = stage_record(report, step)?;
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

fn stage_cache_id(report: &PipelineRunReport, step: &str) -> Result<String> {
    Ok(stage_record(report, step)?.stage_id.clone())
}

fn stage_record<'a>(report: &'a PipelineRunReport, step: &str) -> Result<&'a StageRunRecord> {
    let aliases: [&str; 2] = match step {
        "embed" => ["embed", "embedding"],
        "extract" => ["extract", "extraction_compute"],
        other => [other, other],
    };
    report
        .stages
        .iter()
        .find(|stage| {
            aliases.contains(&stage.step_name.as_str())
                || aliases.contains(&stage.stage_id.as_str())
        })
        .with_context(|| format!("missing successful stage '{step}'"))
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

    /// Resolve a semantic label through the ontology registry before querying
    /// the graph. Tests must not hardcode graph label walks (ADR-050).
    async fn registered_label_nodes(
        store: &dyn DirectGraphStore,
        semantic_label: &str,
    ) -> Vec<u64> {
        let label = da_domain::schema::registered_node_labels_or_yaml()
            .into_iter()
            .find(|label| label == semantic_label)
            .unwrap_or_else(|| panic!("label is not ontology-registered: {semantic_label}"));
        store.get_nodes_by_label(&label).await
    }

    fn stage(step: &str) -> StageRunRecord {
        StageRunRecord {
            step_name: step.into(),
            stage_id: step.into(),
            resolved_from: None,
            cache_key_sha256: Some(format!("key-{step}")),
            admission: None,
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

    #[test]
    fn catalog_revision_bindings_select_exact_versions_without_inventing_identity() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/article_catalog/article_catalog/arxiv/cs-ai/2506.07853");
        let v1 = sha256(&std::fs::read(root.join("source/2506.07853v1.pdf")).unwrap());
        let v5 = sha256(&std::fs::read(root.join("source/2506.07853v5.pdf")).unwrap());
        assert_ne!(v1, v5);
        let first = revision_binding_from_catalog("2506.07853", &v1, &root).unwrap();
        let latest = revision_binding_from_catalog("2506.07853", &v5, &root).unwrap();
        assert_eq!(first.method, "authoritative_version_metadata");
        assert_eq!(latest.method, "authoritative_version_metadata");
        assert_eq!(first.context.revision, 1);
        assert_eq!(latest.context.revision, 5);
        assert_eq!(first.context.canonical_id, "2506.07853");
        assert_ne!(
            first.context.revision_vid().unwrap(),
            latest.context.revision_vid().unwrap()
        );
        assert_ne!(
            first.context.section_vid(0).unwrap(),
            latest.context.section_vid(0).unwrap()
        );
    }

    #[test]
    fn missing_or_ambiguous_revision_evidence_fails_closed() {
        let root = std::env::temp_dir().join(format!("da-revision-missing-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("article.json"), b"{}").unwrap();
        let error =
            revision_binding_from_catalog("2506.07853", &"a".repeat(64), &root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("versionless corpus materialization is blocked")
        );
        std::fs::remove_dir_all(root).unwrap();

        let ambiguous_root =
            std::env::temp_dir().join(format!("da-revision-ambiguous-{}", std::process::id()));
        let source = ambiguous_root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        let bytes = b"same-version-bytes";
        std::fs::write(source.join("2506.07853v1.pdf"), bytes).unwrap();
        std::fs::write(source.join("2506.07853v2.pdf"), bytes).unwrap();
        let digest = sha256(bytes);
        let error =
            revision_binding_from_catalog("2506.07853", &digest, &ambiguous_root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ambiguous checksum revision binding")
        );
        std::fs::remove_dir_all(ambiguous_root).unwrap();
    }

    #[tokio::test]
    async fn gold_wave_materializes_two_revisions_with_distinct_sections_and_replay() {
        use da_adapters::FilePipelineCache;
        use da_ports::pipeline_cache::{SourceFormat, SourceIdentity, StageCacheKey};

        let catalog_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/article_catalog/article_catalog/arxiv/cs-ai/2506.07853");
        let cache_root =
            std::env::temp_dir().join(format!("da-gold-two-revisions-{}", std::process::id()));
        std::fs::create_dir_all(&cache_root).unwrap();
        let cache = FilePipelineCache::new(cache_root.join("pipeline_cache"));
        let extractor = da_adapters::RuleBasedExtractor::with_config(
            da_adapters::rule_extractor::ExtractionConfig::bundled(),
        );
        let extraction_identity = extractor.cache_identity();
        let mut papers = Vec::new();

        for revision in [1_u32, 5_u32] {
            let source_path = catalog_root.join(format!("source/2506.07853v{revision}.pdf"));
            let source_bytes = std::fs::read(&source_path).unwrap();
            let source_sha = sha256(&source_bytes);
            let parsed = ParsedArticle {
                paper_id: "2506.07853".into(),
                title: format!("Revision {revision}"),
                abstract_text: format!("Abstract for revision {revision}"),
                body_text: format!("Body for revision {revision}"),
                sections: vec![da_ports::parser::Section {
                    title: "Methods".into(),
                    text: format!("Methods text for revision {revision}"),
                    level: 1,
                    ..Default::default()
                }],
                citations: vec![da_ports::parser::CitationEntry {
                    raw_text: format!("Citation in revision {revision}"),
                    doi: None,
                    arxiv_id: Some("2507.19457".into()),
                    title: Some("Cited revision-aware paper".into()),
                    anchors: Vec::new(),
                }],
                layout_json: None,
                tei_xml: None,
                tables: Vec::new(),
                table_missingness: None,
                formulas: Vec::new(),
                formula_missingness: None,
                page_text_missingness: None,
                pdf_hash: source_sha.clone(),
            };
            let entities = Vec::<ExtractedEntity>::new();
            let embedding = vec![0.25_f32, -0.5, 1.0];
            let parse_payload = serde_json::to_vec(&parsed).unwrap();
            let extract_payload = serde_json::to_vec(&entities).unwrap();
            let embed_payload = serde_json::to_vec(&embedding).unwrap();
            let source = SourceIdentity {
                article_id: "2506.07853".into(),
                selected_format: SourceFormat::Pdf,
                pdf_sha256: Some(source_sha.clone()),
                html_sha256: None,
                acquisition_fingerprint: format!("acq-v2;pdf={source_sha};html=none"),
                normalization_fingerprint: "identity-v1".into(),
            };
            let parse_key = StageCacheKey {
                source: source.clone(),
                stage_id: "parse".into(),
                dependency_output_sha256: BTreeMap::new(),
                implementation_fingerprint: "test-parser@two-revisions".into(),
                configuration_fingerprint: "profile=structural;test".into(),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: "ParsedArticle.v1".into(),
            };
            let extraction_key = StageCacheKey {
                source: source.clone(),
                stage_id: "extraction_compute".into(),
                dependency_output_sha256: BTreeMap::from([(
                    "parse".into(),
                    sha256(&parse_payload),
                )]),
                implementation_fingerprint: extraction_identity.implementation_fingerprint.clone(),
                configuration_fingerprint: extraction_identity.configuration_fingerprint.clone(),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: extraction_identity.output_schema_fingerprint.clone(),
            };
            let embedding_key = StageCacheKey {
                source,
                stage_id: "embedding".into(),
                dependency_output_sha256: BTreeMap::from([(
                    "parse".into(),
                    sha256(&parse_payload),
                )]),
                implementation_fingerprint: "test-embedder@two-revisions".into(),
                configuration_fingerprint: "model=test;dims=3".into(),
                prompt_fingerprint: None,
                model_fingerprint: Some("test".into()),
                output_schema_fingerprint: "Vec<f32>.json.v1".into(),
            };
            let parse_key_sha = parse_key.key_sha256().unwrap();
            let extraction_key_sha = extraction_key.key_sha256().unwrap();
            let embedding_key_sha = embedding_key.key_sha256().unwrap();
            cache
                .store_success(
                    &parse_key,
                    &parse_payload,
                    BTreeMap::new(),
                    "2026-08-14T00:00:00Z",
                )
                .unwrap();
            cache
                .store_success(
                    &extraction_key,
                    &extract_payload,
                    BTreeMap::new(),
                    "2026-08-14T00:00:00Z",
                )
                .unwrap();
            cache
                .store_success(
                    &embedding_key,
                    &embed_payload,
                    BTreeMap::new(),
                    "2026-08-14T00:00:00Z",
                )
                .unwrap();
            let stages = vec![
                StageRunRecord {
                    step_name: "parse".into(),
                    stage_id: "parse".into(),
                    resolved_from: None,
                    cache_key_sha256: Some(parse_key_sha),
                    admission: None,
                    disposition: StageDisposition::CacheHit {
                        output_sha256: sha256(&parse_payload),
                    },
                },
                StageRunRecord {
                    step_name: "extraction_compute".into(),
                    stage_id: "extraction_compute".into(),
                    resolved_from: None,
                    cache_key_sha256: Some(extraction_key_sha),
                    admission: None,
                    disposition: StageDisposition::CacheHit {
                        output_sha256: sha256(&extract_payload),
                    },
                },
                StageRunRecord {
                    step_name: "embedding".into(),
                    stage_id: "embedding".into(),
                    resolved_from: None,
                    cache_key_sha256: Some(embedding_key_sha),
                    admission: None,
                    disposition: StageDisposition::CacheHit {
                        output_sha256: sha256(&embed_payload),
                    },
                },
            ];
            papers.push(GoldWavePaper {
                paper_id: "2506.07853".into(),
                source_sha256: source_sha,
                replay: PipelineRunReport {
                    pipeline_id: "gold-two-revisions".into(),
                    article_id: "2506.07853".into(),
                    completed: true,
                    stages,
                },
            });
        }

        let report = GoldWaveReport {
            manifest_sha256: "two-revision-test".into(),
            batch: 1,
            wave_passed: true,
            papers,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "2506.07853".to_string(),
            CatalogMetadata {
                primary_domain: Some("cs-ai".into()),
                source_code: Some("arxiv".into()),
                article_root: Some(catalog_root.clone()),
            },
        );
        let metadata_lookup = |paper_id: &str| {
            metadata
                .get(paper_id)
                .cloned()
                .with_context(|| format!("metadata missing for {paper_id}"))
        };
        let clock: Arc<dyn Clock> = Arc::new(da_ports::FixedClock::from_timestamp(1_700_000_000));
        let runtime = MaterializationRuntime {
            policy_sha256: ContentDigest::of_bytes(b"two-revision-policy"),
            clock,
            prepare_workers: 1,
            metadata: &metadata_lookup,
            progress: None,
        };
        let store = Arc::new(kg_storage::MockGraphStore::new());
        let wave = materialize_wave(
            store.clone() as Arc<dyn DirectGraphStore>,
            &cache,
            &report,
            &runtime,
        )
        .await
        .unwrap();
        assert!(
            wave.passed(),
            "two-revision wave failed assurance/replay gate:\n{wave:#?}"
        );
        assert!(!wave.import_eligible);
        assert_eq!(wave.papers.len(), 2);
        assert_eq!(wave.papers[0].revision, 1);
        assert_eq!(wave.papers[1].revision, 5);
        assert_eq!(
            wave.papers[0].binding_method,
            "authoritative_version_metadata"
        );
        for paper in &wave.papers {
            assert!(paper.first_mutations.applied() > 0);
            assert!(paper.replay_mutations.replay_passes());
        }

        let revisions =
            registered_label_nodes(store.as_ref() as &dyn DirectGraphStore, "PaperRevision").await;
        assert_eq!(revisions.len(), 2);
        let mut revision_vids = std::collections::BTreeSet::new();
        for node in &revisions {
            let vid = store.get_node_property_string(*node, "vid").await.unwrap();
            revision_vids.insert(vid);
        }
        assert_eq!(revision_vids.len(), 2);
        let papers_in_graph =
            registered_label_nodes(store.as_ref() as &dyn DirectGraphStore, "Paper").await;
        assert_eq!(papers_in_graph.len(), 1);
        let sections =
            registered_label_nodes(store.as_ref() as &dyn DirectGraphStore, "Section").await;
        assert_eq!(sections.len(), 2);
        let versionless_prefix = "vid:section:2506.07853:";
        for node in &sections {
            let vid = store.get_node_property_string(*node, "vid").await.unwrap();
            assert!(
                !vid.starts_with(versionless_prefix),
                "versionless section VID {vid}"
            );
        }
        std::fs::remove_dir_all(cache_root).unwrap();
    }

    #[test]
    fn load_corpus_and_legacy_revision_adapters_agree_on_identity() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/article_catalog/article_catalog/arxiv/cs-ai/2506.07853");
        let pdf = root.join("source/2506.07853v5.pdf");
        let digest = sha256(&std::fs::read(&pdf).unwrap());
        let corpus = revision_binding_from_catalog("2506.07853", &digest, &root)
            .unwrap()
            .context;
        let legacy = crate::production::revision_context_for_versioned_pdf(&pdf, "2506.07853")
            .unwrap()
            .expect("legacy exact context");
        assert_eq!(corpus.revision, legacy.revision);
        assert_eq!(corpus.canonical_id, legacy.canonical_id);
        assert_eq!(
            corpus.revision_vid().unwrap(),
            legacy.revision_vid().unwrap()
        );
        assert_eq!(
            corpus.section_vid(0).unwrap(),
            legacy.section_vid(0).unwrap()
        );
    }

    #[test]
    fn versioned_source_paths_are_root_jailed_and_digest_bound() {
        let base = std::env::temp_dir().join(format!("da-revision-jail-{}", std::process::id()));
        let root = base.join("article");
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        let bytes = b"versioned-source";
        std::fs::write(source.join("2506.07853v1.pdf"), bytes).unwrap();
        let correct_sha = sha256(bytes);
        let metadata = serde_json::json!({
            "article_key": "2506.07853",
            "requested_source": {
                "revision": 1,
                "source_key": "2506.07853v1",
                "versioned": true
            },
            "source_variants": [{
                "byte_size": bytes.len() as u64,
                "capture_status": "captured_local",
                "path": "source/2506.07853v1.pdf",
                "requested_revision": 1,
                "sha256": correct_sha,
                "source_format": "pdf"
            }]
        });
        std::fs::write(
            root.join("article.v1.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        assert!(revision_binding_from_catalog("2506.07853", &correct_sha, &root).is_ok());

        let outside = base.join("outside.pdf");
        std::fs::write(&outside, bytes).unwrap();
        let escaped = serde_json::from_str::<serde_json::Value>(&metadata.to_string()).unwrap();
        let mut escaped = escaped;
        escaped["source_variants"][0]["path"] =
            serde_json::Value::String(outside.display().to_string());
        std::fs::write(
            root.join("article.v1.json"),
            serde_json::to_vec(&escaped).unwrap(),
        )
        .unwrap();
        let error = revision_binding_from_catalog("2506.07853", &correct_sha, &root).unwrap_err();
        assert!(
            error.to_string().contains("escapes article root"),
            "{error}"
        );

        let mut drifted = serde_json::from_str::<serde_json::Value>(&metadata.to_string()).unwrap();
        drifted["source_variants"][0]["sha256"] = serde_json::Value::String("b".repeat(64));
        std::fs::write(
            root.join("article.v1.json"),
            serde_json::to_vec(&drifted).unwrap(),
        )
        .unwrap();
        let error =
            revision_binding_from_catalog("2506.07853", &"b".repeat(64), &root).unwrap_err();
        assert!(error.to_string().contains("digest mismatch"), "{error}");
        std::fs::remove_dir_all(base).unwrap();
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
