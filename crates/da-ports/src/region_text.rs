//! Region-text extraction port (M419 S02).
//!
//! Extracts the PDF text that falls inside a page region (PDF points,
//! top-left origin), for grounding evidence that captions alone cannot
//! resolve. Implementations must be honest: a miss or a suspect extraction
//! is `None`, never fabricated text.

/// Outcome of a region-text extraction.
pub struct RegionTextOutcome {
    /// Extracted text for the region (may be empty).
    pub text: String,
    /// `true` when the extractor flags the text as untrustworthy
    /// (empty region, GID-encoded fonts, garbage/encoding checks failed).
    pub needs_ocr: bool,
}

/// Port: extract text within a bounding-box region of one PDF page.
pub trait RegionTextPort: Send + Sync {
    /// `page_index` is zero-based; `region` is `[x1, y1, x2, y2]` in PDF
    /// points with top-left origin. `None` on any failure or unreadable PDF.
    fn extract_text_in_region(
        &self,
        pdf_bytes: &[u8],
        page_index: u32,
        region: [f32; 4],
    ) -> Option<RegionTextOutcome>;
}
