---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Validated the small-batch locator guard and review recommendation.

Validate the small-batch rehearsal guard, including missing/ambiguous/conflicting span metrics, import-disabled semantics, no raw payload keys, and next-step recommendation for S04 review.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md`

## Verification

uv run python inline guard assertions and no-raw-payload scan

## Observability Impact

Records batch guard evidence for independent S04 semantic review.
