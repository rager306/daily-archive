//! Pipeline builder DSL.
//!
//! Declarative pipeline construction. Consumers compose pipelines
//! from named steps with dependencies and retry policies. Stage
//! implementations (Ingest, Extract, Enrich) are provided by the
//! consuming project via the `StageName` string — kg-pipeline does
//! not know what the stages do, only how to orchestrate them.

use std::collections::HashMap;

/// A named pipeline step. The `stage` field is a string identifier
/// that the consuming project's ExecutionEngine dispatches on.
#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub name: String,
    pub stage: StageName,
    pub depends_on: Vec<String>,
    pub retry: RetryPolicyRef,
}

/// Stage identifier — a string the consuming project maps to a real
/// stage implementation. kg-pipeline is stage-agnostic.
pub type StageName = String;

/// Reference to a retry policy (by name, resolved by the engine).
pub type RetryPolicyRef = String;

/// A pipeline: ordered steps with a failure policy.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: String,
    pub steps: Vec<PipelineStep>,
    pub failure_policy: crate::failure::FailurePolicy,
}

/// Builder for declarative pipeline construction.
///
/// ```
/// use kg_pipeline::PipelineBuilder;
///
/// let pipeline = PipelineBuilder::new("single-paper")
///     .step("ingest", "IngestPDF", &[])
///     .step("extract", "ExtractEntities", &["ingest"])
///     .step("enrich", "EnrichOpenAlex", &["ingest"])
///     .step("validate", "ValidateGraph", &["extract", "enrich"])
///     .build();
/// ```
pub struct PipelineBuilder {
    id: String,
    steps: Vec<PipelineStep>,
    failure_policy: crate::failure::FailurePolicy,
}

impl PipelineBuilder {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            steps: Vec::new(),
            failure_policy: crate::failure::FailurePolicy::default(),
        }
    }

    pub fn step(mut self, name: &str, stage: &str, depends_on: &[&str]) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            stage: stage.to_string(),
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
            retry: "default".to_string(),
        });
        self
    }

    pub fn step_with_retry(
        mut self,
        name: &str,
        stage: &str,
        depends_on: &[&str],
        retry: &str,
    ) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            stage: stage.to_string(),
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
            retry: retry.to_string(),
        });
        self
    }

    pub fn failure_policy(mut self, policy: crate::failure::FailurePolicy) -> Self {
        self.failure_policy = policy;
        self
    }

    pub fn build(self) -> Result<Pipeline, String> {
        // Validate: no duplicate step names
        let mut seen = std::collections::HashSet::new();
        for step in &self.steps {
            if !seen.insert(&step.name) {
                return Err(format!("duplicate step name: {}", step.name));
            }
        }
        // Validate: all depends_on references exist
        for step in &self.steps {
            for dep in &step.depends_on {
                if !seen.contains(dep) {
                    return Err(format!(
                        "step '{}' depends on unknown step '{}'",
                        step.name, dep
                    ));
                }
            }
        }
        // Validate: no cycles (simple DFS)
        let name_to_idx: HashMap<&str, usize> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), i))
            .collect();
        for (i, step) in self.steps.iter().enumerate() {
            for dep in &step.depends_on {
                if let Some(&dep_idx) = name_to_idx.get(dep.as_str())
                    && dep_idx >= i
                {
                    return Err(format!(
                        "step '{}' depends on '{}' which appears later — possible cycle",
                        step.name, dep
                    ));
                }
            }
        }
        Ok(Pipeline {
            id: self.id,
            steps: self.steps,
            failure_policy: self.failure_policy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_pipeline() {
        let pipeline = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("b", "StageB", &["a"])
            .build()
            .unwrap();
        assert_eq!(pipeline.steps.len(), 2);
        assert_eq!(pipeline.steps[0].name, "a");
        assert_eq!(pipeline.steps[1].depends_on, vec!["a"]);
    }

    #[test]
    fn test_build_rejects_duplicate_names() {
        let result = PipelineBuilder::new("test")
            .step("a", "StageA", &[])
            .step("a", "StageB", &[])
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("duplicate"));
    }

    #[test]
    fn test_build_rejects_unknown_dependency() {
        let result = PipelineBuilder::new("test")
            .step("a", "StageA", &["nonexistent"])
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown step"));
    }

    #[test]
    fn test_build_rejects_forward_dependency() {
        let result = PipelineBuilder::new("test")
            .step("a", "StageA", &["b"])
            .step("b", "StageB", &[])
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cycle"));
    }

    #[test]
    fn test_build_with_retry_policy() {
        let pipeline = PipelineBuilder::new("test")
            .step_with_retry("a", "StageA", &[], "aggressive")
            .build()
            .unwrap();
        assert_eq!(pipeline.steps[0].retry, "aggressive");
    }
}
