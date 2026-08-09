//! Typed article-catalog reconciliation findings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFindingKind {
    MissingArticleMetadata,
    MissingVariantAsset,
    OrphanAsset,
    DuplicateIdentity,
    ConflictingIdentityAssets,
    InvalidMetadata,
}

impl CatalogFindingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingArticleMetadata => "missing_article_metadata",
            Self::MissingVariantAsset => "missing_variant_asset",
            Self::OrphanAsset => "orphan_asset",
            Self::DuplicateIdentity => "duplicate_identity",
            Self::ConflictingIdentityAssets => "conflicting_identity_assets",
            Self::InvalidMetadata => "invalid_metadata",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogFinding {
    pub kind: CatalogFindingKind,
    pub canonical_id: String,
    pub path: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogInventoryReport {
    pub schema_version: String,
    pub article_metadata_count: usize,
    pub pdf_count: usize,
    pub html_count: usize,
    pub canonical_identity_count: usize,
    pub finding_counts: BTreeMap<String, usize>,
    pub findings: Vec<CatalogFinding>,
}

impl CatalogInventoryReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_roundtrips() {
        let report = CatalogInventoryReport {
            schema_version: "catalog-inventory-v1".into(),
            article_metadata_count: 1,
            pdf_count: 1,
            html_count: 0,
            canonical_identity_count: 1,
            finding_counts: BTreeMap::new(),
            findings: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            serde_json::from_str::<CatalogInventoryReport>(&json).unwrap(),
            report
        );
        assert!(report.is_clean());
    }
}
