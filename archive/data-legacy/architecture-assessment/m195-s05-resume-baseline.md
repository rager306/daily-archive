# M195 S05 Resume and Artifact Safety Baseline

## Verdict

**PASS: S05 can proceed with targeted queue edits.** Method-level GitNexus impact is LOW for the specific lifecycle hooks, while the class-level import surface is MEDIUM because queue consumers import `UniversalKBQueue` directly. No HIGH or CRITICAL impact was reported.

## GitNexus planning evidence

| Target | Result |
|---|---|
| `UniversalKBQueue.initialize#0` | LOW, impactedCount=0, processes_affected=0 |
| `UniversalKBQueue.add_dependency#5` | LOW, impactedCount=0, processes_affected=0 |
| `UniversalKBQueue._dependencies_satisfied#1` | LOW, impactedCount=4, direct=2, processes_affected=2 |
| `UniversalKBQueue.mark_stale#4` | LOW, impactedCount=0, processes_affected=0 |
| `UniversalKBQueue` class | MEDIUM, direct import surface: queue tests, soak script, substrate rehearsal, smoke runner, no-write rehearsal |

## Existing behavior

- Enqueue is idempotent by `job_id` and records a single enqueue event.
- Artifact dependencies without a hash-backed registry do not unblock jobs.
- `mark_stale` detects input hash, tool version, and contract version drift.
- Expired leases reclaim to `ready` until attempts are exhausted.
- Retryable failures respect `retry_after` before claim.
- Queue diagnostics reject raw payload refs and secret-shaped values.

## S05 gap

Artifact dependencies already fail closed, but there is no metadata-only way to record that an expected artifact ref/hash exists. That means downstream resume cannot distinguish:

1. missing artifact,
2. present artifact with stale hash,
3. present artifact with expected hash.

## Minimal edit boundary

Use a small local SQLite artifact registry owned by `UniversalKBQueue`:

- Add `artifact_refs` table with `artifact_ref`, `artifact_hash`, and timestamps.
- Add `register_artifact(artifact_ref, artifact_hash)` with metadata-ref validation and non-empty hash validation.
- Update `_dependencies_satisfied` so artifact dependencies are satisfied only when:
  - the artifact ref is registered, and
  - `expected_hash` is present and matches the registered hash.
- Emit an `artifact_registered` event for auditability without payload values.
- Do not read files, calculate hashes, write graph state, or promote import eligibility.

## Required compatibility checks

- `tests/test_universal_kb_queue.py`
- `tests/test_universal_kb_rehearsal.py`
- `tests/test_universal_kb_substrate_rehearsal.py`

## Disallowed in S05

- No production graph writes.
- No LadybugDB/FalkorDB writes.
- No NetworkX adapter changes.
- No live network or arXiv probing.
- No LLM provider calls.
- No scheduler or distributed queue abstraction.
