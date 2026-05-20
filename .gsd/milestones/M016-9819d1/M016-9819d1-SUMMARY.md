---
id: M016-9819d1
title: "MiniMax Limits via 9router Implementation"
status: complete
completed_at: 2026-05-20T12:49:15.746Z
key_decisions:
  - Global MiniMax API remains is verified via the 9router `api.minimax.io` coding_plan fallback endpoint.
  - M015 `ui_only_or_session_required` verdict is overturned for global MiniMax.
  - CN MiniMax remains unverified with current global key.
key_files:
  - .gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md
  - .gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json
  - .gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json
  - .gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json
  - .gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md
  - .gsd/milestones/M016-9819d1/M016-9819d1-VALIDATION.md
lessons_learned:
  - Do not declare MiniMax limit access unresolved until checking the 9router global fallback endpoint.
  - MiniMax usage endpoint families differ in count semantics: token_plan count means used, coding_plan count means remaining.
---

# M016-9819d1: MiniMax Limits via 9router Implementation

**M016 verified MiniMax global API remains via 9router’s `api.minimax.io` coding_plan fallback and overturned M015’s limit verdict.**

## What Happened

M016 remediated the MiniMax limits research by using 9router as the implementation reference. 9router was cloned into `/root/vendor-source/9router`, indexed as GitNexus repo `9router`, and inspected for MiniMax usage/remains handling. Its source showed the missed global fallback endpoint `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`, GET + Authorization Bearer headers, strict `base_resp.status_code==0` success criteria, and `model_remains` quota parsing. The corrected live probe verified API remains for global MiniMax through that fallback with `true_success_count=1`, `model_remains_count=11`, and `quota_row_count_total=8`, while preserving raw-response and secret hygiene.

## Success Criteria Results

All milestone success criteria passed. Fresh verification output: `m016-final-verification-ok` and JSON evidence with `limit_check_verdict=api_remains_verified`, `true_success_count=1`, `quota_row_count_total=8`, `raw_response_persisted=false`, and `credential_values_logged=false`.

## Definition of Done Results

- [x] 9router cloned/indexed and used as source reference.
- [x] Endpoint order and parser semantics documented.
- [x] Corrected live probe used 9router algorithm.
- [x] Final verdict recorded: `api_remains_verified`.
- [x] R044 validated.
- [x] Raw-response/secret/quota hygiene maintained.

## Requirement Outcomes

R044 validated by final guard: `limit_check_verdict=api_remains_verified`, `used_9router_algorithm=true`, `m015_limit_verdict_overturned=true`, and hygiene flags false.

## Deviations

M016 overturned the M015 Token Plan verdict for global MiniMax because 9router revealed a missed working fallback endpoint.

## Follow-ups

Use `https://api.minimax.io/v1/api/openplatform/coding_plan/remains` for global MiniMax limit checks after `www.minimax.io/v1/token_plan/remains` returns 403/auth-like behavior. If CN MiniMax is needed, retest with a CN key. Keep exact quota values and raw bodies out of artifacts by default.
