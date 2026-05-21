---
id: S01
parent: M021-xcfj4p
milestone: M021-xcfj4p
provides:
  - Design and impact boundary for S02 deterministic locator implementation.
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md
  - .gsd/milestones/M021-xcfj4p/slices/S01/implementation-impact-map.md
key_decisions:
  - Do not modify SemanticChunk in S02 because upstream impact is MEDIUM.
  - Implement candidate locators as a new additive module and tests.
  - Keep all locators import-disabled and review-only.
patterns_established:
  - Protocol-to-code milestones should start with an additive edit boundary and documented symbol impact.
  - High-impact existing dataclasses should be referenced, not modified, unless necessary.
observability_surfaces:
  - Design defines diagnostic codes and guard outputs.
  - Impact map records blast radius before edits.
drill_down_paths:
  - .gsd/milestones/M021-xcfj4p/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S01/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T10:11:31.631Z
blocker_discovered: false
---

# S01: Implementation design and impact map

**Completed the deterministic locator implementation design and impact map.**

## What Happened

S01 mapped M020's protocol into a concrete implementation boundary. Existing primitives were inspected, and GitNexus impact analysis documented why S02 should avoid mutating `SemanticChunk`, `ImportCandidate`, or `ValidationBatchState`. The proposed implementation is additive and test-first: a new `candidate_locators.py` module plus tests covering source hash validation, coordinate validation, ambiguity diagnostics, forbidden payload detection, and no-import safety flags.

## Verification

Fresh guard passed: uv run python inline S01 design/impact assertions returned m021-s01-design-impact-guard-ok.

## Requirements Advanced

- R049 — S01 defined the implementation approach and impact boundary for R049.

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

No code behavior changed in S01; implementation begins in S02.

## Follow-ups

S02 should add `src/arxiv_archive/candidate_locators.py` and `tests/test_candidate_locators.py` only. If existing symbols need edits, run fresh GitNexus impact before editing.

## Files Created/Modified

- `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md` — Design for deterministic candidate locator module/API, diagnostics, safety flags, and tests.
- `.gsd/milestones/M021-xcfj4p/slices/S01/implementation-impact-map.md` — GitNexus impact map and additive edit boundary.
