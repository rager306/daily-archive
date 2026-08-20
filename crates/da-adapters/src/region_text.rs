//! pdf-inspector adapter for the RegionTextPort (M419 S02, feature-gated).

use da_ports::region_text::{RegionTextOutcome, RegionTextPort};

/// Thin wrapper over `pdf_inspector::extract_text_in_regions_mem`.
/// A miss or an unreadable PDF is `None`; `needs_ocr` propagates so the
/// caller can refuse suspect text.
pub struct PdfInspectorRegionText;

impl PdfInspectorRegionText {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfInspectorRegionText {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionTextPort for PdfInspectorRegionText {
    fn extract_text_in_region(
        &self,
        pdf_bytes: &[u8],
        page_index: u32,
        region: [f32; 4],
    ) -> Option<RegionTextOutcome> {
        let results =
            pdf_inspector::extract_text_in_regions_mem(pdf_bytes, &[(page_index, vec![region])])
                .ok()?;
        let first = results.first()?;
        let region_text = first.regions.first()?;
        Some(RegionTextOutcome {
            text: region_text.text.clone(),
            needs_ocr: region_text.needs_ocr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_pdf_bytes_are_an_honest_miss() {
        let port = PdfInspectorRegionText::new();
        assert!(
            port.extract_text_in_region(b"not a pdf", 0, [0.0, 0.0, 100.0, 100.0])
                .is_none()
        );
    }
}
