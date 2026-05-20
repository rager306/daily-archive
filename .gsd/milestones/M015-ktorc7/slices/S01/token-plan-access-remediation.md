# Token Plan access remediation

## Verdict

`ui_only_or_session_required`

M015 corrected the M014 mistake: HTTP 200 with only `base_resp` and non-zero MiniMax status code is **not** a successful remains query.

## What was tested

- Key envs tested: `MINIMAX_API_KEY, MINIMAX_TOKEN_PLAN_KEY`
- Distinct key values tested: `1`
- Matrix rows: `32`
- Endpoints tested: `minimax.io`, `minimaxi.com`, `token_plan/remains`, legacy `coding_plan/remains`
- Methods tested: `GET`, `POST`
- Headers tested: `Authorization: Bearer`, `X-Api-Key`

## Result

- True remains success count: `0`
- Observed MiniMax base response status codes: `[1004, 2049]`
- Raw responses persisted: `false`
- Credential values logged: `false`

`MINIMAX_TOKEN_PLAN_KEY` was collected securely, but it has the same key hash as `MINIMAX_API_KEY`, so this session did not actually test a distinct Token Plan Key.

## Correct current answer

The reliable way to check limits right now is:

`Billing > Token Plan` → `https://platform.minimax.io/user-center/payment/token-plan`

The documented API endpoint exists, but programmatic remains checking is **not verified** with the available key material. Evidence suggests an authorized Token Plan key distinct from the ordinary API key or a browser/session-backed endpoint may be required.

## Next if exact live remains are required

Collect a distinct Token Plan Key from Billing > Token Plan and rerun this matrix. If the key still returns `1004`/`2049`/`403`, treat programmatic remains as UI/session-only and file/use vendor support guidance.
