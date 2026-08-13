//! PDF classification domain type (M346).
//!
//! Result of pre-GROBID PDF inspection via pdf-inspector.
//! Identifies scanned pages, garbled fonts, and vector-text pages
//! so the ingest pipeline can route appropriately.
//!
//! Product triage policy is DATA-driven (`pdf_triage_policy.yaml`, M388 S04).

use serde::{Deserialize, Serialize};

const BUNDLED_PDF_TRIAGE_POLICY: &str =
    include_str!("../../../data/ontology/pdf_triage_policy.yaml");

/// Typed pre-GROBID triage outcome for operators and assurance (M388 S04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PdfTriageStatus {
    /// No classifier injected (default CLI build without feature).
    #[default]
    NotConfigured,
    /// Classifier ran and produced PdfClassification.
    Classified,
    /// Classifier/read failed; policy fail_open continued to GROBID.
    FailedOpen,
}

/// DATA-driven PDF triage policy (bundled YAML).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTriagePolicy {
    pub schema_version: String,
    pub import_eligible: bool,
    pub fail_open: bool,
    pub record_in_ingest_result: bool,
    pub feature_gate: String,
    pub encoding_issues_block_grobid: bool,
}

impl PdfTriagePolicy {
    pub fn bundled() -> Result<Self, String> {
        Self::from_yaml(BUNDLED_PDF_TRIAGE_POLICY)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        let policy: Self = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "pdf-triage-policy.v1" {
            return Err(format!(
                "unsupported pdf triage policy schema: {}",
                self.schema_version
            ));
        }
        if self.import_eligible {
            return Err("pdf triage policy must keep import_eligible=false".into());
        }
        if self.feature_gate.trim().is_empty() {
            return Err("feature_gate must be non-empty".into());
        }
        Ok(())
    }
}

/// OCR reason identifiers (match pdf-inspector constants).
pub const OCR_REASON_SCANNED: &str = "scanned";
pub const OCR_REASON_SUSPECTED_GARBLED_TEXT: &str = "suspected_garbled_text";
pub const OCR_REASON_NO_TEXT: &str = "no_text";
pub const OCR_REASON_VECTOR_TEXT: &str = "vector_text";

/// High-level PDF type classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfType {
    /// Text-based PDF — normal digital-born document.
    Text,
    /// Scanned PDF — image-only, no text layer.
    Scanned,
    /// Image-only PDF (distinct from scanned: original images, not scans).
    Image,
    /// Mixed — some pages text, some scanned.
    Mixed,
    /// Could not classify.
    Unknown,
}

impl PdfType {
    /// Returns true if GROBID semantic parsing is likely to succeed.
    ///
    /// Text-based and mixed PDFs have extractable text on at least some
    /// pages. Fully scanned/image-only PDFs need OCR first.
    pub fn grobid_eligible(&self) -> bool {
        matches!(self, PdfType::Text | PdfType::Mixed)
    }
}

/// Per-page OCR reason for a single 1-indexed page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageOcrReasons {
    /// 1-indexed page number.
    pub page: u32,
    /// Machine-readable OCR reason identifiers.
    pub reasons: Vec<String>,
}

/// Result of PDF classification.
///
/// Produced by `PdfClassificationPort` implementations (pdf-inspector adapter).
/// Consumed by the ingest use-case to decide routing before GROBID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfClassification {
    /// Detected PDF type.
    pub pdf_type: PdfType,
    /// Total page count.
    pub page_count: u32,
    /// 1-indexed page numbers that need OCR.
    pub pages_needing_ocr: Vec<u32>,
    /// Machine-readable OCR reasons by 1-indexed page.
    pub ocr_reasons_by_page: Vec<PageOcrReasons>,
    /// Detection confidence score (0.0–1.0).
    pub confidence: f32,
    /// Soft encoding signal from pdf-inspector (M349).
    ///
    /// Can be `true` when document-level markdown shows CID/ToUnicode
    /// quirks **without** any page listed in `pages_needing_ocr`.
    /// Treat as operator warning, not a hard block: GROBID may still
    /// extract usable TEI. Hard routing uses `pages_needing_ocr` only.
    pub has_encoding_issues: bool,
}

impl PdfClassification {
    /// Returns true if no page is flagged for OCR.
    ///
    /// Soft `has_encoding_issues` alone does **not** make the PDF
    /// non-extractable (M349). Hub PDF 2507.19457 is Text / 0 OCR pages
    /// with encoding_issues=true — still GROBID-eligible.
    pub fn is_text_extractable(&self) -> bool {
        self.pages_needing_ocr.is_empty()
    }

    /// Returns true if the PDF needs OCR on all content pages.
    pub fn needs_full_ocr(&self) -> bool {
        !self.pages_needing_ocr.is_empty() && self.pages_needing_ocr.len() as u32 == self.page_count
    }

    /// Returns the OCR reason for a specific 1-indexed page, if any.
    pub fn ocr_reason_for_page(&self, page: u32) -> Option<&[String]> {
        self.ocr_reasons_by_page
            .iter()
            .find(|r| r.page == page)
            .map(|r| r.reasons.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_type_grobid_eligible() {
        assert!(PdfType::Text.grobid_eligible());
        assert!(PdfType::Mixed.grobid_eligible());
        assert!(!PdfType::Scanned.grobid_eligible());
        assert!(!PdfType::Image.grobid_eligible());
        assert!(!PdfType::Unknown.grobid_eligible());
    }

    #[test]
    fn classification_text_extractable() {
        let c = PdfClassification {
            pdf_type: PdfType::Text,
            page_count: 10,
            pages_needing_ocr: vec![],
            ocr_reasons_by_page: vec![],
            confidence: 0.95,
            has_encoding_issues: false,
        };
        assert!(c.is_text_extractable());
        assert!(!c.needs_full_ocr());
    }

    #[test]
    fn classification_needs_full_ocr() {
        let c = PdfClassification {
            pdf_type: PdfType::Scanned,
            page_count: 5,
            pages_needing_ocr: vec![1, 2, 3, 4, 5],
            ocr_reasons_by_page: vec![PageOcrReasons {
                page: 1,
                reasons: vec![OCR_REASON_SCANNED.to_string()],
            }],
            confidence: 0.98,
            has_encoding_issues: false,
        };
        assert!(!c.is_text_extractable());
        assert!(c.needs_full_ocr());
    }

    #[test]
    fn classification_mixed_pdf() {
        let c = PdfClassification {
            pdf_type: PdfType::Mixed,
            page_count: 10,
            pages_needing_ocr: vec![3, 7],
            ocr_reasons_by_page: vec![
                PageOcrReasons {
                    page: 3,
                    reasons: vec![OCR_REASON_SCANNED.to_string()],
                },
                PageOcrReasons {
                    page: 7,
                    reasons: vec![OCR_REASON_NO_TEXT.to_string()],
                },
            ],
            confidence: 0.85,
            has_encoding_issues: false,
        };
        assert!(!c.is_text_extractable());
        assert!(!c.needs_full_ocr());
        assert_eq!(
            c.ocr_reason_for_page(3),
            Some(["scanned".to_string()].as_slice())
        );
        assert_eq!(c.ocr_reason_for_page(1), None);
    }

    #[test]
    fn classification_soft_encoding_still_extractable() {
        // M349: encoding_issues without OCR pages is soft (hub PDF pattern).
        let c = PdfClassification {
            pdf_type: PdfType::Text,
            page_count: 5,
            pages_needing_ocr: vec![],
            ocr_reasons_by_page: vec![],
            confidence: 0.7,
            has_encoding_issues: true,
        };
        assert!(c.is_text_extractable());
        assert!(c.pdf_type.grobid_eligible());
    }

    // --- ADR-051 P0 roundtrip (M346) ---

    #[test]
    fn roundtrip_pdf_type() {
        let json = serde_json::to_string(&PdfType::Text).unwrap();
        let back: PdfType = serde_json::from_str(&json).unwrap();
        assert_eq!(json, "\"text\"");
        assert_eq!(back, PdfType::Text);

        let json = serde_json::to_string(&PdfType::Scanned).unwrap();
        assert_eq!(json, "\"scanned\"");
    }

    #[test]
    fn roundtrip_page_ocr_reasons() {
        let reasons = PageOcrReasons {
            page: 3,
            reasons: vec!["scanned".into(), "no_text".into()],
        };
        let json = serde_json::to_string(&reasons).unwrap();
        let back: PageOcrReasons = serde_json::from_str(&json).unwrap();
        assert_eq!(reasons, back);
    }

    #[test]
    fn roundtrip_pdf_classification() {
        let c = PdfClassification {
            pdf_type: PdfType::Mixed,
            page_count: 10,
            pages_needing_ocr: vec![3, 7],
            ocr_reasons_by_page: vec![PageOcrReasons {
                page: 3,
                reasons: vec!["scanned".into()],
            }],
            confidence: 0.85,
            has_encoding_issues: false,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: PdfClassification = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn bundled_pdf_triage_policy_loads_fail_open() {
        let p = PdfTriagePolicy::bundled().expect("bundled policy");
        assert_eq!(p.schema_version, "pdf-triage-policy.v1");
        assert!(p.fail_open);
        assert!(p.record_in_ingest_result);
        assert!(!p.import_eligible);
        assert!(!p.encoding_issues_block_grobid);
        assert_eq!(p.feature_gate, "pdf-classifier");
    }

    #[test]
    fn pdf_triage_status_default_is_not_configured() {
        assert_eq!(PdfTriageStatus::default(), PdfTriageStatus::NotConfigured);
    }
}
