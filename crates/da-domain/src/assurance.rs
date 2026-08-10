//! Article Context Assurance Plane value types (ADR-055).
//!
//! These types describe evidence and verdicts. They perform no I/O and never
//! write graph truth. Producers live behind ports; application use cases build
//! reports from already-observed stage results.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

pub const ACAP_POLICY_ID: &str = "acap-ontology-v2";
pub const ACAP_REPORT_SCHEMA_VERSION: &str = "article-context-assurance-report.v1";

pub const ACAP_AGGREGATE_REPORT_SCHEMA_VERSION: &str = "article-context-assurance-aggregate.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum AssuranceError {
    #[error("SHA-256 digest must be exactly 64 lowercase hexadecimal characters")]
    InvalidSha256,
    #[error("article ID must not be empty")]
    EmptyArticleId,
    #[error("report schema version does not match per-article ACAP schema")]
    SchemaVersionDrift,
    #[error("report policy ID does not match accepted ACAP policy")]
    PolicyIdDrift,
    #[error("policy SHA-256 must be present")]
    MissingPolicySha256,
    #[error("corpus assurance report must keep import_eligible=false")]
    D127UnlockForbidden,
    #[error("corpus assurance report is invalid: {0}")]
    InvalidCorpusReport(String),
}

/// Validated lowercase SHA-256 hex digest used for source/sink lineage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    pub fn of_text(text: &str) -> Self {
        Self::of_bytes(text.as_bytes())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentDigest {
    type Error = AssuranceError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(Self(value))
        } else {
            Err(AssuranceError::InvalidSha256)
        }
    }
}

impl From<ContentDigest> for String {
    fn from(value: ContentDigest) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryMode {
    FullFidelity,
    LosslessProjection,
    BoundedDerivative,
    SemanticCompression,
    ReviewCandidate,
    NonAuthoritativeDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessReason {
    SourceUnavailable,
    NotApplicable,
    PolicySkipped,
    UpstreamBlocked,
    RetryPending,
    TerminalFailure,
    GenuinelyAbsent,
    NotMaterialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleContextState {
    SourceRegistered,
    SourceVerified,
    Parsed,
    Structured,
    Extracted,
    EvidenceLinked,
    Embedded,
    Materialized,
    Enriched,
    Resolved,
    Summarized,
    Finalized,
    TaskAccepted,
    AcceptedCheckpoint,
    FailedVisible,
}

impl ArticleContextState {
    pub fn can_transition_to(self, target: Self) -> bool {
        use ArticleContextState::*;
        target == FailedVisible
            || matches!(
                (self, target),
                (SourceRegistered, SourceVerified)
                    | (SourceVerified, Parsed)
                    | (Parsed, Structured)
                    | (Structured, Extracted)
                    | (Extracted, EvidenceLinked)
                    | (EvidenceLinked, Embedded)
                    | (Embedded, Materialized)
                    | (Materialized, Enriched)
                    | (Enriched, Resolved)
                    | (Resolved, Summarized)
                    | (Summarized, Finalized)
                    | (Finalized, TaskAccepted)
                    | (TaskAccepted, AcceptedCheckpoint)
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConservationObservation {
    pub field: String,
    pub mode: BoundaryMode,
    pub source_bytes: u64,
    pub sink_bytes: u64,
    pub source_sha256: ContentDigest,
    pub sink_sha256: ContentDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missingness: Option<MissingnessReason>,
}

impl ConservationObservation {
    pub fn hard_failure(&self) -> bool {
        matches!(
            self.mode,
            BoundaryMode::FullFidelity | BoundaryMode::LosslessProjection
        ) && (self.source_bytes != self.sink_bytes || self.source_sha256 != self.sink_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountObservation {
    pub name: String,
    pub expected: u64,
    pub observed: u64,
}

impl CountObservation {
    pub fn hard_failure(&self) -> bool {
        self.expected != self.observed
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteDelta {
    pub node_writes: u64,
    #[serde(default)]
    pub property_writes: u64,
    pub edge_writes: u64,
    pub vector_writes: u64,
    #[serde(default)]
    pub rejected_writes: u64,
}

impl WriteDelta {
    /// Total successfully applied physical writes. Rejections are reported as
    /// a separate hard metric and deliberately excluded from this sum.
    pub fn total(self) -> u64 {
        self.node_writes + self.property_writes + self.edge_writes + self.vector_writes
    }
}

/// One mutation family partition (attempted = applied + unchanged + rejected).
/// Domain-owned mirror of kg-storage counters so reports stay self-validating
/// without depending on infrastructure crates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationFamilyObservation {
    pub attempted: u64,
    pub applied: u64,
    pub unchanged: u64,
    pub rejected: u64,
}

impl MutationFamilyObservation {
    pub fn partition_is_valid(self) -> bool {
        self.attempted == self.applied + self.unchanged + self.rejected
    }
}

/// Full physical replay observation preserved in ACAP reports (ADR-055).
///
/// When present, this is the authoritative replay evidence. `WriteDelta` remains
/// a diagnostic projection of applied/rejected totals only and cannot prove
/// `attempted > 0`, `unchanged > 0`, or `unobserved_operations == 0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayObservation {
    pub node: MutationFamilyObservation,
    pub property: MutationFamilyObservation,
    pub edge: MutationFamilyObservation,
    pub vector: MutationFamilyObservation,
    pub unobserved_operations: u64,
}

impl ReplayObservation {
    pub fn attempted(self) -> u64 {
        self.node.attempted + self.property.attempted + self.edge.attempted + self.vector.attempted
    }

    pub fn applied(self) -> u64 {
        self.node.applied + self.property.applied + self.edge.applied + self.vector.applied
    }

    pub fn unchanged(self) -> u64 {
        self.node.unchanged + self.property.unchanged + self.edge.unchanged + self.vector.unchanged
    }

    pub fn rejected(self) -> u64 {
        self.node.rejected + self.property.rejected + self.edge.rejected + self.vector.rejected
    }

    pub fn partition_is_valid(self) -> bool {
        self.node.partition_is_valid()
            && self.property.partition_is_valid()
            && self.edge.partition_is_valid()
            && self.vector.partition_is_valid()
            && self.attempted() == self.applied() + self.unchanged() + self.rejected()
    }

    /// Authoritative ACAP hard gate for physical replay integrity.
    pub fn integrity_passes(self) -> bool {
        self.partition_is_valid()
            && self.unobserved_operations == 0
            && self.attempted() > 0
            && self.unchanged() > 0
            && self.applied() == 0
            && self.rejected() == 0
    }

    /// Diagnostic applied/rejected projection. Never sufficient for integrity.
    pub fn applied_write_delta(self) -> WriteDelta {
        WriteDelta {
            node_writes: self.node.applied,
            property_writes: self.property.applied,
            edge_writes: self.edge.applied,
            vector_writes: self.vector.applied,
            rejected_writes: self.rejected(),
        }
    }

    /// Test/helper: valid zero-write replay with N unchanged property ops.
    pub fn valid_unchanged_property(unchanged: u64) -> Self {
        Self {
            property: MutationFamilyObservation {
                attempted: unchanged,
                applied: 0,
                unchanged,
                rejected: 0,
            },
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerAssuranceResult {
    pub layer_id: String,
    pub state: ArticleContextState,
    pub article_count: u64,
    pub available_required_fields: u64,
    pub retained_required_fields: u64,
    pub conservations: Vec<ConservationObservation>,
    pub counts: Vec<CountObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_writes: Option<WriteDelta>,
    /// Authoritative physical mutation partition when present.
    /// Absent means legacy v1 report that only carried diagnostic WriteDelta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_observation: Option<ReplayObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_missingness: Option<MissingnessReason>,
    pub d127_unlock_count: u64,
    pub quality_metrics: BTreeMap<String, f64>,
    pub hard_failures: Vec<String>,
}

impl LayerAssuranceResult {
    pub fn required_field_retention(&self) -> Option<f64> {
        (self.available_required_fields != 0)
            .then(|| self.retained_required_fields as f64 / self.available_required_fields as f64)
    }

    pub fn evaluate_hard_failures(&mut self) {
        let mut failures = Vec::new();
        if self.available_required_fields == 0 {
            failures.push("required_field_retention:undefined_denominator".to_string());
        } else if self.retained_required_fields != self.available_required_fields {
            failures.push(format!(
                "required_field_retention:{}/{}",
                self.retained_required_fields, self.available_required_fields
            ));
        }
        for item in &self.conservations {
            if item.hard_failure() {
                failures.push(format!("byte_conservation:{}", item.field));
            }
        }
        for item in &self.counts {
            if item.hard_failure() {
                failures.push(format!(
                    "count:{}:{}/{}",
                    item.name, item.observed, item.expected
                ));
            }
        }
        // Physical replay is materialization-layer evidence. Pre-materialization
        // layers (source/parse/structure/...) must set
        // replay_missingness=NotApplicable with no observation/writes so they are
        // not fail-closed for an inapplicable gate.
        let replay_not_applicable = self.replay_observation.is_none()
            && self.replay_writes.is_none()
            && self.replay_missingness == Some(MissingnessReason::NotApplicable);
        if !replay_not_applicable {
            match self.replay_observation {
                Some(obs) => {
                    // Authoritative path: full physical partition is self-validating.
                    if !obs.partition_is_valid() {
                        failures.push("replay_observation_integrity:invalid_partition".into());
                    }
                    if obs.unobserved_operations != 0 {
                        failures.push(format!(
                            "replay_observation_integrity:unobserved:{}",
                            obs.unobserved_operations
                        ));
                    }
                    if obs.attempted() == 0 {
                        failures.push("replay_observation_integrity:empty_attempt".into());
                    } else if obs.unchanged() == 0 {
                        failures.push("replay_observation_integrity:zero_unchanged".into());
                    }
                    if obs.rejected() != 0 {
                        failures.push(format!("replay_rejected_writes:{}", obs.rejected()));
                    }
                    if obs.applied() != 0 {
                        failures.push(format!("replay_net_writes:{}", obs.applied()));
                    }
                }
                None => {
                    // Legacy v1 path: only diagnostic WriteDelta is available.
                    // NetWrites alone cannot prove integrity; missing full observation
                    // is itself a hard failure when no write delta was recorded either.
                    match self.replay_writes {
                        Some(writes) if writes.rejected_writes != 0 => {
                            failures
                                .push(format!("replay_rejected_writes:{}", writes.rejected_writes));
                            if writes.total() != 0 {
                                failures.push(format!("replay_net_writes:{}", writes.total()));
                            }
                            failures
                                .push("replay_observation_integrity:legacy_v1_incomplete".into());
                        }
                        Some(writes) if writes.total() != 0 => {
                            failures.push(format!("replay_net_writes:{}", writes.total()));
                            failures
                                .push("replay_observation_integrity:legacy_v1_incomplete".into());
                        }
                        None => {
                            failures.push("replay_net_writes:missing_observation".into());
                        }
                        Some(_) => {
                            // Zero applied/rejected WriteDelta without full partition:
                            // compatible with old PASS artifacts but incomplete for
                            // recomputing integrity from the report alone.
                            failures
                                .push("replay_observation_integrity:legacy_v1_incomplete".into());
                        }
                    }
                }
            }
        }
        if self.d127_unlock_count != 0 {
            failures.push(format!("d127_unlock_count:{}", self.d127_unlock_count));
        }
        failures.sort();
        failures.dedup();
        self.hard_failures = failures;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceVerdict {
    Pass,
    Fail,
    NoClaim,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleAssuranceReport {
    pub schema_version: String,
    pub policy_id: String,
    pub policy_sha256: ContentDigest,
    pub article_id: String,
    pub current_state: ArticleContextState,
    pub layer_results: Vec<LayerAssuranceResult>,
    pub verdict: AssuranceVerdict,
}

impl ArticleAssuranceReport {
    pub fn new(
        article_id: impl Into<String>,
        policy_sha256: ContentDigest,
        current_state: ArticleContextState,
        mut layer_results: Vec<LayerAssuranceResult>,
    ) -> Result<Self, AssuranceError> {
        let article_id = article_id.into();
        if article_id.trim().is_empty() {
            return Err(AssuranceError::EmptyArticleId);
        }
        for layer in &mut layer_results {
            layer.evaluate_hard_failures();
        }
        let verdict = if layer_results.is_empty() {
            AssuranceVerdict::NoClaim
        } else if layer_results
            .iter()
            .any(|layer| !layer.hard_failures.is_empty())
        {
            AssuranceVerdict::Fail
        } else {
            AssuranceVerdict::Pass
        };
        Ok(Self {
            schema_version: ACAP_REPORT_SCHEMA_VERSION.to_string(),
            policy_id: ACAP_POLICY_ID.to_string(),
            policy_sha256,
            article_id,
            current_state,
            layer_results,
            verdict,
        })
    }

    /// Recompute all derived fields after deserialization. Serialized verdicts
    /// and hard-failure lists are never trusted as evidence inputs.
    pub fn normalize(mut self) -> Result<Self, AssuranceError> {
        if self.article_id.trim().is_empty() {
            return Err(AssuranceError::EmptyArticleId);
        }
        if self.schema_version != ACAP_REPORT_SCHEMA_VERSION {
            return Err(AssuranceError::SchemaVersionDrift);
        }
        if self.policy_id != ACAP_POLICY_ID {
            return Err(AssuranceError::PolicyIdDrift);
        }
        for layer in &mut self.layer_results {
            layer.evaluate_hard_failures();
        }
        self.verdict = if self.layer_results.is_empty() {
            AssuranceVerdict::NoClaim
        } else if self
            .layer_results
            .iter()
            .any(|layer| !layer.hard_failures.is_empty())
        {
            AssuranceVerdict::Fail
        } else {
            AssuranceVerdict::Pass
        };
        Ok(self)
    }
}

/// Status of a layer row inside a corpus aggregate report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusLayerStatus {
    /// At least one article contributed an observed layer result.
    Observed,
    /// Layer declared by policy but not present in the corpus artifact.
    NoClaim,
}

/// Aggregated layer row for the corpus assurance report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusLayerResult {
    pub layer_id: String,
    pub enforcement: String,
    pub status: CorpusLayerStatus,
    pub article_count: u64,
    pub available_required_fields: u64,
    pub retained_required_fields: u64,
    /// Typed hard-failure codes (product shape). Empty means no hard failures.
    pub hard_failures: Vec<String>,
    pub hard_failure_articles: u64,
    pub pass_articles: u64,
    pub fail_articles: u64,
    pub no_claim_articles: u64,
    pub d127_unlock_count: u64,
    pub replay_net_writes: u64,
    pub replay_integrity_pass_articles: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missingness: Option<MissingnessReason>,
}

/// Corpus-level diagnostic and hard aggregate metrics (ADR-055).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusAggregateMetrics {
    /// Union of hard-failure codes across the corpus (product list shape).
    pub hard_failures: Vec<String>,
    pub hard_pass_rate: f64,
    pub weighted_layer_coverage: f64,
    /// Diagnostic only — never overrides hard invariants.
    pub weighted_quality_score: f64,
    pub replay_net_writes: u64,
    pub d127_unlock_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_p95_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_missingness: Option<MissingnessReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_missingness: Option<MissingnessReason>,
}

/// Canonical corpus ACAP aggregate (distinct schema from per-article reports).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusAssuranceReport {
    pub schema_version: String,
    pub policy_id: String,
    pub policy_sha256: ContentDigest,
    pub article_count: u64,
    /// Opaque pipeline identity (manifest/schema binding), not a git SHA claim.
    pub pipeline_revision: String,
    pub layer_results: Vec<CorpusLayerResult>,
    pub fsm_results: BTreeMap<String, u64>,
    pub cache_results: BTreeMap<String, String>,
    pub prompt_results: BTreeMap<String, String>,
    pub optimizer_results: BTreeMap<String, String>,
    pub aggregate: CorpusAggregateMetrics,
    pub import_eligible: bool,
    pub source_artifact_sha256: ContentDigest,
    pub verdict: AssuranceVerdict,
    /// Deterministic digest of all fields except this one.
    pub report_sha256: String,
}

impl CorpusAssuranceReport {
    pub fn validate(&self) -> Result<(), AssuranceError> {
        if self.schema_version != ACAP_AGGREGATE_REPORT_SCHEMA_VERSION {
            return Err(AssuranceError::SchemaVersionDrift);
        }
        if self.policy_id != ACAP_POLICY_ID {
            return Err(AssuranceError::PolicyIdDrift);
        }
        if self.import_eligible {
            return Err(AssuranceError::D127UnlockForbidden);
        }
        if self.report_sha256 != corpus_report_digest(self) {
            return Err(AssuranceError::InvalidCorpusReport(
                "report digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Digest over the report with `report_sha256` cleared, bound to aggregate schema.
pub fn corpus_report_digest(report: &CorpusAssuranceReport) -> String {
    let mut clone = report.clone();
    clone.report_sha256 = String::new();
    let json = serde_json::to_string(&clone).expect("corpus report must serialize");
    let hash = Sha256::digest(json.as_bytes());
    let mut binder = Vec::with_capacity(hash.len() + ACAP_AGGREGATE_REPORT_SCHEMA_VERSION.len());
    binder.extend_from_slice(&hash);
    binder.extend_from_slice(ACAP_AGGREGATE_REPORT_SCHEMA_VERSION.as_bytes());
    hex::encode(Sha256::digest(&binder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn layer() -> LayerAssuranceResult {
        let text = "TAIL-科学";
        LayerAssuranceResult {
            layer_id: "materialization".into(),
            state: ArticleContextState::Materialized,
            article_count: 1,
            available_required_fields: 2,
            retained_required_fields: 2,
            conservations: vec![ConservationObservation {
                field: "section.text".into(),
                mode: BoundaryMode::FullFidelity,
                source_bytes: text.len() as u64,
                sink_bytes: text.len() as u64,
                source_sha256: ContentDigest::of_text(text),
                sink_sha256: ContentDigest::of_text(text),
                missingness: None,
            }],
            counts: vec![CountObservation {
                name: "sections".into(),
                expected: 1,
                observed: 1,
            }],
            replay_writes: Some(WriteDelta::default()),
            replay_observation: Some(ReplayObservation::valid_unchanged_property(1)),
            replay_missingness: None,
            d127_unlock_count: 0,
            quality_metrics: BTreeMap::from([("quality_score".into(), 1.0)]),
            hard_failures: vec![],
        }
    }

    #[test]
    fn digest_and_report_serde_roundtrip() {
        let report = ArticleAssuranceReport::new(
            "2507.19457",
            ContentDigest::of_text("policy"),
            ArticleContextState::Materialized,
            vec![layer()],
        )
        .unwrap();
        crate::roundtrip_json(&report);
    }

    #[test]
    fn write_delta_reads_pre_property_counter_schema_safely() {
        let delta: WriteDelta =
            serde_json::from_str(r#"{"node_writes":1,"edge_writes":2,"vector_writes":3}"#).unwrap();
        assert_eq!(delta.property_writes, 0);
        assert_eq!(delta.rejected_writes, 0);
        assert_eq!(delta.total(), 6);
    }

    #[test]
    fn content_digest_rejects_noncanonical_hex() {
        assert!(ContentDigest::try_from("A".repeat(64)).is_err());
        assert!(ContentDigest::try_from("0".repeat(63)).is_err());
        assert!(ContentDigest::try_from("g".repeat(64)).is_err());
    }

    #[test]
    fn unicode_full_fidelity_uses_bytes_and_sha256() {
        let text = "TAIL-科学";
        assert_ne!(text.len(), text.chars().count());
        assert_eq!(
            ContentDigest::of_text(text).as_str(),
            hex::encode(Sha256::digest(text.as_bytes()))
        );
        assert!(!layer().conservations[0].hard_failure());
    }

    #[test]
    fn hard_failure_cannot_be_masked_by_quality_score() {
        let mut failed = layer();
        failed.conservations[0].sink_bytes -= 1;
        failed.quality_metrics.insert("quality_score".into(), 1.0);
        let report = ArticleAssuranceReport::new(
            "paper",
            ContentDigest::of_text("policy"),
            ArticleContextState::Materialized,
            vec![failed],
        )
        .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert_eq!(
            report.layer_results[0].hard_failures,
            vec!["byte_conservation:section.text"]
        );
    }

    #[test]
    fn normalize_rejects_wrong_schema_version() {
        let mut report = ArticleAssuranceReport::new(
            "paper",
            ContentDigest::of_text("policy"),
            ArticleContextState::Materialized,
            vec![layer()],
        )
        .unwrap();
        report.schema_version = "tampered.v9".into();
        assert_eq!(
            report.normalize().unwrap_err(),
            AssuranceError::SchemaVersionDrift
        );
    }

    #[test]
    fn normalize_rejects_wrong_policy_id() {
        let mut report = ArticleAssuranceReport::new(
            "paper",
            ContentDigest::of_text("policy"),
            ArticleContextState::Materialized,
            vec![layer()],
        )
        .unwrap();
        report.policy_id = "tampered-policy".into();
        assert_eq!(
            report.normalize().unwrap_err(),
            AssuranceError::PolicyIdDrift
        );
    }

    #[test]
    fn normalize_accepts_correct_identity() {
        let report = ArticleAssuranceReport::new(
            "paper",
            ContentDigest::of_text("policy"),
            ArticleContextState::Materialized,
            vec![layer()],
        )
        .unwrap();
        assert!(report.clone().normalize().is_ok());
        // normalize preserves identity — does not silently rewrite
        let normalized = report.normalize().unwrap();
        assert_eq!(normalized.schema_version, ACAP_REPORT_SCHEMA_VERSION);
        assert_eq!(normalized.policy_id, ACAP_POLICY_ID);
    }

    #[test]
    fn zero_denominator_is_undefined_and_fails_closed() {
        let mut undefined = layer();
        undefined.available_required_fields = 0;
        undefined.retained_required_fields = 0;
        undefined.evaluate_hard_failures();
        assert_eq!(undefined.required_field_retention(), None);
        assert!(
            undefined
                .hard_failures
                .contains(&"required_field_retention:undefined_denominator".to_string())
        );
    }

    #[test]
    fn full_replay_observation_is_self_validating_and_serde_stable() {
        let obs = ReplayObservation::valid_unchanged_property(270);
        assert!(obs.integrity_passes());
        assert_eq!(obs.applied_write_delta().total(), 0);
        crate::roundtrip_json(&obs);
    }

    #[test]
    fn empty_or_applied_replay_observation_fails_integrity() {
        assert!(!ReplayObservation::default().integrity_passes());
        let mut applied = ReplayObservation::valid_unchanged_property(2);
        applied.property.applied = 1;
        applied.property.unchanged = 1;
        assert!(!applied.integrity_passes());
    }

    #[test]
    fn evaluate_hard_failures_uses_full_observation_when_present() {
        let mut layer = layer();
        layer.replay_observation = Some(ReplayObservation {
            property: MutationFamilyObservation {
                attempted: 2,
                applied: 1,
                unchanged: 1,
                rejected: 0,
            },
            ..ReplayObservation::default()
        });
        layer.replay_writes = Some(WriteDelta {
            property_writes: 1,
            ..WriteDelta::default()
        });
        layer.evaluate_hard_failures();
        assert!(
            layer
                .hard_failures
                .contains(&"replay_net_writes:1".to_string())
        );
        assert!(!layer.hard_failures.iter().any(|f| f.contains("legacy_v1")));
    }

    #[test]
    fn legacy_v1_zero_write_delta_without_full_observation_is_incomplete() {
        let mut layer = layer();
        layer.replay_observation = None;
        layer.replay_writes = Some(WriteDelta::default());
        layer.evaluate_hard_failures();
        assert!(
            layer
                .hard_failures
                .contains(&"replay_observation_integrity:legacy_v1_incomplete".to_string())
        );
    }

    #[test]
    fn legacy_v1_artifact_json_deserializes_without_replay_observation() {
        let json = r#"{
            "layer_id": "materialization",
            "state": "materialized",
            "article_count": 1,
            "available_required_fields": 2,
            "retained_required_fields": 2,
            "conservations": [],
            "counts": [],
            "replay_writes": {
                "node_writes": 0,
                "property_writes": 0,
                "edge_writes": 0,
                "vector_writes": 0,
                "rejected_writes": 0
            },
            "d127_unlock_count": 0,
            "quality_metrics": {},
            "hard_failures": []
        }"#;
        let mut layer: LayerAssuranceResult = serde_json::from_str(json).unwrap();
        assert!(layer.replay_observation.is_none());
        layer.evaluate_hard_failures();
        assert!(
            layer
                .hard_failures
                .contains(&"replay_observation_integrity:legacy_v1_incomplete".to_string())
        );
    }

    #[test]
    fn pre_materialization_layer_skips_physical_replay_when_not_applicable() {
        let mut layer = LayerAssuranceResult {
            layer_id: "source".into(),
            state: ArticleContextState::SourceVerified,
            article_count: 1,
            available_required_fields: 1,
            retained_required_fields: 1,
            conservations: vec![],
            counts: vec![],
            replay_writes: None,
            replay_observation: None,
            replay_missingness: Some(MissingnessReason::NotApplicable),
            d127_unlock_count: 0,
            quality_metrics: BTreeMap::new(),
            hard_failures: vec![],
        };
        layer.evaluate_hard_failures();
        assert!(
            layer.hard_failures.is_empty(),
            "failures: {:?}",
            layer.hard_failures
        );
    }

    #[test]
    fn materialization_missing_replay_without_not_applicable_still_fails() {
        let mut layer = LayerAssuranceResult {
            layer_id: "materialization".into(),
            state: ArticleContextState::Materialized,
            article_count: 1,
            available_required_fields: 1,
            retained_required_fields: 1,
            conservations: vec![],
            counts: vec![],
            replay_writes: None,
            replay_observation: None,
            replay_missingness: None,
            d127_unlock_count: 0,
            quality_metrics: BTreeMap::new(),
            hard_failures: vec![],
        };
        layer.evaluate_hard_failures();
        assert!(
            layer
                .hard_failures
                .contains(&"replay_net_writes:missing_observation".into())
        );
    }

    proptest! {
        #[test]
        fn only_declared_forward_or_failure_fsm_transitions_are_legal(a in 0usize..15, b in 0usize..15) {
            let states = [
                ArticleContextState::SourceRegistered, ArticleContextState::SourceVerified,
                ArticleContextState::Parsed, ArticleContextState::Structured,
                ArticleContextState::Extracted, ArticleContextState::EvidenceLinked,
                ArticleContextState::Embedded, ArticleContextState::Materialized,
                ArticleContextState::Enriched, ArticleContextState::Resolved,
                ArticleContextState::Summarized, ArticleContextState::Finalized,
                ArticleContextState::TaskAccepted, ArticleContextState::AcceptedCheckpoint,
                ArticleContextState::FailedVisible,
            ];
            let expected = b == 14 || (a < 13 && b == a + 1);
            prop_assert_eq!(states[a].can_transition_to(states[b]), expected);
        }
    }
}
