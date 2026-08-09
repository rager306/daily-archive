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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum AssuranceError {
    #[error("SHA-256 digest must be exactly 64 lowercase hexadecimal characters")]
    InvalidSha256,
    #[error("article ID must not be empty")]
    EmptyArticleId,
    #[error("policy SHA-256 must be present")]
    MissingPolicySha256,
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
        match self.replay_writes {
            Some(writes) if writes.rejected_writes != 0 => {
                failures.push(format!("replay_rejected_writes:{}", writes.rejected_writes));
                if writes.total() != 0 {
                    failures.push(format!("replay_net_writes:{}", writes.total()));
                }
            }
            Some(writes) if writes.total() != 0 => {
                failures.push(format!("replay_net_writes:{}", writes.total()));
            }
            None => failures.push("replay_net_writes:missing_observation".to_string()),
            Some(_) => {}
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
        self.schema_version = ACAP_REPORT_SCHEMA_VERSION.to_string();
        self.policy_id = ACAP_POLICY_ID.to_string();
        Ok(self)
    }
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
