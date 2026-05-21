# S04: Locator semantic review and recommendation

**Goal:** Review locator semantics and produce a go/defer recommendation for future KG import readiness work.
**Demo:** After S04, independent review decides whether candidate locators are meaningful enough to justify a future positive import-gate milestone.

## Must-Haves

- Independent semantic review completed.
- Recommendation explicitly says go/defer for future positive import-gate work.
- R048 updated with validation or blocker evidence.
- Existing safety gates remain intact.

## Proof Level

- This slice proves: Independent review plus final guard.

## Integration Closure

Closes M020 and determines next KG step.

## Verification

- Records review findings and final guard for downstream planning.

## Tasks

- [x] **T01: Independent locator semantic review** `est:45m`
  Run an independent review over the S01-S03 artifacts. The reviewer should evaluate protocol sufficiency, one-paper fixture meaningfulness, small-batch ambiguity, redaction/safety boundaries, and whether future work should implement deterministic locators, improve chunking, or create review packets before any positive import gate.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md`
  - Verify: review artifact exists and contains PASS/FLAG verdict plus recommendation

- [x] **T02: Finalize M020 recommendation and guard** `est:45m`
  Write final M020 guard and recommendation, update R048, and validate milestone safety invariants before completion.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json`, `.gsd/milestones/M020-uh5kvt/slices/S04/final-locator-recommendation.md`
  - Verify: uv run python inline final guard assertions

## Files Likely Touched

- .gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md
- .gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json
- .gsd/milestones/M020-uh5kvt/slices/S04/final-locator-recommendation.md
