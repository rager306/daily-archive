//! Catalog-wide sweep: inventory, typed deviations, per-article metrics.
//!
//! Processes catalog articles that are NOT part of the gold corpus through
//! the verified parse → extract → embed pipeline (graph-free, like the gold
//! canary), recording per-article stage metrics, timings, and typed
//! deviations. Thresholds are DATA (`data/catalog_sweep_thresholds.yaml`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Tunable sweep thresholds — loaded from DATA, never hardcoded in logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepThresholds {
    /// Body text below this many chars is a parse-quality deviation.
    pub tiny_body_chars: usize,
    /// Sections at or below this count signal a parse-quality problem.
    pub zero_sections_max: usize,
    /// An article with more than this many pages lacking text is flagged.
    pub high_missingness_pages: u32,
    /// Extracted entities at or below this count is a low-yield deviation.
    pub zero_entities_max: usize,
    /// Entity counts above `median * multiplier` are outliers (high side).
    pub entity_outlier_multiplier: f64,
    /// Body chars per page above which an article looks suspiciously dense
    /// (possible OCR-garble inflation). Informational.
    pub dense_body_chars_per_page: usize,
}

impl Default for SweepThresholds {
    fn default() -> Self {
        Self {
            tiny_body_chars: 2_000,
            zero_sections_max: 1,
            high_missingness_pages: 3,
            zero_entities_max: 0,
            entity_outlier_multiplier: 8.0,
            dense_body_chars_per_page: 20_000,
        }
    }
}

impl SweepThresholds {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_yaml::from_slice(&bytes).map_err(|e| format!("parse thresholds: {e}"))
    }
}

/// Typed deviations detected for one article.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviationKind {
    Source(SourceDeviation),
    Triage(TriageDeviation),
    Parse(ParseDeviation),
    Extract(ExtractDeviation),
    Embed { reason: String },
    Stage { stage: String, error_class: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDeviation {
    NoPdf,
    EmptySource,
    NoSourceDir,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriageDeviation {
    NonTextPdf { pdf_type: String },
    OcrPages { pages: u32 },
    EncodingIssues,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseDeviation {
    ZeroSections,
    TinyBody { chars: usize },
    HighMissingness { pages: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractDeviation {
    LowYield { entities: usize },
    OutlierHigh { entities: usize },
}

/// Per-article observable metrics produced by the sweep stages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArticleMetrics {
    pub sections: usize,
    pub body_chars: usize,
    pub citations: usize,
    pub tables: usize,
    pub formulas: usize,
    pub missingness_pages: u32,
    pub pdf_pages: u32,
    pub entities: usize,
    pub entities_by_type: BTreeMap<String, usize>,
    pub parse_ms: u128,
    pub extract_ms: u128,
    pub embed_ms: u128,
    /// Triage classification of the source PDF (Debug form of PdfType).
    #[serde(default)]
    pub pdf_type: Option<String>,
    #[serde(default)]
    pub has_encoding_issues: bool,
}

/// Classify one article's metrics + outcome into typed deviations.
///
/// `entity_median` is the corpus-level median used for outlier detection;
/// `None` disables the high-side outlier rule (e.g., during a partial run).
pub fn classify_deviations(
    metrics: &ArticleMetrics,
    thresholds: &SweepThresholds,
    entity_median: Option<f64>,
) -> Vec<DeviationKind> {
    let mut out = Vec::new();

    // Triage-derived signals (pdf_type Debug strings from PdfClassification).
    if let Some(pdf_type) = &metrics.pdf_type
        && !matches!(pdf_type.as_str(), "TextPdf" | "TextPdfLowText" | "Unknown")
    {
        out.push(DeviationKind::Triage(TriageDeviation::NonTextPdf {
            pdf_type: pdf_type.clone(),
        }));
    }
    if metrics.missingness_pages >= thresholds.high_missingness_pages {
        out.push(DeviationKind::Triage(TriageDeviation::OcrPages {
            pages: metrics.missingness_pages,
        }));
    }
    if metrics.has_encoding_issues {
        out.push(DeviationKind::Triage(TriageDeviation::EncodingIssues));
    }

    // Parse quality.
    if metrics.sections <= thresholds.zero_sections_max {
        out.push(DeviationKind::Parse(ParseDeviation::ZeroSections));
    }
    if metrics.body_chars < thresholds.tiny_body_chars {
        out.push(DeviationKind::Parse(ParseDeviation::TinyBody {
            chars: metrics.body_chars,
        }));
    }

    // Extract quality.
    if metrics.entities <= thresholds.zero_entities_max {
        out.push(DeviationKind::Extract(ExtractDeviation::LowYield {
            entities: metrics.entities,
        }));
    } else if let Some(median) = entity_median {
        let cutoff = median * thresholds.entity_outlier_multiplier;
        if cutoff > 0.0 && metrics.entities as f64 > cutoff {
            out.push(DeviationKind::Extract(ExtractDeviation::OutlierHigh {
                entities: metrics.entities,
            }));
        }
    }

    out
}

/// One catalog article's inventory record (source-side only, pre-pipeline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub paper_id: String,
    pub category: String,
    pub pdf_path: Option<PathBuf>,
    pub html_path: Option<PathBuf>,
    pub source_sha256: Option<String>,
    pub in_gold: bool,
    pub source_deviation: Option<SourceDeviation>,
}

#[cfg(test)]
mod classifier_tests {
    use super::*;

    fn thresholds() -> SweepThresholds {
        SweepThresholds {
            tiny_body_chars: 2_000,
            zero_sections_max: 1,
            high_missingness_pages: 3,
            zero_entities_max: 0,
            entity_outlier_multiplier: 8.0,
            dense_body_chars_per_page: 20_000,
        }
    }

    fn healthy() -> ArticleMetrics {
        ArticleMetrics {
            sections: 12,
            body_chars: 40_000,
            citations: 30,
            entities: 25,
            pdf_pages: 14,
            ..Default::default()
        }
    }

    /// Healthy article with a TextPdf and no missingness yields no deviations.
    #[test]
    fn healthy_article_has_no_deviations() {
        let mut m = healthy();
        m.pdf_type = Some("TextPdf".into());
        assert!(classify_deviations(&m, &thresholds(), None).is_empty());
    }

    /// Triage: scanned/mixed pdf types are flagged; TextPdfLowText is not.
    #[test]
    fn triage_flags_non_text_pdf_and_ocr_and_encoding() {
        let mut m = healthy();
        m.pdf_type = Some("ScannedDocument".into());
        let devs = classify_deviations(&m, &thresholds(), None);
        assert!(
            devs.contains(&DeviationKind::Triage(TriageDeviation::NonTextPdf {
                pdf_type: "ScannedDocument".into()
            }))
        );

        let mut low = healthy();
        low.pdf_type = Some("TextPdfLowText".into());
        assert!(classify_deviations(&low, &thresholds(), None).is_empty());

        let mut ocr = healthy();
        ocr.missingness_pages = 5;
        assert!(
            classify_deviations(&ocr, &thresholds(), None).contains(&DeviationKind::Triage(
                TriageDeviation::OcrPages { pages: 5 }
            ))
        );

        let mut enc = healthy();
        enc.has_encoding_issues = true;
        assert!(
            classify_deviations(&enc, &thresholds(), None)
                .contains(&DeviationKind::Triage(TriageDeviation::EncodingIssues))
        );
    }

    /// Parse quality: zero sections and tiny body are distinct deviations.
    #[test]
    fn parse_quality_rules() {
        let mut m = healthy();
        m.sections = 0;
        m.body_chars = 300;
        let devs = classify_deviations(&m, &thresholds(), None);
        assert!(devs.contains(&DeviationKind::Parse(ParseDeviation::ZeroSections)));
        assert!(
            devs.contains(&DeviationKind::Parse(ParseDeviation::TinyBody {
                chars: 300
            }))
        );
    }

    /// Extract quality: low yield, and high outlier vs corpus median.
    #[test]
    fn extract_quality_low_and_outlier() {
        let mut m = healthy();
        m.entities = 0;
        assert!(classify_deviations(&m, &thresholds(), Some(25.0)).contains(
            &DeviationKind::Extract(ExtractDeviation::LowYield { entities: 0 })
        ));

        let mut big = healthy();
        big.entities = 400; // median 25 * 8.0 = 200 cutoff
        assert!(
            classify_deviations(&big, &thresholds(), Some(25.0)).contains(&DeviationKind::Extract(
                ExtractDeviation::OutlierHigh { entities: 400 }
            ))
        );
        // Below cutoff -> clean.
        let mut ok = healthy();
        ok.entities = 150;
        assert!(
            !classify_deviations(&ok, &thresholds(), Some(25.0))
                .iter()
                .any(|d| matches!(d, DeviationKind::Extract(_)))
        );
    }
}
