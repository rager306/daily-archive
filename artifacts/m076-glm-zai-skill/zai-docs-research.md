# M076 Z.ai GLM Docs Research

## Scope

This research supports a project-local `glm-zai-safe-helper` skill. It does not call Z.ai APIs, collect secrets, or configure local tools.

## Source URLs checked

- Z.ai API introduction: https://docs.z.ai/api-reference/introduction
- Z.ai GLM Coding Plan overview: https://docs.z.ai/devpack/overview
- Z.ai GLM Coding Plan quick start: https://docs.z.ai/devpack/quick-start
- Z.ai Claude Code scenario: https://docs.z.ai/scenario-example/develop-tools/claude.md
- Z.ai Chat Completion API reference: https://docs.z.ai/api-reference/llm/chat-completion.md
- Z.ai docs index: https://docs.z.ai/llms.txt

## Anthropic-compatible emphasis

The Claude Code scenario docs show GLM Coding Plan configuration through Anthropic-compatible Claude environment variables:

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "your_zai_api_key",
    "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
    "API_TIMEOUT_MS": "3000000"
  }
}
```

For future daily-archive GLM work, prefer the Anthropic-compatible route first when adapting existing Anthropic-style helper code or Claude Code style agent integrations.

## Direct chat endpoint

The Chat Completion API reference documents:

```text
POST https://api.z.ai/api/paas/v4/chat/completions
```

The reference describes the endpoint as supporting conversation messages, configurable parameters, tool use, streaming/non-streaming output, and multimodal inputs.

## Multimodal caveat

The Chat Completion API page describes multimodal inputs in the API surface: text, images, audio, video, and file. This must **not** be interpreted as every GLM model supporting every modality.

Skill and future provider code must verify per-model capabilities before using image/PDF/vision/audio/video/file inputs. In particular, do not assume a coding-plan model is multimodal just because the endpoint supports multimodal input shapes.

## Secrets and env handling

- Do not commit API keys.
- Do not echo API keys.
- Do not ask the user to edit `.env` manually.
- If a future implementation needs `ZAI_API_KEY`, `GLM_API_KEY`, or equivalent, collect it with `secure_env_collect`.
- Keep GLM/Z.ai secrets separate from MiniMax secrets unless a future explicit decision says otherwise.

## Architecture boundary

This research supports R066: a future dedicated LLM module should expose provider-neutral calls, then implement providers such as MiniMax and GLM/Z.ai behind that boundary.

Do not scatter GLM-specific HTTP calls through extraction pipeline code. Add them behind the future LLM provider module.

## Headroom and compression boundary

Headroom (`https://github.com/chopratejas/headroom`) remains an option to research later. The GLM skill should mention compression/no-compression scenarios but must not adopt Headroom until a separate research/design step verifies fit, maintenance state, and API compatibility.
