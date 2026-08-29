//! S06/T02 contract tests: the two-tier composite cache
//! ([`TwoStageParseCache`]) and the bounded batch executor over it.
//!
//! Objective proofs required by the S06 plan:
//!
//! 1. a memory hit NEVER touches the backing tier;
//! 2. a backing hit is PROMOTED into the memory tier (canonical and legacy
//!    provenance preserved);
//! 3. configurationally incompatible parse profiles (different projector
//!    schema fingerprints) do not share cache entries; raw and projected
//!    stages never conflate even for identical key strings;
//! 4. the executor over the two-stage cache keeps the concurrency bound,
//!    produces exactly one correlated typed outcome per admitted ordinal,
//!    and a partial failure neither cancels nor duplicates the other
//!    elements;
//! 5. a write-through run followed by a read-only replay in the same
//!    process is served from the memory tier with ZERO backing reads and
//!    ZERO backend calls.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use da_parse::{
    CacheIntegrityCode, CacheMode, MEMORY_TIER_DEFAULT_CAPACITY, ParseArtifactCache, ParseBackend,
    ParseBackendFailure, ParseBackendRequest, ParseBatch, ParseBatchCollector, ParseBatchExecutor,
    ParseBatchLimits, ParseBatchMemoryCheckpoint, ParseCacheFailure, ParseCacheStage,
    ParseDocumentFailure, ParseDocumentOutcome, RawStageRead, RetryAfter, TeiEnvelope, TeiIdentity,
    TwoStageParseCache, project_article_stage_key, raw_tei_stage_key,
};

// ───────────────────────── helpers ─────────────────────────

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bound is nonzero")
}

fn pdf(seed: u8) -> Vec<u8> {
    vec![b'P', b'D', b'F', seed]
}

fn request(paper_id: &str, seed: u8) -> ParseBackendRequest {
    ParseBackendRequest::new(paper_id, pdf(seed))
}

fn sha_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

// ───────────────────────── counting durable backing tier ─────────────────────────

#[derive(Default)]
struct BackingState {
    raw: std::sync::Mutex<HashMap<String, RawStageRead>>,
    projected: std::sync::Mutex<HashMap<String, RawStageRead>>,
    read_raw_calls: AtomicUsize,
    read_projected_calls: AtomicUsize,
    write_raw_calls: AtomicUsize,
    write_projected_calls: AtomicUsize,
    fail_reads: AtomicBool,
    fail_writes: AtomicBool,
}

/// A durable backing tier with observable call counts and injectable
/// typed failures (the production T03 FilePipelineCache adapter is out of
/// scope for this crate — the composite only needs the trait).
#[derive(Clone, Default)]
struct CountingBacking {
    state: Arc<BackingState>,
}

impl CountingBacking {
    fn seed_raw(&self, key: &str, payload: &[u8], legacy: bool) {
        let read = if legacy {
            RawStageRead::from_legacy(payload.to_vec())
        } else {
            RawStageRead::canonical(payload.to_vec())
        };
        self.state.raw.lock().unwrap().insert(key.to_string(), read);
    }

    fn read_raw_calls(&self) -> usize {
        self.state.read_raw_calls.load(Ordering::SeqCst)
    }

    fn read_projected_calls(&self) -> usize {
        self.state.read_projected_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ParseArtifactCache for CountingBacking {
    async fn read_raw(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.state.read_raw_calls.fetch_add(1, Ordering::SeqCst);
        if self.state.fail_reads.load(Ordering::SeqCst) {
            return Err(ParseCacheFailure::new(
                ParseCacheStage::RawTei,
                CacheIntegrityCode::ChecksumMismatch,
            ));
        }
        Ok(self.state.raw.lock().unwrap().get(key).cloned())
    }

    async fn write_raw(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.state.write_raw_calls.fetch_add(1, Ordering::SeqCst);
        if self.state.fail_writes.load(Ordering::SeqCst) {
            return Err(ParseCacheFailure::new(
                ParseCacheStage::RawTei,
                CacheIntegrityCode::StageWriteFailed,
            ));
        }
        self.state
            .raw
            .lock()
            .unwrap()
            .insert(key.to_string(), RawStageRead::canonical(payload.to_vec()));
        Ok(())
    }

    async fn read_projected(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.state
            .read_projected_calls
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.state.projected.lock().unwrap().get(key).cloned())
    }

    async fn write_projected(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.state
            .write_projected_calls
            .fetch_add(1, Ordering::SeqCst);
        self.state
            .projected
            .lock()
            .unwrap()
            .insert(key.to_string(), RawStageRead::canonical(payload.to_vec()));
        Ok(())
    }
}

// ───────────────────────── minimal batch backend ─────────────────────────

#[derive(Default)]
struct BackendState {
    calls: std::sync::Mutex<HashMap<String, usize>>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

#[derive(Clone, Default)]
struct BatchBackend {
    state: Arc<BackendState>,
    failures: Arc<HashMap<String, ParseBackendFailure>>,
}

impl BatchBackend {
    fn with_busy(mut self, paper: &str, status: u16, seconds: u64) -> Self {
        let mut failures = HashMap::clone(&self.failures);
        failures.insert(
            paper.to_string(),
            ParseBackendFailure::busy(status, Some(RetryAfter::Seconds(seconds))),
        );
        self.failures = Arc::new(failures);
        self
    }

    fn call_count(&self, paper: &str) -> usize {
        self.state
            .calls
            .lock()
            .unwrap()
            .get(paper)
            .copied()
            .unwrap_or(0)
    }

    fn max_in_flight(&self) -> usize {
        self.state.max_in_flight.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ParseBackend for BatchBackend {
    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        *self
            .state
            .calls
            .lock()
            .unwrap()
            .entry(request.paper_id().to_string())
            .or_insert(0) += 1;
        let now = self.state.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.max_in_flight.fetch_max(now, Ordering::SeqCst);
        // Yield so overlapping tasks truly interleave under the bounded
        // semaphore.
        tokio::task::yield_now().await;
        self.state.in_flight.fetch_sub(1, Ordering::SeqCst);
        if let Some(failure) = self.failures.get(request.paper_id()) {
            return Err(failure.clone());
        }
        let tei = format!("<TEI>tei-for-{}</TEI>", request.paper_id());
        let identity = TeiIdentity::new("fake-two-stage-backend", "profile=fake;two=stage");
        Ok(TeiEnvelope::new(
            tei,
            identity.with_result(request.result_identity()),
        ))
    }
}

// ───────────────────────── two-tier semantics ─────────────────────────

#[tokio::test]
async fn memory_hit_never_touches_the_backing_tier() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());
    cache.write_raw("k-raw", b"raw-payload").await.unwrap();
    cache
        .write_projected("k-proj", b"projected-payload")
        .await
        .unwrap();
    // The durable publish happened exactly once per stage...
    assert_eq!(backing.state.write_raw_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        backing.state.write_projected_calls.load(Ordering::SeqCst),
        1
    );
    // ...and the write also populated the memory tier (one entry per stage).
    assert_eq!(cache.memory_entries().await, 2);

    // Every later read is served by the memory tier: zero backing reads,
    // byte-equal payloads.
    for _ in 0..3 {
        let raw = cache.read_raw("k-raw").await.unwrap().expect("memory hit");
        assert_eq!(raw.payload(), b"raw-payload");
        assert!(!raw.is_legacy());
        let proj = cache
            .read_projected("k-proj")
            .await
            .unwrap()
            .expect("memory hit");
        assert_eq!(proj.payload(), b"projected-payload");
    }
    assert_eq!(backing.read_raw_calls(), 0, "raw read must hit memory only");
    assert_eq!(
        backing.read_projected_calls(),
        0,
        "projected read must hit memory only"
    );
}

#[tokio::test]
async fn backing_hit_is_promoted_and_memory_hit_follows() {
    let backing = CountingBacking::default();
    backing.seed_raw("promoted-key", b"backing-bytes", false);
    // A LEGACY seeded read must keep its provenance through promotion: the
    // executor's legacy-hit accounting stays honest on later memory hits.
    backing.seed_raw("legacy-key", b"legacy-bytes", true);
    let cache = TwoStageParseCache::new(backing.clone());

    let first = cache.read_raw("promoted-key").await.unwrap().unwrap();
    assert_eq!(first.payload(), b"backing-bytes");
    assert_eq!(backing.read_raw_calls(), 1, "first read consults backing");

    let second = cache.read_raw("promoted-key").await.unwrap().unwrap();
    assert_eq!(second.payload(), b"backing-bytes");
    assert_eq!(
        backing.read_raw_calls(),
        1,
        "promoted entry must be served by the memory tier"
    );

    let legacy = cache.read_raw("legacy-key").await.unwrap().unwrap();
    assert!(
        legacy.is_legacy(),
        "legacy provenance must survive promotion"
    );
    assert_eq!(legacy.payload(), b"legacy-bytes");
    let legacy_again = cache.read_raw("legacy-key").await.unwrap().unwrap();
    assert!(
        legacy_again.is_legacy(),
        "memory hit of a promoted legacy entry stays legacy"
    );
    assert_eq!(
        backing.read_raw_calls(),
        2,
        "one backing read per seeded key"
    );
    assert_eq!(cache.memory_entries().await, 2);
}

#[tokio::test]
async fn miss_stays_a_miss_without_negative_caching() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());
    assert!(cache.read_raw("absent").await.unwrap().is_none());
    assert!(cache.read_raw("absent").await.unwrap().is_none());
    assert_eq!(
        backing.read_raw_calls(),
        2,
        "no negative caching: misses keep consulting the durable tier"
    );
    cache.write_raw("absent", b"now-present").await.unwrap();
    let read = cache.read_raw("absent").await.unwrap().unwrap();
    assert_eq!(read.payload(), b"now-present");
}

#[tokio::test]
async fn incompatible_profiles_and_stages_do_not_share_entries() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());

    // Configurationally incompatible parse profiles: the same paper under
    // two projector schema fingerprints occupies two entries.
    let pdf_sha = sha_hex(b"pdf-a");
    let raw_sha = sha_hex(b"<TEI>tei</TEI>");
    let key_fp1 = project_article_stage_key("paper-a", &pdf_sha, &raw_sha, "fingerprint-v1");
    let key_fp2 = project_article_stage_key("paper-a", &pdf_sha, &raw_sha, "fingerprint-v2");
    assert_ne!(
        key_fp1, key_fp2,
        "fingerprints must separate projector keys"
    );

    cache
        .write_projected(&key_fp1, b"payload-v1")
        .await
        .unwrap();
    assert!(
        cache.read_projected(&key_fp2).await.unwrap().is_none(),
        "a foreign schema fingerprint must never share the entry"
    );
    let read = cache.read_projected(&key_fp1).await.unwrap().unwrap();
    assert_eq!(read.payload(), b"payload-v1");

    // Raw and projected stages never conflate — not even for an identical
    // key string (the tier key includes the stage).
    let request = request("paper-a", 1);
    let raw_key = raw_tei_stage_key(&request);
    cache.write_raw(&raw_key, b"raw-bytes").await.unwrap();
    assert!(
        cache.read_projected(&raw_key).await.unwrap().is_none(),
        "a raw key must never serve the projector stage"
    );
    assert_eq!(
        cache.read_raw(&raw_key).await.unwrap().unwrap().payload(),
        b"raw-bytes"
    );
    assert_eq!(
        backing.read_projected_calls(),
        2,
        "fp2 and raw-as-projected misses"
    );
}

#[tokio::test]
async fn invalidation_demotes_memory_and_backing_remains_authority() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());
    cache
        .write_raw("demote-me", b"durable-bytes")
        .await
        .unwrap();
    assert!(cache.read_raw("demote-me").await.unwrap().is_some());
    assert_eq!(backing.read_raw_calls(), 0);

    assert!(
        cache
            .invalidate_memory(ParseCacheStage::RawTei, "demote-me")
            .await,
        "invalidation reports a present entry"
    );
    assert!(
        !cache
            .invalidate_memory(ParseCacheStage::RawTei, "demote-me")
            .await
    );
    assert_eq!(cache.memory_entries().await, 0);

    // The backing tier kept the payload: the next read re-consults it.
    let read = cache.read_raw("demote-me").await.unwrap().unwrap();
    assert_eq!(read.payload(), b"durable-bytes");
    assert_eq!(backing.read_raw_calls(), 1);
}

#[tokio::test]
async fn clear_memory_demotes_everything() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());
    cache.write_raw("a", b"1").await.unwrap();
    cache.write_projected("b", b"2").await.unwrap();
    assert_eq!(cache.memory_entries().await, 2);
    assert_eq!(cache.clear_memory().await, 2);
    assert_eq!(cache.memory_entries().await, 0);
    // Nothing durable was lost.
    assert_eq!(cache.read_raw("a").await.unwrap().unwrap().payload(), b"1");
    assert_eq!(backing.read_raw_calls(), 1);
}

#[tokio::test]
async fn backing_read_failure_is_typed_and_never_promoted() {
    let backing = CountingBacking::default();
    backing.seed_raw("poisoned", b"bytes", false);
    let cache = TwoStageParseCache::new(backing.clone());
    backing.state.fail_reads.store(true, Ordering::SeqCst);

    let failure = cache
        .read_raw("poisoned")
        .await
        .expect_err("a backing read failure is a typed error, never a miss");
    assert_eq!(failure.stage(), ParseCacheStage::RawTei);
    assert_eq!(failure.code(), CacheIntegrityCode::ChecksumMismatch);
    assert_eq!(
        cache.memory_entries().await,
        0,
        "failures are never promoted into the memory tier"
    );

    // The failure is not cached either: recovery is immediate.
    backing.state.fail_reads.store(false, Ordering::SeqCst);
    let read = cache.read_raw("poisoned").await.unwrap().unwrap();
    assert_eq!(read.payload(), b"bytes");
    assert_eq!(
        backing.read_raw_calls(),
        2,
        "failed read consulted backing again"
    );
}

#[tokio::test]
async fn backing_write_failure_propagates_and_memory_stays_empty() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::new(backing.clone());
    backing.state.fail_writes.store(true, Ordering::SeqCst);

    let failure = cache
        .write_raw("doomed", b"bytes")
        .await
        .expect_err("a backing write failure propagates as a typed error");
    assert_eq!(failure.code(), CacheIntegrityCode::StageWriteFailed);
    assert_eq!(
        cache.memory_entries().await,
        0,
        "a failed durable publish must never populate the memory tier"
    );
    assert!(cache.read_raw("doomed").await.unwrap().is_none());

    backing.state.fail_writes.store(false, Ordering::SeqCst);
    cache.write_raw("doomed", b"bytes").await.unwrap();
    assert_eq!(cache.memory_entries().await, 1);
    assert_eq!(
        backing.read_raw_calls(),
        1,
        "read after failed write misses"
    );
}

#[tokio::test]
async fn memory_capacity_evicts_fifo_deterministically() {
    let backing = CountingBacking::default();
    let cache = TwoStageParseCache::with_memory_capacity(backing.clone(), nz(2));
    cache.write_raw("first", b"1").await.unwrap();
    cache.write_raw("second", b"2").await.unwrap();
    cache.write_raw("third", b"3").await.unwrap();
    assert_eq!(cache.memory_entries().await, 2, "capacity is respected");

    // The oldest inserted entry ("first") was evicted FIFO: reading it
    // consults the backing tier.
    assert_eq!(
        cache.read_raw("first").await.unwrap().unwrap().payload(),
        b"1"
    );
    assert_eq!(
        backing.read_raw_calls(),
        1,
        "evicted entry falls back to backing"
    );

    // The survivors at read time are still memory hits.
    assert_eq!(
        cache.read_raw("third").await.unwrap().unwrap().payload(),
        b"3"
    );
    assert_eq!(backing.read_raw_calls(), 1);

    // FIFO promotion: re-admitting "first" evicted the next-oldest
    // ("second") — still durable in the backing tier, never lost.
    assert_eq!(
        cache.read_raw("second").await.unwrap().unwrap().payload(),
        b"2"
    );
    assert_eq!(
        backing.read_raw_calls(),
        2,
        "second was demoted by FIFO promotion and re-read from backing"
    );
    assert_eq!(cache.memory_entries().await, 2);
}

#[test]
fn default_capacity_is_a_documented_constant() {
    const {
        assert!(MEMORY_TIER_DEFAULT_CAPACITY >= 1);
    }
}

// ───────────────────────── executor over the two-stage cache ─────────────────────────

fn batch_limits(workers: usize, queue: usize, in_flight: usize, docs: usize) -> ParseBatchLimits {
    ParseBatchLimits::new(nz(workers), nz(queue), nz(in_flight), nz(docs))
}

#[tokio::test]
async fn write_through_then_read_only_replays_from_memory_without_backing_or_backend() {
    let backing = CountingBacking::default();
    let backend = BatchBackend::default();
    let cache = TwoStageParseCache::new(backing.clone());
    let ids = ["doc-a", "doc-b", "doc-c"];
    let requests: Vec<ParseBackendRequest> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| request(id, index as u8 + 1))
        .collect();

    // Run 1: write-through populates both tiers.
    let first_batch = ParseBatch::admit(
        CacheMode::WriteThrough,
        batch_limits(2, 4, 2, 64),
        requests.clone(),
        None,
    )
    .expect("run 1 admits");
    let executor = ParseBatchExecutor::new(backend.clone(), cache);
    let sink = Arc::new(ParseBatchCollector::new());
    let summary = executor
        .run(
            first_batch,
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.succeeded(), 3);
    assert_eq!(
        summary.backend_calls(),
        3,
        "cold run: one call per document"
    );
    assert_eq!(summary.raw_writes(), 3);
    assert_eq!(summary.projector_writes(), 3);
    assert_eq!(backing.state.write_raw_calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        backing.state.write_projected_calls.load(Ordering::SeqCst),
        3
    );
    let backend_calls_after_run1 = total_backend_calls(&backend);

    // Run 2: read-only replay in the SAME process — every stage read must
    // be a MEMORY hit: zero backend calls, zero backing reads, zero writes.
    let second_batch = ParseBatch::admit(
        CacheMode::ReadOnly,
        batch_limits(2, 4, 2, 64),
        requests,
        None,
    )
    .expect("run 2 admits");
    let sink2 = Arc::new(ParseBatchCollector::new());
    let summary2 = executor
        .run(
            second_batch,
            sink2.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary2.succeeded(), 3);
    assert_eq!(summary2.backend_calls(), 0, "read-only replay: no backend");
    assert_eq!(summary2.raw_hits(), 3);
    assert_eq!(summary2.projector_hits(), 3);
    assert_eq!(summary2.raw_writes(), 0);
    assert_eq!(summary2.projector_writes(), 0);
    assert_eq!(
        total_backend_calls(&backend),
        backend_calls_after_run1,
        "the replay must not touch the backend at all"
    );
    assert_eq!(
        backing.read_raw_calls(),
        3,
        "run 1 misses: exactly one backing raw read per document"
    );
    assert_eq!(
        backing.read_projected_calls(),
        3,
        "run 1 projector misses only; run 2 is pure memory"
    );

    // Semantic parity: the replayed articles carry the same result
    // identities as the write-through originals.
    let mut first_identities: Vec<(u32, String)> = sink
        .outcomes_sorted()
        .await
        .into_iter()
        .map(|outcome: ParseDocumentOutcome| {
            (
                outcome.ordinal(),
                outcome
                    .result()
                    .as_ref()
                    .expect("run 1 succeeded")
                    .result_identity()
                    .to_string(),
            )
        })
        .collect();
    first_identities.sort_by_key(|(ordinal, _)| *ordinal);
    let mut second_identities: Vec<(u32, String)> = sink2
        .outcomes_sorted()
        .await
        .into_iter()
        .map(|outcome: ParseDocumentOutcome| {
            (
                outcome.ordinal(),
                outcome
                    .result()
                    .as_ref()
                    .expect("run 2 succeeded")
                    .result_identity()
                    .to_string(),
            )
        })
        .collect();
    second_identities.sort_by_key(|(ordinal, _)| *ordinal);
    assert_eq!(
        first_identities, second_identities,
        "cache modes change cost, never the semantic output"
    );
}

fn total_backend_calls(backend: &BatchBackend) -> usize {
    backend.state.calls.lock().unwrap().values().sum()
}

#[tokio::test]
async fn partial_failure_is_deterministic_one_correlated_outcome_per_ordinal() {
    let backing = CountingBacking::default();
    let backend = BatchBackend::default()
        .with_busy("doc-b", 503, 7)
        .with_busy("doc-e", 429, 3);
    let cache = TwoStageParseCache::new(backing);
    let ids = ["doc-a", "doc-b", "doc-c", "doc-d", "doc-e", "doc-f"];
    let requests: Vec<ParseBackendRequest> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| request(id, index as u8 + 1))
        .collect();
    let batch = ParseBatch::admit(
        CacheMode::WriteThrough,
        batch_limits(2, 4, 2, 64),
        requests,
        None,
    )
    .expect("batch admits");

    let executor = ParseBatchExecutor::new(backend.clone(), cache);
    let sink = Arc::new(ParseBatchCollector::new());
    let summary = executor
        .run(
            batch,
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;

    // Exactly one outcome per admitted ordinal, correlated to its input.
    let outcomes = sink.outcomes_sorted().await;
    assert_eq!(
        outcomes.len(),
        ids.len(),
        "one terminal outcome per document"
    );
    let ordinals: Vec<u32> = outcomes.iter().map(|o| o.ordinal()).collect();
    let expected: Vec<u32> = (0..ids.len() as u32).collect();
    assert_eq!(
        ordinals, expected,
        "outcomes are unique and ordinally complete"
    );
    for (outcome, id) in outcomes.iter().zip(ids.iter()) {
        assert_eq!(
            outcome.paper_id(),
            *id,
            "outcome {id} must correlate to its input"
        );
    }

    // The partial failures stay typed, per-document and non-duplicating.
    assert_eq!(summary.completed(), 6);
    assert_eq!(summary.succeeded(), 4);
    assert_eq!(summary.failed(), 2);
    assert_eq!(
        backend.call_count("doc-b"),
        1,
        "typed failure: exactly one attempt"
    );
    assert_eq!(
        backend.call_count("doc-e"),
        1,
        "typed failure: exactly one attempt"
    );
    assert_eq!(
        backend.call_count("doc-f"),
        1,
        "later documents survive earlier failures"
    );
    for outcome in &outcomes {
        if outcome.is_success() {
            continue;
        }
        match outcome.result() {
            Err(ParseDocumentFailure::Backend(failure)) => {
                assert!(
                    failure.retry_after().is_some(),
                    "Busy Retry-After metadata must survive into the outcome"
                );
            }
            other => panic!("expected a typed backend failure, got {other:?}"),
        }
    }

    // The configured concurrency bound held: the semaphore caps
    // simultaneously EXECUTING documents (the enforced bound), and the
    // queue gauge counts admission-pipeline items — the bounded channel
    // buffers at most `queue_capacity` while the feeder's single blocked
    // hand-off is counted too, so the gauge tops out at capacity + 1.
    assert!(
        backend.max_in_flight() <= 2,
        "true overlap is allowed but the bound is never exceeded (observed {})",
        backend.max_in_flight()
    );
    assert!(summary.max_in_flight_observed() <= 2);
    assert!(summary.max_queue_depth_observed() <= 5);
}
