//! Article load use case — single entry for universal acquisition (ADR-056 / D220).
//!
//! resolve → ensure_assets → optional content pipeline (ingest / extract) → LoadReport.
//! Acquisition never calls network from this crate; content is injected.

use da_domain::{
    AcquisitionPolicy, ArticleLocator, LOAD_REPORT_SCHEMA_VERSION, LoadAssetRecord,
    LoadCacheRecord, LoadFinding, LoadQueueRecord, LoadReport, LoadStageName, LoadStageSet,
    StageStatus,
};
use da_ports::{AcquisitionError, EnsuredAssets, SourceAcquisitionPort};
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArticleLoadError {
    #[error("invalid locator: {0}")]
    Locator(String),
    #[error("acquisition: {0}")]
    Acquisition(#[from] AcquisitionError),
    #[error("content: {0}")]
    Content(String),
}

/// Outcome of content ingest under the load plane.
#[derive(Debug, Clone)]
pub struct LoadIngestOutcome {
    pub paper_id: String,
    pub title: String,
    pub body_chars: usize,
    pub section_count: usize,
    pub citation_count: usize,
    pub snapshot_path: Option<String>,
    pub revision_vid: Option<String>,
    pub import_eligible: bool,
}

/// Outcome of entity extraction under the load plane.
#[derive(Debug, Clone)]
pub struct LoadExtractOutcome {
    pub entities_extracted: usize,
    pub entities_created: usize,
    pub mentions_edges: usize,
    pub found_in_edges: usize,
}

/// Full content stage outcome (one process-local store preferred).
#[derive(Debug, Clone)]
pub struct LoadContentOutcome {
    pub ingest: LoadIngestOutcome,
    pub extract: Option<LoadExtractOutcome>,
    /// PipelineCache dispositions for LoadReport (ADR-056 §2.5.1).
    pub cache_records: Vec<LoadCacheRecord>,
}

/// Request for the injected content pipeline.
#[derive(Debug, Clone)]
pub struct LoadContentRequest<'a> {
    pub paper_id: &'a str,
    pub primary_path: &'a Path,
    /// Typed locator and source identity; avoids reparsing provider/path strings in CLI.
    pub locator: &'a ArticleLocator,
    pub source_code: &'a str,
    /// Exact ensured asset evidence already normalized into load-report records.
    pub assets: &'a [LoadAssetRecord],
    pub enable_import: bool,
    pub include_extract: bool,
    pub snapshot_output: Option<&'a Path>,
}

/// Injected content stages — CLI composes IngestUseCase + ExtractionUseCase.
#[async_trait::async_trait]
pub trait LoadContentPipeline: Send + Sync {
    async fn run_content(&self, req: LoadContentRequest<'_>) -> Result<LoadContentOutcome, String>;
}

/// One loader for all providers (ADR-056 §2.2).
pub struct ArticleLoadUseCase<'a, A: SourceAcquisitionPort + ?Sized> {
    pub acquisition: &'a A,
    /// Absolute or CWD-relative root used to resolve relative primary paths.
    pub catalog_root: PathBuf,
}

impl<'a, A: SourceAcquisitionPort + ?Sized> ArticleLoadUseCase<'a, A> {
    pub fn new(acquisition: &'a A) -> Self {
        Self {
            acquisition,
            catalog_root: PathBuf::from("."),
        }
    }

    pub fn with_catalog_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.catalog_root = root.into();
        self
    }

    /// Ensure-only (and dry-run) path — no graph writes.
    pub fn run(&self, locator_raw: &str, policy: &AcquisitionPolicy) -> LoadReport {
        let (mut report, _ensure_ok) = self.run_through_ensure(locator_raw, policy);
        if policy.stages.includes_ingest() && !policy.dry_run {
            report.stages.push(StageStatus {
                stage: LoadStageName::Ingest,
                status: "fail".into(),
                duration_ms: None,
                detail: Some(
                    "ingest requires async run_with_content with LoadContentPipeline bound".into(),
                ),
            });
            report.errors.push(
                "ingest stage requested but run() is ensure-only; use run_with_content".into(),
            );
            report.ok = false;
        } else {
            self.stamp_skipped_downstream(&mut report, policy);
        }
        report.stages.push(StageStatus {
            stage: LoadStageName::EmitLoadReport,
            status: "ok".into(),
            duration_ms: Some(0),
            detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
        });
        if !policy.enable_import {
            report.import_eligible = false;
        }
        report.seal_digest();
        report
    }

    /// Full load path: ensure then optional content pipeline.
    pub async fn run_with_content<P: LoadContentPipeline + ?Sized>(
        &self,
        locator_raw: &str,
        policy: &AcquisitionPolicy,
        content: Option<&P>,
        snapshot_output: Option<&Path>,
    ) -> LoadReport {
        let started = Instant::now();
        let (mut report, ensure_ok) = self.run_through_ensure(locator_raw, policy);

        if policy.dry_run || !ensure_ok {
            self.stamp_skipped_downstream(&mut report, policy);
            report.stages.push(StageStatus {
                stage: LoadStageName::EmitLoadReport,
                status: "ok".into(),
                duration_ms: Some(0),
                detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
            });
            report.duration_ms = started.elapsed().as_millis() as u64;
            if !policy.enable_import {
                report.import_eligible = false;
            }
            report.seal_digest();
            return report;
        }

        if !policy.stages.includes_ingest() {
            self.stamp_skipped_downstream(&mut report, policy);
            report.stages.push(StageStatus {
                stage: LoadStageName::EmitLoadReport,
                status: "ok".into(),
                duration_ms: Some(0),
                detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
            });
            report.duration_ms = started.elapsed().as_millis() as u64;
            if !policy.enable_import {
                report.import_eligible = false;
            }
            report.seal_digest();
            return report;
        }

        let t = Instant::now();
        match content {
            None => {
                report.stages.push(StageStatus {
                    stage: LoadStageName::Ingest,
                    status: "fail".into(),
                    duration_ms: Some(t.elapsed().as_millis() as u64),
                    detail: Some("no LoadContentPipeline bound".into()),
                });
                report
                    .errors
                    .push("content pipeline requested but not bound".into());
                report.ok = false;
            }
            Some(pipe) => {
                let paper_id = report
                    .unit_key
                    .clone()
                    .unwrap_or_else(|| report.locator.canonical_key().to_string());
                let primary_rel = report
                    .assets
                    .iter()
                    .find(|a| a.kind == "pdf")
                    .or_else(|| report.assets.iter().find(|a| a.kind == "html"))
                    .map(|a| a.path.clone())
                    .or_else(|| report.assets.first().map(|a| a.path.clone()));
                match primary_rel {
                    None => {
                        report.stages.push(StageStatus {
                            stage: LoadStageName::Ingest,
                            status: "fail".into(),
                            duration_ms: Some(t.elapsed().as_millis() as u64),
                            detail: Some("no primary asset path".into()),
                        });
                        report
                            .errors
                            .push("no primary asset for content pipeline".into());
                        report.ok = false;
                    }
                    Some(rel) => {
                        let primary_path = resolve_primary_path(&self.catalog_root, &rel);
                        let req = LoadContentRequest {
                            paper_id: &paper_id,
                            primary_path: &primary_path,
                            locator: &report.locator,
                            source_code: &report.source_code,
                            assets: &report.assets,
                            enable_import: policy.enable_import,
                            include_extract: policy.stages.includes_extract(),
                            snapshot_output,
                        };
                        match pipe.run_content(req).await {
                            Ok(out) => {
                                let ing = &out.ingest;
                                report.stages.push(StageStatus {
                                    stage: LoadStageName::Ingest,
                                    status: "ok".into(),
                                    duration_ms: Some(t.elapsed().as_millis() as u64),
                                    detail: Some(format!(
                                        "title={} sections={} body_chars={}",
                                        ing.title, ing.section_count, ing.body_chars
                                    )),
                                });
                                report.findings.push(LoadFinding {
                                    code: "ingest_ok".into(),
                                    message: format!(
                                        "title={} citations={} sections={}",
                                        ing.title, ing.citation_count, ing.section_count
                                    ),
                                });
                                if let Some(revision_vid) = &ing.revision_vid {
                                    report.findings.push(LoadFinding {
                                        code: "revision_graph_ok".into(),
                                        message: format!(
                                            "revision_vid={revision_vid} import_eligible=false"
                                        ),
                                    });
                                }
                                if let Some(ex) = &out.extract {
                                    report.stages.push(StageStatus {
                                        stage: LoadStageName::Extract,
                                        status: "ok".into(),
                                        duration_ms: None,
                                        detail: Some(format!(
                                            "entities={} created={} mentions={}",
                                            ex.entities_extracted,
                                            ex.entities_created,
                                            ex.mentions_edges
                                        )),
                                    });
                                    report.findings.push(LoadFinding {
                                        code: "extract_ok".into(),
                                        message: format!(
                                            "entities_extracted={} found_in={}",
                                            ex.entities_extracted, ex.found_in_edges
                                        ),
                                    });
                                } else if policy.stages.includes_extract() {
                                    report.stages.push(StageStatus {
                                        stage: LoadStageName::Extract,
                                        status: "skipped".into(),
                                        duration_ms: None,
                                        detail: Some("pipeline returned no extract outcome".into()),
                                    });
                                }
                                if let Some(snap) = &ing.snapshot_path {
                                    report.snapshot_path = Some(snap.clone());
                                    report.stages.push(StageStatus {
                                        stage: LoadStageName::ExportSnapshot,
                                        status: "ok".into(),
                                        duration_ms: None,
                                        detail: Some(snap.clone()),
                                    });
                                } else if policy.stages.includes_export_snapshot() {
                                    report.stages.push(StageStatus {
                                        stage: LoadStageName::ExportSnapshot,
                                        status: "skipped".into(),
                                        duration_ms: None,
                                        detail: Some("no snapshot path".into()),
                                    });
                                }
                                if !policy.enable_import {
                                    report.import_eligible = false;
                                } else {
                                    report.import_eligible = ing.import_eligible;
                                }
                                report.cache.extend(out.cache_records);
                            }
                            Err(e) => {
                                report.stages.push(StageStatus {
                                    stage: LoadStageName::Ingest,
                                    status: "fail".into(),
                                    duration_ms: Some(t.elapsed().as_millis() as u64),
                                    detail: Some(e.clone()),
                                });
                                report.errors.push(e);
                                report.ok = false;
                            }
                        }
                    }
                }
            }
        }

        report.stages.push(StageStatus {
            stage: LoadStageName::EmitLoadReport,
            status: "ok".into(),
            duration_ms: Some(0),
            detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
        });
        if !policy.enable_import {
            report.import_eligible = false;
        }
        report.duration_ms = started.elapsed().as_millis() as u64;
        report.seal_digest();
        report
    }

    /// Backward-compatible alias used by earlier CLI wiring.
    pub async fn run_with_ingest<P: LoadContentPipeline + ?Sized>(
        &self,
        locator_raw: &str,
        policy: &AcquisitionPolicy,
        content: Option<&P>,
        snapshot_output: Option<&Path>,
    ) -> LoadReport {
        self.run_with_content(locator_raw, policy, content, snapshot_output)
            .await
    }

    fn run_through_ensure(
        &self,
        locator_raw: &str,
        policy: &AcquisitionPolicy,
    ) -> (LoadReport, bool) {
        let started = Instant::now();
        let mut report = match ArticleLocator::parse(locator_raw) {
            Ok(locator) => LoadReport::new_skeleton(locator_raw, locator),
            Err(e) => {
                let mut r = LoadReport::new_skeleton(
                    locator_raw,
                    ArticleLocator::Raw {
                        value: locator_raw.to_string(),
                    },
                );
                r.errors.push(e.to_string());
                r.ok = false;
                r.import_eligible = false;
                r.duration_ms = started.elapsed().as_millis() as u64;
                r.stages.push(StageStatus {
                    stage: LoadStageName::Resolve,
                    status: "fail".into(),
                    duration_ms: Some(0),
                    detail: Some(e.to_string()),
                });
                return (r, false);
            }
        };

        report.provider = self.acquisition.provider_code().to_string();
        report.queue = LoadQueueRecord {
            run_mode: policy.run_mode,
            tasks_opened: Vec::new(),
            tasks_completed: Vec::new(),
        };
        report.import_eligible = policy.import_eligible_stamp();

        let t0 = Instant::now();
        let locator = report.locator.clone();
        if !self.acquisition.can_resolve(&locator) {
            report.stages.push(StageStatus {
                stage: LoadStageName::Resolve,
                status: "fail".into(),
                duration_ms: Some(t0.elapsed().as_millis() as u64),
                detail: Some(format!(
                    "provider {} cannot resolve {}",
                    self.acquisition.provider_code(),
                    locator.kind_name()
                )),
            });
            report.errors.push(format!(
                "cannot resolve with provider {}",
                self.acquisition.provider_code()
            ));
            report.ok = false;
            report.duration_ms = started.elapsed().as_millis() as u64;
            return (report, false);
        }

        let resolved = match self.acquisition.resolve(&locator) {
            Ok(r) => r,
            Err(e) => {
                report.stages.push(StageStatus {
                    stage: LoadStageName::Resolve,
                    status: "fail".into(),
                    duration_ms: Some(t0.elapsed().as_millis() as u64),
                    detail: Some(e.to_string()),
                });
                report.errors.push(e.to_string());
                report.ok = false;
                report.duration_ms = started.elapsed().as_millis() as u64;
                return (report, false);
            }
        };

        report.source_code = resolved.source_code.clone();
        report.unit_key = Some(resolved.unit_key.clone());
        if let Some(cp) = &resolved.catalog_path {
            report.category_or_topic = Some(cp.clone());
        }
        // ADR-056/057: report metadata only — never invent graph edges here.
        report.source_profile = resolved.source_profile.clone();
        report.container_ref = resolved.container_ref.clone();
        if report.source_profile.as_deref() == Some("textbook") {
            report.graph_readiness = "not_claimed".into();
        }
        report.stages.push(StageStatus {
            stage: LoadStageName::Resolve,
            status: "ok".into(),
            duration_ms: Some(t0.elapsed().as_millis() as u64),
            detail: None,
        });

        let t1 = Instant::now();
        if policy.dry_run {
            report.stages.push(StageStatus {
                stage: LoadStageName::EnsureAssets,
                status: "dry_run".into(),
                duration_ms: Some(t1.elapsed().as_millis() as u64),
                detail: Some("dry_run: no catalog or network writes".into()),
            });
            report.findings.push(LoadFinding {
                code: "dry_run".into(),
                message: "ensure skipped under dry_run".into(),
            });
            report.ok = true;
            report.duration_ms = started.elapsed().as_millis() as u64;
            return (report, true);
        }

        match self.acquisition.ensure_assets(&resolved, policy) {
            Ok(assets) => {
                apply_assets(&mut report, &assets);
                let status = if assets.catalog_hit {
                    "catalog_hit"
                } else {
                    "fetched"
                };
                report.stages.push(StageStatus {
                    stage: LoadStageName::EnsureAssets,
                    status: status.into(),
                    duration_ms: Some(t1.elapsed().as_millis() as u64),
                    detail: Some(assets.primary.path.clone()),
                });
                report.ok = true;
                report.duration_ms = started.elapsed().as_millis() as u64;
                (report, true)
            }
            Err(e) => {
                report.stages.push(StageStatus {
                    stage: LoadStageName::EnsureAssets,
                    status: "fail".into(),
                    duration_ms: Some(t1.elapsed().as_millis() as u64),
                    detail: Some(e.to_string()),
                });
                report.errors.push(e.to_string());
                report.ok = false;
                report.duration_ms = started.elapsed().as_millis() as u64;
                (report, false)
            }
        }
    }

    fn stamp_skipped_downstream(&self, report: &mut LoadReport, policy: &AcquisitionPolicy) {
        if matches!(policy.stages, LoadStageSet::Ensure) {
            for stage in [
                LoadStageName::Ingest,
                LoadStageName::Extract,
                LoadStageName::ExportSnapshot,
            ] {
                report.stages.push(StageStatus {
                    stage,
                    status: "skipped".into(),
                    duration_ms: None,
                    detail: Some("stage set is ensure-only".into()),
                });
            }
            return;
        }
        if policy.stages.includes_ingest() {
            report.stages.push(StageStatus {
                stage: LoadStageName::Ingest,
                status: "skipped".into(),
                duration_ms: None,
                detail: Some("ingest not run".into()),
            });
        }
        if policy.stages.includes_extract() {
            report.stages.push(StageStatus {
                stage: LoadStageName::Extract,
                status: "skipped".into(),
                duration_ms: None,
                detail: Some("extract not run".into()),
            });
        }
        if policy.stages.includes_export_snapshot() && report.snapshot_path.is_none() {
            report.stages.push(StageStatus {
                stage: LoadStageName::ExportSnapshot,
                status: "skipped".into(),
                duration_ms: None,
                detail: Some("no snapshot".into()),
            });
        }
    }
}

fn apply_assets(report: &mut LoadReport, assets: &EnsuredAssets) {
    report.primary_kind = Some(assets.primary.kind);
    for a in &assets.assets {
        report.assets.push(LoadAssetRecord {
            kind: a.kind.clone(),
            path: a.path.clone(),
            sha256: a.sha256.clone(),
            bytes: a.bytes,
            disposition: a.disposition,
        });
    }
    if report.assets.is_empty() {
        report.assets.push(LoadAssetRecord {
            kind: format!("{:?}", assets.primary.kind).to_ascii_lowercase(),
            path: assets.primary.path.clone(),
            sha256: assets.primary.sha256.clone(),
            bytes: assets.primary.bytes,
            disposition: if assets.catalog_hit {
                da_domain::AssetDisposition::CatalogHit
            } else {
                da_domain::AssetDisposition::Fetched
            },
        });
    }
}

fn resolve_primary_path(catalog_root: &Path, rel_or_abs: &str) -> PathBuf {
    let p = PathBuf::from(rel_or_abs);
    if p.is_absolute() {
        return p;
    }
    let under_root = catalog_root.join(&p);
    if under_root.exists() {
        return under_root;
    }
    if p.exists() {
        return p;
    }
    under_root
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;
    use da_ports::MockSourceAcquisition;
    use std::sync::Mutex;

    struct RecordingPipeline {
        calls: Mutex<Vec<String>>,
        fail: bool,
        with_extract: bool,
    }

    #[async_trait::async_trait]
    impl LoadContentPipeline for RecordingPipeline {
        async fn run_content(
            &self,
            req: LoadContentRequest<'_>,
        ) -> Result<LoadContentOutcome, String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{}:{}", req.paper_id, req.include_extract));
            if self.fail {
                return Err("content boom".into());
            }
            Ok(LoadContentOutcome {
                ingest: LoadIngestOutcome {
                    paper_id: req.paper_id.into(),
                    title: "Real Title From GROBID".into(),
                    body_chars: 10,
                    section_count: 2,
                    citation_count: 1,
                    snapshot_path: req.snapshot_output.map(|p| p.display().to_string()),
                    revision_vid: req
                        .locator
                        .arxiv_revision()
                        .map(|revision| format!("test-revision-{revision}")),
                    import_eligible: false,
                },
                extract: if req.include_extract && self.with_extract {
                    Some(LoadExtractOutcome {
                        entities_extracted: 3,
                        entities_created: 3,
                        mentions_edges: 3,
                        found_in_edges: 2,
                    })
                } else {
                    None
                },
                cache_records: vec![LoadCacheRecord {
                    stage_id: "parse".into(),
                    status: "miss".into(),
                    key_sha256: Some("test-key".into()),
                    payload_sha256: None,
                }],
            })
        }
    }

    #[test]
    fn ensure_only_catalog_hit_report_honest() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2602.04837",
            "data/article_catalog/article_catalog/arxiv/cs-ai/2602.04837/source/2602.04837.pdf",
            "abc123",
        );
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy::ensure_only();
        let report = uc.run("2602.04837", &policy);
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(!report.import_eligible);
        assert_eq!(report.source_profile.as_deref(), Some("scientific_paper"));
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::EnsureAssets && s.status == "catalog_hit")
        );
    }

    #[test]
    fn versioned_locator_report_preserves_revision_but_unit_key_is_canonical() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2506.07853",
            "article_catalog/arxiv/cs-ai/2506.07853/source/2506.07853v1.pdf",
            "v1-sha",
        );
        let uc = ArticleLoadUseCase::new(&mock);
        let report = uc.run(
            "https://arxiv.org/html/2506.07853v1",
            &AcquisitionPolicy::ensure_only(),
        );
        assert!(report.ok, "errors={:?}", report.errors);
        assert_eq!(report.unit_key.as_deref(), Some("2506.07853"));
        assert_eq!(report.locator.canonical_key(), "2506.07853");
        assert_eq!(report.locator.source_key(), "2506.07853v1");
        assert_eq!(report.locator.arxiv_revision(), Some(1));
        assert!(!report.import_eligible);
    }

    #[test]
    fn textbook_ensure_stamps_container_not_graph_edges() {
        let mock = MockSourceAcquisition::catalog_hit_html_textbook(
            "gnn-ch-chapters__01-intro-to-graphs",
            "article_catalog/gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs/source/chapter.html",
            "Graph Neural Networks Textbook",
        );
        let uc = ArticleLoadUseCase::new(&mock);
        let report = uc.run(
            "catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs",
            &AcquisitionPolicy::ensure_only(),
        );
        assert!(report.ok, "errors={:?}", report.errors);
        assert_eq!(report.source_profile.as_deref(), Some("textbook"));
        assert_eq!(
            report.container_ref.as_deref(),
            Some("Graph Neural Networks Textbook")
        );
        assert_eq!(report.graph_readiness, "not_claimed");
        assert!(!report.import_eligible);
    }

    #[test]
    fn ensure_miss_network_denied_report_not_ok() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let report = uc.run("9999.99999", &AcquisitionPolicy::ensure_only());
        assert!(!report.ok);
    }

    #[test]
    fn empty_locator_fails_closed() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let report = uc.run("  ", &AcquisitionPolicy::ensure_only());
        assert!(!report.ok);
    }

    #[test]
    fn dry_run_does_not_require_assets() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy {
            dry_run: true,
            stages: LoadStageSet::Ensure,
            ..AcquisitionPolicy::default()
        };
        let report = uc.run("2602.04837", &policy);
        assert!(report.ok, "errors={:?}", report.errors);
    }

    #[tokio::test]
    async fn content_pipeline_records_real_title_and_snapshot() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2608.07067",
            "article_catalog/arxiv/cs-ai/2608.07067/source/2608.07067.pdf",
            "sha",
        );
        let pipe = RecordingPipeline {
            calls: Mutex::new(Vec::new()),
            fail: false,
            with_extract: false,
        };
        let uc = ArticleLoadUseCase::new(&mock).with_catalog_root("/tmp/catalog");
        let policy = AcquisitionPolicy {
            stages: LoadStageSet::EnsureIngest,
            ..AcquisitionPolicy::default()
        };
        let snap = PathBuf::from("/tmp/out.sgsnap");
        let report = uc
            .run_with_content("2608.07067", &policy, Some(&pipe), Some(snap.as_path()))
            .await;
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(report.stages.iter().any(|s| {
            s.detail
                .as_deref()
                .is_some_and(|d| d.contains("Real Title"))
        }));
        assert_eq!(report.snapshot_path.as_deref(), Some("/tmp/out.sgsnap"));
        assert!(!report.import_eligible);
        assert!(
            report.cache.iter().any(|c| c.stage_id == "parse"),
            "content pipeline must stamp LoadReport.cache (ADR-056)"
        );
    }

    #[tokio::test]
    async fn extract_stage_ok_when_pipeline_returns_extract() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let pipe = RecordingPipeline {
            calls: Mutex::new(Vec::new()),
            fail: false,
            with_extract: true,
        };
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy {
            stages: LoadStageSet::EnsureIngestExtract,
            ..AcquisitionPolicy::default()
        };
        let report = uc
            .run_with_content("2602.04837", &policy, Some(&pipe), None)
            .await;
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::Extract && s.status == "ok")
        );
        assert!(pipe.calls.lock().unwrap()[0].ends_with(":true"));
    }

    #[tokio::test]
    async fn content_failure_marks_report_not_ok() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let pipe = RecordingPipeline {
            calls: Mutex::new(Vec::new()),
            fail: true,
            with_extract: false,
        };
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy {
            stages: LoadStageSet::EnsureIngest,
            ..AcquisitionPolicy::default()
        };
        let report = uc
            .run_with_content("2602.04837", &policy, Some(&pipe), None)
            .await;
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("content boom")));
    }
}
