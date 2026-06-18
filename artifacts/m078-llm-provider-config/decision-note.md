# M078 Decision Note

## Decision

Use typed provider config dataclasses for the first LLM provider module boundary.

## Selected shape

- `LLMProviderConfig`
- `load_provider_config(provider, environ=None, compression_mode="none")`
- `to_anthropic_runtime_env()`
- `to_sanitized_dict()`

## Why

Typed provider config is deep enough to preserve MiniMax and GLM/Z.ai provider differences while still avoiding live client behavior. It supports namespaced env keys, no live API calls, explicit runtime mapping to generic Anthropic-compatible names, and secret-safe diagnostics.

## Boundaries

- no live API call
- no global `os.environ` mutation
- no provider client factory yet
- no Headroom dependency
- no graph writes or promotion

## Providers in scope

- MiniMax
- GLM/Z.ai

## Compression

M078 only represents compression scenarios:

- `none`
- `provider_native`
- `headroom_candidate`

It does not implement compression or adopt Headroom.
