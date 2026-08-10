//! Bounded ACAP contract audit CLI canary (M371/S07/T01).
//!
//! Reads the live policy YAML, the embedded 200-paper materialization artifact,
//! and the canonical aggregate path, composes typed observations, runs the
//! detect-only audit, and emits a durable artifact. Does not mutate the graph,
//! does not unlock D127, and does not change governor semantics.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::acap_contract_audit::{
    ACAP_AUDIT_INPUT_SCHEMA_VERSION, AcapContractAuditInput, AcapContractAuditUseCase,
    AggregateReportSurface, EmbeddedReportsSurface, GovernorContractSurface,
    PolicyLayerObservation, RustReportSurface,
};
use da_domain::{ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ACAP_POLICY_ID, ACAP_REPORT_SCHEMA_VERSION};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_POLICY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Parser)]
struct Args {
    /// Path to the ACAP policy YAML (relative to repo root).
    #[arg(
        long,
        default_value = "scripts/governor/checks/article_context_assurance.yaml"
    )]
    policy: PathBuf,

    /// Path to the embedded materialization artifact (relative to repo root).
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.json"
    )]
    materialization: PathBuf,

    /// Output path for the audit artifact (relative to repo root).
    #[arg(
        long,
        default_value = "artifacts/article_context/acap_contract_audit.json"
    )]
    report: PathBuf,
}

#[derive(Default, Deserialize)]
struct PolicyYaml {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    policy_id: String,
    #[serde(default)]
    rollout: String,
    #[serde(default)]
    metric_definitions: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    layers: Vec<LayerYaml>,
    #[serde(default)]
    fsm: FsmYaml,
    #[serde(default)]
    assurance_report_contract: ReportContractYaml,
}

#[derive(Default, Deserialize)]
struct LayerYaml {
    id: String,
    #[serde(default)]
    enforcement: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    hard_metrics: Vec<String>,
    #[serde(default)]
    quality_metrics: Vec<String>,
}

#[derive(Default, Deserialize)]
struct FsmYaml {
    #[serde(default)]
    states: Vec<String>,
}

#[derive(Default, Deserialize)]
struct ReportContractYaml {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    required_top_level: Vec<String>,
}

#[derive(Default, Deserialize)]
struct MaterializationRoot {
    #[serde(default)]
    materialization: MaterializationBody,
    #[serde(default)]
    papers: Vec<PaperEntry>,
}

#[derive(Default, Deserialize)]
struct MaterializationBody {
    #[serde(default)]
    waves: Vec<WaveBody>,
    #[serde(default)]
    papers: Vec<PaperEntry>,
}

#[derive(Default, Deserialize)]
struct WaveBody {
    #[serde(default)]
    papers: Vec<PaperEntry>,
}

#[derive(Default, Deserialize)]
struct PaperEntry {
    #[serde(default)]
    assurance: Option<AssuranceEntry>,
}

#[derive(Default, Deserialize)]
struct AssuranceEntry {
    #[serde(default)]
    layer_results: Vec<LayerResultEntry>,
}

#[derive(Default, Deserialize)]
struct LayerResultEntry {
    #[serde(default)]
    layer_id: String,
}

impl MaterializationRoot {
    fn papers(&self) -> Vec<&PaperEntry> {
        let mut out = Vec::new();
        for wave in &self.materialization.waves {
            out.extend(wave.papers.iter());
        }
        out.extend(self.materialization.papers.iter());
        out.extend(self.papers.iter());
        out
    }
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("acap_contract_audit");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;

    let policy_path = resolve(&repo_root, &args.policy);
    let mat_path = resolve(&repo_root, &args.materialization);
    let report_path = resolve(&repo_root, &args.report);

    let policy_bytes = read_capped(&policy_path, MAX_POLICY_BYTES)?;
    let policy: PolicyYaml = serde_yaml::from_slice(&policy_bytes)
        .with_context(|| format!("parse {}", policy_path.display()))?;
    let policy_sha = hex::encode(Sha256::digest(&policy_bytes));

    // Parse materialization artifact for embedded report observations.
    let (embedded_count, embedded_layers) = if mat_path.exists() {
        let mat_bytes = read_capped(&mat_path, MAX_ARTIFACT_BYTES)?;
        let artifact: MaterializationRoot = serde_json::from_slice(&mat_bytes)
            .with_context(|| format!("parse {}", mat_path.display()))?;
        let papers = artifact.papers();
        let mut layers = std::collections::BTreeSet::new();
        for paper in &papers {
            if let Some(assurance) = &paper.assurance {
                for layer in &assurance.layer_results {
                    layers.insert(layer.layer_id.clone());
                }
            }
        }
        (papers.len(), layers.into_iter().collect::<Vec<_>>())
    } else {
        (0, vec![])
    };
    let mat_sha = if mat_path.exists() {
        let bytes = std::fs::read(&mat_path)?;
        hex::encode(Sha256::digest(&bytes))
    } else {
        String::new()
    };

    // Check canonical aggregate path and merge its observed layers into the
    // cross-surface audit view (product aggregate may project more layers than
    // the raw materialization per-article assurance embeds).
    let aggregate_path = policy.assurance_report_contract.path.clone();
    let aggregate_full = if aggregate_path.is_empty() {
        None
    } else if Path::new(&aggregate_path).is_absolute() {
        Some(PathBuf::from(&aggregate_path))
    } else {
        Some(repo_root.join(&aggregate_path))
    };
    let aggregate_present = aggregate_full
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);
    let mut embedded_layers = embedded_layers;
    let mut embedded_source = "artifacts/article_context/materialization_all_200.json".to_string();
    if let Some(full) = aggregate_full.as_ref().filter(|path| path.is_file()) {
        if let Ok(bytes) = std::fs::read(full) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(layers) = value.get("layer_results").and_then(|v| v.as_array()) {
                    let mut set: std::collections::BTreeSet<String> =
                        embedded_layers.into_iter().collect();
                    for layer in layers {
                        let status = layer.get("status").and_then(|v| v.as_str()).unwrap_or("");
                        if status == "observed" {
                            if let Some(id) = layer.get("layer_id").and_then(|v| v.as_str()) {
                                set.insert(id.to_string());
                            }
                        }
                    }
                    embedded_layers = set.into_iter().collect();
                    embedded_source = format!(
                        "artifacts/article_context/materialization_all_200.json+{}",
                        display_path(&repo_root, full)
                    );
                }
            }
        }
    }

    // Build typed observations.
    let policy_layers = policy
        .layers
        .iter()
        .map(|layer| PolicyLayerObservation {
            id: layer.id.clone(),
            enforcement: layer.enforcement.clone(),
            state: layer.state.clone(),
            hard_metrics: layer.hard_metrics.clone(),
            quality_metrics: layer.quality_metrics.clone(),
            source_ref: format!("article_context_assurance.yaml#layers[{}]", layer.id),
        })
        .collect::<Vec<_>>();

    let input = AcapContractAuditInput {
        schema_version: ACAP_AUDIT_INPUT_SCHEMA_VERSION.into(),
        source_fingerprints: BTreeMap::from([
            ("policy_yaml".into(), policy_sha),
            ("materialization_artifact".into(), mat_sha),
        ]),
        policy_id: policy.policy_id.clone(),
        policy_schema_version: policy.schema_version.clone(),
        policy_rollout: policy.rollout.clone(),
        policy_layers,
        policy_metric_names: policy
            .metric_definitions
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        policy_fsm_states: policy.fsm.states.clone(),
        aggregate_schema_declared: policy.assurance_report_contract.schema_version.clone(),
        aggregate_path_declared: aggregate_path.clone(),
        aggregate_required_keys: policy.assurance_report_contract.required_top_level.clone(),
        rust_surface: RustReportSurface {
            per_article_schema: ACAP_REPORT_SCHEMA_VERSION.into(),
            policy_id: ACAP_POLICY_ID.into(),
            aggregate_schema: ACAP_AGGREGATE_REPORT_SCHEMA_VERSION.into(),
            hard_failures_kind: "list".into(),
            source_ref: "da-domain/src/assurance.rs".into(),
        },
        embedded_reports: EmbeddedReportsSurface {
            count: embedded_count,
            layers_observed: embedded_layers,
            source_ref: embedded_source,
        },
        aggregate_report: AggregateReportSurface {
            present: aggregate_present,
            declared_path: aggregate_path,
            source_ref: "fs_check".into(),
        },
        governor_contract: GovernorContractSurface {
            expected_schema_version: policy.assurance_report_contract.schema_version.clone(),
            // Governor currently checks `result.get("hard_failures", 0) != 0` which
            // expects a numeric value. Rust produces Vec<String>.
            // Product aggregate emits list[str]; governor dual-accepts list/int (T05).
            expected_hard_failures_kind: "list".into(),
            required_top_level: policy.assurance_report_contract.required_top_level.clone(),
            source_ref: "governor/application/trajectory_service.py".into(),
        },
        import_eligible: false,
    };

    let report =
        AcapContractAuditUseCase::audit(input).map_err(|e| anyhow!("audit failed: {e}"))?;

    let report_json = serde_json::to_vec_pretty(&report)?;
    atomic_write(&report_path, &report_json)?;

    let error_count = report
        .findings
        .iter()
        .filter(|f| f.severity == da_application::acap_contract_audit::AcapAuditSeverity::Error)
        .count();
    println!(
        "acap_contract_audit verdict={:?} findings={} errors={} report_sha256={} import_eligible=false",
        report.verdict,
        report.findings.len(),
        error_count,
        report.report_sha256,
    );
    for finding in &report.findings {
        eprintln!(
            "  {:?} {}: {}",
            finding.severity, finding.code, finding.subject
        );
    }
    Ok(())
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn resolve(repo_root: &Path, relative: &Path) -> PathBuf {
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        repo_root.join(relative)
    }
}

fn read_capped(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let metadata = file.metadata()?;
    anyhow::ensure!(metadata.is_file(), "not a regular file: {}", path.display());
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        anyhow::bail!("file exceeds byte budget: {}", path.display());
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
    }
    Ok(())
}
