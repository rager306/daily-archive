//! Decision-time feature producer for the learned policy table (ADR-074).
//!
//! The learned table can only be honest if the features it keys on are the
//! features the pipeline actually measured. Before this module the only
//! constructor of `ArticleFeatures` was a hard-coded corpus stub, so no
//! observation could ever be recorded and the table had nothing to learn from.
//!
//! This bridges the real pre-GROBID [`PdfClassification`] into the four policy
//! features. Every field is derived, never guessed: text-layer quality comes
//! from the OCR page list, and `toc_bookmarks` is left at its typed absence
//! (`0`) because the classifier does not measure bookmarks.

use da_domain::fsm_policy::ArticleFeatures;
use da_domain::pdf_classification::PdfClassification;

/// Project a real PDF classification into the policy's decision features.
///
/// `text_layer_quality` is the extractable-page share: a document with no pages
/// needing OCR is fully extractable (1.0), a fully scanned one is 0.0. The
/// classifier does not report bookmarks, so `toc_bookmarks` is 0 (typed absence,
/// not a measured zero) — the grammar_route native rule already treats
/// `toc_bookmarks >= 1` as a monograph signal, and an absent count must not
/// invent one.
pub fn features_from_classification(
    classification: &PdfClassification,
    domain: &str,
) -> ArticleFeatures {
    let page_count = classification.page_count;
    let text_layer_quality = if page_count == 0 {
        // No pages measured: do not claim a text layer either way.
        0.0
    } else {
        let needing_ocr = classification.pages_needing_ocr.len() as f32;
        let total = page_count as f32;
        (1.0 - (needing_ocr / total)).clamp(0.0, 1.0)
    };
    ArticleFeatures {
        page_count,
        toc_bookmarks: 0,
        text_layer_quality,
        domain: domain.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::pdf_classification::{PdfClassification, PdfType};

    fn classification(pages: u32, ocr_pages: Vec<u32>) -> PdfClassification {
        PdfClassification {
            pdf_type: PdfType::Text,
            page_count: pages,
            pages_needing_ocr: ocr_pages,
            ocr_reasons_by_page: Vec::new(),
            confidence: 0.9,
            has_encoding_issues: false,
        }
    }

    #[test]
    fn fully_extractable_document_has_full_text_layer() {
        let f = features_from_classification(&classification(18, vec![]), "computer_science");
        assert_eq!(f.page_count, 18);
        assert_eq!(f.text_layer_quality, 1.0);
        assert_eq!(f.toc_bookmarks, 0);
        assert_eq!(f.domain, "computer_science");
    }

    #[test]
    fn scanned_document_has_zero_text_layer() {
        let f = features_from_classification(&classification(10, (1..=10).collect()), "");
        assert_eq!(f.text_layer_quality, 0.0);
    }

    #[test]
    fn partial_scan_derives_the_extractable_share() {
        // 2 of 10 pages need OCR -> 0.8 extractable.
        let f = features_from_classification(&classification(10, vec![1, 2]), "");
        assert!((f.text_layer_quality - 0.8).abs() < 1e-6);
    }

    #[test]
    fn zero_page_document_does_not_invent_a_text_layer() {
        let f = features_from_classification(&classification(0, vec![]), "");
        assert_eq!(f.page_count, 0);
        assert_eq!(f.text_layer_quality, 0.0);
    }

    #[test]
    fn derived_features_route_through_native_rules() {
        // A scan routes to grobid_structural via the native grammar_route rule;
        // this proves the produced features are the ones the policy sees.
        use da_domain::fsm_policy::{GrammarRoute, GrammarRouteNative, grammar_route_native};
        let scan = features_from_classification(&classification(10, (1..=10).collect()), "");
        assert_eq!(
            grammar_route_native(&scan, &GrammarRouteNative::default()),
            GrammarRoute::GrobidStructural
        );
        let clean = features_from_classification(&classification(18, vec![]), "");
        assert_eq!(
            grammar_route_native(&clean, &GrammarRouteNative::default()),
            GrammarRoute::XyCut
        );
    }
}
