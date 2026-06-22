# R024 Quality Comparison: M117 Baseline (20) vs M118 (53)

**Generated**: 2026-06-22T16:47:17.339287+00:00  
**Corpus**: M117 = 20 articles, M118 = 53 articles  
**Chunks**: M117 = 40, M118 = 112  
**M118 sources**: PDF=32 (pymupdf), HTML=21 (abs.html)  

## Fail-Closed Invariants

| Flag | Value |
|------|-------|
| network_fetch_attempted | false |
| production_import_attempted | false |
| graph_import_allowed | false |
| ladybugdb_written | false |
| trusted_kg_import_allowed | false |
| graph_readiness_claim | false |

## Per-Article Chunk Counts (M118)

| Article | Source Kind | Chunks |
|---------|-------------|--------|
| arxiv/cond-mat-mtrl-sci/2605.20918 | html_native | 2 |
| arxiv/cs-ai/1207.4167 | pdf_converted | 2 |
| arxiv/cs-ai/1612.00341 | pdf_converted | 2 |
| arxiv/cs-ai/1703.07469 | pdf_converted | 2 |
| arxiv/cs-ai/2502.13025 | html_native | 2 |
| arxiv/cs-ai/2510.21148 | html_native | 2 |
| arxiv/cs-ai/2512.24601 | html_native | 2 |
| arxiv/cs-ai/2605.28617v1 | html_native | 2 |
| arxiv/cs-cl/1206.6423 | pdf_converted | 2 |
| arxiv/cs-cl/1409.0473 | pdf_converted | 2 |
| arxiv/cs-cl/1508.07909 | pdf_converted | 3 |
| arxiv/cs-cl/1602.02410 | pdf_converted | 2 |
| arxiv/cs-cl/1606.02447 | pdf_converted | 2 |
| arxiv/cs-cl/1611.04230 | pdf_converted | 2 |
| arxiv/cs-cl/1702.01806 | pdf_converted | 2 |
| arxiv/cs-cl/2108.12409 | html_native | 2 |
| arxiv/cs-cl/2109.10862 | html_native | 2 |
| arxiv/cs-cl/2507.19457 | html_native | 2 |
| arxiv/cs-cl/2511.20639 | html_native | 2 |
| arxiv/cs-cl/2605.18211 | html_native | 2 |
| arxiv/cs-cv/1504.00325 | pdf_converted | 2 |
| arxiv/cs-cv/1804.02767 | html_native | 2 |
| arxiv/cs-cv/2605.26525v1 | html_native | 2 |
| arxiv/cs-gr/1703.00050 | pdf_converted | 2 |
| arxiv/cs-lg/1412.6980 | pdf_converted | 2 |
| arxiv/cs-lg/1506.02075 | pdf_converted | 2 |
| arxiv/cs-lg/1506.02438 | pdf_converted | 2 |
| arxiv/cs-lg/1511.08228 | pdf_converted | 2 |
| arxiv/cs-lg/1606.01540 | pdf_converted | 2 |
| arxiv/cs-lg/1606.07792 | pdf_converted | 2 |
| arxiv/cs-lg/1606.08415 | pdf_converted | 1 |
| arxiv/cs-lg/1611.00144 | pdf_converted | 2 |
| arxiv/cs-lg/1611.01796 | pdf_converted | 2 |
| arxiv/cs-lg/1611.01989 | pdf_converted | 2 |
| arxiv/cs-lg/1611.02109 | pdf_converted | 8 |
| arxiv/cs-lg/1611.07507 | pdf_converted | 2 |
| arxiv/cs-lg/2111.00396 | html_native | 2 |
| arxiv/cs-lg/2203.14465 | html_native | 2 |
| arxiv/cs-ne/1410.4615 | pdf_converted | 2 |
| arxiv/cs-ne/1410.5401 | pdf_converted | 2 |
| arxiv/cs-ne/1605.06640 | pdf_converted | 2 |
| arxiv/cs-ro/1502.03143 | pdf_converted | 2 |
| arxiv/cs-sd/1609.03499 | pdf_converted | 2 |
| arxiv/cs-se/1505.07002 | pdf_converted | 2 |
| arxiv/math-oc/1203.2295 | pdf_converted | 2 |
| arxiv/mixed-source/2603.04448 | html_native | 2 |
| arxiv/mixed-source/2604.18478 | html_native | 2 |
| arxiv/mixed-source/2605.20897 | html_native | 2 |
| arxiv/mixed-source/2605.21401 | html_native | 2 |
| arxiv/mixed-source/2605.25522 | html_native | 2 |
| arxiv/stat-ml/1503.02531 | pdf_converted | 2 |
| company_blog/cs-ir/pageindex_zhang2025pageindex | html_native | 2 |
| nature/mixed-source/s44387-025-00019-5 | html_native | 2 |

## Summary

- Scale factor: 2.65x baseline (53 vs 20 articles).
- Total chunks: 112 (M118) vs 40 (M117).
- Avg chunks per article: 2.11 (M118) vs 2.0 (M117).
- M118 source mix: 32 PDF-converted (pymupdf) + 21 HTML (abs.html).
- Note: M118 uses same parser+chunking framework as M117 (parse_article + build_page_index_from_parsed).
- Recommendation: extend NetworkX probe at 53-article scale + memory profiling (S04).
