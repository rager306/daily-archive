# M078 LLM Provider Config Usage Scenarios

## Scope

`arxiv_archive.llm_provider_config` is a pure config module. It does not call MiniMax, GLM/Z.ai, Headroom, or any network API. It does not mutate `os.environ`.

## Scenario 1: GLM/Z.ai with no compression

```python
from arxiv_archive.llm_provider_config import PROVIDER_GLM_ZAI, load_provider_config

config = load_provider_config(PROVIDER_GLM_ZAI, environ=os.environ, compression_mode="none")

# Safe to log:
diagnostics = config.to_sanitized_dict()

# Only at client/subprocess boundary:
runtime_env = config.to_anthropic_runtime_env()
```

Required source-of-truth env names:

- `GLM_API_KEY`
- `GLM_ANTHROPIC_BASE_URL`
- `GLM_MODEL`

Runtime mapping produced by `to_anthropic_runtime_env()`:

- `ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY`
- `ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL`
- `ANTHROPIC_MODEL <- GLM_MODEL`

## Scenario 2: MiniMax with no compression

```python
from arxiv_archive.llm_provider_config import PROVIDER_MINIMAX, load_provider_config

config = load_provider_config(PROVIDER_MINIMAX, environ=os.environ, compression_mode="none")
diagnostics = config.to_sanitized_dict()
runtime_env = config.to_anthropic_runtime_env()
```

Required source-of-truth env names:

- `MINIMAX_API_KEY`
- `MINIMAX_ANTHROPIC_BASE_URL`
- `MINIMAX_MODEL`

## Scenario 3: Headroom candidate compression path

```python
from arxiv_archive.llm_provider_config import (
    COMPRESSION_HEADROOM_CANDIDATE,
    PROVIDER_GLM_ZAI,
    load_provider_config,
)

config = load_provider_config(
    PROVIDER_GLM_ZAI,
    environ=os.environ,
    compression_mode=COMPRESSION_HEADROOM_CANDIDATE,
)
```

This only labels the scenario as `headroom_candidate`; it does not import, install, or call Headroom. A future milestone must research Headroom maintenance state, license, API shape, provenance preservation, and failure modes before adopting it.

## Safe diagnostics

Use `to_sanitized_dict()` for logs/artifacts. It includes:

- provider name
- env key names
- non-secret URLs/models/timeouts
- `api_key_present` boolean
- compression mode

It does not include API key values.

## Forbidden usage

- Do not print `runtime_env`.
- Do not store `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL` in project `.env` as source-of-truth keys.
- Do not mutate global `os.environ` with provider mappings unless launching a deliberately isolated subprocess.
- Do not call live provider APIs in config tests.
- Do not assume GLM/Z.ai multimodal support without checking the exact model capability.

## Future migration note

Existing MiniMax worker code can later be migrated to load `MINIMAX_*` via `load_provider_config(PROVIDER_MINIMAX)` while preserving current mock/live transport separation. That migration is intentionally outside M078.
