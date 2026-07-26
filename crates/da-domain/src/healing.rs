//! Graph healing operations (D135).
//!
//! Scenarios for correcting, merging, silencing, and migrating graph nodes.
//! See doc/GRAPH-HEALING-SCENARIOS.md for the full catalog.

use serde::{Deserialize, Serialize};

/// The type of healing operation performed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealingOperation {
    /// Fix a wrong property value on a node.
    Correct,
    /// Fuse two duplicate entities into one.
    Merge,
    /// Split one entity into two.
    Split,
    /// Deprecate/quarantine a node (retrieval_eligible=false).
    Silence,
    /// Migrate from one taxonomy to another (e.g., Concepts → Topics).
    Migrate,
    /// Revert extraction to a previous version.
    Rollback,
    /// Fix a wrong citation edge.
    RepairCites,
}

impl HealingOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealingOperation::Correct => "correct",
            HealingOperation::Merge => "merge",
            HealingOperation::Split => "split",
            HealingOperation::Silence => "silence",
            HealingOperation::Migrate => "migrate",
            HealingOperation::Rollback => "rollback",
            HealingOperation::RepairCites => "repair_cites",
        }
    }
}

/// Who or what initiated the healing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealingActor {
    Human(String),
    Agent(String),
    System(String),
}

impl HealingActor {
    pub fn as_str(&self) -> String {
        match self {
            HealingActor::Human(name) => format!("human:{name}"),
            HealingActor::Agent(name) => format!("agent:{name}"),
            HealingActor::System(name) => format!("system:{name}"),
        }
    }
}

/// A provenance event recording a healing operation.
/// Every healing action creates one of these for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEvent {
    pub operation: HealingOperation,
    pub actor: HealingActor,
    pub timestamp: i64,
    pub affected_vids: Vec<String>,
    pub reason: String,
    /// Key-value changes (property → {old, new}).
    pub changes: Vec<(String, serde_json::Value, serde_json::Value)>,
}

impl ProvenanceEvent {
    pub fn new(
        operation: HealingOperation,
        actor: HealingActor,
        affected_vids: Vec<String>,
        reason: String,
    ) -> Self {
        Self {
            operation,
            actor,
            timestamp: chrono::Utc::now().timestamp(),
            affected_vids,
            reason,
            changes: Vec::new(),
        }
    }

    pub fn with_change(
        mut self,
        key: &str,
        old: serde_json::Value,
        new: serde_json::Value,
    ) -> Self {
        self.changes.push((key.to_string(), old, new));
        self
    }
}

/// Result of a merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub kept_vid: String,
    pub merged_vid: String,
    pub edges_redirected: usize,
    pub provenance: ProvenanceEvent,
}

/// Result of a silence operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilenceResult {
    pub vid: String,
    pub previous_eligible: bool,
    pub provenance: ProvenanceEvent,
}

/// Result of a correct operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectResult {
    pub vid: String,
    pub key: String,
    pub old_value: String,
    pub new_value: String,
    pub provenance: ProvenanceEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healing_operation_str() {
        assert_eq!(HealingOperation::Merge.as_str(), "merge");
        assert_eq!(HealingOperation::Silence.as_str(), "silence");
        assert_eq!(HealingOperation::Migrate.as_str(), "migrate");
    }

    #[test]
    fn test_provenance_event_new() {
        let event = ProvenanceEvent::new(
            HealingOperation::Merge,
            HealingActor::Human("alice".to_string()),
            vec!["vid:a".to_string(), "vid:b".to_string()],
            "same entity".to_string(),
        );
        assert_eq!(event.operation, HealingOperation::Merge);
        assert_eq!(event.affected_vids.len(), 2);
        assert!(event.changes.is_empty());
    }

    #[test]
    fn test_provenance_with_change() {
        let event = ProvenanceEvent::new(
            HealingOperation::Correct,
            HealingActor::System("extractor".to_string()),
            vec!["vid:x".to_string()],
            "fix label".to_string(),
        )
        .with_change(
            "label",
            serde_json::json!("GPT"),
            serde_json::json!("GPT-4"),
        );
        assert_eq!(event.changes.len(), 1);
        assert_eq!(event.changes[0].0, "label");
    }

    #[test]
    fn test_actor_str() {
        assert_eq!(HealingActor::Human("bob".to_string()).as_str(), "human:bob");
        assert_eq!(
            HealingActor::Agent("sona".to_string()).as_str(),
            "agent:sona"
        );
    }
}
