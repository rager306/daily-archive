//! Generic recoverable scheduler runner over the durable task plane.
//!
//! M399/S03 T02: claims and heartbeats fenced leases, honors priority
//! ordering and dependency/resource readiness, recovers expired leases, and
//! publishes exactly one logical result per task while preserving complete
//! attempt lineage. Dispatch stays closed: executors register per `TaskType`
//! and unregistered types fail closed without claiming (Q3 item 3).
//!
//! Durable records are `PendingTask` JSON on the `task_json` node property
//! (Samyama SchedulerTask node, ADR-040). Nodes written by the legacy writer
//! (`GraphScheduler::add_pending_to`, flat properties only) are materialized
//! into durable records and upgraded on first claim — old snapshots stay
//! readable (R092 wire discipline). The legacy OpenAlex enrichment path is
//! preserved as the default `OpenAlexEnrichExecutor` with the verified
//! write-back completion rule (Q4 regression bar).
//!
//! Recovery semantics: an expired lease is reclaimable; the reclaim bumps the
//! monotonic fence epoch and the crashed worker's Running attempt is aborted,
//! so attempt history stays complete. Every authoritative write (attempt
//! finalization, logical-result publish, durable persist) validates the
//! current fence — a stale holder after reclaim is rejected (Q3 item 1).

use crate::EnrichUseCase;
use async_trait::async_trait;
use da_domain::scheduler::{
    AttemptOutcome, DEFAULT_LEASE_TERM_SECS, LeaseError, PendingTask, RetryPolicy, TaskPriority,
    TaskRecordError, TaskResourceClass, TaskStatus, TaskType, all_dependencies_satisfied,
};
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::OpenAlexClient;
use std::collections::HashMap;
use std::sync::Arc;

/// Outcome of one task execution by a registered executor.
#[derive(Debug, Clone)]
pub enum SchedulerExecuteOutcome {
    /// Success — the runner publishes exactly one logical result.
    Completed {
        summary: String,
        /// Stage evidence references (cache-key sha256 / canonical stage ids)
        /// preserved into the attempt lineage and the logical result.
        evidence_refs: Vec<String>,
    },
    /// Retryable failure — the runner advances the retry lifecycle.
    Retryable {
        error_class: String,
        message: String,
    },
    /// Terminal failure — the runner marks the task failed without retry.
    Terminal {
        error_class: String,
        message: String,
    },
}

/// Closed per-task-type executor dispatch (Q3 item 3): only executors
/// registered on the runner may run a task; a task whose type has no
/// executor is never claimed (fail-closed).
#[async_trait]
pub trait SchedulerTaskExecutor: Send + Sync {
    fn task_type(&self) -> TaskType;

    /// Execute the task. The runner owns the lease lifecycle: it claims the
    /// task before calling this and finalizes/publishes afterwards. Executors
    /// must not mutate the task record itself (it is `&PendingTask`).
    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome;
}

/// OpenAlex enrichment executor — the legacy OpenAlex-coupled path.
///
/// Preserves the enrich auto-registration contract (Q4): NotFound creates a
/// pending stub (retryable), and completion is refused until the Paper node
/// carries the exact verified OpenAlex write-back identity.
pub struct OpenAlexEnrichExecutor {
    openalex: Arc<dyn OpenAlexClient>,
    graph_store: Arc<dyn DirectGraphStore>,
    clock: Arc<dyn da_ports::Clock>,
}

impl OpenAlexEnrichExecutor {
    pub fn new(
        openalex: Arc<dyn OpenAlexClient>,
        graph_store: Arc<dyn DirectGraphStore>,
        clock: Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self {
            openalex,
            graph_store,
            clock,
        }
    }

    /// Verified write-back: `Some(true)` only when the Paper node carries the
    /// exact OpenAlex identity the use case wrote. `None` when no Paper node
    /// exists (corrupted scheduler snapshot) — completion refused.
    async fn verified_write_back(&self, arxiv_id: &str, openalex_id: &str) -> Option<bool> {
        let paper = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await?;
        Some(
            self.graph_store
                .get_node_property_string(paper, "openalex_id")
                .await
                .as_deref()
                == Some(openalex_id),
        )
    }
}

#[async_trait]
impl SchedulerTaskExecutor for OpenAlexEnrichExecutor {
    fn task_type(&self) -> TaskType {
        TaskType::OpenAlexEnrich
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        let use_case =
            EnrichUseCase::from_shared(Arc::clone(&self.openalex), Arc::clone(&self.graph_store))
                .with_clock(Arc::clone(&self.clock));
        match use_case.enrich_by_arxiv_id(&task.arxiv_id).await {
            Ok(result) if result.openalex_pending => SchedulerExecuteOutcome::Retryable {
                error_class: "not_found".into(),
                message: "OpenAlex work not found; retry lifecycle advanced".into(),
            },
            Ok(result) => {
                let verified = self
                    .verified_write_back(&task.arxiv_id, &result.openalex_id)
                    .await;
                if verified != Some(true) || result.openalex_id.is_empty() {
                    return SchedulerExecuteOutcome::Retryable {
                        error_class: "write_back_unverified".into(),
                        message: "Paper lacks verified OpenAlex write-back; completion refused"
                            .into(),
                    };
                }
                SchedulerExecuteOutcome::Completed {
                    summary: format!(
                        "{} topics, {} authors, {} citations",
                        result.topics_written, result.authors_written, result.cites_edges_written
                    ),
                    evidence_refs: vec![format!("openalex_work:{}", result.openalex_id)],
                }
            }
            Err(error) => SchedulerExecuteOutcome::Retryable {
                error_class: "openalex_error".into(),
                message: format!("OpenAlex enrichment error: {error:#}"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerTaskDisposition {
    Completed,
    Pending,
    Failed,
    /// Blocked by unsatisfied dependencies or an unavailable resource class.
    Blocked,
    /// No executor registered for the task type (fail-closed dispatch).
    NoExecutor,
    /// Read-side record validation failed (corrupt record).
    Invalid,
}

#[derive(Debug, Clone)]
pub struct SchedulerTaskOutcome {
    pub node_id: u64,
    pub arxiv_id: String,
    pub disposition: SchedulerTaskDisposition,
    pub message: String,
    /// Canonical task type (closed serde enum) for observability.
    pub task_type: String,
    /// 1-based attempt ordinal of the most recent attempt.
    pub attempt: u32,
    /// Fence epoch of the current lease, when held.
    pub epoch: Option<u64>,
    /// Stable logical result identity, when published.
    pub logical_result_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerRunReport {
    pub due: usize,
    pub completed: usize,
    pub pending: usize,
    pub failed: usize,
    /// Blocked by unsatisfied dependencies / unavailable resource class.
    pub blocked: usize,
    /// Fail-closed: no executor registered for the task type.
    pub no_executor: usize,
    /// Expired leases recovered (fence advanced, stale attempt aborted).
    pub recovered: usize,
    pub outcomes: Vec<SchedulerTaskOutcome>,
}

/// One scheduler task loaded for a run pass.
struct TaskRecord {
    node_id: u64,
    task: PendingTask,
    /// True when the node predates the durable plane (no `task_json` yet) —
    /// upgraded to a durable record on first write.
    #[allow(dead_code)]
    legacy: bool,
    /// True when the durable record needs a write.
    dirty: bool,
}

pub struct SchedulerRunner {
    graph_store: Arc<dyn DirectGraphStore>,
    policy: RetryPolicy,
    clock: Arc<dyn da_ports::Clock>,
    worker_id: String,
    executors: HashMap<TaskType, Arc<dyn SchedulerTaskExecutor>>,
    resource_available: Box<dyn Fn(TaskResourceClass) -> bool + Send + Sync>,
    lease_term_secs: i64,
}

impl SchedulerRunner {
    /// Create a runner with the legacy OpenAlex enrichment executor registered
    /// (OpenAlex-coupled convenience; the generic dispatch surface is
    /// `with_executor`).
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, openalex: Arc<dyn OpenAlexClient>) -> Self {
        let clock: Arc<dyn da_ports::Clock> = Arc::new(da_ports::SystemClock);
        let executor: Arc<dyn SchedulerTaskExecutor> = Arc::new(OpenAlexEnrichExecutor::new(
            Arc::clone(&openalex),
            Arc::clone(&graph_store),
            Arc::clone(&clock),
        ));
        Self {
            graph_store,
            policy: RetryPolicy::default(),
            clock,
            worker_id: format!("scheduler-runner:{}", std::process::id()),
            executors: HashMap::from([(TaskType::OpenAlexEnrich, executor)]),
            resource_available: Box::new(|_: TaskResourceClass| true),
            lease_term_secs: DEFAULT_LEASE_TERM_SECS,
        }
    }

    pub fn with_clock(mut self, clock: Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Override the worker identity used for lease claims. Distinct worker ids
    /// let recovery tests simulate a crashed worker being replaced by another.
    pub fn with_worker_id(mut self, worker_id: impl Into<String>) -> Self {
        self.worker_id = worker_id.into();
        self
    }

    /// Register (or replace) a task-type executor. Dispatch stays closed: only
    /// registered types execute; unknown types fail closed without claiming.
    pub fn with_executor(mut self, executor: Arc<dyn SchedulerTaskExecutor>) -> Self {
        self.executors.insert(executor.task_type(), executor);
        self
    }

    /// Resource availability predicate: a task whose `resource_class` is not
    /// available is blocked (never claimed). Default: all classes available.
    pub fn with_resource_availability(
        mut self,
        available: impl Fn(TaskResourceClass) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.resource_available = Box::new(available);
        self
    }

    /// Bounded lease term for claims/heartbeats (clamped by the domain at
    /// `MAX_LEASE_TERM_SECS`).
    pub fn with_lease_term(mut self, term_secs: i64) -> Self {
        self.lease_term_secs = term_secs;
        self
    }

    /// Run one pass over all due tasks: recover expired leases, honor
    /// priorities and dependency/resource readiness, and publish exactly one
    /// logical result per completed task.
    pub async fn run_due(&self) -> anyhow::Result<SchedulerRunReport> {
        let now = self.clock.now().timestamp();
        let mut records = self.load_task_records(now).await;
        let mut report = SchedulerRunReport::default();

        // Priority-ordered deterministic selection: High → Medium → Low, then
        // due time, then enqueue time, then node id.
        records.sort_by(|a, b| {
            b.task
                .priority
                .cmp(&a.task.priority)
                .then_with(|| a.task.next_retry.cmp(&b.task.next_retry))
                .then_with(|| a.task.added_at.cmp(&b.task.added_at))
                .then_with(|| a.node_id.cmp(&b.node_id))
        });

        for mut record in records {
            if record.task.status != TaskStatus::Pending || record.task.next_retry > now {
                continue; // not due — not part of this pass
            }
            report.due += 1;

            // Dependency satisfaction is refreshed from the durable store —
            // a task stays blocked until every dependency is completed.
            self.refresh_dependencies(&mut record, now).await;

            // Readiness gate: all dependencies satisfied AND resource class
            // available. Blocked tasks are never claimed and never retried.
            if !record.task.is_ready(&self.resource_available) {
                let blocker = if !all_dependencies_satisfied(&record.task.dependencies) {
                    "dependencies unsatisfied".to_string()
                } else {
                    format!(
                        "resource class {} unavailable",
                        record.task.resource_class.as_str()
                    )
                };
                report.blocked += 1;
                report.outcomes.push(self.outcome_for(
                    &record,
                    SchedulerTaskDisposition::Blocked,
                    blocker,
                ));
                self.persist_if_dirty(&mut record).await?;
                continue;
            }

            // Fail-closed dispatch (Q3 item 3): no executor → never claim.
            let Some(executor) = self.executors.get(&record.task.task_type).cloned() else {
                report.no_executor += 1;
                report.outcomes.push(self.outcome_for(
                    &record,
                    SchedulerTaskDisposition::NoExecutor,
                    format!(
                        "no executor registered for task type {}",
                        record.task.task_type.as_str()
                    ),
                ));
                continue;
            };

            // A live lease held by another worker is never stolen — the task
            // waits for its own expiry (no duplicate execution while held).
            if !record.task.is_claimable(now) {
                let message = match &record.task.lease {
                    Some(lease) => {
                        format!("lease held by {} until {}", lease.owner, lease.expires_at)
                    }
                    None => "not claimable yet".to_string(),
                };
                report.pending += 1;
                report.outcomes.push(self.outcome_for(
                    &record,
                    SchedulerTaskDisposition::Pending,
                    message,
                ));
                continue;
            }

            // Read-side record validation (Q3 defense in depth). An expired
            // lease is the normal recovery path, not corruption.
            if let Err(error) = record.task.validate_record(now)
                && !matches!(
                    error,
                    TaskRecordError::InvalidLease(LeaseError::Expired { .. })
                )
            {
                report.pending += 1;
                report.outcomes.push(self.outcome_for(
                    &record,
                    SchedulerTaskDisposition::Invalid,
                    format!("record validation failed: {error}"),
                ));
                continue;
            }

            // Claim (or reclaim an expired lease) — the fence increments on
            // reclaim, so a stale holder's later writes are rejected.
            let recovered = record
                .task
                .lease
                .as_ref()
                .is_some_and(|lease| lease.is_expired(now));
            let epoch = match record
                .task
                .claim(&self.worker_id, now, self.lease_term_secs)
            {
                Ok(epoch) => epoch,
                Err(error) => {
                    tracing::warn!(
                        node_id = record.node_id,
                        error = %error,
                        "SchedulerTask claim rejected"
                    );
                    report.pending += 1;
                    report.outcomes.push(self.outcome_for(
                        &record,
                        SchedulerTaskDisposition::Pending,
                        format!("claim rejected: {error}"),
                    ));
                    continue;
                }
            };
            record.dirty = true;
            // Abort Running attempts of crashed workers under the old fence.
            record.task.mark_stale_attempts_aborted(now);
            // Heartbeat once at claim time — extends the lease across the
            // execution window and records the heartbeat for observability.
            if let Err(error) = record.task.heartbeat(now, self.lease_term_secs) {
                tracing::debug!(
                    node_id = record.node_id,
                    error = %error,
                    "claim-time heartbeat skipped"
                );
            }
            if recovered {
                report.recovered += 1;
            }

            // Persist the claim durably BEFORE executing: if the process dies
            // mid-execution, the lease is on disk and recovery is deterministic.
            self.persist_if_dirty(&mut record).await?;

            let outcome = executor.execute(&record.task).await;
            let finished_at = self.clock.now().timestamp();

            // Fence check before any authoritative write: re-read the durable
            // lease. If another worker reclaimed while we executed (higher
            // epoch / other owner), our attempt is stale — finalization and
            // publish are suppressed (Q3 item 1: no duplicate effects).
            match self.durable_lease(record.node_id).await {
                Some((owner, on_disk_epoch))
                    if on_disk_epoch > epoch || owner != self.worker_id =>
                {
                    tracing::warn!(
                        node_id = record.node_id,
                        attempt_epoch = epoch,
                        on_disk_epoch,
                        owner = %owner,
                        "lease reclaimed during execution; stale attempt suppressed"
                    );
                    report.pending += 1;
                    report.outcomes.push(self.outcome_for(
                        &record,
                        SchedulerTaskDisposition::Pending,
                        format!(
                            "lease lost during execution (reclaimed by {owner}@epoch {on_disk_epoch})"
                        ),
                    ));
                    continue;
                }
                _ => {}
            }

            let (disposition, message, logical_result_id) =
                self.finalize(&mut record, epoch, outcome, finished_at);
            match disposition {
                SchedulerTaskDisposition::Completed => report.completed += 1,
                SchedulerTaskDisposition::Failed => report.failed += 1,
                SchedulerTaskDisposition::Pending => report.pending += 1,
                _ => {}
            }
            report.outcomes.push(SchedulerTaskOutcome {
                node_id: record.node_id,
                arxiv_id: record.task.arxiv_id.clone(),
                task_type: record.task.task_type.as_str().to_string(),
                attempt: record.task.attempts.len() as u32,
                epoch: record.task.current_epoch(),
                logical_result_id,
                disposition,
                message,
            });
            self.persist_if_dirty(&mut record).await?;
        }

        tracing::info!(
            due = report.due,
            completed = report.completed,
            pending = report.pending,
            failed = report.failed,
            blocked = report.blocked,
            no_executor = report.no_executor,
            recovered = report.recovered,
            "Scheduler run completed on shared graph store"
        );
        Ok(report)
    }

    /// Heartbeat every lease currently held by this worker (daemon cadence
    /// between passes). Leases held by other workers are never touched
    /// (ownership check — the runner-level fence for heartbeats). Returns the
    /// number of heartbeats performed.
    pub async fn heartbeat_due_tasks(&self) -> anyhow::Result<usize> {
        let now = self.clock.now().timestamp();
        let records = self.load_task_records(now).await;
        let mut count = 0usize;
        for mut record in records {
            let Some(lease) = record.task.lease.as_ref() else {
                continue;
            };
            if !lease.is_held_by(&self.worker_id) {
                continue;
            }
            match record.task.heartbeat(now, self.lease_term_secs) {
                Ok(()) => {
                    record.dirty = true;
                    self.persist_if_dirty(&mut record).await?;
                    count += 1;
                }
                Err(error) => {
                    tracing::debug!(
                        node_id = record.node_id,
                        error = %error,
                        "heartbeat skipped (expired or drift-rejected)"
                    );
                }
            }
        }
        Ok(count)
    }

    // ─── Finalization ─────────────────────────────────────────────────────

    /// Finalize the current attempt and advance the task lifecycle. Returns
    /// (disposition, message, published logical result id).
    fn finalize(
        &self,
        record: &mut TaskRecord,
        epoch: u64,
        outcome: SchedulerExecuteOutcome,
        now: i64,
    ) -> (SchedulerTaskDisposition, String, Option<String>) {
        let task = &mut record.task;
        match outcome {
            SchedulerExecuteOutcome::Completed {
                summary,
                evidence_refs,
            } => {
                if let Err(error) = task.finish_attempt(
                    epoch,
                    AttemptOutcome::Succeeded,
                    None,
                    true,
                    evidence_refs.clone(),
                    now,
                ) {
                    return (
                        SchedulerTaskDisposition::Pending,
                        format!("attempt finalization rejected: {error}"),
                        None,
                    );
                }
                match task.publish_logical_result(epoch, summary.clone(), evidence_refs, now) {
                    Ok(result_id) => {
                        record.dirty = true;
                        (
                            SchedulerTaskDisposition::Completed,
                            summary,
                            Some(result_id),
                        )
                    }
                    Err(error) => (
                        SchedulerTaskDisposition::Pending,
                        format!("logical result publish rejected: {error}"),
                        None,
                    ),
                }
            }
            SchedulerExecuteOutcome::Retryable {
                error_class,
                message,
            } => {
                if let Err(error) = task.finish_attempt(
                    epoch,
                    AttemptOutcome::Failed,
                    Some(error_class),
                    true,
                    Vec::new(),
                    now,
                ) {
                    return (
                        SchedulerTaskDisposition::Pending,
                        format!("attempt finalization rejected: {error}"),
                        None,
                    );
                }
                task.record_retry(&self.policy, now);
                record.dirty = true;
                let disposition = if task.status == TaskStatus::Failed {
                    SchedulerTaskDisposition::Failed
                } else {
                    SchedulerTaskDisposition::Pending
                };
                (disposition, message, None)
            }
            SchedulerExecuteOutcome::Terminal {
                error_class,
                message,
            } => {
                if let Err(error) = task.finish_attempt(
                    epoch,
                    AttemptOutcome::Failed,
                    Some(error_class),
                    false,
                    Vec::new(),
                    now,
                ) {
                    return (
                        SchedulerTaskDisposition::Pending,
                        format!("attempt finalization rejected: {error}"),
                        None,
                    );
                }
                // Terminal failure: no retry is scheduled (never due again).
                task.status = TaskStatus::Failed;
                task.next_retry = i64::MAX;
                record.dirty = true;
                (SchedulerTaskDisposition::Failed, message, None)
            }
        }
    }

    // ─── Record loading / persistence ─────────────────────────────────────

    async fn load_task_records(&self, now: i64) -> Vec<TaskRecord> {
        let mut records = Vec::new();
        for node_id in self.graph_store.get_nodes_by_label("SchedulerTask").await {
            match self.load_task_record(node_id, now).await {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(node_id, error = %error, "SchedulerTask record skipped");
                }
            }
        }
        records
    }

    async fn load_task_record(&self, node_id: u64, now: i64) -> anyhow::Result<Option<TaskRecord>> {
        if let Some(json) = self
            .graph_store
            .get_node_property_string(node_id, "task_json")
            .await
        {
            match serde_json::from_str::<PendingTask>(&json) {
                Ok(task) => {
                    // Read-side validation (defense in depth). An expired lease
                    // is the recovery path, not corruption.
                    if let Err(error) = task.validate_record(now)
                        && !matches!(
                            error,
                            TaskRecordError::InvalidLease(LeaseError::Expired { .. })
                        )
                    {
                        tracing::warn!(
                            node_id,
                            error = %error,
                            "SchedulerTask durable record invalid"
                        );
                        return Ok(None);
                    }
                    Ok(Some(TaskRecord {
                        node_id,
                        task,
                        legacy: false,
                        dirty: false,
                    }))
                }
                Err(error) => {
                    tracing::warn!(node_id, error = %error, "SchedulerTask task_json corrupt");
                    Ok(None)
                }
            }
        } else {
            // Legacy flat node (pre-durable-plane writer) — materialize a
            // durable record; it is upgraded on the first authoritative write.
            match self.materialize_legacy(node_id).await {
                Some(task) => Ok(Some(TaskRecord {
                    node_id,
                    task,
                    legacy: true,
                    dirty: true,
                })),
                None => {
                    tracing::warn!(node_id, "SchedulerTask legacy node unreadable");
                    Ok(None)
                }
            }
        }
    }

    /// Materialize a `PendingTask` from the legacy flat node properties.
    async fn materialize_legacy(&self, node_id: u64) -> Option<PendingTask> {
        let arxiv_id = self
            .graph_store
            .get_node_property_string(node_id, "arxiv_id")
            .await?;
        let task_type = match self
            .graph_store
            .get_node_property_string(node_id, "task_type")
            .await
        {
            Some(raw) => task_type_from_str(&raw)?,
            None => TaskType::OpenAlexEnrich,
        };
        let added_at = self
            .read_datetime_secs(node_id, "added_at")
            .await
            .unwrap_or(0);
        let next_retry = self
            .read_datetime_secs(node_id, "next_retry")
            .await
            .unwrap_or(0);
        let retry_count = self
            .graph_store
            .get_node_property_int(node_id, "retry_count")
            .await
            .unwrap_or(0)
            .max(0) as u32;
        let last_retry = self.read_datetime_secs(node_id, "last_retry").await;
        let status = match self
            .graph_store
            .get_node_property_string(node_id, "status")
            .await
            .as_deref()
        {
            Some("completed") => TaskStatus::Completed,
            Some("failed") => TaskStatus::Failed,
            _ => TaskStatus::Pending,
        };
        let mut task = PendingTask::new_openalex_enrich(&arxiv_id, &self.policy, added_at);
        task.task_type = task_type;
        task.retry_count = retry_count;
        task.next_retry = next_retry;
        task.last_retry = last_retry;
        task.status = status;
        if let Some(raw) = self
            .graph_store
            .get_node_property_string(node_id, "priority")
            .await
            && let Some(priority) = task_priority_from_str(&raw)
        {
            task.priority = priority;
        }
        Some(task)
    }

    /// Re-read the durable lease of a node: `Some((owner, epoch))` when a
    /// durable lease exists. Used as the runner-level fence check before
    /// authoritative writes (attempt finalization / publish).
    async fn durable_lease(&self, node_id: u64) -> Option<(String, u64)> {
        let json = self
            .graph_store
            .get_node_property_string(node_id, "task_json")
            .await?;
        let task: PendingTask = serde_json::from_str(&json).ok()?;
        let lease = task.lease?;
        Some((lease.owner, lease.epoch))
    }

    /// Refresh dependency satisfaction from the durable store. A dependency is
    /// satisfied when the depended-on task is completed (durable logical
    /// result published or flat status completed).
    async fn refresh_dependencies(&self, record: &mut TaskRecord, now: i64) {
        for dependency in &mut record.task.dependencies {
            if dependency.is_satisfied() {
                continue;
            }
            if self.dependency_satisfied(&dependency.task_vid).await {
                dependency.satisfied_at = Some(now);
                record.dirty = true;
            }
        }
    }

    async fn dependency_satisfied(&self, task_vid: &str) -> bool {
        if let Some(node) = self
            .graph_store
            .find_node_by_string_property("SchedulerTask", "vid", task_vid)
            .await
            && self.is_task_completed(node).await
        {
            return true;
        }
        // Logical-result identity form: vid:logical-result:…
        if let Some(node) = self
            .graph_store
            .find_node_by_string_property("SchedulerTask", "logical_result_id", task_vid)
            .await
        {
            return self.is_task_completed(node).await;
        }
        false
    }

    async fn is_task_completed(&self, node_id: u64) -> bool {
        if let Some(json) = self
            .graph_store
            .get_node_property_string(node_id, "task_json")
            .await
            && let Ok(task) = serde_json::from_str::<PendingTask>(&json)
        {
            return task.status == TaskStatus::Completed || task.logical_result.is_some();
        }
        self.graph_store
            .get_node_property_string(node_id, "status")
            .await
            .as_deref()
            == Some("completed")
    }

    /// Persist the durable record (and the flat operational mirrors) only when
    /// dirty. The write is fence-checked: if the on-disk lease shows a higher
    /// epoch than ours, another worker reclaimed while we executed — our write
    /// is stale and suppressed (Q3 item 1).
    async fn persist_if_dirty(&self, record: &mut TaskRecord) -> anyhow::Result<()> {
        if !record.dirty {
            return Ok(());
        }
        if let Some((_, on_disk_epoch)) = self.durable_lease(record.node_id).await
            && let Some(ours) = record.task.current_epoch()
            && on_disk_epoch > ours
        {
            tracing::warn!(
                node_id = record.node_id,
                on_disk_epoch,
                ours,
                "stale persist suppressed (fence)"
            );
            record.dirty = false;
            return Ok(());
        }
        self.persist_record(record).await?;
        Ok(())
    }

    async fn persist_record(&self, record: &TaskRecord) -> anyhow::Result<()> {
        let task = &record.task;
        let node_id = record.node_id;
        self.graph_store
            .set_node_property_string(node_id, "task_json", serde_json::to_string(task)?)
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "status", task_status_str(&task.status).to_string())
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "retry_count", task.retry_count as i64)
            .await?;
        if let Some(wire) = to_rfc3339_wire_opt(task.next_retry) {
            self.graph_store
                .set_node_property_string(node_id, "next_retry", wire)
                .await?;
        }
        if let Some(last_retry) = task.last_retry
            && let Some(wire) = to_rfc3339_wire_opt(last_retry)
        {
            self.graph_store
                .set_node_property_string(node_id, "last_retry", wire)
                .await?;
        }
        self.graph_store
            .set_node_property_string(node_id, "priority", task.priority.as_str().to_string())
            .await?;
        self.graph_store
            .set_node_property_string(
                node_id,
                "resource_class",
                task.resource_class.as_str().to_string(),
            )
            .await?;
        if let Some(lease) = &task.lease {
            self.graph_store
                .set_node_property_string(node_id, "lease_owner", lease.owner.clone())
                .await?;
            self.graph_store
                .set_node_property_int(node_id, "lease_epoch", lease.epoch as i64)
                .await?;
            if let Some(wire) = to_rfc3339_wire_opt(lease.acquired_at) {
                self.graph_store
                    .set_node_property_string(node_id, "lease_acquired_at", wire)
                    .await?;
            }
            if let Some(wire) = to_rfc3339_wire_opt(lease.expires_at) {
                self.graph_store
                    .set_node_property_string(node_id, "lease_expires_at", wire)
                    .await?;
            }
            if let Some(lhb) = lease.last_heartbeat_at
                && let Some(wire) = to_rfc3339_wire_opt(lhb)
            {
                self.graph_store
                    .set_node_property_string(node_id, "lease_last_heartbeat_at", wire)
                    .await?;
            }
        }
        self.graph_store
            .set_node_property_int(node_id, "attempt_count", task.attempts.len() as i64)
            .await?;
        if let Some(result) = &task.logical_result {
            self.graph_store
                .set_node_property_string(node_id, "logical_result_id", result.result_id.clone())
                .await?;
        }
        self.graph_store
            .set_node_property_int(node_id, "schema_version", task.schema_version as i64)
            .await?;
        Ok(())
    }

    fn outcome_for(
        &self,
        record: &TaskRecord,
        disposition: SchedulerTaskDisposition,
        message: String,
    ) -> SchedulerTaskOutcome {
        SchedulerTaskOutcome {
            node_id: record.node_id,
            arxiv_id: record.task.arxiv_id.clone(),
            disposition,
            message,
            task_type: record.task.task_type.as_str().to_string(),
            attempt: record.task.attempts.len() as u32,
            epoch: record.task.current_epoch(),
            logical_result_id: record
                .task
                .logical_result
                .as_ref()
                .map(|result| result.result_id.clone()),
        }
    }

    /// Read a DateTime-typed node property as Unix seconds (canonical RFC3339
    /// UTC string first, legacy integer fallback — PIR-008 bounded decoder).
    async fn read_datetime_secs(&self, node_id: u64, key: &str) -> Option<i64> {
        if let Some(raw) = self
            .graph_store
            .get_node_property_string(node_id, key)
            .await
        {
            if raw.is_empty() || raw == kg_ontology::temporal::OPEN {
                return None;
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&raw) {
                return Some(dt.timestamp());
            }
            return raw.parse::<i64>().ok();
        }
        self.graph_store.get_node_property_int(node_id, key).await
    }
}

// ─── Wire helpers (PIR-008 canonical RFC3339 UTC) ─────────────────────

/// Canonical RFC3339 UTC wire form; `None` for out-of-DateTime-range values
/// (e.g. `i64::MAX` = never-due-again). The durable `task_json` holds the
/// truth; the flat mirror simply stays absent for unrepresentable instants.
fn to_rfc3339_wire_opt(secs: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.to_rfc3339())
}

fn task_status_str(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
    }
}

fn task_type_from_str(raw: &str) -> Option<TaskType> {
    match raw {
        "open_alex_enrich" => Some(TaskType::OpenAlexEnrich),
        "ingest" => Some(TaskType::Ingest),
        "reparse" => Some(TaskType::Reparse),
        "extract" => Some(TaskType::Extract),
        "embed_stale" => Some(TaskType::EmbedStale),
        "graph_write" => Some(TaskType::GraphWrite),
        _ => None,
    }
}

fn task_priority_from_str(raw: &str) -> Option<TaskPriority> {
    match raw {
        "low" => Some(TaskPriority::Low),
        "medium" => Some(TaskPriority::Medium),
        "high" => Some(TaskPriority::High),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use da_adapters::SamyamaGraphStore;
    use da_domain::scheduler::TaskDependency;
    use da_ports::clock::FixedClock;
    use da_ports::openalex::{OpenAlexError, OpenAlexResult, OpenAlexWork};
    use std::sync::Mutex;

    /// Fixed epoch for deterministic runner tests (no real sleep anywhere).
    const T0: i64 = 1_784_000_000;

    fn fixed_clock(secs: i64) -> Arc<dyn da_ports::Clock> {
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

    /// Recording fake executor for deterministic outcomes.
    struct FakeExecutor {
        task_type: TaskType,
        outcome: SchedulerExecuteOutcome,
        /// Optionally records executed task vids in order (priority tests).
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

    fn completed_outcome() -> SchedulerExecuteOutcome {
        SchedulerExecuteOutcome::Completed {
            summary: "done".into(),
            evidence_refs: vec!["stage:parse:cache:sha256:cache-key:sha256:abc123".into()],
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
        let mut task =
            PendingTask::new_openalex_enrich(arxiv_id, &RetryPolicy::default(), added_at);
        task.task_type = task_type;
        task.priority = priority;
        task.resource_class = resource_class;
        task.dependencies = dependencies;
        task.next_retry = next_retry;
        task
    }

    /// Create a durable SchedulerTask node from a PendingTask (task_json +
    /// the flat operational mirrors the runner writes).
    async fn seed_durable(store: &SamyamaGraphStore, task: &PendingTask) -> u64 {
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
            .set_node_property_bool(node, "import_eligible", false) // D127
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

    /// Create a legacy flat SchedulerTask node (pre-durable-plane writer).
    async fn seed_legacy_flat(store: &SamyamaGraphStore, arxiv_id: &str, next_retry: i64) -> u64 {
        let task = PendingTask::new_openalex_enrich(arxiv_id, &RetryPolicy::default(), 1);
        let node = store.create_node("SchedulerTask").await.unwrap();
        store
            .set_node_property_string(node, "vid", task.task_vid())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "arxiv_id", arxiv_id.to_string())
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
            .set_node_property_int(node, "retry_count", 0)
            .await
            .unwrap();
        store
            .set_node_property_string(
                node,
                "next_retry",
                chrono::DateTime::from_timestamp(next_retry, 0)
                    .unwrap()
                    .to_rfc3339(),
            )
            .await
            .unwrap();
        store
            .set_node_property_bool(node, "import_eligible", false)
            .await
            .unwrap();
        store
            .set_node_property_int(node, "schema_version", 1)
            .await
            .unwrap();
        node
    }

    fn shared_store() -> Arc<SamyamaGraphStore> {
        Arc::new(SamyamaGraphStore::new())
    }

    fn runner_with(
        store: Arc<SamyamaGraphStore>,
        executor: FakeExecutor,
        now: i64,
    ) -> SchedulerRunner {
        let store_arc: Arc<dyn DirectGraphStore> = store;
        SchedulerRunner::new(Arc::clone(&store_arc), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(now))
            .with_worker_id("test-worker")
            .with_executor(Arc::new(executor))
    }

    async fn persisted_task(store: &SamyamaGraphStore, node_id: u64) -> PendingTask {
        let json = store
            .get_node_property_string(node_id, "task_json")
            .await
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    // ─── Durable lifecycle ────────────────────────────────────────────────

    #[tokio::test]
    async fn durable_claim_heartbeat_and_one_logical_result() {
        let store = shared_store();
        let task = durable_task(
            "2401.00001",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        let node = seed_durable(&store, &task).await;

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();

        assert_eq!(report.due, 1);
        assert_eq!(report.completed, 1);
        assert_eq!(report.pending, 0);
        assert_eq!(report.recovered, 0);
        assert_eq!(
            report.outcomes[0].disposition,
            SchedulerTaskDisposition::Completed
        );

        let persisted = persisted_task(&store, node).await;
        assert_eq!(persisted.status, TaskStatus::Completed);
        assert_eq!(persisted.attempts.len(), 1);
        assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Succeeded);
        assert_eq!(persisted.attempts[0].worker, "test-worker");
        assert_eq!(
            persisted.attempts[0].evidence_refs,
            vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
        );
        let result = persisted.logical_result.as_ref().expect("published");
        assert_eq!(
            result.evidence_refs,
            vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
        );
        assert_eq!(persisted.lease.as_ref().unwrap().owner, "test-worker");
        assert_eq!(persisted.lease.as_ref().unwrap().epoch, 1);
        assert_eq!(persisted.lease.as_ref().unwrap().heartbeat_count, 1);

        // Flat mirrors expose the durable plane (slice verification surface).
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
            Some("test-worker")
        );
        assert_eq!(
            store.get_node_property_int(node, "lease_epoch").await,
            Some(1)
        );
        assert_eq!(
            store.get_node_property_int(node, "attempt_count").await,
            Some(1)
        );
        assert_eq!(
            store
                .get_node_property_string(node, "logical_result_id")
                .await,
            Some(result.result_id.clone())
        );
        // D127 stays false through the durable lifecycle.
        assert_eq!(
            store.get_node_property_bool(node, "import_eligible").await,
            Some(false)
        );

        // A second pass must not re-execute a completed task.
        let report2 = runner.run_due().await.unwrap();
        assert_eq!(report2.due, 0);
        assert_eq!(report2.completed, 0);
    }

    #[tokio::test]
    async fn retryable_failure_advances_retry_and_keeps_full_lineage() {
        let store = shared_store();
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

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: SchedulerExecuteOutcome::Retryable {
                    error_class: "network".into(),
                    message: "timeout".into(),
                },
                log: None,
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();

        assert_eq!(report.pending, 1);
        assert_eq!(report.failed, 0);
        let persisted = persisted_task(&store, node).await;
        assert_eq!(persisted.status, TaskStatus::Pending);
        assert_eq!(persisted.retry_count, 1);
        assert_eq!(persisted.attempts.len(), 1);
        assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Failed);
        assert!(persisted.attempts[0].retryable);
        assert_eq!(
            persisted.attempts[0].error_class.as_deref(),
            Some("network")
        );
        assert!(persisted.logical_result.is_none());
        assert!(
            persisted.next_retry > T0 + 1000,
            "retry scheduled in future"
        );
        assert_eq!(
            store.get_node_property_int(node, "retry_count").await,
            Some(1)
        );

        // Second attempt after the retry window: full lineage preserved.
        let runner2 = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: SchedulerExecuteOutcome::Retryable {
                    error_class: "network".into(),
                    message: "timeout".into(),
                },
                log: None,
            },
            persisted.next_retry + 1,
        );
        let report2 = runner2.run_due().await.unwrap();
        assert_eq!(report2.pending, 1);
        let persisted2 = persisted_task(&store, node).await;
        assert_eq!(persisted2.attempts.len(), 2);
        assert_eq!(persisted2.attempts[1].attempt, 2);
        assert_eq!(persisted2.attempts[1].outcome, AttemptOutcome::Failed);
        // Attempt lineage is immutable: the first attempt is untouched.
        assert_eq!(persisted2.attempts[0].attempt, 1);
    }

    #[tokio::test]
    async fn expired_lease_reclaimed_with_new_fence_and_stale_attempt_aborted() {
        let store = shared_store();
        // Worker-a claimed at T0 (term 300) and crashed mid-execution:
        // attempt 1 is still Running when the lease expires.
        let mut task = durable_task(
            "2401.00003",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        task.claim("worker-a", T0, 300).unwrap();
        let node = seed_durable(&store, &task).await;

        // Worker-b recovers the expired lease at T0+301.
        let store_arc: Arc<dyn DirectGraphStore> = store.clone();
        let runner_b = SchedulerRunner::new(Arc::clone(&store_arc), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 301))
            .with_worker_id("worker-b")
            .with_executor(Arc::new(FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            }));

        let report = runner_b.run_due().await.unwrap();
        assert_eq!(report.recovered, 1);
        assert_eq!(report.completed, 1);

        let persisted = persisted_task(&store, node).await;
        assert_eq!(persisted.attempts.len(), 2, "crash attempt retained");
        assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Aborted);
        assert_eq!(persisted.attempts[0].epoch, 1);
        assert_eq!(
            persisted.attempts[1].epoch, 2,
            "fence incremented on reclaim"
        );
        assert_eq!(persisted.attempts[1].outcome, AttemptOutcome::Succeeded);
        assert_eq!(persisted.lease.as_ref().unwrap().owner, "worker-b");
        assert_eq!(persisted.lease.as_ref().unwrap().epoch, 2);
        assert!(persisted.logical_result.is_some(), "one logical result");
        assert_eq!(
            store.get_node_property_int(node, "lease_epoch").await,
            Some(2)
        );

        // The stale holder's authoritative writes are rejected at the fence:
        // worker-a cannot re-publish (domain publish is fence-checked) and the
        // durable lease already belongs to worker-b.
        let stale_publish =
            persisted
                .clone()
                .publish_logical_result(1, "stale".into(), vec![], T0 + 400);
        assert!(
            stale_publish.is_err(),
            "stale fence publish must be rejected"
        );
    }

    #[tokio::test]
    async fn blocked_dependency_prevents_claim_until_satisfied() {
        let store = shared_store();
        // The depended-on task is still pending (not due) — the dependent must
        // stay blocked without a lease or attempt.
        let dep = durable_task(
            "2401.00010",
            TaskType::OpenAlexEnrich,
            TaskPriority::High,
            TaskResourceClass::Standard,
            vec![],
            T0 + 1_000_000,
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
                task_vid: dep_vid.clone(),
                satisfied_at: None,
            }],
            T0,
            T0,
        );
        let main_node = seed_durable(&store, &main).await;

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();

        assert_eq!(report.blocked, 1);
        assert_eq!(report.completed, 0);
        let persisted = persisted_task(&store, main_node).await;
        assert_eq!(persisted.status, TaskStatus::Pending);
        assert!(
            persisted.lease.is_none(),
            "blocked task must not be claimed"
        );
        assert!(persisted.attempts.is_empty());
        assert_eq!(
            store
                .get_node_property_string(main_node, "status")
                .await
                .as_deref(),
            Some("pending")
        );

        // Complete the dependency, then the dependent runs and publishes.
        let mut dep_done = persisted_task(&store, dep_node).await;
        dep_done.next_retry = T0 + 1000;
        store
            .set_node_property_string(
                dep_node,
                "task_json",
                serde_json::to_string(&dep_done).unwrap(),
            )
            .await
            .unwrap();
        let report2 = runner.run_due().await.unwrap();
        assert_eq!(
            report2.completed, 2,
            "dependency and dependent both publish in the same priority-ordered pass"
        );
        let persisted2 = persisted_task(&store, main_node).await;
        assert_eq!(persisted2.status, TaskStatus::Completed);
        assert!(persisted2.logical_result.is_some());
        assert!(
            persisted2.dependencies[0].is_satisfied(),
            "dependency satisfaction persisted"
        );
    }

    #[tokio::test]
    async fn priority_ordering_runs_high_first() {
        let store = shared_store();
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut vids = Vec::new();
        for (arxiv, priority) in [
            ("2401.00020", TaskPriority::Low),
            ("2401.00021", TaskPriority::Medium),
            ("2401.00022", TaskPriority::High),
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
            vids.push(task.task_vid());
            seed_durable(&store, &task).await;
        }

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: Some(Arc::clone(&log)),
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();
        assert_eq!(report.completed, 3);

        let order = log.lock().unwrap().clone();
        assert_eq!(
            order,
            vec![vids[2].clone(), vids[1].clone(), vids[0].clone()],
            "High must run before Medium before Low"
        );
    }

    #[tokio::test]
    async fn resource_unavailable_blocks_claim() {
        let store = shared_store();
        let task = durable_task(
            "2401.00030",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Network,
            vec![],
            T0,
            T0,
        );
        let node = seed_durable(&store, &task).await;

        let store_arc: Arc<dyn DirectGraphStore> = store.clone();
        let runner = SchedulerRunner::new(Arc::clone(&store_arc), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 1000))
            .with_worker_id("test-worker")
            .with_executor(Arc::new(FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            }))
            .with_resource_availability(|class| class != TaskResourceClass::Network);

        let report = runner.run_due().await.unwrap();
        assert_eq!(report.blocked, 1);
        assert_eq!(report.completed, 0);
        let persisted = persisted_task(&store, node).await;
        assert!(persisted.lease.is_none());
        assert!(persisted.attempts.is_empty());

        // Once the resource is available the same task runs and completes.
        let runner2 = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            },
            T0 + 1000,
        );
        let report2 = runner2.run_due().await.unwrap();
        assert_eq!(report2.completed, 1);
    }

    #[tokio::test]
    async fn no_executor_fails_closed_without_claim() {
        let store = shared_store();
        // The runner registers only OpenAlexEnrich — Extract must fail closed.
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

        let store_arc: Arc<dyn DirectGraphStore> = store.clone();
        let runner = SchedulerRunner::new(Arc::clone(&store_arc), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 1000))
            .with_worker_id("test-worker");

        let report = runner.run_due().await.unwrap();
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
    async fn terminal_failure_marks_failed_without_retry() {
        let store = shared_store();
        let task = durable_task(
            "2401.00050",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        let node = seed_durable(&store, &task).await;

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: SchedulerExecuteOutcome::Terminal {
                    error_class: "invalid_payload".into(),
                    message: "schema contract rejected output".into(),
                },
                log: None,
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();
        assert_eq!(report.failed, 1);
        let persisted = persisted_task(&store, node).await;
        assert_eq!(persisted.status, TaskStatus::Failed);
        assert_eq!(persisted.retry_count, 0, "terminal failure is not a retry");
        assert_eq!(persisted.attempts[0].outcome, AttemptOutcome::Failed);
        assert!(!persisted.attempts[0].retryable);
        assert_eq!(
            persisted.attempts[0].error_class.as_deref(),
            Some("invalid_payload")
        );
        assert!(persisted.logical_result.is_none());

        // Failed tasks are never due again.
        let report2 = runner.run_due().await.unwrap();
        assert_eq!(report2.due, 0);
    }

    #[tokio::test]
    async fn heartbeat_pass_extends_own_lease_only() {
        let store = shared_store();
        let mut mine = durable_task(
            "2401.00060",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        mine.claim("test-worker", T0, 300).unwrap();
        let my_node = seed_durable(&store, &mine).await;

        let mut other = durable_task(
            "2401.00061",
            TaskType::OpenAlexEnrich,
            TaskPriority::Medium,
            TaskResourceClass::Standard,
            vec![],
            T0,
            T0,
        );
        other.claim("other-worker", T0, 300).unwrap();
        seed_durable(&store, &other).await;

        let store_arc: Arc<dyn DirectGraphStore> = store.clone();
        let runner = SchedulerRunner::new(Arc::clone(&store_arc), Arc::new(NoOpOpenAlex))
            .with_clock(fixed_clock(T0 + 100))
            .with_worker_id("test-worker");

        let count = runner.heartbeat_due_tasks().await.unwrap();
        assert_eq!(count, 1, "only this worker's lease is heartbeated");

        let mine_persisted = persisted_task(&store, my_node).await;
        let other_node = store
            .find_node_by_string_property("SchedulerTask", "vid", &other.task_vid())
            .await
            .unwrap();
        let other_persisted = persisted_task(&store, other_node).await;

        assert_eq!(
            mine_persisted.lease.as_ref().unwrap().expires_at,
            T0 + 400,
            "heartbeat extends expiry by the term"
        );
        assert_eq!(mine_persisted.lease.as_ref().unwrap().heartbeat_count, 1);
        assert_eq!(
            other_persisted.lease.as_ref().unwrap().expires_at,
            T0 + 300,
            "foreign lease untouched"
        );
    }

    #[tokio::test]
    async fn legacy_flat_node_upgraded_to_durable_on_claim() {
        let store = shared_store();
        // Simulate a pre-durable-plane snapshot: flat properties, no task_json.
        let node = seed_legacy_flat(&store, "2401.00070", T0).await;

        let runner = runner_with(
            Arc::clone(&store),
            FakeExecutor {
                task_type: TaskType::OpenAlexEnrich,
                outcome: completed_outcome(),
                log: None,
            },
            T0 + 1000,
        );
        let report = runner.run_due().await.unwrap();
        assert_eq!(report.completed, 1);

        let persisted = persisted_task(&store, node).await;
        assert_eq!(persisted.schema_version, 2, "upgraded to durable plane");
        assert_eq!(persisted.status, TaskStatus::Completed);
        assert_eq!(persisted.attempts.len(), 1);
        assert_eq!(
            store
                .get_node_property_string(node, "status")
                .await
                .as_deref(),
            Some("completed")
        );
    }
}
