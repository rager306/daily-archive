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
/// Byte offset of a section heading inside page text, whitespace-normalized
/// (headings span lines: "3.5" + "Sum-Product Variable Elimination"). A
/// preceding pure-number line ("3.5", "7.4.1") is swallowed into the anchor.
/// Returns None when the heading is absent (page kept whole — fail-open).
fn find_heading_anchor(page_text: &str, title: &str) -> Option<usize> {
    let norm = |s: &str| {
        // типографские апострофы/кавычки → прямые (TOC хранит ASCII,
        // PDF-текст — U+2018/2019/201C/201D: Prisoner's Dilemma)
        s.replace(['\u{2018}', '\u{2019}', '\u{201B}', '\u{2032}'], "'")
            .replace(['\u{201C}', '\u{201D}', '\u{201F}', '\u{2033}'], "\"")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let title_n = norm(title);
    let title_words = title_n.split_whitespace().count();
    if title_words == 0 {
        return None;
    }
    let lines: Vec<&str> = page_text.lines().collect();
    // окна от 1 строки (заголовок целиком на одной) до title_words+1
    // (разбит переносами после дегифенизации)
    for w in 1..=(title_words + 1).min(lines.len()) {
        for start in 0..lines.len().saturating_sub(w - 1) {
            let joined = lines[start..start + w].join(" ");
            if norm(&joined) == title_n {
                // проглотить строку-номер перед заголовком ("3.5")
                let mut s = start;
                if s > 0 {
                    let prev = lines[s - 1].trim();
                    let numeric = !prev.is_empty()
                        && prev
                            .chars()
                            .all(|c| c.is_ascii_digit() || c == '.');
                    if numeric {
                        s -= 1;
                    }
                }
                let offset: usize = lines[..s].iter().map(|l| l.len() + 1).sum();
                return Some(offset.min(page_text.len()));
            }
        }
    }
    None
}

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
                // anchor-splitting: секция начинается СВОИМ заголовком на
                // первой странице — хвост предыдущей секции, фигуры и код
                // до заголовка не входят (algorithms-for-decision-making:
                // Value Iteration начиналась осями графика + хвостом policy
                // iteration; Policy Iteration — Julia-кодом прошлой секции)
                let page_slice: &str = if p == e.page {
                    match find_heading_anchor(t, &e.title) {
                        Some(off) => &t[off..],
                        None => t.as_str(),
                    }
                } else {
                    t.as_str()
                };
                // последняя страница: обрезать на заголовке следующей секции
                // (её текст заберёт свой first-page anchor)
                let page_slice: String = if p == page_end && p >= next_page {
                    match toc.get(i + 1).and_then(|n| find_heading_anchor(t, &n.title)) {
                        Some(off) => page_slice[..off.min(page_slice.len())].to_string(),
                        None => page_slice.to_string(),
                    }
                } else {
                    page_slice.to_string()
                };
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&page_slice);
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
    fn anchor_splitting_removes_previous_section_tail() {
        // algorithms-for-decision-making регрессия: Value Iteration начиналась
        // хвостом policy iteration + осями фигуры; Policy Iteration — Julia-кодом
        let pages = BTreeMap::from([(
            5u32,
            "iteration 1\niteration 2\nPolicy iteration tends to be expensive\n\n7.4\nPolicy Iteration\nPolicy iteration (algorithm 7.6) is one way to iterate.\nBody of policy iteration.".to_string(),
        )]);
        let toc = vec![
            TocEntry { level: 2, title: "Modified Policy Iteration".into(), page: 4 },
            TocEntry { level: 2, title: "Policy Iteration".into(), page: 5 },
            TocEntry { level: 2, title: "Value Iteration".into(), page: 9 },
        ];
        let f = TocFurniture::from_patterns(&[]);
        let sections = build_pdf_toc_sections(&toc, &pages, 10, "bk", &f);
        let pi = sections.iter().find(|s| s.title == "Policy Iteration").unwrap();
        assert!(
            !pi.text.contains("tends to be expensive"),
            "хвост предыдущей секции не должен входить: {}",
            &pi.text[..80.min(pi.text.len())]
        );
        assert!(
            !pi.text.contains("iteration 1"),
            "оси фигуры не должны входить"
        );
        assert!(
            pi.text.contains("algorithm 7.6"),
            "тело секции должно сохраниться"
        );
        // хвост ДОСТУПЕН предыдущей секции (её last page обрезается на якоре)
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
