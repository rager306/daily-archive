use async_trait::async_trait;
use da_application::{ParseJobRunner, ParseJobRunnerError};
use da_domain::ContentDigest;
use da_domain::parse_job::{AttemptFence, JobEvent, JobLedgerError, JobRecord, JobSpec, JobStatus};
use da_ports::{
    BatchCheckpoint, JobLedger, ManagedParseBatchExecutor, ParseBatchCursor, ParseBatchRunSummary,
    ParseBatchSinkError,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeExecutor {
    cancelled: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl ManagedParseBatchExecutor for FakeExecutor {
    async fn run_managed(
        &self,
        _checkpoint: Arc<dyn BatchCheckpoint>,
    ) -> Result<ParseBatchRunSummary, ParseBatchSinkError> {
        Ok(ParseBatchRunSummary {
            cursor: ParseBatchCursor::new(0),
            cancelled: self.cancelled.load(std::sync::atomic::Ordering::SeqCst),
        })
    }

    fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct FakeLedger {
    record: Arc<Mutex<JobRecord>>,
}

impl FakeLedger {
    fn new() -> Self {
        let mut spec = JobSpec {
            key: ContentDigest::of_text("placeholder"),
            content: vec!["paper-a".into()],
            backend: "test".into(),
            projector: "test-v1".into(),
            profile: "structural".into(),
            tuning: vec![],
            locations: vec![],
        };
        spec.key = spec.identity_key().expect("valid fixture");
        let identity = spec.identity().expect("valid fixture");
        Self {
            record: Arc::new(Mutex::new(JobRecord {
                job_id: "job-test".into(),
                identity,
                status: JobStatus::Queued,
                revision: 0,
                attempt: None,
                cursor: 0,
                result_digest: None,
                last_error: None,
            })),
        }
    }
}

#[async_trait]
impl JobLedger for FakeLedger {
    async fn submit(&self, _spec: JobSpec) -> Result<JobRecord, JobLedgerError> {
        Ok(self.record.lock().unwrap().clone())
    }

    async fn get(&self, _job_id: &str) -> Result<JobRecord, JobLedgerError> {
        Ok(self.record.lock().unwrap().clone())
    }

    async fn spec(&self, _job_id: &str) -> Result<JobSpec, JobLedgerError> {
        let mut spec = JobSpec {
            key: ContentDigest::of_text("placeholder"),
            content: vec!["paper-a".into()],
            backend: "test".into(),
            projector: "test-v1".into(),
            profile: "structural".into(),
            tuning: vec![],
            locations: vec![],
        };
        spec.key = spec.identity_key().expect("valid fixture");
        Ok(spec)
    }

    async fn claim(&self, _job_id: &str, _owner: &str) -> Result<JobRecord, JobLedgerError> {
        Err(JobLedgerError::NotFound)
    }

    async fn heartbeat(
        &self,
        _job_id: &str,
        _fence: &AttemptFence,
    ) -> Result<JobRecord, JobLedgerError> {
        Err(JobLedgerError::NotFound)
    }

    async fn checkpoint(
        &self,
        _job_id: &str,
        _fence: &AttemptFence,
        _cursor: u64,
    ) -> Result<JobRecord, JobLedgerError> {
        Err(JobLedgerError::NotFound)
    }

    async fn complete(
        &self,
        _job_id: &str,
        _fence: &AttemptFence,
        _result_digest: ContentDigest,
    ) -> Result<JobRecord, JobLedgerError> {
        Err(JobLedgerError::NotFound)
    }

    async fn fail(
        &self,
        _job_id: &str,
        _fence: &AttemptFence,
        _sanitized_error: String,
    ) -> Result<JobRecord, JobLedgerError> {
        Err(JobLedgerError::NotFound)
    }

    async fn cancel(&self, _job_id: &str) -> Result<JobRecord, JobLedgerError> {
        let mut record = self.record.lock().unwrap();
        if !record.status.is_terminal() {
            record.revision += 1;
            record.status = JobStatus::CancelRequested;
        }
        Ok(record.clone())
    }

    async fn events(&self, _job_id: &str) -> Result<Vec<JobEvent>, JobLedgerError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn cancel_commits_ledger_before_waking_managed_workers() {
    let ledger = Arc::new(FakeLedger::new());
    let runner = ParseJobRunner::new(ledger.clone());
    let executor = FakeExecutor::default();

    let record = runner
        .cancel("job-test", &executor)
        .await
        .expect("durable cancellation");

    assert_eq!(record.status, JobStatus::CancelRequested);
    assert_eq!(
        ledger.get("job-test").await.unwrap().status,
        JobStatus::CancelRequested
    );
    assert!(executor.cancelled.load(std::sync::atomic::Ordering::SeqCst));

    let replay = runner
        .cancel("job-test", &executor)
        .await
        .expect("terminal/replay cancellation remains observable");
    assert_eq!(replay.status, JobStatus::CancelRequested);
}

#[test]
fn runner_error_keeps_cancellation_distinct_from_ledger_failures() {
    let error = ParseJobRunnerError::Cancelled;
    assert_eq!(
        error.to_string(),
        "parse job was cancelled before terminal completion"
    );
}
