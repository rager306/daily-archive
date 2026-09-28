//! Deterministic SONA MicroLoRA/EWC++ candidate trainer (M464/S17/T04 driver).
//!
//! WHY THIS EXISTS: `parse_policy_training::train_microlora_candidate` was a fully
//! implemented candidate-only use case with **no process** — the only callers were
//! `sona_restart_equivalence.rs` tests. `learning.microlora_ewc` therefore read
//! `state: absent, surface: none` in the compatibility ledger: accurate about the
//! wiring, not about the code. This example is the missing driver.
//!
//! WHAT IT DOES NOT DO: no promotion, no route reload, no graph write, no import
//! authorization. The receipt it prints is candidate-only by construction
//! (`deployed=false`, `import_eligible=false`, `promotion_state=candidate`).
//!
//! WHY IT CAN REFUSE: the training contract requires a train/validation split whose
//! identities are disjoint and both non-empty, and identity IS `prompt_sha256`
//! (`parse_policy_training::train_microlora_candidate` builds `SplitIdentity` from it).
//! The canonical experience in `data/agent_memory/<namespace>` currently records ONE
//! prompt identity per namespace (summarizer: 55 records / 1 identity), so no disjoint
//! split exists and the run reports `not_trainable` with the measured counts instead of
//! fabricating a split. That refusal is the finding, not a failure of this driver.

use clap::Parser;
use da_adapters::RvfFileStore;
use da_adapters::agent_memory::CvfAgentMemory;
use da_application::{TrainingConfig, train_microlora_candidate};
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::{
    LearningLineage, RvfExperienceStore, Sha256Digest, parse_learning::MAX_TRAINING_SAMPLES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    about = "Train exactly one deterministic MicroLoRA/EWC++ candidate from canonical RVF experience"
)]
struct Args {
    /// Agent-memory namespace under --memory-root (e.g. summarizer).
    #[arg(long, default_value = "summarizer")]
    namespace: String,
    /// Root of the agent-memory tree. Defaults to the canonical location; point it at a
    /// fixture to exercise the training path without touching canonical experience.
    #[arg(long, default_value = "data/agent_memory")]
    memory_root: PathBuf,
    /// Write the deterministic report here. Required: a candidate run always leaves evidence.
    #[arg(long, default_value = "artifacts/parse_learning/policy_training.json")]
    report: PathBuf,
    /// Optional candidate manifest output path.
    #[arg(long)]
    candidate_out: Option<PathBuf>,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    namespace: String,
    memory_root: String,
    records_read: usize,
    distinct_prompt_identities: usize,
    /// Deterministic reason when the contract cannot be satisfied.
    not_trainable: Option<String>,
    train_count: usize,
    validation_count: usize,
    bank_input_digest: Option<String>,
    bank_patterns_digest: Option<String>,
    bank_pattern_count: Option<u32>,
    candidate_id: Option<String>,
    promotion_state: Option<String>,
    deployed: Option<bool>,
    import_eligible: Option<bool>,
    conservation_ok: Option<bool>,
    promotion_locks: PromotionLocks,
}

#[derive(Serialize)]
struct PromotionLocks {
    auto_promote: &'static str,
    graph_write: &'static str,
    import_eligible: &'static str,
    production_route: &'static str,
}

fn digest_bytes(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(bytes))).expect("sha256 digest is valid")
}

/// Namespace-scoped lineage, mirroring `agent_memory::lineage` so the training
/// receipts land in the same lineage as the trajectories they were learned from.
fn lineage(namespace: &str) -> LearningLineage {
    LearningLineage {
        source: digest_bytes(format!("source:agent_memory/{namespace}").as_bytes()),
        corpus: digest_bytes(format!("corpus:agent_memory/{namespace}").as_bytes()),
        config: digest_bytes(b"config:sona-microlora-train-v1"),
        prior_policy: digest_bytes(b"prior-policy:none"),
        conservation: digest_bytes(b"conservation:rvf-order-v1"),
        split: digest_bytes(format!("split:identity-holdout:{namespace}").as_bytes()),
    }
}

/// Split by identity: first identity in sorted order becomes validation, the rest
/// train. Deterministic, and it keeps `SplitIdentity` disjoint by construction.
fn split_by_identity(
    records: &[TrajectoryRecord],
) -> (Vec<TrajectoryRecord>, Vec<TrajectoryRecord>) {
    let mut by_identity: BTreeMap<&str, &TrajectoryRecord> = BTreeMap::new();
    for record in records {
        by_identity
            .entry(record.prompt_sha256.as_str())
            .or_insert(record);
    }
    let mut identities: Vec<&str> = by_identity.keys().copied().collect();
    identities.sort_unstable();
    // Sorted-first identity is held out; the field split is deterministic and reported.
    let (validation_ids, train_ids) = identities
        .split_first()
        .map_or((&[][..], &[][..]), |(v, t)| (std::slice::from_ref(v), t));
    let pick = |ids: &[&str]| -> Vec<TrajectoryRecord> {
        ids.iter()
            .filter_map(|id| by_identity.get(id).copied().cloned())
            .collect()
    };
    (pick(train_ids), pick(validation_ids))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // ADR-052: a composition root owns the subscriber and must HOLD the guard, or file
    // logging silently stops when it drops.
    let _log_guard = da_cli::logging::init_pipeline_logging("sona_policy_training");
    let mem_dir = args.memory_root.join(&args.namespace);
    let memory = CvfAgentMemory::open(mem_dir).map_err(|e| anyhow::anyhow!("RVF memory: {e}"))?;
    let records = memory
        .read_trajectories()
        .map_err(|e| anyhow::anyhow!("RVF read: {e}"))?;

    let identities: std::collections::BTreeSet<&str> =
        records.iter().map(|r| r.prompt_sha256.as_str()).collect();
    let (train, validation) = split_by_identity(&records);

    let mut report = Report {
        schema_version: "sona-policy-training-run.v1",
        namespace: args.namespace.clone(),
        memory_root: args.memory_root.display().to_string(),
        records_read: records.len(),
        distinct_prompt_identities: identities.len(),
        not_trainable: None,
        train_count: train.len(),
        validation_count: validation.len(),
        bank_input_digest: None,
        bank_patterns_digest: None,
        bank_pattern_count: None,
        candidate_id: None,
        promotion_state: None,
        deployed: None,
        import_eligible: None,
        conservation_ok: None,
        promotion_locks: PromotionLocks {
            auto_promote: "forbidden",
            graph_write: "forbidden",
            import_eligible: "false",
            production_route: "forbidden",
        },
    };

    if records.is_empty() {
        report.not_trainable = Some("no canonical RVF trajectories for this namespace".into());
    } else if identities.len() < 2 {
        // Measured refusal: the split contract needs >= 2 disjoint identities and
        // identity IS prompt_sha256, so a single-identity namespace cannot train.
        report.not_trainable = Some(format!(
            "identity diversity is {} but the split contract needs >= 2 disjoint \
             prompt_sha256 identities (train and validation must both be non-empty and \
             disjoint); the recorder currently writes one identity per namespace",
            identities.len()
        ));
    } else if train.is_empty() || validation.is_empty() {
        report.not_trainable = Some("split produced an empty side".into());
    } else if train.len() + validation.len() > MAX_TRAINING_SAMPLES {
        report.not_trainable = Some(format!(
            "{} samples exceed the training cap {MAX_TRAINING_SAMPLES}",
            train.len() + validation.len()
        ));
    } else {
        // Train on the SAME canonical store the trajectories were read from, so the
        // candidate and the bank receipts land in the lineage they came from.
        let rvf_path = args
            .memory_root
            .join(&args.namespace)
            .join("experience.rvf");
        let mut store = RvfFileStore::open(rvf_path, lineage(&args.namespace))
            .map_err(|e| anyhow::anyhow!("RVF store: {e}"))?;
        let copied = store
            .read_all()
            .map_err(|e| anyhow::anyhow!("RVF read: {e}"))?;
        if copied.is_empty() {
            report.not_trainable = Some("canonical RVF journal has no records to append to".into());
        } else {
            match train_microlora_candidate(
                &mut store,
                &train,
                &validation,
                lineage(&args.namespace),
                None,
                TrainingConfig::default(),
                args.candidate_out.as_deref(),
            ) {
                Ok(outcome) => {
                    report.bank_input_digest = Some(outcome.bank.input_digest.as_str().to_string());
                    report.bank_patterns_digest =
                        Some(outcome.bank.patterns_digest.as_str().to_string());
                    report.bank_pattern_count = Some(outcome.bank.pattern_count);
                    report.candidate_id =
                        Some(outcome.manifest.receipt.candidate_id.as_str().to_string());
                    report.promotion_state =
                        Some(format!("{:?}", outcome.manifest.receipt.promotion_state));
                    report.deployed = Some(outcome.manifest.receipt.deployed);
                    report.import_eligible = Some(outcome.manifest.receipt.import_eligible);
                    report.conservation_ok = Some(outcome.manifest.receipt.conservation_ok);
                }
                Err(error) => report.not_trainable = Some(format!("{error:?}")),
            }
        }
    }

    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;

    println!(
        "== sona-policy-training: {} ==\nrecords={} identities={} train={} validation={}",
        report.namespace,
        report.records_read,
        report.distinct_prompt_identities,
        report.train_count,
        report.validation_count
    );
    match &report.not_trainable {
        Some(reason) => println!("not_trainable: {reason}"),
        None => println!(
            "candidate={} promotion_state={} deployed={} import_eligible={} conservation_ok={}",
            report.candidate_id.as_deref().unwrap_or("-"),
            report.promotion_state.as_deref().unwrap_or("-"),
            report.deployed.unwrap_or(false),
            report.import_eligible.unwrap_or(false),
            report.conservation_ok.unwrap_or(false),
        ),
    }
    println!("report: {}", args.report.display());
    Ok(())
}
