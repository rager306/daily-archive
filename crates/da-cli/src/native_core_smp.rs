//! Bounded SMP qualification of the native scalar CRF core.
//!
//! S03 asked for "then SMP": profile first, then qualify a bounded CPU
//! (multi-worker) execution of the *same* native kernel. T01 produced the
//! profile; this module produces the bounded SMP qualification. It runs the
//! real native CRF core — frozen synthetic Wapiti models, `da_ports::map_input`
//! and `da_ports::viterbi_decode` — over a deterministic job set (family ×
//! document) on every worker count `1..=physical_core_cap`, and it proves the
//! semantics are worker-count-invariant.
//!
//! ## Reuse, never reinvention
//!
//! Every runtime primitive comes from [`crate::layout_runtime`]:
//! [`detect_system_topology`] (via the caller), [`RuntimeConfig::validate`],
//! [`run_bounded`], [`Terminal`] and [`ScratchArena`]. There is no second
//! topology reader, no global pool, no unbounded queue and no worker clamp: a
//! request above `physical_core_cap` fails through `ConfigError::OverCoreCap`
//! *before* any thread is spawned.
//!
//! ## What is deterministic and what is observational
//!
//! Deterministic (the acceptance surface): the job set, per-job semantic
//! digests, the admitted/terminal conservation, ordinals, per-worker terminal
//! counts, private scratch capacity and the *closed* breakpoint verdict
//! vocabulary. The strongest invariant is enforced here, not asserted by a
//! caller: the semantic digest of every worker count must equal the digest of
//! the serial `workers = 1` run, otherwise the call fails with a typed
//! [`NativeCoreSmpError::SemanticDivergence`].
//!
//! Observational (never an acceptance denominator): wall-clock latency
//! percentiles, throughput, run wall time and the runtime queue high-water
//! gauges. They are declared field-by-field in
//! [`NativeCoreSmpObservationalBlock`] and in the emitted artifact, and they
//! only ever *classify* a worker count into the closed breakpoint vocabulary.
//!
//! [`SmpObservation`] is the typed absence: a metric that was not measured is
//! `NotObserved { reason }`, never a fabricated zero. Process peak RSS is
//! injected by the caller (this module installs no allocator and no RSS
//! reader); `queue_wait_ms` is declared but never fabricated.
//!
//! ## Promotion and H05
//!
//! Nothing here selects, splices or promotes a production route: `selection`
//! stays `forbidden`, `promoted` stays `forbidden`, `auto_promote` stays
//! `forbidden`, and H05 (RSS/CPU/byte ceilings) stays typed `not_claimed`.

use crate::layout_runtime::{
    ConfigError, CoordinatorFailure, RuntimeConfig, ScratchArena, Terminal, TopologyReceipt,
    run_bounded,
};
use crate::native_core_profile::derive_rows;
use da_domain::scalar_crf::CrfModel;
use da_ports::{map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

pub const NATIVE_CORE_SMP_SCHEMA_VERSION: &str = "native-core-smp.v1";
pub const CPU_SMP_CANDIDATE_SCHEMA_VERSION: &str = "cpu-smp-candidate.v1";
pub const CPU_SMP_CANDIDATE_KEY: &str = "cpu_smp_candidate";
pub const NATIVE_CORE_SMP_ADR_REF: &str = "ADR-075";
pub const NATIVE_CORE_SMP_MANIFEST_BINDING: &str = "data/parse/comparison_contract.yaml";
/// The ledger the S14 candidate evidence itself binds to. The block lives in
/// [`NATIVE_CORE_SMP_MANIFEST_BINDING`]; this is what the candidate points at,
/// and conflating the two would silently rewrite existing DATA.
pub const CPU_SMP_CANDIDATE_MANIFEST_BINDING: &str = "data/parse/layout_bakeoff.yaml";
pub const NATIVE_CORE_SMP_ARTIFACT_REL: &str = "artifacts/parse_compat/native_core_smp.json";
pub const NATIVE_CORE_SMP_TIMING_DISPOSITION: &str =
    "observational_excluded_from_acceptance_denominators";
pub const NATIVE_CORE_SMP_ROLLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";
pub const NATIVE_CORE_SMP_H05_CLOSURE: &str = "not_claimed";
/// The breakpoint verdicts are DATA-owned; this is the binding site, and a
/// verdict outside it is refused rather than ignored.
pub const NATIVE_CORE_SMP_BREAKPOINT_SOURCE: &str =
    "data/parse/comparison_contract.yaml#cpu_smp_candidate.breakpoints.verdicts";
/// Worker counts are swept from 1 to the physical core cap, inclusive.
pub const NATIVE_CORE_SMP_WORKER_MATRIX: &str = "1..=physical_core_cap";

/// Closed breakpoint vocabulary, mirroring `cpu_smp_candidate.breakpoints.verdicts`.
pub const NATIVE_CORE_SMP_VERDICTS: [&str; 6] = [
    "not_observed",
    "saturation",
    "first_regression",
    "queue_pressure",
    "memory_pressure",
    "inconclusive",
];

/// Closed coordinator failure vocabulary, mirroring
/// `cpu_smp_candidate.terminal_receipts.failure_vocabulary`.
pub const NATIVE_CORE_SMP_COORDINATOR_FAILURES: [&str; 3] =
    ["worker_panic", "channel_closed", "interrupted"];

/// Metric names this receipt actually carries. `cpu_smp_candidate.metrics.required`
/// must be a subset of this list, which is how a DATA metric cannot be silently
/// dropped by the implementation.
pub const NATIVE_CORE_SMP_METRIC_NAMES: [&str; 9] = [
    "p50_ms",
    "p95_ms",
    "p99_ms",
    "throughput_units_per_second",
    "process_rss_bytes",
    "allocation_or_private_capacity_bytes",
    "queue_wait_ms",
    "input_queue_high_water",
    "result_queue_high_water",
];

const REQUIRED_REQUIREMENT_IDS: [&str; 5] = ["R085", "R106", "R109", "R113", "R117"];
const EXECUTION_SCOPE: &str = "invocation_private";
const REQUESTED_ABOVE_CAP: &str = "fail_before_spawn";
const SILENT_CLAMP: &str = "forbidden";
const BOUNDED: &str = "bounded";
const BACKPRESSURE: &str = "backpressure";
const DROP_ON_FULL: &str = "forbidden";
const GLOBAL_POOL: &str = "forbidden";
const PRIVATE_WORKER_SCRATCH: &str = "bounded_reusable";
const UNSAFE_PROJECT_CODE: &str = "forbidden";
const REQUIRED: &str = "required";
const FORBIDDEN: &str = "forbidden";
const CANDIDATE: &str = "candidate";
const TOPOLOGY_SOURCE: &str = "linux_allowed_cpu_set_intersected_with_sysfs_package_core";
const LOGICAL_CPU_SUBSTITUTION: &str = "forbidden";
const NUMA_CLAIM: &str = "forbidden";
const SAFE_RUST_ONLY: bool = true;

// ───────────────────────────── errors ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCoreSmpError {
    /// The manifest text could not be read or parsed as a YAML mapping.
    DataUnreadable { reason: String },
    /// The `cpu_smp_candidate` block is present but violates its contract.
    DataContractViolation { path: String, reason: String },
    /// A frozen synthetic model did not parse.
    ModelParse { family: String, reason: String },
    /// Rows could not be derived from a frozen synthetic model.
    RowDerivation { family: String, reason: String },
    /// A non-positive measurement or workload option.
    NonPositive { name: String },
    /// No family was supplied, so there is nothing to qualify.
    EmptyFamilySet,
    /// The frozen model carries fewer rows than the requested workload needs.
    WorkloadTooSmall {
        family: String,
        rows: usize,
        required: usize,
    },
    /// The topology could not be used (for example a zero core cap).
    Topology { reason: String },
    /// The requested worker count exceeds the physical core cap.
    OverCoreCap { requested: usize, cap: usize },
    /// The bounded runtime refused the configuration for another reason.
    Config { reason: String },
    /// A job returned a typed error; the kernel is deterministic, so a
    /// failure here is an implementation defect and never a data variation.
    JobFailed {
        workers: usize,
        family: String,
        document: usize,
        reason: String,
    },
    /// A worker panicked or a channel closed; the loss is never silent.
    CoordinatorFailed {
        workers: usize,
        kind: String,
        worker: usize,
    },
    /// Two repetitions of one worker count disagreed on a deterministic field.
    NonDeterministicWorkload { workers: usize, field: String },
    /// Admitted and terminal counts disagree, or an ordinal was dropped.
    TerminalConservation {
        workers: usize,
        admitted: usize,
        terminals: usize,
    },
    /// The terminals published are not fully attributed to the workers that ran.
    PerWorkerConservation {
        workers: usize,
        terminals: usize,
        accounted: usize,
    },
    /// A multi-worker digest differed from the serial `workers = 1` oracle.
    SemanticDivergence {
        workers: usize,
        expected: String,
        observed: String,
    },
    /// A declared value is outside its closed DATA vocabulary.
    VocabularyViolation { path: String, value: String },
    /// The emitted artifact could not be serialized.
    Artifact { reason: String },
}

impl fmt::Display for NativeCoreSmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataUnreadable { reason } => {
                write!(f, "cpu smp candidate data unreadable: {reason}")
            }
            Self::DataContractViolation { path, reason } => {
                write!(
                    f,
                    "cpu smp candidate contract violation at {path}: {reason}"
                )
            }
            Self::ModelParse { family, reason } => {
                write!(
                    f,
                    "frozen native model for family {family} did not parse: {reason}"
                )
            }
            Self::RowDerivation { family, reason } => {
                write!(f, "no deterministic rows for family {family}: {reason}")
            }
            Self::NonPositive { name } => write!(f, "option {name} must be positive"),
            Self::EmptyFamilySet => write!(f, "no native core family was supplied"),
            Self::WorkloadTooSmall {
                family,
                rows,
                required,
            } => write!(
                f,
                "family {family} carries {rows} rows but the requested workload needs {required}"
            ),
            Self::Topology { reason } => write!(f, "topology unusable: {reason}"),
            Self::OverCoreCap { requested, cap } => write!(
                f,
                "requested {requested} workers but the physical core cap is {cap}; \
this is refused before spawn and never clamped"
            ),
            Self::Config { reason } => write!(f, "invalid bounded runtime config: {reason}"),
            Self::JobFailed {
                workers,
                family,
                document,
                reason,
            } => write!(
                f,
                "native core job {family}/{document} failed at {workers} workers: {reason}"
            ),
            Self::CoordinatorFailed {
                workers,
                kind,
                worker,
            } => write!(
                f,
                "bounded runtime failed at {workers} workers: {kind} on worker {worker}"
            ),
            Self::NonDeterministicWorkload { workers, field } => write!(
                f,
                "two repetitions at {workers} workers disagreed on `{field}`; \
a non-deterministic workload is not evidence"
            ),
            Self::TerminalConservation {
                workers,
                admitted,
                terminals,
            } => write!(
                f,
                "at {workers} workers {admitted} units were admitted but {terminals} terminals \
were published"
            ),
            Self::PerWorkerConservation {
                workers,
                terminals,
                accounted,
            } => write!(
                f,
                "at {workers} workers {terminals} terminals were published but {accounted} are \
attributed to a worker"
            ),
            Self::SemanticDivergence {
                workers,
                expected,
                observed,
            } => write!(
                f,
                "worker count {workers} changed the native core semantics: {observed} != serial oracle {expected}"
            ),
            Self::VocabularyViolation { path, value } => {
                write!(f, "`{value}` at {path} is outside its closed vocabulary")
            }
            Self::Artifact { reason } => write!(f, "artifact serialization failed: {reason}"),
        }
    }
}

impl std::error::Error for NativeCoreSmpError {}

// ──────────────── DATA-owned `cpu_smp_candidate` block ────────────────

/// The `cpu_smp_candidate` sibling block of the comparison ledger, typed.
///
/// Only the fields this slice consumes are modelled; extra DATA is allowed and
/// every consumed invariant is checked in [`Self::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuSmpCandidateData {
    pub schema_version: String,
    pub manifest_binding: String,
    pub requirement_ids: Vec<String>,
    pub topology: CpuSmpTopologyPolicy,
    pub execution: CpuSmpExecution,
    pub denominators: CpuSmpDenominators,
    pub terminal_receipts: CpuSmpTerminalReceipts,
    pub metrics: CpuSmpMetricsPolicy,
    pub breakpoints: CpuSmpBreakpoints,
    pub optimization: CpuSmpOptimizationPolicy,
    pub redaction: CpuSmpRedactionPolicy,
    pub promotion: CpuSmpPromotionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpTopologyPolicy {
    pub source: String,
    pub identity: String,
    pub unavailable_state: String,
    pub logical_cpu_substitution: String,
    pub numa_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpExecution {
    pub scope: String,
    pub worker_matrix: String,
    pub requested_above_cap: String,
    pub silent_clamp: String,
    pub input_queue: String,
    pub result_queue: String,
    pub full_queue: String,
    pub drop_on_full: String,
    pub global_pool: String,
    pub private_worker_scratch: String,
    pub unsafe_project_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuSmpDenominators {
    pub acceptance: CpuSmpAcceptanceDenominator,
    pub benchmark: CpuSmpBenchmarkPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpAcceptanceDenominator {
    pub corpus_n: u64,
    pub lanes_n: u64,
    pub pages_per_lane: u64,
    pub total_lane_pages: u64,
    pub fixed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpBenchmarkPolicy {
    pub repetitions: String,
    pub separate_from_acceptance: bool,
    pub pooled: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpTerminalReceipts {
    pub admitted_equals_terminal: String,
    pub exactly_one_per_admitted_unit: String,
    pub failure_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpMetricsPolicy {
    pub required: Vec<String>,
    pub per_worker_terminal_counts: String,
    pub per_lane: String,
    pub cpu_only_excludes_subprocess_io: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpBreakpoints {
    pub verdicts: Vec<String>,
    pub closed: bool,
    pub data_defined: bool,
    /// Not declared by the committed DATA today, which is exactly why
    /// `memory_pressure` is typed `not_observed` instead of guessed.
    #[serde(default)]
    pub memory_ceiling_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpOptimizationPolicy {
    pub states: Vec<String>,
    pub safe_rust_only: bool,
    pub scalar_oracle: String,
    pub selection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpRedactionPolicy {
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSmpPromotionPolicy {
    pub state: String,
    pub selected: String,
    pub promoted: String,
    pub auto_promote: String,
    pub rollback_target: String,
}

/// Parse the `cpu_smp_candidate` block out of the manifest text.
///
/// The manifest is passed as text so the reader depends on no filesystem path
/// and no wall-clock state; the emitter and the contract test both feed it the
/// git-tracked ledger.
pub fn load_cpu_smp_candidate_from_manifest(
    manifest_yaml: &str,
) -> Result<CpuSmpCandidateData, NativeCoreSmpError> {
    let manifest: serde_yaml::Value = serde_yaml::from_str(manifest_yaml).map_err(|error| {
        NativeCoreSmpError::DataUnreadable {
            reason: error.to_string(),
        }
    })?;
    let block = manifest.get(CPU_SMP_CANDIDATE_KEY).ok_or_else(|| {
        NativeCoreSmpError::DataContractViolation {
            path: CPU_SMP_CANDIDATE_KEY.to_string(),
            reason: "block missing from the ledger; loss is not silence".to_string(),
        }
    })?;
    let data: CpuSmpCandidateData = serde_yaml::from_value(block.clone()).map_err(|error| {
        NativeCoreSmpError::DataContractViolation {
            path: CPU_SMP_CANDIDATE_KEY.to_string(),
            reason: error.to_string(),
        }
    })?;
    data.validate()?;
    Ok(data)
}

impl CpuSmpCandidateData {
    /// Fail loudly on every invariant this slice relies on.
    pub fn validate(&self) -> Result<(), NativeCoreSmpError> {
        let violation = |path: &str, reason: String| NativeCoreSmpError::DataContractViolation {
            path: path.to_string(),
            reason,
        };
        let expect = |path: &str, actual: &str, expected: &str| -> Result<(), NativeCoreSmpError> {
            if actual != expected {
                return Err(violation(
                    path,
                    format!("expected `{expected}`, got `{actual}`"),
                ));
            }
            Ok(())
        };

        expect(
            "cpu_smp_candidate.schema_version",
            &self.schema_version,
            CPU_SMP_CANDIDATE_SCHEMA_VERSION,
        )?;
        expect(
            "cpu_smp_candidate.manifest_binding",
            &self.manifest_binding,
            CPU_SMP_CANDIDATE_MANIFEST_BINDING,
        )?;
        for required in REQUIRED_REQUIREMENT_IDS {
            if !self.requirement_ids.iter().any(|id| id == required) {
                return Err(violation(
                    "cpu_smp_candidate.requirement_ids",
                    format!("required requirement {required} is missing"),
                ));
            }
        }

        let topology = "cpu_smp_candidate.topology";
        expect(
            &format!("{topology}.source"),
            &self.topology.source,
            TOPOLOGY_SOURCE,
        )?;
        if self.topology.identity.is_empty() {
            return Err(violation(
                &format!("{topology}.identity"),
                "the core identity source must be named".to_string(),
            ));
        }
        if self.topology.unavailable_state.is_empty() {
            return Err(violation(
                &format!("{topology}.unavailable_state"),
                "an unavailable topology must stay typed, not silent".to_string(),
            ));
        }
        expect(
            &format!("{topology}.logical_cpu_substitution"),
            &self.topology.logical_cpu_substitution,
            LOGICAL_CPU_SUBSTITUTION,
        )?;
        expect(
            &format!("{topology}.numa_claim"),
            &self.topology.numa_claim,
            NUMA_CLAIM,
        )?;

        let execution = "cpu_smp_candidate.execution";
        expect(
            &format!("{execution}.scope"),
            &self.execution.scope,
            EXECUTION_SCOPE,
        )?;
        expect(
            &format!("{execution}.worker_matrix"),
            &self.execution.worker_matrix,
            NATIVE_CORE_SMP_WORKER_MATRIX,
        )?;
        expect(
            &format!("{execution}.requested_above_cap"),
            &self.execution.requested_above_cap,
            REQUESTED_ABOVE_CAP,
        )?;
        expect(
            &format!("{execution}.silent_clamp"),
            &self.execution.silent_clamp,
            SILENT_CLAMP,
        )?;
        expect(
            &format!("{execution}.input_queue"),
            &self.execution.input_queue,
            BOUNDED,
        )?;
        expect(
            &format!("{execution}.result_queue"),
            &self.execution.result_queue,
            BOUNDED,
        )?;
        expect(
            &format!("{execution}.full_queue"),
            &self.execution.full_queue,
            BACKPRESSURE,
        )?;
        expect(
            &format!("{execution}.drop_on_full"),
            &self.execution.drop_on_full,
            DROP_ON_FULL,
        )?;
        expect(
            &format!("{execution}.global_pool"),
            &self.execution.global_pool,
            GLOBAL_POOL,
        )?;
        expect(
            &format!("{execution}.private_worker_scratch"),
            &self.execution.private_worker_scratch,
            PRIVATE_WORKER_SCRATCH,
        )?;
        expect(
            &format!("{execution}.unsafe_project_code"),
            &self.execution.unsafe_project_code,
            UNSAFE_PROJECT_CODE,
        )?;

        let receipts = "cpu_smp_candidate.terminal_receipts";
        expect(
            &format!("{receipts}.admitted_equals_terminal"),
            &self.terminal_receipts.admitted_equals_terminal,
            REQUIRED,
        )?;
        expect(
            &format!("{receipts}.exactly_one_per_admitted_unit"),
            &self.terminal_receipts.exactly_one_per_admitted_unit,
            REQUIRED,
        )?;
        for kind in NATIVE_CORE_SMP_COORDINATOR_FAILURES {
            if !self
                .terminal_receipts
                .failure_vocabulary
                .iter()
                .any(|entry| entry == kind)
            {
                return Err(violation(
                    &format!("{receipts}.failure_vocabulary"),
                    format!("coordinator failure kind `{kind}` is missing"),
                ));
            }
        }
        for kind in &self.terminal_receipts.failure_vocabulary {
            if !CLOSED_FAILURE_VOCABULARY.contains(&kind.as_str()) {
                return Err(violation(
                    &format!("{receipts}.failure_vocabulary"),
                    format!("invented failure kind `{kind}`"),
                ));
            }
        }

        let metrics = "cpu_smp_candidate.metrics";
        if self.metrics.required.is_empty() {
            return Err(violation(
                &format!("{metrics}.required"),
                "an empty required set cannot bind the receipt".to_string(),
            ));
        }
        for metric in &self.metrics.required {
            if !NATIVE_CORE_SMP_METRIC_NAMES.contains(&metric.as_str()) {
                return Err(violation(
                    &format!("{metrics}.required"),
                    format!("metric `{metric}` is not carried by this receipt"),
                ));
            }
        }
        expect(
            &format!("{metrics}.per_worker_terminal_counts"),
            &self.metrics.per_worker_terminal_counts,
            REQUIRED,
        )?;
        expect(
            &format!("{metrics}.per_lane"),
            &self.metrics.per_lane,
            REQUIRED,
        )?;
        expect(
            &format!("{metrics}.cpu_only_excludes_subprocess_io"),
            &self.metrics.cpu_only_excludes_subprocess_io,
            REQUIRED,
        )?;

        if !self.breakpoints.closed {
            return Err(violation(
                "cpu_smp_candidate.breakpoints.closed",
                "the breakpoint vocabulary must stay closed".to_string(),
            ));
        }
        if !self.breakpoints.data_defined {
            return Err(violation(
                "cpu_smp_candidate.breakpoints.data_defined",
                "the breakpoint vocabulary must stay DATA-defined".to_string(),
            ));
        }
        let mut declared = self.breakpoints.verdicts.clone();
        declared.sort();
        declared.dedup();
        let mut expected = NATIVE_CORE_SMP_VERDICTS.to_vec();
        expected.sort();
        if declared != expected {
            return Err(violation(
                "cpu_smp_candidate.breakpoints.verdicts",
                format!(
                    "the closed verdict vocabulary diverged: declared {declared:?}, \
implemented {expected:?}"
                ),
            ));
        }

        let optimization = "cpu_smp_candidate.optimization";
        expect(
            &format!("{optimization}.selection"),
            &self.optimization.selection,
            FORBIDDEN,
        )?;
        expect(
            &format!("{optimization}.scalar_oracle"),
            &self.optimization.scalar_oracle,
            REQUIRED,
        )?;
        if self.optimization.safe_rust_only != SAFE_RUST_ONLY {
            return Err(violation(
                &format!("{optimization}.safe_rust_only"),
                "only safe Rust may be considered".to_string(),
            ));
        }
        for state in ["not_applicable", "measured_rejected", "measured_retained"] {
            if !self.optimization.states.iter().any(|entry| entry == state) {
                return Err(violation(
                    &format!("{optimization}.states"),
                    format!("optimization state `{state}` is missing"),
                ));
            }
        }

        for forbidden in [
            "raw_pdf",
            "raw_text",
            "raw_tei",
            "token_text",
            "physical_path",
            "hostname",
        ] {
            if !self
                .redaction
                .forbidden
                .iter()
                .any(|entry| entry == forbidden)
            {
                return Err(violation(
                    "cpu_smp_candidate.redaction.forbidden",
                    format!("required redaction `{forbidden}` is missing"),
                ));
            }
        }

        let promotion = "cpu_smp_candidate.promotion";
        expect(
            &format!("{promotion}.state"),
            &self.promotion.state,
            CANDIDATE,
        )?;
        expect(
            &format!("{promotion}.selected"),
            &self.promotion.selected,
            FORBIDDEN,
        )?;
        expect(
            &format!("{promotion}.promoted"),
            &self.promotion.promoted,
            FORBIDDEN,
        )?;
        expect(
            &format!("{promotion}.auto_promote"),
            &self.promotion.auto_promote,
            FORBIDDEN,
        )?;
        if self.promotion.rollback_target.is_empty() {
            return Err(violation(
                &format!("{promotion}.rollback_target"),
                "a rollback target must be named".to_string(),
            ));
        }

        let denominators = "cpu_smp_candidate.denominators";
        if !self.denominators.acceptance.fixed {
            return Err(violation(
                &format!("{denominators}.acceptance.fixed"),
                "the acceptance denominator must be fixed, never re-derived from a benchmark"
                    .to_string(),
            ));
        }
        if self.denominators.acceptance.total_lane_pages
            != self.denominators.acceptance.lanes_n * self.denominators.acceptance.pages_per_lane
        {
            return Err(violation(
                &format!("{denominators}.acceptance.total_lane_pages"),
                "the acceptance denominator does not add up".to_string(),
            ));
        }
        if !self.denominators.benchmark.separate_from_acceptance {
            return Err(violation(
                &format!("{denominators}.benchmark.separate_from_acceptance"),
                "the benchmark denominator must stay separate from acceptance".to_string(),
            ));
        }
        expect(
            &format!("{denominators}.benchmark.pooled"),
            &self.denominators.benchmark.pooled,
            FORBIDDEN,
        )?;
        Ok(())
    }

    /// The declared memory ceiling, if any. `None` means `memory_pressure` is
    /// not observable and must be reported as `not_observed`.
    pub fn memory_ceiling_bytes(&self) -> Option<u64> {
        self.breakpoints.memory_ceiling_bytes
    }
}

const CLOSED_FAILURE_VOCABULARY: [&str; 7] = [
    "topology_unavailable",
    "invalid_config",
    "over_core_cap",
    "worker_panic",
    "channel_closed",
    "interrupted",
    "semantic_mismatch",
];

// ─────────────────────── typed observation ───────────────────────

/// A measurement or its typed absence. A metric that was not measured is never
/// serialized as a fabricated zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SmpObservation<T> {
    Observed { value: T },
    NotObserved { reason: String },
}

impl<T> SmpObservation<T> {
    pub fn observed(value: T) -> Self {
        Self::Observed { value }
    }
    pub fn not_observed(reason: impl Into<String>) -> Self {
        Self::NotObserved {
            reason: reason.into(),
        }
    }
    pub fn is_observed(&self) -> bool {
        matches!(self, Self::Observed { .. })
    }
}

// ─────────────────────────── breakpoints ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmpBreakpointVerdict {
    NotObserved,
    Saturation,
    FirstRegression,
    QueuePressure,
    MemoryPressure,
    Inconclusive,
}

impl SmpBreakpointVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotObserved => "not_observed",
            Self::Saturation => "saturation",
            Self::FirstRegression => "first_regression",
            Self::QueuePressure => "queue_pressure",
            Self::MemoryPressure => "memory_pressure",
            Self::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakpointVerdictRecord {
    pub workers: usize,
    pub verdict: SmpBreakpointVerdict,
    pub reason: String,
}

// ─────────────────────── workload and jobs ───────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCoreSmpConfig {
    pub documents_per_family: usize,
    pub rows_per_document: usize,
    pub repetitions: usize,
}

impl NativeCoreSmpConfig {
    pub fn new(documents_per_family: usize, rows_per_document: usize, repetitions: usize) -> Self {
        Self {
            documents_per_family,
            rows_per_document,
            repetitions,
        }
    }

    fn validate(&self) -> Result<(), NativeCoreSmpError> {
        for (name, value) in [
            ("documents_per_family", self.documents_per_family),
            ("rows_per_document", self.rows_per_document),
            ("repetitions", self.repetitions),
        ] {
            if value == 0 {
                return Err(NativeCoreSmpError::NonPositive {
                    name: name.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// A frozen synthetic family: the model bytes (parsed exactly once per matrix
/// run, outside every job) plus its deterministic row set.
#[derive(Debug, Clone)]
pub struct NativeCoreSmpFamily {
    pub family: String,
    pub model_bytes: Arc<[u8]>,
    pub model_sha256: String,
    pub model_size_bytes: usize,
    pub rows: Arc<Vec<String>>,
}

impl NativeCoreSmpFamily {
    /// Build a family from frozen model bytes. Rows come from the model's own
    /// observation table through [`derive_rows`], so the workload is
    /// model-derived, deterministic, and never document content.
    pub fn from_frozen_model(
        family: &str,
        model_bytes: &[u8],
        rows_per_family: usize,
    ) -> Result<Self, NativeCoreSmpError> {
        let model =
            CrfModel::parse(model_bytes).map_err(|error| NativeCoreSmpError::ModelParse {
                family: family.to_string(),
                reason: error.to_string(),
            })?;
        let rows = derive_rows(&model, rows_per_family).map_err(|error| {
            NativeCoreSmpError::RowDerivation {
                family: family.to_string(),
                reason: error.to_string(),
            }
        })?;
        Ok(Self {
            family: family.to_string(),
            model_bytes: Arc::from(model_bytes),
            model_sha256: format!("sha256:{:x}", Sha256::digest(model_bytes)),
            model_size_bytes: model_bytes.len(),
            rows: Arc::new(rows),
        })
    }
}

/// One deterministic unit of native core work: a family × document pair.
#[derive(Debug, Clone)]
pub struct NativeCoreSmpJob {
    pub family_index: usize,
    pub family: String,
    pub document: usize,
    pub rows: Arc<Vec<String>>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct NativeCoreSmpJobSet {
    pub jobs: Vec<NativeCoreSmpJob>,
    pub scratch_capacity: usize,
    pub documents_per_family: usize,
    pub rows_per_document: usize,
}

fn rendered_len(rows: &[String]) -> usize {
    rows.iter().map(|row| row.len() + 1).sum()
}

/// Build the family × document job set and size the private scratch arena so
/// that one document fits without ever growing outside the arena.
pub fn build_native_core_jobs(
    families: &[NativeCoreSmpFamily],
    config: &NativeCoreSmpConfig,
) -> Result<NativeCoreSmpJobSet, NativeCoreSmpError> {
    config.validate()?;
    if families.is_empty() {
        return Err(NativeCoreSmpError::EmptyFamilySet);
    }
    let required = config.documents_per_family * config.rows_per_document;
    let mut jobs = Vec::with_capacity(families.len() * config.documents_per_family);
    let mut scratch_capacity = 1usize;
    for (family_index, family) in families.iter().enumerate() {
        if family.rows.len() < required {
            return Err(NativeCoreSmpError::WorkloadTooSmall {
                family: family.family.clone(),
                rows: family.rows.len(),
                required,
            });
        }
        for document in 0..config.documents_per_family {
            let start = document * config.rows_per_document;
            let end = start + config.rows_per_document;
            scratch_capacity = scratch_capacity.max(rendered_len(&family.rows[start..end]));
            jobs.push(NativeCoreSmpJob {
                family_index,
                family: family.family.clone(),
                document,
                rows: family.rows.clone(),
                start,
                end,
            });
        }
    }
    Ok(NativeCoreSmpJobSet {
        jobs,
        scratch_capacity,
        documents_per_family: config.documents_per_family,
        rows_per_document: config.rows_per_document,
    })
}

// ─────────────────────────── job execution ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmpJobError {
    Scratch {
        family: String,
        document: usize,
        requested: usize,
        remaining: usize,
    },
    Kernel {
        family: String,
        document: usize,
        reason: String,
    },
}

impl SmpJobError {
    /// The family the failing job belonged to, so the failure is never anonymous.
    pub fn family(&self) -> &str {
        match self {
            Self::Scratch { family, .. } | Self::Kernel { family, .. } => family,
        }
    }

    /// The document (the job's ordinal within its family) that failed.
    pub fn document(&self) -> usize {
        match self {
            Self::Scratch { document, .. } | Self::Kernel { document, .. } => *document,
        }
    }
}

impl fmt::Display for SmpJobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scratch {
                family,
                document,
                requested,
                remaining,
            } => write!(
                f,
                "private scratch exhausted for {family}/{document}: requested {requested}, \
remaining {remaining}"
            ),
            Self::Kernel {
                family,
                document,
                reason,
            } => write!(f, "native CRF kernel refused {family}/{document}: {reason}"),
        }
    }
}

impl std::error::Error for SmpJobError {}

/// Redacted per-job receipt. `duration_ms` is observational; `digest` covers
/// only counts, label ids and score bit patterns, so it is deterministic and
/// leaks no token text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCoreSmpJobReceipt {
    pub family: String,
    pub document: usize,
    pub rows: usize,
    pub positions: usize,
    pub matched_observations: usize,
    pub labels: usize,
    pub digest: String,
    pub duration_ms: u128,
}

/// Render one document into the private arena and run the real native kernel.
///
/// The model is parsed once per family by [`native_core_smp_processor`]; this
/// function never parses a model and never touches the filesystem.
pub fn execute_native_core_job(
    model: &CrfModel,
    job: &NativeCoreSmpJob,
    scratch: &mut ScratchArena,
) -> Result<NativeCoreSmpJobReceipt, SmpJobError> {
    let rows = &job.rows[job.start..job.end];
    let rendered = rendered_len(rows);
    let buffer = scratch.allocate(rendered).map_err(|error| match error {
        crate::layout_runtime::ScratchError::Exhausted {
            requested,
            remaining,
        } => SmpJobError::Scratch {
            family: job.family.clone(),
            document: job.document,
            requested,
            remaining,
        },
    })?;
    let mut cursor = 0usize;
    for row in rows {
        buffer[cursor..cursor + row.len()].copy_from_slice(row.as_bytes());
        cursor += row.len();
        buffer[cursor] = b'\n';
        cursor += 1;
    }
    let text = std::str::from_utf8(&buffer[..cursor]).map_err(|error| SmpJobError::Kernel {
        family: job.family.clone(),
        document: job.document,
        reason: format!("rendered rows were not UTF-8: {error}"),
    })?;

    let started = Instant::now();
    let positions = map_input(model, text).map_err(|error| SmpJobError::Kernel {
        family: job.family.clone(),
        document: job.document,
        reason: error.to_string(),
    })?;
    let decoded = viterbi_decode(model, &positions).map_err(|error| SmpJobError::Kernel {
        family: job.family.clone(),
        document: job.document,
        reason: error.to_string(),
    })?;
    let duration_ms = started.elapsed().as_millis();

    // Redacted semantic bytes: counts, label ids and score bit patterns only.
    let uobs: u64 = positions.iter().map(|row| row.uobs.len() as u64).sum();
    let bobs: u64 = positions.iter().map(|row| row.bobs.len() as u64).sum();
    let mut receipt = Vec::with_capacity(32 + decoded.label_ids.len() * 24);
    receipt.extend_from_slice(&(positions.len() as u64).to_le_bytes());
    receipt.extend_from_slice(&(uobs + bobs).to_le_bytes());
    receipt.extend_from_slice(&(decoded.label_ids.len() as u64).to_le_bytes());
    for label in &decoded.label_ids {
        receipt.extend_from_slice(&(*label as u64).to_le_bytes());
    }
    for score in &decoded.scores {
        receipt.extend_from_slice(&score.to_bits().to_le_bytes());
    }
    Ok(NativeCoreSmpJobReceipt {
        family: job.family.clone(),
        document: job.document,
        rows: rows.len(),
        positions: positions.len(),
        matched_observations: (uobs + bobs) as usize,
        labels: decoded.label_ids.len(),
        digest: format!("sha256:{:x}", Sha256::digest(&receipt)),
        duration_ms,
    })
}

pub type SmpJobProcessor = Arc<
    dyn Fn(NativeCoreSmpJob, &mut ScratchArena) -> Result<NativeCoreSmpJobReceipt, SmpJobError>
        + Send
        + Sync,
>;

/// Build the production job processor: every frozen model is parsed exactly
/// once per family, outside every job and outside the worker threads.
pub fn native_core_smp_processor(
    families: &[NativeCoreSmpFamily],
) -> Result<SmpJobProcessor, NativeCoreSmpError> {
    if families.is_empty() {
        return Err(NativeCoreSmpError::EmptyFamilySet);
    }
    let mut models = Vec::with_capacity(families.len());
    for family in families {
        let model = CrfModel::parse(&family.model_bytes).map_err(|error| {
            NativeCoreSmpError::ModelParse {
                family: family.family.clone(),
                reason: error.to_string(),
            }
        })?;
        models.push(Arc::new(model));
    }
    let models = Arc::new(models);
    Ok(Arc::new(
        move |job: NativeCoreSmpJob, scratch: &mut ScratchArena| {
            execute_native_core_job(&models[job.family_index], &job, scratch)
        },
    ))
}

// ─────────────────────── runtime configuration ───────────────────────

/// Build and validate the bounded runtime configuration for one worker count.
///
/// The requested worker count is passed through verbatim: a value above the
/// physical core cap is refused here, before any thread is spawned, and is
/// never clamped to the cap.
pub fn native_core_smp_runtime_config(
    topology: &TopologyReceipt,
    requested_workers: usize,
    private_scratch_capacity: usize,
) -> Result<RuntimeConfig, NativeCoreSmpError> {
    let config = RuntimeConfig {
        workers: requested_workers,
        input_queue_capacity: requested_workers,
        result_queue_capacity: requested_workers,
        max_in_flight: requested_workers,
        private_scratch_capacity,
    };
    config.validate(topology).map_err(|error| match error {
        ConfigError::OverCoreCap { requested, cap } => {
            NativeCoreSmpError::OverCoreCap { requested, cap }
        }
        ConfigError::NonPositive(name) => NativeCoreSmpError::Config {
            reason: format!("{name} must be positive"),
        },
    })?;
    Ok(config)
}

// ─────────────────────── worker matrix entry ───────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerWorkerTerminalCounts {
    pub worker: usize,
    pub completed: usize,
    pub failed: usize,
    pub coordinator_failed: usize,
}

impl PerWorkerTerminalCounts {
    fn for_worker(worker: usize) -> Self {
        Self {
            worker,
            completed: 0,
            failed: 0,
            coordinator_failed: 0,
        }
    }

    fn total(&self) -> usize {
        self.completed + self.failed + self.coordinator_failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpQueueObservation {
    pub input_capacity: usize,
    pub result_capacity: usize,
    pub max_in_flight: usize,
    pub input_high_water: usize,
    pub result_high_water: usize,
    pub in_flight_high_water: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpCoordinatorFailureRecord {
    pub kind: String,
    pub worker: usize,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SmpKernelPercentiles {
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerMatrixEntry {
    pub workers: usize,
    pub repetitions: usize,
    pub jobs: usize,
    pub admitted: usize,
    pub terminals: usize,
    pub ordinals: Vec<usize>,
    pub completed: usize,
    pub failed: usize,
    pub coordinator_failed: usize,
    pub per_worker_terminal_counts: Vec<PerWorkerTerminalCounts>,
    pub semantic_digest: String,
    pub scalar_oracle_digest: String,
    pub semantic_equal_to_scalar: bool,
    pub rows_decoded: u64,
    pub decoded_positions: u64,
    pub queue: SmpQueueObservation,
    /// Declared by DATA, not instrumented by the shared bounded runtime: typed
    /// absence, never a fabricated number.
    pub queue_wait_ms: SmpObservation<u64>,
    pub scratch_capacity_bytes: usize,
    pub scratch_high_water_bytes: usize,
    pub per_worker_scratch_high_water_bytes: Vec<usize>,
    /// Observational: pooled per-job kernel durations across repetitions.
    pub latency_ms: SmpKernelPercentiles,
    /// Observational: wall-clock of the whole bounded run, summed over repetitions.
    pub run_wall_ms: u128,
    /// Observational: decoded positions per wall-clock second.
    pub throughput_units_per_second: u64,
    pub cpu_only: bool,
    pub subprocess_io_included: bool,
    pub coordinator_failure: Option<SmpCoordinatorFailureRecord>,
}

/// One bounded run at one worker count (a single repetition).
#[derive(Debug, Clone)]
struct SingleRun {
    workers: usize,
    jobs: usize,
    admitted: usize,
    ordinals: Vec<usize>,
    completed: usize,
    failed: usize,
    coordinator_failed: usize,
    per_worker: Vec<PerWorkerTerminalCounts>,
    semantic_digest: String,
    rows_decoded: u64,
    decoded_positions: u64,
    queue: SmpQueueObservation,
    scratch_capacity_bytes: usize,
    per_worker_scratch_high_water_bytes: Vec<usize>,
    latency_samples: Vec<u128>,
    run_wall_ms: u128,
    coordinator_failure: Option<SmpCoordinatorFailureRecord>,
}

fn coordinator_kind(failure: &CoordinatorFailure) -> &'static str {
    match failure {
        CoordinatorFailure::WorkerPanic { .. } => "worker_panic",
        CoordinatorFailure::ChannelClosed { .. } => "channel_closed",
        CoordinatorFailure::Interrupted => "interrupted",
    }
}

fn coordinator_channel(failure: &CoordinatorFailure) -> Option<&'static str> {
    match failure {
        CoordinatorFailure::ChannelClosed { channel } => Some(channel),
        CoordinatorFailure::WorkerPanic { .. } | CoordinatorFailure::Interrupted => None,
    }
}

fn percentile(sorted: &[u128], rank: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) * rank) / 100]
}

fn validate_failure_kind(data: &CpuSmpCandidateData, kind: &str) -> Result<(), NativeCoreSmpError> {
    if !data
        .terminal_receipts
        .failure_vocabulary
        .iter()
        .any(|entry| entry == kind)
    {
        return Err(NativeCoreSmpError::VocabularyViolation {
            path: "cpu_smp_candidate.terminal_receipts.failure_vocabulary".to_string(),
            value: kind.to_string(),
        });
    }
    Ok(())
}

fn run_once(
    topology: &TopologyReceipt,
    requested_workers: usize,
    data: &CpuSmpCandidateData,
    job_set: &NativeCoreSmpJobSet,
    processor: &SmpJobProcessor,
) -> Result<SingleRun, NativeCoreSmpError> {
    let runtime =
        native_core_smp_runtime_config(topology, requested_workers, job_set.scratch_capacity)?;
    let process = {
        let processor = processor.clone();
        move |job: NativeCoreSmpJob, scratch: &mut ScratchArena| processor(job, scratch)
    };
    let started = Instant::now();
    let report = run_bounded(runtime, topology, job_set.jobs.clone(), process).map_err(
        |error| match error {
            ConfigError::OverCoreCap { requested, cap } => {
                NativeCoreSmpError::OverCoreCap { requested, cap }
            }
            ConfigError::NonPositive(name) => NativeCoreSmpError::Config {
                reason: format!("{name} must be positive"),
            },
        },
    )?;
    let run_wall_ms = started.elapsed().as_millis();

    let mut ordinals = Vec::with_capacity(report.terminals.len());
    let mut per_worker: BTreeMap<usize, PerWorkerTerminalCounts> = BTreeMap::new();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut coordinator_failed = 0usize;
    let mut digests: Vec<(usize, String)> = Vec::with_capacity(report.terminals.len());
    let mut rows_decoded = 0u64;
    let mut decoded_positions = 0u64;
    let mut latency_samples: Vec<u128> = Vec::new();
    let mut coordinator_failure: Option<SmpCoordinatorFailureRecord> = None;
    let mut first_job_failure: Option<(String, usize, String)> = None;

    for terminal in &report.terminals {
        ordinals.push(terminal.ordinal());
        match terminal {
            Terminal::Completed {
                ordinal,
                worker,
                result,
            } => {
                completed += 1;
                per_worker
                    .entry(*worker)
                    .or_insert_with(|| PerWorkerTerminalCounts::for_worker(*worker))
                    .completed += 1;
                rows_decoded += result.rows as u64;
                decoded_positions += result.positions as u64;
                latency_samples.push(result.duration_ms);
                digests.push((*ordinal, result.digest.clone()));
            }
            Terminal::Failed { worker, error, .. } => {
                failed += 1;
                per_worker
                    .entry(*worker)
                    .or_insert_with(|| PerWorkerTerminalCounts::for_worker(*worker))
                    .failed += 1;
                first_job_failure.get_or_insert_with(|| {
                    (
                        error.family().to_string(),
                        error.document(),
                        format!(
                            "job failed through the bounded runtime on worker {worker}: {error}"
                        ),
                    )
                });
            }
            Terminal::CoordinatorFailed {
                worker, failure, ..
            } => {
                coordinator_failed += 1;
                per_worker
                    .entry(*worker)
                    .or_insert_with(|| PerWorkerTerminalCounts::for_worker(*worker))
                    .coordinator_failed += 1;
                let kind = coordinator_kind(failure);
                validate_failure_kind(data, kind)?;
                coordinator_failure.get_or_insert(SmpCoordinatorFailureRecord {
                    kind: kind.to_string(),
                    worker: *worker,
                    channel: coordinator_channel(failure).map(str::to_string),
                });
            }
        }
    }

    digests.sort_by_key(|(ordinal, _)| *ordinal);
    let encoded = digests
        .iter()
        .map(|(ordinal, digest)| format!("{ordinal}:{digest}"))
        .collect::<Vec<_>>()
        .join("\n");
    let semantic_digest = format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()));

    latency_samples.sort_unstable();

    let per_worker_scratch = report.private_scratch_high_water.clone();

    if let Some((family, document, reason)) = first_job_failure {
        return Err(NativeCoreSmpError::JobFailed {
            workers: requested_workers,
            family,
            document,
            reason,
        });
    }

    Ok(SingleRun {
        workers: requested_workers,
        jobs: job_set.jobs.len(),
        admitted: report.admitted,
        ordinals,
        completed,
        failed,
        coordinator_failed,
        per_worker: per_worker.into_values().collect(),
        semantic_digest,
        rows_decoded,
        decoded_positions,
        queue: SmpQueueObservation {
            input_capacity: runtime.input_queue_capacity,
            result_capacity: runtime.result_queue_capacity,
            max_in_flight: runtime.max_in_flight,
            input_high_water: report.metrics.input_queue_high_water,
            result_high_water: report.metrics.result_queue_high_water,
            in_flight_high_water: report.metrics.in_flight_high_water,
        },
        scratch_capacity_bytes: runtime.private_scratch_capacity,
        per_worker_scratch_high_water_bytes: per_worker_scratch,
        latency_samples,
        run_wall_ms,
        coordinator_failure,
    })
}

fn merge_repetitions(runs: Vec<SingleRun>) -> Result<WorkerMatrixEntry, NativeCoreSmpError> {
    let first = runs
        .first()
        .cloned()
        .ok_or(NativeCoreSmpError::NonPositive {
            name: "repetitions".to_string(),
        })?;
    let workers = first.workers;
    for run in runs.iter().skip(1) {
        let checks: [(&str, bool); 6] = [
            ("admitted", run.admitted == first.admitted),
            ("ordinals", run.ordinals == first.ordinals),
            (
                "semantic_digest",
                run.semantic_digest == first.semantic_digest,
            ),
            ("completed", run.completed == first.completed),
            ("failed", run.failed == first.failed),
            (
                "per_worker_terminal_counts",
                run.per_worker == first.per_worker,
            ),
        ];
        for (field, equal) in checks {
            if !equal {
                return Err(NativeCoreSmpError::NonDeterministicWorkload {
                    workers,
                    field: field.to_string(),
                });
            }
        }
    }

    let terminals = first.ordinals.len();
    if first.admitted != terminals {
        return Err(NativeCoreSmpError::TerminalConservation {
            workers,
            admitted: first.admitted,
            terminals,
        });
    }
    let expected_ordinals = (0..terminals).collect::<Vec<_>>();
    if first.ordinals != expected_ordinals {
        return Err(NativeCoreSmpError::TerminalConservation {
            workers,
            admitted: first.admitted,
            terminals,
        });
    }

    let mut latency_samples: Vec<u128> = runs
        .iter()
        .flat_map(|run| run.latency_samples.iter().copied())
        .collect();
    latency_samples.sort_unstable();
    let run_wall_ms: u128 = runs.iter().map(|run| run.run_wall_ms).sum();
    let decoded_positions: u64 = first.decoded_positions;
    let throughput = (decoded_positions as u128 * 1000)
        .checked_div(run_wall_ms)
        .unwrap_or(0)
        .min(u64::MAX as u128) as u64;

    let mut per_worker_scratch = first.per_worker_scratch_high_water_bytes.clone();
    let mut queue = first.queue;
    for run in runs.iter().skip(1) {
        for (slot, value) in per_worker_scratch
            .iter_mut()
            .zip(run.per_worker_scratch_high_water_bytes.iter().copied())
        {
            *slot = (*slot).max(value);
        }
        queue.input_high_water = queue.input_high_water.max(run.queue.input_high_water);
        queue.result_high_water = queue.result_high_water.max(run.queue.result_high_water);
        queue.in_flight_high_water = queue
            .in_flight_high_water
            .max(run.queue.in_flight_high_water);
    }
    let scratch_high_water_bytes = per_worker_scratch.iter().copied().max().unwrap_or(0);

    Ok(WorkerMatrixEntry {
        workers,
        repetitions: runs.len(),
        jobs: first.jobs,
        admitted: first.admitted,
        terminals,
        ordinals: first.ordinals,
        completed: first.completed,
        failed: first.failed,
        coordinator_failed: first.coordinator_failed,
        per_worker_terminal_counts: first.per_worker,
        semantic_digest: first.semantic_digest,
        scalar_oracle_digest: String::new(),
        semantic_equal_to_scalar: false,
        rows_decoded: first.rows_decoded,
        decoded_positions,
        queue,
        queue_wait_ms: SmpObservation::not_observed(
            "the shared bounded runtime publishes queue high-water gauges; per-unit queue \
residence is not instrumented and is never fabricated",
        ),
        scratch_capacity_bytes: first.scratch_capacity_bytes,
        scratch_high_water_bytes,
        per_worker_scratch_high_water_bytes: per_worker_scratch,
        latency_ms: SmpKernelPercentiles {
            p50_ms: percentile(&latency_samples, 50),
            p95_ms: percentile(&latency_samples, 95),
            p99_ms: percentile(&latency_samples, 99),
        },
        run_wall_ms,
        throughput_units_per_second: throughput,
        cpu_only: true,
        subprocess_io_included: false,
        coordinator_failure: first.coordinator_failure,
    })
}

/// Run one worker count `config.repetitions` times and aggregate it.
///
/// The deterministic fields must be identical across repetitions or the call
/// fails with [`NativeCoreSmpError::NonDeterministicWorkload`].
pub fn run_smp_worker_count(
    topology: &TopologyReceipt,
    requested_workers: usize,
    data: &CpuSmpCandidateData,
    job_set: &NativeCoreSmpJobSet,
    config: &NativeCoreSmpConfig,
    processor: &SmpJobProcessor,
) -> Result<WorkerMatrixEntry, NativeCoreSmpError> {
    config.validate()?;
    let mut runs = Vec::with_capacity(config.repetitions);
    for _ in 0..config.repetitions {
        runs.push(run_once(
            topology,
            requested_workers,
            data,
            job_set,
            processor,
        )?);
    }
    merge_repetitions(runs)
}

// ─────────────────────────── breakpoints ───────────────────────────

/// Classify each worker count into the closed breakpoint vocabulary.
///
/// Rules, in precedence order, per worker count:
///
/// 1. `memory_pressure` — an observed process peak RSS exceeded a ceiling that
///    DATA declares (no ceiling is declared today, so this is reported as
///    `not_observed` rather than guessed);
/// 2. `inconclusive` — a coordinator failure, or a throughput observation that
///    was not made at all;
/// 3. `not_observed` — the serial baseline, or a step that still gained;
/// 4. `first_regression` — throughput fell below the immediately preceding step;
/// 5. `queue_pressure` — throughput did not grow *and* the bounded queues stayed
///    at their capacity, so the plateau is attributable to admission
///    backpressure rather than to worker saturation;
/// 6. `saturation` — throughput did not grow with the queue below capacity.
///
/// Throughput is compared exactly, because no gain threshold is declared in
/// DATA: inventing one here would be an undeclared acceptance rule.
pub fn derive_smp_breakpoints(
    data: &CpuSmpCandidateData,
    entries: &[WorkerMatrixEntry],
    peak_rss: &SmpObservation<u64>,
) -> Result<Vec<BreakpointVerdictRecord>, NativeCoreSmpError> {
    let mut records = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let verdict = classify_breakpoint(data, entries, index, entry, peak_rss);
        if !data
            .breakpoints
            .verdicts
            .iter()
            .any(|declared| declared == verdict.0.as_str())
        {
            return Err(NativeCoreSmpError::VocabularyViolation {
                path: NATIVE_CORE_SMP_BREAKPOINT_SOURCE.to_string(),
                value: verdict.0.as_str().to_string(),
            });
        }
        records.push(BreakpointVerdictRecord {
            workers: entry.workers,
            verdict: verdict.0,
            reason: verdict.1,
        });
    }
    Ok(records)
}

fn classify_breakpoint(
    data: &CpuSmpCandidateData,
    entries: &[WorkerMatrixEntry],
    index: usize,
    entry: &WorkerMatrixEntry,
    peak_rss: &SmpObservation<u64>,
) -> (SmpBreakpointVerdict, String) {
    let over_ceiling = match (data.memory_ceiling_bytes(), peak_rss) {
        (Some(ceiling), SmpObservation::Observed { value }) if *value > ceiling => {
            Some((*value, ceiling))
        }
        _ => None,
    };
    if let Some((value, ceiling)) = over_ceiling {
        return (
            SmpBreakpointVerdict::MemoryPressure,
            format!("process peak RSS {value} exceeds the declared ceiling {ceiling}"),
        );
    }
    if entry.coordinator_failure.is_some() {
        return (
            SmpBreakpointVerdict::Inconclusive,
            "a coordinator failure makes this worker count unmeasurable".to_string(),
        );
    }
    if entry.throughput_units_per_second == 0 {
        return (
            SmpBreakpointVerdict::Inconclusive,
            "throughput was not observed for this worker count".to_string(),
        );
    }
    if index == 0 {
        return (
            SmpBreakpointVerdict::NotObserved,
            "serial baseline: there is no lower worker count to compare against".to_string(),
        );
    }
    let previous = &entries[index - 1];
    let previous_throughput = previous.throughput_units_per_second;
    let observed = entry.throughput_units_per_second;
    if observed < previous_throughput {
        return (
            SmpBreakpointVerdict::FirstRegression,
            format!("throughput fell from {previous_throughput} to {observed}"),
        );
    }
    if observed == previous_throughput {
        if entry.queue.input_high_water >= entry.queue.input_capacity
            || entry.queue.result_high_water >= entry.queue.result_capacity
        {
            return (
                SmpBreakpointVerdict::QueuePressure,
                format!(
                    "admission-bound plateau at {observed}/s while the queue stayed at capacity \
(input {}/{}, result {}/{})",
                    entry.queue.input_high_water,
                    entry.queue.input_capacity,
                    entry.queue.result_high_water,
                    entry.queue.result_capacity
                ),
            );
        }
        return (
            SmpBreakpointVerdict::Saturation,
            format!(
                "throughput stayed at {previous_throughput} while worker count grew from {} to {}",
                previous.workers, entry.workers
            ),
        );
    }
    (
        SmpBreakpointVerdict::NotObserved,
        format!("throughput still grew from {previous_throughput} to {observed}"),
    )
}

// ─────────────────────────── matrix ───────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmpMatrix {
    pub scalar_oracle_digest: String,
    pub entries: Vec<WorkerMatrixEntry>,
    pub breakpoints: Vec<BreakpointVerdictRecord>,
    pub breakpoint_vocabulary: Vec<String>,
}

/// Validate and combine the per-worker-count entries into one matrix.
///
/// This is the acceptance surface: the serial `workers = 1` entry is the
/// scalar oracle, every other entry must reproduce its semantic digest, every
/// entry must conserve admitted/terminal/ordinals, and no coordinator failure
/// or job failure may be recorded.
pub fn combine_smp_matrix(
    data: &CpuSmpCandidateData,
    mut entries: Vec<WorkerMatrixEntry>,
    peak_rss: &SmpObservation<u64>,
) -> Result<SmpMatrix, NativeCoreSmpError> {
    if entries.is_empty() {
        return Err(NativeCoreSmpError::EmptyFamilySet);
    }
    if entries[0].workers != 1 {
        return Err(NativeCoreSmpError::DataContractViolation {
            path: "smp_matrix.entries[0].workers".to_string(),
            reason: format!(
                "the serial oracle must be the first entry, got {} workers",
                entries[0].workers
            ),
        });
    }
    let scalar_oracle_digest = entries[0].semantic_digest.clone();
    for entry in entries.iter_mut() {
        if entry.admitted != entry.terminals || entry.ordinals.len() != entry.terminals {
            return Err(NativeCoreSmpError::TerminalConservation {
                workers: entry.workers,
                admitted: entry.admitted,
                terminals: entry.terminals,
            });
        }
        let accounted: usize = entry
            .per_worker_terminal_counts
            .iter()
            .map(PerWorkerTerminalCounts::total)
            .sum();
        if accounted != entry.terminals {
            return Err(NativeCoreSmpError::PerWorkerConservation {
                workers: entry.workers,
                terminals: entry.terminals,
                accounted,
            });
        }
        if entry.failed != 0 {
            return Err(NativeCoreSmpError::JobFailed {
                workers: entry.workers,
                family: "unknown".to_string(),
                document: 0,
                reason: format!("{} job failures were recorded", entry.failed),
            });
        }
        if let Some(failure) = &entry.coordinator_failure {
            validate_failure_kind(data, &failure.kind)?;
            return Err(NativeCoreSmpError::CoordinatorFailed {
                workers: entry.workers,
                kind: failure.kind.clone(),
                worker: failure.worker,
            });
        }
        if entry.semantic_digest != scalar_oracle_digest {
            return Err(NativeCoreSmpError::SemanticDivergence {
                workers: entry.workers,
                expected: scalar_oracle_digest.clone(),
                observed: entry.semantic_digest.clone(),
            });
        }
        entry.scalar_oracle_digest = scalar_oracle_digest.clone();
        entry.semantic_equal_to_scalar = true;
    }
    let breakpoints = derive_smp_breakpoints(data, &entries, peak_rss)?;
    Ok(SmpMatrix {
        scalar_oracle_digest,
        entries,
        breakpoints,
        breakpoint_vocabulary: data.breakpoints.verdicts.clone(),
    })
}

// ─────────────────────────── receipt ───────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpTopologyObservation {
    pub source: String,
    pub allowed_cpus: usize,
    pub physical_cores: usize,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreSmpFamilyBinding {
    pub family: String,
    pub model_sha256: String,
    pub model_size_bytes: usize,
    pub rows: usize,
    pub rows_per_document: usize,
    pub documents: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreSmpWorkloadBinding {
    pub documents_per_family: usize,
    pub rows_per_document: usize,
    pub jobs: usize,
    pub repetitions: usize,
    pub scratch_capacity_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCoreSmpObservationalBlock {
    pub disposition: String,
    pub note: String,
    /// Wall-clock fields: they classify a worker count, never accept one.
    pub timing_fields: Vec<String>,
    /// Runtime-observed structural gauges, also outside acceptance.
    pub runtime_observed_fields: Vec<String>,
    pub excluded_from: Vec<String>,
    pub acceptance_denominator: CpuSmpAcceptanceDenominator,
    pub benchmark_denominator: CpuSmpBenchmarkPolicy,
    pub benchmark_repetitions_per_worker_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreSmpReceipt {
    pub data: CpuSmpCandidateData,
    pub topology: SmpTopologyObservation,
    pub families: Vec<NativeCoreSmpFamilyBinding>,
    pub workload: NativeCoreSmpWorkloadBinding,
    pub matrix: SmpMatrix,
    pub peak_rss_bytes: SmpObservation<u64>,
    pub memory_pressure_ceiling_bytes: SmpObservation<u64>,
    pub cpu_only: bool,
    pub subprocess_io_included: bool,
    pub selection_forbidden: bool,
    pub observations: NativeCoreSmpObservationalBlock,
}

impl NativeCoreSmpReceipt {
    /// Every worker count in the matrix, in sweep order.
    pub fn worker_matrix(&self) -> Vec<usize> {
        self.matrix
            .entries
            .iter()
            .map(|entry| entry.workers)
            .collect()
    }

    /// Deterministic projection: everything an independent run must reproduce.
    ///
    /// Timing, throughput, the runtime gauges *and the breakpoint verdicts*
    /// are deliberately absent: the verdicts are classifications of the
    /// observational throughput, so including them would make a replay claim
    /// that this slice cannot honestly make.
    pub fn deterministic_projection(&self) -> Result<String, NativeCoreSmpError> {
        let projection = NativeCoreSmpDeterministicProjection {
            schema_version: NATIVE_CORE_SMP_SCHEMA_VERSION.to_string(),
            manifest_binding: self.data.manifest_binding.clone(),
            workload_jobs: self.workload.jobs,
            families: self
                .families
                .iter()
                .map(|family| DeterministicFamily {
                    family: family.family.clone(),
                    model_sha256: family.model_sha256.clone(),
                    rows: family.rows,
                    documents: family.documents,
                })
                .collect(),
            scalar_oracle_digest: self.matrix.scalar_oracle_digest.clone(),
            entries: self
                .matrix
                .entries
                .iter()
                .map(|entry| DeterministicEntry {
                    workers: entry.workers,
                    jobs: entry.jobs,
                    admitted: entry.admitted,
                    terminals: entry.terminals,
                    ordinals: entry.ordinals.clone(),
                    completed: entry.completed,
                    failed: entry.failed,
                    coordinator_failed: entry.coordinator_failed,
                    per_worker_terminal_counts: entry.per_worker_terminal_counts.clone(),
                    semantic_digest: entry.semantic_digest.clone(),
                    rows_decoded: entry.rows_decoded,
                    decoded_positions: entry.decoded_positions,
                    scratch_capacity_bytes: entry.scratch_capacity_bytes,
                    scratch_high_water_bytes: entry.scratch_high_water_bytes,
                    per_worker_scratch_high_water_bytes: entry
                        .per_worker_scratch_high_water_bytes
                        .clone(),
                })
                .collect(),
        };
        serde_json::to_string(&projection).map_err(|error| NativeCoreSmpError::Artifact {
            reason: error.to_string(),
        })
    }

    pub fn artifact(&self) -> NativeCoreSmpArtifact {
        NativeCoreSmpArtifact {
            schema_version: NATIVE_CORE_SMP_SCHEMA_VERSION.to_string(),
            adr_ref: NATIVE_CORE_SMP_ADR_REF.to_string(),
            candidate_schema_version: self.data.schema_version.clone(),
            manifest_binding: NATIVE_CORE_SMP_MANIFEST_BINDING.to_string(),
            candidate_manifest_binding: self.data.manifest_binding.clone(),
            requirement_ids: self.data.requirement_ids.clone(),
            topology: self.topology.clone(),
            workload: self.workload.clone(),
            families: self.families.clone(),
            matrix: self.matrix.clone(),
            peak_rss_bytes: self.peak_rss_bytes.clone(),
            memory_pressure_ceiling_bytes: self.memory_pressure_ceiling_bytes.clone(),
            cpu_only: self.cpu_only,
            subprocess_io_included: self.subprocess_io_included,
            selection_forbidden: self.selection_forbidden,
            metrics: SmpArtifactMetrics {
                required: self.data.metrics.required.clone(),
                per_worker_terminal_counts: self.data.metrics.per_worker_terminal_counts.clone(),
                per_lane: self.data.metrics.per_lane.clone(),
                cpu_only_excludes_subprocess_io: self
                    .data
                    .metrics
                    .cpu_only_excludes_subprocess_io
                    .clone(),
            },
            denominators: SmpArtifactDenominators {
                acceptance: self.data.denominators.acceptance.clone(),
                benchmark: SmpArtifactBenchmark {
                    repetitions_per_worker_count: self.workload.repetitions,
                    separate_from_acceptance: self
                        .data
                        .denominators
                        .benchmark
                        .separate_from_acceptance,
                    pooled: self.data.denominators.benchmark.pooled.clone(),
                },
            },
            timing_observational: SmpTimingDeclaration {
                disposition: NATIVE_CORE_SMP_TIMING_DISPOSITION.to_string(),
                timing_fields: self.observations.timing_fields.clone(),
                runtime_observed_fields: self.observations.runtime_observed_fields.clone(),
                excluded_from: self.observations.excluded_from.clone(),
            },
            optimization: self.data.optimization.clone(),
            promotion: self.data.promotion.clone(),
            redaction: self.data.redaction.clone(),
            h05_closure: NATIVE_CORE_SMP_H05_CLOSURE.to_string(),
        }
    }

    /// Emitted artifact bytes: pretty JSON plus a trailing newline.
    ///
    /// Unlike the T01 profile artifact this projection deliberately carries the
    /// observational block — the breakpoint verdicts *are* the object under
    /// qualification — so it is a run snapshot, and it is declared as one
    /// field by field instead of pretending to be a byte-reproducible digest.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, NativeCoreSmpError> {
        let mut bytes = serde_json::to_vec_pretty(&self.artifact()).map_err(|error| {
            NativeCoreSmpError::Artifact {
                reason: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpArtifactMetrics {
    pub required: Vec<String>,
    pub per_worker_terminal_counts: String,
    pub per_lane: String,
    pub cpu_only_excludes_subprocess_io: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpArtifactBenchmark {
    pub repetitions_per_worker_count: usize,
    pub separate_from_acceptance: bool,
    pub pooled: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpArtifactDenominators {
    pub acceptance: CpuSmpAcceptanceDenominator,
    pub benchmark: SmpArtifactBenchmark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmpTimingDeclaration {
    pub disposition: String,
    pub timing_fields: Vec<String>,
    pub runtime_observed_fields: Vec<String>,
    pub excluded_from: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeCoreSmpArtifact {
    pub schema_version: String,
    pub adr_ref: String,
    pub candidate_schema_version: String,
    /// Where the `cpu_smp_candidate` block is read from.
    pub manifest_binding: String,
    /// What the candidate evidence binds to (its own declared ledger).
    pub candidate_manifest_binding: String,
    pub requirement_ids: Vec<String>,
    pub topology: SmpTopologyObservation,
    pub workload: NativeCoreSmpWorkloadBinding,
    pub families: Vec<NativeCoreSmpFamilyBinding>,
    pub matrix: SmpMatrix,
    pub peak_rss_bytes: SmpObservation<u64>,
    pub memory_pressure_ceiling_bytes: SmpObservation<u64>,
    pub cpu_only: bool,
    pub subprocess_io_included: bool,
    pub selection_forbidden: bool,
    pub metrics: SmpArtifactMetrics,
    pub denominators: SmpArtifactDenominators,
    pub timing_observational: SmpTimingDeclaration,
    pub optimization: CpuSmpOptimizationPolicy,
    pub promotion: CpuSmpPromotionPolicy,
    pub redaction: CpuSmpRedactionPolicy,
    pub h05_closure: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeterministicFamily {
    family: String,
    model_sha256: String,
    rows: usize,
    documents: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeterministicEntry {
    workers: usize,
    jobs: usize,
    admitted: usize,
    terminals: usize,
    ordinals: Vec<usize>,
    completed: usize,
    failed: usize,
    coordinator_failed: usize,
    per_worker_terminal_counts: Vec<PerWorkerTerminalCounts>,
    semantic_digest: String,
    rows_decoded: u64,
    decoded_positions: u64,
    scratch_capacity_bytes: usize,
    scratch_high_water_bytes: usize,
    per_worker_scratch_high_water_bytes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NativeCoreSmpDeterministicProjection {
    schema_version: String,
    manifest_binding: String,
    workload_jobs: usize,
    families: Vec<DeterministicFamily>,
    scalar_oracle_digest: String,
    entries: Vec<DeterministicEntry>,
}

// ─────────────────────────── qualification ───────────────────────────

pub const SMP_TIMING_FIELDS: [&str; 4] = [
    "matrix.entries[].latency_ms",
    "matrix.entries[].run_wall_ms",
    "matrix.entries[].throughput_units_per_second",
    "matrix.breakpoints[].verdict",
];

pub const SMP_RUNTIME_OBSERVED_FIELDS: [&str; 6] = [
    "matrix.entries[].queue.input_high_water",
    "matrix.entries[].queue.result_high_water",
    "matrix.entries[].queue.in_flight_high_water",
    "matrix.entries[].scratch_high_water_bytes",
    "matrix.entries[].per_worker_scratch_high_water_bytes",
    "peak_rss_bytes",
];

/// Qualify the bounded SMP execution of the native CRF core over the whole
/// worker matrix `1..=physical_core_cap`.
///
/// The real kernel is executed for every worker count; the semantic digest of
/// every count must equal the serial oracle, and any divergence is a hard
/// typed error. Nothing is selected, spliced or promoted.
pub fn qualify_native_core_smp(
    data: &CpuSmpCandidateData,
    families: &[NativeCoreSmpFamily],
    topology: &TopologyReceipt,
    config: &NativeCoreSmpConfig,
    peak_rss: &mut dyn FnMut() -> Option<u64>,
) -> Result<NativeCoreSmpReceipt, NativeCoreSmpError> {
    data.validate()?;
    config.validate()?;
    if topology.physical_cores == 0 {
        return Err(NativeCoreSmpError::Topology {
            reason: "the physical core cap is zero; the worker matrix is empty".to_string(),
        });
    }
    let job_set = build_native_core_jobs(families, config)?;
    let processor = native_core_smp_processor(families)?;

    let mut entries = Vec::with_capacity(topology.physical_cores);
    for workers in 1..=topology.physical_cores {
        entries.push(run_smp_worker_count(
            topology, workers, data, &job_set, config, &processor,
        )?);
    }

    let peak_rss_bytes = match peak_rss() {
        Some(value) if value > 0 => SmpObservation::observed(value),
        Some(_) => SmpObservation::not_observed(
            "the injected peak RSS reader reported zero, which cannot be a real process peak",
        ),
        None => SmpObservation::not_observed(
            "process peak RSS is unavailable from the injected reader on this platform",
        ),
    };
    let memory_pressure_ceiling_bytes = match data.memory_ceiling_bytes() {
        Some(ceiling) => SmpObservation::observed(ceiling),
        None => SmpObservation::not_observed(
            "cpu_smp_candidate declares no memory ceiling, so memory_pressure is not observable",
        ),
    };

    let matrix = combine_smp_matrix(data, entries, &peak_rss_bytes)?;

    Ok(NativeCoreSmpReceipt {
        data: data.clone(),
        topology: SmpTopologyObservation {
            source: data.topology.source.clone(),
            allowed_cpus: topology.allowed_cpus,
            physical_cores: topology.physical_cores,
            digest: topology.digest.clone(),
        },
        families: families
            .iter()
            .map(|family| NativeCoreSmpFamilyBinding {
                family: family.family.clone(),
                model_sha256: family.model_sha256.clone(),
                model_size_bytes: family.model_size_bytes,
                rows: family.rows.len(),
                rows_per_document: config.rows_per_document,
                documents: config.documents_per_family,
            })
            .collect(),
        workload: NativeCoreSmpWorkloadBinding {
            documents_per_family: job_set.documents_per_family,
            rows_per_document: job_set.rows_per_document,
            jobs: job_set.jobs.len(),
            repetitions: config.repetitions,
            scratch_capacity_bytes: job_set.scratch_capacity,
        },
        matrix,
        peak_rss_bytes,
        memory_pressure_ceiling_bytes,
        cpu_only: true,
        subprocess_io_included: false,
        selection_forbidden: data.optimization.selection == FORBIDDEN,
        observations: NativeCoreSmpObservationalBlock {
            disposition: NATIVE_CORE_SMP_TIMING_DISPOSITION.to_string(),
            note: "wall-clock and runtime gauges are observational: they classify a worker \
count into the closed breakpoint vocabulary and never enter an acceptance denominator; the \
benchmark denominator is separate from the fixed acceptance denominator"
                .to_string(),
            timing_fields: SMP_TIMING_FIELDS.iter().map(|f| (*f).to_string()).collect(),
            runtime_observed_fields: SMP_RUNTIME_OBSERVED_FIELDS
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
            excluded_from: vec![
                "acceptance_denominators".to_string(),
                "production_route_selection".to_string(),
            ],
            acceptance_denominator: data.denominators.acceptance.clone(),
            benchmark_denominator: data.denominators.benchmark.clone(),
            benchmark_repetitions_per_worker_count: config.repetitions,
        },
    })
}

/// Convenience: a redacted, ordered set of every worker count in the matrix.
pub fn smp_worker_matrix(physical_cores: usize) -> impl Iterator<Item = usize> {
    1..=physical_cores
}

/// Re-exported so callers can assert the closed vocabulary without re-declaring it.
pub fn closed_breakpoint_vocabulary() -> BTreeSet<&'static str> {
    NATIVE_CORE_SMP_VERDICTS.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");

    fn data() -> CpuSmpCandidateData {
        load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block")
    }

    #[test]
    fn ledger_block_loads_and_declares_the_closed_vocabulary() {
        let data = data();
        assert_eq!(data.schema_version, CPU_SMP_CANDIDATE_SCHEMA_VERSION);
        assert_eq!(
            data.breakpoints.verdicts,
            NATIVE_CORE_SMP_VERDICTS.to_vec(),
            "the DATA vocabulary and the implementation must not drift"
        );
        assert_eq!(data.memory_ceiling_bytes(), None);
        assert!(data.optimization.selection == FORBIDDEN);
        assert!(data.promotion.auto_promote == FORBIDDEN);
        assert_eq!(closed_breakpoint_vocabulary().len(), 6);
        assert_eq!(smp_worker_matrix(3).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn weakened_ledger_invariants_are_refused() {
        let mut weakened = data();
        weakened.execution.silent_clamp = "allowed".to_string();
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.execution.requested_above_cap = "clamp".to_string();
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.promotion.auto_promote = "allowed".to_string();
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.optimization.selection = "allowed".to_string();
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.breakpoints.closed = false;
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.breakpoints.verdicts.pop();
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened
            .terminal_receipts
            .failure_vocabulary
            .push("invented_failure".to_string());
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened.denominators.benchmark.separate_from_acceptance = false;
        assert!(weakened.validate().is_err());

        let mut weakened = data();
        weakened
            .metrics
            .required
            .push("invented_metric".to_string());
        assert!(weakened.validate().is_err());
    }

    #[test]
    fn typed_absence_never_serializes_a_number() {
        let absent: SmpObservation<u64> = SmpObservation::not_observed("not measured");
        let json = serde_json::to_value(&absent).expect("serialize");
        assert_eq!(json["state"], "not_observed");
        assert!(json.get("value").is_none());
        let observed: SmpObservation<u64> = SmpObservation::observed(7);
        let json = serde_json::to_value(&observed).expect("serialize");
        assert_eq!(json["state"], "observed");
        assert_eq!(json["value"], 7);
    }

    #[test]
    fn runtime_config_refuses_above_cap_without_clamping() {
        let topology = TopologyReceipt {
            allowed_cpus: 4,
            physical_cores: 2,
            digest: "sha256:fixture".to_string(),
        };
        assert!(matches!(
            native_core_smp_runtime_config(&topology, 3, 64),
            Err(NativeCoreSmpError::OverCoreCap {
                requested: 3,
                cap: 2
            })
        ));
        assert!(matches!(
            native_core_smp_runtime_config(&topology, 0, 64),
            Err(NativeCoreSmpError::Config { .. })
        ));
        let config = native_core_smp_runtime_config(&topology, 2, 64).expect("valid");
        assert_eq!(config.workers, 2);
        assert_eq!(config.input_queue_capacity, 2);
    }
}
