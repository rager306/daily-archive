# S03: S03

**Goal:** Add a callable or CLI rehearsal path that runs deterministic locators on the bounded M011 targets.
**Demo:** After S03, a bounded batch run uses the implemented module to reproduce M020-style artifacts and richer ambiguity diagnostics.

## Must-Haves

- Bounded batch generated from existing M011 targets.
- Artifact conforms to protocol and safety guard.
- Ambiguity diagnostics distinguish broad matches, missing signals, source/hash issues, and candidate-type uncertainty.
- Import/write remains blocked.

## Proof Level

- This slice proves: Integration test or command run over bounded artifacts.

## Integration Closure

Batch output feeds S04 review.

## Verification

- Produces reproducible run evidence and per-paper ambiguity diagnostics.

## Tasks

- [x] **T01: Added deterministic bounded batch helper for M011-style targets.** `est:60m`
  Add module-level helper(s) and tests for building a deterministic candidate locator batch from M011-style target records. The helper must preserve source path/hash checks, route specs, per-paper summaries, and no-import safety flags.
  - Files: `src/arxiv_archive/candidate_locators.py`, `tests/test_candidate_locators.py`
  - Verify: uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py

- [x] **T02: Ran deterministic bounded batch rehearsal over M011 targets.** `est:45m`
  Run the deterministic batch helper on the M011 bounded targets and persist M021 S03 run evidence with richer ambiguity diagnostics than M020.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json`, `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md`
  - Verify: uv run python inline batch generation and assertions

- [x] **T03: Validated deterministic batch guard and recommendation.** `est:30m`
  Validate the S03 batch guard and compare key metrics against M020, especially ambiguity, missing signals, and import-disabled safety flags.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S03/run-evidence/deterministic-locator-batch-guard.json`, `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-batch-recommendation.md`
  - Verify: uv run python inline S03 guard assertions

## Files Likely Touched

- src/arxiv_archive/candidate_locators.py
- tests/test_candidate_locators.py
- .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json
- .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md
- .gsd/milestones/M021-xcfj4p/slices/S03/run-evidence/deterministic-locator-batch-guard.json
- .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-batch-recommendation.md
