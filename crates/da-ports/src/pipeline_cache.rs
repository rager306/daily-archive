//! Content-addressed pipeline cache port (M371 S01, ADR-051).
//!
//! A cache hit is valid only when every effective input to a stage is unchanged:
//! source identity, dependency outputs, implementation/configuration contract,
//! prompt/model (when applicable), and output schema. Cached failures are
//! explicit and expire; they never masquerade as successful output.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

/// Explicit source identity. PDF and HTML hashes are never collapsed into one
/// ambiguous content hash, even when one format is selected for this run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub article_id: String,
    pub selected_format: SourceFormat,
    pub pdf_sha256: Option<String>,
    pub html_sha256: Option<String>,
    pub acquisition_fingerprint: String,
    pub normalization_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Pdf,
    Html,
    MetadataOnly,
}

/// Complete effective-input contract for one stage execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageCacheKey {
    pub source: SourceIdentity,
    pub stage_id: String,
    /// Ordered by dependency stage ID for deterministic serialization.
    pub dependency_output_sha256: BTreeMap<String, String>,
    pub implementation_fingerprint: String,
    pub configuration_fingerprint: String,
    pub prompt_fingerprint: Option<String>,
    pub model_fingerprint: Option<String>,
    pub output_schema_fingerprint: String,
}

impl StageCacheKey {
    /// Canonical identity of the complete effective-input contract. This is
    /// intentionally distinct from the output payload digest.
    pub fn key_sha256(&self) -> PipelineCacheResult<String> {
        self.validate()?;
        let canonical = serde_json::to_vec(self).map_err(|error| {
            PipelineCacheError::Contract(format!("cannot serialize stage cache key: {error}"))
        })?;
        Ok(hex::encode(Sha256::digest(canonical)))
    }

    pub fn validate(&self) -> PipelineCacheResult<()> {
        let required = [
            ("article_id", self.source.article_id.as_str()),
            ("stage_id", self.stage_id.as_str()),
            (
                "acquisition_fingerprint",
                self.source.acquisition_fingerprint.as_str(),
            ),
            (
                "normalization_fingerprint",
                self.source.normalization_fingerprint.as_str(),
            ),
            (
                "implementation_fingerprint",
                self.implementation_fingerprint.as_str(),
            ),
            (
                "configuration_fingerprint",
                self.configuration_fingerprint.as_str(),
            ),
            (
                "output_schema_fingerprint",
                self.output_schema_fingerprint.as_str(),
            ),
        ];
        if let Some((field, _)) = required
            .into_iter()
            .find(|(_, value)| value.trim().is_empty())
        {
            return Err(PipelineCacheError::Contract(format!(
                "{field} must not be empty"
            )));
        }
        let selected_identity_missing = match self.source.selected_format {
            SourceFormat::Pdf => self.source.pdf_sha256.is_none(),
            SourceFormat::Html => self.source.html_sha256.is_none(),
            SourceFormat::MetadataOnly => false,
        };
        if selected_identity_missing {
            return Err(PipelineCacheError::Contract(
                "selected source format requires its exact source identity".into(),
            ));
        }
        if self
            .dependency_output_sha256
            .keys()
            .any(|stage| stage.trim().is_empty())
        {
            return Err(PipelineCacheError::Contract(
                "dependency stage ID must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StageOutcome {
    Success {
        payload_sha256: String,
        payload_size: u64,
    },
    RetryableFailure {
        error_class: String,
        message: String,
        retry_after: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageManifestEntry {
    pub key: StageCacheKey,
    pub key_sha256: String,
    pub recorded_at: String,
    pub outcome: StageOutcome,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSuccess {
    pub manifest: StageManifestEntry,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLookup {
    Miss,
    Hit(CachedSuccess),
    RetryableFailure(StageManifestEntry),
}

#[derive(Debug, Error)]
pub enum PipelineCacheError {
    #[error("cache I/O error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("invalid cache manifest at {path}: {message}")]
    InvalidManifest { path: String, message: String },
    #[error("cache checksum mismatch at {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("cache contract violation: {0}")]
    Contract(String),
}

pub type PipelineCacheResult<T> = Result<T, PipelineCacheError>;

/// Driven port owned by the application boundary. Implementations live in
/// infrastructure adapters; da-application must not construct them.
pub trait PipelineCache: Send + Sync {
    fn lookup(&self, key: &StageCacheKey, now: &str) -> PipelineCacheResult<CacheLookup>;

    fn store_success(
        &self,
        key: &StageCacheKey,
        payload: &[u8],
        metadata: BTreeMap<String, String>,
        recorded_at: &str,
    ) -> PipelineCacheResult<StageManifestEntry>;

    fn store_retryable_failure(
        &self,
        key: &StageCacheKey,
        error_class: &str,
        message: &str,
        retry_after: &str,
        recorded_at: &str,
    ) -> PipelineCacheResult<StageManifestEntry>;

    fn manifest(&self, article_id: &str) -> PipelineCacheResult<Vec<StageManifestEntry>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(stage: &str) -> StageCacheKey {
        StageCacheKey {
            source: SourceIdentity {
                article_id: "2507.19457".into(),
                selected_format: SourceFormat::Pdf,
                pdf_sha256: Some("pdf-hash".into()),
                html_sha256: Some("html-hash".into()),
                acquisition_fingerprint: "catalog-v1".into(),
                normalization_fingerprint: "normalize-v2".into(),
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
    fn stage_identity_prevents_cross_stage_collision() {
        assert_ne!(key("parse"), key("summary"));
    }

    #[test]
    fn prompt_change_invalidates_only_prompt_dependent_key() {
        let parse_before = key("parse");
        let parse_after = parse_before.clone();
        assert_eq!(parse_before, parse_after);

        let mut summary_before = key("summary");
        summary_before.prompt_fingerprint = Some("prompt-v1".into());
        let mut summary_after = summary_before.clone();
        summary_after.prompt_fingerprint = Some("prompt-v2".into());
        assert_ne!(summary_before, summary_after);
    }

    #[test]
    fn dependency_output_change_invalidates_descendant() {
        let mut before = key("summary");
        before
            .dependency_output_sha256
            .insert("parse".into(), "parsed-v1".into());
        let mut after = before.clone();
        after
            .dependency_output_sha256
            .insert("parse".into(), "parsed-v2".into());
        assert_ne!(before, after);
    }

    #[test]
    fn pdf_and_html_identity_remain_distinct() {
        let pdf = key("parse");
        let mut html = pdf.clone();
        html.source.selected_format = SourceFormat::Html;
        assert_ne!(pdf, html);
        assert_eq!(pdf.source.pdf_sha256, html.source.pdf_sha256);
        assert_eq!(pdf.source.html_sha256, html.source.html_sha256);
    }

    #[test]
    fn manifest_contract_roundtrips() {
        let entry = StageManifestEntry {
            key: key("parse"),
            key_sha256: "key-hash".into(),
            recorded_at: "2026-08-08T00:00:00Z".into(),
            outcome: StageOutcome::Success {
                payload_sha256: "payload-hash".into(),
                payload_size: 42,
            },
            metadata: BTreeMap::from([("sections".into(), "85".into())]),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: StageManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, entry);
    }
}
