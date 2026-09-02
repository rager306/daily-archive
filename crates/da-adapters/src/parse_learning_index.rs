//! Feature-gated, destructive-rebuild learning index (M464/S17/T03).
//!
//! RVF remains the only authority. This redb file contains derived lookups
//! and a binding to the verified RVF generation/digest; it can be deleted at
//! any time without changing experience or checkpoint state.

use crate::parse_experience_rvf::RvfFileStore;
use da_ports::parse_learning::{ExperienceReceipt, RvfExperienceStore, RvfRecord, Sha256Digest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const META_TABLE: redb::TableDefinition<'_, &str, &str> =
    redb::TableDefinition::new("parse_learning_index_meta_v1");
const EVENT_TABLE: redb::TableDefinition<'_, &str, &str> =
    redb::TableDefinition::new("parse_learning_index_events_v1");
const CORPUS_TABLE: redb::TableDefinition<'_, &str, u64> =
    redb::TableDefinition::new("parse_learning_index_corpus_v1");
const POLICY_TABLE: redb::TableDefinition<'_, &str, u64> =
    redb::TableDefinition::new("parse_learning_index_policy_v1");
const SPLIT_TABLE: redb::TableDefinition<'_, &str, u64> =
    redb::TableDefinition::new("parse_learning_index_split_v1");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningIndexError {
    OpenFailed,
    ReadFailed,
    WriteFailed,
    CorruptIndex,
    CanonicalUnavailable,
    BindingMismatch,
}

impl std::fmt::Display for LearningIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::OpenFailed => "redb learning index cannot be opened",
            Self::ReadFailed => "redb learning index read failed",
            Self::WriteFailed => "redb learning index write failed",
            Self::CorruptIndex => "redb learning index is corrupt",
            Self::CanonicalUnavailable => "verified RVF canonical experience is unavailable",
            Self::BindingMismatch => "redb learning index is not bound to the verified RVF",
        };
        f.write_str(text)
    }
}
impl std::error::Error for LearningIndexError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningIndexReceipt {
    pub schema_version: u16,
    pub generation: u64,
    pub experience_digest: Sha256Digest,
    pub event_count: u64,
    pub corpus_count: u64,
    pub policy_count: u64,
    pub split_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedEvent {
    sequence: u64,
    record_digest: Sha256Digest,
    task_id: String,
    paper_id: String,
    corpus: Sha256Digest,
    policy: Sha256Digest,
    split: Sha256Digest,
}

#[derive(Debug)]
pub struct LearningIndex {
    path: PathBuf,
    db: redb::Database,
}

impl LearningIndex {
    /// Open an existing or new accelerator. Opening never invents canonical
    /// state; a missing row is an index miss.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, LearningIndexError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| LearningIndexError::OpenFailed)?;
        }
        let db = redb::Database::create(&path).map_err(|_| LearningIndexError::OpenFailed)?;
        Ok(Self { path, db })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Build a new database from a fully verified RVF, then atomically publish
    /// it. A failed/interrupted build only leaves the temporary file.
    pub fn rebuild_from_rvf(
        path: impl AsRef<Path>,
        rvf: &RvfFileStore,
    ) -> Result<LearningIndexReceipt, LearningIndexError> {
        if !rvf.exists() {
            return Err(LearningIndexError::CanonicalUnavailable);
        }
        let receipt = rvf
            .verify()
            .map_err(|_| LearningIndexError::CanonicalUnavailable)?;
        let records = rvf
            .read_all()
            .map_err(|_| LearningIndexError::CanonicalUnavailable)?;
        if records.len() as u64 != receipt.event_count || receipt.event_count == 0 {
            return Err(LearningIndexError::CanonicalUnavailable);
        }
        let target = path.as_ref().to_path_buf();
        let tmp = temp_path(&target);
        let build = (|| {
            let index = Self::open(&tmp)?;
            let result = index.populate(&receipt, &records);
            drop(index);
            result
        })();
        if let Err(error) = build {
            let _ = std::fs::remove_file(&tmp);
            return Err(error);
        }
        // Rename only after commit and close. On Unix this replaces the old
        // accelerator atomically; a failed publication leaves the old index
        // intact. Existing/corrupt accelerators are disposable, and the
        // canonical RVF is never touched.
        std::fs::rename(&tmp, &target).map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            LearningIndexError::WriteFailed
        })?;
        Ok(receipt_to_index(&receipt, &records))
    }

    fn populate(
        &self,
        receipt: &ExperienceReceipt,
        records: &[RvfRecord],
    ) -> Result<LearningIndexReceipt, LearningIndexError> {
        let mut events = Vec::with_capacity(records.len());
        let mut corpus = std::collections::BTreeSet::new();
        let mut policy = std::collections::BTreeSet::new();
        let mut split = std::collections::BTreeSet::new();
        for (sequence, record) in records.iter().enumerate() {
            let RvfRecord::Trajectory {
                record: trajectory, ..
            } = record
            else {
                continue;
            };
            let digest = Sha256Digest::new(hex::encode(Sha256::digest(
                serde_json::to_vec(record).map_err(|_| LearningIndexError::CorruptIndex)?,
            )))
            .map_err(|_| LearningIndexError::CorruptIndex)?;
            let event = IndexedEvent {
                sequence: sequence as u64,
                record_digest: digest,
                task_id: trajectory.task_id.clone(),
                paper_id: trajectory.paper_id.clone(),
                corpus: receipt.lineage.corpus.clone(),
                policy: receipt.lineage.prior_policy.clone(),
                split: receipt.lineage.split.clone(),
            };
            corpus.insert(event.corpus.clone());
            policy.insert(event.policy.clone());
            split.insert(event.split.clone());
            events.push(event);
        }
        let index_receipt = receipt_to_index(receipt, records);
        let bytes =
            serde_json::to_string(&index_receipt).map_err(|_| LearningIndexError::CorruptIndex)?;
        let write = self
            .db
            .begin_write()
            .map_err(|_| LearningIndexError::WriteFailed)?;
        {
            let mut meta = write
                .open_table(META_TABLE)
                .map_err(|_| LearningIndexError::WriteFailed)?;
            meta.insert("receipt", bytes.as_str())
                .map_err(|_| LearningIndexError::WriteFailed)?;
            let mut event_table = write
                .open_table(EVENT_TABLE)
                .map_err(|_| LearningIndexError::WriteFailed)?;
            for event in &events {
                let value =
                    serde_json::to_string(event).map_err(|_| LearningIndexError::CorruptIndex)?;
                event_table
                    .insert(event.sequence.to_string().as_str(), value.as_str())
                    .map_err(|_| LearningIndexError::WriteFailed)?;
            }
            let mut corpus_table = write
                .open_table(CORPUS_TABLE)
                .map_err(|_| LearningIndexError::WriteFailed)?;
            for key in &corpus {
                corpus_table
                    .insert(key.as_str(), events.len() as u64)
                    .map_err(|_| LearningIndexError::WriteFailed)?;
            }
            let mut policy_table = write
                .open_table(POLICY_TABLE)
                .map_err(|_| LearningIndexError::WriteFailed)?;
            for key in &policy {
                policy_table
                    .insert(key.as_str(), events.len() as u64)
                    .map_err(|_| LearningIndexError::WriteFailed)?;
            }
            let mut split_table = write
                .open_table(SPLIT_TABLE)
                .map_err(|_| LearningIndexError::WriteFailed)?;
            for key in &split {
                split_table
                    .insert(key.as_str(), events.len() as u64)
                    .map_err(|_| LearningIndexError::WriteFailed)?;
            }
        }
        write
            .commit()
            .map_err(|_| LearningIndexError::WriteFailed)?;
        Ok(index_receipt)
    }

    pub fn receipt(&self) -> Result<Option<LearningIndexReceipt>, LearningIndexError> {
        let read = self
            .db
            .begin_read()
            .map_err(|_| LearningIndexError::ReadFailed)?;
        let table = match read.open_table(META_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let Some(value) = table
            .get("receipt")
            .map_err(|_| LearningIndexError::ReadFailed)?
        else {
            return Ok(None);
        };
        serde_json::from_str(value.value())
            .map(Some)
            .map_err(|_| LearningIndexError::CorruptIndex)
    }

    pub fn event(&self, sequence: u64) -> Result<Option<(String, String)>, LearningIndexError> {
        let read = self
            .db
            .begin_read()
            .map_err(|_| LearningIndexError::ReadFailed)?;
        let table = match read.open_table(EVENT_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let Some(value) = table
            .get(sequence.to_string().as_str())
            .map_err(|_| LearningIndexError::ReadFailed)?
        else {
            return Ok(None);
        };
        let event: IndexedEvent =
            serde_json::from_str(value.value()).map_err(|_| LearningIndexError::CorruptIndex)?;
        Ok(Some((event.task_id, event.paper_id)))
    }

    fn lookup_count(
        &self,
        table_definition: redb::TableDefinition<'_, &str, u64>,
        key: &str,
    ) -> Result<Option<u64>, LearningIndexError> {
        let read = self
            .db
            .begin_read()
            .map_err(|_| LearningIndexError::ReadFailed)?;
        let table = match read.open_table(table_definition) {
            Ok(table) => table,
            Err(_) => return Ok(None),
        };
        table
            .get(key)
            .map_err(|_| LearningIndexError::ReadFailed)
            .map(|value| value.map(|guard| guard.value()))
    }

    pub fn corpus_count(&self, corpus: &Sha256Digest) -> Result<Option<u64>, LearningIndexError> {
        self.lookup_count(CORPUS_TABLE, corpus.as_str())
    }

    pub fn policy_count(&self, policy: &Sha256Digest) -> Result<Option<u64>, LearningIndexError> {
        self.lookup_count(POLICY_TABLE, policy.as_str())
    }

    pub fn split_count(&self, split: &Sha256Digest) -> Result<Option<u64>, LearningIndexError> {
        self.lookup_count(SPLIT_TABLE, split.as_str())
    }

    /// Verify binding before using an index lookup. Callers must obtain the
    /// expected receipt from RVF, never from this database.
    pub fn bound_to(&self, canonical: &ExperienceReceipt) -> Result<bool, LearningIndexError> {
        Ok(self.receipt()?.is_some_and(|r| {
            r.generation == canonical.generation
                && r.event_count == canonical.event_count
                && r.experience_digest == canonical.ordered_digest
        }))
    }
}

fn receipt_to_index(receipt: &ExperienceReceipt, records: &[RvfRecord]) -> LearningIndexReceipt {
    LearningIndexReceipt {
        schema_version: receipt.schema_version,
        generation: receipt.generation,
        experience_digest: receipt.ordered_digest.clone(),
        event_count: records
            .iter()
            .filter(|record| matches!(record, RvfRecord::Trajectory { .. }))
            .count() as u64,
        corpus_count: 1.min(
            records
                .iter()
                .filter(|record| matches!(record, RvfRecord::Trajectory { .. }))
                .count() as u64,
        ),
        policy_count: 1.min(
            records
                .iter()
                .filter(|record| matches!(record, RvfRecord::Trajectory { .. }))
                .count() as u64,
        ),
        split_count: 1.min(
            records
                .iter()
                .filter(|record| matches!(record, RvfRecord::Trajectory { .. }))
                .count() as u64,
        ),
    }
}
fn temp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".rebuild");
    PathBuf::from(tmp)
}
