//! OpenAlex client port — fetch curated scholarly metadata.
//!
//! D133: OpenAlex replaces noisy YAKE/extraction for the metadata layer.
//! Provides Works (title, DOI, topics, concepts), Authors (disambiguated,
//! ORCID), and citation data.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpenAlexError {
    #[error("OpenAlex API error: {0}")]
    Api(String),
    #[error("Work not found: {0}")]
    NotFound(String),
    #[error("Network error: {0}")]
    Network(String),
}

pub type OpenAlexResult<T> = Result<T, OpenAlexError>;

/// A topic from OpenAlex (domain → field → subfield → topic hierarchy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexTopic {
    pub id: String,
    pub display_name: String,
    pub domain: Option<String>,
    pub field: Option<String>,
    pub subfield: Option<String>,
}

/// An author from OpenAlex (disambiguated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexAuthor {
    pub id: String,
    pub display_name: String,
    pub orcid: Option<String>,
}

/// An institution from OpenAlex authorship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexInstitution {
    pub id: String,
    pub display_name: String,
    pub country_code: Option<String>,
    pub ror: Option<String>,
}

/// One OpenAlex authorship row — preserves the author↔institution association
/// that OpenAlex provides per authorship. Use this for wiring
/// Author → Institution edges. The flat `authors` and `institutions`
/// fields on OpenAlexWork are kept for backward compatibility but lose
/// this association.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexAuthorship {
    pub author: OpenAlexAuthor,
    pub institutions: Vec<OpenAlexInstitution>,
}

/// A concept from OpenAlex (deprecated, but kept for historical audit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexConcept {
    pub id: String,
    pub display_name: String,
    pub level: u32,
    pub score: f64,
}

/// Curated work metadata from OpenAlex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexWork {
    pub id: String,
    pub title: String,
    pub doi: Option<String>,
    pub publication_date: Option<String>,
    pub cited_by_count: u32,
    pub primary_topic: Option<OpenAlexTopic>,
    pub topics: Vec<OpenAlexTopic>,
    pub concepts: Vec<OpenAlexConcept>,
    pub authors: Vec<OpenAlexAuthor>,
    pub institutions: Vec<OpenAlexInstitution>,
    /// Per-authorship rows preserving the author↔institution association.
    /// Source of truth for wiring Author → Institution edges.
    pub authorships: Vec<OpenAlexAuthorship>,
    pub referenced_works: Vec<String>,
}

/// OpenAlex client port — fetches curated metadata by arXiv ID.
#[async_trait]
pub trait OpenAlexClient: Send + Sync {
    /// Fetch work metadata by arXiv ID.
    async fn fetch_by_arxiv_id(&self, arxiv_id: &str) -> OpenAlexResult<OpenAlexWork>;

    /// Search works by title/abstract keywords.
    async fn search(&self, query: &str, limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>>;
}
