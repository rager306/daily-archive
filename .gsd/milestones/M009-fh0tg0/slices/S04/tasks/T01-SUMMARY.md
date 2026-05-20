---
id: T01
parent: S04
milestone: M009-fh0tg0
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - tests/test_validation_batch_top_up.py
key_decisions:
  - Top-up planner is deterministic and read-only: it does not acquire sources or mutate batch state.
  - Candidate readiness is inferred from redacted inventory availability metadata.
  - The planner respects `max_candidates_to_consider` and reports an explicit blocker when quota remains short.
duration: 
verification_result: passed
completed_at: 2026-05-20T05:21:11.589Z
blocker_discovered: false
---

# T01: Implemented bounded quota top-up planning with explicit shortage/blocker reporting.

**Implemented bounded quota top-up planning with explicit shortage/blocker reporting.**

## What Happened

Added bounded top-up planning helpers to the validation batch workflow. The planner consumes current batch readiness and a candidate inventory, skips already selected papers, considers candidates in deterministic inventory order up to a max bound, accepts only source-ready replacements, records rejected candidates, computes remaining shortage, and sets scan_allowed only when the target quota is filled. It can also write summary and diagnostics artifacts with an explicit `bounded_top_up_shortage` blocker.

## Verification

Quota-fill and top-up tests passed, and ruff passed for touched files.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_top_up.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py` | 0 | ✅ pass — 8 tests passed; ruff passed | 5700ms |

## Deviations

Implemented tests in the same step as the helper; T02 records the expanded behavioral coverage separately.

## Known Issues

This is planning/reporting automation, not a full acquisition loop. Future integration must connect accepted replacements to real preflight/acquisition runs.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_top_up.py`
