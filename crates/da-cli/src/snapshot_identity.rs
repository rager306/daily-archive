//! Production CLI composition for bounded `.sgsnap` identity evidence.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Args;
use da_domain::ContentDigest;
use da_domain::corpus_checkpoint::{SnapshotArtifactIdentity, SnapshotIdentityEvidence};

const MAX_COMPRESSED_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Args)]
pub struct SnapshotIdentityArgs {
    /// Snapshot whose exact bytes and strict graph state will be identified.
    #[arg(long)]
    pub input: PathBuf,
    /// Atomic JSON evidence output.
    #[arg(long)]
    pub report: PathBuf,
}

pub fn run(args: &SnapshotIdentityArgs) -> Result<SnapshotIdentityEvidence> {
    let metadata = std::fs::metadata(&args.input)
        .with_context(|| format!("stat snapshot {}", args.input.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "snapshot is not a file: {}",
        args.input.display()
    );
    anyhow::ensure!(
        metadata.len() <= MAX_COMPRESSED_SNAPSHOT_BYTES,
        "snapshot compressed size {} exceeds limit {}",
        metadata.len(),
        MAX_COMPRESSED_SNAPSHOT_BYTES
    );
    let bytes = std::fs::read(&args.input)
        .with_context(|| format!("read snapshot {}", args.input.display()))?;
    let fingerprint = kg_storage::fingerprint_snapshot(&bytes)
        .map_err(|error| anyhow!("fingerprint snapshot {}: {error}", args.input.display()))?;
    let report = SnapshotIdentityEvidence::seal(SnapshotArtifactIdentity {
        path: args.input.display().to_string(),
        physical_sha256: ContentDigest::try_from(fingerprint.physical_sha256)
            .map_err(|error| anyhow!("physical snapshot digest: {error}"))?,
        semantic_sha256: ContentDigest::try_from(fingerprint.semantic_sha256)
            .map_err(|error| anyhow!("semantic snapshot digest: {error}"))?,
        semantic_algorithm: fingerprint.semantic_algorithm,
        nodes: fingerprint.node_count,
        edges: fingerprint.edge_count,
    })
    .map_err(|error| anyhow!("seal snapshot identity evidence: {error}"))?;
    atomic_write_json(&args.report, &report)?;
    println!(
        "snapshot-identity physical_sha256={} semantic_sha256={} algorithm={} nodes={} edges={} report={} import_eligible=false",
        report.snapshot.physical_sha256.as_str(),
        report.snapshot.semantic_sha256.as_str(),
        report.snapshot.semantic_algorithm,
        report.snapshot.nodes,
        report.snapshot.edges,
        args.report.display()
    );
    Ok(report)
}

fn atomic_write_json(path: &Path, report: &SnapshotIdentityEvidence) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create report directory {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(report)?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&temp)
            .with_context(|| format!("create temporary report {}", temp.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
            .with_context(|| format!("replace report {}", path.display()))?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::File::open(parent).and_then(|directory| directory.sync_all());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_input_does_not_replace_existing_report() {
        let root =
            std::env::temp_dir().join(format!("da-snapshot-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("bad.sgsnap");
        let report = root.join("evidence.json");
        std::fs::write(&input, b"not-gzip").unwrap();
        std::fs::write(&report, b"preserve").unwrap();
        let error = run(&SnapshotIdentityArgs {
            input,
            report: report.clone(),
        })
        .unwrap_err();
        assert!(
            error.to_string().contains("fingerprint snapshot"),
            "{error:#}"
        );
        assert_eq!(std::fs::read(&report).unwrap(), b"preserve");
        std::fs::remove_dir_all(root).unwrap();
    }
}
