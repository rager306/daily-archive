//! Revision-scoped deterministic readiness observation and materialization.

use std::sync::Arc;

use da_domain::relation::structure::{
    HAS_MANIFESTATION, HAS_PART, HAS_QUALITY_SIGNAL, HAS_SUMMARY,
};
use da_domain::revision_readiness::{
    RevisionReadinessAssessment, RevisionReadinessObservation, RevisionReadinessPolicy,
};
use da_domain::vid::sha256_vid;
use da_ports::{Clock, DirectGraphStore, SystemClock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const AGENT_READINESS_SIGNAL_TYPE: &str = "agent_readiness";
pub const AGENT_READINESS_METHOD: &str = "grounded_revision_policy.v1";

#[derive(Debug, Clone)]
pub struct RevisionReadinessPolicyBinding {
    pub policy: RevisionReadinessPolicy,
    pub configuration_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessResult {
    pub revision_vid: String,
    pub signal_vid: String,
    pub signal_node_id: u64,
    pub evidence_sha256: String,
    pub observation: RevisionReadinessObservation,
    pub assessment: RevisionReadinessAssessment,
    pub previous_signals_invalidated: usize,
    pub edge_created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RevisionReadinessError {
    #[error("PaperRevision node missing or mismatched for VID {0}")]
    MissingRevision(String),
    #[error("invalid readiness policy: {0}")]
    InvalidPolicy(String),
    #[error("invalid readiness policy SHA256 {0:?}")]
    InvalidPolicySha(String),
    #[error("graph operation failed: {0}")]
    Graph(String),
    #[error("serialize readiness evidence: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub struct RevisionReadinessUseCase {
    store: Arc<dyn DirectGraphStore>,
    clock: Arc<dyn Clock>,
    binding: RevisionReadinessPolicyBinding,
}

impl RevisionReadinessUseCase {
    pub fn new(
        store: Arc<dyn DirectGraphStore>,
        binding: RevisionReadinessPolicyBinding,
    ) -> Result<Self, RevisionReadinessError> {
        Self::with_clock(store, binding, Arc::new(SystemClock))
    }

    pub fn with_clock(
        store: Arc<dyn DirectGraphStore>,
        binding: RevisionReadinessPolicyBinding,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, RevisionReadinessError> {
        binding
            .policy
            .validate()
            .map_err(|error| RevisionReadinessError::InvalidPolicy(error.to_string()))?;
        if binding.configuration_sha256.len() != 64
            || !binding
                .configuration_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(RevisionReadinessError::InvalidPolicySha(
                binding.configuration_sha256,
            ));
        }
        Ok(Self {
            store,
            clock,
            binding,
        })
    }

    pub async fn materialize(
        &self,
        revision_node_id: u64,
        revision_vid: &str,
    ) -> Result<RevisionReadinessResult, RevisionReadinessError> {
        if self
            .store
            .find_node_by_string_property("PaperRevision", "vid", revision_vid)
            .await
            != Some(revision_node_id)
        {
            return Err(RevisionReadinessError::MissingRevision(
                revision_vid.to_string(),
            ));
        }

        let observation = self.observe(revision_node_id, revision_vid).await;
        let assessment = self
            .binding
            .policy
            .evaluate(&observation)
            .map_err(|error| RevisionReadinessError::InvalidPolicy(error.to_string()))?;
        let evidence_bytes = serde_json::to_vec(&observation)?;
        let evidence_sha256 = hex::encode(Sha256::digest(&evidence_bytes));
        let signal_vid = sha256_vid(
            "revision_readiness",
            &format!(
                "{revision_vid}\n{}\n{evidence_sha256}",
                self.binding.configuration_sha256
            ),
        );
        let signal_node = match self
            .store
            .find_node_by_string_property("QualitySignal", "vid", &signal_vid)
            .await
        {
            Some(node) => node,
            None => self
                .store
                .create_node("QualitySignal")
                .await
                .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?,
        };

        let previous_signals_invalidated = self
            .invalidate_previous(revision_node_id, signal_node)
            .await?;
        self.write_signal(
            signal_node,
            &signal_vid,
            revision_vid,
            &evidence_sha256,
            &observation,
            &assessment,
        )
        .await?;
        da_domain::validate_compiled_edge_endpoint(
            HAS_QUALITY_SIGNAL,
            "PaperRevision",
            "QualitySignal",
        )
        .map_err(RevisionReadinessError::Graph)?;
        let existing_edges = self
            .store
            .get_edges_between(revision_node_id, signal_node, HAS_QUALITY_SIGNAL)
            .await;
        let edge_created = existing_edges.is_empty();
        if edge_created {
            let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
            crate::temporal_stamp::create_edge_with_temporal(
                self.store.as_ref(),
                revision_node_id,
                signal_node,
                HAS_QUALITY_SIGNAL,
                &temporal,
            )
            .await
            .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        }
        Ok(RevisionReadinessResult {
            revision_vid: revision_vid.to_string(),
            signal_vid,
            signal_node_id: signal_node,
            evidence_sha256,
            observation,
            assessment,
            previous_signals_invalidated,
            edge_created,
        })
    }

    async fn observe(
        &self,
        revision_node_id: u64,
        revision_vid: &str,
    ) -> RevisionReadinessObservation {
        let outgoing = self.store.get_outgoing_edges(revision_node_id).await;
        let mut sections = Vec::new();
        let mut manifestation_count = 0_u32;
        for (target, edge_type) in &outgoing {
            let Some(target_vid) = self.store.get_node_property_string(*target, "vid").await else {
                continue;
            };
            if edge_type == HAS_PART
                && self
                    .store
                    .find_node_by_string_property("Section", "vid", &target_vid)
                    .await
                    == Some(*target)
            {
                sections.push(*target);
            } else if edge_type == HAS_MANIFESTATION
                && self
                    .store
                    .find_node_by_string_property("SourceManifestation", "vid", &target_vid)
                    .await
                    == Some(*target)
            {
                manifestation_count += 1;
            }
        }
        let mut grounded_summary_count = 0_u32;
        let mut summary_type_counts = std::collections::BTreeMap::new();
        for section in &sections {
            let source_vid = self.store.get_node_property_string(*section, "vid").await;
            for (summary, edge_type) in self.store.get_outgoing_edges(*section).await {
                if edge_type != HAS_SUMMARY {
                    continue;
                }
                let exact_source = self
                    .store
                    .get_node_property_string(summary, "source_id")
                    .await
                    == source_vid;
                let exact_revision = self
                    .store
                    .get_node_property_string(summary, "revision_vid")
                    .await
                    .as_deref()
                    == Some(revision_vid);
                if !exact_source || !exact_revision {
                    continue;
                }
                grounded_summary_count += 1;
                if let Some(summary_type) = self
                    .store
                    .get_node_property_string(summary, "summary_type")
                    .await
                {
                    *summary_type_counts.entry(summary_type).or_insert(0) += 1;
                }
            }
        }
        RevisionReadinessObservation {
            revision_vid: revision_vid.to_string(),
            section_count: sections.len() as u32,
            grounded_summary_count,
            summary_type_counts,
            manifestation_count,
        }
    }

    async fn invalidate_previous(
        &self,
        revision_node_id: u64,
        current_signal: u64,
    ) -> Result<usize, RevisionReadinessError> {
        let mut invalidated = 0;
        for (target, edge_type) in self.store.get_outgoing_edges(revision_node_id).await {
            if edge_type != HAS_QUALITY_SIGNAL || target == current_signal {
                continue;
            }
            if self
                .store
                .get_node_property_string(target, "signal_type")
                .await
                .as_deref()
                != Some(AGENT_READINESS_SIGNAL_TYPE)
                || self
                    .store
                    .get_node_property_bool(target, "is_current")
                    .await
                    != Some(true)
            {
                continue;
            }
            self.store
                .set_node_property_bool(target, "is_current", false)
                .await
                .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
            for (edge_id, _) in self
                .store
                .get_edges_between(revision_node_id, target, HAS_QUALITY_SIGNAL)
                .await
            {
                if self
                    .store
                    .get_edge_property_string(edge_id, "invalid_at")
                    .await
                    .is_none()
                {
                    self.store
                        .set_edge_property_string(
                            edge_id,
                            "invalid_at",
                            &self.clock.now().to_rfc3339(),
                        )
                        .await
                        .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
                    invalidated += 1;
                }
            }
        }
        Ok(invalidated)
    }

    async fn write_signal(
        &self,
        node: u64,
        signal_vid: &str,
        revision_vid: &str,
        evidence_sha256: &str,
        observation: &RevisionReadinessObservation,
        assessment: &RevisionReadinessAssessment,
    ) -> Result<(), RevisionReadinessError> {
        let text = serde_json::to_string(assessment)?;
        let blockers = serde_json::to_string(&assessment.blocker_codes)?;
        let diagnostics = serde_json::to_string(&assessment.diagnostic_codes)?;
        let role_counts = serde_json::to_string(&observation.summary_type_counts)?;
        for (key, value) in [
            ("vid", signal_vid.to_string()),
            ("text", text),
            ("signal_type", AGENT_READINESS_SIGNAL_TYPE.into()),
            ("source_label", "PaperRevision".into()),
            ("source_id", revision_vid.to_string()),
            ("revision_vid", revision_vid.to_string()),
            ("state", assessment.state.as_str().into()),
            ("blocker_codes", blockers),
            ("diagnostic_codes", diagnostics),
            ("summary_type_counts", role_counts),
            ("evidence_sha256", evidence_sha256.into()),
            (
                "policy_schema_version",
                self.binding.policy.schema_version.clone(),
            ),
            ("policy_sha256", self.binding.configuration_sha256.clone()),
            ("method", AGENT_READINESS_METHOD.into()),
        ] {
            self.store
                .set_node_property_string(node, key, value)
                .await
                .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        }
        for (key, value) in [
            ("section_count", observation.section_count),
            ("grounded_summary_count", observation.grounded_summary_count),
            ("manifestation_count", observation.manifestation_count),
            ("summary_coverage_bps", assessment.summary_coverage_bps),
        ] {
            self.store
                .set_node_property_int(node, key, i64::from(value))
                .await
                .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        }
        self.store
            .set_node_property_bool(node, "is_current", true)
            .await
            .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        self.store
            .set_node_property_bool(node, "import_eligible", false)
            .await
            .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        self.store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await
            .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        self.store
            .set_node_property_int(node, "schema_version", 1)
            .await
            .map_err(|error| RevisionReadinessError::Graph(error.to_string()))?;
        Ok(())
    }
}
