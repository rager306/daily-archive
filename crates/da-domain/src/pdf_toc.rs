//! PDF TOC grammar (M451 phase-2 port #6, page_outline): embedded
//! bookmarks -> sections with page ranges. Analytic core over INJECTED
//! toc observations (extractor stays reference); text per page is
//! injected as a map. Pure logic.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One TOC bookmark observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TocEntry {
    pub level: u8,
    pub title: String,
    pub page: u32,
}

/// Furniture patterns for monograph TOCs (DATA).
#[derive(Debug, Clone)]
pub struct TocFurniture {
    patterns: Vec<Regex>,
}

impl TocFurniture {
    pub fn from_patterns(pats: &[String]) -> Self {
        Self {
            patterns: pats.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        }
    }
    pub fn is_furniture(&self, title: &str) -> bool {
        self.patterns.iter().any(|re| re.is_match(title.trim()))
    }
}

/// Built section (page range anchor semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfTocSection {
    pub section_id: String,
    pub title: String,
    pub depth: u8,
    pub page_start: u32,
    pub page_end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub is_furniture: bool,
    pub char_count: usize,
    #[serde(default)]
    pub text: String,
}

/// page_end = next entry page - 1 (last: total_pages); text = joined
/// page texts of the range (injected); parent = nearest smaller level
/// above (stack); section_id = <object>:tocN.
pub fn build_pdf_toc_sections(
    toc: &[TocEntry],
    pages_text: &BTreeMap<u32, String>,
    total_pages: u32,
    object_id: &str,
    furniture: &TocFurniture,
) -> Vec<PdfTocSection> {
    let mut out: Vec<PdfTocSection> = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    for (i, e) in toc.iter().enumerate() {
        let next_page = toc.get(i + 1).map(|n| n.page).unwrap_or(total_pages + 1);
        let page_end = e.page.max(next_page.saturating_sub(1).max(e.page));
        while stack.last().map(|(lvl, _)| *lvl >= e.level).unwrap_or(false) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, id)| id.clone());
        let mut text = String::new();
        for p in e.page..=page_end {
            if let Some(t) = pages_text.get(&p) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
        out.push(PdfTocSection {
            section_id: format!("{object_id}:toc{i}"),
            title: e.title.trim().to_string(),
            depth: e.level,
            page_start: e.page,
            page_end,
            parent,
            is_furniture: furniture.is_furniture(&e.title),
            char_count: text.chars().count(),
            text,
        });
        stack.push((e.level, format!("{object_id}:toc{i}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn furniture() -> TocFurniture {
        TocFurniture::from_patterns(&[
            "^Trinity University$".into(),
            "^Table of Contents$".into(),
        ])
    }

    fn pages(from: u32, to: u32) -> BTreeMap<u32, String> {
        (from..=to).map(|p| (p, format!("page-{p} body text"))).collect()
    }

    #[test]
    fn ranges_parent_stack_furniture() {
        let toc = vec![
            TocEntry { level: 1, title: "Trinity University".into(), page: 1 },
            TocEntry { level: 1, title: "Chapter 1 Real Numbers".into(), page: 10 },
            TocEntry { level: 2, title: "Section 1.1 System".into(), page: 10 },
            TocEntry { level: 2, title: "Section 1.2 Induction".into(), page: 19 },
        ];
        let secs = build_pdf_toc_sections(&toc, &pages(1, 30), 30, "m", &furniture());
        assert_eq!(secs.len(), 4);
        assert!(secs[0].is_furniture);
        assert!(!secs[1].is_furniture);
        assert_eq!(secs[1].page_end, 10); // child starts same page: max(page, next-1)=page
        assert_eq!(secs[1].parent, None);
        assert_eq!(secs[2].parent.as_deref(), Some("m:toc1"));
        assert_eq!(secs[2].page_start, 10);
        assert_eq!(secs[2].page_end, 18);
        assert!(secs[3].text.contains("page-19"));
        assert!(secs[3].text.contains("page-28")); // ends before toc3? toc2 ends next-1=18
    }

    #[test]
    fn last_entry_runs_to_total() {
        let toc = vec![TocEntry { level: 1, title: "Tail".into(), page: 25 }];
        let secs = build_pdf_toc_sections(&toc, &pages(1, 30), 30, "m", &furniture());
        assert_eq!(secs[0].page_end, 30);
    }
}
