# MiniMax structured output remediation

## Verdict

`tool_call_recommended`

M015 corrected the M014 mistake: prompt-only OpenAI-compatible JSON was not the right basis for judging MiniMax structured output. The official docs recommend Anthropic-compatible API and document support for `tools` / `tool_choice`.

## Results

- Live calls: `5`
- HTTP successes: `5`
- JSON parse successes: `4`
- Tool call successes: `1`
- Schema-validated tool calls: `1`

## Interface outcomes

- Anthropic text JSON: `True`
- Anthropic forced tool call with `input_schema`: `True`
- OpenAI reasoning_split JSON: `True`
- OpenAI response_format json_object: `True`
- OpenAI response_format json_schema: `True`

## Corrected recommendation

Use Anthropic-compatible forced tool calls with input_schema for helper adapter decisions; use OpenAI response_format/json_schema only as secondary if tool path is unavailable.

## Required controls

- `anthropic_tool_choice_for_schema`
- `local_schema_validation`
- `bounded_retry_on_tool_or_parse_failure`
- `response_hash_only_artifacts`
- `redacted_metadata_only`
- `no_fact_promotion`

## Still blocked

- Production import
- LadybugDB writes
- Trusted fact creation
- Source-of-truth use
- Orchestration
- Raw paper/PDF/chunk text calls
- Raw response/model content persistence
