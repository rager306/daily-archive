//! Article load use case — single entry for universal acquisition (ADR-056 / D220).
//!
//! Orchestrates resolve → ensure_assets → (optional later stages) → LoadReport.
//! This module implements **ensure-only** and report emission. Ingest/extract
//! attach in later waves via existing IngestUseCase; not forked here.

use da_domain::{
    AcquisitionPolicy, ArticleLocator, LOAD_REPORT_SCHEMA_VERSION, LoadAssetRecord, LoadFinding,
    LoadQueueRecord, LoadReport, LoadRunMode, LoadStageName, LoadStageSet, StageStatus,
};
use da_ports::{AcquisitionError, SourceAcquisitionPort};
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArticleLoadError {
    #[error("invalid locator: {0}")]
    Locator(String),
    #[error("acquisition: {0}")]
    Acquisition(#[from] AcquisitionError),
}

/// One loader for all providers (ADR-056 §2.2).
pub struct ArticleLoadUseCase<'a, A: SourceAcquisitionPort + ?Sized> {
    pub acquisition: &'a A,
}

impl<'a, A: SourceAcquisitionPort + ?Sized> ArticleLoadUseCase<'a, A> {
    pub fn new(acquisition: &'a A) -> Self {
        Self { acquisition }
    }

    /// Run load according to policy stages. Ensure-only is fully implemented;
    /// ingest/extract stages are recorded as `skipped` until later waves wire them.
    pub fn run(&self, locator_raw: &str, policy: &AcquisitionPolicy) -> LoadReport {
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
                r.seal_digest();
                return r;
            }
        };

        report.provider = self.acquisition.provider_code().to_string();
        report.queue = LoadQueueRecord {
            run_mode: policy.run_mode,
            tasks_opened: Vec::new(),
            tasks_completed: Vec::new(),
        };
        report.import_eligible = policy.import_eligible_stamp();

        // resolve
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
            report.seal_digest();
            return report;
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
                report.seal_digest();
                return report;
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

        // ensure_assets
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
            self.stamp_skipped_downstream(&mut report, policy);
            report.stages.push(StageStatus {
                stage: LoadStageName::EmitLoadReport,
                status: "ok".into(),
                duration_ms: Some(0),
                detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
            });
            report.seal_digest();
            return report;
        }

        match self.acquisition.ensure_assets(&resolved, policy) {
            Ok(assets) => {
                let status = if assets.catalog_hit {
                    "catalog_hit"
                } else {
                    "ok"
                };
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
                            da_domain::AssetDisposition::Ok
                        },
                    });
                }
                report.stages.push(StageStatus {
                    stage: LoadStageName::EnsureAssets,
                    status: status.into(),
                    duration_ms: Some(t1.elapsed().as_millis() as u64),
                    detail: Some(assets.primary.path.clone()),
                });
                report.ok = true;
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
            }
        }

        self.stamp_skipped_downstream(&mut report, policy);

        report.stages.push(StageStatus {
            stage: LoadStageName::EmitLoadReport,
            status: "ok".into(),
            duration_ms: Some(0),
            detail: Some(LOAD_REPORT_SCHEMA_VERSION.into()),
        });
        // D127: never unlock via ensure success alone
        if !policy.enable_import {
            report.import_eligible = false;
        }
        report.duration_ms = started.elapsed().as_millis() as u64;
        report.seal_digest();
        report
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
        // Later waves wire ingest/extract; for now mark not_implemented when requested.
        if policy.stages.includes_ingest() {
            report.stages.push(StageStatus {
                stage: LoadStageName::Ingest,
                status: "not_implemented".into(),
                duration_ms: None,
                detail: Some("W3 ensure-only; ingest binds IngestUseCase in later wave".into()),
            });
            report.findings.push(LoadFinding {
                code: "ingest_not_wired".into(),
                message: "ingest stage not yet bound in ArticleLoadUseCase".into(),
            });
            // Honesty: requested but not done → not fully ok for full stages
            if report.ok {
                report.ok = matches!(policy.stages, LoadStageSet::Ensure);
            }
        }
        if policy.stages.includes_extract() {
            report.stages.push(StageStatus {
                stage: LoadStageName::Extract,
                status: "not_implemented".into(),
                duration_ms: None,
                detail: Some("extract binds ExtractionUseCase in later wave".into()),
            });
        }
        if policy.stages.includes_export_snapshot()
            && !matches!(policy.stages, LoadStageSet::Ensure)
        {
            report.stages.push(StageStatus {
                stage: LoadStageName::ExportSnapshot,
                status: "not_implemented".into(),
                duration_ms: None,
                detail: Some("snapshot export in later wave".into()),
            });
        }
        let _ = LoadRunMode::Inline; // keep import used for doc clarity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;
    use da_ports::MockSourceAcquisition;

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
        assert_eq!(report.source_code, "arxiv");
        assert_eq!(report.graph_readiness, "not_claimed");
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
        assert!(!report.assets.is_empty());
        assert!(report.report_digest_sha256.is_some());
    }

    #[test]
    fn ensure_miss_network_denied_report_not_ok() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let policy = AcquisitionPolicy::ensure_only();
        let report = uc.run("9999.99999", &policy);
        assert!(!report.ok);
        assert!(!report.import_eligible);
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
        assert!(!report.import_eligible);
    }

    #[test]
    fn enable_import_only_when_policy_says() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let mut policy = AcquisitionPolicy::ensure_only();
        policy.enable_import = true;
        let report = uc.run("2602.04837", &policy);
        // Still only stamps what policy asked — product must not auto-unlock elsewhere.
        assert!(report.import_eligible);
        assert!(report.ok);
    }
}
