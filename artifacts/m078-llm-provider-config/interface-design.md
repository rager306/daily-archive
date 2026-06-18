# M078 LLM Provider Config Interface Design

## Context

R066 requires a dedicated LLM module that can support MiniMax and GLM/Z.ai without leaking provider-specific HTTP/env details into extraction workers. M077 established that provider-specific env keys are the source of truth and generic `ANTHROPIC_*` variables are runtime-only compatibility mappings.

M078 should not perform live API calls and should not migrate existing MiniMax worker behavior yet. It should provide a safe config boundary that future provider clients can consume.

## Option A: thin env helper functions

Shape:

```python
get_glm_env(os.environ) -> dict[str, str]
get_minimax_env(os.environ) -> dict[str, str]
```

Pros:

- Smallest code footprint.
- Easy to test.
- Directly supports runtime mapping.

Cons:

- Too easy to pass secret-bearing dicts into logs.
- No explicit provider/model/compression semantics.
- Harder to extend for Headroom or no-compression/compression scenarios.

Verdict: useful internally, but too shallow as the public module boundary.

## Option B: typed provider config dataclasses

Shape:

```python
LLMProviderConfig(provider, api_key_env, base_url, model, small_fast_model, compression_mode)
load_provider_config(provider, environ) -> LLMProviderConfig
config.to_anthropic_runtime_env() -> dict[str, str]
config.to_sanitized_dict() -> dict[str, object]
```

Pros:

- Keeps source-of-truth env names provider-specific.
- Makes runtime mapping explicit and short-lived.
- Can expose sanitized diagnostics without key values.
- Can represent `compression_mode=none|provider_native|headroom_candidate` without adopting Headroom.
- Easy to test with synthetic env dicts.

Cons:

- Slightly more code than Option A.
- Does not make live calls; future provider client factory still needed.

Verdict: recommended for M078.

## Option C: full provider client factory

Shape:

```python
client = LLMClientFactory.from_env(provider).create_client()
client.messages(...)
```

Pros:

- Most ergonomic for future callers.
- Can hide transport differences.

Cons:

- Premature for current milestone.
- Would tempt live API integration before provider config is stable.
- Larger blast radius if existing MiniMax worker is migrated now.
- Harder to keep Headroom as research-only.

Verdict: defer. Build after provider config is proven.

## Recommended design

Use **Option B: typed provider config dataclasses**.

M078 should add a pure module, tentatively `src/arxiv_archive/llm_provider_config.py`, with:

- `LLMProvider` enum-like literals for `glm_zai` and `minimax`.
- `CompressionMode` values: `none`, `provider_native`, `headroom_candidate`.
- `LLMProviderConfig` dataclass.
- `load_provider_config(provider, environ=None, compression_mode="none")`.
- `to_anthropic_runtime_env()` for explicit generic `ANTHROPIC_*` runtime mapping.
- `to_sanitized_dict()` that exposes env key names and non-secret config, never key values.

## Runtime mapping rule

For GLM/Z.ai:

```text
ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY
ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL
ANTHROPIC_MODEL <- GLM_MODEL
ANTHROPIC_SMALL_FAST_MODEL <- GLM_SMALL_FAST_MODEL
API_TIMEOUT_MS <- GLM_API_TIMEOUT_MS
```

For MiniMax Anthropic-compatible clients:

```text
ANTHROPIC_AUTH_TOKEN <- MINIMAX_API_KEY
ANTHROPIC_BASE_URL <- MINIMAX_ANTHROPIC_BASE_URL
ANTHROPIC_MODEL <- MINIMAX_MODEL
ANTHROPIC_SMALL_FAST_MODEL <- MINIMAX_SMALL_FAST_MODEL
```

The module must not mutate `os.environ`; callers explicitly pass the returned mapping to a subprocess/client.

## Compression scenario representation

M078 should represent, not implement, compression choices:

- `none`: send selected context directly.
- `provider_native`: use provider-native context controls if a future provider supports them.
- `headroom_candidate`: marks a scenario intended for future Headroom research; it must not import or depend on Headroom.

## Non-goals

- No live MiniMax/GLM/Z.ai API call.
- No credential collection.
- No Headroom dependency adoption.
- No migration of `article_artifact_worker.HttpTransport` yet.
- No graph writes or fact promotion.
