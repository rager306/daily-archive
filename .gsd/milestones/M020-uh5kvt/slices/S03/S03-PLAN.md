# S03: Small-batch locator rehearsal

**Goal:** Run the locator protocol over a small reviewed batch to measure missing, ambiguous, conflicting, and review-required locator outcomes.
**Demo:** After S03, a bounded small batch reports locator coverage and failure modes without enabling import.

## Must-Haves

- Bounded batch selected from existing reviewed artifacts.
- Coverage and failure-mode metrics are reported.
- Import-ready count remains zero or explicitly blocked.
- No LadybugDB writes or production import attempted.
- No raw source text persisted in JSON artifacts.

## Proof Level

- This slice proves: Batch summary, diagnostics, and guard assertions.

## Integration Closure

Small-batch results feed independent semantic review in S04.

## Verification

- Adds aggregate diagnostics and per-paper redacted counts for locator quality.

## Tasks

- [x] **T01: Generate small-batch locator rehearsal** `est:75m`
  Generate a bounded small-batch candidate locator rehearsal over existing M011 targets using the S01 protocol and S02 fixture shape. Record per-paper source/hash/coordinate diagnostics, locator counts, and failure-mode categories without raw source text.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json`, `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md`
  - Verify: uv run python inline rehearsal generation and schema assertions

- [x] **T02: Validate small-batch locator guard** `est:45m`
  Validate the small-batch rehearsal guard, including missing/ambiguous/conflicting span metrics, import-disabled semantics, no raw payload keys, and next-step recommendation for S04 review.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json`, `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md`
  - Verify: uv run python inline guard assertions and no-raw-payload scan

## Files Likely Touched

- .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json
- .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md
- .gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json
- .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md
