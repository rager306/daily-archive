# M169 Queue Soak Design

## Verdict

**Implement a bounded multiprocess queue soak in S10.**

The existing M168 stress test proves separate SQLite connections across threads. M169 should add a distinct process-level proof using separate Python processes, each with its own `UniversalKBQueue` connection, while keeping the test short and diagnostic.

## Current baseline

Current queue suite:

```text
uv run pytest tests/test_universal_kb_queue.py -q
24 passed
```

M168 thread stress:

```text
test_bounded_multi_worker_stress_claims_and_completes_each_job_once
job_count=24
worker_count=6
threads with separate UniversalKBQueue connections
assert each job gets exactly one claim event and one complete event
```

Relevant API:

- `UniversalKBQueue(db_path).initialize()` opens a SQLite connection and enables WAL.
- `enqueue(...)` creates pending jobs.
- `unblock_ready_jobs()` moves jobs to ready.
- `claim(worker_id=..., lease_seconds=...)` uses an atomic `UPDATE ... WHERE job_id = ? AND status IN (...)` and checks `cursor.rowcount`.
- `complete(job_id, worker_id=..., output_paths=...)` requires running ownership and writes the complete event.

## S10 multiprocess contract

Add a new test in `tests/test_universal_kb_queue.py` with these bounds:

```text
job_count=16
process_count=4
join_timeout_seconds=15
lease_seconds=30
```

Worker behavior:

1. Each process opens its own `UniversalKBQueue(db_path).initialize()` connection.
2. Each process loops:
   - `claim(worker_id=..., lease_seconds=30)`;
   - if `None`, exit normally;
   - `complete(job_id, worker_id=..., output_paths=(...))`;
   - send a structured result tuple to a multiprocessing result queue.
3. Exceptions are caught and sent to the result queue as diagnostic error tuples.
4. Each process closes its queue connection in `finally`.

Parent assertions:

- No process remains alive after the timeout.
- No worker error tuples were produced.
- Exactly `job_count` claim or complete result tuples were produced.
- Claimed job ids are unique.
- Each final job status is `succeeded`.
- Each job has exactly one `claim` event and one `complete` event.

## Why these bounds

- `16` jobs and `4` processes are enough to exercise process-level contention without creating a slow soak.
- The test is a normal pytest unit/integration check, not an unbounded soak loop.
- A hard join timeout prevents stalled workers from hanging closeout.
- Structured result tuples make failures diagnosable without scraping process stderr.

## Implementation notes

- Prefer top-level worker helper functions so multiprocessing can pickle them if the platform uses spawn semantics.
- Avoid sharing `FixedClock` across processes; use the queue default clock for the multiprocess test.
- Use a unique `tmp_path / "queue.sqlite"` database and initialize/enqueue in the parent before spawning workers.
- Do not change queue internals unless the test exposes a real bug.

## Stop conditions

Stop and record a blocker instead of forcing S10 if:

- the multiprocess test is flaky across repeated local runs;
- the test requires sleeps or unbounded polling;
- process workers hang even with the timeout;
- a queue internals change is required without a reproducible race failure and impact analysis.
