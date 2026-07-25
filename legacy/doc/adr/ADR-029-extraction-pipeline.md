# ADR-029: Extraction Pipeline Architecture

**Status:** Accepted (binding)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0 S03  
**Scope:** extraction / llm / dspy / pipeline  
**Binding Level:** binding  
**Revisable:** yes, with extraction evidence and cost validation

## 0. One-line Decision

> daily-archive will implement a Core-then-Modes extraction pipeline with statistical-first pre-processing (ADR-024), multi-provider LLM routing with rate limits (ADR-025), DSPy BootstrapFewShot optimization, and typed schema output (ADR-028). Estimated cost: ~$0.07/article, ~36 LLM calls/article, ~20 articles/day throughput.

## 1. Context

Current state: fixture-only extraction contracts, no real LLM extraction.
ADR-023 defines Layer 3 (Extraction) as a key gap.
ADR-024 mandates statistical pre-processing before every LLM call.
ADR-025 mandates per-provider rate limit checking.
ADR-027 mandates 3-lane scheduler integration.
ADR-028 defines typed output schema (27 relations, 5 modules).

Agents-K1 uses GRPO-trained 4B model. We have no GPU → DSPy + API instead.

## 2. Decision

### 2.1 Core-then-Modes Factorization

| Stage | LLM calls/chunk | Statistical pre-processing |
|---|---|---|
| Core: Entity extraction | 1 | YAKE keywords, section type |
| Core: Binary relations | 1 | Co-occurrence matrix |
| Projection: binary/provenance | 0 | Deterministic |
| Upgrade: Relation type | 1 | Citation graph structure |
| Upgrade: Abstract entities | 1 | Section type, keywords |
| Upgrade: Citation relation | 0.5 | BFS depth |
| **Total** | **~4.5/chunk** | — |

### 2.2 DSPy Optimization

- Phase 2: BootstrapFewShot with 10 manually labeled chunks
- Phase 2+: MIPRO if F1 < 0.6
- Phase 2+: BootstrapRandomSearch if needed

### 2.3 Provider Routing

- MiniMax M3-512k: primary for Core + Abstract (large context)
- MiniMax M2.7-highspeed: fast classification (Relation type, Citation)
- GLM-5.2/GLM-4.5-Air: fallback when MiniMax rate-limited
- Rate limit check BEFORE call, not after failure

### 2.4 Cost Model

- ~36 LLM calls per article
- ~60K input tokens + ~13K output tokens
- ~$0.07/article (MiniMax blended)
- ~20 articles/day throughput (MiniMax + GLM combined)

### 2.5 Scheduler Integration (ADR-027)

Extraction jobs tagged with `ResourceProfile(llm_required=True)`.
Scheduler checks `can_make_request(provider)` before dispatch.
CPU jobs (parsing/chunking) continue when LLM lane is full.

### 2.6 Headroom

NOT adopted. 7 evaluation criteria defined (ADR-025).
Decision gate: all criteria pass before adoption.

## 3. Applies To

- Layer 3 (Extraction) of 7-layer architecture (ADR-023)
- Typed schema output (ADR-028)
- Queue scheduler LLM lane (ADR-027)
- Review gate (Layer 5): extraction output is CandidatePacket

## 4. LLM Reading Notes

- **Binding**: Core-then-Modes with statistical-first is mandatory.
- **Cost**: ~$0.07/article, ~20 articles/day.
- **DSPy**: BootstrapFewShot first; MIPRO if F1 < 0.6.
- **Provider routing**: MiniMax primary, GLM fallback, rate limit BEFORE call.
- **Not authorized**: graph writes, Headroom adoption, production imports.
