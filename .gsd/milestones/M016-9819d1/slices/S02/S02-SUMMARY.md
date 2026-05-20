---
id: S02
parent: M016-9819d1
milestone: M016-9819d1
provides:
  - verified global MiniMax limit endpoint
  - final corrected verdict
requires:
  - slice: S01
    provides: 9router endpoint order and parser semantics.
affects:
  []
key_files:
  - .gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json
key_decisions:
  - Global MiniMax API remains is verified via 9router fallback endpoint.
  - M015 limit verdict is overturned for global MiniMax.
  - CN MiniMax remains unverified with current key.
patterns_established:
  - Use vendor-proven endpoint order before declaring API access blocked.
  - Classify MiniMax usage success on provider status and model_remains quota rows, not HTTP alone.
observability_surfaces:
  - 9router-compatible-limit-probe.json
  - final-m016-guard.json
drill_down_paths:
  - .gsd/milestones/M016-9819d1/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M016-9819d1/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T12:48:01.801Z
blocker_discovered: false
---

# S02: 9router compatible live limit probe

**S02 verifies global MiniMax API remains using 9router's missed fallback endpoint.**

## What Happened

S02 ran the corrected 9router-compatible live probe and found the missing global fallback works. The first endpoint, `www.minimax.io/v1/token_plan/remains`, returned 403. The second endpoint, `api.minimax.io/v1/api/openplatform/coding_plan/remains`, returned HTTP 200, `base_resp.status_code=0`, 11 `model_remains` rows, and 8 quota rows. The final guard validates API remains for global MiniMax while preserving raw-response and secret hygiene.

## Verification

9router-compatible-limit-probe-ok and final-m016-guard-ok passed; R044 updated to validated.

## Requirements Advanced

None.

## Requirements Validated

- R044 — M016 final guard shows `limit_check_verdict=api_remains_verified`, `used_9router_algorithm=true`, `m015_limit_verdict_overturned=true`, and no raw responses/secrets persisted.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

M016 S02 overturns M015's global MiniMax API remains limitation because M015 missed the 9router global fallback endpoint.

## Known Limitations

Exact quota values were intentionally not persisted. CN endpoints were not verified with the global key.

## Follow-ups

Use the verified global fallback endpoint for future MiniMax limit checks. If CN MiniMax limits are needed, test with a CN key. If exact quota values must be displayed, handle them as account-sensitive operational data.

## Files Created/Modified

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json` — Corrected live probe following 9router endpoint order.
- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json` — Final M016 guard.
- `.gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md` — Final recommendation.
- `.gsd/REQUIREMENTS.md` — R044 validation.
