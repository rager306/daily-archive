//! Run one bounded gold-corpus source batch twice to prove cache replay semantics.
//!
//! This canary is deliberately graph-free. Parser and extractor stages cannot
//! mutate graph state; graph materialization is a later always-run audited step.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use da_adapters::{
    FdApiEmbedder, FilePipelineCache, GrobidParseStage, HtmlParseStage, HtmlParser,
    ParsedArticleEmbeddingStage, RuleBasedExtractor, RuleExtractionStage,
};
use da_application::{PipelineOrchestrator, PipelineRunReport, StageDisposition};
use da_cli::gold_corpus::{CanonicalSourceKind, GoldCorpusManifest};
use da_domain::eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
use da_parse::{GrobidParser, GrobidProfile, PROJECT_ARTICLE_SCHEMA_FINGERPRINT};
use da_ports::extractor::ExtractedEntity;
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, SourceFormat, SourceIdentity, StageOutcome,
};
use da_ports::pipeline_stage::StageContract;
use kg_pipeline::PipelineBuilder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    manifest: PathBuf,
    #[arg(long, default_value_t = 1)]
    batch: usize,
    #[arg(long, default_value = "artifacts/gold_corpus/pipeline_cache")]
    cache_root: PathBuf,
    #[arg(long, default_value = "artifacts/gold_corpus/canary_batch_01.json")]
    report: PathBuf,
    /// Require the first pass to execute every stage (use with a fresh cache root).
    #[arg(long)]
    require_cold_first: bool,
    /// Bounded GROBID request timeout; consolidation may back off on Crossref 429.
    #[arg(long, default_value_t = 300)]
    grobid_timeout_seconds: u64,
    /// Retry suppression window persisted by the pipeline cache.
    #[arg(long, default_value_t = 30)]
    retry_delay_seconds: i64,
    /// Add the cacheable paper embedding stage after parse.
    #[arg(long)]
    include_embedding: bool,
}

#[derive(Debug, Default, Serialize)]
struct DispositionCounts {
    executed: usize,
    cache_hit: usize,
    retry_suppressed: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct PaperMetrics {
    gold_entities: usize,
    predicted_entities: usize,
    true_positives: usize,
    false_positives: usize,
    false_negatives: usize,
    precision: f64,
    recall: f64,
    f1: f64,
    extraction_payload_sha256: String,
}

#[derive(Debug, Serialize)]
struct PaperCanaryReport {
    paper_id: String,
    source_sha256: String,
    metrics: Option<PaperMetrics>,
    failure: Option<String>,
    first: PipelineRunReport,
    replay: PipelineRunReport,
}

#[derive(Debug, Serialize)]
struct BatchMetrics {
    evaluated_papers: usize,
    failed_papers: usize,
    gold_entities: usize,
    predicted_entities: usize,
    true_positives: usize,
    false_positives: usize,
    false_negatives: usize,
    micro_precision: f64,
    micro_recall: f64,
    micro_f1: f64,
    macro_f1: f64,
}

#[derive(Debug, serde::Deserialize)]
struct GoldFixture {
    paper_id: String,
    entities: Vec<GoldEntity>,
}

#[derive(Debug, Serialize)]
struct CanaryReport {
    manifest_version: String,
    manifest_sha256: String,
    cache_root: String,
    batch: usize,
    grobid_timeout_seconds: u64,
    retry_delay_seconds: i64,
    embedding_enabled: bool,
    batch_manifest_records: usize,
    paper_count: usize,
    skipped_unavailable: usize,
    metrics: BatchMetrics,
    first: DispositionCounts,
    replay: DispositionCounts,
    replay_all_cache_hit: bool,
    cold_first_satisfied: Option<bool>,
    wave_passed: bool,
    graph_net_writes: usize,
    graph_boundary: &'static str,
    import_eligible: bool,
    papers: Vec<PaperCanaryReport>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("gold_corpus_canary");
    let args = Args::parse();
    let manifest_bytes = std::fs::read(&args.manifest)
        .with_context(|| format!("read {}", args.manifest.display()))?;
    let manifest_sha256 = hex::encode(Sha256::digest(&manifest_bytes));
    let manifest: GoldCorpusManifest =
        serde_json::from_slice(&manifest_bytes).context("deserialize gold corpus manifest")?;
    let batch_records = manifest
        .records
        .iter()
        .filter(|record| record.batch == args.batch)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !batch_records.is_empty(),
        "batch {} has no records",
        args.batch
    );
    let records = batch_records
        .iter()
        .copied()
        .filter(|record| record.source.kind != CanonicalSourceKind::Unavailable)
        .collect::<Vec<_>>();
    let skipped_unavailable = batch_records.len() - records.len();
    anyhow::ensure!(
        !records.is_empty(),
        "batch {} has no source-ready records",
        args.batch
    );
    anyhow::ensure!(
        args.grobid_timeout_seconds > 0,
        "GROBID timeout must be positive"
    );
    anyhow::ensure!(args.retry_delay_seconds > 0, "retry delay must be positive");
    if args.include_embedding {
        anyhow::ensure!(
            std::env::var("FD_API_KEY").is_ok_and(|value| !value.trim().is_empty()),
            "FD_API_KEY is missing after dotenv load"
        );
    }

    let extraction_configuration_fingerprint =
        file_fingerprint(Path::new("data/extraction_patterns.yaml"))?;
    let cache = Arc::new(FilePipelineCache::new(&args.cache_root));
    let mut papers = Vec::with_capacity(records.len());

    for record in records {
        let source_path = record
            .source
            .path
            .as_deref()
            .context("canonical source path missing")?;
        let source_sha256 = record
            .source
            .sha256
            .as_deref()
            .context("canonical source fingerprint missing")?;
        // The persisted retry time must remain future-dated even when the
        // request consumes its full timeout budget.
        let retry_after = (Utc::now()
            + Duration::seconds(args.grobid_timeout_seconds as i64 + args.retry_delay_seconds))
        .to_rfc3339();
        let (parse_stage_id, parse_stage, source_format): (
            &str,
            Arc<dyn da_ports::pipeline_stage::PipelineStageExecutor>,
            SourceFormat,
        ) = match record.source.kind {
            CanonicalSourceKind::Pdf => (
                "grobid_parse",
                Arc::new(
                    GrobidParseStage::new(
                        Arc::new(
                            GrobidParser::from_env()
                                .with_profile(GrobidProfile::structural())
                                .with_timeout(std::time::Duration::from_secs(
                                    args.grobid_timeout_seconds,
                                )),
                        ),
                        source_path,
                        retry_after,
                    )
                    .with_contract(StageContract {
                        stage_id: "grobid_parse".into(),
                        implementation_fingerprint: "GrobidParser@tei-navigation-v3".into(),
                        configuration_fingerprint:
                            "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula"
                                .into(),
                        prompt_fingerprint: None,
                        model_fingerprint: Some("grobid/grobid:0.9.0-crf".into()),
                        output_schema_fingerprint:
                            PROJECT_ARTICLE_SCHEMA_FINGERPRINT.into(),
                    })
                    .with_retry_delay_seconds(args.retry_delay_seconds),
                ),
                SourceFormat::Pdf,
            ),
            CanonicalSourceKind::Html => (
                "html_parse",
                Arc::new(HtmlParseStage::new(
                    Arc::new(HtmlParser::new()),
                    source_path,
                )),
                SourceFormat::Html,
            ),
            CanonicalSourceKind::Unavailable => unreachable!("unavailable records were filtered"),
        };
        let mut pipeline_builder = PipelineBuilder::new("gold-corpus-parse-extract-v1")
            .step("parse", parse_stage_id, &[])
            .step("extract", "rule_extract", &["parse"]);
        if args.include_embedding {
            pipeline_builder = pipeline_builder.step("embed", "paper_embed", &["parse"]);
        }
        let pipeline = pipeline_builder
            .build()
            .map_err(|error| anyhow::anyhow!("build canary pipeline: {error}"))?;
        let stage_registry =
            da_cli::production::embedded_stage_registry().map_err(anyhow::Error::msg)?;
        let mut orchestrator = PipelineOrchestrator::new(
            cache.clone(),
            Arc::new(da_ports::SystemClock),
            stage_registry,
        )
        .with_executor(parse_stage_id, parse_stage)?
        .with_executor(
            "rule_extract",
            Arc::new(RuleExtractionStage::new(
                Arc::new(RuleBasedExtractor::new()),
                "parse",
                extraction_configuration_fingerprint.clone(),
            )),
        )?;
        if args.include_embedding {
            let embedder = FdApiEmbedder::from_env();
            orchestrator = orchestrator.with_executor(
                "paper_embed",
                Arc::new(ParsedArticleEmbeddingStage::new(
                    Arc::new(embedder),
                    "parse",
                    (Utc::now() + Duration::seconds(args.retry_delay_seconds)).to_rfc3339(),
                )),
            )?;
        }
        let source = SourceIdentity {
            article_id: record.paper_id.clone(),
            selected_format: source_format,
            pdf_sha256: (record.source.kind == CanonicalSourceKind::Pdf)
                .then(|| source_sha256.into()),
            html_sha256: (record.source.kind == CanonicalSourceKind::Html)
                .then(|| source_sha256.into()),
            acquisition_fingerprint: "canonical-article-catalog-v1".into(),
            normalization_fingerprint: match record.source.kind {
                CanonicalSourceKind::Pdf => "grobid-tei-navigation-v3".into(),
                CanonicalSourceKind::Html => "html-structured-sections-v1".into(),
                CanonicalSourceKind::Unavailable => unreachable!(),
            },
        };
        let first = orchestrator.run(&pipeline, source.clone()).await?;
        let replay = orchestrator.run(&pipeline, source).await?;
        let evaluated = evaluate_cached_extraction(cache.as_ref(), record, &replay);
        let (metrics, failure) = match evaluated {
            Ok(metrics) => (Some(metrics), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        papers.push(PaperCanaryReport {
            paper_id: record.paper_id.clone(),
            source_sha256: source_sha256.into(),
            metrics,
            failure,
            first,
            replay,
        });
    }

    let first = count_dispositions(papers.iter().map(|paper| &paper.first));
    let replay = count_dispositions(papers.iter().map(|paper| &paper.replay));
    let metrics = aggregate_metrics(&papers);
    let replay_all_cache_hit = papers.iter().all(|paper| {
        paper.replay.completed
            && paper
                .replay
                .stages
                .iter()
                .all(|stage| matches!(stage.disposition, StageDisposition::CacheHit { .. }))
    });
    let cold_first_satisfied = args.require_cold_first.then_some(
        first.executed == papers.len() * if args.include_embedding { 3 } else { 2 }
            && first.cache_hit == 0
            && first.failed == 0
            && first.retry_suppressed == 0,
    );
    let wave_passed =
        replay_all_cache_hit && cold_first_satisfied.unwrap_or(true) && metrics.failed_papers == 0;
    let report = CanaryReport {
        manifest_version: manifest.version,
        manifest_sha256,
        cache_root: args.cache_root.to_string_lossy().to_string(),
        batch: args.batch,
        grobid_timeout_seconds: args.grobid_timeout_seconds,
        retry_delay_seconds: args.retry_delay_seconds,
        embedding_enabled: args.include_embedding,
        batch_manifest_records: batch_records.len(),
        paper_count: papers.len(),
        skipped_unavailable,
        metrics,
        first,
        replay,
        replay_all_cache_hit,
        cold_first_satisfied,
        wave_passed,
        graph_net_writes: 0,
        graph_boundary: "no GraphStore is injected into cacheable parse/extract stages",
        import_eligible: false,
        papers,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    anyhow::ensure!(
        wave_passed,
        "wave {} did not pass; see {}",
        args.batch,
        args.report.display()
    );
    Ok(())
}

fn evaluate_cached_extraction(
    cache: &dyn PipelineCache,
    record: &da_cli::gold_corpus::GoldCorpusRecord,
    replay: &PipelineRunReport,
) -> Result<PaperMetrics> {
    let (extraction_payload_sha256, extraction_key_sha256) =
        stage_output_identity(replay, "extract")?;
    let extraction_stage_id = stage_run_stage_id(replay, "extract")?;
    let extracted = load_verified_extraction(
        cache,
        &record.paper_id,
        &extraction_payload_sha256,
        &extraction_key_sha256,
        &extraction_stage_id,
    )?;
    let gold: GoldFixture = serde_json::from_slice(
        &std::fs::read(&record.fixture_path)
            .with_context(|| format!("read {}", record.fixture_path))?,
    )
    .with_context(|| format!("parse {}", record.fixture_path))?;
    anyhow::ensure!(
        gold.paper_id == record.paper_id,
        "gold identity changed after manifest"
    );
    let predicted = extracted
        .iter()
        .map(|entity| PredictedEntity {
            label: entity.label.clone(),
            entity_type: entity.entity_type.as_str().into(),
        })
        .collect::<Vec<_>>();
    let metrics = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);
    Ok(PaperMetrics {
        gold_entities: metrics.gold_count,
        predicted_entities: metrics.predicted_count,
        true_positives: metrics.true_positives,
        false_positives: metrics.false_positives,
        false_negatives: metrics.false_negatives,
        precision: metrics.precision,
        recall: metrics.recall,
        f1: metrics.f1,
        extraction_payload_sha256,
    })
}

fn stage_output_identity(report: &PipelineRunReport, step_name: &str) -> Result<(String, String)> {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.step_name == step_name)
        .with_context(|| format!("missing stage '{step_name}' in pipeline report"))?;
    let key_sha256 = stage
        .cache_key_sha256
        .clone()
        .with_context(|| format!("stage '{step_name}' lacks exact cache key identity"))?;
    match &stage.disposition {
        StageDisposition::Executed { output_sha256 }
        | StageDisposition::CacheHit { output_sha256 } => Ok((output_sha256.clone(), key_sha256)),
        other => anyhow::bail!("stage '{step_name}' has no successful output: {other:?}"),
    }
}

/// The canonical stage id the pipeline actually ran this step under.
/// Vocabulary-agnostic: the report is the source of truth (post split the
/// extract step runs as `extraction_compute`, embed as `embedding`).
fn stage_run_stage_id(report: &PipelineRunReport, step_name: &str) -> Result<String> {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.step_name == step_name)
        .with_context(|| format!("missing stage '{step_name}' in pipeline report"))?;
    Ok(stage.stage_id.clone())
}

fn load_verified_extraction(
    cache: &dyn PipelineCache,
    article_id: &str,
    output_sha256: &str,
    key_sha256: &str,
    expected_stage_id: &str,
) -> Result<Vec<ExtractedEntity>> {
    let matches = cache
        .manifest(article_id)?
        .into_iter()
        .filter(|entry| {
            entry.key.stage_id == expected_stage_id
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
        "expected one extraction cache entry for {article_id}/{key_sha256}/{output_sha256}, found {}",
        matches.len()
    );
    let key = &matches[0].key;
    let CacheLookup::Hit(hit) = cache.lookup(key, &Utc::now().to_rfc3339())? else {
        anyhow::bail!("verified extraction entry was not a cache hit for {article_id}");
    };
    serde_json::from_slice(&hit.payload)
        .with_context(|| format!("deserialize cached extraction for {article_id}"))
}

fn aggregate_metrics(papers: &[PaperCanaryReport]) -> BatchMetrics {
    let successful = papers
        .iter()
        .filter_map(|paper| paper.metrics.as_ref())
        .collect::<Vec<_>>();
    let evaluated_papers = successful.len();
    let failed_papers = papers.len() - evaluated_papers;
    let gold_entities = successful.iter().map(|metrics| metrics.gold_entities).sum();
    let predicted_entities = successful
        .iter()
        .map(|metrics| metrics.predicted_entities)
        .sum();
    let true_positives = successful
        .iter()
        .map(|metrics| metrics.true_positives)
        .sum();
    let false_positives = successful
        .iter()
        .map(|metrics| metrics.false_positives)
        .sum();
    let false_negatives = successful
        .iter()
        .map(|metrics| metrics.false_negatives)
        .sum();
    let micro_precision = ratio(true_positives, true_positives + false_positives);
    let micro_recall = ratio(true_positives, true_positives + false_negatives);
    let micro_f1 = harmonic_mean(micro_precision, micro_recall);
    let macro_f1 = if successful.is_empty() {
        0.0
    } else {
        successful.iter().map(|metrics| metrics.f1).sum::<f64>() / successful.len() as f64
    };
    BatchMetrics {
        evaluated_papers,
        failed_papers,
        gold_entities,
        predicted_entities,
        true_positives,
        false_positives,
        false_negatives,
        micro_precision,
        micro_recall,
        micro_f1,
        macro_f1,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn harmonic_mean(left: f64, right: f64) -> f64 {
    if left + right == 0.0 {
        0.0
    } else {
        2.0 * left * right / (left + right)
    }
}

fn count_dispositions<'a>(
    reports: impl Iterator<Item = &'a PipelineRunReport>,
) -> DispositionCounts {
    let mut counts = DispositionCounts::default();
    for stage in reports.flat_map(|report| &report.stages) {
        match stage.disposition {
            StageDisposition::Executed { .. } => counts.executed += 1,
            StageDisposition::CacheHit { .. } => counts.cache_hit += 1,
            StageDisposition::RetrySuppressed { .. } => counts.retry_suppressed += 1,
            StageDisposition::Failed { .. } => counts.failed += 1,
        }
    }
    counts
}

fn file_fingerprint(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod verified_extraction_tests {
    use super::*;
    use da_ports::pipeline_cache::{CachedSuccess, StageCacheKey, StageManifestEntry};
    use std::collections::BTreeMap;

    /// In-memory cache double for the manifest/lookup surface the canary
    /// verification relies on.
    struct MemoryCache {
        entries: Vec<StageManifestEntry>,
        payload: Vec<u8>,
    }

    impl MemoryCache {
        fn with_extraction(stage_id: &str, key_sha256: &str, output_sha256: &str) -> Self {
            let key = StageCacheKey {
                source: SourceIdentity {
                    article_id: "2501.00001".into(),
                    selected_format: SourceFormat::Pdf,
                    pdf_sha256: Some("aa".into()),
                    html_sha256: None,
                    acquisition_fingerprint: "acq".into(),
                    normalization_fingerprint: "norm".into(),
                },
                stage_id: stage_id.into(),
                dependency_output_sha256: BTreeMap::new(),
                implementation_fingerprint: "rule_extract".into(),
                configuration_fingerprint: "cfg".into(),
                prompt_fingerprint: None,
                model_fingerprint: None,
                output_schema_fingerprint: "schema".into(),
            };
            let entities = vec![ExtractedEntity {
                label: "graph neural network".into(),
                entity_type: serde_json::from_str("\"Method\"").unwrap(),
                section_title: "Abstract".into(),
                section_ordinal: Some(0),
                char_start: 0,
                char_end: 21,
                surface: "graph neural network".into(),
            }];
            let payload = serde_json::to_vec(&entities).unwrap();
            let digest = Sha256::digest(&payload);
            let output: String = digest.iter().map(|x| format!("{x:02x}")).collect();
            assert_eq!(
                output, output_sha256,
                "test fixture must bind the real payload hash"
            );
            let entry = StageManifestEntry {
                key: key.clone(),
                key_sha256: key_sha256.into(),
                recorded_at: "2026-08-21T00:00:00Z".into(),
                outcome: StageOutcome::Success {
                    payload_sha256: output,
                    payload_size: payload.len() as u64,
                },
                metadata: BTreeMap::new(),
            };
            Self {
                entries: vec![entry],
                payload,
            }
        }
    }

    impl PipelineCache for MemoryCache {
        fn lookup(
            &self,
            _key: &StageCacheKey,
            _now: &str,
        ) -> da_ports::pipeline_cache::PipelineCacheResult<CacheLookup> {
            Ok(CacheLookup::Hit(CachedSuccess {
                manifest: self.entries[0].clone(),
                payload: self.payload.clone(),
            }))
        }

        fn store_success(
            &self,
            _key: &StageCacheKey,
            _payload: &[u8],
            _metadata: BTreeMap<String, String>,
            _recorded_at: &str,
        ) -> da_ports::pipeline_cache::PipelineCacheResult<StageManifestEntry> {
            unreachable!("verification never stores")
        }

        fn store_retryable_failure(
            &self,
            _key: &StageCacheKey,
            _error_class: &str,
            _message: &str,
            _retry_after: &str,
            _recorded_at: &str,
        ) -> da_ports::pipeline_cache::PipelineCacheResult<StageManifestEntry> {
            unreachable!("verification never stores failures")
        }

        fn manifest(
            &self,
            _article_id: &str,
        ) -> da_ports::pipeline_cache::PipelineCacheResult<Vec<StageManifestEntry>> {
            Ok(self.entries.clone())
        }
    }

    fn report_with_extract(
        stage_id: &str,
        key_sha256: &str,
        output_sha256: &str,
    ) -> PipelineRunReport {
        PipelineRunReport {
            pipeline_id: "gold-corpus-parse-extract-v1".into(),
            article_id: "2501.00001".into(),
            completed: true,
            stages: vec![da_application::StageRunRecord {
                step_name: "extract".into(),
                stage_id: stage_id.into(),
                resolved_from: Some("rule_extract".into()),
                cache_key_sha256: Some(key_sha256.into()),
                admission: None,
                disposition: StageDisposition::Executed {
                    output_sha256: output_sha256.into(),
                },
            }],
        }
    }

    /// Regression: after the extraction/materialization split the extract
    /// step runs under stage id `extraction_compute`; the canary's verified
    /// extraction lookup must resolve the stage id from the report instead
    /// of a hardcoded vocabulary.
    #[test]
    fn verified_extraction_resolves_stage_id_from_report() {
        let key_sha = "ab".repeat(16);
        // Compute the real payload hash for the fixture entities first.
        let entities = vec![ExtractedEntity {
            label: "graph neural network".into(),
            entity_type: serde_json::from_str("\"Method\"").unwrap(),
            section_title: "Abstract".into(),
            section_ordinal: Some(0),
            char_start: 0,
            char_end: 21,
            surface: "graph neural network".into(),
        }];
        let payload = serde_json::to_vec(&entities).unwrap();
        let out_sha: String = Sha256::digest(&payload)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect();

        let cache = MemoryCache::with_extraction("extraction_compute", &key_sha, &out_sha);
        let report = report_with_extract("extraction_compute", &key_sha, &out_sha);
        let (output, key) = stage_output_identity(&report, "extract").unwrap();
        assert_eq!(
            (output.as_str(), key.as_str()),
            (out_sha.as_str(), key_sha.as_str())
        );
        let stage_id = stage_run_stage_id(&report, "extract").unwrap();
        assert_eq!(stage_id, "extraction_compute");
        let entities =
            load_verified_extraction(&cache, "2501.00001", &output, &key, &stage_id).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].label, "graph neural network");
    }
}
