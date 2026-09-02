//! Deterministic, candidate-only SONA MicroLoRA training (M464/S17/T04).
//!
//! The application owns the bounded use case, while RVF remains the injected
//! canonical store. No route reload, promotion transition, graph write, or
//! import authorization is reachable from this module.

use da_ports::agent_memory::TrajectoryRecord;
use da_ports::parse_learning::{
    CandidateManifest, CandidateReceipt, LearningContractError, LearningLineage,
    MAX_TRAINING_SAMPLES, PromotionState, RVF_SCHEMA_VERSION, ReasoningBankReceipt,
    RvfExperienceStore, RvfRecord, Sha256Digest, SplitIdentity,
};
use ruvector_sona::MicroLoRA;
use ruvector_sona::reasoning_bank::{PatternConfig, ReasoningBank};
use ruvector_sona::types::LearningSignal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

pub const MICRO_LORA_RANK: usize = 1;
pub const MICRO_LORA_DIM: usize = 8;
pub const TRAINING_CONFIG: &str = "sona-microlora-v1;rank=1;dim=8;seed=deterministic";

#[derive(Debug, Error)]
pub enum TrainingError {
    #[error("training input is not trainable: {0}")]
    NotTrainable(&'static str),
    #[error("training split is invalid")]
    InvalidSplit,
    #[error("training input contains duplicate identity")]
    DuplicateIdentity,
    #[error("training input exceeds the bounded cap")]
    TooManySamples,
    #[error("vendor training failed: {0}")]
    Vendor(String),
    #[error("RVF persistence failed: {0}")]
    Store(String),
    #[error("candidate artifact failed: {0}")]
    Artifact(String),
    #[error("learning contract failed: {0}")]
    Contract(#[from] LearningContractError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingConfig {
    pub hidden_dim: usize,
    pub rank: usize,
    pub learning_rate: f32,
    pub min_samples: usize,
}
impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            hidden_dim: MICRO_LORA_DIM,
            rank: MICRO_LORA_RANK,
            learning_rate: 0.05,
            min_samples: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedCandidate {
    pub manifest: CandidateManifest,
    pub down_projection: Vec<f32>,
    pub up_projection: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingOutcome {
    pub bank: ReasoningBankReceipt,
    pub manifest: CandidateManifest,
    pub candidate_path: Option<std::path::PathBuf>,
}

fn digest_bytes(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(bytes))).expect("sha256 digest is valid")
}
fn digest_json<T: Serialize>(value: &T) -> Result<Sha256Digest, TrainingError> {
    Ok(digest_bytes(
        &serde_json::to_vec(value).map_err(|e| TrainingError::Artifact(e.to_string()))?,
    ))
}
fn identity(value: &str) -> Sha256Digest {
    digest_bytes(value.as_bytes())
}

fn embedding(record: &TrajectoryRecord, dim: usize) -> Vec<f32> {
    let mut out = vec![0.0; dim];
    let quality = record
        .grounding_hint
        .unwrap_or(if record.ok { 1.0 } else { 0.0 }) as f32;
    let latency = (record.latency_ms as f32 / 30_000.0).min(1.0);
    let tokens = record.completion_tokens.unwrap_or(0) as f32 / 2_000.0;
    let values = [
        quality.clamp(0.0, 1.0),
        latency,
        if record.ok { 1.0 } else { 0.0 },
        tokens.min(1.0),
        1.0 - quality.clamp(0.0, 1.0),
    ];
    out[..values.len().min(dim)].copy_from_slice(&values[..values.len().min(dim)]);
    out
}

fn validate_inputs(
    train: &[TrajectoryRecord],
    validation: &[TrajectoryRecord],
    config: &TrainingConfig,
) -> Result<(), TrainingError> {
    if config.hidden_dim == 0
        || config.rank != MICRO_LORA_RANK
        || config.hidden_dim != MICRO_LORA_DIM
        || config.min_samples == 0
    {
        return Err(TrainingError::NotTrainable(
            "invalid MicroLoRA dimensions or bounds",
        ));
    }
    if train.len() < config.min_samples || validation.is_empty() {
        return Err(TrainingError::NotTrainable(
            "minimum train and validation corpus not met",
        ));
    }
    if train.len() + validation.len() > MAX_TRAINING_SAMPLES {
        return Err(TrainingError::TooManySamples);
    }
    let mut seen = std::collections::BTreeSet::new();
    for record in train.iter().chain(validation) {
        if record.prompt_sha256.is_empty() || !seen.insert(record.prompt_sha256.clone()) {
            return Err(TrainingError::DuplicateIdentity);
        }
    }
    Ok(())
}

/// Train exactly one deterministic candidate and append its bank/candidate
/// receipts to the canonical RVF journal. Existing candidate state is never
/// replaced or promoted; retries become RVF content-derived duplicates.
pub fn train_microlora_candidate<S: RvfExperienceStore>(
    store: &mut S,
    train: &[TrajectoryRecord],
    validation: &[TrajectoryRecord],
    lineage: LearningLineage,
    prior_candidate: Option<Sha256Digest>,
    config: TrainingConfig,
    candidate_path: Option<&Path>,
) -> Result<TrainingOutcome, TrainingError> {
    validate_inputs(train, validation, &config)?;
    let split = SplitIdentity {
        train: train.iter().map(|r| identity(&r.prompt_sha256)).collect(),
        validation: validation
            .iter()
            .map(|r| identity(&r.prompt_sha256))
            .collect(),
    };
    split.validate()?;
    let config_digest = digest_json(&config)?;
    let ordered_input = (train, validation);
    let input_digest = digest_json(&ordered_input)?;

    let bank_config = PatternConfig {
        k_clusters: 1,
        embedding_dim: config.hidden_dim,
        max_iterations: 50,
        convergence_threshold: 0.001,
        min_cluster_size: 2,
        max_trajectories: MAX_TRAINING_SAMPLES,
        quality_threshold: 0.0,
    };
    let mut bank = ReasoningBank::new(bank_config);
    for (index, record) in train.iter().enumerate() {
        let mut trajectory = ruvector_sona::QueryTrajectory::new(
            index as u64 + 1,
            embedding(record, config.hidden_dim),
        );
        trajectory.finalize(
            record
                .grounding_hint
                .unwrap_or(if record.ok { 1.0 } else { 0.0 }) as f32,
            record.latency_ms.saturating_mul(1000),
        );
        bank.add_trajectory(&trajectory);
    }
    let patterns = bank.extract_patterns();
    if patterns.is_empty() {
        return Err(TrainingError::NotTrainable(
            "ReasoningBank produced no patterns",
        ));
    }
    let patterns_digest = digest_json(&patterns)?;
    let bank_receipt = ReasoningBankReceipt {
        schema_version: RVF_SCHEMA_VERSION,
        input_digest: input_digest.clone(),
        patterns_digest,
        pattern_count: patterns.len() as u32,
        config_digest: config_digest.clone(),
    };
    store
        .append(RvfRecord::ReasoningBank {
            schema_version: RVF_SCHEMA_VERSION,
            receipt: bank_receipt.clone(),
        })
        .map_err(|e| TrainingError::Store(e.to_string()))?;

    let mut adapter = MicroLoRA::new(config.hidden_dim, config.rank);
    for record in train {
        let vector = embedding(record, config.hidden_dim);
        let signal = LearningSignal::with_gradient(
            vector.clone(),
            vector,
            record
                .grounding_hint
                .unwrap_or(if record.ok { 1.0 } else { 0.0 }) as f32,
        );
        adapter.accumulate_gradient(&signal);
        adapter.apply_accumulated(config.learning_rate);
    }
    let (down, up) = adapter.get_weights();
    if down.iter().chain(up).any(|value| !value.is_finite()) {
        return Err(TrainingError::Vendor("non-finite MicroLoRA weights".into()));
    }
    let weights_digest = digest_json(&(down, up))?;
    let train_quality = train
        .iter()
        .map(|r| r.grounding_hint.unwrap_or(if r.ok { 1.0 } else { 0.0 }) as f32)
        .sum::<f32>()
        / train.len() as f32;
    let validation_quality = validation
        .iter()
        .map(|r| r.grounding_hint.unwrap_or(if r.ok { 1.0 } else { 0.0 }) as f32)
        .sum::<f32>()
        / validation.len() as f32;
    let receipt = CandidateReceipt {
        schema_version: RVF_SCHEMA_VERSION,
        candidate_id: digest_json(&(input_digest, config_digest, weights_digest.clone()))?,
        lineage,
        prior_candidate,
        conservation_ok: true,
        import_eligible: false,
        promotion_state: PromotionState::Candidate,
        deployed: false,
    };
    let manifest = CandidateManifest {
        receipt,
        rank: config.rank as u8,
        hidden_dim: config.hidden_dim as u32,
        train_count: train.len() as u32,
        validation_count: validation.len() as u32,
        train_quality,
        validation_quality,
        validation_gap: (train_quality - validation_quality).abs(),
        weights_digest,
    };
    manifest.validate()?;
    store
        .append(RvfRecord::Candidate {
            schema_version: RVF_SCHEMA_VERSION,
            manifest: manifest.clone(),
        })
        .map_err(|e| TrainingError::Store(e.to_string()))?;
    let candidate_path = candidate_path
        .map(|path| {
            let persisted = PersistedCandidate {
                manifest: manifest.clone(),
                down_projection: down.clone(),
                up_projection: up.clone(),
            };
            let bytes = serde_json::to_vec_pretty(&persisted)
                .map_err(|e| TrainingError::Artifact(e.to_string()))?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, bytes).map_err(|e| TrainingError::Artifact(e.to_string()))?;
            std::fs::rename(&tmp, path).map_err(|e| TrainingError::Artifact(e.to_string()))?;
            Ok::<_, TrainingError>(path.to_path_buf())
        })
        .transpose()?;
    Ok(TrainingOutcome {
        bank: bank_receipt,
        manifest,
        candidate_path,
    })
}
