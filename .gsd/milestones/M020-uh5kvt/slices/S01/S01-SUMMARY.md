---
id: S01
parent: M020-uh5kvt
milestone: M020-uh5kvt
provides:
  - Candidate locator protocol contract for S02.
  - Machine-readable schema and safety guard for future locator artifacts.
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md
  - .gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json
  - .gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json
  - .gsd/milestones/M020-uh5kvt/slices/S01/protocol-validation-report.md
key_decisions:
  - Candidate locators are review evidence, not KG facts.
  - M020 locators must remain import-disabled and cannot be written to LadybugDB.
  - Counts alone cannot establish semantic KG readiness.
patterns_established:
  - Protocol-first KG locator work before fixture generation.
  - Candidate locators are evidence pointers, not facts.
  - Guard artifacts must prove import/write/raw-payload safety before downstream use.
observability_surfaces:
  - candidate-locator-protocol-schema.json captures allowed fields and values for future assertions.
  - candidate-locator-protocol-guard.json records pass/fail checks and safety flags.
drill_down_paths:
  - .gsd/milestones/M020-uh5kvt/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M020-uh5kvt/slices/S01/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T09:16:26.565Z
blocker_discovered: false
---

# S01: Candidate locator protocol contract

**Defined and validated the candidate locator protocol contract for M020.**

## What Happened

S01 defined the candidate locator and chunk-span provenance protocol that M011 required before positive KG import could be considered. The protocol combines existing daily-archive evidence-path/chunk conventions with M019's protocol-bound source-ledger recommendation. It requires source ledgers, source spans, uncertainty labels, review queue reasons, redacted diagnostics, and safety flags. The guard confirms all import/write/raw-payload/fact-promotion paths remain blocked.

## Verification

Fresh verification command passed: uv run python inline S01 final verification returned m020-s01-final-verification-ok.

## Requirements Advanced

- R048 — S01 defined the protocol contract needed to validate candidate locators with chunk-span provenance before any positive KG import.

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

The protocol has not yet been exercised on a paper fixture; that is S02 scope.

## Follow-ups

S02 should create a one-paper locator fixture using this protocol, with import_eligible=false, promoted_to_fact=false, no raw text in machine artifacts, and no LadybugDB writes.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md` — Defines candidate locator protocol, source ledger, source span coordinates, uncertainty/review labels, and safety semantics.
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json` — Machine-readable protocol schema and allowed values for future locator artifacts.
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json` — Guard proving required fields and safety invariants.
- `.gsd/milestones/M020-uh5kvt/slices/S01/protocol-validation-report.md` — Human-readable validation report for S01.
