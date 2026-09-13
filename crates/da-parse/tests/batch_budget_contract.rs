//! Runtime contract for the ordinal publication window (M506/S04/T01).
//!
//! The test deliberately uses independent backend, cache, sink and checkpoint
//! seams. It proves that a slow frontier cannot allow later documents to be
//! admitted beyond the configured window, while release preserves ordered,
//! exactly-once publication and durable-prefix advancement.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use da_parse::{
    CacheMode, ParseArtifactCache, ParseBackend, ParseBackendFailure, ParseBackendRequest,
    ParseBatch, ParseBatchBudget, ParseBatchExecutor, ParseBatchMemoryCheckpoint,
    ParseBatchResultSink, ParseBatchSinkError, ParseCacheFailure, RawStageRead, TeiEnvelope,
    TeiIdentity,
};
use tokio::sync::{Mutex, Notify};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bounds are nonzero")
}

fn requests() -> Vec<ParseBackendRequest> {
    (0..4)
        .map(|ordinal| {
            ParseBackendRequest::new(
                format!("window-{ordinal}"),
                format!("%PDF-window-{ordinal}").into_bytes(),
            )
        })
        .collect()
}

#[derive(Debug, Default)]
struct BlockingBackendState {
    calls: AtomicUsize,
    frontier_started: Notify,
    frontier_released: AtomicBool,
    release_signal: Notify,
}

#[derive(Debug, Clone, Default)]
struct BlockingBackend {
    state: Arc<BlockingBackendState>,
}

impl BlockingBackend {
    fn call_count(&self) -> usize {
        self.state.calls.load(Ordering::SeqCst)
    }

    fn release_frontier(&self) {
        self.state.frontier_released.store(true, Ordering::SeqCst);
        self.state.release_signal.notify_waiters();
    }
}

#[async_trait]
impl ParseBackend for BlockingBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        self.state.calls.fetch_add(1, Ordering::SeqCst);
        if request.paper_id() == "window-0" {
            self.state.frontier_started.notify_waiters();
            while !self.state.frontier_released.load(Ordering::SeqCst) {
                self.state.release_signal.notified().await;
            }
        }
        Ok(TeiEnvelope::new(
            b"<TEI><teiHeader/><text><body>window fixture</body></text></TEI>".to_vec(),
            TeiIdentity::new("budget-test", "budget-test-v1"),
        ))
    }
}

#[derive(Debug, Default)]
struct MissCache;

#[async_trait]
impl ParseArtifactCache for MissCache {
    async fn read_raw(&self, _key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        Ok(None)
    }

    async fn write_raw(&self, _key: &str, _payload: &[u8]) -> Result<(), ParseCacheFailure> {
        Ok(())
    }

    async fn read_projected(&self, _key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        Ok(None)
    }

    async fn write_projected(&self, _key: &str, _payload: &[u8]) -> Result<(), ParseCacheFailure> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct OrderedSink {
    ordinals: Mutex<Vec<u32>>,
}

#[async_trait]
impl ParseBatchResultSink for OrderedSink {
    async fn record(
        &self,
        outcome: da_parse::ParseDocumentOutcome,
    ) -> Result<(), ParseBatchSinkError> {
        self.ordinals.lock().await.push(outcome.ordinal());
        Ok(())
    }
}

impl OrderedSink {
    async fn ordinals(&self) -> Vec<u32> {
        self.ordinals.lock().await.clone()
    }
}

#[tokio::test(start_paused = true)]
async fn blocked_frontier_does_not_admit_beyond_publication_window() {
    let backend = BlockingBackend::default();
    let backend_signal = backend.clone();
    let executor =
        Arc::new(ParseBatchExecutor::new(backend, MissCache).with_publication_window(nz(1)));
    assert_eq!(executor.publication_window(), Some(nz(1)));

    let batch = ParseBatch::admit(
        CacheMode::Off,
        da_parse::ParseBatchLimits::new(nz(4), nz(4), nz(4), nz(4)),
        requests(),
        None,
    )
    .expect("fixture admits");
    let sink = Arc::new(OrderedSink::default());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let run = tokio::spawn({
        let sink = sink.clone();
        let checkpoint = checkpoint.clone();
        async move { executor.run(batch, sink, checkpoint).await }
    });

    backend_signal.state.frontier_started.notified().await;
    tokio::task::yield_now().await;
    assert_eq!(
        backend_signal.call_count(),
        1,
        "ordinal 1 must not start while ordinal 0 holds the window"
    );
    assert_eq!(sink.ordinals().await, Vec::<u32>::new());

    backend_signal.release_frontier();
    let summary = run.await.expect("executor task completes");

    assert_eq!(summary.succeeded(), 4);
    assert_eq!(summary.failed(), 0);
    assert_eq!(sink.ordinals().await, vec![0, 1, 2, 3]);
    assert_eq!(summary.cursor().next_ordinal(), 4);
    assert_eq!(checkpoint.cursor().await.unwrap().next_ordinal(), 4);
}

fn admitted_batch() -> ParseBatch {
    ParseBatch::admit(
        CacheMode::Off,
        da_parse::ParseBatchLimits::new(nz(4), nz(4), nz(4), nz(4)),
        requests(),
        None,
    )
    .expect("fixture admits")
}

#[tokio::test(start_paused = true)]
async fn shared_publication_budget_limits_concurrent_batches() {
    let backend = BlockingBackend::default();
    let budget = Arc::new(ParseBatchBudget::new(nz(1), nz(4)));
    let first = Arc::new(
        ParseBatchExecutor::new(backend.clone(), MissCache)
            .with_publication_window(nz(4))
            .with_shared_budget(budget.clone()),
    );
    let second = Arc::new(
        ParseBatchExecutor::new(backend.clone(), MissCache)
            .with_publication_window(nz(4))
            .with_shared_budget(budget),
    );
    let first_sink = Arc::new(OrderedSink::default());
    let second_sink = Arc::new(OrderedSink::default());
    let first_checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let second_checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());

    let first_run = tokio::spawn({
        let sink = first_sink.clone();
        let checkpoint = first_checkpoint.clone();
        async move { first.run(admitted_batch(), sink, checkpoint).await }
    });
    backend.state.frontier_started.notified().await;

    let second_run = tokio::spawn({
        let sink = second_sink.clone();
        let checkpoint = second_checkpoint.clone();
        async move { second.run(admitted_batch(), sink, checkpoint).await }
    });
    tokio::task::yield_now().await;
    assert_eq!(
        backend.call_count(),
        1,
        "the shared publication slot must keep the second batch out of the feeder"
    );
    assert!(second_sink.ordinals().await.is_empty());

    backend.release_frontier();
    let first_summary = first_run.await.expect("first executor completes");
    let second_summary = second_run.await.expect("second executor completes");
    assert_eq!(first_summary.succeeded(), 4);
    assert_eq!(second_summary.succeeded(), 4);
    assert_eq!(first_sink.ordinals().await, vec![0, 1, 2, 3]);
    assert_eq!(second_sink.ordinals().await, vec![0, 1, 2, 3]);
}

#[derive(Debug, Default)]
struct SharedExecutionState {
    active: AtomicUsize,
    peak: AtomicUsize,
    calls: AtomicUsize,
    first_started: Notify,
    released: AtomicBool,
    release_signal: Notify,
}

#[derive(Debug, Clone, Default)]
struct SharedExecutionBackend {
    state: Arc<SharedExecutionState>,
}

impl SharedExecutionBackend {
    fn peak(&self) -> usize {
        self.state.peak.load(Ordering::SeqCst)
    }

    fn release(&self) {
        self.state.released.store(true, Ordering::SeqCst);
        self.state.release_signal.notify_waiters();
    }
}

#[async_trait]
impl ParseBackend for SharedExecutionBackend {
    async fn fetch_tei(
        &self,
        _request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        self.state.calls.fetch_add(1, Ordering::SeqCst);
        let now = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.peak.fetch_max(now, Ordering::SeqCst);
        self.state.first_started.notify_waiters();
        while !self.state.released.load(Ordering::SeqCst) {
            self.state.release_signal.notified().await;
        }
        self.state.active.fetch_sub(1, Ordering::SeqCst);
        Ok(TeiEnvelope::new(
            b"<TEI><teiHeader/><text><body>shared execution fixture</body></text></TEI>".to_vec(),
            TeiIdentity::new("budget-test", "budget-test-v1"),
        ))
    }
}

#[tokio::test(start_paused = true)]
async fn shared_execution_budget_caps_concurrent_backend_work() {
    let backend = SharedExecutionBackend::default();
    let budget = Arc::new(ParseBatchBudget::new(nz(8), nz(1)));
    let first = Arc::new(
        ParseBatchExecutor::new(backend.clone(), MissCache)
            .with_publication_window(nz(4))
            .with_shared_budget(budget.clone()),
    );
    let second = Arc::new(
        ParseBatchExecutor::new(backend.clone(), MissCache)
            .with_publication_window(nz(4))
            .with_shared_budget(budget),
    );
    let first_run = tokio::spawn({
        let sink = Arc::new(OrderedSink::default());
        let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
        async move { first.run(admitted_batch(), sink, checkpoint).await }
    });
    backend.state.first_started.notified().await;
    let second_run = tokio::spawn({
        let sink = Arc::new(OrderedSink::default());
        let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
        async move { second.run(admitted_batch(), sink, checkpoint).await }
    });
    tokio::task::yield_now().await;
    assert_eq!(backend.state.calls.load(Ordering::SeqCst), 1);
    assert_eq!(backend.peak(), 1);

    backend.release();
    assert_eq!(
        first_run
            .await
            .expect("first executor completes")
            .succeeded(),
        4
    );
    assert_eq!(
        second_run
            .await
            .expect("second executor completes")
            .succeeded(),
        4
    );
    assert_eq!(backend.peak(), 1, "shared execution cap must remain one");
}
