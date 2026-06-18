# M077 Provider Env Namespace Strategy

## Problem

`/root/glm.sh` uses Anthropic-compatible generic variables such as:

- `ANTHROPIC_BASE_URL`
- `ANTHROPIC_AUTH_TOKEN`
- `ANTHROPIC_MODEL`
- `ANTHROPIC_SMALL_FAST_MODEL`
- `ANTHROPIC_DEFAULT_SONNET_MODEL`
- `ANTHROPIC_DEFAULT_OPUS_MODEL`
- `ANTHROPIC_DEFAULT_HAIKU_MODEL`

These names are useful when launching Claude-compatible clients, but they are unsafe as persistent project `.env` source-of-truth keys because MiniMax, GLM/Z.ai, and real Anthropic integrations can all need Anthropic-compatible environment names.

## Decision

Persist provider-specific, namespaced variables in project `.env` / `.env.example`.

Use generic `ANTHROPIC_*` names only as a **runtime mapping** when starting a specific provider client or subprocess.

## Canonical GLM/Z.ai env keys

| Key | Purpose | Example / placeholder |
|---|---|---|
| `GLM_API_KEY` | Canonical daily-archive GLM/Z.ai secret | `<your-glm-zai-api-key-here>` |
| `GLM_ANTHROPIC_BASE_URL` | Anthropic-compatible Z.ai base URL | `https://api.z.ai/api/anthropic` |
| `GLM_CHAT_COMPLETIONS_URL` | Direct Z.ai chat completion endpoint | `https://api.z.ai/api/paas/v4/chat/completions` |
| `GLM_MODEL` | Primary GLM model for non-small calls | `glm-5.2` |
| `GLM_SMALL_FAST_MODEL` | Smaller/faster GLM model | `GLM-4.5-Air` |
| `GLM_API_TIMEOUT_MS` | Provider timeout | `3000000` |
| `GLM_DISABLE_NONESSENTIAL_TRAFFIC` | Optional compatibility flag for coding tools | `1` |

Optional Claude-Code adapter mapping keys, only if the future LLM module needs them:

| Key | Runtime generic target |
|---|---|
| `GLM_CLAUDE_SONNET_MODEL` | `ANTHROPIC_DEFAULT_SONNET_MODEL` |
| `GLM_CLAUDE_OPUS_MODEL` | `ANTHROPIC_DEFAULT_OPUS_MODEL` |
| `GLM_CLAUDE_HAIKU_MODEL` | `ANTHROPIC_DEFAULT_HAIKU_MODEL` |

## Canonical MiniMax env keys

| Key | Purpose |
|---|---|
| `MINIMAX_API_KEY` | Canonical MiniMax secret. |
| `MINIMAX_ANTHROPIC_BASE_URL` | Anthropic-compatible MiniMax base URL, if using Anthropic-shaped helper code. |
| `MINIMAX_OPENAI_BASE_URL` | OpenAI-compatible MiniMax base URL. |
| `MINIMAX_MODEL` | Primary MiniMax model. |
| `MINIMAX_SMALL_FAST_MODEL` | Optional smaller/faster MiniMax model. |

Existing legacy MiniMax keys such as `MINIMAX_TOKEN_PLAN_KEY` can remain as compatibility aliases, but the future LLM module should normalize them into provider config and avoid exporting generic `ANTHROPIC_*` globally.

## Runtime mapping

For a GLM/Z.ai Anthropic-compatible subprocess:

```text
ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY
ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL
ANTHROPIC_MODEL <- GLM_MODEL
ANTHROPIC_SMALL_FAST_MODEL <- GLM_SMALL_FAST_MODEL
API_TIMEOUT_MS <- GLM_API_TIMEOUT_MS
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC <- GLM_DISABLE_NONESSENTIAL_TRAFFIC
```

For a MiniMax Anthropic-compatible adapter, map provider-specific MiniMax config inside the adapter. Do not set project-global `ANTHROPIC_AUTH_TOKEN` to a MiniMax or GLM key.

## Collision rules

1. Do not store `ANTHROPIC_AUTH_TOKEN` in project `.env` as a provider source of truth.
2. Do not store `ANTHROPIC_BASE_URL` in project `.env` as a provider source of truth.
3. Do not reuse MiniMax key names for GLM/Z.ai or GLM/Z.ai key names for MiniMax.
4. If an external client requires generic Anthropic env names, generate them in a short-lived process env from provider-specific keys.
5. Logs may mention env key names, never values.
6. Use `secure_env_collect` for missing secrets; never ask the user to edit `.env` manually.

## Future LLM module guidance

The future provider-neutral LLM module should load provider config into typed structures, for example:

```text
LLMProviderConfig(provider="glm_zai", api_key_env="GLM_API_KEY", anthropic_base_url_env="GLM_ANTHROPIC_BASE_URL")
LLMProviderConfig(provider="minimax", api_key_env="MINIMAX_API_KEY", anthropic_base_url_env="MINIMAX_ANTHROPIC_BASE_URL")
```

Provider adapters should then create request clients without leaking provider-specific globals into the wider process.

## Secret handling

- No real key values belong in `.env.example`, docs, artifacts, logs, or tests.
- If `GLM_API_KEY` is missing during a future live probe, collect it via `secure_env_collect`.
- Do not copy values from `/root/glm.sh` into repository files.
- Keep `.env` local and uncommitted.
