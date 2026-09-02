//! Compatibility facade for agent memory backed by the canonical RVF journal.
//! `trajectory.jsonl` is read only during explicit one-time migration; all new
//! trajectories and checkpoints are written to `experience.rvf`.

use crate::parse_experience_rvf::RvfFileStore;
use da_ports::agent_memory::{AgentMemory, AgentMemoryError, MemoryStats, TrajectoryRecord};
use da_ports::parse_learning::{LearningLineage, RvfExperienceStore, RvfRecord, Sha256Digest};
use ruvector_cognitive_container::{CognitiveContainer, ContainerConfig, Delta};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct CvfAgentMemory {
    container: CognitiveContainer,
    store: RvfFileStore,
    trajectories: u64,
    last_epoch_in_ms: Option<u64>,
}

fn identity(seed: &str) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(seed.as_bytes()))).expect("sha256 is valid")
}

fn lineage(namespace: &Path) -> LearningLineage {
    let name = namespace.to_string_lossy();
    LearningLineage {
        source: identity(&format!("source:{name}")),
        corpus: identity(&format!("corpus:{name}")),
        config: identity("config:cvf-rvf-v1"),
        prior_policy: identity("prior-policy:cvf-rvf-v1"),
        conservation: identity("conservation:cvf-rvf-v1"),
        split: identity("split:unspecified-v1"),
    }
}

impl CvfAgentMemory {
    /// Open canonical RVF memory and migrate an existing legacy JSONL file
    /// without modifying it. Corrupt legacy input fails closed.
    pub fn open(dir: PathBuf) -> Result<Self, AgentMemoryError> {
        std::fs::create_dir_all(&dir).map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let mut container = CognitiveContainer::new(ContainerConfig::default())
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let rvf_path = dir.join("experience.rvf");
        let mut store = RvfFileStore::open(rvf_path, lineage(&dir))
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let legacy = dir.join("trajectory.jsonl");
        if legacy.is_file()
            && store
                .read_all()
                .map_err(|e| AgentMemoryError::Io(e.to_string()))?
                .is_empty()
        {
            store
                .migrate_legacy_jsonl(&legacy)
                .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        }
        let records = store
            .read_all()
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        for record in &records {
            container
                .tick(&trajectory_deltas(record.trajectory_record()))
                .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        }
        Ok(Self {
            container,
            store,
            trajectories: records.len() as u64,
            last_epoch_in_ms: None,
        })
    }

    /// Read canonical typed trajectories for analysis; no legacy fallback.
    pub fn read_trajectories(&self) -> Result<Vec<TrajectoryRecord>, AgentMemoryError> {
        self.store
            .read_all()
            .map(|records| {
                records
                    .into_iter()
                    .map(|r| r.trajectory_record().clone())
                    .collect()
            })
            .map_err(|e| AgentMemoryError::Io(e.to_string()))
    }
}

/// Map a trajectory to container deltas: observation of success on the
/// (paper, model) nodes + an edge recording the model->paper usage.
fn trajectory_deltas(rec: &TrajectoryRecord) -> Vec<Delta> {
    fn node_for(s: &str, salt: u8) -> usize {
        let mut h: u64 = salt as u64;
        for b in s.as_bytes() {
            h = h.wrapping_mul(31).wrapping_add(*b as u64);
        }
        (h % 100_000) as usize
    }
    let paper = node_for(&rec.paper_id, 1);
    let model = node_for(&rec.model, 2);
    let value = if rec.ok { 1.0 } else { 0.0 };
    vec![
        Delta::Observation { node: paper, value },
        Delta::EdgeAdd {
            u: model,
            v: paper,
            weight: value,
        },
    ]
}

impl AgentMemory for CvfAgentMemory {
    fn record_trajectory(&mut self, rec: &TrajectoryRecord) -> Result<u64, AgentMemoryError> {
        let t0 = std::time::Instant::now();
        let receipt = self
            .store
            .append(RvfRecord::trajectory(rec.clone()))
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        if !receipt.duplicate {
            self.container
                .tick(&trajectory_deltas(rec))
                .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
            self.trajectories += 1;
        }
        self.last_epoch_in_ms = Some(t0.elapsed().as_millis() as u64);
        Ok(self.container.current_epoch())
    }

    fn verify(&self) -> bool {
        use ruvector_cognitive_container::VerificationResult;
        matches!(
            self.container.verify_chain(),
            VerificationResult::Valid { .. }
        ) && self.store.verify().is_ok()
    }

    fn persist(&self) -> Result<(), AgentMemoryError> {
        // The RVF journal is already durable per append; checkpoint adds a
        // typed lineage/digest receipt and never writes a JSONL authority.
        self.store
            .checkpoint()
            .map(|_| ())
            .map_err(|e| AgentMemoryError::Io(e.to_string()))
    }

    fn stats(&self) -> MemoryStats {
        MemoryStats {
            epochs: self.container.current_epoch(),
            trajectories: self.trajectories,
            last_epoch_in_ms: self.last_epoch_in_ms,
        }
    }
}
