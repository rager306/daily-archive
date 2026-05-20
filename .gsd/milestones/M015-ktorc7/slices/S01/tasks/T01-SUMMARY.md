---
id: T01
parent: S01
milestone: M015-ktorc7
key_files:
  - .gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json
key_decisions:
  - Do not count HTTP 200 as limit-check success unless `base_resp.status_code=0` and actual remains/usage fields are present.
  - Treat the collected MINIMAX_TOKEN_PLAN_KEY as not distinct from MINIMAX_API_KEY because their hashes match.
duration: 
verification_result: passed
completed_at: 2026-05-20T12:16:34.292Z
blocker_discovered: false
---

# T01: Ran Token Plan remains matrix; no true API remains success, and collected Token Plan key matched the ordinary API key.

**Ran Token Plan remains matrix; no true API remains success, and collected Token Plan key matched the ordinary API key.**

## What Happened

Ran a 32-row Token Plan access matrix across two available key variables, minimax/minimaxi hosts, current and legacy remains paths, GET/POST, and Bearer/X-Api-Key headers. Raw responses and credential values were not persisted. The matrix showed no true remains success: HTTP 200 JSON responses were base_resp-only with non-zero MiniMax status codes. The collected Token Plan key value matched the ordinary API key hash, so a distinct Token Plan Key was not tested.

## Verification

token-plan-access-matrix-ok passed with matrix_count=32, raw_response_persisted=false, credential_values_logged=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `Token Plan access endpoint/header/method/key matrix` | 0 | ✅ pass — matrix_count=32; raw_response_persisted=false; credential_values_logged=false | 124600ms |

## Deviations

The matrix initially counted HTTP 200 JSON responses, but deeper inspection showed they only contained `base_resp` with non-zero status codes. The guard therefore correctly treats true remains success as zero.

## Known Issues

A distinct Token Plan Key was not actually tested; current `MINIMAX_TOKEN_PLAN_KEY` has the same hash as `MINIMAX_API_KEY`. Programmatic remains access is still unverified.

## Files Created/Modified

- `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json`
