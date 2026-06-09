---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Generated the small-batch locator rehearsal over 10 M011 targets.

Generate a bounded small-batch candidate locator rehearsal over existing M011 targets using the S01 protocol and S02 fixture shape. Record per-paper source/hash/coordinate diagnostics, locator counts, and failure-mode categories without raw source text.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`
- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md`

## Verification

uv run python inline rehearsal generation and schema assertions

## Observability Impact

Creates batch-level locator diagnostics and redacted per-paper counts.
