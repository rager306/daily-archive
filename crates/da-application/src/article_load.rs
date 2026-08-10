//! Article load use case — single entry for universal acquisition (ADR-056 / D220).
//!
//! resolve → ensure_assets → optional content ingest → LoadReport.
//! Acquisition never calls network from this crate; ingest is injected.

use da_domain::{
    AcquisitionPolicy, ArticleLocator, LOAD_REPORT_SCHEMA_VERSION, LoadAssetRecord, LoadFinding,
    LoadQueueRecord, LoadReport, LoadStageName, LoadStageSet, StageStatus,
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
    #[error("ingest: {0}")]
    Ingest(String),
}

/// Outcome of content ingest bound under the load plane (not a second product).
#[derive(Debug, Clone)]
pub struct LoadIngestOutcome {
    pub paper_id: String,
    pub title: String,
    pub body_chars: usize,
    pub section_count: usize,
    pub citation_count: usize,
    pub snapshot_path: Option<String>,
    pub import_eligible: bool,
}

/// Injected content stage — CLI/adapters compose IngestUseCase behind this.
#[async_trait::async_trait]
pub trait LoadContentIngest: Send + Sync {
    async fn ingest_primary(
        &self,
        paper_id: &str,
        primary_path: &Path,
        enable_import: bool,
        snapshot_output: Option<&Path>,
    ) -> Result<LoadIngestOutcome, String>;
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
                    "ingest requires async run_with_ingest with LoadContentIngest bound".into(),
                ),
            });
            report.errors.push(
                "ingest stage requested but run() is ensure-only; use run_with_ingest".into(),
            );
            report.ok = false;
        } else {
            self.stamp_skipped_downstream(&mut report, policy, /*ingest_done*/ false);
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

    /// Full load path: ensure then optional ingest via injected port.
    pub async fn run_with_ingest<I: LoadContentIngest + ?Sized>(
        &self,
        locator_raw: &str,
        policy: &AcquisitionPolicy,
        ingest: Option<&I>,
        snapshot_output: Option<&Path>,
    ) -> LoadReport {
        let started = Instant::now();
        let (mut report, ensure_ok) = self.run_through_ensure(locator_raw, policy);

        if policy.dry_run {
            self.stamp_skipped_downstream(&mut report, policy, false);
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

        if !ensure_ok {
            self.stamp_skipped_downstream(&mut report, policy, false);
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

        if policy.stages.includes_ingest() {
            let t = Instant::now();
            match ingest {
                None => {
                    report.stages.push(StageStatus {
                        stage: LoadStageName::Ingest,
                        status: "fail".into(),
                        duration_ms: Some(t.elapsed().as_millis() as u64),
                        detail: Some("no LoadContentIngest bound".into()),
                    });
                    report.errors.push("ingest requested but not bound".into());
                    report.ok = false;
                }
                Some(ing) => {
                    let paper_id = report
                        .unit_key
                        .clone()
                        .unwrap_or_else(|| report.locator.canonical_key().to_string());
                    let primary_rel = report
                        .assets
                        .iter()
                        .find(|a| a.kind == "pdf" || a.kind == "html")
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
                            report.errors.push("no primary asset for ingest".into());
                            report.ok = false;
                        }
                        Some(rel) => {
                            let primary_path = resolve_primary_path(&self.catalog_root, &rel);
                            match ing
                                .ingest_primary(
                                    &paper_id,
                                    &primary_path,
                                    policy.enable_import,
                                    snapshot_output,
                                )
                                .await
                            {
                                Ok(out) => {
                                    report.stages.push(StageStatus {
                                        stage: LoadStageName::Ingest,
                                        status: "ok".into(),
                                        duration_ms: Some(t.elapsed().as_millis() as u64),
                                        detail: Some(format!(
                                            "title={} sections={} body_chars={}",
                                            out.title, out.section_count, out.body_chars
                                        )),
                                    });
                                    if let Some(snap) = out.snapshot_path {
                                        report.snapshot_path = Some(snap.clone());
                                        report.stages.push(StageStatus {
                                            stage: LoadStageName::ExportSnapshot,
                                            status: "ok".into(),
                                            duration_ms: None,
                                            detail: Some(snap),
                                        });
                                    } else if policy.stages.includes_export_snapshot() {
                                        report.stages.push(StageStatus {
                                            stage: LoadStageName::ExportSnapshot,
                                            status: "skipped".into(),
                                            duration_ms: None,
                                            detail: Some("no snapshot path requested".into()),
                                        });
                                    }
                                    // D127: report stamp follows policy, not ingest unlock alone
                                    if !policy.enable_import {
                                        report.import_eligible = false;
                                    } else {
                                        report.import_eligible = out.import_eligible;
                                    }
                                    report.findings.push(LoadFinding {
                                        code: "ingest_ok".into(),
                                        message: format!(
                                            "citations={} sections={}",
                                            out.citation_count, out.section_count
                                        ),
                                    });
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
        } else {
            self.stamp_skipped_downstream(&mut report, policy, false);
        }

        if policy.stages.includes_extract() {
            report.stages.push(StageStatus {
                stage: LoadStageName::Extract,
                status: "skipped".into(),
                duration_ms: None,
                detail: Some("extract not in this load path (use da extract)".into()),
            });
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

    /// Returns (report skeleton after ensure, ensure_ok).
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

    fn stamp_skipped_downstream(
        &self,
        report: &mut LoadReport,
        policy: &AcquisitionPolicy,
        ingest_done: bool,
    ) {
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
        if policy.stages.includes_ingest() && !ingest_done {
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
                detail: Some("extract not in this load path".into()),
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
    // Also try CWD-relative (report paths may be repo-relative).
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

    struct RecordingIngest {
        calls: Mutex<Vec<(String, String)>>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl LoadContentIngest for RecordingIngest {
        async fn ingest_primary(
            &self,
            paper_id: &str,
            primary_path: &Path,
            _enable_import: bool,
            snapshot_output: Option<&Path>,
        ) -> Result<LoadIngestOutcome, String> {
            self.calls
                .lock()
                .unwrap()
                .push((paper_id.to_string(), primary_path.display().to_string()));
            if self.fail {
                return Err("ingest boom".into());
            }
            Ok(LoadIngestOutcome {
                paper_id: paper_id.into(),
                title: "T".into(),
                body_chars: 10,
                section_count: 2,
                citation_count: 1,
                snapshot_path: snapshot_output.map(|p| p.display().to_string()),
                import_eligible: false,
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

        assert_eq!(report.schema_version, LOAD_REPORT_SCHEMA_VERSION);
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(!report.import_eligible);
        assert_eq!(report.provider, "arxiv");
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::EnsureAssets && s.status == "catalog_hit")
        );
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::Ingest && s.status == "skipped")
        );
    }

    #[test]
    fn ensure_miss_network_denied_report_not_ok() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy::ensure_only();
        let report = uc.run("9999.99999", &policy);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("network") || e.contains("allow_network"))
        );
    }

    #[test]
    fn empty_locator_fails_closed() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let report = uc.run("  ", &AcquisitionPolicy::ensure_only());
        assert!(!report.ok);
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn dry_run_does_not_require_assets() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let mut policy = AcquisitionPolicy::ensure_only();
        policy.dry_run = true;
        let report = uc.run("2602.04837", &policy);
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::EnsureAssets && s.status == "dry_run")
        );
    }

    #[tokio::test]
    async fn ingest_bound_runs_and_exports_snapshot() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2608.07067",
            "article_catalog/arxiv/cs-ai/2608.07067/source/2608.07067.pdf",
            "sha",
        );
        let ing = RecordingIngest {
            calls: Mutex::new(Vec::new()),
            fail: false,
        };
        let uc = ArticleLoadUseCase::new(&mock).with_catalog_root("/tmp/catalog");
        let policy = AcquisitionPolicy {
            stages: LoadStageSet::EnsureIngest,
            ..AcquisitionPolicy::default()
        };
        let snap = PathBuf::from("/tmp/out.sgsnap");
        let report = uc
            .run_with_ingest("2608.07067", &policy, Some(&ing), Some(snap.as_path()))
            .await;
        assert!(report.ok, "errors={:?}", report.errors);
        assert!(
            report
                .stages
                .iter()
                .any(|s| s.stage == LoadStageName::Ingest && s.status == "ok")
        );
        assert_eq!(report.snapshot_path.as_deref(), Some("/tmp/out.sgsnap"));
        assert_eq!(ing.calls.lock().unwrap().len(), 1);
        assert!(!report.import_eligible);
    }

    #[tokio::test]
    async fn ingest_failure_marks_report_not_ok() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let ing = RecordingIngest {
            calls: Mutex::new(Vec::new()),
            fail: true,
        };
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy {
            stages: LoadStageSet::EnsureIngest,
            ..AcquisitionPolicy::default()
        };
        let report = uc
            .run_with_ingest("2602.04837", &policy, Some(&ing), None)
            .await;
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("ingest boom")));
    }
}
