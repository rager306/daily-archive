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
    /// Margin sidenotes of the section's pages (Tufte layouts: algorithm
    /// captions N.N, footnotes, cross-refs — preserved for downstream,
    /// never merged into body reading order)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub margin_notes: String,
}

/// page_end = next entry page - 1 (last: total_pages); text = joined
/// page texts of the range (injected); parent = nearest smaller level
/// above (stack); section_id = <object>:tocN.
/// Byte offset of a section heading inside page text, whitespace-normalized
/// (headings span lines: "3.5" + "Sum-Product Variable Elimination"). A
/// preceding pure-number line ("3.5", "7.4.1") is swallowed into the anchor.
/// Returns None when the heading is absent (page kept whole — fail-open).
pub fn find_heading_anchor(page_text: &str, title: &str) -> Option<(usize, AnchorVerdict)> {
    find_heading_anchor_on_page(page_text, title, 0)
}

/// Same as [`find_heading_anchor`] with the physical page number for
/// evidence features (TOC-page match).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorVerdict {
    /// FSM: evidence says real heading
    Accepted,
    /// FSM rejected (table cell/code) — fail-open position
    Rejected,
}

/// Anchor + verdict. Escalated candidates surface as `Rejected` position
/// with the expectation that the caller marks the overlap for the critic.
pub fn find_heading_anchor_on_page(
    page_text: &str,
    title: &str,
    page_no: u32,
) -> Option<(usize, AnchorVerdict)> {
    let page_no_for_anchor = page_no;
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
    let mut first_match: Option<(usize, AnchorVerdict)> = None;
    let lines: Vec<&str> = page_text.lines().collect();
    // окна от 1 строки (заголовок целиком на одной) до title_words+1
    // (разбит переносами после дегифенизации)
    for w in 1..=(title_words + 1).min(lines.len()) {
        for start in 0..lines.len().saturating_sub(w - 1) {
            let joined = lines[start..start + w].join(" ");
            // Якорь = ЗАГОЛОВОК, не ячейка таблицы/строка кода: первая строка
            // окна обязана начинаться с первого слова заголовка (case- и
            // quote-normalized). «Hex world» как ячейка посреди таблицы F.1
            // начинается с 'hex' — но её ПРЕДЫДУЩАЯ строка 'Problem' не
            // совпадает с началом заголовка... недостаточно; жёстче: вся
            // строка должна быть префиксом нормализованного заголовка
            // ('Hex world' == весь заголовок при w=1 — проходит; но ячейка
            // таблицы тоже пройдёт). Решающий признак: длина строки ячейки
            // мала и она стоит среди других коротких строк-ячеек — неотличимо
            // геометрически. Поэтому: требуем, чтобы СЛЕДУЮЩАЯ за якорем
            // строка была прозой (>60 chars) — у настоящего заголовка далее
            // идёт абзац.
            if norm(&joined) == title_n {
                let off: usize = lines[..start].iter().map(|l| l.len() + 1).sum();
                let off = off.min(page_text.len());
                match crate::boundary_agent::classify(&crate::boundary_agent::extract_features(
                    &lines, start, w, page_no_for_anchor, page_no_for_anchor,
                )) {
                    crate::boundary_agent::Decision::Accept => {
                        return Some((off, AnchorVerdict::Accepted));
                    }
                    _ => {
                        if first_match.is_none() {
                            first_match = Some((off, AnchorVerdict::Rejected));
                        }
                    }
                }
                // FSM boundary-agent (ADR-073 wave): ambiguous anchors
                // classified by evidence features. Accept → split here;
                // Reject/Escalate → keep scanning for a better candidate,
                // and if none is found the FIRST candidate position is used
                // (fail-open split at first match: content must not vanish
                // from both sections — live lesson, Hex World Table F.1).
            }
        }
    }
    // fail-open: ни одного Accept — позиция первого совпадения с вердиктом
    // Rejected (спорный блок попадёт в обе секции; критик разрешит)
    first_match.map(|(mut off, verdict)| { off = off.min(page_text.len()); (off, verdict) })
}

pub fn build_pdf_toc_sections(
    toc: &[TocEntry],
    pages_text: &BTreeMap<u32, String>,
    pages_margin_notes: Option<&BTreeMap<u32, String>>,
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
                let mut start_cut = 0usize;
                let mut end_cut = t.len();
                // свой заголовок на первой странице диапазона
                if p == e.page
                    && let Some((off, _verdict)) = find_heading_anchor_on_page(t, &e.title, p)
                {
                    start_cut = off;
                }
                // следующая секция может начаться на ЛЮБОЙ странице диапазона
                if let Some(next) = toc.get(i + 1)
                    && next.page >= p
                    && next.page <= page_end
                    && let Some(shared) = pages_text.get(&next.page)
                    && let Some((off, _verdict)) = find_heading_anchor_on_page(shared, &next.title, next.page)
                    && p == next.page
                {
                    end_cut = off.min(t.len());
                }
                let page_slice: &str = if start_cut < end_cut { &t[start_cut..end_cut] } else { "" };
if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(page_slice);
            }
        }
        // Boundary tail (103/267 lost, live audit 2026-08-25): content on the
        // NEXT section's first page ABOVE its heading belongs to THIS section —
        // anchor-splitting removed it from the next section, so reassign here.
        // Only when the anchor is actually found (fail-open keeps nothing).
        if let Some(next) = toc.get(i + 1)
            && next.page == page_end + 1
            && let Some(shared) = pages_text.get(&next.page)
            && let Some((off, _verdict)) = find_heading_anchor_on_page(shared, &next.title, next.page)
            && off > 0
        {
            let tail = shared[..off].trim();
            if !tail.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(tail);
            }
        }

        // Margin sidenotes of this section's page range (algorithm captions,
        // footnotes) — attached as a structured field, never into body order
        let margin_notes = pages_margin_notes
            .map(|m| {
                (e.page..=page_end)
                    .filter_map(|p| m.get(&p))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

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
            margin_notes,
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
    fn boundary_tail_reassigned_to_previous_section() {
        // Q-Learning regression: eq (17.10) жил на первой странице Sarsa НАД
        // её заголовком — anchor-фикс удалял его вовсе (103/267 границ)
        let pages = BTreeMap::from([
            (1u32, "intro text of Q-Learning section".to_string()),
            (2u32, "tail paragraph with eq (17.10)\n\n17.3\nSarsa\nSarsa body begins.".to_string()),
        ]);
        let toc = vec![
            TocEntry { level: 3, title: "Q-Learning".into(), page: 1 },
            TocEntry { level: 3, title: "Sarsa".into(), page: 2 },
        ];
        let f = TocFurniture::from_patterns(&[]);
        let s = build_pdf_toc_sections(&toc, &pages, None, 3, "bk", &f);
        let ql = s.iter().find(|x| x.title == "Q-Learning").unwrap();
        assert!(
            ql.text.contains("eq (17.10)"),
            "boundary tail must return to the previous section: {}",
            &ql.text[..80.min(ql.text.len())]
        );
        let sarsa = s.iter().find(|x| x.title == "Sarsa").unwrap();
        assert!(
            sarsa.text.contains("Sarsa body"),
            "next section must have its own body content: {:?}",
            &sarsa.text[..60.min(sarsa.text.len())]
        );
        assert!(!sarsa.text.contains("17.10"));
    }

    #[test]
    fn margin_notes_attached_structured_not_merged() {
        let pages = BTreeMap::from([(1u32, "body text only".to_string())]);
        let margins = BTreeMap::from([(1u32, "Algorithm 7.6. Policy iteration caption".to_string())]);
        let toc = vec![TocEntry { level: 3, title: "Policy Iteration".into(), page: 1 }];
        let f = TocFurniture::from_patterns(&[]);
        let s = build_pdf_toc_sections(&toc, &pages, Some(&margins), 2, "bk", &f);
        assert_eq!(s[0].margin_notes, "Algorithm 7.6. Policy iteration caption");
        assert_eq!(s[0].text, "body text only", "caption stays OUT of body order");
        assert!(s[0].text.is_empty() || !s[0].text.contains("Algorithm"));
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
        let sections = build_pdf_toc_sections(&toc, &pages, None, 10, "bk", &f);
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
        let secs = build_pdf_toc_sections(&toc, &pages(1, 30), None, 30, "m", &furniture());
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
        let secs = build_pdf_toc_sections(&toc, &pages(1, 30), None, 30, "m", &furniture());
        assert_eq!(secs[0].page_end, 30);
    }
}
