//! S08/T04 native route-recovery ledger.
//!
//! S08/T01 profiled the native CRF core, S08/T02 qualified bounded SMP on it and
//! S08/T03 measured 570 Java CRF model invocations over the pinned corpus from
//! captured and verified stage tapes. This module is the fourth and last piece:
//! it JOINS the already-declared surfaces into per-stage recovery verdicts and
//! quantifies them, so "which native routes are recoverable, and how much Java
//! work do they account for?" has one answer that is derived from DATA rather
//! than asserted in prose.
//!
//! Three surfaces are joined, and none of them is duplicated:
//!
//! 1. `data/parse/grobid_compatibility.yaml#defaults.cascade_stages` — the closed
//!    P0..P9 production skeleton (Java stage name, production authority, native
//!    surface). Every stage row must mirror it exactly, so a stage cannot be
//!    invented, dropped, or silently re-pointed at another surface.
//! 2. `data/parse/native_fulltext_composition.yaml#admission.current_matrix` with
//!    `component_receipts` — which components are `passed`, and the four
//!    substitution conditions (`admission.native_substitution_requires`).
//! 3. `artifacts/parse_compat/native_java_work.json` — the measured per-model
//!    Java invocation counts (S08/T03), which this ledger attributes to stages.
//!
//! The verdict is closed and fail-closed. `verdict_rule.order` in DATA declares
//! the guard order; the emitter asserts that order against its own enum, so a
//! reordered rule is a contract violation rather than a silent reclassification.
//! A stage reaches `recoverable` only by passing every guard:
//!
//! ```text
//! native_surface_absent      -> not_qualified
//! no_admitted_component      -> not_qualified
//! component_not_admitted     -> fallback / not_qualified
//! admission_conditions_unmet -> not_qualified
//! core_qualification_missing -> not_qualified
//! java_work_not_observed     -> not_observed
//! no_attributed_java_work    -> not_qualified
//! all_conditions_met         -> recoverable
//! ```
//!
//! Nothing here is a route. `production_route`, `auto_promote`,
//! `graph_mutation` and `selection` stay `forbidden`, `deployed` and
//! `import_eligible` stay false, `decision_point` stays 6 and
//! `independently_selectable` stays exactly `[header, references]`. The ledger
//! qualifies offline receipts; it never dispatches, promotes or writes a graph.
//!
//! Absence is typed, never zero: when the Java-work receipt carries no
//! measurement, every would-be-recoverable stage becomes `not_observed`, every
//! model count becomes a typed absence, and no total is published — a zero from
//! absence is structurally impossible because there is no numeric field to read.
//!
//! Java CPU wall-clock per stage and RSS ceilings remain `not_observed`; H05 is
//! NOT closed by this ledger and says so (`h05_closure: not_claimed`).

use crate::native_java_work::{
    CoverageClaim, DocumentJavaWork, JavaWorkAggregate, ModelInvocationCount, NativeJavaWorkReport,
    NotObservedReason, TapeVerificationState, TypedAbsence, load_corpus_from_manifest,
};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

// ───────────────────────────── identities ─────────────────────────────

pub const ROUTE_RECOVERY_SCHEMA_VERSION: &str = "native-route-recovery.v1";
pub const ROUTE_RECOVERY_ADR_REF: &str = "ADR-075";
pub const ROUTE_RECOVERY_ARTIFACT_REL: &str = "artifacts/parse_compat/native_route_recovery.json";
pub const COMPOSITION_DATA_REL: &str = "data/parse/native_fulltext_composition.yaml";
pub const COMPATIBILITY_DATA_REL: &str = "data/parse/grobid_compatibility.yaml";
pub const CORPUS_MANIFEST_REL: &str = "data/parse/layout_bakeoff.yaml";
pub const JAVA_WORK_RECEIPT_REL: &str = "artifacts/parse_compat/native_java_work.json";
pub const CORE_PROFILE_RECEIPT_REL: &str = "artifacts/parse_compat/native_core_profile.json";
pub const CORE_SMP_RECEIPT_REL: &str = "artifacts/parse_compat/native_core_smp.json";
pub const JAVA_WORK_RECEIPT_SCHEMA_VERSION: &str = "native-java-work.v1";
pub const CORE_PROFILE_SCHEMA_VERSION: &str = "native-core-profile.v1";
pub const CORE_SMP_SCHEMA_VERSION: &str = "native-core-smp.v1";
pub const ROUTE_RECOVERY_H05_CLOSURE: &str = "not_claimed";
pub const ROUTE_RECOVERY_H05_REASON: &str = "per-stage Java CPU wall-clock and RSS ceilings stay typed not_observed; only Java model invocations are measured and no byte, CPU or memory ceiling is claimed";
/// The T03 artifact emits a repo-relative path for the pinned manifest; the
/// receipt binds to the corpus, never to a host layout.
pub const RECOVERY_DECISION_POINT: usize = 6;
/// The stage skeleton the ledger is allowed to qualify. Equal to the key set of
/// `defaults.cascade_stages`; a superset or subset is a contract violation.
pub const CASCADE_STAGES: [&str; 10] = ["P0", "P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8", "P9"];
/// A stage with no native surface declares exactly this token.
pub const NO_NATIVE_SURFACE: &str = "none";
/// The component receipt must pin its source by digest.
pub const REQUIRED_SOURCE_HASH: &str = "required_sha256";
/// The component receipt may carry an artifact digest; when present it is bound.
pub const OPTIONAL_ARTIFACT_HASH: &str = "optional_sha256_when_present";
/// Closed stage verdicts. Must equal `qualification_vocabulary.stage_verdicts`.
pub const STAGE_VERDICT_VOCABULARY: [&str; 4] =
    ["recoverable", "not_qualified", "fallback", "not_observed"];
/// The four substitution conditions, in `admission.native_substitution_requires`
/// order. Must equal both that DATA list and `admission_conditions` keys.
pub const ADMISSION_CONDITION_NAMES: [&str; 4] = [
    "exact_source_identity",
    "exact_schema_version",
    "optional_sha256_when_present",
    "passing_receipt",
];
/// Closed guard order. Must equal `verdict_rule.order`.
pub const VERDICT_GUARD_ORDER: [&str; 8] = [
    "native_surface_absent",
    "no_admitted_component",
    "component_not_admitted",
    "admission_conditions_unmet",
    "core_qualification_missing",
    "java_work_not_observed",
    "no_attributed_java_work",
    "all_conditions_met",
];
/// The ledger's measured unit, restated from `measured_java_work.unit`.
pub const RECOVERY_JAVA_WORK_UNIT: &str = "java_model_invocations";
/// The claim a recovered-stage sum may carry. Never "a deployed reduction".
pub const RECOVERY_CLAIM: &str = "offline_attribution_only_never_a_deployed_reduction";
/// Longest accepted java stage / component / model token.
pub const RECOVERY_MAX_TOKEN_LEN: usize = 128;
/// Markers that must never appear in the emitted artifact. Payload text, model
/// bytes, host layouts and service URLs are all represented here.
pub const LEAK_MARKERS: [&str; 14] = [
    "<TEI",
    "<tei",
    "=== model:",
    "#mdl#",
    "#rdr#",
    ".wapiti",
    "http://",
    "https://",
    "127.0.0.1",
    "localhost",
    "/root",
    "/home",
    "/tmp",
    "SENTINEL-NOT-FOR-ARTIFACT",
];

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals. Every variant is a refusal, never a silent downgrade: the
/// ledger either produces an artifact it can justify or a reason why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRecoveryError {
    /// An input could not even be read or parsed.
    DataUnreadable { path: String, reason: String },
    /// A required field is absent. Loss is not silence.
    DataFieldMissing { path: String },
    /// A field is present but not the expected shape.
    DataFieldType { path: String, expected: String },
    /// The `route_recovery` block is absent from the composition authority.
    BlockMissing,
    /// A declared identity does not match the surface it mirrors.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A closed vocabulary drifted from the list it reuses.
    VocabularyDrift { field: String },
    /// A stage row weakened or reworded the recovery rule.
    RecoveryRuleDrift { stage: String, reason: String },
    /// A stage row names a stage the cascade does not declare.
    UnknownStage { stage: String },
    /// The cascade declares a stage no row qualifies.
    MissingStage { stage: String },
    /// The same stage is qualified twice.
    DuplicateStage { stage: String },
    /// A stage row names a component outside `component_ids`.
    UnknownComponent { stage: String, component: String },
    /// A stage owns the same component twice.
    DuplicateComponent { stage: String, component: String },
    /// A component has no admission receipt to read.
    MissingComponentReceipt { component: String },
    /// A stage attributes a model the verified receipt never observed.
    UnknownJavaModel { stage: String, model: String },
    /// The receipt observed a model no stage attributes.
    UnattributedJavaModel { model: String },
    /// Two stages attribute the same observed model.
    DoubleAttributedJavaModel { model: String, stages: String },
    /// A model name is not a safe token.
    UnsafeModelName { model: String },
    /// A named denominator drifted from the copy it restates.
    NamedGapDrift {
        gap: String,
        expected: String,
        found: String,
    },
    /// A measured count is not bound to the pinned corpus identity.
    CorpusBindingMismatch { document: String, reason: String },
    /// A receipt does not carry the identity this ledger requires.
    ReceiptSchemaDrift { path: String, reason: String },
    /// The emitted artifact leaked a payload marker.
    RedactionViolation { marker: String },
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
}

impl fmt::Display for RouteRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataUnreadable { path, reason } => {
                write!(f, "route recovery: `{path}` is unreadable: {reason}")
            }
            Self::DataFieldMissing { path } => {
                write!(f, "route recovery: required field `{path}` is missing")
            }
            Self::DataFieldType { path, expected } => {
                write!(f, "route recovery: field `{path}` must be {expected}")
            }
            Self::BlockMissing => write!(
                f,
                "route recovery: `{COMPOSITION_DATA_REL}#route_recovery` is missing; \
                 a ledger with no rule is not a ledger"
            ),
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "route recovery: `{field}` is `{found}` but `{expected}` is required"
            ),
            Self::VocabularyDrift { field } => write!(
                f,
                "route recovery: `{field}` drifted from the closed vocabulary it reuses"
            ),
            Self::RecoveryRuleDrift { stage, reason } => write!(
                f,
                "route recovery: stage `{stage}` recovery rule drifted: {reason}"
            ),
            Self::UnknownStage { stage } => write!(
                f,
                "route recovery: stage `{stage}` is not declared by `defaults.cascade_stages`"
            ),
            Self::MissingStage { stage } => write!(
                f,
                "route recovery: cascade stage `{stage}` has no row; a dropped stage is not silence"
            ),
            Self::DuplicateStage { stage } => {
                write!(f, "route recovery: stage `{stage}` is qualified twice")
            }
            Self::UnknownComponent { stage, component } => write!(
                f,
                "route recovery: stage `{stage}` owns unknown component `{component}`"
            ),
            Self::DuplicateComponent { stage, component } => write!(
                f,
                "route recovery: stage `{stage}` owns component `{component}` twice"
            ),
            Self::MissingComponentReceipt { component } => write!(
                f,
                "route recovery: component `{component}` has no admission receipt"
            ),
            Self::UnknownJavaModel { stage, model } => write!(
                f,
                "route recovery: stage `{stage}` attributes `{model}`, \
                 which the verified receipt never observed"
            ),
            Self::UnattributedJavaModel { model } => write!(
                f,
                "route recovery: the receipt observed `{model}` but no stage attributes it"
            ),
            Self::DoubleAttributedJavaModel { model, stages } => write!(
                f,
                "route recovery: `{model}` is attributed by more than one stage ({stages})"
            ),
            Self::UnsafeModelName { model } => {
                write!(f, "route recovery: `{model}` is not a safe model token")
            }
            Self::NamedGapDrift {
                gap,
                expected,
                found,
            } => write!(
                f,
                "route recovery: named gap `{gap}` is `{found}` but `{expected}` is required"
            ),
            Self::CorpusBindingMismatch { document, reason } => write!(
                f,
                "route recovery: corpus document `{document}` is not bound: {reason}"
            ),
            Self::ReceiptSchemaDrift { path, reason } => {
                write!(f, "route recovery: `{path}` is unusable: {reason}")
            }
            Self::RedactionViolation { marker } => write!(
                f,
                "route recovery: the artifact leaked `{marker}`; payload never leaves the tape"
            ),
            Self::ArtifactSerialization { reason } => {
                write!(f, "route recovery: artifact serialization failed: {reason}")
            }
        }
    }
}

impl std::error::Error for RouteRecoveryError {}

// ───────────────────────── closed vocabularies ─────────────────────────

/// Closed stage verdicts. Serialized as the strings DATA declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageVerdict {
    /// Every guard passed: the offline receipts qualify this stage's recovery.
    Recoverable,
    /// A structural guard failed (no surface, no component, unmet condition).
    NotQualified,
    /// The stage owns a component the matrix keeps on Java by design.
    Fallback,
    /// The stage would qualify, but the Java-work receipt carries no measurement.
    NotObserved,
}

impl StageVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recoverable => "recoverable",
            Self::NotQualified => "not_qualified",
            Self::Fallback => "fallback",
            Self::NotObserved => "not_observed",
        }
    }
}

/// The first guard that decided a stage verdict. Closed, and the only thing a
/// consumer may branch on for a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictReason {
    NativeSurfaceAbsent,
    NoAdmittedComponent,
    ComponentNotAdmitted,
    AdmissionConditionsUnmet,
    CoreQualificationMissing,
    JavaWorkNotObserved,
    NoAttributedJavaWork,
    AllConditionsMet,
}

impl VerdictReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeSurfaceAbsent => "native_surface_absent",
            Self::NoAdmittedComponent => "no_admitted_component",
            Self::ComponentNotAdmitted => "component_not_admitted",
            Self::AdmissionConditionsUnmet => "admission_conditions_unmet",
            Self::CoreQualificationMissing => "core_qualification_missing",
            Self::JavaWorkNotObserved => "java_work_not_observed",
            Self::NoAttributedJavaWork => "no_attributed_java_work",
            Self::AllConditionsMet => "all_conditions_met",
        }
    }
}

/// Tri-state evaluation of one substitution condition. `NotApplicable` is used
/// only when a stage owns no component, so there is no evidence surface to bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionState {
    Met,
    Unmet,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceState {
    Present,
    Absent,
    NoneDeclared,
}

/// Why a Java count is not published. Typed absence without a number, so no
/// consumer can read a zero that came from nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JavaWorkAbsence {
    /// The Java-work receipt carries no measurement at all.
    ReceiptNotObserved,
    /// The receipt is measured, but no measured model is attributed to this stage.
    NoAttributedModel,
}

/// A measured Java count, or a typed absence. There is no third state and no
/// default: an absent receipt cannot produce a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JavaClaim {
    Observed { value: usize },
    NotObserved { reason: JavaWorkAbsence },
}

impl JavaClaim {
    pub const fn value(&self) -> Option<usize> {
        match self {
            Self::Observed { value } => Some(*value),
            Self::NotObserved { .. } => None,
        }
    }

    pub const fn absence(reason: JavaWorkAbsence) -> Self {
        Self::NotObserved { reason }
    }
}

/// The four substitution conditions, evaluated per stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionConditions {
    pub exact_source_identity: ConditionState,
    pub exact_schema_version: ConditionState,
    pub optional_sha256_when_present: ConditionState,
    pub passing_receipt: ConditionState,
}

impl AdmissionConditions {
    const fn not_applicable() -> Self {
        Self {
            exact_source_identity: ConditionState::NotApplicable,
            exact_schema_version: ConditionState::NotApplicable,
            optional_sha256_when_present: ConditionState::NotApplicable,
            passing_receipt: ConditionState::NotApplicable,
        }
    }

    const fn all_met(&self) -> bool {
        matches!(self.exact_source_identity, ConditionState::Met)
            && matches!(self.exact_schema_version, ConditionState::Met)
            && matches!(self.optional_sha256_when_present, ConditionState::Met)
            && matches!(self.passing_receipt, ConditionState::Met)
    }

    /// The four conditions as named DATA owns them, paired with their state.
    pub fn rows(&self) -> [(&'static str, ConditionState); 4] {
        [
            ("exact_source_identity", self.exact_source_identity),
            ("exact_schema_version", self.exact_schema_version),
            (
                "optional_sha256_when_present",
                self.optional_sha256_when_present,
            ),
            ("passing_receipt", self.passing_receipt),
        ]
    }
}

/// The recovery rule restated on every stage row. Serialized so a reader sees
/// exactly what the row demanded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRule {
    pub requires_all_of: Vec<String>,
    pub requires_core_qualification: bool,
    pub requires_measured_java_work: bool,
    pub pooling: String,
}

// ───────────────────────── the measured payload ─────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributedModel {
    pub model: String,
    pub invocations: JavaClaim,
}

/// One owned component, resolved against the admission matrix and its receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentAdmission {
    pub component_id: String,
    pub matrix_disposition: String,
    pub receipt_disposition: String,
    pub receipt_evidence_state: String,
    pub receipt_schema_version: String,
}

/// The stage's per-component disposition mapped to what it forces, using the
/// DATA-owned `component_state_verdicts` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageRecovery {
    pub stage: String,
    pub java_stage: String,
    pub production_authority: String,
    pub native_surface: String,
    pub native_surface_state: SurfaceState,
    pub components: Vec<ComponentAdmission>,
    pub conditions: AdmissionConditions,
    pub recovery_rule: RecoveryRule,
    /// Models this stage attributes, ordered by name, each with its own claim.
    pub java_models_observed: Vec<AttributedModel>,
    pub attributed_java_work: JavaClaim,
    pub verdict: StageVerdict,
    pub verdict_reason: VerdictReason,
}

impl StageRecovery {
    pub fn component_ids(&self) -> Vec<&str> {
        self.components
            .iter()
            .map(|component| component.component_id.as_str())
            .collect()
    }
}

/// The corpus the Java counts were measured over, bound by digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusBinding {
    pub corpus_id: String,
    pub corpus_n: usize,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub documents: Vec<CorpusDocumentBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusDocumentBinding {
    pub document_id: String,
    pub source_sha256: String,
}

/// What the Java-work receipt says, or a typed reason it says nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JavaWorkInput {
    Measured {
        unit: String,
        corpus_documents: usize,
        documents_measured: usize,
        coverage: CoverageClaim,
        tape_verification: String,
        total_invocations: usize,
        models: Vec<ModelInvocationCount>,
    },
    NotObserved {
        reason: JavaWorkAbsence,
        /// The receipt's own absence reason, when it had one.
        receipt_reason: Option<NotObservedReason>,
    },
}

impl JavaWorkInput {
    pub const fn is_measured(&self) -> bool {
        matches!(self, Self::Measured { .. })
    }
}

/// The two native-core receipts this stage of the slice produced, as seen by
/// this ledger: presence and the closed dispositions they published. SIMD/PGO
/// were measured and REJECTED (S08/T01); SMP was qualified as bounded and
/// deterministic (S08/T02). Neither is adopted here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreQualification {
    pub profile_receipt_present: bool,
    pub profile_optimization_state: String,
    pub profile_dense_phase: String,
    pub profile_pgo_disposition: String,
    pub smp_receipt_present: bool,
    pub smp_matrix_entries: usize,
    pub smp_scalar_oracle_bound: bool,
    pub missing_receipts: Vec<String>,
}

impl CoreQualification {
    pub const fn complete(&self) -> bool {
        self.profile_receipt_present && self.smp_receipt_present
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceBinding {
    pub path: String,
    pub sha256: String,
}

/// The four conditions and the rule that governs them, echoed from DATA so the
/// artifact is readable without the DATA file open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictRule {
    pub order: Vec<String>,
    pub component_state_verdicts: BTreeMap<String, String>,
    pub admission_conditions: Vec<String>,
    pub recovery_rule_invariants: RecoveryRule,
    pub attribution_rule: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryLocks {
    pub production_route: String,
    pub deployed: bool,
    pub import_eligible: bool,
    pub auto_promote: String,
    pub graph_mutation: String,
    pub selection: String,
    pub decision_point: usize,
    pub independently_selectable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedGaps {
    pub citation_names: i64,
    pub references: i64,
    pub pooling: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryDenominators {
    pub corpus_id: String,
    pub corpus_documents: usize,
    pub corpus_documents_pinned_by_sha256: bool,
    pub unit: String,
    pub pooled: String,
    pub named_gaps: NamedGaps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRedaction {
    pub forbidden: Vec<String>,
}

/// The ceilings this ledger does NOT claim. Typed absences, so no consumer can
/// read a zero or a measured value out of an unmeasured resource, and H05 stays
/// visibly open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryObservations {
    pub java_cpu_wall_clock: TypedAbsence,
    pub rss_ceiling: TypedAbsence,
}

/// The measured Java work that recoverable stages account for, or a typed
/// absence. This is an attribution over offline receipts, never a deployed
/// reduction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RecoverableJavaWork {
    Observed {
        unit: String,
        claim: String,
        value: usize,
        by_stage: BTreeMap<String, usize>,
        corpus_id: String,
        receipt_sha256: String,
    },
    NotObserved {
        unit: String,
        claim: String,
        reason: JavaWorkAbsence,
    },
}

impl RecoverableJavaWork {
    pub const fn value(&self) -> Option<usize> {
        match self {
            Self::Observed { value, .. } => Some(*value),
            Self::NotObserved { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRecoveryLedger {
    pub schema_version: String,
    pub adr_ref: String,
    pub requirement_ids: Vec<String>,
    pub source_bindings: Vec<SourceBinding>,
    pub corpus: CorpusBinding,
    pub java_work: JavaWorkInput,
    pub core_qualification: CoreQualification,
    pub verdict_rule: VerdictRule,
    pub stages: Vec<StageRecovery>,
    pub recoverable_java_work: RecoverableJavaWork,
    pub denominators: RecoveryDenominators,
    pub locks: RecoveryLocks,
    pub named_limitations: Vec<String>,
    pub observations: RecoveryObservations,
    pub h05_closure: String,
    pub h05_reason: String,
    pub redaction: RecoveryRedaction,
}

impl RouteRecoveryLedger {
    pub fn stage(&self, stage: &str) -> Option<&StageRecovery> {
        self.stages.iter().find(|row| row.stage == stage)
    }

    pub fn recoverable_stages(&self) -> Vec<&StageRecovery> {
        self.stages
            .iter()
            .filter(|row| row.verdict == StageVerdict::Recoverable)
            .collect()
    }

    /// Serialize, then refuse to return bytes that leak a payload marker.
    ///
    /// The scan is part of emission rather than a test-only assertion, so a
    /// future field that starts carrying raw material fails here.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, RouteRecoveryError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            RouteRecoveryError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = String::from_utf8_lossy(&bytes);
        for marker in LEAK_MARKERS {
            if text.contains(marker) {
                return Err(RouteRecoveryError::RedactionViolation {
                    marker: marker.to_string(),
                });
            }
        }
        Ok(bytes)
    }
}

// ───────────────────────────── inputs ─────────────────────────────

/// Everything the ledger is assembled from, as text. Passing the inputs in
/// rather than reading paths keeps the ledger independent of a host layout and
/// lets a contract test drive it with a mutated DATA file.
#[derive(Debug, Clone, Copy)]
pub struct RouteRecoveryInputs<'a> {
    pub composition_yaml: &'a str,
    pub compatibility_yaml: &'a str,
    pub corpus_manifest_yaml: &'a str,
    pub java_work_json: &'a str,
    /// Empty when the receipt is absent; presence is then reported as missing
    /// and every recoverable stage is withheld, never silently qualified.
    pub core_profile_json: &'a str,
    pub core_smp_json: &'a str,
    /// Native surfaces verified to exist in the tree. A DATA path alone is a
    /// claim; the caller supplies the observation.
    pub present_surfaces: &'a BTreeSet<String>,
}

/// Read the canonical inputs for `root` and assemble the ledger.
pub fn load_from_root(root: &Path) -> Result<RouteRecoveryLedger, RouteRecoveryError> {
    let composition = read_text(root, COMPOSITION_DATA_REL)?;
    let compatibility = read_text(root, COMPATIBILITY_DATA_REL)?;
    let corpus_manifest = read_text(root, CORPUS_MANIFEST_REL)?;
    let java_work = read_text(root, JAVA_WORK_RECEIPT_REL)?;
    let profile = read_optional_text(root, CORE_PROFILE_RECEIPT_REL)?;
    let smp = read_optional_text(root, CORE_SMP_RECEIPT_REL)?;
    let declared = declared_native_surfaces(&compatibility)?;
    let present = observe_surfaces(root, &declared);
    assemble(&RouteRecoveryInputs {
        composition_yaml: &composition,
        compatibility_yaml: &compatibility,
        corpus_manifest_yaml: &corpus_manifest,
        java_work_json: &java_work,
        core_profile_json: &profile,
        core_smp_json: &smp,
        present_surfaces: &present,
    })
}

/// The native surfaces `defaults.cascade_stages` declares, excluding `none`.
pub fn declared_native_surfaces(
    compatibility_yaml: &str,
) -> Result<BTreeSet<String>, RouteRecoveryError> {
    let value = parse_yaml(compatibility_yaml, COMPATIBILITY_DATA_REL)?;
    let mut declared = BTreeSet::new();
    for (stage, row) in cascade_stages(&value)? {
        let surface = text_field(&row, &["native_surface"]).map_err(|error| {
            RouteRecoveryError::DataFieldMissing {
                path: format!("defaults.cascade_stages.{stage}.native_surface ({error})"),
            }
        })?;
        if surface != NO_NATIVE_SURFACE {
            declared.insert(surface);
        }
    }
    Ok(declared)
}

/// Which declared surfaces actually exist under `root`.
pub fn observe_surfaces(root: &Path, declared: &BTreeSet<String>) -> BTreeSet<String> {
    declared
        .iter()
        .filter(|path| root.join(path).is_file())
        .cloned()
        .collect()
}

fn read_text(root: &Path, relative: &str) -> Result<String, RouteRecoveryError> {
    std::fs::read_to_string(root.join(relative)).map_err(|error| {
        RouteRecoveryError::DataUnreadable {
            path: relative.to_string(),
            reason: error.to_string(),
        }
    })
}

/// A missing optional receipt is a typed absence, not an error: the ledger must
/// still be able to say "the core qualification receipt is missing".
fn read_optional_text(root: &Path, relative: &str) -> Result<String, RouteRecoveryError> {
    let path = root.join(relative);
    if !path.exists() {
        return Ok(String::new());
    }
    read_text(root, relative)
}

// ───────────────────────── DATA extraction ─────────────────────────

fn parse_yaml(text: &str, path: &str) -> Result<Value, RouteRecoveryError> {
    serde_yaml::from_str::<Value>(text).map_err(|error| RouteRecoveryError::DataUnreadable {
        path: path.to_string(),
        reason: error.to_string(),
    })
}

fn field<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, RouteRecoveryError> {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .ok_or_else(|| RouteRecoveryError::DataFieldMissing {
                path: path.join("."),
            })?;
    }
    Ok(current)
}

fn text_field(value: &Value, path: &[&str]) -> Result<String, RouteRecoveryError> {
    match field(value, path)? {
        Value::String(text) => Ok(text.clone()),
        other => Err(RouteRecoveryError::DataFieldType {
            path: path.join("."),
            expected: format!("a string, found {other:?}"),
        }),
    }
}

fn bool_field(value: &Value, path: &[&str]) -> Result<bool, RouteRecoveryError> {
    match field(value, path)? {
        Value::Bool(flag) => Ok(*flag),
        other => Err(RouteRecoveryError::DataFieldType {
            path: path.join("."),
            expected: format!("a bool, found {other:?}"),
        }),
    }
}

fn int_field(value: &Value, path: &[&str]) -> Result<i64, RouteRecoveryError> {
    match field(value, path)? {
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| RouteRecoveryError::DataFieldType {
                path: path.join("."),
                expected: "an integer".to_string(),
            }),
        other => Err(RouteRecoveryError::DataFieldType {
            path: path.join("."),
            expected: format!("an integer, found {other:?}"),
        }),
    }
}

fn text_seq(value: &Value, path: &[&str]) -> Result<Vec<String>, RouteRecoveryError> {
    let sequence =
        field(value, path)?
            .as_sequence()
            .ok_or_else(|| RouteRecoveryError::DataFieldType {
                path: path.join("."),
                expected: "a sequence of strings".to_string(),
            })?;
    sequence
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| RouteRecoveryError::DataFieldType {
                    path: path.join("."),
                    expected: "a sequence of strings".to_string(),
                })
        })
        .collect()
}

/// `defaults.cascade_stages`: stage -> row.
fn cascade_stages(value: &Value) -> Result<Vec<(String, Value)>, RouteRecoveryError> {
    let mapping = field(value, &["defaults", "cascade_stages"])?
        .as_mapping()
        .ok_or_else(|| RouteRecoveryError::DataFieldType {
            path: "defaults.cascade_stages".to_string(),
            expected: "a mapping of stage to row".to_string(),
        })?;
    let mut rows = Vec::with_capacity(mapping.len());
    for (key, row) in mapping {
        let stage = key
            .as_str()
            .ok_or_else(|| RouteRecoveryError::DataFieldType {
                path: "defaults.cascade_stages".to_string(),
                expected: "string stage keys".to_string(),
            })?
            .to_string();
        rows.push((stage, row.clone()));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    if rows.is_empty() {
        return Err(RouteRecoveryError::DataFieldMissing {
            path: "defaults.cascade_stages".to_string(),
        });
    }
    Ok(rows)
}

/// A component receipt row, reduced to the fields the ledger is allowed to read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ComponentReceiptRow {
    producer: String,
    disposition: String,
    evidence_state: String,
    schema_version: String,
    source_hash: String,
    artifact_sha256: String,
}

fn component_receipt(
    value: &Value,
    component: &str,
) -> Result<ComponentReceiptRow, RouteRecoveryError> {
    let row = field(value, &["component_receipts", component]).map_err(|_| {
        RouteRecoveryError::MissingComponentReceipt {
            component: component.to_string(),
        }
    })?;
    let read = |name: &str| -> Result<String, RouteRecoveryError> {
        text_field(row, &[name]).map_err(|_| RouteRecoveryError::MissingComponentReceipt {
            component: component.to_string(),
        })
    };
    Ok(ComponentReceiptRow {
        producer: read("producer")?,
        disposition: read("disposition")?,
        evidence_state: read("evidence_state")?,
        schema_version: read("schema_version")?,
        source_hash: read("source_hash")?,
        artifact_sha256: read("artifact_sha256")?,
    })
}

fn matrix_disposition(value: &Value, component: &str) -> Result<String, RouteRecoveryError> {
    text_field(value, &["admission", "current_matrix", component]).map_err(|_| {
        RouteRecoveryError::UnknownComponent {
            stage: "admission.current_matrix".to_string(),
            component: component.to_string(),
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationVocabulary {
    component_states: Vec<String>,
    stage_verdicts: Vec<String>,
    condition_states: Vec<String>,
    surface_states: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictRuleSpec {
    order: Vec<String>,
    reasons: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasuredJavaWorkSpec {
    unit: String,
    source: String,
    receipt: String,
    java_cpu_wall_clock: String,
    rss_ceiling: String,
    claim: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockDenominators {
    corpus_id: String,
    corpus_manifest: String,
    corpus_documents: usize,
    corpus_documents_pinned_by_sha256: bool,
    unit: String,
    pooled: String,
    named_gaps: NamedGaps,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttributionRule {
    each_observed_model_attributed_exactly_once: bool,
    unattributed_models: String,
    double_attributed_models: String,
    names_are_taken_from_the_verified_receipt: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageSpec {
    stage: String,
    java_stage: String,
    production_authority: String,
    native_surface: String,
    components: Vec<String>,
    java_models_observed: Vec<String>,
    recovery_rule: RecoveryRule,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockLocks {
    production_route: String,
    deployed: bool,
    import_eligible: bool,
    auto_promote: String,
    graph_mutation: String,
    selection: String,
    decision_point: usize,
    independently_selectable: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockRedaction {
    forbidden: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryBlock {
    schema_version: String,
    adr_ref: String,
    requirement_ids: Vec<String>,
    source_tables: Vec<String>,
    source_evidence: Vec<String>,
    qualification_vocabulary: QualificationVocabulary,
    component_state_verdicts: BTreeMap<String, String>,
    verdict_rule: VerdictRuleSpec,
    recovery_rule_invariants: RecoveryRule,
    admission_conditions: BTreeMap<String, String>,
    measured_java_work: MeasuredJavaWorkSpec,
    denominators: BlockDenominators,
    attribution_rule: AttributionRule,
    stages: Vec<StageSpec>,
    locks: BlockLocks,
    redaction: BlockRedaction,
    named_limitations: Vec<String>,
    h05_closure: String,
    h05_reason: String,
    used_by: Vec<String>,
}

// ───────────────────────────── assembly ─────────────────────────────

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A conservative token: short, and made of characters that cannot carry a path,
/// a hostname, a secret or a payload.
fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= RECOVERY_MAX_TOKEN_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

/// An exact schema token: a safe token that is not a wildcard placeholder.
fn is_exact_schema_token(value: &str) -> bool {
    is_safe_token(value) && !matches!(value, "*" | "any" | "optional" | "none" | "unknown")
}

fn context(stage: &str, reason: impl Into<String>) -> RouteRecoveryError {
    RouteRecoveryError::RecoveryRuleDrift {
        stage: stage.to_string(),
        reason: reason.into(),
    }
}

fn schema_drift(field: &str, expected: &str, found: &str) -> RouteRecoveryError {
    RouteRecoveryError::SchemaDrift {
        field: field.to_string(),
        expected: expected.to_string(),
        found: found.to_string(),
    }
}

/// Assemble the ledger, or refuse with the reason.
pub fn assemble(
    inputs: &RouteRecoveryInputs<'_>,
) -> Result<RouteRecoveryLedger, RouteRecoveryError> {
    let composition = parse_yaml(inputs.composition_yaml, COMPOSITION_DATA_REL)?;
    let block_value = composition
        .get("route_recovery")
        .ok_or(RouteRecoveryError::BlockMissing)?
        .clone();
    let block: RecoveryBlock = serde_yaml::from_value(block_value).map_err(|error| {
        RouteRecoveryError::DataUnreadable {
            path: format!("{COMPOSITION_DATA_REL}#route_recovery"),
            reason: error.to_string(),
        }
    })?;

    // Identity and the closed vocabularies it reuses.
    if block.schema_version != ROUTE_RECOVERY_SCHEMA_VERSION {
        return Err(schema_drift(
            "route_recovery.schema_version",
            ROUTE_RECOVERY_SCHEMA_VERSION,
            &block.schema_version,
        ));
    }
    if block.adr_ref != ROUTE_RECOVERY_ADR_REF {
        return Err(schema_drift(
            "route_recovery.adr_ref",
            ROUTE_RECOVERY_ADR_REF,
            &block.adr_ref,
        ));
    }
    let dispositions = text_seq(&composition, &["dispositions"])?;
    if block.qualification_vocabulary.component_states != dispositions {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "qualification_vocabulary.component_states".to_string(),
        });
    }
    if block.qualification_vocabulary.stage_verdicts
        != STAGE_VERDICT_VOCABULARY.map(str::to_string).to_vec()
    {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "qualification_vocabulary.stage_verdicts".to_string(),
        });
    }
    if block.qualification_vocabulary.condition_states
        != ["met", "unmet", "not_applicable"]
            .map(str::to_string)
            .to_vec()
    {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "qualification_vocabulary.condition_states".to_string(),
        });
    }
    if block.qualification_vocabulary.surface_states
        != ["present", "absent", "none_declared"]
            .map(str::to_string)
            .to_vec()
    {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "qualification_vocabulary.surface_states".to_string(),
        });
    }
    if !same_set(
        &block
            .component_state_verdicts
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        &dispositions,
    ) {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "component_state_verdicts".to_string(),
        });
    }
    for (state, forced) in &block.component_state_verdicts {
        if !matches!(forced.as_str(), "admitted" | "fallback" | "not_qualified") {
            return Err(RouteRecoveryError::VocabularyDrift {
                field: format!("component_state_verdicts.{state}"),
            });
        }
    }
    if block.verdict_rule.order != VERDICT_GUARD_ORDER.map(str::to_string).to_vec() {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "verdict_rule.order".to_string(),
        });
    }
    if !same_set(
        &block
            .verdict_rule
            .reasons
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        &block.verdict_rule.order,
    ) {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "verdict_rule.reasons".to_string(),
        });
    }
    let admission_keys = block
        .admission_conditions
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if !same_set(
        &admission_keys,
        ADMISSION_CONDITION_NAMES.map(str::to_string).as_ref(),
    ) {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "admission_conditions".to_string(),
        });
    }
    if block.measured_java_work.source != "verified_stage_tape"
        || block.measured_java_work.receipt != JAVA_WORK_RECEIPT_REL
    {
        return Err(schema_drift(
            "route_recovery.measured_java_work.receipt",
            JAVA_WORK_RECEIPT_REL,
            &block.measured_java_work.receipt,
        ));
    }
    if block
        .denominators
        .corpus_manifest
        .split('#')
        .next()
        .unwrap_or_default()
        != CORPUS_MANIFEST_REL
    {
        return Err(schema_drift(
            "route_recovery.denominators.corpus_manifest",
            CORPUS_MANIFEST_REL,
            &block.denominators.corpus_manifest,
        ));
    }
    if !block.used_by.iter().any(|task| task == "S08-T04") {
        return Err(schema_drift(
            "route_recovery.used_by",
            "S08-T04",
            "a consumer list without this ledger's owner",
        ));
    }
    // Every input the ledger reads must be declared by the block, and every
    // declaration must name an input the ledger actually consumes: an
    // undeclared read and a stale declaration are both drift.
    let declared_inputs: BTreeSet<String> = block
        .source_tables
        .iter()
        .chain(block.source_evidence.iter())
        .map(|entry| entry.split('#').next().unwrap_or(entry).to_string())
        .collect();
    for path in [
        COMPOSITION_DATA_REL,
        COMPATIBILITY_DATA_REL,
        CORPUS_MANIFEST_REL,
        JAVA_WORK_RECEIPT_REL,
        CORE_PROFILE_RECEIPT_REL,
        CORE_SMP_RECEIPT_REL,
    ] {
        if !declared_inputs.contains(path) {
            return Err(schema_drift(
                "route_recovery.source_tables/source_evidence",
                path,
                "an undeclared input",
            ));
        }
    }
    for declared in &declared_inputs {
        if ![
            COMPOSITION_DATA_REL,
            COMPATIBILITY_DATA_REL,
            CORPUS_MANIFEST_REL,
            JAVA_WORK_RECEIPT_REL,
            CORE_PROFILE_RECEIPT_REL,
            CORE_SMP_RECEIPT_REL,
        ]
        .contains(&declared.as_str())
        {
            return Err(schema_drift(
                "route_recovery.source_tables/source_evidence",
                "only consumed inputs",
                declared,
            ));
        }
    }
    if block.measured_java_work.unit != RECOVERY_JAVA_WORK_UNIT {
        return Err(schema_drift(
            "route_recovery.measured_java_work.unit",
            RECOVERY_JAVA_WORK_UNIT,
            &block.measured_java_work.unit,
        ));
    }
    if block.measured_java_work.java_cpu_wall_clock != "not_observed"
        || block.measured_java_work.rss_ceiling != "not_observed"
    {
        return Err(schema_drift(
            "route_recovery.measured_java_work ceilings",
            "not_observed",
            "a claimed observation",
        ));
    }
    if block.measured_java_work.claim != RECOVERY_CLAIM {
        return Err(schema_drift(
            "route_recovery.measured_java_work.claim",
            RECOVERY_CLAIM,
            &block.measured_java_work.claim,
        ));
    }
    if block.h05_closure != ROUTE_RECOVERY_H05_CLOSURE {
        return Err(schema_drift(
            "route_recovery.h05_closure",
            ROUTE_RECOVERY_H05_CLOSURE,
            &block.h05_closure,
        ));
    }

    // The four conditions and the rule as the composition authority states them.
    let admission_requires =
        text_seq(&composition, &["admission", "native_substitution_requires"])?;
    if admission_requires != ADMISSION_CONDITION_NAMES.map(str::to_string).to_vec() {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "admission.native_substitution_requires".to_string(),
        });
    }
    if block.recovery_rule_invariants.requires_all_of != admission_requires {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "recovery_rule_invariants.requires_all_of".to_string(),
        });
    }
    if !block.recovery_rule_invariants.requires_core_qualification
        || !block.recovery_rule_invariants.requires_measured_java_work
        || block.recovery_rule_invariants.pooling != "forbidden"
    {
        return Err(schema_drift(
            "recovery_rule_invariants",
            "core qualification + measured java work required, pooling forbidden",
            "a weakened rule",
        ));
    }
    let terminal_receipt_is_pass =
        bool_field(&composition, &["admission", "terminal_receipt_is_pass"])?;
    if terminal_receipt_is_pass {
        return Err(schema_drift(
            "admission.terminal_receipt_is_pass",
            "false",
            "true",
        ));
    }

    // Named gaps: restated, never pooled.
    let s12 = ["source_evidence", "s12_named_denominators"];
    for (gap, expected, found) in [
        (
            "named_gaps.citation_names",
            int_field(&composition, &[s12[0], s12[1], "citation_names_total"])?,
            block.denominators.named_gaps.citation_names,
        ),
        (
            "named_gaps.references",
            int_field(&composition, &[s12[0], s12[1], "references_total"])?,
            block.denominators.named_gaps.references,
        ),
    ] {
        if expected != found {
            return Err(RouteRecoveryError::NamedGapDrift {
                gap: gap.to_string(),
                expected: expected.to_string(),
                found: found.to_string(),
            });
        }
    }
    if block.denominators.named_gaps.pooling != "forbidden" {
        return Err(schema_drift(
            "route_recovery.denominators.named_gaps.pooling",
            "forbidden",
            &block.denominators.named_gaps.pooling,
        ));
    }
    if block.denominators.pooled != "forbidden" {
        return Err(schema_drift(
            "route_recovery.denominators.pooled",
            "forbidden",
            &block.denominators.pooled,
        ));
    }
    if !block.denominators.corpus_documents_pinned_by_sha256 {
        return Err(schema_drift(
            "route_recovery.denominators.corpus_documents_pinned_by_sha256",
            "true",
            "false",
        ));
    }

    // Locks: restated from every surface they mirror.
    if block.locks.production_route != "forbidden"
        || block.locks.deployed
        || block.locks.import_eligible
        || block.locks.auto_promote != "forbidden"
        || block.locks.graph_mutation != "forbidden"
        || block.locks.selection != "forbidden"
    {
        return Err(schema_drift(
            "route_recovery.locks",
            "production_route/graph_mutation/auto_promote/selection forbidden, deployed and import_eligible false",
            "a weakened lock",
        ));
    }
    if block.locks.decision_point != RECOVERY_DECISION_POINT {
        return Err(schema_drift(
            "route_recovery.locks.decision_point",
            &RECOVERY_DECISION_POINT.to_string(),
            &block.locks.decision_point.to_string(),
        ));
    }
    let independently_selectable =
        text_seq(&composition, &["admission", "independently_selectable"])?;
    if block.locks.independently_selectable != independently_selectable {
        return Err(schema_drift(
            "route_recovery.locks.independently_selectable",
            &independently_selectable.join(", "),
            &block.locks.independently_selectable.join(", "),
        ));
    }
    if text_field(&composition, &["production_route"])? != block.locks.production_route
        || text_field(&composition, &["auto_promote"])? != block.locks.auto_promote
        || text_field(&composition, &["graph_mutation"])? != block.locks.graph_mutation
        || bool_field(&composition, &["import_eligible"])? != block.locks.import_eligible
    {
        return Err(schema_drift(
            "route_recovery.locks vs composed authority locks",
            "identical",
            "drifted",
        ));
    }

    // Redaction: the block restates the file-level closed list.
    let file_forbidden = text_seq(&composition, &["redaction", "forbidden"])?;
    if block.redaction.forbidden != file_forbidden {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "route_recovery.redaction.forbidden".to_string(),
        });
    }

    // Attribution rule: closed and non-negotiable.
    if !block
        .attribution_rule
        .each_observed_model_attributed_exactly_once
        || block.attribution_rule.unattributed_models != "forbidden"
        || block.attribution_rule.double_attributed_models != "forbidden"
        || !block
            .attribution_rule
            .names_are_taken_from_the_verified_receipt
    {
        return Err(schema_drift(
            "route_recovery.attribution_rule",
            "exactly-once attribution from the verified receipt, pooling forbidden",
            "a weakened rule",
        ));
    }

    // The cascade skeleton every row must mirror.
    let mut cascade: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    for (stage, row) in cascade_stages(&parse_yaml(
        inputs.compatibility_yaml,
        COMPATIBILITY_DATA_REL,
    )?)? {
        cascade.insert(
            stage,
            (
                text_field(&row, &["grobid"])?,
                text_field(&row, &["production_authority"])?,
                text_field(&row, &["native_surface"])?,
            ),
        );
    }
    let cascade_keys: Vec<String> = cascade.keys().cloned().collect();
    if cascade_keys != CASCADE_STAGES.map(str::to_string).to_vec() {
        return Err(RouteRecoveryError::VocabularyDrift {
            field: "defaults.cascade_stages".to_string(),
        });
    }

    // Rows: unique, complete, and an exact mirror of the skeleton.
    let mut rows: BTreeMap<String, &StageSpec> = BTreeMap::new();
    for spec in &block.stages {
        if !cascade.contains_key(&spec.stage) {
            return Err(RouteRecoveryError::UnknownStage {
                stage: spec.stage.clone(),
            });
        }
        if rows.insert(spec.stage.clone(), spec).is_some() {
            return Err(RouteRecoveryError::DuplicateStage {
                stage: spec.stage.clone(),
            });
        }
    }
    for stage in &cascade_keys {
        if !rows.contains_key(stage) {
            return Err(RouteRecoveryError::MissingStage {
                stage: stage.clone(),
            });
        }
    }

    // The Java-work receipt, consumed through its own canonical type.
    let report: NativeJavaWorkReport =
        serde_json::from_str(inputs.java_work_json).map_err(|error| {
            RouteRecoveryError::DataUnreadable {
                path: JAVA_WORK_RECEIPT_REL.to_string(),
                reason: error.to_string(),
            }
        })?;
    if report.schema_version != JAVA_WORK_RECEIPT_SCHEMA_VERSION {
        return Err(RouteRecoveryError::ReceiptSchemaDrift {
            path: JAVA_WORK_RECEIPT_REL.to_string(),
            reason: format!("schema_version is `{}`", report.schema_version),
        });
    }
    if report.unit != block.measured_java_work.unit {
        return Err(RouteRecoveryError::ReceiptSchemaDrift {
            path: JAVA_WORK_RECEIPT_REL.to_string(),
            reason: format!(
                "unit is `{}`, not `{}`",
                report.unit, block.measured_java_work.unit
            ),
        });
    }
    if report.locks.production_route != "forbidden"
        || report.locks.auto_promote != "forbidden"
        || report.locks.selection != "forbidden"
        || report.locks.deployed
        || report.locks.import_eligible
    {
        return Err(RouteRecoveryError::ReceiptSchemaDrift {
            path: JAVA_WORK_RECEIPT_REL.to_string(),
            reason: "the Java-work receipt carries a weakened lock".to_string(),
        });
    }
    if report.corpus.id != block.denominators.corpus_id {
        return Err(RouteRecoveryError::CorpusBindingMismatch {
            document: report.corpus.id.clone(),
            reason: format!(
                "receipt corpus id does not match `{}`",
                block.denominators.corpus_id
            ),
        });
    }
    if report.corpus.corpus_n != block.denominators.corpus_documents {
        return Err(RouteRecoveryError::CorpusBindingMismatch {
            document: report.corpus.id.clone(),
            reason: format!(
                "receipt corpus_n is {} but the ledger pins {}",
                report.corpus.corpus_n, block.denominators.corpus_documents
            ),
        });
    }

    // Corpus identity: the manifest is the pin, the receipt must match it, and
    // every measured document must carry the pinned digest.
    let manifest = load_corpus_from_manifest(inputs.corpus_manifest_yaml).map_err(|error| {
        RouteRecoveryError::DataUnreadable {
            path: CORPUS_MANIFEST_REL.to_string(),
            reason: error.to_string(),
        }
    })?;
    if manifest.id != block.denominators.corpus_id
        || manifest.corpus_n != block.denominators.corpus_documents
    {
        return Err(RouteRecoveryError::CorpusBindingMismatch {
            document: manifest.id.clone(),
            reason: "the manifest corpus identity does not match the ledger denominators"
                .to_string(),
        });
    }
    let mut documents = Vec::with_capacity(manifest.documents.len());
    for document in &manifest.documents {
        if !is_sha256_hex(&document.source_sha256) {
            return Err(RouteRecoveryError::CorpusBindingMismatch {
                document: document.document_id.clone(),
                reason: "the manifest pins a malformed sha256".to_string(),
            });
        }
        let found = report
            .corpus
            .documents
            .iter()
            .find(|entry| entry.document_id == document.document_id)
            .ok_or_else(|| RouteRecoveryError::CorpusBindingMismatch {
                document: document.document_id.clone(),
                reason: "the receipt does not carry this document".to_string(),
            })?;
        if found.source_sha256 != document.source_sha256 {
            return Err(RouteRecoveryError::CorpusBindingMismatch {
                document: document.document_id.clone(),
                reason: "the receipt digest differs from the pinned manifest digest".to_string(),
            });
        }
        for entry in &report.documents {
            if entry.document_id() != document.document_id {
                continue;
            }
            if document_source_sha256(entry) != document.source_sha256 {
                return Err(RouteRecoveryError::CorpusBindingMismatch {
                    document: document.document_id.clone(),
                    reason: "a measured document is bound to another source digest".to_string(),
                });
            }
        }
        documents.push(CorpusDocumentBinding {
            document_id: document.document_id.clone(),
            source_sha256: document.source_sha256.clone(),
        });
    }
    documents.sort_by(|left, right| left.document_id.cmp(&right.document_id));

    // The observed model vocabulary, when the receipt carries measurements.
    let mut observed: BTreeMap<String, usize> = BTreeMap::new();
    let java_work = match &report.aggregate {
        JavaWorkAggregate::Measured(aggregate) => {
            for entry in &aggregate.models {
                if !is_safe_token(&entry.model) {
                    return Err(RouteRecoveryError::UnsafeModelName {
                        model: entry.model.clone(),
                    });
                }
                if observed
                    .insert(entry.model.clone(), entry.invocations)
                    .is_some()
                {
                    return Err(RouteRecoveryError::DoubleAttributedJavaModel {
                        model: entry.model.clone(),
                        stages: "twice inside one receipt".to_string(),
                    });
                }
            }
            if aggregate.documents_measured != block.denominators.corpus_documents
                || !matches!(aggregate.coverage, CoverageClaim::Full)
            {
                // A partial measurement cannot support a per-stage reduction
                // claim: it is a typed absence, not a smaller number.
                JavaWorkInput::NotObserved {
                    reason: JavaWorkAbsence::ReceiptNotObserved,
                    receipt_reason: Some(NotObservedReason::NoMeasuredDocument),
                }
            } else {
                JavaWorkInput::Measured {
                    unit: report.unit.clone(),
                    corpus_documents: report.corpus.corpus_n,
                    documents_measured: aggregate.documents_measured,
                    coverage: aggregate.coverage.clone(),
                    tape_verification: verification_summary(&report),
                    total_invocations: aggregate.total_invocations,
                    models: aggregate.models.clone(),
                }
            }
        }
        JavaWorkAggregate::NotObserved { reason } => JavaWorkInput::NotObserved {
            reason: JavaWorkAbsence::ReceiptNotObserved,
            receipt_reason: Some(*reason),
        },
    };
    let java_work_measured = java_work.is_measured();

    // Core qualification receipts: presence is a condition, not an identity.
    let core_qualification =
        read_core_qualification(inputs.core_profile_json, inputs.core_smp_json)?;

    // Attribution: exactly once, and only names the receipt observed.
    let mut attributed_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for spec in &block.stages {
        let mut seen = BTreeSet::new();
        for model in &spec.java_models_observed {
            if !is_safe_token(model) {
                return Err(RouteRecoveryError::UnsafeModelName {
                    model: model.clone(),
                });
            }
            if !seen.insert(model.clone()) {
                return Err(RouteRecoveryError::DoubleAttributedJavaModel {
                    model: model.clone(),
                    stages: spec.stage.clone(),
                });
            }
            if java_work_measured && !observed.contains_key(model) {
                return Err(RouteRecoveryError::UnknownJavaModel {
                    stage: spec.stage.clone(),
                    model: model.clone(),
                });
            }
            attributed_by
                .entry(model.clone())
                .or_default()
                .push(spec.stage.clone());
        }
    }
    for (model, stages) in &attributed_by {
        if stages.len() > 1 {
            return Err(RouteRecoveryError::DoubleAttributedJavaModel {
                model: model.clone(),
                stages: stages.join(", "),
            });
        }
    }
    if java_work_measured {
        for model in observed.keys() {
            if !attributed_by.contains_key(model) {
                return Err(RouteRecoveryError::UnattributedJavaModel {
                    model: model.clone(),
                });
            }
        }
    }

    // Per-stage recovery.
    let mut stages = Vec::with_capacity(block.stages.len());
    for (stage, spec) in &rows {
        if spec.recovery_rule != block.recovery_rule_invariants {
            return Err(context(
                stage,
                "the row does not restate `recovery_rule_invariants`",
            ));
        }
        let (java_stage, production_authority, native_surface) = cascade
            .get(stage)
            .expect("every row was checked against the cascade");
        if &spec.java_stage != java_stage {
            return Err(schema_drift(
                &format!("route_recovery.stages.{stage}.java_stage"),
                java_stage,
                &spec.java_stage,
            ));
        }
        if &spec.production_authority != production_authority {
            return Err(schema_drift(
                &format!("route_recovery.stages.{stage}.production_authority"),
                production_authority,
                &spec.production_authority,
            ));
        }
        if &spec.native_surface != native_surface {
            return Err(schema_drift(
                &format!("route_recovery.stages.{stage}.native_surface"),
                native_surface,
                &spec.native_surface,
            ));
        }

        let surface_state = if spec.native_surface == NO_NATIVE_SURFACE {
            SurfaceState::NoneDeclared
        } else if inputs.present_surfaces.contains(&spec.native_surface) {
            SurfaceState::Present
        } else {
            SurfaceState::Absent
        };

        let mut components = Vec::with_capacity(spec.components.len());
        let mut seen_components = BTreeSet::new();
        let mut receipts = Vec::new();
        for component in &spec.components {
            if !seen_components.insert(component.clone()) {
                return Err(RouteRecoveryError::DuplicateComponent {
                    stage: stage.clone(),
                    component: component.clone(),
                });
            }
            let disposition = matrix_disposition(&composition, component)?;
            let receipt = component_receipt(&composition, component)?;
            receipts.push(receipt.clone());
            components.push(ComponentAdmission {
                component_id: component.clone(),
                matrix_disposition: disposition,
                receipt_disposition: receipt.disposition,
                receipt_evidence_state: receipt.evidence_state,
                receipt_schema_version: receipt.schema_version,
            });
        }

        let conditions = evaluate_conditions(&receipts, terminal_receipt_is_pass);

        let mut models = Vec::with_capacity(spec.java_models_observed.len());
        for model in &spec.java_models_observed {
            let claim = match (&java_work, observed.get(model)) {
                (JavaWorkInput::Measured { .. }, Some(invocations)) => JavaClaim::Observed {
                    value: *invocations,
                },
                _ => JavaClaim::absence(if java_work_measured {
                    JavaWorkAbsence::NoAttributedModel
                } else {
                    JavaWorkAbsence::ReceiptNotObserved
                }),
            };
            models.push(AttributedModel {
                model: model.clone(),
                invocations: claim,
            });
        }
        models.sort_by(|left, right| left.model.cmp(&right.model));
        let attributed = sum_claims(&models, java_work_measured);

        let (verdict, verdict_reason) = decide(
            surface_state,
            &components,
            &block.component_state_verdicts,
            &conditions,
            core_qualification.complete(),
            java_work_measured,
            &attributed,
        );

        stages.push(StageRecovery {
            stage: stage.clone(),
            java_stage: java_stage.clone(),
            production_authority: production_authority.clone(),
            native_surface: spec.native_surface.clone(),
            native_surface_state: surface_state,
            components,
            conditions,
            recovery_rule: spec.recovery_rule.clone(),
            java_models_observed: models,
            attributed_java_work: attributed,
            verdict,
            verdict_reason,
        });
    }
    stages.sort_by(|left, right| left.stage.cmp(&right.stage));

    // The measured sum the recoverable stages account for.
    let mut by_stage = BTreeMap::new();
    let mut total = 0usize;
    for row in &stages {
        if row.verdict != StageVerdict::Recoverable {
            continue;
        }
        let value = row.attributed_java_work.value().ok_or_else(|| {
            context(
                &row.stage,
                "a recoverable stage must carry an observed attribution",
            )
        })?;
        by_stage.insert(row.stage.clone(), value);
        total += value;
    }
    let receipt_sha256 = sha256_hex(inputs.java_work_json);
    let recoverable_java_work = if java_work_measured {
        RecoverableJavaWork::Observed {
            unit: block.measured_java_work.unit.clone(),
            claim: block.measured_java_work.claim.clone(),
            value: total,
            by_stage,
            corpus_id: block.denominators.corpus_id.clone(),
            receipt_sha256,
        }
    } else {
        RecoverableJavaWork::NotObserved {
            unit: block.measured_java_work.unit.clone(),
            claim: block.measured_java_work.claim.clone(),
            reason: JavaWorkAbsence::ReceiptNotObserved,
        }
    };

    // Source bindings: every input the ledger read, by digest.
    let mut source_bindings = vec![
        binding(COMPOSITION_DATA_REL, inputs.composition_yaml),
        binding(COMPATIBILITY_DATA_REL, inputs.compatibility_yaml),
        binding(CORPUS_MANIFEST_REL, inputs.corpus_manifest_yaml),
        binding(JAVA_WORK_RECEIPT_REL, inputs.java_work_json),
    ];
    if core_qualification.profile_receipt_present {
        source_bindings.push(binding(CORE_PROFILE_RECEIPT_REL, inputs.core_profile_json));
    }
    if core_qualification.smp_receipt_present {
        source_bindings.push(binding(CORE_SMP_RECEIPT_REL, inputs.core_smp_json));
    }
    source_bindings.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(RouteRecoveryLedger {
        schema_version: block.schema_version.clone(),
        adr_ref: block.adr_ref.clone(),
        requirement_ids: block.requirement_ids.clone(),
        source_bindings,
        corpus: CorpusBinding {
            corpus_id: manifest.id.clone(),
            corpus_n: manifest.corpus_n,
            manifest_path: CORPUS_MANIFEST_REL.to_string(),
            manifest_sha256: sha256_hex(inputs.corpus_manifest_yaml),
            documents,
        },
        java_work,
        core_qualification,
        verdict_rule: VerdictRule {
            order: block.verdict_rule.order.clone(),
            component_state_verdicts: block.component_state_verdicts.clone(),
            admission_conditions: admission_keys,
            recovery_rule_invariants: block.recovery_rule_invariants.clone(),
            attribution_rule: BTreeMap::from([
                (
                    "each_observed_model_attributed_exactly_once".to_string(),
                    block
                        .attribution_rule
                        .each_observed_model_attributed_exactly_once
                        .to_string(),
                ),
                (
                    "unattributed_models".to_string(),
                    block.attribution_rule.unattributed_models.clone(),
                ),
                (
                    "double_attributed_models".to_string(),
                    block.attribution_rule.double_attributed_models.clone(),
                ),
                (
                    "names_are_taken_from_the_verified_receipt".to_string(),
                    block
                        .attribution_rule
                        .names_are_taken_from_the_verified_receipt
                        .to_string(),
                ),
            ]),
        },
        stages,
        recoverable_java_work,
        denominators: RecoveryDenominators {
            corpus_id: block.denominators.corpus_id.clone(),
            corpus_documents: block.denominators.corpus_documents,
            corpus_documents_pinned_by_sha256: block.denominators.corpus_documents_pinned_by_sha256,
            unit: block.denominators.unit.clone(),
            pooled: block.denominators.pooled.clone(),
            named_gaps: block.denominators.named_gaps.clone(),
        },
        locks: RecoveryLocks {
            production_route: block.locks.production_route.clone(),
            deployed: block.locks.deployed,
            import_eligible: block.locks.import_eligible,
            auto_promote: block.locks.auto_promote.clone(),
            graph_mutation: block.locks.graph_mutation.clone(),
            selection: block.locks.selection.clone(),
            decision_point: block.locks.decision_point,
            independently_selectable: block.locks.independently_selectable.clone(),
        },
        named_limitations: block.named_limitations.clone(),
        observations: RecoveryObservations {
            java_cpu_wall_clock: TypedAbsence::not_observed(),
            rss_ceiling: TypedAbsence::not_observed(),
        },
        h05_closure: block.h05_closure.clone(),
        h05_reason: block.h05_reason.clone(),
        redaction: RecoveryRedaction {
            forbidden: block.redaction.forbidden.clone(),
        },
    })
}

/// Set equality, so a sorted map key set can be compared with a DATA-ordered
/// list without either side being accidentally reordered.
fn same_set(left: &[String], right: &[String]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort();
    right.sort();
    left == right
}

fn binding(path: &str, text: &str) -> SourceBinding {
    SourceBinding {
        path: path.to_string(),
        sha256: sha256_hex(text),
    }
}

/// How many measured documents were covered by an actual verification run.
/// The source digest a per-document receipt is bound to, whichever arm it is in.
fn document_source_sha256(document: &DocumentJavaWork) -> &str {
    match document {
        DocumentJavaWork::Receipt { source_sha256, .. }
        | DocumentJavaWork::Refused { source_sha256, .. }
        | DocumentJavaWork::NotObserved { source_sha256, .. } => source_sha256,
    }
}

fn verification_summary(report: &NativeJavaWorkReport) -> String {
    let verified = report
        .documents
        .iter()
        .filter(|document| match document {
            DocumentJavaWork::Receipt { receipt, .. } => receipt
                .measured()
                .map(|counts| matches!(counts.tape_verification, TapeVerificationState::Verified))
                .unwrap_or(false),
            _ => false,
        })
        .count();
    format!("{verified}/{}", report.documents.len())
}

/// Sum per-model claims. Mixed or all-absent claims produce a typed absence:
/// there is no numeric total to misread.
fn sum_claims(models: &[AttributedModel], java_work_measured: bool) -> JavaClaim {
    let mut total = 0usize;
    for model in models {
        match model.invocations.value() {
            Some(value) => total += value,
            None => {
                return JavaClaim::absence(if java_work_measured {
                    JavaWorkAbsence::NoAttributedModel
                } else {
                    JavaWorkAbsence::ReceiptNotObserved
                });
            }
        }
    }
    if models.is_empty() {
        return JavaClaim::absence(if java_work_measured {
            JavaWorkAbsence::NoAttributedModel
        } else {
            JavaWorkAbsence::ReceiptNotObserved
        });
    }
    JavaClaim::Observed { value: total }
}

/// Evaluate the four substitution conditions over the owned component receipts.
fn evaluate_conditions(
    rows: &[ComponentReceiptRow],
    terminal_receipt_is_pass: bool,
) -> AdmissionConditions {
    if rows.is_empty() {
        return AdmissionConditions::not_applicable();
    }
    let all = |predicate: &dyn Fn(&ComponentReceiptRow) -> bool| {
        if rows.iter().all(predicate) {
            ConditionState::Met
        } else {
            ConditionState::Unmet
        }
    };
    AdmissionConditions {
        exact_source_identity: all(&|row| row.source_hash == REQUIRED_SOURCE_HASH),
        exact_schema_version: all(&|row| is_exact_schema_token(&row.schema_version)),
        optional_sha256_when_present: all(&|row| row.artifact_sha256 == OPTIONAL_ARTIFACT_HASH),
        passing_receipt: all(&|row| {
            row.disposition == "passed"
                && row.evidence_state == "passing"
                && !terminal_receipt_is_pass
                && !row.producer.is_empty()
        }),
    }
}

/// The fail-closed verdict. The guard order is DATA's `verdict_rule.order`; this
/// function is asserted against it by the module unit test.
fn decide(
    surface: SurfaceState,
    components: &[ComponentAdmission],
    component_state_verdicts: &BTreeMap<String, String>,
    conditions: &AdmissionConditions,
    core_qualification_complete: bool,
    java_work_measured: bool,
    attributed: &JavaClaim,
) -> (StageVerdict, VerdictReason) {
    if surface != SurfaceState::Present {
        return (
            StageVerdict::NotQualified,
            VerdictReason::NativeSurfaceAbsent,
        );
    }
    if components.is_empty() {
        return (
            StageVerdict::NotQualified,
            VerdictReason::NoAdmittedComponent,
        );
    }
    if components
        .iter()
        .any(|component| component.matrix_disposition != "passed")
    {
        let forced = components
            .iter()
            .map(|component| {
                component_state_verdicts
                    .get(&component.matrix_disposition)
                    .cloned()
                    .unwrap_or_else(|| "not_qualified".to_string())
            })
            .fold("admitted".to_string(), |worst, next| {
                if worst == "not_qualified" || next == "not_qualified" {
                    "not_qualified".to_string()
                } else if next == "fallback" {
                    "fallback".to_string()
                } else {
                    worst
                }
            });
        let verdict = if forced == "fallback" {
            StageVerdict::Fallback
        } else {
            StageVerdict::NotQualified
        };
        return (verdict, VerdictReason::ComponentNotAdmitted);
    }
    if !conditions.all_met() {
        return (
            StageVerdict::NotQualified,
            VerdictReason::AdmissionConditionsUnmet,
        );
    }
    if !core_qualification_complete {
        return (
            StageVerdict::NotQualified,
            VerdictReason::CoreQualificationMissing,
        );
    }
    if !java_work_measured {
        return (
            StageVerdict::NotObserved,
            VerdictReason::JavaWorkNotObserved,
        );
    }
    match attributed.value() {
        Some(0) | None => (
            StageVerdict::NotQualified,
            VerdictReason::NoAttributedJavaWork,
        ),
        Some(_) => (StageVerdict::Recoverable, VerdictReason::AllConditionsMet),
    }
}

/// Presence and closed dispositions of the two native-core receipts. A missing
/// receipt is reported, never inferred.
fn read_core_qualification(
    profile_json: &str,
    smp_json: &str,
) -> Result<CoreQualification, RouteRecoveryError> {
    let mut missing = Vec::new();
    let (
        profile_receipt_present,
        profile_optimization_state,
        profile_dense_phase,
        profile_pgo_disposition,
    ) = if profile_json.trim().is_empty() {
        missing.push(CORE_PROFILE_RECEIPT_REL.to_string());
        (
            false,
            "not_observed".to_string(),
            "not_observed".to_string(),
            "not_observed".to_string(),
        )
    } else {
        let value: Value = serde_json::from_str(profile_json).map_err(|error| {
            RouteRecoveryError::DataUnreadable {
                path: CORE_PROFILE_RECEIPT_REL.to_string(),
                reason: error.to_string(),
            }
        })?;
        let schema = text_field(&value, &["schema_version"])?;
        if schema != CORE_PROFILE_SCHEMA_VERSION {
            return Err(RouteRecoveryError::ReceiptSchemaDrift {
                path: CORE_PROFILE_RECEIPT_REL.to_string(),
                reason: format!("schema_version is `{schema}`"),
            });
        }
        (
            true,
            text_field(&value, &["disposition", "optimization_state"])?,
            text_field(&value, &["disposition", "dense_phase"])?,
            text_field(&value, &["disposition", "pgo_disposition"])?,
        )
    };
    let (smp_receipt_present, smp_matrix_entries, smp_scalar_oracle_bound) =
        if smp_json.trim().is_empty() {
            missing.push(CORE_SMP_RECEIPT_REL.to_string());
            (false, 0usize, false)
        } else {
            let value: Value = serde_json::from_str(smp_json).map_err(|error| {
                RouteRecoveryError::DataUnreadable {
                    path: CORE_SMP_RECEIPT_REL.to_string(),
                    reason: error.to_string(),
                }
            })?;
            let schema = text_field(&value, &["schema_version"])?;
            if schema != CORE_SMP_SCHEMA_VERSION {
                return Err(RouteRecoveryError::ReceiptSchemaDrift {
                    path: CORE_SMP_RECEIPT_REL.to_string(),
                    reason: format!("schema_version is `{schema}`"),
                });
            }
            let entries = field(&value, &["matrix", "entries"])?
                .as_sequence()
                .map(Vec::len)
                .unwrap_or(0);
            let oracle =
                text_field(&value, &["matrix", "scalar_oracle_digest"])?.starts_with("sha256:");
            (true, entries, oracle)
        };
    Ok(CoreQualification {
        profile_receipt_present,
        profile_optimization_state,
        profile_dense_phase,
        profile_pgo_disposition,
        smp_receipt_present,
        smp_matrix_entries,
        smp_scalar_oracle_bound,
        missing_receipts: missing,
    })
}

// ───────────────────────────── tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
    const COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
    const CORPUS: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
    const RECEIPT: &str = include_str!("../../../artifacts/parse_compat/native_java_work.json");
    const CORE_PROFILE: &str =
        include_str!("../../../artifacts/parse_compat/native_core_profile.json");
    const CORE_SMP: &str = include_str!("../../../artifacts/parse_compat/native_core_smp.json");

    fn contract_surfaces() -> BTreeSet<String> {
        declared_native_surfaces(COMPATIBILITY).expect("declared surfaces")
    }

    fn ledger_with(
        composition: &str,
        java_work: &str,
        profile: &str,
        smp: &str,
        surfaces: &BTreeSet<String>,
    ) -> Result<RouteRecoveryLedger, RouteRecoveryError> {
        assemble(&RouteRecoveryInputs {
            composition_yaml: composition,
            compatibility_yaml: COMPATIBILITY,
            corpus_manifest_yaml: CORPUS,
            java_work_json: java_work,
            core_profile_json: profile,
            core_smp_json: smp,
            present_surfaces: surfaces,
        })
    }

    fn ledger() -> RouteRecoveryLedger {
        ledger_with(
            COMPOSITION,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect("the committed DATA and receipts must assemble")
    }

    #[test]
    fn the_t03_receipt_round_trips_through_its_own_canonical_type() {
        let report: NativeJavaWorkReport =
            serde_json::from_str(RECEIPT).expect("the committed receipt deserializes");
        assert_eq!(report.schema_version, JAVA_WORK_RECEIPT_SCHEMA_VERSION);
        assert_eq!(report.unit, RECOVERY_JAVA_WORK_UNIT);
        match report.aggregate {
            JavaWorkAggregate::Measured(aggregate) => {
                assert_eq!(
                    aggregate
                        .models
                        .iter()
                        .map(|m| m.invocations)
                        .sum::<usize>(),
                    570
                );
                assert_eq!(aggregate.total_invocations, 570);
                assert!(matches!(aggregate.coverage, CoverageClaim::Full));
            }
            JavaWorkAggregate::NotObserved { .. } => panic!("the committed receipt is measured"),
        }
    }

    #[test]
    fn every_cascade_stage_gets_a_verdict_and_only_the_admitted_lanes_recover() {
        let ledger = ledger();
        assert_eq!(ledger.stages.len(), CASCADE_STAGES.len());
        assert_eq!(ledger.stages[0].stage, "P0");
        assert_eq!(ledger.stages[9].stage, "P9");

        let recoverable: Vec<&str> = ledger
            .recoverable_stages()
            .iter()
            .map(|row| row.stage.as_str())
            .collect();
        assert_eq!(recoverable, ["P3", "P4"]);

        assert_eq!(
            ledger.stage("P2").expect("P2").verdict,
            StageVerdict::Fallback
        );
        assert_eq!(
            ledger.stage("P6").expect("P6").verdict,
            StageVerdict::Fallback
        );
        for stage in ["P0", "P1", "P5", "P7", "P8", "P9"] {
            assert_eq!(
                ledger.stage(stage).expect(stage).verdict,
                StageVerdict::NotQualified,
                "{stage} must stay not_qualified"
            );
        }
        assert_eq!(
            ledger.stage("P5").expect("P5").verdict_reason,
            VerdictReason::NoAdmittedComponent
        );
        assert_eq!(
            ledger.stage("P1").expect("P1").verdict_reason,
            VerdictReason::NoAdmittedComponent
        );
        assert_eq!(
            ledger.stage("P0").expect("P0").verdict_reason,
            VerdictReason::NativeSurfaceAbsent
        );
    }

    #[test]
    fn the_measured_sum_conserves_the_receipt_and_is_bound_to_it() {
        let ledger = ledger();
        let observed: usize = ledger
            .stages
            .iter()
            .filter_map(|row| row.attributed_java_work.value())
            .sum();
        assert_eq!(
            observed, 570,
            "attribution must conserve the measured total"
        );
        let recoverable = ledger
            .recoverable_java_work
            .value()
            .expect("measured receipt yields a number");
        assert_eq!(recoverable, 399);
        match &ledger.recoverable_java_work {
            RecoverableJavaWork::Observed {
                by_stage,
                receipt_sha256,
                corpus_id,
                claim,
                ..
            } => {
                assert_eq!(by_stage.get("P3"), Some(&181));
                assert_eq!(by_stage.get("P4"), Some(&218));
                assert_eq!(receipt_sha256, &sha256_hex(RECEIPT));
                assert_eq!(corpus_id, "s13-same-input-pdf-corpus");
                assert_eq!(claim, RECOVERY_CLAIM);
            }
            RecoverableJavaWork::NotObserved { .. } => panic!("expected a measured total"),
        }
        assert_eq!(
            ledger.stage("P3").expect("P3").attributed_java_work.value(),
            Some(181)
        );
        assert_eq!(
            ledger.stage("P4").expect("P4").attributed_java_work.value(),
            Some(218)
        );
    }

    #[test]
    fn an_unobserved_java_receipt_never_yields_recoverable_or_a_zero() {
        // The receipt is replaced wholesale so the shape stays valid while
        // every count disappears.
        let synthetic = "{\"schema_version\":\"native-java-work.v1\",\"adr_ref\":\"ADR-075\",\"unit\":\"java_model_invocations\",\"corpus\":{\"id\":\"s13-same-input-pdf-corpus\",\"corpus_n\":3,\"page_denominator\":202,\"documents\":[{\"id\":\"hub-2507.19457\",\"arxiv_id\":\"2507.19457\",\"source_pdf\":\"data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf\",\"sha256\":\"ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec\"},{\"id\":\"rlm-2512.24601\",\"arxiv_id\":\"2512.24601\",\"source_pdf\":\"data/sources/arxiv/cs-ai/2512.24601/source/original.pdf\",\"sha256\":\"8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca\"},{\"id\":\"reca-2605.26525v1\",\"arxiv_id\":\"2605.26525\",\"source_pdf\":\"data/sources/arxiv/cs-cv/2605.26525v1/source/2605.26525v1.pdf\",\"sha256\":\"a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8\"}]},\"documents\":[],\"aggregate\":{\"measurement_state\":\"not_observed\",\"reason\":\"no_measured_document\"},\"unmatched_tapes\":[],\"unreadable_tape_count\":0,\"verification_requested\":false,\"verification_gaps\":0,\"capture_hint\":\"\",\"observations\":{\"java_cpu_wall_clock\":{\"state\":\"not_observed\"},\"rss_ceiling\":{\"state\":\"not_observed\"}},\"locks\":{\"deployed\":false,\"import_eligible\":false,\"production_route\":\"forbidden\",\"auto_promote\":\"forbidden\",\"selection\":\"forbidden\"},\"h05_closure\":\"not_claimed\",\"h05_reason\":\"\",\"redaction\":{\"forbidden\":[]}}".to_string();
        let ledger = ledger_with(
            COMPOSITION,
            &synthetic,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect("an unobserved receipt still assembles");
        assert!(!ledger.java_work.is_measured());
        assert!(ledger.recoverable_stages().is_empty());
        assert!(matches!(
            ledger.recoverable_java_work,
            RecoverableJavaWork::NotObserved { .. }
        ));
        assert_eq!(
            ledger.stage("P3").expect("P3").verdict,
            StageVerdict::NotObserved
        );
        assert_eq!(
            ledger.stage("P3").expect("P3").verdict_reason,
            VerdictReason::JavaWorkNotObserved
        );
        assert_eq!(
            ledger.stage("P3").expect("P3").attributed_java_work.value(),
            None
        );
        let json =
            String::from_utf8(ledger.artifact_json_bytes().expect("artifact")).expect("utf8");
        assert!(
            !json.contains("\"value\": 0"),
            "absence must not read as zero"
        );
        assert!(!json.contains("\"state\": \"observed\""));
    }

    #[test]
    fn removing_any_one_admission_condition_makes_the_stage_not_recoverable() {
        for (from, to) in [
            (
                "source_hash: required_sha256",
                "source_hash: optional_sha256_when_present",
            ),
            (
                "schema_version: native-composition.outcome/v1",
                "schema_version: any",
            ),
            (
                "artifact_sha256: optional_sha256_when_present",
                "artifact_sha256: required_sha256",
            ),
            ("disposition: passed", "disposition: fallback"),
        ] {
            let mutated = COMPOSITION.replacen(from, to, 1);
            assert_ne!(mutated, COMPOSITION, "mutation `{from}` did not apply");
            let ledger = ledger_with(
                &mutated,
                RECEIPT,
                CORE_PROFILE,
                CORE_SMP,
                &contract_surfaces(),
            )
            .expect("a weakened condition still assembles, but refuses");
            let row = ledger.stage("P3").expect("P3");
            assert_ne!(
                row.verdict,
                StageVerdict::Recoverable,
                "mutation `{from}` must withdraw the recovery verdict"
            );
            assert!(
                matches!(
                    row.verdict_reason,
                    VerdictReason::AdmissionConditionsUnmet | VerdictReason::ComponentNotAdmitted
                ),
                "mutation `{from}` produced an unexpected reason {:?}",
                row.verdict_reason
            );
            assert_ne!(ledger.recoverable_java_work.value(), Some(399));
        }
    }

    #[test]
    fn a_missing_core_qualification_receipt_withholds_recovery() {
        let ledger = ledger_with(COMPOSITION, RECEIPT, "", "", &contract_surfaces())
            .expect("missing core receipts assemble");
        assert!(!ledger.core_qualification.complete());
        assert_eq!(ledger.core_qualification.missing_receipts.len(), 2);
        assert!(ledger.recoverable_stages().is_empty());
        assert_eq!(
            ledger.stage("P3").expect("P3").verdict_reason,
            VerdictReason::CoreQualificationMissing
        );
    }

    #[test]
    fn an_unknown_stage_model_or_dropped_stage_is_refused() {
        // An unknown stage row.
        let unknown_stage = COMPOSITION.replacen("  - stage: P9", "  - stage: P10", 1);
        let error = ledger_with(
            &unknown_stage,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("an invented stage must be refused");
        assert!(
            matches!(error, RouteRecoveryError::UnknownStage { .. }),
            "{error}"
        );

        // A stage the cascade declares but no row qualifies.
        let dropped = COMPOSITION.replacen(
            "  - stage: P5\n    java_stage: fulltext_body_sections\n    production_authority: java_http\n    native_surface: crates/da-parse/src/native_body.rs\n    components: []\n    java_models_observed: [fulltext]\n    recovery_rule: {requires_all_of: [exact_source_identity, exact_schema_version, optional_sha256_when_present, passing_receipt], requires_core_qualification: true, requires_measured_java_work: true, pooling: forbidden}\n",
            "",
            1,
        );
        assert_ne!(dropped, COMPOSITION, "the mutation must land");
        let error = ledger_with(
            &dropped,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("a dropped stage must be refused");
        assert!(
            matches!(&error, RouteRecoveryError::MissingStage { stage } if stage == "P5"),
            "{error}"
        );

        // A model the receipt never observed.
        let unknown_model = COMPOSITION.replacen(
            "java_models_observed: [fulltext]",
            "java_models_observed: [telepathy]",
            1,
        );
        let error = ledger_with(
            &unknown_model,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("an invented model must be refused");
        assert!(
            matches!(error, RouteRecoveryError::UnknownJavaModel { .. }),
            "{error}"
        );

        // A measured model no stage attributes.
        let unattributed = COMPOSITION.replacen(
            "java_models_observed: [fulltext]",
            "java_models_observed: []",
            1,
        );
        let error = ledger_with(
            &unattributed,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("an unattributed observed model must be refused");
        assert!(
            matches!(error, RouteRecoveryError::UnattributedJavaModel { .. }),
            "{error}"
        );

        // The same model attributed twice.
        let doubled = COMPOSITION.replacen(
            "java_models_observed: [fulltext]",
            "java_models_observed: [fulltext, citation]",
            1,
        );
        let error = ledger_with(
            &doubled,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("a double attribution must be refused");
        assert!(
            matches!(error, RouteRecoveryError::DoubleAttributedJavaModel { .. }),
            "{error}"
        );
    }

    #[test]
    fn pooling_the_named_gaps_or_weakening_the_rule_is_refused() {
        let pooled = COMPOSITION.replacen(
            "    named_gaps:\n      citation_names: 2198\n      references: 207\n      pooling: forbidden",
            "    named_gaps:\n      citation_names: 2405\n      references: 207\n      pooling: forbidden",
            1,
        );
        let error = ledger_with(
            &pooled,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("a pooled denominator must be refused");
        assert!(
            matches!(error, RouteRecoveryError::NamedGapDrift { .. }),
            "{error}"
        );

        let collapsed = COMPOSITION.replacen(
            "    named_gaps:\n      citation_names: 2198\n      references: 207\n      pooling: forbidden",
            "    named_gaps:\n      citation_names: 2198\n      references: 207\n      citation_names_and_references: 2405\n      pooling: forbidden",
            1,
        );
        assert!(
            ledger_with(
                &collapsed,
                RECEIPT,
                CORE_PROFILE,
                CORE_SMP,
                &contract_surfaces()
            )
            .is_err(),
            "an extra pooled key must be refused"
        );

        let weakened_rule = COMPOSITION.replacen(
            "    requires_all_of: [exact_source_identity, exact_schema_version, optional_sha256_when_present, passing_receipt]\n    requires_core_qualification: true\n    requires_measured_java_work: true\n    pooling: forbidden\n",
            "    requires_all_of: [exact_source_identity, exact_schema_version, optional_sha256_when_present]\n    requires_core_qualification: true\n    requires_measured_java_work: true\n    pooling: forbidden\n",
            1,
        );
        assert_ne!(weakened_rule, COMPOSITION, "the mutation must land");
        let error = ledger_with(
            &weakened_rule,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("a weakened invariant must be refused");
        assert!(
            matches!(error, RouteRecoveryError::VocabularyDrift { .. }),
            "{error}"
        );
    }

    #[test]
    fn the_artifact_is_byte_reproducible_and_leaks_nothing() {
        let first = ledger().artifact_json_bytes().expect("artifact");
        let second = ledger().artifact_json_bytes().expect("artifact");
        assert_eq!(first, second, "two assemblies must be byte-identical");
        let json = String::from_utf8(first).expect("utf8");
        for marker in LEAK_MARKERS {
            assert!(!json.contains(marker), "artifact leaked `{marker}`");
        }
        assert!(json.contains("\"h05_closure\": \"not_claimed\""));
        assert!(json.contains("\"production_route\": \"forbidden\""));
        assert!(json.contains("\"decision_point\": 6"));
        assert!(json.contains("\"independently_selectable\""));
    }

    #[test]
    fn declared_surfaces_are_observed_from_the_real_tree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let declared = declared_native_surfaces(COMPATIBILITY).expect("declared");
        assert_eq!(
            declared.len(),
            7,
            "P0 and P7 declare no surface; P3 and P4 share one"
        );
        assert!(!declared.contains(NO_NATIVE_SURFACE));
        let present = observe_surfaces(&root, &declared);
        assert_eq!(
            present, declared,
            "every declared native surface must exist in the tree"
        );
        assert!(declared.contains("crates/da-parse/src/native_composition.rs"));
    }

    #[test]
    fn an_absent_surface_is_a_typed_absent_state_not_a_present_one() {
        let empty = BTreeSet::new();
        let ledger = ledger_with(COMPOSITION, RECEIPT, CORE_PROFILE, CORE_SMP, &empty)
            .expect("an unobserved surface set still assembles");
        let row = ledger.stage("P3").expect("P3");
        assert_eq!(row.native_surface_state, SurfaceState::Absent);
        assert_eq!(row.verdict, StageVerdict::NotQualified);
        assert_eq!(row.verdict_reason, VerdictReason::NativeSurfaceAbsent);
        assert!(ledger.recoverable_stages().is_empty());
    }

    #[test]
    fn the_guard_order_in_data_is_the_guard_order_in_code() {
        assert_eq!(VERDICT_GUARD_ORDER.len(), 8);
        assert_eq!(
            VerdictReason::NativeSurfaceAbsent.as_str(),
            VERDICT_GUARD_ORDER[0]
        );
        assert_eq!(
            VerdictReason::NoAdmittedComponent.as_str(),
            VERDICT_GUARD_ORDER[1]
        );
        assert_eq!(
            VerdictReason::ComponentNotAdmitted.as_str(),
            VERDICT_GUARD_ORDER[2]
        );
        assert_eq!(
            VerdictReason::AdmissionConditionsUnmet.as_str(),
            VERDICT_GUARD_ORDER[3]
        );
        assert_eq!(
            VerdictReason::CoreQualificationMissing.as_str(),
            VERDICT_GUARD_ORDER[4]
        );
        assert_eq!(
            VerdictReason::JavaWorkNotObserved.as_str(),
            VERDICT_GUARD_ORDER[5]
        );
        assert_eq!(
            VerdictReason::NoAttributedJavaWork.as_str(),
            VERDICT_GUARD_ORDER[6]
        );
        assert_eq!(
            VerdictReason::AllConditionsMet.as_str(),
            VERDICT_GUARD_ORDER[7]
        );
        assert_eq!(
            StageVerdict::Recoverable.as_str(),
            STAGE_VERDICT_VOCABULARY[0]
        );
    }

    #[test]
    fn a_schema_or_declared_input_drift_is_refused() {
        // A mutation harness that silently no-ops would make every other
        // mutation test vacuous, so the replacement is asserted to have landed.
        let drifted = COMPOSITION.replacen(
            "  schema_version: native-route-recovery.v1",
            "  schema_version: native-route-recovery.v2",
            1,
        );
        assert_ne!(drifted, COMPOSITION, "the mutation must land");
        let error = ledger_with(
            &drifted,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("a schema drift must be refused");
        assert!(
            matches!(error, RouteRecoveryError::SchemaDrift { .. }),
            "{error}"
        );

        // An input consumed but not declared by the block.
        let undeclared =
            COMPOSITION.replacen("  - artifacts/parse_compat/native_core_smp.json\n", "", 1);
        assert_ne!(undeclared, COMPOSITION, "the mutation must land");
        let error = ledger_with(
            &undeclared,
            RECEIPT,
            CORE_PROFILE,
            CORE_SMP,
            &contract_surfaces(),
        )
        .expect_err("an undeclared input must be refused");
        assert!(
            matches!(error, RouteRecoveryError::SchemaDrift { .. }),
            "{error}"
        );
    }
}
