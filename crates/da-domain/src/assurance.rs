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

    /// ACAP runtime admission decision for a transition from `self` to
    /// `target` (PIR-014). Fail-closed: only declared forward transitions or
    /// the failure sink `FailedVisible` are admitted; `FailedVisible` itself
    /// is a terminal sink (nothing transitions out of it); an admission must
    /// carry a durable evidence reference.
    ///
    /// Pure domain value — no I/O, no graph truth. The orchestrator maps a
    /// canonical pipeline stage id to `target` and passes the durable record
    /// (cache key sha256 / manifest key / source checksum) that establishes
    /// `self` as `evidence_ref`; the reference must never be taken verbatim
    /// from caller-supplied input.
    pub fn admit_transition(self, target: Self, evidence_ref: &str) -> AdmissionDecision {
        if evidence_ref.trim().is_empty() {
            return AdmissionDecision::blocked(TransitionAdmissionError::new(
                TransitionAdmissionCode::MissingEvidence,
                "admission requires a durable evidence reference (cache key sha256 / manifest key / source checksum)",
                evidence_ref.to_string(),
                self,
                target,
            ));
        }
        if self == Self::FailedVisible {
            return AdmissionDecision::blocked(TransitionAdmissionError::new(
                TransitionAdmissionCode::TerminalSink,
                "prior state is FailedVisible, a terminal ACAP sink: no stage may follow a failure disposition",
                evidence_ref.to_string(),
                self,
                target,
            ));
        }
        if target == Self::FailedVisible || self.can_transition_to(target) {
            AdmissionDecision::admitted(evidence_ref.to_string())
        } else {
            AdmissionDecision::blocked(TransitionAdmissionError::new(
                TransitionAdmissionCode::IllegalTransition,
                format!(
                    "{self:?} -> {target:?} is not a declared ACAP forward transition (target is not the failure sink)"
                ),
                evidence_ref.to_string(),
                self,
                target,
            ))
        }
    }
}

/// Stable machine-readable ACAP runtime admission failure codes (PIR-014).
///
/// These codes are part of the report/log surface: [`TransitionAdmissionError::error_class`]
/// renders the stable token `admission:<code>` that run reports and logs carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionAdmissionCode {
    /// `prior -> target` is not a declared ACAP forward transition and target
    /// is not the failure sink `FailedVisible`.
    IllegalTransition,
    /// The run has no prior ACAP stage state (first step without entry
    /// evidence). Runs must start from a defined entry state with durable
    /// evidence.
    MissingEntryState,
    /// The prior state is `FailedVisible`, a terminal ACAP sink: no stage may
    /// follow a failure disposition.
    TerminalSink,
    /// Admission was requested without a durable evidence reference.
    MissingEvidence,
}

impl TransitionAdmissionCode {
    /// Stable wire/log token (matches the serde snake_case spelling).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IllegalTransition => "illegal_transition",
            Self::MissingEntryState => "missing_entry_state",
            Self::TerminalSink => "terminal_sink",
            Self::MissingEvidence => "missing_evidence",
        }
    }
}

impl std::fmt::Display for TransitionAdmissionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed ACAP runtime admission refusal (PIR-014).
///
/// Pure domain value: stable code + human/typed reason + durable evidence
/// reference. No I/O. Emitted into run reports (Failed disposition) and logs.
/// The `evidence_ref` must name a durable record produced by the orchestrator
/// itself (cache key sha256 / manifest key / source checksum), never a
/// caller-supplied string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("stage transition {from:?} -> {to:?} not admitted ({code}): {reason}")]
pub struct TransitionAdmissionError {
    pub code: TransitionAdmissionCode,
    pub reason: String,
    pub evidence_ref: String,
    pub from: ArticleContextState,
    pub to: ArticleContextState,
}

impl TransitionAdmissionError {
    pub fn new(
        code: TransitionAdmissionCode,
        reason: impl Into<String>,
        evidence_ref: impl Into<String>,
        from: ArticleContextState,
        to: ArticleContextState,
    ) -> Self {
        Self {
            code,
            reason: reason.into(),
            evidence_ref: evidence_ref.into(),
            from,
            to,
        }
    }

    /// Stable error-class token for reports and logs: `admission:<code>`.
    pub fn error_class(&self) -> String {
        format!("admission:{}", self.code.as_str())
    }
}

/// ACAP runtime admission decision for one pipeline stage transition (PIR-014).
///
/// Computed from a prior stage state and the target stage state. Fail-closed:
/// a blocked decision carries the typed [`TransitionAdmissionError`] and the
/// executor must not run. Pure value — serializable into run reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionDecision {
    pub admitted: bool,
    /// Durable evidence reference backing this decision. For admitted
    /// transitions it names the record establishing the prior state; for
    /// blocked transitions the record the refusal is based on.
    pub evidence_ref: String,
    /// Typed refusal when `admitted == false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TransitionAdmissionError>,
}

impl AdmissionDecision {
    pub fn admitted(evidence_ref: impl Into<String>) -> Self {
        Self {
            admitted: true,
            evidence_ref: evidence_ref.into(),
            error: None,
        }
    }

    pub fn blocked(error: TransitionAdmissionError) -> Self {
        let evidence_ref = error.evidence_ref.clone();
        Self {
            admitted: false,
            evidence_ref,
            error: Some(error),
        }
    }
}

/// ACAP runtime admission for a pipeline stage transition (PIR-014), including
/// the run-entry case.
///
/// `prior` is the established ACAP state before the stage runs. `None` means
/// the run provides no prior stage evidence (first step without entry
/// evidence): fail-closed — [`TransitionAdmissionCode::MissingEntryState`]
/// unless the orchestrator supplies an explicit entry state derived from
/// durable source evidence (e.g. checksum-bound `SourceIdentity`).
pub fn admit_transition(
    prior: Option<ArticleContextState>,
    target: ArticleContextState,
    evidence_ref: &str,
) -> AdmissionDecision {
    match prior {
        Some(prior) => prior.admit_transition(target, evidence_ref),
        None => AdmissionDecision::blocked(TransitionAdmissionError::new(
            TransitionAdmissionCode::MissingEntryState,
            "no prior ACAP stage state: the run must start from a defined entry state with durable evidence",
            evidence_ref.to_string(),
            ArticleContextState::SourceRegistered,
            target,
        )),
    }
}

/// ACAP runtime admission for a pipeline stage transition (PIR-014) in a
/// stage-subset run (the orchestrator gate).
///
/// The strict [`ArticleContextState::admit_transition`] requires declared
/// adjacent forward transitions. Production pipelines execute a *subset* of
/// canonical stages and legitimately skip layers (e.g. `parse -> extract ->
/// embed` or the thin `parse -> summary` pipeline), so the runtime gate admits
/// monotonic forward progress instead: a gated stage may establish any ACAP
/// state at or after the state established by the previous gated stage. It
/// fails closed on: missing durable evidence, a `FailedVisible` prior (terminal
/// sink), a mid-FSM run entry (first gated stage above `Parsed`), and FSM
/// regression (a later stage moving to an earlier layer).
///
/// `prior` is the ACAP state established by the previous gated stage, or
/// `None` for the first gated stage of the run (the entry case — see
/// [`TransitionAdmissionCode::MissingEntryState`]). `evidence_ref` must be a
/// durable record produced by the orchestrator (cache key sha256 / manifest
/// key / source checksum), never a caller-supplied string. Pure domain value —
/// no I/O, no graph truth.
pub fn admit_stage_transition(
    prior: Option<ArticleContextState>,
    target: ArticleContextState,
    evidence_ref: &str,
) -> AdmissionDecision {
    use ArticleContextState::*;
    use TransitionAdmissionCode::*;
    let from = prior.unwrap_or(SourceRegistered);
    if evidence_ref.trim().is_empty() {
        let (code, reason) = match prior {
            None => (
                MissingEntryState,
                "no prior ACAP stage state and no durable source evidence: the run must start from a defined entry state",
            ),
            Some(_) => (
                MissingEvidence,
                "admission requires a durable evidence reference (cache key sha256 / manifest key / source checksum)",
            ),
        };
        return AdmissionDecision::blocked(TransitionAdmissionError::new(
            code,
            reason,
            evidence_ref.to_string(),
            from,
            target,
        ));
    }
    if prior == Some(FailedVisible) {
        return AdmissionDecision::blocked(TransitionAdmissionError::new(
            TerminalSink,
            "prior state is FailedVisible, a terminal ACAP sink: no stage may follow a failure disposition",
            evidence_ref.to_string(),
            FailedVisible,
            target,
        ));
    }
    if target == FailedVisible {
        return AdmissionDecision::admitted(evidence_ref.to_string());
    }
    match prior {
        None => {
            // Entry stage: only the first FSM layers may start a run. A run
            // that begins mid-FSM (e.g. first step = summarized) fails closed
            // instead of default-admitting (Q3 guardrail).
            if target <= Parsed {
                AdmissionDecision::admitted(evidence_ref.to_string())
            } else {
                AdmissionDecision::blocked(TransitionAdmissionError::new(
                    IllegalTransition,
                    format!(
                        "{target:?} is not a valid entry stage: runs must start from the first ACAP FSM layers (source_registered/source_verified/parsed) with durable source evidence"
                    ),
                    evidence_ref.to_string(),
                    from,
                    target,
                ))
            }
        }
        Some(prior) => {
            if target >= prior {
                AdmissionDecision::admitted(evidence_ref.to_string())
            } else {
                AdmissionDecision::blocked(TransitionAdmissionError::new(
                    IllegalTransition,
                    format!(
                        "{prior:?} -> {target:?} regresses the ACAP FSM: stage-subset pipelines may skip layers forward but never move backward"
                    ),
                    evidence_ref.to_string(),
                    prior,
                    target,
                ))
            }
        }
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

    // ── PIR-014 ACAP runtime admission gate (T01) ──

    #[test]
    fn declared_forward_transition_is_admitted_with_evidence() {
        let decision = ArticleContextState::SourceRegistered
            .admit_transition(ArticleContextState::SourceVerified, "sha256:source-v1");
        assert!(decision.admitted);
        assert_eq!(decision.evidence_ref, "sha256:source-v1");
        assert!(decision.error.is_none());
        let decision = ArticleContextState::Parsed
            .admit_transition(ArticleContextState::Structured, "sha256:parse-v1");
        assert!(decision.admitted);
    }

    #[test]
    fn failure_sink_is_always_reachable() {
        for prior in [
            ArticleContextState::SourceRegistered,
            ArticleContextState::Extracted,
            ArticleContextState::AcceptedCheckpoint,
        ] {
            let decision = prior.admit_transition(ArticleContextState::FailedVisible, "ev-1");
            assert!(decision.admitted, "prior {prior:?}");
        }
    }

    #[test]
    fn illegal_jump_fails_closed_with_typed_reason() {
        let decision = ArticleContextState::SourceRegistered
            .admit_transition(ArticleContextState::Parsed, "ev-1");
        assert!(!decision.admitted);
        assert_eq!(decision.evidence_ref, "ev-1");
        let error = decision.error.expect("blocked decision carries error");
        assert_eq!(error.code, TransitionAdmissionCode::IllegalTransition);
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.from, ArticleContextState::SourceRegistered);
        assert_eq!(error.to, ArticleContextState::Parsed);
        assert_eq!(error.evidence_ref, "ev-1");
        assert!(!error.reason.is_empty());
    }

    #[test]
    fn backward_jump_fails_closed() {
        let decision =
            ArticleContextState::Summarized.admit_transition(ArticleContextState::Parsed, "ev-1");
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            TransitionAdmissionCode::IllegalTransition
        );
    }

    #[test]
    fn failed_visible_is_a_terminal_sink() {
        for target in [
            ArticleContextState::SourceRegistered,
            ArticleContextState::Parsed,
            ArticleContextState::FailedVisible,
        ] {
            let decision = ArticleContextState::FailedVisible.admit_transition(target, "ev-1");
            assert!(!decision.admitted, "target {target:?}");
            assert_eq!(
                decision.error.unwrap().code,
                TransitionAdmissionCode::TerminalSink
            );
        }
    }

    #[test]
    fn admission_requires_durable_evidence() {
        let decision = ArticleContextState::SourceRegistered
            .admit_transition(ArticleContextState::SourceVerified, "   ");
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            TransitionAdmissionCode::MissingEvidence
        );
    }

    #[test]
    fn missing_prior_state_fails_closed_on_entry() {
        let decision = admit_transition(None, ArticleContextState::Summarized, "ev-1");
        assert!(!decision.admitted);
        let error = decision.error.unwrap();
        assert_eq!(error.code, TransitionAdmissionCode::MissingEntryState);
        assert_eq!(error.from, ArticleContextState::SourceRegistered);
        assert_eq!(error.to, ArticleContextState::Summarized);
        // An explicit entry state with durable evidence admits the first stage.
        let decision = admit_transition(
            Some(ArticleContextState::SourceRegistered),
            ArticleContextState::SourceVerified,
            "sha256:source-v1",
        );
        assert!(decision.admitted);
    }

    #[test]
    fn admission_decision_serde_roundtrip() {
        crate::roundtrip_json(&AdmissionDecision::admitted("sha256:parse-v1"));
        let blocked = AdmissionDecision::blocked(TransitionAdmissionError::new(
            TransitionAdmissionCode::IllegalTransition,
            "not declared",
            "ev-1",
            ArticleContextState::Parsed,
            ArticleContextState::Summarized,
        ));
        crate::roundtrip_json(&blocked);
        for code in [
            TransitionAdmissionCode::IllegalTransition,
            TransitionAdmissionCode::MissingEntryState,
            TransitionAdmissionCode::TerminalSink,
            TransitionAdmissionCode::MissingEvidence,
        ] {
            crate::roundtrip_json(&code);
        }
    }

    #[test]
    fn admission_error_exposes_stable_error_class_and_display() {
        let error = TransitionAdmissionError::new(
            TransitionAdmissionCode::TerminalSink,
            "terminal sink",
            "ev-1",
            ArticleContextState::FailedVisible,
            ArticleContextState::Parsed,
        );
        assert_eq!(error.error_class(), "admission:terminal_sink");
        assert_eq!(
            error.to_string(),
            "stage transition FailedVisible -> Parsed not admitted (terminal_sink): terminal sink"
        );
    }

    proptest! {
        #[test]
        fn admission_gate_matches_fsm_rule_for_evidence_bound_transitions(a in 0usize..15, b in 0usize..15) {
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
            // Terminal sink: nothing transitions out of FailedVisible.
            let expected = if a == 14 {
                false
            } else {
                b == 14 || b == a + 1
            };
            let decision = states[a].admit_transition(states[b], "sha256:ev");
            prop_assert_eq!(decision.admitted, expected);
            prop_assert_eq!(decision.admitted, decision.error.is_none());
        }
    }

    // ── PIR-014 stage-subset runtime gate (T02) ──

    #[test]
    fn stage_gate_admits_first_layer_entry_with_source_evidence() {
        for entry in [
            ArticleContextState::SourceRegistered,
            ArticleContextState::SourceVerified,
            ArticleContextState::Parsed,
        ] {
            let decision = admit_stage_transition(None, entry, "source:sha256:abc");
            assert!(decision.admitted, "entry {entry:?}");
            assert_eq!(decision.evidence_ref, "source:sha256:abc");
            assert!(decision.error.is_none());
        }
    }

    #[test]
    fn stage_gate_rejects_mid_fsm_entry() {
        for entry in [
            ArticleContextState::Structured,
            ArticleContextState::Extracted,
            ArticleContextState::Embedded,
            ArticleContextState::Summarized,
        ] {
            let decision = admit_stage_transition(None, entry, "source:sha256:abc");
            assert!(!decision.admitted, "entry {entry:?}");
            let error = decision.error.expect("blocked entry carries error");
            assert_eq!(error.code, TransitionAdmissionCode::IllegalTransition);
            assert_eq!(error.error_class(), "admission:illegal_transition");
        }
    }

    #[test]
    fn stage_gate_requires_durable_source_evidence_on_entry() {
        let decision = admit_stage_transition(None, ArticleContextState::Parsed, "   ");
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            TransitionAdmissionCode::MissingEntryState
        );
    }

    #[test]
    fn stage_gate_admits_forward_progress_and_skips_layers() {
        // Thin pipeline parse -> summary skips Structured..Resolved legitimately.
        let first = admit_stage_transition(None, ArticleContextState::Parsed, "source:sha256:abc");
        assert!(first.admitted);
        let second = admit_stage_transition(
            Some(ArticleContextState::Parsed),
            ArticleContextState::Summarized,
            "key:parse-v1",
        );
        assert!(second.admitted, "{second:?}");
        assert_eq!(second.evidence_ref, "key:parse-v1");
        // Repeated same-layer stages (e.g. figure aliases of parse) stay legal.
        let repeat = admit_stage_transition(
            Some(ArticleContextState::Parsed),
            ArticleContextState::Parsed,
            "key:parse-v1",
        );
        assert!(repeat.admitted, "{repeat:?}");
    }

    #[test]
    fn stage_gate_rejects_fsm_regression() {
        let decision = admit_stage_transition(
            Some(ArticleContextState::Summarized),
            ArticleContextState::Parsed,
            "key:summary-v1",
        );
        assert!(!decision.admitted);
        let error = decision.error.unwrap();
        assert_eq!(error.code, TransitionAdmissionCode::IllegalTransition);
        assert_eq!(error.from, ArticleContextState::Summarized);
        assert_eq!(error.to, ArticleContextState::Parsed);
        assert_eq!(error.evidence_ref, "key:summary-v1");
    }

    #[test]
    fn stage_gate_rejects_missing_evidence_after_entry() {
        let decision = admit_stage_transition(
            Some(ArticleContextState::Parsed),
            ArticleContextState::Summarized,
            "  ",
        );
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            TransitionAdmissionCode::MissingEvidence
        );
    }

    #[test]
    fn stage_gate_keeps_failed_visible_terminal() {
        let decision = admit_stage_transition(
            Some(ArticleContextState::FailedVisible),
            ArticleContextState::Parsed,
            "key:failed",
        );
        assert!(!decision.admitted);
        assert_eq!(
            decision.error.unwrap().code,
            TransitionAdmissionCode::TerminalSink
        );
        let sink = admit_stage_transition(
            Some(ArticleContextState::Parsed),
            ArticleContextState::FailedVisible,
            "key:parse-v1",
        );
        assert!(sink.admitted, "any state may reach the failure sink");
    }
}
