use async_trait::async_trait;
use da_domain::ContentDigest;
use da_domain::parse_job::{AttemptFence, JobEvent, JobLedgerError, JobRecord, JobSpec, JobStatus};
use da_ports::JobLedger;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Durable per-job state: the record, frozen spec, and event stream, keyed by job identity.
type JobState = (JobRecord, JobSpec, Vec<JobEvent>);
type LedgerStore = Arc<Mutex<HashMap<String, JobState>>>;

#[derive(Default, Clone)]
pub struct FakeLedger {
    inner: LedgerStore,
    next: Arc<Mutex<u64>>,
}

#[async_trait]
impl JobLedger for FakeLedger {
    async fn submit(&self, spec: JobSpec) -> Result<JobRecord, JobLedgerError> {
        let identity = spec.identity()?;
        let mut jobs = self.inner.lock().unwrap();
        if let Some((record, _, _)) = jobs.get(identity.key.as_str()) {
            if record.identity == identity {
                return Ok(record.clone());
            }
            return Err(JobLedgerError::Conflict);
        }
        let mut next = self.next.lock().unwrap();
        *next += 1;
        let record = JobRecord {
            job_id: format!("job-{next}"),
            identity: identity.clone(),
            status: JobStatus::Queued,
            revision: 0,
            attempt: None,
            cursor: 0,
            result_digest: None,
            last_error: None,
        };
        jobs.insert(
            identity.key.as_str().to_owned(),
            (record.clone(), spec, vec![]),
        );
        Ok(record)
    }
    async fn get(&self, job_id: &str) -> Result<JobRecord, JobLedgerError> {
        self.find(job_id).map(|x| x.0)
    }
    async fn spec(&self, job_id: &str) -> Result<JobSpec, JobLedgerError> {
        self.find(job_id).map(|x| x.1)
    }
    async fn claim(&self, job_id: &str, owner: &str) -> Result<JobRecord, JobLedgerError> {
        let mut jobs = self.inner.lock().unwrap();
        let (r, _, events) = find_mut(&mut jobs, job_id)?;
        if r.status.is_terminal() || r.status == JobStatus::CancelRequested {
            return Err(JobLedgerError::Cancelled);
        }
        if r.attempt.as_ref().is_some_and(|a| a.owner != owner) {
            return Err(JobLedgerError::OwnerBusy);
        }
        r.revision += 1;
        let epoch = r.attempt.as_ref().map_or(1, |a| a.epoch + 1);
        r.attempt = Some(AttemptFence {
            revision: r.revision,
            epoch,
            owner: owner.into(),
            expires_at: i64::MAX,
        });
        r.status = JobStatus::Running;
        events.push(JobEvent {
            job_id: r.job_id.clone(),
            revision: r.revision,
            status: r.status,
            event: "claim".into(),
        });
        Ok(r.clone())
    }
    async fn heartbeat(
        &self,
        job_id: &str,
        fence: &AttemptFence,
    ) -> Result<JobRecord, JobLedgerError> {
        self.fenced(job_id, fence, |r| {
            r.revision += 1;
            Ok(())
        })
    }
    async fn checkpoint(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        cursor: u64,
    ) -> Result<JobRecord, JobLedgerError> {
        self.fenced(job_id, fence, |r| {
            if cursor < r.cursor {
                return Err(JobLedgerError::StaleFence {
                    expected_revision: r.revision,
                    expected_epoch: fence.epoch,
                });
            }
            r.cursor = cursor;
            r.revision += 1;
            Ok(())
        })
    }
    async fn complete(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        digest: ContentDigest,
    ) -> Result<JobRecord, JobLedgerError> {
        self.fenced(job_id, fence, |r| {
            r.revision += 1;
            r.result_digest = Some(digest);
            r.status = JobStatus::Succeeded;
            r.attempt = None;
            Ok(())
        })
    }
    async fn fail(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        error: String,
    ) -> Result<JobRecord, JobLedgerError> {
        self.fenced(job_id, fence, |r| {
            r.revision += 1;
            r.last_error = Some(error);
            r.status = JobStatus::Failed;
            r.attempt = None;
            Ok(())
        })
    }
    async fn cancel(&self, job_id: &str) -> Result<JobRecord, JobLedgerError> {
        let mut jobs = self.inner.lock().unwrap();
        let (r, _, events) = find_mut(&mut jobs, job_id)?;
        if r.status.is_terminal() {
            return Ok(r.clone());
        }
        r.revision += 1;
        r.status = JobStatus::CancelRequested;
        events.push(JobEvent {
            job_id: r.job_id.clone(),
            revision: r.revision,
            status: r.status,
            event: "cancel".into(),
        });
        Ok(r.clone())
    }
    async fn events(&self, job_id: &str) -> Result<Vec<JobEvent>, JobLedgerError> {
        Ok(self.find(job_id)?.2)
    }
}

impl FakeLedger {
    fn find(&self, id: &str) -> Result<(JobRecord, JobSpec, Vec<JobEvent>), JobLedgerError> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .find(|(r, _, _)| r.job_id == id)
            .cloned()
            .ok_or(JobLedgerError::NotFound)
    }
    fn fenced<F>(&self, id: &str, fence: &AttemptFence, f: F) -> Result<JobRecord, JobLedgerError>
    where
        F: FnOnce(&mut JobRecord) -> Result<(), JobLedgerError>,
    {
        let mut jobs = self.inner.lock().unwrap();
        let (r, _, events) = find_mut(&mut jobs, id)?;
        let current = r
            .attempt
            .as_ref()
            .ok_or(JobLedgerError::StaleFence {
                expected_revision: r.revision,
                expected_epoch: fence.epoch,
            })?
            .clone();
        current.assert_matches(fence)?;
        if r.status == JobStatus::CancelRequested {
            return Err(JobLedgerError::Cancelled);
        }
        if r.status.is_terminal() {
            return Err(JobLedgerError::Terminal);
        }
        f(r)?;
        events.push(JobEvent {
            job_id: r.job_id.clone(),
            revision: r.revision,
            status: r.status,
            event: "mutation".into(),
        });
        Ok(r.clone())
    }
}
fn find_mut<'a>(
    jobs: &'a mut HashMap<String, (JobRecord, JobSpec, Vec<JobEvent>)>,
    id: &str,
) -> Result<(&'a mut JobRecord, &'a mut JobSpec, &'a mut Vec<JobEvent>), JobLedgerError> {
    jobs.values_mut()
        .find(|(r, _, _)| r.job_id == id)
        .map(|(r, s, e)| (r, s, e))
        .ok_or(JobLedgerError::NotFound)
}

pub async fn lifecycle_contract<L: JobLedger>(ledger: &L, spec: JobSpec) {
    let first = ledger.submit(spec.clone()).await.unwrap();
    assert_eq!(first, ledger.submit(spec.clone()).await.unwrap());
    let mut conflict = spec.clone();
    conflict.profile = "rich".into();
    assert_eq!(
        ledger.submit(conflict).await.unwrap_err(),
        JobLedgerError::Conflict
    );
    let claimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    assert_eq!(
        ledger.claim(&first.job_id, "worker-b").await.unwrap_err(),
        JobLedgerError::OwnerBusy
    );
    let stale = AttemptFence {
        epoch: claimed.attempt.as_ref().unwrap().epoch - 1,
        ..claimed.attempt.clone().unwrap()
    };
    assert!(matches!(
        ledger.heartbeat(&first.job_id, &stale).await,
        Err(JobLedgerError::StaleFence { .. })
    ));
    let cancelled = ledger.cancel(&first.job_id).await.unwrap();
    assert_eq!(cancelled.status, JobStatus::CancelRequested);
    assert!(matches!(
        ledger
            .complete(
                &first.job_id,
                &claimed.attempt.unwrap(),
                ContentDigest::of_text("result")
            )
            .await,
        Err(JobLedgerError::Cancelled
            | JobLedgerError::Terminal
            | JobLedgerError::StaleFence { .. })
    ));
    assert!(ledger.events(&first.job_id).await.unwrap().len() >= 2);
}
