//! GROBID parser adapter — implements ParserPort.
//!
//! ADR-037 §4.1: GROBID HTTP service for scholarly PDF parsing.
//! GROBID runs at http://127.0.0.1:8070 (Java service, no API key).

use async_trait::async_trait;
use da_ports::parser::{ParserPort, ParserError, ParseResult, ParsedArticle, Section, CitationEntry};

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

        let form = reqwest::multipart::Form::new()
            .part("input", part);

        let resp = self.client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ParserError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ParserError::ParseFailed(format!("GROBID HTTP {}", resp.status())));
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
        use sha2::{Sha256, Digest};
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
        let hash = Self::pdf_hash(&pdf_bytes);

        tracing::info!(
            paper_id,
            title = %title,
            body_chars = body_text.len(),
            "GROBID parsed"
        );

        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title,
            abstract_text,
            body_text,
            sections: vec![],  // TODO: parse TEI sections
            citations: vec![], // TODO: parse TEI citations
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
        let tei = r#"<title level="a" type="main">A Joint Model of Language and Perception</title>"#;
        assert_eq!(GrobidParser::extract_title(tei), "A Joint Model of Language and Perception");
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
}
