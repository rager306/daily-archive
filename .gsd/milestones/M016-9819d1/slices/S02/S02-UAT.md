# S02: 9router compatible live limit probe — UAT

**Milestone:** M016-9819d1
**Written:** 2026-05-20T12:48:01.801Z

# S02: 9router compatible live limit probe — UAT

## Result

- Corrected endpoint order used: yes.
- Global fallback endpoint verified: `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`.
- Success classification: HTTP 200, `base_resp.status_code=0`, `model_remains` present, quota rows present.
- `true_success_count=1`.
- `quota_row_count_total=8`.
- Raw responses persisted: false.
- Exact quota values persisted: false.
- Credential values logged: false.

## User-visible answer

The programmatic MiniMax global limit check works through the 9router fallback endpoint. M015's `ui_only_or_session_required` verdict is no longer correct for global MiniMax.
