//! M440 S01: sweep-cache materialization coverage audit.
//!
//! Walks the M439 catalog sweep pipeline cache and decides, per paper,
//! whether the ADR-065 `tei_declared_revision` fallback can bind a
//! revision-owned identity for materialization. Papers that cannot be bound
//! terminate in a typed skipped state with a reason code — never a silent
//! versionless write and never a crash.
//!
//! The audit is read-only: it opens no graph store and performs no writes
//! beyond the coverage report artifact. D127 stays locked.

use anyhow::{Context, Result};
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, StageOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Cache stage ids used by the catalog sweep orchestrator.
pub const STAGE_PARSE: &str = "parse";
pub const STAGE_EXTRACTION: &str = "extraction_compute";
pub const STAGE_EMBEDDING: &str = "embedding";

/// Cache lookup horizon: sweep payloads never expire within the audit.
const FAR_FUTURE: &str = "9999-12-31T23:59:59Z";

/// Typed skip reasons. `&'static str` keeps the report compact and stable.
pub const REASON_NO_PARSE_CACHE: &str = "no_parse_cache";
pub const REASON_AMBIGUOUS_PARSE_KEYS: &str = "ambiguous_parse_keys";
pub const REASON_NO_TEI_XML: &str = "no_tei_xml";
pub const REASON_NO_TEI_REVISION: &str = "no_tei_revision";
pub const REASON_NO_EXTRACTION_CACHE: &str = "no_extraction_cache";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageVerdict {
    Materializable,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaperCoverage {
    pub paper_id: String,
    pub category: Option<String>,
    pub verdict: CoverageVerdict,
    pub revision: Option<u32>,
    pub reason: Option<String>,
    pub has_parse: bool,
    pub has_extraction: bool,
    pub has_embedding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageTotals {
    pub papers: usize,
    pub materializable: usize,
    pub skipped: usize,
    pub by_reason: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageReconcile {
    pub sweep_ok: usize,
    pub cache_papers: usize,
    /// Sweep `ok` papers with no cache directory — an honest audit error.
    pub ok_missing_from_cache: Vec<String>,
    /// Cache directories not present in the sweep `ok` set (e.g. later
    /// sweeps, recovered papers). Reported, never fatal.
    pub extra_cache_papers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub schema_version: u32,
    pub totals: CoverageTotals,
    pub reconcile: CoverageReconcile,
    pub papers: Vec<PaperCoverage>,
}

/// One `outcome == "ok"` row of the M439 sweep report.
#[derive(Debug, Clone, Deserialize)]
pub struct SweepOkPaper {
    pub paper_id: String,
    pub category: String,
}

#[derive(Debug, Deserialize)]
struct SweepReportFile {
    papers: BTreeMap<String, SweepPaperRow>,
}

#[derive(Debug, Deserialize)]
struct SweepPaperRow {
    paper_id: String,
    category: String,
    outcome: String,
}

/// Load `outcome == "ok"` papers from the M439 sweep report. Order is
/// deterministic (BTreeMap keys).
pub fn load_sweep_ok_papers(path: &Path) -> Result<Vec<SweepOkPaper>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read sweep report {}", path.display()))?;
    let report: SweepReportFile =
        serde_json::from_slice(&bytes).with_context(|| "parse sweep report JSON")?;
    Ok(report
        .papers
        .into_values()
        .filter(|row| row.outcome == "ok")
        .map(|row| SweepOkPaper {
            paper_id: row.paper_id,
            category: row.category,
        })
        .collect())
}

/// Audit every sweep `ok` paper against the pipeline cache.
///
/// Returns [`Err`] only when the reconciliation is dishonest: an `ok` paper
/// without a cache directory means the sweep report overclaims and the
/// operator must fix that before materialization. Typed per-paper skips are
/// report rows, not errors.
pub fn audit(
    cache: &dyn PipelineCache,
    ok_papers: &[SweepOkPaper],
    cache_paper_ids: &[String],
) -> Result<CoverageReport> {
    let cache_id_set: std::collections::BTreeSet<&str> =
        cache_paper_ids.iter().map(String::as_str).collect();
    let ok_missing_from_cache: Vec<String> = ok_papers
        .iter()
        .filter(|p| !cache_id_set.contains(p.paper_id.as_str()))
        .map(|p| p.paper_id.clone())
        .collect();
    anyhow::ensure!(
        ok_missing_from_cache.is_empty(),
        "sweep report claims ok but cache directory is missing for: {}",
        ok_missing_from_cache.join(", ")
    );
    let ok_id_set: std::collections::BTreeSet<&str> =
        ok_papers.iter().map(|p| p.paper_id.as_str()).collect();
    let extra_cache_papers: Vec<String> = cache_paper_ids
        .iter()
        .filter(|id| !ok_id_set.contains(id.as_str()))
        .cloned()
        .collect();

    let mut papers = Vec::with_capacity(ok_papers.len());
    let mut by_reason: BTreeMap<String, usize> = BTreeMap::new();
    let mut materializable = 0usize;
    for paper in ok_papers {
        // A manifest read error (missing directory) is a typed no-cache
        // skip, not an audit crash. The reconciliation above already made
        // missing directories for `ok` papers impossible.
        let manifest = cache.manifest(&paper.paper_id).unwrap_or_default();
        let has_stage = |stage: &str| {
            manifest.iter().any(|e| {
                e.key.stage_id == stage && matches!(e.outcome, StageOutcome::Success { .. })
            })
        };
        let has_parse = has_stage(STAGE_PARSE);
        let has_extraction = has_stage(STAGE_EXTRACTION);
        let has_embedding = has_stage(STAGE_EMBEDDING);

        let parse_successes: Vec<&da_ports::pipeline_cache::StageManifestEntry> = manifest
            .iter()
            .filter(|e| {
                e.key.stage_id == STAGE_PARSE && matches!(e.outcome, StageOutcome::Success { .. })
            })
            .collect();
        let distinct_keys: std::collections::BTreeSet<&str> = parse_successes
            .iter()
            .map(|e| e.key_sha256.as_str())
            .collect();

        let (revision, reason) = if !has_parse || distinct_keys.is_empty() {
            (None, Some(REASON_NO_PARSE_CACHE))
        } else if distinct_keys.len() > 1 {
            (None, Some(REASON_AMBIGUOUS_PARSE_KEYS))
        } else {
            let entry = parse_successes[0];
            let bound = match cache.lookup(&entry.key, FAR_FUTURE) {
                Ok(CacheLookup::Hit(hit)) => serde_json::from_slice::<ParsedArticle>(&hit.payload)
                    .ok()
                    .and_then(|parsed| {
                        parsed.tei_xml.as_deref().and_then(|tei| {
                            crate::gold_graph::declared_arxiv_revision(tei, &paper.paper_id)
                        })
                    }),
                _ => None,
            };
            match bound {
                Some(rev) => (Some(rev), None),
                None => {
                    // Distinguish "payload present but unbindable" from a
                    // vanished payload: only decode misses with a successful
                    // lookup carry the no-tei reasons.
                    match cache.lookup(&entry.key, FAR_FUTURE) {
                        Ok(CacheLookup::Hit(hit)) => {
                            let parsed: Option<ParsedArticle> =
                                serde_json::from_slice(&hit.payload).ok();
                            let reason = match parsed.as_ref().and_then(|p| p.tei_xml.as_deref()) {
                                None => REASON_NO_TEI_XML,
                                Some(_) => REASON_NO_TEI_REVISION,
                            };
                            (None, Some(reason))
                        }
                        _ => (None, Some(REASON_NO_PARSE_CACHE)),
                    }
                }
            }
        };

        let (verdict, reason): (CoverageVerdict, Option<String>) =
            match (revision, reason, has_extraction) {
                (Some(_), None, true) => (CoverageVerdict::Materializable, None),
                (Some(_), None, false) => (
                    CoverageVerdict::Skipped,
                    Some(REASON_NO_EXTRACTION_CACHE.to_string()),
                ),
                (rev, reason, _) => {
                    let _ = rev;
                    (
                        CoverageVerdict::Skipped,
                        reason
                            .map(str::to_string)
                            .or(Some(REASON_NO_PARSE_CACHE.to_string())),
                    )
                }
            };
        if let Some(reason) = &reason {
            *by_reason.entry(reason.clone()).or_default() += 1;
        } else {
            materializable += 1;
        }
        papers.push(PaperCoverage {
            paper_id: paper.paper_id.clone(),
            category: Some(paper.category.clone()),
            verdict,
            revision,
            reason,
            has_parse,
            has_extraction,
            has_embedding,
        });
    }
    let skipped = papers.len() - materializable;
    Ok(CoverageReport {
        schema_version: 1,
        totals: CoverageTotals {
            papers: papers.len(),
            materializable,
            skipped,
            by_reason,
        },
        reconcile: CoverageReconcile {
            sweep_ok: ok_papers.len(),
            cache_papers: cache_paper_ids.len(),
            ok_missing_from_cache,
            extra_cache_papers,
        },
        papers,
    })
}

// ─── S02: typed artifact loading for the materialization runner ─────────────

use da_ports::extractor::ExtractedEntity;
use std::fmt;

/// Everything the runner needs to materialize one paper from cache.
#[derive(Debug, Clone)]
pub struct PaperArtifacts {
    pub parsed: ParsedArticle,
    pub entities: Vec<ExtractedEntity>,
    pub embedding: Vec<f32>,
}

/// Typed artifact-loading failures. The runner maps these to FSM skip
/// reasons or fails; none of them panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadArtifactsError {
    NoParseCache,
    AmbiguousParseKeys {
        count: usize,
    },
    NoExtractionCache,
    NoEmbeddingCache,
    EmptyEmbedding,
    PayloadCorrupt {
        stage: &'static str,
        message: String,
    },
    CacheLookupFailed {
        stage: &'static str,
        message: String,
    },
}

impl fmt::Display for LoadArtifactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoParseCache => write!(f, "no parse cache entry"),
            Self::AmbiguousParseKeys { count } => {
                write!(f, "ambiguous parse cache keys: {count}")
            }
            Self::NoExtractionCache => write!(f, "no extraction cache entry"),
            Self::NoEmbeddingCache => write!(f, "no embedding cache entry"),
            Self::EmptyEmbedding => write!(f, "cached embedding is empty"),
            Self::PayloadCorrupt { stage, message } => {
                write!(f, "corrupt {stage} payload: {message}")
            }
            Self::CacheLookupFailed { stage, message } => {
                write!(f, "{stage} cache lookup failed: {message}")
            }
        }
    }
}

impl std::error::Error for LoadArtifactsError {}

fn unique_success_entry(
    cache: &dyn PipelineCache,
    paper_id: &str,
    stage: &'static str,
    missing: LoadArtifactsError,
) -> std::result::Result<da_ports::pipeline_cache::StageCacheKey, LoadArtifactsError> {
    let manifest = cache
        .manifest(paper_id)
        .map_err(|e| LoadArtifactsError::CacheLookupFailed {
            stage: "manifest",
            message: e.to_string(),
        })?;
    let successes: Vec<&da_ports::pipeline_cache::StageManifestEntry> = manifest
        .iter()
        .filter(|e| e.key.stage_id == stage && matches!(e.outcome, StageOutcome::Success { .. }))
        .collect();
    let distinct: std::collections::BTreeSet<&str> =
        successes.iter().map(|e| e.key_sha256.as_str()).collect();
    if distinct.is_empty() {
        return Err(missing);
    }
    if distinct.len() > 1 {
        return Err(LoadArtifactsError::AmbiguousParseKeys {
            count: distinct.len(),
        });
    }
    Ok(successes[0].key.clone())
}

fn lookup_payload(
    cache: &dyn PipelineCache,
    paper_id: &str,
    stage: &'static str,
    key: &da_ports::pipeline_cache::StageCacheKey,
) -> std::result::Result<Vec<u8>, LoadArtifactsError> {
    let _ = paper_id;
    match cache.lookup(key, FAR_FUTURE) {
        Ok(CacheLookup::Hit(hit)) => Ok(hit.payload),
        Ok(_) => Err(LoadArtifactsError::CacheLookupFailed {
            stage,
            message: "successful manifest entry did not produce a hit".into(),
        }),
        Err(e) => Err(LoadArtifactsError::CacheLookupFailed {
            stage,
            message: e.to_string(),
        }),
    }
}

/// Load parse + extraction + embedding artifacts for one paper.
///
/// Every failure is typed ([`LoadArtifactsError`]); a corrupted payload is
/// a `PayloadCorrupt`, never a panic.
pub fn load_paper_artifacts(
    cache: &dyn PipelineCache,
    paper_id: &str,
) -> std::result::Result<PaperArtifacts, LoadArtifactsError> {
    let parse_key = unique_success_entry(
        cache,
        paper_id,
        STAGE_PARSE,
        LoadArtifactsError::NoParseCache,
    )?;
    let parse_bytes = lookup_payload(cache, paper_id, STAGE_PARSE, &parse_key)?;
    let parsed: ParsedArticle =
        serde_json::from_slice(&parse_bytes).map_err(|e| LoadArtifactsError::PayloadCorrupt {
            stage: STAGE_PARSE,
            message: e.to_string(),
        })?;

    let extract_key = unique_success_entry(
        cache,
        paper_id,
        STAGE_EXTRACTION,
        LoadArtifactsError::NoExtractionCache,
    )?;
    let extract_bytes = lookup_payload(cache, paper_id, STAGE_EXTRACTION, &extract_key)?;
    let entities: Vec<ExtractedEntity> =
        serde_json::from_slice(&extract_bytes).map_err(|e| LoadArtifactsError::PayloadCorrupt {
            stage: STAGE_EXTRACTION,
            message: e.to_string(),
        })?;

    let embed_key = unique_success_entry(
        cache,
        paper_id,
        STAGE_EMBEDDING,
        LoadArtifactsError::NoEmbeddingCache,
    )?;
    let embed_bytes = lookup_payload(cache, paper_id, STAGE_EMBEDDING, &embed_key)?;
    let embedding: Vec<f32> =
        serde_json::from_slice(&embed_bytes).map_err(|e| LoadArtifactsError::PayloadCorrupt {
            stage: STAGE_EMBEDDING,
            message: e.to_string(),
        })?;
    if embedding.is_empty() {
        return Err(LoadArtifactsError::EmptyEmbedding);
    }

    Ok(PaperArtifacts {
        parsed,
        entities,
        embedding,
    })
}

// ─── M442: crash-safe resume (prior-run extraction) ────────────────────────

/// Paper ids that reached the terminal `completed` state in a prior run
/// report. Resume skips exactly these; failed and skipped papers are retried
/// or re-derived. A missing prior report is an empty set (fresh run).
pub fn prior_completed_ids(
    prior_report_json: &[u8],
) -> serde_json::Result<std::collections::BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_slice(prior_report_json)?;
    let mut ids = std::collections::BTreeSet::new();
    if let Some(papers) = value.get("papers").and_then(|p| p.as_array()) {
        for paper in papers {
            // PaperRunRecord serializes the FSM state with #[serde(flatten)]
            // and tag = "state": the verdict is the record's top-level
            // "state" string, not a nested object.
            let completed = paper
                .get("state")
                .and_then(|s| s.as_str())
                .is_some_and(|s| s == "completed");
            if completed && let Some(id) = paper.get("paper_id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_adapters::memory_pipeline_cache::MemoryPipelineCache;
    use da_ports::pipeline_cache::{SourceFormat, SourceIdentity, StageCacheKey};
    use std::collections::BTreeMap as Map;

    fn key(article: &str, stage: &str) -> StageCacheKey {
        StageCacheKey {
            source: SourceIdentity {
                article_id: article.into(),
                selected_format: SourceFormat::Pdf,
                pdf_sha256: Some("deadbeef".into()),
                html_sha256: None,
                acquisition_fingerprint: "test".into(),
                normalization_fingerprint: "test".into(),
            },
            stage_id: stage.into(),
            dependency_output_sha256: Map::new(),
            implementation_fingerprint: "test@v1".into(),
            configuration_fingerprint: "cfg=1".into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: "schema@v1".into(),
        }
    }

    fn tei_for(paper_id: &str, revision: Option<u32>) -> String {
        let idno = match revision {
            Some(v) => format!("arXiv:{paper_id}v{v}"),
            None => format!("arXiv:{paper_id}"),
        };
        format!(
            "<TEI><fileDesc><sourceDesc><biblStruct><idno type=\"arxiv\">{idno}</idno></biblStruct></sourceDesc></fileDesc></TEI>"
        )
    }

    fn parsed_json(paper_id: &str, revision: Option<u32>) -> Vec<u8> {
        let parsed = ParsedArticle {
            paper_id: paper_id.into(),
            title: format!("Title {paper_id}"),
            abstract_text: "abstract".into(),
            body_text: "body".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: Some(tei_for(paper_id, revision)),
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: "deadbeef".into(),
        };
        serde_json::to_vec(&parsed).unwrap()
    }

    fn entities_json() -> Vec<u8> {
        serde_json::to_vec::<Vec<da_ports::extractor::ExtractedEntity>>(&vec![]).unwrap()
    }

    fn store(cache: &MemoryPipelineCache, article: &str, stage: &str, payload: &[u8]) {
        cache
            .store_success(
                &key(article, stage),
                payload,
                Map::new(),
                "2026-08-22T00:00:00Z",
            )
            .unwrap();
    }

    fn ok_paper(id: &str) -> SweepOkPaper {
        SweepOkPaper {
            paper_id: id.into(),
            category: "cs-cl".into(),
        }
    }

    fn audit_one(cache: &MemoryPipelineCache, paper: SweepOkPaper) -> PaperCoverage {
        let ids = vec![paper.paper_id.clone()];
        let report = audit(cache, &[paper], &ids).unwrap();
        report.papers.into_iter().next().unwrap()
    }

    #[test]
    fn bound_paper_with_extraction_is_materializable() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.00001",
            STAGE_PARSE,
            &parsed_json("2401.00001", Some(3)),
        );
        store(&cache, "2401.00001", STAGE_EXTRACTION, &entities_json());
        store(&cache, "2401.00001", STAGE_EMBEDDING, &[1, 2, 3]);
        let cov = audit_one(&cache, ok_paper("2401.00001"));
        assert_eq!(cov.verdict, CoverageVerdict::Materializable);
        assert_eq!(cov.revision, Some(3));
        assert_eq!(cov.reason, None);
        assert!(cov.has_parse && cov.has_extraction && cov.has_embedding);
    }

    #[test]
    fn paper_without_cache_dir_is_typed_skip() {
        let cache = MemoryPipelineCache::new();
        let cov = audit_one(&cache, ok_paper("2401.00002"));
        assert_eq!(cov.verdict, CoverageVerdict::Skipped);
        assert_eq!(cov.reason, Some(REASON_NO_PARSE_CACHE.to_string()));
        assert!(!cov.has_parse);
    }

    #[test]
    fn two_parse_success_keys_are_ambiguous() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.00003",
            STAGE_PARSE,
            &parsed_json("2401.00003", Some(1)),
        );
        // A second, different parse key (different configuration fingerprint).
        let mut second = key("2401.00003", STAGE_PARSE);
        second.configuration_fingerprint = "cfg=2".into();
        cache
            .store_success(
                &second,
                &parsed_json("2401.00003", Some(2)),
                Map::new(),
                "2026-08-22T00:00:00Z",
            )
            .unwrap();
        let cov = audit_one(&cache, ok_paper("2401.00003"));
        assert_eq!(cov.verdict, CoverageVerdict::Skipped);
        assert_eq!(cov.reason, Some(REASON_AMBIGUOUS_PARSE_KEYS.to_string()));
    }

    #[test]
    fn parse_payload_without_tei_xml_is_typed_skip() {
        let cache = MemoryPipelineCache::new();
        let mut parsed =
            serde_json::from_slice::<ParsedArticle>(&parsed_json("2401.00004", None)).unwrap();
        parsed.tei_xml = None;
        store(
            &cache,
            "2401.00004",
            STAGE_PARSE,
            &serde_json::to_vec(&parsed).unwrap(),
        );
        let cov = audit_one(&cache, ok_paper("2401.00004"));
        assert_eq!(cov.reason, Some(REASON_NO_TEI_XML.to_string()));
    }

    #[test]
    fn tei_without_version_suffix_is_typed_skip() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.00005",
            STAGE_PARSE,
            &parsed_json("2401.00005", None),
        );
        store(&cache, "2401.00005", STAGE_EXTRACTION, &entities_json());
        let cov = audit_one(&cache, ok_paper("2401.00005"));
        assert_eq!(cov.reason, Some(REASON_NO_TEI_REVISION.to_string()));
    }

    #[test]
    fn bound_paper_without_extraction_cache_is_typed_skip() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.00006",
            STAGE_PARSE,
            &parsed_json("2401.00006", Some(7)),
        );
        let cov = audit_one(&cache, ok_paper("2401.00006"));
        assert_eq!(cov.verdict, CoverageVerdict::Skipped);
        assert_eq!(cov.reason, Some(REASON_NO_EXTRACTION_CACHE.to_string()));
        assert_eq!(cov.revision, Some(7));
        assert!(cov.has_parse && !cov.has_extraction);
    }

    #[test]
    fn ok_paper_missing_from_cache_ids_is_an_error() {
        let cache = MemoryPipelineCache::new();
        let err = audit(&cache, &[ok_paper("2401.00007")], &[]).unwrap_err();
        assert!(err.to_string().contains("2401.00007"), "err: {err}");
    }

    #[test]
    fn extra_cache_papers_are_reported_not_fatal() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.00008",
            STAGE_PARSE,
            &parsed_json("2401.00008", Some(1)),
        );
        store(&cache, "2401.00008", STAGE_EXTRACTION, &entities_json());
        let ids = vec!["2401.00008".to_string(), "9999.9999".to_string()];
        let report = audit(&cache, &[ok_paper("2401.00008")], &ids).unwrap();
        assert_eq!(
            report.reconcile.extra_cache_papers,
            vec!["9999.9999".to_string()]
        );
        assert_eq!(report.totals.materializable, 1);
    }

    #[test]
    fn failed_stage_outcomes_do_not_count_as_cache() {
        let cache = MemoryPipelineCache::new();
        cache
            .store_retryable_failure(
                &key("2401.00009", STAGE_PARSE),
                "grobid_5xx",
                "boom",
                "2026-08-22T01:00:00Z",
                "2026-08-22T00:00:00Z",
            )
            .unwrap();
        let cov = audit_one(&cache, ok_paper("2401.00009"));
        assert_eq!(cov.reason, Some(REASON_NO_PARSE_CACHE.to_string()));
    }

    #[test]
    fn loader_returns_artifacts_on_full_cache() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.01001",
            STAGE_PARSE,
            &parsed_json("2401.01001", Some(2)),
        );
        store(&cache, "2401.01001", STAGE_EXTRACTION, &entities_json());
        store(
            &cache,
            "2401.01001",
            STAGE_EMBEDDING,
            &serde_json::to_vec(&vec![0.5f32, 0.5]).unwrap(),
        );
        let arts = load_paper_artifacts(&cache, "2401.01001").unwrap();
        assert_eq!(arts.parsed.paper_id, "2401.01001");
        assert!(arts.embedding.len() == 2);
    }

    #[test]
    fn loader_missing_embedding_is_typed() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.01002",
            STAGE_PARSE,
            &parsed_json("2401.01002", Some(2)),
        );
        store(&cache, "2401.01002", STAGE_EXTRACTION, &entities_json());
        match load_paper_artifacts(&cache, "2401.01002") {
            Err(LoadArtifactsError::NoEmbeddingCache) => {}
            other => panic!("expected NoEmbeddingCache, got {other:?}"),
        }
    }

    #[test]
    fn loader_empty_embedding_is_typed() {
        let cache = MemoryPipelineCache::new();
        store(
            &cache,
            "2401.01003",
            STAGE_PARSE,
            &parsed_json("2401.01003", Some(2)),
        );
        store(&cache, "2401.01003", STAGE_EXTRACTION, &entities_json());
        store(
            &cache,
            "2401.01003",
            STAGE_EMBEDDING,
            &serde_json::to_vec(&Vec::<f32>::new()).unwrap(),
        );
        match load_paper_artifacts(&cache, "2401.01003") {
            Err(LoadArtifactsError::EmptyEmbedding) => {}
            other => panic!("expected EmptyEmbedding, got {other:?}"),
        }
    }

    #[test]
    fn loader_corrupt_parse_payload_is_typed_not_panic() {
        let cache = MemoryPipelineCache::new();
        store(&cache, "2401.01004", STAGE_PARSE, b"{not json");
        store(&cache, "2401.01004", STAGE_EXTRACTION, &entities_json());
        store(
            &cache,
            "2401.01004",
            STAGE_EMBEDDING,
            &serde_json::to_vec(&vec![1.0f32]).unwrap(),
        );
        match load_paper_artifacts(&cache, "2401.01004") {
            Err(LoadArtifactsError::PayloadCorrupt { stage, .. }) => assert_eq!(stage, STAGE_PARSE),
            other => panic!("expected PayloadCorrupt, got {other:?}"),
        }
    }

    #[test]
    fn prior_completed_extracts_only_completed() {
        let json = br#"{"papers":[
            {"paper_id":"1111.1111","state":"completed"},
            {"paper_id":"2222.2222","state":"skipped"},
            {"paper_id":"3333.3333","state":"failed"}
        ]}"#;
        let ids = prior_completed_ids(json).unwrap();
        assert!(ids.contains("1111.1111"));
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn prior_completed_empty_and_missing_papers_array() {
        assert!(prior_completed_ids(b"{}").unwrap().is_empty());
        assert!(prior_completed_ids(b"[]").unwrap().is_empty());
    }

    #[test]
    fn prior_completed_corrupt_json_is_typed_error() {
        assert!(prior_completed_ids(b"{nope").is_err());
    }

    #[test]
    fn sweep_report_loader_filters_ok_only() {
        let dir = std::env::temp_dir().join("da_m440_sweep_loader_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sweep.json");
        let json = r#"{
            "papers": {
                "1111.1111": {"paper_id": "1111.1111", "category": "cs-cl", "outcome": "ok"},
                "2222.2222": {"paper_id": "2222.2222", "category": "cs-cl", "outcome": "skipped_source"}
            }
        }"#;
        std::fs::write(&path, json).unwrap();
        let ok = load_sweep_ok_papers(&path).unwrap();
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].paper_id, "1111.1111");
    }
}
