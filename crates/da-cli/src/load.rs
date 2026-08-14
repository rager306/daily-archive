//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Single locator or `--batch-toc` (ensure and/or content into one hub snapshot).

use clap::Args;
use da_adapters::{
    ArxivSourceAcquisition, CatalogHtmlAcquisition, FdApiEmbedder, FilePipelineCache, GrobidParser,
    GrobidProfile, HtmlParser, RuleBasedExtractor, toc_unit_keys,
};
use da_application::{
    ArticleLoadUseCase, ExtractionUseCase, IngestUseCase, LoadContentOutcome, LoadContentPipeline,
    LoadContentRequest, LoadExtractOutcome, LoadIngestOutcome, RevisionClassifyUseCase,
    RevisionContentContext, RevisionManifestationInput,
};
use da_domain::{
    AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadAssetRecord, LoadCacheRecord, LoadStageSet,
    PrimaryContentKind, StageIdRegistry,
};
use da_ports::SourceAcquisitionPort;
use da_ports::embedder::Embedder;
use da_ports::extractor::{ExtractedEntity, Extractor};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{ParsedArticle, ParserPort};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageCacheKey, StageManifestEntry,
    StageOutcome,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static REPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const LEGACY_EMBEDDING_STAGE_ID: &str = "embed";
const LEGACY_EXTRACTION_STAGE_ID: &str = "extract";

#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Locator: arXiv id/URL, catalog:… pin, gnn-ch-* unit, or local path.
    /// Optional when `--batch-toc` is set.
    #[arg(required_unless_present = "batch_toc")]
    pub locator: Option<String>,

    /// Provider: auto | arxiv | gnn_textbook | textbook | catalog
    #[arg(long, default_value = "auto")]
    pub provider: String,

    /// Catalog root for ensure (typically data/article_catalog)
    #[arg(long, default_value = "data/article_catalog")]
    pub catalog_root: PathBuf,

    /// Authorize network fetch when assets missing
    #[arg(long, default_value_t = false)]
    pub allow_network: bool,

    /// Include TeX source archive when fetching arXiv
    #[arg(long, default_value_t = false)]
    pub include_tex_source: bool,

    /// Stage set: ensure | ingest | ingest+extract | full
    /// For `--batch-toc` without explicit stages, defaults to **ensure**.
    /// Pass `--stages ingest` or `ingest+extract` for hub content batch.
    #[arg(long)]
    pub stages: Option<String>,

    /// Dry-run: resolve only
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// D127 unlock (default locked)
    #[arg(long, default_value_t = false)]
    pub enable_import: bool,

    /// Write LoadReport JSON (single locator)
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Export .sgsnap after content stages (single) or hub snap for batch content
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Batch all units from a source-catalog TOC YAML
    #[arg(long)]
    pub batch_toc: Option<PathBuf>,

    /// Directory for per-unit batch load reports
    #[arg(long, default_value = "artifacts/loads/batch")]
    pub batch_report_dir: PathBuf,

    /// Pipeline cache root for parse-stage payloads (ADR-056 reuse of PipelineCache).
    #[arg(long, default_value = "artifacts/pipeline_cache")]
    pub cache_root: PathBuf,

    /// Bypass PipelineCache for parse stage (always re-parse).
    #[arg(long, default_value_t = false)]
    pub no_cache: bool,

    /// Explicit RFC3339 time for deterministic graph materialization/replay.
    /// When omitted, current wall clock is used.
    #[arg(long)]
    pub materialization_time: Option<String>,
}

/// Content pipeline with optional shared process-local store (hub batch).
struct CliContentPipeline {
    store: Arc<da_adapters::SamyamaGraphStore>,
    /// When false, materialize/extract only — caller exports hub snap once.
    export_per_unit: bool,
    /// Content-addressed parse cache (not graph truth).
    cache: Option<Arc<FilePipelineCache>>,
    /// One temporal basis shared by ingest and extraction for this load run.
    clock: Arc<dyn da_ports::Clock>,
    /// DATA-governed canonical identity for every emitted production stage.
    stage_registry: Arc<StageIdRegistry>,
}

#[async_trait::async_trait]
impl LoadContentPipeline for CliContentPipeline {
    async fn run_content(&self, req: LoadContentRequest<'_>) -> Result<LoadContentOutcome, String> {
        if !req.primary_path.is_file() {
            return Err(format!(
                "primary asset missing: {}",
                req.primary_path.display()
            ));
        }
        self.stage_registry.parse_canonical("parse")?;
        self.stage_registry.parse_canonical("embedding")?;
        if req.include_extract {
            self.stage_registry.parse_canonical("extraction_compute")?;
        }
        let primary_path = req.primary_path.display().to_string();
        let is_html = req
            .primary_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
        let grobid_profile = if is_html {
            GrobidProfile::rich()
        } else {
            GrobidParser::from_env().profile()
        };
        let source_identity =
            source_identity_for_assets(req.paper_id, req.primary_path, is_html, req.assets)?;

        let (parsed, mut cache_records, parse_payload_sha) = parse_with_cache(
            self.cache.as_deref(),
            req.paper_id,
            req.primary_path,
            is_html,
            source_identity.clone(),
            grobid_profile,
        )
        .await?;

        let embed_text = if parsed.abstract_text.is_empty() {
            parsed.title.as_str()
        } else {
            parsed.abstract_text.as_str()
        };
        let (vector, embed_records) = embed_with_cache(
            self.cache.as_deref(),
            source_identity.clone(),
            &parse_payload_sha,
            embed_text,
        )
        .await?;
        cache_records.extend(embed_records);

        let materializer = IngestUseCase::materializer_only_with_clock(
            self.store.clone() as Arc<dyn DirectGraphStore>,
            self.clock.clone(),
        );
        let materializer = if req.enable_import {
            materializer.with_import_eligibility(da_domain::ImportEligibility::Eligible)
        } else {
            materializer
        };

        let source_code =
            infer_source_code(&primary_path).or_else(|| Some(req.source_code.to_string()));
        let revision_context = revision_context_from_load(&req)?;
        let result = materializer
            .materialize_parsed_with_context(
                &parsed,
                vector,
                None,
                source_code,
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                revision_context.as_ref(),
            )
            .await
            .map_err(|e| format!("materialize: {e:#}"))?;

        let mut extract_out = None;
        if req.include_extract {
            let extractor = RuleBasedExtractor::new();
            let (extracted, extract_records) = extract_with_cache(
                self.cache.as_deref(),
                source_identity,
                &parse_payload_sha,
                &parsed,
                &extractor,
            )
            .await?;
            cache_records.extend(extract_records);
            let extract_uc = ExtractionUseCase::materializer_only_with_clock(
                self.store.clone() as Arc<dyn DirectGraphStore>,
                self.clock.clone(),
            );
            let er = extract_uc
                .materialize_extracted_with_context(&parsed, extracted, revision_context.as_ref())
                .await
                .map_err(|e| format!("extract materialize: {e:#}"))?;
            extract_out = Some(LoadExtractOutcome {
                entities_extracted: er.entities_extracted,
                entities_created: er.entities_created,
                mentions_edges: er.mentions_edges,
                found_in_edges: er.found_in_edges,
            });
        }

        let snapshot_path = if self.export_per_unit {
            if let Some(out) = req.snapshot_output {
                if let Some(parent) = out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let bytes = GraphStore::export_snapshot(self.store.as_ref())
                    .await
                    .map_err(|e| format!("export snapshot: {e}"))?;
                std::fs::write(out, &bytes)
                    .map_err(|e| format!("write snapshot {}: {e}", out.display()))?;
                Some(out.display().to_string())
            } else {
                None
            }
        } else {
            // Hub mode: defer export to batch end.
            None
        };

        Ok(LoadContentOutcome {
            ingest: LoadIngestOutcome {
                paper_id: result.paper_id,
                title: result.title,
                body_chars: result.body_chars,
                section_count: result.section_count,
                citation_count: result.citation_count,
                snapshot_path,
                revision_vid: result.revision_vid,
                import_eligible: result.import_eligible,
            },
            extract: extract_out,
            cache_records,
        })
    }
}

fn revision_context_from_load(
    req: &LoadContentRequest<'_>,
) -> Result<Option<RevisionContentContext>, String> {
    let mut manifestations = Vec::new();
    for asset in req.assets {
        if asset.kind != "pdf" && asset.kind != "html" {
            continue;
        }
        let sha256 = match asset.sha256.clone() {
            Some(sha) => sha,
            None if req.locator.arxiv_revision().is_some() => {
                return Err(format!(
                    "versioned {} asset lacks sha256: {}",
                    asset.kind, asset.path
                ));
            }
            None => continue,
        };
        let bytes = match asset.bytes {
            Some(n) => n,
            None if req.locator.arxiv_revision().is_some() => {
                return Err(format!(
                    "versioned {} asset lacks byte size: {}",
                    asset.kind, asset.path
                ));
            }
            None => continue,
        };
        manifestations.push(RevisionManifestationInput {
            format: asset.kind.clone(),
            sha256,
            bytes,
            uri: format!("catalog:{}", asset.path),
            path: asset.path.clone(),
            capture_status: "captured_local".into(),
        });
    }
    let classifier =
        RevisionClassifyUseCase::bundled().map_err(|e| format!("revision classification: {e}"))?;
    classifier
        .revision_context_for_classified_load(
            req.locator,
            req.source_code,
            req.paper_id,
            manifestations,
        )
        .map_err(|e| e.to_string())
}

/// Build source identity from every PDF/HTML manifestation known to the
/// catalog while keeping the selected parser input explicit. The primary file
/// is always read: its exact bytes remain authoritative for the selected hash.
fn source_identity_for_assets(
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
    assets: &[LoadAssetRecord],
) -> Result<SourceIdentity, String> {
    let bytes = std::fs::read(primary_path)
        .map_err(|error| format!("read primary {}: {error}", primary_path.display()))?;
    let primary_sha = sha256_hex(&bytes);
    let selected_kind = if is_html { "html" } else { "pdf" };

    let mut known: BTreeMap<&str, String> = BTreeMap::new();
    for asset in assets {
        let kind = if asset.kind.eq_ignore_ascii_case("pdf") {
            "pdf"
        } else if asset.kind.eq_ignore_ascii_case("html") {
            "html"
        } else {
            continue;
        };
        let Some(catalog_sha) = asset
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|sha| !sha.is_empty())
        else {
            continue;
        };
        match known.get(kind) {
            Some(existing) if existing != catalog_sha => {
                return Err(format!(
                    "conflicting {kind} manifestation digests for {}: {existing} != {catalog_sha}",
                    primary_path.display()
                ));
            }
            Some(_) => {}
            None => {
                known.insert(kind, catalog_sha.to_string());
            }
        }
    }

    if let Some(catalog_sha) = known.get(selected_kind)
        && catalog_sha != &primary_sha
    {
        return Err(format!(
            "selected {selected_kind} bytes differ from catalog digest for {}: expected {catalog_sha}, got {primary_sha}",
            primary_path.display()
        ));
    }

    let selected_sha = primary_sha.clone();
    let pdf_sha256 = if is_html {
        known.get("pdf").cloned()
    } else {
        Some(selected_sha.clone())
    };
    let html_sha256 = if is_html {
        Some(selected_sha.clone())
    } else {
        known.get("html").cloned()
    };
    let acquisition_fingerprint = format!(
        "acq-v2;pdf={};html={}",
        pdf_sha256.as_deref().unwrap_or("none"),
        html_sha256.as_deref().unwrap_or("none")
    );

    Ok(SourceIdentity {
        article_id: paper_id.to_string(),
        selected_format: if is_html {
            SourceFormat::Html
        } else {
            SourceFormat::Pdf
        },
        pdf_sha256,
        html_sha256,
        acquisition_fingerprint,
        normalization_fingerprint: "identity-v1".into(),
    })
}

fn retry_suppressed_error(
    stage_id: &str,
    resolved_from: Option<&str>,
    manifest: &StageManifestEntry,
) -> String {
    let retry_after = match &manifest.outcome {
        StageOutcome::RetryableFailure { retry_after, .. } => retry_after.as_str(),
        StageOutcome::Success { .. } => "unknown",
    };
    format!(
        "{stage_id} stage retry suppressed until {retry_after}{}",
        resolved_from
            .map(|alias| format!("; resolved_from={alias}"))
            .unwrap_or_default()
    )
}

fn lookup_with_legacy_stage_aliases(
    cache: &FilePipelineCache,
    canonical_key: &StageCacheKey,
    legacy_stage_ids: &[&str],
    now: &str,
) -> Result<(CacheLookup, Option<String>), String> {
    let canonical = cache
        .lookup(canonical_key, now)
        .map_err(|error| format!("{} cache lookup: {error}", canonical_key.stage_id))?;
    if !matches!(canonical, CacheLookup::Miss) {
        return Ok((canonical, None));
    }
    for legacy_stage_id in legacy_stage_ids {
        let mut legacy_key = canonical_key.clone();
        legacy_key.stage_id = (*legacy_stage_id).to_owned();
        let lookup = cache
            .lookup(&legacy_key, now)
            .map_err(|error| format!("legacy {legacy_stage_id} cache lookup: {error}"))?;
        if !matches!(lookup, CacheLookup::Miss) {
            return Ok((lookup, Some((*legacy_stage_id).to_owned())));
        }
    }
    Ok((CacheLookup::Miss, None))
}

fn parse_cache_key(source: SourceIdentity, is_html: bool, profile: GrobidProfile) -> StageCacheKey {
    let implementation_fingerprint = if is_html {
        "da_adapters::HtmlParser@load-v1"
    } else {
        "da_adapters::GrobidParser@load-v1"
    };
    let configuration_fingerprint = if is_html {
        "html-primary".to_string()
    } else {
        profile.configuration_fingerprint()
    };
    StageCacheKey {
        source,
        stage_id: "parse".into(),
        dependency_output_sha256: BTreeMap::new(),
        implementation_fingerprint: implementation_fingerprint.into(),
        configuration_fingerprint,
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: "ParsedArticle.v1".into(),
    }
}

fn selected_source_sha(source: &SourceIdentity) -> Option<String> {
    match source.selected_format {
        SourceFormat::Pdf => source.pdf_sha256.clone(),
        SourceFormat::Html => source.html_sha256.clone(),
        SourceFormat::MetadataOnly => None,
    }
}

/// Read the exact previous production parse key only for the semantics that
/// form actually had: rich PDF-only or HTML-only collapsed source identity.
/// Structural/custom profiles and dual-manifestation identities never reuse it.
fn lookup_with_legacy_parse_identity(
    cache: &FilePipelineCache,
    canonical_key: &StageCacheKey,
    is_html: bool,
    profile: GrobidProfile,
    now: &str,
) -> Result<(CacheLookup, Option<String>), String> {
    let legacy_configuration = if is_html {
        if canonical_key.source.pdf_sha256.is_some() {
            return Ok((CacheLookup::Miss, None));
        }
        "html-primary"
    } else {
        if profile != GrobidProfile::rich() || canonical_key.source.html_sha256.is_some() {
            return Ok((CacheLookup::Miss, None));
        }
        "pdf-grobid"
    };
    let Some(selected_sha) = selected_source_sha(&canonical_key.source) else {
        return Ok((CacheLookup::Miss, None));
    };
    let mut legacy_key = canonical_key.clone();
    legacy_key.source.pdf_sha256 = if is_html {
        None
    } else {
        Some(selected_sha.clone())
    };
    legacy_key.source.html_sha256 = if is_html {
        Some(selected_sha.clone())
    } else {
        None
    };
    legacy_key.source.acquisition_fingerprint = selected_sha;
    legacy_key.configuration_fingerprint = legacy_configuration.to_string();
    let lookup = cache
        .lookup(&legacy_key, now)
        .map_err(|error| format!("legacy {legacy_configuration} parse cache lookup: {error}"))?;
    if matches!(lookup, CacheLookup::Miss) {
        Ok((lookup, None))
    } else {
        Ok((lookup, Some(legacy_configuration.to_string())))
    }
}

/// Parse primary content with optional PipelineCache (derived payload only).
/// Returns (parsed, cache records, payload_sha256 for dependency keys).
async fn parse_with_cache(
    cache: Option<&FilePipelineCache>,
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
    source: SourceIdentity,
    profile: GrobidProfile,
) -> Result<(ParsedArticle, Vec<LoadCacheRecord>, String), String> {
    let primary_str = primary_path.display().to_string();
    let key = parse_cache_key(source, is_html, profile);

    // Async produce needs to be outside the sync cache helper: pre-run only on miss path
    // is handled by checking lookup first when cache is present.
    let key_sha = key
        .key_sha256()
        .map_err(|error| format!("parse cache key: {error}"))?;

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        let canonical_lookup = cache
            .lookup(&key, &now)
            .map_err(|e| format!("parse cache lookup: {e}"))?;
        let (lookup, resolved_from) = match canonical_lookup {
            CacheLookup::Miss => {
                lookup_with_legacy_parse_identity(cache, &key, is_html, profile, &now)?
            }
            lookup => (lookup, None),
        };
        match lookup {
            CacheLookup::Hit(hit) => {
                let parsed: ParsedArticle = serde_json::from_slice(&hit.payload)
                    .map_err(|e| format!("parse cache payload decode: {e}"))?;
                let payload_sha = sha256_hex(&hit.payload);
                return Ok((
                    parsed,
                    vec![LoadCacheRecord {
                        stage_id: "parse".into(),
                        resolved_from,
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(payload_sha.clone()),
                    }],
                    payload_sha,
                ));
            }
            CacheLookup::RetryableFailure(manifest) => {
                return Err(retry_suppressed_error(
                    "parse",
                    resolved_from.as_deref(),
                    &manifest,
                ));
            }
            CacheLookup::Miss => {
                let parsed = parse_primary(&primary_str, paper_id, is_html, profile).await?;
                let payload =
                    serde_json::to_vec(&parsed).map_err(|e| format!("serialize parse: {e}"))?;
                let payload_sha = sha256_hex(&payload);
                let entry = cache
                    .store_success(&key, &payload, BTreeMap::new(), &now)
                    .map_err(|e| format!("parse cache store: {e}"))?;
                return Ok((
                    parsed,
                    vec![LoadCacheRecord {
                        stage_id: "parse".into(),
                        resolved_from: None,
                        status: "miss".into(),
                        key_sha256: Some(entry.key_sha256),
                        payload_sha256: Some(payload_sha.clone()),
                    }],
                    payload_sha,
                ));
            }
        }
    }

    let parsed = parse_primary(&primary_str, paper_id, is_html, profile).await?;
    let payload = serde_json::to_vec(&parsed).map_err(|e| format!("serialize parse: {e}"))?;
    let payload_sha = sha256_hex(&payload);
    Ok((
        parsed,
        vec![LoadCacheRecord {
            stage_id: "parse".into(),
            resolved_from: None,
            status: "bypass".into(),
            key_sha256: Some(key_sha),
            payload_sha256: Some(payload_sha.clone()),
        }],
        payload_sha,
    ))
}

/// Embed abstract/title with PipelineCache. Depends on parse payload sha so a
/// re-parse with different bytes cannot silently reuse an old vector.
async fn embed_with_cache(
    cache: Option<&FilePipelineCache>,
    source: SourceIdentity,
    parse_payload_sha: &str,
    embed_text: &str,
) -> Result<(Vec<f32>, Vec<LoadCacheRecord>), String> {
    let embedder = FdApiEmbedder::from_env();
    let model_id = embedder.model_id().to_string();
    let text_sha = sha256_hex(embed_text.as_bytes());
    let mut deps = BTreeMap::new();
    deps.insert("parse".into(), parse_payload_sha.to_string());
    deps.insert("embed_text".into(), text_sha);
    let key = StageCacheKey {
        source,
        stage_id: "embedding".into(),
        dependency_output_sha256: deps,
        implementation_fingerprint: "da_adapters::FdApiEmbedder@load-v1".into(),
        configuration_fingerprint: format!("model={model_id};input=abstract_or_title"),
        prompt_fingerprint: None,
        model_fingerprint: Some(model_id),
        output_schema_fingerprint: "Vec<f32>.le-bytes.v1".into(),
    };
    let key_sha = key
        .key_sha256()
        .map_err(|error| format!("embedding cache key: {error}"))?;

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        let (lookup, resolved_from) =
            lookup_with_legacy_stage_aliases(cache, &key, &[LEGACY_EMBEDDING_STAGE_ID], &now)?;
        match lookup {
            CacheLookup::Hit(hit) => {
                let vector = decode_f32_le(&hit.payload, embedder.dimensions())?;
                return Ok((
                    vector,
                    vec![LoadCacheRecord {
                        stage_id: "embedding".into(),
                        resolved_from,
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(sha256_hex(&hit.payload)),
                    }],
                ));
            }
            CacheLookup::RetryableFailure(manifest) => {
                return Err(retry_suppressed_error(
                    "embedding",
                    resolved_from.as_deref(),
                    &manifest,
                ));
            }
            CacheLookup::Miss => {
                require_embed_credentials()?;
                let vector = embedder
                    .embed(embed_text)
                    .await
                    .map_err(|e| format!("embed: {e:#}"))?;
                let payload = encode_f32_le(&vector);
                let entry = cache
                    .store_success(&key, &payload, BTreeMap::new(), &now)
                    .map_err(|e| format!("embed cache store: {e}"))?;
                return Ok((
                    vector,
                    vec![LoadCacheRecord {
                        stage_id: "embedding".into(),
                        resolved_from: None,
                        status: "miss".into(),
                        key_sha256: Some(entry.key_sha256),
                        payload_sha256: Some(sha256_hex(&payload)),
                    }],
                ));
            }
        }
    }

    require_embed_credentials()?;
    let vector = embedder
        .embed(embed_text)
        .await
        .map_err(|e| format!("embed: {e:#}"))?;
    let payload = encode_f32_le(&vector);
    Ok((
        vector,
        vec![LoadCacheRecord {
            stage_id: "embedding".into(),
            resolved_from: None,
            status: "bypass".into(),
            key_sha256: Some(key_sha),
            payload_sha256: Some(sha256_hex(&payload)),
        }],
    ))
}

async fn extract_with_cache(
    cache: Option<&FilePipelineCache>,
    source: SourceIdentity,
    parse_payload_sha: &str,
    parsed: &ParsedArticle,
    extractor: &RuleBasedExtractor,
) -> Result<(Vec<ExtractedEntity>, Vec<LoadCacheRecord>), String> {
    let identity = extractor.cache_identity();
    let mut dependencies = BTreeMap::new();
    dependencies.insert("parse".into(), parse_payload_sha.to_string());
    let key = StageCacheKey {
        source,
        stage_id: "extraction_compute".into(),
        dependency_output_sha256: dependencies,
        implementation_fingerprint: identity.implementation_fingerprint,
        configuration_fingerprint: identity.configuration_fingerprint,
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: identity.output_schema_fingerprint,
    };
    let key_sha = key
        .key_sha256()
        .map_err(|error| format!("extraction_compute cache key: {error}"))?;

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        let (lookup, resolved_from) =
            lookup_with_legacy_stage_aliases(cache, &key, &[LEGACY_EXTRACTION_STAGE_ID], &now)?;
        match lookup {
            CacheLookup::Hit(hit) => {
                let extracted = da_ports::cache_payload::decode_extracted_entities(&hit.payload)
                    .map_err(|error| format!("extract cache payload contract: {error}"))?;
                return Ok((
                    extracted,
                    vec![LoadCacheRecord {
                        stage_id: "extraction_compute".into(),
                        resolved_from,
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(sha256_hex(&hit.payload)),
                    }],
                ));
            }
            CacheLookup::RetryableFailure(manifest) => {
                return Err(retry_suppressed_error(
                    "extraction_compute",
                    resolved_from.as_deref(),
                    &manifest,
                ));
            }
            CacheLookup::Miss => {
                let extracted = compute_extraction(parsed, extractor).await?;
                let payload = serde_json::to_vec(&extracted)
                    .map_err(|error| format!("serialize extraction: {error}"))?;
                let entry = cache
                    .store_success(&key, &payload, BTreeMap::new(), &now)
                    .map_err(|error| format!("extract cache store: {error}"))?;
                return Ok((
                    extracted,
                    vec![LoadCacheRecord {
                        stage_id: "extraction_compute".into(),
                        resolved_from: None,
                        status: "miss".into(),
                        key_sha256: Some(entry.key_sha256),
                        payload_sha256: Some(sha256_hex(&payload)),
                    }],
                ));
            }
        }
    }

    let extracted = compute_extraction(parsed, extractor).await?;
    let payload =
        serde_json::to_vec(&extracted).map_err(|error| format!("serialize extraction: {error}"))?;
    Ok((
        extracted,
        vec![LoadCacheRecord {
            stage_id: "extraction_compute".into(),
            resolved_from: None,
            status: "bypass".into(),
            key_sha256: Some(key_sha),
            payload_sha256: Some(sha256_hex(&payload)),
        }],
    ))
}

async fn compute_extraction(
    parsed: &ParsedArticle,
    extractor: &RuleBasedExtractor,
) -> Result<Vec<ExtractedEntity>, String> {
    let sections = parsed
        .sections
        .iter()
        .map(|section| (section.title.clone(), section.text.clone()))
        .collect::<Vec<_>>();
    extractor
        .extract(&sections)
        .await
        .map_err(|error| format!("extract compute: {error}"))
}

fn encode_f32_le(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn require_embed_credentials() -> Result<(), String> {
    if std::env::var("FD_API_KEY")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err("FD_API_KEY is not set (required for uncached ingest embeddings)".into());
    }
    Ok(())
}

fn decode_f32_le(bytes: &[u8], expected_dimensions: usize) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "embed cache payload length {} not multiple of 4",
            bytes.len()
        ));
    }
    let actual_dimensions = bytes.len() / 4;
    if actual_dimensions != expected_dimensions {
        return Err(format!(
            "embed cache vector dimension mismatch: expected {expected_dimensions}, got {actual_dimensions}"
        ));
    }
    let mut out = Vec::with_capacity(actual_dimensions);
    for chunk in bytes.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !value.is_finite() {
            return Err("embed cache vector contains non-finite value".into());
        }
        out.push(value);
    }
    Ok(out)
}

async fn parse_primary(
    primary_path: &str,
    paper_id: &str,
    is_html: bool,
    profile: GrobidProfile,
) -> Result<ParsedArticle, String> {
    if is_html {
        let parser = HtmlParser::new();
        parser
            .parse_html(primary_path, paper_id)
            .await
            .map_err(|e| format!("html parse: {e:#}"))
    } else {
        let parser = GrobidParser::from_env().with_profile(profile);
        if !parser.is_alive().await {
            return Err("GROBID is not reachable (start sidecar on :8070)".into());
        }
        parser
            .parse_pdf(primary_path, paper_id)
            .await
            .map_err(|e| format!("pdf parse: {e:#}"))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn make_content_pipeline(
    store: Arc<da_adapters::SamyamaGraphStore>,
    export_per_unit: bool,
    args: &LoadArgs,
) -> Result<CliContentPipeline, String> {
    let cache_root = (!args.no_cache).then_some(args.cache_root.as_path());
    let context = crate::production::ProductionContext::new(
        store,
        cache_root,
        args.materialization_time.as_deref(),
    )?;
    Ok(CliContentPipeline {
        store: context.store,
        export_per_unit,
        cache: context.cache,
        clock: context.clock,
        stage_registry: context.stage_registry,
    })
}

fn infer_source_code(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    for code in [
        "gnn_textbook",
        "textbook",
        "stanford",
        "openalex",
        "company_blog",
        "nature",
        "arxiv",
    ] {
        if lower.contains(code) {
            return Some(code.to_string());
        }
    }
    None
}

pub fn open_graph_store_for_load() -> da_adapters::SamyamaGraphStore {
    match da_adapters::SamyamaGraphStore::try_from_env() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("❌ graph store open failed: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_load(args: &LoadArgs) -> Result<(), String> {
    crate::production::parse_materialization_time(args.materialization_time.as_deref())?;
    if let Some(toc) = &args.batch_toc {
        return run_batch_toc(args, toc).await;
    }
    let locator = args
        .locator
        .as_deref()
        .ok_or_else(|| "locator required unless --batch-toc".to_string())?;
    run_single(args, locator).await
}

fn resolve_stages(args: &LoadArgs, batch: bool) -> Result<LoadStageSet, String> {
    match &args.stages {
        Some(s) => parse_stages(s),
        None if batch => Ok(LoadStageSet::Ensure),
        None => Ok(LoadStageSet::EnsureIngest), // single-locator product default
    }
}

async fn run_batch_toc(args: &LoadArgs, toc: &Path) -> Result<(), String> {
    let keys = toc_unit_keys(toc).map_err(|e| format!("TOC: {e}"))?;
    if keys.is_empty() {
        return Err(format!("TOC {} has no units", toc.display()));
    }
    let stages = resolve_stages(args, true)?;
    let policy = AcquisitionPolicy {
        allow_network: args.allow_network,
        include_html: true,
        include_tex_source: false,
        dry_run: args.dry_run,
        enable_import: false,
        stages,
        run_mode: da_domain::LoadRunMode::Inline,
    };

    let provider = if args.provider == "auto" || args.provider == "catalog" {
        "gnn_textbook".to_string()
    } else {
        args.provider.clone()
    };

    let acq = CatalogHtmlAcquisition::new(&args.catalog_root, &provider)
        .with_repo_root(".")
        .with_toc_path(toc);

    let content_mode = policy.stages.includes_ingest() && !policy.dry_run;
    let store = Arc::new(open_graph_store_for_load());
    let pipe = make_content_pipeline(store.clone(), false, args)?;
    let _ = std::fs::create_dir_all(&args.batch_report_dir);

    let mut ok_n = 0usize;
    let mut fail_n = 0usize;
    let mut hit_n = 0usize;
    let mut fetched_n = 0usize;
    let mut total_sections = 0usize;
    let mut total_entities = 0usize;

    println!(
        "batch-toc units={} provider={} stages={:?} allow_network={} hub_content={}",
        keys.len(),
        provider,
        policy.stages,
        policy.allow_network,
        content_mode
    );

    for (i, unit_key) in keys.iter().enumerate() {
        let locator = format!("catalog:{provider}/html/{unit_key}");
        let report = run_with_acq(
            &acq,
            &args.catalog_root,
            &locator,
            &policy,
            if content_mode { Some(&pipe) } else { None },
            None, // no per-unit snap
        )
        .await;
        let status = if report.ok { "ok" } else { "FAIL" };
        if report.ok {
            ok_n += 1;
        } else {
            fail_n += 1;
        }
        if report.stages.iter().any(|s| s.status == "catalog_hit") {
            hit_n += 1;
        }
        if report.stages.iter().any(|s| s.status == "fetched") {
            fetched_n += 1;
        }
        for f in &report.findings {
            if f.code == "extract_ok" {
                total_entities += parse_tagged_usize(&f.message, "entities_extracted=");
            }
        }
        for s in &report.stages {
            if s.stage == da_domain::LoadStageName::Ingest
                && s.status == "ok"
                && let Some(d) = &s.detail
            {
                total_sections += parse_tagged_usize(d, "sections=");
            }
        }
        println!(
            "[{}/{}] {} {} primary={:?}",
            i + 1,
            keys.len(),
            status,
            unit_key,
            report.primary_kind
        );
        let rep_path = args.batch_report_dir.join(format!("{unit_key}.json"));
        write_json_atomic(&rep_path, &report)
            .map_err(|e| format!("persist batch report for {unit_key}: {e}"))?;
        if !report.ok {
            for e in &report.errors {
                eprintln!("    error: {e}");
            }
        }
    }

    let mut hub_path = None;
    if content_mode && fail_n == 0 {
        let out = args.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!("artifacts/article_context/{provider}_hub.sgsnap"))
        });
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let bytes = GraphStore::export_snapshot(store.as_ref())
            .await
            .map_err(|e| format!("hub export snapshot: {e}"))?;
        std::fs::write(&out, &bytes).map_err(|e| format!("write hub {}: {e}", out.display()))?;
        hub_path = Some(out.display().to_string());
        println!(
            "hub snapshot={} bytes={} sections_sum≈{} entities_sum≈{}",
            out.display(),
            bytes.len(),
            total_sections,
            total_entities
        );
    }

    println!(
        "batch done ok={ok_n} fail={fail_n} catalog_hit≈{hit_n} fetched≈{fetched_n} hub={:?} reports={}",
        hub_path,
        args.batch_report_dir.display()
    );
    if fail_n > 0 {
        return Err(format!("batch-toc: {fail_n} unit(s) failed"));
    }
    Ok(())
}

async fn run_single(args: &LoadArgs, locator: &str) -> Result<(), String> {
    let stages = resolve_stages(args, false)?;
    let policy = AcquisitionPolicy {
        allow_network: args.allow_network,
        include_html: true,
        include_tex_source: args.include_tex_source,
        dry_run: args.dry_run,
        enable_import: args.enable_import,
        stages,
        run_mode: da_domain::LoadRunMode::Inline,
    };

    let provider = resolve_provider(&args.provider, locator)?;
    let snap = args
        .output
        .clone()
        .or_else(|| default_snapshot_path(locator));
    let snap_ref = snap.as_deref();
    let store = Arc::new(open_graph_store_for_load());
    let pipe = make_content_pipeline(store, true, args)?;

    let mut report = match provider.as_str() {
        "arxiv" => {
            let acq = ArxivSourceAcquisition::new(&args.catalog_root).with_repo_root(".");
            run_with_acq(
                &acq,
                &args.catalog_root,
                locator,
                &policy,
                if policy.stages.includes_ingest() && !policy.dry_run {
                    Some(&pipe)
                } else {
                    None
                },
                snap_ref,
            )
            .await
        }
        "gnn_textbook" | "textbook" | "stanford" | "company_blog" | "nature" => {
            let acq =
                CatalogHtmlAcquisition::new(&args.catalog_root, &provider).with_repo_root(".");
            run_with_acq(
                &acq,
                &args.catalog_root,
                locator,
                &policy,
                if policy.stages.includes_ingest() && !policy.dry_run {
                    Some(&pipe)
                } else {
                    None
                },
                snap_ref,
            )
            .await
        }
        other => {
            return Err(format!(
                "provider {other:?} not wired; use auto|arxiv|gnn_textbook|textbook|stanford"
            ));
        }
    };

    if let Some(time) =
        crate::production::parse_materialization_time(args.materialization_time.as_deref())?
    {
        report.findings.push(da_domain::LoadFinding {
            code: "materialization_time_explicit".into(),
            message: format!("graph temporal basis={}", time.to_rfc3339()),
        });
    }
    if let Err(error) = enrich_snapshot_identity(&mut report) {
        report.ok = false;
        report.errors.push(error);
    }
    report.seal_digest();

    println!(
        "load schema={} ok={} import_eligible={} provider={} source_code={}",
        report.schema_version,
        report.ok,
        report.import_eligible,
        report.provider,
        report.source_code
    );
    if let Some(uk) = &report.unit_key {
        println!("  unit_key={uk}");
    }
    if let Some(pk) = &report.primary_kind {
        println!("  primary_kind={pk:?}");
    }
    if let Some(sp) = &report.snapshot_path {
        println!("  snapshot={sp}");
    }
    for s in &report.stages {
        println!(
            "  stage {:?}: {}{}",
            s.stage,
            s.status,
            s.detail
                .as_ref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default()
        );
    }
    for e in &report.errors {
        eprintln!("  error: {e}");
    }

    if let Some(path) = &args.report {
        write_json_atomic(path, &report)?;
        println!("  report: {}", path.display());
    }

    assert_eq!(report.schema_version, LOAD_REPORT_SCHEMA_VERSION);
    let _ = PrimaryContentKind::Pdf;
    if !report.ok {
        return Err("load failed (see report/errors)".into());
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create report directory {}: {e}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("serialize report {}: {e}", path.display()))?;
    let sequence = REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));

    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("create temporary report {}: {e}", tmp.display()))?;
        file.write_all(&bytes)
            .map_err(|e| format!("write temporary report {}: {e}", tmp.display()))?;
        file.sync_all()
            .map_err(|e| format!("sync temporary report {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            format!(
                "replace report {} from {}: {e}",
                path.display(),
                tmp.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

async fn run_with_acq<A: SourceAcquisitionPort + ?Sized>(
    acq: &A,
    catalog_root: &Path,
    locator: &str,
    policy: &AcquisitionPolicy,
    pipe: Option<&CliContentPipeline>,
    snap: Option<&Path>,
) -> da_domain::LoadReport {
    let uc = ArticleLoadUseCase::new(acq).with_catalog_root(catalog_root);
    if policy.stages.includes_ingest() && !policy.dry_run {
        if let Some(p) = pipe {
            uc.run_with_content(locator, policy, Some(p), snap).await
        } else {
            uc.run(locator, policy)
        }
    } else {
        uc.run(locator, policy)
    }
}

fn enrich_snapshot_identity(report: &mut da_domain::LoadReport) -> Result<(), String> {
    let Some(path) = report.snapshot_path.as_deref() else {
        return Ok(());
    };
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read exported snapshot {path} for identity: {error}"))?;
    let identity = kg_storage::fingerprint_snapshot(&bytes)
        .map_err(|error| format!("fingerprint exported snapshot {path}: {error}"))?;
    report.snapshot_sha256 = Some(identity.physical_sha256);
    report.snapshot_semantic_sha256 = Some(identity.semantic_sha256.clone());
    report.snapshot_semantic_algorithm = Some(identity.semantic_algorithm.clone());
    report.snapshot_node_count = Some(identity.node_count);
    report.snapshot_edge_count = Some(identity.edge_count);
    report.findings.push(da_domain::LoadFinding {
        code: "snapshot_identity_ok".into(),
        message: format!(
            "semantic_sha256={} algorithm={} nodes={} edges={} physical_sha256=present",
            identity.semantic_sha256,
            identity.semantic_algorithm,
            identity.node_count,
            identity.edge_count
        ),
    });
    Ok(())
}

fn default_snapshot_path(locator: &str) -> Option<PathBuf> {
    let source_key = da_domain::ArticleLocator::parse(locator)
        .ok()
        .map(|locator| locator.source_key())?;
    let safe = source_key.replace('/', "_");
    Some(PathBuf::from(format!(
        "artifacts/article_context/paper_{safe}.sgsnap"
    )))
}

fn parse_tagged_usize(text: &str, tag: &str) -> usize {
    text.split(tag)
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|num| num.parse().ok())
        .unwrap_or(0)
}

fn parse_stages(s: &str) -> Result<LoadStageSet, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "ensure" => Ok(LoadStageSet::Ensure),
        "ingest" | "ensure+ingest" | "ensure_ingest" => Ok(LoadStageSet::EnsureIngest),
        "ingest+extract" | "ensure+ingest+extract" | "full" => {
            Ok(LoadStageSet::EnsureIngestExtract)
        }
        other => Err(format!(
            "unknown --stages {other:?}; use ensure|ingest|ingest+extract|full"
        )),
    }
}

fn resolve_provider(hint: &str, locator: &str) -> Result<String, String> {
    let h = hint.trim().to_ascii_lowercase();
    if h != "auto" && h != "catalog" {
        return Ok(h);
    }
    let loc = da_domain::ArticleLocator::parse(locator).map_err(|e| e.to_string())?;
    match loc {
        da_domain::ArticleLocator::Arxiv { .. } => Ok("arxiv".into()),
        da_domain::ArticleLocator::Catalog { source_code, .. } => Ok(source_code),
        da_domain::ArticleLocator::Local { path } => {
            if path.ends_with(".html") || path.ends_with(".htm") {
                Ok("gnn_textbook".into())
            } else {
                Ok("arxiv".into())
            }
        }
        da_domain::ArticleLocator::Raw { value } => {
            if value.starts_with("gnn-ch-") || value.contains("gnn_textbook") {
                Ok("gnn_textbook".into())
            } else if h == "arxiv" {
                Ok("arxiv".into())
            } else {
                Err(format!(
                    "auto provider could not select adapter for locator {value:?}"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_identity_populates_dual_fields_and_reseals_digest() {
        let mut report = da_domain::LoadReport::new_skeleton(
            "2506.07853",
            da_domain::ArticleLocator::parse("2506.07853").unwrap(),
        );
        report.snapshot_path = Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../artifacts/article_context/revision_graph_2506.07853.sgsnap")
                .display()
                .to_string(),
        );
        enrich_snapshot_identity(&mut report).unwrap();
        report.seal_digest();
        assert_eq!(report.snapshot_sha256.as_deref().unwrap().len(), 64);
        assert_eq!(
            report.snapshot_semantic_sha256.as_deref(),
            Some("86cb28372eedc3a46c0dc547eaff3730b2c7ca42d3c12ba6815cc0dfdd83d9cb")
        );
        assert_eq!(report.snapshot_node_count, Some(33));
        assert_eq!(report.snapshot_edge_count, Some(33));
        assert_eq!(
            report.report_digest_sha256.as_deref(),
            Some(report.compute_digest().as_str())
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "snapshot_identity_ok")
        );
    }

    #[test]
    fn invalid_exported_snapshot_identity_is_visible() {
        let root = std::env::temp_dir().join(format!(
            "da-invalid-snapshot-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&root, b"not-a-snapshot").unwrap();
        let mut report = da_domain::LoadReport::new_skeleton(
            "2506.07853",
            da_domain::ArticleLocator::parse("2506.07853").unwrap(),
        );
        report.snapshot_path = Some(root.display().to_string());
        let error = enrich_snapshot_identity(&mut report).unwrap_err();
        assert!(error.contains("fingerprint exported snapshot"), "{error}");
        assert!(report.snapshot_sha256.is_none());
        std::fs::remove_file(root).unwrap();
    }

    #[test]
    fn parse_stages_shapes() {
        assert_eq!(parse_stages("ingest").unwrap(), LoadStageSet::EnsureIngest);
        assert_eq!(
            parse_stages("ingest+extract").unwrap(),
            LoadStageSet::EnsureIngestExtract
        );
    }

    #[test]
    fn resolve_provider_arxiv_and_textbook() {
        assert_eq!(
            resolve_provider("auto", "https://arxiv.org/html/2608.07067v1").unwrap(),
            "arxiv"
        );
        assert_eq!(
            resolve_provider(
                "auto",
                "catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs"
            )
            .unwrap(),
            "gnn_textbook"
        );
    }

    #[test]
    fn encode_decode_f32_le_roundtrip() {
        let v = vec![0.0_f32, 1.5, -2.25, 1024.0];
        let bytes = encode_f32_le(&v);
        assert_eq!(bytes.len(), 16);
        assert_eq!(decode_f32_le(&bytes, 4).unwrap(), v);
        assert!(decode_f32_le(&[1, 2, 3], 1).is_err());
        assert!(
            decode_f32_le(&bytes, 3)
                .unwrap_err()
                .contains("dimension mismatch")
        );
        let nan = encode_f32_le(&[f32::NAN]);
        assert!(decode_f32_le(&nan, 1).unwrap_err().contains("non-finite"));
    }

    fn asset(kind: &str, path: &str, sha256: Option<String>) -> LoadAssetRecord {
        LoadAssetRecord {
            kind: kind.into(),
            path: path.into(),
            sha256,
            bytes: Some(1),
            disposition: da_domain::AssetDisposition::CatalogHit,
        }
    }

    #[test]
    fn source_identity_binds_all_available_manifestations_and_fails_closed() {
        let root = std::env::temp_dir().join(format!(
            "da-source-identity-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let pdf = root.join("paper.pdf");
        std::fs::write(&pdf, b"primary-pdf").unwrap();
        let pdf_sha = sha256_hex(b"primary-pdf");
        let html_a = "c".repeat(64);
        let html_b = "d".repeat(64);

        let pdf_only = source_identity_for_assets("test.paper", &pdf, false, &[]).unwrap();
        assert_eq!(pdf_only.selected_format, SourceFormat::Pdf);
        assert_eq!(pdf_only.pdf_sha256.as_deref(), Some(pdf_sha.as_str()));
        assert_eq!(pdf_only.html_sha256, None);
        assert!(pdf_only.acquisition_fingerprint.contains("html=none"));

        let assets = vec![
            asset("pdf", "catalog/paper.pdf", Some(pdf_sha.clone())),
            asset("html", "catalog/paper.html", Some(html_a.clone())),
        ];
        let dual = source_identity_for_assets("test.paper", &pdf, false, &assets).unwrap();
        assert_eq!(dual.pdf_sha256.as_deref(), Some(pdf_sha.as_str()));
        assert_eq!(dual.html_sha256.as_deref(), Some(html_a.as_str()));
        assert!(
            dual.acquisition_fingerprint
                .contains(&format!("html={html_a}"))
        );

        let changed_html = vec![asset("html", "catalog/paper.html", Some(html_b.clone()))];
        let changed = source_identity_for_assets("test.paper", &pdf, false, &changed_html).unwrap();
        assert_ne!(dual, changed);
        assert_ne!(
            dual.acquisition_fingerprint,
            changed.acquisition_fingerprint
        );

        let conflicting = vec![
            asset("pdf", "catalog/one.pdf", Some(pdf_sha.clone())),
            asset("pdf", "catalog/two.pdf", Some(html_b)),
        ];
        let error =
            source_identity_for_assets("test.paper", &pdf, false, &conflicting).unwrap_err();
        assert!(error.contains("conflicting pdf manifestation digests"));

        let wrong_selected = vec![asset("pdf", "catalog/paper.pdf", Some("e".repeat(64)))];
        let error =
            source_identity_for_assets("test.paper", &pdf, false, &wrong_selected).unwrap_err();
        assert!(error.contains("differ from catalog digest"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parse_cache_keys_bind_grobid_profile_semantics() {
        let root = std::env::temp_dir().join(format!(
            "da-parse-profile-key-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let pdf = root.join("paper.pdf");
        std::fs::write(&pdf, b"profile-key").unwrap();
        let source = source_identity_for_assets("test.paper", &pdf, false, &[]).unwrap();
        let rich = parse_cache_key(source.clone(), false, GrobidProfile::rich());
        let rich_again = parse_cache_key(source.clone(), false, GrobidProfile::rich());
        let structural = parse_cache_key(source, false, GrobidProfile::structural());
        assert_eq!(
            rich.key_sha256().unwrap(),
            rich_again.key_sha256().unwrap(),
            "identical effective rich inputs must remain stable"
        );
        assert_ne!(
            rich.key_sha256().unwrap(),
            structural.key_sha256().unwrap(),
            "rich and structural profiles must never share a parse key"
        );
        assert!(rich.configuration_fingerprint.contains("profile=rich"));
        assert!(
            structural
                .configuration_fingerprint
                .contains("profile=structural")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn parse_reads_exact_legacy_rich_pdf_only_key_without_canonical_write() {
        let root = std::env::temp_dir().join(format!(
            "da-legacy-parse-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let pdf = root.join("paper.pdf");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&pdf, b"legacy-pdf").unwrap();
        let content_sha = sha256_hex(b"legacy-pdf");
        let mut legacy_source = source_identity_for_assets("test.paper", &pdf, false, &[]).unwrap();
        legacy_source.acquisition_fingerprint = content_sha;
        let mut legacy_key = parse_cache_key(legacy_source, false, GrobidProfile::rich());
        legacy_key.configuration_fingerprint = "pdf-grobid".into();
        let parsed = ParsedArticle {
            paper_id: "test.paper".into(),
            title: "Legacy parse".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        };
        let payload = serde_json::to_vec(&parsed).unwrap();
        let cache = FilePipelineCache::new(root.join("cache"));
        let entry = cache
            .store_success(
                &legacy_key,
                &payload,
                BTreeMap::new(),
                "2026-08-14T00:00:00Z",
            )
            .unwrap();
        let (cached, records, payload_sha) = parse_with_cache(
            Some(&cache),
            "test.paper",
            &pdf,
            false,
            source_identity_for_assets("test.paper", &pdf, false, &[]).unwrap(),
            GrobidProfile::rich(),
        )
        .await
        .unwrap();
        assert_eq!(serde_json::to_vec(&cached).unwrap(), payload);
        assert_eq!(payload_sha, sha256_hex(&payload));
        assert_eq!(records[0].stage_id, "parse");
        assert_eq!(records[0].status, "hit");
        assert_eq!(records[0].resolved_from.as_deref(), Some("pdf-grobid"));
        assert_eq!(
            records[0].key_sha256.as_deref(),
            Some(entry.key_sha256.as_str())
        );
        assert_eq!(cache.manifest("test.paper").unwrap().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_parse_fallback_excludes_structural_and_dual_manifestations() {
        let root = std::env::temp_dir().join(format!(
            "da-parse-fallback-boundary-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let pdf = root.join("paper.pdf");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&pdf, b"boundary").unwrap();
        let pdf_sha = sha256_hex(b"boundary");
        let dual_assets = vec![asset("html", "catalog/paper.html", Some("c".repeat(64)))];
        let dual = source_identity_for_assets("test.paper", &pdf, false, &dual_assets).unwrap();
        assert_eq!(dual.pdf_sha256.as_deref(), Some(pdf_sha.as_str()));
        let cache = FilePipelineCache::new(root.join("cache"));
        let now = "2026-08-14T00:00:00Z";
        let structural_key = parse_cache_key(dual.clone(), false, GrobidProfile::structural());
        let (lookup, resolved) = lookup_with_legacy_parse_identity(
            &cache,
            &structural_key,
            false,
            GrobidProfile::structural(),
            now,
        )
        .unwrap();
        assert!(matches!(lookup, CacheLookup::Miss));
        assert_eq!(resolved, None);

        let rich_dual_key = parse_cache_key(dual, false, GrobidProfile::rich());
        let (lookup, resolved) = lookup_with_legacy_parse_identity(
            &cache,
            &rich_dual_key,
            false,
            GrobidProfile::rich(),
            now,
        )
        .unwrap();
        assert!(matches!(lookup, CacheLookup::Miss));
        assert_eq!(resolved, None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn extraction_compute_reads_legacy_extract_cache_namespace() {
        let root = std::env::temp_dir().join(format!(
            "da-legacy-extract-cache-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("article.html");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &source,
            b"<!doctype html><title>legacy cache fixture</title>",
        )
        .unwrap();
        let parsed = ParsedArticle {
            paper_id: "test.paper".into(),
            title: "Legacy cache fixture".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        };
        let extractor = RuleBasedExtractor::with_config(
            da_adapters::rule_extractor::ExtractionConfig::bundled(),
        );
        let identity = extractor.cache_identity();
        let mut dependencies = BTreeMap::new();
        dependencies.insert("parse".into(), "b".repeat(64));
        let legacy_key = StageCacheKey {
            source: source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            stage_id: LEGACY_EXTRACTION_STAGE_ID.into(),
            dependency_output_sha256: dependencies,
            implementation_fingerprint: identity.implementation_fingerprint,
            configuration_fingerprint: identity.configuration_fingerprint,
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: identity.output_schema_fingerprint,
        };
        let cache = FilePipelineCache::new(root.join("cache"));
        let payload = serde_json::to_vec(&Vec::<ExtractedEntity>::new()).unwrap();
        cache
            .store_success(
                &legacy_key,
                &payload,
                BTreeMap::new(),
                "2026-08-14T00:00:00Z",
            )
            .unwrap();
        let (entities, records) = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap();
        assert!(entities.is_empty());
        assert_eq!(records[0].stage_id, "extraction_compute");
        assert_eq!(records[0].resolved_from.as_deref(), Some("extract"));
        assert_eq!(records[0].status, "hit");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn legacy_retryable_failure_suppresses_extraction_without_canonical_write() {
        let root = std::env::temp_dir().join(format!(
            "da-legacy-extract-failure-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("article.html");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, b"<!doctype html><title>retry fixture</title>").unwrap();
        let parsed = ParsedArticle {
            paper_id: "test.paper".into(),
            title: "Retry fixture".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        };
        let extractor = RuleBasedExtractor::with_config(
            da_adapters::rule_extractor::ExtractionConfig::bundled(),
        );
        let identity = extractor.cache_identity();
        let mut dependencies = BTreeMap::new();
        dependencies.insert("parse".into(), "b".repeat(64));
        let legacy_key = StageCacheKey {
            source: source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            stage_id: LEGACY_EXTRACTION_STAGE_ID.into(),
            dependency_output_sha256: dependencies,
            implementation_fingerprint: identity.implementation_fingerprint,
            configuration_fingerprint: identity.configuration_fingerprint,
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: identity.output_schema_fingerprint,
        };
        let cache = FilePipelineCache::new(root.join("cache"));
        cache
            .store_retryable_failure(
                &legacy_key,
                "sidecar_unavailable",
                "retry later",
                "2099-01-01T00:00:00Z",
                "2026-08-14T00:00:00Z",
            )
            .unwrap();
        let error = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap_err();
        assert!(error.contains("retry suppressed until 2099-01-01T00:00:00Z"));
        assert!(error.contains("resolved_from=extract"));
        let manifests = cache.manifest("test.paper").unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].key.stage_id, LEGACY_EXTRACTION_STAGE_ID);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn same_key_extract_payload_missing_grounding_is_never_a_hit() {
        let root = std::env::temp_dir().join(format!(
            "da-extract-schema-fence-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("article.html");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, b"<!doctype html><title>schema fence</title>").unwrap();
        let parsed = ParsedArticle {
            paper_id: "test.paper".into(),
            title: "Schema fence".into(),
            abstract_text: String::new(),
            body_text: String::new(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        };
        let extractor = RuleBasedExtractor::with_config(
            da_adapters::rule_extractor::ExtractionConfig::bundled(),
        );
        let identity = extractor.cache_identity();
        let mut dependencies = BTreeMap::new();
        dependencies.insert("parse".into(), "b".repeat(64));
        let key = StageCacheKey {
            source: source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            stage_id: "extraction_compute".into(),
            dependency_output_sha256: dependencies,
            implementation_fingerprint: identity.implementation_fingerprint,
            configuration_fingerprint: identity.configuration_fingerprint,
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: identity.output_schema_fingerprint,
        };
        let legacy_payload = serde_json::json!([{
            "label": "GEPA",
            "entity_type": "Method",
            "section_title": "Methods",
            "char_start": 12,
            "char_end": 16,
            "surface": "GEPA"
        }]);
        let payload = serde_json::to_vec(&legacy_payload).unwrap();
        let cache = FilePipelineCache::new(root.join("cache"));
        cache
            .store_success(&key, &payload, BTreeMap::new(), "2026-08-14T00:00:00Z")
            .unwrap();
        let error = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap_err();
        assert!(error.contains("incompatible"), "{error}");
        assert!(error.contains("section_ordinal"), "{error}");
        assert_eq!(cache.manifest("test.paper").unwrap().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn extract_cache_miss_hit_bypass_and_corrupt_payload() {
        let root = std::env::temp_dir().join(format!(
            "da-extract-cache-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("article.html");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, b"<!doctype html><title>cache fixture</title>").unwrap();
        let parsed = ParsedArticle {
            paper_id: "test.paper".into(),
            title: "Cache fixture".into(),
            abstract_text: String::new(),
            body_text: "GEPA improves optimization".into(),
            sections: vec![da_ports::parser::Section {
                title: "Methods".into(),
                text: "GEPA improves optimization".into(),
                level: 1,
                ..Default::default()
            }],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        };
        let extractor = RuleBasedExtractor::with_config(
            da_adapters::rule_extractor::ExtractionConfig::bundled(),
        );
        let cache = FilePipelineCache::new(root.join("cache"));
        let (cold, cold_records) = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap();
        assert_eq!(cold_records[0].status, "miss");
        assert!(!cold.is_empty());
        let (hit, hit_records) = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap();
        assert_eq!(hit_records[0].status, "hit");
        assert_eq!(hit, cold);
        assert_eq!(
            hit_records[0].payload_sha256,
            cold_records[0].payload_sha256
        );
        let (_bypass, bypass_records) = extract_with_cache(
            None,
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap();
        assert_eq!(bypass_records[0].status, "bypass");

        let payload = walk_files(cache.root())
            .into_iter()
            .find(|path| path.file_name().is_some_and(|name| name == "payload.bin"))
            .unwrap();
        std::fs::write(payload, b"not-json").unwrap();
        let error = extract_with_cache(
            Some(&cache),
            source_identity_for_assets("test.paper", &source, true, &[]).unwrap(),
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("extraction_compute cache lookup")
                && (error.contains("checksum mismatch") || error.contains("payload decode")),
            "{error}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn walk_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    stack.extend(entries.filter_map(Result::ok).map(|entry| entry.path()));
                }
            } else {
                files.push(path);
            }
        }
        files
    }

    #[test]
    fn default_snapshot_path_keeps_requested_arxiv_revision() {
        assert_eq!(
            default_snapshot_path("https://arxiv.org/html/2506.07853v1"),
            Some(PathBuf::from(
                "artifacts/article_context/paper_2506.07853v1.sgsnap"
            ))
        );
        assert_eq!(
            default_snapshot_path("2506.07853"),
            Some(PathBuf::from(
                "artifacts/article_context/paper_2506.07853.sgsnap"
            ))
        );
    }

    #[test]
    fn batch_report_atomic_create_and_replace() {
        let root = std::env::temp_dir().join(format!(
            "da-load-report-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("nested/unit.json");
        write_json_atomic(&path, &serde_json::json!({"run": 1})).unwrap();
        write_json_atomic(&path, &serde_json::json!({"run": 2})).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["run"], 2);
        let leftovers = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(leftovers, 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_report_failure_is_explicit_and_preserves_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "da-load-report-fail-{}-{}",
            std::process::id(),
            REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let blocker = root.join("not-a-directory");
        std::fs::write(&blocker, b"preserve-me").unwrap();
        let target = blocker.join("unit.json");
        let err = write_json_atomic(&target, &serde_json::json!({"run": 1})).unwrap_err();
        assert!(err.contains("create report directory"), "{err}");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"preserve-me");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_default_stages_ensure() {
        let args = LoadArgs {
            locator: None,
            provider: "auto".into(),
            catalog_root: PathBuf::from("data/article_catalog"),
            allow_network: false,
            include_tex_source: false,
            stages: None,
            dry_run: false,
            enable_import: false,
            report: None,
            output: None,
            batch_toc: Some(PathBuf::from("toc.yaml")),
            batch_report_dir: PathBuf::from("artifacts/loads/batch"),
            cache_root: PathBuf::from("artifacts/pipeline_cache"),
            no_cache: false,
            materialization_time: None,
        };
        assert_eq!(resolve_stages(&args, true).unwrap(), LoadStageSet::Ensure);
        assert_eq!(
            resolve_stages(&args, false).unwrap(),
            LoadStageSet::EnsureIngest
        );

        let mut fixed = args;
        fixed.materialization_time = Some("2023-11-14T22:13:20Z".into());
        let pipeline = make_content_pipeline(
            Arc::new(da_adapters::SamyamaGraphStore::new()),
            true,
            &fixed,
        )
        .unwrap();
        assert_eq!(
            pipeline.clock.now(),
            chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap()
        );
        fixed.materialization_time = Some("not-a-time".into());
        let error = make_content_pipeline(
            Arc::new(da_adapters::SamyamaGraphStore::new()),
            true,
            &fixed,
        )
        .err()
        .unwrap();
        assert!(error.contains("invalid --materialization-time"), "{error}");
    }
}
