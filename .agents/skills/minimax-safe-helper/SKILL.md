---
name: minimax-safe-helper
description: Applies daily-archive MiniMax integration findings for safe helper implementation, structured output, and usage-limit checks. Use when working with MiniMax, MiniMax limits, Token Plan, coding_plan/remains, structured MiniMax JSON, or Scientific KG MiniMax helper behavior.
---

<objective>
Use MiniMax in daily-archive only through proven, bounded, non-authoritative helper patterns. This skill preserves the M012-M016 findings: MiniMax can be called for synthetic/redacted helper work, structured output should use forced tool calls with local schema validation, and global usage/remains checks must follow the 9router-derived endpoint order.
</objective>

<quick_start>
For MiniMax work in this project:

- Read `references/minimax-findings.md` before designing or changing code.
- Text/structured generation uses the Anthropic-compatible API with `X-Api-Key`.
- Structured helper output should use forced tool calls with `input_schema`, then local schema validation.
- Global usage/remains checks use `Authorization: Bearer <key>` and the 9router endpoint sequence:
  1. `https://www.minimax.io/v1/token_plan/remains`
  2. `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`
- Success requires HTTP ok, `base_resp.status_code == 0`, and quota rows in `model_remains` / `modelRemains`.
- Never persist raw provider bodies, exact quota values, secrets, raw paper text, raw chunk text, PDFs, embeddings, vectors, or MiniMax-generated trusted facts.
</quick_start>

<essential_principles>
<principle name="bounded_helper_only">
MiniMax is not a project orchestrator, source of truth, or KG importer. Its outputs remain helper evidence until deterministic validation and human/project gates approve downstream use.
</principle>

<principle name="schema_before_trust">
Structured output is acceptable only after local schema validation. Prompt-only JSON is not enough evidence.
</principle>

<principle name="usage_success_is_provider_success">
HTTP 200 is not enough for usage/remains checks. Require `base_resp.status_code == 0` and useful `model_remains` quota data.
</principle>

<principle name="redaction_by_default">
Treat provider responses, quota values, paper/chunk text, and credentials as sensitive. Persist only sanitized metadata, counts, hashes, and boolean verdicts unless a later explicit gate says otherwise.
</principle>
</essential_principles>

<workflow>
When asked to implement, review, or debug MiniMax behavior:

1. Load `references/minimax-findings.md`.
2. Classify the task as one of:
   - text/summary call;
   - structured helper output;
   - usage/remains limit check;
   - Scientific KG integration.
3. Apply the matching proven API surface and safety boundaries.
4. If code changes are needed, add sanitized fixture tests before or alongside implementation.
5. Verify that artifacts/logs do not contain secrets, raw responses, exact quota values, raw paper/chunk text, embeddings, vectors, or raw model content.
6. Keep production KG import and LadybugDB writes blocked unless a later milestone explicitly validates them.
</workflow>

<validation>
Before claiming MiniMax work is complete, verify:

- Structured output path has local schema validation.
- Usage/remains path checks provider success, not only HTTP status.
- `coding_plan/remains` count fields are treated as remaining counts.
- `token_plan/remains` count fields are treated as used counts.
- Logs/artifacts contain only sanitized metadata.
- No production KG write/import path was enabled.
</validation>

<reference_index>
Detailed project findings:

- `references/minimax-findings.md` — M012-M016 MiniMax conclusions, endpoints, parser rules, and safety boundaries.
</reference_index>

<anti_patterns>
Do not:

- infer usage success from HTTP 200 alone;
- stop after `www.minimax.io/v1/token_plan/remains` returns 403;
- use `X-Api-Key` for usage/remains checks unless future evidence changes the contract;
- treat prompt-only JSON as structured-output proof;
- send raw scientific corpus content to MiniMax;
- persist exact quota values or raw response bodies by default;
- let MiniMax create trusted KG facts or write to LadybugDB.
</anti_patterns>

<success_criteria>
MiniMax work follows this skill when:

- The correct API surface is used for the task type.
- 9router-derived usage/remains logic is applied for global limit checks.
- Structured helper output is schema-validated locally.
- Safety boundaries and redaction rules are preserved.
- The result is documented as helper evidence, not production KG authority.
</success_criteria>
