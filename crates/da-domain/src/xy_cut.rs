//! XY-Cut reading-order reconstruction (ADR-072) over neutral line boxes.
//!
//! Ported from opendataloader-pdf's `XYCutPlusPlusSorter` (Apache-2.0,
//! arXiv:2504.10258), reduced to our corpus needs: scientific monographs
//! with body + margin-sidenote layouts (Tufte) and two-column spreads.
//!
//! Algorithm (per page):
//! 1. **Pre-mask**: header/footer furniture and margin sidenotes leave the
//!    core set (our proven x>0.65W classification becomes phase 1).
//! 2. **Recursive XY-Cut** on body lines: find best horizontal and vertical
//!    cuts via edge-gap profiles; split along the LARGER gap (min-gap
//!    threshold); narrow-element retry when an outlier bridges a column gap;
//!    fall back to y-then-x order when no valid cuts remain.
//! 3. **Paragraph merge** with geometry guards (same column + small vertical
//!    gap + open sentence) — never across cut boundaries.
//!
//! dedoc insight adopted: paragraph unification is a property of the node
//! (`can_be_multiline`), applied only within one structural block — our
//! geometry guards enforce exactly that boundary discipline.

/// A positioned text line. Neutral geometry type: adapters convert engine
/// specific items (pdf-inspector `TextItem`, PyMuPDF dict lines) into this.
#[derive(Debug, Clone, PartialEq)]
pub struct LineBox {
    pub text: String,
    /// PDF points, origin top-left (y grows downward).
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl LineBox {
    fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    fn center_y(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
}

/// Configuration; defaults mirror opendataloader constants.
#[derive(Debug, Clone)]
pub struct XyCutConfig {
    /// Margin zone starts right of this fraction of page width.
    pub margin_x_fraction: f32,
    /// Lines above this y are header furniture.
    pub header_y_max: f32,
    /// Lines below H - footer_band are footer furniture.
    pub footer_band: f32,
    /// Minimum meaningful gap between blocks (PDF points) — MIN_GAP_THRESHOLD.
    pub min_gap: f32,
    /// Elements narrower than this fraction of region width are outliers for
    /// the narrow-element retry — NARROW_ELEMENT_WIDTH_RATIO.
    pub narrow_element_ratio: f32,
    /// Sidenotes shorter than this many chars are figure labels (dropped).
    pub min_margin_chars: usize,
    /// Merge consecutive body lines reading as one paragraph (geometry-
    /// guarded: same column, vertical gap within leading tolerance).
    pub merge_continuation_lines: bool,
    /// Max vertical gap (× line height) for two lines to be one paragraph.
    pub leading_tolerance: f32,
}

impl Default for XyCutConfig {
    fn default() -> Self {
        Self {
            margin_x_fraction: 0.65,
            header_y_max: 50.0,
            footer_band: 70.0,
            min_gap: 5.0,
            narrow_element_ratio: 0.1,
            min_margin_chars: 4,
            merge_continuation_lines: true,
            leading_tolerance: 0.5,
        }
    }
}

/// Classified page regions.
#[derive(Debug, Default, Clone)]
pub struct PageRegions {
    /// Core content in reconstructed reading order (paragraphs merged when
    /// enabled and geometry-guarded).
    pub body: Vec<String>,
    /// Margin sidenotes in reading order (provenance kept separately).
    pub margin_notes: Vec<String>,
    /// Header/footer furniture texts (page numbers, running heads).
    pub furniture: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct Cut {
    position: f32,
    gap: f32,
}

const NO_CUT: Cut = Cut {
    position: 0.0,
    gap: 0.0,
};

/// Figure-zone pre-mask (XY-Cut++ Phase 2): a compact cluster of ≥3
/// numeric/short lines with no prose inside its bbox is an axis-label field —
/// excluded from body ordering (specimen p61: 6 ticks, 163..306×278..332,
/// 0 prose lines inside).
fn is_figure_zone(zone: &[&LineBox], all: &[LineBox]) -> bool {
    if zone.len() < 3 {
        return false;
    }
    let zx0 = zone.iter().map(|l| l.x0).fold(f32::MAX, f32::min);
    let zy0 = zone.iter().map(|l| l.y0).fold(f32::MAX, f32::min);
    let zx1 = zone.iter().map(|l| l.x1).fold(f32::MIN, f32::max);
    let zy1 = zone.iter().map(|l| l.y1).fold(f32::MIN, f32::max);
    !all.iter().any(|l| {
        let stripped = l.text.replace(['.', ',', '%'], "").trim().to_string();
        let numeric = !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit());
        let short_symbol = l.text.trim().chars().count() <= 2;
        let is_tickish = numeric || short_symbol;
        !is_tickish
            && l.text.trim().chars().count() >= 4
            && l.x0 < zx1
            && l.x1 > zx0
            && l.y0 < zy1
            && l.y1 > zy0
    })
}

fn looks_like_tick(l: &LineBox) -> bool {
    let stripped = l.text.replace(['.', ',', '%'], "").trim().to_string();
    let numeric = !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit());
    numeric || l.text.trim().chars().count() <= 2
}

/// Split one page's lines into ordered regions.
/// Reconstruct reading order while retaining every positioned line.
///
/// Unlike [`split_page`], this is the bakeoff seam: it does not classify,
/// drop, merge, or convert lines to strings. The returned vector is a
/// permutation of the input and therefore preserves provider identity and
/// geometry for the neutral adapter.
pub fn order_lines(lines: &[LineBox], cfg: &XyCutConfig) -> Vec<LineBox> {
    recursive_segment(lines, cfg)
}

pub fn split_page(
    lines: &[LineBox],
    page_width: f32,
    page_height: f32,
    cfg: &XyCutConfig,
) -> PageRegions {
    let mut out = PageRegions::default();
    let margin_x = cfg.margin_x_fraction * page_width;

    let mut core: Vec<LineBox> = Vec::new();
    let mut margins: Vec<(f32, LineBox)> = Vec::new();

    for l in lines {
        if l.text.trim().is_empty() {
            continue;
        }
        // Phase 1 pre-mask: header/footer furniture
        if l.y1 < cfg.header_y_max || l.y0 > page_height - cfg.footer_band {
            out.furniture.push(l.text.clone());
            continue;
        }
        // Margin zone: sidenote vs figure axis label (pure-number ticks like
        // "0.2", "2003." carry no prose)
        if l.x0 >= margin_x {
            let stripped = l.text.replace(['.', ',', '%'], "").trim().to_string();
            let numeric = !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit());
            if !numeric && l.text.trim().chars().count() >= cfg.min_margin_chars {
                margins.push((l.center_y(), l.clone()));
            }
            continue;
        }
        core.push(l.clone());
    }

    // Phase 2 pre-mask: figure zones (compact tick clusters) leave the core
    let mut masked: Vec<usize> = Vec::new();
    {
        let tickish: Vec<usize> = (0..core.len())
            .filter(|i| looks_like_tick(&core[*i]))
            .collect();
        if tickish.len() >= 3 {
            // candidate zones: connected clusters of tickish lines by bbox overlap
            let refs: Vec<&LineBox> = tickish.iter().map(|&i| &core[i]).collect();
            if is_figure_zone(&refs, &core) {
                masked = tickish;
            }
        }
    }
    let core: Vec<LineBox> = core
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !masked.contains(i))
        .map(|(_, l)| l)
        .collect();

    // Phase 3: recursive segmentation over the core → ordered boxes
    let ordered = recursive_segment(&core, cfg);
    out.body = if cfg.merge_continuation_lines {
        merge_paragraphs(&ordered, cfg)
    } else {
        ordered.into_iter().map(|b| b.text).collect()
    };

    // Margins keep their own reading order (top to bottom).
    margins.sort_by_key(|(y, _)| *y as i64);
    out.margin_notes = margins.into_iter().map(|(_, l)| l.text).collect();
    out
}

/// Recursive XY-Cut returning ORDERED BOXES (not merged strings): splitting
/// decisions stay geometric; text fusion happens once, afterwards, under
/// geometry guards so it can never cross a cut boundary.
fn recursive_segment(items: &[LineBox], cfg: &XyCutConfig) -> Vec<LineBox> {
    if items.len() <= 1 {
        return items.to_vec();
    }

    let h_cut = best_horizontal_cut(items);
    let v_cut = best_vertical_cut(items, cfg);

    let h_valid = h_cut.gap >= cfg.min_gap;
    let v_valid = v_cut.gap >= cfg.min_gap;

    let use_horizontal = match (h_valid, v_valid) {
        (true, true) => h_cut.gap > v_cut.gap,
        (true, false) => true,
        (false, true) => false,
        // no valid cuts → plain reading order (opendataloader fallback)
        (false, false) => {
            let mut sorted: Vec<&LineBox> = items.iter().collect();
            sorted.sort_by_key(|l| (l.y0 as i64, l.x0 as i64));
            return sorted.into_iter().cloned().collect();
        }
    };

    let groups = if use_horizontal {
        split_by_horizontal(items, h_cut.position)
    } else {
        split_by_vertical(items, v_cut.position)
    };
    if groups.len() <= 1 {
        // safety: single group would loop forever → flat order
        let mut sorted: Vec<&LineBox> = items.iter().collect();
        sorted.sort_by_key(|l| (l.y0 as i64, l.x0 as i64));
        return sorted.into_iter().cloned().collect();
    }
    let mut out = Vec::new();
    for g in &groups {
        out.extend(recursive_segment(g, cfg));
    }
    out
}

/// Largest vertical gap between consecutive lines (sorted top→bottom).
fn best_horizontal_cut(items: &[LineBox]) -> Cut {
    if items.len() < 2 {
        return NO_CUT;
    }
    let mut sorted: Vec<&LineBox> = items.iter().collect();
    sorted.sort_by_key(|l| (l.y0 as i64, l.y1 as i64));
    let mut largest = NO_CUT;
    let mut prev_bottom: Option<f32> = None;
    for l in sorted {
        if let Some(pb) = prev_bottom
            && pb < l.y0
        {
            let gap = l.y0 - pb;
            if gap > largest.gap {
                largest = Cut {
                    position: (pb + l.y0) / 2.0,
                    gap,
                };
            }
        }
        prev_bottom = Some(match prev_bottom {
            Some(b) => b.max(l.y1),
            None => l.y1,
        });
    }
    largest
}

/// Largest horizontal gap between line edges; retries without narrow
/// outliers (page numbers, footnote markers bridging a column gap).
fn best_vertical_cut(items: &[LineBox], cfg: &XyCutConfig) -> Cut {
    if items.len() < 2 {
        return NO_CUT;
    }
    let edge_cut = vertical_cut_by_edges(items);
    if edge_cut.gap >= cfg.min_gap {
        return edge_cut;
    }
    // Narrow-element retry: elements narrower than ratio*region-width may
    // bridge an otherwise clean column gap.
    if items.len() >= 3 {
        let region_w = items.iter().map(|l| l.x1).fold(f32::MIN, f32::max)
            - items.iter().map(|l| l.x0).fold(f32::MAX, f32::min);
        let narrow_threshold = region_w * cfg.narrow_element_ratio;
        let filtered: Vec<LineBox> = items
            .iter()
            .filter(|l| l.width() >= narrow_threshold)
            .cloned()
            .collect();
        if filtered.len() >= 2 && filtered.len() < items.len() {
            let filtered_cut = vertical_cut_by_edges(&filtered);
            if filtered_cut.gap > edge_cut.gap && filtered_cut.gap >= cfg.min_gap {
                return filtered_cut;
            }
        }
    }
    edge_cut
}

/// Gap between rightX of one element and leftX of the next (x-sorted).
fn vertical_cut_by_edges(items: &[LineBox]) -> Cut {
    let mut sorted: Vec<&LineBox> = items.iter().collect();
    sorted.sort_by_key(|l| (l.x0 as i64, l.x1 as i64));
    let mut largest = NO_CUT;
    let mut prev_right: Option<f32> = None;
    for l in sorted {
        if let Some(pr) = prev_right
            && l.x0 > pr
        {
            let gap = l.x0 - pr;
            if gap > largest.gap {
                largest = Cut {
                    position: (pr + l.x0) / 2.0,
                    gap,
                };
            }
        }
        prev_right = Some(match prev_right {
            Some(r) => r.max(l.x1),
            None => l.x1,
        });
    }
    largest
}

fn split_by_horizontal(items: &[LineBox], cut_y: f32) -> Vec<Vec<LineBox>> {
    let mut above = Vec::new();
    let mut below = Vec::new();
    for l in items {
        if l.center_y() <= cut_y {
            above.push(l.clone());
        } else {
            below.push(l.clone());
        }
    }
    vec![above, below]
}

fn split_by_vertical(items: &[LineBox], cut_x: f32) -> Vec<Vec<LineBox>> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for l in items {
        if l.x1 <= cut_x {
            left.push(l.clone());
        } else {
            right.push(l.clone());
        }
    }
    vec![left, right]
}

/// Geometry-guarded paragraph fusion (dedoc `can_be_multiline` discipline):
/// join a line into the previous paragraph only when it opens mid-sentence,
/// sits in the SAME column (left edge within 25% of width), and the vertical
/// gap is within leading tolerance. Cut boundaries therefore survive.
fn merge_paragraphs(boxes: &[LineBox], cfg: &XyCutConfig) -> Vec<String> {
    struct Open {
        text: String,
        x0: f32,
        y1: f32,
        height: f32,
        width: f32,
    }
    let mut out: Vec<String> = Vec::new();
    let mut open: Option<Open> = None;
    for b in boxes {
        let joins = match &open {
            Some(prev) => {
                let same_column =
                    (b.x0 - prev.x0).abs() <= 0.25 * prev.width.max(b.width()).max(1.0);
                let gap_ok = (b.y0 - prev.y1) <= cfg.leading_tolerance * prev.height;
                !prev.text.ends_with(['.', '!', '?', ':', ';']) && same_column && gap_ok
            }
            None => false,
        };
        match (&mut open, joins) {
            (Some(prev), true) => {
                prev.text.push(' ');
                prev.text.push_str(&b.text);
                prev.y1 = b.y1;
            }
            _ => {
                if let Some(prev) = open.take() {
                    out.push(prev.text);
                }
                open = Some(Open {
                    text: b.text.clone(),
                    x0: b.x0,
                    y1: b.y1,
                    height: b.height(),
                    width: b.width(),
                });
            }
        }
    }
    if let Some(prev) = open {
        out.push(prev.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: f32, y: f32, w: f32, text: &str) -> LineBox {
        LineBox {
            text: text.to_string(),
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + 10.0,
        }
    }

    const W: f32 = 576.0;
    const H: f32 = 648.0;

    #[test]
    fn margin_notes_separated_and_body_ordered() {
        let lines = vec![
            line(36.0, 20.0, 100.0, "20"),
            line(35.0, 80.0, W - 90.0, "Body first paragraph."),
            line(430.0, 95.0, 120.0, "3 See discussion in E. T. Jaynes"),
            line(35.0, 110.0, W - 90.0, "Body second line continues"),
            line(430.0, 130.0, 120.0, "Probability Theory. Cambridge"),
            line(35.0, H - 40.0, 300.0, "(c) 2022 MIT Press"),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(r.body.len(), 2, "{:?}", r.body);
        assert!(r.body[0].starts_with("Body first paragraph."));
        assert!(r.body[1].starts_with("Body second line"));
        assert_eq!(r.margin_notes.len(), 2);
        assert_eq!(
            r.furniture,
            vec!["20".to_string(), "(c) 2022 MIT Press".to_string()]
        );
    }

    #[test]
    fn figure_axis_labels_dropped_not_sidenotes() {
        let lines = vec![
            line(35.0, 80.0, 400.0, "Real body text."),
            line(420.0, 200.0, 30.0, "0.2"),
            line(420.0, 220.0, 30.0, "2003."),
            line(430.0, 260.0, 120.0, "A real margin note about the figure."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(
            r.margin_notes,
            vec!["A real margin note about the figure.".to_string()]
        );
        assert_eq!(r.body, vec!["Real body text.".to_string()]);
    }

    // ── ported opendataloader behaviour ──

    #[test]
    fn two_column_layout_interleaves_correctly() {
        let lines = vec![
            line(40.0, 80.0, 230.0, "LEFT para one."),
            line(300.0, 80.0, 230.0, "RIGHT para one."),
            line(40.0, 140.0, 230.0, "LEFT para two."),
            line(300.0, 140.0, 230.0, "RIGHT para two."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(
            r.body,
            vec![
                "LEFT para one.".to_string(),
                "RIGHT para one.".to_string(),
                "LEFT para two.".to_string(),
                "RIGHT para two.".to_string(),
            ],
            "vertical cut sequences columns before rows: {:?}",
            r.body
        );
    }

    #[test]
    fn full_width_title_precedes_columns() {
        let lines = vec![
            line(36.0, 60.0, W - 72.0, "Chapter Title Spanning Full Width"),
            line(40.0, 120.0, 230.0, "LEFT body under title."),
            line(300.0, 120.0, 230.0, "RIGHT body under title."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(r.body.first().unwrap(), "Chapter Title Spanning Full Width");
        assert_eq!(r.body[1], "LEFT body under title.");
        assert_eq!(r.body[2], "RIGHT body under title.");
    }

    #[test]
    fn narrow_page_number_does_not_bridge_column_gap() {
        let lines = vec![
            line(40.0, 80.0, 230.0, "Left column content here."),
            line(W / 2.0 - 10.0, 84.0, 20.0, "7"),
            line(300.0, 80.0, 230.0, "Right column content here."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert!(
            r.body.contains(&"Left column content here.".to_string())
                && r.body.contains(&"Right column content here.".to_string()),
            "columns must stay distinct blocks: {:?}",
            r.body
        );
    }

    #[test]
    fn axis_tick_cluster_masked_as_figure_zone() {
        // specimen algorithms-for-decision-making p61: compact tick cluster
        // (Z Y 0.13 0.05 X...) must not interleave into body reading order
        let mut lines = vec![
            line(
                35.0,
                80.0,
                500.0,
                "Body prose above the figure continues here.",
            ),
            line(163.0, 278.0, 20.0, "Z"),
            line(200.0, 280.0, 20.0, "Y"),
            line(240.0, 300.0, 30.0, "0.13"),
            line(170.0, 320.0, 30.0, "0.05"),
            line(260.0, 330.0, 20.0, "X"),
            line(
                35.0,
                400.0,
                500.0,
                "Body prose below the figure resumes normally.",
            ),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(r.body.len(), 2, "only two prose paragraphs: {:?}", r.body);
        assert!(r.body.iter().all(|b| !b.contains("0.13")));
        let _ = &mut lines;
    }

    #[test]
    fn continuation_lines_merge_within_column_only() {
        let lines = vec![
            line(35.0, 80.0, 400.0, "A sentence that wraps across"),
            line(35.0, 92.0, 400.0, "two physical lines without end."),
            line(35.0, 110.0, 400.0, "New paragraph starts here."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(
            r.body,
            vec![
                "A sentence that wraps across two physical lines without end.".to_string(),
                "New paragraph starts here.".to_string(),
            ]
        );
    }

    #[test]
    fn wrapped_line_never_jumps_across_columns() {
        // left column line ending mid-sentence, right column physically
        // adjacent on the page — must NOT fuse even though the sentence is
        // open (different column ⇒ different block)
        let lines = vec![
            line(40.0, 80.0, 230.0, "A thought that continues"),
            line(40.0, 92.0, 230.0, "in the left column."),
            line(300.0, 92.0, 230.0, "Right column separate text."),
        ];
        let r = split_page(&lines, W, H, &XyCutConfig::default());
        assert_eq!(
            r.body,
            vec![
                "A thought that continues in the left column.".to_string(),
                "Right column separate text.".to_string(),
            ],
            "{:?}",
            r.body
        );
    }
}
