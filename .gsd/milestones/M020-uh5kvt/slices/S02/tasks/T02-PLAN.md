---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Validated the one-paper locator fixture and semantic spot-check boundary.

Validate the one-paper fixture against the protocol guard and perform a targeted semantic spot check that records only categorical judgments and coordinates, not raw text.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md`

## Verification

uv run python inline guard assertions and no-raw-payload scan

## Observability Impact

Records whether the fixture is meaningful enough to feed small-batch rehearsal.
