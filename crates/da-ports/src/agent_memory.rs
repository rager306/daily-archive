//! Agent memory port (M451 W1): persistent, verifiable substrate for
//! agent trajectories (ruVector CVF cognitive container behind it).
//! Application depends on this trait; adapters own the container.

use serde::{Deserialize, Serialize};

/// One LLM-call trajectory record (the SONA Trajectory shape, stage 1:
/// record-only, no learning).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryRecord {
    pub task_id: String,        // e.g. "article_summary"
    pub paper_id: String,
    pub model: String,
    pub prompt_sha256: String,
    pub ok: bool,
    pub latency_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    /// deterministic quality hint (e.g. grounding score) — feedback later
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding_hint: Option<f64>,
    /// article features for routing decisions (SONA W3): what the
    /// decision CONDITION depends on, not just the outcome
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article_features: Option<ArticleFeatures>,
}

/// Compact article signature for memory-driven routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleFeatures {
    pub grammar: String,           // arxiv_pdf_dot | html_headings | page_outline | …
    pub body_chars: usize,
    pub furniture_share: f64,
    pub formulas_per_section: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryStats {
    pub epochs: u64,
    pub trajectories: u64,
    pub last_epoch_in_ms: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentMemoryError {
    #[error("agent memory: {0}")]
    Io(String),
}

/// Persistent verifiable agent memory (CVF container).
pub trait AgentMemory: Send + Sync {
    /// Append one trajectory as a container tick; returns the epoch.
    fn record_trajectory(&mut self, rec: &TrajectoryRecord) -> Result<u64, AgentMemoryError>;
    /// Verify the witness chain (integrity of the memory history).
    fn verify(&self) -> bool;
    /// Persist (snapshot + replay log); restore happens on open.
    fn persist(&self) -> Result<(), AgentMemoryError>;
    fn stats(&self) -> MemoryStats;
}
