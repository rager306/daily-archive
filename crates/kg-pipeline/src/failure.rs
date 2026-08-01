//! Failure handling policies for pipeline execution.

/// How the engine handles stage failures.
#[derive(Debug, Clone)]
pub enum FailurePolicy {
    /// Abort the entire pipeline on first failure.
    Abort,
    /// Continue executing independent steps after a failure.
    Continue,
    /// Retry failed steps up to N times with backoff, then abort.
    RetryThenAbort {
        retries: u32,
        backoff_ms: u64,
    },
}

impl Default for FailurePolicy {
    fn default() -> Self {
        Self::RetryThenAbort {
            retries: 3,
            backoff_ms: 1000,
        }
    }
}

/// Retry policy for individual steps.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_ms: 1000,
        }
    }
}

/// What the engine should do when a stage fails.
#[derive(Debug, Clone)]
pub enum FailureAction {
    /// Retry the stage after a delay.
    Retry { after_ms: u64 },
    /// Skip this stage and continue.
    Skip,
    /// Abort the entire pipeline.
    Abort,
    /// Escalate — record a decision and abort.
    Escalate { reason: String },
}
