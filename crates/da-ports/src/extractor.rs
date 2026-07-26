//! Extractor port — text sections → entities.
//!
//! ADR-038 Module B: extract textually mentioned entities from paper sections.
//! Phase 3: rule-based first, GLiNER 2 later (optional).

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("Extraction failed: {0}")]
    ExtractFailed(String),
}

pub type ExtractResult<T> = Result<T, ExtractorError>;

/// An extracted entity with its source location.
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    pub label: String,
    pub entity_type: da_domain::entity::EntityType,
    pub section_title: String,
    pub char_start: usize,
    pub char_end: usize,
    pub surface: String,
}

/// Extractor port — sections → entities.
/// Rule-based extractor implements this first (Phase 3 Slice 1).
/// GLiNER 2 adapter implements this later (Phase 3 Slice 3).
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Extract entities from a list of (section_title, section_text) pairs.
    async fn extract(
        &self,
        sections: &[(String, String)], // (title, text)
    ) -> ExtractResult<Vec<ExtractedEntity>>;

    /// Extractor name (e.g. "rule-based", "gliner2").
    fn name(&self) -> &str;
}
