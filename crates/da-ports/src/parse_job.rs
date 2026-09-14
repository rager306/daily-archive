//! Port contract for an atomic local parse-job ledger.
//!
//! Implementations must commit a whole transition before acknowledging it. The
//! port intentionally exposes no storage representation or filesystem path.

use async_trait::async_trait;
use da_domain::parse_job::{AttemptFence, JobEvent, JobLedgerError, JobRecord, JobSpec};

#[async_trait]
pub trait JobLedger: Send + Sync {
    /// Submit by immutable identity. Same key + same identity is idempotent;
    /// same key with another identity returns `Conflict`.
    async fn submit(&self, spec: JobSpec) -> Result<JobRecord, JobLedgerError>;

    async fn get(&self, job_id: &str) -> Result<JobRecord, JobLedgerError>;

    /// Read back the immutable source specification and operational locations.
    /// Locations are not part of identity, but are required to rehydrate a
    /// frozen local manifest after a restart.
    async fn spec(&self, job_id: &str) -> Result<JobSpec, JobLedgerError>;

    /// Claim or reclaim an expired attempt. `owner` is an opaque non-secret id.
    async fn claim(&self, job_id: &str, owner: &str) -> Result<JobRecord, JobLedgerError>;

    /// Heartbeat is fenced by the complete attempt token, not just owner.
    async fn heartbeat(
        &self,
        job_id: &str,
        fence: &AttemptFence,
    ) -> Result<JobRecord, JobLedgerError>;

    /// Persist a monotonic durable cursor under the exact attempt fence.
    async fn checkpoint(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        cursor: u64,
    ) -> Result<JobRecord, JobLedgerError>;

    /// Terminal success and failure race with cancellation as one atomic winner.
    async fn complete(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        result_digest: da_domain::ContentDigest,
    ) -> Result<JobRecord, JobLedgerError>;

    async fn fail(
        &self,
        job_id: &str,
        fence: &AttemptFence,
        sanitized_error: String,
    ) -> Result<JobRecord, JobLedgerError>;

    /// Cancellation is durable and may be requested without an active owner.
    async fn cancel(&self, job_id: &str) -> Result<JobRecord, JobLedgerError>;

    async fn events(&self, job_id: &str) -> Result<Vec<JobEvent>, JobLedgerError>;
}
