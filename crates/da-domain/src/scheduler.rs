//! Scheduler domain types — durable task plane (leases, priorities,
//! dependencies, resource classes, attempt lineage, one logical result).
//!
//! Tracks papers awaiting OpenAlex data with exponential backoff.
//! File-based persistence (survives process restarts).
//!
//! M399/S03 generalizes the scheduler beyond one adapter into a durable
//! task plane: tasks carry a fenced lease (monotonic epoch token that
//! increments on every claim), a bounded lease term with drift-checked
//! heartbeats, a closed priority domain, dependency references with DAG
//! acyclicity validation, a closed resource-class domain, immutable attempt
//! lineage, and a terminal logical-result identity published exactly once.
//!
//! The model is pure (no IO) and serializes stably: every new field on
//! `PendingTask` carries `#[serde(default)]` so persisted queues written
//! before the durable-plane extension stay readable (R092 wire discipline).

use crate::load::try_normalize_arxiv_id;
use crate::schema::{FieldType, NodeSchemaDef};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

// ─── Lease bounds (Q3: bounded term + drift window) ───

/// Default lease term in seconds (5 minutes).
pub const DEFAULT_LEASE_TERM_SECS: i64 = 300;
/// Hard upper bound on any single lease term (1 hour). A lease whose
/// `expires_at - acquired_at` exceeds this is rejected as tampered/corrupt.
pub const MAX_LEASE_TERM_SECS: i64 = 3600;
/// Bounded clock-drift window in seconds. Claims/heartbeats/records whose
/// timestamps precede the server reference by more than this are rejected
/// (skewed or replayed clock).
pub const MAX_CLOCK_DRIFT_SECS: i64 = 60;
/// Suggested heartbeat cadence in seconds (runner-side, not enforced here).
pub const HEARTBEAT_INTERVAL_SECS: i64 = 60;

/// Errors from the fenced-lease lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseError {
    #[error("lease not held")]
    NotHeld,
    #[error("lease already held by {owner} (epoch {epoch})")]
    AlreadyHeld { owner: String, epoch: u64 },
    #[error("lease expired at {expires_at}, now {now}")]
    Expired { expires_at: i64, now: i64 },
    #[error("stale fence: current {current}, presented {presented}")]
    StaleFence { current: u64, presented: u64 },
    #[error("clock drift: now {now} precedes reference {reference}")]
    ClockDrift { now: i64, reference: i64 },
    #[error("lease term {term}s exceeds bound")]
    TermTooLong { term: i64 },
    #[error("lease term {term}s out of range")]
    TermOutOfRange { term: i64 },
    #[error("epoch counter overflow")]
    EpochOverflow,
    #[error("timestamp overflow")]
    TimestampOverflow,
}

/// A durable lease: one worker's fenced claim on a task.
///
/// `epoch` is the monotonic fence token (Q3 requirement). It increments on
/// every claim, so a crashed/stalled worker that resumes after its lease was
/// reclaimed carries an older epoch; every authoritative write must validate
/// the current fence via [`Self::assert_fence`] before applying effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLease {
    /// Worker identity that holds the lease.
    pub owner: String,
    /// Fence token — monotonic, increments on every (re)claim.
    pub epoch: u64,
    /// When the current claim was made.
    pub acquired_at: i64,
    /// When the lease expires (bounded term). Server-side expiry check.
    pub expires_at: i64,
    /// Last successful heartbeat (drift-checked), if any.
    pub last_heartbeat_at: Option<i64>,
    /// Number of successful heartbeats under the current claim.
    pub heartbeat_count: u64,
}

impl TaskLease {
    /// Create a new lease claim with the given fence epoch. The term must be
    /// within `1..=MAX_LEASE_TERM_SECS` (bounded lease term).
    pub fn claim(owner: &str, now: i64, term: i64, epoch: u64) -> Result<Self, LeaseError> {
        if term <= 0 || term > MAX_LEASE_TERM_SECS {
            return Err(LeaseError::TermOutOfRange { term });
        }
        let expires_at = now.checked_add(term).ok_or(LeaseError::TimestampOverflow)?;
        Ok(Self {
            owner: owner.to_string(),
            epoch,
            acquired_at: now,
            expires_at,
            last_heartbeat_at: None,
            heartbeat_count: 0,
        })
    }

    /// True when the lease has expired at `now` (server-side expiry check).
    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at
    }

    /// True when `owner` is the current holder.
    pub fn is_held_by(&self, owner: &str) -> bool {
        self.owner == owner
    }

    /// Current fence token.
    pub fn fence(&self) -> u64 {
        self.epoch
    }

    /// Reject a presented epoch older than the current fence — the
    /// stale-holder-after-reclaim guard (Q3 item 1).
    pub fn assert_fence(&self, presented: u64) -> Result<(), LeaseError> {
        if presented == self.epoch {
            Ok(())
        } else {
            Err(LeaseError::StaleFence {
                current: self.epoch,
                presented,
            })
        }
    }

    /// Drift/term/expiry validation of a stored lease against server `now`
    /// (read-side, defense in depth): rejects future-dated records beyond the
    /// drift window, tampered terms, and expired leases.
    pub fn validate(&self, now: i64) -> Result<(), LeaseError> {
        if now < self.acquired_at.saturating_sub(MAX_CLOCK_DRIFT_SECS) {
            return Err(LeaseError::ClockDrift {
                now,
                reference: self.acquired_at,
            });
        }
        let term = self.expires_at.saturating_sub(self.acquired_at);
        if term > MAX_LEASE_TERM_SECS {
            return Err(LeaseError::TermTooLong { term });
        }
        if now >= self.expires_at {
            return Err(LeaseError::Expired {
                expires_at: self.expires_at,
                now,
            });
        }
        Ok(())
    }

    /// Extend the lease with a drift-checked, bounded heartbeat.
    /// Rejects: expired leases, future-dated heartbeats beyond the drift
    /// window, backward time travel, and out-of-range terms. The extended
    /// expiry is additionally capped at `acquired_at + MAX_LEASE_TERM_SECS`.
    pub fn heartbeat(&mut self, now: i64, term: i64) -> Result<(), LeaseError> {
        if term <= 0 || term > MAX_LEASE_TERM_SECS {
            return Err(LeaseError::TermOutOfRange { term });
        }
        self.validate(now)?;
        if let Some(lhb) = self.last_heartbeat_at
            && now < lhb.saturating_sub(MAX_CLOCK_DRIFT_SECS)
        {
            return Err(LeaseError::ClockDrift {
                now,
                reference: lhb,
            });
        }
        let new_expiry = now.checked_add(term).ok_or(LeaseError::TimestampOverflow)?;
        let cap = self
            .acquired_at
            .checked_add(MAX_LEASE_TERM_SECS)
            .ok_or(LeaseError::TimestampOverflow)?;
        self.expires_at = new_expiry.min(cap);
        self.last_heartbeat_at = Some(now);
        self.heartbeat_count += 1;
        Ok(())
    }
}

/// Priority level for task scheduling (ADR-037 §4.3).
///
/// `Default = Medium` keeps legacy persisted tasks (which carry no `priority`
/// field) at the same priority their `TaskType` implies, so `#[serde(default)]`
/// is behavior-preserving for existing queues.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    #[default]
    Medium,
    High,
}

impl TaskPriority {
    /// Canonical string form (matches serde `snake_case` wire).
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskPriority::Low => "low",
            TaskPriority::Medium => "medium",
            TaskPriority::High => "high",
        }
    }
}

/// Resource class a task must run under (closed domain — unknown classes
/// fail closed at deserialization, Q3 item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskResourceClass {
    /// No special resource constraints (default).
    #[default]
    Standard,
    /// Requires outbound network access.
    Network,
    /// Requires compute-intensive execution.
    Compute,
    /// Requires durable storage access.
    Storage,
}

impl TaskResourceClass {
    /// Canonical string form (matches serde `snake_case` wire).
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskResourceClass::Standard => "standard",
            TaskResourceClass::Network => "network",
            TaskResourceClass::Compute => "compute",
            TaskResourceClass::Storage => "storage",
        }
    }
}

/// Reference to a task this task depends on (blocked until satisfied).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskDependency {
    /// Logical identity of the depended-on task (its scheduler task vid or
    /// logical result identity).
    pub task_vid: String,
    /// When the dependency was observed satisfied; `None` = not yet.
    pub satisfied_at: Option<i64>,
}

impl TaskDependency {
    /// True when this dependency has been observed satisfied.
    pub fn is_satisfied(&self) -> bool {
        self.satisfied_at.is_some()
    }
}

/// True when every dependency is satisfied.
pub fn all_dependencies_satisfied(deps: &[TaskDependency]) -> bool {
    deps.iter().all(TaskDependency::is_satisfied)
}

/// A node in a task dependency graph (for cycle detection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    /// Logical task identity.
    pub vid: String,
    /// Vids of tasks this node depends on.
    pub depends_on: Vec<String>,
}

/// Detect a cycle in a dependency graph (Kahn + three-color DFS).
///
/// Returns the first cycle found as a list of vids (with the closing repeat
/// of the start vid appended), or `None` if the graph is acyclic. Edges to
/// unknown vids are ignored (the caller decides how to treat them). Duplicate
/// input vids are treated as an invalid record and reported as a trivial
/// cycle containing the duplicated vid.
pub fn find_dependency_cycle(nodes: &[DependencyNode]) -> Option<Vec<String>> {
    let known: HashMap<&str, &DependencyNode> = nodes.iter().map(|n| (n.vid.as_str(), n)).collect();
    if known.len() != nodes.len() {
        // Duplicate vid — invalid record; surface the duplicated identity.
        let mut seen = std::collections::HashSet::new();
        for n in nodes {
            if !seen.insert(n.vid.as_str()) {
                return Some(vec![n.vid.clone(), n.vid.clone()]);
            }
        }
        return None; // unreachable
    }

    // Adjacency vid -> depended-on vids that are themselves known.
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut indegree: HashMap<&str, usize> = nodes.iter().map(|n| (n.vid.as_str(), 0)).collect();
    for n in nodes {
        let mut deps: Vec<&str> = Vec::new();
        for d in &n.depends_on {
            if known.contains_key(d.as_str()) {
                deps.push(d.as_str());
                *indegree.entry(d.as_str()).or_insert(0) += 1;
            }
        }
        adj.insert(n.vid.as_str(), deps);
    }

    // Kahn's algorithm: a DAG fully drains the zero-indegree queue.
    let mut queue: VecDeque<&str> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(v, _)| *v)
        .collect();
    let mut processed = 0usize;
    while let Some(v) = queue.pop_front() {
        processed += 1;
        if let Some(deps) = adj.get(v) {
            for d in deps {
                let e = indegree.get_mut(d).expect("known node");
                *e -= 1;
                if *e == 0 {
                    queue.push_back(d);
                }
            }
        }
    }
    if processed == nodes.len() {
        return None;
    }

    // Cycle exists: three-color DFS from the first unprocessed node.
    let start = indegree
        .iter()
        .find(|(_, d)| **d > 0)
        .map(|(v, _)| v.to_string())?;
    let mut color: HashMap<&str, u8> = HashMap::new(); // 0=white 1=gray 2=black
    let mut stack: Vec<&str> = Vec::new();
    fn dfs<'a>(
        v: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        color: &mut HashMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        color.insert(v, 1);
        stack.push(v);
        if let Some(deps) = adj.get(v) {
            for d in deps {
                match color.get(d).copied().unwrap_or(0) {
                    1 => {
                        let pos = stack.iter().position(|x| *x == *d)?;
                        let mut cycle: Vec<String> =
                            stack[pos..].iter().map(|s| s.to_string()).collect();
                        cycle.push((*d).to_string());
                        return Some(cycle);
                    }
                    0 => {
                        if let Some(c) = dfs(d, adj, color, stack) {
                            return Some(c);
                        }
                    }
                    _ => {}
                }
            }
        }
        stack.pop();
        color.insert(v, 2);
        None
    }
    dfs(&start, &adj, &mut color, &mut stack)
}

/// Outcome of a single execution attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    /// Attempt claimed and executing (lease held).
    Running,
    /// Attempt succeeded.
    Succeeded,
    /// Attempt failed (retryable per `TaskAttempt::retryable`).
    Failed,
    /// Attempt abandoned (worker lost, lease reclaimed, or execution aborted).
    Aborted,
}

/// One execution attempt of a task — immutable lineage record.
///
/// Persisted error information is sanitized to an error *class* plus a
/// retryable flag (Q3 item "sanitize before persistence"); raw paths and
/// stack traces never enter the durable record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAttempt {
    /// 1-based attempt ordinal.
    pub attempt: u32,
    /// Worker that ran this attempt.
    pub worker: String,
    /// Fence token at claim time (epoch of the lease this attempt ran under).
    pub epoch: u64,
    /// When the attempt started.
    pub started_at: i64,
    /// When the attempt finished (None while Running).
    pub finished_at: Option<i64>,
    /// Terminal outcome (Running while in flight).
    pub outcome: AttemptOutcome,
    /// Sanitized error class (e.g. `network`, `not_found`, `timeout`).
    pub error_class: Option<String>,
    /// Whether a failure of this attempt is retryable.
    pub retryable: bool,
    /// Stage evidence references (cache-key sha256 / canonical stage ids).
    pub evidence_refs: Vec<String>,
}

/// Terminal logical result — the single authoritative outcome of a task
/// across all attempts. Publishing is idempotent keyed by `result_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalTaskResult {
    /// Stable identity (deterministic from task identity).
    pub result_id: String,
    /// When the result was published.
    pub published_at: i64,
    /// Fence token that published it.
    pub published_by_epoch: u64,
    /// Sanitized one-line outcome summary.
    pub summary: String,
    /// Stage evidence references backing the result.
    pub evidence_refs: Vec<String>,
}

/// Deterministic identity of the logical result for a scheduler task vid.
/// Stable across attempts and workers — idempotent publish key.
pub fn logical_result_vid(task_vid: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("logical-result|{task_vid}").as_bytes());
    format!("vid:logical-result:{}", hex::encode(digest))
}

/// Errors from attempt-record finalization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttemptError {
    #[error("no running attempt for epoch {epoch}")]
    NoRunningAttempt { epoch: u64 },
    #[error("stale fence: current {current}, presented {presented}")]
    StaleFence { current: u64, presented: u64 },
}

/// Errors from logical-result publishing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PublishError {
    #[error("cannot publish logical result without a held lease")]
    NoLease,
    #[error("stale fence: current {current}, presented {presented}")]
    StaleFence { current: u64, presented: u64 },
}

/// Errors from read-side task record validation (Q3 defense in depth).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskRecordError {
    #[error("invalid arxiv id: {0}")]
    InvalidArxivId(String),
    #[error("task depends on itself")]
    SelfDependency,
    #[error("invalid lease: {0}")]
    InvalidLease(#[from] LeaseError),
}

/// A pending enrichment task in the retry queue.
///
/// M399/S03: every new field carries `#[serde(default)]` so queues persisted
/// before the durable-plane extension remain readable (R092 wire stability).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTask {
    pub arxiv_id: String,
    pub task_type: TaskType,
    pub added_at: i64,
    pub retry_count: u32,
    pub last_retry: Option<i64>,
    pub next_retry: i64,
    pub status: TaskStatus,
    /// Schema version of the durable task plane (1 = original retry queue,
    /// 2 = durable lease plane). Defaults to 1 for legacy records.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Scheduling priority; defaults to the legacy `TaskType::priority()`
    /// value for records written before the plane extension.
    #[serde(default)]
    pub priority: TaskPriority,
    /// Current fenced lease, if the task is claimed by a worker.
    #[serde(default)]
    pub lease: Option<TaskLease>,
    /// Dependency references; execution is blocked until all satisfied.
    #[serde(default)]
    pub dependencies: Vec<TaskDependency>,
    /// Resource class the task runs under.
    #[serde(default)]
    pub resource_class: TaskResourceClass,
    /// Complete attempt lineage (immutable once recorded).
    #[serde(default)]
    pub attempts: Vec<TaskAttempt>,
    /// Terminal logical result (published exactly once, idempotently).
    #[serde(default)]
    pub logical_result: Option<LogicalTaskResult>,
}

fn default_schema_version() -> u32 {
    1
}

/// What kind of lazy-load task this is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Paper not yet indexed by OpenAlex — retry enrichment later.
    OpenAlexEnrich,
    /// New paper to ingest (HIGH priority, ADR-037 §4.3).
    Ingest,
    /// Re-parse a failed paper (HIGH priority).
    Reparse,
    /// Extract entities from an ingested paper (MED priority).
    Extract,
    /// Re-embed stale embeddings (LOW priority).
    EmbedStale,
    /// Write accumulated data to graph (LOW priority).
    GraphWrite,
}

pub fn scheduler_task_vid(task_type: &TaskType, arxiv_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{}|{}", task_type.as_str(), arxiv_id.trim()).as_bytes());
    format!("vid:scheduler-task:{}", hex::encode(digest))
}

impl TaskType {
    /// Canonical string representation for graph node property `task_type`.
    /// MUST match `#[serde(rename_all = "snake_case")]` serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::OpenAlexEnrich => "open_alex_enrich",
            TaskType::Ingest => "ingest",
            TaskType::Reparse => "reparse",
            TaskType::Extract => "extract",
            TaskType::EmbedStale => "embed_stale",
            TaskType::GraphWrite => "graph_write",
        }
    }

    /// Priority level from ADR-037 §4.3.
    pub fn priority(&self) -> TaskPriority {
        match self {
            TaskType::Ingest | TaskType::Reparse => TaskPriority::High,
            TaskType::Extract | TaskType::OpenAlexEnrich => TaskPriority::Medium,
            TaskType::EmbedStale | TaskType::GraphWrite => TaskPriority::Low,
        }
    }
}

/// Task lifecycle state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting for next retry window.
    Pending,
    /// Successfully completed — task resolved.
    Completed,
    /// Max retries exceeded — gave up.
    Failed,
}

/// Retry policy — re-exported from kg_ontology::retry (M289-u4fzx5/S05).
pub use kg_ontology::retry::RetryPolicy;

/// Is this task due for retry now?
pub fn is_due(task: &PendingTask, now: i64) -> bool {
    task.status == TaskStatus::Pending && task.next_retry <= now
}

impl PendingTask {
    /// Create a new pending task for OpenAlex enrichment.
    pub fn new_openalex_enrich(arxiv_id: &str, policy: &RetryPolicy, now: i64) -> Self {
        Self {
            arxiv_id: arxiv_id.to_string(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: now,
            retry_count: 0,
            last_retry: None,
            next_retry: policy.next_retry_ts(0, now),
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskType::OpenAlexEnrich.priority(),
            lease: None,
            dependencies: Vec::new(),
            resource_class: TaskResourceClass::Standard,
            attempts: Vec::new(),
            logical_result: None,
        }
    }

    /// Record a retry attempt and compute next retry time.
    pub fn record_retry(&mut self, policy: &RetryPolicy, now: i64) {
        self.retry_count += 1;
        self.last_retry = Some(now);
        if policy.should_give_up(self.retry_count) {
            self.status = TaskStatus::Failed;
            self.next_retry = i64::MAX; // never due again
        } else {
            self.next_retry = policy.next_retry_ts(self.retry_count, now);
        }
    }

    /// Mark task as successfully completed.
    pub fn complete(&mut self, now: i64) {
        self.status = TaskStatus::Completed;
        self.last_retry = Some(now);
    }

    /// Logical identity of this task.
    pub fn task_vid(&self) -> String {
        scheduler_task_vid(&self.task_type, &self.arxiv_id)
    }

    /// True when the task is claimable: pending, due, and either unleased or
    /// holding an expired lease (recoverable).
    pub fn is_claimable(&self, now: i64) -> bool {
        is_due(self, now)
            && match &self.lease {
                None => true,
                Some(lease) => lease.is_expired(now),
            }
    }

    /// True when all dependencies are satisfied and the resource class is
    /// available (readiness gate).
    pub fn is_ready(&self, resource_available: impl Fn(TaskResourceClass) -> bool) -> bool {
        all_dependencies_satisfied(&self.dependencies) && resource_available(self.resource_class)
    }

    /// Claim (or reclaim an expired lease) for `owner`, opening a Running
    /// attempt. Returns the new fence epoch. Reclaiming an expired lease
    /// increments the fence — a stale holder's later writes are rejected.
    pub fn claim(&mut self, owner: &str, now: i64, term: i64) -> Result<u64, LeaseError> {
        match self.lease.as_ref() {
            None => {
                self.lease = Some(TaskLease::claim(owner, now, term, 1)?);
            }
            Some(lease) if lease.is_expired(now) => {
                // Reclaim of an expired lease: fence increments, stale holder
                // (old epoch) is rejected on its next authoritative write.
                let next_epoch = lease
                    .epoch
                    .checked_add(1)
                    .ok_or(LeaseError::EpochOverflow)?;
                self.lease = Some(TaskLease::claim(owner, now, term, next_epoch)?);
            }
            Some(lease) => {
                // Still held (by anyone) — no reclaim while valid.
                return Err(LeaseError::AlreadyHeld {
                    owner: lease.owner.clone(),
                    epoch: lease.epoch,
                });
            }
        }
        let epoch = self.lease.as_ref().expect("just claimed").epoch;
        let attempt = (self.attempts.len() as u32).saturating_add(1);
        self.attempts.push(TaskAttempt {
            attempt,
            worker: owner.to_string(),
            epoch,
            started_at: now,
            finished_at: None,
            outcome: AttemptOutcome::Running,
            error_class: None,
            retryable: true,
            evidence_refs: Vec::new(),
        });
        Ok(epoch)
    }

    /// Current fence epoch, if a lease is held.
    pub fn current_epoch(&self) -> Option<u64> {
        self.lease.as_ref().map(TaskLease::fence)
    }

    /// Drift-checked, bounded heartbeat under the current lease.
    pub fn heartbeat(&mut self, now: i64, term: i64) -> Result<(), LeaseError> {
        let lease = self.lease.as_mut().ok_or(LeaseError::NotHeld)?;
        lease.heartbeat(now, term)
    }

    /// Finalize the Running attempt for `epoch` (fence-checked). The error
    /// class is sanitized by the caller before persistence.
    pub fn finish_attempt(
        &mut self,
        epoch: u64,
        outcome: AttemptOutcome,
        error_class: Option<String>,
        retryable: bool,
        evidence_refs: Vec<String>,
        now: i64,
    ) -> Result<(), AttemptError> {
        if let Some(current) = self.current_epoch()
            && epoch != current
        {
            return Err(AttemptError::StaleFence {
                current,
                presented: epoch,
            });
        }
        let attempt = self
            .attempts
            .iter_mut()
            .rev()
            .find(|a| a.epoch == epoch && a.outcome == AttemptOutcome::Running)
            .ok_or(AttemptError::NoRunningAttempt { epoch })?;
        attempt.finished_at = Some(now);
        attempt.outcome = outcome;
        attempt.error_class = error_class;
        attempt.retryable = retryable;
        attempt.evidence_refs = evidence_refs;
        Ok(())
    }

    /// Finalize any Running attempts whose epoch no longer matches the current
    /// lease fence as Aborted — crashed/stolen workers. Called by the
    /// recoverer so attempt history stays complete.
    pub fn mark_stale_attempts_aborted(&mut self, now: i64) {
        let current = self.current_epoch();
        for a in self.attempts.iter_mut() {
            if a.outcome == AttemptOutcome::Running && Some(a.epoch) != current {
                a.outcome = AttemptOutcome::Aborted;
                a.finished_at = Some(now);
            }
        }
    }

    /// Publish the terminal logical result (fence-checked, idempotent).
    /// Returns the stable result identity. A stale worker (older fence) is
    /// rejected even if a result already exists — it must not re-publish.
    pub fn publish_logical_result(
        &mut self,
        epoch: u64,
        summary: String,
        evidence_refs: Vec<String>,
        now: i64,
    ) -> Result<String, PublishError> {
        let current = self.current_epoch().ok_or(PublishError::NoLease)?;
        if epoch != current {
            return Err(PublishError::StaleFence {
                current,
                presented: epoch,
            });
        }
        if let Some(existing) = &self.logical_result {
            return Ok(existing.result_id.clone());
        }
        let result_id = logical_result_vid(&self.task_vid());
        self.logical_result = Some(LogicalTaskResult {
            result_id: result_id.clone(),
            published_at: now,
            published_by_epoch: epoch,
            summary,
            evidence_refs,
        });
        self.status = TaskStatus::Completed;
        Ok(result_id)
    }

    /// Read-side record validation (Q3 items 4–5, defense in depth):
    /// rejects non-arXiv identities, self-dependencies, and corrupt leases
    /// (drift/term/expiry) against server `now`.
    pub fn validate_record(&self, now: i64) -> Result<(), TaskRecordError> {
        if try_normalize_arxiv_id(&self.arxiv_id).is_none() {
            return Err(TaskRecordError::InvalidArxivId(self.arxiv_id.clone()));
        }
        if self
            .dependencies
            .iter()
            .any(|d| d.task_vid == self.task_vid())
        {
            return Err(TaskRecordError::SelfDependency);
        }
        if let Some(lease) = &self.lease {
            lease.validate(now)?;
        }
        Ok(())
    }
}

// ─── SchedulerTask node schema (Layer 1 — Operational State) ───

/// SchedulerTask node schema.
/// Used by ETL Scheduler (ADR-040) to track pending/retry tasks in graph.
/// Not a knowledge node — operational state only.
pub struct SchedulerTaskSchema;

impl NodeSchemaDef for SchedulerTaskSchema {
    fn label(&self) -> &'static str {
        "SchedulerTask"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("arxiv_id", FieldType::String),
            ("task_type", FieldType::String),
            ("status", FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("retry_count", FieldType::Integer),
            ("next_retry", FieldType::DateTime),
            ("added_at", FieldType::DateTime),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            // ── durable task plane (M399/S03) ──
            ("priority", FieldType::String),
            ("resource_class", FieldType::String),
            ("lease_owner", FieldType::String),
            ("lease_epoch", FieldType::Integer),
            ("lease_acquired_at", FieldType::DateTime),
            ("lease_expires_at", FieldType::DateTime),
            ("lease_last_heartbeat_at", FieldType::DateTime),
            ("attempt_count", FieldType::Integer),
            ("logical_result_id", FieldType::String),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_default() {
        let p = RetryPolicy::default();
        assert_eq!(p.initial_delay, 86400);
        assert_eq!(p.backoff_multiplier, 3.0);
        assert_eq!(p.max_retries, 10);
    }

    #[test]
    fn test_delay_for_retry() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_for_retry(0), 86400); // 1 day
        assert_eq!(p.delay_for_retry(1), 3 * 86400); // 3 days
        assert_eq!(p.delay_for_retry(2), 9 * 86400); // 9 days
    }

    #[test]
    fn test_delay_capped_at_max() {
        let p = RetryPolicy::default();
        let delay = p.delay_for_retry(20); // very high retry count
        assert_eq!(delay, 30 * 86400); // capped at 30 days
    }

    #[test]
    fn test_new_task_is_pending() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.next_retry, now + 86400); // 1 day later
        // Durable plane defaults (M399/S03)
        assert_eq!(task.schema_version, 2);
        assert_eq!(task.priority, TaskPriority::Medium);
        assert_eq!(task.resource_class, TaskResourceClass::Standard);
        assert!(task.lease.is_none());
        assert!(task.attempts.is_empty());
        assert!(task.logical_result.is_none());
    }

    #[test]
    fn test_task_type_as_str_matches_serde_snake_case() {
        // as_str() MUST match serde rename_all="snake_case" serialization.
        // If these diverge, graph node properties won't match serialized
        // PendingTask JSON, breaking scheduler queries.
        let tt = TaskType::OpenAlexEnrich;
        let json = serde_json::to_string(&tt).unwrap();
        // serde produces "open_alex_enrich" (quoted JSON string)
        assert_eq!(json, "\"open_alex_enrich\"");
        // as_str() returns the same string without quotes
        assert_eq!(tt.as_str(), "open_alex_enrich");
        // NOT "openalex_enrich" (the old hardcoded value — missing underscore)
        assert_ne!(tt.as_str(), "openalex_enrich");
    }

    #[test]
    fn test_record_retry_increments_count() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);

        task.record_retry(&policy, now);
        assert_eq!(task.retry_count, 1);
        assert_eq!(task.next_retry, now + 3 * 86400); // 3 days later
    }

    #[test]
    fn test_give_up_after_max_retries() {
        let policy = RetryPolicy {
            max_retries: 3,
            ..Default::default()
        };
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);

        task.record_retry(&policy, now); // retry 1
        assert_eq!(task.status, TaskStatus::Pending);
        task.record_retry(&policy, now); // retry 2
        assert_eq!(task.status, TaskStatus::Pending);
        task.record_retry(&policy, now); // retry 3 = max
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[test]
    fn test_is_due() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);

        // Not due immediately (next_retry is now + 1 day)
        assert!(!is_due(&task, now));

        // Due after 1 day
        assert!(is_due(&task, now + 86400));
    }

    #[test]
    fn test_complete() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        task.complete(now);
        assert_eq!(task.status, TaskStatus::Completed);
    }

    // ─── Durable lease plane (M399/S03) ───

    #[test]
    fn test_lease_claim_expiry_and_fence_monotonic() {
        let mut task = PendingTask {
            arxiv_id: "2401.00001".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 1000,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![],
            resource_class: TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        };
        let now = 10_000;

        // Fresh claim → fence epoch 1, one Running attempt.
        let epoch1 = task
            .claim("worker-a", now, DEFAULT_LEASE_TERM_SECS)
            .unwrap();
        assert_eq!(epoch1, 1);
        assert_eq!(task.current_epoch(), Some(1));
        assert_eq!(task.attempts.len(), 1);
        assert_eq!(task.attempts[0].outcome, AttemptOutcome::Running);
        assert!(!task.lease.as_ref().unwrap().is_expired(now));
        assert!(
            task.lease
                .as_ref()
                .unwrap()
                .is_expired(now + DEFAULT_LEASE_TERM_SECS)
        );

        // Second claim while held → rejected (no fence bump).
        assert!(matches!(
            task.claim("worker-b", now + 10, DEFAULT_LEASE_TERM_SECS),
            Err(LeaseError::AlreadyHeld { .. })
        ));

        // Expire the lease, reclaim → fence increments (stale holder dies).
        let later = now + DEFAULT_LEASE_TERM_SECS + 1;
        let epoch2 = task
            .claim("worker-b", later, DEFAULT_LEASE_TERM_SECS)
            .unwrap();
        assert_eq!(epoch2, 2);
        assert_eq!(task.attempts.len(), 2);
        // Old epoch is now stale.
        assert!(matches!(
            task.lease.as_ref().unwrap().assert_fence(epoch1),
            Err(LeaseError::StaleFence {
                current: 2,
                presented: 1
            })
        ));
        assert!(task.lease.as_ref().unwrap().assert_fence(epoch2).is_ok());
    }

    #[test]
    fn test_heartbeat_extends_expiry_and_clock_drift_rejected() {
        let mut task = PendingTask {
            arxiv_id: "2401.00001".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 1000,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![],
            resource_class: TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        };
        let now = 10_000;
        task.claim("worker-a", now, 100).unwrap();
        let expires_before = task.lease.as_ref().unwrap().expires_at;

        // Valid heartbeat extends expiry.
        task.heartbeat(now + 50, 100).unwrap();
        let lease = task.lease.as_ref().unwrap();
        assert!(lease.expires_at > expires_before);
        assert_eq!(lease.heartbeat_count, 1);
        assert_eq!(lease.last_heartbeat_at, Some(now + 50));

        // Backward heartbeat (time travel beyond drift window) → rejected.
        let backdated = now + 50 - MAX_CLOCK_DRIFT_SECS - 10;
        assert!(matches!(
            task.heartbeat(backdated, 100),
            Err(LeaseError::ClockDrift { .. })
        ));

        // Heartbeat past expiry → Expired (server-side expiry check).
        let past_expiry = now + 50 + 100 + 1;
        assert!(matches!(
            task.heartbeat(past_expiry, 100),
            Err(LeaseError::Expired { .. })
        ));

        // Heartbeat on an expired lease → Expired.
        let mut expired = PendingTask {
            arxiv_id: "2401.00002".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 0,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: Some(TaskLease {
                owner: "w".into(),
                epoch: 1,
                acquired_at: 0,
                expires_at: 10,
                last_heartbeat_at: None,
                heartbeat_count: 0,
            }),
            dependencies: vec![],
            resource_class: TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        };
        assert!(matches!(
            expired.heartbeat(100, 100),
            Err(LeaseError::Expired { .. })
        ));
    }

    #[test]
    fn test_lease_term_bounded() {
        let lease = TaskLease::claim("w", 1000, MAX_LEASE_TERM_SECS + 1, 1);
        assert!(matches!(lease, Err(LeaseError::TermOutOfRange { .. })));
        let lease = TaskLease::claim("w", 1000, 0, 1);
        assert!(matches!(lease, Err(LeaseError::TermOutOfRange { .. })));
        let ok = TaskLease::claim("w", 1000, MAX_LEASE_TERM_SECS, 1).unwrap();
        assert_eq!(ok.expires_at - ok.acquired_at, MAX_LEASE_TERM_SECS);
    }

    #[test]
    fn test_validate_rejects_tampered_term() {
        let lease = TaskLease {
            owner: "w".into(),
            epoch: 1,
            acquired_at: 1000,
            expires_at: 1000 + MAX_LEASE_TERM_SECS + 500, // tampered term
            last_heartbeat_at: None,
            heartbeat_count: 0,
        };
        assert!(matches!(
            lease.validate(2000),
            Err(LeaseError::TermTooLong { .. })
        ));
    }

    #[test]
    fn test_validate_rejects_future_dated_claim() {
        let lease = TaskLease {
            owner: "w".into(),
            epoch: 1,
            acquired_at: 1_000_000,
            expires_at: 1_000_300,
            last_heartbeat_at: None,
            heartbeat_count: 0,
        };
        // Server clock far behind the record's acquired_at → drift.
        assert!(matches!(
            lease.validate(1_000_000 - MAX_CLOCK_DRIFT_SECS - 10),
            Err(LeaseError::ClockDrift { .. })
        ));
    }

    #[test]
    fn test_attempt_lineage_and_stale_fence() {
        let mut task = PendingTask {
            arxiv_id: "2401.00001".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 1000,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![],
            resource_class: TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        };
        let now = 10_000;
        let epoch = task.claim("worker-a", now, 100).unwrap();

        // Finish with the correct fence.
        task.finish_attempt(
            epoch,
            AttemptOutcome::Failed,
            Some("network".into()),
            true,
            vec!["sha256:abc".into()],
            now + 20,
        )
        .unwrap();
        assert_eq!(task.attempts[0].outcome, AttemptOutcome::Failed);
        assert_eq!(task.attempts[0].error_class.as_deref(), Some("network"));
        assert!(task.attempts[0].retryable);
        assert_eq!(task.attempts[0].finished_at, Some(now + 20));

        // Stale fence (older epoch after reclaim) → rejected.
        let later = now + 101;
        let epoch2 = task.claim("worker-b", later, 100).unwrap();
        assert_eq!(epoch2, 2);
        assert!(matches!(
            task.finish_attempt(
                epoch,
                AttemptOutcome::Succeeded,
                None,
                false,
                vec![],
                later + 1
            ),
            Err(AttemptError::StaleFence { .. })
        ));

        // worker-b's attempt is orphaned when worker-c reclaims after expiry;
        // the recoverer aborts stale Running attempts, history stays complete.
        let later2 = later + 101;
        let epoch3 = task.claim("worker-c", later2, 100).unwrap();
        assert_eq!(epoch3, 3);
        assert_eq!(task.attempts.len(), 3);
        task.mark_stale_attempts_aborted(later2 + 1);
        // worker-b's attempt (epoch 2, Running) → Aborted.
        assert_eq!(task.attempts[1].outcome, AttemptOutcome::Aborted);
        assert_eq!(task.attempts[1].finished_at, Some(later2 + 1));
        // worker-c's current attempt (epoch 3) stays Running.
        assert_eq!(task.attempts[2].outcome, AttemptOutcome::Running);

        // Stale epoch (2) is rejected at the fence before attempt lookup.
        assert!(matches!(
            task.finish_attempt(
                epoch2,
                AttemptOutcome::Succeeded,
                None,
                false,
                vec![],
                later2 + 2
            ),
            Err(AttemptError::StaleFence { .. })
        ));

        // Finish epoch 3's attempt; re-finishing → NoRunningAttempt.
        task.finish_attempt(
            epoch3,
            AttemptOutcome::Succeeded,
            None,
            false,
            vec![],
            later2 + 2,
        )
        .unwrap();
        assert_eq!(task.attempts[2].outcome, AttemptOutcome::Succeeded);
        assert!(matches!(
            task.finish_attempt(
                epoch3,
                AttemptOutcome::Succeeded,
                None,
                false,
                vec![],
                later2 + 3
            ),
            Err(AttemptError::NoRunningAttempt { .. })
        ));
    }

    #[test]
    fn test_publish_logical_result_idempotent_and_fenced() {
        let mut task = PendingTask {
            arxiv_id: "2401.00001".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 1000,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![],
            resource_class: TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        };
        let now = 10_000;

        // No lease → cannot publish.
        assert!(matches!(
            task.publish_logical_result(1, "done".into(), vec![], now),
            Err(PublishError::NoLease)
        ));

        let epoch = task.claim("worker-a", now, 100).unwrap();
        let id1 = task
            .publish_logical_result(epoch, "done".into(), vec!["sha256:abc".into()], now + 10)
            .unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(id1, logical_result_vid(&task.task_vid()));
        // Deterministic across tasks with the same identity.
        let other = PendingTask::new_openalex_enrich("2401.00001", &RetryPolicy::default(), now);
        assert_eq!(id1, logical_result_vid(&other.task_vid()));

        // Idempotent re-publish returns the same id.
        let id2 = task
            .publish_logical_result(epoch, "done".into(), vec![], now + 20)
            .unwrap();
        assert_eq!(id1, id2);

        // Stale worker cannot re-publish (fence check even when published).
        let later = now + 101;
        let epoch2 = task.claim("worker-b", later, 100).unwrap();
        assert_eq!(epoch2, 2);
        assert!(matches!(
            task.publish_logical_result(epoch, "stale".into(), vec![], later),
            Err(PublishError::StaleFence { .. })
        ));
    }

    #[test]
    fn test_is_ready_blocks_on_dependencies_and_resource() {
        let mut task = PendingTask {
            arxiv_id: "2401.00001".into(),
            task_type: TaskType::OpenAlexEnrich,
            added_at: 1000,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![TaskDependency {
                task_vid: "vid:scheduler-task:dep".into(),
                satisfied_at: None,
            }],
            resource_class: TaskResourceClass::Network,
            attempts: vec![],
            logical_result: None,
        };
        let available = |rc: TaskResourceClass| rc == TaskResourceClass::Network;
        assert!(!task.is_ready(available), "unsatisfied dependency blocks");

        task.dependencies[0].satisfied_at = Some(2000);
        let available = |rc: TaskResourceClass| rc == TaskResourceClass::Network;
        assert!(task.is_ready(available), "satisfied dep + resource ok");

        let blocked_resource = |rc: TaskResourceClass| rc != TaskResourceClass::Network;
        assert!(!task.is_ready(blocked_resource), "missing resource blocks");
    }

    #[test]
    fn test_claimable_only_when_due_and_unleased_or_expired() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        // Not due yet (next_retry = now + 1 day).
        assert!(!task.is_claimable(now));
        // Due + unleased → claimable.
        assert!(task.is_claimable(now + 86400));
        // Claimed → not claimable while lease valid.
        task.claim("w", now + 86400, 100).unwrap();
        assert!(!task.is_claimable(now + 86400 + 50));
        // Lease expired → claimable again.
        assert!(task.is_claimable(now + 86400 + 101));
    }

    #[test]
    fn test_serde_roundtrip_extended_task() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        task.claim("worker-a", now, 100).unwrap();
        task.heartbeat(now + 10, 100).unwrap();
        let epoch = task.current_epoch().unwrap();
        task.finish_attempt(
            epoch,
            AttemptOutcome::Succeeded,
            None,
            false,
            vec!["sha256:abc".into()],
            now + 20,
        )
        .unwrap();
        task.publish_logical_result(epoch, "done".into(), vec!["sha256:abc".into()], now + 20)
            .unwrap();

        let json = serde_json::to_string(&task).unwrap();
        let back: PendingTask = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2, "roundtrip JSON mismatch");
        assert_eq!(back.lease.as_ref().unwrap().epoch, 1);
        assert_eq!(back.attempts.len(), 1);
        assert_eq!(back.attempts[0].outcome, AttemptOutcome::Succeeded);
        assert_eq!(
            back.logical_result.as_ref().unwrap().result_id,
            task.logical_result.as_ref().unwrap().result_id
        );
    }

    #[test]
    fn test_legacy_json_reads_with_defaults() {
        // A PendingTask JSON written before the durable-plane extension must
        // still deserialize: new fields fall back to behavior-preserving
        // defaults (R092 wire stability).
        let legacy = r#"{
            "arxiv_id": "2401.00001",
            "task_type": "open_alex_enrich",
            "added_at": 1000,
            "retry_count": 0,
            "last_retry": null,
            "next_retry": 2000,
            "status": "pending"
        }"#;
        let task: PendingTask = serde_json::from_str(legacy).unwrap();
        assert_eq!(task.schema_version, 1);
        assert_eq!(
            task.priority,
            TaskPriority::Medium,
            "legacy default = Medium"
        );
        assert_eq!(task.resource_class, TaskResourceClass::Standard);
        assert!(task.lease.is_none());
        assert!(task.dependencies.is_empty());
        assert!(task.attempts.is_empty());
        assert!(task.logical_result.is_none());
        assert_eq!(task.arxiv_id, "2401.00001");
    }

    #[test]
    fn test_unknown_variants_fail_closed() {
        // Closed dispatch: unknown task types and resource classes must not
        // deserialize (Q3 items 3–4).
        assert!(serde_json::from_str::<TaskType>("\"arbitrary_stage\"").is_err());
        assert!(serde_json::from_str::<TaskResourceClass>("\"gpu\"").is_err());
    }

    #[test]
    fn test_priority_and_resource_as_str_match_wire() {
        assert_eq!(
            serde_json::to_string(&TaskPriority::High).unwrap(),
            "\"high\""
        );
        assert_eq!(TaskPriority::High.as_str(), "high");
        assert_eq!(TaskPriority::Low.as_str(), "low");
        assert_eq!(TaskPriority::Medium.as_str(), "medium");
        assert_eq!(
            serde_json::to_string(&TaskResourceClass::Network).unwrap(),
            "\"network\""
        );
        assert_eq!(TaskResourceClass::Network.as_str(), "network");
        assert_eq!(TaskResourceClass::Standard.as_str(), "standard");
        assert_eq!(TaskResourceClass::Compute.as_str(), "compute");
        assert_eq!(TaskResourceClass::Storage.as_str(), "storage");
    }

    #[test]
    fn test_find_dependency_cycle_detects_cycles() {
        // a → b → c → a
        let nodes = vec![
            DependencyNode {
                vid: "a".into(),
                depends_on: vec!["b".into()],
            },
            DependencyNode {
                vid: "b".into(),
                depends_on: vec!["c".into()],
            },
            DependencyNode {
                vid: "c".into(),
                depends_on: vec!["a".into()],
            },
        ];
        let cycle = find_dependency_cycle(&nodes).expect("cycle must be found");
        assert_eq!(cycle.first(), cycle.last(), "cycle closes on its start");
        assert!(cycle.len() >= 3);

        // Self-dependency.
        let self_dep = vec![DependencyNode {
            vid: "x".into(),
            depends_on: vec!["x".into()],
        }];
        assert!(find_dependency_cycle(&self_dep).is_some());

        // Duplicate vids → trivial cycle report.
        let dup = vec![
            DependencyNode {
                vid: "a".into(),
                depends_on: vec![],
            },
            DependencyNode {
                vid: "a".into(),
                depends_on: vec![],
            },
        ];
        assert!(find_dependency_cycle(&dup).is_some());
    }

    #[test]
    fn test_find_dependency_cycle_accepts_dag() {
        let nodes = vec![
            DependencyNode {
                vid: "a".into(),
                depends_on: vec!["b".into(), "c".into()],
            },
            DependencyNode {
                vid: "b".into(),
                depends_on: vec!["c".into()],
            },
            DependencyNode {
                vid: "c".into(),
                depends_on: vec![],
            },
            // Unknown dependency edge is ignored for cycle purposes.
            DependencyNode {
                vid: "d".into(),
                depends_on: vec!["missing".into()],
            },
        ];
        assert_eq!(find_dependency_cycle(&nodes), None);
    }

    #[test]
    fn test_validate_record_rejects_bad_input() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;

        // Valid record passes.
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        assert!(task.validate_record(now).is_ok());

        // Invalid arxiv id rejected (Q3 item 5).
        task.arxiv_id = "not-an-arxiv-id".into();
        assert!(matches!(
            task.validate_record(now),
            Err(TaskRecordError::InvalidArxivId(_))
        ));

        // Self-dependency rejected (Q3 item 4).
        let mut task2 = PendingTask::new_openalex_enrich("2401.00002", &policy, now);
        let self_vid = task2.task_vid();
        task2.dependencies.push(TaskDependency {
            task_vid: self_vid,
            satisfied_at: None,
        });
        assert!(matches!(
            task2.validate_record(now),
            Err(TaskRecordError::SelfDependency)
        ));

        // Corrupt lease rejected (tampered term).
        let mut task3 = PendingTask::new_openalex_enrich("2401.00003", &policy, now);
        task3.lease = Some(TaskLease {
            owner: "w".into(),
            epoch: 1,
            acquired_at: now,
            expires_at: now + MAX_LEASE_TERM_SECS + 100,
            last_heartbeat_at: None,
            heartbeat_count: 0,
        });
        assert!(matches!(
            task3.validate_record(now),
            Err(TaskRecordError::InvalidLease(
                LeaseError::TermTooLong { .. }
            ))
        ));
    }

    #[test]
    fn test_task_priority_from_type() {
        assert_eq!(TaskType::Ingest.priority(), TaskPriority::High);
        assert_eq!(TaskType::Reparse.priority(), TaskPriority::High);
        assert_eq!(TaskType::Extract.priority(), TaskPriority::Medium);
        assert_eq!(TaskType::OpenAlexEnrich.priority(), TaskPriority::Medium);
        assert_eq!(TaskType::EmbedStale.priority(), TaskPriority::Low);
        assert_eq!(TaskType::GraphWrite.priority(), TaskPriority::Low);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(TaskPriority::High > TaskPriority::Medium);
        assert!(TaskPriority::Medium > TaskPriority::Low);
        assert!(TaskPriority::High > TaskPriority::Low);
        assert_eq!(TaskPriority::default(), TaskPriority::Medium);
    }

    #[test]
    fn test_scheduler_task_schema_label() {
        assert_eq!(SchedulerTaskSchema.label(), "SchedulerTask");
    }

    #[test]
    fn test_scheduler_task_schema_has_d127_d134_fields() {
        let required = SchedulerTaskSchema.required_fields();
        let req_names: Vec<&str> = required.iter().map(|(n, _)| *n).collect();
        assert!(req_names.contains(&"vid"));
        assert!(req_names.contains(&"arxiv_id"));

        let optional = SchedulerTaskSchema.optional_fields();
        let opt_names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        assert!(
            opt_names.contains(&"retrieval_eligible"),
            "D134: retrieval_eligible missing"
        );
        assert!(
            opt_names.contains(&"import_eligible"),
            "D127: import_eligible missing"
        );
    }

    #[test]
    fn test_scheduler_task_schema_exposes_durable_plane() {
        // Slice verification: scheduler state exposes lease owner, heartbeat,
        // expiry, priority, dependencies, resource class, attempts, and
        // logical result identity on the operational graph surface.
        let optional = SchedulerTaskSchema.optional_fields();
        let opt_names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        for field in [
            "priority",
            "resource_class",
            "lease_owner",
            "lease_epoch",
            "lease_acquired_at",
            "lease_expires_at",
            "lease_last_heartbeat_at",
            "attempt_count",
            "logical_result_id",
        ] {
            assert!(
                opt_names.contains(&field),
                "durable plane field {field} missing from SchedulerTaskSchema"
            );
        }
    }

    #[test]
    fn test_logical_result_vid_deterministic() {
        let a = logical_result_vid("vid:scheduler-task:abc");
        let b = logical_result_vid("vid:scheduler-task:abc");
        let c = logical_result_vid("vid:scheduler-task:abd");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("vid:logical-result:"));
    }
}
