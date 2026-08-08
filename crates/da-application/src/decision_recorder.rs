//! DecisionRecorder use case — record first-class Decision nodes (ADR-048 Phase 1).
//!
//! Captures every consequential system action as a Decision node in the graph:
//! healing actions, conflict resolutions, promotions, schema migrations,
//! extraction overrides, hypothesis tests, retractions, deprecations.
//!
//! Data-driven: `Decision.category` and `Decision.outcome` are validated against
//! `data/ontology/decision_categories.yaml` via `da_domain::process` registry
//! (`is_known_decision_category`, `is_valid_decision_outcome`).
//! `Decision.decision_maker` must start with `agent:`/`human:`/`system:` prefix.
//!
//! Phase 1 scope:
//! - creates Decision node with required + bi-temporal fields
//! - emits registered decision audit/causal wires when linking to other nodes;
//!   orchestration remains the caller's responsibility
//! - stamps `valid_from` + `recorded_at` from the injected Clock (ADR-051 / M330)
//! - rejects unknown categories/outcomes at the ontology write boundary
//!
//! Phase 2 scope (NOT here):
//! - PolicyEngine integration (policy_applied category)
//! - Agent/Person node materialisation
//!
//! ADR-048 compliance: every consequential system action records a Decision.
//! ADR-053 Layer G (entity resolution merge) calls this to record each merge.

use std::sync::Arc;

use da_domain::process::{
    is_known_decision_category, is_valid_decision_maker, is_valid_decision_outcome,
};
use da_ports::graph_store::DirectGraphStore;
use kg_ontology::temporal::TemporalEdge;

/// A request to record a Decision node.
///
/// Build with [`DecisionRequest::builder`]; fields map 1:1 to DecisionSchema.
/// `category` must be in `data/ontology/decision_categories.yaml` Phase 1.
/// `outcome` must be valid for the declared category.
/// `decision_maker` must start with `agent:` / `human:` / `system:`.
#[derive(Debug, Clone)]
pub struct DecisionRequest {
    /// Stable versioned id (vid) of the Decision.
    pub vid: String,
    /// Phase 1 category id from `decision_categories.yaml`.
    pub category: String,
    /// Short human-readable scenario identifier (e.g. "merge-entity-LORA").
    pub scenario: String,
    /// Free-form reasoning text — why this Decision was made.
    pub reasoning: String,
    /// Controlled-vocabulary outcome for the declared category.
    pub outcome: String,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    /// Decision-maker string (`agent:`, `human:`, or `system:` prefix).
    pub decision_maker: String,
    /// Optional policy_id (Phase 2: materialises Policy node).
    pub policy_id: Option<String>,
}

/// Result of recording a Decision.
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// The graph node id assigned by the store.
    pub node_id: u64,
    /// The request as submitted (echoed for caller convenience).
    pub request: DecisionRequest,
    /// Warnings emitted by semantic vocabulary checks (non-empty does NOT
    /// block the write; caller may surface them in logs).
    pub warnings: Vec<String>,
}

/// Errors returned by [`DecisionRecorder::record`].
#[derive(Debug, thiserror::Error)]
pub enum DecisionRecordError {
    /// Category is not in `decision_categories.yaml` Phase 1.
    #[error("unknown decision category: '{0}' (not in decision_categories.yaml Phase 1)")]
    UnknownCategory(String),
    /// Outcome is not valid for the declared category.
    #[error("outcome '{outcome}' not valid for category '{category}'")]
    InvalidOutcome { category: String, outcome: String },
    /// Confidence out of [0.0, 1.0].
    #[error("confidence {0} out of range [0.0, 1.0]")]
    ConfidenceOutOfRange(f64),
    /// Decision-maker string does not start with a known prefix.
    #[error("decision_maker '{0}' missing agent:/human:/system: prefix")]
    InvalidMaker(String),
    /// An existing Decision VID carries different immutable semantics.
    #[error("decision VID '{vid}' already exists with conflicting field '{field}'")]
    ConflictingVid { vid: String, field: String },
    /// Store error during node creation or property set.
    #[error("graph store error: {0}")]
    Store(String),
}

/// DecisionRecorder use case (ADR-048 Phase 1).
///
/// Records a first-class Decision node in the graph for every consequential
/// system action. Node shape matches `da_domain::process::DecisionSchema`.
pub struct DecisionRecorder {
    pub graph_store: Arc<dyn DirectGraphStore>,
    pub clock: Arc<dyn da_ports::Clock>,
}

impl DecisionRecorder {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self::from_shared(Arc::from(graph_store))
    }

    pub fn from_shared(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self::from_shared_with_clock(graph_store, Arc::new(da_ports::SystemClock))
    }

    pub fn with_clock(
        graph_store: Box<dyn DirectGraphStore>,
        clock: Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self::from_shared_with_clock(Arc::from(graph_store), clock)
    }

    pub fn from_shared_with_clock(
        graph_store: Arc<dyn DirectGraphStore>,
        clock: Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self { graph_store, clock }
    }

    /// Validate the request against the decision_categories.yaml vocabulary.
    /// Returns a list of warnings (non-blocking) or an error (blocking).
    fn validate(&self, req: &DecisionRequest) -> Result<Vec<String>, DecisionRecordError> {
        let warnings = Vec::new();

        // Confidence must be in [0.0, 1.0].
        if !(0.0..=1.0).contains(&req.confidence) {
            return Err(DecisionRecordError::ConfidenceOutOfRange(req.confidence));
        }

        // Ontology vocabulary is a hard write boundary. Extend the YAML first;
        // never free-invent categories or outcomes in graph truth.
        if !is_known_decision_category(&req.category) {
            return Err(DecisionRecordError::UnknownCategory(req.category.clone()));
        }
        if !is_valid_decision_outcome(&req.category, &req.outcome) {
            return Err(DecisionRecordError::InvalidOutcome {
                category: req.category.clone(),
                outcome: req.outcome.clone(),
            });
        }

        // Decision-maker prefix.
        if !is_valid_decision_maker(&req.decision_maker) {
            return Err(DecisionRecordError::InvalidMaker(
                req.decision_maker.clone(),
            ));
        }

        Ok(warnings)
    }

    /// Record a Decision node in the graph.
    ///
    /// Returns the assigned node id + any semantic warnings. Hard errors
    /// (unknown maker prefix, confidence out of range, store failure) prevent
    /// the write; soft warnings allow it.
    pub async fn record(
        &self,
        req: DecisionRequest,
    ) -> Result<DecisionRecord, DecisionRecordError> {
        let warnings = self.validate(&req)?;
        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property("Decision", "vid", &req.vid)
            .await
        {
            for (field, expected) in [
                ("category", req.category.as_str()),
                ("scenario", req.scenario.as_str()),
                ("outcome", req.outcome.as_str()),
            ] {
                let actual = self
                    .graph_store
                    .get_node_property_string(existing, field)
                    .await;
                if actual.as_deref() != Some(expected) {
                    return Err(DecisionRecordError::ConflictingVid {
                        vid: req.vid.clone(),
                        field: field.to_string(),
                    });
                }
            }
            return Ok(DecisionRecord {
                node_id: existing,
                request: req,
                warnings,
            });
        }
        let now = self.clock.now();

        // Create the Decision node.
        let node_id = self
            .graph_store
            .create_node("Decision")
            .await
            .map_err(|e| DecisionRecordError::Store(e.to_string()))?;

        // Required fields.
        macro_rules! set_str {
            ($key:expr, $val:expr) => {
                self.graph_store
                    .set_node_property_string(node_id, $key, $val.to_string())
                    .await
                    .map_err(|e| DecisionRecordError::Store(e.to_string()))?
            };
        }
        set_str!("vid", &req.vid);
        set_str!("category", &req.category);
        set_str!("scenario", &req.scenario);
        set_str!("reasoning", &req.reasoning);
        set_str!("outcome", &req.outcome);
        self.graph_store
            .set_node_property_float(node_id, "confidence", req.confidence)
            .await
            .map_err(|e| DecisionRecordError::Store(e.to_string()))?;
        set_str!("decision_maker", &req.decision_maker);
        set_str!("valid_from", &now.to_rfc3339());
        set_str!("recorded_at", &now.to_rfc3339());

        // Optional fields.
        if let Some(policy_id) = &req.policy_id {
            set_str!("policy_id", policy_id);
        }

        // Always-false gates (D127 + retrieval-eligibility defaults).
        self.graph_store
            .set_node_property_bool(node_id, "import_eligible", false)
            .await
            .map_err(|e| DecisionRecordError::Store(e.to_string()))?;
        self.graph_store
            .set_node_property_bool(node_id, "retrieval_eligible", true)
            .await
            .map_err(|e| DecisionRecordError::Store(e.to_string()))?;
        self.graph_store
            .set_node_property_int(node_id, "schema_version", 1)
            .await
            .map_err(|e| DecisionRecordError::Store(e.to_string()))?;

        Ok(DecisionRecord {
            node_id,
            request: req,
            warnings,
        })
    }

    /// Helper: stamp a `CAUSED` causal edge from one Decision to another (ADR-048).
    /// Phase 1 helper — actual edge wiring is the caller's responsibility; this
    /// is a convenience for the common case.
    pub async fn stamp_caused_edge(
        &self,
        from_decision_id: u64,
        to_decision_id: u64,
    ) -> Result<u64, DecisionRecordError> {
        crate::temporal_stamp::create_edge_with_temporal(
            self.graph_store.as_ref(),
            from_decision_id,
            to_decision_id,
            da_domain::relation::decision::CAUSED,
            &self.temporal_stamp_now(),
        )
        .await
        .map_err(|error| DecisionRecordError::Store(error.to_string()))
    }

    /// Build a TemporalEdge stamp at the recorder's current clock time.
    /// Callers use this when wiring causal edges manually.
    pub fn temporal_stamp_now(&self) -> TemporalEdge {
        let now = self.clock.now();
        TemporalEdge {
            valid_at: now,
            created_at: now,
            invalid_at: None,
            expired_at: None,
            reference_time: None,
        }
    }
}

/// Builder for [`DecisionRequest`].
#[derive(Debug, Clone)]
pub struct DecisionRequestBuilder {
    vid: String,
    category: String,
    scenario: String,
    reasoning: String,
    outcome: String,
    confidence: f64,
    decision_maker: String,
    policy_id: Option<String>,
}

impl DecisionRequestBuilder {
    pub fn new(vid: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            vid: vid.into(),
            category: category.into(),
            scenario: String::new(),
            reasoning: String::new(),
            outcome: String::new(),
            confidence: 1.0,
            decision_maker: "system:unknown".to_string(),
            policy_id: None,
        }
    }
    pub fn scenario(mut self, s: impl Into<String>) -> Self {
        self.scenario = s.into();
        self
    }
    pub fn reasoning(mut self, s: impl Into<String>) -> Self {
        self.reasoning = s.into();
        self
    }
    pub fn outcome(mut self, s: impl Into<String>) -> Self {
        self.outcome = s.into();
        self
    }
    pub fn confidence(mut self, c: f64) -> Self {
        self.confidence = c;
        self
    }
    pub fn decision_maker(mut self, m: impl Into<String>) -> Self {
        self.decision_maker = m.into();
        self
    }
    pub fn policy_id(mut self, p: impl Into<String>) -> Self {
        self.policy_id = Some(p.into());
        self
    }
    pub fn build(self) -> DecisionRequest {
        DecisionRequest {
            vid: self.vid,
            category: self.category,
            scenario: self.scenario,
            reasoning: self.reasoning,
            outcome: self.outcome,
            confidence: self.confidence,
            decision_maker: self.decision_maker,
            policy_id: self.policy_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Builder tests ───

    #[test]
    fn test_builder_minimal() {
        let req = DecisionRequestBuilder::new("dec-001", "healing_action")
            .scenario("merge-entity-LORA")
            .reasoning("LoRA mentioned in 3 papers; merged to canonical")
            .outcome("approved")
            .decision_maker("system:healing-pipeline")
            .build();
        assert_eq!(req.vid, "dec-001");
        assert_eq!(req.category, "healing_action");
        assert_eq!(req.outcome, "approved");
        assert_eq!(req.decision_maker, "system:healing-pipeline");
        assert_eq!(req.confidence, 1.0);
        assert_eq!(req.policy_id, None);
    }

    #[test]
    fn test_builder_with_policy_id() {
        let req = DecisionRequestBuilder::new("dec-002", "promotion")
            .outcome("promoted")
            .policy_id("policy:retention-365d")
            .build();
        assert_eq!(req.policy_id.as_deref(), Some("policy:retention-365d"));
    }

    // ─── Validation tests (no store needed) ───

    fn recorder() -> DecisionRecorder {
        DecisionRecorder::new(Box::new(kg_storage::MockGraphStore::new()))
    }

    #[test]
    fn test_validate_confidence_out_of_range_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "healing_action")
            .outcome("approved")
            .confidence(1.5)
            .decision_maker("system:test")
            .build();
        let err = r.validate(&req).unwrap_err();
        assert!(matches!(
            err,
            DecisionRecordError::ConfidenceOutOfRange(1.5)
        ));
    }

    #[test]
    fn test_validate_negative_confidence_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "healing_action")
            .outcome("approved")
            .confidence(-0.1)
            .decision_maker("system:test")
            .build();
        assert!(matches!(
            r.validate(&req).unwrap_err(),
            DecisionRecordError::ConfidenceOutOfRange(_)
        ));
    }

    #[test]
    fn test_validate_unknown_maker_prefix_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "healing_action")
            .outcome("approved")
            .decision_maker("anonymous")
            .build();
        assert!(matches!(
            r.validate(&req).unwrap_err(),
            DecisionRecordError::InvalidMaker(_)
        ));
    }

    #[test]
    fn test_validate_unknown_category_is_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "bogus_category")
            .outcome("approved")
            .decision_maker("system:test")
            .build();
        assert!(matches!(
            r.validate(&req).unwrap_err(),
            DecisionRecordError::UnknownCategory(category) if category == "bogus_category"
        ));
    }

    #[test]
    fn test_validate_valid_request_no_warnings() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "healing_action")
            .scenario("merge")
            .reasoning("because")
            .outcome("approved")
            .confidence(0.95)
            .decision_maker("system:healing-pipeline")
            .build();
        let warnings = r.validate(&req).unwrap();
        assert!(
            warnings.is_empty(),
            "expected no warnings for valid request, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validate_outcome_mismatch_is_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "healing_action")
            .outcome("definitely_bogus_outcome")
            .decision_maker("system:test")
            .build();
        assert!(matches!(
            r.validate(&req).unwrap_err(),
            DecisionRecordError::InvalidOutcome { .. }
        ));
    }

    #[test]
    fn test_validate_unknown_phase2_category_is_rejected() {
        let r = recorder();
        let req = DecisionRequestBuilder::new("dec-x", "policy_applied")
            .outcome("allowed")
            .decision_maker("system:policy-engine")
            .build();
        assert!(matches!(
            r.validate(&req).unwrap_err(),
            DecisionRecordError::UnknownCategory(category) if category == "policy_applied"
        ));
    }

    #[test]
    fn test_temporal_stamp_now_has_valid_created() {
        let r = recorder();
        let stamp = r.temporal_stamp_now();
        assert!(stamp.valid_at <= stamp.created_at + chrono::Duration::seconds(1));
        assert!(stamp.invalid_at.is_none());
        assert!(stamp.expired_at.is_none());
    }

    // ─── Integration tests with MockGraphStore (write path) ───

    #[tokio::test]
    async fn test_record_valid_decision_writes_node() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let req = DecisionRequestBuilder::new("dec-100", "healing_action")
            .scenario("merge-entity-LORA")
            .reasoning("LoRA mentioned in 3 papers")
            .outcome("approved")
            .confidence(0.95)
            .decision_maker("system:healing-pipeline")
            .build();
        let record = recorder.record(req).await.expect("record ok");
        // MockGraphStore assigns node ids starting from 0 (internally AtomicU64).
        assert_eq!(record.node_id, 0);
        assert!(
            record.warnings.is_empty(),
            "no warnings: {:?}",
            record.warnings
        );
    }

    #[tokio::test]
    async fn test_record_invalid_maker_fails_before_write() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let req = DecisionRequestBuilder::new("dec-101", "healing_action")
            .outcome("approved")
            .decision_maker("anonymous")
            .build();
        let err = recorder.record(req).await.unwrap_err();
        assert!(matches!(err, DecisionRecordError::InvalidMaker(_)));
    }

    #[tokio::test]
    async fn test_record_confidence_out_of_range_fails_before_write() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let req = DecisionRequestBuilder::new("dec-102", "healing_action")
            .outcome("approved")
            .confidence(2.0)
            .decision_maker("system:test")
            .build();
        assert!(matches!(
            recorder.record(req).await.unwrap_err(),
            DecisionRecordError::ConfidenceOutOfRange(_)
        ));
    }

    #[tokio::test]
    async fn test_record_unknown_category_fails_before_write() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let req = DecisionRequestBuilder::new("dec-103", "bogus_category")
            .outcome("approved")
            .decision_maker("system:test")
            .build();
        assert!(matches!(
            recorder.record(req).await.unwrap_err(),
            DecisionRecordError::UnknownCategory(_)
        ));
        assert_eq!(recorder.graph_store.node_count().await, 0);
    }

    #[tokio::test]
    async fn test_record_same_vid_is_idempotent_and_conflict_is_rejected() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let request = DecisionRequestBuilder::new("dec-idempotent", "healing_action")
            .scenario("entity-merge")
            .reasoning("reviewed duplicate")
            .outcome("approved")
            .decision_maker("human:reviewer")
            .build();
        let first = recorder.record(request.clone()).await.unwrap();
        let second = recorder.record(request).await.unwrap();
        assert_eq!(first.node_id, second.node_id);
        assert_eq!(recorder.graph_store.node_count().await, 1);

        let conflict = DecisionRequestBuilder::new("dec-idempotent", "healing_action")
            .scenario("different-scenario")
            .outcome("approved")
            .decision_maker("human:reviewer")
            .build();
        assert!(matches!(
            recorder.record(conflict).await.unwrap_err(),
            DecisionRecordError::ConflictingVid { .. }
        ));
    }

    #[tokio::test]
    async fn caused_edge_uses_temporal_string_contract() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let left = recorder.graph_store.create_node("Decision").await.unwrap();
        let right = recorder.graph_store.create_node("Decision").await.unwrap();
        let edge = recorder.stamp_caused_edge(left, right).await.unwrap();
        let valid_at = recorder
            .graph_store
            .get_edge_property_string(edge, "valid_at")
            .await
            .expect("valid_at");
        let created_at = recorder
            .graph_store
            .get_edge_property_string(edge, "created_at")
            .await
            .expect("created_at");
        assert!(chrono::DateTime::parse_from_rfc3339(&valid_at).is_ok());
        assert!(chrono::DateTime::parse_from_rfc3339(&created_at).is_ok());
    }

    #[tokio::test]
    async fn test_record_policy_id_optional_field_written() {
        let mock = kg_storage::MockGraphStore::new();
        let recorder = DecisionRecorder::new(Box::new(mock));
        let req = DecisionRequestBuilder::new("dec-104", "promotion")
            .outcome("promoted")
            .decision_maker("system:gate")
            .policy_id("policy:retention-365d")
            .build();
        let record = recorder.record(req).await.expect("record ok");
        assert_eq!(
            record.request.policy_id.as_deref(),
            Some("policy:retention-365d")
        );
    }
}
