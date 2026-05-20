---
id: T01
parent: S02
milestone: M016-9819d1
key_files:
  - .gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json
key_decisions:
  - Use `https://api.minimax.io/v1/api/openplatform/coding_plan/remains` as the working global MiniMax limits endpoint when `www.minimax.io/v1/token_plan/remains` returns 403.
  - Treat coding_plan count fields as remaining counts, per 9router semantics.
duration: 
verification_result: passed
completed_at: 2026-05-20T12:40:44.012Z
blocker_discovered: false
---

# T01: Ran 9router-compatible MiniMax limit probe; global API remains is verified via the `api.minimax.io` coding_plan fallback.

**Ran 9router-compatible MiniMax limit probe; global API remains is verified via the `api.minimax.io` coding_plan fallback.**

## What Happened

Ran the corrected 9router-compatible live probe using GET + Authorization Bearer and the 9router endpoint order. The first global endpoint `www.minimax.io/v1/token_plan/remains` returned 403, but the 9router global fallback `api.minimax.io/v1/api/openplatform/coding_plan/remains` returned HTTP 200, `base_resp.status_code=0`, a `model_remains` array with 11 rows, and 8 quota rows. Raw responses, exact quota values, and credential values were not persisted.

## Verification

9router-compatible-limit-probe-ok passed with true_success_count=1, quota_row_count_total=8, raw_response_persisted=false, credential_values_logged=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `GET 9router endpoint order with Bearer key, sanitized metadata only` | 0 | ✅ pass — true_success_count=1; quota_row_count_total=8; raw_response_persisted=false | 7000ms |
| 2 | `JSON invariant check for 9router-compatible-limit-probe.json` | 0 | ✅ pass — 9router-compatible-limit-probe-ok | 7000ms |

## Deviations

The corrected 9router-compatible probe overturned the previous M015 limit verdict: API remains is verified for global MiniMax via the 9router fallback endpoint.

## Known Issues

CN minimaxi endpoints still returned auth-like provider errors with the available key, which is expected for a global key. Exact quota values were not persisted, only row counts/status metadata.

## Files Created/Modified

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json`
