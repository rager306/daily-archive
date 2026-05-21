---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify candidate locator module

Run focused verification and diagnostics for changed files, including pytest, ruff, and LSP diagnostics if available.

## Inputs

- `src/arxiv_archive/candidate_locators.py`
- `tests/test_candidate_locators.py`

## Expected Output

- `.gsd/milestones/M021-xcfj4p/slices/S02/run-evidence/candidate-locator-module-guard.json`

## Verification

uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py && uv run python inline guard

## Observability Impact

Records module-level pass/fail evidence and safety summary.
