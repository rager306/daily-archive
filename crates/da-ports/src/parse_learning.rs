//! Provider-neutral contracts for native scholarly-parse learning (M464/S17).
//! The port deliberately carries identities and bounded receipts, never raw
//! PDF, TEI, prompts, completions, or provider-specific payloads.

use crate::agent_memory::TrajectoryRecord;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const CVF_SCHEMA_VERSION: u16 = 1;
pub const RVF_SCHEMA_VERSION: u16 = 1;
pub const LEARNING_CONTRACT_VERSION: &str = "parse-learning.v1";
pub const MAX_ID_LEN: usize = 128;
pub const MAX_RECEIPT_BYTES: usize = 64 * 1024;
const RVF_RECORD_SCHEMA_VERSION: u16 = 1;
pub const MAX_TRAINING_SAMPLES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LearningContractError {
    #[error("learning contract has an unsupported schema version")]
    UnsupportedVersion,
    #[error("learning contract identity is not a lowercase SHA-256 digest")]
    InvalidDigest,
    #[error("learning contract identity is blank or too long")]
    InvalidIdentity,
    #[error("train and validation identities overlap")]
    SplitOverlap,
    #[error("learning contract bound is zero or exceeds the safe cap")]
    InvalidBound,
    #[error("learning paths overlap")]
    PathOverlap,
    #[error("raw content is forbidden in a learning receipt")]
    RawContentForbidden,
    #[error("candidate receipt violates the candidate-only promotion lock")]
    PromotionLockViolation,
}

/// A content-derived identity. Serialization is closed and validates on read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn new(value: impl Into<String>) -> Result<Self, LearningContractError> {
        let value = value.into();
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(LearningContractError::InvalidDigest);
        }
        let value = value.to_ascii_lowercase();
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitIdentity {
    pub train: Vec<Sha256Digest>,
    pub validation: Vec<Sha256Digest>,
}
impl SplitIdentity {
    pub fn validate(&self) -> Result<(), LearningContractError> {
        let train: BTreeSet<_> = self.train.iter().collect();
        let validation: BTreeSet<_> = self.validation.iter().collect();
        if train.is_empty()
            || validation.is_empty()
            || train.intersection(&validation).next().is_some()
        {
            return Err(LearningContractError::SplitOverlap);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningLineage {
    pub source: Sha256Digest,
    pub corpus: Sha256Digest,
    pub config: Sha256Digest,
    pub prior_policy: Sha256Digest,
    pub conservation: Sha256Digest,
    pub split: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CvfEvent {
    pub schema_version: u16,
    pub event_id: Sha256Digest,
    pub sequence: u64,
    pub lineage: LearningLineage,
    pub split: SplitIdentity,
    pub evidence_digest: Sha256Digest,
}
impl CvfEvent {
    pub fn validate(&self) -> Result<(), LearningContractError> {
        if self.schema_version != CVF_SCHEMA_VERSION {
            return Err(LearningContractError::UnsupportedVersion);
        }
        self.split.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceReceipt {
    pub schema_version: u16,
    pub generation: u64,
    pub event_count: u64,
    pub ordered_digest: Sha256Digest,
    pub lineage: LearningLineage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointReceipt {
    pub schema_version: u16,
    pub generation: u64,
    pub event_sequence: u64,
    pub experience_digest: Sha256Digest,
    pub previous_checkpoint: Option<Sha256Digest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateReceipt {
    pub schema_version: u16,
    pub candidate_id: Sha256Digest,
    pub lineage: LearningLineage,
    pub prior_candidate: Option<Sha256Digest>,
    pub conservation_ok: bool,
    pub import_eligible: bool,
    pub promotion_state: PromotionState,
    pub deployed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningBankReceipt {
    pub schema_version: u16,
    pub input_digest: Sha256Digest,
    pub patterns_digest: Sha256Digest,
    pub pattern_count: u32,
    pub config_digest: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateManifest {
    pub receipt: CandidateReceipt,
    pub rank: u8,
    pub hidden_dim: u32,
    pub train_count: u32,
    pub validation_count: u32,
    pub train_quality: f32,
    pub validation_quality: f32,
    pub validation_gap: f32,
    pub weights_digest: Sha256Digest,
}
impl CandidateManifest {
    pub fn validate(&self) -> Result<(), LearningContractError> {
        self.receipt.validate()?;
        if !(1..=2).contains(&self.rank)
            || self.hidden_dim == 0
            || self.train_count == 0
            || self.validation_count == 0
            || !self.train_quality.is_finite()
            || !self.validation_quality.is_finite()
            || !self.validation_gap.is_finite()
        {
            return Err(LearningContractError::InvalidBound);
        }
        Ok(())
    }
}
impl CandidateReceipt {
    pub fn validate(&self) -> Result<(), LearningContractError> {
        if self.schema_version != RVF_SCHEMA_VERSION {
            return Err(LearningContractError::UnsupportedVersion);
        }
        if self.import_eligible
            || self.deployed
            || self.promotion_state != PromotionState::Candidate
            || !self.conservation_ok
        {
            return Err(LearningContractError::PromotionLockViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionState {
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticReceipt {
    pub schema_version: u16,
    pub evaluation_id: Sha256Digest,
    pub candidate_id: Sha256Digest,
    pub corpus: Sha256Digest,
    pub n: u64,
    pub metric_digest: Sha256Digest,
    pub failure_count: u64,
    pub producer: String,
}

/// Canonical RVF journal entry. The adapter frames these records on disk;
/// callers never need to know the framing or legacy JSONL representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum RvfRecord {
    Trajectory {
        schema_version: u16,
        record: TrajectoryRecord,
    },
    ReasoningBank {
        schema_version: u16,
        receipt: ReasoningBankReceipt,
    },
    Candidate {
        schema_version: u16,
        manifest: CandidateManifest,
    },
}
impl RvfRecord {
    pub fn trajectory(record: TrajectoryRecord) -> Self {
        Self::Trajectory {
            schema_version: RVF_RECORD_SCHEMA_VERSION,
            record,
        }
    }
    pub fn validate(&self) -> Result<(), LearningContractError> {
        match self {
            Self::Trajectory {
                schema_version,
                record,
            } => {
                if *schema_version != RVF_RECORD_SCHEMA_VERSION
                    || record.task_id.is_empty()
                    || record.paper_id.is_empty()
                    || record.model.is_empty()
                {
                    return Err(LearningContractError::InvalidIdentity);
                }
            }
            Self::ReasoningBank {
                schema_version,
                receipt,
            } => {
                if *schema_version != RVF_RECORD_SCHEMA_VERSION || receipt.pattern_count == 0 {
                    return Err(LearningContractError::InvalidBound);
                }
            }
            Self::Candidate {
                schema_version,
                manifest,
            } => {
                if *schema_version != RVF_RECORD_SCHEMA_VERSION {
                    return Err(LearningContractError::UnsupportedVersion);
                }
                manifest.validate()?;
            }
        }
        Ok(())
    }
    pub fn trajectory_record(&self) -> &TrajectoryRecord {
        match self {
            Self::Trajectory { record, .. } => record,
            _ => panic!("not a trajectory record"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RvfAppendReceipt {
    pub sequence: u64,
    pub record_digest: Sha256Digest,
    pub ordered_digest: Sha256Digest,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RvfStoreError {
    #[error("RVF I/O failure: {0}")]
    Io(String),
    #[error("RVF record is truncated at byte {offset}")]
    Truncated { offset: u64 },
    #[error("RVF record is corrupt at byte {offset}")]
    Corrupt { offset: u64 },
    #[error("RVF record is unknown or unsupported")]
    UnknownRecord,
    #[error("RVF record exceeds the bounded size")]
    RecordTooLarge,
    #[error("RVF contract: {0}")]
    Contract(#[from] LearningContractError),
}

/// Persistent experience boundary. RVF is authoritative; indexes and legacy
/// files are intentionally absent from this interface.
pub trait RvfExperienceStore: Send + Sync {
    fn append(&mut self, record: RvfRecord) -> Result<RvfAppendReceipt, RvfStoreError>;
    fn read_all(&self) -> Result<Vec<RvfRecord>, RvfStoreError>;
    fn verify(&self) -> Result<ExperienceReceipt, RvfStoreError>;
    fn checkpoint(&self) -> Result<CheckpointReceipt, RvfStoreError>;
    fn migrate_legacy_jsonl(&mut self, path: &std::path::Path) -> Result<u64, RvfStoreError>;
}

pub const MAX_LEARNING_CAPACITY: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningStorageConfig {
    pub rvf_path: PathBuf,
    pub redb_index_path: Option<PathBuf>,
    pub rvf_capacity: usize,
    pub redb_capacity: usize,
}
impl LearningStorageConfig {
    pub fn validate(&self) -> Result<(), LearningContractError> {
        if self.rvf_capacity == 0
            || self.rvf_capacity > MAX_LEARNING_CAPACITY
            || self.redb_capacity == 0
            || self.redb_capacity > MAX_LEARNING_CAPACITY
        {
            return Err(LearningContractError::InvalidBound);
        }
        if let Some(redb) = &self.redb_index_path {
            let rvf = normalize(&self.rvf_path);
            let redb = normalize(redb);
            if rvf == redb || rvf.starts_with(&redb) || redb.starts_with(&rvf) {
                return Err(LearningContractError::PathOverlap);
            }
        }
        Ok(())
    }
}
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Reject accidental persistence of source payloads at a contract boundary.
pub fn reject_raw_content(value: &str) -> Result<(), LearningContractError> {
    let lower = value.to_ascii_lowercase();
    if [
        "<tei",
        "pdf_bytes",
        "raw_tei",
        "prompt",
        "completion",
        "body_text",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Err(LearningContractError::RawContentForbidden)
    } else {
        Ok(())
    }
}
