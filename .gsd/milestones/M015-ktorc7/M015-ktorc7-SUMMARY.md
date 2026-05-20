---
id: M015-ktorc7
title: "MiniMax Limits and Structured Output Remediation"
status: complete
completed_at: 2026-05-20T12:25:38.942Z
key_decisions:
  - MiniMax structured helper output should use Anthropic-compatible forced tool calls with input_schema.
  - OpenAI response_format json_schema/json_object passed in this probe but is secondary to tool calls for helper decisions.
  - Programmatic Token Plan remains is not verified with available key material; Billing > Token Plan UI is the reliable current method.
  - Do not count HTTP 200 as MiniMax success if base_resp.status_code is non-zero or no useful fields are present.
key_files:
  - .gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json
  - .gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json
  - .gsd/milestones/M015-ktorc7/slices/S03/run-evidence/final-m015-guard.json
  - .gsd/milestones/M015-ktorc7/slices/S03/m015-final-recommendation.md
  - .gsd/milestones/M015-ktorc7/M015-ktorc7-VALIDATION.md
lessons_learned:
  - MiniMax endpoint success must be judged by provider status fields and useful payload fields, not HTTP status alone.
  - MiniMax prompt-only JSON is the wrong basis for structured-output verdict; tool calls are the correct helper interface.
  - Securely collecting a key does not guarantee it is semantically a different key; compare non-secret hashes to verify distinct key material.
---

# M015-ktorc7: MiniMax Limits and Structured Output Remediation

**M015 corrected M014: MiniMax structured output is viable via Anthropic tool calls; Token Plan API remains still needs distinct key/session evidence.**

## What Happened

M015 remediated the user's valid criticism of M014. It re-debugged Token Plan limits through a 32-row access matrix and found no true API remains success with available key material; the collected Token Plan key matched the ordinary API key, and HTTP 200 base_resp-only responses had non-zero provider status codes. It re-debugged MiniMax structured output on the correct surfaces: Anthropic-compatible API, Anthropic forced tool calls, OpenAI reasoning_split, and OpenAI response_format variants. The corrected result is that MiniMax is viable for structured helper output via Anthropic forced tool calls with input_schema, local validation, and bounded retry. Independent review passed after artifact placement was fixed, R043 was validated, and production/import/write/source-of-truth/orchestration remain blocked.

## Success Criteria Results

- Token Plan matrix: pass, with API remains limitation.
- Structured-output matrix: pass, tool_call_recommended.
- Independent review: PASS.
- Final artifact gate: pass.
- Safety blocks: preserved.

## Definition of Done Results

- Token Plan limit-check path debugged with matrix: met, with precise limitation.
- Structured-output path debugged on recommended surfaces: met, Anthropic forced tool call schema-validated.
- Final verdict corrected M014: met.
- Evidence hygiene: met.
- Independent review: PASS.
- R043: validated.
- Production/import/source-of-truth blocks: preserved.

## Requirement Outcomes

- R043 validated.
- R042 interpretation narrowed by M015: real calls were valid, but structured output should use tool calls and API remains was not proven.
- No requirement authorizes production import, writes, source-of-truth use, or orchestration.

## Deviations

The secure-collected Token Plan key was not distinct from the existing API key, so S01 could not prove a distinct Token Plan Key. This is recorded as an unresolved capability gap. S02 overturned the too-negative M014 structured-output conclusion.

## Follow-ups

Next safe work: implement a dev-only MiniMax Anthropic forced-tool helper adapter over redacted metadata, with local schema validation, bounded retry, response-hash-only artifacts, and no fact promotion. If exact Token Plan remains are required, collect a truly distinct Token Plan Key or use a supported session/API method and rerun S01 matrix.
