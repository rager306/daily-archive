---
id: S02
parent: M021-xcfj4p
milestone: M021-xcfj4p
provides:
  - Callable deterministic candidate locator API for S03.
  - Tested safety guard and validation behavior for R049.
requires:
  - slice: S01
    provides: Design and additive impact boundary.
affects:
  []
key_files:
  - src/arxiv_archive/candidate_locators.py
  - tests/test_candidate_locators.py
  - .gsd/milestones/M021-xcfj4p/slices/S02/run-evidence/candidate-locator-module-guard.json
key_decisions:
  - Keep candidate locator implementation additive in a new module.
  - Use recursive forbidden exact-key validation for raw payload boundary.
  - Treat broad repeated route signals as ambiguity diagnostics, not semantic support.
patterns_established:
  - New KG validation infrastructure should serialize artifacts only after recursive safety validation.
  - Ambiguous spans are first-class diagnostic outputs, not failures to hide.
  - Pure module first, CLI integration later.
observability_surfaces:
  - candidate-locator-module-guard.json records module-level pass/fail and safety summary.
  - Validation diagnostics expose unsafe flags, forbidden keys, invalid coordinates, source/hash issues, and ambiguity classes.
drill_down_paths:
  - .gsd/milestones/M021-xcfj4p/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M021-xcfj4p/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T10:19:41.480Z
blocker_discovered: false
---

# S02: Deterministic locator module

**Implemented and verified deterministic candidate locator generation code.**

## What Happened

S02 implemented the deterministic candidate locator module under the additive boundary from S01. Tests were written first and initially failed because the module did not exist. The new module builds protocol-conformant candidate locator artifacts, computes source hashes and coordinate spans without serializing raw text, classifies source/hash/signal/broad-match diagnostics, validates safety and schema invariants, and writes only validated JSON. Focused verification passed: 8 tests, ruff, LSP diagnostics, and module guard.

## Verification

Fresh verification passed: 8 tests, ruff clean, LSP no diagnostics on changed files, and m021-s02-module-guard-ok.

## Requirements Advanced

- R049 — S02 implemented and tested the core deterministic candidate locator generation required by R049.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The invalid-coordinate test was adjusted after the first green attempt because it used a route whose signal was absent, yielding an artifact-record span instead of coordinate span. The corrected test now targets the method route.

## Known Limitations

No CLI or persisted batch runner yet; S03 will add/rehearse bounded batch generation. The module still uses deterministic route signal patterns and does not perform semantic fact validation.

## Follow-ups

S03 should add a bounded batch rehearsal path using the implemented module over M011 targets and compare ambiguity diagnostics to M020.

## Files Created/Modified

- `src/arxiv_archive/candidate_locators.py` — Deterministic candidate locator module with source/hash checks, coordinate spans, ambiguity diagnostics, safety flags, validation, and writer.
- `tests/test_candidate_locators.py` — Focused tests for generation, hash mismatch, ambiguity, missing signals, forbidden payload keys, unsafe flags, coordinate validation, and writer safety.
- `.gsd/milestones/M021-xcfj4p/slices/S02/run-evidence/candidate-locator-module-guard.json` — S02 guard artifact for module behavior and safety invariants.
