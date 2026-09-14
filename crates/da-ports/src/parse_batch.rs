//! Managed parse-batch execution port.
//!
//! This contract keeps the application layer independent from the concrete
//! parser implementation. The composition root owns the batch, result sink,
//! backend and cancellation implementation; the supervisor only owns durable
//! fencing and the narrow lifecycle seam below.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseBatchCursor {
    pub next_ordinal: u64,
}

impl ParseBatchCursor {
    pub const fn new(next_ordinal: u64) -> Self {
        Self { next_ordinal }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseBatchRunSummary {
    pub cursor: ParseBatchCursor,
    pub cancelled: bool,
}

impl ParseBatchRunSummary {
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }

    pub const fn cursor(&self) -> &ParseBatchCursor {
        &self.cursor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBatchSinkError(pub String);

impl std::fmt::Display for ParseBatchSinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse batch sink failure: {}", self.0)
    }
}

impl std::error::Error for ParseBatchSinkError {}

#[async_trait]
pub trait BatchCheckpoint: Send + Sync {
    async fn save(&self, cursor: &ParseBatchCursor) -> Result<(), ParseBatchSinkError>;
}

/// Composition-root implementation of a managed parser run.
///
/// The executor owns concrete parser values and its cancellation handle. The
/// application supervisor can therefore enforce ledger ordering without
/// importing the parser crate.
#[async_trait]
pub trait ManagedParseBatchExecutor: Send + Sync {
    async fn run_managed(
        &self,
        checkpoint: Arc<dyn BatchCheckpoint>,
    ) -> Result<ParseBatchRunSummary, ParseBatchSinkError>;

    /// Wake/abort local workers after the durable cancel transition.
    fn cancel(&self);
}
