# ADR-024: Statistical-First Extraction Principle

**Status:** Accepted  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0  
**Scope:** extraction / llm / data-preparation  
**Binding Level:** binding  
**Revisable:** yes, with implementation evidence

## 0. One-line Decision

> Every LLM extraction stage MUST be preceded by deterministic/statistical pre-processing (YAKE keywords, TF-IDF summaries, graph community detection, embedding similarity). The LLM receives both the raw text AND the statistical context.

## 1. Context

LLM API calls are expensive, rate-limited, and can hallucinate. Statistical algorithms are free, deterministic, and fast. The Core-then-Modes pipeline (Agents-K1) already reduces LLM calls by ~50%. We can reduce further by grounding each LLM call with statistical evidence.

## 2. Decision

### Per-stage statistical pre-processing

| Extraction stage | Statistical pre-processing | LLM receives |
|---|---|---|
| Core: Entity extraction | YAKE keywords per chunk + per section | Chunk text + keyword list with scores |
| Core: Binary relations | Co-occurrence matrix from keywords | Entity pairs + co-occurrence frequency |
| Upgrade: Citation type | Citation graph structure (who cites whom) | Citation pair + graph path length |
| Upgrade: Causal relations | Correlation hints from co-occurrence + embeddings | Entity pair + similarity score + co-occurrence |
| Upgrade: Abstract entities | Section type classification (regex/TF-IDF) | Section text + predicted type (Introduction/Method/Results) |
| Grounding for queries | Per-page keyword statistics → query routing | Query + relevant page IDs from keyword match |

### YAKE usage expansion

Current: `retrieval/keyword_extractor.py` extracts keywords for scoring.

New uses:
1. **Per-chunk keyword profile**: YAKE keywords for each SemanticChunk → stored as metadata
2. **Per-section keyword statistics**: aggregate keywords per section → section fingerprint
3. **Query grounding**: match query keywords against per-page keyword index → route to relevant pages before LLM
4. **Entity pre-detection**: YAKE keyword candidates → LLM classifies into typed entities (Method, Dataset, etc.)
5. **Dedup assist**: keyword overlap between chunks → dedup signal

### Rule

```python
# Every extraction function MUST accept statistical_context parameter:
def extract_entities(
    chunk: str,
    statistical_context: StatisticalContext,  # keywords, co-occurrence, embeddings
) -> list[TypedEntity]:
    ...
```

`StatisticalContext` includes:
- `yake_keywords: list[tuple[str, float]]` — keywords with YAKE scores
- `section_type: str` — predicted section type (Introduction/Method/Results/etc.)
- `embedding: list[float]` — BGE-M3 embedding for similarity
- `co_occurrence: dict[str, dict[str, int]]` — entity co-occurrence counts
- `citation_position: int | None` — position in citation graph (BFS depth)

## 3. Applies To

- All extraction pipeline stages (Core and Upgrade modes)
- Query grounding for retrieval
- Agent cognitive map building (future)

## 4. Safety

Statistical pre-processing is deterministic and cannot:
- Write to graph
- Promote facts
- Authorize imports
- Make claims about extraction quality

It ONLY provides context for LLM calls and deterministic routing.

## 5. LLM Reading Notes

- **Binding**: Statistical pre-processing is MANDATORY before LLM extraction.
- **YAKE** is the primary keyword extraction algorithm (already in codebase).
- **Cost reduction**: statistical pre-processing reduces LLM calls by providing grounded context.
- **Not authorized**: graph writes, fact promotion, quality claims.
