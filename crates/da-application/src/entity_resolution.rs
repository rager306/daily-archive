//! Decision-backed entity resolution orchestration (ADR-053 Layer G).

use std::sync::Arc;

use da_domain::entity_resolution::{
    BlockingResult, EntityResolutionInput, EntityResolutionProfile, block_candidates,
    pair_is_candidate,
};
use da_domain::healing::{HealingActor, MergeResult};
use da_domain::promotion::{EvidenceRef, PromotionCandidate, PromotionError, PromotionKind};
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

/// Deterministic verifier identity for the entity-resolution surface: the
/// blocking profile + `pair_is_candidate` check (ADR-053).
pub const ER_BLOCKING_VERIFIER: &str = "entity-resolution:blocking-check";

/// Deterministic reviewer identity for the entity-resolution surface: the
/// decision-record review gate (ADR-053 Layer G).
pub const ER_DECISION_REVIEWER: &str = "entity-resolution:decision-review";

impl ReviewedMergeResult {
    /// Adapter to the shared authority-neutral promotion protocol (PIR-015).
    ///
    /// A reviewed entity-resolution merge becomes a
    /// [`PromotionKind::EntityResolution`] candidate at
    /// `PromotionState::Reviewed`: the pair already passed the independent
    /// blocking check (verifier) and the decision-backed review (reviewer),
    /// so no further verification is replayed. The candidate is **never
    /// produced as `Promoted`** — deployment authority stays separate and a
    /// separate authority must promote through the application fail-closed
    /// service. Durable evidence refs: the merge pair (proposal), the
    /// blocking-profile outcome (verification), and the decision record
    /// (review). `import_eligible` stays false (D127 lock).
    pub fn to_promotion_candidate(&self) -> Result<PromotionCandidate, PromotionError> {
        let proposer = self.merge.provenance.actor.as_str();
        let pair = format!("{}:{}", self.merge.kept_vid, self.merge.merged_vid);
        let proposed = PromotionCandidate::propose(
            PromotionKind::EntityResolution,
            format!("er-merge:{pair}"),
            proposer,
            vec![EvidenceRef::new(format!("merge:{pair}")).with_kind("merge_provenance")],
        )?;
        let verified = proposed.verify(
            ER_BLOCKING_VERIFIER,
            vec![EvidenceRef::new(format!("blocking:{pair}")).with_kind("blocking_profile")],
        )?;
        verified.review(
            ER_DECISION_REVIEWER,
            vec![
                EvidenceRef::new(format!("decision:{}", self.decision_vid))
                    .with_kind("decision_record"),
            ],
        )
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemExactAliasApplied {
    pub keep_vid: String,
    pub merge_vid: String,
    pub decision_vid: String,
    pub decision_node_id: u64,
    pub replay: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemExactAliasSkip {
    pub keep_vid: String,
    pub merge_vid: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemExactAliasFailure {
    pub keep_vid: String,
    pub merge_vid: String,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemExactAliasBatchResult {
    pub candidate_count: usize,
    pub exact_pair_count: usize,
    pub applied: Vec<SystemExactAliasApplied>,
    pub skipped: Vec<SystemExactAliasSkip>,
    pub failed: Vec<SystemExactAliasFailure>,
    pub oversized_blocks: usize,
    pub profile_version: String,
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
            if self
                .graph_store
                .get_node_property_string(node, "canonical_vid")
                .await
                .is_some()
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

    /// Apply system-reviewed merges for blocking candidates (Exact and/or
    /// AlphanumericFold keys). Profile keeps `auto_merge: false` — merges are
    /// never silent: each apply records a Decision (ADR-053 Layer G) with actor
    /// `system:entity-resolution-blocking-alias-v1`.
    pub async fn apply_system_exact_alias_candidates(
        &self,
    ) -> Result<SystemExactAliasBatchResult, EntityResolutionError> {
        // Backward-compatible name: historically Exact-only; now all blocking pairs.
        self.apply_system_blocking_alias_candidates().await
    }

    /// System-reviewed Decision merges for every blocking candidate pair.
    ///
    /// Runs to closure: after each applied merge, re-inventory candidates so
    /// residual fold/exact pairs involving survivors are resolved in the same
    /// batch. Replay of a fully-closed graph applies zero new Decisions.
    pub async fn apply_system_blocking_alias_candidates(
        &self,
    ) -> Result<SystemExactAliasBatchResult, EntityResolutionError> {
        let initial = self.candidates().await;
        let candidate_count = initial.candidates.len();
        let oversized_blocks = initial.oversized_blocks.len();

        let mut applied = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = Vec::new();
        let mut resolved: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        // Closure loop: keep applying until a pass produces no new merges.
        for _round in 0..64 {
            let blocking = self.candidates().await;
            let mut pairs = blocking.candidates;
            pairs.sort_by(|a, b| (&a.left_vid, &a.right_vid).cmp(&(&b.left_vid, &b.right_vid)));
            let mut round_applied = 0_usize;
            for pair in pairs {
                let key_kinds: Vec<String> = pair
                    .shared_keys
                    .iter()
                    .map(|key| format!("{:?}", key.kind).to_ascii_lowercase())
                    .collect();
                let (keep_vid, merge_vid) = if pair.left_vid <= pair.right_vid {
                    (pair.left_vid.clone(), pair.right_vid.clone())
                } else {
                    (pair.right_vid.clone(), pair.left_vid.clone())
                };
                if resolved.contains(&merge_vid) {
                    skipped.push(SystemExactAliasSkip {
                        keep_vid: keep_vid.clone(),
                        merge_vid: merge_vid.clone(),
                        reason: "merge_already_resolved_in_batch".into(),
                    });
                    continue;
                }
                // If keep was itself merged into something else this batch, skip.
                if resolved.contains(&keep_vid) {
                    skipped.push(SystemExactAliasSkip {
                        keep_vid,
                        merge_vid,
                        reason: "keep_already_merged_away_in_batch".into(),
                    });
                    continue;
                }
                // Prefer keep retrieval-eligible if one side already silenced.
                let keep_node = self
                    .graph_store
                    .find_node_by_string_property("Entity", "vid", &keep_vid)
                    .await;
                let merge_node = self
                    .graph_store
                    .find_node_by_string_property("Entity", "vid", &merge_vid)
                    .await;
                let (keep_vid, merge_vid) = match (keep_node, merge_node) {
                    (Some(k), Some(m)) => {
                        let k_ret = self
                            .graph_store
                            .get_node_property_bool(k, "retrieval_eligible")
                            .await
                            != Some(false);
                        let m_ret = self
                            .graph_store
                            .get_node_property_bool(m, "retrieval_eligible")
                            .await
                            != Some(false);
                        if !k_ret && m_ret {
                            (merge_vid, keep_vid)
                        } else {
                            (keep_vid, merge_vid)
                        }
                    }
                    _ => (keep_vid, merge_vid),
                };

                let reason = format!(
                    "System-reviewed blocking alias (profile v{}, keys=[{}]); merge {} into {}",
                    self.profile.version,
                    key_kinds.join(","),
                    merge_vid,
                    keep_vid
                );
                match self
                    .resolve_reviewed_merge(ReviewedMergeRequest {
                        keep_vid: keep_vid.clone(),
                        merge_vid: merge_vid.clone(),
                        reason,
                        actor: HealingActor::System("entity-resolution-blocking-alias-v1".into()),
                    })
                    .await
                {
                    Ok(result) => {
                        // Only the silenced merge side is closed; keep can absorb
                        // residual fold/exact aliases in later rounds.
                        resolved.insert(merge_vid.clone());
                        applied.push(SystemExactAliasApplied {
                            keep_vid,
                            merge_vid,
                            decision_vid: result.decision_vid,
                            decision_node_id: result.decision_node_id,
                            replay: false,
                        });
                        round_applied += 1;
                        // One merge per round then re-inventory for residual pairs.
                        break;
                    }
                    Err(error) => {
                        failed.push(SystemExactAliasFailure {
                            keep_vid,
                            merge_vid,
                            error: error.to_string(),
                        });
                    }
                }
            }
            if round_applied == 0 {
                break;
            }
        }

        Ok(SystemExactAliasBatchResult {
            candidate_count,
            exact_pair_count: applied.len() + skipped.len() + failed.len(),
            applied,
            skipped,
            failed,
            oversized_blocks,
            profile_version: self.profile.version.clone(),
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
