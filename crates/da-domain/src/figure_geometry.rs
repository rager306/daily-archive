//! Figure geometry matching (M451 phase-2 port #3): caption <-> image
//! pairing by page + vertical proximity, page-range -> section join.
//! Pure analytic core; PDF page observations are INJECTED by the
//! adapter (extractor stays reference until a Rust PDF backend lands).

use serde::{Deserialize, Serialize};

/// Caption rectangle observed on a page (PDF points, origin top-left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionRect {
    pub label: String, // e.g. "Figure 1.2.8"
    pub page: u32,
    pub y0: f64,
    pub y1: f64,
}

/// Image observed on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObs {
    pub xref: u64,
    pub page: u32,
    pub bbox: [f64; 4], // x0, y0, x1, y1
}

/// One matched figure record (index.json entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureRecord {
    pub figure_id: String,
    pub label: String,
    pub page: u32,
    pub bbox: [f64; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_file: Option<String>,
}

/// TOC/section range for the section join (page_outline grammars).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionRange {
    pub section_id: String,
    pub page_start: u32,
    pub page_end: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub is_furniture: bool,
}

/// Match each caption to the vertically-nearest image on its page.
/// Unmatched captions (no image on the page) are skipped; images may
/// be reused only once per caption (first-nearest wins).
pub fn match_figures(
    captions: &[CaptionRect],
    images: &[ImageObs],
    sections: &[SectionRange],
) -> Vec<FigureRecord> {
    let mut used: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cap in captions {
        let cap_y = (cap.y0 + cap.y1) / 2.0;
        let candidates: Vec<&ImageObs> = images
            .iter()
            .filter(|im| im.page == cap.page && !used.contains(&im.xref))
            .collect();
        let best = candidates.iter().min_by(|a, b| {
            let da = ((a.bbox[1] + a.bbox[3]) / 2.0 - cap_y).abs();
            let db = ((b.bbox[1] + b.bbox[3]) / 2.0 - cap_y).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some(im) = best else { continue };
        used.insert(im.xref);
        let section = sections
            .iter()
            .find(|s| !s.is_furniture && s.page_start <= cap.page && cap.page <= s.page_end)
            .map(|s| s.section_id.clone());
        let num = cap.label.trim_start_matches("Figure ").trim().to_string();
        out.push(FigureRecord {
            figure_id: format!("fig-{num}"),
            label: cap.label.clone(),
            page: cap.page,
            bbox: im.bbox,
            section_id: section,
            image_file: None, // extraction is the exporter's side
        });
    }
    out
}

/// Aggregated stats (manifest artifact entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureStats {
    pub captions_in_text: usize,
    pub figures_indexed: usize,
    pub with_image_file: usize,
    pub unassigned_images: usize,
}

pub fn stats(
    captions: &[CaptionRect],
    records: &[FigureRecord],
    total_images: usize,
) -> FigureStats {
    FigureStats {
        captions_in_text: captions.len(),
        figures_indexed: records.len(),
        with_image_file: records.iter().filter(|r| r.image_file.is_some()).count(),
        unassigned_images: total_images.saturating_sub(records.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(label: &str, page: u32, y0: f64, y1: f64) -> CaptionRect {
        CaptionRect {
            label: label.into(),
            page,
            y0,
            y1,
        }
    }
    fn img(xref: u64, page: u32, y: f64) -> ImageObs {
        ImageObs {
            xref,
            page,
            bbox: [0.0, y, 100.0, y + 50.0],
        }
    }

    #[test]
    fn nearest_vertical_match() {
        let caps = [
            cap("Figure 1.1", 5, 200.0, 210.0),
            cap("Figure 1.2", 5, 600.0, 610.0),
        ];
        let imgs = [img(10, 5, 300.0), img(11, 5, 100.0), img(12, 6, 100.0)];
        let recs = match_figures(&caps, &imgs, &[]);
        assert_eq!(recs.len(), 2);
        // Figure 1.1 (cap y=205): image 10 center 325 vs image 11 center 125 -> 11 nearer
        assert_eq!(recs[0].bbox[1], 100.0);
        // Figure 1.2 (cap y=605): only image 10 remains (11 used) -> 10
        assert_eq!(recs[1].bbox[1], 300.0);
    }

    #[test]
    fn caption_without_image_is_skipped_honestly() {
        let caps = [cap("Figure 2.1", 9, 100.0, 110.0)];
        let recs = match_figures(&caps, &[], &[]);
        assert!(recs.is_empty());
        let st = stats(&caps, &recs, 0);
        assert_eq!(st.captions_in_text, 1);
        assert_eq!(st.figures_indexed, 0);
    }

    #[test]
    fn section_join_skips_furniture_ranges() {
        let caps = [cap("Figure 3.1", 20, 50.0, 60.0)];
        let imgs = [img(1, 20, 40.0)];
        let sections = [
            SectionRange {
                section_id: "cover".into(),
                page_start: 1,
                page_end: 25,
                title: "Trinity University".into(),
                is_furniture: true,
            },
            SectionRange {
                section_id: "s2".into(),
                page_start: 18,
                page_end: 30,
                title: "Chapter 2".into(),
                is_furniture: false,
            },
        ];
        let recs = match_figures(&caps, &imgs, &sections);
        assert_eq!(recs[0].section_id.as_deref(), Some("s2"));
    }
}
