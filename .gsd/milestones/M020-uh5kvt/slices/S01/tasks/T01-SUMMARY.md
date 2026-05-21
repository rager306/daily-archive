---
id: T01
parent: S01
milestone: M020-uh5kvt
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md
  - .gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T09:15:46.541Z
blocker_discovered: false
---

# T01: Defined the M020 candidate locator and chunk-span provenance protocol contract.

**Defined the M020 candidate locator and chunk-span provenance protocol contract.**

## What Happened

Drafted the candidate locator and chunk-span provenance protocol as a review-only evidence pointer contract. The protocol defines source ledger fields, source span coordinate fields, allowed candidate types/routes/states, support levels, uncertainty labels, review queue reasons, safety flags, and M020 import-disabled semantics. It explicitly states candidate locators are not KG facts and cannot be imported or written to LadybugDB during M020.

## Verification

Verified with uv run python inline assertions over candidate-locator-protocol-schema.json and candidate-locator-protocol.md. Fresh S01 verification returned m020-s01-final-verification-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline schema/protocol assertions` | 0 | ✅ pass: m020-s01-protocol-guard-ok | 5000ms |
| 2 | `uv run python inline S01 final verification` | 0 | ✅ pass: m020-s01-final-verification-ok | 4700ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`
