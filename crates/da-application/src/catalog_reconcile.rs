//! Read-only catalog reconciliation use case.

use da_domain::catalog::{
    CatalogAction, CatalogActionKind, CatalogActionRisk, CatalogApplyDisposition,
    CatalogApplyReport, CatalogApplyResult, CatalogAssetDescriptor, CatalogAssetKind,
    CatalogFindingKind, CatalogInventoryReport, CatalogReconcilePlan,
};
use da_ports::{
    CatalogInventoryError, CatalogInventoryPort, CatalogMutationPort, CatalogWriteDisposition,
};
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

    pub fn transform_metadata(
        action: &CatalogAction,
        donor: &serde_json::Value,
        assets: &[CatalogAssetDescriptor],
    ) -> Result<serde_json::Value, CatalogInventoryError> {
        if action.kind != CatalogActionKind::MaterializeMissingMetadata
            || action.risk != CatalogActionRisk::Low
            || action.donor_metadata_path.is_none()
            || action.expected_content_sha256.is_none()
        {
            return Err(CatalogInventoryError::Infrastructure(
                "only checksum-bound low-risk metadata actions may transform".into(),
            ));
        }
        if assets.is_empty() {
            return Err(CatalogInventoryError::Infrastructure(
                "metadata transform requires live target assets".into(),
            ));
        }
        for asset in assets {
            let path = std::path::Path::new(&asset.relative_path);
            if path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
                || asset.sha256.len() != 64
            {
                return Err(CatalogInventoryError::Infrastructure(format!(
                    "invalid target asset descriptor: {}",
                    asset.relative_path
                )));
            }
        }
        let expected = action.expected_content_sha256.as_deref().unwrap();
        if !assets
            .iter()
            .any(|asset| asset.kind == CatalogAssetKind::Pdf && asset.sha256 == expected)
        {
            return Err(CatalogInventoryError::Infrastructure(
                "target PDF does not satisfy donor checksum precondition".into(),
            ));
        }
        let donor_identity = donor
            .get("identity")
            .and_then(|value| value.as_object())
            .ok_or_else(|| {
                CatalogInventoryError::Infrastructure("donor identity missing".into())
            })?;
        let title = donor_identity
            .get("title")
            .and_then(|value| value.as_str())
            .ok_or_else(|| CatalogInventoryError::Infrastructure("donor title missing".into()))?;
        let catalog_path = action
            .target_path
            .strip_prefix("article_catalog/")
            .unwrap_or(&action.target_path);
        let category = catalog_path.split('/').nth(1).ok_or_else(|| {
            CatalogInventoryError::Infrastructure("target category missing".into())
        })?;
        let variants = assets
            .iter()
            .map(|asset| {
                let (suffix, role, format, media_type, requires_conversion) = match asset.kind {
                    CatalogAssetKind::Pdf => ("pdf", "arxiv_pdf", "pdf", "application/pdf", true),
                    CatalogAssetKind::Html => ("html", "arxiv_html", "html", "text/html", false),
                };
                serde_json::json!({
                    "variant_id": format!("{}:source:{suffix}", action.canonical_id),
                    "source_role": role,
                    "source_format": format,
                    "source_origin": "canonical_catalog_reconciliation",
                    "is_primary": asset.kind == CatalogAssetKind::Pdf,
                    "is_content_bearing": true,
                    "is_metadata_only": false,
                    "path": asset.relative_path,
                    "url": null,
                    "media_type": media_type,
                    "sha256": asset.sha256,
                    "byte_size": asset.byte_size,
                    "capture_status": "captured_local",
                    "capture_policy": "checksum_bound_catalog_reconciliation",
                    "loader_outcome": "not_loaded",
                    "requires_conversion": requires_conversion,
                    "conversion_hint": if requires_conversion { Some("parse_before_graph_use") } else { None },
                    "raw_text_embedded": false,
                    "raw_binary_embedded": false,
                    "network_fetch_attempted": false,
                    "parser_readiness_claimed": false,
                    "chunk_readiness_claimed": false,
                    "graph_readiness_claimed": false
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "schema_version": "article.v00.02",
            "article_key": action.canonical_id,
            "catalog_path": catalog_path,
            "source_code": donor.get("source_code").cloned().unwrap_or_else(|| serde_json::json!("arxiv")),
            "source_type": donor.get("source_type").cloned().unwrap_or_else(|| serde_json::json!("preprint_server")),
            "publisher": donor.get("publisher").cloned().unwrap_or_else(|| serde_json::json!("arxiv")),
            "coarse_topic_code": category,
            "topic_tags": donor.get("topic_tags").cloned().unwrap_or_else(|| serde_json::json!([])),
            "identity": {
                "arxiv_id": action.canonical_id,
                "title": title,
                "canonical_url": format!("https://arxiv.org/abs/{}", action.canonical_id),
                "abs_url": format!("https://arxiv.org/abs/{}", action.canonical_id),
                "pdf_url": format!("https://arxiv.org/pdf/{}", action.canonical_id),
                "normalized_identity": format!("arxiv:{}", action.canonical_id),
                "source_kind": "catalog_reconciled_local_assets"
            },
            "source_strategy": {
                "primary_source_variant_id": format!("{}:source:pdf", action.canonical_id),
                "preferred_content_order": ["arxiv_pdf", "arxiv_html"],
                "metadata_order": [],
                "pdf_policy": "checksum_bound_local_asset",
                "fallback_policy": "no graph writes; D127 remains locked",
                "parser_readiness": "not_claimed",
                "chunk_readiness": "not_claimed",
                "graph_readiness": "not_claimed"
            },
            "source_variants": variants,
            "expected_profile": {
                "should_load": false,
                "should_parse_text": false,
                "should_chunk": false,
                "graph_ready": false,
                "parser_ready": false,
                "chunk_ready": false,
                "known_risks": ["metadata_reconciled_from_checksum_equal_versioned_donor"]
            },
            "safety_flags": {
                "graph_import_allowed": false,
                "production_import_attempted": false,
                "raw_text_embedded_in_metadata": false,
                "raw_binary_embedded_in_metadata": false
            },
            "safety_defaults": {
                "external_network_authorized": false,
                "graph_writes_authorized": false,
                "production_import_authorized": false,
                "fact_promotion_authorized": false,
                "llm_calls_authorized": false
            },
            "reconciliation": {
                "action_id": action.action_id,
                "donor_metadata_path": action.donor_metadata_path,
                "expected_content_sha256": expected,
                "network_used": false,
                "graph_mutated": false,
                "import_eligible": false
            }
        }))
    }

    pub fn inventory_sha256(report: &CatalogInventoryReport) -> String {
        let mut canonical = report.clone();
        canonical.findings.sort_by(|left, right| {
            (left.kind, &left.canonical_id, &left.path, &left.evidence).cmp(&(
                right.kind,
                &right.canonical_id,
                &right.path,
                &right.evidence,
            ))
        });
        hex::encode(Sha256::digest(
            serde_json::to_vec(&canonical).expect("catalog inventory must serialize"),
        ))
    }

    pub fn apply_low<M: CatalogMutationPort>(
        plan: &CatalogReconcilePlan,
        live_report: &CatalogInventoryReport,
        mutation: &M,
    ) -> Result<CatalogApplyReport, CatalogInventoryError> {
        let live_inventory_sha256 = Self::inventory_sha256(live_report);
        if live_inventory_sha256 != plan.source_inventory_sha256 {
            return Err(CatalogInventoryError::Infrastructure(format!(
                "stale catalog plan: plan={} live={live_inventory_sha256}",
                plan.source_inventory_sha256
            )));
        }
        let mut results = Vec::new();
        let mut write_ops = 0;
        for action in plan.actions.iter().filter(|action| {
            action.risk == CatalogActionRisk::Low
                && action.kind == CatalogActionKind::MaterializeMissingMetadata
        }) {
            let donor_path = action.donor_metadata_path.as_deref().ok_or_else(|| {
                CatalogInventoryError::Infrastructure(format!(
                    "low action {} lacks donor metadata path",
                    action.action_id
                ))
            })?;
            let donor_bytes = mutation.read_metadata(donor_path)?;
            let donor: serde_json::Value =
                serde_json::from_slice(&donor_bytes).map_err(|error| {
                    CatalogInventoryError::Infrastructure(format!(
                        "invalid donor metadata {donor_path}: {error}"
                    ))
                })?;
            let assets = mutation.describe_assets(&action.target_path)?;
            let transformed = Self::transform_metadata(action, &donor, &assets)?;
            let mut bytes = serde_json::to_vec_pretty(&transformed).map_err(|error| {
                CatalogInventoryError::Infrastructure(format!(
                    "cannot serialize action {}: {error}",
                    action.action_id
                ))
            })?;
            bytes.push(b'\n');
            let metadata_sha256 = hex::encode(Sha256::digest(&bytes));
            let target = format!("{}/article.json", action.target_path.trim_end_matches('/'));
            let write = mutation.write_metadata_atomic_if_absent(&target, &bytes)?;
            let (disposition, evidence) = match write {
                CatalogWriteDisposition::Applied => {
                    write_ops += 1;
                    (
                        CatalogApplyDisposition::Applied,
                        "metadata committed atomically",
                    )
                }
                CatalogWriteDisposition::Unchanged => (
                    CatalogApplyDisposition::SkippedUnchanged,
                    "identical metadata already present",
                ),
            };
            results.push(CatalogApplyResult {
                action_id: action.action_id.clone(),
                disposition,
                target_metadata_path: target,
                metadata_sha256: Some(metadata_sha256),
                evidence: evidence.into(),
            });
        }
        results.sort_by(|left, right| left.action_id.cmp(&right.action_id));
        Ok(CatalogApplyReport {
            schema_version: "catalog-apply-report-v1".into(),
            plan_inventory_sha256: plan.source_inventory_sha256.clone(),
            live_inventory_sha256,
            attempted: results.len(),
            applied: results
                .iter()
                .filter(|result| result.disposition == CatalogApplyDisposition::Applied)
                .count(),
            skipped: results
                .iter()
                .filter(|result| result.disposition == CatalogApplyDisposition::SkippedUnchanged)
                .count(),
            rejected: results
                .iter()
                .filter(|result| result.disposition == CatalogApplyDisposition::Rejected)
                .count(),
            write_ops,
            results,
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
        let source_inventory_sha256 = Self::inventory_sha256(&canonical);
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
    use da_ports::{CatalogInventorySnapshot, CatalogWriteDisposition};
    use std::sync::Mutex;
    struct Fake;
    struct MutationFake {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        assets: Vec<CatalogAssetDescriptor>,
    }
    impl CatalogMutationPort for MutationFake {
        fn read_metadata(&self, relative_path: &str) -> Result<Vec<u8>, CatalogInventoryError> {
            self.files
                .lock()
                .unwrap()
                .get(relative_path)
                .cloned()
                .ok_or_else(|| {
                    CatalogInventoryError::Infrastructure(format!("missing {relative_path}"))
                })
        }
        fn describe_assets(
            &self,
            _article_root: &str,
        ) -> Result<Vec<CatalogAssetDescriptor>, CatalogInventoryError> {
            Ok(self.assets.clone())
        }
        fn write_file_atomic(
            &self,
            relative_path: &str,
            bytes: &[u8],
        ) -> Result<CatalogWriteDisposition, CatalogInventoryError> {
            self.write_metadata_atomic_if_absent(relative_path, bytes)
        }
        fn write_metadata_atomic_if_absent(
            &self,
            relative_path: &str,
            bytes: &[u8],
        ) -> Result<CatalogWriteDisposition, CatalogInventoryError> {
            let mut files = self.files.lock().unwrap();
            match files.get(relative_path) {
                Some(existing) if existing == bytes => Ok(CatalogWriteDisposition::Unchanged),
                Some(_) => Err(CatalogInventoryError::Infrastructure(
                    "target exists with different content".into(),
                )),
                None => {
                    files.insert(relative_path.into(), bytes.to_vec());
                    Ok(CatalogWriteDisposition::Applied)
                }
            }
        }
    }
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
    fn apply_low_is_fenced_and_state_based() {
        let mut report = CatalogReconcileUseCase::new(Fake).inspect().unwrap();
        report.findings[0].kind = CatalogFindingKind::MissingArticleMetadata;
        report.findings[0].path = "article_catalog/arxiv/cs-ai/x".into();
        report.findings[0].repair_candidate = Some(da_domain::catalog::CatalogRepairCandidate {
            donor_metadata_path: "donor.json".into(),
            content_sha256: "f".repeat(64),
        });
        let plan = CatalogReconcileUseCase::<Fake>::plan(&report);
        let mutation = MutationFake {
            files: Mutex::new(BTreeMap::from([(
                "donor.json".into(),
                serde_json::to_vec(&serde_json::json!({"identity":{"title":"A paper"}})).unwrap(),
            )])),
            assets: vec![CatalogAssetDescriptor {
                kind: CatalogAssetKind::Pdf,
                relative_path: "source/paper.pdf".into(),
                sha256: "f".repeat(64),
                byte_size: 1,
            }],
        };
        let first = CatalogReconcileUseCase::<Fake>::apply_low(&plan, &report, &mutation).unwrap();
        assert_eq!((first.applied, first.write_ops), (1, 1));
        let replay = CatalogReconcileUseCase::<Fake>::apply_low(&plan, &report, &mutation).unwrap();
        assert_eq!(
            (replay.applied, replay.skipped, replay.write_ops),
            (0, 1, 0)
        );
        let mut changed = report.clone();
        changed.pdf_count += 1;
        assert!(CatalogReconcileUseCase::<Fake>::apply_low(&plan, &changed, &mutation).is_err());
        assert_eq!(mutation.files.lock().unwrap().len(), 2);
    }

    #[test]
    fn transform_rebuilds_canonical_metadata_without_inheriting_operational_history() {
        let action = CatalogAction {
            action_id: "a".repeat(64),
            kind: CatalogActionKind::MaterializeMissingMetadata,
            risk: CatalogActionRisk::Low,
            canonical_id: "1234.5678".into(),
            target_path: "article_catalog/arxiv/cs-ai/1234.5678".into(),
            precondition: "checksum equal".into(),
            evidence: "target metadata absent".into(),
            donor_metadata_path: Some(
                "article_catalog/arxiv/cs-ai/1234.5678v1/article.json".into(),
            ),
            expected_content_sha256: Some("f".repeat(64)),
        };
        let donor = serde_json::json!({
            "source_code":"arxiv", "source_type":"preprint_server", "publisher":"arxiv",
            "identity":{"title":"A paper", "arxiv_id":"1234.5678v1"},
            "safety_override":{"external_network_authorized":true},
            "loader_events":[{"graph_readiness_claimed":true}]
        });
        let assets = vec![
            CatalogAssetDescriptor {
                kind: CatalogAssetKind::Pdf,
                relative_path: "article_catalog/arxiv/cs-ai/1234.5678/source/1234.5678.pdf".into(),
                sha256: "f".repeat(64),
                byte_size: 42,
            },
            CatalogAssetDescriptor {
                kind: CatalogAssetKind::Html,
                relative_path: "article_catalog/arxiv/cs-ai/1234.5678/article.html".into(),
                sha256: "e".repeat(64),
                byte_size: 84,
            },
        ];
        let result =
            CatalogReconcileUseCase::<Fake>::transform_metadata(&action, &donor, &assets).unwrap();
        assert_eq!(result["article_key"], "1234.5678");
        assert_eq!(result["identity"]["arxiv_id"], "1234.5678");
        assert_eq!(result["source_variants"].as_array().unwrap().len(), 2);
        assert!(result.get("safety_override").is_none());
        assert!(result.get("loader_events").is_none());
        assert_eq!(result["reconciliation"]["import_eligible"], false);
    }

    #[test]
    fn transform_rejects_traversal_and_checksum_mismatch() {
        let action = CatalogAction {
            action_id: "a".repeat(64),
            kind: CatalogActionKind::MaterializeMissingMetadata,
            risk: CatalogActionRisk::Low,
            canonical_id: "1234.5678".into(),
            target_path: "article_catalog/arxiv/cs-ai/1234.5678".into(),
            precondition: "x".into(),
            evidence: "x".into(),
            donor_metadata_path: Some("donor.json".into()),
            expected_content_sha256: Some("f".repeat(64)),
        };
        let donor = serde_json::json!({"identity":{"title":"A paper"}});
        let traversal = vec![CatalogAssetDescriptor {
            kind: CatalogAssetKind::Pdf,
            relative_path: "../escape.pdf".into(),
            sha256: "f".repeat(64),
            byte_size: 1,
        }];
        assert!(
            CatalogReconcileUseCase::<Fake>::transform_metadata(&action, &donor, &traversal)
                .is_err()
        );
        let mismatch = vec![CatalogAssetDescriptor {
            kind: CatalogAssetKind::Pdf,
            relative_path: "source/paper.pdf".into(),
            sha256: "e".repeat(64),
            byte_size: 1,
        }];
        assert!(
            CatalogReconcileUseCase::<Fake>::transform_metadata(&action, &donor, &mismatch)
                .is_err()
        );
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
