//! Canonical, append-only RVF experience journal (M464/S17/T02).
//! Legacy CVF JSONL is accepted only by the explicit migration method.

use da_ports::parse_learning::{
    CheckpointReceipt, ExperienceReceipt, LearningLineage, RVF_SCHEMA_VERSION, RvfAppendReceipt,
    RvfExperienceStore, RvfRecord, RvfStoreError, Sha256Digest,
};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_FRAME: usize = 64 * 1024;
const HEADER: &[u8; 4] = b"RVF1";

pub struct RvfFileStore {
    path: PathBuf,
    lineage: LearningLineage,
}

impl RvfFileStore {
    pub fn open(path: PathBuf, lineage: LearningLineage) -> Result<Self, RvfStoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        }
        let store = Self { path, lineage };
        // Opening validates the complete journal before a caller can append.
        store.read_all()?;
        Ok(store)
    }

    fn digest(bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::new(hex::encode(Sha256::digest(bytes))).expect("sha256 is always valid")
    }

    fn encode(record: &RvfRecord) -> Result<Vec<u8>, RvfStoreError> {
        record.validate()?;
        let payload = serde_json::to_vec(record).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        if payload.len() > MAX_FRAME {
            return Err(RvfStoreError::RecordTooLarge);
        }
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(HEADER);
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    fn frames(&self) -> Result<Vec<(u64, Vec<u8>)>, RvfStoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        File::open(&self.path)
            .map_err(|e| RvfStoreError::Io(e.to_string()))?
            .read_to_end(&mut bytes)
            .map_err(|e| RvfStoreError::Io(e.to_string()))?;
        let mut offset = 0usize;
        let mut out = Vec::new();
        while offset < bytes.len() {
            let start = offset;
            if bytes.len() - offset < 8 {
                return Err(RvfStoreError::Truncated {
                    offset: start as u64,
                });
            }
            if &bytes[offset..offset + 4] != HEADER {
                return Err(RvfStoreError::UnknownRecord);
            }
            offset += 4;
            let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            if len == 0 || len > MAX_FRAME {
                return Err(RvfStoreError::RecordTooLarge);
            }
            if bytes.len() - offset < len {
                return Err(RvfStoreError::Truncated {
                    offset: start as u64,
                });
            }
            out.push((start as u64, bytes[offset..offset + len].to_vec()));
            offset += len;
        }
        Ok(out)
    }

    fn ordered_digest(records: &[RvfRecord]) -> Result<Sha256Digest, RvfStoreError> {
        let mut hasher = Sha256::new();
        for record in records {
            let payload =
                serde_json::to_vec(record).map_err(|e| RvfStoreError::Io(e.to_string()))?;
            hasher.update((payload.len() as u64).to_be_bytes());
            hasher.update(payload);
        }
        Ok(Sha256Digest::new(hex::encode(hasher.finalize())).expect("sha256 is always valid"))
    }
}

impl RvfExperienceStore for RvfFileStore {
    fn append(&mut self, record: RvfRecord) -> Result<RvfAppendReceipt, RvfStoreError> {
        let frame = Self::encode(&record)?;
        let record_digest = Self::digest(&frame);
        let records = self.read_all()?;
        if let Some((sequence, _existing)) = records.iter().enumerate().find(|(_, r)| {
            Self::digest(&Self::encode(r).expect("previous records were validated"))
                == record_digest
        }) {
            return Ok(RvfAppendReceipt {
                sequence: sequence as u64,
                record_digest,
                ordered_digest: Self::ordered_digest(&records)?,
                duplicate: true,
            });
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| RvfStoreError::Io(e.to_string()))?;
        file.write_all(&frame)
            .and_then(|_| file.sync_data())
            .map_err(|e| RvfStoreError::Io(e.to_string()))?;
        let mut next = records;
        next.push(record);
        Ok(RvfAppendReceipt {
            sequence: (next.len() - 1) as u64,
            record_digest,
            ordered_digest: Self::ordered_digest(&next)?,
            duplicate: false,
        })
    }

    fn read_all(&self) -> Result<Vec<RvfRecord>, RvfStoreError> {
        self.frames()?
            .into_iter()
            .map(|(offset, payload)| {
                serde_json::from_slice::<RvfRecord>(&payload)
                    .map_err(|e| {
                        if e.to_string().contains("unknown variant") {
                            RvfStoreError::UnknownRecord
                        } else {
                            RvfStoreError::Corrupt { offset }
                        }
                    })
                    .and_then(|record| {
                        record.validate()?;
                        Ok(record)
                    })
            })
            .collect()
    }

    fn verify(&self) -> Result<ExperienceReceipt, RvfStoreError> {
        let records = self.read_all()?;
        Ok(ExperienceReceipt {
            schema_version: RVF_SCHEMA_VERSION,
            generation: 1,
            event_count: records.len() as u64,
            ordered_digest: Self::ordered_digest(&records)?,
            lineage: self.lineage.clone(),
        })
    }

    fn checkpoint(&self) -> Result<CheckpointReceipt, RvfStoreError> {
        let experience = self.verify()?;
        let checkpoint_path = self.path.with_extension("checkpoint.json");
        let previous_checkpoint = std::fs::read(&checkpoint_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CheckpointReceipt>(&bytes).ok())
            .map(|c| c.experience_digest);
        let receipt = CheckpointReceipt {
            schema_version: RVF_SCHEMA_VERSION,
            generation: experience.generation,
            event_sequence: experience.event_count.saturating_sub(1),
            experience_digest: experience.ordered_digest.clone(),
            previous_checkpoint,
        };
        let path = checkpoint_path;
        let bytes = serde_json::to_vec(&receipt).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        let tmp = path.with_extension("checkpoint.json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        std::fs::rename(tmp, path).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        Ok(receipt)
    }

    fn migrate_legacy_jsonl(&mut self, path: &Path) -> Result<u64, RvfStoreError> {
        let text = std::fs::read_to_string(path).map_err(|e| RvfStoreError::Io(e.to_string()))?;
        // Parse the complete source before appending anything, so a malformed
        // migration can never leave a partially imported canonical journal.
        let records: Vec<RvfRecord> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .map(RvfRecord::trajectory)
                    .map_err(|_| RvfStoreError::Corrupt { offset: 0 })
            })
            .collect::<Result<_, _>>()?;
        records.into_iter().try_fold(0u64, |count, record| {
            let receipt = self.append(record)?;
            Ok(count + u64::from(!receipt.duplicate))
        })
    }
}
