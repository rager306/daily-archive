# M016 final MiniMax limits verdict via 9router

## Verdict

`api_remains_verified`

M016 overturns the M015 limit-check verdict for global MiniMax. The API remains path is verified when using 9router's actual global fallback endpoint.

## Working method

- Provider: `minimax`
- Endpoint: `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`
- Method: `GET`
- Header: `Authorization: Bearer <key>`
- Count semantics: `coding_plan/remains` counts mean remaining counts.

## Evidence

- true_success_count: `1`
- model_remains_count: `11`
- quota_row_count_total: `8`
- success required `base_resp.status_code == 0`: `True`
- raw response persisted: `False`
- exact quota values persisted: `False`
- credential values logged: `False`

## Correction to M015

M015 missed 9router's global fallback endpoint:

`https://api.minimax.io/v1/api/openplatform/coding_plan/remains`

The endpoint that returned 403 was only the first 9router global endpoint:

`https://www.minimax.io/v1/token_plan/remains`

The second endpoint works with the available key.

## Still true

- CN endpoints are not verified with the current global key.
- Quota values should be handled as account-sensitive operational data.
- Do not persist raw response bodies or exact quota values unless explicitly needed and sanitized.
- This limit-check work does not authorize KG import, LadybugDB writes, trusted fact creation, or MiniMax source-of-truth behavior.

## Practical answer

For global MiniMax usage/remains checks, use the 9router sequence:

1. `GET https://www.minimax.io/v1/token_plan/remains`
2. if that fails with 403/auth-like behavior, use `GET https://api.minimax.io/v1/api/openplatform/coding_plan/remains`

with:

```text
Authorization: Bearer <MiniMax key>
Accept: application/json
Content-Type: application/json
```

Then require:

```text
base_resp.status_code == 0
model_remains/modelRemains array exists
quota rows have current interval or weekly totals
```
