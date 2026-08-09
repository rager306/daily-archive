//! Driven port for read-only article-catalog inventory.

use da_domain::catalog::CatalogFinding;

#[derive(Debug, thiserror::Error)]
pub enum CatalogInventoryError {
    #[error("catalog inventory failed: {0}")]
    Infrastructure(String),
}

pub trait CatalogInventoryPort: Send + Sync {
    fn inventory(&self) -> Result<CatalogInventorySnapshot, CatalogInventoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogInventorySnapshot {
    pub article_metadata_count: usize,
    pub pdf_count: usize,
    pub html_count: usize,
    pub canonical_identity_count: usize,
    pub findings: Vec<CatalogFinding>,
}
