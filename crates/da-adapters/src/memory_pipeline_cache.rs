//! Deterministic in-memory `PipelineCache` for contracts and bounded ephemeral runs.

use std::{collections::BTreeMap, sync::Mutex};

use chrono::{DateTime, Utc};
use da_ports::pipeline_cache::{
    CacheLookup, CachedSuccess, PipelineCache, PipelineCacheError, PipelineCacheResult,
    StageCacheKey, StageManifestEntry, StageOutcome,
};
use sha2::{Digest, Sha256};

type MemoryEntry = (StageManifestEntry, Option<Vec<u8>>);
type MemoryEntries = BTreeMap<String, MemoryEntry>;

#[derive(Default)]
pub struct MemoryPipelineCache {
    entries: Mutex<MemoryEntries>,
}

impl MemoryPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }
    fn parse(value: &str, field: &str) -> PipelineCacheResult<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(value)
            .map(|x| x.with_timezone(&Utc))
            .map_err(|e| PipelineCacheError::Contract(format!("{field} must be RFC3339: {e}")))
    }
    fn payload_sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }
}

impl PipelineCache for MemoryPipelineCache {
    fn lookup(&self, key: &StageCacheKey, now: &str) -> PipelineCacheResult<CacheLookup> {
        let now = Self::parse(now, "now")?;
        let id = key.key_sha256()?;
        let entries = self
            .entries
            .lock()
            .map_err(|_| PipelineCacheError::Contract("memory cache lock poisoned".into()))?;
        let Some((manifest, payload)) = entries.get(&id) else {
            return Ok(CacheLookup::Miss);
        };
        if manifest.key != *key || manifest.key_sha256 != id {
            return Err(PipelineCacheError::InvalidManifest {
                path: "memory".into(),
                message: "key identity mismatch".into(),
            });
        }
        match &manifest.outcome {
            StageOutcome::Success {
                payload_sha256,
                payload_size,
            } => {
                let payload =
                    payload
                        .clone()
                        .ok_or_else(|| PipelineCacheError::InvalidManifest {
                            path: "memory".into(),
                            message: "success payload missing".into(),
                        })?;
                let actual = Self::payload_sha(&payload);
                if actual != *payload_sha256 || payload.len() as u64 != *payload_size {
                    return Err(PipelineCacheError::ChecksumMismatch {
                        path: "memory".into(),
                        expected: payload_sha256.clone(),
                        actual,
                    });
                }
                Ok(CacheLookup::Hit(CachedSuccess {
                    manifest: manifest.clone(),
                    payload,
                }))
            }
            StageOutcome::RetryableFailure { retry_after, .. }
                if now < Self::parse(retry_after, "retry_after")? =>
            {
                Ok(CacheLookup::RetryableFailure(manifest.clone()))
            }
            StageOutcome::RetryableFailure { .. } => Ok(CacheLookup::Miss),
        }
    }
    fn store_success(
        &self,
        key: &StageCacheKey,
        payload: &[u8],
        metadata: BTreeMap<String, String>,
        recorded_at: &str,
    ) -> PipelineCacheResult<StageManifestEntry> {
        Self::parse(recorded_at, "recorded_at")?;
        let id = key.key_sha256()?;
        let entry = StageManifestEntry {
            key: key.clone(),
            key_sha256: id.clone(),
            recorded_at: recorded_at.into(),
            outcome: StageOutcome::Success {
                payload_sha256: Self::payload_sha(payload),
                payload_size: payload.len() as u64,
            },
            metadata,
        };
        self.entries
            .lock()
            .map_err(|_| PipelineCacheError::Contract("memory cache lock poisoned".into()))?
            .insert(id, (entry.clone(), Some(payload.to_vec())));
        Ok(entry)
    }
    fn store_retryable_failure(
        &self,
        key: &StageCacheKey,
        error_class: &str,
        message: &str,
        retry_after: &str,
        recorded_at: &str,
    ) -> PipelineCacheResult<StageManifestEntry> {
        let recorded = Self::parse(recorded_at, "recorded_at")?;
        let retry = Self::parse(retry_after, "retry_after")?;
        if retry <= recorded {
            return Err(PipelineCacheError::Contract(
                "retry_after must be later than recorded_at".into(),
            ));
        }
        let id = key.key_sha256()?;
        let entry = StageManifestEntry {
            key: key.clone(),
            key_sha256: id.clone(),
            recorded_at: recorded_at.into(),
            outcome: StageOutcome::RetryableFailure {
                error_class: error_class.into(),
                message: message.into(),
                retry_after: retry_after.into(),
            },
            metadata: BTreeMap::new(),
        };
        self.entries
            .lock()
            .map_err(|_| PipelineCacheError::Contract("memory cache lock poisoned".into()))?
            .insert(id, (entry.clone(), None));
        Ok(entry)
    }
    fn manifest(&self, article_id: &str) -> PipelineCacheResult<Vec<StageManifestEntry>> {
        let mut out = self
            .entries
            .lock()
            .map_err(|_| PipelineCacheError::Contract("memory cache lock poisoned".into()))?
            .values()
            .filter(|(m, _)| m.key.source.article_id == article_id)
            .map(|(m, _)| m.clone())
            .collect::<Vec<_>>();
        out.sort_by(|a, b| {
            a.key
                .stage_id
                .cmp(&b.key.stage_id)
                .then(a.recorded_at.cmp(&b.recorded_at))
        });
        Ok(out)
    }
}
