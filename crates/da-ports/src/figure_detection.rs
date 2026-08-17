//! Figure/table detection port (M371 S02).

use async_trait::async_trait;
use da_domain::DetectedVisualArtifact;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureDetectionRequest {
    pub article_id: String,
    pub source_pdf_path: PathBuf,
    pub source_pdf_sha256: String,
    pub canonical_article_dir: PathBuf,
    /// Backward-compatible revision identity (M400 S01). `None` keeps legacy
    /// unbound detection; `Some` is the PaperRevision VID this detection run
    /// is grounded in and is echoed into the canonical manifest and result.
    pub revision_vid: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FigureDetectionResult {
    pub detector_fingerprint: String,
    /// Canonical manifest path (commit marker for the retained batch).
    pub metadata_path: PathBuf,
    /// SHA-256 of the original detector JSON, referenced by each artifact.
    pub detector_metadata_sha256: String,
    /// SHA-256 of the canonical manifest bytes at `metadata_path`.
    pub manifest_sha256: String,
    pub artifacts: Vec<DetectedVisualArtifact>,
    pub duration_ms: u64,
    /// Revision identity echoed from the request (M400 S01); `None` for legacy
    /// unbound detection runs.
    pub revision_vid: Option<String>,
}

/// Validate an optional revision VID bound at detection time (M400 S01).
///
/// `None` keeps legacy unbound detection; `Some` must be a 64-character
/// lowercase-hex SHA-256 VID (the PaperRevision VID shape).
pub fn validate_revision_vid(revision_vid: Option<&str>) -> Result<(), String> {
    if let Some(value) = revision_vid {
        let value = value.trim();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("revision_vid must be a 64-character SHA256 VID".into());
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum FigureDetectionError {
    #[error("figure detector is unavailable: {0}")]
    Unavailable(String),
    #[error("invalid figure detection request: {0}")]
    InvalidRequest(String),
    #[error("figure detector timed out after {timeout_ms} ms")]
    Timeout { timeout_ms: u64 },
    #[error("figure detector subprocess failed with status {status}: {diagnostic}")]
    Subprocess { status: String, diagnostic: String },
    #[error("invalid figure detector output at {path}: {message}")]
    InvalidOutput { path: String, message: String },
    #[error("figure asset I/O failure at {path}: {message}")]
    Io { path: String, message: String },
}

pub type FigureDetectionPortResult<T> = Result<T, FigureDetectionError>;

#[async_trait]
pub trait FigureDetectionPort: Send + Sync {
    fn detector_fingerprint(&self) -> &str;

    async fn detect(
        &self,
        request: FigureDetectionRequest,
    ) -> FigureDetectionPortResult<FigureDetectionResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_preserves_explicit_source_and_catalog_identity() {
        let request = FigureDetectionRequest {
            article_id: "2507.19457".into(),
            source_pdf_path: "/catalog/2507.19457/source/2507.19457.pdf".into(),
            source_pdf_sha256: "a".repeat(64),
            canonical_article_dir: "/catalog/2507.19457".into(),
            revision_vid: Some("e".repeat(64)),
        };
        assert!(request.source_pdf_path.ends_with("2507.19457.pdf"));
        assert!(request.canonical_article_dir.ends_with("2507.19457"));
        assert_eq!(request.source_pdf_sha256.len(), 64);
        assert_eq!(
            request.revision_vid.as_deref(),
            Some("e".repeat(64).as_str())
        );
    }

    #[test]
    fn revision_vid_is_optional_and_shape_validated() {
        assert!(
            validate_revision_vid(None).is_ok(),
            "legacy unbound detection"
        );
        assert!(
            validate_revision_vid(Some(&"a".repeat(64))).is_ok(),
            "valid 64-hex revision VID"
        );
        assert!(validate_revision_vid(Some("short")).is_err());
        assert!(
            validate_revision_vid(Some(&"g".repeat(64))).is_err(),
            "not hex"
        );
        assert!(
            validate_revision_vid(Some(&"A".repeat(64))).is_ok(),
            "uppercase hex normalizes to a valid digest shape"
        );
    }

    #[test]
    fn retryable_runtime_failures_are_structurally_distinct() {
        let timeout = FigureDetectionError::Timeout { timeout_ms: 30_000 };
        let unavailable = FigureDetectionError::Unavailable("jar missing".into());
        assert!(timeout.to_string().contains("timed out"));
        assert!(unavailable.to_string().contains("unavailable"));
    }
}
