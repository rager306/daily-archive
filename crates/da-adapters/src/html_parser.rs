//! HTML parser adapter for textbook/lecture HTML content.
//!
//! Implements ParserPort for Material for MkDocs / standard HTML.
//! Used for GNN textbook chapters and other non-PDF sources.
//! No external HTML parsing crate — uses string-based extraction
//! (same approach as GrobidParser::extract_sections).

use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort, Section};

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
    fn extract_sections(html: &str) -> Vec<Section> {
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

        let mut sections = Vec::new();
        let mut current_title = String::new();
        let mut current_level = 1u32;
        let mut current_text = String::new();
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
                            sections.push(Section {
                                title: current_title.clone(),
                                text,
                                level: current_level,
                            });
                        }
                    }
                    current_text.clear();
                    current_level = if tag.starts_with("<h1") {
                        1
                    } else if tag.starts_with("<h2") {
                        2
                    } else {
                        3
                    };
                } else if tag.starts_with("</h") {
                    current_title = Self::strip_tags(&current_text).trim().to_string();
                    current_text.clear();
                }
            } else if in_tag {
                tag_buffer.push(c);
            } else {
                current_text.push(c);
            }
        }

        // Don't forget the last section
        let text = Self::strip_tags(&current_text).trim().to_string();
        if !text.is_empty() || !current_title.is_empty() {
            sections.push(Section {
                title: current_title,
                text,
                level: current_level,
            });
        }

        // Filter out empty sections
        sections.retain(|s| !s.text.is_empty() || !s.title.is_empty());
        sections
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
