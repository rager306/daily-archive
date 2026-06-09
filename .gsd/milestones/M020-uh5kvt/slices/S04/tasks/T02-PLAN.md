---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Finalized M020 recommendation and guard.

Write final M020 guard and recommendation, update R048, and validate milestone safety invariants before completion.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md`
- `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S04/final-locator-recommendation.md`

## Verification

uv run python inline final guard assertions

## Observability Impact

Provides final downstream decision and safety guard.
