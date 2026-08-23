# M167 UniversalKBQueue Concurrency Result

## Verdict

**Item 2 status: CLOSED for focused review and contract-probe scope.**

M167 did not run a long multi-process stress test, but it did review queue concurrency semantics, harden the primary claim race shape, and add an executable multi-connection claim contract.

## Change

`UniversalKBQueue.claim()` now checks whether its guarded `UPDATE ... WHERE job_id = ? AND status IN (...)` actually changed one row before inserting a claim event or returning a job.

If `cursor.rowcount != 1`, the method returns `None`. This prevents a worker that lost a claim race from emitting a false `claim` event or returning a row it does not own.

## Test added

`tests/test_universal_kb_queue.py::test_multi_connection_claim_allows_only_one_worker`

The test:

1. creates one SQLite queue DB,
2. starts two separate `UniversalKBQueue` instances in two threads,
3. has both attempt to claim the same ready job,
4. asserts exactly one worker receives the job,
5. asserts the event log contains exactly one `claim`,
6. asserts a third worker cannot claim while the lease is active.

## Verification

```text
uv run pytest tests/test_universal_kb_queue.py -q
23 passed
```

## Remaining limitation

This is a focused contention contract, not a full stress test. Before high-concurrency queue activation, a future milestone should run longer multi-process stress checks around claim, heartbeat, reclaim, complete/fail races, and SQLite busy timeout behavior.
