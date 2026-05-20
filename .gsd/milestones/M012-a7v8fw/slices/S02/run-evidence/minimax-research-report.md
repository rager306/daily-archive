# MiniMax compatibility research report

## Verdict

MiniMax is plausible as an **optional bounded helper** for redacted paper-repair review, schema/diagnostic review, or suggestion generation, but it must not become the orchestrator, parser, source of truth, trusted fact creator, or production writer.

The strongest initial fit is a text/chat helper using `MiniMax-M2.7` through the official Anthropic-compatible API, with OpenAI-compatible API as a fallback if local adapter seams prefer it. Direct PDF/raw document ingestion is not supported by the consulted text APIs and should remain out of scope.

## Sources consulted

Official MiniMax sources:

- API Overview: `https://platform.minimax.io/docs/api-reference/api-overview`
- Text Generation Guide
- Compatible Anthropic API
- Text Chat, Anthropic-compatible
- Compatible OpenAI API
- Text Chat, OpenAI-compatible
- AI SDK
- Tool Use and interleaved thinking guide
- Models guide
- Rate limits
- Error codes
- Pay-as-you-go pricing
- File upload and file retrieval docs
- Token Plan MCP / image understanding docs
- API privacy policy

Parent verification fetched the API overview page and confirmed the official navigation exposes Text, Speech, Video, Image, Music, File, rate limit, pricing, and error-code surfaces.

## Auth and base URLs

MiniMax provides international and China-region endpoints. For this project, international endpoints are the expected initial target unless user/project config says otherwise.

| Surface | Endpoint/base URL | Auth shape | Notes |
|---|---|---|---|
| Anthropic-compatible SDK | `https://api.minimax.io/anthropic` | API key passed as Anthropic client key | Officially recommended text surface. |
| Anthropic-compatible HTTP | `https://api.minimax.io/anthropic/v1/messages` | Docs show `X-Api-Key` and examples also show bearer-style headers | Live probe must verify accepted header. |
| OpenAI-compatible SDK/API | `https://api.minimax.io/v1`, `/v1/chat/completions` | `Authorization: Bearer <token>` | Useful if local provider abstraction is OpenAI-compatible. |
| China region | `https://api.minimaxi.com/...` | Same concept | Separate regional decision; not default for this project. |

Key handling requirements:

- Do not ask user to paste keys.
- Use `secure_env_collect` if a live probe is explicitly approved.
- Never log or persist key values.

## Model/capability findings

Relevant text models:

- `MiniMax-M2.7` — recommended first target; long context; reasoning/tool-use capable.
- `MiniMax-M2.7-highspeed` — higher speed/cost variant.
- `MiniMax-M2.5` / `MiniMax-M2.5-highspeed` — fallback family.
- `MiniMax-M2.1` / `MiniMax-M2` — older supported surfaces.
- `M2-her` — not a scientific extraction/review fit.

Text/chat capabilities:

- system prompt;
- chat messages;
- streaming;
- reasoning/thinking fields;
- tool calls;
- temperature/top_p/max token controls;
- long-context text input.

Important API constraints:

- Some OpenAI/Anthropic parameters may be unsupported or ignored.
- OpenAI-compatible `function_call` is not supported; tools should be used instead.
- Temperature has strict accepted range.
- Multi-turn tool use may require preserving complete assistant responses including thinking/tool-use blocks, which conflicts with conservative no-CoT/no-raw telemetry unless carefully controlled.

## Modalities and document/PDF implications

Direct text/chat APIs are not a safe PDF/document ingestion path:

- Anthropic-compatible docs indicate `image` and `document` message types are not supported yet.
- OpenAI-compatible docs indicate image/audio inputs are not currently supported.
- AI SDK docs indicate image/file inputs are not supported yet.
- File upload API is oriented to other MiniMax workflows such as audio/TTS and is not a scientific PDF parser.
- Token Plan MCP includes image understanding for common image formats, but that is a separate surface and not direct PDF ingestion.

Implication:

- MiniMax should not replace Docling, Marker, GROBID, or local conversion.
- For paper repair/review, MiniMax can only receive redacted/bounded text metadata or explicitly approved tiny snippets/rendered images.
- Full PDFs and full Markdown should not be sent by default.

## Structured output and tools

Supported:

- Tool definitions/tool calls in compatible APIs.
- JSON-like arguments in tool calls.
- Model outputs that can be requested in JSON style.

Not proven in consulted docs:

- Strict JSON schema constrained decoding equivalent to guaranteed provider-side schema enforcement.
- Reliable direct PDF/document analysis.

Project policy:

```text
MiniMax helper output -> local parse -> local schema validate -> review_required artifact -> no trusted fact creation
```

Invalid JSON or schema mismatch is a helper failure, not a pipeline failure.

## Pricing/rate/privacy notes

Rate limits from docs are high enough for bounded probes, but do not justify batch automation.

Pricing from current docs makes small helper probes inexpensive, but full-paper calls can still create cost/privacy exposure.

Privacy/data handling:

- MiniMax processes service input/output.
- Cross-border/cloud processing may apply.
- Sending raw paper text, full extracted Markdown, PDFs, images, or user/private content requires explicit data-handling approval.

Default for this project: no raw paper text or PDFs in MiniMax calls.

## Error/failure modes

Known documented error classes include:

- unknown/internal errors;
- request timeout;
- rate limit;
- auth/token mismatch;
- insufficient balance;
- input sensitive/output sensitive;
- token limit;
- invalid parameters;
- usage limit exceeded;
- connection/rate growth limits.

Adapter implications:

- Fail closed.
- Persist only status code, error code, model, endpoint, request ID if available, token counts, and redacted payload hashes.
- Do not retry sensitive-input/sensitive-output failures with the same payload.
- Do not escalate from bounded probe to batch mode automatically.

## Marker/custom adapter implications

MiniMax should be a review/suggestion provider, not a parser.

Recommended future adapter boundary:

```text
local converter/parser output
-> redaction and bounded sample builder
-> optional MiniMax helper call
-> local JSON/schema validation
-> review_required artifact
-> no production import/write
```

Allowed future helper roles:

- classify redacted conversion/chunking diagnostics;
- suggest repair category for a bounded artifact;
- compare redacted section/table/figure metadata;
- identify why a target is insufficient for graph readiness.

Disallowed roles:

- source-of-truth extraction;
- autonomous paper repair;
- direct PDF ingestion;
- trusted KG claim/entity/relation creation;
- production LadybugDB write;
- MiniMax as orchestrator.

## Minimal bounded probe plan

No live calls were run in S02 research.

Recommended probe sequence:

1. **No-call dry run**: build a redacted request payload preview and validate no raw text/secrets/PDF bytes.
2. **Auth/header smoke test**: if credentials are explicitly collected later, send one tiny synthetic prompt asking for `{ "ok": true }`.
3. **Structured JSON helper test**: synthetic non-paper fixture, local schema validation.
4. **Redacted paper-review helper**: metadata-only artifact row, no raw text.
5. **Optional image understanding**: only if separately approved and only synthetic/non-sensitive images first.

## Go/no-go

### Go

- Go for future non-production optional helper probes.
- Go for no-call request-payload dry run now.
- Go for a live smoke test only after secure credential collection and explicit approval.

### No-go

- No-go for MiniMax as production orchestrator.
- No-go for source-of-truth extraction.
- No-go for direct PDF/raw paper ingestion.
- No-go for trusted KG import or LadybugDB writes.
- No-go for unattended repair/scaling.

## Preconditions before activation

- Credentials collected with `secure_env_collect` only.
- Privacy/data-handling decision recorded.
- Endpoint/header behavior verified with a tiny synthetic call.
- Output schema validation implemented locally.
- Budget/rate caps configured.
- Redaction and no-raw-text payload guard implemented.
- Helper output stored as `review_required`, never import-ready.

## Safety flags

- raw_text_included: false
- chunk_text_included: false
- embeddings_included: false
- vectors_included: false
- secrets_included: false
- minimax_orchestrator_allowed: false
- production_import_attempted: false
- ladybugdb_written: false
