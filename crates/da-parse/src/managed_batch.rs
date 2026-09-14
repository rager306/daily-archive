//! Composition-root adapter for the application managed parse-batch port.

use crate::{
    BatchCheckpoint as ConcreteCheckpoint, ParseBatch, ParseBatchCancellation, ParseBatchExecutor,
    ParseBatchResultSink,
};
use da_ports::{
    BatchCheckpoint, ManagedParseBatchExecutor, ParseBatchCursor, ParseBatchRunSummary,
    ParseBatchSinkError,
};
use std::sync::Arc;
use tokio::sync::Mutex;

struct CheckpointAdapter {
    inner: Arc<dyn BatchCheckpoint>,
}

#[async_trait::async_trait]
impl ConcreteCheckpoint for CheckpointAdapter {
    async fn save(&self, cursor: &crate::BatchCursor) -> Result<(), crate::ParseBatchSinkError> {
        self.inner
            .save(&ParseBatchCursor::new(u64::from(cursor.next_ordinal())))
            .await
            .map_err(|error| crate::ParseBatchSinkError(error.to_string()))
    }
}

/// Binds concrete parser execution to the narrow application port.
pub struct ManagedParseBatch<B, C>
where
    B: crate::ParseBackend + 'static,
    C: crate::ParseArtifactCache + 'static,
{
    executor: ParseBatchExecutor<B, C>,
    batch: Mutex<Option<ParseBatch>>,
    sink: Arc<dyn ParseBatchResultSink>,
    cancellation: ParseBatchCancellation,
}

impl<B, C> ManagedParseBatch<B, C>
where
    B: crate::ParseBackend + 'static,
    C: crate::ParseArtifactCache + 'static,
{
    pub fn new(
        executor: ParseBatchExecutor<B, C>,
        batch: ParseBatch,
        sink: Arc<dyn ParseBatchResultSink>,
        cancellation: ParseBatchCancellation,
    ) -> Self {
        Self {
            executor,
            batch: Mutex::new(Some(batch)),
            sink,
            cancellation,
        }
    }
}

#[async_trait::async_trait]
impl<B, C> ManagedParseBatchExecutor for ManagedParseBatch<B, C>
where
    B: crate::ParseBackend + 'static,
    C: crate::ParseArtifactCache + 'static,
{
    async fn run_managed(
        &self,
        checkpoint: Arc<dyn BatchCheckpoint>,
    ) -> Result<ParseBatchRunSummary, ParseBatchSinkError> {
        let batch = self
            .batch
            .lock()
            .await
            .take()
            .ok_or_else(|| ParseBatchSinkError("managed parse batch already started".into()))?;
        let summary = self
            .executor
            .run_managed(
                batch,
                self.sink.clone(),
                Arc::new(CheckpointAdapter { inner: checkpoint }),
                self.cancellation.clone(),
            )
            .await;
        Ok(ParseBatchRunSummary {
            cursor: ParseBatchCursor::new(u64::from(summary.cursor().next_ordinal())),
            cancelled: summary.cancelled(),
        })
    }

    fn cancel(&self) {
        self.cancellation.cancel();
    }
}
