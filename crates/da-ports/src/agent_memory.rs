//! Agent memory port (M451 W1): persistent, verifiable substrate for
//! agent trajectories (ruVector CVF cognitive container behind it).
//! Application depends on this trait; adapters own the container.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// failure pattern ref (agent_failure_patterns.yaml id) — SONA
    /// learns on TYPES of failures, not just numeric grounding
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_pattern: Option<String>,
    /// construction trace: input context, output JSON, per-field metrics —
    /// the training data for prompt optimization (AgentMercury insight:
    /// construction traces as supervision, authoring success 3.3→83.3%)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_trace: Option<ConstructionTrace>,
}

/// Full call context for future prompt optimization / SONA training.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstructionTrace {
    /// section titles fed to the LLM (input structure)
    pub input_sections: Vec<String>,
    /// output field names + char counts (output structure)
    pub output_fields: BTreeMap<String, usize>,
    /// injection scrub count (guardrail effectiveness)
    pub injection_scrubbed: usize,
    /// grounding score computed from output vs parsed text
    pub grounding_score: f64,
}

/// Compact article signature for memory-driven routing.
/// Includes PageIndex-derived structural features (navigation layer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleFeatures {
    pub grammar: String,
    pub body_chars: usize,
    pub furniture_share: f64,
    pub formulas_per_section: f64,
    /// PageIndex: max tree depth (0=flat, 1=one level, …)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav_max_depth: Option<usize>,
    /// PageIndex: count of body nodes vs total
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav_body_nodes: Option<usize>,
    /// PageIndex: sections with children (branching factor)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav_branching_nodes: Option<usize>,
    /// PageIndex: body section titles — pattern matching for routing
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_body_titles: Vec<String>,
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
