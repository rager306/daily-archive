//! GROBID parser adapter — implements ParserPort.
//!
//! ADR-037 §4.1: GROBID HTTP service for scholarly PDF parsing.
//! GROBID runs at http://127.0.0.1:8070 (Java service, no API key).

use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort};

const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";

/// GROBID HTTP adapter for PDF → scholarly structure (TEI XML).
#[derive(Clone)]
pub struct GrobidParser {
    url: String,
    client: reqwest::Client,
}

impl GrobidParser {
    pub fn new(url: Option<&str>) -> Self {
        Self {
            url: url.unwrap_or(DEFAULT_GROBID_URL).to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("GROBID_URL").unwrap_or_else(|_| DEFAULT_GROBID_URL.to_string());
        Self::new(Some(&url))
    }

    /// Check if GROBID is alive.
    pub async fn is_alive(&self) -> bool {
        let url = format!("{}/api/isalive", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Process PDF via GROBID full-text endpoint → TEI XML.
    async fn process_fulltext(&self, pdf_bytes: &[u8]) -> ParseResult<String> {
        let url = format!("{}/api/processFulltextDocument", self.url);
        let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|e| ParserError::ParseFailed(e.to_string()))?;

        let form = reqwest::multipart::Form::new().part("input", part);

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ParserError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ParserError::ParseFailed(format!(
                "GROBID HTTP {}",
                resp.status()
            )));
        }

        let tei_xml = resp
            .text()
            .await
            .map_err(|e| ParserError::ParseFailed(e.to_string()))?;

        Ok(tei_xml)
    }

    /// Extract title from TEI XML.
    /// Handles both `<title>Text</title>` and `<title level="a" type="main">Text</title>`.
    fn extract_title(tei: &str) -> String {
        if let Some(start) = tei.find("<title") {
            // Find the closing > of the opening tag (may have attributes)
            if let Some(tag_end) = tei[start..].find('>') {
                let after = &tei[start + tag_end + 1..];
                if let Some(end) = after.find("</title>") {
                    return Self::strip_xml_tags(&after[..end]);
                }
            }
        }
        String::new()
    }

    /// Extract abstract from TEI XML.
    fn extract_abstract(tei: &str) -> String {
        if let Some(start) = tei.find("<abstract") {
            if let Some(content_start) = tei[start..].find('>') {
                let after = &tei[start + content_start + 1..];
                if let Some(end) = after.find("</abstract>") {
                    // Strip nested tags
                    return Self::strip_xml_tags(&after[..end]);
                }
            }
        }
        String::new()
    }

    /// Extract body text from TEI XML.
    fn extract_body(tei: &str) -> String {
        if let Some(start) = tei.find("<body") {
            if let Some(content_start) = tei[start..].find('>') {
                let after = &tei[start + content_start + 1..];
                if let Some(end) = after.find("</body>") {
                    return Self::strip_xml_tags(&after[..end]);
                }
            }
        }
        String::new()
    }

    /// Extract sections from TEI body.
    /// GROBID structures body as nested `<div><head n="1">Title</head><p>...</p></div>`.
    /// Level is derived from the `n` attribute on `<head>` (1, 2, 3...).
    fn extract_sections(tei: &str) -> Vec<da_ports::parser::Section> {
        use da_ports::parser::Section;
        let body = match tei.find("<body") {
            Some(start) => {
                let after = &tei[start..];
                match after.find('>') {
                    Some(tag_end) => {
                        let rest = &after[tag_end + 1..];
                        match rest.find("</body>") {
                            Some(end) => &rest[..end],
                            None => return Vec::new(),
                        }
                    }
                    None => return Vec::new(),
                }
            }
            None => return Vec::new(),
        };

        let mut sections = Vec::new();
        let mut pos = 0;
        while let Some(head_start) = body[pos..].find("<head") {
            let abs = pos + head_start;
            // Find end of opening <head ...> tag
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            // Extract level from n="..." attribute
            let open_tag = &body[abs..=tag_end];
            let level = Self::extract_head_level(open_tag);
            // Find closing </head>
            let content_start = tag_end + 1;
            let head_close = match body[content_start..].find("</head>") {
                Some(e) => content_start + e,
                None => break,
            };
            let title = Self::strip_xml_tags(&body[content_start..head_close]);
            // Section text: from after </head> to next <head> or end of body
            let after_head = head_close + "</head>".len();
            let next_head = body[after_head..].find("<head").map(|e| after_head + e);
            let text_end = next_head.unwrap_or(body.len());
            let text = Self::strip_xml_tags(&body[after_head..text_end]);
            if !title.is_empty() || !text.is_empty() {
                sections.push(Section { title, text, level });
            }
            pos = text_end;
            if pos <= abs {
                pos = abs + 1;
            }
        }
        sections
    }

    /// Extract the `n` attribute value from a `<head n="1">` tag.
    fn extract_head_level(open_tag: &str) -> u32 {
        if let Some(n_pos) = open_tag.find("n=\"") {
            let after = &open_tag[n_pos + 3..];
            if let Some(end) = after.find('"') {
                return after[..end].parse().unwrap_or(1);
            }
        }
        1
    }

    /// Extract citations from TEI `<back>` references.
    /// GROBID places references in `<div type="references"><listBibl><biblStruct>`.
    fn extract_citations(tei: &str) -> Vec<da_ports::parser::CitationEntry> {
        use da_ports::parser::CitationEntry;
        let back = match tei.find("<back>") {
            Some(s) => match tei[s..].find("</back>") {
                Some(e) => &tei[s..s + e],
                None => return Vec::new(),
            },
            None => return Vec::new(),
        };

        let mut citations = Vec::new();
        let mut pos = 0;
        while let Some(bibl_start) = back[pos..].find("<biblStruct") {
            let abs = pos + bibl_start;
            // Find end of opening <biblStruct ...> tag (may have attributes)
            let tag_end = match back[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let bibl_end = match back[tag_end..].find("</biblStruct>") {
                Some(e) => tag_end + e + "</biblStruct>".len(),
                None => break,
            };
            let entry = &back[tag_end..bibl_end];

            // Title: first <title level="a" type="main"> inside <analytic>
            let title = Self::extract_citation_title(entry);
            // DOI: <idno type="DOI">
            let doi = Self::extract_idno(entry, "DOI");
            // arxiv: <idno type="arXiv">
            let arxiv_id = Self::extract_idno(entry, "arXiv").map(|s| {
                // strip "arXiv:" prefix and version suffix
                s.strip_prefix("arXiv:")
                    .unwrap_or(&s)
                    .split('v')
                    .next()
                    .unwrap_or(&s)
                    .to_string()
            });
            let raw_text = Self::strip_xml_tags(entry);

            citations.push(CitationEntry {
                raw_text,
                doi,
                arxiv_id,
                title,
            });
            pos = bibl_end;
        }
        citations
    }

    /// Extract the analytic title from a biblStruct entry.
    fn extract_citation_title(entry: &str) -> Option<String> {
        let analytic = entry
            .find("<analytic>")
            .and_then(|s| entry[s..].find("</analytic>").map(|e| &entry[s..s + e]))?;
        if let Some(t_start) = analytic.find("<title") {
            if let Some(tag_end) = analytic[t_start..].find('>') {
                let content = &analytic[t_start + tag_end + 1..];
                if let Some(end) = content.find("</title>") {
                    let title = Self::strip_xml_tags(&content[..end]);
                    if !title.is_empty() {
                        return Some(title);
                    }
                }
            }
        }
        None
    }

    /// Extract an <idno type="...">value</idno> from a TEI fragment.
    fn extract_idno(entry: &str, idno_type: &str) -> Option<String> {
        let needle = format!("type=\"{}\"", idno_type);
        let type_pos = entry.find(&needle)?;
        // Find the <idno ...> opening tag containing this attribute
        let tag_start = entry[..type_pos].rfind("<idno")?;
        let tag_end = entry[tag_start..].find('>')? + tag_start;
        let content = &entry[tag_end + 1..];
        let close = content.find("</idno>")?;
        let value = Self::strip_xml_tags(&content[..close]);
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// Strip XML tags from text.
    fn strip_xml_tags(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_tag = false;
        for ch in s.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }
        result.trim().to_string()
    }

    /// Extract SHA256 hash of PDF bytes.
    fn pdf_hash(pdf_bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pdf_bytes);
        hex::encode(hasher.finalize())
    }
}

#[async_trait]
impl ParserPort for GrobidParser {
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        tracing::info!(paper_id, pdf_path, "GROBID parsing");

        // Read PDF bytes
        let pdf_bytes = tokio::fs::read(pdf_path)
            .await
            .map_err(|e| ParserError::ParseFailed(format!("read PDF: {e}")))?;

        // Process via GROBID
        let tei_xml = self.process_fulltext(&pdf_bytes).await?;

        // Extract structured content
        let title = Self::extract_title(&tei_xml);
        let abstract_text = Self::extract_abstract(&tei_xml);
        let body_text = Self::extract_body(&tei_xml);
        let sections = Self::extract_sections(&tei_xml);
        let citations = Self::extract_citations(&tei_xml);
        let hash = Self::pdf_hash(&pdf_bytes);

        tracing::info!(
            paper_id,
            title = %title,
            body_chars = body_text.len(),
            sections = sections.len(),
            citations = citations.len(),
            "GROBID parsed"
        );

        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title,
            abstract_text,
            body_text,
            sections,
            citations,
            layout_json: None, // ODL provides this separately
            tei_xml: Some(tei_xml),
            pdf_hash: hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let tei = r#"<TEI><teiHeader><fileDesc><titleStmt><title>Seq2Seq Models</title></titleStmt></fileDesc></teiHeader></TEI>"#;
        assert_eq!(GrobidParser::extract_title(tei), "Seq2Seq Models");
    }

    #[test]
    fn test_extract_title_with_attributes() {
        // GROBID TEI uses attributes: <title level="a" type="main">
        let tei =
            r#"<title level="a" type="main">A Joint Model of Language and Perception</title>"#;
        assert_eq!(
            GrobidParser::extract_title(tei),
            "A Joint Model of Language and Perception"
        );
    }

    #[test]
    fn test_strip_xml_tags() {
        let s = "<p>Hello <ref>world</ref></p>";
        assert_eq!(GrobidParser::strip_xml_tags(s), "Hello world");
    }

    #[test]
    fn test_pdf_hash() {
        let bytes = b"test pdf content";
        let hash = GrobidParser::pdf_hash(bytes);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_extract_sections() {
        let tei = r#"<TEI><text><body>
            <div><head n="1">INTRODUCTION</head><p>Intro text here.</p></div>
            <div><head n="2">Methods</head><p>Method details.</p></div>
        </body></text></TEI>"#;
        let sections = GrobidParser::extract_sections(tei);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "INTRODUCTION");
        assert_eq!(sections[0].level, 1);
        assert!(sections[0].text.contains("Intro text"));
        assert_eq!(sections[1].title, "Methods");
        assert_eq!(sections[1].level, 2);
    }

    #[test]
    fn test_extract_sections_no_body() {
        let tei = r#"<TEI><teiHeader><title>No body</title></teiHeader></TEI>"#;
        let sections = GrobidParser::extract_sections(tei);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_citations() {
        let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct><analytic><title level="a" type="main">First Citation</title></analytic>
            <idno type="DOI">10.1000/cite.1</idno></biblStruct>
            <biblStruct><analytic><title level="a" type="main">Second Citation</title></analytic>
            <idno type="arXiv">arXiv:2001.00002v1</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].title.as_deref(), Some("First Citation"));
        assert_eq!(citations[0].doi.as_deref(), Some("10.1000/cite.1"));
        assert_eq!(citations[1].title.as_deref(), Some("Second Citation"));
        assert_eq!(citations[1].arxiv_id.as_deref(), Some("2001.00002"));
    }

    #[test]
    fn test_extract_citations_no_back() {
        let tei = r#"<TEI><text><body><p>no refs</p></body></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert!(citations.is_empty());
    }

    #[test]
    fn test_extract_citations_with_attributes() {
        // Real GROBID output: <biblStruct status="extracted" xml:id="b0">
        let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct status="extracted" xml:id="b0"><analytic>
            <title level="a" type="main">Cited With Attrs</title></analytic>
            <idno type="DOI">10.2000/x</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].title.as_deref(), Some("Cited With Attrs"));
        assert_eq!(citations[0].doi.as_deref(), Some("10.2000/x"));
    }

    #[test]
    fn test_extract_head_level() {
        assert_eq!(GrobidParser::extract_head_level(r#"<head n="3">"#), 3);
        assert_eq!(GrobidParser::extract_head_level("<head>"), 1);
        assert_eq!(GrobidParser::extract_head_level(r#"<head n="abc">"#), 1);
    }
}

#[cfg(test)]
mod real_tei_tests {
    use super::*;
    #[test]
    fn test_real_grobid_tei_sections_and_citations() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/grobid.tei.xml"
        );
        let tei = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("skipping real TEI test (file not found)");
                return;
            }
        };
        let sections = GrobidParser::extract_sections(&tei);
        let citations = GrobidParser::extract_citations(&tei);
        println!(
            "real TEI: {} sections, {} citations",
            sections.len(),
            citations.len()
        );
        assert!(
            sections.len() > 10,
            "expected many sections, got {}",
            sections.len()
        );
        assert!(
            citations.len() > 5,
            "expected many citations, got {}",
            citations.len()
        );
        // First section should be INTRODUCTION
        assert_eq!(sections[0].title, "INTRODUCTION");
    }
}
