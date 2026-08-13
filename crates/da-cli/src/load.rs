//! `da load` — composition root for universal article load plane (ADR-056 / D220).
//!
//! Single locator or `--batch-toc` (ensure and/or content into one hub snapshot).

use clap::Args;
use da_adapters::{
    ArxivSourceAcquisition, CatalogHtmlAcquisition, FdApiEmbedder, FilePipelineCache, GrobidParser,
    HtmlParser, RuleBasedExtractor, toc_unit_keys,
};
use da_application::{
    ArticleLoadUseCase, ExtractionUseCase, IngestUseCase, LoadContentOutcome, LoadContentPipeline,
    LoadContentRequest, LoadExtractOutcome, LoadIngestOutcome, RevisionClassifyUseCase,
    RevisionContentContext, RevisionManifestationInput,
};
use da_domain::{
    AcquisitionPolicy, LOAD_REPORT_SCHEMA_VERSION, LoadCacheRecord, LoadStageSet,
    PrimaryContentKind,
};
use da_ports::SourceAcquisitionPort;
use da_ports::embedder::Embedder;
use da_ports::extractor::{ExtractedEntity, Extractor};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{ParsedArticle, ParserPort};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageCacheKey,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static REPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
        let primary_path = req.primary_path.display().to_string();
        let is_html = req
            .primary_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));

        let (parsed, mut cache_records, parse_payload_sha) = parse_with_cache(
            self.cache.as_deref(),
            req.paper_id,
            req.primary_path,
            is_html,
        )
        .await?;

        let embed_text = if parsed.abstract_text.is_empty() {
            parsed.title.as_str()
        } else {
            parsed.abstract_text.as_str()
        };
        let (vector, embed_records) = embed_with_cache(
            self.cache.as_deref(),
            req.paper_id,
            req.primary_path,
            is_html,
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
                req.paper_id,
                req.primary_path,
                is_html,
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

fn source_identity_for_primary(
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
) -> Result<SourceIdentity, String> {
    let bytes = std::fs::read(primary_path)
        .map_err(|e| format!("read primary {}: {e}", primary_path.display()))?;
    let content_sha = sha256_hex(&bytes);
    let format = if is_html {
        SourceFormat::Html
    } else {
        SourceFormat::Pdf
    };
    Ok(SourceIdentity {
        article_id: paper_id.to_string(),
        selected_format: format,
        pdf_sha256: if is_html {
            None
        } else {
            Some(content_sha.clone())
        },
        html_sha256: if is_html {
            Some(content_sha.clone())
        } else {
            None
        },
        acquisition_fingerprint: content_sha,
        normalization_fingerprint: "identity-v1".into(),
    })
}

/// Parse primary content with optional PipelineCache (derived payload only).
/// Returns (parsed, cache records, payload_sha256 for dependency keys).
async fn parse_with_cache(
    cache: Option<&FilePipelineCache>,
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
) -> Result<(ParsedArticle, Vec<LoadCacheRecord>, String), String> {
    let primary_str = primary_path.display().to_string();
    let source = source_identity_for_primary(paper_id, primary_path, is_html)?;
    let impl_fp = if is_html {
        "da_adapters::HtmlParser@load-v1"
    } else {
        "da_adapters::GrobidParser@load-v1"
    };
    let key = StageCacheKey {
        source,
        stage_id: "parse".into(),
        dependency_output_sha256: BTreeMap::new(),
        implementation_fingerprint: impl_fp.into(),
        configuration_fingerprint: if is_html {
            "html-primary".into()
        } else {
            "pdf-grobid".into()
        },
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: "ParsedArticle.v1".into(),
    };

    // Async produce needs to be outside the sync cache helper: pre-run only on miss path
    // is handled by checking lookup first when cache is present.
    let key_sha = serde_json::to_vec(&key)
        .map(|v| sha256_hex(&v))
        .unwrap_or_default();

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        match cache
            .lookup(&key, &now)
            .map_err(|e| format!("parse cache lookup: {e}"))?
        {
            CacheLookup::Hit(hit) => {
                let parsed: ParsedArticle = serde_json::from_slice(&hit.payload)
                    .map_err(|e| format!("parse cache payload decode: {e}"))?;
                let payload_sha = sha256_hex(&hit.payload);
                return Ok((
                    parsed,
                    vec![LoadCacheRecord {
                        stage_id: "parse".into(),
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(payload_sha.clone()),
                    }],
                    payload_sha,
                ));
            }
            CacheLookup::RetryableFailure(m) => {
                let parsed = parse_primary(&primary_str, paper_id, is_html).await?;
                let payload =
                    serde_json::to_vec(&parsed).map_err(|e| format!("serialize parse: {e}"))?;
                let payload_sha = sha256_hex(&payload);
                let _ = cache.store_success(&key, &payload, BTreeMap::new(), &now);
                return Ok((
                    parsed,
                    vec![LoadCacheRecord {
                        stage_id: "parse".into(),
                        status: "retryable_failure".into(),
                        key_sha256: Some(m.key_sha256),
                        payload_sha256: Some(payload_sha.clone()),
                    }],
                    payload_sha,
                ));
            }
            CacheLookup::Miss => {
                let parsed = parse_primary(&primary_str, paper_id, is_html).await?;
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
                        status: "miss".into(),
                        key_sha256: Some(entry.key_sha256),
                        payload_sha256: Some(payload_sha.clone()),
                    }],
                    payload_sha,
                ));
            }
        }
    }

    let parsed = parse_primary(&primary_str, paper_id, is_html).await?;
    let payload = serde_json::to_vec(&parsed).map_err(|e| format!("serialize parse: {e}"))?;
    let payload_sha = sha256_hex(&payload);
    Ok((
        parsed,
        vec![LoadCacheRecord {
            stage_id: "parse".into(),
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
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
    parse_payload_sha: &str,
    embed_text: &str,
) -> Result<(Vec<f32>, Vec<LoadCacheRecord>), String> {
    let source = source_identity_for_primary(paper_id, primary_path, is_html)?;
    let embedder = FdApiEmbedder::from_env();
    let model_id = embedder.model_id().to_string();
    let text_sha = sha256_hex(embed_text.as_bytes());
    let mut deps = BTreeMap::new();
    deps.insert("parse".into(), parse_payload_sha.to_string());
    deps.insert("embed_text".into(), text_sha);
    let key = StageCacheKey {
        source,
        stage_id: "embed".into(),
        dependency_output_sha256: deps,
        implementation_fingerprint: "da_adapters::FdApiEmbedder@load-v1".into(),
        configuration_fingerprint: format!("model={model_id};input=abstract_or_title"),
        prompt_fingerprint: None,
        model_fingerprint: Some(model_id),
        output_schema_fingerprint: "Vec<f32>.le-bytes.v1".into(),
    };
    let key_sha = serde_json::to_vec(&key)
        .map(|v| sha256_hex(&v))
        .unwrap_or_default();

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        match cache
            .lookup(&key, &now)
            .map_err(|e| format!("embed cache lookup: {e}"))?
        {
            CacheLookup::Hit(hit) => {
                let vector = decode_f32_le(&hit.payload, embedder.dimensions())?;
                return Ok((
                    vector,
                    vec![LoadCacheRecord {
                        stage_id: "embed".into(),
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(sha256_hex(&hit.payload)),
                    }],
                ));
            }
            CacheLookup::RetryableFailure(m) => {
                require_embed_credentials()?;
                let vector = embedder
                    .embed(embed_text)
                    .await
                    .map_err(|e| format!("embed: {e:#}"))?;
                let payload = encode_f32_le(&vector);
                let _ = cache.store_success(&key, &payload, BTreeMap::new(), &now);
                return Ok((
                    vector,
                    vec![LoadCacheRecord {
                        stage_id: "embed".into(),
                        status: "retryable_failure".into(),
                        key_sha256: Some(m.key_sha256),
                        payload_sha256: Some(sha256_hex(&payload)),
                    }],
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
                        stage_id: "embed".into(),
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
            stage_id: "embed".into(),
            status: "bypass".into(),
            key_sha256: Some(key_sha),
            payload_sha256: Some(sha256_hex(&payload)),
        }],
    ))
}

async fn extract_with_cache(
    cache: Option<&FilePipelineCache>,
    paper_id: &str,
    primary_path: &Path,
    is_html: bool,
    parse_payload_sha: &str,
    parsed: &ParsedArticle,
    extractor: &RuleBasedExtractor,
) -> Result<(Vec<ExtractedEntity>, Vec<LoadCacheRecord>), String> {
    let source = source_identity_for_primary(paper_id, primary_path, is_html)?;
    let identity = extractor.cache_identity();
    let mut dependencies = BTreeMap::new();
    dependencies.insert("parse".into(), parse_payload_sha.to_string());
    let key = StageCacheKey {
        source,
        stage_id: "extract".into(),
        dependency_output_sha256: dependencies,
        implementation_fingerprint: identity.implementation_fingerprint,
        configuration_fingerprint: identity.configuration_fingerprint,
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: identity.output_schema_fingerprint,
    };
    let key_sha = serde_json::to_vec(&key)
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_default();

    if let Some(cache) = cache {
        let now = chrono::Utc::now().to_rfc3339();
        match cache
            .lookup(&key, &now)
            .map_err(|error| format!("extract cache lookup: {error}"))?
        {
            CacheLookup::Hit(hit) => {
                let extracted: Vec<ExtractedEntity> = serde_json::from_slice(&hit.payload)
                    .map_err(|error| format!("extract cache payload decode: {error}"))?;
                return Ok((
                    extracted,
                    vec![LoadCacheRecord {
                        stage_id: "extract".into(),
                        status: "hit".into(),
                        key_sha256: Some(hit.manifest.key_sha256),
                        payload_sha256: Some(sha256_hex(&hit.payload)),
                    }],
                ));
            }
            CacheLookup::RetryableFailure(manifest) => {
                let extracted = compute_extraction(parsed, extractor).await?;
                let payload = serde_json::to_vec(&extracted)
                    .map_err(|error| format!("serialize extraction: {error}"))?;
                let _ = cache.store_success(&key, &payload, BTreeMap::new(), &now);
                return Ok((
                    extracted,
                    vec![LoadCacheRecord {
                        stage_id: "extract".into(),
                        status: "retryable_failure".into(),
                        key_sha256: Some(manifest.key_sha256),
                        payload_sha256: Some(sha256_hex(&payload)),
                    }],
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
                        stage_id: "extract".into(),
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
            stage_id: "extract".into(),
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
) -> Result<ParsedArticle, String> {
    if is_html {
        let parser = HtmlParser::new();
        parser
            .parse_html(primary_path, paper_id)
            .await
            .map_err(|e| format!("html parse: {e:#}"))
    } else {
        let parser = GrobidParser::from_env();
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
    let cache = if args.no_cache {
        None
    } else {
        let _ = std::fs::create_dir_all(&args.cache_root);
        Some(Arc::new(FilePipelineCache::new(&args.cache_root)))
    };
    let clock: Arc<dyn da_ports::Clock> =
        match validate_materialization_time(args.materialization_time.as_deref())? {
            None => Arc::new(da_ports::SystemClock),
            Some(parsed) => Arc::new(da_ports::FixedClock::from_timestamp(parsed.timestamp())),
        };
    Ok(CliContentPipeline {
        store,
        export_per_unit,
        cache,
        clock,
    })
}

fn validate_materialization_time(
    value: Option<&str>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    value
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map(|time| time.with_timezone(&chrono::Utc))
                .map_err(|error| format!("invalid --materialization-time {value:?}: {error}"))
        })
        .transpose()
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
    validate_materialization_time(args.materialization_time.as_deref())?;
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

    if let Some(time) = validate_materialization_time(args.materialization_time.as_deref())? {
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
            "test.paper",
            &source,
            true,
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
            "test.paper",
            &source,
            true,
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
            "test.paper",
            &source,
            true,
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
            "test.paper",
            &source,
            true,
            &"b".repeat(64),
            &parsed,
            &extractor,
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("extract cache lookup")
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
