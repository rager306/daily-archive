//! Supervised local parse-job execution over the durable [`JobLedger`] port.
//!
//! The runner depends only on application ports. Concrete parsing is supplied
//! by the composition root through [`ManagedParseBatchExecutor`].

use da_domain::ContentDigest;
use da_domain::parse_job::{AttemptFence, JobLedgerError, JobRecord, JobStatus};
use da_ports::{
    BatchCheckpoint, JobLedger, ManagedParseBatchExecutor, ParseBatchCursor, ParseBatchRunSummary,
    ParseBatchSinkError,
};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum ParseJobRunnerError {
    #[error("job ledger transition failed: {0}")]
    Ledger(#[from] JobLedgerError),
    #[error("parse batch checkpoint failed: {0}")]
    Checkpoint(#[from] ParseBatchSinkError),
    #[error("parse job was cancelled before terminal completion")]
    Cancelled,
    #[error("parse job completed without a claimed attempt")]
    MissingAttempt,
}

/// Result of one supervised attempt. The record is read after the terminal
/// transition, so callers never infer success from the in-memory summary.
#[derive(Debug, Clone)]
pub struct ParseJobRunReport {
    pub record: JobRecord,
    pub summary: ParseBatchRunSummary,
}

#[derive(Clone)]
struct FencedCheckpoint {
    ledger: Arc<dyn JobLedger>,
    job_id: String,
    fence: Arc<Mutex<AttemptFence>>,
}

#[async_trait::async_trait]
impl BatchCheckpoint for FencedCheckpoint {
    async fn save(&self, cursor: &ParseBatchCursor) -> Result<(), ParseBatchSinkError> {
        let mut fence = self.fence.lock().await;
        let record = self
            .ledger
            .checkpoint(&self.job_id, &fence, cursor.next_ordinal)
            .await
            .map_err(|error| ParseBatchSinkError(error.to_string()))?;
        let Some(next) = record.attempt else {
            return Err(ParseBatchSinkError("checkpoint lost active attempt".into()));
        };
        *fence = next;
        Ok(())
    }
}

/// Durable supervisor for one local parse job.
#[derive(Clone)]
pub struct ParseJobRunner {
    ledger: Arc<dyn JobLedger>,
    heartbeat_interval: Duration,
}

impl ParseJobRunner {
    pub fn new(ledger: Arc<dyn JobLedger>) -> Self {
        Self {
            ledger,
            heartbeat_interval: Duration::from_secs(5),
        }
    }

    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval.max(Duration::from_millis(1));
        self
    }

    pub async fn cancel(
        &self,
        job_id: &str,
        executor: &dyn ManagedParseBatchExecutor,
    ) -> Result<JobRecord, ParseJobRunnerError> {
        // Durable transition is intentionally first: local cancellation is
        // only a wake-up optimization and is never the source of authority.
        let record = self.ledger.cancel(job_id).await?;
        executor.cancel();
        tracing::info!(job_id = %job_id, status = ?record.status, "parse job cancellation committed");
        Ok(record)
    }

    pub async fn run(
        &self,
        job_id: &str,
        owner: &str,
        executor: Arc<dyn ManagedParseBatchExecutor>,
    ) -> Result<ParseJobRunReport, ParseJobRunnerError> {
        let claimed = self.ledger.claim(job_id, owner).await?;
        let initial_fence = claimed
            .attempt
            .clone()
            .ok_or(ParseJobRunnerError::MissingAttempt)?;
        let fence = Arc::new(Mutex::new(initial_fence));
        let checkpoint = Arc::new(FencedCheckpoint {
            ledger: self.ledger.clone(),
            job_id: job_id.to_string(),
            fence: fence.clone(),
        });
        let heartbeat_ledger = self.ledger.clone();
        let heartbeat_job = job_id.to_string();
        let heartbeat_fence = fence.clone();
        let heartbeat_executor = executor.clone();
        let interval = self.heartbeat_interval;
        let heartbeat = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let mut current = heartbeat_fence.lock().await;
                match heartbeat_ledger.heartbeat(&heartbeat_job, &current).await {
                    Ok(record) => {
                        if let Some(next) = record.attempt {
                            *current = next;
                        } else {
                            heartbeat_executor.cancel();
                            return;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(job_id = %heartbeat_job, error = %error, "parse job heartbeat lost authority");
                        heartbeat_executor.cancel();
                        return;
                    }
                }
            }
        });

        let summary = executor.run_managed(checkpoint).await?;
        heartbeat.abort();

        if summary.cancelled() {
            let record = self.ledger.get(job_id).await?;
            return Err(if record.status == JobStatus::CancelRequested {
                ParseJobRunnerError::Cancelled
            } else {
                ParseJobRunnerError::Ledger(JobLedgerError::StaleFence {
                    expected_revision: record.revision,
                    expected_epoch: record.attempt.as_ref().map_or(0, |attempt| attempt.epoch),
                })
            });
        }

        let final_fence = fence.lock().await.clone();
        let digest = ContentDigest::of_bytes(
            serde_json::to_string(&summary)
                .expect("ParseBatchRunSummary serialization is infallible")
                .as_bytes(),
        );
        let record = self.ledger.complete(job_id, &final_fence, digest).await?;
        tracing::info!(job_id = %job_id, cursor = summary.cursor().next_ordinal, "parse job terminal success committed");
        Ok(ParseJobRunReport { record, summary })
    }
}
