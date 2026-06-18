# Z.ai GLM Findings for daily-archive

Source research artifact: `artifacts/m076-glm-zai-skill/zai-docs-research.md`.

## Official docs checked

- API introduction: https://docs.z.ai/api-reference/introduction
- GLM Coding Plan overview: https://docs.z.ai/devpack/overview
- GLM Coding Plan quick start: https://docs.z.ai/devpack/quick-start
- Claude Code scenario: https://docs.z.ai/scenario-example/develop-tools/claude.md
- Chat Completion API: https://docs.z.ai/api-reference/llm/chat-completion.md
- Docs index: https://docs.z.ai/llms.txt

## Anthropic-compatible configuration

Z.ai Claude Code docs show this environment shape:

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "your_zai_api_key",
    "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
    "API_TIMEOUT_MS": "3000000"
  }
}
```

For daily-archive provider work, prefer this Anthropic-compatible path first when adapting Anthropic-shaped helper code or tool-call flows.

Project `.env` should still store provider-namespaced keys rather than generic Anthropic-compatible names:

```text
GLM_API_KEY=<your-glm-zai-api-key-here>
GLM_ANTHROPIC_BASE_URL=https://api.z.ai/api/anthropic
GLM_MODEL=glm-5.2
```

At runtime, a specific GLM client process may map:

```text
ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY
ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL
ANTHROPIC_MODEL <- GLM_MODEL
```

Do not store `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL` in project `.env` as source-of-truth keys because they collide with MiniMax and real Anthropic client setups.

## Direct chat completion route

Z.ai Chat Completion API reference documents:

```text
POST https://api.z.ai/api/paas/v4/chat/completions
```

The OpenAPI page declares bearer auth and describes chat completions with configurable parameters, tool use, streaming/non-streaming output, and multimodal input shapes.

## Capability rule

Endpoint-level multimodal support does not mean every GLM model supports every modality. Before using images, PDFs, audio, video, or files, verify the exact model capability in current docs and add runtime/test coverage for unsupported modalities.

## Secret handling

Never commit or print keys. Use env-driven provider-specific config and `secure_env_collect` if a key is needed. Keep Z.ai/GLM credentials separate from MiniMax unless a future explicit decision says otherwise.

Canonical secret/config names for this project:

- `GLM_API_KEY`
- `GLM_ANTHROPIC_BASE_URL`
- `GLM_CHAT_COMPLETIONS_URL`
- `GLM_MODEL`
- `GLM_SMALL_FAST_MODEL`
- `MINIMAX_API_KEY`

Generic `ANTHROPIC_*` names are runtime compatibility mappings only.

## LLM module boundary

GLM/Z.ai should be implemented behind the future provider-neutral LLM module, not directly in extraction workers or benchmark scripts.

## Compression boundary

Design both no-compression and compression call scenarios. Treat Headroom as a candidate requiring separate research; do not adopt it as a dependency from this skill alone.
