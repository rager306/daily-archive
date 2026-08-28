//! Negative memory port (M455). Separate from [`crate::agent_memory::AgentMemory`]
//! so CVF trajectories stay write-only experience; this port is the
//! fail-closed reuse gate.

use da_domain::negative_memory::NegativeMemoryRecord;

#[derive(Debug, thiserror::Error)]
pub enum NegativeMemoryError {
    #[error("negative memory: io: {0}")]
    Io(String),
}

pub trait NegativeMemoryPort: Send + Sync {
    fn append(&mut self, rec: &NegativeMemoryRecord) -> Result<(), NegativeMemoryError>;
    fn list_for_paper(
        &self,
        paper_id: &str,
    ) -> Result<Vec<NegativeMemoryRecord>, NegativeMemoryError>;
}
