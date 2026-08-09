//! Typed article-catalog reconciliation findings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_candidate: Option<CatalogRepairCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRepairCandidate {
    pub donor_metadata_path: String,
    pub content_sha256: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogActionRisk {
    Low,
    Medium,
    High,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogActionKind {
    NormalizeVariantPath,
    BackfillVariantChecksum,
    MaterializeMissingMetadata,
    InvestigateMissingVariant,
    ResolveIdentityConflict,
    RepairInvalidMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogAction {
    pub action_id: String,
    pub kind: CatalogActionKind,
    pub risk: CatalogActionRisk,
    pub canonical_id: String,
    pub target_path: String,
    pub precondition: String,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub donor_metadata_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_content_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogReconcilePlan {
    pub schema_version: String,
    pub source_inventory_sha256: String,
    pub actions: Vec<CatalogAction>,
    pub action_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAssetKind {
    Pdf,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogAssetDescriptor {
    pub kind: CatalogAssetKind,
    pub relative_path: String,
    pub sha256: String,
    pub byte_size: u64,
}

impl CatalogReconcilePlan {
    pub fn auto_applicable_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| action.risk == CatalogActionRisk::Low)
            .count()
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

        let action = CatalogAction {
            action_id: "a".into(),
            kind: CatalogActionKind::NormalizeVariantPath,
            risk: CatalogActionRisk::Low,
            canonical_id: "1234.5678".into(),
            target_path: "article.json".into(),
            precondition: "old path resolves".into(),
            evidence: "source/paper.pdf".into(),
            donor_metadata_path: None,
            expected_content_sha256: None,
        };
        let plan = CatalogReconcilePlan {
            schema_version: "catalog-reconcile-plan-v1".into(),
            source_inventory_sha256: "f".repeat(64),
            actions: vec![action],
            action_counts: BTreeMap::from([("low".into(), 1)]),
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(
            serde_json::from_str::<CatalogReconcilePlan>(&json).unwrap(),
            plan
        );
        assert_eq!(plan.auto_applicable_count(), 1);
    }
}
