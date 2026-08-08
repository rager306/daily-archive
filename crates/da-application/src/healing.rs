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
    pub graph_store: std::sync::Arc<dyn DirectGraphStore>,
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
}

impl GraphHealingUseCase {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self::from_shared(std::sync::Arc::from(graph_store))
    }

    pub fn from_shared(graph_store: std::sync::Arc<dyn DirectGraphStore>) -> Self {
        Self {
            graph_store,
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// SILENCE: set retrieval_eligible=false on a node.
    /// The node stays in the graph for audit but is excluded from all retrieval.
    #[tracing::instrument(skip(self, vid, label, reason), fields(vid = %vid, label = %label), name = "heal_silence")]
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
    #[tracing::instrument(skip(self, vid, label, key, new_value, reason), fields(vid = %vid, key = %key), name = "heal_correct")]
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

        // Read old value before overwriting (D135 audit trail)
        let old_value = self
            .graph_store
            .get_node_property_string(node_id, key)
            .await
            .unwrap_or_else(|| "(null)".to_string());

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
    #[tracing::instrument(skip(self, vid_keep, vid_merge, reason), fields(vid_keep = %vid_keep, vid_merge = %vid_merge), name = "heal_merge")]
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
        let keep_type = self
            .graph_store
            .get_node_property_string(keep_id, "entity_type")
            .await;
        let merge_type = self
            .graph_store
            .get_node_property_string(merge_id, "entity_type")
            .await;
        if keep_type != merge_type {
            anyhow::bail!(
                "cannot merge Entity types {:?} and {:?}",
                keep_type,
                merge_type
            );
        }

        if vid_keep == vid_merge {
            anyhow::bail!("cannot merge Entity '{vid_keep}' into itself");
        }
        let existing_canonical = self
            .graph_store
            .get_node_property_string(merge_id, "canonical_vid")
            .await;
        if let Some(canonical) = &existing_canonical
            && canonical != vid_keep
        {
            anyhow::bail!(
                "Entity {vid_merge} is already resolved to a different canonical VID {canonical}"
            );
        }

        // Preserve the source node for audit, but make the canonical reference
        // explicit and exclude the duplicate from retrieval.
        self.graph_store
            .set_node_property_string(merge_id, "canonical_vid", vid_keep.to_string())
            .await?;
        self.graph_store
            .set_node_property_bool(merge_id, "retrieval_eligible", false)
            .await?;

        // Create one SUPERSEDES edge: merge → keep — temporal fact (M329).
        if self
            .graph_store
            .get_edges_between(
                merge_id,
                keep_id,
                da_domain::relation::bibliographic::SUPERSEDES,
            )
            .await
            .is_empty()
        {
            let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
            crate::temporal_stamp::create_edge_with_temporal(
                self.graph_store.as_ref(),
                merge_id,
                keep_id,
                da_domain::relation::bibliographic::SUPERSEDES,
                &temporal,
            )
            .await?;
        }

        // Redirect incoming edges: for each edge (source → merge_id),
        // create a parallel edge (source → keep_id) so queries find the kept node.
        let incoming = self.graph_store.get_incoming_edges(merge_id).await;
        let mut redirected = 0;
        for (source_id, edge_type) in &incoming {
            // Don't redirect SUPERSEDES edges
            if edge_type != da_domain::relation::bibliographic::SUPERSEDES
                && self
                    .graph_store
                    .get_edges_between(*source_id, keep_id, edge_type)
                    .await
                    .is_empty()
            {
                // Fact edges must carry TemporalEdge fields (M329).
                if crate::temporal_stamp::edge_type_is_temporal_fact(edge_type) {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        *source_id,
                        keep_id,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                } else {
                    self.graph_store
                        .create_edge(*source_id, keep_id, edge_type)
                        .await?;
                }
                redirected += 1;
            }
        }

        let provenance = ProvenanceEvent::new(
            HealingOperation::Merge,
            actor,
            vec![vid_keep.to_string(), vid_merge.to_string()],
            reason.to_string(),
        );

        tracing::info!(
            keep_vid = vid_keep,
            merge_vid = vid_merge,
            edges_redirected = redirected,
            "Entities merged"
        );

        Ok(MergeResult {
            kept_vid: vid_keep.to_string(),
            merged_vid: vid_merge.to_string(),
            edges_redirected: redirected,
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

    /// Add a temporal edge between two nodes, resolving contradictions with
    /// existing edges of the same type. Uses kg-algorithms::resolve_temporal_edges
    /// to determine which existing edges should be invalidated.
    ///
    /// This wires ADR-047 temporal resolution into the healing use case.
    /// Returns the number of edges invalidated.
    pub async fn add_temporal_edge(
        &self,
        source_vid: &str,
        source_label: &str,
        target_vid: &str,
        target_label: &str,
        edge_type: &str,
        new_edge: &kg_ontology::temporal::TemporalEdge,
    ) -> anyhow::Result<usize> {
        use kg_algorithms::temporal::resolve_temporal_edges;

        let source_id = self
            .graph_store
            .find_node_by_string_property(source_label, "vid", source_vid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("Source node {source_vid} not found (label={source_label})")
            })?;
        let target_id = self
            .graph_store
            .find_node_by_string_property(target_label, "vid", target_vid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("Target node {target_vid} not found (label={target_label})")
            })?;

        // Gather existing edges between these nodes with the same type.
        let existing_edges_meta = self
            .graph_store
            .get_edges_between(source_id, target_id, edge_type)
            .await;

        // Read temporal properties for each existing edge to build TemporalEdge structs.
        let mut existing_temporal: Vec<kg_ontology::temporal::TemporalEdge> = Vec::new();
        for (edge_id, _) in &existing_edges_meta {
            let valid_at = self
                .graph_store
                .get_edge_property_string(*edge_id, "valid_at")
                .await
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
            let invalid_at = self
                .graph_store
                .get_edge_property_string(*edge_id, "invalid_at")
                .await
                .and_then(|s| s.parse().ok());
            let created_at = self
                .graph_store
                .get_edge_property_string(*edge_id, "created_at")
                .await
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
            existing_temporal.push(kg_ontology::temporal::TemporalEdge {
                valid_at,
                invalid_at,
                expired_at: None,
                reference_time: None,
                created_at,
            });
        }

        // Resolve: determine which existing edges should be invalidated.
        let outcomes = resolve_temporal_edges(new_edge, &existing_temporal);
        let mut invalidated_count = 0usize;
        for (idx, outcome) in outcomes.iter().enumerate() {
            if outcome.should_invalidate {
                let (edge_id, _) = &existing_edges_meta[idx];
                // Invalidate the old edge by setting invalid_at to new edge's valid_at.
                self.graph_store
                    .set_edge_property_string(
                        *edge_id,
                        "invalid_at",
                        &new_edge.valid_at.to_rfc3339(),
                    )
                    .await?;
                invalidated_count += 1;
                tracing::info!(
                    edge_id,
                    rule = ?outcome.rule,
                    "Temporal edge invalidated"
                );
            }
        }

        // Create the new edge and stamp temporal fields via shared helper (M329).
        let new_edge_id = crate::temporal_stamp::create_edge_with_temporal(
            self.graph_store.as_ref(),
            source_id,
            target_id,
            edge_type,
            new_edge,
        )
        .await?;

        tracing::info!(
            new_edge_id,
            source_vid,
            target_vid,
            edge_type,
            invalidated = invalidated_count,
            "Temporal edge added with contradiction resolution"
        );

        Ok(invalidated_count)
    }
}
