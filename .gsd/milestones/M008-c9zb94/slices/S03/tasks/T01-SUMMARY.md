---
id: T01
parent: S03
milestone: M008-c9zb94
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - tests/test_validation_batch_quota_fill.py
key_decisions:
  - Added quota-fill as a separate workflow helper to avoid changing the high-blast-radius validation_batch_state_preview function.
  - Quota-fill helper is read-only and does not acquire sources or mutate state; future top-up loops can consume its replacement candidate metadata.
duration: 
verification_result: passed
completed_at: 2026-05-20T03:59:46.350Z
blocker_discovered: false
---

# T01: Implemented and tested the quota-fill gate helpers for validation batches.

**Implemented and tested the quota-fill gate helpers for validation batches.**

## What Happened

Implemented quota-fill helpers that classify selected papers by final source readiness, compute accepted_ready_count and shortage_count, expose replacement candidates from a deterministic inventory, and write redacted summary/diagnostic artifacts. Tests cover a fully ready quota, an underfilled quota with replacement candidates, and diagnostic artifact writing.

## Verification

Focused quota-fill and validation batch workflow tests passed, and ruff passed for touched files.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_quota_fill.py` | 0 | ✅ pass — 11 tests passed; ruff passed | 5100ms |

## Deviations

None.

## Known Issues

Current helper emits replacement candidates but does not itself implement a looping top-up acquisition command. That remains future workflow expansion if a batch is underfilled.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_quota_fill.py`
