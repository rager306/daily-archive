//! Bounded composition of independently validated ontology v2 evidence.
//!
//! This is not the first-class acceptance runner. It binds the read-only
//! 200-paper checkpoint audit/identity evidence to a separate process-local
//! endpoint canary and preserves every declared incomplete surface.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use da_application::identity_compatibility::IdentityCompatibilityReport;
use da_application::ontology_contract_audit::OntologyContractAuditReport;
use da_application::ontology_endpoint_evidence::OntologyEndpointEvidence;
use da_application::ontology_v2_convergence::{
    EvidenceBinding, OntologyV2ConvergenceInput, OntologyV2ConvergenceUseCase,
};
use da_cli::gold_graph::atomic_write;
use da_cli::graph_tasks::{parse_tasks_cypher, project_task_edge_wires};
use da_domain::ContentDigest;

const MAX_REPORT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TASK_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(about = "Compose bounded ontology v2 convergence evidence")]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_contract_audit.json"
    )]
    contract_audit: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_identity_compatibility.json"
    )]
    identity_compatibility: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_endpoint_canary.json"
    )]
    endpoint_evidence: PathBuf,
    #[arg(long, default_value = "data/graph_tasks/gepa_hub.tasks.cypher")]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_convergence_canary.json"
    )]
    report: PathBuf,
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("ontology_v2_convergence_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let contract_path = input_path(&repo_root, &args.contract_audit)?;
    let identity_path = input_path(&repo_root, &args.identity_compatibility)?;
    let endpoint_path = input_path(&repo_root, &args.endpoint_evidence)?;
    let tasks_path = input_path(&repo_root, &args.tasks)?;
    let report_path = output_path(&repo_root, &args.report)?;

    let contract_bytes = read_capped(&contract_path, MAX_REPORT_BYTES)?;
    let identity_bytes = read_capped(&identity_path, MAX_REPORT_BYTES)?;
    let endpoint_bytes = read_capped(&endpoint_path, MAX_REPORT_BYTES)?;
    let task_bytes = read_capped(&tasks_path, MAX_TASK_BYTES)?;

    let contract: OntologyContractAuditReport = serde_json::from_slice(&contract_bytes)
        .with_context(|| format!("parse {}", display_path(&repo_root, &contract_path)))?;
    let identity: IdentityCompatibilityReport = serde_json::from_slice(&identity_bytes)
        .with_context(|| format!("parse {}", display_path(&repo_root, &identity_path)))?;
    let endpoint: OntologyEndpointEvidence = serde_json::from_slice(&endpoint_bytes)
        .with_context(|| format!("parse {}", display_path(&repo_root, &endpoint_path)))?;
    contract.validate()?;
    identity.validate()?;
    endpoint.validate()?;

    let task_text = std::str::from_utf8(&task_bytes).context("task suite must be UTF-8")?;
    let task_suite = parse_tasks_cypher(task_text).map_err(anyhow::Error::msg)?;
    let task_projection = project_task_edge_wires(&task_suite);
    anyhow::ensure!(
        task_projection.diagnostics.is_empty(),
        "task wire projection is incomplete: {:?}",
        task_projection.diagnostics
    );
    let task_wires = task_projection
        .wires
        .into_iter()
        .map(|(wire, _task)| wire)
        .collect::<Vec<_>>();
    let task_wire_source_sha256 = fingerprint(&task_bytes);

    let report = OntologyV2ConvergenceUseCase::compose(OntologyV2ConvergenceInput {
        bindings: vec![
            binding(
                "checkpoint_audit",
                &repo_root,
                &contract_path,
                &contract_bytes,
                &contract.report_sha256,
            ),
            binding(
                "checkpoint_identity",
                &repo_root,
                &identity_path,
                &identity_bytes,
                &identity.report_sha256,
            ),
            binding(
                "endpoint_runtime",
                &repo_root,
                &endpoint_path,
                &endpoint_bytes,
                &endpoint.report_sha256,
            ),
        ],
        checkpoint_audit: contract,
        checkpoint_identity: identity,
        endpoint_runtime: endpoint,
        task_wires,
        task_wire_source_sha256,
    })?;
    atomic_write(&report_path, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "ontology_v2_convergence verdict={:?} remainders={} checkpoint={} endpoint={} report_sha256={} import_eligible=false",
        report.verdict,
        report.remainders.len(),
        report.checkpoint_graph_sha256,
        report.endpoint_graph_sha256,
        report.report_sha256,
    );
    Ok(())
}

fn binding(
    role: &str,
    repo_root: &Path,
    path: &Path,
    bytes: &[u8],
    report_sha256: &str,
) -> EvidenceBinding {
    EvidenceBinding {
        role: role.into(),
        source_ref: display_path(repo_root, path),
        source_sha256: fingerprint(bytes),
        report_sha256: report_sha256.into(),
    }
}

fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "input must be a regular file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.len() <= limit,
        "input exceeds {} bytes: {}",
        limit,
        path.display()
    );
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    anyhow::ensure!(
        u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= limit,
        "input exceeds {} bytes: {}",
        limit,
        path.display()
    );
    Ok(bytes)
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
    anyhow::ensure!(
        resolved.starts_with(repo_root),
        "input path escapes repository: {}",
        path.display()
    );
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
    anyhow::ensure!(
        parent == artifact_root && !file_name.to_string_lossy().starts_with('.'),
        "report must be a visible file directly under artifacts/ontology"
    );
    Ok(parent.join(file_name))
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{}", ContentDigest::of_bytes(bytes).as_str())
}
