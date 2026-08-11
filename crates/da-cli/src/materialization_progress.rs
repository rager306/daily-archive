//! Lock-independent progress state and heartbeat rendering for long materialization runs.

use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use kg_storage::MutationCounters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationPhase {
    Initializing,
    WaveStart,
    Prepare,
    FirstWrite,
    Replay,
    Assurance,
    Export,
    Complete,
}

impl MaterializationPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::WaveStart => "wave_start",
            Self::Prepare => "prepare",
            Self::FirstWrite => "first_write",
            Self::Replay => "replay",
            Self::Assurance => "assurance",
            Self::Export => "export",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterializationProgressSnapshot {
    pub phase: String,
    pub wave: usize,
    pub waves_total: usize,
    pub completed: usize,
    pub total: usize,
    pub work_units_completed: usize,
    pub work_units_total: usize,
    pub current_paper: Option<String>,
    pub elapsed_ms: u64,
    pub since_progress_ms: u64,
    pub papers_per_second: f64,
    pub work_units_per_second: f64,
    pub eta_seconds: Option<u64>,
    pub mutations_attempted: u64,
    pub mutations_applied: u64,
    pub mutations_unchanged: u64,
    pub mutations_rejected: u64,
    pub stalled: bool,
}

struct State {
    phase: MaterializationPhase,
    wave: usize,
    completed: usize,
    work_units_completed: usize,
    current_paper: Option<String>,
    mutations: MutationCounters,
    started: Instant,
    last_progress: Instant,
    completed_event: bool,
}

#[derive(Clone)]
pub struct MaterializationProgress {
    state: Arc<Mutex<State>>,
    waves_total: usize,
    total: usize,
}

impl MaterializationProgress {
    pub fn new(waves_total: usize, total: usize) -> Self {
        let now = Instant::now();
        Self {
            state: Arc::new(Mutex::new(State {
                phase: MaterializationPhase::Initializing,
                wave: 0,
                completed: 0,
                work_units_completed: 0,
                current_paper: None,
                mutations: MutationCounters::default(),
                started: now,
                last_progress: now,
                completed_event: false,
            })),
            waves_total,
            total,
        }
    }

    pub fn phase(&self, phase: MaterializationPhase, wave: usize) {
        let mut state = self.state.lock().expect("progress state poisoned");
        state.phase = phase;
        state.wave = wave;
        state.last_progress = Instant::now();
    }

    pub fn paper_started(&self, phase: MaterializationPhase, wave: usize, paper_id: &str) {
        let mut state = self.state.lock().expect("progress state poisoned");
        state.phase = phase;
        state.wave = wave;
        state.current_paper = Some(paper_id.chars().take(128).collect());
        state.last_progress = Instant::now();
    }

    pub fn work_unit_completed(&self, mutations: MutationCounters) {
        let mut state = self.state.lock().expect("progress state poisoned");
        state.work_units_completed += 1;
        state.mutations.merge(mutations);
        state.last_progress = Instant::now();
    }

    pub fn paper_completed(&self) {
        let mut state = self.state.lock().expect("progress state poisoned");
        state.completed += 1;
        state.work_units_completed += 1;
        state.current_paper = None;
        state.last_progress = Instant::now();
    }

    pub fn complete(&self) {
        let mut state = self.state.lock().expect("progress state poisoned");
        if !state.completed_event {
            state.phase = MaterializationPhase::Complete;
            state.current_paper = None;
            state.completed_event = true;
            state.last_progress = Instant::now();
        }
    }

    pub fn snapshot(&self, stall_after: Duration) -> MaterializationProgressSnapshot {
        let state = self.state.lock().expect("progress state poisoned");
        let now = Instant::now();
        let elapsed = now.duration_since(state.started);
        let since_progress = now.duration_since(state.last_progress);
        let work_rate = if state.work_units_completed == 0 || elapsed.is_zero() {
            0.0
        } else {
            state.work_units_completed as f64 / elapsed.as_secs_f64()
        };
        let paper_rate = if state.completed == 0 || elapsed.is_zero() {
            0.0
        } else {
            state.completed as f64 / elapsed.as_secs_f64()
        };
        let work_units_total = self.total.saturating_mul(3);
        let remaining = work_units_total.saturating_sub(state.work_units_completed);
        MaterializationProgressSnapshot {
            phase: state.phase.as_str().into(),
            wave: state.wave,
            waves_total: self.waves_total,
            completed: state.completed,
            total: self.total,
            work_units_completed: state.work_units_completed,
            work_units_total,
            current_paper: state.current_paper.clone(),
            elapsed_ms: millis(elapsed),
            since_progress_ms: millis(since_progress),
            papers_per_second: paper_rate,
            work_units_per_second: work_rate,
            eta_seconds: (work_rate > 0.0).then(|| (remaining as f64 / work_rate).ceil() as u64),
            mutations_attempted: state.mutations.attempted(),
            mutations_applied: state.mutations.applied(),
            mutations_unchanged: state.mutations.unchanged(),
            mutations_rejected: state.mutations.rejected(),
            stalled: since_progress >= stall_after && !state.completed_event,
        }
    }
}

pub struct ProgressReporter {
    stop: Option<mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ProgressReporter {
    pub fn spawn(
        progress: MaterializationProgress,
        interval: Duration,
        stall_after: Duration,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            emit(&progress.snapshot(stall_after));
            loop {
                match receiver.recv_timeout(interval) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => emit(&progress.snapshot(stall_after)),
                }
            }
            emit(&progress.snapshot(stall_after));
        });
        Self {
            stop: Some(sender),
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.stop.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProgressReporter {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn emit(snapshot: &MaterializationProgressSnapshot) {
    let kind = if snapshot.stalled {
        "STALL"
    } else {
        "PROGRESS"
    };
    eprintln!(
        "{kind} phase={} wave={}/{} papers={}/{} work={}/{} current={} elapsed={}s idle={}s paper_rate={:.3}/s work_rate={:.3}/s eta={} mutations={}/{}/{} rejected={}",
        snapshot.phase,
        snapshot.wave,
        snapshot.waves_total,
        snapshot.completed,
        snapshot.total,
        snapshot.work_units_completed,
        snapshot.work_units_total,
        snapshot.current_paper.as_deref().unwrap_or("-"),
        snapshot.elapsed_ms / 1000,
        snapshot.since_progress_ms / 1000,
        snapshot.papers_per_second,
        snapshot.work_units_per_second,
        snapshot
            .eta_seconds
            .map_or_else(|| "unknown".into(), |v| format!("{v}s")),
        snapshot.mutations_attempted,
        snapshot.mutations_applied,
        snapshot.mutations_unchanged,
        snapshot.mutations_rejected,
    );
    tracing::info!(
        event = if snapshot.stalled {
            "materialization_progress_stall"
        } else {
            "materialization_progress_tick"
        },
        phase = snapshot.phase,
        wave = snapshot.wave,
        waves_total = snapshot.waves_total,
        completed = snapshot.completed,
        total = snapshot.total,
        work_units_completed = snapshot.work_units_completed,
        work_units_total = snapshot.work_units_total,
        current_paper = snapshot.current_paper,
        elapsed_ms = snapshot.elapsed_ms,
        since_progress_ms = snapshot.since_progress_ms,
        papers_per_second = snapshot.papers_per_second,
        work_units_per_second = snapshot.work_units_per_second,
        eta_seconds = snapshot.eta_seconds,
        mutations_attempted = snapshot.mutations_attempted,
        mutations_applied = snapshot.mutations_applied,
        mutations_unchanged = snapshot.mutations_unchanged,
        mutations_rejected = snapshot.mutations_rejected,
    );
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_are_monotonic_serializable_and_stall_visible() {
        let progress = MaterializationProgress::new(2, 2);
        progress.phase(MaterializationPhase::Prepare, 1);
        progress.paper_started(MaterializationPhase::FirstWrite, 1, "2506.07853");
        std::thread::sleep(Duration::from_millis(2));
        let started = progress.snapshot(Duration::from_millis(1));
        assert!(started.stalled);
        assert_eq!(started.current_paper.as_deref(), Some("2506.07853"));
        progress.work_unit_completed(MutationCounters::default());
        progress.work_unit_completed(MutationCounters::default());
        progress.paper_completed();
        let completed = progress.snapshot(Duration::from_secs(10));
        assert_eq!(completed.completed, 1);
        assert!(!completed.stalled);
        let roundtrip: MaterializationProgressSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&completed).unwrap()).unwrap();
        assert_eq!(roundtrip.phase, completed.phase);
        assert_eq!(roundtrip.completed, completed.completed);
        assert_eq!(roundtrip.mutations_attempted, completed.mutations_attempted);
        assert!((roundtrip.papers_per_second - completed.papers_per_second).abs() < 1e-9);
    }

    #[test]
    fn completion_is_idempotent() {
        let progress = MaterializationProgress::new(1, 0);
        progress.complete();
        progress.complete();
        let snapshot = progress.snapshot(Duration::ZERO);
        assert_eq!(snapshot.phase, "complete");
        let reporter =
            ProgressReporter::spawn(progress, Duration::from_millis(1), Duration::from_millis(1));
        reporter.stop();
        assert!(!snapshot.stalled);
    }
}
