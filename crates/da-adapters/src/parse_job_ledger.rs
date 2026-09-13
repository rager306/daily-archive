//! Durable local parse-job ledger backed by Samyama PersistenceManager.
//! Operational state is isolated in its own Samyama tenant and never enters
//! the scientific graph or import path.

use async_trait::async_trait;
use da_domain::ContentDigest;
use da_domain::parse_job::{AttemptFence, JobEvent, JobLedgerError, JobRecord, JobSpec, JobStatus};
use da_ports::JobLedger;
use samyama::graph::node::Node;
use samyama::graph::property::PropertyValue;
use samyama::graph::types::{Label, NodeId};
use samyama::persistence::PersistenceManager;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

const TENANT: &str = "daily_archive_parse_jobs";
const LABEL: &str = "ParseJobLedger";
const STATE_KEY: &str = "state";
const EVENTS_KEY: &str = "events";
const LEASE_TERM_MS: i64 = 30_000;

#[derive(Clone)]
pub struct SamyamaJobLedger {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    persistence: PersistenceManager,
    jobs: HashMap<String, NodeId>,
    poisoned: Option<String>,
}

fn storage_error(error: impl std::fmt::Display) -> JobLedgerError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("lock") {
        JobLedgerError::OwnerBusy
    } else {
        JobLedgerError::Storage(message)
    }
}

impl SamyamaJobLedger {
    /// Opens the durable operational store. The PersistenceManager's RocksDB
    /// lock is the process-owner guard; a second live opener returns OwnerBusy.
    pub fn open(base_path: impl AsRef<Path>) -> Result<Self, JobLedgerError> {
        let persistence = PersistenceManager::new(base_path).map_err(storage_error)?;
        if !persistence.tenants().is_tenant_enabled(TENANT) {
            persistence
                .tenants()
                .create_tenant(
                    TENANT.to_string(),
                    "Daily Archive parse jobs".to_string(),
                    None,
                )
                .map_err(storage_error)?;
        }
        let (nodes, _) = persistence.recover(TENANT).map_err(storage_error)?;
        let mut jobs = HashMap::new();
        for node in nodes {
            if !node.labels.iter().any(|label| label.as_str() == LABEL) {
                continue;
            }
            if let Some(PropertyValue::String(value)) = node.properties.get(STATE_KEY)
                && let Ok(record) = serde_json::from_str::<JobRecord>(value)
            {
                jobs.insert(record.job_id, node.id);
            }
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                persistence,
                jobs,
                poisoned: None,
            })),
        })
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, Inner>, JobLedgerError> {
        self.inner
            .lock()
            .map_err(|_| JobLedgerError::Storage("ledger mutex poisoned".into()))
    }

    fn read_record(
        inner: &Inner,
        job_id: &str,
    ) -> Result<(NodeId, JobRecord, Vec<JobEvent>), JobLedgerError> {
        let node_id = inner
            .jobs
            .get(job_id)
            .copied()
            .ok_or(JobLedgerError::NotFound)?;
        let node = inner
            .persistence
            .storage()
            .get_node(TENANT, node_id.as_u64())
            .map_err(storage_error)?
            .ok_or(JobLedgerError::NotFound)?;
        let record = match node.properties.get(STATE_KEY) {
            Some(PropertyValue::String(value)) => serde_json::from_str(value)
                .map_err(|error| JobLedgerError::Storage(error.to_string()))?,
            _ => return Err(JobLedgerError::Storage("ledger node has no state".into())),
        };
        let events = match node.properties.get(EVENTS_KEY) {
            Some(PropertyValue::String(value)) => serde_json::from_str(value)
                .map_err(|error| JobLedgerError::Storage(error.to_string()))?,
            _ => Vec::new(),
        };
        Ok((node_id, record, events))
    }

    fn event(record: &JobRecord, name: &str) -> JobEvent {
        JobEvent {
            job_id: record.job_id.clone(),
            revision: record.revision,
            status: record.status,
            event: name.to_string(),
        }
    }

    fn assert_active(record: &JobRecord, fence: &AttemptFence) -> Result<(), JobLedgerError> {
        let current = record.attempt.as_ref().ok_or(JobLedgerError::StaleFence {
            expected_revision: record.revision,
            expected_epoch: fence.epoch,
        })?;
        current.assert_matches(fence)?;
        if record.status == JobStatus::CancelRequested {
            return Err(JobLedgerError::Cancelled);
        }
        if record.status.is_terminal() {
            return Err(JobLedgerError::Terminal);
        }
        if current.expires_at <= chrono::Utc::now().timestamp_millis() {
            return Err(JobLedgerError::StaleFence {
                expected_revision: record.revision,
                expected_epoch: current.epoch,
            });
        }
        Ok(())
    }

    fn mutate<F>(&self, transition: F) -> Result<JobRecord, JobLedgerError>
    where
        F: FnOnce(&mut Inner) -> Result<(NodeId, JobRecord, &'static str), JobLedgerError>,
    {
        let mut inner = self.locked()?;
        if let Some(reason) = inner.poisoned.clone() {
            return Err(JobLedgerError::Storage(reason));
        }
        let (node_id, record, name) = transition(&mut inner)?;
        let (_, _, mut events) = Self::read_record(&inner, &record.job_id)?;
        events.push(Self::event(&record, name));
        let mut properties = HashMap::new();
        properties.insert(
            STATE_KEY.to_string(),
            PropertyValue::String(serde_json::to_string(&record).map_err(storage_error)?),
        );
        properties.insert(
            EVENTS_KEY.to_string(),
            PropertyValue::String(serde_json::to_string(&events).map_err(storage_error)?),
        );
        let mut node = inner
            .persistence
            .storage()
            .get_node(TENANT, node_id.as_u64())
            .map_err(storage_error)?
            .ok_or(JobLedgerError::NotFound)?;
        for (key, value) in &properties {
            node.set_property(key.clone(), value.clone());
        }
        let result = inner
            .persistence
            .persist_update_node_properties_versioned(
                TENANT,
                node_id.as_u64(),
                &properties,
                record.revision,
            )
            .and_then(|_| {
                inner
                    .persistence
                    .storage()
                    .put_node(TENANT, &node)
                    .map_err(samyama::persistence::PersistenceError::from)
            })
            .and_then(|_| inner.persistence.checkpoint());
        if let Err(error) = result {
            let message = error.to_string();
            inner.poisoned = Some(message.clone());
            return Err(storage_error(message));
        }
        Ok(record)
    }
}

#[async_trait]
impl JobLedger for SamyamaJobLedger {
    async fn submit(&self, spec: JobSpec) -> Result<JobRecord, JobLedgerError> {
        let identity = spec.identity()?;
        let mut inner = self.locked()?;
        if let Some(reason) = inner.poisoned.clone() {
            return Err(JobLedgerError::Storage(reason));
        }
        for job_id in inner.jobs.keys() {
            let (_, existing, _) = Self::read_record(&inner, job_id)?;
            if existing.identity.key == identity.key {
                return if existing.identity == identity {
                    Ok(existing)
                } else {
                    Err(JobLedgerError::Conflict)
                };
            }
        }
        let job_id = format!("job-{}", identity.key.as_str());
        let record = JobRecord {
            job_id: job_id.clone(),
            identity,
            status: JobStatus::Queued,
            revision: 0,
            attempt: None,
            cursor: 0,
            result_digest: None,
            last_error: None,
        };
        let mut node = Node::new(NodeId::new(next_node_id(&inner)), Label::new(LABEL));
        node.set_property(
            STATE_KEY,
            PropertyValue::String(serde_json::to_string(&record).map_err(storage_error)?),
        );
        node.set_property(
            EVENTS_KEY,
            PropertyValue::String(
                serde_json::to_string(&[Self::event(&record, "submit")]).map_err(storage_error)?,
            ),
        );
        inner
            .persistence
            .persist_create_node(TENANT, &node)
            .and_then(|_| inner.persistence.checkpoint())
            .map_err(storage_error)?;
        inner.jobs.insert(job_id, node.id);
        Ok(record)
    }

    async fn get(&self, job_id: &str) -> Result<JobRecord, JobLedgerError> {
        let inner = self.locked()?;
        Ok(Self::read_record(&inner, job_id)?.1)
    }

    async fn claim(&self, job_id: &str, owner: &str) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            if record.status.is_terminal() || record.status == JobStatus::CancelRequested {
                return Err(JobLedgerError::Cancelled);
            }
            if let Some(attempt) = &record.attempt
                && attempt.owner != owner
                && attempt.expires_at > chrono::Utc::now().timestamp_millis()
            {
                return Err(JobLedgerError::OwnerBusy);
            }
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            let epoch = match record.attempt.as_ref() {
                Some(attempt) => attempt
                    .epoch
                    .checked_add(1)
                    .ok_or(JobLedgerError::CounterOverflow)?,
                None => 1,
            };
            let expires_at = chrono::Utc::now()
                .timestamp_millis()
                .checked_add(LEASE_TERM_MS)
                .ok_or(JobLedgerError::CounterOverflow)?;
            record.attempt = Some(AttemptFence {
                revision: record.revision,
                epoch,
                owner: owner.to_string(),
                expires_at,
            });
            record.status = JobStatus::Running;
            Ok((node_id, record, "claim"))
        })
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        fence: &AttemptFence,
    ) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            Self::assert_active(&record, fence)?;
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            let expires_at = chrono::Utc::now()
                .timestamp_millis()
                .checked_add(LEASE_TERM_MS)
                .ok_or(JobLedgerError::CounterOverflow)?;
            let attempt = record.attempt.as_mut().ok_or(JobLedgerError::NotFound)?;
            attempt.revision = record.revision;
            attempt.expires_at = expires_at;
            Ok((node_id, record, "heartbeat"))
        })
    }

    async fn checkpoint(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        cursor: u64,
    ) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            Self::assert_active(&record, fence)?;
            if cursor < record.cursor {
                return Err(JobLedgerError::StaleFence {
                    expected_revision: record.revision,
                    expected_epoch: fence.epoch,
                });
            }
            record.cursor = cursor;
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            record
                .attempt
                .as_mut()
                .ok_or(JobLedgerError::NotFound)?
                .revision = record.revision;
            Ok((node_id, record, "checkpoint"))
        })
    }

    async fn complete(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        digest: ContentDigest,
    ) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            Self::assert_active(&record, fence)?;
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            record.result_digest = Some(digest);
            record.status = JobStatus::Succeeded;
            record.attempt = None;
            Ok((node_id, record, "complete"))
        })
    }

    async fn fail(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        error: String,
    ) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            Self::assert_active(&record, fence)?;
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            record.last_error = Some(error.chars().take(512).collect());
            record.status = JobStatus::Failed;
            record.attempt = None;
            Ok((node_id, record, "fail"))
        })
    }

    async fn cancel(&self, job_id: &str) -> Result<JobRecord, JobLedgerError> {
        self.mutate(|inner| {
            let (node_id, mut record, _) = Self::read_record(inner, job_id)?;
            if record.status.is_terminal() {
                return Ok((node_id, record, "cancel_replay"));
            }
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(JobLedgerError::CounterOverflow)?;
            record.status = JobStatus::CancelRequested;
            Ok((node_id, record, "cancel"))
        })
    }

    async fn events(&self, job_id: &str) -> Result<Vec<JobEvent>, JobLedgerError> {
        let inner = self.locked()?;
        Ok(Self::read_record(&inner, job_id)?.2)
    }
}

fn next_node_id(inner: &Inner) -> u64 {
    inner
        .jobs
        .values()
        .map(|node| node.as_u64())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}
