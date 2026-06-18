# ADR-025: Multi-Provider LLM Architecture with Per-Provider Rate Limits

**Status:** Accepted  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0  
**Scope:** llm / queue / provider-management  
**Binding Level:** binding  
**Revisable:** yes, with provider API change evidence

## 0. One-line Decision

> The LLM module must support multiple providers (MiniMax primary, GLM secondary, future hot-pluggable) with per-provider rate limit checking before API calls, automatic fallback on rate limit exhaustion, and durable usage tracking per provider in the queue.

## 1. Context

We have two active LLM providers:
- **MiniMax** (M3-512k, M2.7-highspeed) — primary; has token_plan/remains endpoint for checking limits
- **GLM/Z.ai** (GLM-5.2, GLM-4.5-Air) — secondary; 5-hour rolling limit on subscription

Both share the Anthropic-compatible API surface. `provider_config.py` already provides provider-neutral config with namespaced env keys.

## 2. Decision

### 2.1 Provider Abstraction

```python
class LLMProviderInterface(Protocol):
    def can_make_request(self, estimated_tokens: int) -> bool: ...
    def make_request(self, messages: list[dict], **kwargs) -> LLMResponse: ...
    def get_usage(self) -> ProviderUsage: ...
```

### 2.2 Rate Limit Strategy

| Provider | Limit mechanism | Check before call | Fallback |
|---|---|---|---|
| MiniMax | Token plan remains endpoint | `minimax_usage.py` → `token_plan/remains` | GLM-5.2 |
| GLM | 5-hour rolling window | Track calls in queue; estimate remaining | Queue job for later |
| Future | Per-provider config | Provider-specific checker | Per config |

### 2.3 Queue Integration

The durable queue (when activated per ADR-017) must:
1. Track token usage per provider per job
2. Check `can_make_request(provider)` before dispatching extraction jobs
3. Route to fallback provider when primary is exhausted
4. Backoff and retry when all providers are rate-limited
5. Expose usage metrics for monitoring

### 2.4 Compression (Headroom)

Headroom (`https://github.com/chopratejas/headroom`) is registered as `COMPRESSION_HEADROOM_CANDIDATE`.

Adoption criteria:
1. Maintenance state verified
2. Dependency footprint acceptable
3. License compatible (self-hosted)
4. API works with MiniMax-M3 and GLM-5.2
5. Provenance preserved (no evidence/span loss)
6. Measurable F1 improvement or cost reduction
7. No quality degradation on extraction benchmarks

**Current status**: NOT adopted. Research only.

## 3. Applies To

- All LLM calls in extraction pipeline
- Agent LLM calls (future)
- Provider routing in queue
- models.yaml configuration

## 4. LLM Reading Notes

- **Binding**: Multi-provider with rate limit checking is mandatory.
- **MiniMax** primary, **GLM** secondary. Config-driven, no code changes for new providers.
- **Headroom**: candidate only, not adopted.
- **Rate limits**: check BEFORE call, not after failure.
