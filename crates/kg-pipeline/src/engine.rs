//! Pipeline execution engine.
//!
//! Executes a Pipeline sequentially, respecting step dependencies.
//! Stage implementations are provided by the consuming project via
//! a trait object — kg-pipeline does not know what stages do.

use crate::builder::Pipeline;
use crate::failure::RetryPolicy;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// Determinism-infrastructure port (ADR-051 contour 1): injectable sleep.
/// Production uses [`TokioSleeper`]; tests use counting no-op sleepers so
/// backoff never introduces real wall-clock delays or flakiness.
pub trait Sleeper: Send + Sync + 'static {
    fn sleep(&self, ms: u64) -> Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}

/// Production sleeper backed by tokio's timer.
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, ms: u64) -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(std::time::Duration::from_millis(ms)))
    }
}

/// Test double: never waits, counts invocations. Determinism infra —
/// backoff in tests must not introduce real wall-clock delays.
#[derive(Default)]
pub struct NoopSleeper {
    pub sleep_calls: std::sync::atomic::AtomicUsize,
}

impl Sleeper for NoopSleeper {
    fn sleep(&self, _ms: u64) -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        self.sleep_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async {})
    }
}

/// Pipeline execution status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Stopped,
}

/// Observable step-level event emitted by the engine's observer callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepEvent {
    /// A step attempt is starting (attempt numbers start at 1).
    Started { step: String, attempt: u32 },
    /// A failed attempt will be retried; `attempt` is the upcoming attempt.
    Retried { step: String, attempt: u32 },
    /// The step exhausted its attempts (terminal for this run).
    Failed { step: String, error: String },
    /// The step did not run: skipped by Continue policy or transitively
    /// because a dependency was skipped.
    Skipped { step: String },
    /// The step completed successfully.
    Completed { step: String },
    /// FSM status transition: Pending→Running at start, then
    /// Running→{Completed,Failed,Paused,Stopped} at the terminal edge.
    StatusChanged {
        from: PipelineStatus,
        to: PipelineStatus,
    },
}

/// Cooperative control plane checked between steps.
/// `stop` wins over `pause`.
#[derive(Debug, Clone, Default)]
pub struct EngineControl {
    paused: Arc<std::sync::atomic::AtomicBool>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl EngineControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pause(&self) {
        self.paused.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Result of a pipeline execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub status: PipelineStatus,
    pub completed_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub error: Option<String>,
    /// Attempts spent per step name. Steps that were never reached are
    /// absent; a step retried twice reports its total attempt count.
    pub attempts_per_step: HashMap<String, u32>,
    /// Steps that did not run: skipped by a Continue failure policy or
    /// skipped transitively because a dependency was skipped.
    pub skipped_steps: Vec<String>,
}

impl ExecutionResult {
    pub fn success(completed: Vec<String>) -> Self {
        Self {
            success: true,
            status: PipelineStatus::Completed,
            completed_steps: completed,
            failed_step: None,
            error: None,
            attempts_per_step: HashMap::new(),
            skipped_steps: Vec::new(),
        }
    }

    pub fn failure(failed_step: &str, error: &str, completed: Vec<String>) -> Self {
        Self {
            success: false,
            status: PipelineStatus::Failed,
            completed_steps: completed,
            failed_step: Some(failed_step.to_string()),
            error: Some(error.to_string()),
            attempts_per_step: HashMap::new(),
            skipped_steps: Vec::new(),
        }
    }
}

/// Execution engine. Consumers provide a stage handler closure.
///
/// The engine is stage-agnostic — it calls the handler for each step
/// in dependency order. The handler receives the stage name and returns
/// Ok(()) or Err(message). The engine handles retry, failure policy,
/// and status tracking.
pub struct ExecutionEngine {
    sleeper: Arc<dyn Sleeper>,
    observer: Option<Box<dyn Fn(StepEvent) + Send + Sync>>,
}

impl ExecutionEngine {
    pub fn new() -> Self {
        Self {
            sleeper: Arc::new(TokioSleeper),
            observer: None,
        }
    }

    /// Test/determinism constructor: inject a custom sleeper (ADR-051).
    pub fn with_sleeper(sleeper: Arc<dyn Sleeper>) -> Self {
        Self {
            sleeper,
            observer: None,
        }
    }

    /// Observability constructor: receive every step event and status
    /// transition during execution.
    pub fn with_observer<F>(observer: F) -> Self
    where
        F: Fn(StepEvent) + Send + Sync + 'static,
    {
        Self {
            sleeper: Arc::new(TokioSleeper),
            observer: Some(Box::new(observer)),
        }
    }

    /// Combined constructor: sleeper + observer.
    pub fn with_sleeper_and_observer<F>(sleeper: Arc<dyn Sleeper>, observer: F) -> Self
    where
        F: Fn(StepEvent) + Send + Sync + 'static,
    {
        Self {
            sleeper,
            observer: Some(Box::new(observer)),
        }
    }

    fn emit(&self, event: StepEvent) {
        if let Some(cb) = &self.observer {
            cb(event);
        }
    }

    /// Execute a pipeline sequentially, calling the handler for each step.
    /// Steps are executed in declaration order (the builder validates that
    /// dependencies appear before dependents).
    ///
    /// Convenience form: no policy registry, one attempt per step.
    pub async fn execute<F, Fut>(&self, pipeline: &Pipeline, handler: F) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        self.execute_with_policies(pipeline, &HashMap::new(), handler)
            .await
    }

    /// Execute with a per-step retry-policy registry keyed by the step's
    /// `retry` reference (`RetryPolicyRef`). Steps whose reference is absent
    /// from the registry get exactly one attempt. Backoff between attempts
    /// goes through the injected sleeper (ADR-051 determinism port).
    pub async fn execute_with_policies<F, Fut>(
        &self,
        pipeline: &Pipeline,
        policies: &HashMap<String, RetryPolicy>,
        handler: F,
    ) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        self.execute_with_failure_policy(
            pipeline,
            &crate::failure::FailurePolicy::RetryThenAbort {
                retries: 0,
                backoff_ms: 0,
            },
            policies,
            handler,
        )
        .await
    }

    /// Execute with an explicit pipeline-level failure policy.
    ///
    /// - `Abort`: stop at the first failing step (after its per-step retries
    ///   from the registry).
    /// - `Continue`: mark the failed step skipped, keep running steps whose
    ///   dependencies completed; steps depending on a skipped step are
    ///   skipped transitively. The result is `Failed` (not all steps ran)
    ///   with `skipped_steps` populated.
    /// - `RetryThenAbort`: per-step retry via the registry, then abort.
    pub async fn execute_with_failure_policy<F, Fut>(
        &self,
        pipeline: &Pipeline,
        failure_policy: &crate::failure::FailurePolicy,
        policies: &HashMap<String, RetryPolicy>,
        handler: F,
    ) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let control = EngineControl::new();
        self.execute_controlled_with_failure_policy(
            pipeline,
            failure_policy,
            policies,
            &control,
            handler,
        )
        .await
    }

    /// Controlled execution with the default single-attempt abort policy:
    /// pause/stop are checked between steps.
    pub async fn execute_controlled<F, Fut>(
        &self,
        pipeline: &Pipeline,
        policies: &HashMap<String, RetryPolicy>,
        control: &EngineControl,
        handler: F,
    ) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        self.execute_controlled_with_failure_policy(
            pipeline,
            &crate::failure::FailurePolicy::RetryThenAbort {
                retries: 0,
                backoff_ms: 0,
            },
            policies,
            control,
            handler,
        )
        .await
    }

    /// Core FSM execution: status transitions Pending→Running→terminal are
    /// emitted to the observer; pause/stop are checked between steps (stop
    /// wins); step events carry step name and attempt number.
    pub async fn execute_controlled_with_failure_policy<F, Fut>(
        &self,
        pipeline: &Pipeline,
        failure_policy: &crate::failure::FailurePolicy,
        policies: &HashMap<String, RetryPolicy>,
        control: &EngineControl,
        mut handler: F,
    ) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        use crate::failure::FailurePolicy;

        self.emit(StepEvent::StatusChanged {
            from: PipelineStatus::Pending,
            to: PipelineStatus::Running,
        });

        let mut completed = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut attempts_per_step = HashMap::new();
        for step in &pipeline.steps {
            // Cooperative control: checked between steps, stop wins.
            if control.is_stopped() {
                self.emit(StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Stopped,
                });
                return ExecutionResult {
                    success: false,
                    status: PipelineStatus::Stopped,
                    completed_steps: completed,
                    failed_step: None,
                    error: Some("stopped by control".to_string()),
                    attempts_per_step,
                    skipped_steps: skipped,
                };
            }
            if control.is_paused() {
                self.emit(StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Paused,
                });
                return ExecutionResult {
                    success: false,
                    status: PipelineStatus::Paused,
                    completed_steps: completed,
                    failed_step: None,
                    error: Some("paused between steps".to_string()),
                    attempts_per_step,
                    skipped_steps: skipped,
                };
            }

            // Dependency constraint: a step runs only if no dependency was
            // skipped. A skipped dependency skips the step transitively.
            let blocked = step
                .depends_on
                .iter()
                .any(|dep| skipped.iter().any(|s| s == dep));
            if blocked {
                self.emit(StepEvent::Skipped {
                    step: step.name.clone(),
                });
                skipped.push(step.name.clone());
                continue;
            }

            let policy = policies.get(&step.retry);
            let max_attempts = policy.map_or(1, |p| p.max_attempts.max(1));
            let backoff_ms = policy.map_or(0, |p| p.backoff_ms);
            let mut attempts = 0u32;
            loop {
                attempts += 1;
                attempts_per_step.insert(step.name.clone(), attempts);
                self.emit(StepEvent::Started {
                    step: step.name.clone(),
                    attempt: attempts,
                });
                match handler(&step.stage).await {
                    Ok(()) => {
                        completed.push(step.name.clone());
                        self.emit(StepEvent::Completed {
                            step: step.name.clone(),
                        });
                        break;
                    }
                    Err(e) => {
                        if attempts >= max_attempts {
                            self.emit(StepEvent::Failed {
                                step: step.name.clone(),
                                error: e.clone(),
                            });
                            match failure_policy {
                                FailurePolicy::Continue => {
                                    self.emit(StepEvent::Skipped {
                                        step: step.name.clone(),
                                    });
                                    skipped.push(step.name.clone());
                                }
                                FailurePolicy::Abort | FailurePolicy::RetryThenAbort { .. } => {
                                    self.emit(StepEvent::StatusChanged {
                                        from: PipelineStatus::Running,
                                        to: PipelineStatus::Failed,
                                    });
                                    return ExecutionResult {
                                        attempts_per_step,
                                        skipped_steps: skipped,
                                        ..ExecutionResult::failure(&step.name, &e, completed)
                                    };
                                }
                            }
                            break;
                        }
                        self.emit(StepEvent::Retried {
                            step: step.name.clone(),
                            attempt: attempts + 1,
                        });
                        self.sleeper.sleep(backoff_ms).await;
                    }
                }
            }
        }
        let all_ran = skipped.is_empty();
        let to = if all_ran {
            PipelineStatus::Completed
        } else {
            PipelineStatus::Failed
        };
        self.emit(StepEvent::StatusChanged {
            from: PipelineStatus::Running,
            to: to.clone(),
        });
        if all_ran {
            ExecutionResult {
                attempts_per_step,
                skipped_steps: skipped,
                ..ExecutionResult::success(completed)
            }
        } else {
            let failed_step = skipped
                .first()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            ExecutionResult {
                success: false,
                status: PipelineStatus::Failed,
                completed_steps: completed,
                failed_step: Some(failed_step),
                error: Some(format!("{} step(s) skipped", skipped.len())),
                attempts_per_step,
                skipped_steps: skipped,
            }
        }
    }
}

impl Default for ExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PipelineBuilder;

    #[tokio::test]
    async fn test_execute_success() {
        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "StageB", &["a"])
            .build()
            .unwrap();

        let engine = ExecutionEngine::new();
        let result = engine
            .execute(&pipeline, |_stage| async move { Ok(()) })
            .await;

        assert!(result.success);
        assert_eq!(result.status, PipelineStatus::Completed);
        assert_eq!(result.completed_steps, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_execute_failure_aborts() {
        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "FailHere", &["a"])
            .step("c", "StageC", &["b"])
            .build()
            .unwrap();

        let engine = ExecutionEngine::new();
        let result = engine
            .execute(&pipeline, |stage| {
                let stage = stage.to_string();
                async move {
                    if stage == "FailHere" {
                        Err("stage failed".to_string())
                    } else {
                        Ok(())
                    }
                }
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.status, PipelineStatus::Failed);
        assert_eq!(result.failed_step, Some("b".to_string()));
        assert_eq!(result.completed_steps, vec!["a"]);
    }

    /// S01 red-first: retry policy from the registry must actually be
    /// applied — a flaky stage succeeds on attempt 3 of 3, attempts are
    /// recorded, and the injected sleeper is invoked between attempts.
    #[tokio::test]
    async fn test_retry_applied_from_policy_registry() {
        use crate::engine::NoopSleeper;
        use std::sync::atomic::AtomicUsize;

        let pipeline = PipelineBuilder::new("test")
            .step_with_retry("a", "FlakyStage", &[], "patient")
            .step("b", "StageB", &["a"])
            .build()
            .unwrap();

        let mut policies = HashMap::new();
        policies.insert(
            "patient".to_string(),
            RetryPolicy {
                max_attempts: 3,
                backoff_ms: 0,
            },
        );

        let sleeper = Arc::new(NoopSleeper::default());
        let engine = ExecutionEngine::with_sleeper(sleeper.clone());

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_h = calls.clone();
        let result = engine
            .execute_with_policies(&pipeline, &policies, move |_stage| {
                let n = calls_h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move {
                    if n < 2 {
                        Err(format!("flaky failure #{n}"))
                    } else {
                        Ok(())
                    }
                }
            })
            .await;

        assert!(
            result.success,
            "expected success on 3rd attempt: {result:?}"
        );
        assert_eq!(result.status, PipelineStatus::Completed);
        assert_eq!(result.completed_steps, vec!["a", "b"]);
        assert_eq!(result.attempts_per_step.get("a"), Some(&3));
        assert_eq!(result.attempts_per_step.get("b"), Some(&1));
        assert_eq!(
            sleeper
                .sleep_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            2,
            "backoff between retries must go through the injected sleeper"
        );
    }

    /// S02 red-first: Abort stops at the first failing step.
    #[tokio::test]
    async fn test_failure_policy_abort_stops_pipeline() {
        use crate::failure::FailurePolicy;

        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "FailHere", &["a"])
            .step("c", "StageC", &["b"])
            .build()
            .unwrap();

        let engine = ExecutionEngine::new();
        let result = engine
            .execute_with_failure_policy(
                &pipeline,
                &FailurePolicy::Abort,
                &HashMap::new(),
                |stage| {
                    let stage = stage.to_string();
                    async move {
                        if stage == "FailHere" {
                            Err("stage failed".to_string())
                        } else {
                            Ok(())
                        }
                    }
                },
            )
            .await;

        assert!(!result.success);
        assert_eq!(result.status, PipelineStatus::Failed);
        assert_eq!(result.failed_step, Some("b".to_string()));
        assert_eq!(result.completed_steps, vec!["a"]);
        assert!(
            result.skipped_steps.is_empty(),
            "Abort must not skip; it stops: {result:?}"
        );
    }

    /// S02 red-first: Continue marks the failed step skipped and still runs
    /// independent steps, but steps depending on the skipped one are skipped
    /// transitively (dependency constraint holds).
    #[tokio::test]
    async fn test_failure_policy_continue_skips_dependents_transitively() {
        use crate::failure::FailurePolicy;
        use std::sync::atomic::AtomicUsize;

        // a -> b(fail) -> c(depends b) ; d independent ; e depends on c (2nd hop)
        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "FailHere", &["a"])
            .step("c", "StageC", &["b"])
            .step("d", "StageD", &[])
            .step("e", "StageE", &["c"])
            .build()
            .unwrap();

        let executed = Arc::new(AtomicUsize::new(0));
        let engine = ExecutionEngine::new();
        let result = engine
            .execute_with_failure_policy(
                &pipeline,
                &FailurePolicy::Continue,
                &HashMap::new(),
                |stage| {
                    executed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let stage = stage.to_string();
                    async move {
                        if stage == "FailHere" {
                            Err("stage failed".to_string())
                        } else {
                            Ok(())
                        }
                    }
                },
            )
            .await;

        assert!(
            !result.success,
            "pipeline with a skipped step is not fully successful: {result:?}"
        );
        assert_eq!(result.status, PipelineStatus::Failed);
        // executed: a, b(fail), d — but NOT c, NOT e
        assert_eq!(
            executed.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "dependents of the skipped step must not run"
        );
        assert_eq!(result.completed_steps, vec!["a", "d"]);
        assert!(result.skipped_steps.contains(&"b".to_string()));
        assert!(result.skipped_steps.contains(&"c".to_string()));
        assert!(
            result.skipped_steps.contains(&"e".to_string()),
            "transitive dependents are skipped too: {:?}",
            result.skipped_steps
        );
        assert_eq!(result.failed_step, Some("b".to_string()));
    }

    /// S02 red-first: RetryThenAbort retries via the per-step registry and
    /// aborts once attempts are exhausted.
    #[tokio::test]
    async fn test_failure_policy_retry_then_abort_after_exhaustion() {
        use crate::failure::FailurePolicy;

        let pipeline = PipelineBuilder::new("test")
            .step_with_retry("a", "AlwaysFails", &[], "patient")
            .step("b", "StageB", &["a"])
            .build()
            .unwrap();

        let mut policies = HashMap::new();
        policies.insert(
            "patient".to_string(),
            RetryPolicy {
                max_attempts: 3,
                backoff_ms: 0,
            },
        );

        let engine = ExecutionEngine::with_sleeper(Arc::new(NoopSleeper::default()));
        let result = engine
            .execute_with_failure_policy(
                &pipeline,
                &FailurePolicy::RetryThenAbort {
                    retries: 2,
                    backoff_ms: 0,
                },
                &policies,
                |_| async { Err("boom".to_string()) },
            )
            .await;

        assert!(!result.success);
        assert_eq!(result.status, PipelineStatus::Failed);
        assert_eq!(result.failed_step, Some("a".to_string()));
        assert_eq!(
            result.attempts_per_step.get("a"),
            Some(&3),
            "registry policy governs per-step attempts: {:?}",
            result.attempts_per_step
        );
        assert!(result.completed_steps.is_empty());
    }

    /// S03 red-first: observer receives the full event stream with step name
    /// and attempt numbers; pause yields Paused; resume continues to
    /// Completed; stop yields Stopped.
    #[tokio::test]
    async fn test_observer_event_stream_and_pause_stop() {
        use crate::engine::{EngineControl, StepEvent};
        use std::sync::Mutex;

        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "StageB", &["a"])
            .step("c", "StageC", &["b"])
            .build()
            .unwrap();

        let events: Arc<Mutex<Vec<StepEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let engine = ExecutionEngine::with_observer(move |event: StepEvent| {
            ev.lock().unwrap().push(event);
        });

        let control = EngineControl::new();
        // Pause after "a" completes: flip the flag when StageB starts.
        let control2 = control.clone();
        let result = engine
            .execute_controlled(&pipeline, &HashMap::new(), &control, |stage| {
                let stage = stage.to_string();
                let ctl = control2.clone();
                async move {
                    if stage == "StageB" {
                        ctl.pause();
                    }
                    Ok(())
                }
            })
            .await;
        assert_eq!(
            result.status,
            PipelineStatus::Paused,
            "paused between steps: {result:?}"
        );
        // Resume: clear pause and run again from scratch (completed steps are
        // re-run in this minimal engine; resume semantics = continue the walk).
        control.resume();
        let result2 = engine
            .execute_controlled(&pipeline, &HashMap::new(), &control, |_| async { Ok(()) })
            .await;
        assert_eq!(result2.status, PipelineStatus::Completed);

        // Stop: yields Stopped before remaining steps.
        let control3 = EngineControl::new();
        let c3 = control3.clone();
        let result3 = engine
            .execute_controlled(&pipeline, &HashMap::new(), &control3, |stage| {
                let stage = stage.to_string();
                let ctl = c3.clone();
                async move {
                    if stage == "StageA" {
                        ctl.stop();
                    }
                    Ok(())
                }
            })
            .await;
        assert_eq!(
            result3.status,
            PipelineStatus::Stopped,
            "stopped: {result3:?}"
        );
        assert_eq!(result3.completed_steps, vec!["a"]);

        let log = events.lock().unwrap().clone();
        assert!(
            log.iter().any(|e| matches!(e, StepEvent::Started { step, attempt } if step == "a" && *attempt == 1)),
            "expected StepStarted a/1 in {log:?}"
        );
        assert!(
            log.iter()
                .any(|e| matches!(e, StepEvent::Completed { step } if step == "c")),
            "expected StepCompleted c in {log:?}"
        );
        // FSM transitions are observable: Pending→Running at start,
        // Running→Paused on pause, Running→Stopped on stop,
        // Running→Completed at the happy end.
        assert!(
            log.iter().any(|e| matches!(
                e,
                StepEvent::StatusChanged {
                    from: PipelineStatus::Pending,
                    to: PipelineStatus::Running
                }
            )),
            "expected Pending→Running in {log:?}"
        );
        assert!(
            log.iter().any(|e| matches!(
                e,
                StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Paused
                }
            )),
            "expected Running→Paused in {log:?}"
        );
        assert!(
            log.iter().any(|e| matches!(
                e,
                StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Stopped
                }
            )),
            "expected Running→Stopped in {log:?}"
        );
        assert!(
            log.iter().any(|e| matches!(
                e,
                StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Completed
                }
            )),
            "expected Running→Completed in {log:?}"
        );
    }

    /// S03 red-first: Retried/Skipped/Failed events on a Continue run with
    /// per-step retries.
    #[tokio::test]
    async fn test_observer_retry_skip_fail_events() {
        use crate::engine::{EngineControl, StepEvent};
        use crate::failure::FailurePolicy;
        use std::sync::Mutex;

        // a retries (fails once, succeeds on attempt 2); b always fails ->
        // skipped by Continue; c depends on b -> transitively skipped.
        let pipeline = PipelineBuilder::new("test")
            .step_with_retry("a", "Flaky", &[], "patient")
            .step("b", "AlwaysFails", &["a"])
            .step("c", "StageC", &["b"])
            .build()
            .unwrap();

        let mut policies = HashMap::new();
        policies.insert(
            "patient".to_string(),
            RetryPolicy {
                max_attempts: 2,
                backoff_ms: 0,
            },
        );

        let events: Arc<Mutex<Vec<StepEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let engine = ExecutionEngine::with_observer(move |event: StepEvent| {
            ev.lock().unwrap().push(event);
        });

        let control = EngineControl::new();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_h = calls.clone();
        let result = engine
            .execute_controlled_with_failure_policy(
                &pipeline,
                &FailurePolicy::Continue,
                &policies,
                &control,
                move |stage| {
                    let stage = stage.to_string();
                    let n = calls_h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    async move {
                        if stage == "Flaky" {
                            if n == 0 {
                                Err("transient".to_string())
                            } else {
                                Ok(())
                            }
                        } else if stage == "AlwaysFails" {
                            Err("boom".to_string())
                        } else {
                            Ok(())
                        }
                    }
                },
            )
            .await;

        assert!(!result.success);
        let log = events.lock().unwrap().clone();
        assert!(log.iter().any(|e| matches!(e, StepEvent::Retried { step, attempt } if step == "a" && *attempt == 2)), "expected Retried a/2 in {log:?}");
        assert!(
            log.iter()
                .any(|e| matches!(e, StepEvent::Failed { step, .. } if step == "b")),
            "expected Failed b in {log:?}"
        );
        assert!(
            log.iter()
                .any(|e| matches!(e, StepEvent::Skipped { step } if step == "b")),
            "expected Skipped b in {log:?}"
        );
        assert!(
            log.iter()
                .any(|e| matches!(e, StepEvent::Skipped { step } if step == "c")),
            "expected transitive Skipped c in {log:?}"
        );
        assert!(
            log.iter().any(|e| matches!(
                e,
                StepEvent::StatusChanged {
                    from: PipelineStatus::Running,
                    to: PipelineStatus::Failed
                }
            )),
            "expected Running→Failed (skip tail) in {log:?}"
        );
    }
}
