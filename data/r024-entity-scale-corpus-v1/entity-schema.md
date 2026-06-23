# R024 Entity-Scale Schema (M119)

**Generated**: 2026-06-22T17:00Z  
**Milestone**: M119-3eb3vy (catalog-limited pivot)

## Pivot Rationale

R024 staged validation (M116/M117/M118) covered 10/20/53 articles. The original
goal was 200+ articles for the "one-week" stage, but the local catalog is
exhausted at **55 articles** (53 extractable: 21 HTML-native + 32 PDF-converted
via pymupdf, 2 with no local source).

To continue R024 scale validation without requiring new catalog sources (which
would violate the no-network constraint), M119 pivots to **entity-level scale
validation**: instead of more articles, use **more entity types per article**.

## Schema

10 entity types per article (vs 5 in M118):

| # | Entity Type | Source | Derivation | M025 Chunk Type |
|---|-------------|--------|------------|-----------------|
| 1 | metadata | m025_chunk_types | M025 chunk_type | metadata |
| 2 | table_context | m025_chunk_types | M025 chunk_type | table_context |
| 3 | figure_caption_context | m025_chunk_types | M025 chunk_type | figure_caption_context |
| 4 | citation_context | m025_chunk_types | M025 chunk_type | citation_context |
| 5 | retrieval_context | m025_chunk_types | M025 chunk_type | retrieval_context |
| 6 | title | article_metadata | article.identity.title | n/a |
| 7 | authors | article_metadata | article.identity.authors | n/a |
| 8 | abstract | article_metadata | article.identity.abstract | n/a |
| 9 | references | synthetic | derived from citation_context chunks | citation_context |
| 10 | keywords | article_metadata | article.topic_tags | n/a |

**Scale**:
- M118 baseline: 53 articles × 5 entities = **265 entities**
- M119 target: 53 articles × 10 entities = **530 entities** (2.0x scale)

## Source Breakdown

- **5 from M025 chunk types**: structural entity types derived from existing
  M025 chunk_type fields
- **4 from article metadata**: title, authors, abstract, keywords (sourced
  from `article.json` identity/topics)
- **1 synthetic**: references (derived from citation_context chunks; no
  real citation parsing)

## Fail-Closed Constraints

All entity types are derived deterministically from existing artifacts:

- **No network fetches**: all sources are local (M025 chunks + article.json)
- **No production extraction**: no LLM-based extraction
- **No LadybugDB/FalkorDB/Neo4j writes**: entities stored as JSON only
- **Synthetic only**: documented; real LLM-based extraction deferred

## Future Work

1. **Catalog expansion**: source 200+ articles via real production pipeline
   (with explicit no-network override per M025 patterns)
2. **Real entity extraction**: replace synthetic with LLM-based extraction
3. **Real citation parsing**: extract reference lists from article bodies
4. **Schema evolution**: add new entity types as new sources become available
5. **Production-readiness gate**: when/if approved per R019-R023/R050-R056

## Pivot Decision Tree

```
catalog ≥ 200 articles  → corpus expansion (not feasible, catalog exhausted)
catalog < 200 articles  → entity-level scale (this milestone)
catalog = 0 articles    → abort
```

This pivot preserves the spirit of R024 (scale validation) while adapting to
the constraint (no network, no new catalog sources).