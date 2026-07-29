//! Scheduler domain types — retry queue for lazy-loaded metadata.
//!
//! Tracks papers awaiting OpenAlex data with exponential backoff.
//! File-based persistence (survives process restarts).

use serde::{Deserialize, Serialize};

/// A pending enrichment task in the retry queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTask {
    pub arxiv_id: String,
    pub task_type: TaskType,
    pub added_at: i64,
    pub retry_count: u32,
    pub last_retry: Option<i64>,
    pub next_retry: i64,
    pub status: TaskStatus,
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

/// Priority level for task scheduling (ADR-037 §4.3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
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

/// Retry policy with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Initial delay in seconds (default: 1 day = 86400).
    pub initial_delay: i64,
    /// Backoff multiplier (default: 3.0 — 1d → 3d → 9d → 27d).
    pub backoff_multiplier: f64,
    /// Maximum delay between retries in seconds (default: 30 days).
    pub max_delay: i64,
    /// Maximum number of retries before giving up (default: 10).
    pub max_retries: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            initial_delay: 86400,    // 1 day
            backoff_multiplier: 3.0, // 1d → 3d → 9d → 27d
            max_delay: 30 * 86400,   // 30 days cap
            max_retries: 10,
        }
    }
}

impl RetryPolicy {
    /// Calculate next retry timestamp given current retry count.
    pub fn next_retry_ts(&self, retry_count: u32, now: i64) -> i64 {
        let delay = self.delay_for_retry(retry_count);
        now + delay
    }

    /// Get delay in seconds for a given retry attempt (0-based).
    pub fn delay_for_retry(&self, retry_count: u32) -> i64 {
        let base = self.initial_delay as f64;
        let mult = self.backoff_multiplier.powi(retry_count as i32);
        let delay = (base * mult) as i64;
        delay.min(self.max_delay)
    }

    /// Should we give up on this task?
    pub fn should_give_up(&self, retry_count: u32) -> bool {
        retry_count >= self.max_retries
    }

    /// Is this task due for retry now?
    pub fn is_due(&self, task: &PendingTask, now: i64) -> bool {
        task.status == TaskStatus::Pending && task.next_retry <= now
    }
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
        assert!(!policy.is_due(&task, now));

        // Due after 1 day
        assert!(policy.is_due(&task, now + 86400));
    }

    #[test]
    fn test_complete() {
        let policy = RetryPolicy::default();
        let now = 1_000_000;
        let mut task = PendingTask::new_openalex_enrich("2401.00001", &policy, now);
        task.complete(now);
        assert_eq!(task.status, TaskStatus::Completed);
    }
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
}
