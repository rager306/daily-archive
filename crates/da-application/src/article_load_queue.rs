//! Explicit enqueue/worker contract for the load plane (ADR-056 §2.5.1 C, W8).
//!
//! One loader, three run modes on the same use case — no second queue
//! product, no JSONL side channel, no parallel enum:
//!
//! - `inline`  — [`ArticleLoadUseCase::run`] / `run_with_content` run the
//!   selected stages now (default).
//! - `enqueue` — [`ArticleLoadUseCase::run_enqueue`]: resolve + ensure run
//!   inline first (fail-closed: an ensure failure opens **no** queue rows),
//!   then the remaining stages become `SchedulerTask` rows in the graph
//!   queue via [`GraphScheduler`]. The report's `queue.tasks_opened` carries
//!   `task_type + vid` for every opened row.
//! - `worker`  — the existing [`SchedulerRunner`] path (`da scheduler-run`):
//!   it drains due tasks through the **same** stage executors the inline
//!   path uses ([`IngestSchedulerExecutor`] calls `IngestUseCase`, etc.), so
//!   no second ingest implementation exists.
//!
//! Queue identity is one `SchedulerTask` node per `(task_type, article_id)`
//! (`scheduler_task_vid`). Re-enqueue reopens completed rows instead of
//! duplicating them, and every queue row stays `import_eligible=false`
//! (D127: queue state is never scientific import authority).

use da_domain::scheduler::{RetryPolicy, TaskType, scheduler_task_vid};
use da_domain::{
    AcquisitionPolicy, LoadFinding, LoadQueueTaskRef, LoadReport, LoadRunMode, LoadStageName,
    StageStatus,
};
use da_ports::SourceAcquisitionPort;
use da_ports::graph_store::DirectGraphStore;

use crate::{ArticleLoadUseCase, GraphScheduler};

/// Stage-set → queue task mapping (ADR-056 §2.5.1 C.3). Priority is owned by
/// [`TaskType::priority`]; no parallel enum is invented here. `ensure`-only
/// stage sets enqueue nothing.
pub fn load_stage_task_types(policy: &AcquisitionPolicy) -> Vec<TaskType> {
    let mut tasks = Vec::new();
    if policy.stages.includes_ingest() {
        tasks.push(TaskType::Ingest);
    }
    if policy.stages.includes_extract() {
        tasks.push(TaskType::Extract);
    }
    tasks
}

/// Open/update one `SchedulerTask` row per remaining load stage and stamp
/// `report.queue.tasks_opened` with `task_type + vid`, plus an honest
/// `enqueued` stage status. Idempotent: a pending row is left untouched, a
/// completed row is reopened — never duplicated.
///
/// Fail-closed: any store error aborts before the report stays green; the
/// caller marks the report failed and clears partial `tasks_opened`.
pub async fn enqueue_remaining_load_stages(
    store: &dyn DirectGraphStore,
    retry_policy: &RetryPolicy,
    policy: &AcquisitionPolicy,
    report: &mut LoadReport,
    now: i64,
) -> Result<(), String> {
    let Some(article_id) = report.unit_key.clone() else {
        return Err("report has no unit_key; cannot address queue rows".into());
    };
    for task_type in load_stage_task_types(policy) {
        GraphScheduler::add_pending_task_to(store, retry_policy, task_type, &article_id, now)
            .await
            .map_err(|e| format!("scheduler enqueue {} failed: {e}", task_type.as_str()))?;
        let vid = scheduler_task_vid(&task_type, &article_id);
        report.queue.tasks_opened.push(LoadQueueTaskRef {
            task_type: task_type.as_str().to_string(),
            vid: Some(vid.clone()),
        });
        let stage = match task_type {
            TaskType::Ingest | TaskType::Reparse => LoadStageName::Ingest,
            TaskType::Extract => LoadStageName::Extract,
            // Load never opens enrich/embed/graph-write rows (ADR-056 §2.5:
            // separate use cases). Defensive only — unreachable via mapping.
            _ => continue,
        };
        report.stages.push(StageStatus {
            stage,
            status: "enqueued".into(),
            duration_ms: Some(0),
            detail: Some(format!(
                "SchedulerTask {} opened (vid={vid}); worker drains via scheduler-run",
                task_type.as_str()
            )),
        });
    }
    report.findings.push(LoadFinding {
        code: "enqueue_ok".into(),
        message: format!(
            "run_mode=enqueue opened {} task row(s); import_eligible=false (D127)",
            report.queue.tasks_opened.len()
        ),
    });
    Ok(())
}

impl<'a, A: SourceAcquisitionPort + ?Sized> ArticleLoadUseCase<'a, A> {
    /// `run_mode=enqueue` path (ADR-056 §2.5.1 C.2): resolve + ensure inline,
    /// then open graph-queue rows for the remaining stages. The worker side
    /// is the existing [`SchedulerRunner`] (`da scheduler-run`); it executes
    /// the same stage functions as inline load.
    ///
    /// Guarantees:
    /// - ensure failure / dry-run opens **no** rows and the report is not ok
    ///   (fail-closed, ADR-056 §2.9);
    /// - `run_mode != Enqueue` is a contract violation and fails closed
    ///   instead of silently running inline;
    /// - queue rows never flip `import_eligible` (D127).
    pub async fn run_enqueue(
        &self,
        store: &dyn DirectGraphStore,
        retry_policy: &RetryPolicy,
        locator_raw: &str,
        policy: &AcquisitionPolicy,
        now: i64,
    ) -> LoadReport {
        if policy.run_mode != LoadRunMode::Enqueue {
            let (mut report, _) = self.run_through_ensure(locator_raw, policy);
            report.errors.push(format!(
                "run_enqueue requires policy.run_mode=enqueue, got {:?}",
                policy.run_mode
            ));
            report.ok = false;
            return report;
        }

        let (mut report, ensure_ok) = self.run_through_ensure(locator_raw, policy);
        if !ensure_ok {
            report.findings.push(LoadFinding {
                code: "ensure_failed_no_enqueue".into(),
                message: "ensure failed or was skipped; no queue rows opened".into(),
            });
            return report;
        }

        report.queue.run_mode = LoadRunMode::Enqueue;
        let ensure_ok_flag = report.ok;
        if let Err(e) =
            enqueue_remaining_load_stages(store, retry_policy, policy, &mut report, now).await
        {
            report.errors.push(e);
            report.ok = false;
            report.queue.tasks_opened.clear();
            return report;
        }
        // Downstream stages are queue work now — the enqueue call itself must
        // not silently become an inline run, but ensure success stays truth.
        report.ok = ensure_ok_flag;
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IngestSchedulerExecutor, SchedulerRunner};
    use da_domain::{AcquisitionPolicy, LoadRunMode, LoadStageName, LoadStageSet};
    use da_ports::MockSourceAcquisition;
    use kg_storage::MockGraphStore;
    use std::sync::Arc;

    /// Fixed epoch for deterministic queue rows (no wall clock anywhere).
    const T0: i64 = 1_784_000_000;

    fn immediate_retry_policy() -> RetryPolicy {
        RetryPolicy {
            initial_delay: 0,
            ..RetryPolicy::default()
        }
    }

    fn enqueue_policy(stages: LoadStageSet) -> AcquisitionPolicy {
        AcquisitionPolicy {
            allow_network: false,
            stages,
            run_mode: LoadRunMode::Enqueue,
            ..AcquisitionPolicy::default()
        }
    }

    async fn scheduler_task_rows(store: &MockGraphStore) -> Vec<(u64, String, String)> {
        let mut rows = Vec::new();
        for (id, label) in store.nodes_snapshot() {
            if label != "SchedulerTask" {
                continue;
            }
            let task_type = store
                .get_node_property_string(id, "task_type")
                .await
                .unwrap_or_default();
            let status = store
                .get_node_property_string(id, "status")
                .await
                .unwrap_or_default();
            rows.push((id, task_type, status));
        }
        rows.sort_by_key(|(id, _, _)| *id);
        rows
    }

    #[tokio::test]
    async fn enqueue_stamps_report_queue_and_opens_one_graph_row() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2602.04837",
            "article_catalog/arxiv/cs-ai/2602.04837/source/2602.04837.pdf",
            "sha",
        );
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let report = uc
            .run_enqueue(
                &store,
                &immediate_retry_policy(),
                "2602.04837",
                &enqueue_policy(LoadStageSet::EnsureIngest),
                T0,
            )
            .await;

        assert!(report.ok, "errors={:?}", report.errors);
        assert_eq!(report.queue.run_mode, LoadRunMode::Enqueue);
        assert!(
            !report.import_eligible,
            "D127: enqueue never unlocks import"
        );
        let expected_vid = scheduler_task_vid(&TaskType::Ingest, "2602.04837");
        assert_eq!(report.queue.tasks_opened.len(), 1);
        assert_eq!(report.queue.tasks_opened[0].task_type, "ingest");
        assert_eq!(
            report.queue.tasks_opened[0].vid.as_deref(),
            Some(expected_vid.as_str())
        );
        // Honest stage statuses: ensure ran, ingest is queue work.
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
                .any(|s| s.stage == LoadStageName::Ingest && s.status == "enqueued")
        );
        // Exactly one SchedulerTask row in the graph.
        let rows = scheduler_task_rows(&store).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].1, "ingest");
        assert_eq!(rows[0].2, "pending");
    }

    #[tokio::test]
    async fn enqueue_extract_stage_set_opens_both_rows() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let report = uc
            .run_enqueue(
                &store,
                &immediate_retry_policy(),
                "2602.04837",
                &enqueue_policy(LoadStageSet::EnsureIngestExtract),
                T0,
            )
            .await;
        assert!(report.ok, "errors={:?}", report.errors);
        let types: Vec<&str> = report
            .queue
            .tasks_opened
            .iter()
            .map(|t| t.task_type.as_str())
            .collect();
        assert_eq!(types, vec!["ingest", "extract"]);
        assert_eq!(scheduler_task_rows(&store).await.len(), 2);
    }

    #[tokio::test]
    async fn ensure_failure_opens_no_queue_rows() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let report = uc
            .run_enqueue(
                &store,
                &immediate_retry_policy(),
                "9999.99999",
                &enqueue_policy(LoadStageSet::EnsureIngest),
                T0,
            )
            .await;
        assert!(!report.ok);
        assert!(
            report.queue.tasks_opened.is_empty(),
            "fail-closed: no rows on ensure failure"
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "ensure_failed_no_enqueue")
        );
        assert_eq!(scheduler_task_rows(&store).await.len(), 0);
    }

    #[tokio::test]
    async fn wrong_run_mode_fails_closed() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let mut policy = enqueue_policy(LoadStageSet::EnsureIngest);
        policy.run_mode = LoadRunMode::Inline;
        let report = uc
            .run_enqueue(&store, &immediate_retry_policy(), "2602.04837", &policy, T0)
            .await;
        assert!(!report.ok);
        assert!(report.queue.tasks_opened.is_empty());
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("run_enqueue requires"))
        );
    }

    #[tokio::test]
    async fn re_enqueue_is_reopen_never_duplicate() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let policy = enqueue_policy(LoadStageSet::EnsureIngest);
        let first = uc
            .run_enqueue(&store, &immediate_retry_policy(), "2602.04837", &policy, T0)
            .await;
        assert!(first.ok);
        let second = uc
            .run_enqueue(
                &store,
                &immediate_retry_policy(),
                "2602.04837",
                &policy,
                T0 + 30,
            )
            .await;
        assert!(second.ok, "errors={:?}", second.errors);
        let rows = scheduler_task_rows(&store).await;
        assert_eq!(
            rows.len(),
            1,
            "re-enqueue must reopen, not duplicate: {rows:?}"
        );
        assert_eq!(rows[0].2, "pending");
    }

    // ─── Worker side: same stage functions, no second ingest ────────────────

    struct FixedLocator(Option<std::path::PathBuf>);
    impl da_ports::CatalogPdfLocator for FixedLocator {
        fn locate(&self, _arxiv_id: &str) -> Option<std::path::PathBuf> {
            self.0.clone()
        }
    }

    struct DummyParser;
    #[async_trait::async_trait]
    impl da_ports::parser::ParserPort for DummyParser {
        async fn parse_pdf(
            &self,
            _pdf_path: &str,
            paper_id: &str,
        ) -> da_ports::parser::ParseResult<da_ports::parser::ParsedArticle> {
            Ok(da_ports::parser::ParsedArticle {
                paper_id: paper_id.to_string(),
                title: format!("Worker Title {paper_id}"),
                abstract_text: "abs".into(),
                body_text: "x".repeat(32),
                sections: vec![da_ports::parser::Section {
                    title: "Intro".into(),
                    text: "body".into(),
                    level: 1,
                    ..Default::default()
                }],
                citations: Vec::new(),
                layout_json: None,
                tei_xml: None,
                tables: Vec::new(),
                table_missingness: None,
                formulas: Vec::new(),
                formula_missingness: None,
                page_text_missingness: None,
                declared_arxiv_id: None,
                declared_id_missingness: None,
                header_authors: Vec::new(),
                figures: Vec::new(),
                visual_callouts: Vec::new(),
                pdf_hash: "hash".into(),
            })
        }
    }

    struct DummyEmbedder;
    #[async_trait::async_trait]
    impl da_ports::embedder::Embedder for DummyEmbedder {
        async fn embed(&self, _text: &str) -> da_ports::embedder::EmbedResult<Vec<f32>> {
            Ok(vec![0.1; 8])
        }
        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> da_ports::embedder::EmbedResult<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1; 8]).collect())
        }
        fn dimensions(&self) -> usize {
            8
        }
        fn model_id(&self) -> &str {
            "dummy"
        }
    }

    struct NoOpOpenAlex;
    #[async_trait::async_trait]
    impl da_ports::openalex::OpenAlexClient for NoOpOpenAlex {
        async fn fetch_by_arxiv_id(
            &self,
            _arxiv_id: &str,
        ) -> Result<da_ports::openalex::OpenAlexWork, da_ports::openalex::OpenAlexError> {
            Err(da_ports::openalex::OpenAlexError::Network("noop".into()))
        }
        async fn search(
            &self,
            _query: &str,
            _limit: usize,
        ) -> Result<Vec<da_ports::openalex::OpenAlexWork>, da_ports::openalex::OpenAlexError>
        {
            Ok(vec![])
        }
    }

    fn fixed_clock(secs: i64) -> Arc<dyn da_ports::Clock> {
        Arc::new(da_ports::clock::FixedClock(
            chrono::TimeZone::timestamp_opt(&chrono::Utc, secs, 0).unwrap(),
        ))
    }

    /// The W8 round trip: enqueue → worker completes the SAME stage the
    /// inline path would run, through the shared graph queue, on one node.
    #[tokio::test]
    async fn worker_completes_enqueued_ingest_via_same_stage_executor() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("2602.04837", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();

        // Operator hands the worker a real catalog PDF (locator hit).
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("2602.04837.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 worker input").unwrap();

        let policy = enqueue_policy(LoadStageSet::EnsureIngest);
        let report = uc
            .run_enqueue(&store, &immediate_retry_policy(), "2602.04837", &policy, T0)
            .await;
        assert!(report.ok, "errors={:?}", report.errors);

        let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
        let runner = SchedulerRunner::new(graph.clone(), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 1))
            .with_executor(Arc::new(IngestSchedulerExecutor::new(
                Arc::new(FixedLocator(Some(pdf))),
                Box::new(DummyParser),
                Box::new(DummyEmbedder),
                graph.clone(),
            )));
        let run = runner.run_due().await.unwrap();
        assert_eq!(run.completed, 1, "outcomes={:?}", run.outcomes);
        assert_eq!(run.due, 1);

        // The worker produced the same artifact the inline path would:
        // a materialized Paper node (IngestUseCase, not a fork).
        let paper = graph
            .find_node_by_string_property("Paper", "arxiv_id", "2602.04837")
            .await
            .expect("Paper materialized by worker");
        let title = graph
            .get_node_property_string(paper, "title")
            .await
            .unwrap();
        assert_eq!(title, "Worker Title 2602.04837");

        // Completed queue row — still exactly one node.
        let rows = scheduler_task_rows(&store).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, "completed");
    }

    /// Re-enqueue after completion reopens the row for reprocessing
    /// (ADR-056 §2.5.1 C.4) — dual-run idempotence on one node.
    #[tokio::test]
    async fn re_enqueue_after_worker_completion_reopens_row() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("2602.04837", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let store = MockGraphStore::new();
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("2602.04837.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 reopen").unwrap();

        let policy = enqueue_policy(LoadStageSet::EnsureIngest);
        let report = uc
            .run_enqueue(&store, &immediate_retry_policy(), "2602.04837", &policy, T0)
            .await;
        assert!(report.ok);

        let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
        let runner = SchedulerRunner::new(graph.clone(), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 1))
            .with_executor(Arc::new(IngestSchedulerExecutor::new(
                Arc::new(FixedLocator(Some(pdf))),
                Box::new(DummyParser),
                Box::new(DummyEmbedder),
                graph.clone(),
            )));
        assert_eq!(runner.run_due().await.unwrap().completed, 1);

        let again = uc
            .run_enqueue(
                &store,
                &immediate_retry_policy(),
                "2602.04837",
                &policy,
                T0 + 60,
            )
            .await;
        assert!(again.ok, "errors={:?}", again.errors);
        let rows = scheduler_task_rows(&store).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].2, "pending", "completed row reopened");
    }

    #[test]
    fn stage_set_mapping_matches_adr_table() {
        // ADR-056 §2.5.1 C.3: ensure-only enqueues nothing; ingest → Ingest
        // (High); ingest+extract → Ingest then Extract (Medium).
        assert!(load_stage_task_types(&enqueue_policy(LoadStageSet::Ensure)).is_empty());
        assert_eq!(
            load_stage_task_types(&enqueue_policy(LoadStageSet::EnsureIngest)),
            vec![TaskType::Ingest]
        );
        assert_eq!(
            load_stage_task_types(&enqueue_policy(LoadStageSet::EnsureIngestExtract)),
            vec![TaskType::Ingest, TaskType::Extract]
        );
        assert_eq!(
            TaskType::Ingest.priority(),
            da_domain::scheduler::TaskPriority::High
        );
        assert_eq!(
            TaskType::Extract.priority(),
            da_domain::scheduler::TaskPriority::Medium
        );
    }

    /// Inline run-mode contract: run() fails closed when the policy asks for
    /// enqueue — no silent degradation to inline execution.
    #[test]
    fn run_rejects_enqueue_mode() {
        let mock = MockSourceAcquisition::catalog_hit_pdf("x", "p.pdf", "s");
        let uc = ArticleLoadUseCase::new(&mock);
        let mut policy = enqueue_policy(LoadStageSet::EnsureIngest);
        policy.run_mode = LoadRunMode::Enqueue;
        let report = uc.run("2602.04837", &policy);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("run() is inline-only"))
        );
    }
}
