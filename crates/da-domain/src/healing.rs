//! Graph healing operations (D135).
//!
//! Universal types (HealingOperation, HealingActor, ProvenanceEvent)
//! re-exported from kg_ontology::healing (M289-u4fzx5/S06).
//! Project-specific result types (MergeResult, SilenceResult, CorrectResult)
//! stay here.

// Re-export universal healing types from kg-ontology (ADR-050).
pub use kg_ontology::healing::{HealingActor, HealingOperation, ProvenanceEvent};

use serde::{Deserialize, Serialize};

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
