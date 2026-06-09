---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Validated deterministic batch guard and recommendation.

Validate the S03 batch guard and compare key metrics against M020, especially ambiguity, missing signals, and import-disabled safety flags.

## Inputs

- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json`

## Expected Output

- `.gsd/milestones/M021-xcfj4p/slices/S03/run-evidence/deterministic-locator-batch-guard.json`
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-batch-recommendation.md`

## Verification

uv run python inline S03 guard assertions

## Observability Impact

Records final S03 pass/fail and review recommendation.
