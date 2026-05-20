---
id: T02
parent: S02
milestone: M016-9819d1
key_files:
  - .gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json
  - .gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md
  - .gsd/REQUIREMENTS.md
key_decisions:
  - Use 9router global fallback endpoint for MiniMax limit checks.
  - Do not persist exact quota values by default; treat them as account-sensitive operational data.
duration: 
verification_result: passed
completed_at: 2026-05-20T12:42:24.057Z
blocker_discovered: false
---

# T02: Final M016 guard verifies global MiniMax API remains through the 9router fallback endpoint and overturns M015's limit verdict.

**Final M016 guard verifies global MiniMax API remains through the 9router fallback endpoint and overturns M015's limit verdict.**

## What Happened

Wrote the final M016 guard and recommendation, then updated R044 to validated. The final verdict is `api_remains_verified`: global MiniMax usage/remains can be checked with the 9router fallback endpoint `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`, GET + Authorization Bearer. Success was classified only after HTTP 200, `base_resp.status_code=0`, `model_remains` presence, and quota rows. Raw response, exact quota values, and credential values were not persisted.

## Verification

final-m016-guard-ok passed and R044 updated to validated.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `write final-m016-guard.json and m016-final-recommendation.md` | 0 | ✅ pass — final-m016-guard-ok | 6600ms |
| 2 | `gsd_requirement_update R044` | 0 | ✅ pass — R044 validated | 0ms |

## Deviations

The final verdict overturns M015 for global MiniMax limits: API remains is verified via the 9router fallback endpoint.

## Known Issues

CN endpoints still returned auth-like errors with the global key and are not verified. This does not affect global MiniMax limit checking.

## Files Created/Modified

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json`
- `.gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md`
- `.gsd/REQUIREMENTS.md`
