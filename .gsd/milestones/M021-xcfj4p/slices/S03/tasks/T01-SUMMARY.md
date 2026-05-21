---
id: T01
parent: S03
milestone: M021-xcfj4p
key_files:
  - src/arxiv_archive/candidate_locators.py
  - tests/test_candidate_locators.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T10:30:55.642Z
blocker_discovered: false
---

# T01: Added deterministic bounded batch helper for M011-style targets.

**Added deterministic bounded batch helper for M011-style targets.**

## What Happened

Added `build_candidate_locator_batch_from_targets` to generate deterministic candidate locator batches from M011-style target records. The helper filters route specs based on `counts_by_route`, preserves source path/hash checks, merges per-paper artifacts, and records per-paper summaries. Added tests for batch generation and route metadata filtering.

## Verification

Verified with focused pytest and ruff. Final S03 verification returned 10 passed, ruff clean, and m021-s03-final-verification-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py` | 0 | ✅ pass: 10 passed; All checks passed! | 5400ms |
| 2 | `lsp diagnostics src/arxiv_archive/candidate_locators.py and tests/test_candidate_locators.py` | 0 | ✅ pass: No diagnostics | 0ms |

## Deviations

The batch helper initially failed tests because route mapping assumed all default route specs were present; fixed to tolerate route-spec subsets. Test fixture also needed distinct source files to avoid accidental overwrite.

## Known Issues

None.

## Files Created/Modified

- `src/arxiv_archive/candidate_locators.py`
- `tests/test_candidate_locators.py`
