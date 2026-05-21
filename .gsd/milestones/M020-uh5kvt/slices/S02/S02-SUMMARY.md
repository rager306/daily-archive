---
id: S02
parent: M020-uh5kvt
milestone: M020-uh5kvt
provides:
  - Validated fixture shape for S03 small-batch rehearsal.
  - One real paper example of candidate locators with exact redacted source coordinates.
requires:
  - slice: S01
    provides: Candidate locator protocol schema and guard.
affects:
  []
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md
  - .gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md
key_decisions:
  - Use 2001.00281v1 as the one-paper fixture target because its source exists, hash matches, and M011 marked it claim/method-heavy but import-not-ready.
  - Locator fixture stores coordinate packets and span hashes, not raw source/chunk/claim text.
  - Semantic spot check may record FAIL_EXPECTED for trusted import readiness while still passing fixture usefulness.
patterns_established:
  - One-paper locator fixtures can use source path/hash/coordinates only.
  - FAIL_EXPECTED for import readiness is acceptable when the slice goal is protocol proof, not fact promotion.
  - Future batch locators should preserve the same redaction and guard shape.
observability_surfaces:
  - one-paper-locator-guard.json records pass/fail checks and safety flags.
  - one-paper-semantic-spot-check.md records categorical review implications without raw text.
drill_down_paths:
  - .gsd/milestones/M020-uh5kvt/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M020-uh5kvt/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T09:22:03.283Z
blocker_discovered: false
---

# S02: One-paper locator fixture

**Produced and validated a one-paper candidate locator fixture with exact redacted coordinates.**

## What Happened

S02 selected M011 target 2001.00281v1 and generated a one-paper candidate locator fixture under the S01 protocol. The source file exists and its hash matches the M011 source record. The fixture contains four locators covering claim, method, retrieval-only, and repair-required contexts, each with exact coordinate spans, line offsets, and span hashes. The guard validates schema conformity, coordinate validity, redaction boundaries, import-disabled semantics, and no exact forbidden payload keys. The semantic spot check concludes the fixture is useful for S03 rehearsal but not sufficient for trusted KG import.

## Verification

Fresh verification command passed: uv run python inline S02 final verification returned m020-s02-final-verification-ok.

## Requirements Advanced

- R048 — S02 demonstrated R048's candidate locator protocol on one source-backed paper while keeping import/write blocked.

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

The fixture uses coordinate-bearing candidate locators and categorical diagnostics but does not prove semantic fact correctness or positive KG import readiness.

## Follow-ups

S03 should run a bounded small-batch rehearsal using the S01 protocol and S02 fixture shape, measuring missing/ambiguous/conflicting spans while preserving import-disabled semantics.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json` — One-paper candidate locator fixture under S01 protocol.
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md` — Human-readable report for selected one-paper fixture.
- `.gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json` — Guard validating fixture schema, coordinates, safety flags, and redaction boundaries.
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md` — Categorical semantic spot check without raw text or fact claims.
