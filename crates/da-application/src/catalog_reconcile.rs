//! Read-only catalog reconciliation use case.

use da_domain::catalog::{
    CatalogAction, CatalogActionKind, CatalogActionRisk, CatalogFindingKind,
    CatalogInventoryReport, CatalogReconcilePlan,
};
use da_ports::{CatalogInventoryError, CatalogInventoryPort};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub struct CatalogReconcileUseCase<P> {
    inventory: P,
}
impl<P: CatalogInventoryPort> CatalogReconcileUseCase<P> {
    pub fn new(inventory: P) -> Self {
        Self { inventory }
    }
    pub fn inspect(&self) -> Result<CatalogInventoryReport, CatalogInventoryError> {
        let snapshot = self.inventory.inventory()?;
        let mut counts = BTreeMap::new();
        for finding in &snapshot.findings {
            *counts.entry(finding.kind.as_str().to_string()).or_insert(0) += 1;
        }
        Ok(CatalogInventoryReport {
            schema_version: "catalog-inventory-v1".into(),
            article_metadata_count: snapshot.article_metadata_count,
            pdf_count: snapshot.pdf_count,
            html_count: snapshot.html_count,
            canonical_identity_count: snapshot.canonical_identity_count,
            finding_counts: counts,
            findings: snapshot.findings,
        })
    }

    pub fn plan(report: &CatalogInventoryReport) -> CatalogReconcilePlan {
        let mut canonical = report.clone();
        canonical.findings.sort_by(|left, right| {
            (left.kind, &left.canonical_id, &left.path, &left.evidence).cmp(&(
                right.kind,
                &right.canonical_id,
                &right.path,
                &right.evidence,
            ))
        });
        let source_inventory_sha256 = hex::encode(Sha256::digest(
            serde_json::to_vec(&canonical).expect("catalog inventory must serialize"),
        ));
        let mut seen = BTreeSet::new();
        let mut actions = Vec::new();
        for finding in canonical.findings {
            let repair_candidate = finding.repair_candidate.clone();
            let (kind, risk, precondition) = match finding.kind {
                CatalogFindingKind::MissingArticleMetadata if repair_candidate.is_some() => (
                    CatalogActionKind::MaterializeMissingMetadata,
                    CatalogActionRisk::Low,
                    "target metadata absent; donor PDF checksum equals target PDF",
                ),
                CatalogFindingKind::MissingArticleMetadata => (
                    CatalogActionKind::MaterializeMissingMetadata,
                    CatalogActionRisk::High,
                    "target metadata absent; no unique checksum-equal donor",
                ),
                CatalogFindingKind::MissingVariantAsset => (
                    CatalogActionKind::InvestigateMissingVariant,
                    CatalogActionRisk::Blocked,
                    "referenced asset unresolved",
                ),
                CatalogFindingKind::OrphanAsset => (
                    CatalogActionKind::MaterializeMissingMetadata,
                    CatalogActionRisk::High,
                    "orphan asset identity requires review",
                ),
                CatalogFindingKind::DuplicateIdentity => (
                    CatalogActionKind::ResolveIdentityConflict,
                    CatalogActionRisk::Medium,
                    "duplicate metadata identity",
                ),
                CatalogFindingKind::ConflictingIdentityAssets => (
                    CatalogActionKind::ResolveIdentityConflict,
                    CatalogActionRisk::Blocked,
                    "identity has distinct content checksums",
                ),
                CatalogFindingKind::InvalidMetadata => (
                    CatalogActionKind::RepairInvalidMetadata,
                    CatalogActionRisk::Blocked,
                    "metadata does not satisfy readable schema",
                ),
            };
            let identity = serde_json::json!({
                "kind": kind,
                "risk": risk,
                "canonical_id": finding.canonical_id,
                "target_path": finding.path,
                "precondition": precondition,
                "evidence": finding.evidence,
                "donor_metadata_path": repair_candidate.as_ref().map(|candidate| &candidate.donor_metadata_path),
                "expected_content_sha256": repair_candidate.as_ref().map(|candidate| &candidate.content_sha256),
            });
            let action_id = hex::encode(Sha256::digest(
                serde_json::to_vec(&identity).expect("catalog action must serialize"),
            ));
            if seen.insert(action_id.clone()) {
                actions.push(CatalogAction {
                    action_id,
                    kind,
                    risk,
                    canonical_id: identity["canonical_id"].as_str().unwrap().to_string(),
                    target_path: identity["target_path"].as_str().unwrap().to_string(),
                    precondition: precondition.to_string(),
                    evidence: identity["evidence"].as_str().unwrap().to_string(),
                    donor_metadata_path: repair_candidate
                        .as_ref()
                        .map(|candidate| candidate.donor_metadata_path.clone()),
                    expected_content_sha256: repair_candidate
                        .as_ref()
                        .map(|candidate| candidate.content_sha256.clone()),
                });
            }
        }
        actions.sort_by(|left, right| {
            (
                left.risk,
                left.kind,
                &left.canonical_id,
                &left.target_path,
                &left.action_id,
            )
                .cmp(&(
                    right.risk,
                    right.kind,
                    &right.canonical_id,
                    &right.target_path,
                    &right.action_id,
                ))
        });
        let mut action_counts = BTreeMap::new();
        for action in &actions {
            *action_counts
                .entry(format!("{:?}", action.risk).to_ascii_lowercase())
                .or_insert(0) += 1;
        }
        CatalogReconcilePlan {
            schema_version: "catalog-reconcile-plan-v1".into(),
            source_inventory_sha256,
            actions,
            action_counts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::catalog::{CatalogFinding, CatalogFindingKind};
    use da_ports::CatalogInventorySnapshot;
    struct Fake;
    impl CatalogInventoryPort for Fake {
        fn inventory(&self) -> Result<CatalogInventorySnapshot, CatalogInventoryError> {
            Ok(CatalogInventorySnapshot {
                article_metadata_count: 1,
                pdf_count: 1,
                html_count: 1,
                canonical_identity_count: 1,
                findings: vec![CatalogFinding {
                    kind: CatalogFindingKind::MissingVariantAsset,
                    canonical_id: "x".into(),
                    path: "a".into(),
                    evidence: "missing".into(),
                    repair_candidate: None,
                }],
            })
        }
    }
    #[test]
    fn summarizes_findings_deterministically() {
        let r = CatalogReconcileUseCase::new(Fake).inspect().unwrap();
        assert_eq!(r.finding_counts["missing_variant_asset"], 1);
        assert_eq!(r.schema_version, "catalog-inventory-v1");
        let plan = CatalogReconcileUseCase::<Fake>::plan(&r);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].risk, CatalogActionRisk::Blocked);
        assert_eq!(plan.auto_applicable_count(), 0);
    }

    #[test]
    fn plan_is_permutation_invariant_and_deduplicates_findings() {
        let mut report = CatalogReconcileUseCase::new(Fake).inspect().unwrap();
        let finding = report.findings[0].clone();
        report.findings.extend([finding.clone(), finding]);
        let first = CatalogReconcileUseCase::<Fake>::plan(&report);
        report.findings.reverse();
        let second = CatalogReconcileUseCase::<Fake>::plan(&report);
        assert_eq!(first, second);
        assert_eq!(first.actions.len(), 1);
    }
}
