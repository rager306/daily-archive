//! Detect-only ontology v2 contract audit (M371/S06/T01).
//!
//! This composition root loads project policy surfaces, imports one bounded
//! checkpoint into an empty process-local Samyama store, observes it through
//! read-only ports, and writes a deterministic report. It never mutates durable
//! graph or ontology state and never changes D127.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::SamyamaGraphStore;
use da_application::ontology_contract_audit::{
    AUDIT_INPUT_SCHEMA_VERSION, ContractAuditInput, IdentityIndexObservation,
    OntologyContractAuditUseCase, SurfaceObservation, TaskWireObservation, observe_runtime_graph,
};
use da_cli::graph_tasks::{parse_tasks_cypher, project_task_edge_wires};
use da_domain::{ContentDigest, finalize::FinalizePolicy};
use da_ports::GraphStore;
use flate2::read::GzDecoder;
use kg_ontology::registry::OntologyRegistry;
use serde::Deserialize;

const MAX_POLICY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TASK_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_DECOMPRESSED_SNAPSHOT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SNAPSHOT_NODES: u64 = 100_000;
const MAX_SNAPSHOT_EDGES: u64 = 500_000;
const MAX_SNAPSHOT_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Parser)]
#[command(about = "Audit ontology v2 contracts against project surfaces and a real checkpoint")]
struct Args {
    #[arg(long, default_value = "data/ontology")]
    ontology_dir: PathBuf,
    #[arg(long, default_value = "data/ontology/identity_indexes.yaml")]
    identity_indexes: PathBuf,
    #[arg(long, default_value = "data/ontology/finalize_policy.yaml")]
    finalize_policy: PathBuf,
    #[arg(long, default_value = "data/graph_tasks/gepa_hub.tasks.cypher")]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.sgsnap"
    )]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_contract_audit.json"
    )]
    report: PathBuf,
}

#[derive(Debug, Deserialize)]
struct IdentityIndexesFile {
    schema_version: String,
    indexes: Vec<IdentityIndexEntry>,
}

#[derive(Debug, Deserialize)]
struct IdentityIndexEntry {
    label: String,
    property: String,
    identity: bool,
}

#[derive(Debug, Deserialize)]
struct SnapshotHeader {
    format: String,
    version: u32,
    node_count: u64,
    edge_count: u64,
}

#[derive(Debug, Deserialize)]
struct SnapshotRecordKind {
    t: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let ontology_dir = input_path(&repo_root, &args.ontology_dir)?;
    let identity_path = input_path(&repo_root, &args.identity_indexes)?;
    let finalize_path = input_path(&repo_root, &args.finalize_policy)?;
    let tasks_path = input_path(&repo_root, &args.tasks)?;
    let snapshot_path = input_path(&repo_root, &args.snapshot)?;
    let report_path = output_path(&repo_root, &args.report)?;
    let node_schema_path = input_path(&repo_root, &ontology_dir.join("node_schemas.yaml"))?;
    let edge_contract_path = input_path(&repo_root, &ontology_dir.join("edge_contracts.yaml"))?;
    let node_schema_source = read_capped(&node_schema_path, MAX_POLICY_BYTES)?;
    let edge_contract_source = read_capped(&edge_contract_path, MAX_POLICY_BYTES)?;
    let registry = load_registry_from_bounded_bytes(&node_schema_source, &edge_contract_source)?;

    let identity_source = read_capped(&identity_path, MAX_POLICY_BYTES)?;
    let identity: IdentityIndexesFile = serde_yaml::from_slice(&identity_source)
        .with_context(|| format!("parse {}", display_path(&repo_root, &identity_path)))?;
    if identity.schema_version != "ontology-v2-identity-indexes.v1" {
        anyhow::bail!(
            "unsupported identity index schema: {}",
            identity.schema_version
        );
    }
    let identity_indexes = identity
        .indexes
        .into_iter()
        .filter(|entry| entry.identity)
        .map(|entry| IdentityIndexObservation {
            label: entry.label,
            property: entry.property,
            source_ref: display_path(&repo_root, &identity_path),
        })
        .collect();

    let finalize_source = read_capped(&finalize_path, MAX_POLICY_BYTES)?;
    let finalize: FinalizePolicy = serde_yaml::from_slice(&finalize_source)
        .with_context(|| format!("parse {}", display_path(&repo_root, &finalize_path)))?;
    finalize.validate().map_err(anyhow::Error::msg)?;
    let finalize_required_labels = finalize
        .required_nonempty_labels
        .into_iter()
        .map(|value| SurfaceObservation {
            value,
            source_ref: display_path(&repo_root, &finalize_path),
        })
        .collect();

    let task_source = read_capped(&tasks_path, MAX_TASK_BYTES)?;
    let task_text = std::str::from_utf8(&task_source).context("task suite must be UTF-8")?;
    let suite = parse_tasks_cypher(task_text).map_err(anyhow::Error::msg)?;
    let task_projection = project_task_edge_wires(&suite);
    let task_wires = task_projection
        .wires
        .into_iter()
        .map(|(edge_type, task_id)| TaskWireObservation {
            edge_type,
            source_ref: format!("{}#{task_id}", display_path(&repo_root, &tasks_path)),
        })
        .collect();

    let snapshot = read_capped(&snapshot_path, MAX_SNAPSHOT_BYTES)?;
    preflight_snapshot(&snapshot)?;
    let store = SamyamaGraphStore::try_from_env().map_err(anyhow::Error::msg)?;
    store
        .import_snapshot(&snapshot)
        .await
        .with_context(|| format!("import {}", display_path(&repo_root, &snapshot_path)))?;
    let runtime_source = display_path(&repo_root, &snapshot_path);
    let runtime = observe_runtime_graph(&store, &runtime_source).await;
    if runtime.nodes.iter().map(|node| node.count).sum::<usize>()
        > usize::try_from(MAX_SNAPSHOT_NODES).unwrap_or(usize::MAX)
        || runtime.edges.iter().map(|edge| edge.count).sum::<usize>()
            > usize::try_from(MAX_SNAPSHOT_EDGES).unwrap_or(usize::MAX)
    {
        anyhow::bail!("runtime graph exceeds observation budget");
    }

    let source_fingerprints = BTreeMap::from([
        ("node_schemas".into(), fingerprint(&node_schema_source)),
        ("edge_contracts".into(), fingerprint(&edge_contract_source)),
        ("identity_indexes".into(), fingerprint(&identity_source)),
        ("finalize_policy".into(), fingerprint(&finalize_source)),
        ("graph_tasks".into(), fingerprint(&task_source)),
        ("runtime_snapshot".into(), fingerprint(&snapshot)),
    ]);

    let mut coverage_gaps = runtime.coverage_gaps;
    coverage_gaps.extend(
        task_projection
            .diagnostics
            .into_iter()
            .map(|(task_id, diagnostic)| SurfaceObservation {
                value: format!("task_wire_projection:{diagnostic}"),
                source_ref: format!("{}#{task_id}", display_path(&repo_root, &tasks_path)),
            }),
    );
    coverage_gaps.push(SurfaceObservation {
        value: "static_materializer_endpoint_declarations_not_yet_typed".into(),
        source_ref: "M371-pr59ty/S06/T03".into(),
    });
    coverage_gaps.push(SurfaceObservation {
        value: "production_consumer_label_walk_not_yet_registry_driven".into(),
        source_ref: "crates/da-cli/src/main.rs".into(),
    });

    let report = OntologyContractAuditUseCase::new(&registry).audit(ContractAuditInput {
        schema_version: AUDIT_INPUT_SCHEMA_VERSION.into(),
        source_fingerprints,
        identity_indexes,
        finalize_required_labels,
        task_wires,
        runtime_nodes: runtime.nodes,
        runtime_edges: runtime.edges,
        consumer_labels: vec![],
        coverage_gaps,
        import_eligible: false,
    })?;
    atomic_write_report(&report_path, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "ontology_contract_audit verdict={:?} labels={} edges={} findings={} report_sha256={} import_eligible={}",
        report.verdict,
        report.ontology_labels,
        report.ontology_edges,
        report.findings.len(),
        report.report_sha256,
        report.import_eligible
    );
    Ok(())
}

fn input_path(repo_root: &Path, path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("resolve input {}", path.display()))?;
    if !resolved.starts_with(repo_root) {
        anyhow::bail!("input path escapes repository: {}", path.display());
    }
    Ok(resolved)
}

fn output_path(repo_root: &Path, path: &Path) -> Result<PathBuf> {
    let artifact_root = repo_root.join("artifacts/ontology");
    std::fs::create_dir_all(&artifact_root)?;
    let artifact_root = artifact_root.canonicalize()?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    let file_name = candidate
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("report file name missing"))?;
    let parent = candidate
        .parent()
        .ok_or_else(|| anyhow::anyhow!("report parent missing"))?
        .canonicalize()
        .with_context(|| format!("resolve report parent {}", candidate.display()))?;
    if parent != artifact_root || file_name.to_string_lossy().starts_with('.') {
        anyhow::bail!("report must be a visible file directly under artifacts/ontology");
    }
    Ok(parent.join(file_name))
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn read_capped(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("metadata {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("input is not a regular file: {}", path.display());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        anyhow::bail!("input exceeds byte budget: {}", path.display());
    }
    Ok(bytes)
}

fn load_registry_from_bounded_bytes(
    node_schemas: &[u8],
    edge_contracts: &[u8],
) -> Result<OntologyRegistry> {
    let root = std::env::temp_dir().join(format!(
        "da-ontology-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let result = (|| {
        write_new(&root.join("node_schemas.yaml"), node_schemas)?;
        write_new(&root.join("edge_contracts.yaml"), edge_contracts)?;
        OntologyRegistry::load_from_dir(&root).map_err(anyhow::Error::msg)
    })();
    let cleanup = std::fs::remove_dir_all(&root);
    result.and_then(|registry| {
        cleanup.with_context(|| format!("remove {}", root.display()))?;
        Ok(registry)
    })
}

fn atomic_write_report(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("report parent missing")?;
    let temp = parent.join(format!(
        ".ontology-audit-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let result = (|| {
        write_new(&temp, bytes)?;
        std::fs::rename(&temp, path)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn preflight_snapshot(compressed: &[u8]) -> Result<()> {
    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .by_ref()
        .take(MAX_DECOMPRESSED_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut decompressed)
        .context("decompress snapshot preflight")?;
    if decompressed.len() as u64 > MAX_DECOMPRESSED_SNAPSHOT_BYTES {
        anyhow::bail!("decompressed snapshot exceeds byte budget");
    }
    let mut lines = decompressed.split(|byte| *byte == b'\n');
    let header_line = lines.next().context("snapshot header missing")?;
    if header_line.len() > MAX_SNAPSHOT_LINE_BYTES {
        anyhow::bail!("snapshot header exceeds line budget");
    }
    let header: SnapshotHeader = serde_json::from_slice(header_line)?;
    if header.format != "sgsnap"
        || !matches!(header.version, 1 | 2)
        || header.node_count > MAX_SNAPSHOT_NODES
        || header.edge_count > MAX_SNAPSHOT_EDGES
    {
        anyhow::bail!("snapshot header violates format or graph budget");
    }
    let mut nodes = 0_u64;
    let mut edges = 0_u64;
    for line in lines.filter(|line| !line.is_empty()) {
        if line.len() > MAX_SNAPSHOT_LINE_BYTES {
            anyhow::bail!("snapshot record exceeds line budget");
        }
        match serde_json::from_slice::<SnapshotRecordKind>(line)?
            .t
            .as_str()
        {
            "n" => nodes += 1,
            "e" => edges += 1,
            other => anyhow::bail!("unknown snapshot record type: {other}"),
        }
        if nodes > MAX_SNAPSHOT_NODES || edges > MAX_SNAPSHOT_EDGES {
            anyhow::bail!("snapshot body exceeds graph budget");
        }
    }
    if nodes != header.node_count || edges != header.edge_count {
        anyhow::bail!(
            "snapshot header/body count mismatch: header={}/{} body={nodes}/{edges}",
            header.node_count,
            header.edge_count
        );
    }
    Ok(())
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{}", ContentDigest::of_bytes(bytes).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    fn snapshot(lines: &[serde_json::Value]) -> Vec<u8> {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        for line in lines {
            serde_json::to_writer(&mut gzip, line).unwrap();
            gzip.write_all(b"\n").unwrap();
        }
        gzip.finish().unwrap()
    }

    #[test]
    fn snapshot_preflight_accepts_exact_bounded_counts() {
        let bytes = snapshot(&[
            serde_json::json!({"format":"sgsnap","version":2,"node_count":1,"edge_count":1}),
            serde_json::json!({"t":"n","id":1}),
            serde_json::json!({"t":"e","id":1}),
        ]);
        assert!(preflight_snapshot(&bytes).is_ok());
    }

    #[test]
    fn snapshot_preflight_rejects_header_body_drift_and_unknown_records() {
        let mismatch = snapshot(&[
            serde_json::json!({"format":"sgsnap","version":2,"node_count":2,"edge_count":0}),
            serde_json::json!({"t":"n","id":1}),
        ]);
        assert!(preflight_snapshot(&mismatch).is_err());
        let unknown = snapshot(&[
            serde_json::json!({"format":"sgsnap","version":2,"node_count":0,"edge_count":0}),
            serde_json::json!({"t":"x"}),
        ]);
        assert!(preflight_snapshot(&unknown).is_err());
    }

    #[test]
    fn path_policy_rejects_input_escape_and_output_outside_artifact_root() {
        let root = std::env::temp_dir().join(format!("da-audit-path-{}", std::process::id()));
        let outside = std::env::temp_dir().join(format!("da-audit-outside-{}", std::process::id()));
        std::fs::create_dir_all(root.join("artifacts/ontology")).unwrap();
        std::fs::write(&outside, b"outside").unwrap();
        assert!(input_path(&root.canonicalize().unwrap(), &outside).is_err());
        assert!(output_path(&root.canonicalize().unwrap(), Path::new("elsewhere.json")).is_err());
        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
