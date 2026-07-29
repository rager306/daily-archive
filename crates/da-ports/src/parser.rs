//! Parser port — raw source → ParsedArticle.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Parse failed: {0}")]
    ParseFailed(String),
    #[error("Service unavailable: {0}")]
    Unavailable(String),
}

pub type ParseResult<T> = Result<T, ParserError>;

/// A parsed article from GROBID + ODL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedArticle {
    pub paper_id: String,
    pub title: String,
    pub abstract_text: String,
    pub body_text: String,
    pub sections: Vec<Section>,
    pub citations: Vec<CitationEntry>,
    pub layout_json: Option<serde_json::Value>,
    pub tei_xml: Option<String>,
    pub pdf_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub text: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationEntry {
    pub raw_text: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
}

/// Parser port — GROBID HTTP + HTML parser implement this.
#[async_trait]
pub trait ParserPort: Send + Sync {
    /// Parse a PDF file into a ParsedArticle (GROBID path).
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle>;

    /// Parse an HTML file into a ParsedArticle (textbook/lecture path).
    /// Default implementation returns Unavailable — adapters override.
    async fn parse_html(&self, _html_path: &str, _paper_id: &str) -> ParseResult<ParsedArticle> {
        Err(ParserError::Unavailable(
            "HTML parsing not supported by this parser".to_string(),
        ))
    }
}
