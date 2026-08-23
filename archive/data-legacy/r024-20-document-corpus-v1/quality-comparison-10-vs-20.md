# R024 Quality Comparison: M116 Baseline (10) vs M117 (20)

**Generated**: 2026-06-22T15:19:08.136405+00:00  
**Corpus**: M116 baseline = 10 articles, M117 = 20 articles  
**Chunks**: M116 = 20, M117 = 40  

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
| arxiv/cond-mat-mtrl-sci/2605.20918 | data/article_catalog/article_catalog/arxiv/cond-mat-mtrl-sci/2605.20918/source/abs.html | 2 |
| arxiv/cs-ai/2502.13025 | data/article_catalog/article_catalog/arxiv/cs-ai/2502.13025/source/abs.html | 2 |
| arxiv/cs-ai/2510.21148 | data/article_catalog/article_catalog/arxiv/cs-ai/2510.21148/source/abs.html | 2 |
| arxiv/cs-ai/2512.24601 | data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/abs.html | 2 |
| arxiv/cs-ai/2605.28617v1 | data/article_catalog/article_catalog/arxiv/cs-ai/2605.28617v1/source/article.html | 2 |
| arxiv/cs-cl/2108.12409 | data/article_catalog/article_catalog/arxiv/cs-cl/2108.12409/source/abs.html | 2 |
| arxiv/cs-cl/2109.10862 | data/article_catalog/article_catalog/arxiv/cs-cl/2109.10862/source/abs.html | 2 |
| arxiv/cs-cl/2507.19457 | data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/abs.html | 2 |
| arxiv/cs-cl/2511.20639 | data/article_catalog/article_catalog/arxiv/cs-cl/2511.20639/source/abs.html | 2 |
| arxiv/cs-cl/2605.18211 | data/article_catalog/article_catalog/arxiv/cs-cl/2605.18211/source/abs.html | 2 |
| arxiv/cs-cv/1804.02767 | data/article_catalog/article_catalog/arxiv/cs-cv/1804.02767/source/abs.html | 2 |
| arxiv/cs-cv/2605.26525v1 | data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/article.html | 2 |
| arxiv/cs-lg/2111.00396 | data/article_catalog/article_catalog/arxiv/cs-lg/2111.00396/source/abs.html | 2 |
| arxiv/cs-lg/2203.14465 | data/article_catalog/article_catalog/arxiv/cs-lg/2203.14465/source/abs.html | 2 |
| arxiv/mixed-source/2603.04448 | data/article_catalog/article_catalog/arxiv/mixed-source/2603.04448/source/abs.html | 2 |
| arxiv/mixed-source/2604.18478 | data/article_catalog/article_catalog/arxiv/mixed-source/2604.18478/source/abs.html | 2 |
| arxiv/mixed-source/2605.20897 | data/article_catalog/article_catalog/arxiv/mixed-source/2605.20897/source/abs.html | 2 |
| arxiv/mixed-source/2605.21401 | data/article_catalog/article_catalog/arxiv/mixed-source/2605.21401/source/abs.html | 2 |
| arxiv/mixed-source/2605.25522 | data/article_catalog/article_catalog/arxiv/mixed-source/2605.25522/source/abs.html | 2 |
| company_blog/cs-ir/pageindex_zhang2025pageindex | data/article_catalog/article_catalog/company_blog/cs-ir/pageindex_zhang2025pageindex/source/article.html | 2 |

## Summary

- Scale factor: 2.0x baseline (20 vs 10 articles).
- Total chunks: 40 (M117) vs 20 (M116).
- Avg chunks per article: 2.0 (M117) vs 2.0 (M116).
- Note: M117 uses same parser+chunking framework as M116 (parse_article + build_page_index_from_parsed).
- Note: All 20 articles parsed+chunked successfully (0 errors).
- Recommendation: extend NetworkX probe to test if 20-article graph remains manageable (S04).
