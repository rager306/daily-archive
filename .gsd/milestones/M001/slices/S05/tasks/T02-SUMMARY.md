---
id: T02
parent: S05
milestone: M001
key_files:
  - src/arxiv_archive/cli.py
  - tests/test_analysis.py
key_decisions:
  - Queue state is persisted by overwriting one per-date JSON file under `QUEUE_DIR`, with `running`, `done`, `empty`, and `failed` statuses and last-writer-wins semantics.
duration: 
verification_result: passed
completed_at: 2026-05-16T16:32:44.838Z
blocker_discovered: false
---

# T02: Persisted cron/Hermes queue state JSON transitions around the daily CLI analysis run.

**Persisted cron/Hermes queue state JSON transitions around the daily CLI analysis run.**

## What Happened

Added `QUEUE_DIR` and a `QueueStateStatus` contract in `src/arxiv_archive/cli.py`, plus `write_state_json()` to create `~/.research/ops/queue`, overwrite the per-date JSON file, serialize UTC timestamps with the existing portable `Z` timestamp helper, and include `error` only when non-empty. Updated `run()` so valid dates write `running`/`fetch` before analysis, successful runs write `done` or `empty` with `done` stage, and raised analysis exceptions write `failed`/`failed` with `str(exc)` before reraising to preserve non-zero CLI behavior. Added focused tests in `tests/test_analysis.py` with monkeypatched `QUEUE_DIR` for schema, empty final state, and failure final state without tracebacks.

## Verification

Ran the task-required focused pytest command and the slice lint check for touched files. `uv run pytest tests/test_analysis.py -q` passed with 17 tests. `uv run ruff check src/arxiv_archive/cli.py tests/test_analysis.py` passed with no lint findings.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_analysis.py -q` | 0 | ✅ pass | 1721ms |
| 2 | `uv run ruff check src/arxiv_archive/cli.py tests/test_analysis.py` | 0 | ✅ pass | 99ms |

## Deviations

GitNexus and Skill activation tools requested by project instructions were not exposed in the available tool namespace for this session, so the change was kept localized to the two planned files and verified with focused tests plus ruff.

## Known Issues

None.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
- `tests/test_analysis.py`
