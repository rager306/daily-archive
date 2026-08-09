//! Read-only catalog reconciliation use case.

use da_domain::catalog::CatalogInventoryReport;
use da_ports::{CatalogInventoryError, CatalogInventoryPort};
use std::collections::BTreeMap;

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
                }],
            })
        }
    }
    #[test]
    fn summarizes_findings_deterministically() {
        let r = CatalogReconcileUseCase::new(Fake).inspect().unwrap();
        assert_eq!(r.finding_counts["missing_variant_asset"], 1);
        assert_eq!(r.schema_version, "catalog-inventory-v1");
    }
}
