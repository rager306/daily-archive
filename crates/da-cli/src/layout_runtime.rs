//! Invocation-scoped bounded CPU/SMP runtime for the shadow layout bakeoff.
//!
//! This module deliberately owns orchestration only. Donors and production parse
//! routes remain outside it. All mutable scratch belongs to one worker and all
//! channels are bounded, so admission applies backpressure rather than dropping.

use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyError {
    Unsupported,
    AllowedCpuSetUnavailable,
    MalformedAllowedCpuSet,
    CpuIdentityUnavailable(u32),
    MalformedCpuIdentity(u32),
}
impl fmt::Display for TopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "topology unavailable: {self:?}")
    }
}
impl std::error::Error for TopologyError {}

/// A redacted topology receipt. It contains no physical path or hostname.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyReceipt {
    pub allowed_cpus: usize,
    pub physical_cores: usize,
    pub digest: String,
}

pub trait TopologyReader {
    fn allowed_cpu_set(&self) -> Result<String, TopologyError>;
    fn core_identity(&self, cpu: u32) -> Result<(String, String), TopologyError>;
}

struct LinuxTopologyReader;
impl TopologyReader for LinuxTopologyReader {
    fn allowed_cpu_set(&self) -> Result<String, TopologyError> {
        fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find_map(|l| {
                    l.strip_prefix("Cpus_allowed_list:")
                        .map(str::trim)
                        .map(str::to_owned)
                })
            })
            .ok_or(TopologyError::AllowedCpuSetUnavailable)
    }
    fn core_identity(&self, cpu: u32) -> Result<(String, String), TopologyError> {
        let base = format!("/sys/devices/system/cpu/cpu{cpu}/topology/");
        let package = fs::read_to_string(format!("{base}physical_package_id"))
            .map_err(|_| TopologyError::CpuIdentityUnavailable(cpu))?
            .trim()
            .to_owned();
        let core = fs::read_to_string(format!("{base}core_id"))
            .map_err(|_| TopologyError::CpuIdentityUnavailable(cpu))?
            .trim()
            .to_owned();
        if package.is_empty() || core.is_empty() {
            return Err(TopologyError::MalformedCpuIdentity(cpu));
        }
        Ok((package, core))
    }
}

fn parse_cpu_set(raw: &str) -> Result<Vec<u32>, TopologyError> {
    let mut cpus = BTreeSet::new();
    for item in raw.trim().split(',').filter(|s| !s.is_empty()) {
        let mut bounds = item.split('-');
        let start = bounds
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or(TopologyError::MalformedAllowedCpuSet)?;
        let end = bounds
            .next()
            .map(|v| v.parse::<u32>().ok())
            .flatten()
            .unwrap_or(start);
        if bounds.next().is_some() || start > end {
            return Err(TopologyError::MalformedAllowedCpuSet);
        }
        for cpu in start..=end {
            cpus.insert(cpu);
        }
    }
    if cpus.is_empty() {
        return Err(TopologyError::MalformedAllowedCpuSet);
    }
    Ok(cpus.into_iter().collect())
}

pub fn detect_physical_topology<R: TopologyReader>(
    reader: &R,
) -> Result<TopologyReceipt, TopologyError> {
    let cpus = parse_cpu_set(&reader.allowed_cpu_set()?)?;
    let mut identities = BTreeSet::new();
    for cpu in &cpus {
        identities.insert(reader.core_identity(*cpu)?);
    }
    if identities.is_empty() {
        return Err(TopologyError::MalformedAllowedCpuSet);
    }
    let mut digest_input = String::new();
    for (package, core) in &identities {
        digest_input.push_str(package);
        digest_input.push(':');
        digest_input.push_str(core);
        digest_input.push(';');
    }
    Ok(TopologyReceipt {
        allowed_cpus: cpus.len(),
        physical_cores: identities.len(),
        digest: format!("sha256:{:x}", sha2::Sha256::digest(digest_input.as_bytes())),
    })
}

pub fn detect_system_topology() -> Result<TopologyReceipt, TopologyError> {
    #[cfg(target_os = "linux")]
    {
        detect_physical_topology(&LinuxTopologyReader)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(TopologyError::Unsupported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    NonPositive(&'static str),
    OverCoreCap { requested: usize, cap: usize },
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid runtime config: {self:?}")
    }
}
impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub workers: usize,
    pub input_queue_capacity: usize,
    pub result_queue_capacity: usize,
    pub max_in_flight: usize,
    pub private_scratch_capacity: usize,
}
impl RuntimeConfig {
    pub fn validate(&self, topology: &TopologyReceipt) -> Result<(), ConfigError> {
        for (name, value) in [
            ("workers", self.workers),
            ("input_queue_capacity", self.input_queue_capacity),
            ("result_queue_capacity", self.result_queue_capacity),
            ("max_in_flight", self.max_in_flight),
            ("private_scratch_capacity", self.private_scratch_capacity),
        ] {
            if value == 0 {
                return Err(ConfigError::NonPositive(name));
            }
        }
        if self.workers > topology.physical_cores {
            return Err(ConfigError::OverCoreCap {
                requested: self.workers,
                cap: topology.physical_cores,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScratchError {
    Exhausted { requested: usize, remaining: usize },
}
impl fmt::Display for ScratchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scratch arena exhausted: {self:?}")
    }
}
impl std::error::Error for ScratchError {}

/// Safe offset arena. A worker may reuse it after reset; no allocation escapes
/// the callback invocation and capacity is fixed at construction.
#[derive(Debug)]
pub struct ScratchArena {
    storage: Vec<u8>,
    offset: usize,
    high_water: usize,
}
impl ScratchArena {
    pub fn new(capacity: usize) -> Result<Self, ScratchError> {
        if capacity == 0 {
            return Err(ScratchError::Exhausted {
                requested: 0,
                remaining: 0,
            });
        }
        Ok(Self {
            storage: vec![0; capacity],
            offset: 0,
            high_water: 0,
        })
    }
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }
    pub fn high_water(&self) -> usize {
        self.high_water
    }
    pub fn used(&self) -> usize {
        self.offset
    }
    pub fn allocate(&mut self, size: usize) -> Result<&mut [u8], ScratchError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or(ScratchError::Exhausted {
                requested: size,
                remaining: self.storage.len().saturating_sub(self.offset),
            })?;
        if end > self.storage.len() {
            return Err(ScratchError::Exhausted {
                requested: size,
                remaining: self.storage.len().saturating_sub(self.offset),
            });
        }
        let start = self.offset;
        self.offset = end;
        self.high_water = self.high_water.max(end);
        Ok(&mut self.storage[start..end])
    }
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationState {
    NotApplicable,
    MeasuredRejected,
    MeasuredRetained,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPercentiles {
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
}

/// Redacted, CPU-only characterization of an in-process kernel. The kernel
/// returns receipt bytes, never document text; the digest is independently
/// computed here so worker/report code cannot manufacture the oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuKernelCharacterization {
    pub warmup_iterations: usize,
    pub repetitions: usize,
    pub samples: usize,
    pub percentiles: KernelPercentiles,
    pub throughput_units_per_second: u64,
    pub allocation_or_private_capacity_bytes: usize,
    pub private_scratch_high_water_bytes: usize,
    pub semantic_digest: String,
    pub scalar_oracle_digest: String,
    pub cpu_only: bool,
    pub subprocess_io_included: bool,
    pub optimization_state: OptimizationState,
    pub optimization_reason: String,
    pub selection_forbidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterizationError {
    NonPositive(&'static str),
    ScratchExhausted(ScratchError),
    SemanticMismatch { expected: String, observed: String },
}
impl fmt::Display for CharacterizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid CPU characterization: {self:?}")
    }
}
impl std::error::Error for CharacterizationError {}

/// Run a deterministic scalar CPU kernel after warmup. Timing is observational
/// evidence only; it never chooses a production route. `work_units` is kept
/// separate from acceptance denominators and is used solely for throughput.
pub fn characterize_cpu_kernel<F>(
    warmup_iterations: usize,
    repetitions: usize,
    scratch_capacity: usize,
    work_units: u64,
    mut scalar_kernel: F,
) -> Result<CpuKernelCharacterization, CharacterizationError>
where
    F: FnMut(&mut ScratchArena) -> Result<Vec<u8>, ScratchError>,
{
    if repetitions == 0 {
        return Err(CharacterizationError::NonPositive("repetitions"));
    }
    if scratch_capacity == 0 {
        return Err(CharacterizationError::NonPositive("scratch_capacity"));
    }
    let mut scratch = ScratchArena::new(scratch_capacity)
        .map_err(|_| CharacterizationError::NonPositive("scratch_capacity"))?;
    for _ in 0..warmup_iterations {
        scratch.reset();
        scalar_kernel(&mut scratch).map_err(CharacterizationError::ScratchExhausted)?;
    }
    let mut samples = Vec::with_capacity(repetitions);
    let mut oracle: Option<String> = None;
    let mut observed_capacity = 0;
    for _ in 0..repetitions {
        scratch.reset();
        let started = std::time::Instant::now();
        let receipt =
            scalar_kernel(&mut scratch).map_err(CharacterizationError::ScratchExhausted)?;
        let elapsed = started.elapsed().as_millis();
        let digest = format!("sha256:{:x}", sha2::Sha256::digest(&receipt));
        if let Some(expected) = &oracle {
            if expected != &digest {
                return Err(CharacterizationError::SemanticMismatch {
                    expected: expected.clone(),
                    observed: digest,
                });
            }
        } else {
            oracle = Some(digest);
        }
        observed_capacity = observed_capacity.max(scratch.high_water());
        samples.push(elapsed);
    }
    samples.sort_unstable();
    let percentile = |p: usize| samples[((samples.len() - 1) * p) / 100];
    let p50 = percentile(50);
    let p95 = percentile(95);
    let p99 = percentile(99);
    let total_ms: u128 = samples.iter().sum();
    let throughput = if total_ms == 0 {
        0
    } else {
        ((work_units as u128 * repetitions as u128 * 1000) / total_ms).min(u64::MAX as u128) as u64
    };
    let digest = oracle.expect("repetitions guarantees an oracle");
    Ok(CpuKernelCharacterization {
        warmup_iterations,
        repetitions,
        samples: samples.len(),
        percentiles: KernelPercentiles { p50_ms: p50, p95_ms: p95, p99_ms: p99 },
        throughput_units_per_second: throughput,
        allocation_or_private_capacity_bytes: scratch.capacity(),
        private_scratch_high_water_bytes: observed_capacity,
        semantic_digest: digest.clone(),
        scalar_oracle_digest: digest,
        cpu_only: true,
        subprocess_io_included: false,
        optimization_state: OptimizationState::NotApplicable,
        optimization_reason: "no dense numeric hot kernel was selected; scalar CPU path retained as measurement oracle".into(),
        selection_forbidden: true,
    })
}

/// Apply DATA-gated SIMD disposition without silently selecting a candidate.
pub fn gate_simd_candidate(
    dense_numeric_kernel_found: bool,
    semantic_equal: bool,
    repeated_cpu_improvement: bool,
    full_pipeline_p95_regressed: bool,
) -> (OptimizationState, &'static str) {
    if !dense_numeric_kernel_found {
        return (
            OptimizationState::NotApplicable,
            "no dense numeric hot kernel measured",
        );
    }
    if semantic_equal && repeated_cpu_improvement && !full_pipeline_p95_regressed {
        (
            OptimizationState::MeasuredRetained,
            "safe candidate met repeated CPU and full-pipeline thresholds",
        )
    } else {
        (
            OptimizationState::MeasuredRejected,
            "candidate failed semantic, repeatability, or full-pipeline regression gate",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorFailure {
    WorkerPanic { worker: usize },
    ChannelClosed { channel: &'static str },
    Interrupted,
}

#[derive(Debug)]
pub enum Terminal<R, E> {
    Completed {
        ordinal: usize,
        worker: usize,
        result: R,
    },
    Failed {
        ordinal: usize,
        worker: usize,
        error: E,
    },
    CoordinatorFailed {
        ordinal: usize,
        worker: usize,
        failure: CoordinatorFailure,
    },
}
impl<R, E> Terminal<R, E> {
    pub fn ordinal(&self) -> usize {
        match self {
            Self::Completed { ordinal, .. }
            | Self::Failed { ordinal, .. }
            | Self::CoordinatorFailed { ordinal, .. } => *ordinal,
        }
    }
}

#[derive(Debug)]
pub struct RunReport<R, E> {
    pub terminals: Vec<Terminal<R, E>>,
    pub coordinator_failure: Option<CoordinatorFailure>,
    pub admitted: usize,
    pub private_scratch_high_water: Vec<usize>,
}

pub fn run_bounded<T, R, E, F>(
    config: RuntimeConfig,
    topology: &TopologyReceipt,
    jobs: Vec<T>,
    process: F,
) -> Result<RunReport<R, E>, ConfigError>
where
    T: Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
    F: Fn(T, &mut ScratchArena) -> Result<R, E> + Send + Sync + 'static,
{
    config.validate(topology)?;
    let count = jobs.len();
    if count == 0 {
        return Ok(RunReport {
            terminals: Vec::new(),
            coordinator_failure: None,
            admitted: 0,
            private_scratch_high_water: vec![0; config.workers],
        });
    }
    let process = Arc::new(process);
    let (result_tx, result_rx) = mpsc::sync_channel::<Terminal<R, E>>(config.result_queue_capacity);
    let mut inputs = Vec::new();
    let mut handles = Vec::new();
    let mut high_water = Vec::new();
    for worker in 0..config.workers {
        let (tx, rx) = mpsc::sync_channel::<(usize, T)>(config.input_queue_capacity);
        inputs.push(tx);
        let result_tx = result_tx.clone();
        let process = process.clone();
        let capacity = config.private_scratch_capacity;
        handles.push(thread::spawn(move || {
            let mut scratch = ScratchArena::new(capacity).expect("validated scratch capacity");
            while let Ok((ordinal, job)) = rx.recv() {
                scratch.reset();
                let terminal =
                    match panic::catch_unwind(AssertUnwindSafe(|| process(job, &mut scratch))) {
                        Ok(Ok(result)) => Terminal::Completed {
                            ordinal,
                            worker,
                            result,
                        },
                        Ok(Err(error)) => Terminal::Failed {
                            ordinal,
                            worker,
                            error,
                        },
                        Err(_) => Terminal::CoordinatorFailed {
                            ordinal,
                            worker,
                            failure: CoordinatorFailure::WorkerPanic { worker },
                        },
                    };
                if result_tx.send(terminal).is_err() {
                    break;
                }
            }
            scratch.high_water()
        }));
    }
    drop(result_tx);
    let (permit_tx, permit_rx) = mpsc::sync_channel::<()>(config.max_in_flight);
    for _ in 0..config.max_in_flight {
        permit_tx.send(()).expect("permit channel has capacity");
    }
    let dispatcher = thread::spawn(move || {
        let mut admitted = 0usize;
        for (ordinal, job) in jobs.into_iter().enumerate() {
            if permit_rx.recv().is_err() {
                return Err((
                    admitted,
                    CoordinatorFailure::ChannelClosed { channel: "permit" },
                ));
            }
            let worker = ordinal % inputs.len();
            if inputs[worker].send((ordinal, job)).is_err() {
                return Err((
                    admitted,
                    CoordinatorFailure::ChannelClosed { channel: "input" },
                ));
            }
            admitted += 1;
        }
        drop(inputs);
        Ok(admitted)
    });
    let mut terminals = Vec::with_capacity(count);
    let mut failure = None;
    while terminals.len() < count {
        match result_rx.recv() {
            Ok(terminal) => {
                terminals.push(terminal);
                // Admission is released only after a terminal receipt is
                // observed, so max_in_flight bounds unresolved work.
                // The sender stays alive until collection is finished.
                permit_tx.send(()).map_err(|_| ()).ok();
            }
            Err(_) => {
                failure = Some(CoordinatorFailure::ChannelClosed { channel: "result" });
                break;
            }
        }
    }
    drop(permit_tx);
    let (admitted, dispatch_failure) = match dispatcher.join() {
        Ok(Ok(admitted)) => (admitted, None),
        Ok(Err((admitted, error))) => (admitted, Some(error)),
        Err(_) => (terminals.len(), Some(CoordinatorFailure::Interrupted)),
    };
    if failure.is_none() {
        failure = dispatch_failure;
    }
    if failure.is_none() {
        failure = terminals.iter().find_map(|terminal| match terminal {
            Terminal::CoordinatorFailed { failure, .. } => Some(failure.clone()),
            _ => None,
        });
    }
    for handle in handles {
        match handle.join() {
            Ok(value) => high_water.push(value),
            Err(_) => {
                failure = Some(CoordinatorFailure::WorkerPanic {
                    worker: high_water.len(),
                });
                high_water.push(0);
            }
        }
    }
    terminals.sort_by_key(Terminal::ordinal);
    Ok(RunReport {
        terminals,
        coordinator_failure: failure,
        admitted,
        private_scratch_high_water: high_water,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct FixtureReader {
        allowed: &'static str,
        ids: BTreeMap<u32, (&'static str, &'static str)>,
    }
    impl TopologyReader for FixtureReader {
        fn allowed_cpu_set(&self) -> Result<String, TopologyError> {
            Ok(self.allowed.into())
        }
        fn core_identity(&self, cpu: u32) -> Result<(String, String), TopologyError> {
            self.ids
                .get(&cpu)
                .map(|(p, c)| ((*p).into(), (*c).into()))
                .ok_or(TopologyError::CpuIdentityUnavailable(cpu))
        }
    }
    fn topology() -> TopologyReceipt {
        detect_physical_topology(&FixtureReader {
            allowed: "0-3",
            ids: [
                (0, ("0", "0")),
                (1, ("0", "0")),
                (2, ("0", "1")),
                (3, ("0", "1")),
            ]
            .into_iter()
            .collect(),
        })
        .unwrap()
    }
    fn config(workers: usize) -> RuntimeConfig {
        RuntimeConfig {
            workers,
            input_queue_capacity: 1,
            result_queue_capacity: 1,
            max_in_flight: 2,
            private_scratch_capacity: 8,
        }
    }
    #[test]
    fn topology_intersects_allowed_cpus_with_unique_cores() {
        assert_eq!(topology().physical_cores, 2);
        assert_eq!(topology().allowed_cpus, 4);
    }
    #[test]
    fn config_rejects_zero_and_over_cap_without_clamp() {
        assert!(matches!(
            config(0).validate(&topology()),
            Err(ConfigError::NonPositive("workers"))
        ));
        assert!(matches!(
            config(3).validate(&topology()),
            Err(ConfigError::OverCoreCap {
                requested: 3,
                cap: 2
            })
        ));
    }
    #[test]
    fn scratch_is_bounded_and_resets() {
        let mut s = ScratchArena::new(4).unwrap();
        s.allocate(3).unwrap();
        assert_eq!(s.high_water(), 3);
        s.reset();
        s.allocate(4).unwrap();
        assert!(s.allocate(1).is_err());
    }
    #[test]
    fn bounded_runtime_has_one_terminal_per_admitted_job() {
        let report = run_bounded(
            config(2),
            &topology(),
            (0..7).collect::<Vec<_>>(),
            |n, s| {
                s.allocate(2).unwrap();
                Ok::<_, ()>(n * 2)
            },
        )
        .unwrap();
        assert_eq!(report.admitted, 7);
        assert_eq!(report.terminals.len(), 7);
        assert_eq!(
            report
                .terminals
                .iter()
                .map(Terminal::ordinal)
                .collect::<Vec<_>>(),
            (0..7).collect::<Vec<_>>()
        );
        assert!(report.coordinator_failure.is_none());
    }
    #[test]
    fn shared_source_identity_and_private_scratch_are_preserved() {
        let source = Arc::new(vec![1_u8, 2, 3]);
        let jobs = (0..4).map(|_| source.clone()).collect::<Vec<_>>();
        let expected = source.clone();
        let report = run_bounded(config(2), &topology(), jobs, move |input, scratch| {
            assert!(Arc::ptr_eq(&input, &expected));
            scratch.allocate(3).map_err(|_| "scratch")?;
            Ok::<_, &str>(input.len())
        })
        .unwrap();
        assert_eq!(report.admitted, 4);
        assert_eq!(report.terminals.len(), 4);
        assert_eq!(report.private_scratch_high_water, vec![3, 3]);
    }

    #[test]
    fn cpu_characterization_is_scalar_only_repeated_and_redacted() {
        let report = characterize_cpu_kernel(2, 5, 16, 4, |scratch| {
            let slot = scratch.allocate(4)?;
            slot.copy_from_slice(&[1, 2, 3, 4]);
            Ok(slot.to_vec())
        })
        .unwrap();
        assert_eq!(report.warmup_iterations, 2);
        assert_eq!(report.repetitions, 5);
        assert_eq!(report.samples, 5);
        assert_eq!(report.scalar_oracle_digest, report.semantic_digest);
        assert_eq!(report.private_scratch_high_water_bytes, 4);
        assert_eq!(report.allocation_or_private_capacity_bytes, 16);
        assert!(report.cpu_only);
        assert!(!report.subprocess_io_included);
        assert_eq!(report.optimization_state, OptimizationState::NotApplicable);
        assert!(report.selection_forbidden);
        assert!(report.percentiles.p50_ms <= report.percentiles.p95_ms);
        assert!(report.percentiles.p95_ms <= report.percentiles.p99_ms);
    }

    #[test]
    fn cpu_characterization_rejects_semantic_drift_and_invalid_bounds() {
        assert!(matches!(
            characterize_cpu_kernel(0, 0, 8, 1, |_| Ok(vec![1])),
            Err(CharacterizationError::NonPositive("repetitions"))
        ));
        assert!(matches!(
            characterize_cpu_kernel(0, 1, 0, 1, |_| Ok(vec![1])),
            Err(CharacterizationError::NonPositive("scratch_capacity"))
        ));
        let mut n = 0;
        assert!(matches!(
            characterize_cpu_kernel(0, 2, 8, 1, |_| {
                n += 1;
                Ok(vec![n])
            }),
            Err(CharacterizationError::SemanticMismatch { .. })
        ));
        assert!(matches!(
            characterize_cpu_kernel(0, 1, 2, 1, |scratch| {
                scratch.allocate(3).map(|_| vec![1])
            }),
            Err(CharacterizationError::ScratchExhausted(_))
        ));
    }

    #[test]
    fn simd_gate_is_closed_and_never_selects_without_all_evidence() {
        assert_eq!(
            gate_simd_candidate(false, true, true, false),
            (
                OptimizationState::NotApplicable,
                "no dense numeric hot kernel measured"
            )
        );
        assert_eq!(
            gate_simd_candidate(true, true, false, false).0,
            OptimizationState::MeasuredRejected
        );
        assert_eq!(
            gate_simd_candidate(true, true, true, true).0,
            OptimizationState::MeasuredRejected
        );
        assert_eq!(
            gate_simd_candidate(true, true, true, false).0,
            OptimizationState::MeasuredRetained
        );
    }

    #[test]
    fn characterization_types_roundtrip_without_selection_or_raw_payload() {
        let report = characterize_cpu_kernel(0, 1, 8, 1, |scratch| {
            scratch.allocate(2).unwrap().copy_from_slice(&[7, 9]);
            Ok(vec![7, 9])
        })
        .unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("not_applicable"));
        assert!(json.contains("selection_forbidden"));
        assert!(!json.contains("raw_pdf"));
        let restored: CpuKernelCharacterization = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, report);
    }

    #[test]
    fn job_error_is_terminal_and_panic_is_not_silent() {
        let report = run_bounded(config(1), &topology(), vec![0usize, 1], |n, _| {
            if n == 0 {
                Err::<usize, _>("typed")
            } else {
                panic!("fixture")
            }
        })
        .unwrap();
        assert_eq!(report.admitted, 2);
        assert_eq!(report.terminals.len(), 2);
        assert!(matches!(
            report.terminals[0],
            Terminal::Failed { error: "typed", .. }
        ));
        assert!(matches!(
            report.terminals[1],
            Terminal::CoordinatorFailed {
                failure: CoordinatorFailure::WorkerPanic { .. },
                ..
            }
        ));
        assert!(matches!(
            report.coordinator_failure,
            Some(CoordinatorFailure::WorkerPanic { .. })
        ));
    }
}
