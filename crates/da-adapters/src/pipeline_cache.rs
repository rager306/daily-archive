//! Atomic filesystem implementation of `PipelineCache`.
//!
//! Entries live under `<root>/<article_id>/<stage_id>/<key_sha256>/`.
//! Payloads are written before manifests; the manifest is the commit marker.
//! Every read verifies the payload checksum before reporting a hit.

use chrono::{DateTime, Utc};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, PipelineCacheResult, StageCacheKey,
    StageManifestEntry, StageOutcome,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST_FILE: &str = "manifest.json";
const PAYLOAD_FILE: &str = "payload.bin";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct FilePipelineCache {
    root: PathBuf,
}

impl FilePipelineCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn safe_component(value: &str, field: &str) -> PipelineCacheResult<String> {
        if value.is_empty()
            || value == "."
            || value == ".."
            || value.contains('/')
            || value.contains('\\')
        {
            return Err(PipelineCacheError::Contract(format!(
                "unsafe {field} path component: {value:?}"
            )));
        }
        Ok(value.to_owned())
    }

    fn entry_dir(&self, key: &StageCacheKey) -> PipelineCacheResult<PathBuf> {
        let article = Self::safe_component(&key.source.article_id, "article_id")?;
        let stage = Self::safe_component(&key.stage_id, "stage_id")?;
        Ok(self.root.join(article).join(stage).join(key.key_sha256()?))
    }

    fn io(path: &Path, error: impl std::fmt::Display) -> PipelineCacheError {
        PipelineCacheError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    }

    fn write_atomic(path: &Path, bytes: &[u8]) -> PipelineCacheResult<()> {
        let parent = path.parent().ok_or_else(|| {
            PipelineCacheError::Contract(format!("path has no parent: {}", path.display()))
        })?;
        fs::create_dir_all(parent).map_err(|error| Self::io(parent, error))?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|error| Self::io(&tmp, error))?;
        file.write_all(bytes)
            .map_err(|error| Self::io(&tmp, error))?;
        file.sync_all().map_err(|error| Self::io(&tmp, error))?;
        fs::rename(&tmp, path).map_err(|error| Self::io(path, error))
    }

    fn read_manifest(path: &Path) -> PipelineCacheResult<StageManifestEntry> {
        let bytes = fs::read(path).map_err(|error| Self::io(path, error))?;
        serde_json::from_slice(&bytes).map_err(|error| PipelineCacheError::InvalidManifest {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    fn write_manifest(&self, entry: &StageManifestEntry) -> PipelineCacheResult<()> {
        let dir = self.entry_dir(&entry.key)?;
        let path = dir.join(MANIFEST_FILE);
        let json = serde_json::to_vec_pretty(entry).map_err(|error| {
            PipelineCacheError::Contract(format!("cannot serialize cache manifest: {error}"))
        })?;
        Self::write_atomic(&path, &json)
    }

    fn parse_time(value: &str, field: &str) -> PipelineCacheResult<DateTime<Utc>> {
        let parsed = DateTime::parse_from_rfc3339(value).map_err(|error| {
            PipelineCacheError::Contract(format!(
                "{field} must be a valid RFC3339 timestamp: {value:?}: {error}"
            ))
        })?;
        Ok(parsed.with_timezone(&Utc))
    }
}

impl PipelineCache for FilePipelineCache {
    fn lookup(&self, key: &StageCacheKey, now: &str) -> PipelineCacheResult<CacheLookup> {
        let now = Self::parse_time(now, "now")?;
        let dir = self.entry_dir(key)?;
        let manifest_path = dir.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Ok(CacheLookup::Miss);
        }

        let manifest = Self::read_manifest(&manifest_path)?;
        if manifest.key != *key || manifest.key_sha256 != key.key_sha256()? {
            return Err(PipelineCacheError::InvalidManifest {
                path: manifest_path.display().to_string(),
                message: "manifest key does not match requested effective inputs".into(),
            });
        }

        match &manifest.outcome {
            StageOutcome::Success {
                payload_sha256,
                payload_size,
            } => {
                let payload_path = dir.join(PAYLOAD_FILE);
                let payload =
                    fs::read(&payload_path).map_err(|error| Self::io(&payload_path, error))?;
                let actual = Self::sha256(&payload);
                if &actual != payload_sha256 || payload.len() as u64 != *payload_size {
                    return Err(PipelineCacheError::ChecksumMismatch {
                        path: payload_path.display().to_string(),
                        expected: payload_sha256.clone(),
                        actual,
                    });
                }
                Ok(CacheLookup::Hit(da_ports::pipeline_cache::CachedSuccess {
                    manifest,
                    payload,
                }))
            }
            StageOutcome::RetryableFailure { retry_after, .. }
                if now < Self::parse_time(retry_after, "retry_after")? =>
            {
                Ok(CacheLookup::RetryableFailure(manifest))
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
        Self::parse_time(recorded_at, "recorded_at")?;
        let dir = self.entry_dir(key)?;
        let payload_sha256 = Self::sha256(payload);
        Self::write_atomic(&dir.join(PAYLOAD_FILE), payload)?;
        let entry = StageManifestEntry {
            key: key.clone(),
            key_sha256: key.key_sha256()?,
            recorded_at: recorded_at.into(),
            outcome: StageOutcome::Success {
                payload_sha256,
                payload_size: payload.len() as u64,
            },
            metadata,
        };
        self.write_manifest(&entry)?;
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
        let recorded_at_time = Self::parse_time(recorded_at, "recorded_at")?;
        let retry_after_time = Self::parse_time(retry_after, "retry_after")?;
        if retry_after_time <= recorded_at_time {
            return Err(PipelineCacheError::Contract(
                "retry_after must be later than recorded_at".into(),
            ));
        }
        let entry = StageManifestEntry {
            key: key.clone(),
            key_sha256: key.key_sha256()?,
            recorded_at: recorded_at.into(),
            outcome: StageOutcome::RetryableFailure {
                error_class: error_class.into(),
                message: message.into(),
                retry_after: retry_after.into(),
            },
            metadata: BTreeMap::new(),
        };
        self.write_manifest(&entry)?;
        Ok(entry)
    }

    fn manifest(&self, article_id: &str) -> PipelineCacheResult<Vec<StageManifestEntry>> {
        let article = Self::safe_component(article_id, "article_id")?;
        let article_dir = self.root.join(article);
        if !article_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        let stages = fs::read_dir(&article_dir).map_err(|error| Self::io(&article_dir, error))?;
        for stage in stages {
            let stage = stage.map_err(|error| Self::io(&article_dir, error))?;
            if !stage.path().is_dir() {
                continue;
            }
            let keys =
                fs::read_dir(stage.path()).map_err(|error| Self::io(&stage.path(), error))?;
            for key in keys {
                let key = key.map_err(|error| Self::io(&stage.path(), error))?;
                let path = key.path().join(MANIFEST_FILE);
                if path.is_file() {
                    entries.push(Self::read_manifest(&path)?);
                }
            }
        }
        entries.sort_by(|left, right| {
            left.key
                .stage_id
                .cmp(&right.key.stage_id)
                .then(left.recorded_at.cmp(&right.recorded_at))
        });
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::pipeline_cache::{SourceFormat, SourceIdentity};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(test: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("da-pipeline-cache-{test}-{nonce}"))
    }

    fn key(stage: &str) -> StageCacheKey {
        StageCacheKey {
            source: SourceIdentity {
                article_id: "2507.19457".into(),
                selected_format: SourceFormat::Pdf,
                pdf_sha256: Some("pdf-v1".into()),
                html_sha256: None,
                acquisition_fingerprint: "catalog-v1".into(),
                normalization_fingerprint: "normalize-v1".into(),
            },
            stage_id: stage.into(),
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: "impl-v1".into(),
            configuration_fingerprint: "config-v1".into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: "schema-v1".into(),
        }
    }

    #[test]
    fn success_roundtrip_is_a_verified_hit() {
        let root = temp_root("roundtrip");
        let cache = FilePipelineCache::new(&root);
        let key = key("parse");
        cache
            .store_success(
                &key,
                br#"{"sections":85}"#,
                BTreeMap::from([("sections".into(), "85".into())]),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let lookup = cache.lookup(&key, "2026-08-08T00:01:00Z").unwrap();
        let CacheLookup::Hit(hit) = lookup else {
            panic!("expected cache hit")
        };
        assert_eq!(hit.payload, br#"{"sections":85}"#);
        assert_eq!(cache.manifest("2507.19457").unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_prompt_misses_without_invalidating_parse() {
        let root = temp_root("prompt");
        let cache = FilePipelineCache::new(&root);
        let parse = key("parse");
        let mut summary_v1 = key("summary");
        summary_v1.prompt_fingerprint = Some("prompt-v1".into());
        cache
            .store_success(&parse, b"parsed", BTreeMap::new(), "2026-08-08T00:00:00Z")
            .unwrap();
        cache
            .store_success(
                &summary_v1,
                b"summary",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let mut summary_v2 = summary_v1.clone();
        summary_v2.prompt_fingerprint = Some("prompt-v2".into());

        assert!(matches!(
            cache.lookup(&parse, "2026-08-08T00:01:00Z").unwrap(),
            CacheLookup::Hit(_)
        ));
        assert_eq!(
            cache.lookup(&summary_v2, "2026-08-08T00:01:00Z").unwrap(),
            CacheLookup::Miss
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_content_invalidates_source_dependent_stage() {
        let root = temp_root("source-change");
        let cache = FilePipelineCache::new(&root);
        let parse_v1 = key("parse");
        cache
            .store_success(
                &parse_v1,
                b"parsed-v1",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let mut parse_v2 = parse_v1.clone();
        parse_v2.source.pdf_sha256 = Some("pdf-v2".into());
        assert_eq!(
            cache.lookup(&parse_v2, "2026-08-08T00:01:00Z").unwrap(),
            CacheLookup::Miss
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupted_payload_is_never_a_hit() {
        let root = temp_root("corrupt");
        let cache = FilePipelineCache::new(&root);
        let key = key("parse");
        cache
            .store_success(&key, b"good", BTreeMap::new(), "2026-08-08T00:00:00Z")
            .unwrap();
        fs::write(cache.entry_dir(&key).unwrap().join(PAYLOAD_FILE), b"bad").unwrap();
        assert!(matches!(
            cache.lookup(&key, "2026-08-08T00:01:00Z"),
            Err(PipelineCacheError::ChecksumMismatch { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retryable_failure_expires_to_miss() {
        let root = temp_root("failure");
        let cache = FilePipelineCache::new(&root);
        let key = key("summary");
        cache
            .store_retryable_failure(
                &key,
                "timeout",
                "LLM timed out",
                "2026-08-08T01:00:00Z",
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        assert!(matches!(
            cache.lookup(&key, "2026-08-08T00:30:00Z").unwrap(),
            CacheLookup::RetryableFailure(_)
        ));
        assert_eq!(
            cache.lookup(&key, "2026-08-08T01:00:00Z").unwrap(),
            CacheLookup::Miss
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_failure_timestamp() {
        let cache = FilePipelineCache::new(temp_root("invalid-time"));
        let error = cache
            .store_retryable_failure(
                &key("summary"),
                "timeout",
                "timed out",
                "not-a-date",
                "2026-08-08T00:00:00Z",
            )
            .unwrap_err();
        assert!(matches!(error, PipelineCacheError::Contract(_)));
    }

    #[test]
    fn rejects_path_traversal_components() {
        let cache = FilePipelineCache::new(temp_root("traversal"));
        let mut unsafe_key = key("parse");
        unsafe_key.source.article_id = "../escape".into();
        assert!(matches!(
            cache.store_success(
                &unsafe_key,
                b"payload",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z"
            ),
            Err(PipelineCacheError::Contract(_))
        ));
    }
}
