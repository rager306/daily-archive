//! Failure handling policies for pipeline execution.

/// How the engine handles stage failures.
#[derive(Debug, Clone)]
pub enum FailurePolicy {
    /// Abort the entire pipeline on first failure.
    Abort,
    /// Continue executing independent steps after a failure.
    Continue,
    /// Retry failed steps up to N times with backoff, then abort.
    RetryThenAbort { retries: u32, backoff_ms: u64 },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_failure_policy_is_retry() {
        let policy = FailurePolicy::default();
        match policy {
            FailurePolicy::RetryThenAbort {
                retries,
                backoff_ms,
            } => {
                assert_eq!(retries, 3);
                assert_eq!(backoff_ms, 1000);
            }
            _ => panic!("Expected RetryThenAbort as default"),
        }
    }

    #[test]
    fn test_default_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_ms, 1000);
    }

    #[test]
    fn test_failure_action_abort() {
        let action = FailureAction::Abort;
        assert!(matches!(action, FailureAction::Abort));
    }

    #[test]
    fn test_failure_action_retry() {
        let action = FailureAction::Retry { after_ms: 500 };
        assert!(matches!(action, FailureAction::Retry { after_ms: 500 }));
    }

    #[test]
    fn test_failure_action_skip() {
        let action = FailureAction::Skip;
        assert!(matches!(action, FailureAction::Skip));
    }

    #[test]
    fn test_failure_action_escalate() {
        let action = FailureAction::Escalate {
            reason: "stage3 failed 3 times".to_string(),
        };
        if let FailureAction::Escalate { reason } = action {
            assert_eq!(reason, "stage3 failed 3 times");
        } else {
            panic!("Expected Escalate");
        }
    }
}
