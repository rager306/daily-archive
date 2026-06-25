# M167 UniversalKBQueue Concurrency Recon

## Verdict

**Item 2 status after recon: partially covered; needs focused multi-connection probe and small claim hardening.**

Existing tests cover lease semantics well in a single queue instance. They do not fully prove contention between two SQLite connections or simultaneous workers. The implementation is close, but `claim()` currently performs a `SELECT` followed by an `UPDATE` and does not check the update row count before inserting a claim event and returning the fetched job.

## Implementation evidence

Reviewed file: `src/research_graph/workflows/universal_kb/queue.py`

Key properties:

- `sqlite3.connect(self.db_path)` with one connection per queue object.
- `PRAGMA busy_timeout` configured.
- `PRAGMA foreign_keys = ON` configured.
- `PRAGMA journal_mode = WAL` configured in `initialize()`.
- Mutating methods use `with self.connection:` transaction blocks.
- Claimable index exists: `idx_jobs_claimable`.
- Lease recovery index exists: `idx_jobs_lease_recovery`.

## Existing test coverage

Reviewed file: `tests/test_universal_kb_queue.py`

Existing lease/concurrency-adjacent tests include:

- `test_claim_is_exclusive_and_sets_lease_fields`
- `test_heartbeat_extends_matching_lease_and_rejects_wrong_owner`
- `test_complete_persists_safe_outputs_and_clears_lease`
- `test_retryable_failure_respects_retry_after_before_claim`
- `test_expired_lease_reclaims_to_ready_until_attempts_exhausted`

These are useful and should remain. They prove important state transitions, owner checks, and stale lease behavior.

## Concurrency-critical operations

| Operation | Current behavior | Risk |
|---|---|---|
| `claim()` | Selects first ready/retryable row, then updates it to running in same transaction | Race-prone shape unless update rowcount is checked or claim is atomic. |
| `heartbeat()` | Requires running owner before update | Good owner contract, but precheck happens outside transaction. |
| `complete()` | Requires running owner before update, clears lease | Good owner contract, but precheck happens outside transaction. |
| `fail_retryable()` / `fail_terminal()` | Requires running owner and clears lease | Same owner precheck concern. |
| `reclaim_expired_leases()` | Selects all expired running rows then updates each | Good sequential behavior; multi-worker reclaim should avoid double event insertion. |

## Main finding

### Q1 — `claim()` should verify it won the state transition

`claim()` currently:

1. selects a ready/retryable row,
2. updates it with `WHERE job_id = ? AND status IN ('ready', 'failed_retryable')`,
3. inserts a claim event,
4. fetches and returns the row.

If another connection claims the row between select and update, the `UPDATE` can affect zero rows. The method should treat that as a lost race and return `None` or retry. Without a rowcount check, a losing worker can still emit a claim event and return a row it does not own.

This is hard to trigger deterministically without intrusive synchronization, but the code shape is enough to justify a small hardening and a multi-connection probe.

## S05 plan

1. Add a multi-connection test using two `UniversalKBQueue` instances on the same DB path.
2. Add a regression test for claim contention behavior: only one worker gets the job and exactly one claim event is emitted.
3. Harden `claim()` by checking `cursor.rowcount`; if it is zero, return `None` without emitting a claim event.
4. Run full `tests/test_universal_kb_queue.py`.

## Remaining gap after S05

Even with a multi-connection probe, this will not be a full multi-process stress test. A later queue activation milestone should add stress or transaction-level tests if the queue becomes a high-concurrency worker substrate.
