//! Graph-based scheduler for lazy-loaded metadata (D135 + ADR-040).
//!
//! Pending tasks stored as SchedulerTask nodes in Samyama Graph (not JSONL).
//! Aligns with ADR-040 §1: "Samyama Graph sole KG+vector+persist".

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
        let now = chrono::Utc::now().timestamp();

        // Skip if already pending
        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property("SchedulerTask", "arxiv_id", arxiv_id)
            .await
        {
            let status = self
                .graph_store
                .get_node_property_string(existing, "status")
                .await
                .unwrap_or_default();
            if status == "pending" {
                tracing::info!(arxiv_id, "Task already pending — skipping");
                return Ok(());
            }
        }

        let task = PendingTask::new_openalex_enrich(arxiv_id, &self.policy, now);
        let node = self.graph_store.create_node("SchedulerTask").await?;

        self.graph_store
            .set_node_property_string(node, "arxiv_id", arxiv_id.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "task_type", "openalex_enrich".to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "status", "pending".to_string())
            .await?;
        self.graph_store
            .set_node_property_int(node, "retry_count", task.retry_count as i64)
            .await?;
        self.graph_store
            .set_node_property_int(node, "next_retry", task.next_retry)
            .await?;
        self.graph_store
            .set_node_property_int(node, "added_at", task.added_at)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await?;

        tracing::info!(arxiv_id, node_id = node, "SchedulerTask created in graph");
        Ok(())
    }

    /// Load all due pending tasks (next_retry <= now, status=pending).
    pub async fn load_due_tasks(&self) -> Vec<(u64, String)> {
        let now = chrono::Utc::now().timestamp();
        let nodes = self.graph_store.get_nodes_by_label("SchedulerTask").await;

        let mut due = Vec::new();
        for node_id in nodes {
            let status = self
                .graph_store
                .get_node_property_string(node_id, "status")
                .await
                .unwrap_or_default();
            if status != "pending" {
                continue;
            }
            let next_retry = self
                .graph_store
                .get_node_property_int(node_id, "next_retry")
                .await
                .unwrap_or(0);
            if next_retry <= now {
                if let Some(arxiv_id) = self
                    .graph_store
                    .get_node_property_string(node_id, "arxiv_id")
                    .await
                {
                    due.push((node_id, arxiv_id));
                }
            }
        }
        due
    }

    /// Record a retry attempt for a task.
    pub async fn record_retry(&self, node_id: u64, now: i64) -> anyhow::Result<()> {
        let retry_count: i64 = self
            .graph_store
            .get_node_property_int(node_id, "retry_count")
            .await
            .unwrap_or(0);
        let new_count = retry_count + 1;

        self.graph_store
            .set_node_property_int(node_id, "retry_count", new_count)
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "last_retry", now)
            .await?;

        if self.policy.should_give_up(new_count as u32) {
            self.graph_store
                .set_node_property_string(node_id, "status", "failed".to_string())
                .await?;
            tracing::warn!(
                node_id,
                retry_count = new_count,
                "SchedulerTask failed (max retries)"
            );
        } else {
            let next = self.policy.next_retry_ts(new_count as u32, now);
            self.graph_store
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

    /// Mark task as completed.
    pub async fn complete_task(&self, node_id: u64, now: i64) -> anyhow::Result<()> {
        self.graph_store
            .set_node_property_string(node_id, "status", "completed".to_string())
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "last_retry", now)
            .await?;
        tracing::info!(node_id, "SchedulerTask completed");
        Ok(())
    }
}
