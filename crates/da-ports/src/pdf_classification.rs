//! PDF classification port — pre-GROBID triage (M346).
//!
//! Identifies scanned pages, garbled fonts, and vector-text pages
//! so the ingest pipeline can route before attempting GROBID parsing.

use async_trait::async_trait;
use da_domain::PdfClassification;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClassificationError {
    #[error("Classification failed: {0}")]
    Failed(String),
    #[error("Classification unavailable: {0}")]
    Unavailable(String),
}

pub type ClassificationResult<T> = Result<T, ClassificationError>;

/// Port for PDF classification (pdf-inspector adapter implements this).
///
/// Implementations classify PDFs in ~10ms (pure Rust, no sidecar)
/// to determine whether GROBID semantic parsing will succeed.
#[async_trait]
pub trait PdfClassificationPort: Send + Sync {
    /// Classify a PDF from raw bytes.
    ///
    /// Returns `PdfClassification` with per-page OCR triage.
    /// Does NOT extract text — only classifies.
    async fn classify(&self, pdf_bytes: &[u8]) -> ClassificationResult<PdfClassification>;

    /// Returns true if the classifier is available (adapter is compiled).
    fn is_available(&self) -> bool {
        true
    }
}

/// Null implementation — classification not available.
/// Use when pdf-inspector feature is not compiled.
pub struct NullClassifier;

#[async_trait]
impl PdfClassificationPort for NullClassifier {
    async fn classify(&self, _pdf_bytes: &[u8]) -> ClassificationResult<PdfClassification> {
        Err(ClassificationError::Unavailable(
            "PDF classification not compiled (enable pdf-classifier feature)".into(),
        ))
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_classifier_unavailable() {
        let classifier = NullClassifier;
        assert!(!classifier.is_available());
        let result = classifier.classify(b"fake pdf").await;
        assert!(result.is_err());
    }
}
