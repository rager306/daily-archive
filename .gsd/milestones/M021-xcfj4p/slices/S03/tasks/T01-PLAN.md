---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Added deterministic bounded batch helper for M011-style targets.

Add module-level helper(s) and tests for building a deterministic candidate locator batch from M011-style target records. The helper must preserve source path/hash checks, route specs, per-paper summaries, and no-import safety flags.

## Inputs

- `src/arxiv_archive/candidate_locators.py`
- `tests/test_candidate_locators.py`
- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `src/arxiv_archive/candidate_locators.py`
- `tests/test_candidate_locators.py`

## Verification

uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py

## Observability Impact

Adds reproducible batch helper and tests for per-paper diagnostics.
