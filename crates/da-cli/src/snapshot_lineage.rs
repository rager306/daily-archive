//! DATA-governed mapping of stage producer reports into exact-byte lineage.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use da_domain::ContentDigest;
use da_domain::corpus_checkpoint::{SnapshotDerivationLineage, SnapshotDerivationLink};
use serde::Deserialize;

const MAX_POLICY_BYTES: u64 = 256 * 1024;
const MAX_REPORT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: String,
    stages: Vec<PolicyStage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyStage {
    stage_id: String,
    report_schema: String,
}

pub fn load_lineage(
    materialization_physical_sha256: ContentDigest,
    policy_path: &Path,
    report_paths: &[PathBuf],
) -> Result<SnapshotDerivationLineage> {
    let policy_bytes = read_bounded(policy_path, MAX_POLICY_BYTES, "derivation policy")?;
    let policy: Policy = serde_yaml::from_slice(&policy_bytes)
        .with_context(|| format!("parse derivation policy {}", policy_path.display()))?;
    anyhow::ensure!(
        policy.schema_version == "snapshot-derivation-policy.v1",
        "unsupported snapshot derivation policy schema {}",
        policy.schema_version
    );
    anyhow::ensure!(
        !policy.stages.is_empty(),
        "snapshot derivation policy has no stages"
    );

    let mut links = Vec::with_capacity(report_paths.len());
    for (index, path) in report_paths.iter().enumerate() {
        let expected = policy.stages.get(index).ok_or_else(|| {
            anyhow!(
                "derivation report {} exceeds DATA policy stage count",
                path.display()
            )
        })?;
        let bytes = read_bounded(path, MAX_REPORT_BYTES, "derivation report")?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse derivation report {}", path.display()))?;
        let schema = required_string(&value, "schema_version", path)?;
        anyhow::ensure!(
            schema == expected.report_schema,
            "derivation report {} at index {} has schema {}, expected {} for stage {}",
            path.display(),
            index,
            schema,
            expected.report_schema,
            expected.stage_id
        );
        let import_eligible = value
            .get("import_eligible")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                anyhow!(
                    "derivation report {} missing boolean import_eligible",
                    path.display()
                )
            })?;
        anyhow::ensure!(
            !import_eligible,
            "derivation report {} unlocks D127",
            path.display()
        );
        links.push(SnapshotDerivationLink {
            stage_id: expected.stage_id.clone(),
            report_path: path.display().to_string(),
            report_sha256: ContentDigest::of_bytes(&bytes),
            input_physical_sha256: parse_digest(&value, "input_snapshot_sha256", path)?,
            output_physical_sha256: parse_digest(&value, "output_snapshot_sha256", path)?,
            import_eligible: false,
        });
    }
    Ok(SnapshotDerivationLineage {
        materialization_physical_sha256,
        links,
    })
}

fn parse_digest(value: &serde_json::Value, key: &str, path: &Path) -> Result<ContentDigest> {
    ContentDigest::try_from(required_string(value, key, path)?.to_string()).map_err(|error| {
        anyhow!(
            "derivation report {} invalid {key}: {error}",
            path.display()
        )
    })
}

fn required_string<'a>(value: &'a serde_json::Value, key: &str, path: &Path) -> Result<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("derivation report {} missing string {key}", path.display()))
}

fn read_bounded(path: &Path, limit: u64, kind: &str) -> Result<Vec<u8>> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("stat {kind} {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{kind} is not a file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.len() <= limit,
        "{kind} {} exceeds {} byte limit",
        path.display(),
        limit
    );
    std::fs::read(path).with_context(|| format!("read {kind} {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reordered_or_d127_report() {
        let root = std::env::temp_dir().join(format!("da-lineage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let policy = root.join("policy.yaml");
        std::fs::write(&policy, "schema_version: snapshot-derivation-policy.v1\nstages:\n  - stage_id: summary\n    report_schema: summary.v1\n").unwrap();
        let report = root.join("report.json");
        let invalid = serde_json::json!({
            "schema_version": "cluster.v1",
            "input_snapshot_sha256": "a".repeat(64),
            "output_snapshot_sha256": "b".repeat(64),
            "import_eligible": "true".parse::<bool>().unwrap(),
        });
        std::fs::write(&report, serde_json::to_vec(&invalid).unwrap()).unwrap();
        let error = load_lineage(
            ContentDigest::of_text("materialization"),
            &policy,
            &[report],
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("expected summary.v1"),
            "{error:#}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn digest(n: u8) -> String {
        format!("{n:064x}")
    }

    fn write_report(path: &std::path::Path, schema: &str) {
        let body = serde_json::json!({
            "schema_version": schema,
            "input_snapshot_sha256": digest(1),
            "output_snapshot_sha256": digest(2),
            "import_eligible": false,
        });
        std::fs::write(path, serde_json::to_vec(&body).unwrap()).unwrap();
    }

    #[test]
    fn bundled_policy_keeps_two_report_prefix_and_requires_extractive_schema() {
        let policy = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/ontology/snapshot_derivation_policy.yaml");
        let root = std::env::temp_dir().join(format!("da-lineage-s04-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let cluster = root.join("cluster.json");
        let resolution = root.join("resolution.json");
        let summary = root.join("summary.json");
        write_report(&cluster, "concept-cluster-canary.v1");
        write_report(&resolution, "entity-resolution-canary.v1");
        let prefix = load_lineage(
            ContentDigest::of_text("materialization"),
            policy.as_path(),
            &[cluster.clone(), resolution.clone()],
        )
        .unwrap();
        assert_eq!(prefix.links.len(), 2);
        assert_eq!(prefix.links[0].stage_id, "concept_cluster");
        assert_eq!(prefix.links[1].stage_id, "entity_resolution");

        write_report(&summary, "wrong.v1");
        let err = load_lineage(
            ContentDigest::of_text("materialization"),
            policy.as_path(),
            &[cluster.clone(), resolution.clone(), summary.clone()],
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("summarize-revisions.v1"),
            "{err:#}"
        );

        write_report(&summary, "summarize-revisions.v1");
        let full = load_lineage(
            ContentDigest::of_text("materialization"),
            policy.as_path(),
            &[cluster, resolution, summary],
        )
        .unwrap();
        assert_eq!(full.links.len(), 3);
        assert_eq!(full.links[2].stage_id, "extractive_summary");
        assert!(!full.links[2].import_eligible);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_lineage_accepts_serialized_summarize_revisions_report() {
        let policy = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/ontology/snapshot_derivation_policy.yaml");
        let root = std::env::temp_dir().join(format!("da-lineage-s02-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let cluster = root.join("cluster.json");
        let resolution = root.join("resolution.json");
        let summary = root.join("summary.json");
        write_report(&cluster, "concept-cluster-canary.v1");
        write_report(&resolution, "entity-resolution-canary.v1");
        let producer = crate::summarize_revisions::SummarizeRevisionsReport {
            schema_version: "summarize-revisions.v1".into(),
            method: "extractive_lead_v1".into(),
            revisions_processed: 1,
            revisions_skipped: 0,
            summaries_written: 2,
            skipped_empty_sections: 0,
            replay_failures: vec![],
            input_snapshot_sha256: digest(3),
            output_snapshot_sha256: digest(4),
            import_eligible: false,
        };
        assert!(producer.derivation_ready());
        std::fs::write(&summary, serde_json::to_vec(&producer).unwrap()).unwrap();
        let full = load_lineage(
            ContentDigest::of_text("materialization"),
            policy.as_path(),
            &[cluster, resolution, summary],
        )
        .unwrap();
        assert_eq!(full.links.len(), 3);
        assert_eq!(full.links[2].stage_id, "extractive_summary");
        assert_eq!(full.links[2].input_physical_sha256.as_str(), digest(3));
        assert_eq!(full.links[2].output_physical_sha256.as_str(), digest(4));
        std::fs::remove_dir_all(root).unwrap();
    }
}
