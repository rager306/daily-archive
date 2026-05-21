---
id: S03
parent: M021-xcfj4p
milestone: M021-xcfj4p
provides:
  - Deterministic batch artifact for S04 independent review.
  - Metrics: 10 papers, 26 locators, 19 ambiguous spans, 0 import-eligible locators, 0 fact promotions.
requires:
  - slice: S02
    provides: Candidate locator module and tests.
affects:
  []
key_files:
  - src/arxiv_archive/candidate_locators.py
  - tests/test_candidate_locators.py
  - .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json
  - .gsd/milestones/M021-xcfj4p/slices/S03/run-evidence/deterministic-locator-batch-guard.json
key_decisions:
  - Use M011 counts_by_route to filter route specs per paper.
  - Treat reduced ambiguity as better reviewability, not semantic correctness.
  - Continue deferring positive import gate.
patterns_established:
  - Route metadata filtering reduces locator noise before semantic review.
  - M020 comparison metrics should be persisted in guards when improving protocol implementations.
  - Reduced ambiguity is useful but still not import readiness.
observability_surfaces:
  - deterministic-locator-batch-guard.json records validity, safety, and M020 comparison metrics.
  - deterministic-locator-batch-report.md records state and diagnostic counts.
drill_down_paths:
  - .gsd/milestones/M021-xcfj4p/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S03/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T10:32:10.123Z
blocker_discovered: false
---

# S03: Bounded batch implementation rehearsal

**Generated and verified deterministic bounded locator batch evidence.**

## What Happened

S03 extended the implemented module with a bounded batch helper for M011-style targets and used it to generate deterministic run evidence over the 10-paper M011 batch. Route filtering reduced locator count from M020's 35 to 26 and ambiguous spans from 27 to 19. The output includes source ledgers, locators, per-paper summaries, aggregate diagnostics, and safety flags. The guard passed, tests and ruff passed, and LSP reported no diagnostics.

## Verification

Fresh verification passed: 10 tests, ruff clean, LSP no diagnostics, m021-s03-deterministic-batch-guard-ok, and m021-s03-final-verification-ok.

## Requirements Advanced

- R049 — S03 exercised R049's deterministic implementation over bounded targets and produced ambiguity diagnostics.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Route-spec subset handling and test fixture source filenames were corrected during TDD. No plan-invalidating deviations.

## Known Limitations

The deterministic run still has 19 ambiguous spans and no semantic fact validation. Positive import remains blocked.

## Follow-ups

S04 should independently review whether route filtering made outputs more reviewable and whether next work should be chunk/structure repair, reviewer packets, or stronger route-specific heuristics.

## Files Created/Modified

- `src/arxiv_archive/candidate_locators.py` — Added bounded batch helper and route metadata filtering.
- `tests/test_candidate_locators.py` — Added batch helper and route filtering tests.
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json` — Deterministic batch run over M011 targets.
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md` — Batch report with counts and diagnostics.
- `.gsd/milestones/M021-xcfj4p/slices/S03/run-evidence/deterministic-locator-batch-guard.json` — Guard validating S03 batch safety and M020 comparison.
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-batch-recommendation.md` — Recommendation for independent review and import deferral.
