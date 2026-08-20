//! Graph-based scheduler for lazy-loaded metadata (D135 + ADR-040).
//!
//! Pending tasks stored as SchedulerTask nodes in Samyama Graph (not JSONL).
//! Aligns with ADR-040 §1: "Samyama Graph sole KG+vector+persist".
//!
//! All operations are available as **associate functions** taking
//! `&dyn DirectGraphStore` so the CLI can use a single shared store
//! instance for both scheduler queries and enrich writes. The struct-based
//! API (`GraphScheduler::new(...).load_due_tasks()`) remains as a thin
//! wrapper for future Phase 3+ server-mode ownership patterns.

use da_domain::scheduler::{PendingTask, RetryPolicy, TaskType};
use da_ports::graph_store::DirectGraphStore;

use chrono::DateTime;

/// Scheduler that persists pending tasks in the graph (SchedulerTask nodes).
pub struct GraphScheduler {
    pub graph_store: Box<dyn DirectGraphStore>,
    pub policy: RetryPolicy,
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
}

impl GraphScheduler {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            graph_store,
            policy: RetryPolicy::default(),
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Add a paper to the pending queue (extract task, M410 S02).
    pub async fn add_pending_extract(&self, arxiv_id: &str) -> anyhow::Result<()> {
        let now = self.clock.now().timestamp();
        Self::add_pending_task_to(
            &*self.graph_store,
            &self.policy,
            TaskType::Extract,
            arxiv_id,
            now,
        )
        .await
    }

    /// Static form for composition roots that hold an `Arc` store (M410 S02).
    pub async fn add_pending_extract_to(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        arxiv_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        Self::add_pending_task_to(store, policy, TaskType::Extract, arxiv_id, now).await
    }

    /// Add a paper to the pending queue (ingest task, M411 S02).
    pub async fn add_pending_ingest(&self, arxiv_id: &str) -> anyhow::Result<()> {
        let now = self.clock.now().timestamp();
        Self::add_pending_task_to(
            &*self.graph_store,
            &self.policy,
            TaskType::Ingest,
            arxiv_id,
            now,
        )
        .await
    }

    /// Static form for composition roots that hold an `Arc` store (M411 S02).
    pub async fn add_pending_ingest_to(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        arxiv_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        Self::add_pending_task_to(store, policy, TaskType::Ingest, arxiv_id, now).await
    }

    /// Add a paper to the pending queue (reparse task, M412 S02).
    pub async fn add_pending_reparse(&self, arxiv_id: &str) -> anyhow::Result<()> {
        let now = self.clock.now().timestamp();
        Self::add_pending_task_to(
            &*self.graph_store,
            &self.policy,
            TaskType::Reparse,
            arxiv_id,
            now,
        )
        .await
    }

    /// Static form for composition roots that hold an `Arc` store (M412 S02).
    pub async fn add_pending_reparse_to(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        arxiv_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        Self::add_pending_task_to(store, policy, TaskType::Reparse, arxiv_id, now).await
    }

    /// Add a paper to the pending queue (for the OpenAlex enrichment task).
    pub async fn add_pending(&self, arxiv_id: &str) -> anyhow::Result<()> {
        let now = self.clock.now().timestamp();
        Self::add_pending_to(&*self.graph_store, &self.policy, arxiv_id, now).await
    }

    /// Load all due pending tasks (next_retry <= now, status=pending).
    pub async fn load_due_tasks(&self) -> Vec<(u64, String)> {
        let now = self.clock.now().timestamp();
        Self::load_due_tasks_from(&*self.graph_store, now).await
    }

    /// Record a retry attempt for a task.
    pub async fn record_retry(&self, node_id: u64, now: i64) -> anyhow::Result<()> {
        Self::record_retry_on(&*self.graph_store, &self.policy, node_id, now).await
    }

    /// Mark task as completed.
    pub async fn complete_task(&self, node_id: u64, now: i64) -> anyhow::Result<()> {
        Self::complete_task_on(&*self.graph_store, node_id, now).await
    }

    // ----------------------------------------------------------------
    // Associate functions — accept &dyn DirectGraphStore so callers
    // (CLI, future server mode) can share one store instance.
    // ----------------------------------------------------------------

    /// Add a paper to the pending queue on a given store.
    pub async fn add_pending_to(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        arxiv_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        // OpenAlex is the enrich producer; the generic path below covers all
        // task types (M410 S02).
        Self::add_pending_task_to(store, policy, TaskType::OpenAlexEnrich, arxiv_id, now).await
    }

    /// Add any task type to the pending queue on a given store.
    pub async fn add_pending_task_to(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        task_type: TaskType,
        arxiv_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let task = PendingTask::new_for(task_type, arxiv_id, policy, now);
        // PIR-008 / M398-S04: SchedulerTask DateTime properties are written as
        // canonical RFC3339 UTC strings; integer timestamps are the legacy
        // read-only wire form (read back via read_datetime_secs below).
        let added_at_wire = to_rfc3339_wire(now)?;
        let next_retry_wire = to_rfc3339_wire(task.next_retry)?;
        let task_vid = da_domain::scheduler::scheduler_task_vid(&task.task_type, arxiv_id);
        // Scheduler lifecycle is one node per (task_type, arxiv_id). Re-scheduling
        // reopens that node instead of creating ambiguous duplicates.
        let node = match store
            .find_node_by_string_property("SchedulerTask", "vid", &task_vid)
            .await
        {
            Some(existing) => {
                if store
                    .get_node_property_string(existing, "status")
                    .await
                    .as_deref()
                    == Some("pending")
                {
                    return Ok(());
                }
                existing
            }
            None => store.create_node("SchedulerTask").await?,
        };

        store
            .set_node_property_string(node, "vid", task_vid)
            .await?;
        store
            .set_node_property_string(node, "arxiv_id", arxiv_id.to_string())
            .await?;
        store
            .set_node_property_string(node, "task_type", task.task_type.as_str().to_string())
            .await?;
        store
            .set_node_property_string(node, "status", "pending".to_string())
            .await?;
        store
            .set_node_property_int(node, "retry_count", task.retry_count as i64)
            .await?;
        store
            .set_node_property_string(node, "next_retry", next_retry_wire.clone())
            .await?;
        store
            .set_node_property_string(node, "added_at", added_at_wire.clone())
            .await?;
        store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await?;
        store
            .set_node_property_bool(node, "import_eligible", false) // D127
            .await?;
        store
            .set_node_property_int(node, "schema_version", 1)
            .await?;

        tracing::info!(arxiv_id, node_id = node, "SchedulerTask opened in graph");
        Ok(())
    }

    /// Load all due pending tasks (next_retry <= now, status=pending) from a
    /// given store.
    pub async fn load_due_tasks_from(store: &dyn DirectGraphStore, now: i64) -> Vec<(u64, String)> {
        let nodes = store.get_nodes_by_label("SchedulerTask").await;

        let mut due = Vec::new();
        for node_id in nodes {
            let status = store
                .get_node_property_string(node_id, "status")
                .await
                .unwrap_or_default();
            if status != "pending" {
                continue;
            }
            let next_retry = read_datetime_secs(store, node_id, "next_retry")
                .await
                .unwrap_or(0);
            if next_retry <= now
                && let Some(arxiv_id) = store.get_node_property_string(node_id, "arxiv_id").await
            {
                due.push((node_id, arxiv_id));
            }
        }
        due
    }

    /// Record a retry attempt for a task on a given store.
    pub async fn record_retry_on(
        store: &dyn DirectGraphStore,
        policy: &RetryPolicy,
        node_id: u64,
        now: i64,
    ) -> anyhow::Result<()> {
        let retry_count: i64 = store
            .get_node_property_int(node_id, "retry_count")
            .await
            .unwrap_or(0);
        let new_count = retry_count + 1;

        store
            .set_node_property_int(node_id, "retry_count", new_count)
            .await?;
        let now_wire = to_rfc3339_wire(now)?;
        store
            .set_node_property_string(node_id, "last_retry", now_wire.clone())
            .await?;

        if policy.should_give_up(new_count as u32) {
            store
                .set_node_property_string(node_id, "status", "failed".to_string())
                .await?;
            tracing::warn!(
                node_id,
                retry_count = new_count,
                "SchedulerTask failed (max retries)"
            );
        } else {
            let next = policy.next_retry_ts(new_count as u32, now);
            store
                .set_node_property_string(node_id, "next_retry", to_rfc3339_wire(next)?)
                .await?;
            tracing::info!(
                node_id,
                retry_count = new_count,
                next_retry = next,
                "SchedulerTask retry scheduled"
            );
        }
        Ok(())
    }

    /// Mark task as completed on a given store.
    pub async fn complete_task_on(
        store: &dyn DirectGraphStore,
        node_id: u64,
        now: i64,
    ) -> anyhow::Result<()> {
        store
            .set_node_property_string(node_id, "status", "completed".to_string())
            .await?;
        store
            .set_node_property_string(node_id, "last_retry", to_rfc3339_wire(now)?)
            .await?;
        tracing::info!(node_id, "SchedulerTask completed");
        Ok(())
    }
}

/// Convert a Unix-seconds timestamp to the canonical RFC3339 UTC wire form
/// (PIR-008 / M398-S04). Errors only on out-of-range timestamps.
fn to_rfc3339_wire(secs: i64) -> anyhow::Result<String> {
    DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.to_rfc3339())
        .ok_or_else(|| anyhow::anyhow!("timestamp {secs} out of DateTime range"))
}

/// Read a DateTime-typed node property as Unix seconds. Accepts the canonical
/// RFC3339 UTC string form first; falls back to the legacy integer form
/// (bounded compatibility decoder, PIR-008) so old snapshots stay readable.
/// None for missing/null/empty/OPEN values.
async fn read_datetime_secs(store: &dyn DirectGraphStore, node_id: u64, key: &str) -> Option<i64> {
    if let Some(s) = store.get_node_property_string(node_id, key).await {
        if s.is_empty() || s == kg_ontology::temporal::OPEN {
            return None;
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
            return Some(dt.timestamp());
        }
        // Legacy integer-string form (bounded compat read).
        return s.parse::<i64>().ok();
    }
    // Legacy integer property form (bounded compat read).
    store.get_node_property_int(node_id, key).await
}
