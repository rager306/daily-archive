//! Native scalar CRF core profile and the closed SIMD/PGO disposition.
//!
//! S03 recorded `da_domain::scalar_crf::map_input` as an *unmeasured* hot spot:
//! every call rebuilds a `BTreeMap` over the whole observation table and a
//! `Vec<Vec<&str>>` view of every input row, while `da-domain` was declared
//! "not to be optimized without separate proof". This module is that proof
//! surface. It measures the native CRF core on frozen synthetic Wapiti models,
//! derives `dense_numeric_kernel_found` from a DATA threshold, and turns the
//! measurement into a *closed* disposition through
//! [`crate::layout_runtime::gate_simd_candidate`]. Nothing here adopts,
//! selects, promotes, or splices an optimization: `da-domain` is untouched,
//! every optimizable route stays `selection: forbidden`, and PGO stays
//! `not_adopted`.
//!
//! ## Measurement contract
//!
//! Four disjoint phases are timed inside one pass over one family:
//!
//! 1. `render_feature_strings` — the rows are rendered into the bounded
//!    private [`ScratchArena`], which is where the payload `map_input` consumes
//!    is materialised;
//! 2. `observation_index_build` — the observation index `map_input` rebuilds on
//!    every call, measured on the same observation table with the same shape;
//! 3. `map_input_lookup` — the real `map_input` call, its internal rebuild
//!    included (phase 2 is an attribution of that rebuild, not extra pipeline
//!    work: the dense-kernel rule deliberately considers only phase 4);
//! 4. `viterbi_decode` — the real `viterbi_decode` call.
//!
//! Phase *shares* are ratios over the sum of the four. They attribute cost and
//! never enter an acceptance denominator.
//!
//! Timing is observational. [`NativeCoreProfileReceipt`] carries it under
//! `observations`, and it is deliberately absent from
//! [`NativeCoreProfileArtifact`] — the bytes written to
//! `artifacts/parse_compat/native_core_profile.json` — so that two independent
//! profiles of the same frozen models are byte-identical.
//!
//! Allocation deltas come from an *injected* snapshot closure. This module
//! never installs a global allocator; the counting allocator lives in the
//! example binary only. When two consecutive passes disagree the receipt
//! carries a typed unstable state instead of numbers.
//!
//! H05 is not closed here: RSS ceilings, per-stage Java CPU wall-clock, and
//! byte ceilings stay typed `not_observed` (see `h05_closure` in DATA).

use crate::layout_runtime::{
    CharacterizationError, CpuKernelCharacterization, KernelPercentiles, OptimizationState,
    ScratchArena, ScratchError, characterize_cpu_kernel, gate_simd_candidate,
};
use da_domain::scalar_crf::{CrfModel, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::time::Instant;

pub const NATIVE_CORE_PROFILE_SCHEMA_VERSION: &str = "native-core-profile.v1";
pub const NATIVE_CORE_QUALIFICATION_SCHEMA_VERSION: &str = "native-core-qualification.v1";
pub const NATIVE_CORE_QUALIFICATION_KEY: &str = "native_core_qualification";
pub const NATIVE_CORE_ADR_REF: &str = "ADR-075";
pub const NATIVE_CORE_MANIFEST_BINDING: &str = "data/parse/comparison_contract.yaml";
pub const NATIVE_CORE_PROFILE_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_core_profile.json";
pub const NATIVE_CORE_ROLLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";
/// Timing exists in the receipt but is excluded from the emitted artifact.
pub const NATIVE_CORE_TIMING_DISPOSITION: &str = "observational_excluded_from_artifact";

pub const PHASE_RENDER_FEATURE_STRINGS: &str = "render_feature_strings";
pub const PHASE_OBSERVATION_INDEX_BUILD: &str = "observation_index_build";
pub const PHASE_MAP_INPUT_LOOKUP: &str = "map_input_lookup";
pub const PHASE_VITERBI_DECODE: &str = "viterbi_decode";

/// Closed phase vocabulary. A phase outside this list is rejected, not ignored.
pub const NATIVE_CORE_PHASES: [&str; 4] = [
    PHASE_RENDER_FEATURE_STRINGS,
    PHASE_OBSERVATION_INDEX_BUILD,
    PHASE_MAP_INPUT_LOOKUP,
    PHASE_VITERBI_DECODE,
];

pub const WORK_KIND_TEXT: &str = "text";
pub const WORK_KIND_NUMERIC: &str = "numeric";
pub const STORAGE_CONTIGUOUS_NUMERIC_ROWS: &str = "contiguous_numeric_rows";
pub const OUTCOME_NOT_APPLICABLE: &str = "not_applicable";
pub const PGO_NOT_ADOPTED: &str = "not_adopted";
pub const H05_NOT_CLAIMED: &str = "not_claimed";

const REQUIRED_OPTIMIZATION_STATES: [&str; 3] =
    ["not_applicable", "measured_rejected", "measured_retained"];
const REQUIRED_PGO_VOCABULARY: [&str; 2] = ["not_adopted", "adopted"];
const REQUIRED_PGO_ADOPTION_REQUIRES: [&str; 4] = [
    "profile",
    "scalar_equivalence",
    "full_pipeline_p95_non_regression",
    "data_thresholds",
];
const REQUIRED_REDACTION_FORBIDDEN: [&str; 8] = [
    "raw_pdf",
    "raw_text",
    "raw_tei",
    "token_text",
    "model_bytes",
    "lexicon_bytes",
    "physical_path",
    "hostname",
];
const REQUIRED_REQUIREMENT_IDS: [&str; 3] = ["R109", "R085", "R117"];

const KERNEL_INVARIANT: &str = "the dry run executed map_input/viterbi_decode on this exact \
model and row set; both are pure, so the measured passes cannot fail";

// ───────────────────────────── errors ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCoreProfileError {
    /// The manifest text could not be read or parsed as a YAML mapping.
    DataUnreadable { reason: String },
    /// The `native_core_qualification` block is present but violates its contract.
    DataContractViolation { path: String, reason: String },
    /// A frozen synthetic model did not parse.
    ModelParse { family: String, reason: String },
    /// The workload never reached the CRF kernel (no matched observations).
    DegenerateWorkload {
        family: String,
        matched_observations: usize,
    },
    /// A non-positive measurement option.
    NonPositive { name: String },
    /// No family was supplied, so there is nothing to profile.
    EmptyFamilySet,
    /// The qualified primitive refused the workload.
    Characterization { family: String, reason: String },
    /// A retention was produced although this slice measures no candidate.
    RetentionNotAuthorized { family: String },
    /// Two independent passes produced different artifact bytes.
    ArtifactReplayDiverged { first: String, second: String },
}

impl fmt::Display for NativeCoreProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataUnreadable { reason } => {
                write!(f, "native core qualification data unreadable: {reason}")
            }
            Self::DataContractViolation { path, reason } => {
                write!(
                    f,
                    "native core qualification contract violation at {path}: {reason}"
                )
            }
            Self::ModelParse { family, reason } => {
                write!(
                    f,
                    "frozen native model for family {family} did not parse: {reason}"
                )
            }
            Self::DegenerateWorkload {
                family,
                matched_observations,
            } => write!(
                f,
                "native core workload for family {family} matched {matched_observations} \
observations; a profile that never reaches the kernel is not evidence"
            ),
            Self::NonPositive { name } => write!(f, "measurement option {name} must be positive"),
            Self::EmptyFamilySet => write!(f, "no native core family was supplied to profile"),
            Self::Characterization { family, reason } => {
                write!(
                    f,
                    "native core characterization failed for family {family}: {reason}"
                )
            }
            Self::RetentionNotAuthorized { family } => write!(
                f,
                "family {family} produced measured_retained; this slice measures no candidate \
and must never adopt one"
            ),
            Self::ArtifactReplayDiverged { first, second } => write!(
                f,
                "two independent native core profiles diverged: {first} != {second}"
            ),
        }
    }
}

impl std::error::Error for NativeCoreProfileError {}

// ─────────────────────── DATA-owned qualification block ───────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreQualificationData {
    pub schema_version: String,
    pub adr_ref: String,
    pub requirement_ids: Vec<String>,
    pub manifest_binding: String,
    pub profile: NativeCoreProfileConfig,
    pub dense_numeric_kernel_rule: DenseNumericKernelRule,
    pub optimization: NativeCoreOptimizationPolicy,
    pub pgo: NativeCorePgoPolicy,
    pub redaction: NativeCoreRedactionPolicy,
    pub promotion: NativeCorePromotionPolicy,
    pub h05_closure: String,
    pub h05_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreProfileConfig {
    pub warmup_iterations: usize,
    pub repetitions: usize,
    pub percentiles: NativeCorePercentileConfig,
    pub throughput_units_per_second: String,
    pub allocator: NativeCoreAllocatorConfig,
    pub private_scratch: NativeCoreScratchConfig,
    pub scalar_oracle: String,
    pub selection_forbidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCorePercentileConfig {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreAllocatorConfig {
    pub allocations: String,
    pub allocated_bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreScratchConfig {
    pub capacity_bytes: String,
    pub high_water_bytes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DenseNumericPhaseKind {
    pub work_kind: String,
    pub storage: String,
    pub contiguous_numeric_arrays: bool,
    #[serde(default)]
    pub sparse_weight_reads: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DenseNumericKernelRule {
    pub rule_id: String,
    pub share_metric: String,
    pub threshold: f64,
    pub requires_contiguous_numeric_arrays: bool,
    pub eligible_phases: Vec<String>,
    pub outcome_when_unmet: String,
    pub phase_kinds: BTreeMap<String, DenseNumericPhaseKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreRetainedThresholds {
    pub minimum_dense_phase_share: f64,
    pub minimum_repeated_cpu_improvement_ratio: f64,
    pub maximum_full_pipeline_p95_regression_ratio: f64,
    pub requires_scalar_equivalence: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreOptimizationPolicy {
    pub states: Vec<String>,
    pub retained_requires_data_thresholds: bool,
    pub retained_thresholds: NativeCoreRetainedThresholds,
    pub selection: String,
    pub safe_rust_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCorePgoPolicy {
    pub disposition_vocabulary: Vec<String>,
    pub adoption_requires: Vec<String>,
    pub disposition: String,
    pub selection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreRedactionPolicy {
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCorePromotionPolicy {
    pub state: String,
    pub selected: String,
    pub promoted: String,
    pub auto_promote: String,
    pub rollback_target: String,
}

/// Parse the `native_core_qualification` sibling block out of the manifest text.
///
/// The manifest is passed in as text so the reader depends on no filesystem
/// path and no wall-clock state; the emitter and the contract test both feed it
/// the git-tracked contract.
pub fn load_qualification_from_manifest(
    manifest_yaml: &str,
) -> Result<NativeCoreQualificationData, NativeCoreProfileError> {
    let manifest: serde_yaml::Value = serde_yaml::from_str(manifest_yaml).map_err(|error| {
        NativeCoreProfileError::DataUnreadable {
            reason: error.to_string(),
        }
    })?;
    let block = manifest.get(NATIVE_CORE_QUALIFICATION_KEY).ok_or_else(|| {
        NativeCoreProfileError::DataContractViolation {
            path: NATIVE_CORE_QUALIFICATION_KEY.to_string(),
            reason: "block missing from the manifest; loss is not silence".to_string(),
        }
    })?;
    let data: NativeCoreQualificationData =
        serde_yaml::from_value(block.clone()).map_err(|error| {
            NativeCoreProfileError::DataContractViolation {
                path: NATIVE_CORE_QUALIFICATION_KEY.to_string(),
                reason: error.to_string(),
            }
        })?;
    data.validate()?;
    Ok(data)
}

impl NativeCoreQualificationData {
    /// Fail loudly on every invariant this slice relies on. Extra DATA is
    /// allowed; a missing or weakened invariant is not.
    pub fn validate(&self) -> Result<(), NativeCoreProfileError> {
        let violation =
            |path: &str, reason: String| NativeCoreProfileError::DataContractViolation {
                path: path.to_string(),
                reason,
            };
        if self.schema_version != NATIVE_CORE_QUALIFICATION_SCHEMA_VERSION {
            return Err(violation(
                "native_core_qualification.schema_version",
                format!(
                    "expected {NATIVE_CORE_QUALIFICATION_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }
        if self.adr_ref != NATIVE_CORE_ADR_REF {
            return Err(violation(
                "native_core_qualification.adr_ref",
                format!("expected {NATIVE_CORE_ADR_REF}, got {}", self.adr_ref),
            ));
        }
        if self.manifest_binding != NATIVE_CORE_MANIFEST_BINDING {
            return Err(violation(
                "native_core_qualification.manifest_binding",
                format!(
                    "expected {NATIVE_CORE_MANIFEST_BINDING}, got {}",
                    self.manifest_binding
                ),
            ));
        }
        for required in REQUIRED_REQUIREMENT_IDS {
            if !self.requirement_ids.iter().any(|id| id == required) {
                return Err(violation(
                    "native_core_qualification.requirement_ids",
                    format!("required requirement {required} is missing"),
                ));
            }
        }
        if self.profile.repetitions < 2 {
            return Err(violation(
                "native_core_qualification.profile.repetitions",
                format!(
                    "a replayable scalar oracle needs at least 2 repetitions, got {}",
                    self.profile.repetitions
                ),
            ));
        }
        if self.profile.scalar_oracle != "required" {
            return Err(violation(
                "native_core_qualification.profile.scalar_oracle",
                format!(
                    "the scalar oracle is mandatory, got {}",
                    self.profile.scalar_oracle
                ),
            ));
        }
        if !self.profile.selection_forbidden {
            return Err(violation(
                "native_core_qualification.profile.selection_forbidden",
                "selection must stay forbidden".to_string(),
            ));
        }
        if self.profile.throughput_units_per_second.is_empty() {
            return Err(violation(
                "native_core_qualification.profile.throughput_units_per_second",
                "throughput unit must be named".to_string(),
            ));
        }
        if self.profile.private_scratch.capacity_bytes.is_empty()
            || self.profile.private_scratch.high_water_bytes.is_empty()
        {
            return Err(violation(
                "native_core_qualification.profile.private_scratch",
                "private scratch units must be named".to_string(),
            ));
        }
        if self.profile.allocator.allocations.is_empty()
            || self.profile.allocator.allocated_bytes.is_empty()
        {
            return Err(violation(
                "native_core_qualification.profile.allocator",
                "allocator units must be named".to_string(),
            ));
        }

        let rule = &self.dense_numeric_kernel_rule;
        let rule_path = "native_core_qualification.dense_numeric_kernel_rule";
        if rule.rule_id.is_empty() || rule.share_metric.is_empty() {
            return Err(violation(
                rule_path,
                "rule id and share metric must be named".to_string(),
            ));
        }
        if rule.threshold <= 0.0 || rule.threshold > 1.0 {
            return Err(violation(
                &format!("{rule_path}.threshold"),
                format!("threshold must be in (0,1], got {}", rule.threshold),
            ));
        }
        if rule.eligible_phases.is_empty() {
            return Err(violation(
                &format!("{rule_path}.eligible_phases"),
                "an empty eligible set can never produce a decision".to_string(),
            ));
        }
        if rule.outcome_when_unmet != OUTCOME_NOT_APPLICABLE {
            return Err(violation(
                &format!("{rule_path}.outcome_when_unmet"),
                format!(
                    "expected {OUTCOME_NOT_APPLICABLE}, got {}",
                    rule.outcome_when_unmet
                ),
            ));
        }
        for phase in NATIVE_CORE_PHASES {
            if !rule.phase_kinds.contains_key(phase) {
                return Err(violation(
                    &format!("{rule_path}.phase_kinds"),
                    format!("closed phase vocabulary is missing `{phase}`"),
                ));
            }
        }
        for phase in rule.phase_kinds.keys() {
            if !NATIVE_CORE_PHASES.contains(&phase.as_str()) {
                return Err(violation(
                    &format!("{rule_path}.phase_kinds"),
                    format!("invented phase `{phase}` is outside the closed vocabulary"),
                ));
            }
        }
        for phase in &rule.eligible_phases {
            let Some(kind) = rule.phase_kinds.get(phase) else {
                return Err(violation(
                    &format!("{rule_path}.eligible_phases"),
                    format!("eligible phase `{phase}` is not classified in phase_kinds"),
                ));
            };
            if kind.work_kind != WORK_KIND_NUMERIC {
                return Err(violation(
                    &format!("{rule_path}.eligible_phases"),
                    format!("eligible phase `{phase}` is not numeric work"),
                ));
            }
            if kind.contiguous_numeric_arrays && kind.storage != STORAGE_CONTIGUOUS_NUMERIC_ROWS {
                return Err(violation(
                    &format!("{rule_path}.phase_kinds.{phase}"),
                    format!(
                        "contiguous numeric arrays require storage {STORAGE_CONTIGUOUS_NUMERIC_ROWS}, got {}",
                        kind.storage
                    ),
                ));
            }
        }

        let optimization = &self.optimization;
        let optimization_path = "native_core_qualification.optimization";
        for state in REQUIRED_OPTIMIZATION_STATES {
            if !optimization.states.iter().any(|declared| declared == state) {
                return Err(violation(
                    &format!("{optimization_path}.states"),
                    format!("closed disposition vocabulary is missing `{state}`"),
                ));
            }
        }
        if optimization.states.len() != REQUIRED_OPTIMIZATION_STATES.len() {
            return Err(violation(
                &format!("{optimization_path}.states"),
                "the disposition vocabulary is closed; an extra state is a new capability"
                    .to_string(),
            ));
        }
        if !optimization.retained_requires_data_thresholds {
            return Err(violation(
                &format!("{optimization_path}.retained_requires_data_thresholds"),
                "retention must require DATA thresholds".to_string(),
            ));
        }
        if optimization.retained_thresholds.minimum_dense_phase_share <= 0.0
            || optimization
                .retained_thresholds
                .minimum_repeated_cpu_improvement_ratio
                <= 0.0
            || !optimization.retained_thresholds.requires_scalar_equivalence
        {
            return Err(violation(
                &format!("{optimization_path}.retained_thresholds"),
                "retention thresholds must stay positive and require scalar equivalence"
                    .to_string(),
            ));
        }
        if optimization.selection != "forbidden" || !optimization.safe_rust_only {
            return Err(violation(
                optimization_path,
                "selection must stay forbidden and the route safe-Rust only".to_string(),
            ));
        }

        let pgo = &self.pgo;
        let pgo_path = "native_core_qualification.pgo";
        if pgo.disposition_vocabulary.len() != REQUIRED_PGO_VOCABULARY.len()
            || !REQUIRED_PGO_VOCABULARY
                .iter()
                .all(|state| pgo.disposition_vocabulary.iter().any(|d| d == state))
        {
            return Err(violation(
                &format!("{pgo_path}.disposition_vocabulary"),
                "the PGO disposition vocabulary is closed".to_string(),
            ));
        }
        if pgo.adoption_requires.len() != REQUIRED_PGO_ADOPTION_REQUIRES.len()
            || !REQUIRED_PGO_ADOPTION_REQUIRES
                .iter()
                .all(|state| pgo.adoption_requires.iter().any(|d| d == state))
        {
            return Err(violation(
                &format!("{pgo_path}.adoption_requires"),
                "PGO adoption requires profile, scalar equivalence, non-regression and DATA thresholds"
                    .to_string(),
            ));
        }
        if pgo.disposition != PGO_NOT_ADOPTED || pgo.selection != "forbidden" {
            return Err(violation(
                pgo_path,
                format!(
                    "PGO must stay {PGO_NOT_ADOPTED} with selection forbidden, got {} / {}",
                    pgo.disposition, pgo.selection
                ),
            ));
        }

        for forbidden in REQUIRED_REDACTION_FORBIDDEN {
            if !self
                .redaction
                .forbidden
                .iter()
                .any(|value| value == forbidden)
            {
                return Err(violation(
                    "native_core_qualification.redaction.forbidden",
                    format!("redaction vocabulary is missing `{forbidden}`"),
                ));
            }
        }

        let promotion = &self.promotion;
        let promotion_path = "native_core_qualification.promotion";
        if promotion.state != "candidate"
            || promotion.selected != "forbidden"
            || promotion.promoted != "forbidden"
            || promotion.auto_promote != "forbidden"
            || promotion.rollback_target != NATIVE_CORE_ROLLBACK_TARGET
        {
            return Err(violation(
                promotion_path,
                "promotion locks or the rollback target drifted".to_string(),
            ));
        }

        if self.h05_closure != H05_NOT_CLAIMED {
            return Err(violation(
                "native_core_qualification.h05_closure",
                format!("H05 is not closed by this slice, got {}", self.h05_closure),
            ));
        }
        if self.h05_reason.trim().is_empty() {
            return Err(violation(
                "native_core_qualification.h05_reason",
                "an unclaimed ceiling needs a stated reason".to_string(),
            ));
        }
        Ok(())
    }
}

// ─────────────────────────── decisions ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseNumericKernelDecision {
    pub found: bool,
    pub phase: Option<String>,
    pub reason_key: String,
}

/// Evaluate the DATA rule: an eligible phase must be numeric work over declared
/// contiguous storage *and* reach the DATA threshold share.
pub fn evaluate_dense_numeric_kernel(
    rule: &DenseNumericKernelRule,
    shares: &BTreeMap<String, f64>,
) -> DenseNumericKernelDecision {
    let mut eligible_declared = false;
    let mut above: Option<(String, f64)> = None;
    for phase in &rule.eligible_phases {
        let Some(kind) = rule.phase_kinds.get(phase) else {
            continue;
        };
        if kind.work_kind != WORK_KIND_NUMERIC {
            continue;
        }
        if rule.requires_contiguous_numeric_arrays && !kind.contiguous_numeric_arrays {
            continue;
        }
        eligible_declared = true;
        let share = shares.get(phase).copied().unwrap_or(0.0);
        let better_than_best = match above.as_ref() {
            Some((_, best)) => share > *best,
            None => true,
        };
        if share >= rule.threshold && better_than_best {
            above = Some((phase.clone(), share));
        }
    }
    match above {
        Some((phase, _)) => DenseNumericKernelDecision {
            found: true,
            phase: Some(phase),
            reason_key: "eligible_phase_at_or_above_threshold".to_string(),
        },
        None if eligible_declared => DenseNumericKernelDecision {
            found: false,
            phase: None,
            reason_key: "no_eligible_phase_at_or_above_threshold".to_string(),
        },
        None => DenseNumericKernelDecision {
            found: false,
            phase: None,
            reason_key: "no_contiguous_numeric_phase_declared".to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreDisposition {
    pub dense_numeric_kernel_found: bool,
    pub dense_phase: Option<String>,
    pub dense_rule_reason_key: String,
    pub optimization_state: OptimizationState,
    pub optimization_reason: String,
    pub selection_forbidden: bool,
    pub pgo_disposition: String,
}

/// Turn a dense-kernel decision into the closed disposition.
///
/// `semantic_equal` and `repeated_cpu_improvement` are always false here: this
/// slice implements and measures no SIMD or PGO candidate, and an unmeasured
/// candidate may never be reported as equivalent or improved. That is what
/// keeps `measured_retained` unreachable.
pub fn gate_disposition(
    data: &NativeCoreQualificationData,
    decision: &DenseNumericKernelDecision,
    selection_forbidden: bool,
) -> Result<NativeCoreDisposition, NativeCoreProfileError> {
    let (state, gate_reason) = gate_simd_candidate(decision.found, false, false, false);
    if state == OptimizationState::MeasuredRetained {
        return Err(NativeCoreProfileError::RetentionNotAuthorized {
            family: decision
                .phase
                .clone()
                .unwrap_or_else(|| "aggregate".to_string()),
        });
    }
    let rule = &data.dense_numeric_kernel_rule;
    let phase = decision.phase.clone().unwrap_or_else(|| "none".to_string());
    let reason = format!(
        "{gate_reason}; rule={}; threshold={:.3}; dense_phase={}; candidate=none_implemented; \
pgo={}",
        rule.rule_id, rule.threshold, phase, data.pgo.disposition
    );
    Ok(NativeCoreDisposition {
        dense_numeric_kernel_found: decision.found,
        dense_phase: decision.phase.clone(),
        dense_rule_reason_key: decision.reason_key.clone(),
        optimization_state: state,
        optimization_reason: reason,
        selection_forbidden,
        pgo_disposition: data.pgo.disposition.clone(),
    })
}

// ─────────────────────────── workload ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocatorSnapshot {
    pub allocations: u64,
    pub allocated_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AllocatorObservation {
    Stable {
        allocations: u64,
        allocated_bytes: u64,
    },
    Unstable {
        first: AllocatorSnapshot,
        second: AllocatorSnapshot,
    },
}

/// Artifact-side allocator observation. An unstable replay is recorded as a
/// typed state and carries no numbers, which keeps the emitted artifact
/// byte-reproducible even when the two passes disagree; the raw snapshots stay
/// in the in-process receipt for diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ArtifactAllocatorObservation {
    Stable {
        allocations: u64,
        allocated_bytes: u64,
    },
    Unstable,
}

impl AllocatorObservation {
    pub fn artifact_observation(&self) -> ArtifactAllocatorObservation {
        match self {
            Self::Stable {
                allocations,
                allocated_bytes,
            } => ArtifactAllocatorObservation::Stable {
                allocations: *allocations,
                allocated_bytes: *allocated_bytes,
            },
            Self::Unstable { .. } => ArtifactAllocatorObservation::Unstable,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativeCoreFamilySpec {
    pub family: String,
    pub model: CrfModel,
    pub model_size_bytes: usize,
    pub model_sha256: String,
    pub rows: Vec<String>,
}

impl NativeCoreFamilySpec {
    /// Build a family from frozen model bytes. Rows are drawn from the frozen
    /// model's own observation table — deterministic, model-derived vocabulary,
    /// never document content.
    pub fn from_frozen_model(
        family: &str,
        model_bytes: &[u8],
        rows_per_family: usize,
    ) -> Result<Self, NativeCoreProfileError> {
        let model =
            CrfModel::parse(model_bytes).map_err(|error| NativeCoreProfileError::ModelParse {
                family: family.to_string(),
                reason: error.to_string(),
            })?;
        let rows = derive_rows(&model, rows_per_family).map_err(|error| match error {
            NativeCoreProfileError::NonPositive { name } => {
                NativeCoreProfileError::NonPositive { name }
            }
            other => NativeCoreProfileError::ModelParse {
                family: family.to_string(),
                reason: other.to_string(),
            },
        })?;
        Ok(Self {
            family: family.to_string(),
            model,
            model_size_bytes: model_bytes.len(),
            model_sha256: format!("sha256:{:x}", Sha256::digest(model_bytes)),
            rows,
        })
    }
}

/// Split a Wapiti pattern into its literal prefix and the single
/// `%x[0,column]` reference it carries. Patterns that are not exactly one
/// in-place column reference are not invertible and are skipped.
fn single_column_zero(pattern: &str) -> Option<(String, usize)> {
    let start = pattern.find("%x[")?;
    if pattern[start + 3..].contains("%x[") {
        return None;
    }
    let end = start + 3 + pattern[start + 3..].find(']')?;
    let spec = &pattern[start + 3..end];
    let (offset, column) = spec.split_once(',')?;
    if offset.trim() != "0" {
        return None;
    }
    let column = column.trim().parse::<usize>().ok()?;
    Some((pattern[..start].to_string(), column))
}

/// Deterministic row set drawn from the model's own observation table.
///
/// Wapiti stores an observation as `<pattern literal prefix><token>`. Inverting
/// that relation recovers, per referenced column, the token pool the pattern
/// expands back into: the longest matching prefix wins, so `u:w25:segments`
/// populates column 25 and never column 0. Rows are then built by cycling each
/// column pool, which keeps the workload model-derived, deterministic, and
/// never document content. A column no observation populates is filled with a
/// placeholder that provably expands to a non-observation.
pub fn derive_rows(
    model: &CrfModel,
    rows_per_family: usize,
) -> Result<Vec<String>, NativeCoreProfileError> {
    if rows_per_family == 0 {
        return Err(NativeCoreProfileError::NonPositive {
            name: "rows_per_family".to_string(),
        });
    }
    if model.observations.is_empty() {
        return Err(NativeCoreProfileError::DataContractViolation {
            path: "model.observations".to_string(),
            reason: "the model declares no observations, so no row can reach the kernel"
                .to_string(),
        });
    }

    let mut pools: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for observation in &model.observations {
        let mut best: Option<(usize, String, usize)> = None;
        for pattern in &model.patterns {
            let Some((prefix, column)) = single_column_zero(&pattern.source) else {
                continue;
            };
            if prefix.is_empty() || !observation.starts_with(&prefix) {
                continue;
            }
            let value = &observation[prefix.len()..];
            if value.is_empty() {
                continue;
            }
            let better = match best.as_ref() {
                Some((length, _, _)) => prefix.len() > *length,
                None => true,
            };
            if better {
                best = Some((prefix.len(), value.to_string(), column));
            }
        }
        if let Some((_, value, column)) = best {
            pools.entry(column).or_default().push(value);
        }
    }
    if pools.is_empty() {
        return Err(NativeCoreProfileError::DataContractViolation {
            path: "model.observations".to_string(),
            reason: "no observation could be inverted into a row column, so the profile would \
never reach the kernel"
                .to_string(),
        });
    }

    let placeholder = ["_", "_x", "_unmatched_", "?"]
        .into_iter()
        .find(|candidate| {
            model
                .patterns
                .iter()
                .all(|pattern| match single_column_zero(&pattern.source) {
                    Some((prefix, _)) => !model
                        .observations
                        .iter()
                        .any(|observation| *observation == format!("{prefix}{candidate}")),
                    None => true,
                })
        });
    let Some(placeholder) = placeholder else {
        return Err(NativeCoreProfileError::DataContractViolation {
            path: "model.observations".to_string(),
            reason: "no placeholder token is safe for this model".to_string(),
        });
    };

    // Arity follows the model, not the populated pools: `map_input` refuses a
    // row that is shorter than a referenced column, so a column no observation
    // populates is still materialised with the placeholder.
    let referenced_max = model
        .patterns
        .iter()
        .filter_map(|pattern| single_column_zero(&pattern.source).map(|(_, column)| column))
        .max()
        .unwrap_or(0);
    let max_column = referenced_max.max(pools.keys().copied().max().unwrap_or(0));
    let mut rows = Vec::with_capacity(rows_per_family);
    for index in 0..rows_per_family {
        let mut row = String::new();
        for column in 0..=max_column {
            if column > 0 {
                row.push(' ');
            }
            match pools.get(&column) {
                Some(values) => row.push_str(&values[index % values.len()]),
                None => row.push_str(placeholder),
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Render rows exactly as `map_input` consumes them: one row per line.
fn render_rows(rows: &[String], buffer: &mut [u8]) -> usize {
    let mut cursor = 0usize;
    for row in rows {
        buffer[cursor..cursor + row.len()].copy_from_slice(row.as_bytes());
        cursor += row.len();
        buffer[cursor] = b'\n';
        cursor += 1;
    }
    cursor
}

fn rendered_len(rows: &[String]) -> usize {
    rows.iter().map(|row| row.len() + 1).sum()
}

/// One measured pass. The caller decides which totals map receives the phase
/// timings, so warmup passes are excluded from the shares without a second
/// code path.
fn run_pass(
    model: &CrfModel,
    rows: &[String],
    arena: &mut ScratchArena,
    totals: &mut BTreeMap<&'static str, u128>,
) -> Result<Vec<u8>, ScratchError> {
    let rendered = rendered_len(rows);
    let started = Instant::now();
    let buffer = arena.allocate(rendered)?;
    let written = render_rows(rows, buffer);
    let text = std::str::from_utf8(&buffer[..written])
        .expect("scratch bytes were copied verbatim from UTF-8 row strings");
    *totals.entry(PHASE_RENDER_FEATURE_STRINGS).or_insert(0) += started.elapsed().as_nanos();

    // Phase 2 attributes the observation-index rebuild that `map_input`
    // performs internally on every call. The probe keeps the work observable.
    let started = Instant::now();
    let index: BTreeMap<&str, usize> = model
        .observations
        .iter()
        .enumerate()
        .map(|(position, observation)| (observation.as_str(), position))
        .collect();
    let probe = text
        .split('\n')
        .next()
        .and_then(|first| index.get(first).copied())
        .unwrap_or(usize::MAX);
    *totals.entry(PHASE_OBSERVATION_INDEX_BUILD).or_insert(0) += started.elapsed().as_nanos();

    let started = Instant::now();
    let positions = map_input(model, text).expect(KERNEL_INVARIANT);
    *totals.entry(PHASE_MAP_INPUT_LOOKUP).or_insert(0) += started.elapsed().as_nanos();

    let started = Instant::now();
    let decoded = viterbi_decode(model, &positions).expect(KERNEL_INVARIANT);
    *totals.entry(PHASE_VITERBI_DECODE).or_insert(0) += started.elapsed().as_nanos();

    // Redacted receipt bytes: counts, label ids and score bit patterns only.
    let mut receipt = Vec::with_capacity(64 + decoded.scores.len() * 24);
    let uobs: u64 = positions.iter().map(|row| row.uobs.len() as u64).sum();
    let bobs: u64 = positions.iter().map(|row| row.bobs.len() as u64).sum();
    receipt.extend_from_slice(&(positions.len() as u64).to_le_bytes());
    receipt.extend_from_slice(&(uobs + bobs).to_le_bytes());
    receipt.extend_from_slice(&(probe as u64).to_le_bytes());
    receipt.extend_from_slice(&(decoded.label_ids.len() as u64).to_le_bytes());
    for label in &decoded.label_ids {
        receipt.extend_from_slice(&(*label as u64).to_le_bytes());
    }
    for score in &decoded.scores {
        receipt.extend_from_slice(&score.to_bits().to_le_bytes());
    }
    Ok(receipt)
}

// ─────────────────────────── profile ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCoreProfileOptions {
    pub warmup_iterations: usize,
    pub repetitions: usize,
    pub rows_per_family: usize,
}

impl NativeCoreProfileOptions {
    pub fn from_data(data: &NativeCoreQualificationData, rows_per_family: usize) -> Self {
        Self {
            warmup_iterations: data.profile.warmup_iterations,
            repetitions: data.profile.repetitions,
            rows_per_family,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FamilyProfile {
    pub family: String,
    pub model_sha256: String,
    pub model_size_bytes: usize,
    pub rows: usize,
    pub positions: usize,
    pub matched_observations: usize,
    pub characterization: CpuKernelCharacterization,
    pub decision: DenseNumericKernelDecision,
    pub disposition: NativeCoreDisposition,
    /// Observational: nanoseconds per phase, warmup passes excluded.
    pub phase_nanos: BTreeMap<String, u128>,
    /// Observational: share of the four-phase total in basis points.
    pub phase_share_basis_points: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreFamilyObservations {
    pub family: String,
    pub percentiles: KernelPercentiles,
    pub throughput_units_per_second: u64,
    pub phase_nanos: BTreeMap<String, u128>,
    pub phase_share_basis_points: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreObservationalBlock {
    pub note: String,
    pub excluded_from: Vec<String>,
    pub families: Vec<NativeCoreFamilyObservations>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreProfileReceipt {
    pub data: NativeCoreQualificationData,
    pub families: Vec<FamilyProfile>,
    pub allocator: AllocatorObservation,
    pub disposition: NativeCoreDisposition,
    pub observations: NativeCoreObservationalBlock,
}

fn share_basis_points(nanos: &BTreeMap<&'static str, u128>) -> BTreeMap<String, u64> {
    let total: u128 = nanos.values().copied().sum();
    NATIVE_CORE_PHASES
        .iter()
        .map(|phase| {
            let value = nanos.get(phase).copied().unwrap_or(0);
            let basis_points = if total == 0 {
                0
            } else {
                ((value as f64 / total as f64) * 10_000.0).round() as u64
            };
            ((*phase).to_string(), basis_points)
        })
        .collect()
}

fn share_fraction(nanos: &BTreeMap<&'static str, u128>) -> BTreeMap<String, f64> {
    let total: u128 = nanos.values().copied().sum();
    NATIVE_CORE_PHASES
        .iter()
        .map(|phase| {
            let value = nanos.get(phase).copied().unwrap_or(0);
            let share = if total == 0 {
                0.0
            } else {
                value as f64 / total as f64
            };
            ((*phase).to_string(), share)
        })
        .collect()
}

fn nanos_to_string_map(nanos: &BTreeMap<&'static str, u128>) -> BTreeMap<String, u128> {
    nanos
        .iter()
        .map(|(phase, value)| ((*phase).to_string(), *value))
        .collect()
}

fn profile_family(
    data: &NativeCoreQualificationData,
    spec: &NativeCoreFamilySpec,
    options: &NativeCoreProfileOptions,
) -> Result<FamilyProfile, NativeCoreProfileError> {
    let rendered = rendered_len(&spec.rows);
    let scratch_capacity = rendered.max(1);
    let work_units = spec.rows.len() as u64;

    // Dry run: proves the kernel cannot fail inside the measured closure and
    // supplies the deterministic position/fan-out counts for the receipt.
    let mut dry_text = String::with_capacity(rendered);
    for row in &spec.rows {
        dry_text.push_str(row);
        dry_text.push('\n');
    }
    let dry_positions = map_input(&spec.model, &dry_text).map_err(|error| {
        NativeCoreProfileError::Characterization {
            family: spec.family.clone(),
            reason: error.to_string(),
        }
    })?;
    let matched_observations: usize = dry_positions
        .iter()
        .map(|row| row.uobs.len() + row.bobs.len())
        .sum();
    if matched_observations == 0 {
        return Err(NativeCoreProfileError::DegenerateWorkload {
            family: spec.family.clone(),
            matched_observations,
        });
    }
    viterbi_decode(&spec.model, &dry_positions).map_err(|error| {
        NativeCoreProfileError::Characterization {
            family: spec.family.clone(),
            reason: error.to_string(),
        }
    })?;

    let mut totals: BTreeMap<&'static str, u128> = BTreeMap::new();
    let mut warmup_totals: BTreeMap<&'static str, u128> = BTreeMap::new();
    let mut pass_index = 0usize;
    let warmup = options.warmup_iterations;
    let model = &spec.model;
    let rows = &spec.rows;
    let characterization = characterize_cpu_kernel(
        warmup,
        options.repetitions,
        scratch_capacity,
        work_units,
        |arena| {
            let measured = pass_index >= warmup;
            pass_index += 1;
            let sink = if measured {
                &mut totals
            } else {
                &mut warmup_totals
            };
            run_pass(model, rows, arena, sink)
        },
    )
    .map_err(|error| match error {
        CharacterizationError::SemanticMismatch { .. } => {
            NativeCoreProfileError::Characterization {
                family: spec.family.clone(),
                reason: error.to_string(),
            }
        }
        other => NativeCoreProfileError::Characterization {
            family: spec.family.clone(),
            reason: other.to_string(),
        },
    })?;
    if !characterization.selection_forbidden {
        return Err(NativeCoreProfileError::DataContractViolation {
            path: "native_core_qualification.profile.selection_forbidden".to_string(),
            reason: "the qualified primitive reported selection as allowed".to_string(),
        });
    }

    let shares = share_fraction(&totals);
    let decision = evaluate_dense_numeric_kernel(&data.dense_numeric_kernel_rule, &shares);
    let disposition = gate_disposition(data, &decision, characterization.selection_forbidden)?;

    Ok(FamilyProfile {
        family: spec.family.clone(),
        model_sha256: spec.model_sha256.clone(),
        model_size_bytes: spec.model_size_bytes,
        rows: spec.rows.len(),
        positions: dry_positions.len(),
        matched_observations,
        characterization,
        decision,
        disposition,
        phase_nanos: nanos_to_string_map(&totals),
        phase_share_basis_points: share_basis_points(&totals),
    })
}

fn aggregate_decision(families: &[FamilyProfile]) -> DenseNumericKernelDecision {
    let found = families
        .iter()
        .filter(|family| family.decision.found)
        .max_by_key(|family| {
            family
                .phase_share_basis_points
                .get(PHASE_VITERBI_DECODE)
                .copied()
                .unwrap_or(0)
        });
    match found {
        Some(family) => DenseNumericKernelDecision {
            found: true,
            phase: family.decision.phase.clone(),
            reason_key: "eligible_phase_at_or_above_threshold".to_string(),
        },
        None => DenseNumericKernelDecision {
            found: false,
            phase: None,
            reason_key: if families.iter().any(|family| {
                family.decision.reason_key == "no_eligible_phase_at_or_above_threshold"
            }) {
                "no_eligible_phase_at_or_above_threshold".to_string()
            } else {
                "no_contiguous_numeric_phase_declared".to_string()
            },
        },
    }
}

/// Profile every family twice and return the receipt.
///
/// The two passes exist so the allocator observation is a *replay* comparison,
/// not a single sample: equal deltas yield numbers, disagreeing deltas yield a
/// typed unstable state. The artifact projection of both passes must be
/// byte-identical or the call fails loudly instead of emitting a flaky claim.
pub fn profile_native_core(
    data: &NativeCoreQualificationData,
    specs: &[NativeCoreFamilySpec],
    options: &NativeCoreProfileOptions,
    snapshot: &mut dyn FnMut() -> AllocatorSnapshot,
) -> Result<NativeCoreProfileReceipt, NativeCoreProfileError> {
    if specs.is_empty() {
        return Err(NativeCoreProfileError::EmptyFamilySet);
    }
    if options.repetitions < 2 {
        return Err(NativeCoreProfileError::NonPositive {
            name: "repetitions (a replayable scalar oracle needs at least 2)".to_string(),
        });
    }

    let mut passes: Vec<Vec<FamilyProfile>> = Vec::with_capacity(2);
    let mut deltas: Vec<AllocatorSnapshot> = Vec::with_capacity(2);
    for _ in 0..2 {
        let before = snapshot();
        let mut families = Vec::with_capacity(specs.len());
        for spec in specs {
            families.push(profile_family(data, spec, options)?);
        }
        let after = snapshot();
        passes.push(families);
        deltas.push(AllocatorSnapshot {
            allocations: after.allocations.saturating_sub(before.allocations),
            allocated_bytes: after.allocated_bytes.saturating_sub(before.allocated_bytes),
        });
    }

    let allocator = if deltas[0] == deltas[1] {
        AllocatorObservation::Stable {
            allocations: deltas[0].allocations,
            allocated_bytes: deltas[0].allocated_bytes,
        }
    } else {
        AllocatorObservation::Unstable {
            first: deltas[0].clone(),
            second: deltas[1].clone(),
        }
    };

    let second = passes.pop().expect("two passes");
    let first = passes.pop().expect("two passes");
    let observations = NativeCoreObservationalBlock {
        note: "timing is observational evidence: it stays in the in-process receipt and is \
excluded from the emitted artifact and from every acceptance denominator"
            .to_string(),
        excluded_from: vec![
            "native_core_profile_artifact".to_string(),
            "acceptance_denominators".to_string(),
        ],
        families: first
            .iter()
            .map(|family| NativeCoreFamilyObservations {
                family: family.family.clone(),
                percentiles: family.characterization.percentiles.clone(),
                throughput_units_per_second: family.characterization.throughput_units_per_second,
                phase_nanos: family.phase_nanos.clone(),
                phase_share_basis_points: family.phase_share_basis_points.clone(),
            })
            .collect(),
    };
    let aggregate = aggregate_decision(&first);
    let disposition = gate_disposition(
        data,
        &aggregate,
        first
            .iter()
            .all(|family| family.disposition.selection_forbidden),
    )?;

    let receipt = NativeCoreProfileReceipt {
        data: data.clone(),
        families: first,
        allocator,
        disposition,
        observations,
    };
    let replay = NativeCoreProfileReceipt {
        data: data.clone(),
        families: second,
        allocator: receipt.allocator.clone(),
        disposition: receipt.disposition.clone(),
        observations: NativeCoreObservationalBlock {
            note: receipt.observations.note.clone(),
            excluded_from: receipt.observations.excluded_from.clone(),
            families: Vec::new(),
        },
    };
    let first_bytes = receipt.artifact_json_bytes()?;
    let second_bytes = replay.artifact_json_bytes()?;
    if first_bytes != second_bytes {
        return Err(NativeCoreProfileError::ArtifactReplayDiverged {
            first: format!("sha256:{:x}", Sha256::digest(&first_bytes)),
            second: format!("sha256:{:x}", Sha256::digest(&second_bytes)),
        });
    }
    Ok(receipt)
}

// ─────────────────────────── artifact ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreArtifactProfile {
    pub warmup_iterations: usize,
    pub repetitions: usize,
    pub percentile_ranks: NativeCorePercentileConfig,
    pub throughput_units: String,
    pub allocator_units: NativeCoreAllocatorConfig,
    pub private_scratch_units: NativeCoreScratchConfig,
    pub scalar_oracle: String,
    pub selection_forbidden: bool,
    pub timing_disposition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreArtifactRule {
    pub rule_id: String,
    pub share_metric: String,
    pub threshold: f64,
    pub requires_contiguous_numeric_arrays: bool,
    pub eligible_phases: Vec<String>,
    pub outcome_when_unmet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreArtifactPhase {
    pub phase: String,
    pub work_kind: String,
    pub storage: String,
    pub contiguous_numeric_arrays: bool,
    pub sparse_weight_reads: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreArtifactFamily {
    pub family: String,
    pub model_sha256: String,
    pub model_size_bytes: usize,
    pub rows: usize,
    pub positions: usize,
    pub matched_observations: usize,
    pub scalar_oracle_digest: String,
    pub semantic_digest: String,
    pub cpu_only: bool,
    pub subprocess_io_included: bool,
    pub private_scratch_capacity_bytes: usize,
    pub private_scratch_high_water_bytes: usize,
    pub dense_numeric_kernel_found: bool,
    pub dense_phase: Option<String>,
    pub dense_rule_reason_key: String,
    pub optimization_state: OptimizationState,
    pub optimization_reason: String,
    pub selection_forbidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreArtifactDisposition {
    pub dense_numeric_kernel_found: bool,
    pub dense_phase: Option<String>,
    pub dense_rule_reason_key: String,
    pub optimization_state: OptimizationState,
    pub optimization_reason: String,
    pub selection_forbidden: bool,
    pub pgo_disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreArtifactPromotion {
    pub state: String,
    pub selected: String,
    pub promoted: String,
    pub auto_promote: String,
    pub rollback_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreArtifactRedaction {
    pub forbidden: Vec<String>,
    pub timing_disposition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreProfileArtifact {
    pub schema_version: String,
    pub adr_ref: String,
    pub qualification_schema_version: String,
    pub manifest_binding: String,
    pub requirement_ids: Vec<String>,
    pub profile: NativeCoreArtifactProfile,
    pub rule: NativeCoreArtifactRule,
    pub phases: Vec<NativeCoreArtifactPhase>,
    pub families: Vec<NativeCoreArtifactFamily>,
    pub disposition: NativeCoreArtifactDisposition,
    pub allocator: ArtifactAllocatorObservation,
    pub promotion: NativeCoreArtifactPromotion,
    pub redaction: NativeCoreArtifactRedaction,
    pub h05_closure: String,
}

impl NativeCoreProfileReceipt {
    /// Deterministic projection: identity, DATA binding, oracle digests, the
    /// typed disposition and the allocator state. Timing is deliberately
    /// absent, which is what makes two independent profiles byte-identical.
    pub fn artifact(&self) -> NativeCoreProfileArtifact {
        let data = &self.data;
        NativeCoreProfileArtifact {
            schema_version: NATIVE_CORE_PROFILE_SCHEMA_VERSION.to_string(),
            adr_ref: data.adr_ref.clone(),
            qualification_schema_version: data.schema_version.clone(),
            manifest_binding: data.manifest_binding.clone(),
            requirement_ids: data.requirement_ids.clone(),
            profile: NativeCoreArtifactProfile {
                warmup_iterations: data.profile.warmup_iterations,
                repetitions: data.profile.repetitions,
                percentile_ranks: data.profile.percentiles.clone(),
                throughput_units: data.profile.throughput_units_per_second.clone(),
                allocator_units: data.profile.allocator.clone(),
                private_scratch_units: data.profile.private_scratch.clone(),
                scalar_oracle: data.profile.scalar_oracle.clone(),
                selection_forbidden: data.profile.selection_forbidden,
                timing_disposition: NATIVE_CORE_TIMING_DISPOSITION.to_string(),
            },
            rule: NativeCoreArtifactRule {
                rule_id: data.dense_numeric_kernel_rule.rule_id.clone(),
                share_metric: data.dense_numeric_kernel_rule.share_metric.clone(),
                threshold: data.dense_numeric_kernel_rule.threshold,
                requires_contiguous_numeric_arrays: data
                    .dense_numeric_kernel_rule
                    .requires_contiguous_numeric_arrays,
                eligible_phases: data.dense_numeric_kernel_rule.eligible_phases.clone(),
                outcome_when_unmet: data.dense_numeric_kernel_rule.outcome_when_unmet.clone(),
            },
            phases: NATIVE_CORE_PHASES
                .iter()
                .filter_map(|phase| {
                    data.dense_numeric_kernel_rule
                        .phase_kinds
                        .get(*phase)
                        .map(|kind| NativeCoreArtifactPhase {
                            phase: (*phase).to_string(),
                            work_kind: kind.work_kind.clone(),
                            storage: kind.storage.clone(),
                            contiguous_numeric_arrays: kind.contiguous_numeric_arrays,
                            sparse_weight_reads: kind.sparse_weight_reads,
                        })
                })
                .collect(),
            families: self
                .families
                .iter()
                .map(|family| NativeCoreArtifactFamily {
                    family: family.family.clone(),
                    model_sha256: family.model_sha256.clone(),
                    model_size_bytes: family.model_size_bytes,
                    rows: family.rows,
                    positions: family.positions,
                    matched_observations: family.matched_observations,
                    scalar_oracle_digest: family.characterization.scalar_oracle_digest.clone(),
                    semantic_digest: family.characterization.semantic_digest.clone(),
                    cpu_only: family.characterization.cpu_only,
                    subprocess_io_included: family.characterization.subprocess_io_included,
                    private_scratch_capacity_bytes: family
                        .characterization
                        .allocation_or_private_capacity_bytes,
                    private_scratch_high_water_bytes: family
                        .characterization
                        .private_scratch_high_water_bytes,
                    dense_numeric_kernel_found: family.disposition.dense_numeric_kernel_found,
                    dense_phase: family.disposition.dense_phase.clone(),
                    dense_rule_reason_key: family.disposition.dense_rule_reason_key.clone(),
                    optimization_state: family.disposition.optimization_state,
                    optimization_reason: family.disposition.optimization_reason.clone(),
                    selection_forbidden: family.disposition.selection_forbidden,
                })
                .collect(),
            disposition: NativeCoreArtifactDisposition {
                dense_numeric_kernel_found: self.disposition.dense_numeric_kernel_found,
                dense_phase: self.disposition.dense_phase.clone(),
                dense_rule_reason_key: self.disposition.dense_rule_reason_key.clone(),
                optimization_state: self.disposition.optimization_state,
                optimization_reason: self.disposition.optimization_reason.clone(),
                selection_forbidden: self.disposition.selection_forbidden,
                pgo_disposition: self.disposition.pgo_disposition.clone(),
            },
            allocator: self.allocator.artifact_observation(),
            promotion: NativeCoreArtifactPromotion {
                state: data.promotion.state.clone(),
                selected: data.promotion.selected.clone(),
                promoted: data.promotion.promoted.clone(),
                auto_promote: data.promotion.auto_promote.clone(),
                rollback_target: data.promotion.rollback_target.clone(),
            },
            redaction: NativeCoreArtifactRedaction {
                forbidden: data.redaction.forbidden.clone(),
                timing_disposition: NATIVE_CORE_TIMING_DISPOSITION.to_string(),
            },
            h05_closure: data.h05_closure.clone(),
        }
    }

    /// Emitted artifact bytes: pretty JSON plus a trailing newline.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, NativeCoreProfileError> {
        let mut bytes = serde_json::to_vec_pretty(&self.artifact()).map_err(|error| {
            NativeCoreProfileError::DataUnreadable {
                reason: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with_threshold(threshold: f64) -> NativeCoreQualificationData {
        let manifest = include_str!("../../../data/parse/comparison_contract.yaml");
        let mut data = load_qualification_from_manifest(manifest).expect("qualification block");
        data.dense_numeric_kernel_rule.threshold = threshold;
        data
    }

    #[test]
    fn roundtrip_artifact_and_allocator_observation() {
        let manifest = include_str!("../../../data/parse/comparison_contract.yaml");
        let data = load_qualification_from_manifest(manifest).expect("qualification block");
        assert_eq!(
            data.schema_version,
            NATIVE_CORE_QUALIFICATION_SCHEMA_VERSION
        );
        assert!(data.dense_numeric_kernel_rule.threshold > 0.0);
        let observation = AllocatorObservation::Unstable {
            first: AllocatorSnapshot {
                allocations: 1,
                allocated_bytes: 2,
            },
            second: AllocatorSnapshot {
                allocations: 3,
                allocated_bytes: 4,
            },
        };
        let json = serde_json::to_string(&observation).expect("serialize");
        let back: AllocatorObservation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(observation, back);
    }

    #[test]
    fn invented_phase_and_weakened_lock_are_rejected() {
        let mut data = data_with_threshold(0.25);
        data.dense_numeric_kernel_rule.phase_kinds.insert(
            "invented_phase".to_string(),
            DenseNumericPhaseKind {
                work_kind: WORK_KIND_TEXT.to_string(),
                storage: "arena_bytes".to_string(),
                contiguous_numeric_arrays: false,
                sparse_weight_reads: false,
            },
        );
        assert!(data.validate().is_err());

        let mut data = data_with_threshold(0.25);
        data.promotion.auto_promote = "allowed".to_string();
        assert!(data.validate().is_err());

        let mut data = data_with_threshold(0.25);
        data.profile.selection_forbidden = false;
        assert!(data.validate().is_err());
    }
}
