---
id: S03
parent: M020-uh5kvt
milestone: M020-uh5kvt
provides:
  - Small-batch locator artifact for S04 review.
  - Failure-mode metrics: 35 locators, 27 ambiguous spans, 0 missing spans, 0 conflicting evidence, 0 import-eligible locators.
requires:
  - slice: S01
    provides: Candidate locator protocol schema and guard.
  - slice: S02
    provides: One-paper fixture shape and validation pattern.
affects:
  []
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md
  - .gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md
key_decisions:
  - High ambiguous-span count is useful evidence and requires review; it is not import readiness.
  - S03 can recommend S04 review but cannot recommend positive KG import.
  - Batch artifacts continue to use paths/hashes/coordinates/counts only.
patterns_established:
  - Batch locator rehearsals should report missing/ambiguous/conflicting spans as first-class metrics.
  - A passed guard can still require semantic review when ambiguity is high.
  - Positive import remains blocked even when locator coverage exists.
observability_surfaces:
  - small-batch-locator-guard.json records paper/locator/failure-mode counts and pass/fail checks.
  - small-batch-rehearsal-recommendation.md records S04 review questions.
drill_down_paths:
  - .gsd/milestones/M020-uh5kvt/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M020-uh5kvt/slices/S03/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T09:27:51.618Z
blocker_discovered: false
---

# S03: Small-batch locator rehearsal

**Ran a small-batch locator rehearsal over 10 M011 targets and exposed review-required ambiguity.**

## What Happened

S03 ran the S01 candidate locator protocol over all 10 M011 semantic review targets using the S02 redacted fixture shape. It generated 35 locator records with source ledgers and coordinate spans. The guard passed and confirmed no raw payload keys, no production import, no LadybugDB writes, no embeddings/vectors/secrets, zero import-eligible locators, and zero fact promotions. The key result is that locator shape scales, but ambiguity remains high and must be reviewed independently before any positive import-gate milestone.

## Verification

Fresh verification command passed: uv run python inline S03 final verification returned m020-s03-final-verification-ok.

## Requirements Advanced

- R048 — S03 exercised R048 across a bounded batch and reported locator coverage/failure modes while keeping import/write blocked.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

35 locators were generated with 27 ambiguous spans. This proves protocol scalability and exposes ambiguity, but it does not prove semantic support for facts.

## Follow-ups

S04 must independently review locator meaningfulness, especially the high ambiguous-span count, and recommend whether the next KG milestone should implement deterministic locator code, improve chunking, or create reviewer packets/UI.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json` — Small-batch locator rehearsal over 10 M011 targets.
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md` — Human-readable batch report with aggregate metrics.
- `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json` — Guard validating schema, spans, redaction, and safety semantics.
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md` — Recommendation to proceed to S04 independent semantic review before import gates.
