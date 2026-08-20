//! Driven port for read-only article-catalog inventory.

use da_domain::catalog::{CatalogAssetDescriptor, CatalogFinding};

#[derive(Debug, thiserror::Error)]
pub enum CatalogInventoryError {
    #[error("catalog inventory failed: {0}")]
    Infrastructure(String),
}

pub trait CatalogInventoryPort: Send + Sync {
    fn inventory(&self) -> Result<CatalogInventorySnapshot, CatalogInventoryError>;
}

/// Locate an already-catalogued PDF for an arXiv id. Never invents a path:
/// `None` means the catalog has no `source/<id>.pdf` (or versioned sibling).
pub trait CatalogPdfLocator: Send + Sync {
    fn locate(&self, arxiv_id: &str) -> Option<std::path::PathBuf>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogWriteDisposition {
    Applied,
    Unchanged,
}

pub trait CatalogMutationPort: Send + Sync {
    fn read_metadata(&self, relative_path: &str) -> Result<Vec<u8>, CatalogInventoryError>;
    fn describe_assets(
        &self,
        article_root: &str,
    ) -> Result<Vec<CatalogAssetDescriptor>, CatalogInventoryError>;
    fn write_metadata_atomic_if_absent(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<CatalogWriteDisposition, CatalogInventoryError>;
    fn write_file_atomic(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<CatalogWriteDisposition, CatalogInventoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogInventorySnapshot {
    pub article_metadata_count: usize,
    pub pdf_count: usize,
    pub html_count: usize,
    pub canonical_identity_count: usize,
    pub findings: Vec<CatalogFinding>,
}
