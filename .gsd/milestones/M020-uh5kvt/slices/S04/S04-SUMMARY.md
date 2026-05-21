---
id: S04
parent: M020-uh5kvt
milestone: M020-uh5kvt
provides:
  - Final M020 recommendation: deterministic locator implementation plus ambiguity diagnostics next.
  - Validated R048 evidence with positive import deferred.
requires:
  - slice: S01
    provides: Protocol/schema/guard.
  - slice: S02
    provides: One-paper fixture.
  - slice: S03
    provides: Small-batch rehearsal metrics.
affects:
  []
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md
  - .gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json
  - .gsd/milestones/M020-uh5kvt/slices/S04/final-locator-recommendation.md
key_decisions:
  - Independent review verdict is FLAG for positive import readiness, not a failure of M020.
  - Next work should implement deterministic locators and ambiguity diagnostics, not positive import-gate work.
  - R048 validated for protocol/rehearsal evidence while import remains blocked.
patterns_established:
  - Independent review may return FLAG while the milestone succeeds if the flag blocks unsafe promotion.
  - Final guards should encode downstream decision, not just pass/fail state.
  - Validated protocol evidence can still defer implementation/import work.
observability_surfaces:
  - final-locator-protocol-guard.json records the final milestone decision and safety gates.
  - independent-semantic-review.md records external review findings and risks.
drill_down_paths:
  - .gsd/milestones/M020-uh5kvt/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M020-uh5kvt/slices/S04/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T09:35:26.804Z
blocker_discovered: false
---

# S04: Locator semantic review and recommendation

**Completed independent review and final recommendation for candidate locator protocol work.**

## What Happened

S04 performed independent semantic review and final closeout for M020. The review confirmed the protocol is sufficient for review-only locator evidence and that redaction/safety boundaries hold, but flagged high ambiguity in the small-batch rehearsal as blocking positive import work. The final guard passed and recorded 10 papers, 35 locators, 27 ambiguous spans, zero import-eligible locators, zero fact promotions, and all no-import/no-write/no-raw-payload gates intact. R048 was updated to validated with the explicit caveat that positive import remains deferred.

## Verification

Fresh verification command passed: uv run python inline M020 final verification returned m020-final-verification-ok.

## Requirements Advanced

None.

## Requirements Validated

- R048 — Validated by M020 protocol/schema/guards, one-paper fixture, 10-paper rehearsal, independent semantic review, and final guard m020-final-verification-ok.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Final verification initially failed on a brittle case-sensitive text assertion, then passed after using a case-insensitive assertion. No artifact or guard semantics changed.

## Known Limitations

M020 did not implement production locator code; it produced protocol and rehearsal evidence. Positive KG import remains blocked.

## Follow-ups

Plan a next milestone for deterministic candidate locator implementation plus ambiguity diagnostics. Do not plan positive KG import until ambiguity is reduced and semantic review passes.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md` — Independent semantic review of S01-S03 locator artifacts.
- `.gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json` — Final guard for M020 locator protocol milestone.
- `.gsd/milestones/M020-uh5kvt/slices/S04/final-locator-recommendation.md` — Final recommendation deferring positive import and recommending deterministic locator implementation.
