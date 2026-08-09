//! Driven boundary for candidate-only TeX archive analysis.

use da_domain::{ContentDigest, ProviderDenominator, TexSourceAnalysisReport};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexArchiveMember {
    pub path: String,
    pub byte_size: u64,
    pub sha256: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexArchiveInventory {
    pub article_id: String,
    pub archive_sha256: ContentDigest,
    pub members: Vec<TexArchiveMember>,
}

#[derive(Debug, Error)]
pub enum TexArchiveError {
    #[error("unsafe archive member: {0}")]
    UnsafeMember(String),
    #[error("archive member missing: {0}")]
    MissingMember(String),
    #[error("archive member checksum mismatch: {0}")]
    ChecksumMismatch(String),
    #[error("archive I/O failure: {0}")]
    Io(String),
}

pub trait TexArchivePort: Send + Sync {
    fn inventory(&self) -> Result<TexArchiveInventory, TexArchiveError>;
    fn read_members(&self, paths: &[String]) -> Result<BTreeMap<String, Vec<u8>>, TexArchiveError>;
}

pub trait TexAnalysisReportStore: Send + Sync {
    fn store(&self, report: &TexSourceAnalysisReport) -> Result<(), TexArchiveError>;
}

#[derive(Debug, Clone)]
pub struct TexAnalysisRequest {
    pub providers: Vec<ProviderDenominator>,
}
