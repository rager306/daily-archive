---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Generate one-paper locator fixture

Select one M011/M010 source-backed target and generate a candidate locator fixture under the S01 protocol. The fixture should use source path/hash and exact redacted coordinates only, with locator states that remain review-only and import-disabled.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`
- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md`

## Verification

uv run python inline fixture schema and safety assertions

## Observability Impact

Creates first locator artifact and report with source/hash/coordinate diagnostics.
