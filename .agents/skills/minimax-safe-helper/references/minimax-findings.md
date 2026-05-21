<overview>
This reference records the durable MiniMax findings from M012-M016. It is the source to load before implementing or reviewing MiniMax helper behavior in daily-archive.
</overview>

<status>
MiniMax is allowed only as a bounded helper.

Allowed:
- synthetic or redacted text calls;
- forced-tool structured helper output with local schema validation;
- sanitized global usage/remains checks;
- dev-only helper wrappers with tests over sanitized fixtures.

Blocked:
- MiniMax as orchestrator;
- MiniMax as source of truth;
- trusted KG fact creation from MiniMax output;
- production LadybugDB writes;
- raw paper/chunk/PDF text sent to MiniMax;
- raw provider bodies or exact quota values in artifacts;
- unattended Scientific KG scaling based on MiniMax output.
</status>

<text_generation_api>
Use the Anthropic-compatible text API for generation and helper calls.

```text
Base URL: https://api.minimax.io/anthropic
Auth: X-Api-Key
Model proven in project probes: MiniMax-M2.7-highspeed
```

Notes:
- Temperature cannot be exactly `0`; use a small positive value when low randomness is needed.
- The Anthropic-compatible text API uses `X-Api-Key`, not Bearer auth.
- Keep payloads synthetic or redacted unless a later milestone explicitly approves broader corpus use.
</text_generation_api>

<structured_output>
Recommended path:

```text
Anthropic-compatible forced tool call + input_schema + local schema validation
```

Rationale:
- M015 showed prompt-only JSON is not a valid suitability test.
- Forced tool call with schema validation succeeded and is the preferred MiniMax structured helper path.
- OpenAI-compatible `response_format=json_object` and `response_format=json_schema` parsed in probes, but are secondary options.

Rule: MiniMax structured output is helper evidence only. It is not a trusted fact until deterministic/project review gates validate it.
</structured_output>

<usage_limits>
M016 corrected the previous limit-check conclusion by using decolua/9router as implementation reference.

Global MiniMax endpoint order:

```text
1. GET https://www.minimax.io/v1/token_plan/remains
2. GET https://api.minimax.io/v1/api/openplatform/coding_plan/remains
```

Request headers for usage/remains:

```text
Authorization: Bearer <MiniMax key>
Accept: application/json
Content-Type: application/json
```

Important: this usage/remains path uses Bearer auth. Do not reuse the `X-Api-Key` text-generation auth shape here.

Success criteria:

```text
HTTP status is 2xx
base_resp.status_code == 0
model_remains or modelRemains is an array
at least one row contains interval or weekly quota totals
```

Do not treat HTTP 200 as success when `base_resp.status_code` is non-zero.

Verified M016 result:

```text
limit_check_verdict=api_remains_verified
working_endpoint=https://api.minimax.io/v1/api/openplatform/coding_plan/remains
true_success_count=1
model_remains_count=11
quota_row_count_total=8
raw_response_persisted=false
exact_quota_values_persisted=false
credential_values_logged=false
```

CN endpoints remain unverified with the current global key. Test CN separately with a CN key if needed.
</usage_limits>

<usage_fields>
Support both snake_case and camelCase response fields.

| Meaning | Snake case | Camel case |
|---|---|---|
| model list | `model_remains` | `modelRemains` |
| model name | `model_name` | `modelName` |
| 5h total | `current_interval_total_count` | `currentIntervalTotalCount` |
| 5h count | `current_interval_usage_count` | `currentIntervalUsageCount` |
| weekly total | `current_weekly_total_count` | `currentWeeklyTotalCount` |
| weekly count | `current_weekly_usage_count` | `currentWeeklyUsageCount` |
| 5h remaining time | `remains_time` | `remainsTime` |
| weekly remaining time | `weekly_remains_time` | `weeklyRemainsTime` |
| 5h end time | `end_time` | `endTime` |
| weekly end time | `weekly_end_time` | `weeklyEndTime` |
</usage_fields>

<count_semantics>
The two endpoint families interpret count fields differently.

| Endpoint family | Count means | Used calculation | Remaining calculation |
|---|---|---|---|
| `token_plan/remains` | used count | `used = count` | `remaining = total - count` |
| `coding_plan/remains` | remaining count | `used = total - count` | `remaining = count` |

This came from 9router source and tests. Do not normalize both endpoint families the same way.
</count_semantics>

<safe_artifacts>
Allowed persisted evidence:
- endpoint family;
- HTTP status class;
- provider status code;
- presence/absence of `model_remains`;
- quota row counts;
- schema validation verdict;
- response hash for correlation;
- boolean safety flags.

Forbidden by default:
- raw provider response bodies;
- exact quota values;
- secrets or token values;
- raw paper text;
- raw chunk text;
- raw PDFs/base64 payloads;
- embeddings or vectors;
- raw model output containing sensitive content;
- trusted KG facts inferred only by MiniMax.
</safe_artifacts>

<evidence_trail>
Milestone meanings:

- M012: MiniMax researched and classified as conditional optional helper only.
- M013: bounded synthetic callability smoke test passed.
- M014: real bounded calls ran, but early limit and JSON conclusions were incomplete.
- M015: structured output was corrected; forced tool calls with schema validation became the recommended path. Limit access was still under-debugged.
- M016: 9router corrected the usage/remains endpoint sequence; global API remains was verified through the `api.minimax.io` coding-plan fallback.

Use M016 as source of truth for global MiniMax usage/remains behavior.
</evidence_trail>
