//! Invocation-scoped bounded CPU/SMP runtime for the shadow layout bakeoff.
//!
//! This module deliberately owns orchestration only. Donors and production parse
//! routes remain outside it. All mutable scratch belongs to one worker and all
//! channels are bounded, so admission applies backpressure rather than dropping.

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
