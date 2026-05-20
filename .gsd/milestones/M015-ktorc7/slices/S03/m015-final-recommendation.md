# M015 final corrected MiniMax verdict

## Why this milestone exists

The M014 conclusion was too pessimistic on structured JSON and too accepting on Token Plan limit checking. M015 re-debugged both issues with a matrix instead of a single call.

## Final verdict

- Review verdict: `PASS`
- M014 under-debugged: `True`
- Structured output verdict: `tool_call_recommended`
- Token Plan limit-check verdict: `ui_only_or_session_required`

## Corrected Token Plan limit-check conclusion

The current reliable method to check limits is:

`Billing > Token Plan UI at https://platform.minimax.io/user-center/payment/token-plan`

Programmatic API remains status:

`not_verified_with_api_key; requires distinct Token Plan Key or browser/session-supported endpoint if MiniMax exposes one`

Evidence:

- True remains success count: `0`
- Token Plan key distinct from API key: `False`
- API remains verified: `False`

Meaning: M014's single 403 was under-debugged, but after matrix testing the API remains path is still not proven because no distinct authorized Token Plan Key/session-backed endpoint was available. This is a precise remaining limitation, not acceptance of failure.

## Corrected structured-output conclusion

MiniMax is **usable for structured helper output** through the right interface.

Recommended interface:

`anthropic_forced_tool_call`

Evidence:

- Anthropic forced tool schema validated: `True`
- OpenAI response_format json_schema passed: `True`
- OpenAI response_format json_object passed: `True`
- JSON parse success count: `4`
- Tool call success count: `1`
- Schema validated count: `1`

Correct recommendation:

Use Anthropic-compatible forced tool calls with input_schema for helper adapter decisions; use OpenAI response_format/json_schema only as secondary if tool path is unavailable.

## Required controls

- `anthropic_tool_choice_for_schema`
- `local_schema_validation`
- `bounded_retry_on_tool_or_parse_failure`
- `response_hash_only_artifacts`
- `redacted_metadata_only`
- `no_fact_promotion`

## Still blocked

- Production KG import
- LadybugDB writes
- Trusted fact creation
- MiniMax source-of-truth role
- MiniMax orchestration
- Unattended batch use
- Raw paper/PDF/chunk text calls
- Raw prompt/response/model-content persistence

## Next safe step

`dev_only_minimax_anthropic_tool_helper_adapter_over_redacted_metadata_with_local_schema_validation`
