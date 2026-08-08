//! Decision-backed entity resolution orchestration (ADR-053 Layer G).

use std::sync::Arc;

use da_domain::entity_resolution::{
    BlockingResult, EntityResolutionInput, EntityResolutionProfile, block_candidates,
    pair_is_candidate,
};
use da_domain::healing::{HealingActor, MergeResult};
use da_ports::graph_store::DirectGraphStore;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::decision_recorder::{DecisionRecordError, DecisionRecorder, DecisionRequestBuilder};
use crate::healing::GraphHealingUseCase;

#[derive(Debug, Error)]
pub enum EntityResolutionError {
    #[error("entity resolution profile: {0}")]
    Profile(String),
    #[error("Entity '{0}' not found")]
    EntityNotFound(String),
    #[error("entity type mismatch: '{keep}' vs '{merge}'")]
    TypeMismatch { keep: String, merge: String },
    #[error("pair is not produced by the configured blocking profile")]
    NotCandidate,
    #[error("invalid reviewed merge: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Decision(#[from] DecisionRecordError),
    #[error("graph operation: {0}")]
    Graph(String),
}

#[derive(Debug, Clone)]
pub struct ReviewedMergeRequest {
    pub keep_vid: String,
    pub merge_vid: String,
    pub reason: String,
    pub actor: HealingActor,
}

#[derive(Debug)]
pub struct ReviewedMergeResult {
    pub decision_vid: String,
    pub decision_node_id: u64,
    pub merge: MergeResult,
}

pub struct EntityResolutionUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
    clock: Arc<dyn da_ports::Clock>,
    profile: EntityResolutionProfile,
}

impl EntityResolutionUseCase {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Result<Self, EntityResolutionError> {
        Ok(Self {
            graph_store,
            clock: Arc::new(da_ports::SystemClock),
            profile: EntityResolutionProfile::load_project()
                .map_err(EntityResolutionError::Profile)?,
        })
    }

    pub fn with_clock(mut self, clock: Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub async fn candidates(&self) -> BlockingResult {
        let mut entities = Vec::new();
        for node in self.graph_store.get_nodes_by_label("Entity").await {
            if self
                .graph_store
                .get_node_property_bool(node, "retrieval_eligible")
                .await
                == Some(false)
            {
                continue;
            }
            let Some(vid) = self.graph_store.get_node_property_string(node, "vid").await else {
                continue;
            };
            let Some(entity_type) = self
                .graph_store
                .get_node_property_string(node, "entity_type")
                .await
            else {
                continue;
            };
            let Some(label) = self
                .graph_store
                .get_node_property_string(node, "label")
                .await
            else {
                continue;
            };
            entities.push(EntityResolutionInput {
                vid,
                entity_type,
                label,
            });
        }
        let result = block_candidates(&entities, &self.profile);
        tracing::info!(
            profile_version = self.profile.version,
            entity_count = entities.len(),
            candidate_count = result.candidates.len(),
            oversized_block_count = result.oversized_blocks.len(),
            "Entity resolution blocking completed"
        );
        result
    }

    pub async fn resolve_reviewed_merge(
        &self,
        request: ReviewedMergeRequest,
    ) -> Result<ReviewedMergeResult, EntityResolutionError> {
        if request.keep_vid == request.merge_vid {
            return Err(EntityResolutionError::InvalidRequest("self merge".into()));
        }
        if request.reason.trim().is_empty() || request.reason.len() > 4096 {
            return Err(EntityResolutionError::InvalidRequest(
                "reason must contain 1..=4096 bytes".into(),
            ));
        }
        let keep = self.read_entity(&request.keep_vid).await?;
        let merge = self.read_entity(&request.merge_vid).await?;
        if keep.entity_type != merge.entity_type {
            return Err(EntityResolutionError::TypeMismatch {
                keep: keep.entity_type,
                merge: merge.entity_type,
            });
        }
        let merge_node = self
            .graph_store
            .find_node_by_string_property("Entity", "vid", &request.merge_vid)
            .await
            .ok_or_else(|| EntityResolutionError::EntityNotFound(request.merge_vid.clone()))?;
        let existing_canonical = self
            .graph_store
            .get_node_property_string(merge_node, "canonical_vid")
            .await;
        let replay = existing_canonical.as_deref() == Some(&request.keep_vid);
        if !replay && !pair_is_candidate(&keep, &merge, &self.profile) {
            return Err(EntityResolutionError::NotCandidate);
        }
        if let Some(other) = existing_canonical
            && other != request.keep_vid
        {
            return Err(EntityResolutionError::InvalidRequest(format!(
                "merge entity already resolves to {other}"
            )));
        }

        let decision_vid = decision_vid(&request.keep_vid, &request.merge_vid);
        let maker = actor_wire(&request.actor);
        let decision = DecisionRecorder::from_shared_with_clock(
            Arc::clone(&self.graph_store),
            Arc::clone(&self.clock),
        )
        .record(
            DecisionRequestBuilder::new(&decision_vid, &self.profile.decision.category)
                .scenario(&self.profile.decision.scenario)
                .reasoning(request.reason.clone())
                .outcome(&self.profile.decision.outcome)
                .decision_maker(maker)
                .build(),
        )
        .await?;

        for entity_node in [
            self.graph_store
                .find_node_by_string_property("Entity", "vid", &request.keep_vid)
                .await,
            Some(merge_node),
        ]
        .into_iter()
        .flatten()
        {
            if self
                .graph_store
                .get_edges_between(
                    decision.node_id,
                    entity_node,
                    da_domain::relation::decision::AFFECTS,
                )
                .await
                .is_empty()
            {
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    decision.node_id,
                    entity_node,
                    da_domain::relation::decision::AFFECTS,
                    &crate::temporal_stamp::open_fact_edge_now(self.clock.now()),
                )
                .await
                .map_err(|error| EntityResolutionError::Graph(error.to_string()))?;
            }
        }

        let merge = GraphHealingUseCase::from_shared(Arc::clone(&self.graph_store))
            .with_clock(Arc::clone(&self.clock))
            .merge(
                &request.keep_vid,
                &request.merge_vid,
                &request.reason,
                request.actor,
            )
            .await
            .map_err(|error| EntityResolutionError::Graph(error.to_string()))?;
        tracing::info!(
            decision_vid,
            keep_vid = request.keep_vid,
            merge_vid = request.merge_vid,
            replay,
            "Reviewed entity resolution applied"
        );
        Ok(ReviewedMergeResult {
            decision_vid,
            decision_node_id: decision.node_id,
            merge,
        })
    }

    async fn read_entity(&self, vid: &str) -> Result<EntityResolutionInput, EntityResolutionError> {
        let node = self
            .graph_store
            .find_node_by_string_property("Entity", "vid", vid)
            .await
            .ok_or_else(|| EntityResolutionError::EntityNotFound(vid.to_string()))?;
        let entity_type = self
            .graph_store
            .get_node_property_string(node, "entity_type")
            .await
            .ok_or_else(|| {
                EntityResolutionError::InvalidRequest(format!("{vid} lacks entity_type"))
            })?;
        let label = self
            .graph_store
            .get_node_property_string(node, "label")
            .await
            .ok_or_else(|| EntityResolutionError::InvalidRequest(format!("{vid} lacks label")))?;
        Ok(EntityResolutionInput {
            vid: vid.to_string(),
            entity_type,
            label,
        })
    }
}

fn decision_vid(keep_vid: &str, merge_vid: &str) -> String {
    let digest = Sha256::digest(format!("{keep_vid}|{merge_vid}").as_bytes());
    let encoded = hex::encode(digest);
    format!("vid:decision:entity-merge:{}", &encoded[..24])
}

fn actor_wire(actor: &HealingActor) -> String {
    actor.as_str()
}
