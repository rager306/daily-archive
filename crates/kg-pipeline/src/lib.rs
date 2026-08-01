//! kg-pipeline — Universal pipeline DSL and execution engine.
//!
//! Declarative pipeline construction, execution with retry/failure
//! handling, pre-flight validation, and YAML template loading.
//! Zero project-specific dependencies — stage implementations are
//! plugged in by the consuming project.
//!
//! See ADR-049/050 for the architecture decision.

pub mod builder;
pub mod engine;
pub mod failure;

pub use builder::{Pipeline, PipelineStep, PipelineBuilder, StageName};
pub use engine::{ExecutionEngine, ExecutionResult, PipelineStatus};
pub use failure::{FailurePolicy, RetryPolicy, FailureAction};
