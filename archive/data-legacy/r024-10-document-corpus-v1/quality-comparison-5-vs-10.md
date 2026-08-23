# R024 Quality Comparison: M025 Baseline (5) vs R024 (10)

**Generated**: 2026-06-22T11:41:35.063795+00:00  
**Corpus**: M025 baseline = 5 articles, R024 = 10 articles  
**Chunks**: M025 = 25, R024 = 20  

## Fail-Closed Invariants

| Flag | Value |
|------|-------|
| network_fetch_attempted | false |
| production_import_attempted | false |
| graph_import_allowed | false |
| ladybugdb_written | false |
| trusted_kg_import_allowed | false |
| graph_readiness_claim | false |

## Per-Article Chunk Counts

| Article | Source | Chunks |
|---------|--------|--------|
| arxiv/cs-ai/2512.24601 (M025) | article_catalog | 5 |
| arxiv/cs-ai/2605.28617v1 (M025) | article_catalog | 5 |
| arxiv/cs-cl/2507.19457 (M025) | article_catalog | 5 |
| arxiv/cs-cv/2605.26525v1 (M025) | article_catalog | 5 |
| company_blog/cs-ir/pageindex_zhang2025pageindex (M025) | article_catalog | 5 |
| arxiv/cond-mat-mtrl-sci/2605.20918 (R024) | data/article_catalog/article_catalog/arxiv/cond-mat-mtrl-sci/2605.20918/source/abs.html | 2 |
| arxiv/cs-ai/2502.13025 (R024) | data/article_catalog/article_catalog/arxiv/cs-ai/2502.13025/source/abs.html | 2 |
| arxiv/cs-ai/2510.21148 (R024) | data/article_catalog/article_catalog/arxiv/cs-ai/2510.21148/source/abs.html | 2 |
| arxiv/cs-ai/2512.24601 (R024) | data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/abs.html | 2 |
| arxiv/cs-ai/2605.28617v1 (R024) | data/article_catalog/article_catalog/arxiv/cs-ai/2605.28617v1/source/article.html | 2 |
| arxiv/cs-cl/2108.12409 (R024) | data/article_catalog/article_catalog/arxiv/cs-cl/2108.12409/source/abs.html | 2 |
| arxiv/cs-cl/2109.10862 (R024) | data/article_catalog/article_catalog/arxiv/cs-cl/2109.10862/source/abs.html | 2 |
| arxiv/cs-cl/2507.19457 (R024) | data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/abs.html | 2 |
| arxiv/cs-cv/2605.26525v1 (R024) | data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/article.html | 2 |
| company_blog/cs-ir/pageindex_zhang2025pageindex (R024) | data/article_catalog/article_catalog/company_blog/cs-ir/pageindex_zhang2025pageindex/source/article.html | 2 |

## Summary

- Scale factor: 10x baseline (10 vs 5 articles).
- Total chunks: 20 (R024) vs 25 (M025).
- Avg chunks per article: 2.0 (R024) vs 5.0 (M025).
- Note: R024 uses parse_article + build_page_index_from_parsed (M025 S04 framework).
- Note: M025 S07 chunking produced more granular chunks (5 vs 2 per article).
- Recommendation: investigate chunk-count discrepancy at S04 (NetworkX probe).
