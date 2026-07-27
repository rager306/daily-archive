//! Scheduler use case — processes pending lazy-load tasks with retry.
//!
//! File-based pending queue (survives process restarts).
//! Integrates with EnrichUseCase for OpenAlex retries.

use da_domain::scheduler::{PendingTask, RetryPolicy, TaskStatus};
use std::path::{Path, PathBuf};

/// Result of a scheduler run.
#[derive(Debug, Clone)]
pub struct SchedulerRunResult {
    pub total_pending: usize,
    pub due_now: usize,
    pub completed: usize,
    pub still_pending: usize,
    pub failed: usize,
    pub details: Vec<TaskResult>,
}

/// Result of processing one task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub arxiv_id: String,
    pub status: String,
    pub message: String,
}

/// File-based scheduler that manages pending OpenAlex enrichment tasks.
pub struct FileScheduler {
    queue_path: PathBuf,
    policy: RetryPolicy,
}

impl FileScheduler {
    pub fn new(queue_dir: &Path) -> Self {
        std::fs::create_dir_all(queue_dir).ok();
        Self {
            queue_path: queue_dir.join("openalex_pending.jsonl"),
            policy: RetryPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Add a paper to the pending queue (lazy load registration).
    pub fn add_pending(&self, arxiv_id: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp();
        let task = PendingTask::new_openalex_enrich(arxiv_id, &self.policy, now);

        let line = serde_json::to_string(&task)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.queue_path)?
            .write_all(line.as_bytes())?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.queue_path)?
            .write_all(b"\n")?;

        tracing::info!(
            arxiv_id,
            next_retry = task.next_retry,
            "Task added to pending queue"
        );
        Ok(())
    }

    /// Load all tasks from the queue file.
    pub fn load_queue(&self) -> Vec<PendingTask> {
        if !self.queue_path.exists() {
            return Vec::new();
        }
        let content = std::fs::read_to_string(&self.queue_path).unwrap_or_default();
        content
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str::<PendingTask>(l).ok())
            .collect()
    }

    /// Save the full queue back to file.
    pub fn save_queue(&self, tasks: &[PendingTask]) -> anyhow::Result<()> {
        let content: String = tasks
            .iter()
            .map(|t| serde_json::to_string(t).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&self.queue_path, content + "\n")?;
        Ok(())
    }

    /// Process all due tasks. Returns summary of what happened.
    pub async fn run<F, Fut>(&self, process_fn: F) -> anyhow::Result<SchedulerRunResult>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        let now = chrono::Utc::now().timestamp();
        let mut tasks = self.load_queue();
        let total_pending = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count();

        let due: Vec<usize> = tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| self.policy.is_due(t, now))
            .map(|(i, _)| i)
            .collect();

        let due_count = due.len();
        let mut completed = 0;
        let mut failed = 0;
        let mut details = Vec::new();

        for idx in due {
            let task = &mut tasks[idx];
            match process_fn(task.arxiv_id.clone()).await {
                Ok(msg) => {
                    task.complete(now);
                    completed += 1;
                    details.push(TaskResult {
                        arxiv_id: task.arxiv_id.clone(),
                        status: "completed".to_string(),
                        message: msg,
                    });
                }
                Err(msg) => {
                    task.record_retry(&self.policy, now);
                    if task.status == TaskStatus::Failed {
                        failed += 1;
                    }
                    details.push(TaskResult {
                        arxiv_id: task.arxiv_id.clone(),
                        status: if task.status == TaskStatus::Failed {
                            "failed".to_string()
                        } else {
                            "still_pending".to_string()
                        },
                        message: msg,
                    });
                }
            }
        }

        self.save_queue(&tasks)?;

        let still_pending = total_pending
            .saturating_sub(completed)
            .saturating_sub(failed);

        Ok(SchedulerRunResult {
            total_pending,
            due_now: due_count,
            completed,
            still_pending,
            failed,
            details,
        })
    }
}

use std::io::Write;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_add_and_load() {
        let dir = tempdir().unwrap();
        let sched = FileScheduler::new(dir.path());
        sched.add_pending("2401.00001").unwrap();

        let queue = sched.load_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].arxiv_id, "2401.00001");
        assert_eq!(queue[0].status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_run_completes_successful_task() {
        let dir = tempdir().unwrap();
        let sched = FileScheduler::new(dir.path());
        sched.add_pending("2401.00001").unwrap();

        // Make task immediately due by manipulating next_retry
        let mut tasks = sched.load_queue();
        tasks[0].next_retry = 0; // due immediately
        sched.save_queue(&tasks).unwrap();

        let result = sched
            .run(|arxiv_id: String| async move {
                let id = arxiv_id.to_string();
                Ok(format!("enriched {id}"))
            })
            .await
            .unwrap();

        assert_eq!(result.completed, 1);
        assert_eq!(result.total_pending, 1);
        assert_eq!(result.details.len(), 1);
        assert_eq!(result.details[0].status, "completed");
    }

    #[tokio::test]
    async fn test_run_records_retry_on_failure() {
        let dir = tempdir().unwrap();
        let sched = FileScheduler::new(dir.path());
        sched.add_pending("2401.00001").unwrap();

        let mut tasks = sched.load_queue();
        tasks[0].next_retry = 0;
        sched.save_queue(&tasks).unwrap();

        let result = sched
            .run(|_| async { Err("still not in OpenAlex".to_string()) })
            .await
            .unwrap();

        assert_eq!(result.completed, 0);
        assert_eq!(result.still_pending, 1);
        assert_eq!(result.details[0].status, "still_pending");

        // Verify retry count incremented
        let queue = sched.load_queue();
        assert_eq!(queue[0].retry_count, 1);
    }

    #[tokio::test]
    async fn test_run_skips_not_due_tasks() {
        let dir = tempdir().unwrap();
        let sched = FileScheduler::new(dir.path());
        sched.add_pending("2401.00001").unwrap();

        // Task not due (next_retry is in the future)
        let result = sched
            .run(|_| async { Ok("should not reach".to_string()) })
            .await
            .unwrap();

        assert_eq!(result.due_now, 0);
        assert_eq!(result.completed, 0);
    }

    #[tokio::test]
    async fn test_empty_queue() {
        let dir = tempdir().unwrap();
        let sched = FileScheduler::new(dir.path());

        let result = sched.run(|_| async { Ok("ok".to_string()) }).await.unwrap();

        assert_eq!(result.total_pending, 0);
        assert_eq!(result.completed, 0);
    }
}
