//! HTML headings grammar (M451 phase-2 port): h1-h6 tree IS the
//! structure (textbook chapters / KB pages). Port of the reference
//! python parser (scripts/m451_textbook_import.py) — cross-validated
//! on gnn-textbook chapters. Pure logic.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// One parsed section (html_headings grammar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlSection {
    pub section_id: String,
    pub title: String,
    pub path_id: String,
    pub level: u8,
    pub parent: Option<String>,
    pub text: String,
    pub char_count: usize,
}

/// Split HTML into sections at h1-h6 boundaries.
/// - permalink markers ('¶') scrubbed (html furniture)
/// - path_id from numbering in the title ('1.1.1') when present, else hN
/// - parent = nearest heading of a smaller level above (stack)
pub fn parse_html_headings(html: &str, object_id: &str) -> Vec<HtmlSection> {
    // rust regex has no backreferences: match any closing hN and verify
    let head_re = Regex::new(r"(?s)<h([1-6])[^>]*>(.*?)</h([1-6])[^>]*>").expect("static");
    let tag_re = Regex::new(r"<[^>]+>").expect("static");
    let num_re = Regex::new(r"^(\d+(?:\.\d+)*)\.?\s").expect("static");

    let matches: Vec<_> = head_re.find_iter(html).collect();
    let mut sections = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    for (i, m) in matches.iter().enumerate() {
        let caps = head_re
            .captures(&html[m.start()..m.end()])
            .expect("matched");
        if caps[1] != caps[3] {
            continue; // mismatched open/close level
        }
        let level: u8 = caps[1].parse().unwrap_or(1);
        let raw_title = caps[2].to_string();
        let title = tag_re
            .replace_all(&raw_title.replace('¶', ""), " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if title.is_empty() {
            continue;
        }
        let body = &html[m.end()..matches.get(i + 1).map(|n| n.start()).unwrap_or(html.len())];
        let text = tag_re
            .replace_all(body, " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let path_id = num_re
            .captures(&title)
            .map(|c| c[1].to_string())
            .unwrap_or_else(|| format!("h{level}"));
        while stack.last().map(|(lvl, _)| *lvl >= level).unwrap_or(false) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, id)| id.clone());
        let sid = format!("{object_id}:h{i}");
        sections.push(HtmlSection {
            section_id: sid.clone(),
            title,
            path_id,
            level,
            parent,
            char_count: text.chars().count(),
            text,
        });
        stack.push((level, sid));
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_by_headings_with_numbering_path() {
        let html = r#"<html><body>
<h1>Chapter 1 <a>¶</a></h1><p>intro text here</p>
<h2>Summary</h2><p>chapter summary</p>
<h3>1.1.1 Defining a Graph</h3><p>definition text</p>
<h2>2 Next</h2><p>more</p>
</body></html>"#;
        let s = parse_html_headings(html, "ch");
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].title, "Chapter 1"); // permalink scrubbed
        assert_eq!(s[0].path_id, "h1");
        assert!(s[0].parent.is_none());
        assert_eq!(s[1].parent.as_deref(), Some("ch:h0"));
        assert_eq!(s[2].path_id, "1.1.1"); // numbering wins
        assert_eq!(s[2].parent.as_deref(), Some("ch:h1"));
        assert_eq!(s[2].level, 3);
        assert!(s[2].text.contains("definition text"));
        assert_eq!(s[3].path_id, "2");
        assert_eq!(s[3].parent.as_deref(), Some("ch:h0")); // stack popped h3
    }

    #[test]
    fn empty_title_heading_is_skipped() {
        let html = r#"<h2><a>¶</a></h2><h2>Real</h2><p>t</p>"#;
        let s = parse_html_headings(html, "x");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].title, "Real");
    }
}
