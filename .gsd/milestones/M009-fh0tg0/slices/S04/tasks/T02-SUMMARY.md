---
id: T02
parent: S04
milestone: M009-fh0tg0
key_files:
  - tests/test_validation_batch_top_up.py
key_decisions:
  - Test both successful replacement and bounded failure cases.
  - Assert selected candidates are excluded so replacements cannot duplicate the existing batch.
  - Assert blocker diagnostics are written for unresolved shortages.
duration: 
verification_result: passed
completed_at: 2026-05-20T05:21:27.546Z
blocker_discovered: false
---

# T02: Added bounded top-up tests for success, shortage, duplicate exclusion, and blocker diagnostics.

**Added bounded top-up tests for success, shortage, duplicate exclusion, and blocker diagnostics.**

## What Happened

Added top-up behavior tests covering an already full quota, underfilled quota with enough deterministic replacements, max-candidate exhaustion that blocks scan, exclusion of already selected candidates, and blocker diagnostic writing for unresolved shortages. The tests confirm safety flags remain false and no raw content is embedded.

## Verification

Top-up tests and quota-fill tests passed with ruff.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_top_up.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py` | 0 | ✅ pass — 8 tests passed; ruff passed | 5700ms |

## Deviations

Tests were created while implementing T01, then verified as the T02 behavior contract.

## Known Issues

Tests use redacted in-memory candidate availability metadata; they do not perform source acquisition.

## Files Created/Modified

- `tests/test_validation_batch_top_up.py`
