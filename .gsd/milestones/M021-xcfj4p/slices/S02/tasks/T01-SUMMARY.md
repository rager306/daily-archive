---
id: T01
parent: S02
milestone: M021-xcfj4p
key_files:
  - tests/test_candidate_locators.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T10:18:45.006Z
blocker_discovered: false
---

# T01: Added candidate locator tests covering diagnostics and safety invariants.

**Added candidate locator tests covering diagnostics and safety invariants.**

## What Happened

Wrote tests first for deterministic candidate locator generation. The tests cover safe artifact generation, source hash mismatch, broad-signal ambiguity, missing signal, recursive forbidden payload key detection, invalid coordinate detection, safe writer behavior, and writer rejection for invalid artifacts.

## Verification

Initial test run failed with ModuleNotFoundError as expected before implementation. Final focused verification passed: 8 tests passed and ruff was clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_candidate_locators.py -q before implementation` | 2 | ✅ expected red: ModuleNotFoundError | 6000ms |
| 2 | `uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py` | 0 | ✅ pass: 8 passed; All checks passed! | 9400ms |

## Deviations

The red test initially failed at collection because the module did not exist, as expected. One test then needed correction to use the method route when validating coordinate corruption.

## Known Issues

None.

## Files Created/Modified

- `tests/test_candidate_locators.py`
