# R024 Entity Quality Comparison: M118 (5 types) vs M119 (10 types)

**Generated**: 2026-06-23T03:52:51.773626+00:00  
**Corpus**: 53 articles (M118 baseline + M119 same articles)  
**Entity types**: M118 = 5, M119 = 10 (2x)  
**Total entities**: M118 = 265, M119 = 530 (2x)  

## Fail-Closed Invariants

| Flag | Value |
|------|-------|
| network_fetch_attempted | false |
| production_import_attempted | false |
| graph_import_allowed | false |
| ladybugdb_written | false |
| trusted_kg_import_allowed | false |
| graph_readiness_claim | false |
| real_llm_extraction_used | false |
| synthetic_only | true |

## M119 Entity Types Distribution (530 entities, 10 types)

| Entity Type | Count | Source |
|-------------|-------|--------|
| abstract | 53 | article_metadata |
| authors | 53 | article_metadata |
| citation_context | 53 | m025_chunk_types |
| figure_caption_context | 53 | m025_chunk_types |
| keywords | 53 | article_metadata |
| metadata | 53 | m025_chunk_types |
| references | 53 | synthetic_from_citation_context |
| retrieval_context | 53 | m025_chunk_types |
| table_context | 53 | m025_chunk_types |
| title | 53 | article_metadata |

## Summary

- Scale factor: 2.0x entities (vs M118).
- All 53 articles have full 10 entity types.
- Note: 5 new types (title, authors, abstract, keywords, references) added beyond M118 baseline.
- Note: synthetic_only=true; no real LLM-based extraction used.
- Recommendation: extend NetworkX probe at entity-scale (S04).
