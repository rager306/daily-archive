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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Section {
    /// Display title of the section (from TEI `<head>`).
    pub title: String,
    /// Full text content of the section.
    pub text: String,
    /// Depth level (0 = root, 1 = top-level, ...). Matches TEI nesting depth.
    pub level: u32,

    // ─── ADR-053 Layer A navigation fields (M362-d4qoaf/S01 T01) ───
    // All optional for backward compat with parsers that do not populate them.
    // When present, enable document-tree navigation: path_id, parent_section_id,
    // order_in_parent, section_id. The first_child / next_sibling / parent_section
    // edges are created in the ingest use case (T04) from these fields.
    /// Structural path computed from TEI nested `<div n="N">` structure,
    /// e.g. "3.2.1". Stable across re-ingest of the same PDF.
    /// `None` for legacy parsers that produce flat section lists.
    #[serde(default)]
    pub path_id: Option<String>,

    /// `section_id` (not graph node id) of the parent section in the document
    /// tree. `None` for root sections or legacy flat lists.
    #[serde(default)]
    pub parent_section_id: Option<String>,

    /// 0-based sibling index within the parent. `None` for legacy flat lists.
    #[serde(default)]
    pub order_in_parent: Option<u32>,

    /// Stable hash of `path_id + title` — used as the graph node identifier
    /// and the parent_section_id reference target. `None` for legacy lists.
    #[serde(default)]
    pub section_id: Option<String>,
}

/// One in-text occurrence of a citation in the source document
/// (GROBID TEI `<ref type="bibr" target="#bN">`). Anchors are
/// revision-owned evidence: they are stored on PaperRevision CITES/HAS_PART
/// edges, never on the globally deduped Citation/Reference nodes (D247/D249),
/// so v1/v5 spans and cross-paper locations cannot mix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationAnchor {
    /// GROBID reference id — the `xml:id` of the cited `<biblStruct>`
    /// (e.g. "b12"), with any leading `#` from the `<ref target>` stripped.
    pub ref_id: String,
    /// `path_id` of the section containing the anchor (e.g. "3.2"), or `None`
    /// when the ref is inside the body but not within any section text window
    /// (typed missingness `anchor_section_path_unresolved` at ingest).
    pub section_path_id: Option<String>,
    /// In-text surface text of the ref (e.g. "[12]" or "(Shao et al., 2024)").
    pub quote: String,
    /// 0-based char offset of the quote within the containing section's
    /// stripped text (`Section.text`). `None` when there is no section text
    /// to anchor against.
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationEntry {
    pub raw_text: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    /// In-text anchors (GROBID `<ref type="bibr">` occurrences) mapped to
    /// this bibliography entry. Empty when the TEI has no resolvable in-text
    /// refs — ingest then records typed missingness `no_in_text_anchor` on
    /// the revision-owned edge. `#[serde(default)]` keeps legacy cached/fixture
    /// JSON decodable (R085).
    #[serde(default)]
    pub anchors: Vec<CitationAnchor>,
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
