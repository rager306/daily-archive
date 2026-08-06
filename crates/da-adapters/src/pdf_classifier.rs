//! PDF classifier adapter — pdf-inspector integration (M346).
//!
//! Pure-Rust PDF classification (~10ms) identifies scanned pages,
//! garbled fonts, and vector-text pages before GROBID parsing.
//!
//! Enable with feature `pdf-classifier`.

use async_trait::async_trait;
use da_domain::{PageOcrReasons, PdfClassification, PdfType};
use da_ports::{ClassificationError, ClassificationResult, PdfClassificationPort};

/// pdf-inspector backed PDF classifier.
///
/// Classifies PDFs in ~10ms (pure Rust, no Docker sidecar).
/// Identifies pages needing OCR with machine-readable reasons.
pub struct PdfInspectorClassifier;

impl PdfInspectorClassifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfInspectorClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PdfClassificationPort for PdfInspectorClassifier {
    async fn classify(&self, pdf_bytes: &[u8]) -> ClassificationResult<PdfClassification> {
        // pdf_inspector runs synchronously (~10ms), wrap in blocking task
        let bytes = pdf_bytes.to_vec();
        tokio::task::spawn_blocking(move || classify_sync(&bytes))
            .await
            .map_err(|e| ClassificationError::Failed(format!("join error: {e}")))?
    }
}

/// Synchronous classification via pdf_inspector.
///
/// M349: single source of truth is `process_pdf_mem`.
/// Mixing `classify_pdf_mem` pages_needing_ocr with `process_pdf_mem`
/// has_encoding_issues produced inconsistent pairs (e.g. 0 OCR pages +
/// encoding_issues=true from document-level markdown detectors).
fn classify_sync(pdf_bytes: &[u8]) -> ClassificationResult<PdfClassification> {
    let full = pdf_inspector::process_pdf_mem(pdf_bytes)
        .map_err(|e| ClassificationError::Failed(format!("pdf-inspector process error: {e}")))?;

    Ok(PdfClassification {
        pdf_type: map_pdf_type(&full.pdf_type),
        page_count: full.page_count,
        pages_needing_ocr: full.pages_needing_ocr,
        ocr_reasons_by_page: map_ocr_reasons(&full.ocr_reasons_by_page),
        confidence: full.confidence,
        has_encoding_issues: full.has_encoding_issues,
    })
}

/// Map pdf_inspector PdfType to domain PdfType.
fn map_pdf_type(t: &pdf_inspector::PdfType) -> PdfType {
    match t {
        pdf_inspector::PdfType::TextBased => PdfType::Text,
        pdf_inspector::PdfType::Scanned => PdfType::Scanned,
        pdf_inspector::PdfType::ImageBased => PdfType::Image,
        pdf_inspector::PdfType::Mixed => PdfType::Mixed,
    }
}
fn map_ocr_reasons(reasons: &[pdf_inspector::PageOcrReasons]) -> Vec<PageOcrReasons> {
    reasons
        .iter()
        .map(|r| PageOcrReasons {
            page: r.page,
            reasons: r.reasons.clone(),
        })
        .collect()
}

// Re-export OCR reason constants for convenience
#[allow(unused_imports)]
pub use da_domain::{
    OCR_REASON_NO_TEXT as REASON_NO_TEXT, OCR_REASON_SCANNED as REASON_SCANNED,
    OCR_REASON_SUSPECTED_GARBLED_TEXT as REASON_GARBLED,
    OCR_REASON_VECTOR_TEXT as REASON_VECTOR_TEXT,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_pdf_type() {
        assert_eq!(
            map_pdf_type(&pdf_inspector::PdfType::TextBased),
            PdfType::Text
        );
        assert_eq!(
            map_pdf_type(&pdf_inspector::PdfType::Scanned),
            PdfType::Scanned
        );
        assert_eq!(
            map_pdf_type(&pdf_inspector::PdfType::ImageBased),
            PdfType::Image
        );
        assert_eq!(map_pdf_type(&pdf_inspector::PdfType::Mixed), PdfType::Mixed);
    }

    #[tokio::test]
    async fn classify_real_arxiv_pdf() {
        let pdf_path = std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| format!("{d}/../../data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/2507.19457.pdf"))
            .unwrap_or_else(|_| "data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/2507.19457.pdf".to_string());
        if !std::path::Path::new(&pdf_path).exists() {
            eprintln!("skipping: test PDF not found at {pdf_path}");
            return;
        }
        let bytes = std::fs::read(pdf_path).expect("read PDF");
        let classifier = PdfInspectorClassifier::new();
        let result = classifier.classify(&bytes).await.expect("classify");

        // arxiv digital-born PDF should be text-based
        assert!(
            result.pdf_type == PdfType::Text || result.pdf_type == PdfType::Mixed,
            "expected Text or Mixed, got {:?}",
            result.pdf_type
        );
        assert!(result.page_count > 0, "page_count should be > 0");
        assert!(
            result.confidence > 0.5,
            "confidence should be > 0.5, got {}",
            result.confidence
        );
        // M349: single process_pdf_mem source — every reason page ⊆ OCR list
        // (OCR list may be a superset via gid-encoded merge).
        for r in &result.ocr_reasons_by_page {
            assert!(
                result.pages_needing_ocr.contains(&r.page),
                "reason page {} missing from pages_needing_ocr={:?}",
                r.page,
                result.pages_needing_ocr
            );
        }
        // Soft encoding alone does not block extractability.
        if result.pages_needing_ocr.is_empty() {
            assert!(result.is_text_extractable());
        }
        eprintln!(
            "pdf_type={:?} pages={} ocr_pages={:?} confidence={:.2} encoding_issues={}",
            result.pdf_type,
            result.page_count,
            result.pages_needing_ocr,
            result.confidence,
            result.has_encoding_issues
        );
    }
}
