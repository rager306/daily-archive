# S04: S04

**Goal:** Review deterministic locator outputs and close the milestone with next-step recommendation.
**Demo:** After S04, independent review decides whether deterministic locators are useful enough for reviewer packets/chunk repair next, still not positive import.

## Must-Haves

- Independent review completed.
- R049 validated or blocked with evidence.
- Final guard proves no import/write/raw payloads.
- Recommendation explicitly avoids positive import unless evidence supports it.

## Proof Level

- This slice proves: Independent review plus final verification.

## Integration Closure

Determines next KG milestone after deterministic implementation.

## Verification

- Records independent findings and final guard.

## Tasks

- [x] **T01: Completed independent review and remediated concrete locator gaps.** `est:45m`
  Run independent review over M021 design, module/tests, S03 batch artifact/guard, and M020 comparison. Assess reproducibility, safety, ambiguity diagnostics, and whether next work should be chunk/structure repair, reviewer packets, route heuristics, or positive import.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S04/independent-deterministic-locator-review.md`
  - Verify: review artifact contains PASS/FLAG verdict and recommendation

- [x] **T02: Finalized M021 recommendation and guard.** `est:45m`
  Write final M021 guard and recommendation, update R049, and run final verification before milestone closeout.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S04/run-evidence/final-deterministic-locator-guard.json`, `.gsd/milestones/M021-xcfj4p/slices/S04/final-deterministic-locator-recommendation.md`
  - Verify: uv run python inline final guard assertions

## Files Likely Touched

- .gsd/milestones/M021-xcfj4p/slices/S04/independent-deterministic-locator-review.md
- .gsd/milestones/M021-xcfj4p/slices/S04/run-evidence/final-deterministic-locator-guard.json
- .gsd/milestones/M021-xcfj4p/slices/S04/final-deterministic-locator-recommendation.md
