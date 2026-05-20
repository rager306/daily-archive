# 9router MiniMax usage/remains implementation

## Source

Repository:

```text
/root/vendor-source/9router
GitNexus repo: 9router
Commit checked: 9dde485
```

Primary files:

```text
/root/vendor-source/9router/open-sse/services/usage.js
/root/vendor-source/9router/tests/unit/minimax-usage.test.js
```

## Endpoint order

9router does **not** use the same endpoint matrix I used in M015.

From `open-sse/services/usage.js`, `MINIMAX_USAGE_URLS` is:

```js
const MINIMAX_USAGE_URLS = {
  minimax: [
    "https://www.minimax.io/v1/token_plan/remains",
    "https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
  ],
  "minimax-cn": [
    "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains",
    "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains",
  ],
};
```

Important correction to M015:

- M015 tested `www.minimax.io/v1/token_plan/remains`.
- M015 tested `www.minimax.io/v1/api/openplatform/coding_plan/remains`, which is **not** the 9router global fallback.
- 9router's global fallback is:

```text
https://api.minimax.io/v1/api/openplatform/coding_plan/remains
```

So the M015 API-remains limitation was still under-debugged for global MiniMax.

## Method and headers

9router calls each usage URL with:

```js
method: "GET"
headers: {
  Authorization: `Bearer ${apiKey}`,
  Accept: "application/json",
  "Content-Type": "application/json",
}
```

It does not use `X-Api-Key` for this MiniMax usage path.

## Fallback behavior

9router tries endpoints in order. It may continue to the next endpoint on:

- `404`
- `405`
- `>=500`
- thrown network/proxy error while another endpoint remains

It does **not** continue on auth-like errors. It returns an invalid/inactive key message when:

- HTTP status is `401` or `403`
- provider `base_resp.status_code` is `1004`
- response text/status message matches token/coding plan auth-like strings, invalid key, unauthorized, or inactive

## Provider success criteria

9router does not treat HTTP 200 as enough.

It parses response text as JSON, then checks:

```js
const baseResp = (payload?.base_resp ?? payload?.baseResp) || {};
const apiStatusCode = Number(baseResp.status_code ?? baseResp.statusCode) || 0;
```

If `apiStatusCode !== 0`, it returns an upstream quota API error message.

A useful MiniMax usage response must contain:

```text
base_resp.status_code = 0
model_remains or modelRemains array
at least one model row with quota totals
```

## Response fields parsed

9router supports both snake_case and camelCase:

- model name:
  - `model_name`
  - `modelName`
- 5h total:
  - `current_interval_total_count`
  - `currentIntervalTotalCount`
- 5h count:
  - `current_interval_usage_count`
  - `currentIntervalUsageCount`
- weekly total:
  - `current_weekly_total_count`
  - `currentWeeklyTotalCount`
- weekly count:
  - `current_weekly_usage_count`
  - `currentWeeklyUsageCount`
- reset/remains fields:
  - `remains_time` / `remainsTime`
  - `weekly_remains_time` / `weeklyRemainsTime`
  - `end_time` / `endTime`
  - `weekly_end_time` / `weeklyEndTime`

## Token Plan vs Coding Plan count semantics

This is another important 9router behavior.

9router determines:

```js
const countMeansRemaining = usageUrl.includes("/coding_plan/remains");
```

Then `buildMiniMaxQuota(total, count, resetAt, countMeansRemaining)` interprets the count differently:

- `token_plan/remains`: count means **used count**.
  - used = count
  - remaining = total - used
- `coding_plan/remains`: count means **remaining count**.
  - used = total - count
  - remaining = count

The unit tests verify this difference.

## Unit-test evidence

`tests/unit/minimax-usage.test.js` includes:

1. Token-plan TTS quota counts as used counts:

```text
current_interval_total_count=4000
current_interval_usage_count=25
=> used=25, remaining=3975
```

2. Coding-plan TTS quota counts as remaining counts:

```text
currentIntervalTotalCount=4000
currentIntervalUsageCount=4000
=> used=0, remaining=4000
```

3. Non-TTS rows are retained:

```text
music-2.6
image-01
```

This matters because M014/M015 focused mostly on text usage, but MiniMax Token Plan can expose multimodal rows.

## Corrected probe rule for M016

M016 S02 should run the exact 9router endpoint order:

For provider `minimax`:

1. `https://www.minimax.io/v1/token_plan/remains`
2. `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`

For provider `minimax-cn`:

1. `https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains`
2. `https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains`

Only use:

```text
GET
Authorization: Bearer <key>
Accept: application/json
Content-Type: application/json
```

Success requires:

```text
HTTP ok
base_resp.status_code == 0
model_remains/modelRemains array present
at least one row with current interval or weekly quota total
```

Artifacts must persist only sanitized metadata, not raw body, secrets, exact quota values, raw prompts, or raw model content.
