//! CVF agent memory (ruVector cognitive container): every trajectory is
//! a container tick (ingest->evidence->witness); persistence = JSON
//! snapshot + append-only replay log (deltas); witness chain verifies
//! the history. Operational domain — outside the Samyama knowledge
//! store (ADR-040 untouched).

use da_ports::agent_memory::{AgentMemory, AgentMemoryError, MemoryStats, TrajectoryRecord};
use ruvector_cognitive_container::{CognitiveContainer, ContainerConfig, Delta};
use std::io::Write;
use std::path::PathBuf;

pub struct CvfAgentMemory {
    container: CognitiveContainer,
    dir: PathBuf,
    trajectories: u64,
    last_epoch_in_ms: Option<u64>,
}

impl CvfAgentMemory {
    /// Open (or bootstrap) the memory under a directory; restores the
    /// container by replaying the trajectory log over a fresh container.
    pub fn open(dir: PathBuf) -> Result<Self, AgentMemoryError> {
        std::fs::create_dir_all(&dir).map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let mut container = CognitiveContainer::new(ContainerConfig::default())
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let mut trajectories = 0u64;
        let log = dir.join("trajectory.jsonl");
        if log.is_file() {
            for line in std::fs::read_to_string(&log)
                .map_err(|e| AgentMemoryError::Io(e.to_string()))?
                .lines()
            {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(rec) = serde_json::from_str::<TrajectoryRecord>(line) {
                    let deltas = trajectory_deltas(&rec);
                    container
                        .tick(&deltas)
                        .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
                    trajectories += 1;
                }
            }
        }
        Ok(Self {
            container,
            dir,
            trajectories,
            last_epoch_in_ms: None,
        })
    }

    fn log_path(&self) -> PathBuf {
        self.dir.join("trajectory.jsonl")
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
        self.container
            .tick(&trajectory_deltas(rec))
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        self.trajectories += 1;
        self.last_epoch_in_ms = Some(t0.elapsed().as_millis() as u64);
        // append-only replay log (restore path)
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        let line = serde_json::to_string(rec).map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        f.write_all(line.as_bytes())
            .and_then(|_| f.write_all(b"\n"))
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        Ok(self.container.current_epoch())
    }

    fn verify(&self) -> bool {
        use ruvector_cognitive_container::VerificationResult;
        matches!(
            self.container.verify_chain(),
            VerificationResult::Valid { .. }
        )
    }

    fn persist(&self) -> Result<(), AgentMemoryError> {
        let snap = serde_json::to_string(&self.container.snapshot())
            .map_err(|e| AgentMemoryError::Io(e.to_string()))?;
        std::fs::write(self.dir.join("snapshot.json"), snap)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_persist_replay_verify() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = CvfAgentMemory::open(dir.path().to_path_buf()).unwrap();
        let rec = TrajectoryRecord {
            task_id: "article_summary".into(),
            paper_id: "p1".into(),
            model: "agnes-2.5".into(),
            prompt_sha256: "abc".into(),
            ok: true,
            latency_ms: 120,
            completion_tokens: Some(300),
            grounding_hint: Some(0.86),
            article_features: None,
            failure_pattern: None,
            construction_trace: None,
        };
        let epoch = m.record_trajectory(&rec).unwrap();
        assert!(epoch >= 1);
        assert!(m.verify());
        m.persist().unwrap();
        assert_eq!(m.stats().trajectories, 1);

        // reopen: replay restores epoch/trajectories
        let m2 = CvfAgentMemory::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(m2.stats().trajectories, 1);
        assert_eq!(m2.stats().epochs, m.stats().epochs);
        assert!(m2.verify());
    }
}
