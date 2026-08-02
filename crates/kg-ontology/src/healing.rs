//! Universal graph healing types (ADR-040, ADR-050).
//!
//! Provenance audit trail for graph corrections, merges, silencing.
//! Extracted from da-domain in M289-u4fzx5/S06.

use serde::{Deserialize, Serialize};

/// The type of healing operation performed on a graph.
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
    /// Migrate from one taxonomy to another.
    Migrate,
    /// Revert extraction to a previous version.
    Rollback,
    /// Fix a wrong edge (e.g., citation).
    RepairEdge,
}

impl HealingOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Correct => "correct",
            Self::Merge => "merge",
            Self::Split => "split",
            Self::Silence => "silence",
            Self::Migrate => "migrate",
            Self::Rollback => "rollback",
            Self::RepairEdge => "repair_edge",
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
            Self::Human(name) => format!("human:{name}"),
            Self::Agent(name) => format!("agent:{name}"),
            Self::System(name) => format!("system:{name}"),
        }
    }
}

/// A provenance event recording a healing operation.
///
/// Every healing action creates one of these for the audit trail.
/// This is universal — any graph system benefits from tracking who
/// changed what and why.
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
    /// Create a new provenance event with auto-timestamp.
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

    /// Builder: add a property change (old → new).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healing_operation_as_str() {
        assert_eq!(HealingOperation::Correct.as_str(), "correct");
        assert_eq!(HealingOperation::Merge.as_str(), "merge");
        assert_eq!(HealingOperation::RepairEdge.as_str(), "repair_edge");
    }

    #[test]
    fn test_healing_actor_as_str() {
        assert_eq!(HealingActor::Human("alice".into()).as_str(), "human:alice");
        assert_eq!(HealingActor::Agent("bot".into()).as_str(), "agent:bot");
        assert_eq!(
            HealingActor::System("scheduler".into()).as_str(),
            "system:scheduler"
        );
    }

    #[test]
    fn test_provenance_new() {
        let event = ProvenanceEvent::new(
            HealingOperation::Correct,
            HealingActor::Human("alice".into()),
            vec!["vid1".into()],
            "fixed typo".into(),
        );
        assert_eq!(event.operation, HealingOperation::Correct);
        assert!(event.timestamp > 0);
        assert!(event.changes.is_empty());
    }

    #[test]
    fn test_provenance_with_change() {
        let event = ProvenanceEvent::new(
            HealingOperation::Correct,
            HealingActor::System("auto".into()),
            vec!["vid1".into()],
            "fix".into(),
        )
        .with_change("title", serde_json::json!("old"), serde_json::json!("new"));

        assert_eq!(event.changes.len(), 1);
        assert_eq!(event.changes[0].0, "title");
    }

    #[test]
    fn test_serde_round_trip() {
        let event = ProvenanceEvent::new(
            HealingOperation::Merge,
            HealingActor::Agent("bot".into()),
            vec!["a".into(), "b".into()],
            "dedup".into(),
        );
        let json = serde_json::to_string(&event).unwrap();
        let back: ProvenanceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.operation, HealingOperation::Merge);
        assert_eq!(back.affected_vids, vec!["a".to_string(), "b".to_string()]);
    }
}
