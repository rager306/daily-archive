//! Immutable identity and fenced lifecycle types for local parse jobs.
//!
//! This module is deliberately infrastructure-free.  A ledger adapter owns the
//! authority clock and must apply each transition atomically under the exact
//! revision/epoch/owner fence supplied by the caller.

use crate::assurance::ContentDigest;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    /// Idempotency key, normally produced by [`Self::identity_key`].
    pub key: ContentDigest,
    pub content: Vec<String>,
    pub backend: String,
    pub projector: String,
    pub profile: String,
    #[serde(default)]
    pub tuning: Vec<(String, String)>,
    /// Operational hints only: never used in identity or conflict checks.
    #[serde(default)]
    pub locations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobIdentity {
    pub key: ContentDigest,
    pub content: Vec<String>,
    pub backend: String,
    pub projector: String,
    pub profile: String,
}

impl JobSpec {
    pub fn validate(&self) -> Result<(), JobSpecError> {
        if self.content.is_empty() || self.content.iter().any(|v| v.is_empty()) {
            return Err(JobSpecError::EmptyContent);
        }
        for (name, value) in [
            ("backend", &self.backend),
            ("projector", &self.projector),
            ("profile", &self.profile),
        ] {
            if value.trim().is_empty() {
                return Err(JobSpecError::EmptyField(name));
            }
        }
        if self
            .tuning
            .iter()
            .any(|(k, v)| k.is_empty() || v.is_empty())
        {
            return Err(JobSpecError::EmptyTuning);
        }
        Ok(())
    }

    pub fn identity_key(&self) -> Result<ContentDigest, JobSpecError> {
        self.validate()?;
        // serde_json preserves the ordered content vector and gives a stable
        // representation of the identity tuple. Tuning and locations are
        // intentionally absent from this material.
        let material =
            serde_json::to_vec(&(&self.content, &self.backend, &self.projector, &self.profile))
                .expect("identity tuple is serializable");
        Ok(ContentDigest::of_bytes(&material))
    }

    pub fn identity(&self) -> Result<JobIdentity, JobSpecError> {
        self.validate()?;
        Ok(JobIdentity {
            key: self.key.clone(),
            content: self.content.clone(),
            backend: self.backend.clone(),
            projector: self.projector.clone(),
            profile: self.profile.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobSpecError {
    #[error("job content must contain at least one non-empty item")]
    EmptyContent,
    #[error("job spec field is empty: {0}")]
    EmptyField(&'static str),
    #[error("job tuning contains an empty key or value")]
    EmptyTuning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptFence {
    pub revision: u64,
    pub epoch: u64,
    pub owner: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub identity: JobIdentity,
    pub status: JobStatus,
    pub revision: u64,
    pub attempt: Option<AttemptFence>,
    pub cursor: u64,
    pub result_digest: Option<ContentDigest>,
    /// Sanitized diagnostic only; payloads, secrets, and absolute paths are forbidden.
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: String,
    pub revision: u64,
    pub status: JobStatus,
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobLedgerError {
    #[error("job specification is invalid: {0}")]
    InvalidSpec(#[from] JobSpecError),
    #[error("job key already exists with a different immutable specification")]
    Conflict,
    #[error("job or attempt is not found")]
    NotFound,
    #[error("another owner holds the active attempt")]
    OwnerBusy,
    #[error(
        "presented fence is stale: expected revision {expected_revision}, epoch {expected_epoch}"
    )]
    StaleFence {
        expected_revision: u64,
        expected_epoch: u64,
    },
    #[error("job is terminal and immutable")]
    Terminal,
    #[error("job cancellation has already won")]
    Cancelled,
    #[error("counter overflow")]
    CounterOverflow,
    #[error("authority clock moved backwards")]
    ClockBackwards,
    #[error("durable storage failure: {0}")]
    Storage(String),
}

impl fmt::Display for JobIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key.as_str())
    }
}

impl AttemptFence {
    pub fn assert_matches(&self, presented: &Self) -> Result<(), JobLedgerError> {
        if self == presented {
            Ok(())
        } else {
            Err(JobLedgerError::StaleFence {
                expected_revision: self.revision,
                expected_epoch: self.epoch,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn spec() -> JobSpec {
        let mut spec = JobSpec {
            key: ContentDigest::of_text("placeholder"),
            content: vec!["paper-a".into(), "paper-b".into()],
            backend: "grobid".into(),
            projector: "native-v1".into(),
            profile: "structural".into(),
            tuning: vec![("window".into(), "2".into())],
            locations: vec!["relative/source".into()],
        };
        spec.key = spec.identity_key().unwrap();
        spec
    }

    #[test]
    fn identity_excludes_operational_fields_and_tuning() {
        let mut other = spec();
        other.locations = vec!["elsewhere".into()];
        other.tuning = vec![("window".into(), "99".into())];
        assert_eq!(
            spec().identity_key().unwrap(),
            other.identity_key().unwrap()
        );
    }

    #[test]
    fn identity_binds_order_and_immutable_components() {
        let mut other = spec();
        other.content.reverse();
        assert_ne!(
            spec().identity_key().unwrap(),
            other.identity_key().unwrap()
        );
        other.content = spec().content;
        other.profile = "rich".into();
        other.key = other.identity_key().unwrap();
        assert_ne!(
            spec().identity_key().unwrap(),
            other.identity_key().unwrap()
        );
    }

    #[test]
    fn identity_is_a_sha256_digest_and_roundtrips() {
        let id = spec().identity().unwrap();
        assert_eq!(id.key.as_str().len(), 64);
        let encoded = serde_json::to_string(&id).unwrap();
        let decoded: JobIdentity = serde_json::from_str(&encoded).unwrap();
        assert_eq!(id, decoded);
        let _ = Sha256::digest(b"proof the digest crate is linked");
    }
}
