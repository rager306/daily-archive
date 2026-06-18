---
name: glm-zai-safe-helper
description: Applies GLM/Z.ai integration rules for Anthropic-compatible endpoints, model capability checks, env-driven secrets, and future LLM provider module work. Use when working with GLM, Z.ai, GLM Coding Plan, Anthropic-compatible Z.ai APIs, GLM tool use, GLM model selection, multimodal GLM support, or compression/no-compression LLM call scenarios.
---

<objective>
Use GLM/Z.ai through documented, capability-checked API surfaces without scattering provider-specific logic through the extraction pipeline. This skill is for future daily-archive LLM module work: MiniMax and GLM/Z.ai should sit behind a provider abstraction, with explicit scenarios for calls with compression and without compression.
</objective>

<quick_start>
1. Read `references/zai-findings.md` before implementing or reviewing GLM/Z.ai code.
2. Prefer the Z.ai Anthropic-compatible path first for agent/tool integrations, but keep project `.env` provider-namespaced:
   - stored source-of-truth: `GLM_API_KEY`, `GLM_ANTHROPIC_BASE_URL=https://api.z.ai/api/anthropic`, `GLM_MODEL`
   - runtime mapping only: `ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY`, `ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL`
3. Do not store generic `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL` in project `.env` as provider source-of-truth keys; they collide with MiniMax and real Anthropic clients.
4. Do not assume a GLM model is multimodal. Check the specific model capability before using image/PDF/vision/audio/video/file inputs.
5. Do not collect secrets manually. If credentials are missing, use `secure_env_collect` for the project-approved env key.
6. Do not add GLM HTTP calls directly to extraction workers. Put them behind the future LLM module/provider boundary.
</quick_start>

<known_docs_facts>
- Z.ai Claude Code docs show Anthropic-compatible configuration via `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL=https://api.z.ai/api/anthropic`, and `API_TIMEOUT_MS`.
- Z.ai Chat Completion docs expose `POST https://api.z.ai/api/paas/v4/chat/completions` and declare bearer auth.
- The Chat Completion API surface describes multimodal inputs, tool use, streaming, and non-streaming modes.
- Endpoint-level multimodal support is not proof that every GLM model supports every modality.
</known_docs_facts>

<workflow>
## 1. Decide whether GLM/Z.ai is in scope

Use this skill only when the task mentions GLM/Z.ai or the LLM provider module.

If the task is a benchmark fixture, queue, graph, or parser milestone, do not add live GLM calls unless the milestone explicitly authorizes model-provider work.

## 2. Choose the integration surface

Prefer this order:

1. **Anthropic-compatible route** for agent/tool-style calls, Anthropic SDK compatibility, or existing Anthropic-shaped helper code.
2. **Direct Z.ai chat completion route** only when Anthropic-compatible mode lacks a required feature or the future LLM module explicitly needs OpenAI-like chat completion semantics.
3. **No live call** for architecture/design/research milestones.

## 3. Verify model capabilities before non-text use

Before using image, PDF, audio, video, file, or other multimodal inputs:

- identify the exact model ID;
- check current Z.ai docs for that model;
- record the capability source in an artifact or code comment;
- add a runtime capability guard in provider code;
- add a test or fixture proving unsupported modalities fail safely.

Never infer multimodality from the generic endpoint alone.

## 4. Keep secrets safe

Allowed patterns:

- provider-namespaced env-driven config such as `GLM_API_KEY`, `GLM_ANTHROPIC_BASE_URL`, `GLM_CHAT_COMPLETIONS_URL`, `GLM_MODEL`, and `GLM_SMALL_FAST_MODEL`;
- `secure_env_collect` when a required key is missing;
- redacted logs that mention key names only;
- short-lived runtime mapping from `GLM_*` keys to generic `ANTHROPIC_*` names for a specific Z.ai-compatible client process.

Forbidden patterns:

- hardcoded API keys;
- echoing key values;
- asking the user to edit `.env` manually;
- storing generic `ANTHROPIC_AUTH_TOKEN` / `ANTHROPIC_BASE_URL` as project provider source-of-truth keys;
- reusing MiniMax keys for Z.ai unless an explicit project decision says so.

## 5. Preserve the LLM module boundary

Future implementation should expose provider-neutral concepts first, for example:

- `LLMProvider`
- `LLMRequest`
- `LLMResponse`
- `ToolCallResult`
- `CompressionMode = none | provider_native | headroom_candidate`

Provider-specific modules can then implement MiniMax and GLM/Z.ai behind the interface.

Do not import Z.ai HTTP clients directly from extraction pipeline code.

## 6. Compression / no-compression scenarios

Every design should describe two call paths:

- **No compression**: send the selected prompt/context directly to the provider.
- **With compression**: compress or route context before provider call.

Headroom (`https://github.com/chopratejas/headroom`) is only a candidate option. Research it before adoption; verify maintenance state, dependency footprint, license, API compatibility, and whether it preserves evidence/provenance requirements.
</workflow>

<verification_rules>
For code changes involving GLM/Z.ai:

- Run provider-unit tests without live network by default.
- Add a live smoke only when explicitly authorized and credentials are collected securely.
- Verify both success and failure/diagnostic surfaces.
- Ensure logs do not contain secrets, prompts with sensitive corpus text, embeddings, vectors, or raw model payloads unless the milestone explicitly permits controlled artifacts.
- For extraction work, keep graph writes and fact promotion disabled unless separately authorized.
</verification_rules>

<anti_patterns>
- Adding `requests.post("https://api.z.ai/...")` directly inside extraction workers.
- Assuming all GLM models support images/PDFs because Chat Completion is multimodal.
- Copying Claude Code user-level settings into repo files.
- Committing API keys or `.env` changes.
- Introducing Headroom as a dependency before research/design approval.
- Mixing GLM provider refactor into benchmark fixture milestones.
</anti_patterns>

<related_project_context>
- Requirement `R066` tracks the future dedicated LLM module architecture.
- MiniMax remains an existing provider candidate; GLM/Z.ai is an additional provider, not a replacement by default.
- DSPy/optimizer work remains blocked until benchmark/baseline outputs exist.
</related_project_context>
