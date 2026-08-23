# M168 Queue Stress Recon

## Current state

M167 added a focused two-connection claim race test and hardened `claim()` with a `cursor.rowcount` check. M168 will add a broader bounded contention probe.

Relevant queue behavior:

- `initialize()` enables SQLite WAL, foreign keys, and a busy timeout.
- `enqueue()` is idempotent by `job_id` and emits one `enqueue` event.
- `unblock_ready_jobs()` moves pending jobs to `ready` when gates are open.
- `claim()` selects the next `ready`/retryable job and updates it to `running`; M167 now returns `None` if the guarded update loses.
- `complete()` requires the matching running lease owner and transitions to `succeeded`.
- `reclaim_expired_leases()` handles expired running leases separately.

## Existing proof

`tests/test_universal_kb_queue.py::test_multi_connection_claim_allows_only_one_worker` proves two concurrent connections cannot both claim the same single ready job.

Baseline:

```text
uv run pytest tests/test_universal_kb_queue.py -q
23 passed
```

## Selected stress model

Use a bounded thread stress test in `tests/test_universal_kb_queue.py`.

Parameters:

```text
job_count=24
worker_count=6
lease_seconds=30
barrier_timeout=5s
thread_join_timeout=10s
```

Each worker opens its own `UniversalKBQueue` connection, waits on a barrier, then loops:

1. `claim(worker_id=..., lease_seconds=30)`
2. if no job is claimable, exit loop;
3. `complete(job_id, worker_id=..., output_paths=(...))`
4. append claimed job ID to a shared results list protected by a lock.

## Assertions

The stress probe should assert:

- every worker thread terminates before timeout;
- exactly `job_count` jobs are claimed/completed;
- claimed job IDs are unique;
- every job has final `status='succeeded'`;
- each job has exactly one `claim` event and one `complete` event;
- no worker errors were captured.

## Why threads, not processes

Threads are sufficient for this local SQLite queue probe because each worker uses a separate SQLite connection. This keeps the test fast and deterministic while exercising the same database-level contention path that matters for `claim()`.

A later production-readiness milestone can add a slower multiprocess soak script if queue activation requires it. That is intentionally outside S06 to avoid slow/flaky closeout.

## Implementation target

Add one normal pytest test:

```text
tests/test_universal_kb_queue.py::test_bounded_multi_worker_stress_claims_and_completes_each_job_once
```

Expected runtime should remain well under one second on the current suite.

## Verification target

```text
uv run pytest tests/test_universal_kb_queue.py -q
```

S06 should also run the new test repeatedly enough to catch obvious flakiness without turning it into a soak test.
