//! HTML parser adapter for textbook/lecture HTML content.
//!
//! Implements ParserPort for Material for MkDocs / standard HTML.
//! Used for GNN textbook chapters and other non-PDF sources.
//! No external HTML parsing crate — uses string-based extraction
//! (same approach as GrobidParser::extract_sections).
//!
//! Section identity (ADR-053 Layer A navigation fields) is derived
//! deterministically from the actual h1/h2/h3 heading stack: dotted
//! `path_id`, `parent_section_id`, `order_in_parent`, and `section_id`.
//! No heading or section is created without source evidence, and repeated
//! paths resolve with the deterministic `~N` restart convention (dot-free
//! suffixes are never misread as dotted parents).

use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort, Section};
use std::collections::{HashMap, HashSet};

/// One structural chunk between HTML heading boundaries.
///
/// `tag_level` is the h1/h2/h3 tag level that opened the chunk; `0` for
/// content before the first heading (preamble), which has no heading
/// evidence and never opens a tree node.
struct HeadingChunk {
    tag_level: u32,
    title: String,
    text: String,
}

/// HTML parser for Material for MkDocs and standard HTML documents.
pub struct HtmlParser;

impl HtmlParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract the page title from <title> tag.
    fn extract_title(html: &str) -> String {
        if let Some(start) = html.find("<title>")
            && let Some(end) = html[start..].find("</title>")
        {
            let raw = &html[start + 7..start + end];
            return Self::strip_tags(raw).trim().to_string();
        }
        "Untitled".to_string()
    }

    /// Extract meta description as abstract.
    fn extract_abstract(html: &str) -> String {
        if let Some(pos) = html.find(r#"name="description""#) {
            let after = &html[pos..];
            if let Some(content_start) = after.find(r#"content=""#) {
                let cs = pos + content_start + 9;
                if let Some(end) = html[cs..].find('"') {
                    return html[cs..cs + end].to_string();
                }
            }
        }
        String::new()
    }

    /// Extract sections from HTML by splitting on heading tags.
    /// Each <h1>/<h2>/<h3> starts a new section; text between headings
    /// becomes the section body.
    ///
    /// Section identity is derived deterministically from the actual
    /// h1/h2/h3 stack (ADR-053 Layer A navigation fields):
    ///
    /// - `path_id` = dotted tree path ("1", "1.2", "1.2.1", ...): a heading
    ///   opens a child of the nearest still-open heading with a strictly
    ///   lower tag level; siblings get 1-based ordinals per parent.
    /// - `parent_section_id` = `sec-<parent path>` (None for root sections).
    /// - `order_in_parent` = 0-based sibling index within the parent.
    /// - `section_id` = `sec-<path_id>` (same convention as GrobidParser).
    /// - `level` = dotted depth (matches the navigation validator's
    ///   `level_matches_dotted_depth` contract; GROBID uses the same rule).
    ///
    /// Content before the first heading (preamble) is retained as a
    /// root-level section with derived identity, but it never opens a tree
    /// node — later headings are its siblings, not its children. No heading
    /// or section is created without source evidence, and repeated paths are
    /// resolved with the deterministic `~N` restart convention (dot-free
    /// suffixes never become dotted parents).
    fn extract_sections(html: &str) -> Vec<Section> {
        let chunks = Self::split_heading_chunks(html);
        let mut sections = Vec::with_capacity(chunks.len());
        // Heading stack: (tag_level, path_id) of open heading sections.
        let mut stack: Vec<(u32, String)> = Vec::new();
        // Next 1-based sibling ordinal per parent path ("" = root).
        let mut next_ordinal: HashMap<String, u32> = HashMap::new();

        for chunk in chunks {
            let path = if chunk.tag_level == 0 {
                // Preamble: root-level sibling that never opens a tree node.
                let ordinal = next_ordinal.entry(String::new()).or_insert(0);
                *ordinal += 1;
                ordinal.to_string()
            } else {
                // Close every open heading at or below the new heading's
                // rank (an h2 closes an open h2/h3 subtree, an h1 closes
                // everything).
                while stack
                    .last()
                    .is_some_and(|(level, _)| *level >= chunk.tag_level)
                {
                    stack.pop();
                }
                let parent_path = stack
                    .last()
                    .map(|(_, path)| path.clone())
                    .unwrap_or_default();
                let ordinal = next_ordinal.entry(parent_path.clone()).or_insert(0);
                *ordinal += 1;
                let path = if parent_path.is_empty() {
                    ordinal.to_string()
                } else {
                    format!("{parent_path}.{ordinal}")
                };
                stack.push((chunk.tag_level, path.clone()));
                path
            };
            sections.push(Section {
                title: chunk.title,
                text: chunk.text,
                level: 0, // set from the path tree in populate_parent_and_order
                path_id: Some(path.clone()),
                parent_section_id: None,
                order_in_parent: None,
                section_id: None,
            });
        }

        // Defensive uniqueness: the stack derivation is collision-free by
        // construction (per-parent ordinals), but keep the deterministic
        // `~N` restart convention so any pathological duplicate still
        // resolves deterministically with descendants remapped to the
        // resolved parent prefix.
        Self::uniquify_heading_paths(&mut sections);
        // Populate parent_section_id / order_in_parent / section_id / level
        // from the now-unique path tree.
        Self::populate_parent_and_order(&mut sections);
        sections
    }

    /// Split the HTML main content into heading-delimited chunks.
    ///
    /// Each chunk carries the heading tag level that opened it (1/2/3) plus
    /// the accumulated title and body text. Content before the first heading
    /// is one chunk with `tag_level = 0` (preamble). h4-h6 tags are not
    /// heading boundaries — they stay inside the enclosing section's text
    /// (only h1/h2/h3 form the section tree).
    fn split_heading_chunks(html: &str) -> Vec<HeadingChunk> {
        // Focus on the main content area if present
        let content = html
            .find("<article")
            .or_else(|| html.find(r#"<div class="md-content""#))
            .map(|pos| &html[pos..])
            .unwrap_or(html);

        let content_end = content
            .find("</article>")
            .or_else(|| content.find(r#"<footer"#))
            .unwrap_or(content.len());
        let content = &content[..content_end];

        let mut chunks = Vec::new();
        let mut current_title = String::new();
        let mut current_tag_level = 0u32;
        let mut current_text = String::new();
        let mut heading_seen = false;
        let mut tag_buffer = String::new();
        let mut in_tag = false;

        for c in content.chars() {
            if c == '<' {
                in_tag = true;
                tag_buffer.clear();
                tag_buffer.push(c);
            } else if c == '>' {
                in_tag = false;
                tag_buffer.push(c);
                let tag = tag_buffer.as_str();

                // Check for heading start
                if tag.starts_with("<h1") || tag.starts_with("<h2") || tag.starts_with("<h3") {
                    if !current_text.is_empty() || !current_title.is_empty() {
                        let text = Self::strip_tags(&current_text).trim().to_string();
                        if !text.is_empty() || !current_title.is_empty() {
                            chunks.push(HeadingChunk {
                                tag_level: current_tag_level,
                                title: current_title.clone(),
                                text,
                            });
                        }
                    }
                    current_text.clear();
                    current_tag_level = if tag.starts_with("<h1") {
                        1
                    } else if tag.starts_with("<h2") {
                        2
                    } else {
                        3
                    };
                    heading_seen = true;
                } else if tag.starts_with("</h1")
                    || tag.starts_with("</h2")
                    || tag.starts_with("</h3")
                {
                    current_title = Self::strip_tags(&current_text).trim().to_string();
                    current_text.clear();
                }
            } else if in_tag {
                tag_buffer.push(c);
            } else {
                current_text.push(c);
            }
        }

        // Don't forget the last chunk
        let text = Self::strip_tags(&current_text).trim().to_string();
        if !text.is_empty() || !current_title.is_empty() {
            chunks.push(HeadingChunk {
                tag_level: if heading_seen { current_tag_level } else { 0 },
                title: current_title,
                text,
            });
        }
        chunks
    }

    /// Guarantee unique `path_id`s with the deterministic `~N` restart
    /// convention (same semantics as GrobidParser::uniquify_flat_paths): a
    /// repeated path becomes `<path>~2`, `<path>~3`, ... in document order,
    /// and descendants of a resolved parent are remapped to the resolved
    /// prefix so a dot-free suffix never becomes a dotted parent.
    fn uniquify_heading_paths(sections: &mut [Section]) {
        let mut used = HashSet::new();
        let mut latest_resolved: HashMap<String, String> = HashMap::new();
        for section in sections.iter_mut() {
            let Some(raw_path) = section.path_id.clone() else {
                continue;
            };
            let candidate = match raw_path.rsplit_once('.') {
                Some((raw_parent, component)) => latest_resolved
                    .get(raw_parent)
                    .map(|parent| format!("{parent}.{component}"))
                    .unwrap_or_else(|| raw_path.clone()),
                None => raw_path.clone(),
            };
            let mut resolved = candidate.clone();
            let mut collision = 2u32;
            while used.contains(&resolved) {
                resolved = format!("{candidate}~{collision}");
                collision += 1;
            }
            used.insert(resolved.clone());
            latest_resolved.insert(raw_path.clone(), resolved.clone());
            if resolved != raw_path {
                section.path_id = Some(resolved);
            }
        }
    }

    /// Populate `parent_section_id`, `order_in_parent`, `section_id`, and
    /// `level` for each Section from the path_id tree. Mutates `sections` in
    /// place. No-op for sections without a path_id.
    fn populate_parent_and_order(sections: &mut [Section]) {
        // Track 0-based sibling counters per parent path.
        let mut sibling_counters: HashMap<String, u32> = HashMap::new();
        for section in sections.iter_mut() {
            let Some(path_id) = section.path_id.clone() else {
                continue;
            };
            // parent_path = everything before the last '.'. Dot-free `~N`
            // restart suffixes never introduce a parent level.
            let parent_path = path_id.rfind('.').map(|pos| &path_id[..pos]);
            let counter_key = parent_path.unwrap_or("").to_string();
            let order = sibling_counters.entry(counter_key).or_insert(0);
            section.order_in_parent = Some(*order);
            *order += 1;
            if let Some(pp) = parent_path {
                section.parent_section_id = Some(format!("sec-{pp}"));
            }
            // section_id: sec-<path_id> (same convention as GrobidParser).
            section.section_id = Some(format!("sec-{path_id}"));
            // level = dotted depth (matches GrobidParser and the navigation
            // validator's level_matches_dotted_depth contract).
            section.level = path_id.split('.').count() as u32;
        }
    }

    /// Strip HTML tags from text (same approach as GrobidParser).
    fn strip_tags(s: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for c in s.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(c);
            }
        }
        result
    }

    /// Compute a simple hash for deduplication (not cryptographic).
    fn simple_hash(s: &str) -> String {
        let mut hash: u64 = 5381;
        for b in s.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u64);
        }
        format!("{:016x}", hash)
    }
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ParserPort for HtmlParser {
    async fn parse_pdf(&self, _pdf_path: &str, _paper_id: &str) -> ParseResult<ParsedArticle> {
        Err(ParserError::Unavailable(
            "HtmlParser does not support PDF".to_string(),
        ))
    }

    async fn parse_html(&self, html_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        let html = std::fs::read_to_string(html_path)
            .map_err(|e| ParserError::ParseFailed(format!("Cannot read HTML: {e}")))?;

        let title = Self::extract_title(&html);
        let abstract_text = Self::extract_abstract(&html);
        let sections = Self::extract_sections(&html);
        let body_text: String = sections
            .iter()
            .map(|s| s.text.clone())
            .collect::<Vec<_>>()
            .join("\n\n");

        tracing::info!(
            paper_id,
            sections = sections.len(),
            body_chars = body_text.len(),
            "HTML parsed"
        );

        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title,
            abstract_text,
            body_text,
            sections,
            citations: Vec::new(),
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            // S07 P0 families (M464-z84in2): the HTML path carries no TEI
            // header/figures — typed emptiness, never invented here.
            declared_arxiv_id: None,
            declared_id_missingness: None,
            header_authors: Vec::new(),
            figures: Vec::new(),
            visual_callouts: Vec::new(),
            page_text_missingness: None,
            pdf_hash: Self::simple_hash(&html),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>Chapter 1: Graphs</title></head><body></body></html>";
        assert_eq!(HtmlParser::extract_title(html), "Chapter 1: Graphs");
    }

    #[test]
    fn test_extract_abstract() {
        let html = r#"<meta name="description" content="Introduces graph theory basics">"#;
        assert_eq!(
            HtmlParser::extract_abstract(html),
            "Introduces graph theory basics"
        );
    }

    #[test]
    fn test_extract_sections() {
        let html = r#"
        <article>
        <h1>Chapter 1</h1>
        <p>Intro text here.</p>
        <h2>Section A</h2>
        <p>Section A content.</p>
        <h2>Section B</h2>
        <p>Section B content.</p>
        </article>
        "#;
        let sections = HtmlParser::extract_sections(html);
        assert!(!sections.is_empty());
        // Should have at least 3 sections (h1 + 2× h2)
        assert!(
            sections.len() >= 3,
            "expected 3+ sections, got {}",
            sections.len()
        );
    }

    #[test]
    fn test_extract_sections_heading_stack_paths() {
        let html = r#"
        <article>
        <h1>Chapter 1</h1>
        <p>Intro text here.</p>
        <h2>Section A</h2>
        <p>Section A content.</p>
        <h2>Section B</h2>
        <p>Section B content.</p>
        <h3>Sub B.1</h3>
        <p>Sub content.</p>
        </article>
        "#;
        let sections = HtmlParser::extract_sections(html);
        let paths: Vec<&str> = sections
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "1.1", "1.2", "1.2.1"]);
        // section_id: sec-<path_id> (same convention as GrobidParser).
        let ids: Vec<&str> = sections
            .iter()
            .map(|s| s.section_id.as_deref().unwrap())
            .collect();
        assert_eq!(ids, vec!["sec-1", "sec-1.1", "sec-1.2", "sec-1.2.1"]);
        // parent_section_id: None for roots, sec-<parent path> otherwise.
        assert_eq!(sections[0].parent_section_id, None);
        assert_eq!(sections[1].parent_section_id.as_deref(), Some("sec-1"));
        assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-1"));
        assert_eq!(sections[3].parent_section_id.as_deref(), Some("sec-1.2"));
        // order_in_parent: 0-based sibling index per parent.
        assert_eq!(sections[0].order_in_parent, Some(0));
        assert_eq!(sections[1].order_in_parent, Some(0));
        assert_eq!(sections[2].order_in_parent, Some(1));
        assert_eq!(sections[3].order_in_parent, Some(0));
        // level = dotted depth (navigation validator contract).
        assert_eq!(sections[0].level, 1);
        assert_eq!(sections[1].level, 2);
        assert_eq!(sections[2].level, 2);
        assert_eq!(sections[3].level, 3);
    }

    #[test]
    fn test_extract_sections_preamble_is_root_sibling() {
        let html = r#"
        <article>
        <p>Preamble text before any heading.</p>
        <h1>Chapter</h1>
        <p>Body.</p>
        <h2>Sub</h2>
        <p>Sub body.</p>
        </article>
        "#;
        let sections = HtmlParser::extract_sections(html);
        let paths: Vec<&str> = sections
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "2", "2.1"]);
        // Preamble is a root sibling: the h2 belongs to the h1 ("2"), not
        // to the preamble ("1").
        assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-2"));
        assert_eq!(sections[1].section_id.as_deref(), Some("sec-2"));
        assert_eq!(sections[0].parent_section_id, None);
        assert_eq!(sections[0].level, 1);
        assert!(!sections[0].text.is_empty());
        assert!(sections[0].title.is_empty());
    }

    #[test]
    fn test_extract_sections_level_skip_is_child_not_gap() {
        // An h3 directly under an h1 is a child of the h1 (skipped h2 does
        // not create a phantom level); a later h2 closes the h3 subtree and
        // becomes a sibling of it under the same parent.
        let html = r#"
        <article>
        <h1>Root</h1>
        <h3>Deep</h3>
        <h2>Back</h2>
        </article>
        "#;
        let sections = HtmlParser::extract_sections(html);
        let paths: Vec<&str> = sections
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "1.1", "1.2"]);
        assert_eq!(sections[1].parent_section_id.as_deref(), Some("sec-1"));
        assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-1"));
        assert_eq!(sections[1].level, 2);
        assert_eq!(sections[2].level, 2);
    }

    #[test]
    fn test_extract_sections_no_heading_document() {
        let html = "<article><p>All text, no headings.</p></article>";
        let sections = HtmlParser::extract_sections(html);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].path_id.as_deref(), Some("1"));
        assert_eq!(sections[0].section_id.as_deref(), Some("sec-1"));
        assert_eq!(sections[0].parent_section_id, None);
        assert_eq!(sections[0].order_in_parent, Some(0));
        assert_eq!(sections[0].level, 1);
    }

    #[test]
    fn test_extract_sections_deterministic_paths() {
        let html = r#"
        <article>
        <h1>Chapter</h1>
        <h2>A</h2>
        <h3>A.1</h3>
        <h3>A.2</h3>
        <h2>B</h2>
        <h1>Chapter 2</h1>
        <h2>C</h2>
        </article>
        "#;
        let first = HtmlParser::extract_sections(html);
        let second = HtmlParser::extract_sections(html);
        let identity = |s: &Section| {
            (
                s.path_id.clone(),
                s.section_id.clone(),
                s.parent_section_id.clone(),
                s.order_in_parent,
                s.level,
            )
        };
        let a: Vec<_> = first.iter().map(identity).collect();
        let b: Vec<_> = second.iter().map(identity).collect();
        assert_eq!(a, b);
        let paths: Vec<&str> = first
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        // New h1 restarts the root numbering; h2/h3 restart under it.
        assert_eq!(paths, vec!["1", "1.1", "1.1.1", "1.1.2", "1.2", "2", "2.1"]);
    }

    #[test]
    fn test_extract_sections_ignores_h4_and_heading_attributes() {
        // h4-h6 are not section boundaries and heading attributes (id, ...)
        // do not affect identity: only the actual h1/h2/h3 stack matters.
        let html = r#"
        <article>
        <h1 id="chapter-1">Chapter</h1>
        <p>text</p>
        <h2 id="sec-1">Section</h2>
        <h4 id="diagram">Diagram heading is not a section</h4>
        <p>diagram text</p>
        <h3 id="sec-1-1">Sub</h3>
        </article>
        "#;
        let sections = HtmlParser::extract_sections(html);
        let paths: Vec<&str> = sections
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "1.1", "1.1.1"]);
        // The h4 heading text stays inside the enclosing h2 section instead
        // of fabricating a new section.
        assert!(
            sections[1]
                .text
                .contains("Diagram heading is not a section")
        );
    }

    #[test]
    fn test_uniquify_heading_paths_restart_suffix_is_not_parent() {
        // Pathological duplicate paths (defensive path): the deterministic
        // `~N` restart convention resolves them in document order, and a
        // dot-free suffix never becomes a dotted parent for descendants.
        let mut sections = vec![
            Section {
                title: "First".into(),
                path_id: Some("1".into()),
                ..Default::default()
            },
            Section {
                title: "Repeat".into(),
                path_id: Some("1".into()),
                ..Default::default()
            },
            Section {
                title: "Child of repeat".into(),
                path_id: Some("1.1".into()),
                ..Default::default()
            },
        ];
        HtmlParser::uniquify_heading_paths(&mut sections);
        let paths: Vec<&str> = sections
            .iter()
            .map(|s| s.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "1~2", "1~2.1"]);

        HtmlParser::populate_parent_and_order(&mut sections);
        // The suffix is dot-free: "1~2" is a root (no parent), and its
        // child's parent is "1~2", never "1".
        assert_eq!(sections[1].section_id.as_deref(), Some("sec-1~2"));
        assert_eq!(sections[1].parent_section_id, None);
        assert_eq!(sections[1].level, 1);
        assert_eq!(sections[2].section_id.as_deref(), Some("sec-1~2.1"));
        assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-1~2"));
        assert_eq!(sections[2].level, 2);
        // Sibling order is per resolved parent path.
        assert_eq!(sections[0].order_in_parent, Some(0));
        assert_eq!(sections[1].order_in_parent, Some(1));
        assert_eq!(sections[2].order_in_parent, Some(0));
    }

    #[test]
    fn test_strip_tags() {
        assert_eq!(
            HtmlParser::strip_tags("<b>Hello</b> <i>World</i>"),
            "Hello World"
        );
    }

    #[tokio::test]
    async fn test_parse_html_returns_parsed_article() {
        let dir = std::env::temp_dir().join("test_html_parser.html");
        std::fs::write(
            &dir,
            r#"<html><head><title>Test Chapter</title>
            <meta name="description" content="Test abstract">
            </head><body><article>
            <h1>Test Chapter</h1>
            <p>This is a test paragraph about GCN.</p>
            <h2>Section 1</h2>
            <p>Graph Convolutional Networks are powerful.</p>
            </article></body></html>"#,
        )
        .unwrap();

        let parser = HtmlParser::new();
        let result = parser.parse_html(dir.to_str().unwrap(), "test-001").await;
        assert!(result.is_ok());
        let article = result.unwrap();
        assert_eq!(article.paper_id, "test-001");
        assert_eq!(article.title, "Test Chapter");
        assert_eq!(article.abstract_text, "Test abstract");
        assert!(!article.sections.is_empty());
        assert!(article.body_text.contains("GCN"));
    }
}
