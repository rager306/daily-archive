//! Deterministic crash-recovery and dependency proof for the generic durable
//! pipeline scheduler (M399/S03 T03).
//!
//! Integration-level counterpart to the runner unit tests: exercises only the
//! public API (`SchedulerRunner` + `SchedulerTaskExecutor`) against a shared
//! in-memory graph store with injected `FixedClock`s. No real sleep, no
//! network, no process spawning — every scenario is deterministic.
//!
//! Proven here (plan verify + must-haves):
//! - lease expiry reclaim with a monotonic fence (Q3 item 1),
//! - worker loss: crashed Running attempts are aborted, history stays complete,
//! - stale-holder-after-reclaim: a resumed crashed worker cannot re-execute,
//!   re-finalize, or re-publish (duplicate execution prevention),
//! - live leases are never stolen; heartbeats extend only the holder's lease,
//! - retry lifecycle keeps the full attempt lineage and publishes exactly one
//!   logical result,
//! - blocked dependencies and unavailable resource classes prevent claims,
//! - deterministic priority ordering (High → Medium → Low),
//! - fail-closed dispatch: unregistered task types are never claimed, while
//!   registered generic types run through the same plane,
//! - the operational surface exposes lease owner/heartbeat/expiry/priority/
//!   resource/attempt/logical-result state; D127 import stays false.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_application::{
    SchedulerExecuteOutcome, SchedulerRunReport, SchedulerRunner, SchedulerTaskDisposition,
    SchedulerTaskExecutor,
};
use da_domain::scheduler::{
    AttemptError, AttemptOutcome, PendingTask, PublishError, RetryPolicy, TaskDependency,
    TaskPriority, TaskResourceClass, TaskStatus, TaskType, logical_result_vid,
};
use da_ports::clock::{Clock, FixedClock};
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::{OpenAlexClient, OpenAlexError, OpenAlexResult, OpenAlexWork};

mod common;
use common::mock_graph_store::MockGraphStore;

/// Fixed epoch for deterministic scenarios (no real sleep anywhere).
const T0: i64 = 1_784_000_000;
/// Default lease term used by most scenarios (expiry lands inside the window).
const LEASE_TERM: i64 = 300;

fn fixed_clock(secs: i64) -> Arc<dyn Clock> {
    Arc::new(FixedClock(Utc.timestamp_opt(secs, 0).unwrap()))
}

struct NoOpOpenAlex;

#[async_trait]
impl OpenAlexClient for NoOpOpenAlex {
    async fn fetch_by_arxiv_id(&self, _arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        Err(OpenAlexError::Network("noop".into()))
    }

    async fn search(&self, _query: &str, _limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        Ok(vec![])
    }
}

fn completed_outcome(summary: &str) -> SchedulerExecuteOutcome {
    SchedulerExecuteOutcome::Completed {
        summary: summary.to_string(),
        evidence_refs: vec!["stage:parse:cache:sha256:cache-key:sha256:abc123".into()],
    }
}

/// Recording fake executor: returns a fixed outcome and optionally logs
/// executed vids in order.
struct FakeExecutor {
    task_type: TaskType,
    outcome: SchedulerExecuteOutcome,
    log: Option<Arc<Mutex<Vec<String>>>>,
}

#[async_trait]
impl SchedulerTaskExecutor for FakeExecutor {
    fn task_type(&self) -> TaskType {
        self.task_type
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        if let Some(log) = &self.log {
            log.lock().unwrap().push(task.task_vid());
        }
        self.outcome.clone()
    }
}

/// Retry-then-succeed executor: retryable until `task.attempts.len()`
/// reaches `succeed_on` (the current pass's claim is already recorded), then
/// Completed. Deterministic without shared mutable counters.
struct CountingExecutor {
    task_type: TaskType,
    succeed_on: u32,
}

#[async_trait]
impl SchedulerTaskExecutor for CountingExecutor {
    fn task_type(&self) -> TaskType {
        self.task_type
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        if (task.attempts.len() as u32) >= self.succeed_on {
            completed_outcome(&format!("success on attempt {}", task.attempts.len()))
        } else {
            SchedulerExecuteOutcome::Retryable {
                error_class: "transient".into(),
                message: "deterministic transient failure".into(),
            }
        }
    }
}

/// Executor that triggers a second worker's full recovery pass mid-execution.
///
/// Simulates the Q3 headline: worker A claims (fence 1) and is paused; its
/// lease expires; worker B reclaims (fence 2), completes the task and
/// publishes the logical result; then A resumes. The runner-level fence
/// check (`durable_lease` re-read) must suppress A's finalize/publish — no
/// duplicate authoritative effects.
struct ReclaimingExecutor {
    task_type: TaskType,
    store: MockGraphStore,
    reclaim_at: i64,
    worker_b: String,
    inner_report: Arc<Mutex<Option<SchedulerRunReport>>>,
}

#[async_trait]
impl SchedulerTaskExecutor for ReclaimingExecutor {
    fn task_type(&self) -> TaskType {
        self.task_type
    }

    async fn execute(&self, _task: &PendingTask) -> SchedulerExecuteOutcome {
        let graph: Arc<dyn DirectGraphStore> = Arc::new(self.store.clone());
        let inner = SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(self.reclaim_at))
            .with_worker_id(self.worker_b.clone())
            .with_lease_term(LEASE_TERM)
            .with_executor(Arc::new(FakeExecutor {
                task_type: self.task_type,
                outcome: completed_outcome("recovered by worker-b"),
                log: None,
            }));
        let report = inner.run_due().await.expect("inner recovery pass");
        *self.inner_report.lock().unwrap() = Some(report);
        // A's own outcome is irrelevant: the fence check after this returns
        // suppresses finalization because the on-disk fence is higher.
        completed_outcome("stale worker finished")
    }
}

fn durable_task(
    arxiv_id: &str,
    task_type: TaskType,
    priority: TaskPriority,
    resource_class: TaskResourceClass,
    dependencies: Vec<TaskDependency>,
    next_retry: i64,
    added_at: i64,
) -> PendingTask {
    let mut task = PendingTask::new_openalex_enrich(arxiv_id, &RetryPolicy::default(), added_at);
    task.task_type = task_type;
    task.priority = priority;
    task.resource_class = resource_class;
    task.dependencies = dependencies;
    task.next_retry = next_retry;
    task
}

/// Seed a durable SchedulerTask node (task_json + flat operational mirrors;
/// `import_eligible` always false — D127).
async fn seed_durable(store: &MockGraphStore, task: &PendingTask) -> u64 {
    let node = store.create_node("SchedulerTask").await.unwrap();
    store
        .set_node_property_string(node, "vid", task.task_vid())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "arxiv_id", task.arxiv_id.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "task_type", task.task_type.as_str().to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "status", "pending".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(node, "schema_version", task.schema_version as i64)
        .await
        .unwrap();
    store
        .set_node_property_string(node, "task_json", serde_json::to_string(task).unwrap())
        .await
        .unwrap();
    node
}

async fn persisted_task(store: &MockGraphStore, node_id: u64) -> PendingTask {
    let json = store
        .get_node_property_string(node_id, "task_json")
        .await
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

fn runner_with(
    store: &MockGraphStore,
    worker_id: &str,
    now: i64,
    lease_term: i64,
    executor: Arc<dyn SchedulerTaskExecutor>,
) -> SchedulerRunner {
    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
        .with_clock(fixed_clock(now))
        .with_worker_id(worker_id)
        .with_lease_term(lease_term)
        .with_executor(executor)
}

// ─── Crash recovery: lease expiry, worker loss, one logical result ────

#[tokio::test]
async fn crash_recovery_publishes_one_logical_result_with_full_lineage() {
    let store = MockGraphStore::new();
    // Worker-a claimed at T0 (term 300) and crashed mid-execution: attempt 1
    // is still Running on disk when the lease expires.
    let mut task = durable_task(
        "2401.00001",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    task.claim("worker-a", T0, LEASE_TERM).unwrap();
    let node = seed_durable(&store, &task).await;

    // Worker-b recovers the expired lease at T0+301.
    let runner_b = runner_with(
        &store,
        "worker-b",
        T0 + LEASE_TERM + 1,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("enriched by worker-b"),
            log: None,
        }),
    );
    let report = runner_b.run_due().await.unwrap();
    assert_eq!(report.due, 1);
    assert_eq!(report.recovered, 1, "expired lease recovered");
    assert_eq!(report.completed, 1);
    assert_eq!(report.pending, 0);

    let persisted = persisted_task(&store, node).await;
    // Complete attempt history: the crashed attempt is retained as Aborted.
    assert_eq!(persisted.attempts.len(), 2);
    assert_eq!(persisted.attempts[0].attempt, 1);
    assert_eq!(persisted.attempts[0].worker, "worker-a");
    assert_eq!(persisted.attempts[0].epoch, 1);
    assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Aborted);
    assert_eq!(persisted.attempts[0].finished_at, Some(T0 + LEASE_TERM + 1));
    assert_eq!(persisted.attempts[1].attempt, 2);
    assert_eq!(persisted.attempts[1].worker, "worker-b");
    assert_eq!(
        persisted.attempts[1].epoch, 2,
        "fence incremented on reclaim"
    );
    assert_eq!(persisted.attempts[1].outcome, AttemptOutcome::Succeeded);
    assert_eq!(
        persisted.attempts[1].evidence_refs,
        vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
    );

    // Exactly one logical result with the stable deterministic identity.
    let result = persisted.logical_result.as_ref().expect("published once");
    assert_eq!(result.result_id, logical_result_vid(&persisted.task_vid()));
    assert_eq!(result.published_by_epoch, 2);
    assert_eq!(
        result.evidence_refs,
        vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
    );
    assert_eq!(persisted.status, TaskStatus::Completed);
    assert_eq!(persisted.lease.as_ref().unwrap().owner, "worker-b");
    assert_eq!(persisted.lease.as_ref().unwrap().epoch, 2);
    assert_eq!(persisted.lease.as_ref().unwrap().heartbeat_count, 1);

    // A completed task is never due again — no duplicate execution.
    let rerun = runner_b.run_due().await.unwrap();
    assert_eq!(rerun.due, 0);
    assert_eq!(rerun.completed, 0);
    assert_eq!(persisted_task(&store, node).await.attempts.len(), 2);

    // D127 stays false through the crash-recovery lifecycle.
    assert_eq!(
        store.get_node_property_bool(node, "import_eligible").await,
        Some(false)
    );
}

// ─── Duplicate execution prevention: stale holder after reclaim ───────

#[tokio::test]
async fn stale_holder_after_reclaim_cannot_execute_or_publish() {
    let store = MockGraphStore::new();
    let inner_report: Arc<Mutex<Option<SchedulerRunReport>>> = Arc::new(Mutex::new(None));
    let task = durable_task(
        "2401.00002",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    let node = seed_durable(&store, &task).await;

    // Worker-a claims (fence 1) and its executor triggers worker-b's recovery
    // pass (reclaim at T0+301 → fence 2 → complete + publish) mid-execution.
    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    let runner_a = SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
        .with_clock(fixed_clock(T0))
        .with_worker_id("worker-a")
        .with_lease_term(LEASE_TERM)
        .with_executor(Arc::new(ReclaimingExecutor {
            task_type: TaskType::OpenAlexEnrich,
            store: store.clone(),
            reclaim_at: T0 + LEASE_TERM + 1,
            worker_b: "worker-b".into(),
            inner_report: Arc::clone(&inner_report),
        }));
    let report_a = runner_a.run_due().await.unwrap();

    // Worker-b recovered the lease and published while worker-a was paused.
    let report_b = inner_report
        .lock()
        .unwrap()
        .clone()
        .expect("inner pass ran");
    assert_eq!(report_b.recovered, 1);
    assert_eq!(report_b.completed, 1);

    // Worker-a's post-execution fence check suppressed its finalize/publish:
    // no completion from the stale holder, no second logical result.
    assert_eq!(report_a.due, 1);
    assert_eq!(report_a.completed, 0);
    assert_eq!(report_a.pending, 1);
    assert!(
        report_a.outcomes[0]
            .message
            .contains("lease lost during execution"),
        "stale holder must observe the reclaim, got: {}",
        report_a.outcomes[0].message
    );

    let persisted = persisted_task(&store, node).await;
    // Exactly ONE authoritative success (worker-b's) — worker-a's attempt is
    // retained as Aborted; the logical result is published exactly once.
    assert_eq!(persisted.attempts.len(), 2);
    assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Aborted);
    assert_eq!(persisted.attempts[0].epoch, 1);
    assert_eq!(persisted.attempts[1].outcome, AttemptOutcome::Succeeded);
    assert_eq!(persisted.attempts[1].epoch, 2);
    let result = persisted
        .logical_result
        .as_ref()
        .expect("one logical result");
    assert_eq!(result.result_id, logical_result_vid(&persisted.task_vid()));
    assert_eq!(result.published_by_epoch, 2);
    assert_eq!(persisted.status, TaskStatus::Completed);

    // Domain-level fence (defense in depth): the stale holder cannot
    // re-finalize or re-publish even holding a stale copy of the record.
    let mut stale = persisted.clone();
    assert!(matches!(
        stale.publish_logical_result(1, "stale".into(), vec![], T0 + 400),
        Err(PublishError::StaleFence {
            current: 2,
            presented: 1
        })
    ));
    assert!(matches!(
        stale.finish_attempt(1, AttemptOutcome::Succeeded, None, false, vec![], T0 + 400),
        Err(AttemptError::StaleFence { .. })
    ));

    // A resumed stale worker run (same worker id, post-completion) finds the
    // task completed: not due, no new attempts, result unchanged.
    let stale_runner = runner_with(
        &store,
        "worker-a",
        T0 + 400,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("stale"),
            log: None,
        }),
    );
    let rerun = stale_runner.run_due().await.unwrap();
    assert_eq!(rerun.due, 0);
    assert_eq!(rerun.completed, 0);
    let after = persisted_task(&store, node).await;
    assert_eq!(after.attempts.len(), 2, "no stale attempt recorded");
    assert_eq!(
        after.logical_result.as_ref().unwrap().result_id,
        result.result_id
    );
}

// ─── Live lease never stolen; heartbeat extends only the holder ───────

#[tokio::test]
async fn live_lease_is_never_stolen_and_heartbeat_extends_it() {
    let store = MockGraphStore::new();
    let mut task = durable_task(
        "2401.00003",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    task.claim("worker-a", T0, LEASE_TERM).unwrap();
    let node = seed_durable(&store, &task).await;

    // Worker-b at T0+50: the lease is live → the task stays pending (never
    // stolen), no recovery, no execution.
    let runner_b = runner_with(
        &store,
        "worker-b",
        T0 + 50,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("b"),
            log: None,
        }),
    );
    let report = runner_b.run_due().await.unwrap();
    assert_eq!(report.due, 1);
    assert_eq!(report.pending, 1);
    assert_eq!(report.recovered, 0);
    assert_eq!(report.completed, 0);
    let mid = persisted_task(&store, node).await;
    assert_eq!(mid.attempts.len(), 1, "no duplicate attempt while held");
    assert_eq!(mid.lease.as_ref().unwrap().owner, "worker-a");
    assert_eq!(mid.lease.as_ref().unwrap().epoch, 1);

    // Worker-a heartbeats at T0+100: expiry extends (T0+300 → T0+400) and the
    // heartbeat is observable.
    let runner_a = runner_with(
        &store,
        "worker-a",
        T0 + 100,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("a"),
            log: None,
        }),
    );
    let heartbeats = runner_a.heartbeat_due_tasks().await.unwrap();
    assert_eq!(heartbeats, 1, "only the holder's lease is heartbeated");
    let after_hb = persisted_task(&store, node).await;
    let lease = after_hb.lease.as_ref().unwrap();
    assert_eq!(lease.expires_at, T0 + 400, "heartbeat extends by the term");
    assert_eq!(lease.heartbeat_count, 1);
    assert_eq!(lease.last_heartbeat_at, Some(T0 + 100));

    // Worker-b at T0+301: still within the extended lease → pending again.
    let report2 = runner_b.run_due().await.unwrap();
    assert_eq!(report2.pending, 1);
    assert_eq!(report2.recovered, 0);

    // Worker-b at T0+401 (after the extended expiry): recover + complete.
    let runner_b_late = runner_with(
        &store,
        "worker-b",
        T0 + 401,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("b late"),
            log: None,
        }),
    );
    let report3 = runner_b_late.run_due().await.unwrap();
    assert_eq!(report3.recovered, 1);
    assert_eq!(report3.completed, 1);
    let done = persisted_task(&store, node).await;
    assert_eq!(done.attempts.len(), 2);
    assert_eq!(done.attempts[0].outcome, AttemptOutcome::Aborted);
    assert_eq!(done.attempts[1].outcome, AttemptOutcome::Succeeded);
    assert_eq!(done.lease.as_ref().unwrap().epoch, 2);
    assert!(done.logical_result.is_some());
}

// ─── Retry lifecycle: full lineage, one logical result ─────────────────

#[tokio::test]
async fn retry_then_success_keeps_full_lineage_and_one_result() {
    let store = MockGraphStore::new();
    // Fast policy (1s initial, 2x backoff, 10s cap) + short lease (10s) so
    // every pass deterministically lands past next_retry AND lease expiry.
    let mut task = PendingTask::new_openalex_enrich("2401.00004", &RetryPolicy::fast(), T0);
    task.next_retry = T0; // due immediately
    let node = seed_durable(&store, &task).await;

    let exec: Arc<CountingExecutor> = Arc::new(CountingExecutor {
        task_type: TaskType::OpenAlexEnrich,
        succeed_on: 3,
    });
    let exec_dyn: Arc<dyn SchedulerTaskExecutor> = exec.clone();
    let make_runner = |now: i64| {
        let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
        SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(now))
            .with_worker_id("worker-r")
            .with_lease_term(10)
            .with_policy(RetryPolicy::fast())
            .with_executor(Arc::clone(&exec_dyn))
    };

    // Pass 1 at T0: attempt 1 → retryable (lease expires T0+10).
    let report1 = make_runner(T0).run_due().await.unwrap();
    assert_eq!(report1.pending, 1);
    assert_eq!(report1.completed, 0);
    let after1 = persisted_task(&store, node).await;
    assert_eq!(after1.attempts.len(), 1);
    assert_eq!(after1.attempts[0].attempt, 1);
    assert_eq!(after1.attempts[0].outcome, AttemptOutcome::Failed);
    assert!(after1.attempts[0].retryable);
    assert_eq!(after1.retry_count, 1);
    assert!(after1.next_retry > T0, "retry scheduled in the future");
    assert!(after1.logical_result.is_none());

    // Pass 2 at T0+50: lease expired + due → reclaim (fence 2) → retryable.
    let report2 = make_runner(T0 + 50).run_due().await.unwrap();
    assert_eq!(report2.pending, 1);
    assert_eq!(report2.completed, 0);
    let after2 = persisted_task(&store, node).await;
    assert_eq!(after2.attempts.len(), 2);
    assert_eq!(after2.attempts[1].attempt, 2);
    assert_eq!(
        after2.attempts[1].epoch, 2,
        "fence monotonic across retries"
    );
    assert_eq!(after2.attempts[1].outcome, AttemptOutcome::Failed);
    assert_eq!(after2.retry_count, 2);

    // Pass 3 at T0+100: reclaim (fence 3) → attempt 3 succeeds.
    let report3 = make_runner(T0 + 100).run_due().await.unwrap();
    assert_eq!(report3.completed, 1);
    assert_eq!(report3.recovered, 1);
    let done = persisted_task(&store, node).await;
    assert_eq!(done.attempts.len(), 3);
    assert_eq!(done.attempts[2].attempt, 3);
    assert_eq!(done.attempts[2].epoch, 3);
    assert_eq!(done.attempts[2].outcome, AttemptOutcome::Succeeded);
    assert_eq!(done.retry_count, 2, "success does not add a retry");
    assert_eq!(done.status, TaskStatus::Completed);
    let result = done.logical_result.as_ref().expect("one logical result");
    assert_eq!(result.result_id, logical_result_vid(&done.task_vid()));
    assert_eq!(
        result.evidence_refs,
        vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
    );
    // Full immutable lineage preserved: the first two attempts stay Failed.
    assert_eq!(done.attempts[0].outcome, AttemptOutcome::Failed);
    assert_eq!(done.attempts[1].outcome, AttemptOutcome::Failed);
    assert!(done.attempts[0].retryable && done.attempts[1].retryable);
    // Exactly one logical result survives the retries.
    assert_eq!(done.attempts.len(), 3, "no duplicate publish attempts");
}

// ─── Dependency readiness ─────────────────────────────────────────────

#[tokio::test]
async fn blocked_dependency_runs_after_dependency_completes() {
    let store = MockGraphStore::new();
    let dep = durable_task(
        "2401.00010",
        TaskType::OpenAlexEnrich,
        TaskPriority::High,
        TaskResourceClass::Standard,
        vec![],
        T0 + 1000, // not due in the first pass
        T0,
    );
    let dep_node = seed_durable(&store, &dep).await;
    let dep_vid = dep.task_vid();
    let main = durable_task(
        "2401.00011",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![TaskDependency {
            task_vid: dep_vid,
            satisfied_at: None,
        }],
        T0,
        T0,
    );
    let main_node = seed_durable(&store, &main).await;

    // Pass 1: dependency not completed → the dependent is blocked (no claim,
    // no attempt, no retry).
    let runner = runner_with(
        &store,
        "worker-d",
        T0,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: None,
        }),
    );
    let report = runner.run_due().await.unwrap();
    assert_eq!(report.due, 1, "only the dependent is due");
    assert_eq!(report.blocked, 1);
    assert_eq!(report.completed, 0);
    let blocked = persisted_task(&store, main_node).await;
    assert_eq!(blocked.status, TaskStatus::Pending);
    assert!(blocked.lease.is_none(), "blocked task must not be claimed");
    assert!(blocked.attempts.is_empty());
    assert!(!blocked.dependencies[0].is_satisfied());

    // Pass 2 at T0+1000: the dependency (High) completes first, then the
    // dependent's readiness refreshes from the durable store and it runs.
    let runner2 = runner_with(
        &store,
        "worker-d",
        T0 + 1000,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: None,
        }),
    );
    let report2 = runner2.run_due().await.unwrap();
    assert_eq!(report2.due, 2);
    assert_eq!(report2.completed, 2);
    let done = persisted_task(&store, main_node).await;
    assert_eq!(done.status, TaskStatus::Completed);
    assert!(done.logical_result.is_some());
    assert!(
        done.dependencies[0].is_satisfied(),
        "dependency satisfaction persisted"
    );
    assert_eq!(done.attempts.len(), 1);
    assert_eq!(
        store
            .get_node_property_string(dep_node, "status")
            .await
            .as_deref(),
        Some("completed")
    );
}

// ─── Resource readiness ───────────────────────────────────────────────

#[tokio::test]
async fn resource_unavailability_blocks_claim_until_available() {
    let store = MockGraphStore::new();
    let task = durable_task(
        "2401.00020",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Network,
        vec![],
        T0,
        T0,
    );
    let node = seed_durable(&store, &task).await;

    // Network is unavailable → the task is blocked: no claim, no attempt.
    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    let runner_blocked = SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
        .with_clock(fixed_clock(T0))
        .with_worker_id("worker-r")
        .with_lease_term(LEASE_TERM)
        .with_executor(Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: None,
        }))
        .with_resource_availability(|class| class != TaskResourceClass::Network);
    let report = runner_blocked.run_due().await.unwrap();
    assert_eq!(report.blocked, 1);
    assert_eq!(report.completed, 0);
    let blocked = persisted_task(&store, node).await;
    assert!(blocked.lease.is_none());
    assert!(blocked.attempts.is_empty());
    assert_eq!(blocked.status, TaskStatus::Pending);

    // Network becomes available → the same task runs and completes.
    let runner_open = runner_with(
        &store,
        "worker-r",
        T0,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: None,
        }),
    );
    let report2 = runner_open.run_due().await.unwrap();
    assert_eq!(report2.completed, 1);
    assert_eq!(
        persisted_task(&store, node).await.status,
        TaskStatus::Completed
    );
}

// ─── Priority ordering ────────────────────────────────────────────────

#[tokio::test]
async fn priority_ordering_high_medium_low_is_deterministic() {
    let store = MockGraphStore::new();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut expected = Vec::new();
    for (arxiv, priority) in [
        ("2401.00030", TaskPriority::Low),
        ("2401.00031", TaskPriority::Medium),
        ("2401.00032", TaskPriority::High),
    ] {
        let task = durable_task(
            arxiv,
            TaskType::OpenAlexEnrich,
            priority,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        expected.push(task.task_vid());
        seed_durable(&store, &task).await;
    }

    let runner = runner_with(
        &store,
        "worker-p",
        T0,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: Some(Arc::clone(&log)),
        }),
    );
    let report = runner.run_due().await.unwrap();
    assert_eq!(report.completed, 3);
    let order = log.lock().unwrap().clone();
    assert_eq!(
        order,
        vec![
            expected[2].clone(),
            expected[1].clone(),
            expected[0].clone()
        ],
        "High must run before Medium before Low"
    );

    // Same pass → one logical result per task, all published.
    for vid in &expected {
        let node = store
            .find_node_by_string_property("SchedulerTask", "vid", vid)
            .await
            .unwrap();
        let task = persisted_task(&store, node).await;
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.logical_result.is_some());
    }
}

// ─── Closed dispatch: fail-closed + generic registered types ──────────

#[tokio::test]
async fn no_executor_fails_closed_without_claim() {
    let store = MockGraphStore::new();
    // Task type Extract; the runner only registers OpenAlexEnrich → the type
    // must fail closed without a claim (Q3 item 3).
    let task = durable_task(
        "2401.00040",
        TaskType::Extract,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    let node = seed_durable(&store, &task).await;

    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    let runner = SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
        .with_clock(fixed_clock(T0))
        .with_worker_id("worker-x")
        .with_lease_term(LEASE_TERM);
    let report = runner.run_due().await.unwrap();
    assert_eq!(report.due, 1);
    assert_eq!(report.no_executor, 1);
    assert_eq!(
        report.outcomes[0].disposition,
        SchedulerTaskDisposition::NoExecutor
    );
    let persisted = persisted_task(&store, node).await;
    assert!(
        persisted.lease.is_none(),
        "unregistered type must not claim"
    );
    assert!(persisted.attempts.is_empty());
    assert_eq!(persisted.status, TaskStatus::Pending);
}

#[tokio::test]
async fn generic_task_type_runs_through_registered_executor() {
    let store = MockGraphStore::new();
    // A non-OpenAlex type runs through the same durable plane once its
    // executor is registered (generalization beyond one adapter).
    let task = durable_task(
        "2401.00041",
        TaskType::Extract,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    let node = seed_durable(&store, &task).await;

    let runner = runner_with(
        &store,
        "worker-g",
        T0,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::Extract,
            outcome: completed_outcome("extracted"),
            log: None,
        }),
    );
    let report = runner.run_due().await.unwrap();
    assert_eq!(report.completed, 1);
    let done = persisted_task(&store, node).await;
    assert_eq!(done.task_type, TaskType::Extract);
    assert_eq!(done.status, TaskStatus::Completed);
    assert!(done.logical_result.is_some());
    assert_eq!(done.attempts[0].worker, "worker-g");
    assert_eq!(done.attempts[0].outcome, AttemptOutcome::Succeeded);
}

// ─── Read-side validation (Q3 item 5) ──────────────────────────────────

#[tokio::test]
async fn corrupt_record_is_skipped_on_read() {
    let store = MockGraphStore::new();
    // A durable record whose identity fails read-side validation must never
    // be claimed or executed (defense in depth on the runner load path).
    let task = durable_task(
        "not-an-arxiv-id",
        TaskType::OpenAlexEnrich,
        TaskPriority::Medium,
        TaskResourceClass::Standard,
        vec![],
        T0,
        T0,
    );
    let node = seed_durable(&store, &task).await;

    let runner = runner_with(
        &store,
        "worker-v",
        T0,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("never runs"),
            log: None,
        }),
    );
    let report = runner.run_due().await.unwrap();
    assert_eq!(report.due, 0, "corrupt record never enters the pass");
    assert_eq!(report.completed, 0);
    let persisted = persisted_task(&store, node).await;
    assert!(persisted.lease.is_none(), "corrupt record must not claim");
    assert!(persisted.attempts.is_empty());
    assert_eq!(persisted.status, TaskStatus::Pending);
}

// ─── Slice verification surface ───────────────────────────────────────

#[tokio::test]
async fn scheduler_state_exposes_durable_plane_fields() {
    // Slice verification: the operational surface exposes lease owner,
    // heartbeat, expiry, priority, dependencies, resource class, attempts,
    // and the logical result identity after a completed recovery.
    let store = MockGraphStore::new();
    let mut task = durable_task(
        "2401.00050",
        TaskType::OpenAlexEnrich,
        TaskPriority::High,
        TaskResourceClass::Compute,
        vec![],
        T0,
        T0,
    );
    task.claim("worker-a", T0, LEASE_TERM).unwrap();
    let node = seed_durable(&store, &task).await;

    let runner_b = runner_with(
        &store,
        "worker-b",
        T0 + LEASE_TERM + 1,
        LEASE_TERM,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("done"),
            log: None,
        }),
    );
    let report = runner_b.run_due().await.unwrap();
    assert_eq!(report.recovered, 1);
    assert_eq!(report.completed, 1);

    // Flat operational mirrors expose the durable plane.
    assert_eq!(
        store
            .get_node_property_string(node, "status")
            .await
            .as_deref(),
        Some("completed")
    );
    assert_eq!(
        store
            .get_node_property_string(node, "lease_owner")
            .await
            .as_deref(),
        Some("worker-b")
    );
    assert_eq!(
        store.get_node_property_int(node, "lease_epoch").await,
        Some(2)
    );
    assert!(
        store
            .get_node_property_string(node, "lease_expires_at")
            .await
            .is_some(),
        "lease expiry mirrored"
    );
    assert_eq!(
        store
            .get_node_property_string(node, "priority")
            .await
            .as_deref(),
        Some("high")
    );
    assert_eq!(
        store
            .get_node_property_string(node, "resource_class")
            .await
            .as_deref(),
        Some("compute")
    );
    assert_eq!(
        store.get_node_property_int(node, "attempt_count").await,
        Some(2)
    );
    let result_id = store
        .get_node_property_string(node, "logical_result_id")
        .await
        .expect("logical result id mirror");
    assert_eq!(result_id, logical_result_vid(&task.task_vid()));

    // The durable record carries the full state, including the claim-time
    // heartbeat and the fenced lease.
    let persisted = persisted_task(&store, node).await;
    let lease = persisted.lease.as_ref().unwrap();
    assert_eq!(lease.owner, "worker-b");
    assert_eq!(lease.epoch, 2);
    assert_eq!(lease.heartbeat_count, 1, "claim-time heartbeat recorded");
    assert_eq!(lease.expires_at, (T0 + LEASE_TERM + 1) + LEASE_TERM);
    assert_eq!(persisted.priority, TaskPriority::High);
    assert_eq!(persisted.resource_class, TaskResourceClass::Compute);
    assert_eq!(persisted.attempts.len(), 2);
    assert_eq!(
        persisted.logical_result.as_ref().unwrap().result_id,
        result_id
    );

    // D127: import_eligible stays false on the operational node.
    assert_eq!(
        store.get_node_property_bool(node, "import_eligible").await,
        Some(false)
    );
}
