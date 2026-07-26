//! Graph healing use case — correct, merge, silence operations.
//!
//! D135: scenarios for evolving and repairing the knowledge graph.
//! See doc/GRAPH-HEALING-SCENARIOS.md for the full catalog.

use da_domain::healing::{
    CorrectResult, HealingActor, HealingOperation, MergeResult, ProvenanceEvent, SilenceResult,
};
use da_ports::graph_store::DirectGraphStore;

/// Graph healing use case: correct, merge, silence nodes in the graph.
pub struct GraphHealingUseCase {
    pub graph_store: Box<dyn DirectGraphStore>,
}

impl GraphHealingUseCase {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// SILENCE: set retrieval_eligible=false on a node.
    /// The node stays in the graph for audit but is excluded from all retrieval.
    pub async fn silence(
        &self,
        vid: &str,
        label: &str,
        reason: &str,
        actor: HealingActor,
    ) -> anyhow::Result<SilenceResult> {
        let node_id = self
            .graph_store
            .find_node_by_string_property(label, "vid", vid)
            .await
            .ok_or_else(|| anyhow::anyhow!("Node {vid} not found (label={label})"))?;

        self.graph_store
            .set_node_property_bool(node_id, "retrieval_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "deprecated_reason", reason.to_string())
            .await?;

        let provenance = ProvenanceEvent::new(
            HealingOperation::Silence,
            actor,
            vec![vid.to_string()],
            reason.to_string(),
        )
        .with_change(
            "retrieval_eligible",
            serde_json::json!(true),
            serde_json::json!(false),
        );

        tracing::info!(vid, label, reason, "Node silenced");

        Ok(SilenceResult {
            vid: vid.to_string(),
            previous_eligible: true,
            provenance,
        })
    }

    /// CORRECT: fix a wrong string property on a node.
    /// Logs the old→new change for audit.
    pub async fn correct(
        &self,
        vid: &str,
        label: &str,
        key: &str,
        new_value: &str,
        reason: &str,
        actor: HealingActor,
    ) -> anyhow::Result<CorrectResult> {
        let node_id = self
            .graph_store
            .find_node_by_string_property(label, "vid", vid)
            .await
            .ok_or_else(|| anyhow::anyhow!("Node {vid} not found (label={label})"))?;

        // Note: we can't read the old value via DirectGraphStore (no get_property).
        // In production, this would use a read API. For now, old_value is "unknown".
        let old_value = "unknown".to_string();

        self.graph_store
            .set_node_property_string(node_id, key, new_value.to_string())
            .await?;

        let provenance = ProvenanceEvent::new(
            HealingOperation::Correct,
            actor,
            vec![vid.to_string()],
            reason.to_string(),
        )
        .with_change(
            key,
            serde_json::json!(old_value),
            serde_json::json!(new_value),
        );

        tracing::info!(vid, key, old = %old_value, new = %new_value, "Node corrected");

        Ok(CorrectResult {
            vid: vid.to_string(),
            key: key.to_string(),
            old_value,
            new_value: new_value.to_string(),
            provenance,
        })
    }

    /// MERGE: fuse two duplicate entities.
    /// vid_merge gets retrieval_eligible=false + SUPERSEDES edge → vid_keep.
    pub async fn merge(
        &self,
        vid_keep: &str,
        vid_merge: &str,
        reason: &str,
        actor: HealingActor,
    ) -> anyhow::Result<MergeResult> {
        let keep_id = self
            .graph_store
            .find_node_by_string_property("Entity", "vid", vid_keep)
            .await
            .ok_or_else(|| anyhow::anyhow!("Entity {vid_keep} not found"))?;

        let merge_id = self
            .graph_store
            .find_node_by_string_property("Entity", "vid", vid_merge)
            .await
            .ok_or_else(|| anyhow::anyhow!("Entity {vid_merge} not found"))?;

        // Silence the merged node
        self.graph_store
            .set_node_property_bool(merge_id, "retrieval_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_string(merge_id, "superseded_by", vid_keep.to_string())
            .await?;

        // Create SUPERSEDES edge: merge → keep
        self.graph_store
            .create_edge(merge_id, keep_id, "SUPERSEDES")
            .await?;

        let provenance = ProvenanceEvent::new(
            HealingOperation::Merge,
            actor,
            vec![vid_keep.to_string(), vid_merge.to_string()],
            reason.to_string(),
        );

        tracing::info!(
            keep_vid = vid_keep,
            merge_vid = vid_merge,
            "Entities merged"
        );

        Ok(MergeResult {
            kept_vid: vid_keep.to_string(),
            merged_vid: vid_merge.to_string(),
            edges_redirected: 0, // edge redirect not yet implemented
            provenance,
        })
    }

    /// UNSILENCE: restore a silenced node (reverse of silence).
    pub async fn unsilence(
        &self,
        vid: &str,
        label: &str,
        actor: HealingActor,
    ) -> anyhow::Result<SilenceResult> {
        let node_id = self
            .graph_store
            .find_node_by_string_property(label, "vid", vid)
            .await
            .ok_or_else(|| anyhow::anyhow!("Node {vid} not found (label={label})"))?;

        self.graph_store
            .set_node_property_bool(node_id, "retrieval_eligible", true)
            .await?;

        let provenance = ProvenanceEvent::new(
            HealingOperation::Silence,
            actor,
            vec![vid.to_string()],
            "unsilence: restored retrieval_eligible".to_string(),
        )
        .with_change(
            "retrieval_eligible",
            serde_json::json!(false),
            serde_json::json!(true),
        );

        tracing::info!(vid, label, "Node un-silenced");

        Ok(SilenceResult {
            vid: vid.to_string(),
            previous_eligible: false,
            provenance,
        })
    }
}
