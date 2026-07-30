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

use da_domain::scheduler::{PendingTask, RetryPolicy};
use da_ports::graph_store::DirectGraphStore;

/// Scheduler that persists pending tasks in the graph (SchedulerTask nodes).
pub struct GraphScheduler {
    pub graph_store: Box<dyn DirectGraphStore>,
    pub policy: RetryPolicy,
}

impl GraphScheduler {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            graph_store,
            policy: RetryPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Add a paper to the pending queue (creates SchedulerTask node).
    pub async fn add_pending(&self, arxiv_id: &str) -> anyhow::Result<()> {
        Self::add_pending_to(&*self.graph_store, &self.policy, arxiv_id).await
    }

    /// Load all due pending tasks (next_retry <= now, status=pending).
    pub async fn load_due_tasks(&self) -> Vec<(u64, String)> {
        Self::load_due_tasks_from(&*self.graph_store).await
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
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp();

        // Skip if already pending
        if let Some(existing) = store
            .find_node_by_string_property("SchedulerTask", "arxiv_id", arxiv_id)
            .await
        {
            let status = store
                .get_node_property_string(existing, "status")
                .await
                .unwrap_or_default();
            if status == "pending" {
                tracing::info!(arxiv_id, "Task already pending — skipping");
                return Ok(());
            }
        }

        let task = PendingTask::new_openalex_enrich(arxiv_id, policy, now);
        let node = store.create_node("SchedulerTask").await?;

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
            .set_node_property_int(node, "next_retry", task.next_retry)
            .await?;
        store
            .set_node_property_int(node, "added_at", task.added_at)
            .await?;
        store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await?;
        store
            .set_node_property_bool(node, "import_eligible", false) // D127
            .await?;

        tracing::info!(arxiv_id, node_id = node, "SchedulerTask created in graph");
        Ok(())
    }

    /// Load all due pending tasks (next_retry <= now, status=pending) from a
    /// given store.
    pub async fn load_due_tasks_from(store: &dyn DirectGraphStore) -> Vec<(u64, String)> {
        let now = chrono::Utc::now().timestamp();
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
            let next_retry = store
                .get_node_property_int(node_id, "next_retry")
                .await
                .unwrap_or(0);
            if next_retry <= now {
                if let Some(arxiv_id) = store.get_node_property_string(node_id, "arxiv_id").await {
                    due.push((node_id, arxiv_id));
                }
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
        store
            .set_node_property_int(node_id, "last_retry", now)
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
                .set_node_property_int(node_id, "next_retry", next)
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
            .set_node_property_int(node_id, "last_retry", now)
            .await?;
        tracing::info!(node_id, "SchedulerTask completed");
        Ok(())
    }
}
