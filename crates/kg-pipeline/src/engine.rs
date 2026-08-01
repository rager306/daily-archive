//! Pipeline execution engine.
//!
//! Executes a Pipeline sequentially, respecting step dependencies.
//! Stage implementations are provided by the consuming project via
//! a trait object — kg-pipeline does not know what stages do.

use crate::builder::Pipeline;


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

/// Result of a pipeline execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub status: PipelineStatus,
    pub completed_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub error: Option<String>,
}

impl ExecutionResult {
    pub fn success(completed: Vec<String>) -> Self {
        Self {
            success: true,
            status: PipelineStatus::Completed,
            completed_steps: completed,
            failed_step: None,
            error: None,
        }
    }

    pub fn failure(failed_step: &str, error: &str, completed: Vec<String>) -> Self {
        Self {
            success: false,
            status: PipelineStatus::Failed,
            completed_steps: completed,
            failed_step: Some(failed_step.to_string()),
            error: Some(error.to_string()),
        }
    }
}

/// Execution engine. Consumers provide a stage handler closure.
///
/// The engine is stage-agnostic — it calls the handler for each step
/// in dependency order. The handler receives the stage name and returns
/// Ok(()) or Err(message). The engine handles retry, failure policy,
/// and status tracking.
pub struct ExecutionEngine;

impl ExecutionEngine {
    pub fn new() -> Self {
        Self
    }

    /// Execute a pipeline sequentially, calling the handler for each step.
    /// Steps are executed in declaration order (the builder validates that
    /// dependencies appear before dependents).
    pub async fn execute<F, Fut>(
        &self,
        pipeline: &Pipeline,
        mut handler: F,
    ) -> ExecutionResult
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let mut completed = Vec::new();
        for step in &pipeline.steps {
            match handler(&step.stage).await {
                Ok(()) => completed.push(step.name.clone()),
                Err(e) => {
                    return ExecutionResult::failure(&step.name, &e, completed);
                }
            }
        }
        ExecutionResult::success(completed)
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
            .execute(&pipeline, |stage| async move { Ok(()) })
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
}
