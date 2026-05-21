---
id: S04
parent: M021-xcfj4p
milestone: M021-xcfj4p
provides:
  - Validated deterministic candidate locator implementation.
  - Next recommendation: chunk/section structure repair plus reviewer packets.
requires:
  - slice: S02
    provides: Deterministic locator module.
  - slice: S03
    provides: Bounded batch output and diagnostics.
affects:
  []
key_files:
  - src/arxiv_archive/candidate_locators.py
  - tests/test_candidate_locators.py
  - .gsd/milestones/M021-xcfj4p/slices/S04/run-evidence/final-deterministic-locator-guard.json
  - .gsd/milestones/M021-xcfj4p/slices/S04/final-deterministic-locator-recommendation.md
key_decisions:
  - Validate R049 after remediating review findings.
  - Next work should be chunk structure repair and reviewer packets, not positive import.
  - Stable span identity must not depend on local source path.
patterns_established:
  - Independent review findings should be remediated inside the milestone when concrete and bounded.
  - Span hashes must use stable provenance fields, not local paths.
  - Overlap diagnostics are required for route-window ambiguity.
observability_surfaces:
  - final-deterministic-locator-guard.json records final metrics, remediation status, and safety flags.
  - independent-deterministic-locator-review.md records review findings and risks.
drill_down_paths:
  - .gsd/milestones/M021-xcfj4p/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S04/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T10:46:55.622Z
blocker_discovered: false
---

# S04: Review and final recommendation

**Completed review, remediation, and final recommendation for deterministic candidate locators.**

## What Happened

S04 reviewed the deterministic locator implementation and initially flagged two reproducibility/diagnostic gaps. Both were fixed before closeout. The final implementation now uses stable span hashes based on source ID, source hash, coordinate space, offsets, and route name, and it emits `overlapping_signal_window` diagnostics through a coordinate-only pass. Final verification passed with 12 tests, ruff clean, LSP no diagnostics, and final guard. R049 was validated. Positive import and LadybugDB writes remain blocked.

## Verification

Fresh final verification passed: 12 tests, ruff clean, LSP no diagnostics, m021-final-guard-ok, and m021-final-verification-ok.

## Requirements Advanced

None.

## Requirements Validated

- R049 — Validated by 12 focused tests, S02/S03/final guards, independent review, remediated stable span hashes and overlap diagnostics, and m021-final-verification-ok.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Independent review found two concrete implementation gaps during S04. They were fixed before final closeout rather than deferred: span hashes now use stable provenance fields, and overlap diagnostics are emitted and tested.

## Known Limitations

Semantic KG readiness remains unproven. 20 ambiguous spans remain in the deterministic batch.

## Follow-ups

Plan next milestone around chunk/section structure repair plus reviewer packet prototype. Positive import remains blocked.

## Files Created/Modified

- `.gsd/milestones/M021-xcfj4p/slices/S04/independent-deterministic-locator-review.md` — Independent review with initial FLAG and concrete findings.
- `.gsd/milestones/M021-xcfj4p/slices/S04/final-deterministic-locator-recommendation.md` — Final recommendation after remediation.
- `.gsd/milestones/M021-xcfj4p/slices/S04/run-evidence/final-deterministic-locator-guard.json` — Final guard proving remediation and safety gates.
- `src/arxiv_archive/candidate_locators.py` — Remediated implementation for stable span hashes and overlap diagnostics.
- `tests/test_candidate_locators.py` — Regression tests for stable span hashes and overlap diagnostics.
